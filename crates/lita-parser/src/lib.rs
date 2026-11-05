pub mod decode;
#[cfg(feature = "_tokio-io")]
pub use decode::EncodingTokioExt;
pub use decode::{Encoding, OneshotDecoder, StreamEncoding, UnknownEncoding, Utf8};

#[cfg(feature = "decode")]
pub use decode::{Dynamic, EucJp, ShiftJis, Utf16Be, Utf16Le};

pub mod dispatch;

#[cfg(any(feature = "txt-sync", feature = "txt-tokio-io"))]
pub mod txt;

#[cfg(any(feature = "json-sync", feature = "json-tokio-io"))]
pub mod json;

#[cfg(any(feature = "csv-sync", feature = "csv-tokio-io"))]
pub mod csv;

#[cfg(feature = "stream")]
use futures_util::stream::FusedStream;

use std::borrow::Borrow;
use std::collections::btree_map::{self, BTreeMap};
use std::convert::Infallible;
use std::iter::FusedIterator;
use std::num::NonZeroUsize;
use std::slice::{self, IterMut};
use std::vec;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Title(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Body(String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Metadata(BTreeMap<String, String>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedDoc {
    pub title: Title,
    pub body: Body,
    pub metadata: Option<Metadata>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Batch(Box<[ParsedDoc]>);

pub trait IntoParser {
    type Error;
    type Parser: Parser;

    fn parser(self) -> impl Future<Output = Result<Self::Parser, Self::Error>>;
}

impl<P: Parser> IntoParser for P {
    type Error = Infallible;
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

    fn batched(self, batch_size: NonZeroUsize) -> Batched<Self> {
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

    fn batch_size(&self) -> NonZeroUsize;

    fn next(&mut self) -> impl Future<Output = Option<Result<Batch, Self::Error>>>;
}

impl<P: Parser> BatchParser for P {
    type Error = P::Error;

    fn size_hint(&self) -> Option<usize> {
        <P as Parser>::size_hint(self)
    }

    fn batch_size(&self) -> NonZeroUsize {
        NonZeroUsize::MIN
    }

    async fn next(&mut self) -> Option<Result<Batch, Self::Error>> {
        <P as Parser>::next(self)
            .await
            .map(|res| res.map(|doc| Batch::from([doc])))
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

impl Batch {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn as_slice(&self) -> &[ParsedDoc] {
        &self.0
    }

    pub fn as_mut_slice(&mut self) -> &mut [ParsedDoc] {
        &mut self.0
    }

    pub fn into_slice(self) -> Box<[ParsedDoc]> {
        self.0
    }

    pub fn iter(&self) -> slice::Iter<'_, ParsedDoc> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> IterMut<'_, ParsedDoc> {
        self.0.iter_mut()
    }

    pub fn get(&self, idx: usize) -> Option<&ParsedDoc> {
        self.0.get(idx)
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut ParsedDoc> {
        self.0.get_mut(idx)
    }
}

impl IntoIterator for Batch {
    type Item = ParsedDoc;
    type IntoIter = vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl From<Vec<ParsedDoc>> for Batch {
    fn from(value: Vec<ParsedDoc>) -> Self {
        Self(value.into_boxed_slice())
    }
}

impl From<Box<[ParsedDoc]>> for Batch {
    fn from(value: Box<[ParsedDoc]>) -> Self {
        Self(value)
    }
}

impl<const N: usize> From<[ParsedDoc; N]> for Batch {
    fn from(value: [ParsedDoc; N]) -> Self {
        Self(Box::from(value))
    }
}

impl From<&[ParsedDoc]> for Batch {
    fn from(value: &[ParsedDoc]) -> Self {
        Self(Box::from(value))
    }
}

pub struct Batched<P> {
    parser: P,
    batch_size: NonZeroUsize,
}

impl<P> Batched<P> {
    pub fn flatten(self) -> P {
        self.parser
    }
}

impl<P: Parser> Batched<P> {
    fn size_impl(&self) -> Option<usize> {
        self.parser
            .size_hint()
            .map(|size| size.div_ceil(self.batch_size.get()))
    }

    async fn next_impl(&mut self) -> Option<Result<Batch, P::Error>> {
        use std::cmp::min;

        let first = match self.parser.next().await? {
            Ok(doc) => doc,
            Err(e) => return Some(Err(e)),
        };

        let size = self.parser.size_hint().unwrap_or(usize::MAX - 1) + 1;
        let batch_size = self.batch_size.get();
        let mut batch = Vec::with_capacity(min(size, batch_size));

        batch.push(first);

        for _ in 1..batch_size {
            match self.parser.next().await {
                Some(Ok(doc)) => batch.push(doc),
                Some(Err(e)) => return Some(Err(e)),
                None => break,
            }
        }

        Some(Ok(Batch::from(batch)))
    }
}

impl<P: Parser> IntoParser for Batched<P> {
    type Error = Infallible;
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

    fn batch_size(&self) -> NonZeroUsize {
        self.batch_size
    }

    async fn next(&mut self) -> Option<Result<Batch, Self::Error>> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_batch_size() {
        struct TestParser;

        impl Parser for TestParser {
            type Error = ();

            fn size_hint(&self) -> Option<usize> {
                None
            }

            async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
                None
            }
        }

        assert_eq!(
            <TestParser as BatchParser>::batch_size(&TestParser),
            NonZeroUsize::new(1).unwrap()
        );
    }
}
