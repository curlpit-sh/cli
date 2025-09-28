use std::{
    fs,
    io::Cursor,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

use crate::env::EnvMap;

pub fn load_env_file_sync(path: &Path, env: &mut EnvMap) -> Result<PathBuf> {
    let content =
        fs::read_to_string(path).with_context(|| format!("reading env file {}", path.display()))?;
    let iter = dotenvy::from_read_iter(Cursor::new(content));

    for item in iter {
        let (key, value) = item.with_context(|| format!("parsing env file {}", path.display()))?;
        env.insert(key, value);
    }

    Ok(path.to_path_buf())
}

pub fn load_env_directive(
    path: &Path,
    env: &mut EnvMap,
    env_files: &mut Vec<PathBuf>,
) -> Result<()> {
    let loaded = load_env_file_sync(path, env)?;
    env_files.push(loaded);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use tempfile::tempdir;

    #[test]
    fn load_env_file_sync_merges_values() -> Result<()> {
        let temp = tempdir()?;
        let env_path = temp.path().join("vars.env");
        fs::write(&env_path, "FOO=bar\nBAZ=qux\n")?;

        let mut env_map = EnvMap::new();
        load_env_file_sync(&env_path, &mut env_map)?;

        assert_eq!(env_map.get("FOO"), Some(&"bar".to_string()));
        assert_eq!(env_map.get("BAZ"), Some(&"qux".to_string()));
        Ok(())
    }

    #[test]
    fn load_env_file_sync_propagates_io_errors() {
        let mut env_map = EnvMap::new();
        let path = PathBuf::from("does-not-exist.env");
        let err = load_env_file_sync(&path, &mut env_map).unwrap_err();
        assert!(err.to_string().contains("reading env file"));
    }
}
