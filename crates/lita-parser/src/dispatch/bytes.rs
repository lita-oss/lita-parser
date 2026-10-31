use crate::{IntoParser, ParsedDoc, Parser};

use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;

#[cfg(feature = "csv-bytes")]
use crate::csv::bytes::{Csv, CsvParser};
#[cfg(feature = "json-bytes")]
use crate::json::{JsonParser, bytes::Json};
#[cfg(feature = "txt-bytes")]
use crate::txt::{TxtParser, bytes::Txt};
#[cfg(feature = "csv-bytes")]
use std::io::Cursor;

#[cfg(all(feature = "decode", feature = "csv-bytes"))]
use crate::csv::bytes::CsvWithCharset;
#[cfg(all(feature = "decode", feature = "txt-bytes"))]
use crate::txt::bytes::TxtWithCharset;

pub enum Dispatch<S> {
    #[cfg(feature = "txt-bytes")]
    Txt(Txt<S>),
    #[cfg(feature = "json-bytes")]
    Json(Json<S>),
    #[cfg(feature = "csv-bytes")]
    Csv(Csv<S>),
}

pub enum DispatchParser {
    #[cfg(feature = "txt-bytes")]
    Txt(TxtParser),
    #[cfg(feature = "json-bytes")]
    Json(JsonParser),
    #[cfg(feature = "csv-bytes")]
    Csv(CsvParser<Cursor<Vec<u8>>>),
}

#[derive(Debug)]
pub enum IntoParserError {
    #[cfg(feature = "json-bytes")]
    Json(serde_json::Error),
}

#[derive(Debug)]
pub enum ParserError {
    #[cfg(feature = "csv-bytes")]
    Csv(csv::Error),
}

impl IntoParser for Dispatch<&str> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(txt) => {
                let Ok(parser) = txt.parser().await;
                Ok(DispatchParser::Txt(parser))
            }
            #[cfg(feature = "json-bytes")]
            Self::Json(json) => json
                .parser()
                .await
                .map(DispatchParser::Json)
                .map_err(IntoParserError::Json),
            #[cfg(feature = "csv-bytes")]
            Self::Csv(csv) => {
                let Ok(parser) = Csv::from(csv.bytes.to_owned()).parser().await;
                Ok(DispatchParser::Csv(parser))
            }
        }
    }
}

impl IntoParser for Dispatch<Cow<'_, str>> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(txt) => {
                let Ok(parser) = txt.parser().await;
                Ok(DispatchParser::Txt(parser))
            }
            #[cfg(feature = "json-bytes")]
            Self::Json(json) => json
                .parser()
                .await
                .map(DispatchParser::Json)
                .map_err(IntoParserError::Json),
            #[cfg(feature = "csv-bytes")]
            Self::Csv(csv) => {
                let Ok(parser) = Csv::from(csv.bytes.into_owned()).parser().await;
                Ok(DispatchParser::Csv(parser))
            }
        }
    }
}

impl IntoParser for Dispatch<String> {
    type Error = IntoParserError;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(txt) => {
                let Ok(parser) = txt.parser().await;
                Ok(DispatchParser::Txt(parser))
            }
            #[cfg(feature = "json-bytes")]
            Self::Json(json) => json
                .parser()
                .await
                .map(DispatchParser::Json)
                .map_err(IntoParserError::Json),
            #[cfg(feature = "csv-bytes")]
            Self::Csv(csv) => {
                let Ok(parser) = csv.parser().await;
                Ok(DispatchParser::Csv(parser))
            }
        }
    }
}

impl Parser for DispatchParser {
    type Error = ParserError;

    fn size_hint(&self) -> Option<usize> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(parser) => parser.size_hint(),
            #[cfg(feature = "json-bytes")]
            Self::Json(parser) => parser.size_hint(),
            #[cfg(feature = "csv-bytes")]
            Self::Csv(parser) => parser.size_hint(),
        }
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(parser) => parser.next().await.map(|res| res.map_err(|e| match e {})),
            #[cfg(feature = "json-bytes")]
            Self::Json(parser) => parser.next().await.map(|res| res.map_err(|e| match e {})),
            #[cfg(feature = "csv-bytes")]
            Self::Csv(parser) => parser.next().await.map(|res| res.map_err(ParserError::Csv)),
        }
    }
}

#[cfg(feature = "decode")]
pub enum DispatchWithCharset<S> {
    #[cfg(feature = "txt-bytes")]
    Txt(TxtWithCharset<S>),
    #[cfg(feature = "json-bytes")]
    Json(Json<S>),
    #[cfg(feature = "csv-bytes")]
    Csv(CsvWithCharset<S>),
}

#[cfg(feature = "decode")]
#[derive(Debug)]
pub enum IntoParserErrorWithCharset {
    #[cfg(feature = "json-bytes")]
    Json(serde_json::Error),
    UnknownEncoding,
}

