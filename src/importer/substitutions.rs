use std::collections::HashMap;

use chrono::Utc;
use url::Url;

pub(crate) fn build_substitutions(
    template_vars: &HashMap<String, String>,
    env_vars: &HashMap<String, String>,
) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = template_vars
        .iter()
        .chain(env_vars.iter())
        .filter_map(|(key, value)| {
            if value.is_empty() {
                None
            } else {
                Some((key.clone(), value.clone()))
            }
        })
        .collect();

    entries.sort_by(|a, b| b.1.len().cmp(&a.1.len()));
    entries
}

pub(crate) fn apply_substitutions(input: &str, substitutions: &[(String, String)]) -> String {
    let mut output = input.to_string();
    for (key, value) in substitutions {
        if !value.is_empty() && output.contains(value) {
            output = output.replace(value, &format!("{{{key}}}"));
        }
    }
    output
}

pub(crate) fn format_curl_contents(
    method: &str,
    url: &str,
    headers: &[(String, String)],
    body_text: Option<&str>,
    warnings: &[String],
) -> String {
    let mut lines = Vec::new();
    let timestamp = Utc::now().to_rfc3339();
    lines.push(format!("# Imported by curlpit on {timestamp}"));
    for warning in warnings {
        lines.push(format!("# WARNING: {warning}"));
    }

    lines.push(format!("{method} {url}"));
    for (name, value) in headers {
        lines.push(format!("{}: {}", name, value));
    }

    if body_text.is_some() {
        lines.push(String::new());
    }

    if let Some(text) = body_text {
        lines.push(text.to_string());
    }

    lines.push(String::new());
    lines.join("\n")
}

pub(crate) fn suggest_file_name(method: &str, url: &str) -> Option<String> {
    let parsed = Url::parse(url).ok()?;
    let host = parsed.host_str()?.replace('.', "-");
    let segments: Vec<String> = parsed
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .map(|segment| segment.replace(|c: char| !c.is_ascii_alphanumeric(), "-"))
                .collect()
        })
        .unwrap_or_default();

    let path_part = if segments.is_empty() {
        "root".to_string()
    } else {
        segments.join("-")
    };

    let base = format!("{}-{}-{}", method.to_lowercase(), host, path_part)
        .replace("--", "-")
        .trim_matches('-')
        .to_string();

    Some(format!(
        "{}.curl",
        if base.is_empty() { "request" } else { &base }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutions_favor_longer_matches_first() {
        let mut template = HashMap::new();
        template.insert("LONG".to_string(), "abcdef".to_string());
        template.insert("SHORT".to_string(), "abc".to_string());
        let substitutions = build_substitutions(&template, &HashMap::new());

        let replaced = apply_substitutions("abcdef", &substitutions);
        assert_eq!(replaced, "{LONG}");
    }

    #[test]
    fn suggest_file_name_produces_sanitized_output() {
        let name = suggest_file_name("POST", "https://api.example.com/a/b?c=d").unwrap();
        assert_eq!(name, "post-api-example-com-a-b.curl");
    }
}
