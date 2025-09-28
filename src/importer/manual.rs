use anyhow::{anyhow, Result};
use shell_words::split;

use super::headers::apply_header_rules;
use super::model::{ImportOptions, ImportResult};
use super::substitutions::{
    apply_substitutions, build_substitutions, format_curl_contents, suggest_file_name,
};

pub(crate) fn import_via_manual(options: &ImportOptions<'_>) -> Result<ImportResult> {
    let tokens = split(options.command.trim()).map_err(|err| anyhow!("{err}"))?;
    if tokens.is_empty() {
        return Err(anyhow!("No command provided"));
    }
    if tokens[0].to_lowercase() != "curl" {
        return Err(anyhow!("Command must start with 'curl'"));
    }

    let parsed = parse_tokens(&tokens)?;
    let method = parsed.method.clone().unwrap_or_else(|| {
        if parsed.body_text.is_some() || parsed.body_file.is_some() {
            "POST".to_string()
        } else {
            "GET".to_string()
        }
    });
    let url = parsed
        .url
        .clone()
        .ok_or_else(|| anyhow!("Unable to determine request URL"))?;

    let substitutions = build_substitutions(options.template_variables, options.env_variables);
    let substituted_url = apply_substitutions(&url, &substitutions);
    let substituted_headers: Vec<(String, String)> = parsed
        .headers
        .iter()
        .map(|(name, value)| (name.clone(), apply_substitutions(value, &substitutions)))
        .collect();
    let final_headers = apply_header_rules(
        substituted_headers,
        options.include_headers,
        options.exclude_headers,
        options.append_headers,
    );
    let substituted_body = parsed
        .body_text
        .as_ref()
        .map(|body| apply_substitutions(body, &substitutions));

    let contents = format_curl_contents(
        &method,
        &substituted_url,
        &final_headers,
        substituted_body.as_deref(),
        &parsed.warnings,
    );

    Ok(ImportResult {
        contents,
        suggested_filename: suggest_file_name(&method, &url),
        method,
        url: substituted_url,
        warnings: parsed.warnings,
    })
}

#[derive(Debug, Default)]
struct ParsedCurl {
    method: Option<String>,
    url: Option<String>,
    headers: Vec<(String, String)>,
    body_text: Option<String>,
    body_file: Option<String>,
    warnings: Vec<String>,
}

const DATA_OPTIONS: &[&str] = &[
    "-d",
    "--data",
    "--data-raw",
    "--data-binary",
    "--data-urlencode",
    "--data-ascii",
];

fn parse_tokens(tokens: &[String]) -> Result<ParsedCurl> {
    let mut parsed = ParsedCurl::default();
    let mut index = 1;

    while index < tokens.len() {
        let token = &tokens[index];
        if token == "--" {
            break;
        }

        if token.starts_with('-') {
            match token.as_str() {
                "-X" | "--request" => {
                    let value = next_value(tokens, &mut index)?;
                    parsed.method = Some(value.to_uppercase());
                }
                opt if opt.starts_with("-X") && opt.len() > 2 => {
                    parsed.method = Some(opt[2..].to_uppercase());
                    index += 1;
                    continue;
                }
                opt if opt.starts_with("--request=") => {
                    parsed.method = Some(opt[10..].to_uppercase());
                    index += 1;
                    continue;
                }
                "-H" | "--header" => {
                    let value = next_value(tokens, &mut index)?;
                    if let Some(header) = parse_header(&value) {
                        parsed.headers.push(header);
                    } else {
                        parsed
                            .warnings
                            .push(format!("Unrecognised header: {value}"));
                    }
                }
                opt if opt.starts_with("-H") && opt.len() > 2 => {
                    let value = opt[2..].to_string();
                    if let Some(header) = parse_header(&value) {
                        parsed.headers.push(header);
                    } else {
                        parsed
                            .warnings
                            .push(format!("Unrecognised header: {value}"));
                    }
                    index += 1;
                }
                opt if opt.starts_with("--header=") => {
                    let value = opt[9..].to_string();
                    if let Some(header) = parse_header(&value) {
                        parsed.headers.push(header);
                    } else {
                        parsed
                            .warnings
                            .push(format!("Unrecognised header: {value}"));
                    }
                    index += 1;
                }
                "--url" => {
                    let value = next_value(tokens, &mut index)?;
                    parsed.url = Some(value);
                }
                opt if opt.starts_with("--url=") => {
                    parsed.url = Some(opt[6..].to_string());
                    index += 1;
                    continue;
                }
                "--json" => {
                    let json_value = next_value(tokens, &mut index)?;
                    ensure_json_header(&mut parsed.headers);
                    parsed.body_text = Some(json_value);
                }
                opt if opt.starts_with("--json=") => {
                    let json_value = opt[7..].to_string();
                    ensure_json_header(&mut parsed.headers);
                    parsed.body_text = Some(json_value);
                    index += 1;
                    continue;
                }
                "-u" | "--user" => {
                    let value = next_value(tokens, &mut index)?;
                    parsed.headers.push(build_basic_auth_header(&value));
                }
                opt if opt.starts_with("-u") && opt.len() > 2 => {
                    let value = opt[2..].to_string();
                    parsed.headers.push(build_basic_auth_header(&value));
                    index += 1;
                    continue;
                }
                opt if opt.starts_with("--user=") => {
                    let value = opt[7..].to_string();
                    parsed.headers.push(build_basic_auth_header(&value));
                    index += 1;
                    continue;
                }
                option if DATA_OPTIONS.contains(&option) => {
                    let value = next_value(tokens, &mut index)?;
                    handle_data_option(&mut parsed, value);
                    continue;
                }
                option
                    if DATA_OPTIONS
                        .iter()
                        .any(|prefix| option.starts_with(prefix) && option.contains('=')) =>
                {
                    let value = option
                        .split_once('=')
                        .map(|(_, v)| v.to_string())
                        .unwrap_or_default();
                    handle_data_option(&mut parsed, value);
                    index += 1;
                }
                _ => {
                    parsed.warnings.push(format!("Ignored option: {token}"));
                    index += 1;
                }
            }
        } else {
            if parsed.url.is_none() {
                parsed.url = Some(token.clone());
            } else {
                parsed
                    .warnings
                    .push(format!("Ignoring positional argument: {token}"));
            }
            index += 1;
        }
    }

    Ok(parsed)
}

