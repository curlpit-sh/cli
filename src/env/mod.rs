use std::collections::HashMap;

pub type EnvMap = HashMap<String, String>;

mod loader;
mod placeholders;

pub use loader::{load_env_directive, load_env_file_sync};
pub use placeholders::expand_placeholders;
