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
