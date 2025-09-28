use std::path::PathBuf;

use crate::config::EnvironmentContext;

pub struct ExecutionOptions<'a> {
    pub preview_bytes: Option<usize>,
    pub environment: &'a EnvironmentContext,
    pub response_output_dir: Option<PathBuf>,
}

pub struct ExecutionResult {
    pub request: RequestSummary,
    pub response: ResponseSummary,
    pub env_files: Vec<PathBuf>,
}

pub struct RequestSummary {
    pub method: String,
    pub url: String,
    pub body_bytes: Option<usize>,
}

pub struct ResponseSummary {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub duration_ms: f64,
    pub body_path: PathBuf,
    pub body_bytes: usize,
    pub preview: Option<String>,
}
