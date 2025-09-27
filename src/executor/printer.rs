use colored::{Color, Colorize};
use url::Url;

use super::models::ExecutionResult;

pub fn print_execution_result(result: &ExecutionResult) {
    let status_color = if result.response.status >= 400 {
        Color::Red
    } else if result.response.status >= 300 {
        Color::Yellow
    } else {
        Color::Green
    };

    println!(
        "{} {}",
        result.request.method.bold(),
        result.request.url.cyan()
    );
    println!(
        "{} {} {}",
        "Status:".bold(),
        format!("{}", result.response.status).color(status_color),
        format!("({:.1} ms)", result.response.duration_ms).dimmed()
    );

    if let Some(bytes) = result.request.body_bytes {
        println!(
            "{} {}",
            "Request body:".bold(),
            format!("{} bytes", bytes).dimmed()
        );
    }

    if !result.env_files.is_empty() {
        let files = result
            .env_files
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| p.display().to_string())
            })
            .collect::<Vec<_>>()
            .join(", ");
        println!("{} {}", "Env:".bold(), files.dimmed());
    }

    println!("{}", "Response headers".bold());
    for (name, value) in &result.response.headers {
        println!("  {}: {}", name.cyan(), value.dimmed());
    }

    println!(
        "{} {} {}",
        "Body:".bold(),
        format_body_link(&result.response.body_path),
        format!("({} bytes)", result.response.body_bytes).dimmed()
    );

    if let Some(preview) = &result.response.preview {
        println!("{}", "Preview".bold());
        println!("{}", preview.dimmed());
    }
}

fn format_body_link(path: &std::path::Path) -> String {
    let display = path.to_string_lossy();
    match Url::from_file_path(path) {
        Ok(url) => format!("\x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\", url, display.cyan()),
        Err(_) => display.cyan().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn format_body_link_wraps_file_urls() {
        let temp = tempdir().unwrap();
        let file = temp.path().join("output.json");
        std::fs::write(&file, "{}").unwrap();

        let link = format_body_link(&file);
        assert!(link.contains("output.json"));
        assert!(link.contains("\u{1b}]8;;"));
    }

    #[test]
    fn format_body_link_handles_relative_paths() {
        let relative = std::path::Path::new("relative.bin");
        let link = format_body_link(relative);
        assert!(link.contains("relative.bin"));
        assert!(!link.contains("\u{1b}]8;;"));
    }
}
