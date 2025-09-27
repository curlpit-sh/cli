use std::str::FromStr;

use anyhow::{anyhow, Result};
use curl_parser::ParsedRequest;

use super::model::{ImportOptions, ImportResult};
use super::substitutions::{
    apply_substitutions, build_substitutions, format_curl_contents, suggest_file_name,
};

pub(crate) fn import_via_curl_parser(options: &ImportOptions<'_>) -> Result<ImportResult> {
    let parsed = ParsedRequest::from_str(options.command.trim())
        .map_err(|error| anyhow!("Failed to parse curl command: {error}"))?;

    let method = parsed.method.as_str().to_string();
    let url = parsed.url.to_string();

    let mut warnings = Vec::new();
    if parsed.insecure {
        warnings.push("--insecure detected; certificate validation disabled".to_string());
    }

    let headers: Vec<(String, String)> = parsed
        .headers
        .iter()
        .map(|(name, value)| {
            (
                name.to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();

    let body_text = if parsed.body.is_empty() {
        None
    } else {
        Some(parsed.body.join("\n"))
    };

    let substitutions = build_substitutions(options.template_variables, options.env_variables);

    let substituted_url = apply_substitutions(&url, &substitutions);
    let substituted_headers: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| (name.clone(), apply_substitutions(value, &substitutions)))
        .collect();
    let substituted_body = body_text
        .as_ref()
        .map(|body| apply_substitutions(body, &substitutions));

    let contents = format_curl_contents(
        &method,
        &substituted_url,
        &substituted_headers,
        substituted_body.as_deref(),
        &warnings,
    );

    Ok(ImportResult {
        contents,
        suggested_filename: suggest_file_name(&method, &url),
        method,
        url: substituted_url,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use anyhow::Result;

    fn options(command: &str) -> ImportOptions<'_> {
        static TEMPLATE: once_cell::sync::Lazy<HashMap<String, String>> =
            once_cell::sync::Lazy::new(|| {
                let mut map = HashMap::new();
                map.insert(
                    "API_BASE".to_string(),
                    "https://api.example.com".to_string(),
                );
                map
            });
        static ENV: once_cell::sync::Lazy<HashMap<String, String>> =
            once_cell::sync::Lazy::new(|| {
                let mut map = HashMap::new();
                map.insert("API_TOKEN".to_string(), "secret-token".to_string());
                map
            });
        ImportOptions {
            command,
            template_variables: &TEMPLATE,
            env_variables: &ENV,
        }
    }

    #[test]
    fn import_via_curl_parser_emits_warnings_and_substitutions() -> Result<()> {
        let command = "curl --insecure https://api.example.com/widgets -H 'Authorization: Bearer secret-token'";
        let result = import_via_curl_parser(&options(command))?;

        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("--insecure")));
        assert!(result.contents.contains("GET {API_BASE}/widgets"));
        assert!(result
            .contents
            .contains("authorization: Bearer {API_TOKEN}"));
        Ok(())
    }
}
