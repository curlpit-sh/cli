mod curl_parser;
mod headers;
mod manual;
mod model;
mod substitutions;

pub use model::{ImportOptions, ImportResult};

use anyhow::{anyhow, Result};

use curl_parser::import_via_curl_parser;
use manual::import_via_manual;

pub fn import_curl_command(options: &ImportOptions<'_>) -> Result<ImportResult> {
    match import_via_curl_parser(options) {
        Ok(result) => Ok(result),
        Err(primary) => {
            import_via_manual(options).map_err(|fallback| anyhow!("{}\n{}", primary, fallback))
        }
    }
}
