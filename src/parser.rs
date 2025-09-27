use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use once_cell::sync::Lazy;
use tokio::fs;

use crate::config::EnvironmentContext;
use crate::env::{expand_placeholders, load_env_directive};

static HTTP_METHODS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "TRACE",
    ]
    .into_iter()
    .collect()
});

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

pub async fn parse_request_file(
    path: &Path,
    environment: &EnvironmentContext,
) -> Result<ParsedRequest> {
    let raw = fs::read_to_string(path)
        .await
        .with_context(|| format!("reading request file {}", path.display()))?;

    parse_request_contents(&raw, path, environment).await
}

async fn parse_request_contents(
    contents: &str,
    path: &Path,
    environment: &EnvironmentContext,
) -> Result<ParsedRequest> {
    let mut env = environment.initial_env.clone();
    let mut env_files = environment.env_files.clone();
    let request_dir = path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| environment.base_dir.clone());

    let mut lines = contents.lines().enumerate().peekable();

    let mut method = None;
    let mut url = None;

    while let Some((_, raw_line)) = lines.peek() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.next();
            continue;
        }

        if trimmed.starts_with("@env") {
            lines.next();
            let path_raw = trimmed["@env".len()..].trim();
            if path_raw.is_empty() {
                bail!("@env directive requires a file path");
            }
            let expanded_path = expand_placeholders(path_raw, &env)?;
            let env_path = request_dir.join(expanded_path);
            load_env_directive(&env_path, &mut env, &mut env_files)?;
            continue;
        }

        let expanded = expand_placeholders(trimmed, &env)?;
        let mut parts = expanded.split_whitespace();
        if let Some(first) = parts.next() {
            let upper = first.to_ascii_uppercase();
            if HTTP_METHODS.contains(upper.as_str()) && parts.clone().next().is_some() {
                method = Some(upper);
                url = Some(parts.collect::<Vec<_>>().join(" "));
            } else {
                method = Some("GET".to_string());
                url = Some(expanded);
            }
        }
        lines.next();
        break;
    }

    let method = method.ok_or_else(|| anyhow!("Missing request line"))?;
    let url = url.ok_or_else(|| anyhow!("Missing request URL"))?;

    let mut headers: Vec<(String, String)> = Vec::new();
    while let Some((_, raw_line)) = lines.peek() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            lines.next();
            break;
        }
        if trimmed.starts_with('#') {
            lines.next();
            continue;
        }
        if trimmed.starts_with('@') {
            break;
        }
        lines.next();
        let expanded = expand_placeholders(trimmed, &env)?;
        if let Some((name, value)) = expanded.split_once(':') {
            headers.push((name.trim().to_string(), value.trim().to_string()));
        } else {
            bail!("Invalid header line: {}", trimmed);
        }
    }

    // Consume blank lines between headers and body
    while let Some((_, raw_line)) = lines.peek() {
        if raw_line.trim().is_empty() {
            lines.next();
        } else {
            break;
        }
    }

    let mut body_text: Option<String> = None;
    let mut body_file: Option<PathBuf> = None;
    let mut body_bytes: Option<usize> = None;
    let mut body: Option<RequestBody> = None;

    if let Some((_, raw_line)) = lines.peek() {
        let trimmed = raw_line.trim();
        if trimmed.starts_with("@body") {
            lines.next();
            let body_path_raw = trimmed["@body".len()..].trim();
            if body_path_raw.is_empty() {
                bail!("@body directive requires a file path");
            }
            let expanded_path = expand_placeholders(body_path_raw, &env)?;
            let resolved = request_dir.join(expanded_path);
            let bytes = fs::read(&resolved)
                .await
                .with_context(|| format!("reading body file {}", resolved.display()))?;
            body_bytes = Some(bytes.len());
            body = Some(RequestBody::Bytes(bytes));
            body_file = Some(resolved);
        } else {
            let mut body_lines = Vec::new();
            while let Some((_, line)) = lines.next() {
                body_lines.push(line.to_string());
            }
            let raw_body = body_lines.join("\n");
            if !raw_body.trim().is_empty() {
                let expanded_body = expand_placeholders(&raw_body, &env)?;
                body_bytes = Some(expanded_body.as_bytes().len());
                body_text = Some(expanded_body.clone());
                body = Some(RequestBody::Text(expanded_body));
            }
        }
    }

    Ok(ParsedRequest {
        request: RequestDefinition {
            method,
            url,
            headers,
            body,
            body_bytes,
            body_text,
            body_file,
        },
        env_files,
    })
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
