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
    use crate::executor::models::{ExecutionResult, RequestSummary, ResponseSummary};
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

    #[test]
    fn print_execution_result_handles_success() {
        let temp = tempdir().unwrap();
        let body_path = temp.path().join("body.json");
        std::fs::write(&body_path, "{}").unwrap();

        let result = ExecutionResult {
            request: RequestSummary {
                method: "GET".to_string(),
                url: "https://example.com/resource".to_string(),
                body_bytes: Some(12),
            },
            response: ResponseSummary {
                status: 200,
                headers: vec![("content-type".to_string(), "application/json".to_string())],
                duration_ms: 12.5,
                body_path: body_path.clone(),
                body_bytes: 2,
                preview: Some("{}".to_string()),
            },
            env_files: vec![body_path.clone()],
        };

        print_execution_result(&result);
    }

    #[test]
    fn print_execution_result_handles_errors() {
        let temp = tempdir().unwrap();
        let body_path = temp.path().join("body.bin");
        std::fs::write(&body_path, b"binary").unwrap();

        let result = ExecutionResult {
            request: RequestSummary {
                method: "POST".to_string(),
                url: "https://example.com/error".to_string(),
                body_bytes: None,
            },
            response: ResponseSummary {
                status: 404,
                headers: vec![("x-trace".to_string(), "abc".to_string())],
                duration_ms: 42.0,
                body_path,
                body_bytes: 6,
                preview: None,
            },
            env_files: Vec::new(),
        };

        print_execution_result(&result);
    }
}
