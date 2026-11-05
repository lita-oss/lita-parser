use crate::decode::{Encoding, OneshotDecoder as _, UnknownEncoding, Utf8};
use crate::{IntoParser, ParsedDoc, Parser};

use super::de::DocCsv;

use csv::{DeserializeRecordsIntoIter, Error as CsvError};

use std::borrow::Cow;
use std::convert::Infallible;
use std::io::{Cursor, Read};

pub struct CsvParser<R> {
    rec: DeserializeRecordsIntoIter<R, DocCsv>,
}

impl<R: Read> Parser for CsvParser<R> {
    type Error = CsvError;

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        self.rec.next().map(|res| res.map(DocCsv::into_parsed))
    }
}

pub struct Csv<S, E = Utf8> {
    bytes: S,
    encoding: E,
}

impl Csv<String> {
    pub fn new(bytes: String) -> Self {
        Self {
            bytes,
            encoding: Utf8,
        }
    }
}

impl<'a> Csv<&'a str> {
    pub fn new(bytes: &'a str) -> Self {
        Self {
            bytes,
            encoding: Utf8,
        }
    }
}

impl<'a> Csv<Cow<'a, str>> {
    pub fn new(bytes: Cow<'a, str>) -> Self {
        Self {
            bytes,
            encoding: Utf8,
        }
    }
}

impl<E: Encoding> Csv<Vec<u8>, E> {
    pub fn new(bytes: Vec<u8>, encoding: E) -> Self {
        Self { bytes, encoding }
    }
}

impl<'a, E: Encoding> Csv<&'a [u8], E> {
    pub fn new(bytes: &'a [u8], encoding: E) -> Self {
        Self { bytes, encoding }
    }
}

impl<'a, E: Encoding> Csv<Cow<'a, [u8]>, E> {
    pub fn new(bytes: Cow<'a, [u8]>, encoding: E) -> Self {
        Self { bytes, encoding }
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
        Ok(make_parser(Cursor::new(into_bytes(self.bytes))))
    }
}

impl<E: Encoding> IntoParser for Csv<Vec<u8>, E> {
    type Error = UnknownEncoding;
    type Parser = CsvParser<Cursor<Vec<u8>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        use crate::decode::decode_owned;

        let bytes = decode_owned(self.bytes, self.encoding.oneshot_decoder())?.into_bytes();
        Ok(make_parser(Cursor::new(bytes)))
    }
}

impl<'a, E: Encoding> IntoParser for Csv<&'a [u8], E> {
    type Error = UnknownEncoding;
    type Parser = CsvParser<Cursor<Cow<'a, [u8]>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let bytes = into_bytes(self.encoding.oneshot_decoder().decode(self.bytes)?);
        Ok(make_parser(Cursor::new(bytes)))
    }
}

impl<'a, E: Encoding> IntoParser for Csv<Cow<'a, [u8]>, E> {
    type Error = UnknownEncoding;
    type Parser = CsvParser<Cursor<Cow<'a, [u8]>>>;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        use crate::decode::decode_cow;

        let bytes = into_bytes(decode_cow(self.bytes, self.encoding.oneshot_decoder())?);
        Ok(make_parser(Cursor::new(bytes)))
    }
}

fn into_bytes(s: Cow<'_, str>) -> Cow<'_, [u8]> {
    match s {
        Cow::Borrowed(s) => Cow::Borrowed(s.as_bytes()),
        Cow::Owned(s) => Cow::Owned(s.into_bytes()),
    }
}

fn make_parser<R: Read>(rdr: R) -> CsvParser<R> {
    use csv::Reader;

    let rdr = Reader::from_reader(rdr);
    let rec = rdr.into_deserialize();
    CsvParser { rec }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "decode")]
    use crate::IntoParser as _;
    use crate::csv::tests::{INVALID, VALID, collect, doc, expected};
    #[cfg(feature = "decode")]
    use crate::decode::{Dynamic, Utf16Be};

    use super::Csv;

    #[tokio::test]
    async fn parses_borrowed_and_owned_strings() {
        assert_eq!(collect(Csv::<&str>::new(VALID)).await, (expected(), false));
        assert_eq!(
            collect(Csv::<String>::new(VALID.to_owned())).await,
            (expected(), false)
        );
    }

    #[tokio::test]
    async fn stops_at_invalid_records_in_borrowed_and_owned_strings() {
        let expected = vec![doc("春", "あけぼの。", &[])];
        assert_eq!(
            collect(Csv::<&str>::new(INVALID)).await,
            (expected.clone(), true)
        );
        assert_eq!(
            collect(Csv::<String>::new(INVALID.to_owned())).await,
            (expected, true)
        );
    }

    #[cfg(feature = "decode")]
    fn utf16_be(input: &'static str) -> Vec<u8> {
        use encoding_rs::mem::convert_str_to_utf16;

        let mut units = vec![0; input.len()];
        let written = convert_str_to_utf16(input, &mut units);
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
            collect(Csv::<&[u8], _>::new(input.as_slice(), Utf16Be)).await,
            (expected(), false)
        );
        assert_eq!(
            collect(Csv::<Vec<u8>, _>::new(input.clone(), Utf16Be)).await,
            (expected(), false)
        );
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn stops_at_invalid_utf16_be_csv_records() {
        let input = utf16_be(INVALID);
        let expected = vec![doc("春", "あけぼの。", &[])];
        assert_eq!(
            collect(Csv::<&[u8], _>::new(input.as_slice(), Utf16Be)).await,
            (expected.clone(), true)
        );
        assert_eq!(
            collect(Csv::<Vec<u8>, _>::new(input.clone(), Utf16Be)).await,
            (expected.clone(), true)
        );
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn replaces_malformed_utf16_be_sequences() {
        let mut input = utf16_be("title,body\n春,あけぼの。");
        input.extend_from_slice(&[0xd8, 0x00]);
        assert_eq!(
            collect(Csv::<&[u8], _>::new(input.as_slice(), Utf16Be)).await,
            (vec![doc("春", "あけぼの。�", &[])], false)
        );
        assert_eq!(
            collect(Csv::<Vec<u8>, _>::new(input, Utf16Be)).await,
            (vec![doc("春", "あけぼの。�", &[])], false)
        );
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn rejects_unknown_encodings() {
        let input = [0xff];
        assert!(
            Csv::<&[u8], _>::new(input.as_slice(), Dynamic)
                .parser()
                .await
                .is_err()
        );
        assert!(
            Csv::<Vec<u8>, _>::new(input.to_vec(), Dynamic)
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
            let csv = Csv::<&str>::new(input);
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
            let csv = Csv::<&str>::new(input);
            assert_eq!(collect(csv).await, (vec![], true), "{input}");
        }
    }
}
