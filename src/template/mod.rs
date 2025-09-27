use anyhow::{bail, Result};

use crate::parser::RequestTemplate;

mod js_fetch;

pub fn render_export_template(name: &str, template: &RequestTemplate) -> Result<String> {
    match name {
        "js-fetch" => js_fetch::render_js_fetch(template),
        other => bail!("Unknown export template: {other}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn base_template() -> RequestTemplate {
        RequestTemplate {
            method: "get".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![("accept".to_string(), "application/json".to_string())],
            body_text: None,
            body_file: None,
        }
    }

    #[test]
    fn render_js_fetch_includes_headers() -> Result<()> {
        let template = base_template();
        let rendered = render_export_template("js-fetch", &template)?;

        assert!(rendered.contains("await fetch(\"https://example.com\""));
        assert!(rendered.contains("method: \"GET\""));
        assert!(rendered.contains("headers"));
        Ok(())
    }

    #[test]
    fn render_js_fetch_handles_body_variants() -> Result<()> {
        let mut with_text = base_template();
        with_text.body_text = Some("{\"ok\":true}".to_string());
        let rendered = render_export_template("js-fetch", &with_text)?;
        assert!(rendered.contains("body: \"{\\\"ok\\\":true}\""));

        let mut with_file = base_template();
        with_file.body_file = Some(PathBuf::from("payload.json"));
        let rendered = render_export_template("js-fetch", &with_file)?;
        assert!(rendered.contains("Original file: payload.json"));
        Ok(())
    }

    #[test]
    fn render_export_template_rejects_unknown_templates() {
        let template = base_template();
        let err = render_export_template("unknown", &template).unwrap_err();
        assert!(err.to_string().contains("Unknown export template"));
    }
}
