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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::txt::tests::collect;
    use crate::{Body, ParsedDoc};

    fn expected(text: &str) -> Vec<ParsedDoc> {
        vec![ParsedDoc {
            title: Title("title".to_owned()),
            body: Body(text.to_owned()),
            metadata: Some(Metadata([("key".to_owned(), "value".to_owned())].into())),
        }]
    }

    fn metadata() -> Option<Metadata> {
        Some(Metadata([("key".to_owned(), "value".to_owned())].into()))
    }

    #[tokio::test]
    async fn parses_async_read() {
        for text in ["春はあけぼの。\nSecond line", ""] {
            let source = Txt::from(text.as_bytes(), Title("title".to_owned()), metadata());
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[tokio::test]
    async fn rejects_invalid_utf8() {
        let source = Txt::from(&b"valid\xff"[..], Title("title".to_owned()), metadata());
        assert!(matches!(
            source.parser().await,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn decodes_async_read() {
        for text in ["春はあけぼの。", ""] {
            let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode(text);
            assert!(!had_errors);
            let source = TxtWithCharset::from(
                bytes.as_ref(),
                Title("title".to_owned()),
                metadata(),
                Charset::ShiftJis,
            );
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn rejects_unknown_async_read_encoding() {
        let source = TxtWithCharset::from(
            &b"\xff"[..],
            Title("title".to_owned()),
            metadata(),
            Charset::Dynamic,
        );
        assert!(matches!(
            source.parser().await,
            Err(ErrorWithCharset::UnknownEncoding(_))
        ));
    }

    #[cfg(feature = "txt-fs")]
    #[tokio::test]
    async fn parses_files() {
        use std::io::Write;

        for text in ["春はあけぼの。\nSecond line", ""] {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(text.as_bytes()).unwrap();
            let source = Txt::from_path(file.path(), Title("title".to_owned()), metadata())
                .await
                .unwrap();
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[cfg(feature = "txt-fs")]
    #[tokio::test]
    async fn rejects_invalid_utf8_files() {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"valid\xff").unwrap();
        let source = Txt::from_path(file.path(), Title("title".to_owned()), metadata())
            .await
            .unwrap();
        assert!(matches!(
            source.parser().await,
            Err(error) if error.kind() == std::io::ErrorKind::InvalidData
        ));
    }

    #[cfg(feature = "txt-fs")]
    #[tokio::test]
    async fn rejects_missing_files() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        file.close().unwrap();
        assert!(matches!(
            Txt::from_path(&path, Title("title".to_owned()), metadata()).await,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
        #[cfg(feature = "decode")]
        assert!(matches!(
            TxtWithCharset::from_path(&path, Title("title".to_owned()), metadata(), Charset::Dynamic).await,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
    }

    #[cfg(all(feature = "txt-fs", feature = "decode"))]
    #[tokio::test]
    async fn decodes_files() {
        use std::io::Write;

        for text in ["春はあけぼの。", ""] {
            let (bytes, _, had_errors) = encoding_rs::SHIFT_JIS.encode(text);
            assert!(!had_errors);
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(&bytes).unwrap();
            let source = TxtWithCharset::from_path(
                file.path(),
                Title("title".to_owned()),
                metadata(),
                Charset::ShiftJis,
            )
            .await
            .unwrap();
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[cfg(all(feature = "txt-fs", feature = "decode"))]
    #[tokio::test]
    async fn rejects_unknown_file_encoding() {
        use std::io::Write;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"\xff").unwrap();
        let source = TxtWithCharset::from_path(
            file.path(),
            Title("title".to_owned()),
            metadata(),
            Charset::Dynamic,
        )
        .await
        .unwrap();
        assert!(matches!(
            source.parser().await,
            Err(ErrorWithCharset::UnknownEncoding(_))
        ));
    }
}
