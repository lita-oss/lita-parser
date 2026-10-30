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
}

pub struct TxtParser {
    imp: Option<imp::Txt<String>>,
}

impl Parser for TxtParser {
    type Error = Infallible;

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        imp::next_doc(&mut self.imp)
    }
}

impl TxtParser {
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
