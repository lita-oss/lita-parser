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
use std::iter::FusedIterator;

#[cfg(feature = "stream")]
use futures_util::stream::FusedStream;

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

impl<P: Parser> IntoParser for P {
    type Error = std::convert::Infallible;
    type Parser = Self;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(self)
    }
}

pub trait Parser: Sized {
    type Error;

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn next(&mut self) -> impl Future<Output = Option<Result<ParsedDoc, Self::Error>>>;

    fn batched(self, batch_size: usize) -> Batched<Self> {
        let batch_size = if batch_size == 0 { 1 } else { batch_size };
        Batched {
            parser: self,
            batch_size,
        }
    }
}

pub trait BatchParser: Sized {
    type Error;

    fn size_hint(&self) -> Option<usize> {
        None
    }

    fn batch_size(&self) -> usize;

    fn next(&mut self) -> impl Future<Output = Option<Result<Box<[ParsedDoc]>, Self::Error>>>;
}

impl<P: Parser> BatchParser for P {
    type Error = P::Error;

    fn size_hint(&self) -> Option<usize> {
        <P as Parser>::size_hint(self)
    }

    fn batch_size(&self) -> usize {
        1
    }

    async fn next(&mut self) -> Option<Result<Box<[ParsedDoc]>, Self::Error>> {
        <P as Parser>::next(self)
            .await
            .map(|res| res.map(|doc| Box::from([doc])))
    }
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

pub struct Batched<P> {
    parser: P,
    batch_size: usize,
}

impl<P> Batched<P> {
    pub fn flatten(self) -> P {
        self.parser
    }
}

impl<P: Parser> Batched<P> {
    fn size_impl(&self) -> Option<usize> {
        self.parser.size_hint().map(|size| {
            if size == 0 {
                0
            } else {
                (size - 1) / self.batch_size + 1
            }
        })
    }

    async fn next_impl(&mut self) -> Option<Result<Box<[ParsedDoc]>, P::Error>> {
        let first = match self.parser.next().await? {
            Ok(doc) => doc,
            Err(e) => return Some(Err(e)),
        };

        let size = self.parser.size_hint().unwrap_or(usize::MAX - 1) + 1;
        let mut batch = Vec::with_capacity(std::cmp::min(size, self.batch_size));

        batch.push(first);

        for _ in 1..self.batch_size {
            match self.parser.next().await {
                Some(Ok(doc)) => batch.push(doc),
                Some(Err(e)) => return Some(Err(e)),
                None => return Some(Ok(batch.into_boxed_slice())),
            }
        }

        Some(Ok(batch.into_boxed_slice()))
    }
}

impl<P: Parser> IntoParser for Batched<P> {
    type Error = std::convert::Infallible;
    type Parser = P;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        Ok(self.parser)
    }
}

impl<P: Parser> BatchParser for Batched<P> {
    type Error = P::Error;

    fn size_hint(&self) -> Option<usize> {
        self.size_impl()
    }

    fn batch_size(&self) -> usize {
        self.batch_size
    }

    async fn next(&mut self) -> Option<Result<Box<[ParsedDoc]>, Self::Error>> {
        self.next_impl().await
    }
}

pub struct DocIterator<I>(I);

impl<E, I: FusedIterator<Item = Result<ParsedDoc, E>>> DocIterator<I> {
    pub fn new(it: I) -> Self {
        Self(it)
    }
}

impl<E, I: FusedIterator<Item = Result<ParsedDoc, E>>> Parser for DocIterator<I> {
    type Error = E;

    fn size_hint(&self) -> Option<usize> {
        let (_, upper) = self.0.size_hint();
        upper
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        self.0.next()
    }
}

#[cfg(feature = "stream")]
pub struct DocStream<S>(S);

#[cfg(feature = "stream")]
impl<E, S: FusedStream<Item = Result<ParsedDoc, E>>> DocStream<S> {
    pub fn new(stream: S) -> Self {
        Self(stream)
    }
}

#[cfg(feature = "stream")]
impl<E, S: FusedStream<Item = Result<ParsedDoc, E>> + Unpin> Parser for DocStream<S> {
    type Error = E;

    fn size_hint(&self) -> Option<usize> {
        let (_, upper) = self.0.size_hint();
        upper
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        use futures_util::StreamExt as _;
        self.0.next().await
    }
}
