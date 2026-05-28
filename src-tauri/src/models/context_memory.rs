use serde::{Deserialize, Serialize};

/// 会话摘要（用于情景记忆/跨会话上下文注入）
/// 在 Agent 执行完成时自动生成，新会话启动时可检索同工作区的历史摘要注入上下文
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ContextSessionSummary {
    pub id: String,
    pub session_id: String,
    pub workspace_id: String,
    /// 用户原始目标
    pub user_goal: String,
    /// Agent 执行结果摘要
    pub result_summary: String,
    /// 涉及的文件列表（JSON 数组字符串）
    pub files_involved: String,
    /// 使用的工具列表（JSON 数组字符串）
    pub tools_used: String,
    /// 遇到的错误及解决方案（JSON 数组字符串）
    pub errors_resolved: String,
    /// 创建时间
    pub created_at: String,
}

impl ContextSessionSummary {
    /// 解析涉及的文件列表
    pub fn get_files_involved(&self) -> Vec<String> {
        serde_json::from_str(&self.files_involved).unwrap_or_default()
    }

    /// 解析使用的工具列表
    pub fn get_tools_used(&self) -> Vec<String> {
        serde_json::from_str(&self.tools_used).unwrap_or_default()
    }

    /// 解析遇到的错误及解决方案
    pub fn get_errors_resolved(&self) -> Vec<String> {
        serde_json::from_str(&self.errors_resolved).unwrap_or_default()
    }
}

/// 用户偏好（用于语义记忆/长期知识积累）
/// 从历史交互中提取并持久化，在后续会话中自动应用
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UserPreference {
    pub id: String,
    /// 偏好类别（format/style/language/naming 等）
    pub category: String,
    /// 偏好键
    pub key: String,
    /// 偏好值
    pub value: String,
    /// 置信度（0.0-1.0，基于观察次数）
    pub confidence: f64,
    /// 观察次数
    pub observation_count: u32,
    /// 最后观察时间
    pub last_observed_at: String,
}
