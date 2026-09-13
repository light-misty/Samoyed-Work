use std::collections::HashMap;
use std::sync::Arc;

use reqwest::Client;
use tauri::State;

use crate::config::llm_config::{AdvancedConfig, ProviderType};
use crate::errors::{CommandError, LLM_AUTH_FAILED, LLM_CONNECTION_FAILED};
use crate::models::llm::{ConnectionResult, ProviderConfig, ProviderInfo};
use crate::services::llm::anthropic_adapter::AnthropicAdapter;
use crate::services::llm::gemini_adapter::GeminiAdapter;
use crate::services::llm::openai_adapter::OpenAiAdapter;
use crate::services::llm::provider::LlmProvider;
use crate::AppState;

/// 测试 LLM Provider 连接
#[tauri::command]
pub async fn test_connection(
    provider_id: String,
    state: State<'_, AppState>,
) -> Result<ConnectionResult, CommandError> {
    log::info!("测试 LLM Provider 连接: provider_id={}", provider_id);
    let router = state.llm_router.read().await;
    router.test_connection(&provider_id).await
}

/// 使用临时配置测试 LLM Provider 连接（用于添加/编辑模式，不需要已保存的 provider）
/// 编辑模式下传入 provider_id，当 api_key 为空时从已保存的 Provider 中查找 API Key
#[tauri::command]
pub async fn test_connection_with_config(
    config: ProviderConfig,
    provider_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConnectionResult, CommandError> {
    log::info!(
        "使用临时配置测试连接: provider_type={}, api_base={}, model={}, provider_id={:?}",
        config.provider_type,
        config.api_base,
        config.model,
        provider_id
    );

    // 验证必要参数（顺序与表单字段顺序一致）
    if config.api_base.trim().is_empty() {
        return Ok(ConnectionResult {
            success: false,
            provider_id: None,
            latency_ms: 0,
            model_info: None,
            model: None,
            error_message: Some("请输入 API Base URL".to_string()),
            error: Some("请输入 API Base URL".to_string()),
        });
    }
    if config.model.trim().is_empty() {
        return Ok(ConnectionResult {
            success: false,
            provider_id: None,
            latency_ms: 0,
            model_info: None,
            model: None,
            error_message: Some("请输入模型名称".to_string()),
            error: Some("请输入模型名称".to_string()),
        });
    }

    // Ollama 等本地部署不需要 API Key，为空时使用空字符串
    let is_ollama = config.provider_type == "ollama";
    // 编辑模式下 api_key 为空时，从已保存的 Provider 中查找 API Key
    let api_key = if config.api_key.trim().is_empty() {
        if is_ollama {
            // Ollama 不需要 API Key，使用空字符串
            String::new()
        } else if let Some(ref pid) = provider_id {
            let cfg_manager = state.config.lock().await;
            let llm_config = cfg_manager.load_llm_config().map_err(|e| {
                log::error!("加载 LLM 配置失败: {}", e);
                e
            })?;
            match llm_config.providers.iter().find(|p| p.id == *pid) {
                Some(existing) => {
                    log::info!(
                        "API Key 为空，使用已保存 Provider 的密钥: provider_id={}",
                        pid
                    );
                    existing.api_key_encrypted.clone()
                }
                None => {
                    return Ok(ConnectionResult {
                        success: false,
                        provider_id: None,
                        latency_ms: 0,
                        model_info: None,
                        model: None,
                        error_message: Some("请输入 API Key".to_string()),
                        error: Some("请输入 API Key".to_string()),
                    });
                }
            }
        } else {
            return Ok(ConnectionResult {
                success: false,
                provider_id: None,
                latency_ms: 0,
                model_info: None,
                model: None,
                error_message: Some("请输入 API Key".to_string()),
                error: Some("请输入 API Key".to_string()),
            });
        }
    } else {
        config.api_key.clone()
    };

    // 创建临时 AdvancedConfig
    let advanced = AdvancedConfig::default();

    // 根据 Provider 类型创建临时 adapter
    let provider_type_enum = match config.provider_type.as_str() {
        "openai" => ProviderType::OpenAI,
        "anthropic" => ProviderType::Anthropic,
        "ollama" => ProviderType::Ollama,
        "gemini" => ProviderType::Gemini,
        _ => ProviderType::Custom,
    };

    // 根据 Provider 类型和 API base URL 自动检测 reasoning_in_content 配置
    let mut advanced = advanced;
    let is_deepseek = config.api_base.to_lowercase().contains("deepseek");
    if is_deepseek {
        advanced.reasoning_in_content = false;
        log::info!("检测到 DeepSeek API, 设置 reasoning_in_content=false");
    }

    let adapter: Box<dyn LlmProvider> = match provider_type_enum {
        ProviderType::OpenAI | ProviderType::Custom | ProviderType::Ollama => {
            Box::new(OpenAiAdapter::new(
                config.api_base.clone(),
                api_key.clone(),
                config.model.clone(),
                advanced,
            ))
        }
        ProviderType::Anthropic => Box::new(AnthropicAdapter::new(
            config.api_base.clone(),
            api_key.clone(),
            config.model.clone(),
            advanced,
        )),
        ProviderType::Gemini => Box::new(GeminiAdapter::new(
            config.api_base.clone(),
            api_key.clone(),
            config.model.clone(),
            advanced,
        )),
    };

    // 执行测试连接
    let start = std::time::Instant::now();
    let result = adapter.test_connection().await?;
    log::info!(
        "临时配置测试连接完成: 成功={}, 延迟={}ms",
        result.success,
        start.elapsed().as_millis()
    );
    Ok(result)
}

/// 根据 API Key 与 API Base URL 获取可用的模型列表
/// OpenAI 兼容格式请求 GET {api_base}/models；Anthropic 兼容格式依次尝试
/// {api_base}/v1/models 与 {api_base}/models（DeepSeek 官方 Anthropic 端点未文档化模型列表接口）
#[tauri::command]
pub async fn list_models(
    api_base: String,
    api_key: String,
    provider_type: String,
) -> Result<Vec<String>, CommandError> {
    log::info!(
        "获取模型列表: provider_type={}, api_base={}",
        provider_type,
        api_base
    );

    if api_base.trim().is_empty() {
        return Err(CommandError::llm(
            crate::errors::LLM_INVALID_REQUEST,
            "请输入 API Base URL".to_string(),
        ));
    }
    // Ollama 等本地部署不需要 API Key
    let is_ollama = provider_type == "ollama";
    if !is_ollama && api_key.trim().is_empty() {
        return Err(CommandError::llm(
            crate::errors::LLM_INVALID_REQUEST,
            "请输入 API Key".to_string(),
        ));
    }

    let base = api_base.trim_end_matches('/');
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| {
            log::error!("创建 HTTP 客户端失败: {}", e);
            CommandError::llm(
                LLM_CONNECTION_FAILED,
                format!("创建 HTTP 客户端失败: {}", e),
            )
        })?;

    // 构造候选端点：Anthropic 兼容格式先试 /v1/models，再试 /models；其他格式直接 /models
    let mut urls: Vec<String> = Vec::new();
    if provider_type == "anthropic" {
        urls.push(format!("{}/v1/models", base));
    }
    urls.push(format!("{}/models", base));

    let mut last_error: Option<CommandError> = None;
    for url in &urls {
        log::info!("请求模型列表端点: {}", url);
        // 构建请求：Ollama 不需要 Authorization 头部
        let mut request = client.get(url);
        if !is_ollama && !api_key.trim().is_empty() {
            request = request.header("Authorization", format!("Bearer {}", api_key));
        }
        let response = request.send().await;

        match response {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    let value: serde_json::Value = resp.json().await.map_err(|e| {
                        log::error!("解析模型列表响应失败: {}", e);
                        CommandError::llm(
                            crate::errors::LLM_RESPONSE_PARSE_ERROR,
                            format!("解析模型列表响应失败: {}", e),
                        )
                    })?;
                    let models: Vec<String> = value["data"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|m| m["id"].as_str().map(String::from))
                                .collect()
                        })
                        .unwrap_or_default();
                    log::info!("获取模型列表成功: count={}", models.len());
                    return Ok(models);
                }
                let error_body = resp.text().await.unwrap_or_default();
                let msg = format!("API 请求失败 ({}): {}", status, error_body);
                log::warn!("模型列表端点 {} 请求失败: {}", url, msg);
                // 401 认证失败直接返回，不再尝试其他端点
                if status.as_u16() == 401 {
                    return Err(CommandError::llm(
                        LLM_AUTH_FAILED,
                        format!("认证失败: {}", error_body),
                    ));
                }
                last_error = Some(CommandError::llm(LLM_CONNECTION_FAILED, msg));
            }
            Err(e) => {
                let msg = format!("请求失败: {}", e);
                log::warn!("模型列表端点 {} {}", url, msg);
                last_error = Some(CommandError::llm(LLM_CONNECTION_FAILED, msg));
            }
        }
    }

    let err = last_error.unwrap_or_else(|| {
        CommandError::llm(LLM_CONNECTION_FAILED, "获取模型列表失败".to_string())
    });
    log::error!("获取模型列表最终失败: {}", err.message);
    Err(err)
}

