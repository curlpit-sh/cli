use std::path::PathBuf;

#[derive(Debug, Clone)]
pub enum RequestBody {
    Text(String),
    Bytes(Vec<u8>),
}

#[derive(Debug, Clone)]
pub struct RequestDefinition {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<RequestBody>,
    pub body_bytes: Option<usize>,
    pub body_text: Option<String>,
    pub body_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ParsedRequest {
    pub request: RequestDefinition,
    pub env_files: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct RequestTemplate {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body_text: Option<String>,
    pub body_file: Option<PathBuf>,
}

impl From<&RequestDefinition> for RequestTemplate {
    fn from(value: &RequestDefinition) -> Self {
        Self {
            method: value.method.clone(),
            url: value.url.clone(),
            headers: value.headers.clone(),
            body_text: value.body_text.clone(),
            body_file: value.body_file.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn request_template_from_definition_preserves_fields() {
        let definition = RequestDefinition {
            method: "POST".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![("Content-Type".to_string(), "application/json".to_string())],
            body: Some(RequestBody::Text("{}".to_string())),
            body_bytes: Some(2),
            body_text: Some("{}".to_string()),
            body_file: Some(PathBuf::from("payload.json")),
        };

        let template = RequestTemplate::from(&definition);
        assert_eq!(template.method, "POST");
        assert_eq!(template.url, "https://example.com");
        assert_eq!(template.headers.len(), 1);
        assert_eq!(template.body_text.as_deref(), Some("{}"));
        assert_eq!(
            template.body_file.as_deref(),
            Some(Path::new("payload.json"))
        );
    }
}
