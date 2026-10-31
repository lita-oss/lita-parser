use super::{TxtParser, imp};
use crate::{IntoParser, Metadata, Title};
use std::borrow::Cow;
use std::convert::Infallible;

#[cfg(feature = "decode")]
use crate::decode::{Charset, UnknownEncoding};

pub struct Txt<S> {
    imp: imp::Txt<S>,
}

impl<S> Txt<S> {
    pub fn from(txt: S, title: Title, metadata: Option<Metadata>) -> Self {
        Self {
            imp: imp::Txt {
                bytes: txt,
                title,
                metadata,
            },
        }
    }
}

impl IntoParser for Txt<String> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp))
    }
}

impl IntoParser for Txt<&str> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp))
    }
}

impl IntoParser for Txt<Cow<'_, str>> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp))
    }
}

#[cfg(feature = "decode")]
pub struct TxtWithCharset<S> {
    imp: imp::Txt<S>,
    charset: Charset,
}

#[cfg(feature = "decode")]
impl<S> TxtWithCharset<S> {
    pub fn from(txt: S, title: Title, metadata: Option<Metadata>, charset: Charset) -> Self {
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
impl IntoParser for TxtWithCharset<Vec<u8>> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp.decode(self.charset)?))
    }
}

#[cfg(feature = "decode")]
impl IntoParser for TxtWithCharset<&[u8]> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp.decode(self.charset)?))
    }
}

#[cfg(feature = "decode")]
impl IntoParser for TxtWithCharset<Cow<'_, [u8]>> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp.decode(self.charset)?))
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
            metadata: None,
        }]
    }

    #[tokio::test]
    async fn parses_borrowed_and_owned_strings() {
        for text in ["春はあけぼの。\nSecond line", "", "\0\u{fffd}"] {
            let title = Title("title".to_owned());
            let borrowed = Txt::<&str>::from(text, title.clone(), None);
            let owned = Txt::<String>::from(text.to_owned(), title, None);
            assert_eq!(collect(borrowed).await, expected(text));
            assert_eq!(collect(owned).await, expected(text));
        }
    }

    #[cfg(feature = "decode")]
    fn utf16_be(text: &'static str) -> Vec<u8> {
        let mut units = vec![0; text.len()];
        let len = encoding_rs::mem::convert_str_to_utf16(text, &mut units);
        units[..len]
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect()
    }

    #[cfg(feature = "decode")]
    async fn assert_decoded(bytes: &[u8], text: &str) {
        let title = Title("title".to_owned());
        let borrowed = TxtWithCharset::<&[u8]>::from(bytes, title.clone(), None, Charset::Utf16Be);
        let owned =
            TxtWithCharset::<Vec<u8>>::from(bytes.to_vec(), title.clone(), None, Charset::Utf16Be);
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
        let borrowed = TxtWithCharset::<&[u8]>::from(bytes, title.clone(), None, Charset::Dynamic);
        let owned =
            TxtWithCharset::<Vec<u8>>::from(bytes.to_vec(), title.clone(), None, Charset::Dynamic);
        assert!(borrowed.parser().await.is_err());
        assert!(owned.parser().await.is_err());
    }
}