/// 列出所有 LLM Provider
#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<ProviderInfo>, CommandError> {
    log::info!("列出所有 LLM Provider");
    let router = state.llm_router.read().await;
    let providers = router.list_providers();
    log::info!("列出 Provider 完成: count={}", providers.len());
    Ok(providers)
}

/// 添加 LLM Provider 并重建路由器
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
        "gemini" => crate::config::llm_config::ProviderType::Gemini,
        _ => crate::config::llm_config::ProviderType::Custom,
    };

    let provider = crate::config::llm_config::LlmProvider {
        id: uuid::Uuid::new_v4().to_string(),
        provider_type,
        name: config.name,
        api_base_url: config.api_base,
        api_key_encrypted: config.api_key,
        model: config.model,
        advanced: crate::config::llm_config::AdvancedConfig {
            context_window: config.context_window,
            reasoning_effort: config.reasoning_effort,
            ..Default::default()
        },
        supports_vision: config.supports_vision,
    };

    crate::config::llm_config::add_provider(&mut llm_config, provider).map_err(|e| {
        log::error!("添加 Provider 失败: {}", e);
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("Provider 添加成功");
    Ok(())
}

/// 更新 LLM Provider 并重建路由器
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
        "gemini" => crate::config::llm_config::ProviderType::Gemini,
        _ => crate::config::llm_config::ProviderType::Custom,
    };

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

    // 如果传入的 api_key 为空，保留原有的 api_key_encrypted（编辑时"留空则保持不变"）
    let api_key_to_save = if config.api_key.trim().is_empty() {
        log::info!(
            "API Key 为空，保留原有加密密钥: provider_id={}",
            provider_id
        );
        existing.api_key_encrypted.clone()
    } else {
        config.api_key.clone()
    };

    let mut advanced = existing.advanced.clone();
    // 如果前端传入了 context_window，更新；否则保留原值
    if config.context_window.is_some() {
        advanced.context_window = config.context_window;
    }
    // 如果前端传入了 reasoning_effort，更新；否则保留原值（空字符串表示清除档位，恢复模型默认行为）
    if config.reasoning_effort.is_some() {
        advanced.reasoning_effort = config.reasoning_effort.clone().filter(|e| !e.is_empty());
    }

    let provider = crate::config::llm_config::LlmProvider {
        id: provider_id.clone(),
        provider_type,
        name: config.name,
        api_base_url: config.api_base,
        api_key_encrypted: api_key_to_save,
        model: config.model,
        advanced,
        supports_vision: config.supports_vision,
    };

    crate::config::llm_config::update_provider(&mut llm_config, &provider_id, provider).map_err(
        |e| {
            log::error!(
                "更新 Provider 失败: provider_id={}, error={}",
                provider_id,
                e
            );
            e
        },
    )?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("Provider 更新成功: provider_id={}", provider_id);
    Ok(())
}

