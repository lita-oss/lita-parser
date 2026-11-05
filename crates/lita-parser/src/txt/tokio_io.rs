use crate::decode::{Encoding, EncodingTokioExt as _, StreamEncoding, UnknownEncoding, Utf8};
use crate::{IntoParser, Metadata, Title};

pub use super::TxtParser;

#[cfg(feature = "fs")]
use tokio::fs::File;
use tokio::io::AsyncRead;

use std::error::Error as StdError;
use std::fmt;
use std::io::Error as IoError;
#[cfg(feature = "fs")]
use std::path::Path;

pub struct Txt<R, E = Utf8> {
    rdr: R,
    title: Title,
    metadata: Option<Metadata>,
    encoding: E,
}

impl<R: AsyncRead, E: Encoding> Txt<R, E> {
    pub fn new(rdr: R, title: Title, metadata: Option<Metadata>, encoding: E) -> Self {
        Self {
            rdr,
            title,
            metadata,
            encoding,
        }
    }
}

impl<R: AsyncRead + Unpin, E: StreamEncoding> IntoParser for Txt<R, E> {
    type Error = Error;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        use tokio::io::AsyncReadExt as _;

        let mut rdr = self.encoding.stream_decoder(self.rdr);
        let mut bytes = String::new();
        rdr.read_to_string(&mut bytes).await.map_err(Error::Io)?;
        Ok(TxtParser::new(bytes, self.title, self.metadata))
    }
}

#[cfg(feature = "fs")]
impl<E: Encoding> Txt<File, E> {
    pub async fn from_path<P: AsRef<Path>>(
        path: P,
        title: Title,
        metadata: Option<Metadata>,
        encoding: E,
    ) -> Result<Self, IoError> {
        File::open(path)
            .await
            .map(|f| Self::new(f, title, metadata, encoding))
    }
}

#[derive(Debug)]
pub enum Error {
    Io(IoError),
    UnknownEncoding(UnknownEncoding),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io error when reading text: {e}"),
            Self::UnknownEncoding(e) => write!(f, "could not decode text: {e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "decode")]
    use crate::decode::{Dynamic, ShiftJis};
    use crate::txt::tests::collect;
    use crate::{Body, ParsedDoc};

    use super::*;

    #[cfg(feature = "decode")]
    use encoding_rs::SHIFT_JIS;

    #[cfg(feature = "fs")]
    use tempfile::NamedTempFile;

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
            let source = Txt::new(text.as_bytes(), Title("title".to_owned()), metadata(), Utf8);
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[tokio::test]
    async fn rejects_invalid_utf8() {
        let source = Txt::new(
            &b"valid\xff"[..],
            Title("title".to_owned()),
            metadata(),
            Utf8,
        );
        assert!(matches!(source.parser().await, Err(Error::Io(_))));
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn decodes_async_read() {
        for text in ["春はあけぼの。", ""] {
            let (bytes, _, had_errors) = SHIFT_JIS.encode(text);
            assert!(!had_errors);
            let source = Txt::new(
                bytes.as_ref(),
                Title("title".to_owned()),
                metadata(),
                ShiftJis,
            );
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn rejects_unknown_async_read_encoding() {
        let source = Txt::new(&b"\xff"[..], Title("title".to_owned()), metadata(), Dynamic);
        assert!(matches!(source.parser().await, Err(Error::Io(_))));
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn parses_files() {
        use std::io::Write as _;

        for text in ["春はあけぼの。\nSecond line", ""] {
            let mut file = NamedTempFile::new().unwrap();
            file.write_all(text.as_bytes()).unwrap();
            let source = Txt::from_path(file.path(), Title("title".to_owned()), metadata(), Utf8)
                .await
                .unwrap();
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_invalid_utf8_files() {
        use std::io::Write as _;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"valid\xff").unwrap();
        let source = Txt::from_path(file.path(), Title("title".to_owned()), metadata(), Utf8)
            .await
            .unwrap();
        assert!(matches!(source.parser().await, Err(Error::Io(_))));
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_missing_files() {
        use std::io::ErrorKind;

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        file.close().unwrap();
        assert!(matches!(
            Txt::from_path(&path, Title("title".to_owned()), metadata(), Utf8).await,
            Err(error) if error.kind() == ErrorKind::NotFound
        ));
        #[cfg(feature = "decode")]
        assert!(matches!(
            Txt::from_path(&path, Title("title".to_owned()), metadata(), Dynamic).await,
            Err(error) if error.kind() == ErrorKind::NotFound
        ));
    }

    #[cfg(all(feature = "fs", feature = "decode"))]
    #[tokio::test]
    async fn decodes_files() {
        use std::io::Write as _;

        for text in ["春はあけぼの。", ""] {
            let (bytes, _, had_errors) = SHIFT_JIS.encode(text);
            assert!(!had_errors);
            let mut file = NamedTempFile::new().unwrap();
            file.write_all(&bytes).unwrap();
            let source =
                Txt::from_path(file.path(), Title("title".to_owned()), metadata(), ShiftJis)
                    .await
                    .unwrap();
            assert_eq!(collect(source).await, expected(text));
        }
    }

    #[cfg(all(feature = "fs", feature = "decode"))]
    #[tokio::test]
    async fn rejects_unknown_file_encoding() {
        use std::io::Write as _;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"\xff").unwrap();
        let source = Txt::from_path(file.path(), Title("title".to_owned()), metadata(), Dynamic)
            .await
            .unwrap();
        assert!(matches!(source.parser().await, Err(Error::Io(_))));
    }
}
