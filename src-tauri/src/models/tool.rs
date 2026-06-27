use serde::{Deserialize, Serialize};

/// Tool 执行结果（与 HandlerResult 格式一致，便于 AgentExecutor 统一处理）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolResult {
    pub success: bool,
    pub output: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: u64,
    /// 统一错误码（参见 errors.rs），success=true 时为 None
    /// 用于前端精确处理和日志统计，向后兼容（旧反序列化无此字段时为 None）
    #[serde(default)]
    pub error_code: Option<u32>,
}

/// Scratchpad（智能体草稿本）单条笔记
/// 由 agent 自主通过 update_notes 工具写入，用于跨迭代保持上下文
/// 设计参考 Anthropic《Effective Context Engineering for AI Agents》的
/// "Structured Note-taking" 模式，避免外部硬编码迭代元数据注入
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ScratchpadEntry {
    /// 笔记内容（由 agent 自主撰写）
    pub content: String,
    /// 写入时的迭代轮次（用于排序和调试，不暴露给 LLM）
    pub iteration: u32,
    /// 写入时间戳（Unix 毫秒，用于排序和清理）
    pub timestamp_ms: u64,
}

/// 单个会话的 Scratchpad 状态
/// 按 timestamp_ms 升序排列，最新的笔记在末尾
pub type ScratchpadState = Vec<ScratchpadEntry>;

/// Tool 信息（用于前端展示）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: String,
    /// 工具始终为内置
    pub is_builtin: bool,
    /// 工具始终启用
    pub enabled: bool,
    pub version: String,
    pub params_schema: Option<serde_json::Value>,
}
