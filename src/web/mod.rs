use std::collections::{HashMap, HashSet};

use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use thiserror::Error;

use crate::env::{expand_placeholders, EnvMap};
use crate::importer::{import_curl_command, ImportOptions, ImportResult};
use crate::parser::RequestTemplate;
use crate::template;

#[derive(Debug, Error)]
pub enum WebProcessError {
    #[error("{0}")]
    Message(String),
}

pub type WebResult<T> = Result<T, WebProcessError>;

#[derive(Debug, Serialize)]
pub struct WebHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct WebRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<WebHeader>,
    pub body: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct WebInterpolationDetail {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct WebProcessedRequest {
    pub request: WebRequest,
    pub interpolation: Vec<WebInterpolationDetail>,
}

static HTTP_METHODS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "TRACE",
    ]
    .into_iter()
    .collect()
});

static PLACEHOLDER_PATTERN: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\{([A-Z_][A-Z0-9_\-.]*)\}").expect("valid regex"));

pub fn process_request(
    curl: &str,
    env: &HashMap<String, String>,
) -> WebResult<WebProcessedRequest> {
    let request = parse_request(curl, env)?;
    let interpolation = collect_interpolation(curl, env);

    Ok(WebProcessedRequest {
        request,
        interpolation,
    })
}

pub fn import_curl_command_web(
    command: &str,
    template_variables: &HashMap<String, String>,
    env_variables: &HashMap<String, String>,
) -> WebResult<ImportResult> {
    import_curl_command(&ImportOptions {
        command,
        template_variables,
        template_variants: &[],
        env_variables,
        include_headers: None,
        exclude_headers: None,
        append_headers: None,
    })
    .map_err(|err| WebProcessError::Message(err.to_string()))
}

pub fn render_export_template_web(
    name: &str,
    curl: &str,
    env: &HashMap<String, String>,
) -> WebResult<String> {
    let processed = process_request(curl, env)?;
    let headers = processed
        .request
        .headers
        .into_iter()
        .map(|h| (h.name, h.value))
        .collect();

    let template = RequestTemplate {
        method: processed.request.method,
        url: processed.request.url,
        headers,
        body_text: processed.request.body.clone(),
        body_file: None,
    };

    template::render_export_template(name, &template)
        .map_err(|err| WebProcessError::Message(err.to_string()))
}

fn parse_request(curl: &str, env: &EnvMap) -> WebResult<WebRequest> {
    let mut lines = curl.lines().peekable();

    let (method, url) = parse_request_line(&mut lines, env)?;
    let headers = parse_headers(&mut lines, env)?;
    let body = parse_body(&mut lines, env)?;

    Ok(WebRequest {
        method,
        url,
        headers,
        body,
    })
}

fn parse_request_line<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    env: &EnvMap,
) -> WebResult<(String, String)>
where
    I: Iterator<Item = &'a str>,
{
    while let Some(raw_line) = lines.peek().copied() {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            lines.next();
            continue;
        }
        if trimmed.starts_with("@env") {
            return Err(WebProcessError::Message(
                "@env directives are not supported in the web playground".into(),
            ));
        }
        if trimmed.starts_with('@') {
            return Err(WebProcessError::Message(format!(
                "Directive '{trimmed}' is not supported in the web playground"
            )));
        }

        lines.next();
        let expanded = expand_placeholders(trimmed, env)
            .map_err(|err| WebProcessError::Message(err.to_string()))?;
        let mut parts = expanded.split_whitespace();
        if let Some(first) = parts.next() {
            let upper = first.to_ascii_uppercase();
            if HTTP_METHODS.contains(upper.as_str()) {
                let url = parts.collect::<Vec<_>>().join(" ");
                if url.is_empty() {
                    return Err(WebProcessError::Message("Missing request URL".into()));
                }
                return Ok((upper, url));
            } else {
                return Ok(("GET".to_string(), expanded));
            }
        }
    }

    Err(WebProcessError::Message("Missing request line".into()))
}

fn parse_headers<'a, I>(
    lines: &mut std::iter::Peekable<I>,
    env: &EnvMap,
) -> WebResult<Vec<WebHeader>>
where
    I: Iterator<Item = &'a str>,
{
    let mut headers = Vec::new();

    while let Some(raw_line) = lines.peek().copied() {
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
            return Err(WebProcessError::Message(format!(
                "Directive '{trimmed}' is not supported in the web playground"
            )));
        }

        lines.next();
        let expanded = expand_placeholders(trimmed, env)
            .map_err(|err| WebProcessError::Message(err.to_string()))?;
        let (name, value) = expanded
            .split_once(':')
            .ok_or_else(|| WebProcessError::Message(format!("Invalid header line: {trimmed}")))?;
        headers.push(WebHeader {
            name: name.trim().to_string(),
            value: value.trim().to_string(),
        });
    }

    Ok(headers)
}

fn parse_body<'a, I>(lines: &mut std::iter::Peekable<I>, env: &EnvMap) -> WebResult<Option<String>>
where
    I: Iterator<Item = &'a str>,
{
    while let Some(raw_line) = lines.peek().copied() {
        if raw_line.trim().is_empty() {
            lines.next();
        } else {
            break;
        }
    }

    if lines.peek().is_none() {
        return Ok(None);
    }

    let mut body_lines = Vec::new();
    for line in lines.by_ref() {
        body_lines.push(line);
    }
    let raw_body = body_lines.join("\n");
    if raw_body.trim().is_empty() {
        return Ok(None);
    }

    let expanded = expand_placeholders(&raw_body, env)
        .map_err(|err| WebProcessError::Message(err.to_string()))?;
    Ok(Some(expanded))
}

fn collect_interpolation(curl: &str, env: &EnvMap) -> Vec<WebInterpolationDetail> {
    let mut seen = HashSet::new();
    let mut result = Vec::new();

    for caps in PLACEHOLDER_PATTERN.captures_iter(curl) {
        if let Some(key) = caps.get(1) {
            let key_str = key.as_str();
            if seen.insert(key_str.to_string()) {
                if let Some(value) = env.get(key_str) {
                    result.push(WebInterpolationDetail {
                        key: key_str.to_string(),
                        value: value.clone(),
                    });
                }
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env_map() -> EnvMap {
        HashMap::from([
            ("API_BASE".into(), "https://example.com".into()),
            ("TOKEN".into(), "secret".into()),
            ("USER".into(), "demo".into()),
        ])
    }

    #[test]
    fn parse_simple_request() {
        let curl = r"GET {API_BASE}/users\nAuthorization: Bearer {TOKEN}\n\nHello {USER}";
        let env = env_map();
        let processed = process_request(curl, &env).unwrap();
        assert_eq!(processed.request.method, "GET");
        assert_eq!(processed.request.url, "https://example.com/users");
        assert_eq!(processed.request.headers.len(), 1);
        assert_eq!(processed.request.body.unwrap(), "Hello demo");
        assert_eq!(processed.interpolation.len(), 3);
    }

    #[test]
    fn errors_on_missing_variable() {
        let curl = "GET {UNKNOWN}";
        let env = EnvMap::new();
        let err = process_request(curl, &env).unwrap_err();
        assert!(
            matches!(err, WebProcessError::Message(message) if message.contains("Missing template variable"))
        );
    }
}
