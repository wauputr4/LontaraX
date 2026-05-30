use crate::error::{LontaraXError, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub default_output: OutputFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,
}

impl Config {
    pub fn default_path() -> Result<PathBuf> {
        let base = dirs::config_dir().ok_or_else(|| {
            LontaraXError::InvalidConfig("unable to determine config directory".into())
        })?;
        Ok(base.join("lontarax").join("config.yaml"))
    }

    pub fn default_database_path() -> Result<PathBuf> {
        let base = dirs::data_local_dir().ok_or_else(|| {
            LontaraXError::InvalidConfig("unable to determine data directory".into())
        })?;
        Ok(base.join("lontarax").join("lontarax.db"))
    }

    pub fn load(path: Option<&Path>) -> Result<Self> {
        let path = match path {
            Some(path) => path.to_path_buf(),
            None => Self::default_path()?,
        };

        let mut config = if path.exists() {
            let raw = fs::read_to_string(&path)?;
            serde_yaml::from_str(&raw)?
        } else {
            Self::default()
        };

        config.apply_env_overrides();
        Ok(config)
    }

    pub fn save(&self, path: Option<&Path>) -> Result<PathBuf> {
        let path = match path {
            Some(path) => path.to_path_buf(),
            None => Self::default_path()?,
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let raw = serde_yaml::to_string(self)?;
        fs::write(&path, raw)?;
        Ok(path)
    }

    pub fn apply_env_overrides(&mut self) {
        if let Ok(value) = env::var("LONTARAX_DEFAULT_PROVIDER") {
            if !value.trim().is_empty() {
                self.default_provider = Some(value);
            }
        }

        if let Ok(value) = env::var("LONTARAX_DATABASE_URL") {
            if !value.trim().is_empty() {
                self.database_url = Some(value);
            }
        }
    }

    pub fn effective_database_url(&self) -> Result<String> {
        if let Ok(value) = env::var("LONTARAX_DATABASE_URL") {
            if !value.trim().is_empty() {
                return Ok(value);
            }
        }

        if let Some(value) = &self.database_url {
            return Ok(value.clone());
        }

        Ok(format!(
            "sqlite://{}",
            Self::default_database_path()?.display()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.yaml");
        let config = Config {
            default_provider: Some("kie".into()),
            default_output: OutputFormat::Json,
            database_url: Some("sqlite::memory:".into()),
        };

        config.save(Some(&path)).unwrap();
        let loaded = Config::load(Some(&path)).unwrap();

        assert_eq!(loaded.default_provider.as_deref(), Some("kie"));
        assert_eq!(loaded.default_output, OutputFormat::Json);
        assert_eq!(loaded.database_url.as_deref(), Some("sqlite::memory:"));
    }

    #[test]
    fn env_override_applies() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("missing.yaml");

        env::set_var("LONTARAX_DEFAULT_PROVIDER", "kie");
        env::set_var("LONTARAX_DATABASE_URL", "sqlite::memory:");

        let loaded = Config::load(Some(&path)).unwrap();

        unsafe {
            env::remove_var("LONTARAX_DEFAULT_PROVIDER");
            env::remove_var("LONTARAX_DATABASE_URL");
        }

        assert_eq!(loaded.default_provider.as_deref(), Some("kie"));
        assert_eq!(loaded.database_url.as_deref(), Some("sqlite::memory:"));
    }
}
