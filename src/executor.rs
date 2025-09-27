use std::{
    fs,
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result};
use colored::{Color, Colorize};
use petname::petname;
use reqwest::{header::HeaderMap, Client, Method};
use url::Url;
use uuid::Uuid;

use crate::{
    config::EnvironmentContext,
    parser::{parse_request_file, ParsedRequest, RequestBody},
};

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

pub async fn execute_request_file(
    path: &Path,
    options: ExecutionOptions<'_>,
) -> Result<ExecutionResult> {
    let parsed = parse_request_file(path, options.environment).await?;
    execute_parsed_request(parsed, path, options).await
}

async fn execute_parsed_request(
    parsed: ParsedRequest,
    request_file: &Path,
    options: ExecutionOptions<'_>,
) -> Result<ExecutionResult> {
    let client = Client::new();

    let method = Method::from_bytes(parsed.request.method.as_bytes())
        .with_context(|| format!("invalid HTTP method {}", parsed.request.method))?;
    let mut request_builder = client.request(method.clone(), &parsed.request.url);

    for (name, value) in &parsed.request.headers {
        request_builder = request_builder.header(name, value);
    }

    match &parsed.request.body {
        Some(RequestBody::Text(body)) => {
            request_builder = request_builder.body(body.clone());
        }
        Some(RequestBody::Bytes(bytes)) => {
            request_builder = request_builder.body(bytes.clone());
        }
        None => {}
    }

    let start = Instant::now();
    let response = request_builder.send().await?;
    let duration_ms = start.elapsed().as_secs_f64() * 1000.0;

    let status = response.status();
    let header_map = response.headers().clone();
    let content_type_value = header_map
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let bytes = response.bytes().await?;
    let body_bytes = bytes.len();
    let headers = collect_headers(&header_map);

    let body_path = write_response_body(
        &bytes,
        content_type_value.as_deref(),
        options.response_output_dir.as_deref(),
        request_file,
    )?;

    let preview = options
        .preview_bytes
        .filter(|limit| *limit > 0)
        .map(|limit| create_preview(&bytes, limit));

    Ok(ExecutionResult {
        request: RequestSummary {
            method: parsed.request.method,
            url: parsed.request.url,
            body_bytes: parsed.request.body_bytes,
        },
        response: ResponseSummary {
            status: status.as_u16(),
            headers,
            duration_ms,
            body_path,
            body_bytes,
            preview,
        },
        env_files: parsed.env_files,
    })
}

fn collect_headers(headers: &HeaderMap) -> Vec<(String, String)> {
    headers
        .iter()
        .map(|(name, value)| {
            (
                name.as_str().to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect()
}

fn write_response_body(
    bytes: &[u8],
    content_type: Option<&str>,
    response_dir: Option<&Path>,
    request_file: &Path,
) -> Result<PathBuf> {
    let extension = extension_for_content_type(content_type);

    if let Some(base_dir) = response_dir {
        let stem = request_file
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed("request"));
        let sanitized = sanitize_component(&stem);
        let request_dir = base_dir.join(&sanitized);
        fs::create_dir_all(&request_dir)
            .with_context(|| format!("creating response directory {}", request_dir.display()))?;

        let index = next_index(&request_dir)?;
        let pet = petname(2, "-");
        let file_name = format!("{:03}-{}{}", index, pet, extension);
        let path = request_dir.join(file_name);
        fs::write(&path, bytes)
            .with_context(|| format!("writing response body to {}", path.display()))?;
        Ok(path)
    } else {
        let filename = format!("curlpit-{}{}", Uuid::new_v4(), extension);
        let path = std::env::temp_dir().join(filename);
        fs::write(&path, bytes)
            .with_context(|| format!("writing response body to {}", path.display()))?;
        Ok(path)
    }
}

fn sanitize_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "request".to_string()
    } else {
        trimmed.to_string()
    }
}

fn extension_for_content_type(content_type: Option<&str>) -> &'static str {
    match content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
    {
        "application/json" => ".json",
        "text/html" => ".html",
        "text/plain" => ".txt",
        "application/xml" | "text/xml" => ".xml",
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "application/pdf" => ".pdf",
        "application/octet-stream" => ".bin",
        _ => ".bin",
    }
}

fn create_preview(bytes: &[u8], limit: usize) -> String {
    let slice = if bytes.len() > limit {
        &bytes[..limit]
    } else {
        bytes
    };
    match std::str::from_utf8(slice) {
        Ok(text) => text.to_string(),
        Err(_) => hex::encode(slice),
    }
}

pub fn print_execution_result(result: &ExecutionResult) {
    let status_color = if result.response.status >= 400 {
        Color::Red
    } else if result.response.status >= 300 {
        Color::Yellow
    } else {
        Color::Green
    };

    println!(
        "{} {}",
        result.request.method.bold(),
        result.request.url.cyan()
    );
    println!(
        "{} {} {}",
        "Status:".bold(),
        format!("{}", result.response.status).color(status_color),
        format!("({:.1} ms)", result.response.duration_ms).dimmed()
    );

    if let Some(bytes) = result.request.body_bytes {
        println!(
            "{} {}",
            "Request body:".bold(),
            format!("{} bytes", bytes).dimmed()
        );
    }

    if !result.env_files.is_empty() {
        let files = result
            .env_files
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.display().to_string())
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("{} {}", "Env:".bold(), files.dimmed());
    }

    println!("{}", "Response headers".bold());
    for (name, value) in &result.response.headers {
        println!("  {}: {}", name.cyan(), value.dimmed());
    }

    println!(
        "{} {} {}",
        "Body:".bold(),
        format_body_link(&result.response.body_path),
        format!("({} bytes)", result.response.body_bytes).dimmed()
    );

    if let Some(preview) = &result.response.preview {
        println!("{}", "Preview".bold());
        println!("{}", preview.dimmed());
    }
}

