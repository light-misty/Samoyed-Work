use std::time::Duration;
use async_trait::async_trait;
use futures::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use tokio::sync::mpsc;

use crate::config::llm_config::AdvancedConfig;
use crate::errors::CommandError;
use crate::models::llm::*;
use super::provider::LlmProvider;

/// OpenAI 兼容 API 适配器
/// 支持 OpenAI、Azure OpenAI、以及所有兼容 OpenAI API 格式的服务
pub struct OpenAiAdapter {
    api_base_url: String,
    api_key: String,
    model: String,
    advanced: AdvancedConfig,
    client: Client,
}

impl OpenAiAdapter {
    pub fn new(
        api_base_url: String,
        api_key: String,
        model: String,
        advanced: AdvancedConfig,
    ) -> Self {
        let timeout = Duration::from_secs(advanced.timeout_seconds as u64);
        let client = Client::builder()
            .timeout(timeout)
            .build()
            .unwrap_or_default();
        Self {
            api_base_url,
            api_key,
            model,
            advanced,
            client,
        }
    }

    /// 构建请求体
    fn build_request_body(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        stream: bool,
    ) -> Value {
        let mut body = json!({
            "model": self.model,
            "messages": messages.iter().map(|m| {
                let mut msg = json!({
                    "role": m.role,
                    "content": m.content,
                });
                if let Some(tool_calls) = &m.tool_calls {
                    msg["tool_calls"] = json!(tool_calls.iter().map(|tc| {
                        json!({
                            "id": tc.id,
                            "type": "function",
                            "function": {
                                "name": tc.name,
                                "arguments": tc.arguments,
                            }
                        })
                    }).collect::<Vec<_>>());
                }
                if let Some(call_id) = &m.tool_call_id {
                    msg["tool_call_id"] = json!(call_id);
                }
                msg
            }).collect::<Vec<_>>(),
            "stream": stream,
        });

        if !tools.is_empty() {
            body["tools"] = json!(tools.iter().map(|t| {
                json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            }).collect::<Vec<_>>());
        }

        body["temperature"] = json!(self.advanced.temperature);
        body["max_tokens"] = json!(self.advanced.max_tokens);
        body["top_p"] = json!(self.advanced.top_p);

        body
    }

    /// 发送请求，带重试逻辑
    async fn send_with_retry(
        &self,
        url: &str,
        body: &Value,
    ) -> Result<reqwest::Response, CommandError> {
        let max_retries = self.advanced.max_retries;
        let mut last_error = None;

        for attempt in 0..=max_retries {
            if attempt > 0 {
                let delay = Duration::from_millis(500 * 2u64.pow(attempt as u32 - 1));
                tokio::time::sleep(delay).await;
            }

            let mut request = self.client.post(url);
            request = request
                .header("Authorization", format!("Bearer {}", self.api_key))
                .header("Content-Type", "application/json");

            // 添加额外请求头
            for (key, value) in &self.advanced.extra_headers {
                request = request.header(key.as_str(), value.as_str());
            }

            match request.json(body).send().await {
                Ok(response) => {
                    let status = response.status();
                    if status.is_success() {
                        return Ok(response);
                    }

                    let error_body = response.text().await.unwrap_or_default();

                    if status.as_u16() == 401 {
                        return Err(CommandError::llm(1001, format!("认证失败: {}", error_body)));
                    }
                    if status.as_u16() == 429 {
                        if attempt < max_retries {
                            last_error = Some(CommandError::llm(1003, "请求频率受限，正在重试".to_string()));
                            continue;
                        }
                        return Err(CommandError::llm(1003, format!("请求频率受限: {}", error_body)));
                    }
                    if status.as_u16() == 404 {
                        return Err(CommandError::llm(1004, format!("模型不存在: {}", error_body)));
                    }

                    last_error = Some(CommandError::llm(1000, format!("API 请求失败 ({}): {}", status, error_body)));
                }
                Err(e) => {
                    if e.is_timeout() {
                        if attempt < max_retries {
                            last_error = Some(CommandError::llm(1005, "请求超时，正在重试".to_string()));
                            continue;
                        }
                        return Err(CommandError::llm(1005, "请求超时".to_string()));
                    }
                    last_error = Some(CommandError::llm(1000, format!("网络错误: {}", e)));
                }
            }
        }

        Err(last_error.unwrap_or_else(|| CommandError::llm(1000, "未知错误".to_string())))
    }

