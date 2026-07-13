//! TodoWrite 数据模型
//! 结构化任务管理,支持 pending/in_progress/completed 三种状态

use serde::{Deserialize, Serialize};

/// Todo 任务状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TodoStatus {
    /// 待处理
    Pending,
    /// 进行中(同一时间只能有一个任务处于此状态)
    InProgress,
    /// 已完成
    Completed,
}

impl TodoStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TodoStatus::Pending => "pending",
            TodoStatus::InProgress => "in_progress",
            TodoStatus::Completed => "completed",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(TodoStatus::Pending),
            "in_progress" => Some(TodoStatus::InProgress),
            "completed" => Some(TodoStatus::Completed),
            _ => None,
        }
    }
}

/// Todo 优先级
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum TodoPriority {
    High,
    #[default]
    Medium,
    Low,
}

impl TodoPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            TodoPriority::High => "high",
            TodoPriority::Medium => "medium",
            TodoPriority::Low => "low",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "high" => Some(TodoPriority::High),
            "medium" => Some(TodoPriority::Medium),
            "low" => Some(TodoPriority::Low),
            _ => None,
        }
    }
}

/// 单个 Todo 任务
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoItem {
    /// 任务唯一 ID(UUID)
    pub id: String,
    /// 任务内容(简短描述)
    pub content: String,
    /// 任务状态
    pub status: TodoStatus,
    /// 优先级
    #[serde(default)]
    pub priority: TodoPriority,
    /// 创建时间(UNIX 时间戳,毫秒)
    pub created_at: u64,
    /// 更新时间(UNIX 时间戳,毫秒)
    pub updated_at: u64,
    /// 完成时间(UNIX 时间戳,毫秒,仅 status=completed 时有值)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<u64>,
}

/// Todo 列表(按 session_id 隔离)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodoList {
    /// 会话 ID
    pub session_id: String,
    /// 任务列表
    pub items: Vec<TodoItem>,
    /// 最后更新时间
    pub updated_at: u64,
}

impl TodoList {
    /// 创建空的 Todo 列表
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            items: Vec::new(),
            updated_at: current_timestamp_ms(),
        }
    }

    /// 获取进行中的任务(应最多一个)
    pub fn get_in_progress(&self) -> Option<&TodoItem> {
        self.items
            .iter()
            .find(|t| t.status == TodoStatus::InProgress)
    }

    /// 获取待处理任务数
    pub fn pending_count(&self) -> usize {
        self.items
            .iter()
            .filter(|t| t.status == TodoStatus::Pending)
            .count()
    }

    /// 获取已完成任务数
    pub fn completed_count(&self) -> usize {
        self.items
            .iter()
            .filter(|t| t.status == TodoStatus::Completed)
            .count()
    }

    /// 生成摘要文本(注入到系统提示词)
    /// 空列表返回 None
    pub fn build_summary(&self) -> Option<String> {
        if self.items.is_empty() {
            return None;
        }

        let mut summary = String::from("\n## Current Task List\n");
        for item in &self.items {
            let status_icon = match item.status {
                TodoStatus::Pending => "[ ]",
                TodoStatus::InProgress => "[>]",
                TodoStatus::Completed => "[x]",
            };
            summary.push_str(&format!(
                "{} {} ({})\n",
                status_icon,
                item.content,
                item.priority.as_str()
            ));
        }

        let total = self.items.len();
        let completed = self.completed_count();
        let pending = self.pending_count();
        summary.push_str(&format!(
            "\nProgress: {}/{} completed, {} pending\n",
            completed, total, pending
        ));

        Some(summary)
    }
}

/// 获取当前时间戳(毫秒)
fn current_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
