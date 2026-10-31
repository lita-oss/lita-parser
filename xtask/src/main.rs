use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use toml::Spanned;

const USAGE: &str = "Usage: cargo x set-version <VERSION|vVERSION>";

#[derive(Deserialize)]
struct WorkspaceManifest {
    workspace: Workspace,
}

#[derive(Deserialize)]
struct Workspace {
    dependencies: Dependencies,
}

#[derive(Deserialize)]
struct Dependencies {
    #[serde(rename = "lita-parser")]
    parser: ParserDependency,
}

#[derive(Deserialize)]
struct ParserDependency {
    version: Spanned<String>,
}

#[derive(Deserialize)]
struct PackageManifest {
    package: Package,
}

#[derive(Deserialize)]
struct Package {
    name: String,
    version: Spanned<String>,
}

struct ManifestUpdate {
    path: PathBuf,
    original: String,
    updated: String,
}

impl ManifestUpdate {
    fn new(path: PathBuf, original: String, current: Spanned<String>, version: &str) -> Self {
        let mut updated = original.clone();
        if current.get_ref() != version {
            updated.replace_range(current.span(), &format!("\"{version}\""));
        }
        Self {
            path,
            original,
            updated,
        }
    }
}

fn release_version(input: &str) -> Result<String> {
    let version = Version::parse(input.strip_prefix('v').unwrap_or(input))
        .with_context(|| format!("invalid release version {input:?}; expected e.g. v0.2.0"))?;
    if !version.build.is_empty() {
        bail!("release versions must not contain build metadata");
    }
    Ok(version.to_string())
}

fn plan_updates(root: &Path, version: &str) -> Result<Vec<ManifestUpdate>> {
    let path = root.join("Cargo.toml");
    let original =
        fs::read_to_string(&path).with_context(|| format!("could not read {}", path.display()))?;
    let workspace: WorkspaceManifest =
        toml::from_str(&original).with_context(|| format!("could not parse {}", path.display()))?;
    let mut updates = vec![ManifestUpdate::new(
        path,
        original,
        workspace.workspace.dependencies.parser.version,
        version,
    )];

    for name in ["lita-parser", "lita-parser-cli"] {
        let path = root.join("crates").join(name).join("Cargo.toml");
        let original = fs::read_to_string(&path)
            .with_context(|| format!("could not read {}", path.display()))?;
        let manifest: PackageManifest = toml::from_str(&original)
            .with_context(|| format!("could not parse {}", path.display()))?;
        if manifest.package.name != name {
            bail!("expected package {name:?} in {}", path.display());
        }
        updates.push(ManifestUpdate::new(
            path,
            original,
            manifest.package.version,
            version,
        ));
    }
    Ok(updates)
}

fn set_version(root: &Path, version: &str) -> Result<()> {
    let updates = plan_updates(root, version)?;
    let lock_path = root.join("Cargo.lock");
    let original_lock = match fs::read(&lock_path) {
        Ok(contents) => Some(contents),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error).context("could not read Cargo.lock"),
    };

    let result = (|| -> Result<()> {
        for update in &updates {
            if update.updated != update.original {
                fs::write(&update.path, &update.updated)
                    .with_context(|| format!("could not write {}", update.path.display()))?;
            }
        }

        let status = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
            .args(["update", "--workspace"])
            .current_dir(root)
            .status()
            .context("could not run cargo update --workspace")?;
        if !status.success() {
            bail!("cargo update --workspace failed with {status}");
        }
        Ok(())
    })();

    if let Err(error) = result {
        for update in &updates {
            fs::write(&update.path, &update.original).with_context(|| {
                format!("{error:#}; could not restore {}", update.path.display())
            })?;
        }
        match original_lock {
            Some(contents) => fs::write(&lock_path, contents)
                .with_context(|| format!("{error:#}; could not restore Cargo.lock"))?,
            None if lock_path.exists() => fs::remove_file(&lock_path)
                .with_context(|| format!("{error:#}; could not remove generated Cargo.lock"))?,
            None => {}
        }
        return Err(error);
    }

    println!("Set lita-parser and lita-parser-cli to {version}; refreshed Cargo.lock.");
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = env::args().skip(1).collect();
    match args.as_slice() {
        [help] if help == "--help" || help == "-h" => println!("{USAGE}"),
        [task, input] if task == "set-version" => {
            let version = release_version(input)?;
            let root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .context("xtask must be inside the workspace root")?;
            set_version(root, &version)?;
        }
        _ => bail!("{USAGE}"),
    }
    Ok(())
}
