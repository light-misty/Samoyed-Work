//! 子 Agent 模型定义
//! 子 Agent 是主 Agent 委托的独立执行单元，拥有独立上下文但继承父 Agent 配置

use serde::{Deserialize, Serialize};

/// 子 Agent 配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentConfig {
    /// 子 Agent 唯一 ID
    pub agent_id: String,
    /// 父 Agent 的会话 ID
    pub parent_session_id: String,
    /// 子任务描述
    pub task_description: String,
    /// 子 Agent 的系统提示词（继承自父 Agent，可追加）
    pub system_prompt: String,
    /// 工作区根目录（继承自父 Agent）
    pub workspace_root: String,
    /// 最大迭代次数（默认 10）
    pub max_iterations: u32,
    /// 超时时间（秒，默认 300）
    pub timeout_seconds: u64,
    /// 是否允许子 Agent 调用 Task 工具（默认 false，防止递归）
    pub allow_nested_task: bool,
    /// 可用工具列表（空表示继承所有工具）
    pub allowed_tools: Vec<String>,
    /// Agent 模式（继承自父 Agent）
    /// 取值为 "plan" / "build" / "document"，子 Agent 必须与父 Agent 模式一致
    /// Document 模式下子 Agent 也能看到文档 Handler（docx/xlsx/pptx/pdf）
    pub agent_mode: String,
    /// 嵌套深度（0 表示主 Agent 直接委托的子 Agent，1 表示子 Agent 委托的孙 Agent，以此类推）
    /// 限制最大深度为 3 层，超过则拒绝执行
    pub nesting_depth: u32,
}

impl Default for SubAgentConfig {
    fn default() -> Self {
        Self {
            agent_id: String::new(),
            parent_session_id: String::new(),
            task_description: String::new(),
            system_prompt: String::new(),
            workspace_root: String::new(),
            max_iterations: 10,
            timeout_seconds: 300,
            allow_nested_task: false,
            allowed_tools: Vec::new(),
            agent_mode: "build".to_string(),
            nesting_depth: 0,
        }
    }
}

/// 子 Agent 工具调用记录（用于持久化和前端恢复）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallRecord {
    /// 工具名称
    pub tool_name: String,
    /// 工具参数（JSON Value）
    pub arguments: serde_json::Value,
}

/// 子 Agent 执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubAgentResult {
    /// 子 Agent ID
    pub agent_id: String,
    /// 是否成功
    pub success: bool,
    /// 执行结果文本
    pub result: String,
    /// 错误信息（失败时）
    pub error: Option<String>,
    /// 执行迭代次数
    pub iterations: u32,
    /// 执行耗时（毫秒）
    pub duration_ms: u64,
    /// 使用的工具调用次数
    pub tool_calls: u32,
    /// 任务描述(从 SubAgentConfig 透传,供 TaskTool 构建 metadata 时使用)
    pub task_description: String,
    /// 工具调用记录列表（完整工具调用历史）
    pub tool_call_records: Vec<ToolCallRecord>,
}

/// 子 Agent 状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SubAgentStatus {
    /// 待执行
    Pending,
    /// 执行中
    Running,
    /// 已完成
    Completed,
    /// 已失败
    Failed,
    /// 已取消
    Cancelled,
}
