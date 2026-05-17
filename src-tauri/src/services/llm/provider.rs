use async_trait::async_trait;
use crate::errors::CommandError;
use crate::models::llm::{ChatMessage, ChatResponse, StreamChunk, ToolDefinition, ConnectionResult};

/// LLM Provider trait，所有 LLM 适配器必须实现此接口
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// 获取 Provider 名称
    fn provider_name(&self) -> &str;

    /// 非流式对话
    async fn chat(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<ChatResponse, CommandError>;

    /// 流式对话，返回 StreamChunk 的接收端
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError>;

    /// 测试连接是否可用
    async fn test_connection(&self) -> Result<ConnectionResult, CommandError>;
}
