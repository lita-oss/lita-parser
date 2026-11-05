use crate::IntoParser;
use crate::decode::{Encoding, OneshotDecoder as _, UnknownEncoding, Utf8};

pub use super::JsonParser;

use serde_json::Error as JsonError;

use std::borrow::Cow;
use std::error::Error as StdError;
use std::fmt;

pub struct Json<S, E = Utf8> {
    bytes: S,
    encoding: E,
}

impl<'a> Json<&'a str> {
    pub fn new(json: &'a str) -> Self {
        Self {
            bytes: json,
            encoding: Utf8,
        }
    }
}

impl Json<String> {
    pub fn new(json: String) -> Self {
        Self {
            bytes: json,
            encoding: Utf8,
        }
    }
}

impl<'a> Json<Cow<'a, str>> {
    pub fn new(json: Cow<'a, str>) -> Self {
        Self {
            bytes: json,
            encoding: Utf8,
        }
    }
}

impl IntoParser for Json<&str> {
    type Error = JsonError;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(self.bytes)
    }
}

impl IntoParser for Json<String> {
    type Error = JsonError;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(self.bytes.as_ref())
    }
}

impl IntoParser for Json<Cow<'_, str>> {
    type Error = JsonError;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(self.bytes.as_ref())
    }
}

impl<'a, E: Encoding> Json<&'a [u8], E> {
    pub fn new(json: &'a [u8], encoding: E) -> Self {
        Self {
            bytes: json,
            encoding,
        }
    }
}

impl<E: Encoding> Json<Vec<u8>, E> {
    pub fn new(json: Vec<u8>, encoding: E) -> Self {
        Self {
            bytes: json,
            encoding,
        }
    }
}

impl<'a, E: Encoding> Json<Cow<'a, [u8]>, E> {
    pub fn new(json: Cow<'a, [u8]>, encoding: E) -> Self {
        Self {
            bytes: json,
            encoding,
        }
    }
}

impl<S: AsRef<[u8]>, E: Encoding> Json<S, E> {
    fn parse_bytes(self) -> Result<JsonParser, Error> {
        use std::str::from_utf8;

        let bytes = self.bytes.as_ref();
        let json = if E::UTF8_COMPAT {
            self.encoding.oneshot_decoder().decode(bytes)
        } else {
            let json = from_utf8(bytes).map_err(|_| Error::UnknownEncoding(UnknownEncoding))?;
            if !E::OneshotDecoder::decodable(json) {
                return Err(Error::UnknownEncoding(UnknownEncoding));
            }
            Utf8.oneshot_decoder().decode(bytes)
        }
        .map_err(Error::UnknownEncoding)?;
        JsonParser::from_str(&json).map_err(Error::Json)
    }
}

impl<E: Encoding> IntoParser for Json<&[u8], E> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        self.parse_bytes()
    }
}

impl<E: Encoding> IntoParser for Json<Vec<u8>, E> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        self.parse_bytes()
    }
}

impl<E: Encoding> IntoParser for Json<Cow<'_, [u8]>, E> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        self.parse_bytes()
    }
}

#[derive(Debug)]
pub enum Error {
    Json(JsonError),
    UnknownEncoding(UnknownEncoding),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "could not parse JSON: {e}"),
            Self::UnknownEncoding(e) => write!(f, "could not decode JSON: {e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Json(e) => Some(e),
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::json::tests::{INPUT, collect, expected};

    use super::Json;

    #[tokio::test]
    async fn parses_borrowed_json() {
        let input = Json::<&str>::new(INPUT);
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[tokio::test]
    async fn parses_owned_json() {
        let input = Json::<String>::new(INPUT.to_owned());
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        for json in ["", "[", r#"[{"title":"春"}]"#, "[] []"] {
            assert!(collect(Json::<&str>::new(json)).await.is_err());
            assert!(collect(Json::<String>::new(json.to_owned())).await.is_err());
        }
    }

    #[tokio::test]
    async fn rejects_invalid_document_shapes() {
        for input in [
            "",
            "{}",
            "null",
            "[null]",
            "[{}]",
            r#"[{"title":"春"}]"#,
            r#"[{"body":"春はあけぼの。"}]"#,
            r#"[{"title":1,"body":"text"}]"#,
            r#"[{"id":1,"body":"text"}]"#,
            r#"[{"title":"春","body":null}]"#,
            r#"[{"title":"春","body":"text","author":{}}]"#,
            r#"[{"title":"春","title":"夏","body":"text"}]"#,
            r#"[{"id":"1","id":"2","body":"text"}]"#,
            r#"[{"title":"春","body":"text","body":"other"}]"#,
            r#"[{"title":"春","body":"text"},{}]"#,
            r#"[{"title":"春","body":"text"}] trailing"#,
            r#"[{"title":"春","body":"text"},]"#,
        ] {
            let json = Json::<&str>::new(input);
            assert!(collect(json).await.is_err(), "{input}");
        }
    }
}
