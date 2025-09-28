use std::{
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::{bail, Result};
use chrono::{DateTime, Local};
use inquire::{Confirm, Select, Text};
use walkdir::WalkDir;

use crate::{
    config::{load_config, EnvironmentBuilder, LoadedConfig},
    executor::{execute_request_file, print_execution_result, ExecutionOptions},
    importer::{import_curl_command, ImportOptions, ImportResult},
};

#[derive(Clone)]
pub struct InteractiveOptions {
    pub base_dir: PathBuf,
    pub config_target: PathBuf,
    pub config: Option<LoadedConfig>,
    pub requested_profile: Option<String>,
    pub explicit_env: Option<PathBuf>,
    pub preview_bytes: Option<usize>,
    pub explicit_output_dir: Option<PathBuf>,
}

pub async fn run_interactive(mut options: InteractiveOptions) -> Result<()> {
    let mut profile = options.requested_profile.clone();
    let mut files = discover_curl_files(&options.base_dir)?;

    loop {
        if files.is_empty() {
            println!("No .curl files found under {}", options.base_dir.display());
            return Ok(());
        }

        let mut menu_items: Vec<MenuItem> = files
            .iter()
            .map(|file| MenuItem::Run(file.clone()))
            .collect();
        menu_items.push(MenuItem::Refresh);
        if options.config.is_some() {
            menu_items.push(MenuItem::ChangeProfile);
        }
        menu_items.push(MenuItem::Import);
        menu_items.push(MenuItem::Quit);

        let choice = Select::new("curlpit", menu_items)
            .with_page_size(10)
            .prompt()?;

        match choice {
            MenuItem::Run(file) => {
                let builder = EnvironmentBuilder::new(
                    options.base_dir.clone(),
                    options.config_target.clone(),
                    options.config.clone(),
                    profile.clone(),
                    options.explicit_env.clone(),
                    options.explicit_output_dir.clone(),
                );
                let env = builder.build().await?;
                println!(
                    "\nRunning {} (profile: {})\n",
                    file.relative.display(),
                    env.profile_name.as_deref().unwrap_or("<none>")
                );

                match execute_request_file(
                    &file.absolute,
                    ExecutionOptions {
                        preview_bytes: options.preview_bytes,
                        environment: &env,
                        response_output_dir: env.response_output_dir.clone(),
                    },
                )
                .await
                {
                    Ok(result) => {
                        print_execution_result(&result);
                    }
                    Err(err) => {
                        eprintln!("Error: {err:?}");
                    }
                }

                let _ = Text::new("Press Enter to continue").prompt();
            }
            MenuItem::Refresh => {
                files = discover_curl_files(&options.base_dir)?;
                options.config = load_config(&options.config_target)?;
                if let Some(cfg) = options.config.as_ref() {
                    let profile_valid = profile
                        .as_ref()
                        .map(|name| cfg.config.profiles.contains_key(name))
                        .unwrap_or(true);
                    if !profile_valid {
                        profile = cfg.config.default_profile.clone();
                    }
                } else {
                    profile = None;
                }
                continue;
            }
            MenuItem::ChangeProfile => {
                if let Some(cfg) = &options.config {
                    profile = Some(prompt_profile(cfg, profile.as_deref())?);
                } else {
                    println!("No configuration to select profiles from");
                }
            }
            MenuItem::Import => {
                if let Err(error) = handle_import(&options, &mut profile).await {
                    eprintln!("Import failed: {error}");
                } else {
                    files = discover_curl_files(&options.base_dir)?;
                }
            }
            MenuItem::Quit => break,
        }
    }

    Ok(())
}

#[derive(Clone)]
struct CurlFile {
    absolute: PathBuf,
    relative: PathBuf,
    modified: SystemTime,
    size: u64,
}

impl fmt::Display for CurlFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}, {})",
            self.relative.display(),
            format_size(self.size),
            format_relative(self.modified)
        )
    }
}

#[derive(Clone)]
enum MenuItem {
    Run(CurlFile),
    Refresh,
    ChangeProfile,
    Import,
    Quit,
}

