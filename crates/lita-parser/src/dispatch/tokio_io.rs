#[cfg(feature = "csv-tokio-io")]
use crate::csv::tokio_io::{Csv, CsvParser, Error as CsvInitError};
use crate::decode::{Encoding, StreamDecoder, StreamEncoding, UnknownEncoding, Utf8};
#[cfg(feature = "json-tokio-io")]
use crate::json::tokio_io::{Error as JsonInitError, Json, JsonParser};
#[cfg(feature = "txt-tokio-io")]
use crate::txt::tokio_io::{Error as TxtInitError, Txt, TxtParser};
use crate::{IntoParser, ParsedDoc, Parser};
#[cfg(feature = "txt-tokio-io")]
use crate::{Metadata, Title};

#[cfg(feature = "csv-tokio-io")]
use csv_async::Error as CsvError;

#[cfg(feature = "json-tokio-io")]
use serde_json::Error as JsonError;

use tokio::io::AsyncRead;

use std::error::Error;
use std::fmt;
#[cfg(any(feature = "txt-tokio-io", feature = "json-tokio-io"))]
use std::{io::Error as IoError, marker::PhantomData};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    #[cfg(feature = "txt-tokio-io")]
    Txt {
        title: Title,
        metadata: Option<Metadata>,
    },
    #[cfg(feature = "json-tokio-io")]
    Json,
    #[cfg(feature = "csv-tokio-io")]
    Csv,
}

pub struct Dispatch<R, E = Utf8> {
    bytes: R,
    encoding: E,
    format: Format,
}

impl<R: AsyncRead, E: Encoding> Dispatch<R, E> {
    pub fn new(bytes: R, format: Format, encoding: E) -> Self {
        Self {
            bytes,
            encoding,
            format,
        }
    }
}

pub struct DispatchParser<R> {
    imp: DispatchParserImpl<R>,
}

enum DispatchParserImpl<R> {
    #[cfg(feature = "txt-tokio-io")]
    Txt(TxtParser, PhantomData<R>),
    #[cfg(feature = "json-tokio-io")]
    Json(JsonParser, PhantomData<R>),
    #[cfg(feature = "csv-tokio-io")]
    Csv(CsvParser<R>),
}

impl<R: AsyncRead + Unpin + Send, E: StreamEncoding> IntoParser for Dispatch<R, E>
where
    StreamDecoder<R, E::MakeStreamDecoder>: Send,
{
    type Error = IntoParserError;
    type Parser = DispatchParser<StreamDecoder<R, E::MakeStreamDecoder>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let Self {
            bytes,
            encoding,
            format,
        } = self;

        let imp = match format {
            #[cfg(feature = "txt-tokio-io")]
            Format::Txt { title, metadata } => {
                let parser = Txt::new(bytes, title, metadata, encoding).parser().await?;
                DispatchParserImpl::Txt(parser, PhantomData)
            }
            #[cfg(feature = "json-tokio-io")]
            Format::Json => {
                let parser = Json::new(bytes, encoding).parser().await?;
                DispatchParserImpl::Json(parser, PhantomData)
            }
            #[cfg(feature = "csv-tokio-io")]
            Format::Csv => {
                let parser = Csv::new(bytes, encoding).parser().await?;
                DispatchParserImpl::Csv(parser)
            }
        };
        Ok(DispatchParser { imp })
    }
}

impl<R: AsyncRead + Unpin + Send> Parser for DispatchParser<R> {
    type Error = ParserError;

    fn size_hint(&self) -> Option<usize> {
        match &self.imp {
            #[cfg(feature = "txt-tokio-io")]
            DispatchParserImpl::Txt(parser, _) => parser.size_hint(),
            #[cfg(feature = "json-tokio-io")]
            DispatchParserImpl::Json(parser, _) => parser.size_hint(),
            #[cfg(feature = "csv-tokio-io")]
            DispatchParserImpl::Csv(parser) => parser.size_hint(),
        }
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        match &mut self.imp {
            #[cfg(feature = "txt-tokio-io")]
            DispatchParserImpl::Txt(parser, _) => {
                parser.next().await.map(|res| res.map_err(|e| match e {}))
            }
            #[cfg(feature = "json-tokio-io")]
            DispatchParserImpl::Json(parser, _) => {
                parser.next().await.map(|res| res.map_err(|e| match e {}))
            }
            #[cfg(feature = "csv-tokio-io")]
            DispatchParserImpl::Csv(parser) => {
                parser.next().await.map(|res| res.map_err(ParserError::Csv))
            }
        }
    }
}

