pub use anyhow::Error;
use anyhow::{Context as _, bail};
use clap::{Parser, ValueEnum};
use futures_util::stream::{FuturesUnordered, StreamExt as _};
#[cfg(not(feature = "decode"))]
use lita_parser::dispatch::io::Dispatch;
#[cfg(feature = "decode")]
use lita_parser::dispatch::io::DispatchWithCharset;
use lita_parser::{IntoParser as _, ParsedDoc, Parser as _};
use serde::Serialize;
use tracing_subscriber::EnvFilter;

use std::collections::{BTreeMap, HashSet};
use std::io::{BufWriter, Write};
use std::num::NonZeroUsize;
use std::path::PathBuf;

use tokio::fs;

#[derive(Debug, Clone, Parser)]
#[command(version, about, long_about = None)]
struct Args {
    /// Input file or directory to parse recursively
    #[arg(short, long)]
    input: PathBuf,

    /// Output JSONL file
    #[arg(short, long)]
    output: PathBuf,

    /// Character encoding of the input files
    #[arg(short, long, default_value = "utf-8", ignore_case = true)]
    encoding: Encoding,

    /// Maximum number of concurrent files when parsing a directory
    #[arg(short, long, default_value = "1")]
    jobs: NonZeroUsize,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Encoding {
    #[value(name = "utf-8", aliases = ["utf8", "utf_8"])]
    Utf8,
    #[cfg(feature = "decode")]
    #[value(name = "utf-16be", aliases = ["utf16be", "utf16-be", "utf-16-be", "utf16_be", "utf_16_be"])]
    Utf16Be,
    #[cfg(feature = "decode")]
    #[value(name = "utf-16le", aliases = ["utf16le", "utf16-le", "utf-16-le", "utf16_le", "utf_16_le"])]
    Utf16Le,
    #[cfg(feature = "decode")]
    #[value(name = "shift-jis", aliases = ["shiftjis", "shift_jis"])]
    ShiftJis,
    #[cfg(feature = "decode")]
    #[value(name = "euc-jp", aliases = ["eucjp", "euc_jp"])]
    EucJp,
    #[cfg(feature = "decode")]
    Dynamic,
}

#[cfg(feature = "decode")]
impl From<Encoding> for lita_parser::Charset {
    fn from(value: Encoding) -> Self {
        match value {
            Encoding::Utf8 => Self::Utf8,
            Encoding::Utf16Be => Self::Utf16Be,
            Encoding::Utf16Le => Self::Utf16Le,
            Encoding::ShiftJis => Self::ShiftJis,
            Encoding::EucJp => Self::EucJp,
            Encoding::Dynamic => Self::Dynamic,
        }
    }
}

#[derive(Serialize)]
struct ParsedDocRef<'a> {
    title: &'a str,
    body: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<&'a BTreeMap<String, String>>,
}

impl<'a> From<&'a ParsedDoc> for ParsedDocRef<'a> {
    fn from(value: &'a ParsedDoc) -> Self {
        Self {
            title: value.title.as_str(),
            body: value.body.as_str(),
            metadata: value.metadata.as_ref().map(|metadata| metadata.as_map()),
        }
    }
}

async fn parse_file(path: PathBuf, _encoding: Encoding) -> Result<Vec<ParsedDoc>, Error> {
    tracing::debug!(path = %path.display(), "Parsing input file");

    if let Some(ext) = path.extension().and_then(|extension| extension.to_str()) {
        if ext.eq_ignore_ascii_case("json") && !cfg!(feature = "json") {
            tracing::warn!(path = %path.display(), "JSON support is disabled; parsing as text");
        }
        if ext.eq_ignore_ascii_case("csv") && !cfg!(feature = "csv") {
            tracing::warn!(path = %path.display(), "CSV support is disabled; parsing as text");
        }
    }

    #[cfg(not(feature = "decode"))]
    let parser = Dispatch::from_path(&path);
    #[cfg(feature = "decode")]
    let parser = DispatchWithCharset::from_path(&path, _encoding.into());

    let parser = parser
        .await
        .with_context(|| format!("could not open input file {}", path.display()))?;

    let mut parser = parser
        .parser()
        .await
        .with_context(|| format!("could not initialize parser for {}", path.display()))?;

    let mut docs = Vec::new();
    while let Some(doc) = parser.next().await {
        docs.push(doc.with_context(|| format!("could not parse {}", path.display()))?);
    }

    Ok(docs)
}

async fn collect_files(path: PathBuf) -> Result<Vec<PathBuf>, Error> {
    let mut dirs = vec![path];
    let mut visited = HashSet::new();
    let mut files = Vec::new();

    while let Some(dir) = dirs.pop() {
        let canon = fs::canonicalize(&dir)
            .await
            .with_context(|| format!("could not resolve directory {}", dir.display()))?;
        if !visited.insert(canon) {
            continue;
        }

        let mut entries = fs::read_dir(&dir)
            .await
            .with_context(|| format!("could not read directory {}", dir.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            let file_type = if file_type.is_symlink() {
                match fs::metadata(&path).await {
                    Ok(metadata) => metadata.file_type(),
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!("could not read metadata for {}", path.display())
                        });
                    }
                }
            } else {
                file_type
            };

            if file_type.is_dir() {
                dirs.push(path);
            } else if file_type.is_file() {
                files.push(path);
            }
        }
    }

    Ok(files)
}

fn write_docs(output: &mut impl Write, docs: &[ParsedDoc]) -> Result<(), Error> {
    for doc in docs {
        writeln!(
            output,
            "{}",
            serde_json::to_string(&ParsedDocRef::from(doc))?
        )?;
    }

    Ok(())
}

async fn run(args: Args) -> Result<(), Error> {
    let metadata = fs::metadata(&args.input)
        .await
        .with_context(|| format!("could not read metadata for {}", args.input.display()))?;

    let files = if metadata.is_dir() {
        Some(collect_files(args.input.clone()).await?)
    } else if metadata.is_file() {
        None
    } else {
        bail!("input must be a regular file or directory");
    };

    let file = fs::File::create(&args.output)
        .await
        .with_context(|| format!("could not create output file {}", args.output.display()))?;
    let mut output = BufWriter::new(file.into_std().await);

    if let Some(files) = files {
        for paths in files.chunks(args.jobs.get()) {
            let mut tasks = paths
                .iter()
                .map(|path| tokio::spawn(parse_file(path.clone(), args.encoding)))
                .collect::<FuturesUnordered<_>>();

            let mut batch = Vec::new();
            while let Some(res) = tasks.next().await {
                batch.push(res??);
            }

            for docs in batch {
                write_docs(&mut output, &docs)?;
            }
        }
    } else {
        let docs = parse_file(args.input, args.encoding).await?;
        write_docs(&mut output, &docs)?;
    }

    output
        .flush()
        .with_context(|| format!("could not flush output file {}", args.output.display()))?;

    Ok(())
}

pub async fn main_impl() -> Result<(), Error> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(std::io::stdout)
        .init();

    run(args).await
}
