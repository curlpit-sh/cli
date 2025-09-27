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

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    fn write_file(path: &Path, contents: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[tokio::test]
    async fn environment_builder_prefers_explicit_env_and_output() -> Result<()> {
        let temp = tempdir()?;
        let base_dir = temp.path().join("workspace");
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&base_dir)?;
        std::fs::create_dir_all(&config_dir)?;

        write_file(
            &config_dir.join("curlpit.json"),
            r#"{
  "variables": {"GLOBAL": "global"},
  "vars": {"ALIAS": "alias"},
  "profiles": {
    "dev": {
      "env": "dev.env",
      "variables": {"PROFILE": "dev"},
      "vars": {"PROFILE_ALIAS": "dev-alias"},
      "responseOutputDir": "profile-responses"
    },
    "staging": {
      "env": "staging.env",
      "variables": {"PROFILE": "staging"},
      "vars": {"PROFILE_ALIAS": "staging-alias"},
      "responseOutputDir": "staging-responses"
    }
  },
  "defaultProfile": "dev",
  "responseOutputDir": "root-responses"
}
"#,
        );

        write_file(
            &config_dir.join("staging.env"),
            "TOKEN=from-staging\nEXTRA=profile\n",
        );
        write_file(
            &config_dir.join("override.env"),
            "TOKEN=override\nEXTRA=explicit\n",
        );

        let loaded = load_config(&config_dir)?.expect("config should be present");
        let builder = EnvironmentBuilder::new(
            base_dir.clone(),
            config_dir.clone(),
            Some(loaded.clone()),
            Some("staging".to_string()),
            Some(config_dir.join("override.env")),
            Some(PathBuf::from("custom-out")),
        );

        let environment = builder.build().await?;

        assert_eq!(environment.profile_name.as_deref(), Some("staging"));
        assert_eq!(
            environment.template_variables.get("GLOBAL"),
            Some(&"global".to_string())
        );
        assert_eq!(
            environment.template_variables.get("PROFILE"),
            Some(&"staging".to_string())
        );
        assert_eq!(
            environment.template_variables.get("PROFILE_ALIAS"),
            Some(&"staging-alias".to_string())
        );

        assert_eq!(
            environment.initial_env.get("TOKEN"),
            Some(&"override".to_string())
        );
        assert_eq!(
            environment.initial_env.get("EXTRA"),
            Some(&"explicit".to_string())
        );

        assert_eq!(environment.env_files.len(), 1);
        assert!(environment
            .env_files
            .first()
            .unwrap()
            .ends_with(Path::new("override.env")));

        let expected_output = base_dir.join("custom-out");
        assert_eq!(
            environment.response_output_dir.as_ref(),
            Some(&expected_output)
        );

        Ok(())
    }

    #[tokio::test]
    async fn environment_builder_without_config_still_loads_explicit_env() -> Result<()> {
        let temp = tempdir()?;
        let base_dir = temp.path().join("workspace");
        std::fs::create_dir_all(&base_dir)?;
        let env_path = base_dir.join("local.env");
        write_file(&env_path, "FOO=bar\n");

        let builder = EnvironmentBuilder::new(
            base_dir.clone(),
            base_dir.clone(),
            None,
            None,
            Some(env_path.clone()),
            Some(PathBuf::from("responses")),
        );

        let environment = builder.build().await?;

        assert_eq!(environment.template_variables.len(), 0);
        assert_eq!(environment.initial_env.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(environment.env_files, vec![env_path]);
        assert_eq!(
            environment.response_output_dir,
            Some(base_dir.join("responses"))
        );

        Ok(())
    }
}
