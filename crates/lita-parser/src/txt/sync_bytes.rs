use crate::decode::{Encoding, OneshotDecoder as _, UnknownEncoding, Utf8};
use crate::{IntoParser, Metadata, Title};

pub use super::TxtParser;

use std::borrow::Cow;
use std::convert::Infallible;

pub struct Txt<S, E = Utf8> {
    bytes: S,
    title: Title,
    metadata: Option<Metadata>,
    encoding: E,
}

impl<S, E> Txt<S, E> {
    fn from_parts(bytes: S, title: Title, metadata: Option<Metadata>, encoding: E) -> Self {
        Self {
            bytes,
            title,
            metadata,
            encoding,
        }
    }
}

impl Txt<String> {
    pub fn new(bytes: String, title: Title, metadata: Option<Metadata>) -> Self {
        Self::from_parts(bytes, title, metadata, Utf8)
    }
}

impl IntoParser for Txt<String> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::new(self.bytes, self.title, self.metadata))
    }
}

impl<'a> Txt<&'a str> {
    pub fn new(bytes: &'a str, title: Title, metadata: Option<Metadata>) -> Self {
        Self::from_parts(bytes, title, metadata, Utf8)
    }
}

impl IntoParser for Txt<&str> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::new(
            self.bytes.to_owned(),
            self.title,
            self.metadata,
        ))
    }
}

impl<'a> Txt<Cow<'a, str>> {
    pub fn new(bytes: Cow<'a, str>, title: Title, metadata: Option<Metadata>) -> Self {
        Self::from_parts(bytes, title, metadata, Utf8)
    }
}

impl IntoParser for Txt<Cow<'_, str>> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::new(
            self.bytes.into_owned(),
            self.title,
            self.metadata,
        ))
    }
}

impl<E: Encoding> Txt<Vec<u8>, E> {
    pub fn new(bytes: Vec<u8>, title: Title, metadata: Option<Metadata>, encoding: E) -> Self {
        Self::from_parts(bytes, title, metadata, encoding)
    }
}

impl<E: Encoding> IntoParser for Txt<Vec<u8>, E> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        use crate::decode::decode_owned;

        let bytes = decode_owned(self.bytes, self.encoding.oneshot_decoder())?;
        Ok(TxtParser::new(bytes, self.title, self.metadata))
    }
}

impl<'a, E: Encoding> Txt<&'a [u8], E> {
    pub fn new(bytes: &'a [u8], title: Title, metadata: Option<Metadata>, encoding: E) -> Self {
        Self::from_parts(bytes, title, metadata, encoding)
    }
}

impl<E: Encoding> IntoParser for Txt<&[u8], E> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        let bytes = self
            .encoding
            .oneshot_decoder()
            .decode(self.bytes)?
            .into_owned();
        Ok(TxtParser::new(bytes, self.title, self.metadata))
    }
}

impl<'a, E: Encoding> Txt<Cow<'a, [u8]>, E> {
    pub fn new(
        bytes: Cow<'a, [u8]>,
        title: Title,
        metadata: Option<Metadata>,
        encoding: E,
    ) -> Self {
        Self::from_parts(bytes, title, metadata, encoding)
    }
}

impl<'a, E: Encoding> IntoParser for Txt<Cow<'a, [u8]>, E> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        use crate::decode::decode_cow;

        let bytes = decode_cow(self.bytes, self.encoding.oneshot_decoder())?.into_owned();
        Ok(TxtParser::new(bytes, self.title, self.metadata))
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "decode")]
    use crate::decode::{Dynamic, Utf16Be};
    use crate::txt::tests::collect;
    use crate::{Body, ParsedDoc};

    use super::*;

    fn expected(text: &str) -> Vec<ParsedDoc> {
        vec![ParsedDoc {
            title: Title("title".to_owned()),
            body: Body(text.to_owned()),
            metadata: None,
        }]
    }

    #[tokio::test]
    async fn parses_borrowed_and_owned_strings() {
        for text in ["春はあけぼの。\nSecond line", "", "\0\u{fffd}"] {
            let title = Title("title".to_owned());
            let borrowed = Txt::<&str>::new(text, title.clone(), None);
            let owned = Txt::<String>::new(text.to_owned(), title, None);
            assert_eq!(collect(borrowed).await, expected(text));
            assert_eq!(collect(owned).await, expected(text));
        }
    }

    #[cfg(feature = "decode")]
    fn utf16_be(text: &'static str) -> Vec<u8> {
        use encoding_rs::mem::convert_str_to_utf16;

        let mut units = vec![0; text.len()];
        let len = convert_str_to_utf16(text, &mut units);
        units[..len]
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect()
    }

    #[cfg(feature = "decode")]
    async fn assert_decoded(bytes: &[u8], text: &str) {
        let title = Title("title".to_owned());
        let borrowed = Txt::<&[u8], _>::new(bytes, title.clone(), None, Utf16Be);
        let owned = Txt::<Vec<u8>, _>::new(bytes.to_vec(), title.clone(), None, Utf16Be);
        assert_eq!(collect(borrowed).await, expected(text));
        assert_eq!(collect(owned).await, expected(text));
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn decodes_utf16_be() {
        for text in ["春はあけぼの。\n🌸", ""] {
            let bytes = utf16_be(text);
            assert_decoded(&bytes, text).await;
            let bytes_with_bom = [vec![0xfe, 0xff], bytes].concat();
            assert_decoded(&bytes_with_bom, text).await;
        }
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn replaces_malformed_utf16_be() {
        let mut bytes = utf16_be("春はあけぼの。");
        bytes.push(0);
        assert_decoded(&bytes, "春はあけぼの。\u{fffd}").await;
    }

    #[cfg(feature = "decode")]
    #[tokio::test]
    async fn rejects_unknown_encoding() {
        let bytes = &[0xff][..];
        let title = Title("title".to_owned());
        let borrowed = Txt::<&[u8], _>::new(bytes, title.clone(), None, Dynamic);
        let owned = Txt::<Vec<u8>, _>::new(bytes.to_vec(), title.clone(), None, Dynamic);
        assert!(borrowed.parser().await.is_err());
        assert!(owned.parser().await.is_err());
    }
}