    /// 解析非流式响应
    fn parse_response(&self, value: Value) -> Result<ChatResponse, CommandError> {
        let id = value["id"].as_str().unwrap_or("").to_string();
        let choices = value["choices"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|c| {
                        let index = c["index"].as_u64().unwrap_or(0) as u32;
                        let message = &c["message"];
                        let role = message["role"].as_str().unwrap_or("assistant").to_string();
                        let content = message["content"].as_str().unwrap_or("").to_string();

                        let tool_calls = message["tool_calls"].as_array().map(|tc_arr| {
                            tc_arr.iter()
                                .filter_map(|tc| {
                                    let id = tc["id"].as_str().unwrap_or("").to_string();
                                    let func = &tc["function"];
                                    let name = func["name"].as_str().unwrap_or("").to_string();
                                    let arguments = func["arguments"].as_str().unwrap_or("{}").to_string();
                                    Some(LlmToolCall { id, name, arguments })
                                })
                                .collect::<Vec<_>>()
                        });

                        let finish_reason = c["finish_reason"].as_str().map(String::from);

                        Some(ChatChoice {
                            index,
                            message: ChatMessage {
                                role,
                                content,
                                tool_calls,
                                tool_call_id: None,
                            },
                            finish_reason,
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        let usage = value["usage"].as_object().map(|u| ChatUsage {
            prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0),
            completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0),
            total_tokens: u["total_tokens"].as_u64().unwrap_or(0),
        });

        Ok(ChatResponse {
            id,
            choices,
            usage,
        })
    }
}

#[async_trait]
impl LlmProvider for OpenAiAdapter {
    fn provider_name(&self) -> &str {
        &self.model
    }

    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError> {
        let url = format!("{}/chat/completions", self.api_base_url.trim_end_matches('/'));
        let body = self.build_request_body(messages, tools, false);
        let response = self.send_with_retry(&url, &body).await?;
        let value: Value = response.json().await.map_err(|e| {
            CommandError::llm(1000, format!("解析响应失败: {}", e))
        })?;
        self.parse_response(value)
    }

    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError> {
        let url = format!("{}/chat/completions", self.api_base_url.trim_end_matches('/'));
        let body = self.build_request_body(messages, tools, true);
        let response = self.send_with_retry(&url, &body).await?;

        let (tx, rx) = mpsc::channel(100);

        tokio::spawn(async move {
            let mut stream = response.bytes_stream();
            let mut buffer = String::new();

            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(bytes) => {
                        let text = String::from_utf8_lossy(&bytes);
                        buffer.push_str(&text);

                        // 解析 SSE 事件
                        while let Some(pos) = buffer.find("\n\n") {
                            let event_text = buffer[..pos].to_string();
                            buffer = buffer[pos + 2..].to_string();

                            for line in event_text.lines() {
                                if let Some(data) = line.strip_prefix("data: ") {
                                    let data = data.trim();
                                    if data == "[DONE]" {
                                        let _ = tx.send(Err(CommandError::llm(9999, "stream_done".to_string()))).await;
                                        return;
                                    }

                                    match serde_json::from_str::<Value>(data) {
                                        Ok(value) => {
                                            let id = value["id"].as_str().unwrap_or("").to_string();
                                            let choices = value["choices"]
                                                .as_array()
                                                .map(|arr| {
                                                    arr.iter().filter_map(|c| {
                                                        let index = c["index"].as_u64().unwrap_or(0) as u32;
                                                        let delta = &c["delta"];
                                                        let role = delta["role"].as_str().map(String::from);
                                                        let content = delta["content"].as_str().map(String::from);
                                                        let tool_calls = delta["tool_calls"].as_array().map(|tc_arr| {
                                                            tc_arr.iter().filter_map(|tc| {
                                                                let id = tc["id"].as_str().unwrap_or("").to_string();
                                                                let func = &tc["function"];
                                                                let name = func["name"].as_str().unwrap_or("").to_string();
                                                                let arguments = func["arguments"].as_str().unwrap_or("").to_string();
                                                                Some(LlmToolCall { id, name, arguments })
                                                            }).collect::<Vec<_>>()
                                                        });
                                                        let finish_reason = c["finish_reason"].as_str().map(String::from);

                                                        Some(StreamChoice {
                                                            index,
                                                            delta: StreamDelta {
                                                                role,
                                                                content,
                                                                tool_calls,
                                                            },
                                                            finish_reason,
                                                        })
                                                    }).collect::<Vec<_>>()
                                                }).unwrap_or_default();

                                            let chunk = StreamChunk { id, choices };
                                            if tx.send(Ok(chunk)).await.is_err() {
                                                return;
                                            }
                                        }
                                        Err(e) => {
                                            let _ = tx.send(Err(CommandError::llm(1000, format!("解析 SSE 数据失败: {}", e)))).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(CommandError::llm(1000, format!("流读取错误: {}", e)))).await;
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn test_connection(&self) -> Result<ConnectionResult, CommandError> {
        let start = std::time::Instant::now();
        let test_messages = vec![ChatMessage {
            role: "user".to_string(),
            content: "Hi".to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];
        let url = format!("{}/chat/completions", self.api_base_url.trim_end_matches('/'));
        let body = self.build_request_body(&test_messages, &[], false);

        match self.send_with_retry(&url, &body).await {
            Ok(response) => {
                let latency_ms = start.elapsed().as_millis() as u64;
                let value: Value = response.json().await.unwrap_or_default();
                let model_name = value["model"].as_str().unwrap_or(&self.model).to_string();
                Ok(ConnectionResult {
                    success: true,
                    provider_id: None,
                    latency_ms,
                    model_info: None,
                    model: Some(model_name),
                    error_message: None,
                    error: None,
                })
            }
            Err(e) => Ok(ConnectionResult {
                success: false,
                provider_id: None,
                latency_ms: start.elapsed().as_millis() as u64,
                model_info: None,
                model: None,
                error_message: Some(e.message.clone()),
                error: Some(e.message.clone()),
            }),
        }
    }
}
