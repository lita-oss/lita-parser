#[cfg(feature = "csv-sync")]
use crate::csv::sync_bytes::{Csv, CsvParser};
use crate::decode::{Encoding, UnknownEncoding, Utf8};
#[cfg(feature = "json-sync")]
use crate::json::sync_bytes::{Error as JsonInitError, Json, JsonParser};
#[cfg(feature = "txt-sync")]
use crate::txt::sync_bytes::{Txt, TxtParser};
use crate::{IntoParser, ParsedDoc, Parser};
#[cfg(feature = "txt-sync")]
use crate::{Metadata, Title};

#[cfg(feature = "csv-sync")]
use csv::Error as CsvError;

#[cfg(feature = "json-sync")]
use serde_json::Error as JsonError;

use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;
#[cfg(feature = "csv-sync")]
use std::io::Cursor;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Format {
    #[cfg(feature = "txt-sync")]
    Txt {
        title: Title,
        metadata: Option<Metadata>,
    },
    #[cfg(feature = "json-sync")]
    Json,
    #[cfg(feature = "csv-sync")]
    Csv,
}

pub struct Dispatch<S, E = Utf8> {
    bytes: S,
    encoding: E,
    format: Format,
}

pub struct DispatchParser {
    imp: DispatchParserImpl,
}

enum DispatchParserImpl {
    #[cfg(feature = "txt-sync")]
    Txt(TxtParser),
    #[cfg(feature = "json-sync")]
    Json(JsonParser),
    #[cfg(feature = "csv-sync")]
    Csv(CsvParser<Cursor<Vec<u8>>>),
}

impl<'a> Dispatch<&'a str> {
    pub fn new(bytes: &'a str, format: Format) -> Self {
        Self {
            bytes,
            format,
            encoding: Utf8,
        }
    }
}

impl Dispatch<String> {
    pub fn new(bytes: String, format: Format) -> Self {
        Self {
            bytes,
            format,
            encoding: Utf8,
        }
    }
}

impl<'a> Dispatch<Cow<'a, str>> {
    pub fn new(bytes: Cow<'a, str>, format: Format) -> Self {
        Self {
            bytes,
            format,
            encoding: Utf8,
        }
    }
}

impl<'a, E: Encoding> Dispatch<&'a [u8], E> {
    pub fn new(bytes: &'a [u8], format: Format, encoding: E) -> Self {
        Self {
            bytes,
            format,
            encoding,
        }
    }
}

impl<E: Encoding> Dispatch<Vec<u8>, E> {
    pub fn new(bytes: Vec<u8>, format: Format, encoding: E) -> Self {
        Self {
            bytes,
            format,
            encoding,
        }
    }
}

impl<'a, E: Encoding> Dispatch<Cow<'a, [u8]>, E> {
    pub fn new(bytes: Cow<'a, [u8]>, format: Format, encoding: E) -> Self {
        Self {
            bytes,
            format,
            encoding,
        }
    }
}

impl IntoParser for Dispatch<&str> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Dispatch::<Cow<'_, str>>::new(Cow::Borrowed(self.bytes), self.format)
            .parser()
            .await
    }
}

impl IntoParser for Dispatch<String> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Dispatch::<Cow<'_, str>>::new(Cow::Owned(self.bytes), self.format)
            .parser()
            .await
    }
}

impl IntoParser for Dispatch<Cow<'_, str>> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let imp = match self.format {
            #[cfg(feature = "txt-sync")]
            Format::Txt { title, metadata } => {
                let Ok(parser) = Txt::<Cow<'_, str>>::new(self.bytes, title, metadata)
                    .parser()
                    .await;
                DispatchParserImpl::Txt(parser)
            }
            #[cfg(feature = "json-sync")]
            Format::Json => {
                let parser = Json::<Cow<'_, str>>::new(self.bytes)
                    .parser()
                    .await
                    .map_err(IntoParserError::Json)?;
                DispatchParserImpl::Json(parser)
            }
            #[cfg(feature = "csv-sync")]
            Format::Csv => {
                let Ok(parser) = Csv::<String>::new(self.bytes.into_owned()).parser().await;
                DispatchParserImpl::Csv(parser)
            }
        };
        Ok(DispatchParser { imp })
    }
}

