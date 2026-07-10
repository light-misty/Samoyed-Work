//! SourceCode 工具:基于 tree-sitter 的代码语义搜索
//! 支持 action=search(在目录中搜索符号)和 action=list_symbols(列出单文件符号)
//! workspace_root 由 executor 注入,用于将相对路径解析为绝对路径

use crate::errors::{self, CommandError};
use crate::models::tool::ToolResult;
use crate::services::code::parser::ProgrammingLanguage;
use crate::services::code::search::{SearchQuery, SourceCodeSearcher};
use crate::services::tool::trait_def::Tool;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Mutex;

/// SourceCode 工具:代码语义搜索
/// 基于 tree-sitter 解析代码语法树,支持按符号类型和名称通配符查询
pub struct SourceCodeTool {
    /// 内部持有的搜索器(Mutex 保护,因为 LanguageParser 是有状态的)
    searcher: Mutex<SourceCodeSearcher>,
}

impl SourceCodeTool {
    /// 创建 SourceCode 工具实例
    pub fn new() -> Result<Self, CommandError> {
        Ok(Self {
            searcher: Mutex::new(SourceCodeSearcher::new()?),
        })
    }

    /// 将相对路径解析为绝对路径
    /// - 绝对路径直接返回
    /// - 相对路径与 workspace_root 拼接
    fn resolve_path(path: &str, workspace_root: &str) -> String {
        if path.is_empty() {
            return path.to_string();
        }
        let p = Path::new(path);
        if p.is_absolute() {
            return path.to_string();
        }
        if workspace_root.is_empty() {
            return path.to_string();
        }
        Path::new(workspace_root)
            .join(path)
            .to_string_lossy()
            .to_string()
    }
}

#[async_trait]
impl Tool for SourceCodeTool {
    fn tool_name(&self) -> &str {
        "source_code"
    }

    fn description(&self) -> &str {
        "Code semantic search tool. Based on tree-sitter parsing of code syntax trees, supports \
         querying by symbol type (function/method/class/struct/interface/enum, etc.) and name wildcards. \
         Supports 8 languages: Rust/Python/JavaScript/TypeScript/Go/Java/C/C++."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["search", "list_symbols"],
                    "description": "Action type: search=search symbols in a directory, list_symbols=list all symbols in a single file"
                },
                "path": {
                    "type": "string",
                    "description": "Search directory (search) or file path (list_symbols)"
                },
                "symbolName": {
                    "type": "string",
                    "description": "Symbol name wildcard (e.g., 'get_*'), optional for search"
                },
                "symbolType": {
                    "type": "string",
                    "description": "Symbol type filter (function/method/class/struct/enum/interface/trait/type_alias/macro/variable/constant/module), optional for search"
                },
                "extensions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File extension filter (e.g., ['rs', 'py']), optional for search"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to search subdirectories recursively, default true"
                },
                "maxResults": {
                    "type": "number",
                    "description": "Maximum number of results, default 100"
                }
            },
            "required": ["action", "path"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = std::time::Instant::now();
        let action = params.get("action").and_then(|v| v.as_str()).unwrap_or("");
        let path = params.get("path").and_then(|v| v.as_str()).unwrap_or("");

        if path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing path parameter".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(errors::TOOL_INVALID_PARAMS),
            };
        }

        // 获取 workspace_root(executor 注入)
        let workspace_root = params
            .get("workspace_root")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 构建完整路径(若 path 是相对路径且 workspace_root 存在)
        let full_path = Self::resolve_path(path, workspace_root);

        let result = match action {
            "search" => self.handle_search(&full_path, &params),
            "list_symbols" => self.handle_list_symbols(&full_path),
            _ => Err(CommandError::tool(
                errors::TOOL_INVALID_PARAMS,
                format!("Unknown action: {}", action),
            )),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        match result {
            Ok(output) => ToolResult {
                success: true,
                output: Some(output),
                error: None,
                duration_ms,
                error_code: None,
            },
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(e.to_string()),
                duration_ms,
                error_code: Some(e.code),
            },
        }
    }
}

impl SourceCodeTool {
    /// 处理 search 操作:在目录中搜索符号
    fn handle_search(&self, dir: &str, params: &Value) -> Result<Value, CommandError> {
        let query = SearchQuery {
            directory: dir.to_string(),
            symbol_name: params
                .get("symbolName")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            symbol_type: params
                .get("symbolType")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            extensions: params
                .get("extensions")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                }),
            recursive: params
                .get("recursive")
                .and_then(|v| v.as_bool())
                .unwrap_or(true),
            max_results: params
                .get("maxResults")
                .and_then(|v| v.as_u64())
                .map(|n| n as usize)
                .unwrap_or(100),
        };

        let mut searcher = self
            .searcher
            .lock()
            .map_err(|e| CommandError::runtime(7001, format!("Searcher lock poisoned: {}", e)))?;
        let results = searcher.search(&query)?;

        Ok(json!({
            "results": results,
            "total": results.len(),
            "directory": dir,
        }))
    }

    /// 处理 list_symbols 操作:列出单个文件的所有符号
    fn handle_list_symbols(&self, file_path: &str) -> Result<Value, CommandError> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(CommandError::fs(
                errors::FS_PATH_NOT_FOUND,
                format!("File does not exist: {}", file_path),
            ));
        }

        let language = ProgrammingLanguage::from_path(path);
        if !language.is_supported() {
            return Err(CommandError::tool(
                errors::TOOL_INVALID_PARAMS,
                format!("Unsupported language: {:?}", language),
            ));
        }

        let source = std::fs::read_to_string(path).map_err(|e| {
            CommandError::fs(
                errors::FS_IO_ERROR,
                format!("Failed to read file {}: {}", file_path, e),
            )
        })?;

        let mut searcher = self
            .searcher
            .lock()
            .map_err(|e| CommandError::runtime(7001, format!("Searcher lock poisoned: {}", e)))?;
        let symbols = searcher.parse_symbols(&source, language)?;

        Ok(json!({
            "symbols": symbols,
            "total": symbols.len(),
            "file": file_path,
        }))
    }
}
