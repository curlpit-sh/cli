use std::collections::HashMap;

pub type EnvMap = HashMap<String, String>;

#[cfg(feature = "cli")]
mod loader;
mod placeholders;

#[cfg(feature = "cli")]
pub use loader::{load_env_directive, load_env_file_sync};
pub use placeholders::expand_placeholders;
