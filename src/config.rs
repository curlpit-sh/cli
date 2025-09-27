use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::env::{load_env_file_sync, EnvMap};

fn resolve_relative(base: &Path, value: &str) -> PathBuf {
    let candidate = Path::new(value);
    if candidate.is_absolute() {
        candidate.to_path_buf()
    } else {
        base.join(candidate)
    }
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CurlpitProfileConfig {
    pub env: Option<String>,
    pub variables: HashMap<String, String>,
    pub vars: HashMap<String, String>,
    #[serde(rename = "responseOutputDir")]
    pub response_output_dir: Option<String>,
    #[serde(flatten)]
    pub extras: HashMap<String, Value>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct CurlpitConfig {
    pub profiles: HashMap<String, CurlpitProfileConfig>,
    pub variables: HashMap<String, String>,
    pub vars: HashMap<String, String>,
    #[serde(rename = "defaultProfile")]
    pub default_profile: Option<String>,
    #[serde(rename = "responseOutputDir")]
    pub response_output_dir: Option<String>,
    #[serde(flatten)]
    pub extras: HashMap<String, Value>,
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct EnvironmentContext {
    pub base_dir: PathBuf,
    pub config_dir: PathBuf,
    pub template_variables: EnvMap,
    pub initial_env: EnvMap,
    pub env_files: Vec<PathBuf>,
    pub profile_name: Option<String>,
    pub response_output_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct EnvironmentBuilder {
    base_dir: PathBuf,
    config_dir: PathBuf,
    config: Option<LoadedConfig>,
    requested_profile: Option<String>,
    explicit_env: Option<PathBuf>,
    explicit_output_dir: Option<PathBuf>,
}

impl EnvironmentBuilder {
    pub fn new(
        base_dir: PathBuf,
        config_dir: PathBuf,
        config: Option<LoadedConfig>,
        requested_profile: Option<String>,
        explicit_env: Option<PathBuf>,
        explicit_output_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            base_dir,
            config_dir,
            config,
            requested_profile,
            explicit_env,
            explicit_output_dir,
        }
    }

    pub async fn build(&self) -> Result<EnvironmentContext> {
        let mut template_variables: EnvMap = HashMap::new();
        let mut profile_name = None;
        let mut env_files = Vec::new();
        let mut initial_env: EnvMap = HashMap::new();
        let mut response_output_dir = self.explicit_output_dir.clone();

        if let Some(cfg) = &self.config {
            let profile = resolve_profile(&cfg.config, self.requested_profile.as_deref())?;
            profile_name = Some(profile.name.clone());
            template_variables.extend(cfg.config.variables.clone());
            template_variables.extend(cfg.config.vars.clone());
            template_variables.extend(profile.config.variables.clone());
            template_variables.extend(profile.config.vars.clone());

            initial_env.extend(template_variables.clone());

            let mut env_path: Option<PathBuf> = None;
            if let Some(explicit) = &self.explicit_env {
                env_path = Some(explicit.clone());
            } else if let Some(profile_env) = &profile.config.env {
                env_path = Some(self.config_dir.join(profile_env));
            }

            if let Some(env_path) = env_path {
                let loaded = load_env_file_sync(&env_path, &mut initial_env)?;
                env_files.push(loaded);
            }

            if response_output_dir.is_none() {
                if let Some(dir) = &profile.config.response_output_dir {
                    response_output_dir = Some(resolve_relative(&self.config_dir, dir));
                }
            }

            if response_output_dir.is_none() {
                if let Some(dir) = &cfg.config.response_output_dir {
                    response_output_dir = Some(resolve_relative(&self.config_dir, dir));
                }
            }
        }

        // explicit env might come without config present
        if self.config.is_none() {
            if let Some(explicit) = &self.explicit_env {
                let loaded = load_env_file_sync(explicit, &mut initial_env)?;
                env_files.push(loaded);
            }
        }

        if let Some(dir) = &mut response_output_dir {
            if !dir.is_absolute() {
                let joined = self.base_dir.join(dir.as_path());
                *dir = joined;
            }
        }

        Ok(EnvironmentContext {
            base_dir: self.base_dir.clone(),
            config_dir: self.config_dir.clone(),
            template_variables,
            initial_env,
            env_files,
            profile_name,
            response_output_dir,
        })
    }
}

struct ResolvedProfile<'a> {
    name: String,
    config: &'a CurlpitProfileConfig,
}

fn resolve_profile<'a>(
    config: &'a CurlpitConfig,
    requested: Option<&str>,
) -> Result<ResolvedProfile<'a>> {
    if config.profiles.is_empty() {
        bail!("No profiles defined in configuration");
    }

    if let Some(name) = requested {
        if let Some(profile) = config.profiles.get(name) {
            return Ok(ResolvedProfile {
                name: name.to_string(),
                config: profile,
            });
        }
        bail!("Unknown profile: {}", name);
    }

    if let Some(default) = &config.default_profile {
        if let Some(profile) = config.profiles.get(default) {
            return Ok(ResolvedProfile {
                name: default.to_string(),
                config: profile,
            });
        }
    }

    if let Some((name, profile)) = config.profiles.iter().next() {
        return Ok(ResolvedProfile {
            name: name.to_string(),
            config: profile,
        });
    }

    bail!("No profile candidates available");
}
