use super::DocCsv;
use crate::{IntoParser, ParsedDoc, Parser};

use std::borrow::Cow;
use std::convert::Infallible;
use std::io::{Cursor, Read};

use csv::DeserializeRecordsIntoIter;
use csv::Error;

#[cfg(feature = "decode")]
use crate::decode::{Charset, UnknownEncoding, decode};

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

impl<S> Csv<S> {
    pub fn from(csv: S) -> Self {
        Self { bytes: csv }
    }
}

impl IntoParser for Csv<String> {
    type Error = Infallible;
    type Parser = CsvParser<Cursor<Vec<u8>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(make_parser(Cursor::new(self.bytes.into_bytes())))
    }
}

impl<'a> IntoParser for Csv<&'a str> {
    type Error = Infallible;
    type Parser = CsvParser<&'a [u8]>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(make_parser(self.bytes.as_bytes()))
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
        Ok(make_parser(Cursor::new(bytes)))
    }
}

#[cfg(feature = "decode")]
pub struct CsvWithCharset<S> {
    bytes: S,
    charset: Charset,
}

#[cfg(feature = "decode")]
impl<S> CsvWithCharset<S> {
    pub fn from(csv: S, charset: Charset) -> Self {
        Self {
            bytes: csv,
            charset,
        }
    }
}

#[cfg(feature = "decode")]
impl IntoParser for CsvWithCharset<Vec<u8>> {
    type Error = UnknownEncoding;
    type Parser = CsvParser<Cursor<Vec<u8>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let bytes = decode(&self.bytes, self.charset)?.into_owned().into_bytes();
        Ok(make_parser(Cursor::new(bytes)))
    }
}

#[cfg(feature = "decode")]
impl<'a> IntoParser for CsvWithCharset<&'a [u8]> {
    type Error = UnknownEncoding;
    type Parser = CsvParser<Cursor<Cow<'a, [u8]>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let bytes = match decode(self.bytes, self.charset)? {
            Cow::Borrowed(v) => Cow::Borrowed(v.as_bytes()),
            Cow::Owned(v) => Cow::Owned(v.into_bytes()),
        };
        Ok(make_parser(Cursor::new(bytes)))
    }
}

#[cfg(feature = "decode")]
impl<'a> IntoParser for CsvWithCharset<Cow<'a, [u8]>> {
    type Error = UnknownEncoding;
    type Parser = CsvParser<Cursor<Cow<'a, [u8]>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        match self.bytes {
            Cow::Borrowed(v) => {
                let bytes = match decode(v, self.charset)? {
                    Cow::Borrowed(v) => Cow::Borrowed(v.as_bytes()),
                    Cow::Owned(v) => Cow::Owned(v.into_bytes()),
                };
                Ok(make_parser(Cursor::new(bytes)))
            }
            Cow::Owned(v) => {
                let bytes = decode(&v, self.charset)?.into_owned().into_bytes();
                Ok(make_parser(Cursor::new(Cow::Owned(bytes))))
            }
        }
    }
}

fn make_parser<R: Read>(rdr: R) -> CsvParser<R> {
    let rdr = csv::Reader::from_reader(rdr);
    let rec = rdr.into_deserialize();
    CsvParser { rec }
}
