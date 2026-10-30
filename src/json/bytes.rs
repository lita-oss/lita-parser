use super::JsonParser;
use crate::IntoParser;

use std::borrow::Cow;

use serde_json::Error;

pub struct Json<S> {
    bytes: S,
}

impl Json<String> {
    pub fn from(json: String) -> Self {
        Self { bytes: json }
    }
}

impl IntoParser for Json<String> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(&self.bytes)
    }
}

impl<'a> Json<&'a str> {
    pub fn from(json: &'a str) -> Self {
        Self { bytes: json }
    }
}

impl IntoParser for Json<&str> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(self.bytes)
    }
}

impl<'a> Json<Cow<'a, str>> {
    pub fn from(json: Cow<'a, str>) -> Self {
        Self { bytes: json }
    }
}

impl IntoParser for Json<Cow<'_, str>> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(&self.bytes)
    }
}
