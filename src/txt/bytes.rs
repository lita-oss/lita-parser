use super::{TxtParser, imp};
use crate::{IntoParser, Metadata, Title};
use std::borrow::Cow;
use std::convert::Infallible;

#[cfg(feature = "decode")]
use crate::decode::{Charset, UnknownEncoding};

pub struct Txt<S> {
    imp: imp::Txt<S>,
}

impl Txt<String> {
    pub fn from(txt: String, title: Title, metadata: Option<Metadata>) -> Self {
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

impl<'a> Txt<&'a str> {
    pub fn from(txt: &'a str, title: Title, metadata: Option<Metadata>) -> Self {
        Self {
            imp: imp::Txt {
                bytes: txt,
                title,
                metadata,
            },
        }
    }
}

impl IntoParser for Txt<&str> {
    type Error = Infallible;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp))
    }
}

impl<'a> Txt<Cow<'a, str>> {
    pub fn from(txt: Cow<'a, str>, title: Title, metadata: Option<Metadata>) -> Self {
        Self {
            imp: imp::Txt {
                bytes: txt,
                title,
                metadata,
            },
        }
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
impl TxtWithCharset<Vec<u8>> {
    pub fn from(txt: Vec<u8>, title: Title, metadata: Option<Metadata>, charset: Charset) -> Self {
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
impl<'a> TxtWithCharset<&'a [u8]> {
    pub fn from(txt: &'a [u8], title: Title, metadata: Option<Metadata>, charset: Charset) -> Self {
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
impl IntoParser for TxtWithCharset<&[u8]> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp.decode(self.charset)?))
    }
}

#[cfg(feature = "decode")]
impl<'a> TxtWithCharset<Cow<'a, [u8]>> {
    pub fn from(
        txt: Cow<'a, [u8]>,
        title: Title,
        metadata: Option<Metadata>,
        charset: Charset,
    ) -> Self {
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
impl IntoParser for TxtWithCharset<Cow<'_, [u8]>> {
    type Error = UnknownEncoding;
    type Parser = TxtParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(TxtParser::from(self.imp.decode(self.charset)?))
    }
}
