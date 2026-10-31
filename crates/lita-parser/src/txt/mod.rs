#[cfg(feature = "txt-bytes")]
pub mod bytes;

#[cfg(feature = "txt-io")]
pub mod io;

use crate::{ParsedDoc, Parser};
use std::convert::Infallible;

mod imp {
    use crate::{Body, Metadata, ParsedDoc, Title};
    use std::borrow::Cow;
    use std::convert::Infallible;

    #[cfg(feature = "decode")]
    use crate::decode::{Charset, UnknownEncoding, decode};

    pub(super) struct Txt<S> {
        pub(super) bytes: S,
        pub(super) title: Title,
        pub(super) metadata: Option<Metadata>,
    }

    pub(super) trait IntoString {
        fn into_string(self) -> String;
    }

    impl IntoString for String {
        #[inline]
        fn into_string(self) -> String {
            self
        }
    }

    impl IntoString for &str {
        #[inline]
        fn into_string(self) -> String {
            self.to_owned()
        }
    }

    impl IntoString for Cow<'_, str> {
        #[inline]
        fn into_string(self) -> String {
            self.into_owned()
        }
    }

    pub(super) fn next_doc<S: IntoString>(
        txt: &mut Option<Txt<S>>,
    ) -> Option<Result<ParsedDoc, Infallible>> {
        let txt = txt.take()?;
        Some(Ok(ParsedDoc {
            title: txt.title,
            body: Body(txt.bytes.into_string()),
            metadata: txt.metadata,
        }))
    }

    #[cfg(feature = "decode")]
    impl Txt<Vec<u8>> {
        pub(super) fn decode(self, charset: Charset) -> Result<Txt<String>, UnknownEncoding> {
            decode(&self.bytes, charset).map(|decoded| Txt {
                bytes: decoded.into_owned(),
                title: self.title,
                metadata: self.metadata,
            })
        }
    }

    #[cfg(all(feature = "decode", feature = "txt-bytes"))]
    impl<'a> Txt<&'a [u8]> {
        pub(super) fn decode(self, charset: Charset) -> Result<Txt<Cow<'a, str>>, UnknownEncoding> {
            decode(self.bytes, charset).map(|decoded| Txt {
                bytes: decoded,
                title: self.title,
                metadata: self.metadata,
            })
        }
    }

    #[cfg(all(feature = "decode", feature = "txt-bytes"))]
    impl Txt<Cow<'_, [u8]>> {
        pub(super) fn decode(self, charset: Charset) -> Result<Txt<String>, UnknownEncoding> {
            decode(&self.bytes, charset).map(|decoded| Txt {
                bytes: decoded.into_owned(),
                title: self.title,
                metadata: self.metadata,
            })
        }
    }
}

pub struct TxtParser {
    imp: Option<imp::Txt<String>>,
}

impl Parser for TxtParser {
    type Error = Infallible;

    fn size_hint(&self) -> Option<usize> {
        let len = if self.imp.is_some() { 1 } else { 0 };
        Some(len)
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        imp::next_doc(&mut self.imp)
    }
}

impl TxtParser {
    #[cfg(feature = "txt-bytes")]
    fn from<S: imp::IntoString>(txt: imp::Txt<S>) -> Self {
        Self {
            imp: Some(imp::Txt {
                bytes: txt.bytes.into_string(),
                title: txt.title,
                metadata: txt.metadata,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{IntoParser, ParsedDoc, Parser};
    use std::fmt::Debug;

    pub(super) async fn collect<S>(source: S) -> Vec<ParsedDoc>
    where
        S: IntoParser,
        S::Error: Debug,
        <S::Parser as Parser>::Error: Debug,
    {
        let mut parser = source.parser().await.unwrap();
        let mut docs = Vec::new();
        while let Some(doc) = parser.next().await {
            docs.push(doc.unwrap());
        }
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
        docs
    }
}
