use crate::{IntoParser, ParsedDoc, Parser};

use tokio::io::AsyncRead;

use std::error::Error as StdError;
use std::fmt;

#[cfg(all(
    feature = "decode",
    any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs")
))]
use crate::Charset;
#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
use std::{ffi::OsStr, path::Path};
#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
use tokio::fs::File;

#[cfg(feature = "csv-io")]
use crate::csv::io::{Csv, CsvParser};
#[cfg(feature = "json-io")]
use crate::json::{
    JsonParser,
    io::{Error as JsonError, Json},
};
#[cfg(feature = "txt-io")]
use crate::txt::{TxtParser, io::Txt};
#[cfg(feature = "csv-io")]
use csv_async::Error as CsvError;

#[cfg(any(feature = "txt-io", feature = "json-fs", feature = "csv-fs"))]
use std::io::Error as IoError;

#[cfg(all(feature = "decode", feature = "txt-io"))]
use crate::txt::io::TxtWithCharset;

pub enum Dispatch<R> {
    #[cfg(feature = "txt-io")]
    Txt(Txt<R>),
    #[cfg(feature = "json-io")]
    Json(Json<R>),
    #[cfg(feature = "csv-io")]
    Csv(Csv<R>),
}

pub enum DispatchParser<R> {
    #[cfg(feature = "txt-io")]
    Txt(TxtParser),
    #[cfg(feature = "json-io")]
    Json(JsonParser),
    #[cfg(feature = "csv-io")]
    Csv(CsvParser<R>),
    #[cfg(not(feature = "csv-io"))]
    __Marker(std::convert::Infallible, std::marker::PhantomData<R>),
}

#[derive(Debug)]
pub enum IntoParserError {
    #[cfg(feature = "txt-io")]
    Txt(IoError),
    #[cfg(feature = "json-io")]
    Json(JsonError),
}

#[derive(Debug)]
pub enum ParserError {
    #[cfg(feature = "csv-io")]
    Csv(CsvError),
}

#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
#[derive(Debug)]
pub enum FromPathError {
    Io(IoError),
    #[cfg(not(feature = "txt-fs"))]
    UnknownFormat,
}

impl<R: AsyncRead + Unpin + Send> IntoParser for Dispatch<R> {
    type Error = IntoParserError;
    type Parser = DispatchParser<R>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(parser) => parser
                .parser()
                .await
                .map(DispatchParser::Txt)
                .map_err(IntoParserError::Txt),
            #[cfg(feature = "json-io")]
            Self::Json(parser) => parser
                .parser()
                .await
                .map(DispatchParser::Json)
                .map_err(IntoParserError::Json),
            #[cfg(feature = "csv-io")]
            Self::Csv(parser) => {
                let Ok(parser) = parser.parser().await;
                Ok(DispatchParser::Csv(parser))
            }
        }
    }
}

impl<R: AsyncRead + Unpin + Send> Parser for DispatchParser<R> {
    type Error = ParserError;

    fn size_hint(&self) -> Option<usize> {
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(parser) => parser.size_hint(),
            #[cfg(feature = "json-io")]
            Self::Json(parser) => parser.size_hint(),
            #[cfg(feature = "csv-io")]
            Self::Csv(parser) => parser.size_hint(),
            #[cfg(not(feature = "csv-io"))]
            Self::__Marker(never, _) => match *never {},
        }
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(parser) => parser.next().await.map(|res| res.map_err(|e| match e {})),
            #[cfg(feature = "json-io")]
            Self::Json(parser) => parser.next().await.map(|res| res.map_err(|e| match e {})),
            #[cfg(feature = "csv-io")]
            Self::Csv(parser) => parser.next().await.map(|res| res.map_err(ParserError::Csv)),
            #[cfg(not(feature = "csv-io"))]
            Self::__Marker(never, _) => match *never {},
        }
    }
}

#[cfg(feature = "decode")]
pub enum DispatchWithCharset<R> {
    #[cfg(feature = "txt-io")]
    Txt(TxtWithCharset<R>),
    #[cfg(feature = "json-io")]
    Json(Json<R>),
    #[cfg(feature = "csv-io")]
    Csv(Csv<R>),
}

#[cfg(feature = "decode")]
#[derive(Debug)]
pub enum IntoParserErrorWithCharset {
    #[cfg(feature = "txt-io")]
    Txt(IoError),
    #[cfg(feature = "json-io")]
    Json(JsonError),
    UnknownEncoding,
}

