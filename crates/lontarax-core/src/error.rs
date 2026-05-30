use thiserror::Error;

pub type Result<T> = std::result::Result<T, LontaraXError>;

#[derive(Debug, Error)]
pub enum LontaraXError {
    #[error("provider not found: {0}")]
    ProviderNotFound(String),

    #[error("factory not found: {0}")]
    FactoryNotFound(String),

    #[error("model not found: {0}")]
    ModelNotFound(String),

    #[error("invalid capability: {0}")]
    InvalidCapability(String),

    #[error("missing API key for provider: {0}")]
    MissingApiKey(String),

    #[error("invalid config: {0}")]
    InvalidConfig(String),

    #[error("invalid database: {0}")]
    InvalidDatabase(String),

    #[error("HTTP {status}: {message}")]
    Http { status: u16, message: String },

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("provider error: {0}")]
    Provider(String),

    #[error("database error: {0}")]
    Database(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    SerdeJson(#[from] serde_json::Error),

    #[error(transparent)]
    SerdeYaml(#[from] serde_yaml::Error),

    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

impl LontaraXError {
    pub fn http(status: u16, message: impl Into<String>) -> Self {
        Self::Http {
            status,
            message: message.into(),
        }
    }
}
