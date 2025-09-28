use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use curlpit::config::{load_config, EnvironmentBuilder};
use curlpit::executor::{execute_request_file, ExecutionOptions};
use curlpit::interactive::run_interactive;

#[derive(Parser, Debug)]
#[command(
    name = "curlpit",
    version,
    about = "File-first HTTP runner",
    disable_help_subcommand = true
)]
struct Cli {
    /// Request file to execute (.curl)
    #[arg(value_name = "REQUEST")]
    request: Option<PathBuf>,

    /// Preview the first N bytes of the response body
    #[arg(short, long)]
    preview: Option<usize>,

    /// Select a profile from curlpit.json
    #[arg(short = 'P', long)]
    profile: Option<String>,

    /// Directory or file containing curlpit.json
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Override env file relative to config directory
    #[arg(short, long)]
    env: Option<PathBuf>,

    /// Override base directory used for resolving paths
    #[arg(long)]
    cwd: Option<PathBuf>,

    /// Directory to store response bodies
    #[arg(long = "output", short = 'O')]
    output: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Export a request using a named template
    Export {
        /// Export template name (e.g. js-fetch)
        #[arg(value_name = "TEMPLATE")]
        template: String,
        /// Request file to export
        #[arg(value_name = "REQUEST")]
        request: PathBuf,
        /// Output file (defaults to stdout)
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    let base_dir = cli
        .cwd
        .as_ref()
        .map(|p| resolve_path(Path::new(p)))
        .transpose()?
        .unwrap_or(std::env::current_dir()?);

    let config_target = cli
        .config
        .as_ref()
        .map(|p| resolve_relative(&base_dir, p))
        .unwrap_or_else(|| base_dir.clone());

    let cfg = load_config(&config_target).context("loading configuration")?;
    let config_dir = cfg.as_ref().map(|c| c.dir.clone()).unwrap_or_else(|| {
        if config_target.is_dir() {
            config_target.clone()
        } else {
            config_target
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| config_target.clone())
        }
    });

    let output_override = cli.output.as_ref().map(|p| resolve_relative(&base_dir, p));

    match &cli.command {
        Some(Commands::Export {
            template,
            request,
            out,
        }) => {
            exports::handle_export(
                template.clone(),
                resolve_relative(&base_dir, request),
                out.as_ref().map(|p| resolve_relative(&base_dir, p)),
                cfg.as_ref(),
                cli.profile.as_deref(),
                cli.env.as_ref().map(|p| resolve_relative(&config_dir, p)),
            )
            .await?;
            return Ok(());
        }
        None => {}
    }

    if cli.request.is_none() {
        run_interactive(curlpit::interactive::InteractiveOptions {
            base_dir: base_dir.clone(),
            config_target: config_dir.clone(),
            config: cfg.clone(),
            requested_profile: cli.profile.clone(),
            explicit_env: cli.env.as_ref().map(|p| resolve_relative(&config_dir, p)),
            preview_bytes: cli.preview,
            explicit_output_dir: output_override.clone(),
        })
        .await?;
        return Ok(());
    }

    let request_path = resolve_relative(&base_dir, &cli.request.unwrap());

    let env_builder = EnvironmentBuilder::new(
        base_dir.clone(),
        config_dir.clone(),
        cfg.clone(),
        cli.profile.clone(),
        cli.env.as_ref().map(|p| resolve_relative(&config_dir, p)),
        output_override.clone(),
    );

    let environment = env_builder.build().await?;

    let result = execute_request_file(
        &request_path,
        ExecutionOptions {
            preview_bytes: cli.preview,
            environment: &environment,
            response_output_dir: environment.response_output_dir.clone(),
        },
    )
    .await?;

    curlpit::executor::print_execution_result(&result);

    Ok(())
}

fn resolve_path(path: &Path) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn resolve_relative(base: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    struct DirGuard {
        original: PathBuf,
    }

    impl DirGuard {
        fn new(new_dir: &Path) -> Result<Self> {
            let original = std::env::current_dir()?;
            std::env::set_current_dir(new_dir)?;
            Ok(Self { original })
        }
    }

    impl Drop for DirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.original);
        }
    }

    #[test]
    fn resolve_path_uses_current_directory() -> Result<()> {
        let temp = tempdir()?;
        let _guard = DirGuard::new(temp.path())?;
        let relative = Path::new("requests/test.curl");
        std::fs::create_dir_all(temp.path().join("requests"))?;
        std::fs::write(temp.path().join(relative), "GET https://example.com")?;
        let resolved = resolve_path(relative)?;
        assert!(resolved.is_absolute());
        assert_eq!(
            resolved.canonicalize()?,
            temp.path().join(relative).canonicalize()?
        );
        Ok(())
    }

    #[test]
    fn resolve_relative_joins_when_needed() {
        let base = Path::new("/tmp/base");
        let relative = Path::new("sub/request.curl");
        assert_eq!(resolve_relative(base, relative), base.join(relative));

        let absolute = Path::new("/var/data/request.curl");
        assert_eq!(resolve_relative(base, absolute), absolute);
    }

    #[tokio::test]
    async fn handle_export_writes_to_file() -> Result<()> {
        let temp = tempdir()?;
        let request_path = temp.path().join("sample.curl");
        std::fs::write(&request_path, "GET https://example.com/api\n")?;

        let output_path = temp.path().join("output.js");

        exports::handle_export(
            "js-fetch".to_string(),
            request_path.clone(),
            Some(output_path.clone()),
            None,
            None,
            None,
        )
        .await?;

        let exported = std::fs::read_to_string(output_path)?;
        assert!(exported.contains("fetch(\"https://example.com/api\""));
        assert!(exported.contains("method: \"GET\""));
        Ok(())
    }

    #[tokio::test]
    async fn handle_export_prints_to_stdout_when_no_path() -> Result<()> {
        let temp = tempdir()?;
        let request_path = temp.path().join("sample.curl");
        std::fs::write(
            &request_path,
            "POST https://example.com/items\n\n{\"ok\":true}\n",
        )?;

        exports::handle_export("js-fetch".to_string(), request_path, None, None, None, None)
            .await?;

        Ok(())
    }
}

mod exports {
    use std::path::PathBuf;

    use anyhow::{Context, Result};

    use curlpit::{
        config::{EnvironmentBuilder, LoadedConfig},
        parser::{parse_request_file, RequestTemplate},
        template,
    };

    pub async fn handle_export(
        template_name: String,
        request_path: PathBuf,
        out_path: Option<PathBuf>,
        config: Option<&LoadedConfig>,
        profile: Option<&str>,
        explicit_env: Option<PathBuf>,
    ) -> Result<()> {
        let cwd = std::env::current_dir()?;
        let builder = EnvironmentBuilder::new(
            request_path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| cwd.clone()),
            config.map(|c| c.dir.clone()).unwrap_or_else(|| cwd.clone()),
            config.cloned(),
            profile.map(|s| s.to_string()),
            explicit_env,
            None,
        );

        let environment = builder.build().await?;
        let parsed = parse_request_file(&request_path, &environment)
            .await
            .with_context(|| format!("parsing request {}", request_path.display()))?;

        let tpl = RequestTemplate::from(&parsed.request);
        let rendered = template::render_export_template(&template_name, &tpl)?;

        if let Some(out) = out_path {
            std::fs::write(&out, rendered)
                .with_context(|| format!("writing export to {}", out.display()))?;
            println!("Export written to {}", out.display());
        } else {
            println!("{}", rendered);
        }
        Ok(())
    }
}