#[cfg(feature = "decode")]
impl<R: AsyncRead + Unpin + Send> IntoParser for DispatchWithCharset<R> {
    type Error = IntoParserErrorWithCharset;
    type Parser = DispatchParser<R>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(parser) => {
                use crate::txt::io::ErrorWithCharset;

                parser
                    .parser()
                    .await
                    .map(DispatchParser::Txt)
                    .map_err(|e| match e {
                        ErrorWithCharset::Io(e) => IntoParserErrorWithCharset::Txt(e),
                        ErrorWithCharset::UnknownEncoding(_) => {
                            IntoParserErrorWithCharset::UnknownEncoding
                        }
                    })
            }
            #[cfg(feature = "json-io")]
            Self::Json(parser) => parser
                .parser()
                .await
                .map(DispatchParser::Json)
                .map_err(IntoParserErrorWithCharset::Json),
            #[cfg(feature = "csv-io")]
            Self::Csv(parser) => {
                let Ok(parser) = parser.parser().await;
                Ok(DispatchParser::Csv(parser))
            }
        }
    }
}

#[cfg(all(
    feature = "decode",
    any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs")
))]
impl DispatchWithCharset<File> {
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
        charset: Charset,
    ) -> Result<Self, FromPathError> {
        #[cfg(not(feature = "txt-fs"))]
        let _ = charset;
        let path = path.as_ref();
        match path.extension().and_then(OsStr::to_str) {
            #[cfg(feature = "json-fs")]
            Some(ext) if ext.eq_ignore_ascii_case("json") => {
                let reader = File::open(path).await?;
                Ok(Self::Json(Json::from(reader)))
            }
            #[cfg(feature = "csv-fs")]
            Some(ext) if ext.eq_ignore_ascii_case("csv") => {
                let reader = File::open(path).await?;
                Ok(Self::Csv(Csv::from(reader)))
            }
            #[cfg(feature = "txt-fs")]
            _ => {
                use crate::Title;

                let reader = File::open(path).await?;
                Ok(Self::Txt(TxtWithCharset::from(
                    reader,
                    Title(path.to_string_lossy().into_owned()),
                    None,
                    charset,
                )))
            }
            #[cfg(not(feature = "txt-fs"))]
            _ => Err(FromPathError::UnknownFormat),
        }
    }
}

#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
impl Dispatch<File> {
    pub async fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, FromPathError> {
        let path = path.as_ref();
        match path.extension().and_then(OsStr::to_str) {
            #[cfg(feature = "json-fs")]
            Some(ext) if ext.eq_ignore_ascii_case("json") => {
                let reader = File::open(path).await?;
                Ok(Self::Json(Json::from(reader)))
            }
            #[cfg(feature = "csv-fs")]
            Some(ext) if ext.eq_ignore_ascii_case("csv") => {
                let reader = File::open(path).await?;
                Ok(Self::Csv(Csv::from(reader)))
            }
            #[cfg(feature = "txt-fs")]
            _ => {
                use crate::Title;

                let reader = File::open(path).await?;
                Ok(Self::Txt(Txt::from(
                    reader,
                    Title(path.to_string_lossy().into_owned()),
                    None,
                )))
            }
            #[cfg(not(feature = "txt-fs"))]
            _ => Err(FromPathError::UnknownFormat),
        }
    }
}

#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
impl From<IoError> for FromPathError {
    fn from(value: IoError) -> Self {
        Self::Io(value)
    }
}

#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
impl fmt::Display for FromPathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error when parsing file: {e}"),
            #[cfg(not(feature = "txt-fs"))]
            Self::UnknownFormat => write!(f, "unknown file format"),
        }
    }
}

#[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
impl StdError for FromPathError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            #[cfg(not(feature = "txt-fs"))]
            Self::UnknownFormat => None,
        }
    }
}

impl fmt::Display for IntoParserError {
    #[allow(unused_variables)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(not(any(feature = "txt-io", feature = "json-io")))]
        match *self {}

        #[cfg(any(feature = "txt-io", feature = "json-io"))]
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(e) => write!(f, "could not initialize TXT parser: {e}"),
            #[cfg(feature = "json-io")]
            Self::Json(e) => write!(f, "could not initialize JSON parser: {e}"),
        }
    }
}

impl StdError for IntoParserError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        #[cfg(not(any(feature = "txt-io", feature = "json-io")))]
        match *self {}

        #[cfg(any(feature = "txt-io", feature = "json-io"))]
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(e) => Some(e),
            #[cfg(feature = "json-io")]
            Self::Json(e) => Some(e),
        }
    }
}

impl fmt::Display for ParserError {
    #[allow(unused_variables)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(not(feature = "csv-io"))]
        match *self {}

