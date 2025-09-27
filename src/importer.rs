use std::collections::HashMap;
use std::str::FromStr;

use anyhow::{anyhow, Result};
use chrono::Utc;
use curl_parser::ParsedRequest;
use shell_words::split;
use url::Url;

#[derive(Debug, Clone)]
pub struct ImportOptions<'a> {
    pub command: &'a str,
    pub template_variables: &'a HashMap<String, String>,
    pub env_variables: &'a HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub contents: String,
    pub suggested_filename: Option<String>,
    pub method: String,
    pub url: String,
    pub warnings: Vec<String>,
}

pub fn import_curl_command(options: &ImportOptions<'_>) -> Result<ImportResult> {
    match import_via_curl_parser(options) {
        Ok(result) => Ok(result),
        Err(primary) => {
            import_via_manual(options).map_err(|fallback| anyhow!("{}\n{}", primary, fallback))
        }
    }
}

fn import_via_curl_parser(options: &ImportOptions<'_>) -> Result<ImportResult> {
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

fn import_via_manual(options: &ImportOptions<'_>) -> Result<ImportResult> {
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
    let substituted_body = parsed
        .body_text
        .as_ref()
        .map(|body| apply_substitutions(body, &substitutions));

    let contents = format_curl_contents(
        &method,
        &substituted_url,
        &substituted_headers,
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

fn build_substitutions(
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

fn apply_substitutions(input: &str, substitutions: &[(String, String)]) -> String {
    let mut output = input.to_string();
    for (key, value) in substitutions {
        if !value.is_empty() && output.contains(value) {
            output = output.replace(value, &format!("{{{key}}}"));
        }
    }
    output
}

fn format_curl_contents(
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

fn suggest_file_name(method: &str, url: &str) -> Option<String> {
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