#[cfg(feature = "decode")]
impl IntoParser for DispatchWithCharset<&[u8]> {
    type Error = IntoParserErrorWithCharset;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(txt) => txt
                .parser()
                .await
                .map(DispatchParser::Txt)
                .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding),
            #[cfg(feature = "json-bytes")]
            Self::Json(json) => {
                let text = std::str::from_utf8(json.bytes)
                    .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding)?;
                Json::from(text)
                    .parser()
                    .await
                    .map(DispatchParser::Json)
                    .map_err(IntoParserErrorWithCharset::Json)
            }
            #[cfg(feature = "csv-bytes")]
            Self::Csv(csv) => CsvWithCharset::from(csv.bytes.to_owned(), csv.charset)
                .parser()
                .await
                .map(DispatchParser::Csv)
                .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding),
        }
    }
}

#[cfg(feature = "decode")]
impl IntoParser for DispatchWithCharset<Cow<'_, [u8]>> {
    type Error = IntoParserErrorWithCharset;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(txt) => txt
                .parser()
                .await
                .map(DispatchParser::Txt)
                .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding),
            #[cfg(feature = "json-bytes")]
            Self::Json(json) => {
                let text = std::str::from_utf8(&json.bytes)
                    .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding)?;
                Json::from(text)
                    .parser()
                    .await
                    .map(DispatchParser::Json)
                    .map_err(IntoParserErrorWithCharset::Json)
            }
            #[cfg(feature = "csv-bytes")]
            Self::Csv(csv) => CsvWithCharset::from(csv.bytes.into_owned(), csv.charset)
                .parser()
                .await
                .map(DispatchParser::Csv)
                .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding),
        }
    }
}

#[cfg(feature = "decode")]
impl IntoParser for DispatchWithCharset<Vec<u8>> {
    type Error = IntoParserErrorWithCharset;
    type Parser = DispatchParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self {
            #[cfg(feature = "txt-bytes")]
            Self::Txt(txt) => txt
                .parser()
                .await
                .map(DispatchParser::Txt)
                .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding),
            #[cfg(feature = "json-bytes")]
            Self::Json(json) => {
                let text = std::str::from_utf8(&json.bytes)
                    .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding)?;
                Json::from(text)
                    .parser()
                    .await
                    .map(DispatchParser::Json)
                    .map_err(IntoParserErrorWithCharset::Json)
            }
            #[cfg(feature = "csv-bytes")]
            Self::Csv(csv) => csv
                .parser()
                .await
                .map(DispatchParser::Csv)
                .map_err(|_| IntoParserErrorWithCharset::UnknownEncoding),
        }
    }
}

impl fmt::Display for IntoParserError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(feature = "json-bytes")]
            Self::Json(ref e) => write!(_f, "could not parse JSON: {e}"),
        }
    }
}

impl StdError for IntoParserError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            #[cfg(feature = "json-bytes")]
            Self::Json(ref e) => Some(e),
        }
    }
}

impl fmt::Display for ParserError {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(feature = "csv-bytes")]
            Self::Csv(ref e) => write!(_f, "could not parse CSV: {e}"),
        }
    }
}

impl StdError for ParserError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match *self {
            #[cfg(feature = "csv-bytes")]
            Self::Csv(ref e) => Some(e),
        }
    }
}

#[cfg(feature = "decode")]
impl fmt::Display for IntoParserErrorWithCharset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "json-bytes")]
            Self::Json(e) => write!(f, "could not parse JSON: {e}"),
            Self::UnknownEncoding => crate::UnknownEncoding.fmt(f),
        }
    }
}

