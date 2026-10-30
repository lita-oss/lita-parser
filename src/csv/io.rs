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
