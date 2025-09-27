use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct ImportOptions<'a> {
    pub command: &'a str,
    pub template_variables: &'a HashMap<String, String>,
    pub env_variables: &'a HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct ImportResult {
    pub contents: String,
    pub suggested_filename: Option<String>,
    pub method: String,
    pub url: String,
    pub warnings: Vec<String>,
}