impl fmt::Display for MenuItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MenuItem::Run(file) => write!(f, "{}", file),
            MenuItem::Refresh => write!(f, "↻ Refresh"),
            MenuItem::ChangeProfile => write!(f, "⇄ Change profile"),
            MenuItem::Import => write!(f, "⇢ Import from curl"),
            MenuItem::Quit => write!(f, "Quit"),
        }
    }
}

fn discover_curl_files(base_dir: &Path) -> Result<Vec<CurlFile>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(base_dir).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension() {
                if ext == "curl" {
                    let metadata = fs::metadata(entry.path())?;
                    files.push(CurlFile {
                        absolute: entry.path().to_path_buf(),
                        relative: entry
                            .path()
                            .strip_prefix(base_dir)
                            .unwrap_or(entry.path())
                            .to_path_buf(),
                        modified: metadata.modified().unwrap_or(SystemTime::now()),
                        size: metadata.len(),
                    });
                }
            }
        }
    }
    files.sort_by(|a, b| b.modified.cmp(&a.modified));
    Ok(files)
}

fn prompt_profile(config: &LoadedConfig, current: Option<&str>) -> Result<String> {
    let mut profiles: Vec<String> = config.config.profiles.keys().cloned().collect();
    profiles.sort();

    let default_profile = current
        .map(|s| s.to_string())
        .or_else(|| config.config.default_profile.clone())
        .unwrap_or_else(|| profiles.first().cloned().unwrap_or_default());

    let start = profiles
        .iter()
        .position(|name| name == &default_profile)
        .unwrap_or(0);

    let selection = Select::new("Select profile", profiles)
        .with_starting_cursor(start)
        .prompt()?;

    Ok(selection)
}

async fn handle_import(options: &InteractiveOptions, profile: &mut Option<String>) -> Result<()> {
    println!("Paste a curl command. Finish with an empty line (or press Ctrl+C to cancel).");
    let command = match read_multiline_command()? {
        Some(value) => value,
        None => {
            println!("Import cancelled.");
            return Ok(());
        }
    };

    if command.trim().is_empty() {
        println!("Import cancelled (empty command).");
        return Ok(());
    }

    let prepared = prepare_import(options, profile.as_deref(), &command).await?;
    let PreparedImport {
        import,
        default_name,
    } = prepared;

    if !import.warnings.is_empty() {
        println!("Warnings:");
        for warning in &import.warnings {
            println!(" - {}", warning);
        }
    }

    let save_as = match Text::new("Save as").with_default(&default_name).prompt() {
        Ok(value) => value.trim().to_string(),
        Err(_) => {
            println!("Import cancelled.");
            return Ok(());
        }
    };

    if save_as.is_empty() {
        println!("Import cancelled (empty file name).");
        return Ok(());
    }

    let save_path = resolve_save_path(&options.base_dir, &save_as, &default_name);

    let allow_overwrite = if save_path.exists() {
        let overwrite = Confirm::new("File exists. Overwrite?")
            .with_default(false)
            .prompt();
        match overwrite {
            Ok(true) => true,
            _ => {
                println!("Import cancelled.");
                return Ok(());
            }
        }
    } else {
        true
    };

    let display = write_imported_file(
        &save_path,
        &options.base_dir,
        &import.contents,
        allow_overwrite,
    )?;
    println!("Imported request written to {}", display);

    Ok(())
}

struct PreparedImport {
    import: ImportResult,
    default_name: String,
}

