use crate::IntoParser;
use crate::decode::{Encoding, EncodingTokioExt as _, StreamEncoding, UnknownEncoding, Utf8};

pub use super::JsonParser;

use serde_json::Error as JsonError;

#[cfg(feature = "fs")]
use tokio::fs::File;
use tokio::io::AsyncRead;

use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;
#[cfg(feature = "fs")]
use std::path::Path;

pub struct Json<R, E = Utf8> {
    rdr: R,
    encoding: E,
}

impl<R: AsyncRead, E: Encoding> Json<R, E> {
    pub fn new(rdr: R, encoding: E) -> Self {
        Self { rdr, encoding }
    }
}

impl<R: AsyncRead + Unpin, E: StreamEncoding> IntoParser for Json<R, E> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        use tokio::io::AsyncReadExt as _;

        if !E::UTF8_INTERSECT {
            return Err(Error::UnknownEncoding(UnknownEncoding));
        }

        let mut rdr = self.encoding.stream_decoder_utf8(self.rdr);
        let mut json = String::new();
        rdr.read_to_string(&mut json).await.map_err(Error::Io)?;
        JsonParser::from_str(&json).map_err(Error::Json)
    }
}

#[cfg(feature = "fs")]
impl<E: Encoding> Json<File, E> {
    pub async fn from_path<P: AsRef<Path>>(path: P, encoding: E) -> Result<Self, IoError> {
        File::open(path).await.map(|f| Self::new(f, encoding))
    }
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    Json(JsonError),
    UnknownEncoding(UnknownEncoding),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error when reading JSON: {e}"),
            Self::Json(e) => write!(f, "could not parse JSON: {e}"),
            Self::UnknownEncoding(e) => write!(f, "could not decode JSON: {e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Json(e) => Some(e),
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Utf8;
    use crate::json::tests::{INPUT, collect, expected};

    use super::{Error, Json};

    #[cfg(feature = "fs")]
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn parses_async_reader() {
        let input = Json::new(INPUT.as_bytes(), Utf8);
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        for json in ["", "[", r#"[{"title":"春"}]"#, "[] []"] {
            let input = Json::new(json.as_bytes(), Utf8);
            assert!(matches!(collect(input).await, Err(Error::Json(_))));
        }
    }

    #[tokio::test]
    async fn rejects_invalid_utf8() {
        let input = Json::new(&b"[\xff]"[..], Utf8);
        assert!(matches!(collect(input).await, Err(Error::Io(_))));
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn parses_file() {
        use std::io::Write as _;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(INPUT.as_bytes()).unwrap();
        let input = Json::from_path(file.path(), Utf8).await.unwrap();
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_invalid_file_contents() {
        use std::io::Write as _;

        for json in ["", "[", r#"[{"title":"春"}]"#, "[] []"] {
            let mut file = NamedTempFile::new().unwrap();
            file.write_all(json.as_bytes()).unwrap();
            let input = Json::from_path(file.path(), Utf8).await.unwrap();
            assert!(matches!(collect(input).await, Err(Error::Json(_))));
        }

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"[\xff]").unwrap();
        let input = Json::from_path(file.path(), Utf8).await.unwrap();
        assert!(matches!(collect(input).await, Err(Error::Io(_))));
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_missing_file() {
        use std::io::ErrorKind;

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        file.close().unwrap();
        assert!(matches!(
            Json::from_path(&path, Utf8).await,
            Err(error) if error.kind() == ErrorKind::NotFound
        ));
    }
}
