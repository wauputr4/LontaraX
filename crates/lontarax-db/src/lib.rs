pub mod repositories;

use lontarax_core::{Config, LontaraXError, Result};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::fs;
use std::path::PathBuf;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        ensure_sqlite_parent_dir(database_url)?;
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|error| LontaraXError::InvalidDatabase(error.to_string()))?;

        Ok(Self { pool })
    }

    pub async fn from_config(config: &Config) -> Result<Self> {
        let database_url = config.effective_database_url()?;
        Self::connect(&database_url).await
    }

    pub async fn migrate(&self) -> Result<()> {
        sqlx::migrate!("../../migrations")
            .run(&self.pool)
            .await
            .map_err(|error| LontaraXError::InvalidDatabase(error.to_string()))?;
        Ok(())
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }
}

fn ensure_sqlite_parent_dir(database_url: &str) -> Result<()> {
    if database_url.contains(":memory:") {
        return Ok(());
    }

    if let Some(path) = sqlite_path_from_url(database_url) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
    }

    Ok(())
}

fn sqlite_path_from_url(database_url: &str) -> Option<PathBuf> {
    if let Some(path) = database_url.strip_prefix("sqlite://") {
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    } else if let Some(path) = database_url.strip_prefix("sqlite:") {
        if path.is_empty() || path.starts_with(":memory:") {
            None
        } else {
            Some(PathBuf::from(path.trim_start_matches("//")))
        }
    } else {
        None
    }
}
