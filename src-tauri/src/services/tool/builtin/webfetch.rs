//! WebFetch 工具：获取指定 URL 的网页内容并转换为 Markdown
//! 支持 HTML（自动转 Markdown）、JSON（格式化）、纯文本/Markdown
//! 受权限系统控制（PermissionType::WebFetch）

use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Instant;

use crate::errors::{TOOL_EXECUTION_ERROR, TOOL_INVALID_PARAMS};
use crate::models::tool::ToolResult;
use crate::services::tool::trait_def::Tool;
use crate::services::web::fetcher::WebFetcher;

/// WebFetch 工具：获取 URL 内容并转换为 Markdown
/// 每次执行时根据 maxLength 参数创建新的 WebFetcher（builder 模式）
pub struct WebFetchTool;

impl WebFetchTool {
    /// 创建 WebFetchTool 实例
    pub fn new() -> Self {
        Self
    }
}

impl Default for WebFetchTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for WebFetchTool {
    fn tool_name(&self) -> &str {
        "webfetch"
    }

    fn description(&self) -> &str {
        "Fetch the web content of a specified URL and automatically convert it to Markdown format. \
         Supports HTML pages, JSON APIs, and plain text resources. Suitable for reading online docs, \
         API references, technical articles, etc. Intranet addresses and sensitive ports are automatically blocked."
    }

    fn category(&self) -> &str {
        "web"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch (must include http:// or https:// protocol)"
                },
                "maxLength": {
                    "type": "integer",
                    "description": "Maximum number of characters to return (default 100000)",
                    "default": 100000,
                    "minimum": 100,
                    "maximum": 500000
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        // 1. 提取 url（必填）
        let url = match params.get("url").and_then(|v| v.as_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("Missing url parameter".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(TOOL_INVALID_PARAMS),
                };
            }
        };

        // 2. 提取 maxLength（默认 100000）
        let max_length = params
            .get("maxLength")
            .and_then(|v| v.as_u64())
            .unwrap_or(100_000)
            .clamp(100, 500_000) as usize;

        // 3. 使用 with_max_content_length 构建新的 fetcher（builder 模式）
        //    注意：WebFetcher 的 client 内部是 Arc，克隆成本低
        let fetcher = WebFetcher::new().with_max_content_length(max_length);

        // 4. 执行 fetch
        match fetcher.fetch(&url).await {
            Ok(result) => ToolResult {
                success: true,
                output: Some(json!({
                    "url": result.url,
                    "finalUrl": result.final_url,
                    "contentType": result.content_type,
                    "content": result.markdown,
                    "contentLength": result.content_length,
                    "fetchDurationMs": result.fetch_duration_ms,
                })),
                error: None,
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            },
            Err(e) => ToolResult {
                success: false,
                output: Some(json!({
                    "url": url,
                    "error": e,
                })),
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(TOOL_EXECUTION_ERROR),
            },
        }
    }
}
