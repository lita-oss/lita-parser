#[cfg(feature = "json-sync")]
pub mod sync_bytes;

#[cfg(feature = "json-tokio-io")]
pub mod tokio_io;

use crate::{ParsedDoc, Parser};

use de::{DocsIter, parse_json};

use serde_json::Error;

use std::convert::Infallible;

mod de {
    use crate::ParsedDoc;

    use serde_json::{Deserializer, Error};

    use serde::de::{self, Deserialize};

    use std::{collections::BTreeMap, vec::IntoIter};

    pub(super) fn parse_json(json: &str) -> Result<DocsIter, Error> {
        use de::DeserializeSeed as _;

        let mut deserializer = Deserializer::from_str(json);
        let docs = VecDocJsonSeed.deserialize(&mut deserializer)?;
        deserializer.end()?;

        Ok(DocsIter {
            inner: docs.map(Vec::into_iter),
        })
    }

    pub(super) struct DocsIter {
        inner: Option<IntoIter<ParsedDoc>>,
    }

    impl DocsIter {
        pub(super) fn next(&mut self) -> Option<ParsedDoc> {
            self.inner.as_mut()?.next()
        }

        pub(super) fn len(&self) -> usize {
            self.inner.as_ref().map_or(0, ExactSizeIterator::len)
        }
    }

    struct DocJson {
        title: String,
        body: String,
        metadata: Option<BTreeMap<String, String>>,
    }

    struct VecDocJsonSeed;

    impl<'de> de::DeserializeSeed<'de> for VecDocJsonSeed {
        type Value = Option<Vec<ParsedDoc>>;

        fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            use crate::{Body, Metadata, Title};

            use de::{SeqAccess, Visitor};

            use std::fmt;

            struct VecDocVisitor;

            impl<'de> Visitor<'de> for VecDocVisitor {
                type Value = Option<Vec<ParsedDoc>>;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("an array of documents")
                }

                fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
                where
                    A: SeqAccess<'de>,
                {
                    let mut docs: Option<Vec<ParsedDoc>> = None;
                    let size_hint = seq.size_hint().unwrap_or_default();

                    while let Some(doc) = seq.next_element::<DocJson>()? {
                        docs.get_or_insert_with(|| Vec::with_capacity(size_hint))
                            .push(ParsedDoc {
                                title: Title(doc.title),
                                body: Body(doc.body),
                                metadata: doc.metadata.map(Metadata),
                            });
                    }

                    Ok(docs)
                }
            }

            deserializer.deserialize_seq(VecDocVisitor)
        }
    }

    impl<'de> Deserialize<'de> for DocJson {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            use de::{MapAccess, Visitor};

            use std::fmt;

            enum Field {
                Title,
                Id,
                Body,
                Metadata(String),
            }

            struct FieldVisitor;

            impl<'de> Visitor<'de> for FieldVisitor {
                type Value = Field;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a document field name")
                }

                fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(match value {
                        "title" => Field::Title,
                        "id" => Field::Id,
                        "body" => Field::Body,
                        _ => Field::Metadata(value.to_owned()),
                    })
                }

                fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    Ok(match value.as_str() {
                        "title" => Field::Title,
                        "id" => Field::Id,
                        "body" => Field::Body,
                        _ => Field::Metadata(value),
                    })
                }
            }

            impl<'de> Deserialize<'de> for Field {
                fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where
                    D: de::Deserializer<'de>,
                {
                    deserializer.deserialize_identifier(FieldVisitor)
                }
            }

            struct DocVisitor;

            impl<'de> Visitor<'de> for DocVisitor {
                type Value = DocJson;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a document object")
                }

                fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
                where
                    A: MapAccess<'de>,
                {
                    let mut title = None;
                    let mut id = None;
                    let mut body = None;
                    let mut metadata: Option<BTreeMap<String, String>> = None;

                    while let Some(field) = map.next_key::<Field>()? {
                        let (value, field) = match field {
                            Field::Title => (&mut title, "title"),
                            Field::Id => (&mut id, "id"),
                            Field::Body => (&mut body, "body"),
                            Field::Metadata(key) => {
                                let value = map.next_value()?;
                                metadata.get_or_insert_default().insert(key, value);
                                continue;
                            }
                        };
                        if value.is_some() {
                            return Err(de::Error::duplicate_field(field));
                        }
                        *value = Some(map.next_value()?);
                    }

                    let (title, metadata_title) = match (id, title) {
                        (Some(id), title) => (id, title),
                        (None, Some(title)) => (title, None),
                        (None, None) => return Err(de::Error::missing_field("title or id")),
                    };
                    let body = body.ok_or_else(|| de::Error::missing_field("body"))?;

                    if let Some(title) = metadata_title {
                        metadata
                            .get_or_insert_default()
                            .insert("title".to_owned(), title);
                    }

                    Ok(DocJson {
                        title,
                        body,
                        metadata,
                    })
                }
            }

            deserializer.deserialize_map(DocVisitor)
        }
    }
}

pub struct JsonParser {
    it: DocsIter,
}

impl Parser for JsonParser {
    type Error = Infallible;

    fn size_hint(&self) -> Option<usize> {
        Some(self.it.len())
    }

    async fn next(&mut self) -> Option<Result<ParsedDoc, Self::Error>> {
        self.it.next().map(Ok)
    }
}

impl JsonParser {
    fn from_str(s: &str) -> Result<Self, Error> {
        parse_json(s).map(|it| Self { it })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Body, IntoParser, Metadata, ParsedDoc, Parser, Title};
    use std::fmt::Debug;

    pub(super) const INPUT: &str = r#"[
        {"title":"春","body":"春はあけぼの。"},
        {"id":"doc-2","title":"夏","body":"夏は夜。","author":"清少納言"},
        {"id":"doc-3","body":"秋は夕暮れ。"},
        {"title":"","body":""}
    ]"#;

    pub(super) fn expected() -> Vec<ParsedDoc> {
        vec![
            ParsedDoc {
                title: Title("春".to_owned()),
                body: Body("春はあけぼの。".to_owned()),
                metadata: None,
            },
            ParsedDoc {
                title: Title("doc-2".to_owned()),
                body: Body("夏は夜。".to_owned()),
                metadata: Some(Metadata(
                    [
                        ("author".to_owned(), "清少納言".to_owned()),
                        ("title".to_owned(), "夏".to_owned()),
                    ]
                    .into(),
                )),
            },
            ParsedDoc {
                title: Title("doc-3".to_owned()),
                body: Body("秋は夕暮れ。".to_owned()),
                metadata: None,
            },
            ParsedDoc {
                title: Title(String::new()),
                body: Body(String::new()),
                metadata: None,
            },
        ]
    }

    pub(super) async fn collect<I: IntoParser>(input: I) -> Result<Vec<ParsedDoc>, I::Error>
    where
        <I::Parser as Parser>::Error: Debug,
    {
        let mut parser = input.parser().await?;
        let mut docs = Vec::new();
        while let Some(result) = parser.next().await {
            docs.push(result.unwrap());
        }
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
        Ok(docs)
    }
}
