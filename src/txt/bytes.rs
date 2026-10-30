use super::{TxtParser, imp};
use crate::{IntoParser, Metadata, Title};
use std::borrow::Cow;
use std::convert::Infallible;

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
