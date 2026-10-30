#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "decode")]
mod decode;
pub use decode::{Charset, UnknownEncoding};

#[cfg(any(feature = "txt-bytes", feature = "txt-io"))]
pub mod txt;

#[cfg(any(feature = "json-bytes", feature = "json-io"))]
pub mod json;

#[cfg(any(feature = "csv-bytes", feature = "csv-io"))]
pub mod csv;

use std::borrow::Borrow;
use std::collections::btree_map::{self, BTreeMap};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Title(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Body(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Metadata(BTreeMap<String, String>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDoc {
    pub title: Title,
    pub body: Body,
    pub metadata: Option<Metadata>,
}

pub trait IntoParser {
    type Error;
    type Parser: Parser;

    fn parser(self) -> impl Future<Output = Result<Self::Parser, Self::Error>>;
}

pub trait Parser {
    type Error;

    fn next(&mut self) -> impl Future<Output = Option<Result<ParsedDoc, Self::Error>>>;
}

impl Title {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Body {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }
}

impl Metadata {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> btree_map::Iter<'_, String, String> {
        self.0.iter()
    }

    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.0.contains_key(key)
    }

    pub fn get<Q>(&self, key: &Q) -> Option<&str>
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.0.get(key).map(|s| s.as_str())
    }

    pub fn as_map(&self) -> &BTreeMap<String, String> {
        &self.0
    }

    pub fn into_map(self) -> BTreeMap<String, String> {
        self.0
    }
}

impl IntoIterator for Metadata {
    type Item = (String, String);
    type IntoIter = btree_map::IntoIter<String, String>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