async fn prepare_import(
    options: &InteractiveOptions,
    profile: Option<&str>,
    command: &str,
) -> Result<PreparedImport> {
    let normalized = normalize_command(command);

    let builder = EnvironmentBuilder::new(
        options.base_dir.clone(),
        options.config_target.clone(),
        options.config.clone(),
        profile.map(|s| s.to_string()),
        options.explicit_env.clone(),
        options.explicit_output_dir.clone(),
    );
    let environment = builder.build().await?;

    let import_cfg = options
        .config
        .as_ref()
        .and_then(|cfg| cfg.config.import.as_ref());
    let (include_headers, exclude_headers, append_headers) = if let Some(cfg) = import_cfg {
        (
            cfg.include_headers.as_deref(),
            cfg.exclude_headers.as_deref(),
            if cfg.append_headers.is_empty() {
                None
            } else {
                Some(&cfg.append_headers)
            },
        )
    } else {
        (None, None, None)
    };

    let import = import_curl_command(&ImportOptions {
        command: &normalized,
        template_variables: &environment.template_variables,
        env_variables: &environment.initial_env,
        include_headers,
        exclude_headers,
        append_headers,
    })?;

    let default_name = import
        .suggested_filename
        .clone()
        .unwrap_or_else(|| "request.curl".to_string());

    Ok(PreparedImport {
        import,
        default_name,
    })
}

fn write_imported_file(
    path: &Path,
    base_dir: &Path,
    contents: &str,
    allow_overwrite: bool,
) -> Result<String> {
    if path.exists() && !allow_overwrite {
        bail!("Refusing to overwrite {}", path.display());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)?;

    let display = path
        .strip_prefix(base_dir)
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| path.display().to_string());
    Ok(display)
}

fn resolve_save_path(base_dir: &Path, input: &str, default_name: &str) -> PathBuf {
    let trimmed = input.trim();
    let mut candidate = PathBuf::from(trimmed);

    let treat_as_dir = trimmed.ends_with('/') || trimmed.ends_with("\\");

    if treat_as_dir {
        candidate = candidate.join(default_name);
    }

    if candidate
        .extension()
        .map(|ext| ext != "curl")
        .unwrap_or(true)
    {
        candidate.set_extension("curl");
    }

    if candidate.is_absolute() {
        candidate
    } else {
        base_dir.join(candidate)
    }
}

fn format_size(bytes: u64) -> String {
    match bytes {
        0..=1024 => format!("{} B", bytes),
        1025..=1_048_576 => format!("{:.1} KiB", bytes as f64 / 1024.0),
        _ => format!("{:.1} MiB", bytes as f64 / 1024.0 / 1024.0),
    }
}