#[derive(Debug)]
pub enum IntoParserError {
    #[cfg(feature = "csv-tokio-io")]
    Csv(CsvError),
    #[cfg(any(feature = "txt-tokio-io", feature = "json-tokio-io"))]
    Io(IoError),
    #[cfg(feature = "json-tokio-io")]
    Json(JsonError),
    UnknownEncoding(UnknownEncoding),
}

#[derive(Debug)]
pub enum ParserError {
    #[cfg(feature = "csv-tokio-io")]
    Csv(CsvError),
}

#[cfg(feature = "csv-tokio-io")]
impl From<CsvInitError> for IntoParserError {
    fn from(e: CsvInitError) -> Self {
        match e {
            CsvInitError::Csv(e) => Self::Csv(e),
            #[cfg(feature = "decode")]
            CsvInitError::UnknownEncoding(e) => Self::UnknownEncoding(e),
        }
    }
}

#[cfg(feature = "txt-tokio-io")]
impl From<TxtInitError> for IntoParserError {
    fn from(e: TxtInitError) -> Self {
        match e {
            TxtInitError::Io(e) => Self::Io(e),
            TxtInitError::UnknownEncoding(e) => Self::UnknownEncoding(e),
        }
    }
}

#[cfg(feature = "json-tokio-io")]
impl From<JsonInitError> for IntoParserError {
    fn from(e: JsonInitError) -> Self {
        match e {
            JsonInitError::Io(e) => Self::Io(e),
            JsonInitError::Json(e) => Self::Json(e),
            JsonInitError::UnknownEncoding(e) => Self::UnknownEncoding(e),
        }
    }
}

impl fmt::Display for IntoParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "csv-tokio-io")]
            Self::Csv(e) => write!(f, "could not initialize CSV parser: {e}"),
            #[cfg(any(feature = "txt-tokio-io", feature = "json-tokio-io"))]
            Self::Io(e) => write!(f, "I/O error when initializing parser: {e}"),
            #[cfg(feature = "json-tokio-io")]
            Self::Json(e) => write!(f, "could not parse JSON: {e}"),
            Self::UnknownEncoding(e) => e.fmt(f),
        }
    }
}

impl Error for IntoParserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            #[cfg(feature = "csv-tokio-io")]
            Self::Csv(e) => Some(e),
            #[cfg(any(feature = "txt-tokio-io", feature = "json-tokio-io"))]
            Self::Io(e) => Some(e),
            #[cfg(feature = "json-tokio-io")]
            Self::Json(e) => Some(e),
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(feature = "csv-tokio-io")]
            Self::Csv(ref e) => write!(f, "could not parse CSV: {e}"),
        }
    }
}

impl Error for ParserError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            #[cfg(feature = "csv-tokio-io")]
            Self::Csv(ref e) => Some(e),
        }
    }
}

#[cfg(feature = "fs")]
pub use fs::FromPathError;

#[cfg(feature = "fs")]
mod fs {
    #[cfg(feature = "txt-tokio-io")]
    use crate::Title;
    use crate::decode::Encoding;

    use super::{Dispatch, Format};

    use tokio::fs::File;

    use std::{error::Error, fmt, io::Error as IoError, path::Path};

    impl<E: Encoding> Dispatch<File, E> {
        pub async fn from_path<P: AsRef<Path>>(
            path: P,
            encoding: E,
        ) -> Result<Self, FromPathError> {
            use std::ffi::OsStr;

            let path = path.as_ref();
            match path.extension().and_then(OsStr::to_str) {
                #[cfg(feature = "json-tokio-io")]
                Some(ext) if ext.eq_ignore_ascii_case("json") => {
                    let bytes = File::open(path).await?;
                    Ok(Self::new(bytes, Format::Json, encoding))
                }
                #[cfg(feature = "csv-tokio-io")]
                Some(ext) if ext.eq_ignore_ascii_case("csv") => {
                    let bytes = File::open(path).await?;
                    Ok(Self::new(bytes, Format::Csv, encoding))
                }
                #[cfg(feature = "txt-tokio-io")]
                _ => {
                    let bytes = File::open(path).await?;
                    let format = Format::Txt {
                        title: Title(path.to_string_lossy().into_owned()),
                        metadata: None,
                    };
                    Ok(Self::new(bytes, format, encoding))
                }
                #[cfg(not(feature = "txt-tokio-io"))]
                _ => Err(FromPathError::UnknownFormat),
            }
        }
    }

    #[derive(Debug)]
    pub enum FromPathError {
        Io(IoError),
        #[cfg(not(feature = "txt-tokio-io"))]
        UnknownFormat,
    }

