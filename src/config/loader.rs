use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CurlpitProfileConfig {
    pub env: Option<String>,
    pub variables: HashMap<String, String>,
    #[serde(rename = "responseOutputDir")]
    pub response_output_dir: Option<String>,
    #[serde(rename = "defaultHeaders")]
    pub default_headers: HashMap<String, String>,
    #[serde(flatten)]
    pub extras: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CurlpitConfig {
    pub profiles: HashMap<String, CurlpitProfileConfig>,
    pub variables: HashMap<String, String>,
    #[serde(rename = "defaultProfile")]
    pub default_profile: Option<String>,
    #[serde(rename = "responseOutputDir")]
    pub response_output_dir: Option<String>,
    pub env: Option<String>,
    #[serde(rename = "defaultHeaders")]
    pub default_headers: HashMap<String, String>,
    #[serde(rename = "import")]
    pub import: Option<ImportConfig>,
    #[serde(flatten)]
    pub extras: HashMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct ImportConfig {
    #[serde(rename = "includeHeaders")]
    pub include_headers: Option<Vec<String>>,
    #[serde(rename = "excludeHeaders")]
    pub exclude_headers: Option<Vec<String>>,
    #[serde(rename = "appendHeaders")]
    pub append_headers: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
    pub config: CurlpitConfig,
    pub path: PathBuf,
    pub dir: PathBuf,
}

pub fn load_config(target: &Path) -> Result<Option<LoadedConfig>> {
    let resolved = if target.is_absolute() {
        target.to_path_buf()
    } else {
        std::env::current_dir()?.join(target)
    };

    let (file_path, dir) = if resolved.is_dir() {
        (resolved.join("curlpit.json"), resolved)
    } else {
        (
            resolved.clone(),
            resolved
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap()),
        )
    };

    if !file_path.exists() {
        return Ok(None);
    }

    let contents = fs::read_to_string(&file_path)
        .with_context(|| format!("reading config {}", file_path.display()))?;

    let config: CurlpitConfig = serde_json::from_str(&contents)
        .with_context(|| format!("parsing config {}", file_path.display()))?;

    Ok(Some(LoadedConfig {
        config,
        path: file_path,
        dir,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn returns_none_when_config_missing() -> Result<()> {
        let temp = tempdir()?;
        let result = load_config(temp.path())?;
        assert!(result.is_none());
        Ok(())
    }

    #[test]
    fn loads_config_from_directory() -> Result<()> {
        let temp = tempdir()?;
        let config_path = temp.path().join("curlpit.json");
        std::fs::write(&config_path, r#"{"profiles":{"test":{}}}"#)?;

        let result = load_config(temp.path())?.expect("config should load");
        assert_eq!(result.path, config_path);
        assert_eq!(result.dir, temp.path());
        assert!(result.config.profiles.contains_key("test"));
        Ok(())
    }
}
