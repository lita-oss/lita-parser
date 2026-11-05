#[cfg(feature = "decode")]
use crate::decode::UnknownEncoding;
use crate::decode::{Encoding, EncodingTokioExt as _, StreamDecoder, StreamEncoding, Utf8};
use crate::{IntoParser, ParsedDoc, Parser};

use super::de::DocCsv;

use csv_async::{AsyncDeserializer, Error as CsvError, StringRecord};

#[cfg(feature = "fs")]
use tokio::fs::File;
use tokio::io::AsyncRead;

use std::error::Error as StdError;
use std::fmt;

#[cfg(feature = "fs")]
use std::{io::Error as IoError, path::Path};

pub struct CsvParser<R> {
    rdr: AsyncDeserializer<R>,
    rec: StringRecord,
}

impl<R: AsyncRead + Unpin + Send> Parser for CsvParser<R> {
    type Error = CsvError;

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        async {
            if !self.rdr.read_record(&mut self.rec).await? {
                return Ok(None);
            }

            let headers = self.rdr.headers().await?;
            self.rec
                .deserialize::<DocCsv>(Some(headers))
                .map(DocCsv::into_parsed)
                .map(Some)
        }
        .await
        .transpose()
    }
}

pub struct Csv<R, E = Utf8> {
    rdr: R,
    encoding: E,
}

impl<R: AsyncRead, E: Encoding> Csv<R, E> {
    pub fn new(rdr: R, encoding: E) -> Self {
        Self { rdr, encoding }
    }
}

impl<R: AsyncRead + Unpin + Send, E: StreamEncoding> IntoParser for Csv<R, E>
where
    StreamDecoder<R, E::MakeStreamDecoder>: Send,
{
    type Error = Error;
    type Parser = CsvParser<StreamDecoder<R, E::MakeStreamDecoder>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(CsvParser {
            rdr: AsyncDeserializer::from_reader(self.encoding.stream_decoder(self.rdr)),
            rec: StringRecord::new(),
        })
    }
}

#[derive(Debug)]
pub enum Error {
    Csv(CsvError),
    #[cfg(feature = "decode")]
    UnknownEncoding(UnknownEncoding),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Csv(e) => write!(f, "could not parse CSV: {e}"),
            #[cfg(feature = "decode")]
            Self::UnknownEncoding(e) => write!(f, "could not decode CSV: {e}"),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Csv(e) => Some(e),
            #[cfg(feature = "decode")]
            Self::UnknownEncoding(e) => Some(e),
        }
    }
}

#[cfg(feature = "fs")]
impl<E: Encoding> Csv<File, E> {
    pub async fn from_path<P: AsRef<Path>>(path: P, encoding: E) -> Result<Self, IoError> {
        File::open(path).await.map(|rdr| Self::new(rdr, encoding))
    }
}

#[cfg(test)]
mod tests {
    use crate::csv::tests::{INVALID, VALID, collect, doc, expected};
    use crate::decode::Utf8;

    use super::Csv;

    #[cfg(feature = "fs")]
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn parses_async_reader() {
        assert_eq!(
            collect(Csv::new(VALID.as_bytes(), Utf8)).await,
            (expected(), false)
        );
        assert_eq!(
            collect(Csv::new(b"".as_slice(), Utf8)).await,
            (vec![], false)
        );
    }

    #[tokio::test]
    async fn stops_at_invalid_records() {
        assert_eq!(
            collect(Csv::new(INVALID.as_bytes(), Utf8)).await,
            (vec![doc("春", "あけぼの。", &[])], true)
        );
    }

    #[tokio::test]
    async fn rejects_invalid_utf8() {
        let mut input = b"title,body\nvalid,body\ninvalid,".to_vec();
        input.push(0xff);
        assert_eq!(
            collect(Csv::new(input.as_slice(), Utf8)).await,
            (vec![doc("valid", "body", &[])], true)
        );
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn parses_files() {
        use std::io::Write as _;

        for input in [VALID, ""] {
            let mut file = NamedTempFile::new().unwrap();
            file.write_all(input.as_bytes()).unwrap();
            let source = Csv::from_path(file.path(), Utf8).await.unwrap();
            let expected = if input.is_empty() { vec![] } else { expected() };
            assert_eq!(collect(source).await, (expected, false));
        }
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn stops_at_invalid_file_records() {
        use std::io::Write as _;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(INVALID.as_bytes()).unwrap();
        let source = Csv::from_path(file.path(), Utf8).await.unwrap();
        assert_eq!(
            collect(source).await,
            (vec![doc("春", "あけぼの。", &[])], true)
        );
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_invalid_utf8_in_files() {
        use std::io::Write as _;

        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"title,body\nvalid,body\ninvalid,\xff")
            .unwrap();
        let source = Csv::from_path(file.path(), Utf8).await.unwrap();
        assert_eq!(
            collect(source).await,
            (vec![doc("valid", "body", &[])], true)
        );
    }

    #[cfg(feature = "fs")]
    #[tokio::test]
    async fn rejects_missing_files() {
        use std::io::ErrorKind;

        let file = NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        file.close().unwrap();
        assert!(matches!(
            Csv::from_path(&path, Utf8).await,
            Err(error) if error.kind() == ErrorKind::NotFound
        ));
    }
}