fn format_body_link(path: &Path) -> String {
    let display = path.to_string_lossy();
    match Url::from_file_path(path) {
        Ok(url) => format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, display.cyan()),
        Err(_) => display.cyan().to_string(),
    }
}

fn next_index(dir: &Path) -> Result<u32> {
    let mut max_index = 0;
    for entry in
        fs::read_dir(dir).with_context(|| format!("reading directory {}", dir.display()))?
    {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if name.len() >= 3 && name.chars().take(3).all(|c| c.is_ascii_digit()) {
                if let Ok(value) = name[0..3].parse::<u32>() {
                    max_index = max_index.max(value + 1);
                }
            }
        }
    }
    Ok(max_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use regex::Regex;
    use tempfile::tempdir;

    #[test]
    fn sanitize_component_replaces_invalid_characters() {
        assert_eq!(sanitize_component("Hello World!"), "Hello-World");
        assert_eq!(sanitize_component("***"), "request");
        assert_eq!(sanitize_component("foo_bar"), "foo_bar");
    }

    #[test]
    fn extension_for_content_type_matches_common_types() {
        assert_eq!(
            extension_for_content_type(Some("application/json; charset=utf-8")),
            ".json"
        );
        assert_eq!(extension_for_content_type(Some("text/html")), ".html");
        assert_eq!(
            extension_for_content_type(Some("application/unknown")),
            ".bin"
        );
        assert_eq!(extension_for_content_type(None), ".bin");
    }

    #[test]
    fn create_preview_handles_binary_data() {
        let text = create_preview("hello".as_bytes(), 10);
        assert_eq!(text, "hello");

        let binary = create_preview(&[0, 159, 146, 150], 4);
        assert_eq!(binary, "009f9296");
    }

    #[test]
    fn collect_headers_preserves_values() {
        let mut map = HeaderMap::new();
        map.insert("X-Test", "value".parse().unwrap());
        map.insert("content-type", "application/json".parse().unwrap());

        let headers = collect_headers(&map);
        assert!(headers
            .iter()
            .any(|(name, value)| name == "x-test" && value == "value"));
        assert!(headers
            .iter()
            .any(|(name, value)| name == "content-type" && value == "application/json"));
    }

    #[test]
    fn format_body_link_wraps_file_urls() {
        let temp = tempdir().unwrap();
        let path = temp.path().join("result.json");
        std::fs::write(&path, "{}").unwrap();

        let link = format_body_link(&path);
        assert!(link.contains("result.json"));
        assert!(link.contains("\u{1b}]8;;"));
    }

    #[test]
    fn next_index_detects_existing_files() -> Result<()> {
        let temp = tempdir()?;
        std::fs::create_dir_all(temp.path())?;
        std::fs::write(temp.path().join("000-first.bin"), b"one")?;
        std::fs::write(temp.path().join("010-second.bin"), b"two")?;

        let idx = next_index(temp.path())?;
        assert_eq!(idx, 11);
        Ok(())
    }

    #[test]
    fn write_response_body_creates_files_under_response_dir() -> Result<()> {
        let temp = tempdir()?;
        let response_dir = temp.path().join("responses");
        let request_dir = temp.path().join("requests");
        std::fs::create_dir_all(&request_dir)?;
        let request_file = request_dir.join("Sample Request!.curl");
        std::fs::write(&request_file, "")?;

        let first = write_response_body(
            b"{}",
            Some("application/json"),
            Some(response_dir.as_path()),
            &request_file,
        )?;
        let second = write_response_body(
            b"{}",
            Some("application/json"),
            Some(response_dir.as_path()),
            &request_file,
        )?;

        let request_subdir = response_dir.join("Sample-Request");
        assert_eq!(first.parent().unwrap(), request_subdir);
        assert!(first
            .extension()
            .unwrap()
            .to_string_lossy()
            .ends_with("json"));
        assert!(second.parent().unwrap().exists());

        let filename_pattern = Regex::new(r"^\d{3}-[a-z]+-[a-z]+\.json$").unwrap();
        assert!(filename_pattern.is_match(first.file_name().unwrap().to_str().unwrap()));
        assert!(filename_pattern.is_match(second.file_name().unwrap().to_str().unwrap()));
        assert_eq!(&second.file_name().unwrap().to_str().unwrap()[0..3], "001");

        let first_contents = std::fs::read(first)?;
        assert_eq!(first_contents, b"{}");

        Ok(())
    }

    #[test]
    fn write_response_body_defaults_to_temp_dir() -> Result<()> {
        let temp = tempdir()?;
        let request_file = temp.path().join("req.curl");
        std::fs::write(&request_file, "")?;

        let path = write_response_body(b"abc", None, None, &request_file)?;
        assert!(path.exists());
        assert!(path.extension().unwrap().to_string_lossy().ends_with("bin"));
        assert_eq!(std::fs::read(path)?, b"abc");
        Ok(())
    }
}
