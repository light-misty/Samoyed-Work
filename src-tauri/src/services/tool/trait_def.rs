use async_trait::async_trait;
use serde_json::Value;

use crate::models::tool::ToolResult;

/// Tool trait，所有工具必须实现此接口
/// 工具是轻量级基础操作，始终启用，不可禁用
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称（唯一标识）
    fn tool_name(&self) -> &str;

    /// 工具描述
    fn description(&self) -> &str;

    /// 参数 JSON Schema
    fn parameters(&self) -> Value;

    /// 工具分类
    fn category(&self) -> &str {
        "filesystem"
    }

    /// 执行工具
    async fn execute(&self, params: Value) -> ToolResult;
}
