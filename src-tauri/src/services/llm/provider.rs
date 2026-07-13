use crate::errors::{CommandError, LLM_CONNECTION_REFUSED, LLM_NETWORK_UNREACHABLE, LLM_SSL_ERROR};
use crate::models::llm::{
    ChatMessage, ChatResponse, ConnectionResult, StreamChunk, ToolDefinition,
};
use async_trait::async_trait;

/// 检测 reqwest 错误是否为 DNS 解析失败
/// DNS 失败通常是网络切换后的瞬时问题，应更积极地重试
pub fn is_dns_error(e: &reqwest::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("dns")
        || msg.contains("resolve")
        || msg.contains("name resolution")
        || msg.contains("getaddrinfo")
        || msg.contains("nodename")
}

/// 检测 reqwest 错误是否为连接被拒绝
/// 通常表示 API 地址错误或目标服务未运行
pub fn is_connection_refused_error(e: &reqwest::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("connection refused") || msg.contains("refused")
}

/// 检测 reqwest 错误是否为 SSL/TLS 握手失败
/// 通常由系统时间不正确或证书问题导致
pub fn is_ssl_error(e: &reqwest::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("ssl")
        || msg.contains("tls")
        || msg.contains("certificate")
        || msg.contains("handshake")
}

/// 检测 reqwest 错误是否为网络不可达
/// 通常表示没有可用的网络接口
pub fn is_network_unreachable_error(e: &reqwest::Error) -> bool {
    let msg = e.to_string().to_lowercase();
    msg.contains("network unreachable")
        || msg.contains("unreachable")
        || msg.contains("no route to host")
}

/// 将 reqwest 连接错误细分为精确的错误码
/// 优先级：DNS > 连接被拒绝 > SSL > 网络不可达 > 通用连接失败
pub fn classify_connection_error(e: &reqwest::Error) -> (u32, String) {
    if is_dns_error(e) {
        (
            crate::errors::LLM_DNS_RESOLVE_FAILED,
            format!("DNS解析失败: {}", e),
        )
    } else if is_connection_refused_error(e) {
        (LLM_CONNECTION_REFUSED, format!("AI服务拒绝连接: {}", e))
    } else if is_ssl_error(e) {
        (LLM_SSL_ERROR, format!("安全连接失败: {}", e))
    } else if is_network_unreachable_error(e) {
        (LLM_NETWORK_UNREACHABLE, format!("网络不可达: {}", e))
    } else {
        (
            crate::errors::LLM_CONNECTION_FAILED,
            format!("网络错误: {}", e),
        )
    }
}

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

    /// 流式对话，支持覆盖 max_tokens 参数
    /// 用于响应截断时以更大的 max_tokens 重试，避免因输出限制导致 tool_call 参数不完整
    async fn chat_stream_with_max_tokens(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        max_tokens_override: u32,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamChunk, CommandError>>, CommandError>;

    /// 测试连接是否可用
    async fn test_connection(&self) -> Result<ConnectionResult, CommandError>;

    /// 重建 HTTP 客户端（用于网络切换后清理连接池）
    fn rebuild_client(&mut self);

    /// 轻量级健康检查，仅发送 HTTP HEAD 请求检测 API 端点是否可达
    /// 默认实现回退到 test_connection()，各适配器可覆盖以减少 Token 消耗
    async fn lightweight_health_check(&self) -> Result<ConnectionResult, CommandError> {
        self.test_connection().await
    }

    /// 获取当前 Provider 的 max_tokens 配置
    /// 用于截断重试时计算翻倍后的 max_tokens
    fn get_max_tokens(&self) -> u32;
}
