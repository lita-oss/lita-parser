use super::DocCsv;
use crate::{IntoParser, ParsedDoc, Parser};

use std::borrow::Cow;
use std::convert::Infallible;
use std::io::{Cursor, Read};

use csv::DeserializeRecordsIntoIter;
use csv::Error;

pub struct CsvParser<R> {
    rec: DeserializeRecordsIntoIter<R, DocCsv>,
}

impl<R: Read> Parser for CsvParser<R> {
    type Error = Error;

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        self.rec.next().map(|res| res.map(DocCsv::into_parsed))
    }
}

pub struct Csv<S> {
    bytes: S,
}

impl Csv<String> {
    pub fn from(csv: String) -> Self {
        Self { bytes: csv }
    }
}

impl IntoParser for Csv<String> {
    type Error = Infallible;
    type Parser = CsvParser<Cursor<Vec<u8>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        make_parser(Cursor::new(self.bytes.into_bytes()))
    }
}

impl<'a> IntoParser for Csv<&'a str> {
    type Error = Infallible;
    type Parser = CsvParser<&'a [u8]>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        make_parser(self.bytes.as_bytes())
    }
}

impl<'a> IntoParser for Csv<Cow<'a, str>> {
    type Error = Infallible;
    type Parser = CsvParser<Cursor<Cow<'a, [u8]>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let bytes = match self.bytes {
            Cow::Borrowed(v) => Cow::Borrowed(v.as_bytes()),
            Cow::Owned(v) => Cow::Owned(v.into_bytes()),
        };
        make_parser(Cursor::new(bytes))
    }
}

fn make_parser<R: Read>(rdr: R) -> Result<CsvParser<R>, Infallible> {
    let rdr = csv::Reader::from_reader(rdr);
    let rec = rdr.into_deserialize();
    Ok(CsvParser { rec })
}
