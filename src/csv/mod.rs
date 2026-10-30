#[cfg(feature = "csv-bytes")]
pub mod bytes;

#[cfg(feature = "csv-io")]
pub mod io;

use de::DocCsv;
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
                    let value = std::str::from_utf8(value)
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
                        match field {
                            Field::Title => {
                                if title.is_some() {
                                    return Err(de::Error::duplicate_field("title"));
                                }
                                title = Some(map.next_value()?);
                            }
                            Field::Id => {
                                if id.is_some() {
                                    return Err(de::Error::duplicate_field("id"));
                                }
                                id = Some(map.next_value()?);
                            }
                            Field::Body => {
                                if body.is_some() {
                                    return Err(de::Error::duplicate_field("body"));
                                }
                                body = Some(map.next_value()?);
                            }
                            Field::Metadata(key) => {
                                let value = map.next_value()?;
                                metadata.get_or_insert_default().insert(key, value);
                            }
                        }
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
