pub mod repositories;

use lontarax_core::{Config, LontaraXError, Result};
use sqlx::{sqlite::SqliteConnectOptions, sqlite::SqlitePoolOptions, SqlitePool};
use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        ensure_sqlite_parent_dir(database_url)?;
        let connect_options = SqliteConnectOptions::from_str(database_url)
            .map_err(|error| LontaraXError::InvalidDatabase(error.to_string()))?
            .create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(connect_options)
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use uuid::Uuid;

    #[tokio::test]
    async fn connect_creates_missing_sqlite_file() {
        let temp_db = env::temp_dir().join(format!("lontarax-{}.db", Uuid::new_v4()));
        let database_url = format!("sqlite://{}", temp_db.display());

        if temp_db.exists() {
            fs::remove_file(&temp_db).unwrap();
        }

        let db = Database::connect(&database_url).await.unwrap();
        assert!(temp_db.exists(), "expected database file to be created");
        drop(db);

        fs::remove_file(&temp_db).unwrap();
    }
}
