use super::{TxtParser, imp};
use crate::{IntoParser, Metadata, Title};

use std::io::Error as IoError;

use tokio::io::AsyncRead;

#[cfg(feature = "txt-fs")]
use std::path::Path;

#[cfg(feature = "txt-fs")]
use tokio::fs::File;

#[cfg(feature = "decode")]
use crate::decode::{Charset, UnknownEncoding};

#[cfg(feature = "decode")]
use std::{error::Error as StdError, fmt};

pub struct Txt<R> {
    imp: imp::Txt<R>,
}

impl<R: AsyncRead> Txt<R> {
    pub fn from(txt: R, title: Title, metadata: Option<Metadata>) -> Self {
        Self {
            imp: imp::Txt {
                bytes: txt,
                title,
                metadata,
            },
        }
    }
}

impl<R: AsyncRead + Unpin> IntoParser for Txt<R> {
    type Error = IoError;
    type Parser = TxtParser;

    async fn parser(mut self) -> Result<Self::Parser, Self::Error> {
        use tokio::io::AsyncReadExt as _;

        let mut buf = String::new();
        if let Err(e) = self.imp.bytes.read_to_string(&mut buf).await {
            Err(e)
        } else {
            Ok(TxtParser {
                imp: Some(imp::Txt {
                    bytes: buf,
                    title: self.imp.title,
                    metadata: self.imp.metadata,
                }),
            })
        }
    }
}

#[cfg(feature = "txt-fs")]
impl Txt<File> {
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
        title: Title,
        metadata: Option<Metadata>,
    ) -> Result<Self, IoError> {
        File::open(path)
            .await
            .map(|f| Self::from(f, title, metadata))
    }
}

#[cfg(feature = "decode")]
pub struct TxtWithCharset<R> {
    imp: imp::Txt<R>,
    charset: Charset,
}

#[cfg(feature = "decode")]
impl<R: AsyncRead> TxtWithCharset<R> {
    pub fn from(txt: R, title: Title, metadata: Option<Metadata>, charset: Charset) -> Self {
        Self {
            imp: imp::Txt {
                bytes: txt,
                title,
                metadata,
            },
            charset,
        }
    }
}

#[cfg(feature = "decode")]
#[derive(Debug)]
pub enum ErrorWithCharset {
    Io(IoError),
    UnknownEncoding(UnknownEncoding),
}

#[cfg(feature = "decode")]
impl<R: AsyncRead + Unpin> IntoParser for TxtWithCharset<R> {
    type Error = ErrorWithCharset;
    type Parser = TxtParser;

    async fn parser(mut self) -> Result<Self::Parser, Self::Error> {
        use tokio::io::AsyncReadExt as _;

        let mut buf = Vec::new();
        if let Err(e) = self.imp.bytes.read_to_end(&mut buf).await {
            Err(ErrorWithCharset::Io(e))
        } else {
            let imp = imp::Txt {
                bytes: buf,
                title: self.imp.title,
                metadata: self.imp.metadata,
            }
            .decode(self.charset)
            .map_err(ErrorWithCharset::UnknownEncoding)?;
            Ok(TxtParser { imp: Some(imp) })
        }
    }
}

#[cfg(all(feature = "txt-fs", feature = "decode"))]
impl TxtWithCharset<File> {
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
        title: Title,
        metadata: Option<Metadata>,
        charset: Charset,
    ) -> Result<Self, IoError> {
        File::open(path)
            .await
            .map(|f| Self::from(f, title, metadata, charset))
    }
}

#[cfg(feature = "decode")]
impl fmt::Display for ErrorWithCharset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error when reading text: {e}"),
            Self::UnknownEncoding(e) => write!(f, "could not decode text: {e}"),
        }
    }
}
#[cfg(feature = "decode")]
impl StdError for ErrorWithCharset {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}
