use std::{
    collections::HashMap,
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::{anyhow, bail, Context, Result};

pub type EnvMap = HashMap<String, String>;

pub fn expand_placeholders(input: &str, env: &EnvMap) -> Result<String> {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(&next) = chars.peek() {
                    match next {
                        '{' | '}' => {
                            output.push(next);
                            chars.next();
                        }
                        _ => {
                            output.push('\\');
                            output.push(next);
                            chars.next();
                        }
                    }
                } else {
                    output.push('\\');
                }
            }
            '{' => {
                let Some(&next_char) = chars.peek() else {
                    output.push('{');
                    continue;
                };
                if !is_start_char(next_char) {
                    output.push('{');
                    continue;
                }

                let mut key = String::new();
                while let Some(&next) = chars.peek() {
                    if next == '}' {
                        chars.next();
                        break;
                    }
                    key.push(next);
                    chars.next();
                }

                if key.is_empty() {
                    bail!("Empty template placeholder");
                }

                if !is_valid_key(&key) {
                    bail!("Invalid template variable: {key}");
                }

                let value = env
                    .get(&key)
                    .cloned()
                    .or_else(|| std::env::var(&key).ok())
                    .ok_or_else(|| anyhow!("Missing template variable: {key}"))?;
                output.push_str(&value);
            }
            _ => output.push(ch),
        }
    }

    Ok(output)
}

fn is_valid_key(key: &str) -> bool {
    let mut chars = key.chars();
    match chars.next() {
        Some(c) if is_start_char(c) => {}
        _ => return false,
    }

    for ch in chars {
        let valid = ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.' | '-');
        if !valid {
            return false;
        }
    }
    true
}

fn is_start_char(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

pub fn load_env_file_sync(path: &Path, env: &mut EnvMap) -> Result<PathBuf> {
    let content =
        fs::read_to_string(path).with_context(|| format!("reading env file {}", path.display()))?;
    let iter = dotenvy::from_read_iter(Cursor::new(content));

    for item in iter {
        let (key, value) = item.with_context(|| format!("parsing env file {}", path.display()))?;
        env.insert(key, value);
    }

    Ok(path.to_path_buf())
}

pub fn load_env_directive(
    path: &Path,
    env: &mut EnvMap,
    env_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let loaded = load_env_file_sync(path, env)?;
    env_files.push(loaded);
    Ok(())
}
