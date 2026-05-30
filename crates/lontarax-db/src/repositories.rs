use lontarax_core::{
    new_id, now_rfc3339, LontaraXError, Model, ModelCapability, RequestStatus, Result, TaskStatus,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderEntity {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub api_key: String,
    pub enabled: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelEntity {
    pub id: String,
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
    pub capability: ModelCapability,
    pub metadata_json: Value,
    pub enabled: bool,
    pub created_at: String,
}

impl ModelEntity {
    pub fn into_model(self) -> Model {
        Model {
            id: Some(self.id),
            provider: self.provider,
            model_id: self.model_id,
            display_name: self.display_name,
            capability: self.capability,
            metadata_json: self.metadata_json,
            enabled: self.enabled,
            created_at: Some(self.created_at),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RequestEntity {
    pub id: String,
    pub provider: String,
    pub model: String,
    pub capability: RequestCapability,
    pub status: RequestStatus,
    pub cost: Option<f64>,
    pub input_hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskEntity {
    pub id: String,
    pub provider: String,
    pub external_task_id: String,
    pub status: TaskStatus,
    pub result_url: Option<String>,
    pub metadata_json: Value,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestCapability {
    Chat,
    Image,
    Video,
    Audio,
    Embedding,
    Credits,
}

impl std::fmt::Display for RequestCapability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Self::Chat => "chat",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Embedding => "embedding",
            Self::Credits => "credits",
        };
        write!(f, "{value}")
    }
}

impl std::str::FromStr for RequestCapability {
    type Err = LontaraXError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "chat" => Ok(Self::Chat),
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            "audio" => Ok(Self::Audio),
            "embedding" => Ok(Self::Embedding),
            "credits" => Ok(Self::Credits),
            other => Err(LontaraXError::InvalidCapability(other.to_string())),
        }
    }
}

impl From<ModelCapability> for RequestCapability {
    fn from(value: ModelCapability) -> Self {
        match value {
            ModelCapability::Chat => Self::Chat,
            ModelCapability::Image => Self::Image,
            ModelCapability::Video => Self::Video,
            ModelCapability::Audio => Self::Audio,
            ModelCapability::Embedding => Self::Embedding,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> ProviderRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(
        &self,
        name: &str,
        base_url: &str,
        api_key: &str,
        enabled: bool,
    ) -> Result<ProviderEntity> {
        let id = new_id();
        let created_at = now_rfc3339();

        sqlx::query(
            r#"
            INSERT INTO providers (id, name, base_url, api_key, enabled, created_at)
            VALUES (?, ?, ?, ?, ?, ?)
            ON CONFLICT(name) DO UPDATE SET
                base_url = excluded.base_url,
                api_key = excluded.api_key,
                enabled = excluded.enabled
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(base_url)
        .bind(api_key)
        .bind(enabled as i64)
        .bind(&created_at)
        .execute(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        self.get(name)
            .await?
            .ok_or_else(|| LontaraXError::Database("failed to reload provider after upsert".into()))
    }

    pub async fn get(&self, name: &str) -> Result<Option<ProviderEntity>> {
        let row = sqlx::query(
            r#"SELECT id, name, base_url, api_key, enabled, created_at
               FROM providers WHERE name = ? LIMIT 1"#,
        )
        .bind(name)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        Ok(row.map(|row| ProviderEntity {
            id: row.get("id"),
            name: row.get("name"),
            base_url: row.get("base_url"),
            api_key: row.get("api_key"),
            enabled: row.get::<i64, _>("enabled") != 0,
            created_at: row.get("created_at"),
        }))
    }

    pub async fn list(&self) -> Result<Vec<ProviderEntity>> {
        let rows = sqlx::query(
            r#"SELECT id, name, base_url, api_key, enabled, created_at
               FROM providers ORDER BY name ASC"#,
        )
        .fetch_all(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| ProviderEntity {
                id: row.get("id"),
                name: row.get("name"),
                base_url: row.get("base_url"),
                api_key: row.get("api_key"),
                enabled: row.get::<i64, _>("enabled") != 0,
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn remove(&self, name: &str) -> Result<u64> {
        let result = sqlx::query("DELETE FROM providers WHERE name = ?")
            .bind(name)
            .execute(self.pool)
            .await
            .map_err(|error| LontaraXError::Database(error.to_string()))?;
        Ok(result.rows_affected())
    }
}

#[derive(Debug, Clone)]
pub struct ModelRepository<'a> {
    pool: &'a SqlitePool,
}

impl<'a> ModelRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_many(&self, provider: &str, models: &[Model]) -> Result<usize> {
        let mut count = 0usize;
        for model in models {
            let id = model.id.clone().unwrap_or_else(new_id);
            let created_at = model.created_at.clone().unwrap_or_else(now_rfc3339);
            let metadata = serde_json::to_string(&model.metadata_json)?;

            sqlx::query(
                r#"
                INSERT INTO models (id, provider, model_id, display_name, capability, metadata_json, enabled, created_at)
                VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(provider, model_id, capability) DO UPDATE SET
                    display_name = excluded.display_name,
                    metadata_json = excluded.metadata_json,
                    enabled = excluded.enabled
                "#,
            )
            .bind(&id)
            .bind(provider)
            .bind(&model.model_id)
            .bind(&model.display_name)
            .bind(model.capability.to_string())
            .bind(metadata)
            .bind(model.enabled as i64)
            .bind(&created_at)
            .execute(self.pool)
            .await
            .map_err(|error| LontaraXError::Database(error.to_string()))?;
            count += 1;
        }

        Ok(count)
    }

    pub async fn list(&self, provider: Option<&str>) -> Result<Vec<ModelEntity>> {
        let rows = match provider {
            Some(provider) => {
                sqlx::query(
                    r#"SELECT id, provider, model_id, display_name, capability, metadata_json, enabled, created_at
                       FROM models WHERE provider = ? ORDER BY enabled DESC, display_name ASC"#,
                )
                .bind(provider)
                .fetch_all(self.pool)
                .await
            }
            None => {
                sqlx::query(
                    r#"SELECT id, provider, model_id, display_name, capability, metadata_json, enabled, created_at
                       FROM models ORDER BY provider ASC, enabled DESC, display_name ASC"#,
                )
                .fetch_all(self.pool)
                .await
            }
        }
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        rows.into_iter().map(row_to_model_entity).collect()
    }

    pub async fn list_by_capability(
        &self,
        provider: &str,
        capability: ModelCapability,
    ) -> Result<Vec<ModelEntity>> {
        let rows = sqlx::query(
            r#"SELECT id, provider, model_id, display_name, capability, metadata_json, enabled, created_at
               FROM models WHERE provider = ? AND capability = ? ORDER BY enabled DESC, display_name ASC"#,
        )
        .bind(provider)
        .bind(capability.to_string())
        .fetch_all(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        rows.into_iter().map(row_to_model_entity).collect()
    }

    pub async fn find_by_model_id(
        &self,
        provider: &str,
        model_id: &str,
    ) -> Result<Option<ModelEntity>> {
        let row = sqlx::query(
            r#"SELECT id, provider, model_id, display_name, capability, metadata_json, enabled, created_at
               FROM models WHERE provider = ? AND model_id = ? LIMIT 1"#,
        )
        .bind(provider)
        .bind(model_id)
        .fetch_optional(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        row.map(row_to_model_entity).transpose()
    }
}

#[derive(Debug, Clone)]
pub struct RequestRepository<'a> {
    pool: &'a SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestLogInput {
    pub provider: String,
    pub model: String,
    pub capability: RequestCapability,
    pub status: RequestStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
}

impl<'a> RequestRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn log(&self, input: RequestLogInput) -> Result<RequestEntity> {
        let id = new_id();
        let created_at = now_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO requests (id, provider, model, capability, status, cost, input_hash, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.provider)
        .bind(&input.model)
        .bind(input.capability.to_string())
        .bind(input.status.to_string())
        .bind(input.cost)
        .bind(input.input_hash.as_deref())
        .bind(&created_at)
        .execute(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        Ok(RequestEntity {
            id,
            provider: input.provider,
            model: input.model,
            capability: input.capability,
            status: input.status,
            cost: input.cost,
            input_hash: input.input_hash,
            created_at,
        })
    }

    pub async fn list(&self, limit: i64) -> Result<Vec<RequestEntity>> {
        let rows = sqlx::query(
            r#"SELECT id, provider, model, capability, status, cost, input_hash, created_at
               FROM requests ORDER BY created_at DESC LIMIT ?"#,
        )
        .bind(limit)
        .fetch_all(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        rows.into_iter().map(row_to_request_entity).collect()
    }
}

#[derive(Debug, Clone)]
pub struct TaskRepository<'a> {
    pool: &'a SqlitePool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskLogInput {
    pub provider: String,
    pub external_task_id: String,
    pub status: TaskStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_url: Option<String>,
    #[serde(default)]
    pub metadata_json: Value,
}

impl<'a> TaskRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, input: TaskLogInput) -> Result<TaskEntity> {
        let id = new_id();
        let created_at = now_rfc3339();
        sqlx::query(
            r#"
            INSERT INTO tasks (id, provider, external_task_id, status, result_url, metadata_json, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&input.provider)
        .bind(&input.external_task_id)
        .bind(input.status.to_string())
        .bind(input.result_url.as_deref())
        .bind(serde_json::to_string(&input.metadata_json)?)
        .bind(&created_at)
        .execute(self.pool)
        .await
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        Ok(TaskEntity {
            id,
            provider: input.provider,
            external_task_id: input.external_task_id,
            status: input.status,
            result_url: input.result_url,
            metadata_json: input.metadata_json,
            created_at,
        })
    }

    pub async fn list(&self, provider: Option<&str>) -> Result<Vec<TaskEntity>> {
        let rows = match provider {
            Some(provider) => {
                sqlx::query(
                    r#"SELECT id, provider, external_task_id, status, result_url, metadata_json, created_at
                       FROM tasks WHERE provider = ? ORDER BY created_at DESC"#,
                )
                .bind(provider)
                .fetch_all(self.pool)
                .await
            }
            None => {
                sqlx::query(
                    r#"SELECT id, provider, external_task_id, status, result_url, metadata_json, created_at
                       FROM tasks ORDER BY created_at DESC"#,
                )
                .fetch_all(self.pool)
                .await
            }
        }
        .map_err(|error| LontaraXError::Database(error.to_string()))?;

        rows.into_iter().map(row_to_task_entity).collect()
    }
}

fn row_to_model_entity(row: sqlx::sqlite::SqliteRow) -> Result<ModelEntity> {
    Ok(ModelEntity {
        id: row.get("id"),
        provider: row.get("provider"),
        model_id: row.get("model_id"),
        display_name: row.get("display_name"),
        capability: row.get::<String, _>("capability").parse()?,
        metadata_json: parse_json(row.get::<String, _>("metadata_json")),
        enabled: row.get::<i64, _>("enabled") != 0,
        created_at: row.get("created_at"),
    })
}

fn row_to_request_entity(row: sqlx::sqlite::SqliteRow) -> Result<RequestEntity> {
    Ok(RequestEntity {
        id: row.get("id"),
        provider: row.get("provider"),
        model: row.get("model"),
        capability: row.get::<String, _>("capability").parse()?,
        status: row.get::<String, _>("status").parse()?,
        cost: row.try_get::<Option<f64>, _>("cost").ok().flatten(),
        input_hash: row
            .try_get::<Option<String>, _>("input_hash")
            .ok()
            .flatten(),
        created_at: row.get("created_at"),
    })
}

fn row_to_task_entity(row: sqlx::sqlite::SqliteRow) -> Result<TaskEntity> {
    Ok(TaskEntity {
        id: row.get("id"),
        provider: row.get("provider"),
        external_task_id: row.get("external_task_id"),
        status: row.get::<String, _>("status").parse()?,
        result_url: row
            .try_get::<Option<String>, _>("result_url")
            .ok()
            .flatten(),
        metadata_json: parse_json(row.get::<String, _>("metadata_json")),
        created_at: row.get("created_at"),
    })
}

fn parse_json(value: String) -> Value {
    serde_json::from_str(&value).unwrap_or(Value::Null)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lontarax_core::{mask_secret, Model, ModelCapability};
    use sqlx::SqlitePool;

    async fn memory_pool() -> SqlitePool {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(5)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn migration_creates_tables() {
        let pool = memory_pool().await;
        let table_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('providers', 'models', 'requests', 'tasks')",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(table_count, 4);
    }

    #[tokio::test]
    async fn models_can_be_cached_and_filtered_by_capability() {
        let pool = memory_pool().await;
        let repo = ModelRepository::new(&pool);
        let models = vec![
            Model {
                id: None,
                provider: "kie".into(),
                model_id: "chat-model".into(),
                display_name: "Chat Model".into(),
                capability: ModelCapability::Chat,
                metadata_json: Value::Null,
                enabled: true,
                created_at: None,
            },
            Model {
                id: None,
                provider: "kie".into(),
                model_id: "image-model".into(),
                display_name: "Image Model".into(),
                capability: ModelCapability::Image,
                metadata_json: Value::Null,
                enabled: true,
                created_at: None,
            },
        ];

        let written = repo.upsert_many("kie", &models).await.unwrap();
        assert_eq!(written, 2);

        let chat = repo
            .list_by_capability("kie", ModelCapability::Chat)
            .await
            .unwrap();
        assert_eq!(chat.len(), 1);
        assert_eq!(chat[0].display_name, "Chat Model");
    }

    #[tokio::test]
    async fn provider_repository_masks_cleanly_in_cli_layer() {
        let pool = memory_pool().await;
        let repo = ProviderRepository::new(&pool);
        let inserted = repo
            .upsert("kie", "https://api.kie.ai", "sk-1234567890abcdef", true)
            .await
            .unwrap();

        assert_eq!(inserted.name, "kie");
        assert!(mask_secret(&inserted.api_key).contains("sk-1234"));
    }
}