#[cfg(feature = "decode")]
impl StdError for IntoParserErrorWithCharset {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            #[cfg(feature = "json-bytes")]
            Self::Json(e) => Some(e),
            Self::UnknownEncoding => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(all(feature = "decode", any(feature = "txt-bytes", feature = "csv-bytes")))]
    use crate::Charset;
    #[cfg(any(feature = "txt-bytes", feature = "csv-bytes"))]
    use std::borrow::Cow;

    #[cfg(feature = "decode")]
    async fn assert_body(
        source: impl IntoParser<Error = IntoParserErrorWithCharset, Parser = DispatchParser>,
        expected: &str,
    ) {
        let mut parser = source.parser().await.unwrap();
        assert_eq!(
            parser.next().await.unwrap().unwrap().body.as_str(),
            expected
        );
        assert!(parser.next().await.is_none());
    }

    #[cfg(feature = "txt-bytes")]
    #[tokio::test]
    async fn dispatches_text_and_forwards_size_hints() {
        for source in [
            Cow::Borrowed("春はあけぼの。"),
            Cow::Owned("春はあけぼの。".to_owned()),
        ] {
            let metadata = crate::Metadata([("source".to_owned(), "book".to_owned())].into());
            let mut parser = Dispatch::Txt(Txt::from(
                source,
                crate::Title("title".to_owned()),
                Some(metadata.clone()),
            ))
            .parser()
            .await
            .unwrap();
            assert!(matches!(parser, DispatchParser::Txt(_)));
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

    #[cfg(feature = "json-bytes")]
    #[tokio::test]
    async fn dispatches_json_and_reports_initialization_errors() {
        let mut parser = Dispatch::Json(Json::from(r#"[{"title":"spring","body":"dawn"}]"#))
            .parser()
            .await
            .unwrap();
        assert!(matches!(parser, DispatchParser::Json(_)));
        assert_eq!(parser.size_hint(), Some(1));
        assert_eq!(parser.next().await.unwrap().unwrap().body.as_str(), "dawn");
        assert_eq!(parser.size_hint(), Some(0));
        assert!(parser.next().await.is_none());
        assert!(matches!(
            Dispatch::Json(Json::from("[")).parser().await,
            Err(IntoParserError::Json(_))
        ));
    }

    #[cfg(feature = "csv-bytes")]
    #[tokio::test]
    async fn normalizes_csv_sources_and_reports_iteration_errors() {
        for input in [
            Cow::Borrowed("title,body\nspring,dawn\ninvalid\n"),
            Cow::Owned("title,body\nspring,dawn\ninvalid\n".to_owned()),
        ] {
            let mut parser = Dispatch::Csv(Csv::from(input)).parser().await.unwrap();
            assert!(matches!(parser, DispatchParser::Csv(_)));
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

    #[cfg(all(feature = "decode", feature = "txt-bytes"))]
    #[tokio::test]
    async fn decodes_text_at_parser_initialization() {
        let text = "春はあけぼの。";
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(text);
        assert_body(
            DispatchWithCharset::Txt(TxtWithCharset::from(
                bytes.as_ref(),
                crate::Title("title".to_owned()),
                None,
                Charset::ShiftJis,
            )),
            text,
        )
        .await;
        assert_body(
            DispatchWithCharset::Txt(TxtWithCharset::from(
                bytes.to_vec(),
                crate::Title("title".to_owned()),
                None,
                Charset::ShiftJis,
            )),
            text,
        )
        .await;
        for input in [Cow::Borrowed(bytes.as_ref()), Cow::Owned(bytes.to_vec())] {
            assert_body(
                DispatchWithCharset::Txt(TxtWithCharset::from(
                    input,
                    crate::Title("title".to_owned()),
                    None,
                    Charset::ShiftJis,
                )),
                text,
            )
            .await;
        }
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

    #[cfg(all(feature = "decode", feature = "json-bytes"))]
    #[tokio::test]
    async fn parses_utf8_json_and_reports_errors() {
        let bytes = br#"[{"title":"spring","body":"dawn"}]"#.as_slice();
        assert_body(DispatchWithCharset::Json(Json::from(bytes)), "dawn").await;
        assert_body(
            DispatchWithCharset::Json(Json::from(bytes.to_vec())),
            "dawn",
        )
        .await;
        for input in [Cow::Borrowed(bytes), Cow::Owned(bytes.to_vec())] {
            assert_body(DispatchWithCharset::Json(Json::from(input)), "dawn").await;
        }
        let source = DispatchWithCharset::Json(Json::from(b"[".as_slice()));
        assert!(matches!(
            source.parser().await,
            Err(IntoParserErrorWithCharset::Json(_))
        ));
        let source = DispatchWithCharset::Json(Json::from(b"\xff".as_slice()));
        assert!(matches!(
            source.parser().await,
            Err(IntoParserErrorWithCharset::UnknownEncoding)
        ));
    }

    #[cfg(all(feature = "decode", feature = "csv-bytes"))]
    #[tokio::test]
    async fn decodes_csv_and_preserves_iteration_errors() {
        let (valid, _, _) = encoding_rs::SHIFT_JIS.encode("title,body\n春,あけぼの。\n");
        assert_body(
            DispatchWithCharset::Csv(CsvWithCharset::from(valid.as_ref(), Charset::ShiftJis)),
            "あけぼの。",
        )
        .await;
        assert_body(
            DispatchWithCharset::Csv(CsvWithCharset::from(valid.to_vec(), Charset::ShiftJis)),
            "あけぼの。",
        )
        .await;
        for input in [Cow::Borrowed(valid.as_ref()), Cow::Owned(valid.to_vec())] {
            assert_body(
                DispatchWithCharset::Csv(CsvWithCharset::from(input, Charset::ShiftJis)),
                "あけぼの。",
            )
            .await;
        }
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode("title,body\n春,あけぼの。\ninvalid\n");
        let mut parser = DispatchWithCharset::Csv(CsvWithCharset::from(bytes, Charset::ShiftJis))
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
        let source = DispatchWithCharset::Csv(CsvWithCharset::from(vec![0xff], Charset::Dynamic));
        assert!(matches!(
            source.parser().await,
            Err(IntoParserErrorWithCharset::UnknownEncoding)
        ));
    }
}
