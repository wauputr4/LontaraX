use crate::error::{LontaraXError, Result};
use crate::types::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait AiProvider: Send + Sync {
    fn name(&self) -> &'static str;

    async fn models(&self) -> Result<Vec<Model>>;

    async fn credits(&self) -> Result<Credits>;

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse>;

    async fn image(&self, req: ImageRequest) -> Result<ImageResponse>;

    async fn video(&self, req: VideoRequest) -> Result<VideoResponse>;
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct ProviderRuntimeConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

pub trait ProviderFactory: Send + Sync {
    fn name(&self) -> &'static str;

    fn default_base_url(&self) -> Option<&'static str> {
        None
    }

    fn create(&self, config: ProviderRuntimeConfig) -> Result<Arc<dyn AiProvider>>;
}

#[derive(Default)]
pub struct ProviderRegistry {
    providers: HashMap<String, Arc<dyn AiProvider>>,
}

impl ProviderRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<P>(&mut self, provider: P) -> Option<Arc<dyn AiProvider>>
    where
        P: AiProvider + 'static,
    {
        let name = provider.name().to_string();
        self.providers.insert(name, Arc::new(provider))
    }

    pub fn register_arc(&mut self, provider: Arc<dyn AiProvider>) -> Option<Arc<dyn AiProvider>> {
        let name = provider.name().to_string();
        self.providers.insert(name, provider)
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn AiProvider>> {
        self.providers.get(name).cloned()
    }

    pub fn require(&self, name: &str) -> Result<Arc<dyn AiProvider>> {
        self.get(name)
            .ok_or_else(|| LontaraXError::ProviderNotFound(name.to_string()))
    }

    pub fn list(&self) -> Vec<String> {
        let mut items: Vec<String> = self.providers.keys().cloned().collect();
        items.sort_unstable();
        items
    }
}

#[derive(Default)]
pub struct ProviderFactoryRegistry {
    factories: HashMap<String, Arc<dyn ProviderFactory>>,
}

impl ProviderFactoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, factory: F) -> Option<Arc<dyn ProviderFactory>>
    where
        F: ProviderFactory + 'static,
    {
        let name = factory.name().to_string();
        self.factories.insert(name, Arc::new(factory))
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn ProviderFactory>> {
        self.factories.get(name).cloned()
    }

    pub fn list(&self) -> Vec<String> {
        let mut items: Vec<String> = self.factories.keys().cloned().collect();
        items.sort_unstable();
        items
    }

    pub fn create(&self, name: &str, config: ProviderRuntimeConfig) -> Result<Arc<dyn AiProvider>> {
        let factory = self
            .get(name)
            .ok_or_else(|| LontaraXError::FactoryNotFound(name.to_string()))?;
        factory.create(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    struct DummyProvider;

    #[async_trait]
    impl AiProvider for DummyProvider {
        fn name(&self) -> &'static str {
            "dummy"
        }

        async fn models(&self) -> Result<Vec<Model>> {
            Ok(vec![])
        }

        async fn credits(&self) -> Result<Credits> {
            Ok(Credits {
                remaining: 1.0,
                unit: "credits".into(),
                raw: Value::Null,
            })
        }

        async fn chat(&self, req: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                provider: self.name().into(),
                model: req.model,
                content: "ok".into(),
                raw: Value::Null,
                task_id: None,
                usage: None,
            })
        }

        async fn image(&self, req: ImageRequest) -> Result<ImageResponse> {
            Ok(ImageResponse {
                provider: self.name().into(),
                model: req.model,
                urls: vec![],
                raw: Value::Null,
                task_id: None,
            })
        }

        async fn video(&self, req: VideoRequest) -> Result<VideoResponse> {
            Ok(VideoResponse {
                provider: self.name().into(),
                model: req.model,
                urls: vec![],
                raw: Value::Null,
                task_id: None,
            })
        }
    }

    struct DummyFactory;

    impl ProviderFactory for DummyFactory {
        fn name(&self) -> &'static str {
            "dummy"
        }

        fn create(&self, _config: ProviderRuntimeConfig) -> Result<Arc<dyn AiProvider>> {
            Ok(Arc::new(DummyProvider))
        }
    }

    #[tokio::test]
    async fn provider_registry_registers_and_gets() {
        let mut registry = ProviderRegistry::new();
        registry.register(DummyProvider);

        assert_eq!(registry.list(), vec!["dummy".to_string()]);
        let provider = registry.require("dummy").unwrap();
        assert_eq!(provider.name(), "dummy");

        let chat = provider
            .chat(ChatRequest {
                model: "test-model".into(),
                messages: vec![ChatMessage {
                    role: "user".into(),
                    content: "hello".into(),
                }],
                temperature: None,
                max_tokens: None,
                metadata_json: Value::Null,
            })
            .await
            .unwrap();

        assert_eq!(chat.content, "ok");
    }

    #[tokio::test]
    async fn factory_registry_creates_provider() {
        let mut registry = ProviderFactoryRegistry::new();
        registry.register(DummyFactory);

        let provider = registry
            .create(
                "dummy",
                ProviderRuntimeConfig {
                    name: "dummy".into(),
                    base_url: None,
                    api_key: None,
                    timeout_secs: None,
                },
            )
            .unwrap();

        assert_eq!(provider.name(), "dummy");
    }
}
