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
