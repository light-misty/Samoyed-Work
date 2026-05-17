use tauri::State;

use crate::errors::{CommandError, LLM_CONNECTION_FAILED};
use crate::models::llm::{ConnectionResult, ProviderConfig, ProviderInfo};
use crate::AppState;

/// 测试 LLM Provider 连接
#[tauri::command]
pub async fn test_connection(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionResult, CommandError> {
    let config = state.config.lock().await;
    let llm_config = config.load_llm_config()?;

    let provider = llm_config
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| {
            CommandError::llm(
                LLM_CONNECTION_FAILED,
                format!("Provider '{}' 不存在", provider_id),
            )
        })?;

    // 构造测试请求，向 LLM 发送简单消息以验证连接
    let client = reqwest::Client::new();
    let start = std::time::Instant::now();

    let body = serde_json::json!({
        "model": provider.model,
        "messages": [{"role": "user", "content": "Hi"}],
        "max_tokens": 1,
        "temperature": provider.advanced.temperature,
    });

    let url = format!(
        "{}/chat/completions",
        provider.api_base_url.trim_end_matches('/')
    );

    let result = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", provider.api_key_encrypted))
        .header("Content-Type", "application/json")
        .timeout(std::time::Duration::from_secs(provider.advanced.timeout_seconds as u64))
        .json(&body)
        .send()
        .await;

    let latency_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(response) => {
            if response.status().is_success() {
                Ok(ConnectionResult {
                    success: true,
                    provider_id: Some(provider_id),
                    latency_ms,
                    model_info: None,
                    model: Some(provider.model.clone()),
                    error_message: None,
                    error: None,
                })
            } else {
                let status = response.status();
                let body_text = response.text().await.unwrap_or_default();
                Ok(ConnectionResult {
                    success: false,
                    provider_id: Some(provider_id),
                    latency_ms,
                    model_info: None,
                    model: None,
                    error_message: Some(format!("HTTP {}: {}", status, body_text)),
                    error: Some(format!("HTTP {}: {}", status, body_text)),
                })
            }
        }
        Err(e) => Ok(ConnectionResult {
            success: false,
            provider_id: Some(provider_id),
            latency_ms,
            model_info: None,
            model: None,
            error_message: Some(e.to_string()),
            error: Some(e.to_string()),
        }),
    }
}

/// 列出所有 LLM Provider
#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, CommandError> {
    let config = state.config.lock().await;
    let llm_config = config.load_llm_config()?;

    Ok(llm_config
        .providers
        .iter()
        .map(|p| ProviderInfo {
            id: p.id.clone(),
            name: p.name.clone(),
            provider_type: format!("{:?}", p.provider_type).to_lowercase(),
            api_base: p.api_base_url.clone(),
            model: p.model.clone(),
            is_default: p.is_default,
            is_available: true,
            created_at: String::new(),
            is_connected: None,
        })
        .collect())
}

/// 添加 LLM Provider
#[tauri::command]
pub async fn add_provider(
    config: ProviderConfig,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config()?;

    let provider_type = match config.provider_type.as_str() {
        "openai" => crate::config::llm_config::ProviderType::OpenAI,
        "anthropic" => crate::config::llm_config::ProviderType::Anthropic,
        "ollama" => crate::config::llm_config::ProviderType::Ollama,
        _ => crate::config::llm_config::ProviderType::Custom,
    };

    let provider = crate::config::llm_config::LlmProvider {
        id: uuid::Uuid::new_v4().to_string(),
        provider_type,
        name: config.name,
        api_base_url: config.api_base,
        api_key_encrypted: config.api_key,
        model: config.model,
        is_default: llm_config.providers.is_empty(),
        advanced: crate::config::llm_config::AdvancedConfig::default(),
    };

    crate::config::llm_config::add_provider(&mut llm_config, provider)?;
    cfg_manager.save_llm_config(&llm_config)?;
    Ok(())
}

/// 更新 LLM Provider
#[tauri::command]
pub async fn update_provider(
    provider_id: String,
    config: ProviderConfig,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config()?;

    let provider_type = match config.provider_type.as_str() {
        "openai" => crate::config::llm_config::ProviderType::OpenAI,
        "anthropic" => crate::config::llm_config::ProviderType::Anthropic,
        "ollama" => crate::config::llm_config::ProviderType::Ollama,
        _ => crate::config::llm_config::ProviderType::Custom,
    };

    // 保留原有的 id、is_default、advanced 配置
    let existing = llm_config
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| {
            CommandError::llm(
                LLM_CONNECTION_FAILED,
                format!("Provider '{}' 不存在", provider_id),
            )
        })?;

    let provider = crate::config::llm_config::LlmProvider {
        id: provider_id.clone(),
        provider_type,
        name: config.name,
        api_base_url: config.api_base,
        api_key_encrypted: config.api_key,
        model: config.model,
        is_default: existing.is_default,
        advanced: existing.advanced.clone(),
    };

    crate::config::llm_config::update_provider(&mut llm_config, &provider_id, provider)?;
    cfg_manager.save_llm_config(&llm_config)?;
    Ok(())
}

/// 删除 LLM Provider
#[tauri::command]
pub async fn delete_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config()?;
    crate::config::llm_config::delete_provider(&mut llm_config, &provider_id)?;
    cfg_manager.save_llm_config(&llm_config)?;
    Ok(())
}

/// 设置默认 LLM Provider
#[tauri::command]
pub async fn set_default_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config()?;
    crate::config::llm_config::set_default_provider(&mut llm_config, &provider_id)?;
    cfg_manager.save_llm_config(&llm_config)?;
    Ok(())
}
