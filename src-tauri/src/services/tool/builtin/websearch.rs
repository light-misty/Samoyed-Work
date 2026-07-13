//! WebSearch 工具：执行网络搜索，返回相关结果列表
//! 支持多种搜索后端（MCP/Tavily/SerpAPI），由 WebSearchConfig 配置
//! 受权限系统控制（PermissionType::WebSearch）

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Instant;

use crate::config::app_settings::WebSearchConfig;
use crate::errors::{TOOL_EXECUTION_ERROR, TOOL_INVALID_PARAMS};
use crate::models::tool::ToolResult;
use crate::services::tool::trait_def::Tool;
use crate::services::web::searcher::WebSearcher;

/// WebSearch 工具：执行网络搜索
pub struct WebSearchTool {
    /// Web 搜索器（内部含 reqwest::Client，线程安全）
    searcher: Arc<WebSearcher>,
}

impl WebSearchTool {
    /// 创建 WebSearchTool 实例
    /// config: WebSearchConfig，从 AppSettings 读取
    pub fn new(config: WebSearchConfig) -> Self {
        Self {
            searcher: Arc::new(WebSearcher::new(config)),
        }
    }
}

#[async_trait]
impl Tool for WebSearchTool {
    fn tool_name(&self) -> &str {
        "websearch"
    }

    fn description(&self) -> &str {
        "Perform a web search and return a list of relevant results (title, URL, snippet). \
         Suitable for finding latest information, technical docs, API usage, etc. \
         The search backend is determined by configuration (MCP/Tavily/SerpAPI)."
    }

    fn category(&self) -> &str {
        "web"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query keywords"
                },
                "maxResults": {
                    "type": "integer",
                    "description": "Maximum number of results to return (default 5, max 20)",
                    "default": 5,
                    "minimum": 1,
                    "maximum": 20
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        // 1. 提取 query（必填）
        let query = match params.get("query").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("Missing query parameter".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(TOOL_INVALID_PARAMS),
                };
            }
        };

        // 2. 执行搜索（maxResults 由 WebSearcher 内部 config 控制，此处不覆盖）
        match self.searcher.search(&query).await {
            Ok(response) => {
                // 序列化结果列表
                let results: Vec<Value> = response
                    .results
                    .iter()
                    .map(|item| {
                        json!({
                            "title": item.title,
                            "url": item.url,
                            "snippet": item.snippet,
                        })
                    })
                    .collect();

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "query": response.query,
                        "engine": response.engine,
                        "results": results,
                        "totalResults": response.total_results,
                        "searchDurationMs": response.search_duration_ms,
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => ToolResult {
                success: false,
                output: Some(json!({
                    "query": query,
                    "error": e,
                })),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(TOOL_EXECUTION_ERROR),
            },
        }
    }
}
