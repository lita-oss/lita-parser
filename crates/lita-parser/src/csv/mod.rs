#[cfg(feature = "csv-sync")]
pub mod sync_bytes;

#[cfg(feature = "csv-tokio-io")]
pub mod tokio_io;

mod de {
    use crate::{Body, Metadata, ParsedDoc, Title};

    use serde::de::{self, Deserialize};

    use std::collections::BTreeMap;

    pub(super) struct DocCsv {
        title: String,
        body: String,
        metadata: Option<BTreeMap<String, String>>,
    }

    impl DocCsv {
        pub(super) fn into_parsed(self) -> ParsedDoc {
            ParsedDoc {
                title: Title(self.title),
                body: Body(self.body),
                metadata: self.metadata.map(Metadata),
            }
        }
    }

    impl<'de> Deserialize<'de> for DocCsv {
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

                fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
                where
                    E: de::Error,
                {
                    use std::str::from_utf8;

                    let value = from_utf8(value)
                        .map_err(|_| E::invalid_value(de::Unexpected::Bytes(value), &self))?;
                    self.visit_str(value)
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
                type Value = DocCsv;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("a document record with headers")
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
                        let (field, key) = match field {
                            Field::Title => (&mut title, "title"),
                            Field::Id => (&mut id, "id"),
                            Field::Body => (&mut body, "body"),
                            Field::Metadata(key) => {
                                let value = map.next_value()?;
                                metadata.get_or_insert_default().insert(key, value);
                                continue;
                            }
                        };
                        if field.is_some() {
                            return Err(de::Error::duplicate_field(key));
                        }
                        *field = Some(map.next_value()?);
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

                    Ok(DocCsv {
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

#[cfg(test)]
mod tests {
    use crate::{Body, IntoParser, Metadata, ParsedDoc, Parser, Title};

    use std::fmt::Debug;

    pub(super) const VALID: &str =
        "id,title,body,author\n1,春,\"あけぼの。\n山ぎは\",清少納言\n2,夏,\"夜,月\",作者\n";
    pub(super) const INVALID: &str = "title,body\n春,あけぼの。\n夏\n秋,夕暮れ。\n";

    pub(super) fn doc(title: &str, body: &str, metadata: &[(&str, &str)]) -> ParsedDoc {
        ParsedDoc {
            title: Title(title.to_owned()),
            body: Body(body.to_owned()),
            metadata: (!metadata.is_empty()).then(|| {
                Metadata(
                    metadata
                        .iter()
                        .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                        .collect(),
                )
            }),
        }
    }

    pub(super) fn expected() -> Vec<ParsedDoc> {
        vec![
            doc(
                "1",
                "あけぼの。\n山ぎは",
                &[("title", "春"), ("author", "清少納言")],
            ),
            doc("2", "夜,月", &[("title", "夏"), ("author", "作者")]),
        ]
    }

    pub(super) async fn collect<S: IntoParser>(source: S) -> (Vec<ParsedDoc>, bool)
    where
        S::Error: Debug,
    {
        let mut parser = source.parser().await.unwrap();
        let mut docs = Vec::new();
        while let Some(result) = parser.next().await {
            match result {
                Ok(doc) => docs.push(doc),
                Err(_) => return (docs, true),
            }
        }
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
        assert!(parser.next().await.is_none());
        (docs, false)
    }
}
