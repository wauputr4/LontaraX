use async_trait::async_trait;
use lontarax_core::{
    AiProvider, ChatRequest, ChatResponse, Credits, ImageRequest, ImageResponse, LontaraXError,
    Model, ModelCapability, ProviderFactory, ProviderFactoryRegistry, ProviderRuntimeConfig,
    Result, VideoRequest, VideoResponse,
};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

pub const DEFAULT_BASE_URL: &str = "https://api.kie.ai";
const DOCS_INDEX_URL: &str = "https://docs.kie.ai/llms.txt";

#[derive(Debug, Clone)]
pub struct KieProvider {
    client: Client,
    base_url: String,
    api_key: String,
}

impl KieProvider {
    pub fn new(base_url: impl Into<String>, api_key: impl Into<String>) -> Result<Self> {
        let api_key = api_key.into();
        if api_key.trim().is_empty() {
            return Err(LontaraXError::MissingApiKey("kie".into()));
        }

        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|error| {
                LontaraXError::Provider(format!("failed to build HTTP client: {error}"))
            })?;

        Ok(Self {
            client,
            base_url: normalize_base_url(base_url.into()),
            api_key,
        })
    }

    fn url(&self, path: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            path.trim_start_matches('/')
        )
    }

    fn auth_request(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        builder
            .bearer_auth(&self.api_key)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
    }

    async fn get_envelope(&self, url: String) -> Result<KieEnvelope> {
        let response = self
            .auth_request(self.client.get(&url))
            .send()
            .await
            .map_err(map_reqwest_error)?;
        self.parse_envelope_response(response).await
    }

    async fn post_json(&self, url: String, payload: Value) -> Result<KieEnvelope> {
        let response = self
            .auth_request(self.client.post(&url).json(&payload))
            .send()
            .await
            .map_err(map_reqwest_error)?;
        self.parse_envelope_response(response).await
    }

    async fn parse_envelope_response(&self, response: reqwest::Response) -> Result<KieEnvelope> {
        let status = response.status();
        let body = response.text().await.map_err(map_reqwest_error)?;

        if !status.is_success() {
            return Err(parse_http_error(status, &body));
        }

        let envelope: KieEnvelope = serde_json::from_str(&body).map_err(|error| {
            LontaraXError::Provider(format!(
                "failed to parse Kie response as JSON (status {status}): {error}; body={body}"
            ))
        })?;

        if envelope.code != 200 {
            return Err(LontaraXError::Http {
                status: envelope.code,
                message: envelope.msg,
            });
        }

        Ok(envelope)
    }

    async fn fetch_kie_models(&self) -> Result<Vec<Model>> {
        let response = self
            .client
            .get(DOCS_INDEX_URL)
            .send()
            .await
            .map_err(map_reqwest_error)?;

        let status = response.status();
        let text = response.text().await.map_err(map_reqwest_error)?;
        if !status.is_success() {
            return Err(parse_http_error(status, &text));
        }

        let mut models = Vec::new();
        let mut seen = HashSet::new();
        let mut in_api_docs = false;

        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed == "## API Docs" {
                in_api_docs = true;
                continue;
            }

            if !in_api_docs || !trimmed.starts_with("- ") || !trimmed.contains("](") {
                continue;
            }

            if let Some(parsed) = parse_model_line(trimmed) {
                let (capability, model_id, display_name, url) = parsed;
                let key = format!("{model_id}:{capability}");
                if seen.insert(key) {
                    models.push(Model {
                        id: None,
                        provider: "kie".into(),
                        model_id,
                        display_name,
                        capability,
                        metadata_json: json!({
                            "source": "docs.kie.ai/llms.txt",
                            "source_url": url,
                            "discovery": "documentation scrape",
                            "todo": "replace with an official model-list endpoint if KIE publishes one",
                        }),
                        enabled: true,
                        created_at: Some(lontarax_core::now_rfc3339()),
                    });
                }
            }
        }

        if models.is_empty() {
            return Err(LontaraXError::Provider(
                "no Kie models discovered from docs.kie.ai/llms.txt".into(),
            ));
        }

        Ok(models)
    }

    async fn create_task(&self, endpoint: &str, payload: Value) -> Result<KieTaskHandle> {
        let envelope = self.post_json(self.url(endpoint), payload).await?;
        let task_id = extract_task_id(&envelope.data).ok_or_else(|| {
            LontaraXError::Provider("Kie task response did not include a task id".into())
        })?;

        Ok(KieTaskHandle {
            provider: "kie".into(),
            task_id,
            raw: envelope.data,
        })
    }

    async fn poll_task(&self, task_id: &str) -> Result<KieTaskRecord> {
        let url = format!("{}?taskId={}", self.url("/api/v1/jobs/recordInfo"), task_id);
        let envelope = self.get_envelope(url).await?;
        Ok(KieTaskRecord {
            code: envelope.code,
            msg: envelope.msg,
            data: envelope.data,
        })
    }

    async fn download_url(&self, url: &str) -> Result<String> {
        let envelope = self
            .post_json(
                self.url("/api/v1/common/download-url"),
                json!({ "url": url }),
            )
            .await?;
        envelope
            .data
            .as_str()
            .map(|value| value.to_string())
            .ok_or_else(|| {
                LontaraXError::Provider("Kie download-url response did not contain a string".into())
            })
    }
}

