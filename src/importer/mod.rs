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

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;

    static EMPTY: Lazy<HashMap<String, String>> = Lazy::new(HashMap::new);

    fn options(command: &str) -> ImportOptions<'_> {
        ImportOptions {
            command,
            template_variables: &EMPTY,
            env_variables: &EMPTY,
            include_headers: None,
            exclude_headers: None,
            append_headers: None,
        }
    }

    #[test]
    fn import_curl_command_combines_errors_when_all_strategies_fail() {
        let err =
            import_curl_command(&options("curl --data")).expect_err("invalid command should fail");
        let message = err.to_string();
        assert!(message.contains("Option '--data' missing value"));
        assert!(message.contains('\n'));
    }
}