fn format_relative(time: SystemTime) -> String {
    let now = SystemTime::now();
    let delta = now.duration_since(time).unwrap_or(Duration::ZERO);
    if delta < Duration::from_secs(60) {
        return format!("{}s ago", delta.as_secs());
    }
    if delta < Duration::from_secs(3600) {
        return format!("{}m ago", delta.as_secs() / 60);
    }
    if delta < Duration::from_secs(86400) {
        return format!("{}h ago", delta.as_secs() / 3600);
    }
    let datetime: DateTime<Local> = DateTime::<Local>::from(time);
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

fn read_multiline_command() -> Result<Option<String>> {
    let stdin = io::stdin();
    let mut lines = Vec::new();

    loop {
        print!("curl> ");
        io::stdout().flush()?;

        let mut buffer = String::new();
        let bytes = stdin.read_line(&mut buffer)?;
        if bytes == 0 {
            // EOF
            return Ok(None);
        }

        let trimmed = buffer.trim_end_matches(['\n', '\r']);
        if trimmed.is_empty() {
            if lines.is_empty() {
                return Ok(None);
            }
            break;
        }

        lines.push(trimmed.to_string());

        if !trimmed.ends_with('\\') {
            break;
        }
    }

    Ok(Some(lines.join("\n")))
}

fn normalize_command(input: &str) -> String {
    let mut normalized = String::new();
    for line in input.lines() {
        let trimmed = line.trim_end();
        if trimmed.ends_with('\\') {
            normalized.push_str(trimmed.trim_end_matches('\\'));
            normalized.push(' ');
        } else {
            normalized.push_str(trimmed);
            normalized.push(' ');
        }
    }
    normalized.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;
    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn discover_curl_files_finds_requests() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();
        let foo = base.join("foo.curl");
        let bar = base.join("nested/bar.curl");
        std::fs::write(&foo, "GET https://example.com")?;
        std::fs::create_dir_all(bar.parent().unwrap())?;
        std::fs::write(&bar, "POST https://example.com")?;
        std::fs::write(base.join("ignore.txt"), "noop")?;

        let files = discover_curl_files(base)?;
        let relative_paths: Vec<_> = files.iter().map(|f| f.relative.clone()).collect();

        assert_eq!(files.len(), 2);
        assert!(relative_paths.contains(&PathBuf::from("foo.curl")));
        assert!(relative_paths.contains(&PathBuf::from("nested/bar.curl")));
        Ok(())
    }

    #[test]
    fn resolve_save_path_adds_extension_and_respects_directory_hint() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();
        let path = resolve_save_path(base, "requests/", "default.curl");
        assert_eq!(path, base.join("requests/default.curl"));

        let explicit = resolve_save_path(base, "output/test.req", "default.curl");
        assert_eq!(explicit, base.join("output/test.curl"));
        Ok(())
    }

    #[test]
    fn format_size_represents_ranges() {
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(2048), "2.0 KiB");
        assert_eq!(format_size(2 * 1024 * 1024), "2.0 MiB");
    }

    #[test]
    fn format_relative_handles_recent_durations() {
        let now = SystemTime::now();
        assert_eq!(format_relative(now), "0s ago");
        assert_eq!(format_relative(now - Duration::from_secs(45)), "45s ago");
        assert_eq!(format_relative(now - Duration::from_secs(600)), "10m ago");
        assert_eq!(format_relative(now - Duration::from_secs(7200)), "2h ago");
    }

    #[test]
    fn normalize_command_merges_continuation_lines() {
        let input = concat!(
            "curl https://example.com \\",
            "\n",
            "  -H 'Accept: application/json'"
        );
        let normalized = normalize_command(input);
        assert_eq!(
            normalized,
            "curl https://example.com    -H 'Accept: application/json'"
        );
    }

    #[tokio::test]
    async fn prepare_import_resolves_environment_variables() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();
        std::fs::write(
            base.join("curlpit.json"),
            r#"{
  "defaultProfile": "test",
  "profiles": {
    "test": {
      "env": "example.env",
      "variables": {
        "API_BASE": "https://api.example.com"
      }
    }
  },
  "import": {
    "includeHeaders": ["Authorization"]
  }
}
"#,
        )?;
        std::fs::write(base.join("example.env"), "API_TOKEN=token-123\n")?;

        let loaded = load_config(base)?.expect("config should load");
        let options = InteractiveOptions {
            base_dir: base.to_path_buf(),
            config_target: base.to_path_buf(),
            config: Some(loaded),
            requested_profile: Some("test".to_string()),
            explicit_env: None,
            preview_bytes: None,
            explicit_output_dir: None,
        };

        let command = "curl -X POST https://api.example.com/items -H 'Authorization: Bearer token-123' --data '{\"ok\":true}'";
        let prepared =
            prepare_import(&options, options.requested_profile.as_deref(), command).await?;

        assert!(prepared.import.contents.contains("POST {API_BASE}/items"));
        assert!(prepared
            .import
            .contents
            .contains("authorization: Bearer {API_TOKEN}"));
        assert_eq!(prepared.default_name, "post-api-example-com-items.curl");
        Ok(())
    }

    #[test]
    fn write_imported_file_creates_directories() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();
        let target = base.join("requests").join("sample.curl");

        let display = write_imported_file(&target, base, "GET https://example.com", true)?;

        assert_eq!(display, "requests/sample.curl");
        assert_eq!(std::fs::read_to_string(&target)?, "GET https://example.com");
        Ok(())
    }

    #[test]
    fn write_imported_file_respects_overwrite_flag() -> Result<()> {
        let temp = tempdir()?;
        let base = temp.path();
        let target = base.join("existing.curl");
        std::fs::write(&target, "old")?;

        let err =
            write_imported_file(&target, base, "new", false).expect_err("should refuse overwrite");
        assert!(err.to_string().contains("Refusing to overwrite"));

        // Allow overwrite succeeds
        let display = write_imported_file(&target, base, "new", true)?;
        assert_eq!(display, "existing.curl");
        assert_eq!(std::fs::read_to_string(&target)?, "new");
        Ok(())
    }
}