#[async_trait]
impl AiProvider for KieProvider {
    fn name(&self) -> &'static str {
        "kie"
    }

    async fn models(&self) -> Result<Vec<Model>> {
        self.fetch_kie_models().await
    }

    async fn credits(&self) -> Result<Credits> {
        let envelope = self.get_envelope(self.url("/api/v1/chat/credit")).await?;
        let remaining = parse_credit_value(&envelope.data)?;

        Ok(Credits {
            remaining,
            unit: "credits".into(),
            raw: envelope.data,
        })
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
        let _ = req;
        Err(LontaraXError::Provider(
            "Kie chat is TODO: wire the task-based chat workflow once the final endpoint contract is confirmed".into(),
        ))
    }

    async fn image(&self, req: ImageRequest) -> Result<ImageResponse> {
        let _ = req;
        Err(LontaraXError::Provider(
            "Kie image is TODO: wire the task-based image workflow once the final endpoint contract is confirmed".into(),
        ))
    }

    async fn video(&self, req: VideoRequest) -> Result<VideoResponse> {
        let _ = req;
        Err(LontaraXError::Provider(
            "Kie video is TODO: wire the task-based video workflow once the final endpoint contract is confirmed".into(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct KieTaskClient {
    inner: Arc<KieProvider>,
}

impl KieTaskClient {
    pub fn new(provider: KieProvider) -> Self {
        Self {
            inner: Arc::new(provider),
        }
    }

    pub async fn create_task(&self, endpoint: &str, payload: Value) -> Result<KieTaskHandle> {
        self.inner.create_task(endpoint, payload).await
    }

    pub async fn poll_task(&self, task_id: &str) -> Result<KieTaskRecord> {
        self.inner.poll_task(task_id).await
    }

    pub async fn download_url(&self, url: &str) -> Result<String> {
        self.inner.download_url(url).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KieTaskHandle {
    pub provider: String,
    pub task_id: String,
    #[serde(default)]
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KieTaskRecord {
    pub code: u16,
    pub msg: String,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct KieEnvelope {
    code: u16,
    msg: String,
    #[serde(default)]
    data: Value,
}

#[derive(Debug, Clone)]
pub struct KieProviderFactory;

impl ProviderFactory for KieProviderFactory {
    fn name(&self) -> &'static str {
        "kie"
    }

    fn default_base_url(&self) -> Option<&'static str> {
        Some(DEFAULT_BASE_URL)
    }

    fn create(&self, config: ProviderRuntimeConfig) -> Result<Arc<dyn AiProvider>> {
        let base_url = config
            .base_url
            .or_else(|| self.default_base_url().map(ToString::to_string))
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
        let api_key = config
            .api_key
            .ok_or_else(|| LontaraXError::MissingApiKey("kie".into()))?;
        Ok(Arc::new(KieProvider::new(base_url, api_key)?))
    }
}

pub fn register_factories(registry: &mut ProviderFactoryRegistry) {
    registry.register(KieProviderFactory);
}

fn normalize_base_url(base_url: String) -> String {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        DEFAULT_BASE_URL.to_string()
    } else {
        trimmed.trim_end_matches('/').to_string()
    }
}

fn parse_model_line(line: &str) -> Option<(ModelCapability, String, String, String)> {
    let prefix_end = line.find('[')?;
    let prefix = line[2..prefix_end].trim();
    let rest = &line[prefix_end + 1..];
    let title_end = rest.find("](")?;
    let display_name = rest[..title_end].trim().to_string();
    let rest = &rest[title_end + 2..];
    let url_end = rest.find(')')?;
    let url = rest[..url_end].trim().to_string();
    let capability = capability_from_prefix(prefix)?;
    let model_id = url
        .trim_end_matches(".md")
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();

    if model_id.is_empty() || display_name.is_empty() {
        return None;
    }

    Some((capability, model_id, display_name, url))
}

fn capability_from_prefix(prefix: &str) -> Option<ModelCapability> {
    let lower = prefix.to_lowercase();
    if lower.contains("chat") {
        Some(ModelCapability::Chat)
    } else if lower.contains("image") {
        Some(ModelCapability::Image)
    } else if lower.contains("video") {
        Some(ModelCapability::Video)
    } else if lower.contains("music") || lower.contains("audio") || lower.contains("suno") {
        Some(ModelCapability::Audio)
    } else if lower.contains("embedding") {
        Some(ModelCapability::Embedding)
    } else {
        None
    }
}

fn parse_credit_value(value: &Value) -> Result<f64> {
    match value {
        Value::Number(number) => number.as_f64().ok_or_else(|| {
            LontaraXError::Provider(
                "Kie credit payload number could not be represented as f64".into(),
            )
        }),
        Value::String(text) => text.parse::<f64>().map_err(|error| {
            LontaraXError::Provider(format!("Kie credit payload string is not numeric: {error}"))
        }),
        _ => Err(LontaraXError::Provider(
            "Kie credit payload did not contain a numeric data field".into(),
        )),
    }
}

fn extract_task_id(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => Some(text.clone()),
        Value::Object(map) => {
            for key in ["task_id", "taskId", "id"] {
                if let Some(value) = map.get(key) {
                    if let Some(text) = value.as_str() {
                        return Some(text.to_string());
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn parse_http_error(status: StatusCode, body: &str) -> LontaraXError {
    match status.as_u16() {
        401 | 403 => LontaraXError::Http {
            status: status.as_u16(),
            message: format!("authentication failed: {body}"),
        },
        429 => LontaraXError::Http {
            status: 429,
            message: format!("rate limited: {body}"),
        },
        500..=599 => LontaraXError::Http {
            status: status.as_u16(),
            message: format!("server error: {body}"),
        },
        _ => LontaraXError::Http {
            status: status.as_u16(),
            message: body.to_string(),
        },
    }
}

fn map_reqwest_error(error: reqwest::Error) -> LontaraXError {
    if error.is_timeout() {
        LontaraXError::Timeout(error.to_string())
    } else if let Some(status) = error.status() {
        LontaraXError::Http {
            status: status.as_u16(),
            message: error.to_string(),
        }
    } else {
        LontaraXError::Provider(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use httpmock::Method::{GET, POST};
    use httpmock::MockServer;

    #[tokio::test]
    async fn credits_endpoint_parses_response() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/api/v1/chat/credit");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"code":200,"msg":"success","data":123}"#);
        });

        let provider = KieProvider::new(server.url(""), "sk-test").unwrap();
        let credits = provider.credits().await.unwrap();
        assert_eq!(credits.remaining, 123.0);
        assert_eq!(credits.unit, "credits");
    }

    #[tokio::test]
    async fn unauthorized_response_maps_to_http_error() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/api/v1/chat/credit");
            then.status(401)
                .header("content-type", "application/json")
                .body(r#"{"code":401,"msg":"unauthorized","data":null}"#);
        });

        let provider = KieProvider::new(server.url(""), "sk-test").unwrap();
        let error = provider.credits().await.unwrap_err();
        match error {
            LontaraXError::Http { status, .. } => assert_eq!(status, 401),
            other => panic!("unexpected error: {other}"),
        }
    }

    #[tokio::test]
    async fn task_helpers_roundtrip_download_url() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(POST).path("/api/v1/common/download-url");
            then.status(200)
                .header("content-type", "application/json")
                .body(r#"{"code":200,"msg":"success","data":"https://example.com/file"}"#);
        });

        let provider = KieProvider::new(server.url(""), "sk-test").unwrap();
        let client = KieTaskClient::new(provider);
        let url = client
            .download_url("https://tempfile.example")
            .await
            .unwrap();
        assert_eq!(url, "https://example.com/file");
    }
}
