use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::{LontaraXError, Result};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum ModelCapability {
    Chat,
    Image,
    Video,
    Audio,
    Embedding,
}

impl fmt::Display for ModelCapability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Chat => "chat",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Embedding => "embedding",
        };
        write!(f, "{value}")
    }
}

impl FromStr for ModelCapability {
    type Err = LontaraXError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "chat" => Ok(Self::Chat),
            "image" => Ok(Self::Image),
            "video" => Ok(Self::Video),
            "audio" => Ok(Self::Audio),
            "embedding" => Ok(Self::Embedding),
            other => Err(LontaraXError::InvalidCapability(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RequestStatus {
    Pending,
    Success,
    Failed,
}

impl fmt::Display for RequestStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Pending => "pending",
            Self::Success => "success",
            Self::Failed => "failed",
        };
        write!(f, "{value}")
    }
}

impl FromStr for RequestStatus {
    type Err = LontaraXError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "pending" => Ok(Self::Pending),
            "success" | "succeeded" | "completed" => Ok(Self::Success),
            "failed" | "error" => Ok(Self::Failed),
            other => Err(LontaraXError::Provider(format!(
                "unknown request status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        };
        write!(f, "{value}")
    }
}

impl FromStr for TaskStatus {
    type Err = LontaraXError;

    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_lowercase().as_str() {
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" | "success" | "completed" => Ok(Self::Succeeded),
            "failed" | "error" => Ok(Self::Failed),
            "cancelled" | "canceled" => Ok(Self::Cancelled),
            other => Err(LontaraXError::Provider(format!(
                "unknown task status: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub total_tokens: u64,
    #[serde(default)]
    pub metadata_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub metadata_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ChatResponse {
    pub provider: String,
    pub model: String,
    pub content: String,
    #[serde(default)]
    pub raw: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default = "default_one")]
    pub n: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
    #[serde(default)]
    pub metadata_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageResponse {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub raw: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoRequest {
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub metadata_json: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct VideoResponse {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub urls: Vec<String>,
    #[serde(default)]
    pub raw: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Credits {
    pub remaining: f64,
    #[serde(default = "default_credit_unit")]
    pub unit: String,
    #[serde(default)]
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Model {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub provider: String,
    pub model_id: String,
    pub display_name: String,
    pub capability: ModelCapability,
    #[serde(default)]
    pub metadata_json: Value,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TaskHandle {
    pub provider: String,
    pub task_id: String,
    #[serde(default)]
    pub raw: Value,
}

pub fn default_one() -> u32 {
    1
}

pub fn default_credit_unit() -> String {
    "credits".to_string()
}

pub fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn mask_secret(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let len = chars.len();
    if len <= 4 {
        return "*".repeat(len.max(4));
    }

    let prefix_len = len.min(7);
    let suffix_len = len.min(4);
    if len <= prefix_len + suffix_len {
        return "*".repeat(len);
    }

    let prefix: String = chars[..prefix_len].iter().collect();
    let suffix: String = chars[len - suffix_len..].iter().collect();
    let stars = "*".repeat(len - prefix_len - suffix_len);
    format!("{prefix}{stars}{suffix}")
}
