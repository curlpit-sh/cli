use std::{
    fmt, fs,
    io::{self, Write},
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use anyhow::Result;
use chrono::{DateTime, Local};
use inquire::{validator::Validation, Confirm, Select, Text};
use walkdir::WalkDir;

use crate::{
    config::{EnvironmentBuilder, LoadedConfig},
    executor::{execute_request_file, print_execution_result, ExecutionOptions},
    importer::{import_curl_command, ImportOptions},
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

pub async fn run_interactive(options: InteractiveOptions) -> Result<()> {
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

                let _ = Text::new("Press Enter to continue")
                    .with_validator(|input: &str| {
                        if input.is_empty() {
                            Ok(Validation::Valid)
                        } else {
                            Ok(Validation::Valid)
                        }
                    })
                    .prompt();
            }
            MenuItem::Refresh => {
                files = discover_curl_files(&options.base_dir)?;
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

    let normalized = normalize_command(&command);

    let builder = EnvironmentBuilder::new(
        options.base_dir.clone(),
        options.config_target.clone(),
        options.config.clone(),
        profile.clone(),
        options.explicit_env.clone(),
        options.explicit_output_dir.clone(),
    );
    let environment = builder.build().await?;

    let import = import_curl_command(&ImportOptions {
        command: &normalized,
        template_variables: &environment.template_variables,
        env_variables: &environment.initial_env,
    })?;

    if !import.warnings.is_empty() {
        println!("Warnings:");
        for warning in &import.warnings {
            println!(" - {}", warning);
        }
    }

    let default_name = import
        .suggested_filename
        .unwrap_or_else(|| "request.curl".to_string());

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

    if save_path.exists() {
        let overwrite = Confirm::new("File exists. Overwrite?")
            .with_default(false)
            .prompt();
        match overwrite {
            Ok(true) => {}
            _ => {
                println!("Import cancelled.");
                return Ok(());
            }
        }
    }

    if let Some(parent) = save_path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&save_path, import.contents)?;

    println!(
        "Imported request written to {}",
        save_path
            .strip_prefix(&options.base_dir)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| save_path.display().to_string())
    );

    Ok(())
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