fn ensure_json_header(headers: &mut Vec<(String, String)>) {
    if !headers
        .iter()
        .any(|(name, _)| name.eq_ignore_ascii_case("content-type"))
    {
        headers.push(("Content-Type".to_string(), "application/json".to_string()));
    }
}

fn next_value(tokens: &[String], index: &mut usize) -> Result<String> {
    let value_pos = *index + 1;
    if value_pos >= tokens.len() {
        return Err(anyhow!("Option '{}' missing value", tokens[*index]));
    }
    let value = tokens[value_pos].clone();
    *index += 2;
    Ok(value)
}

fn handle_data_option(parsed: &mut ParsedCurl, value: String) {
    if value.starts_with('@') {
        if parsed.body_file.is_none() {
            parsed.body_file = Some(value.trim_start_matches('@').to_string());
        } else {
            parsed
                .warnings
                .push("Multiple @file bodies detected; only the first is kept".to_string());
        }
    } else {
        parsed.body_text = Some(value);
    }
}

fn parse_header(value: &str) -> Option<(String, String)> {
    let (name, val) = value.split_once(':')?;
    Some((name.trim().to_string(), val.trim().to_string()))
}

fn build_basic_auth_header(value: &str) -> (String, String) {
    ("Authorization".to_string(), format!("Basic {}", value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;

    static TEMPLATE: Lazy<HashMap<String, String>> = Lazy::new(|| {
        let mut map = HashMap::new();
        map.insert(
            "API_BASE".to_string(),
            "https://api.example.com".to_string(),
        );
        map
    });
    static ENV: Lazy<HashMap<String, String>> = Lazy::new(|| {
        let mut map = HashMap::new();
        map.insert("API_TOKEN".to_string(), "secret-token".to_string());
        map
    });

    fn options(command: &str) -> ImportOptions<'_> {
        ImportOptions {
            command,
            template_variables: &TEMPLATE,
            env_variables: &ENV,
            include_headers: None,
            exclude_headers: None,
            append_headers: None,
        }
    }

    #[test]
    fn import_via_manual_handles_data_files_and_basic_auth() -> Result<()> {
        let command = "curl --request PATCH https://api.example.com/items -H 'X-Test: value' --data '@/tmp/input.json' --data '@/tmp/other.json' --user user:pass";
        let result = import_via_manual(&options(command))?;

        assert_eq!(result.method, "PATCH");
        assert!(result
            .warnings
            .iter()
            .any(|w| w.contains("Multiple @file bodies")));
        assert!(result.contents.contains("PATCH {API_BASE}/items"));
        assert!(result.contents.contains("Authorization: Basic user:pass"));
        assert!(result.contents.contains("X-Test: value"));
        Ok(())
    }

    #[test]
    fn header_rules_are_applied_to_manual_import() -> Result<()> {
        let include = ["Accept".to_string(), "X-Custom".to_string()];
        let exclude = ["X-Custom".to_string()];
        let mut append = HashMap::new();
        append.insert("X-Trace".to_string(), "trace-id".to_string());

        let options = ImportOptions {
            command:
                "curl https://api.example.com --header 'Accept: */*' --header 'X-Custom: keep'",
            template_variables: &TEMPLATE,
            env_variables: &ENV,
            include_headers: Some(&include),
            exclude_headers: Some(&exclude),
            append_headers: Some(&append),
        };

        let result = import_via_manual(&options)?;
        assert!(result.contents.contains("Accept: */*"));
        assert!(!result.contents.contains("X-Custom"));
        assert!(result.contents.contains("X-Trace: trace-id"));
        Ok(())
    }
}
