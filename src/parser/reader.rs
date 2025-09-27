use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};
use once_cell::sync::Lazy;
use tokio::fs;

use crate::config::EnvironmentContext;
use crate::env::{expand_placeholders, load_env_directive};

use super::model::{ParsedRequest, RequestBody, RequestDefinition};

static HTTP_METHODS: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    [
        "GET", "POST", "PUT", "PATCH", "DELETE", "OPTIONS", "HEAD", "TRACE",
    ]
    .into_iter()
    .collect()
});

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::EnvironmentContext;
    use crate::env::EnvMap;
    use anyhow::Result;
    use tempfile::tempdir;

    fn base_environment(base: &Path) -> EnvironmentContext {
        EnvironmentContext {
            base_dir: base.to_path_buf(),
            config_dir: base.to_path_buf(),
            template_variables: EnvMap::new(),
            initial_env: EnvMap::new(),
            env_files: Vec::new(),
            profile_name: None,
            response_output_dir: None,
        }
    }

    #[tokio::test]
    async fn parse_request_file_resolves_body_directive() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();

        let mut env_context = base_environment(base);
        env_context
            .initial_env
            .insert("PAYLOAD".to_string(), "payload.bin".to_string());

        let payload_path = base.join("payload.bin");
        tokio::fs::write(&payload_path, b"\x00\x01\x02").await?;

        let request_path = base.join("request.curl");
        tokio::fs::write(&request_path, "POST https://example.com\n@body {PAYLOAD}\n").await?;

        let parsed = parse_request_file(&request_path, &env_context).await?;

        assert_eq!(parsed.request.method, "POST");
        assert!(matches!(
            parsed.request.body,
            Some(RequestBody::Bytes(ref bytes)) if bytes == &[0, 1, 2]
        ));
        assert_eq!(parsed.request.body_file, Some(payload_path));
        assert_eq!(parsed.request.body_bytes, Some(3));

        Ok(())
    }

    #[tokio::test]
    async fn parse_request_file_reports_missing_placeholders() {
        let temp = tempdir().unwrap();
        let base = temp.path();
        let env_context = base_environment(base);

        let request_path = base.join("missing.curl");
        tokio::fs::write(&request_path, "GET {UNKNOWN}\n")
            .await
            .unwrap();

        let err = parse_request_file(&request_path, &env_context)
            .await
            .expect_err("expected missing placeholder error");
        assert!(err
            .to_string()
            .contains("Missing template variable: UNKNOWN"));
    }

    #[tokio::test]
    async fn parse_request_file_validates_headers() {
        let temp = tempdir().unwrap();
        let base = temp.path();
        let env_context = base_environment(base);

        let request_path = base.join("bad_header.curl");
        tokio::fs::write(&request_path, "GET https://example.com\nInvalidHeader\n")
            .await
            .unwrap();

        let err = parse_request_file(&request_path, &env_context)
            .await
            .expect_err("expected invalid header error");
        assert!(err.to_string().contains("Invalid header line"));
    }

    #[tokio::test]
    async fn parse_request_file_tracks_env_directives() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();

        let env_context = base_environment(base);
        let env_file = base.join("extra.env");
        tokio::fs::write(&env_file, "PATH_SEGMENT=widgets\nTOKEN=abc123\n").await?;

        let request_path = base.join("env.curl");
        tokio::fs::write(
            &request_path,
            "@env extra.env\nGET https://example.com/{PATH_SEGMENT}\nauthorization: Bearer {TOKEN}\n",
        )
        .await?;

        let parsed = parse_request_file(&request_path, &env_context).await?;

        assert_eq!(parsed.env_files.len(), 1);
        assert!(parsed.env_files[0].ends_with("extra.env"));
        assert_eq!(parsed.request.url, "https://example.com/widgets");
        assert_eq!(parsed.request.headers.len(), 1);
        assert_eq!(
            parsed.request.headers[0],
            ("authorization".to_string(), "Bearer abc123".to_string())
        );

        Ok(())
    }
}
