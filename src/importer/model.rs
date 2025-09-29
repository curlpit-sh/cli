use std::collections::HashMap;

use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ImportOptions<'a> {
    pub command: &'a str,
    pub template_variables: &'a HashMap<String, String>,
    pub env_variables: &'a HashMap<String, String>,
    pub template_variants: &'a [(String, String)],
    pub include_headers: Option<&'a [String]>,
    pub exclude_headers: Option<&'a [String]>,
    pub append_headers: Option<&'a HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportResult {
    pub contents: String,
    pub suggested_filename: Option<String>,
    pub method: String,
    pub url: String,
    pub warnings: Vec<String>,
}
