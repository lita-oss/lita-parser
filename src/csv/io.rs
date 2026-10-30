use super::DocCsv;
use crate::{IntoParser, ParsedDoc, Parser};

use std::convert::Infallible;

use tokio::io::AsyncRead;

use csv_async::Error;
use csv_async::{AsyncDeserializer, StringRecord};

#[cfg(feature = "csv-fs")]
use std::path::Path;

#[cfg(feature = "csv-fs")]
use tokio::fs::File;

pub struct CsvParser<R> {
    rdr: AsyncDeserializer<R>,
    rec: StringRecord,
}

impl<R: AsyncRead + Unpin + Send> Parser for CsvParser<R> {
    type Error = Error;

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        match self.rdr.read_record(&mut self.rec).await {
            Ok(true) => match self.rdr.headers().await {
                Ok(headers) => Some(
                    self.rec
                        .deserialize::<DocCsv>(Some(headers))
                        .map(DocCsv::into_parsed),
                ),
                Err(e) => Some(Err(e)),
            },
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

pub struct Csv<R> {
    rdr: R,
}

impl<R: AsyncRead> Csv<R> {
    pub fn from(csv: R) -> Self {
        Self { rdr: csv }
    }
}

impl<R: AsyncRead + Unpin + Send> IntoParser for Csv<R> {
    type Error = Infallible;
    type Parser = CsvParser<R>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(CsvParser {
            rdr: AsyncDeserializer::from_reader(self.rdr),
            rec: StringRecord::new(),
        })
    }
}

#[cfg(feature = "csv-fs")]
impl Csv<File> {
    pub async fn from_path<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        File::open(path).await.map(Self::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::tests::{INVALID, VALID, collect, doc, expected};

    #[tokio::test]
    async fn parses_async_reader() {
        assert_eq!(
            collect(Csv::from(VALID.as_bytes())).await,
            (expected(), false)
        );
        assert_eq!(collect(Csv::from(b"".as_slice())).await, (vec![], false));
    }

    #[tokio::test]
    async fn stops_at_invalid_records() {
        assert_eq!(
            collect(Csv::from(INVALID.as_bytes())).await,
            (vec![doc("春", "あけぼの。", &[])], true)
        );
    }

    #[tokio::test]
    async fn rejects_invalid_utf8() {
        let mut input = b"title,body\nvalid,body\ninvalid,".to_vec();
        input.push(0xff);
        assert_eq!(
            collect(Csv::from(input.as_slice())).await,
            (vec![doc("valid", "body", &[])], true)
        );
    }

    #[cfg(feature = "csv-fs")]
    #[tokio::test]
    async fn parses_files() {
        use std::io::Write as _;

        for input in [VALID, ""] {
            let mut file = tempfile::NamedTempFile::new().unwrap();
            file.write_all(input.as_bytes()).unwrap();
            let source = Csv::from_path(file.path()).await.unwrap();
            let expected = if input.is_empty() { vec![] } else { expected() };
            assert_eq!(collect(source).await, (expected, false));
        }
    }

    #[cfg(feature = "csv-fs")]
    #[tokio::test]
    async fn stops_at_invalid_file_records() {
        use std::io::Write as _;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(INVALID.as_bytes()).unwrap();
        let source = Csv::from_path(file.path()).await.unwrap();
        assert_eq!(
            collect(source).await,
            (vec![doc("春", "あけぼの。", &[])], true)
        );
    }

    #[cfg(feature = "csv-fs")]
    #[tokio::test]
    async fn rejects_invalid_utf8_in_files() {
        use std::io::Write as _;

        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"title,body\nvalid,body\ninvalid,\xff")
            .unwrap();
        let source = Csv::from_path(file.path()).await.unwrap();
        assert_eq!(
            collect(source).await,
            (vec![doc("valid", "body", &[])], true)
        );
    }

    #[cfg(feature = "csv-fs")]
    #[tokio::test]
    async fn rejects_missing_files() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let path = file.path().to_owned();
        file.close().unwrap();
        assert!(matches!(
            Csv::from_path(&path).await,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound
        ));
    }
}