/// 删除 LLM Provider 并重建路由器
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
        log::error!(
            "删除 Provider 失败: provider_id={}, error={}",
            provider_id,
            e
        );
        e
    })?;
    cfg_manager.save_llm_config(&llm_config).map_err(|e| {
        log::error!("保存 LLM 配置失败: {}", e);
        e
    })?;

    // 重建 LlmRouter
    rebuild_router(&state, &llm_config).await;

    log::info!("Provider 删除成功: provider_id={}", provider_id);
    Ok(())
}

/// 根据 LLM 配置重建 LlmRouter
async fn rebuild_router(
    state: &State<'_, AppState>,
    llm_config: &crate::config::llm_config::LlmConfig,
) {
    // 保留旧路由器的 AppHandle，避免重建后丢失事件通知能力
    let app_handle = {
        let guard = state.llm_router.read().await;
        guard.app_handle()
    };
    let new_router = crate::services::llm::router::LlmRouter::from_config(llm_config)
        .with_app_handle(app_handle);
    let mut guard = state.llm_router.write().await;
    *guard = Arc::new(new_router);
    log::info!("LlmRouter 已重建");
}

/// 对所有 LLM Provider 执行健康检查
#[tauri::command]
pub async fn health_check_providers(
    state: State<'_, AppState>,
) -> Result<HashMap<String, ConnectionResult>, CommandError> {
    log::info!("手动触发 Provider 健康检查");
    let router = state.llm_router.read().await;
    let results = router.health_check_all().await;
    log::info!("手动健康检查完成, 检查了 {} 个 Provider", results.len());
    Ok(results)
}

/// 强制恢复所有 Provider 为可用状态
/// 用于网络恢复后或用户手动点击"检查连接"时，立即重置所有 Provider 健康状态
#[tauri::command]
pub async fn force_recover_providers(state: State<'_, AppState>) -> Result<(), CommandError> {
    log::info!("强制恢复所有 Provider 为可用状态");
    let router = state.llm_router.read().await;
    router.force_recover_all().await;
    router.rebuild_all_clients().await;
    log::info!("所有 Provider 已恢复为可用状态，HTTP 客户端已重建");
    Ok(())
}

/// 获取当前网络状态
#[tauri::command]
pub async fn get_network_status(state: State<'_, AppState>) -> Result<String, CommandError> {
    let status = state.network_monitor.get_status().await;
    Ok(status.as_str().to_string())
}
