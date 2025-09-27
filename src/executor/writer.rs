use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use hex;
use petname::petname;
use uuid::Uuid;

pub(super) fn write_response_body(
    bytes: &[u8],
    content_type: Option<&str>,
    response_dir: Option<&Path>,
    request_file: &Path,
) -> Result<PathBuf> {
    let extension = extension_for_content_type(content_type);

    if let Some(base_dir) = response_dir {
        let stem = request_file
            .file_stem()
            .map(|s| s.to_string_lossy())
            .unwrap_or_else(|| std::borrow::Cow::Borrowed("request"));
        let sanitized = sanitize_component(&stem);
        let request_dir = base_dir.join(&sanitized);
        fs::create_dir_all(&request_dir)
            .with_context(|| format!("creating response directory {}", request_dir.display()))?;

        let index = next_index(&request_dir)?;
        let pet = petname(2, "-");
        let file_name = format!("{:03}-{}{}", index, pet, extension);
        let path = request_dir.join(file_name);
        fs::write(&path, bytes)
            .with_context(|| format!("writing response body to {}", path.display()))?;
        Ok(path)
    } else {
        let filename = format!("curlpit-{}{}", Uuid::new_v4(), extension);
        let path = std::env::temp_dir().join(filename);
        fs::write(&path, bytes)
            .with_context(|| format!("writing response body to {}", path.display()))?;
        Ok(path)
    }
}

pub(super) fn create_preview(bytes: &[u8], limit: usize) -> String {
    let slice = if bytes.len() > limit {
        &bytes[..limit]
    } else {
        bytes
    };
    match std::str::from_utf8(slice) {
        Ok(text) => text.to_string(),
        Err(_) => hex::encode(slice),
    }
}

pub(super) fn sanitize_component(value: &str) -> String {
    let sanitized: String = value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "request".to_string()
    } else {
        trimmed.to_string()
    }
}

pub(super) fn extension_for_content_type(content_type: Option<&str>) -> &'static str {
    match content_type
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
    {
        "application/json" => ".json",
        "text/html" => ".html",
        "text/plain" => ".txt",
        "application/xml" | "text/xml" => ".xml",
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "application/pdf" => ".pdf",
        "application/octet-stream" => ".bin",
        _ => ".bin",
    }
}

pub(super) fn next_index(dir: &Path) -> Result<u32> {
    let mut max_index = 0;
    for entry in
        fs::read_dir(dir).with_context(|| format!("reading directory {}", dir.display()))?
    {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str() {
            if name.len() >= 3 && name.chars().take(3).all(|c| c.is_ascii_digit()) {
                if let Ok(value) = name[0..3].parse::<u32>() {
                    max_index = max_index.max(value + 1);
                }
            }
        }
    }
    Ok(max_index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use regex::Regex;
    use tempfile::tempdir;

    #[test]
    fn sanitize_component_replaces_invalid_characters() {
        assert_eq!(sanitize_component("Hello World!"), "Hello-World");
        assert_eq!(sanitize_component("***"), "request");
        assert_eq!(sanitize_component("foo_bar"), "foo_bar");
    }

    #[test]
    fn extension_for_content_type_matches_common_types() {
        assert_eq!(
            extension_for_content_type(Some("application/json; charset=utf-8")),
            ".json"
        );
        assert_eq!(extension_for_content_type(Some("text/html")), ".html");
        assert_eq!(
            extension_for_content_type(Some("application/unknown")),
            ".bin"
        );
        assert_eq!(extension_for_content_type(None), ".bin");
    }

    #[test]
    fn create_preview_handles_binary_data() {
        let text = create_preview("hello".as_bytes(), 10);
        assert_eq!(text, "hello");

        let binary = create_preview(&[0, 159, 146, 150], 4);
        assert_eq!(binary, "009f9296");
    }

    #[test]
    fn write_response_body_creates_files_under_response_dir() -> Result<()> {
        let temp = tempdir()?;
        let response_dir = temp.path().join("responses");
        let request_dir = temp.path().join("requests");
        std::fs::create_dir_all(&request_dir)?;
        let request_file = request_dir.join("Sample Request!.curl");
        std::fs::write(&request_file, "")?;

        let first = write_response_body(
            b"{}",
            Some("application/json"),
            Some(response_dir.as_path()),
            &request_file,
        )?;
        let second = write_response_body(
            b"{}",
            Some("application/json"),
            Some(response_dir.as_path()),
            &request_file,
        )?;

        let request_subdir = response_dir.join("Sample-Request");
        assert_eq!(first.parent().unwrap(), request_subdir);
        assert!(first
            .extension()
            .unwrap()
            .to_string_lossy()
            .ends_with("json"));
        assert!(second.parent().unwrap().exists());

        let filename_pattern = Regex::new(r"^\d{3}-[a-z]+-[a-z]+\.json$").unwrap();
        assert!(filename_pattern.is_match(first.file_name().unwrap().to_str().unwrap()));
        assert!(filename_pattern.is_match(second.file_name().unwrap().to_str().unwrap()));
        assert_eq!(&second.file_name().unwrap().to_str().unwrap()[0..3], "001");

        let first_contents = std::fs::read(first)?;
        assert_eq!(first_contents, b"{}");

        Ok(())
    }

    #[test]
    fn write_response_body_defaults_to_temp_dir() -> Result<()> {
        let temp = tempdir()?;
        let request_file = temp.path().join("req.curl");
        std::fs::write(&request_file, "")?;

        let path = write_response_body(b"abc", None, None, &request_file)?;
        assert!(path.exists());
        assert!(path.extension().unwrap().to_string_lossy().ends_with("bin"));
        assert_eq!(std::fs::read(path)?, b"abc");
        Ok(())
    }

    #[test]
    fn next_index_detects_existing_files() -> Result<()> {
        let temp = tempdir()?;
        std::fs::create_dir_all(temp.path())?;
        std::fs::write(temp.path().join("000-first.bin"), b"one")?;
        std::fs::write(temp.path().join("010-second.bin"), b"two")?;

        let idx = next_index(temp.path())?;
        assert_eq!(idx, 11);
        Ok(())
    }
}
