use super::{TxtParser, imp};
use crate::{IntoParser, Metadata, Title};

use std::io::Error;

use tokio::io::AsyncRead;

#[cfg(feature = "txt-fs")]
use std::path::Path;

#[cfg(feature = "txt-fs")]
use tokio::fs::File;

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
    type Error = Error;
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
    ) -> Result<Self, Error> {
        File::open(path)
            .await
            .map(|f| Self::from(f, title, metadata))
    }
}