    impl From<IoError> for FromPathError {
        fn from(e: IoError) -> Self {
            Self::Io(e)
        }
    }

    impl fmt::Display for FromPathError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Io(e) => write!(f, "I/O error when parsing file: {e}"),
                #[cfg(not(feature = "txt-tokio-io"))]
                Self::UnknownFormat => write!(f, "unknown file format"),
            }
        }
    }

    impl Error for FromPathError {
        fn source(&self) -> Option<&(dyn Error + 'static)> {
            match self {
                Self::Io(e) => Some(e),
                #[cfg(not(feature = "txt-tokio-io"))]
                Self::UnknownFormat => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "fs")]
    use tempfile::NamedTempFile;

    #[cfg(any(feature = "txt-tokio-io", feature = "json-tokio-io", feature = "fs"))]
    use std::io::ErrorKind;

    #[cfg(feature = "txt-tokio-io")]
    #[tokio::test]
    async fn dispatches_txt_and_forwards_size_hint() {
        let source = Dispatch::new(
            "春はあけぼの。".as_bytes(),
            Format::Txt {
                title: Title("title".to_owned()),
                metadata: None,
            },
            Utf8,
        );
        let mut parser = source.parser().await.unwrap();
        assert!(matches!(parser.imp, DispatchParserImpl::Txt(_, _)));
        assert_eq!(parser.size_hint(), Some(1));
        let doc = parser.next().await.unwrap().unwrap();
        assert_eq!(doc.title.as_str(), "title");
        assert_eq!(doc.body.as_str(), "春はあけぼの。");
        assert_eq!(parser.size_hint(), Some(0));
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
    }

    #[cfg(feature = "txt-tokio-io")]
    #[tokio::test]
    async fn reports_txt_initialization_io_error() {
        let source = Dispatch::new(
            &b"\xff"[..],
            Format::Txt {
                title: Title("title".to_owned()),
                metadata: None,
            },
            Utf8,
        );
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Io(e)) if e.kind() == ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "json-tokio-io")]
    #[tokio::test]
    async fn dispatches_json_and_forwards_size_hint() {
        let source = Dispatch::new(
            br#"[{"title":"one","body":"first"},{"title":"two","body":"second"}]"#.as_slice(),
            Format::Json,
            Utf8,
        );
        let mut parser = source.parser().await.unwrap();
        assert!(matches!(parser.imp, DispatchParserImpl::Json(_, _)));
        assert_eq!(parser.size_hint(), Some(2));
        assert_eq!(parser.next().await.unwrap().unwrap().title.as_str(), "one");
        assert_eq!(parser.size_hint(), Some(1));
        assert_eq!(parser.next().await.unwrap().unwrap().title.as_str(), "two");
        assert_eq!(parser.size_hint(), Some(0));
        assert!(parser.next().await.is_none());
    }

    #[cfg(feature = "json-tokio-io")]
    #[tokio::test]
    async fn reports_json_initialization_errors() {
        let source = Dispatch::new(b"[".as_slice(), Format::Json, Utf8);
        let error = match source.parser().await {
            Err(error @ IntoParserError::Json(_)) => error,
            _ => panic!("expected a JSON deserialization error"),
        };
        assert!(error.source().is_some());
        assert!(error.to_string().contains("JSON"));

        let source = Dispatch::new(b"[\xff]".as_slice(), Format::Json, Utf8);
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Io(e)) if e.kind() == ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "csv-tokio-io")]
    #[tokio::test]
    async fn dispatches_csv_and_reports_record_errors() {
        let source = Dispatch::new(
            b"title,body\none,first\ntwo\n".as_slice(),
            Format::Csv,
            Utf8,
        );
        let mut parser = source.parser().await.unwrap();
        assert!(matches!(parser.imp, DispatchParserImpl::Csv(_)));
        assert_eq!(parser.size_hint(), None);
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let error = parser.next().await.unwrap().unwrap_err();
        assert!(matches!(error, ParserError::Csv(_)));
        assert!(error.source().is_some());
        assert!(error.to_string().contains("CSV"));
    }

    #[cfg(all(feature = "txt-tokio-io", feature = "decode"))]
    #[tokio::test]
    async fn decodes_txt_without_losing_metadata() {
        use crate::decode::ShiftJis;

        use encoding_rs::SHIFT_JIS;

        let (encoded, _, _) = SHIFT_JIS.encode("春はあけぼの。");
        let metadata = Metadata([("author".to_owned(), "清少納言".to_owned())].into());
        let source = Dispatch::new(
            encoded.as_ref(),
            Format::Txt {
                title: Title("title".to_owned()),
                metadata: Some(metadata.clone()),
            },
            ShiftJis,
        );
        let mut parser = source.parser().await.unwrap();
        let doc = parser.next().await.unwrap().unwrap();
        assert_eq!(doc.title.as_str(), "title");
        assert_eq!(doc.body.as_str(), "春はあけぼの。");
        assert_eq!(doc.metadata, Some(metadata));
    }

    #[cfg(all(feature = "txt-tokio-io", feature = "decode"))]
    #[tokio::test]
    async fn reports_txt_charset_errors() {
        use crate::decode::Dynamic;

        let source = Dispatch::new(
            &b"\xff"[..],
            Format::Txt {
                title: Title("title".to_owned()),
                metadata: None,
            },
            Dynamic,
        );
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Io(e)) if e.kind() == ErrorKind::InvalidData
        ));
    }

    #[cfg(all(feature = "json-tokio-io", feature = "decode"))]
    #[tokio::test]
    async fn dispatches_json_without_a_charset() {
        let source = Dispatch::new(
            br#"[{"title":"one","body":"first"}]"#.as_slice(),
            Format::Json,
            Utf8,
        );
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let source = Dispatch::new(b"[".as_slice(), Format::Json, Utf8);
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Json(_))
        ));
    }

    #[cfg(all(feature = "csv-tokio-io", feature = "decode"))]
    #[tokio::test]
    async fn dispatches_csv_without_a_charset() {
        let source = Dispatch::new(
            b"title,body\none,first\ntwo\n".as_slice(),
            Format::Csv,
            Utf8,
        );
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        assert!(matches!(
            parser.next().await,
            Some(Err(ParserError::Csv(_)))
        ));
    }

    #[cfg(any(
        all(feature = "txt-tokio-io", feature = "fs"),
        all(feature = "json-tokio-io", feature = "fs"),
        all(feature = "csv-tokio-io", feature = "fs")
    ))]
    fn file_with_contents(extension: &str, contents: &[u8]) -> NamedTempFile {
        use tempfile::Builder;

        use std::io::Write as _;

        let mut file = Builder::new().suffix(extension).tempfile().unwrap();
        file.write_all(contents).unwrap();
        file
    }

    #[cfg(all(all(feature = "txt-tokio-io", feature = "fs"), feature = "decode"))]
    #[tokio::test]
    async fn decodes_txt_files_with_charset() {
        use crate::decode::ShiftJis;

        use encoding_rs::SHIFT_JIS;

        let (encoded, _, _) = SHIFT_JIS.encode("春はあけぼの。");
        let file = file_with_contents(".unknown", encoded.as_ref());
        let source = Dispatch::from_path(file.path(), ShiftJis).await.unwrap();
        let mut parser = source.parser().await.unwrap();
        let doc = parser.next().await.unwrap().unwrap();
        assert_eq!(doc.body.as_str(), "春はあけぼの。");
        assert_eq!(doc.title.as_str(), file.path().to_string_lossy());
    }

    #[cfg(all(all(feature = "json-tokio-io", feature = "fs"), feature = "decode"))]
    #[tokio::test]
    async fn dispatches_json_files_without_a_charset() {
        use crate::decode::ShiftJis;

        let file = file_with_contents(".JSON", br#"[{"title":"one","body":"first"}]"#);
        let source = Dispatch::from_path(file.path(), Utf8).await.unwrap();
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let file = file_with_contents(".json", b"[");
        let source = Dispatch::from_path(file.path(), ShiftJis).await.unwrap();
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Json(_))
        ));
    }

    #[cfg(all(all(feature = "csv-tokio-io", feature = "fs"), feature = "decode"))]
    #[tokio::test]
    async fn dispatches_csv_files_without_a_charset() {
        use crate::decode::Dynamic;

        let file = file_with_contents(".CSV", b"title,body\none,first\n");
        let source = Dispatch::from_path(file.path(), Utf8).await.unwrap();
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let source = Dispatch::from_path(file.path(), Dynamic).await.unwrap();
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
    }

    #[cfg(all(feature = "txt-tokio-io", feature = "fs"))]
    #[tokio::test]
    async fn defaults_to_txt_for_unknown_or_missing_extensions() {
        for extension in [
            ".txt",
            ".unknown",
            "",
            #[cfg(not(all(feature = "json-tokio-io", feature = "fs")))]
            ".json",
            #[cfg(not(all(feature = "csv-tokio-io", feature = "fs")))]
            ".csv",
        ] {
            let file = file_with_contents(extension, "春はあけぼの。".as_bytes());
            let source = Dispatch::from_path(file.path(), Utf8).await.unwrap();
            let mut parser = source.parser().await.unwrap();
            assert!(matches!(parser.imp, DispatchParserImpl::Txt(_, _)));
            let doc = parser.next().await.unwrap().unwrap();
            assert_eq!(doc.title.as_str(), file.path().to_string_lossy());
            assert_eq!(doc.body.as_str(), "春はあけぼの。");
            assert!(doc.metadata.is_none());
        }
    }

    #[cfg(all(
        not(all(feature = "txt-tokio-io", feature = "fs")),
        any(
            all(feature = "json-tokio-io", feature = "fs"),
            all(feature = "csv-tokio-io", feature = "fs")
        )
    ))]
    #[tokio::test]
    async fn rejects_unknown_formats_without_txt_fs() {
        for extension in [
            ".txt",
            ".unknown",
            "",
            #[cfg(not(all(feature = "json-tokio-io", feature = "fs")))]
            ".json",
            #[cfg(not(all(feature = "csv-tokio-io", feature = "fs")))]
            ".csv",
        ] {
            let file = file_with_contents(extension, b"text");
            assert!(matches!(
                Dispatch::from_path(file.path(), Utf8).await,
                Err(FromPathError::UnknownFormat)
            ));
            #[cfg(feature = "decode")]
            assert!(matches!(
                Dispatch::from_path(file.path(), Utf8).await,
                Err(FromPathError::UnknownFormat)
            ));
            let path = file.path().to_owned();
            file.close().unwrap();
            assert!(matches!(
                Dispatch::from_path(&path, Utf8).await,
                Err(FromPathError::UnknownFormat)
            ));
            #[cfg(feature = "decode")]
            assert!(matches!(
                Dispatch::from_path(path, Utf8).await,
                Err(FromPathError::UnknownFormat)
            ));
        }
        let error = FromPathError::UnknownFormat;
        assert!(error.source().is_none());
        assert_eq!(error.to_string(), "unknown file format");
    }

    #[cfg(all(feature = "json-tokio-io", feature = "fs"))]
    #[tokio::test]
    async fn selects_json_files_and_reports_invalid_contents_when_initializing_parser() {
        for extension in [".json", ".JSON"] {
            let file = file_with_contents(extension, br#"[{"title":"one","body":"first"}]"#);
            let source = Dispatch::from_path(file.path(), Utf8).await.unwrap();
            let mut parser = source.parser().await.unwrap();
            assert!(matches!(parser.imp, DispatchParserImpl::Json(_, _)));
            assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        }
        let file = file_with_contents(".json", b"[");
        let source = Dispatch::from_path(file.path(), Utf8).await.unwrap();
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Json(_))
        ));
    }

    #[cfg(all(feature = "csv-tokio-io", feature = "fs"))]
    #[tokio::test]
    async fn selects_csv_files_and_delays_record_errors_until_iteration() {
        for extension in [".csv", ".CSV"] {
            let file = file_with_contents(extension, b"title,body\none,first\ntwo\n");
            let source = Dispatch::from_path(file.path(), Utf8).await.unwrap();
            let mut parser = source.parser().await.unwrap();
            assert!(matches!(parser.imp, DispatchParserImpl::Csv(_)));
            assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
            assert!(matches!(
                parser.next().await,
                Some(Err(ParserError::Csv(_)))
            ));
        }
    }

    #[cfg(any(
        all(feature = "txt-tokio-io", feature = "fs"),
        all(feature = "json-tokio-io", feature = "fs"),
        all(feature = "csv-tokio-io", feature = "fs")
    ))]
    #[tokio::test]
    async fn preserves_filesystem_io_errors() {
        for extension in [
            #[cfg(all(feature = "txt-tokio-io", feature = "fs"))]
            ".unknown",
            #[cfg(all(feature = "json-tokio-io", feature = "fs"))]
            ".json",
            #[cfg(all(feature = "csv-tokio-io", feature = "fs"))]
            ".csv",
        ] {
            let file = file_with_contents(extension, b"");
            let path = file.path().to_owned();
            file.close().unwrap();
            #[cfg(feature = "decode")]
            assert!(matches!(
                Dispatch::from_path(&path, Utf8).await,
                Err(FromPathError::Io(error)) if error.kind() == ErrorKind::NotFound
            ));
            let error = match Dispatch::from_path(path, Utf8).await {
                Err(error @ FromPathError::Io(_)) => error,
                _ => panic!("expected an I/O error"),
            };
            assert!(error.source().is_some());
            assert!(matches!(error,
                FromPathError::Io(error) if error.kind() == ErrorKind::NotFound));
        }
    }
}