impl<E: Encoding> IntoParser for Dispatch<&[u8], E> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Dispatch::<Cow<'_, [u8]>, E>::new(Cow::Borrowed(self.bytes), self.format, self.encoding)
            .parser()
            .await
    }
}

impl<E: Encoding> IntoParser for Dispatch<Vec<u8>, E> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Dispatch::<Cow<'_, [u8]>, E>::new(Cow::Owned(self.bytes), self.format, self.encoding)
            .parser()
            .await
    }
}

impl<E: Encoding> IntoParser for Dispatch<Cow<'_, [u8]>, E> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let imp = match self.format {
            #[cfg(feature = "txt-sync")]
            Format::Txt { title, metadata } => {
                let parser =
                    Txt::<Cow<'_, [u8]>, E>::new(self.bytes, title, metadata, self.encoding)
                        .parser()
                        .await?;
                DispatchParserImpl::Txt(parser)
            }
            #[cfg(feature = "json-sync")]
            Format::Json => {
                let parser = Json::<Cow<'_, [u8]>, E>::new(self.bytes, self.encoding)
                    .parser()
                    .await?;
                DispatchParserImpl::Json(parser)
            }
            #[cfg(feature = "csv-sync")]
            Format::Csv => {
                use crate::decode::decode_cow;

                let bytes = decode_cow(self.bytes, self.encoding.oneshot_decoder())?.into_owned();
                let Ok(parser) = Csv::<String>::new(bytes).parser().await;
                DispatchParserImpl::Csv(parser)
            }
        };
        Ok(DispatchParser { imp })
    }
}

impl Parser for DispatchParser {
    type Error = ParserError;

    fn size_hint(&self) -> Option<usize> {
        match &self.imp {
            #[cfg(feature = "txt-sync")]
            DispatchParserImpl::Txt(parser) => parser.size_hint(),
            #[cfg(feature = "json-sync")]
            DispatchParserImpl::Json(parser) => parser.size_hint(),
            #[cfg(feature = "csv-sync")]
            DispatchParserImpl::Csv(parser) => parser.size_hint(),
        }
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        match &mut self.imp {
            #[cfg(feature = "txt-sync")]
            DispatchParserImpl::Txt(parser) => {
                parser.next().await.map(|res| res.map_err(|e| match e {}))
            }
            #[cfg(feature = "json-sync")]
            DispatchParserImpl::Json(parser) => {
                parser.next().await.map(|res| res.map_err(|e| match e {}))
            }
            #[cfg(feature = "csv-sync")]
            DispatchParserImpl::Csv(parser) => {
                parser.next().await.map(|res| res.map_err(ParserError::Csv))
            }
        }
    }
}

#[derive(Debug)]
pub enum IntoParserError {
    #[cfg(feature = "json-sync")]
    Json(JsonError),
    UnknownEncoding(UnknownEncoding),
}

impl From<UnknownEncoding> for IntoParserError {
    fn from(e: UnknownEncoding) -> Self {
        Self::UnknownEncoding(e)
    }
}

#[cfg(feature = "json-sync")]
impl From<JsonInitError> for IntoParserError {
    fn from(e: JsonInitError) -> Self {
        match e {
            JsonInitError::Json(e) => Self::Json(e),
            JsonInitError::UnknownEncoding(e) => Self::UnknownEncoding(e),
        }
    }
}

impl fmt::Display for IntoParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "json-sync")]
            Self::Json(e) => write!(f, "could not parse JSON: {e}"),
            Self::UnknownEncoding(e) => e.fmt(f),
        }
    }
}

impl StdError for IntoParserError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            #[cfg(feature = "json-sync")]
            Self::Json(e) => Some(e),
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}

