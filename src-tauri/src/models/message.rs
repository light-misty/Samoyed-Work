use serde::{Deserialize, Serialize};

/// 消息角色枚举
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum MessageRole {
    #[serde(rename = "user")]
    User,
    #[serde(rename = "assistant")]
    Assistant,
    #[serde(rename = "system")]
    System,
    #[serde(rename = "tool")]
    Tool,
}

/// 对话消息
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_content: Option<String>,
    pub created_at: String,
}

/// 工具调用记录
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
    pub result: Option<serde_json::Value>,
}
