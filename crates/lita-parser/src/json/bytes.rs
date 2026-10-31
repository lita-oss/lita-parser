use super::JsonParser;
use crate::IntoParser;

use std::borrow::Cow;

use serde_json::Error;

pub struct Json<S> {
    bytes: S,
}

impl<S> Json<S> {
    pub fn from(json: S) -> Self {
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

impl IntoParser for Json<&str> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(self.bytes)
    }
}

impl IntoParser for Json<Cow<'_, str>> {
    type Error = Error;
    type Parser = JsonParser;

    async fn parser(self) -> Result<Self::Parser, Self::Error> {
        JsonParser::from_str(&self.bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::Json;
    use crate::json::tests::{INPUT, collect, expected};

    #[tokio::test]
    async fn parses_borrowed_json() {
        let input = Json::<&str>::from(INPUT);
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[tokio::test]
    async fn parses_owned_json() {
        let input = Json::<String>::from(INPUT.to_owned());
        assert_eq!(collect(input).await.unwrap(), expected());
    }

    #[tokio::test]
    async fn rejects_invalid_json() {
        for json in ["", "[", r#"[{"title":"春"}]"#, "[] []"] {
            assert!(collect(Json::<&str>::from(json)).await.is_err());
            assert!(
                collect(Json::<String>::from(json.to_owned()))
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn rejects_invalid_document_shapes() {
        for input in [
            "",
            "{}",
            "null",
            "[null]",
            "[{}]",
            r#"[{"title":"春"}]"#,
            r#"[{"body":"春はあけぼの。"}]"#,
            r#"[{"title":1,"body":"text"}]"#,
            r#"[{"id":1,"body":"text"}]"#,
            r#"[{"title":"春","body":null}]"#,
            r#"[{"title":"春","body":"text","author":{}}]"#,
            r#"[{"title":"春","title":"夏","body":"text"}]"#,
            r#"[{"id":"1","id":"2","body":"text"}]"#,
            r#"[{"title":"春","body":"text","body":"other"}]"#,
            r#"[{"title":"春","body":"text"},{}]"#,
            r#"[{"title":"春","body":"text"}] trailing"#,
            r#"[{"title":"春","body":"text"},]"#,
        ] {
            let json = Json::<&str>::from(input);
            assert!(collect(json).await.is_err(), "{input}");
        }
    }
}
