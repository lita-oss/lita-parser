use super::JsonParser;
use crate::IntoParser;

use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;

use tokio::io::AsyncRead;

use serde_json::Error as JsonError;

#[cfg(feature = "json-fs")]
use std::path::Path;

#[cfg(feature = "json-fs")]
use tokio::fs::File;

pub struct Json<R> {
    reader: R,
}

impl<R: AsyncRead> Json<R> {
    pub fn from(json: R) -> Self {
        Self { reader: json }
    }
}

impl<R: AsyncRead + Unpin> IntoParser for Json<R> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(mut self) -> Result<Self::Parser, Self::Error> {
        use tokio::io::AsyncReadExt as _;

        let mut buf = String::new();
        if let Err(e) = self.reader.read_to_string(&mut buf).await {
            Err(Error::Io(e))
        } else {
            JsonParser::from_str(&buf).map_err(Error::Json)
        }
    }
}

#[cfg(feature = "json-fs")]
impl Json<File> {
    pub async fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, Error> {
        File::open(path).await.map(Self::from).map_err(Error::Io)
    }
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Json(JsonError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error when deserializing JSON: {e}"),
            Self::Json(e) => write!(f, "could not parse JSON: {e}"),
        }
    }
}
impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Error, Json};
    use crate::json::tests::{INPUT, collect, expected};
    use std::io::ErrorKind;

    #[tokio::test]
    async fn parses_async_reader() {
        let input = Json::from(INPUT.as_bytes());
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        for json in ["", "[", r#"[{"title":"春"}]"#, "[] []"] {
            let input = Json::from(json.as_bytes());
            assert!(matches!(collect(input).await, Err(Error::Json(_))));
        }
    }

    #[tokio::test]
    async fn rejects_invalid_utf8() {
        let input = Json::from(&b"[\xff]"[..]);
        assert!(matches!(
            collect(input).await,
            Err(Error::Io(error)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "json-fs")]
    #[tokio::test]
    async fn parses_file() {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(INPUT.as_bytes()).unwrap();
        let input = Json::from_path(file.path()).await.unwrap();
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[cfg(feature = "json-fs")]
    #[tokio::test]
    async fn rejects_invalid_file_contents() {
        use std::io::Write;

        for json in ["", "[", r#"[{"title":"春"}]"#, "[] []"] {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(json.as_bytes()).unwrap();
            let input = Json::from_path(file.path()).await.unwrap();
            assert!(matches!(collect(input).await, Err(Error::Json(_))));
        }

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"[\xff]").unwrap();
        let input = Json::from_path(file.path()).await.unwrap();
        assert!(matches!(
            collect(input).await,
            Err(Error::Io(error)) if error.kind() == ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "json-fs")]
    #[tokio::test]
    async fn rejects_missing_file() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        file.close().unwrap();
        assert!(matches!(
            Json::from_path(&path).await,
            Err(Error::Io(error)) if error.kind() == ErrorKind::NotFound
        ));
    }
}
