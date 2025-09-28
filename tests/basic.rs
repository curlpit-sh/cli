use std::collections::HashMap;
use std::fs;
use std::path::Path;

use anyhow::Result;
use curlpit::config::{load_config, EnvironmentBuilder};
use curlpit::env::expand_placeholders;
use curlpit::importer::{import_curl_command, ImportOptions};
use curlpit::parser::parse_request_file;
use tempfile::tempdir;

#[test]
fn expand_placeholders_respects_escapes() -> Result<()> {
    let mut env = HashMap::new();
    env.insert("NAME".to_string(), "curlpit".to_string());

    let rendered = expand_placeholders(r"Hello \{literal\} {NAME}!", &env)?;
    assert_eq!(rendered, "Hello {literal} curlpit!");
    Ok(())
}

#[tokio::test]
async fn parse_request_file_resolves_templates() -> Result<()> {
    let temp = tempdir()?;
    let base = temp.path();

    write_file(
        base.join("curlpit.json"),
        r#"{
  "defaultProfile": "test",
  "profiles": {
    "test": {
      "env": "example.env",
      "variables": {
        "API_BASE": "https://example.com"
      }
    }
  }
}
"#,
    )?;

    write_file(base.join("example.env"), "API_TOKEN=token-123\n")?;

    write_file(
        base.join("sample.curl"),
        r#"# comment
@env extra.env
POST {API_BASE}/widgets
authorization: Bearer {API_TOKEN}
Content-Type: application/json

{"ok":true}
"#,
    )?;

    write_file(base.join("extra.env"), "API_TOKEN=overridden\n")?;

    let config = load_config(base)?.expect("config should load");
    let builder = EnvironmentBuilder::new(
        base.to_path_buf(),
        base.to_path_buf(),
        Some(config.clone()),
        Some("test".to_string()),
        None,
        None,
    );

    let environment = builder.build().await?;
    let parsed = parse_request_file(&base.join("sample.curl"), &environment).await?;

    assert_eq!(parsed.request.method, "POST");
    assert_eq!(parsed.request.url, "https://example.com/widgets");
    assert_eq!(parsed.request.headers.len(), 2);
    assert_eq!(parsed.request.body_bytes, Some(11));
    assert_eq!(parsed.request.body_text.as_deref(), Some("{\"ok\":true}"));
    assert!(parsed
        .env_files
        .iter()
        .any(|path| path.file_name().unwrap() == "example.env"));
    assert!(parsed
        .env_files
        .iter()
        .any(|path| path.file_name().unwrap() == "extra.env"));

    Ok(())
}

#[test]
fn import_curl_command_performs_substitutions() -> Result<()> {
    let mut template_vars = HashMap::new();
    template_vars.insert(
        "API_BASE".to_string(),
        "https://api.example.com".to_string(),
    );

    let mut env_vars = HashMap::new();
    env_vars.insert("API_TOKEN".to_string(), "secret-token".to_string());

    let result = import_curl_command(&ImportOptions {
        command: r#"curl -X POST https://api.example.com/widgets -H 'Authorization: Bearer secret-token' -H 'Content-Type: application/json' -d '{"ok":true}'"#,
        template_variables: &template_vars,
        env_variables: &env_vars,
        include_headers: None,
        exclude_headers: None,
        append_headers: None,
    })?;

    assert!(result.contents.contains("POST {API_BASE}/widgets"));
    assert!(result
        .contents
        .contains("authorization: Bearer {API_TOKEN}"));
    assert!(result.contents.contains("{\"ok\":true}"));
    Ok(())
}

fn write_file(path: impl AsRef<Path>, contents: &str) -> Result<()> {
    fs::write(path, contents)?;
    Ok(())
}
