use std::collections::HashMap;

use crate::config::llm_config::{LlmConfig, ProviderType};
use crate::errors::CommandError;
use crate::models::llm::*;
use super::provider::LlmProvider;
use super::openai_adapter::OpenAiAdapter;

/// LLM Provider 路由器
/// 管理多个 LLM Provider，支持默认选择和 Fallback 切换
pub struct LlmRouter {
    providers: HashMap<String, Box<dyn LlmProvider>>,
    default_id: Option<String>,
    fallback_order: Vec<String>,
}

impl LlmRouter {
    /// 从配置创建路由器
    pub fn from_config(config: &LlmConfig) -> Self {
        let mut providers: HashMap<String, Box<dyn LlmProvider>> = HashMap::new();
        let mut default_id = None;

        for provider in &config.providers {
            let advanced = provider.advanced.clone();
            let adapter: Box<dyn LlmProvider> = match provider.provider_type {
                ProviderType::OpenAI | ProviderType::Custom => {
                    Box::new(OpenAiAdapter::new(
                        provider.api_base_url.clone(),
                        provider.api_key_encrypted.clone(),
                        provider.model.clone(),
                        advanced,
                    ))
                }
                ProviderType::Anthropic => {
                    // Anthropic 暂时使用 OpenAI 兼容模式
                    Box::new(OpenAiAdapter::new(
                        provider.api_base_url.clone(),
                        provider.api_key_encrypted.clone(),
                        provider.model.clone(),
                        advanced,
                    ))
                }
                ProviderType::Ollama => {
                    // Ollama 兼容 OpenAI API 格式
                    Box::new(OpenAiAdapter::new(
                        provider.api_base_url.clone(),
                        provider.api_key_encrypted.clone(),
                        provider.model.clone(),
                        advanced,
                    ))
                }
            };
            if provider.is_default {
                default_id = Some(provider.id.clone());
            }
            providers.insert(provider.id.clone(), adapter);
        }

        Self {
            providers,
            default_id,
            fallback_order: config.fallback_order.clone(),
        }
    }

    /// 创建空路由器
    pub fn empty() -> Self {
        Self {
            providers: HashMap::new(),
            default_id: None,
            fallback_order: Vec::new(),
        }
    }

    /// 非流式对话，自动选择 Provider
    pub async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        let provider = self.get_default_provider()
            .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?;

        match provider.chat(messages, tools).await {
            Ok(response) => Ok(response),
            Err(e) => {
                // 尝试 Fallback
                for fallback_id in &self.fallback_order {
                    if let Some(fb_provider) = self.providers.get(fallback_id) {
                        if let Ok(response) = fb_provider.chat(messages, tools).await {
                            return Ok(response);
                        }
                    }
                }
                Err(e)
            }
        }
    }

    /// 流式对话，自动选择 Provider
    pub async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        let provider = self.get_default_provider()
            .ok_or_else(|| CommandError::llm(1002, "未配置 LLM Provider".to_string()))?;
        provider.chat_stream(messages, tools).await
    }

    /// 测试指定 Provider 的连接
    pub async fn test_connection(&self, provider_id: &str) -> Result<ConnectionResult, CommandError> {
        let provider = self.providers.get(provider_id)
            .ok_or_else(|| CommandError::llm(1002, format!("Provider 不存在: {}", provider_id)))?;
        let mut result = provider.test_connection().await?;
        result.provider_id = Some(provider_id.to_string());
        Ok(result)
    }

    /// 获取默认 Provider
    fn get_default_provider(&self) -> Option<&dyn LlmProvider> {
        if let Some(id) = &self.default_id {
            self.providers.get(id).map(|p| p.as_ref())
        } else {
            self.providers.values().next().map(|p| p.as_ref())
        }
    }

    /// 列出所有 Provider 信息
    pub fn list_providers(&self) -> Vec<ProviderInfo> {
        self.providers.keys().map(|id| {
            ProviderInfo {
                id: id.clone(),
                name: String::new(),
                provider_type: String::new(),
                api_base: String::new(),
                model: String::new(),
                is_default: self.default_id.as_ref() == Some(id),
                is_available: true,
                created_at: String::new(),
                is_connected: None,
            }
        }).collect()
    }
}
