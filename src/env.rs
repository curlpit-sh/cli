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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    struct EnvVarGuard {
        key: String,
        original: Option<String>,
    }

    impl EnvVarGuard {
        fn set(key: &str, value: &str) -> Self {
            let original = std::env::var(key).ok();
            std::env::set_var(key, value);
            Self {
                key: key.to_string(),
                original,
            }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            if let Some(ref value) = self.original {
                std::env::set_var(&self.key, value);
            } else {
                std::env::remove_var(&self.key);
            }
        }
    }

    #[test]
    fn expand_placeholders_uses_environment_fallback() {
        let _guard = EnvVarGuard::set("FROM_OS", "value");
        let env = EnvMap::new();
        let rendered = expand_placeholders("token={FROM_OS}", &env).unwrap();
        assert_eq!(rendered, "token=value");
    }

    #[test]
    fn expand_placeholders_rejects_invalid_keys() {
        let env = EnvMap::new();
        let err = expand_placeholders("{BAD!}", &env).unwrap_err();
        assert!(err.to_string().contains("Invalid template variable"));
    }

    #[test]
    fn expand_placeholders_reports_missing_values() {
        let env = EnvMap::new();
        let err = expand_placeholders("{MISSING}", &env).unwrap_err();
        assert!(err.to_string().contains("Missing template variable"));
    }

    #[test]
    fn load_env_file_sync_merges_values() -> Result<()> {
        let temp = tempdir()?;
        let env_path = temp.path().join("vars.env");
        fs::write(&env_path, "FOO=bar\nBAZ=qux\n")?;

        let mut env_map = EnvMap::new();
        load_env_file_sync(&env_path, &mut env_map)?;

        assert_eq!(env_map.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(env_map.get("BAZ"), Some(&"qux".to_string()));
        Ok(())
    }

    #[test]
    fn load_env_file_sync_propagates_io_errors() {
        let mut env_map = EnvMap::new();
        let path = PathBuf::from("does-not-exist.env");
        let err = load_env_file_sync(&path, &mut env_map).unwrap_err();
        assert!(err.to_string().contains("reading env file"));
    }
}
