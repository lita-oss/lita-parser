#[cfg(feature = "txt-sync")]
pub mod sync_bytes;

#[cfg(feature = "txt-tokio-io")]
pub mod tokio_io;

use crate::{Metadata, ParsedDoc, Parser, Title};

use std::convert::Infallible;

pub struct TxtParser {
    doc: Option<ParsedDoc>,
}

impl TxtParser {
    fn new(bytes: String, title: Title, metadata: Option<Metadata>) -> Self {
        use crate::Body;

        Self {
            doc: Some(ParsedDoc {
                title,
                body: Body(bytes),
                metadata,
            }),
        }
    }
}

impl Parser for TxtParser {
    type Error = Infallible;

    fn size_hint(&self) -> Option<usize> {
        Some(usize::from(self.doc.is_some()))
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        self.doc.take().map(Ok)
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