        #[cfg(feature = "csv-io")]
        match self {
            #[cfg(feature = "csv-io")]
            Self::Csv(e) => write!(f, "could not parse CSV record: {e}"),
        }
    }
}

impl StdError for ParserError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        #[cfg(not(feature = "csv-io"))]
        match *self {}

        #[cfg(feature = "csv-io")]
        match self {
            #[cfg(feature = "csv-io")]
            Self::Csv(e) => Some(e),
        }
    }
}

#[cfg(feature = "decode")]
impl fmt::Display for IntoParserErrorWithCharset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use crate::UnknownEncoding;

        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(e) => write!(f, "could not initialize TXT parser: {e}"),
            #[cfg(feature = "json-io")]
            Self::Json(e) => write!(f, "could not initialize JSON parser: {e}"),
            Self::UnknownEncoding => fmt::Display::fmt(&UnknownEncoding, f),
        }
    }
}

#[cfg(feature = "decode")]
impl StdError for IntoParserErrorWithCharset {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            #[cfg(feature = "txt-io")]
            Self::Txt(e) => Some(e),
            #[cfg(feature = "json-io")]
            Self::Json(e) => Some(e),
            Self::UnknownEncoding => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(
        feature = "decode",
        any(feature = "txt-io", feature = "json-fs", feature = "csv-fs")
    ))]
    use crate::Charset;

    #[cfg(feature = "txt-io")]
    #[tokio::test]
    async fn dispatches_txt_and_forwards_size_hint() {
        let source = Dispatch::Txt(Txt::from(
            "春はあけぼの。".as_bytes(),
            crate::Title("title".to_owned()),
            None,
        ));
        let mut parser = source.parser().await.unwrap();
        assert!(matches!(parser, DispatchParser::Txt(_)));
        assert_eq!(parser.size_hint(), Some(1));
        let doc = parser.next().await.unwrap().unwrap();
        assert_eq!(doc.title.as_str(), "title");
        assert_eq!(doc.body.as_str(), "春はあけぼの。");
        assert_eq!(parser.size_hint(), Some(0));
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
    }

    #[cfg(feature = "txt-io")]
    #[tokio::test]
    async fn reports_txt_initialization_io_error() {
        let source = Dispatch::Txt(Txt::from(
            &b"\xff"[..],
            crate::Title("title".to_owned()),
            None,
        ));
        assert!(matches!(source.parser().await,
            Err(IntoParserError::Txt(error)) if error.kind() == std::io::ErrorKind::InvalidData));
    }

    #[cfg(feature = "json-io")]
    #[tokio::test]
    async fn dispatches_json_and_forwards_size_hint() {
        let source = Dispatch::Json(Json::from(
            br#"[{"title":"one","body":"first"},{"title":"two","body":"second"}]"#.as_slice(),
        ));
        let mut parser = source.parser().await.unwrap();
        assert!(matches!(parser, DispatchParser::Json(_)));
        assert_eq!(parser.size_hint(), Some(2));
        assert_eq!(parser.next().await.unwrap().unwrap().title.as_str(), "one");
        assert_eq!(parser.size_hint(), Some(1));
        assert_eq!(parser.next().await.unwrap().unwrap().title.as_str(), "two");
        assert_eq!(parser.size_hint(), Some(0));
        assert!(parser.next().await.is_none());
    }

    #[cfg(feature = "json-io")]
    #[tokio::test]
    async fn reports_json_initialization_errors() {
        let source = Dispatch::Json(Json::from(b"[".as_slice()));
        let error = match source.parser().await {
            Err(error @ IntoParserError::Json(JsonError::Json(_))) => error,
            _ => panic!("expected a JSON deserialization error"),
        };
        assert!(error.source().is_some());
        assert!(error.to_string().contains("JSON"));

        let source = Dispatch::Json(Json::from(b"[\xff]".as_slice()));
        assert!(matches!(source.parser().await,
            Err(IntoParserError::Json(JsonError::Io(error)))
                if error.kind() == std::io::ErrorKind::InvalidData));
    }

    #[cfg(feature = "csv-io")]
    #[tokio::test]
    async fn dispatches_csv_and_reports_record_errors() {
        let source = Dispatch::Csv(Csv::from(b"title,body\none,first\ntwo\n".as_slice()));
        let mut parser = source.parser().await.unwrap();
        assert!(matches!(parser, DispatchParser::Csv(_)));
        assert_eq!(parser.size_hint(), None);
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let error = parser.next().await.unwrap().unwrap_err();
        assert!(matches!(error, ParserError::Csv(_)));
        assert!(error.source().is_some());
        assert!(error.to_string().contains("CSV"));
    }

    #[cfg(all(feature = "txt-io", feature = "decode"))]
    #[tokio::test]
    async fn decodes_txt_without_losing_metadata() {
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode("春はあけぼの。");
        let metadata = crate::Metadata([("author".to_owned(), "清少納言".to_owned())].into());
        let source = DispatchWithCharset::Txt(TxtWithCharset::from(
            encoded.as_ref(),
            crate::Title("title".to_owned()),
            Some(metadata.clone()),
            Charset::ShiftJis,
        ));
        let mut parser = source.parser().await.unwrap();
        let doc = parser.next().await.unwrap().unwrap();
        assert_eq!(doc.title.as_str(), "title");
        assert_eq!(doc.body.as_str(), "春はあけぼの。");
        assert_eq!(doc.metadata, Some(metadata));
    }

    #[cfg(all(feature = "txt-io", feature = "decode"))]
    #[tokio::test]
    async fn reports_txt_charset_errors() {
        let source = DispatchWithCharset::Txt(TxtWithCharset::from(
            &b"\xff"[..],
            crate::Title("title".to_owned()),
            None,
            Charset::Dynamic,
        ));
        assert!(matches!(
            source.parser().await,
            Err(IntoParserErrorWithCharset::UnknownEncoding)
        ));
    }

    #[cfg(all(feature = "json-io", feature = "decode"))]
    #[tokio::test]
    async fn dispatches_json_without_a_charset() {
        let source = DispatchWithCharset::Json(Json::from(
            br#"[{"title":"one","body":"first"}]"#.as_slice(),
        ));
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let source = DispatchWithCharset::Json(Json::from(b"[".as_slice()));
        assert!(matches!(
            source.parser().await,
            Err(IntoParserErrorWithCharset::Json(JsonError::Json(_)))
        ));
    }

    #[cfg(all(feature = "csv-io", feature = "decode"))]
    #[tokio::test]
    async fn dispatches_csv_without_a_charset() {
        let source =
            DispatchWithCharset::Csv(Csv::from(b"title,body\none,first\ntwo\n".as_slice()));
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        assert!(matches!(
            parser.next().await,
            Some(Err(ParserError::Csv(_)))
        ));
    }

    #[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
    fn file_with_contents(extension: &str, contents: &[u8]) -> tempfile::NamedTempFile {
        use std::io::Write;
        let mut file = tempfile::Builder::new()
            .suffix(extension)
            .tempfile()
            .unwrap();
        file.write_all(contents).unwrap();
        file
    }

    #[cfg(all(feature = "txt-fs", feature = "decode"))]
    #[tokio::test]
    async fn decodes_txt_files_with_charset() {
        let (encoded, _, _) = encoding_rs::SHIFT_JIS.encode("春はあけぼの。");
        let file = file_with_contents(".unknown", encoded.as_ref());
        let source = DispatchWithCharset::from_path(file.path(), Charset::ShiftJis)
            .await
            .unwrap();
        let mut parser = source.parser().await.unwrap();
        let doc = parser.next().await.unwrap().unwrap();
        assert_eq!(doc.body.as_str(), "春はあけぼの。");
        assert_eq!(doc.title.as_str(), file.path().to_string_lossy());
    }

    #[cfg(all(feature = "json-fs", feature = "decode"))]
    #[tokio::test]
    async fn dispatches_json_files_without_a_charset() {
        let file = file_with_contents(".JSON", br#"[{"title":"one","body":"first"}]"#);
        let source = DispatchWithCharset::from_path(file.path(), Charset::Utf8)
            .await
            .unwrap();
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let file = file_with_contents(".json", b"[");
        let source = DispatchWithCharset::from_path(file.path(), Charset::ShiftJis)
            .await
            .unwrap();
        assert!(matches!(
            source.parser().await,
            Err(IntoParserErrorWithCharset::Json(JsonError::Json(_)))
        ));
    }

    #[cfg(all(feature = "csv-fs", feature = "decode"))]
    #[tokio::test]
    async fn dispatches_csv_files_without_a_charset() {
        let file = file_with_contents(".CSV", b"title,body\none,first\n");
        let source = DispatchWithCharset::from_path(file.path(), Charset::Utf8)
            .await
            .unwrap();
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        let source = DispatchWithCharset::from_path(file.path(), Charset::Dynamic)
            .await
            .unwrap();
        let mut parser = source.parser().await.unwrap();
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
    }

    #[cfg(feature = "txt-fs")]
    #[tokio::test]
    async fn defaults_to_txt_for_unknown_or_missing_extensions() {
        for extension in [
            ".txt",
            ".unknown",
            "",
            #[cfg(not(feature = "json-fs"))]
            ".json",
            #[cfg(not(feature = "csv-fs"))]
            ".csv",
        ] {
            let file = file_with_contents(extension, "春はあけぼの。".as_bytes());
            let source = Dispatch::from_path(file.path()).await.unwrap();
            let mut parser = source.parser().await.unwrap();
            assert!(matches!(parser, DispatchParser::Txt(_)));
            let doc = parser.next().await.unwrap().unwrap();
            assert_eq!(doc.title.as_str(), file.path().to_string_lossy());
            assert_eq!(doc.body.as_str(), "春はあけぼの。");
            assert!(doc.metadata.is_none());
        }
    }

    #[cfg(all(not(feature = "txt-fs"), any(feature = "json-fs", feature = "csv-fs")))]
    #[tokio::test]
    async fn rejects_unknown_formats_without_txt_fs() {
        for extension in [
            ".txt",
            ".unknown",
            "",
            #[cfg(not(feature = "json-fs"))]
            ".json",
            #[cfg(not(feature = "csv-fs"))]
            ".csv",
        ] {
            let file = file_with_contents(extension, b"text");
            assert!(matches!(
                Dispatch::from_path(file.path()).await,
                Err(FromPathError::UnknownFormat)
            ));
            #[cfg(feature = "decode")]
            assert!(matches!(
                DispatchWithCharset::from_path(file.path(), Charset::Utf8).await,
                Err(FromPathError::UnknownFormat)
            ));
            let path = file.path().to_owned();
            file.close().unwrap();
            assert!(matches!(
                Dispatch::from_path(&path).await,
                Err(FromPathError::UnknownFormat)
            ));
            #[cfg(feature = "decode")]
            assert!(matches!(
                DispatchWithCharset::from_path(path, Charset::Utf8).await,
                Err(FromPathError::UnknownFormat)
            ));
        }
        let error = FromPathError::UnknownFormat;
        assert!(error.source().is_none());
        assert_eq!(error.to_string(), "unknown file format");
    }

    #[cfg(feature = "json-fs")]
    #[tokio::test]
    async fn selects_json_files_and_reports_invalid_contents_when_initializing_parser() {
        for extension in [".json", ".JSON"] {
            let file = file_with_contents(extension, br#"[{"title":"one","body":"first"}]"#);
            let source = Dispatch::from_path(file.path()).await.unwrap();
            let mut parser = source.parser().await.unwrap();
            assert!(matches!(parser, DispatchParser::Json(_)));
            assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
        }
        let file = file_with_contents(".json", b"[");
        let source = Dispatch::from_path(file.path()).await.unwrap();
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Json(JsonError::Json(_)))
        ));
    }

    #[cfg(feature = "csv-fs")]
    #[tokio::test]
    async fn selects_csv_files_and_delays_record_errors_until_iteration() {
        for extension in [".csv", ".CSV"] {
            let file = file_with_contents(extension, b"title,body\none,first\ntwo\n");
            let source = Dispatch::from_path(file.path()).await.unwrap();
            let mut parser = source.parser().await.unwrap();
            assert!(matches!(parser, DispatchParser::Csv(_)));
            assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "first");
            assert!(matches!(
                parser.next().await,
                Some(Err(ParserError::Csv(_)))
            ));
        }
    }

    #[cfg(any(feature = "txt-fs", feature = "json-fs", feature = "csv-fs"))]
    #[tokio::test]
    async fn preserves_filesystem_io_errors() {
        for extension in [
            #[cfg(feature = "txt-fs")]
            ".unknown",
            #[cfg(feature = "json-fs")]
            ".json",
            #[cfg(feature = "csv-fs")]
            ".csv",
        ] {
            let file = file_with_contents(extension, b"");
            let path = file.path().to_owned();
            file.close().unwrap();
            #[cfg(feature = "decode")]
            assert!(matches!(
                DispatchWithCharset::from_path(&path, Charset::Utf8).await,
                Err(FromPathError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound
            ));
            let error = match Dispatch::from_path(path).await {
                Err(error @ FromPathError::Io(_)) => error,
                _ => panic!("expected an I/O error"),
            };
            assert!(error.source().is_some());
            assert!(matches!(error,
                FromPathError::Io(error) if error.kind() == std::io::ErrorKind::NotFound));
        }
    }
}