#[derive(Debug)]
pub enum ParserError {
    #[cfg(feature = "csv-sync")]
    Csv(CsvError),
}

impl fmt::Display for ParserError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(feature = "csv-sync")]
            Self::Csv(ref e) => write!(f, "could not parse CSV: {e}"),
        }
    }
}

impl StdError for ParserError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            #[cfg(feature = "csv-sync")]
            Self::Csv(ref e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(all(feature = "decode", any(feature = "txt-sync", feature = "csv-sync")))]
    use crate::decode::{Dynamic, ShiftJis};

    use super::*;

    #[cfg(all(feature = "decode", any(feature = "txt-sync", feature = "csv-sync")))]
    use encoding_rs::SHIFT_JIS;

    #[cfg(feature = "decode")]
    async fn assert_body(
        source: impl IntoParser<Error = IntoParserError, Parser = DispatchParser>,
        expected: &str,
    ) {
        let mut parser = source.parser().await.unwrap();
        assert_eq!(
            parser.next().await.unwrap().unwrap().body.as_str(),
            expected
        );
        assert!(parser.next().await.is_none());
    }

    #[cfg(feature = "txt-sync")]
    #[tokio::test]
    async fn dispatches_text_and_forwards_size_hints() {
        for source in [
            Cow::Borrowed("春はあけぼの。"),
            Cow::Owned("春はあけぼの。".to_owned()),
        ] {
            let metadata = Metadata([("source".to_owned(), "book".to_owned())].into());
            let mut parser = Dispatch::<Cow<'_, str>>::new(
                source,
                Format::Txt {
                    title: Title("title".to_owned()),
                    metadata: Some(metadata.clone()),
                },
            )
            .parser()
            .await
            .unwrap();
            assert!(matches!(parser.imp, DispatchParserImpl::Txt(_)));
            assert_eq!(parser.size_hint(), Some(1));
            let doc = parser.next().await.unwrap().unwrap();
            assert_eq!(doc.title.as_str(), "title");
            assert_eq!(doc.body.as_str(), "春はあけぼの。");
            assert_eq!(doc.metadata, Some(metadata));
            assert_eq!(parser.size_hint(), Some(0));
            assert!(parser.next().await.is_none());
            assert!(parser.next().await.is_none());
        }
    }

    #[cfg(feature = "json-sync")]
    #[tokio::test]
    async fn dispatches_json_and_reports_initialization_errors() {
        let mut parser =
            Dispatch::<&str>::new(r#"[{"title":"spring","body":"dawn"}]"#, Format::Json)
                .parser()
                .await
                .unwrap();
        assert!(matches!(parser.imp, DispatchParserImpl::Json(_)));
        assert_eq!(parser.size_hint(), Some(1));
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "dawn");
        assert_eq!(parser.size_hint(), Some(0));
        assert!(parser.next().await.is_none());
        assert!(matches!(
            Dispatch::<&str>::new("[", Format::Json).parser().await,
            Err(IntoParserError::Json(_))
        ));
    }

    #[cfg(feature = "csv-sync")]
    #[tokio::test]
    async fn normalizes_csv_sources_and_reports_iteration_errors() {
        for input in [
            Cow::Borrowed("title,body\nspring,dawn\ninvalid\n"),
            Cow::Owned("title,body\nspring,dawn\ninvalid\n".to_owned()),
        ] {
            let mut parser = Dispatch::<Cow<'_, str>>::new(input, Format::Csv)
                .parser()
                .await
                .unwrap();
            assert!(matches!(parser.imp, DispatchParserImpl::Csv(_)));
            assert_eq!(parser.size_hint(), None);
            assert_eq!(
                parser.next().await.unwrap().unwrap().title.as_str(),
                "spring"
            );
            assert!(matches!(
                parser.next().await,
                Some(Err(ParserError::Csv(_)))
            ));
        }
    }

    #[cfg(all(feature = "decode", feature = "txt-sync"))]
    #[tokio::test]
    async fn decodes_text_at_parser_initialization() {
        let text = "春はあけぼの。";
        let (bytes, _, _) = SHIFT_JIS.encode(text);
        assert_body(
            Dispatch::<&[u8], _>::new(
                bytes.as_ref(),
                Format::Txt {
                    title: Title("title".to_owned()),
                    metadata: None,
                },
                ShiftJis,
            ),
            text,
        )
        .await;
        assert_body(
            Dispatch::<Vec<u8>, _>::new(
                bytes.to_vec(),
                Format::Txt {
                    title: Title("title".to_owned()),
                    metadata: None,
                },
                ShiftJis,
            ),
            text,
        )
        .await;
        for input in [Cow::Borrowed(bytes.as_ref()), Cow::Owned(bytes.to_vec())] {
            assert_body(
                Dispatch::<Cow<'_, [u8]>, _>::new(
                    input,
                    Format::Txt {
                        title: Title("title".to_owned()),
                        metadata: None,
                    },
                    ShiftJis,
                ),
                text,
            )
            .await;
        }
        let source = Dispatch::<&[u8], _>::new(
            &b"\xff"[..],
            Format::Txt {
                title: Title("title".to_owned()),
                metadata: None,
            },
            Dynamic,
        );
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::UnknownEncoding(_))
        ));
    }

    #[cfg(all(feature = "decode", feature = "json-sync"))]
    #[tokio::test]
    async fn parses_utf8_json_and_reports_errors() {
        let bytes = br#"[{"title":"spring","body":"dawn"}]"#.as_slice();
        assert_body(Dispatch::<&[u8]>::new(bytes, Format::Json, Utf8), "dawn").await;
        assert_body(
            Dispatch::<Vec<u8>>::new(bytes.to_vec(), Format::Json, Utf8),
            "dawn",
        )
        .await;
        for input in [Cow::Borrowed(bytes), Cow::Owned(bytes.to_vec())] {
            assert_body(
                Dispatch::<Cow<'_, [u8]>>::new(input, Format::Json, Utf8),
                "dawn",
            )
            .await;
        }
        let source = Dispatch::<&[u8]>::new(b"[".as_slice(), Format::Json, Utf8);
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::Json(_))
        ));
        let source = Dispatch::<&[u8]>::new(b"\xff".as_slice(), Format::Json, Utf8);
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::UnknownEncoding(_))
        ));
    }

    #[cfg(all(feature = "decode", feature = "csv-sync"))]
    #[tokio::test]
    async fn decodes_csv_and_preserves_iteration_errors() {
        let (valid, _, _) = SHIFT_JIS.encode("title,body\n春,あけぼの。\n");
        assert_body(
            Dispatch::<&[u8], _>::new(valid.as_ref(), Format::Csv, ShiftJis),
            "あけぼの。",
        )
        .await;
        assert_body(
            Dispatch::<Vec<u8>, _>::new(valid.to_vec(), Format::Csv, ShiftJis),
            "あけぼの。",
        )
        .await;
        for input in [Cow::Borrowed(valid.as_ref()), Cow::Owned(valid.to_vec())] {
            assert_body(
                Dispatch::<Cow<'_, [u8]>, _>::new(input, Format::Csv, ShiftJis),
                "あけぼの。",
            )
            .await;
        }
        let (bytes, _, _) = SHIFT_JIS.encode("title,body\n春,あけぼの。\ninvalid\n");
        let mut parser = Dispatch::<Cow<'_, [u8]>, _>::new(bytes, Format::Csv, ShiftJis)
            .parser()
            .await
            .unwrap();
        assert_eq!(
            parser.next().await.unwrap().unwrap().body.as_str(),
            "あけぼの。"
        );
        assert!(matches!(
            parser.next().await,
            Some(Err(ParserError::Csv(_)))
        ));
        let source = Dispatch::<Vec<u8>, _>::new(vec![0xff], Format::Csv, Dynamic);
        assert!(matches!(
            source.parser().await,
            Err(IntoParserError::UnknownEncoding(_))
        ));
    }
}
