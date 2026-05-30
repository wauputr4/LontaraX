use crate::cli::{
    ChatArgs, Cli, Commands, HistoryArgs, ImageArgs, ModelsArgs, ProviderCommands,
    ProviderSelectionArgs, VideoArgs,
};
use crate::output::{print_bullet, print_json, print_section};
use lontarax_core::{
    mask_secret, AiProvider, ChatMessage, ChatRequest, Config, Credits, ImageRequest,
    LontaraXError, ModelCapability, OutputFormat, ProviderFactoryRegistry, ProviderRuntimeConfig,
    RequestStatus, Result, VideoRequest,
};
use lontarax_db::repositories::{
    ModelRepository, ProviderRepository, RequestCapability, RequestLogInput, RequestRepository,
};
use lontarax_db::Database;
use serde::Serialize;
use serde_json::json;
use std::collections::hash_map::DefaultHasher;
use std::env;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

#[derive(Debug, Serialize)]
struct InitOutput {
    config_path: String,
    database_url: String,
}

#[derive(Debug, Serialize)]
struct ProviderListOutput {
    providers: Vec<ProviderOutput>,
}

#[derive(Debug, Serialize)]
struct ProviderOutput {
    id: String,
    name: String,
    base_url: String,
    api_key: String,
    enabled: bool,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct ModelsOutput {
    provider: String,
    cached: usize,
    models: Vec<lontarax_core::Model>,
}

#[derive(Debug, Serialize)]
struct CreditsOutput {
    provider: String,
    credits: Credits,
}

#[derive(Debug, Serialize)]
struct HistoryOutput {
    requests: Vec<lontarax_db::repositories::RequestEntity>,
}

#[derive(Debug, Serialize)]
struct CommandOutput {
    message: String,
}

pub async fn run(cli: Cli) -> Result<()> {
    let config_path = cli.config.clone();
    let mut config = Config::load(config_path.as_deref())?;
    config.apply_env_overrides();
    let json_output = cli.json || matches!(config.default_output, OutputFormat::Json);
    let db = Database::from_config(&config).await?;
    db.migrate().await?;

    match cli.command {
        Commands::Init => init(config, config_path.as_deref(), json_output).await,
        Commands::Provider { command } => provider_command(&db, json_output, command).await,
        Commands::Models(args) => models_command(&db, &config, json_output, args).await,
        Commands::Credits(args) => credits_command(&db, &config, json_output, args).await,
        Commands::Chat(args) => chat_command(&db, &config, json_output, args).await,
        Commands::Image(args) => image_command(&db, &config, json_output, args).await,
        Commands::Video(args) => video_command(&db, &config, json_output, args).await,
        Commands::History(args) => history_command(&db, json_output, args).await,
    }
}

async fn init(
    config: Config,
    config_path: Option<&std::path::Path>,
    json_output: bool,
) -> Result<()> {
    let saved_config_path = config.save(config_path)?;
    let database_url = config.effective_database_url()?;
    let output = InitOutput {
        config_path: saved_config_path.display().to_string(),
        database_url,
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_section("Initialization complete");
        print_bullet("config", output.config_path);
        print_bullet("database", output.database_url);
    }

    Ok(())
}

async fn provider_command(
    db: &Database,
    json_output: bool,
    command: ProviderCommands,
) -> Result<()> {
    let repo = ProviderRepository::new(db.pool());
    let factories = provider_factories();

    match command {
        ProviderCommands::Add {
            name,
            api_key,
            base_url,
        } => {
            let api_key = resolve_api_key(&name, api_key)?;
            let factory = factories
                .get(&name)
                .ok_or_else(|| LontaraXError::FactoryNotFound(name.clone()))?;
            let base_url = base_url
                .or_else(|| factory.default_base_url().map(|value| value.to_string()))
                .ok_or_else(|| {
                    LontaraXError::InvalidConfig(format!("provider {name} requires a base_url"))
                })?;

            let runtime_config = ProviderRuntimeConfig {
                name: name.clone(),
                base_url: Some(base_url.clone()),
                api_key: Some(api_key.clone()),
                timeout_secs: None,
            };

            let _instance = factory.create(runtime_config)?;
            let record = repo.upsert(&name, &base_url, &api_key, true).await?;
            let output = ProviderOutput {
                id: record.id,
                name: record.name,
                base_url: record.base_url,
                api_key: mask_secret(&record.api_key),
                enabled: record.enabled,
                created_at: record.created_at,
            };

            if json_output {
                print_json(&output)?;
            } else {
                print_section("Provider added");
                print_bullet("name", output.name);
                print_bullet("base_url", output.base_url);
                print_bullet("api_key", output.api_key);
                print_bullet("enabled", output.enabled);
            }
        }
        ProviderCommands::List => {
            let providers = repo
                .list()
                .await?
                .into_iter()
                .map(|record| ProviderOutput {
                    id: record.id,
                    name: record.name,
                    base_url: record.base_url,
                    api_key: mask_secret(&record.api_key),
                    enabled: record.enabled,
                    created_at: record.created_at,
                })
                .collect::<Vec<_>>();
            let output = ProviderListOutput { providers };

            if json_output {
                print_json(&output)?;
            } else {
                print_section("Configured providers");
                for provider in output.providers {
                    print_bullet(
                        &provider.name,
                        format!(
                            "base_url={} api_key={} enabled={} created_at={}",
                            provider.base_url,
                            provider.api_key,
                            provider.enabled,
                            provider.created_at
                        ),
                    );
                }
            }
        }
        ProviderCommands::Remove { name } => {
            let deleted = repo.remove(&name).await?;
            let output = CommandOutput {
                message: format!("removed {deleted} provider(s) named {name}"),
            };
            if json_output {
                print_json(&output)?;
            } else {
                println!("{}", output.message);
            }
        }
    }

    Ok(())
}

async fn models_command(
    db: &Database,
    config: &Config,
    json_output: bool,
    args: ModelsArgs,
) -> Result<()> {
    let provider_name = resolve_provider_name(&args.provider, config)?;
    let provider = resolve_provider_instance(db, &provider_name).await?;
    let models = provider.models().await?;
    let repo = ModelRepository::new(db.pool());
    let cached = repo.upsert_many(&provider_name, &models).await?;
    let output = ModelsOutput {
        provider: provider_name,
        cached,
        models,
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_section(&format!("Models for {}", output.provider));
        print_bullet("cached", output.cached);
        for model in output.models {
            print_bullet(
                &model.display_name,
                format!(
                    "model_id={} capability={} enabled={}",
                    model.model_id, model.capability, model.enabled
                ),
            );
        }
    }

    Ok(())
}

async fn credits_command(
    db: &Database,
    config: &Config,
    json_output: bool,
    args: ProviderSelectionArgs,
) -> Result<()> {
    let provider_name = resolve_provider_name(&args.provider, config)?;
    let provider = resolve_provider_instance(db, &provider_name).await?;
    let credits = provider.credits().await?;
    let output = CreditsOutput {
        provider: provider_name,
        credits,
    };

    if json_output {
        print_json(&output)?;
    } else {
        print_section(&format!("Credits for {}", output.provider));
        print_bullet("remaining", output.credits.remaining);
        print_bullet("unit", output.credits.unit);
    }

    Ok(())
}

async fn chat_command(
    db: &Database,
    config: &Config,
    json_output: bool,
    args: ChatArgs,
) -> Result<()> {
    let provider_name = resolve_provider_name(&args.provider, config)?;
    let provider = resolve_provider_instance(db, &provider_name).await?;
    let model =
        resolve_model_for_capability(db, &provider_name, args.model, ModelCapability::Chat).await?;
    let prompt = args.prompt;
    let request = ChatRequest {
        model: model.clone(),
        messages: vec![ChatMessage {
            role: "user".into(),
            content: prompt.clone(),
        }],
        temperature: None,
        max_tokens: None,
        metadata_json: json!({"source": "cli", "command": "chat"}),
    };
    let result = provider.chat(request).await;
    let request_repo = RequestRepository::new(db.pool());
    let input_hash = Some(hash_input(&format!("{provider_name}:{model}:{prompt}")));

    match result {
        Ok(response) => {
            request_repo
                .log(RequestLogInput {
                    provider: provider_name.clone(),
                    model: model.clone(),
                    capability: RequestCapability::Chat,
                    status: RequestStatus::Success,
                    cost: None,
                    input_hash,
                })
                .await?;
            if json_output {
                print_json(&response)?;
            } else {
                print_section("Chat response");
                println!("{}", response.content);
            }
        }
        Err(error) => {
            request_repo
                .log(RequestLogInput {
                    provider: provider_name.clone(),
                    model: model.clone(),
                    capability: RequestCapability::Chat,
                    status: RequestStatus::Failed,
                    cost: None,
                    input_hash,
                })
                .await?;
            return Err(error);
        }
    }

    Ok(())
}

async fn image_command(
    db: &Database,
    config: &Config,
    json_output: bool,
    args: ImageArgs,
) -> Result<()> {
    let provider_name = resolve_provider_name(&args.provider, config)?;
    let provider = resolve_provider_instance(db, &provider_name).await?;
    let model =
        resolve_model_for_capability(db, &provider_name, args.model, ModelCapability::Image)
            .await?;
    let prompt = args.prompt;
    let request = ImageRequest {
        model: model.clone(),
        prompt: prompt.clone(),
        n: 1,
        size: None,
        metadata_json: json!({"source": "cli", "command": "image"}),
    };
    let result = provider.image(request).await;
    let request_repo = RequestRepository::new(db.pool());
    let input_hash = Some(hash_input(&format!("{provider_name}:{model}:{prompt}")));

    match result {
        Ok(response) => {
            request_repo
                .log(RequestLogInput {
                    provider: provider_name.clone(),
                    model: model.clone(),
                    capability: RequestCapability::Image,
                    status: RequestStatus::Success,
                    cost: None,
                    input_hash,
                })
                .await?;
            if json_output {
                print_json(&response)?;
            } else {
                print_section("Image response");
                for url in response.urls {
                    println!("{url}");
                }
            }
        }
        Err(error) => {
            request_repo
                .log(RequestLogInput {
                    provider: provider_name.clone(),
                    model: model.clone(),
                    capability: RequestCapability::Image,
                    status: RequestStatus::Failed,
                    cost: None,
                    input_hash,
                })
                .await?;
            return Err(error);
        }
    }

    Ok(())
}

async fn video_command(
    db: &Database,
    config: &Config,
    json_output: bool,
    args: VideoArgs,
) -> Result<()> {
    let provider_name = resolve_provider_name(&args.provider, config)?;
    let provider = resolve_provider_instance(db, &provider_name).await?;
    let model =
        resolve_model_for_capability(db, &provider_name, args.model, ModelCapability::Video)
            .await?;
    let prompt = args.prompt;
    let request = VideoRequest {
        model: model.clone(),
        prompt: prompt.clone(),
        metadata_json: json!({"source": "cli", "command": "video"}),
    };
    let result = provider.video(request).await;
    let request_repo = RequestRepository::new(db.pool());
    let input_hash = Some(hash_input(&format!("{provider_name}:{model}:{prompt}")));

    match result {
        Ok(response) => {
            request_repo
                .log(RequestLogInput {
                    provider: provider_name.clone(),
                    model: model.clone(),
                    capability: RequestCapability::Video,
                    status: RequestStatus::Success,
                    cost: None,
                    input_hash,
                })
                .await?;
            if json_output {
                print_json(&response)?;
            } else {
                print_section("Video response");
                for url in response.urls {
                    println!("{url}");
                }
            }
        }
        Err(error) => {
            request_repo
                .log(RequestLogInput {
                    provider: provider_name.clone(),
                    model: model.clone(),
                    capability: RequestCapability::Video,
                    status: RequestStatus::Failed,
                    cost: None,
                    input_hash,
                })
                .await?;
            return Err(error);
        }
    }

    Ok(())
}

async fn history_command(db: &Database, json_output: bool, args: HistoryArgs) -> Result<()> {
    let repo = RequestRepository::new(db.pool());
    let requests = repo.list(args.limit).await?;
    let output = HistoryOutput { requests };

    if json_output {
        print_json(&output)?;
    } else {
        print_section("Request history");
        for request in output.requests {
            print_bullet(
                &request.provider,
                format!(
                    "model={} capability={} status={} cost={:?} created_at={}",
                    request.model,
                    request.capability,
                    request.status,
                    request.cost,
                    request.created_at
                ),
            );
        }
    }

    Ok(())
}

async fn resolve_provider_instance(
    db: &Database,
    provider_name: &str,
) -> Result<Arc<dyn AiProvider>> {
    let repo = ProviderRepository::new(db.pool());
    let record = repo
        .get(provider_name)
        .await?
        .ok_or_else(|| LontaraXError::ProviderNotFound(provider_name.to_string()))?;

    if !record.enabled {
        return Err(LontaraXError::Provider(format!(
            "provider {provider_name} is disabled"
        )));
    }

    let factories = provider_factories();
    let factory = factories
        .get(provider_name)
        .ok_or_else(|| LontaraXError::FactoryNotFound(provider_name.to_string()))?;

    factory.create(ProviderRuntimeConfig {
        name: provider_name.to_string(),
        base_url: Some(record.base_url),
        api_key: Some(record.api_key),
        timeout_secs: None,
    })
}

async fn resolve_model_for_capability(
    db: &Database,
    provider_name: &str,
    requested_model: Option<String>,
    capability: ModelCapability,
) -> Result<String> {
    if let Some(model) = requested_model {
        let repo = ModelRepository::new(db.pool());
        if repo
            .find_by_model_id(provider_name, &model)
            .await?
            .is_none()
        {
            return Err(LontaraXError::ModelNotFound(model));
        }
        return Ok(model);
    }

    let repo = ModelRepository::new(db.pool());
    let models = repo
        .list_by_capability(provider_name, capability.clone())
        .await?;
    let model = models
        .into_iter()
        .find(|model| model.enabled)
        .ok_or_else(|| LontaraXError::ModelNotFound(format!("{provider_name}:{capability}")))?;
    Ok(model.model_id)
}

fn resolve_provider_name(requested: &Option<String>, config: &Config) -> Result<String> {
    requested
        .clone()
        .or_else(|| config.default_provider.clone())
        .ok_or_else(|| {
            LontaraXError::InvalidConfig(
                "no provider selected; pass --provider or set default_provider".into(),
            )
        })
}

fn resolve_api_key(name: &str, api_key: Option<String>) -> Result<String> {
    if let Some(api_key) = api_key {
        if !api_key.trim().is_empty() {
            return Ok(api_key);
        }
    }

    let env_var = format!("LONTARAX_{}_API_KEY", name.to_uppercase().replace('-', "_"));
    if let Ok(value) = env::var(env_var) {
        if !value.trim().is_empty() {
            return Ok(value);
        }
    }

    Err(LontaraXError::MissingApiKey(name.to_string()))
}

fn provider_factories() -> ProviderFactoryRegistry {
    let mut registry = ProviderFactoryRegistry::new();
    lontarax_provider_kie::register_factories(&mut registry);
    registry
}

fn hash_input(value: &str) -> String {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
