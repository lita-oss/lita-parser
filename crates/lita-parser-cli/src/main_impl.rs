use lita_parser::decode::Encoding as EncodingTrait;
use lita_parser::dispatch::tokio_io::Dispatch;
use lita_parser::{IntoParser, ParsedDoc, Parser};

pub use anyhow::Error;
use anyhow::{Context as _, bail};

use clap::ValueEnum;

use tokio::fs::{File, metadata};

use std::{
    collections::BTreeMap,
    error::Error as StdError,
    io::Write,
    num::NonZeroUsize,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, clap::Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, help = "Input file or directory to parse recursively")]
    input: PathBuf,

    #[arg(short, long, help = "Output JSONL file")]
    output: PathBuf,

    #[arg(
        short,
        long,
        default_value = "utf-8",
        ignore_case = true,
        help = "Character encoding of the input files"
    )]
    encoding: Encoding,

    #[arg(
        short,
        long,
        default_value = "1",
        help = "Maximum number of concurrent files when parsing a directory"
    )]
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

#[derive(serde::Serialize)]
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

async fn parse<I>(src: I, path: &Path) -> Result<Vec<ParsedDoc>, Error>
where
    I: IntoParser,
    I::Error: StdError + Send + Sync + 'static,
    <I::Parser as Parser>::Error: StdError + Send + Sync + 'static,
{
    let mut parser = src
        .parser()
        .await
        .with_context(|| format!("could not initialize parser for {}", path.display()))?;

    let mut docs = Vec::with_capacity(parser.size_hint().unwrap_or_default());
    while let Some(doc) = parser.next().await {
        docs.push(doc.with_context(|| format!("could not parse {}", path.display()))?);
    }

    Ok(docs)
}

async fn parse_file(path: PathBuf, encoding: Encoding) -> Result<Vec<ParsedDoc>, Error> {
    use lita_parser::decode::Utf8;
    #[cfg(feature = "decode")]
    use lita_parser::decode::{Dynamic, EucJp, ShiftJis, Utf16Be, Utf16Le};

    use tracing::{debug, warn};

    use std::ffi::OsStr;

    debug!(path = %path.display(), "Parsing input file");

    let ext = path.extension().and_then(OsStr::to_str);
    if let Some(ext) = ext {
        if ext.eq_ignore_ascii_case("json") && !cfg!(feature = "json") {
            warn!(path = %path.display(), "JSON support is disabled; parsing as text");
        }
        if ext.eq_ignore_ascii_case("csv") && !cfg!(feature = "csv") {
            warn!(path = %path.display(), "CSV support is disabled; parsing as text");
        }
    }

    match encoding {
        Encoding::Utf8 => parse_path(&path, Utf8).await,
        #[cfg(feature = "decode")]
        Encoding::Utf16Be => parse_path(&path, Utf16Be).await,
        #[cfg(feature = "decode")]
        Encoding::Utf16Le => parse_path(&path, Utf16Le).await,
        #[cfg(feature = "decode")]
        Encoding::ShiftJis => parse_path(&path, ShiftJis).await,
        #[cfg(feature = "decode")]
        Encoding::EucJp => parse_path(&path, EucJp).await,
        #[cfg(feature = "decode")]
        Encoding::Dynamic => parse_path(&path, Dynamic).await,
    }
}

async fn parse_path<E: EncodingTrait>(path: &Path, encoding: E) -> Result<Vec<ParsedDoc>, Error>
where
    Dispatch<File, E>: IntoParser<
            Error: StdError + Send + Sync + 'static,
            Parser: Parser<Error: StdError + Send + Sync + 'static>,
        >,
{
    let src = Dispatch::from_path(path, encoding)
        .await
        .with_context(|| format!("could not open input file {}", path.display()))?;
    parse(src, path).await
}

async fn collect_files(path: PathBuf) -> Result<Vec<PathBuf>, Error> {
    use tokio::fs::{canonicalize, read_dir};

    use std::{collections::HashSet, io::ErrorKind};

    let mut dirs = vec![path];
    let mut visited = HashSet::new();
    let mut files = Vec::new();

    while let Some(dir) = dirs.pop() {
        let canon = canonicalize(&dir)
            .await
            .with_context(|| format!("could not resolve directory {}", dir.display()))?;
        if !visited.insert(canon) {
            continue;
        }

        let mut entries = read_dir(&dir)
            .await
            .with_context(|| format!("could not read directory {}", dir.display()))?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let file_type = entry.file_type().await?;
            let file_type = if file_type.is_symlink() {
                match metadata(&path).await {
                    Ok(metadata) => metadata.file_type(),
                    Err(e) if e.kind() == ErrorKind::NotFound => continue,
                    Err(e) => {
                        return Err(e).with_context(|| {
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
    use serde_json::to_writer;

    for doc in docs {
        to_writer(&mut *output, &ParsedDocRef::from(doc))?;
        output.write_all(b"\n")?;
    }

    Ok(())
}

async fn run(args: Args) -> Result<(), Error> {
    use futures_util::{StreamExt as _, stream::FuturesUnordered};

    use tokio::spawn;

    use std::io::BufWriter;

    let metadata = metadata(&args.input)
        .await
        .with_context(|| format!("could not read metadata for {}", args.input.display()))?;

    let files = if metadata.is_dir() {
        Some(collect_files(args.input.clone()).await?)
    } else if metadata.is_file() {
        None
    } else {
        bail!("input must be a regular file or directory");
    };

    let file = File::create(&args.output)
        .await
        .with_context(|| format!("could not create output file {}", args.output.display()))?;
    let mut output = BufWriter::new(file.into_std().await);

    if let Some(files) = files {
        let mut files = files.into_iter();
        while files.len() > 0 {
            let mut tasks = files
                .by_ref()
                .take(args.jobs.get())
                .map(|path| spawn(parse_file(path, args.encoding)))
                .collect::<FuturesUnordered<_>>();

            while let Some(res) = tasks.next().await {
                write_docs(&mut output, &res??)?;
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
    use clap::Parser as _;

    use tracing_subscriber::{EnvFilter, fmt};

    use std::io::stdout;

    let args = Args::parse();

    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_writer(stdout)
        .init();

    run(args).await
}
