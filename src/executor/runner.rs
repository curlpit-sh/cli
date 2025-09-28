use std::time::Instant;

use anyhow::{Context, Result};
use reqwest::{header::HeaderMap, Client, Method};

use crate::parser::{parse_request_file, ParsedRequest, RequestBody};

use super::{
    models::{ExecutionOptions, ExecutionResult, RequestSummary, ResponseSummary},
    writer::{create_preview, write_response_body},
};

pub async fn execute_request_file(
    path: &std::path::Path,
    options: ExecutionOptions<'_>,
) -> Result<ExecutionResult> {
    let parsed = parse_request_file(path, options.environment).await?;
    execute_parsed_request(parsed, path, options).await
}

async fn execute_parsed_request(
    parsed: ParsedRequest,
    request_file: &std::path::Path,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EnvironmentBuilder;
    use anyhow::Result;
    use httpmock::prelude::*;
    use tempfile::tempdir;

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

    #[tokio::test]
    async fn execute_request_file_writes_preview_and_body() -> Result<()> {
        let server = MockServer::start_async().await;
        let _mock = server
            .mock_async(|when, then| {
                when.method(GET).path("/items");
                then.status(200)
                    .header("content-type", "application/json")
                    .body("{\"ok\":true}");
            })
            .await;

        let temp = tempdir()?;
        let request_path = temp.path().join("sample.curl");
        std::fs::write(&request_path, format!("GET {}/items\n", server.url("")))?;

        let response_dir = temp.path().join("responses");
        let builder = EnvironmentBuilder::new(
            temp.path().to_path_buf(),
            temp.path().to_path_buf(),
            None,
            None,
            None,
            Some(response_dir.clone()),
        );

        let environment = builder.build().await?;

        let result = execute_request_file(
            &request_path,
            ExecutionOptions {
                preview_bytes: Some(5),
                environment: &environment,
                response_output_dir: environment.response_output_dir.clone(),
            },
        )
        .await?;

        assert_eq!(result.response.status, 200);
        assert_eq!(result.request.method, "GET");
        let preview = result.response.preview.as_deref().unwrap();
        assert_eq!(preview, r#"{"ok""#);
        assert!(result.response.body_path.starts_with(&response_dir));
        assert_eq!(
            result
                .response
                .body_path
                .extension()
                .and_then(|s| s.to_str()),
            Some("json")
        );

        Ok(())
    }
}
