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
    log::info!("测试 LLM Provider 连接: provider_id={}", provider_id);
    let config = state.config.lock().await;
    let llm_config = config.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;

    let provider = llm_config
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .ok_or_else(|| {
            log::error!("Provider 不存在: provider_id={}", provider_id);
            CommandError::llm(
                LLM_CONNECTION_FAILED,
                format!("Provider '{}' 不存在", provider_id),
            )
        })?;

    log::debug!(
        "找到 Provider: provider_id={}, model={}",
        provider_id,
        provider.model
    );

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
                log::info!(
                    "Provider 连接测试成功: provider_id={}, model={}, latency_ms={}",
                    provider_id,
                    provider.model,
                    latency_ms
                );
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
                log::warn!(
                    "Provider 连接测试失败: provider_id={}, model={}, http_status={}, latency_ms={}",
                    provider_id,
                    provider.model,
                    status,
                    latency_ms
                );
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
        Err(e) => {
            log::warn!(
                "Provider 连接测试网络错误: provider_id={}, model={}, error={}, latency_ms={}",
                provider_id,
                provider.model,
                e,
                latency_ms
            );
            Ok(ConnectionResult {
                success: false,
                provider_id: Some(provider_id),
                latency_ms,
                model_info: None,
                model: None,
                error_message: Some(e.to_string()),
                error: Some(e.to_string()),
            })
        }
    }
}

/// 列出所有 LLM Provider
#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, CommandError> {
    log::info!("列出所有 LLM Provider");
    let config = state.config.lock().await;
    let llm_config = config.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;

    let providers: Vec<ProviderInfo> = llm_config
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
        .collect();

    log::info!("列出 Provider 完成: count={}", providers.len());
    Ok(providers)
}

/// 添加 LLM Provider
#[tauri::command]
pub async fn add_provider(
    config: ProviderConfig,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "添加 LLM Provider: name={}, provider_type={}, model={}",
        config.name,
        config.provider_type,
        config.model
    );
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;

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

    crate::config::llm_config::add_provider(&mut llm_config, provider).map_err(|e| {
        log::error!("添加 Provider 失败: {}", e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;
    log::info!("Provider 添加成功");
    Ok(())
}

/// 更新 LLM Provider
#[tauri::command]
pub async fn update_provider(
    provider_id: String,
    config: ProviderConfig,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "更新 LLM Provider: provider_id={}, name={}, provider_type={}, model={}",
        provider_id,
        config.name,
        config.provider_type,
        config.model
    );
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;

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
            log::error!("Provider 不存在: provider_id={}", provider_id);
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

    crate::config::llm_config::update_provider(&mut llm_config, &provider_id, provider).map_err(|e| {
        log::error!("更新 Provider 失败: provider_id={}, error={}", provider_id, e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;
    log::info!("Provider 更新成功: provider_id={}", provider_id);
    Ok(())
}

/// 删除 LLM Provider
#[tauri::command]
pub async fn delete_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("删除 LLM Provider: provider_id={}", provider_id);
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;
    crate::config::llm_config::delete_provider(&mut llm_config, &provider_id).map_err(|e| {
        log::error!("删除 Provider 失败: provider_id={}, error={}", provider_id, e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;
    log::info!("Provider 删除成功: provider_id={}", provider_id);
    Ok(())
}

/// 设置默认 LLM Provider
#[tauri::command]
pub async fn set_default_provider(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("设置默认 LLM Provider: provider_id={}", provider_id);
    let cfg_manager = state.config.lock().await;
    let mut llm_config = cfg_manager.load_llm_config().map_err(|e| {
        log::error!("加载 LLM 配置失败: {}", e);
        e
    })?;
    crate::config::llm_config::set_default_provider(&mut llm_config, &provider_id).map_err(|e| {
        log::error!("设置默认 Provider 失败: provider_id={}, error={}", provider_id, e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;
    log::info!("默认 Provider 设置成功: provider_id={}", provider_id);
    Ok(())
}
