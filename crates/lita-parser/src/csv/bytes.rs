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
    pub(crate) bytes: S,
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
    pub(crate) bytes: S,
    pub(crate) charset: Charset,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::csv::tests::{INVALID, VALID, collect, doc, expected};

    #[tokio::test]
    async fn parses_borrowed_and_owned_strings() {
        assert_eq!(collect(Csv::from(VALID)).await, (expected(), false));
        assert_eq!(
            collect(Csv::from(VALID.to_owned())).await,
            (expected(), false)
        );
    }

    #[tokio::test]
    async fn stops_at_invalid_records_in_borrowed_and_owned_strings() {
        let expected = vec![doc("春", "あけぼの。", &[])];
        assert_eq!(collect(Csv::from(INVALID)).await, (expected.clone(), true));
        assert_eq!(
            collect(Csv::from(INVALID.to_owned())).await,
            (expected, true)
        );
    }

    #[cfg(feature = "decode")]
    fn utf16_be(input: &'static str) -> Vec<u8> {
        let mut units = vec![0; input.len()];
        let written = encoding_rs::mem::convert_str_to_utf16(input, &mut units);
        units[..written]
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect()
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn decodes_utf16_be_bytes() {
        let input = utf16_be(VALID);
        assert_eq!(
            collect(CsvWithCharset::from(input.as_slice(), Charset::Utf16Be)).await,
            (expected(), false)
        );
        assert_eq!(
            collect(CsvWithCharset::from(input.clone(), Charset::Utf16Be)).await,
            (expected(), false)
        );
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn stops_at_invalid_utf16_be_csv_records() {
        let input = utf16_be(INVALID);
        let expected = vec![doc("春", "あけぼの。", &[])];
        assert_eq!(
            collect(CsvWithCharset::from(input.as_slice(), Charset::Utf16Be)).await,
            (expected.clone(), true)
        );
        assert_eq!(
            collect(CsvWithCharset::from(input.clone(), Charset::Utf16Be)).await,
            (expected.clone(), true)
        );
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn replaces_malformed_utf16_be_sequences() {
        let mut input = utf16_be("title,body\n春,あけぼの。");
        input.extend_from_slice(&[0xd8, 0x00]);
        assert_eq!(
            collect(CsvWithCharset::from(input.as_slice(), Charset::Utf16Be)).await,
            (vec![doc("春", "あけぼの。�", &[])], false)
        );
        assert_eq!(
            collect(CsvWithCharset::from(input, Charset::Utf16Be)).await,
            (vec![doc("春", "あけぼの。�", &[])], false)
        );
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn rejects_unknown_encodings() {
        let input = [0xff];
        assert!(
            CsvWithCharset::from(input.as_slice(), Charset::Dynamic)
                .parser()
                .await
                .is_err()
        );
        assert!(
            CsvWithCharset::from(input.to_vec(), Charset::Dynamic)
                .parser()
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn parses_title_id_and_metadata() {
        for (input, expected) in [
            (
                "title,body\n春,あけぼの。\n",
                vec![doc("春", "あけぼの。", &[])],
            ),
            ("id,body\n1,あけぼの。\n", vec![doc("1", "あけぼの。", &[])]),
            (
                "body,title,id,source\nあけぼの。,春,1,枕草子\n",
                vec![doc(
                    "1",
                    "あけぼの。",
                    &[("title", "春"), ("source", "枕草子")],
                )],
            ),
            (
                "title,body,source\n,,\n",
                vec![doc("", "", &[("source", "")])],
            ),
            ("title,body\n", vec![]),
            ("", vec![]),
        ] {
            let csv = Csv::from(input);
            assert_eq!(collect(csv).await, (expected, false), "{input}");
        }
    }

    #[tokio::test]
    async fn rejects_missing_and_duplicate_required_fields() {
        for input in [
            "body\nあけぼの。\n",
            "title\n春\n",
            "id\n1\n",
            "title,title,body\n春,夏,あけぼの。\n",
            "id,id,body\n1,2,あけぼの。\n",
            "title,body,body\n春,あけぼの。,夜\n",
        ] {
            let csv = Csv::from(input);
            assert_eq!(collect(csv).await, (vec![], true), "{input}");
        }
    }
}
