// 允许在测试模块之后定义工具：项目原有结构将测试模块置于文件中部，
// WriteTextFileTool 及阶段三 3.5 新增的 5 个工具均位于测试模块之后。
// 完整重构文件结构（移动测试模块到末尾）超出当前任务范围，这里以 allow 抑制 lint。
#![allow(clippy::items_after_test_module)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::models::tool::{ScratchpadEntry, ScratchpadState, ToolResult};
use super::trait_def::Tool;
use super::registry::ToolRegistry;

/// Scratchpad 共享状态类型
/// 全局唯一实例，按 session_id 隔离不同会话的笔记
/// 由 ScratchpadTool 持有写权限，AgentContext 持有读权限（用于每轮刷新摘要）
pub type SharedScratchpadStates = Arc<RwLock<HashMap<String, ScratchpadState>>>;

/// 将相对路径解析为绝对路径
fn resolve_path(path: &str, workspace_root: &str) -> String {
    if path.is_empty() {
        return path.to_string();
    }
    let p = std::path::Path::new(path);
    if p.is_absolute() {
        return path.to_string();
    }
    let root = std::path::Path::new(workspace_root);
    root.join(path).to_string_lossy().to_string()
}

/// 注册所有内置工具
/// 返回 Scratchpad 共享状态 Arc，供 AgentContext 在每轮迭代时读取笔记摘要
pub fn register_builtin_tools(registry: &mut ToolRegistry) -> SharedScratchpadStates {
    log::info!("开始注册内置工具");
    registry.register(Box::new(ListDirectoryTool));
    registry.register(Box::new(SearchFilesTool));
    registry.register(Box::new(ReadFileTool));
    registry.register(Box::new(FileInfoTool));
    registry.register(Box::new(FileExistsTool));
    registry.register(Box::new(DeleteFileTool));
    registry.register(Box::new(CreateDirectoryTool));
    registry.register(Box::new(WriteTextFileTool));
    // 阶段三 3.5 新增的 5 个基础文件系统工具
    registry.register(Box::new(RenameFileTool));
    registry.register(Box::new(CopyFileTool));
    registry.register(Box::new(DeleteDirectoryTool));
    registry.register(Box::new(GetFileHashTool));
    registry.register(Box::new(ReadFileLinesTool));

    // Scratchpad 工具：智能体草稿本，由 agent 自主调用 update_notes 写入
    // 设计参考 Anthropic《Effective Context Engineering for AI Agents》的
    // "Structured Note-taking" 模式，替代外部硬编码迭代元数据注入
    let scratchpad_states: SharedScratchpadStates = Arc::new(RwLock::new(HashMap::new()));
    registry.register(Box::new(ScratchpadTool {
        states: scratchpad_states.clone(),
    }));

    log::info!("内置工具注册完成, 共注册 14 个工具");
    scratchpad_states
}

// ============================================================
// list_directory - 列出目录内容
// ============================================================

struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn tool_name(&self) -> &str { "list_directory" }
    fn description(&self) -> &str { "列出指定目录中的文件和子目录结构。使用场景：浏览工作区内容、查找文件位置、了解目录层级。支持深度控制和扩展名过滤。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "目录路径，默认为当前工作目录"
                },
                "depth": {
                    "type": "integer",
                    "description": "遍历深度，默认1",
                    "default": 1
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "筛选文件扩展名，如 [\"docx\", \"pdf\"]"
                }
            }
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or(".");
        let max_depth = params["depth"].as_u64().unwrap_or(1) as u32;
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 入参校验：depth 必须 >= 1，否则会导致递归条件 u32 下溢（0-1=4294967295）无限递归
        if max_depth == 0 {
            return ToolResult {
                success: false,
                output: None,
                error: Some("depth 参数必须大于等于 1".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let resolved_dir = resolve_path(dir_path, workspace_root);
        let dir = std::path::Path::new(&resolved_dir);
        if !dir.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        if !dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 路径安全校验
        if !workspace_root.is_empty() {
            let canonical_dir = match crate::utils::canonicalize(dir) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("目录路径无效: {}", dir_path)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let resolved_dir_owned = resolved_dir.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&resolved_dir_owned);
            tool_list_dir(dir, dir, max_depth, 0, &extensions_clone)
        }).await {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("list_directory spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("目录列出任务执行失败: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        log::info!("列出目录完成: {}, 结果数: {}", dir_path, results.len());
        ToolResult {
            success: true,
            output: Some(json!({
                "path": dir_path,
                "items": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        }
    }
}

/// 递归列出目录内容
fn tool_list_dir(
    dir: &std::path::Path,
    root: &std::path::Path,
    max_depth: u32,
    current_depth: u32,
    extensions: &[String],
) -> Vec<Value> {
    let mut nodes = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return nodes,
    };

    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by(|a, b| {
        let a_is_dir = a.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let b_is_dir = b.file_type().map(|t| t.is_dir()).unwrap_or(false);
        b_is_dir.cmp(&a_is_dir).then(
            a.file_name()
                .to_string_lossy()
                .to_lowercase()
                .cmp(&b.file_name().to_string_lossy().to_lowercase()),
        )
    });

    for entry in entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let is_dir = metadata.is_dir();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if !is_dir && !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext) {
            continue;
        }

        let mut node = json!({
            "name": name,
            "path": relative,
            "is_dir": is_dir,
        });

        if !is_dir {
            node["size"] = json!(metadata.len());
            if !ext.is_empty() {
                node["extension"] = json!(ext);
            }
        }

        // 递归条件使用加法避免 u32 下溢：max_depth=0 时 current_depth+1 < max_depth 为 false
        // 与原条件 current_depth < max_depth - 1 在 max_depth >= 1 时等价
        if is_dir && current_depth + 1 < max_depth {
            let children = tool_list_dir(&path, root, max_depth, current_depth + 1, extensions);
            node["children"] = json!(children);
        }

        nodes.push(node);
    }

    nodes
}

// ============================================================
// search_files - 搜索文件
// ============================================================

struct SearchFilesTool;

#[async_trait]
impl Tool for SearchFilesTool {
    fn tool_name(&self) -> &str { "search_files" }
    fn description(&self) -> &str { "在指定目录中搜索文件，支持按文件名或内容搜索。使用场景：按名称查找文件、按内容关键词搜索、按扩展名筛选。设置include_content=true可搜索文件内容。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "搜索关键词（可选，仅按扩展名过滤时可省略）"
                },
                "directory": {
                    "type": "string",
                    "description": "搜索的目录路径，默认为工作区根目录"
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "限定文件扩展名，如 [\"docx\", \"pdf\"]"
                },
                "include_content": {
                    "type": "boolean",
                    "description": "是否搜索文件内容（仅对文本文件有效）",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "description": "最大结果数",
                    "default": 50
                }
            },
            "required": []
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let query = params["query"].as_str().unwrap_or("");
        let directory = params["directory"].as_str().unwrap_or(".");
        let max_results = params["max_results"].as_u64().unwrap_or(50) as usize;
        let include_content = params["include_content"].as_bool().unwrap_or(false);
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        if query.is_empty() && extensions.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("搜索关键词和文件扩展名不能同时为空，请至少提供一项".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let resolved_directory = resolve_path(directory, workspace_root);
        let dir_path = std::path::Path::new(&resolved_directory);
        if !dir_path.exists() || !dir_path.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录不存在或不是目录: {}", directory)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        if !workspace_root.is_empty() {
            let canonical_dir = match crate::utils::canonicalize(dir_path) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("目录路径无效: {}", directory)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("搜索目录不在工作区内，拒绝访问".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let query_lower = query.to_lowercase();
        let resolved_directory_owned = resolved_directory.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir_path = std::path::Path::new(&resolved_directory_owned);
            let mut results = Vec::new();
            tool_search_files(dir_path, dir_path, &query_lower, &extensions_clone, include_content, max_results, &mut results);
            results
        }).await {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("search_files spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("文件搜索任务执行失败: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        log::info!("文件搜索完成: query={}, directory={}, 结果数: {}", query, directory, results.len());
        ToolResult {
            success: true,
            output: Some(json!({
                "query": query,
                "directory": directory,
                "total": results.len(),
                "results": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        }
    }
}

/// 递归搜索文件
fn tool_search_files(
    dir: &std::path::Path,
    root: &std::path::Path,
    query: &str,
    extensions: &[String],
    include_content: bool,
    max_results: usize,
    results: &mut Vec<Value>,
) {
    if results.len() >= max_results {
        return;
    }

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.filter_map(|e| e.ok()) {
        if results.len() >= max_results {
            return;
        }

        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }

        let path = entry.path();

        if path.is_dir() {
            tool_search_files(&path, root, query, extensions, include_content, max_results, results);
            continue;
        }

        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext) {
            continue;
        }

        let name_lower = name.to_lowercase();
        let mut name_matched = query.is_empty() || name_lower.contains(query);
        let mut content_preview = None;

        if include_content && !name_matched && !query.is_empty() {
            let text_extensions = ["txt", "md", "markdown", "csv", "json", "xml", "html", "css", "js", "ts", "py", "rs", "toml", "yaml", "yml"];
            if text_extensions.contains(&ext.as_str()) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if content.to_lowercase().contains(query) {
                        name_matched = true;
                        if let Some(pos) = content.to_lowercase().find(query) {
                            // 修复：直接按字节切片可能切到非 UTF-8 字符边界导致 panic
                            // 使用 is_char_boundary 调整 start/end 到字符边界
                            let raw_start = pos.saturating_sub(30);
                            let raw_end = (pos + query.len() + 30).min(content.len());

                            // 调整 start 到字符边界（向后移动直到遇到边界）
                            let mut start = raw_start;
                            while start < raw_end && !content.is_char_boundary(start) {
                                start += 1;
                            }

                            // 调整 end 到字符边界（向前移动直到遇到边界）
                            let mut end = raw_end;
                            while end > start && !content.is_char_boundary(end) {
                                end -= 1;
                            }

                            // 仅在有效区间内生成预览，避免空切片
                            if start < end {
                                content_preview = Some(format!("...{}...", &content[start..end]));
                            }
                        }
                    }
                }
            }
        }

        if !name_matched {
            continue;
        }

        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };

        let match_type = if content_preview.is_some() {
            "content"
        } else if !query.is_empty() {
            "name"
        } else {
            "extension"
        };

        let mut result = json!({
            "path": relative,
            "name": name,
            "extension": ext,
            "size": metadata.len(),
            "match_type": match_type,
        });

        if let Some(preview) = content_preview {
            result["match_preview"] = json!(preview);
        }

        results.push(result);
    }
}

// ============================================================
// read_file - 读取纯文本文件
// ============================================================

struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn tool_name(&self) -> &str { "read_file" }
    fn description(&self) -> &str { "读取纯文本文件内容（.txt/.md/.csv/.json/.xml等），不依赖Sidecar，速度更快。注意：仅适用于纯文本文件，读取Word/Excel/PPT/PDF等结构化文档请使用docx_handler/xlsx_handler/pptx_handler/pdf_handler的read操作。文件大小限制1MB。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认utf-8",
                    "default": "utf-8"
                },
                "max_size": {
                    "type": "integer",
                    "description": "最大读取字节数，默认1MB",
                    "default": 1048576
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let max_size = params["max_size"].as_u64().unwrap_or(1048576) as usize;
        // 读取 encoding 参数（默认 utf-8），支持 GBK/GB2312/Big5/Shift_JIS/Latin1 等
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            log::warn!("read_file 失败: 缺少文件路径");
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        if !workspace_root.is_empty() {
            let (canonical_file, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok(result) => result,
                Err(e) => {
                    // 根据错误消息区分错误码：路径越界 vs 路径不存在
                    let is_out_of_bounds = e.contains("路径不在工作区内");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!("read_file 失败: {}, path={}, workspace={}", e, file_path, workspace_root);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64, error_code,
                    };
                }
            };
            // 校验通过后，使用 canonical 路径继续读取
            let _ = canonical_file; // 已通过校验，path 变量继续使用（下方会重新 canonicalize 或直接读取）
        }

        if !path.exists() {
            log::warn!("read_file 失败: 文件不存在, path={}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        if !path.is_file() {
            log::warn!("read_file 失败: 路径不是文件, path={}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 检查文件大小
        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                log::warn!("read_file 失败: 获取文件信息失败, path={}, 错误: {}", file_path, e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("获取文件信息失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        if metadata.len() as usize > max_size {
            log::warn!("read_file 失败: 文件过大, path={}, size={}字节, max={}字节", file_path, metadata.len(), max_size);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件过大 ({}字节)，超过最大读取限制 ({}字节)", metadata.len(), max_size)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 读取文件字节，根据 encoding 参数解码
        // 支持 UTF-8/GBK/GB2312/Big5/Shift_JIS/Latin1 等多种编码
        match tokio::fs::read(&resolved_path).await {
            Ok(bytes) => {
                // 根据 encoding 标签解析编码器
                let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())
                    .unwrap_or(encoding_rs::UTF_8);
                // 解码字节为字符串（encoding_rs 自动处理 BOM 和无效字节）
                let (content, _actual_encoding, _had_errors) = encoding.decode(&bytes);
                let content = content.into_owned();

                let ext = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "content": content,
                        "size": metadata.len(),
                        "extension": ext,
                        "encoding": encoding.name(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("读取文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("读取文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_absolute() {
        let result = resolve_path("/absolute/path/file.txt", "/workspace");
        assert_eq!(result, "/absolute/path/file.txt");
    }

    #[test]
    fn test_resolve_path_relative() {
        let result = resolve_path("relative/path/file.txt", "/workspace");
        let expected = std::path::Path::new("/workspace")
            .join("relative/path/file.txt")
            .to_string_lossy()
            .to_string();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_resolve_path_empty() {
        let result = resolve_path("", "/workspace");
        assert_eq!(result, "");
    }

    #[test]
    fn test_register_builtin_tools() {
        let mut registry = ToolRegistry::new();
        let _scratchpad_states = register_builtin_tools(&mut registry);

        // 验证 14 个工具都已注册（8 个原有 + 5 个阶段三新增 + 1 个 scratchpad）
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 14);

        // 验证每个工具的基本属性
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"list_directory"));
        assert!(tool_names.contains(&"search_files"));
        assert!(tool_names.contains(&"read_file"));
        assert!(tool_names.contains(&"file_info"));
        assert!(tool_names.contains(&"file_exists"));
        assert!(tool_names.contains(&"delete_file"));
        assert!(tool_names.contains(&"create_directory"));
        assert!(tool_names.contains(&"write_text_file"));
        // 阶段三 3.5 新增工具
        assert!(tool_names.contains(&"rename_file"));
        assert!(tool_names.contains(&"copy_file"));
        assert!(tool_names.contains(&"delete_directory"));
        assert!(tool_names.contains(&"get_file_hash"));
        assert!(tool_names.contains(&"read_file_lines"));
        // Scratchpad 工具
        assert!(tool_names.contains(&"update_notes"));
    }

    #[test]
    fn test_tool_definitions_count() {
        let mut registry = ToolRegistry::new();
        let _scratchpad_states = register_builtin_tools(&mut registry);

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 14);

        // 验证每个定义都有 type 和 function 字段
        for def in &defs {
            assert_eq!(def["type"], "function");
            assert!(def["function"]["name"].is_string());
            assert!(def["function"]["description"].is_string());
            assert!(def["function"]["parameters"].is_object());
        }
    }

    #[test]
    fn test_tool_info_properties() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tools = registry.list_tools();
        for tool in &tools {
            assert!(tool.is_builtin);
            assert!(tool.enabled);
            assert_eq!(tool.version, "1.0.0");
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            // 文件系统工具为 "filesystem"，Scratchpad 笔记工具为 "memory"
            assert!(tool.category == "filesystem" || tool.category == "memory");
        }
    }

    #[tokio::test]
    async fn test_file_exists_nonexistent() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("file_exists").unwrap();
        let result = tool.execute(json!({
            "path": "/nonexistent/path/file.txt",
            "workspace_root": ""
        })).await;

        assert!(result.success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert_eq!(output["exists"], false);
    }

    #[tokio::test]
    async fn test_read_file_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("read_file").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    #[tokio::test]
    async fn test_create_directory_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("create_directory").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少目录路径"));
    }

    #[tokio::test]
    async fn test_write_text_file_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("write_text_file").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "content": "test",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    #[tokio::test]
    async fn test_delete_file_missing_workspace() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("delete_file").unwrap();
        let result = tool.execute(json!({
            "path": "test.txt",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少工作区根目录路径"));
    }

    #[tokio::test]
    async fn test_search_files_empty_query_and_extensions() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("search_files").unwrap();
        let result = tool.execute(json!({
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("不能同时为空"));
    }

    #[tokio::test]
    async fn test_file_info_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("file_info").unwrap();
        let result = tool.execute(json!({
            "path": "",
            "workspace_root": ""
        })).await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("缺少文件路径"));
    }

    /// 测试 encoding 参数：使用 GBK 编码写入中文内容，再用 GBK 编码读取
    /// 验证 encoding_rs 集成是否正确工作
    #[tokio::test]
    async fn test_write_and_read_file_with_gbk_encoding() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_encoding_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "你好，世界！这是 GBK 编码测试。";
        let file_path = "gbk_test.txt";

        // 使用 GBK 编码写入文件
        let write_tool = registry.get_arc("write_text_file").unwrap();
        let write_result = write_tool.execute(json!({
            "path": file_path,
            "content": test_content,
            "workspace_root": temp_dir.to_string_lossy(),
            "encoding": "gbk"
        })).await;

        assert!(write_result.success, "GBK 编码写入失败: {:?}", write_result.error);
        let output = write_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(output["encoding"], "GBK");

        // 使用 GBK 编码读取文件
        let read_tool = registry.get_arc("read_file").unwrap();
        let read_result = read_tool.execute(json!({
            "path": file_path,
            "workspace_root": temp_dir.to_string_lossy(),
            "encoding": "gbk"
        })).await;

        assert!(read_result.success, "GBK 编码读取失败: {:?}", read_result.error);
        let read_output = read_result.output.unwrap();
        assert_eq!(read_output["encoding"], "GBK");
        assert_eq!(read_output["content"].as_str().unwrap(), test_content);

        // 清理临时文件
        let abs_path = temp_dir.join(file_path);
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：UTF-8 默认编码应保持向后兼容
    #[tokio::test]
    async fn test_read_file_default_utf8_encoding() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_utf8_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Hello, 世界！UTF-8 默认编码测试。";
        let file_path = "utf8_test.txt";
        let abs_path = temp_dir.join(file_path);

        // 直接用 UTF-8 写入文件（模拟已存在的 UTF-8 文件）
        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 不传 encoding 参数读取（应默认 UTF-8）
        let read_tool = registry.get_arc("read_file").unwrap();
        let read_result = read_tool.execute(json!({
            "path": file_path,
            "workspace_root": temp_dir.to_string_lossy()
        })).await;

        assert!(read_result.success, "UTF-8 默认读取失败: {:?}", read_result.error);
        let read_output = read_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(read_output["encoding"], "UTF-8");
        assert_eq!(read_output["content"].as_str().unwrap(), test_content);

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：不支持的编码标签应回退到 UTF-8
    #[tokio::test]
    async fn test_read_file_unsupported_encoding_fallback() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(&mut registry);

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("docagent_fallback_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Fallback test 你好";
        let file_path = "fallback_test.txt";
        let abs_path = temp_dir.join(file_path);

        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 传入不支持的编码标签
        let read_tool = registry.get_arc("read_file").unwrap();
        let read_result = read_tool.execute(json!({
            "path": file_path,
            "workspace_root": temp_dir.to_string_lossy(),
            "encoding": "nonexistent-encoding"
        })).await;

        assert!(read_result.success, "不支持的编码应回退到 UTF-8，但读取失败: {:?}", read_result.error);
        let read_output = read_result.output.unwrap();
        // 不支持的编码回退到 UTF-8（encoding_rs 返回大写名称）
        assert_eq!(read_output["encoding"], "UTF-8");
        assert_eq!(read_output["content"].as_str().unwrap(), test_content);

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 Scratchpad 工具的 add 操作
    #[tokio::test]
    async fn test_scratchpad_add_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);

        let tool = registry.get_arc("update_notes").unwrap();

        // 第一条笔记
        let result = tool.execute(json!({
            "action": "add",
            "content": "已读取 sample.docx，包含 3 个章节",
            "_session_id": "test-session-1",
            "_iteration": 1
        })).await;

        assert!(result.success, "add 失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "add");
        assert_eq!(output["total_notes"], 1);

        // 第二条笔记
        let result2 = tool.execute(json!({
            "action": "add",
            "content": "识别到需要修改第 2 章的日期",
            "_session_id": "test-session-1",
            "_iteration": 2
        })).await;

        assert!(result2.success);
        assert_eq!(result2.output.unwrap()["total_notes"], 2);
    }

    /// 测试 Scratchpad 工具的 read 操作
    #[tokio::test]
    async fn test_scratchpad_read_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        // 先添加两条笔记
        tool.execute(json!({
            "action": "add",
            "content": "笔记 A",
            "_session_id": "test-session-read"
        })).await;
        tool.execute(json!({
            "action": "add",
            "content": "笔记 B",
            "_session_id": "test-session-read"
        })).await;

        // 读取笔记
        let result = tool.execute(json!({
            "action": "read",
            "_session_id": "test-session-read"
        })).await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "read");
        assert_eq!(output["total_notes"], 2);
        let notes = output["notes"].as_array().unwrap();
        assert_eq!(notes.len(), 2);
        assert_eq!(notes[0], "笔记 A");
        assert_eq!(notes[1], "笔记 B");
    }

    /// 测试 Scratchpad 工具的 clear 操作
    #[tokio::test]
    async fn test_scratchpad_clear_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        // 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "待清理的笔记",
            "_session_id": "test-session-clear"
        })).await;

        // 清空
        let result = tool.execute(json!({
            "action": "clear",
            "_session_id": "test-session-clear"
        })).await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "clear");
        assert_eq!(output["cleared_notes"], 1);

        // 验证已清空
        let read_result = tool.execute(json!({
            "action": "read",
            "_session_id": "test-session-clear"
        })).await;
        assert_eq!(read_result.output.unwrap()["total_notes"], 0);
    }

    /// 测试 Scratchpad 工具的会话隔离
    #[tokio::test]
    async fn test_scratchpad_session_isolation() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        // session-A 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "会话 A 的笔记",
            "_session_id": "session-A"
        })).await;

        // session-B 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "会话 B 的笔记 1",
            "_session_id": "session-B"
        })).await;
        tool.execute(json!({
            "action": "add",
            "content": "会话 B 的笔记 2",
            "_session_id": "session-B"
        })).await;

        // 验证 session-A 只有 1 条
        let result_a = tool.execute(json!({
            "action": "read",
            "_session_id": "session-A"
        })).await;
        assert_eq!(result_a.output.unwrap()["total_notes"], 1);

        // 验证 session-B 有 2 条
        let result_b = tool.execute(json!({
            "action": "read",
            "_session_id": "session-B"
        })).await;
        assert_eq!(result_b.output.unwrap()["total_notes"], 2);
    }

    /// 测试 Scratchpad 缺少 _session_id 时返回错误
    #[tokio::test]
    async fn test_scratchpad_missing_session_id() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        let result = tool.execute(json!({
            "action": "add",
            "content": "测试笔记"
        })).await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("缺少会话标识"));
        assert_eq!(result.error_code, Some(crate::errors::TOOL_INVALID_PARAMS));
    }

    /// 测试 Scratchpad add 时 content 为空返回错误
    #[tokio::test]
    async fn test_scratchpad_add_empty_content() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        let result = tool.execute(json!({
            "action": "add",
            "content": "",
            "_session_id": "test-session"
        })).await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("content 不能为空"));
    }

    /// 测试 Scratchpad 未知 action 返回错误
    #[tokio::test]
    async fn test_scratchpad_unknown_action() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        let result = tool.execute(json!({
            "action": "delete",
            "_session_id": "test-session"
        })).await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("未知 action"));
    }

    /// 测试 Scratchpad 笔记长度限制（500 字符）
    #[tokio::test]
    async fn test_scratchpad_content_length_limit() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(&mut registry);
        let tool = registry.get_arc("update_notes").unwrap();

        // 构造 1000 字符的长内容
        let long_content = "a".repeat(1000);

        let result = tool.execute(json!({
            "action": "add",
            "content": long_content,
            "_session_id": "test-session-limit"
        })).await;

        assert!(result.success);

        // 验证存储的内容被截断到 500 字符
        let read_result = tool.execute(json!({
            "action": "read",
            "_session_id": "test-session-limit"
        })).await;
        let binding = read_result.output.unwrap();
        let notes = binding["notes"].as_array().unwrap();
        assert_eq!(notes[0].as_str().unwrap().len(), 500);
    }

    /// 测试 format_scratchpad_summary 函数
    #[test]
    fn test_format_scratchpad_summary() {
        use std::time::SystemTime;

        let states: SharedScratchpadStates = Arc::new(RwLock::new(HashMap::new()));

        // 空状态返回 None
        assert!(format_scratchpad_summary(&states, "empty-session").is_none());

        // 添加笔记
        {
            let mut states_write = states.write().unwrap();
            states_write.insert("test-session".to_string(), vec![
                ScratchpadEntry {
                    content: "第一条笔记".to_string(),
                    iteration: 1,
                    timestamp_ms: SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                },
                ScratchpadEntry {
                    content: "第二条笔记".to_string(),
                    iteration: 2,
                    timestamp_ms: SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_millis() as u64,
                },
            ]);
        }

        let summary = format_scratchpad_summary(&states, "test-session");
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert!(summary.contains("<scratchpad>"));
        assert!(summary.contains("第一条笔记"));
        assert!(summary.contains("第二条笔记"));
        assert!(summary.contains("1. 第一条笔记"));
        assert!(summary.contains("2. 第二条笔记"));
        assert!(summary.contains("update_notes"));
    }
}

// ============================================================
// file_info - 获取文件元数据
// ============================================================

struct FileInfoTool;

#[async_trait]
impl Tool for FileInfoTool {
    fn tool_name(&self) -> &str { "file_info" }
    fn description(&self) -> &str { "获取文件元数据（大小、修改时间、类型等）。使用场景：在读取文件前了解文件信息、检查文件类型、确认文件是否存在且可访问。不需要读取文件内容时优先使用此工具。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!("file_info 路径校验失败: {}, path={}, workspace={}", e, file_path, workspace_root);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code,
                };
            }
        }

        if !path.exists() {
            log::error!("文件不存在: {}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("文件不存在: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                log::error!("获取文件信息失败: {}, 错误: {}", file_path, e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("获取文件信息失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        };

        let is_dir = metadata.is_dir();
        let ext = path.extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let modified = metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let file_type = if is_dir {
            "directory"
        } else {
            match ext.as_str() {
                "docx" | "doc" => "word",
                "xlsx" | "xls" => "excel",
                "pptx" | "ppt" => "powerpoint",
                "pdf" => "pdf",
                "md" | "markdown" => "markdown",
                "txt" => "text",
                "csv" => "csv",
                "json" => "json",
                "xml" => "xml",
                "html" | "htm" => "html",
                _ => "file",
            }
        };

        ToolResult {
            success: true,
            output: Some(json!({
                "path": file_path,
                "name": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                "is_dir": is_dir,
                "size": metadata.len(),
                "extension": ext,
                "file_type": file_type,
                "modified": modified,
                "read_only": metadata.permissions().readonly(),
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        }
    }
}

// ============================================================
// file_exists - 检查文件或目录是否存在
// ============================================================

struct FileExistsTool;

#[async_trait]
impl Tool for FileExistsTool {
    fn tool_name(&self) -> &str { "file_exists" }
    fn description(&self) -> &str { "检查文件或目录是否存在。使用场景：在读取或修改文件前验证路径、避免对不存在的文件执行操作。比list_directory更轻量。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件或目录路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        // 注意：file_exists 即使路径不存在也必须先校验越界，否则攻击者可探测工作区外文件
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                // 路径不存在时 validate 会返回"路径不存在或无效"，但需要先检查是否越界
                // validate 内部已先做词法归一化，越界会返回"路径不在工作区内"
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                // 路径不存在但未越界时，返回 exists=false 而非错误
                if !is_out_of_bounds {
                    return ToolResult {
                        success: true,
                        output: Some(json!({
                            "path": file_path,
                            "exists": false,
                            "is_dir": false,
                            "is_file": false,
                        })),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
                log::warn!("file_exists 路径越界: {}, path={}, workspace={}", e, file_path, workspace_root);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code,
                };
            }
        }

        let exists = path.exists();
        let is_dir = exists && path.is_dir();
        let is_file = exists && path.is_file();

        ToolResult {
            success: true,
            output: Some(json!({
                "path": file_path,
                "exists": exists,
                "is_dir": is_dir,
                "is_file": is_file,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
        }
    }
}

// ============================================================
// delete_file - 删除文件
// ============================================================

struct DeleteFileTool;

#[async_trait]
impl Tool for DeleteFileTool {
    fn tool_name(&self) -> &str { "delete_file" }
    fn description(&self) -> &str { "删除指定文件，删除前可选创建备份。注意：此操作不可逆，会自动触发用户确认。建议在删除前先创建版本快照。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的文件路径（相对于工作区）"
                },
                "create_backup": {
                    "type": "boolean",
                    "description": "删除前是否创建备份文件",
                    "default": true
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        if workspace_root.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少工作区根目录路径，无法进行安全校验".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        let canonical_file = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok((canonical_file, _)) => canonical_file,
            Err(e) => {
                let is_out_of_bounds = e.contains("路径不在工作区内");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!("delete_file 路径校验失败: {}, path={}, workspace={}", e, file_path, workspace_root);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code,
                };
            }
        };

        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let safe_path = canonical_file.to_string_lossy().to_string();
        let create_backup = params["create_backup"].as_bool().unwrap_or(true);
        let mut backup_path_str = String::new();

        if create_backup {
            let backup_path = format!("{}.bak", safe_path);
            match tokio::fs::copy(&safe_path, &backup_path).await {
                Ok(_) => {
                    log::info!("删除前已创建备份: {}", backup_path);
                    backup_path_str = backup_path;
                }
                Err(e) => {
                    // 备份失败时拒绝删除，避免数据丢失
                    // 用户可显式设置 create_backup=false 跳过备份后再删除
                    log::error!("创建备份失败: {}, 拒绝删除操作", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "创建备份失败: {}。如需跳过备份强制删除，请设置 create_backup=false 后重试",
                            e
                        )),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        match tokio::fs::remove_file(&safe_path).await {
            Ok(_) => {
                log::info!("文件已删除: {}", safe_path);
                let mut result = json!({
                    "path": file_path,
                    "message": format!("文件已删除: {}", file_path),
                });
                if !backup_path_str.is_empty() {
                    result["backup_path"] = json!(backup_path_str);
                }
                ToolResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除文件失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// create_directory - 创建目录
// ============================================================

struct CreateDirectoryTool;

#[async_trait]
impl Tool for CreateDirectoryTool {
    fn tool_name(&self) -> &str { "create_directory" }
    fn description(&self) -> &str { "创建目录（支持递归创建）。使用场景：在写入文件前确保目标目录存在、组织文件结构。默认递归创建父目录。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "目录路径（相对于工作区）"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "是否递归创建父目录",
                    "default": true
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let recursive = params["recursive"].as_bool().unwrap_or(true);

        if dir_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目录路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(dir_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验：目标路径必须在工作区内
        if !workspace_root.is_empty() {
            // 对于尚不存在的路径，检查其父目录是否在工作区内
            let check_path = if path.exists() {
                match crate::utils::canonicalize(path) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("路径无效: {}", dir_path)),
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                }
            } else {
                // 路径不存在，检查父目录
                match path.parent() {
                    Some(parent) if parent.exists() => {
                        match crate::utils::canonicalize(parent) {
                            Ok(p) => p,
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("父目录路径无效: {}", dir_path)),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
                    _ => {
                        // 如果父目录也不存在且 recursive=true，继续尝试
                        // 但仍需校验工作区根目录
                        match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                            Ok(root) => {
                                // 检查解析后的路径是否以工作区根目录开头
                                let resolved_abs = if path.is_absolute() {
                                    path.to_path_buf()
                                } else {
                                    std::path::Path::new(workspace_root).join(dir_path)
                                };
                                // 修复：使用 Path::starts_with 进行路径组件级别比较
                                // 字符串 starts_with 会将 "C:\workspace-evil" 误判为在 "C:\workspace" 内
                                if !resolved_abs.starts_with(&root) {
                                    return ToolResult {
                                        success: false,
                                        output: None,
                                        error: Some("目录路径不在工作区内，拒绝创建".to_string()),
                                        duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                // 校验通过，继续执行
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some("工作区根目录路径无效".to_string()),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("目录路径不在工作区内，拒绝创建".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        // 检查目录是否已存在
        if path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("目录已存在: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 检查工作区根目录是否存在，防止自动重建已删除的工作区目录
        if !workspace_root.is_empty() {
            let root_path = std::path::Path::new(workspace_root);
            if !root_path.exists() {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("工作区目录已被删除，请移除该工作区后重新选择".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                };
            }
        }

        let result = if recursive {
            tokio::fs::create_dir_all(&resolved_path).await
        } else {
            tokio::fs::create_dir(&resolved_path).await
        };

        match result {
            Ok(_) => {
                log::info!("目录已创建: {}", dir_path);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": dir_path,
                        "message": format!("目录已创建: {}", dir_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("创建目录失败: {}, 错误: {}", dir_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("创建目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// write_text_file - 写入纯文本文件
// ============================================================

struct WriteTextFileTool;

#[async_trait]
impl Tool for WriteTextFileTool {
    fn tool_name(&self) -> &str { "write_text_file" }
    fn description(&self) -> &str { "写入纯文本文件内容（.txt/.md/.csv/.json等），不依赖Sidecar。使用场景：创建纯文本文件、修改Markdown文件、保存JSON配置。支持追加模式。注意：仅适用于纯文本，生成结构化文档请使用docx_handler/xlsx_handler/pptx_handler/pdf_handler的generate操作。内容大小限制4KB（约4000字符），超出可能触发LLM响应截断；对于大文件（如6000+字符的测试文件、长文档），请改用code_interpreter_handler通过Python代码生成（如循环写入、字符串拼接），避免将大段内容作为工具参数传输。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "content": {
                    "type": "string",
                    "description": "文件内容"
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认utf-8",
                    "default": "utf-8"
                },
                "append": {
                    "type": "boolean",
                    "description": "是否追加写入",
                    "default": false
                }
            },
            "required": ["path", "content"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let content = params["content"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let append = params["append"].as_bool().unwrap_or(false);
        // 读取 encoding 参数（默认 utf-8），支持 GBK/GB2312/Big5/Shift_JIS/Latin1 等
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验
        if !workspace_root.is_empty() {
            // 如果文件已存在，直接校验
            // 如果文件不存在，校验父目录
            let check_path = if path.exists() {
                match crate::utils::canonicalize(path) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("路径无效: {}", file_path)),
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                }
            } else {
                // 文件不存在，校验父目录
                match path.parent() {
                    Some(parent) if parent.exists() => {
                        match crate::utils::canonicalize(parent) {
                            Ok(p) => p,
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some(format!("父目录路径无效: {}", file_path)),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
                    _ => {
                        // 父目录也不存在，检查解析路径是否在工作区内
                        match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                            Ok(root) => {
                                let resolved_abs = if path.is_absolute() {
                                    path.to_path_buf()
                                } else {
                                    std::path::Path::new(workspace_root).join(file_path)
                                };
                                // 修复：使用 Path::starts_with 进行路径组件级别比较
                                // 字符串 starts_with 会将 "C:\workspace-evil" 误判为在 "C:\workspace" 内
                                if !resolved_abs.starts_with(&root) {
                                    return ToolResult {
                                        success: false,
                                        output: None,
                                        error: Some("文件路径不在工作区内，拒绝写入".to_string()),
                                        duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some("工作区根目录路径无效".to_string()),
                                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root = match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("工作区根目录路径无效".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("文件路径不在工作区内，拒绝写入".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        // 确保父目录存在
        // 但如果工作区根目录已被删除，不允许自动重建，应提示用户重新选择工作区
        if let Some(parent) = path.parent() {
            if !parent.exists() {
                // 检查工作区根目录是否存在
                if !workspace_root.is_empty() {
                    let root_path = std::path::Path::new(workspace_root);
                    if !root_path.exists() {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("工作区目录已被删除，请移除该工作区后重新选择".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                }
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        // 根据 encoding 参数编码内容为字节
        // 支持 UTF-8/GBK/GB2312/Big5/Shift_JIS/Latin1 等多种编码
        let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())
            .unwrap_or(encoding_rs::UTF_8);
        // 编码字符串为字节（encoding_rs 自动处理无法编码的字符）
        let (encoded_bytes, _actual_encoding, _had_errors) = encoding.encode(content);
        let encoded_bytes = encoded_bytes.into_owned();

        let write_result = if append {
            // 追加模式：直接追加到目标文件（原子写入不适用于追加场景）
            match tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&resolved_path)
                .await
            {
                Ok(mut file) => tokio::io::AsyncWriteExt::write_all(&mut file, &encoded_bytes).await,
                Err(e) => Err(e),
            }
        } else {
            // 非追加模式：原子写入（先写临时文件，再 rename 到目标路径）
            // 防止写入过程中崩溃导致原文件损坏
            let tmp_path = format!("{}.tmp", resolved_path);
            match tokio::fs::write(&tmp_path, &encoded_bytes).await {
                Ok(_) => {
                    // rename 是原子操作（同文件系统内）
                    match tokio::fs::rename(&tmp_path, &resolved_path).await {
                        Ok(_) => Ok(()),
                        Err(rename_err) => {
                            // rename 失败，清理临时文件
                            let _ = tokio::fs::remove_file(&tmp_path).await;
                            Err(rename_err)
                        }
                    }
                }
                Err(e) => {
                    // 写入临时文件失败，清理可能残留的临时文件
                    let _ = tokio::fs::remove_file(&tmp_path).await;
                    Err(e)
                }
            }
        };

        match write_result {
            Ok(_) => {
                log::info!("文件已写入: {}, 编码: {}", file_path, encoding.name());
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "message": format!("文件已{}: {}", if append { "追加" } else { "写入" }, file_path),
                        "size": encoded_bytes.len(),
                        "encoding": encoding.name(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("写入文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("写入文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// 阶段三 3.5 新增工具：rename_file / copy_file / delete_directory
// / get_file_hash / read_file_lines
// ============================================================

/// 校验已存在的路径是否在工作区内
/// 返回 Ok((canonical_path, canonical_root)) 表示通过校验
/// 返回 Err(error_message) 表示校验失败
/// 用于需要路径安全校验的工具，减少重复代码
fn validate_existing_path_in_workspace(
    resolved_path: &str,
    workspace_root: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    if workspace_root.is_empty() {
        return Err("缺少工作区根目录路径，无法进行安全校验".to_string());
    }

    let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
        .map_err(|_| format!("工作区根目录不存在或无效: {}", workspace_root))?;

    // 安全防线 1：词法归一化检查（不依赖文件系统）
    // 即使目标文件不存在（canonicalize 会失败），也能识别 `../` 越界并拒绝
    // 避免攻击者通过路径遍历探测文件存在性
    let normalized_path = normalize_path_lexically(resolved_path, &canonical_root);
    if !normalized_path.starts_with(&canonical_root) {
        return Err(format!(
            "路径不在工作区内，拒绝访问: {} (工作区: {})",
            resolved_path,
            canonical_root.display()
        ));
    }

    // 安全防线 2：canonicalize 确认路径真实存在
    let canonical_path = crate::utils::canonicalize(std::path::Path::new(resolved_path))
        .map_err(|_| format!("路径不存在或无效: {}", resolved_path))?;

    // 安全防线 3：组件级 starts_with 比较（避免字符串前缀匹配的绕过风险）
    // 防止符号链接等文件系统层面的绕过
    if !canonical_path.starts_with(&canonical_root) {
        return Err(format!(
            "路径不在工作区内，拒绝访问: {} (工作区: {})",
            canonical_path.display(),
            canonical_root.display()
        ));
    }

    Ok((canonical_path, canonical_root))
}

/// 对路径进行词法归一化（不访问文件系统）
/// 用于在 canonicalize 失败前识别 `..` 越界，避免泄露文件存在性信息
/// 注意：这是安全防护的补充手段，不能替代 canonicalize（无法识别符号链接）
/// Rust 标准库的 Path::components() 会保留 ParentDir(`..`) 组件，
/// 因此必须手动解析 `..` 才能正确判断越界
fn normalize_path_lexically(resolved_path: &str, workspace_root: &std::path::Path) -> std::path::PathBuf {
    use std::path::Component;
    let path = std::path::Path::new(resolved_path);
    // 如果是相对路径，基于工作区拼接
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace_root.join(path)
    };

    // 手动解析 `.` 和 `..` 组件（不访问文件系统）
    let mut stack: Vec<std::path::Component<'_>> = Vec::new();
    for comp in absolute.components() {
        match comp {
            Component::CurDir => { /* `.` 忽略 */ }
            Component::ParentDir => {
                // 弹出最后一个 Normal 组件（不弹出根前缀如 Prefix/RootDir）
                if let Some(last) = stack.last() {
                    match last {
                        Component::Normal(_) => {
                            stack.pop();
                        }
                        // 根目录或前缀（如 C:\）下不能再 `..`，忽略
                        Component::RootDir | Component::Prefix(_) => {}
                        Component::ParentDir => stack.push(comp), // 连续 .. 保留
                        Component::CurDir => unreachable!(),
                    }
                }
            }
            _ => stack.push(comp),
        }
    }
    stack.iter().collect::<std::path::PathBuf>()
}

/// 校验目标路径（可能不存在）的父目录是否在工作区内
/// 用于 rename_file/copy_file 的目标路径校验（目标文件可能尚不存在）
/// 返回 Ok(canonical_root) 表示通过校验
fn validate_target_path_in_workspace(
    resolved_target: &str,
    workspace_root: &str,
) -> Result<std::path::PathBuf, String> {
    if workspace_root.is_empty() {
        return Err("缺少工作区根目录路径，无法进行安全校验".to_string());
    }

    let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
        .map_err(|_| format!("工作区根目录不存在或无效: {}", workspace_root))?;

    let target_path = std::path::Path::new(resolved_target);
    // 目标路径可能不存在，规范化父目录
    let check_path = if target_path.exists() {
        crate::utils::canonicalize(target_path)
            .map_err(|_| format!("目标路径无效: {}", resolved_target))?
    } else {
        // 父目录必须存在且在工作区内
        let parent = target_path.parent().unwrap_or(std::path::Path::new(""));
        if parent.as_os_str().is_empty() {
            // 没有父目录（如 "file.txt"），用工作区根目录
            canonical_root.clone()
        } else {
            crate::utils::canonicalize(parent)
                .map_err(|_| format!("目标路径的父目录无效: {}", parent.display()))?
        }
    };

    if !check_path.starts_with(&canonical_root) {
        return Err(format!(
            "目标路径不在工作区内，拒绝访问: {} (工作区: {})",
            resolved_target,
            canonical_root.display()
        ));
    }

    Ok(canonical_root)
}

// ============================================================
// rename_file - 重命名/移动文件
// ============================================================

struct RenameFileTool;

#[async_trait]
impl Tool for RenameFileTool {
    fn tool_name(&self) -> &str { "rename_file" }
    fn description(&self) -> &str { "重命名或移动文件。使用场景：整理文件结构、修改文件名。注意：跨文件系统移动可能失败，此操作不可逆。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "源文件路径（相对于工作区）"
                },
                "target_path": {
                    "type": "string",
                    "description": "目标文件路径（相对于工作区）"
                }
            },
            "required": ["source_path", "target_path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let source_path = params["source_path"].as_str().unwrap_or("");
        let target_path = params["target_path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if source_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少源文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目标文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) = match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("rename_file 源路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 校验目标路径在工作区内
        if let Err(e) = validate_target_path_in_workspace(&resolved_target, workspace_root) {
            log::warn!("rename_file 目标路径校验失败: {}", e);
            return ToolResult {
                success: false,
                output: None,
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("源路径不是文件: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 确保目标父目录存在
        let target_p = std::path::Path::new(&resolved_target);
        if let Some(parent) = target_p.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建目标父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        // 执行重命名
        match tokio::fs::rename(&canonical_source, &resolved_target).await {
            Ok(_) => {
                log::info!("文件已重命名: {} -> {}", source_path, target_path);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "source_path": source_path,
                        "target_path": target_path,
                        "message": format!("文件已重命名: {} -> {}", source_path, target_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("重命名文件失败: {} -> {}, 错误: {}", source_path, target_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("重命名文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// copy_file - 复制文件
// ============================================================

struct CopyFileTool;

#[async_trait]
impl Tool for CopyFileTool {
    fn tool_name(&self) -> &str { "copy_file" }
    fn description(&self) -> &str { "复制文件到新路径。使用场景：创建文件副本、备份文件、复制模板。支持二进制文件复制。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "源文件路径（相对于工作区）"
                },
                "target_path": {
                    "type": "string",
                    "description": "目标文件路径（相对于工作区）"
                }
            },
            "required": ["source_path", "target_path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let source_path = params["source_path"].as_str().unwrap_or("");
        let target_path = params["target_path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if source_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少源文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目标文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) = match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("copy_file 源路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 校验目标路径在工作区内
        if let Err(e) = validate_target_path_in_workspace(&resolved_target, workspace_root) {
            log::warn!("copy_file 目标路径校验失败: {}", e);
            return ToolResult {
                success: false,
                output: None,
                error: Some(e),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("源路径不是文件: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 确保目标父目录存在
        let target_p = std::path::Path::new(&resolved_target);
        if let Some(parent) = target_p.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("创建目标父目录失败: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        // 执行复制
        match tokio::fs::copy(&canonical_source, &resolved_target).await {
            Ok(bytes_copied) => {
                log::info!("文件已复制: {} -> {}, 字节数: {}", source_path, target_path, bytes_copied);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "source_path": source_path,
                        "target_path": target_path,
                        "bytes_copied": bytes_copied,
                        "message": format!("文件已复制: {} -> {}", source_path, target_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("复制文件失败: {} -> {}, 错误: {}", source_path, target_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("复制文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// delete_directory - 删除目录
// ============================================================

struct DeleteDirectoryTool;

#[async_trait]
impl Tool for DeleteDirectoryTool {
    fn tool_name(&self) -> &str { "delete_directory" }
    fn description(&self) -> &str { "递归删除目录及其所有内容。注意：此操作不可逆，会自动触发用户确认。建议在删除前确认目录内容。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "要删除的目录路径（相对于工作区）"
                },
                "create_backup": {
                    "type": "boolean",
                    "description": "删除前是否创建备份目录（复制到 .bak 后缀目录），默认 false（目录备份开销较大）",
                    "default": false
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let create_backup = params["create_backup"].as_bool().unwrap_or(false);

        if dir_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少目录路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(dir_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_dir, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("delete_directory 路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 必须是目录
        if !canonical_dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是目录: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 禁止删除工作区根目录本身
        let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
            .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root));
        if canonical_dir == canonical_root {
            return ToolResult {
                success: false,
                output: None,
                error: Some("禁止删除工作区根目录".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        let safe_path = canonical_dir.to_string_lossy().to_string();
        let mut backup_path_str = String::new();

        // 可选备份：复制到 .bak 目录
        if create_backup {
            let backup_path = format!("{}.bak", safe_path);
            match tokio::fs::create_dir_all(&backup_path).await {
                Ok(_) => {
                    // 递归复制目录内容到备份目录
                    if let Err(e) = copy_dir_recursive(&safe_path, &backup_path).await {
                        log::error!("创建目录备份失败: {}, 拒绝删除操作", e);
                        // 清理部分创建的备份
                        let _ = tokio::fs::remove_dir_all(&backup_path).await;
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!(
                                "创建备份失败: {}。如需跳过备份强制删除，请设置 create_backup=false 后重试",
                                e
                            )),
                            duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                        };
                    }
                    log::info!("删除前已创建备份: {}", backup_path);
                    backup_path_str = backup_path;
                }
                Err(e) => {
                    log::error!("创建备份目录失败: {}, 拒绝删除操作", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!(
                            "创建备份失败: {}。如需跳过备份强制删除，请设置 create_backup=false 后重试",
                            e
                        )),
                        duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }
            }
        }

        // 执行删除
        match tokio::fs::remove_dir_all(&safe_path).await {
            Ok(_) => {
                log::info!("目录已删除: {}", safe_path);
                let mut result = json!({
                    "path": dir_path,
                    "message": format!("目录已删除: {}", dir_path),
                });
                if !backup_path_str.is_empty() {
                    result["backup_path"] = json!(backup_path_str);
                }
                ToolResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除目录失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("删除目录失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

/// 递归复制目录内容到目标目录
/// 用于 delete_directory 的备份功能
async fn copy_dir_recursive(src: &str, dst: &str) -> Result<(), std::io::Error> {
    tokio::task::spawn_blocking({
        let src = src.to_string();
        let dst = dst.to_string();
        move || {
            fn copy_inner(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
                if !dst.exists() {
                    std::fs::create_dir_all(dst)?;
                }
                for entry in std::fs::read_dir(src)? {
                    let entry = entry?;
                    let path = entry.path();
                    let file_name = entry.file_name();
                    let dest_path = dst.join(&file_name);
                    if path.is_dir() {
                        copy_inner(&path, &dest_path)?;
                    } else {
                        std::fs::copy(&path, &dest_path)?;
                    }
                }
                Ok(())
            }
            copy_inner(std::path::Path::new(&src), std::path::Path::new(&dst))
        }
    })
    .await
    .map_err(std::io::Error::other)?
}

// ============================================================
// get_file_hash - 计算文件哈希
// ============================================================

struct GetFileHashTool;

#[async_trait]
impl Tool for GetFileHashTool {
    fn tool_name(&self) -> &str { "get_file_hash" }
    fn description(&self) -> &str { "计算文件的 SHA-256 哈希值。使用场景：文件去重、完整性校验、变更检测。返回十六进制哈希字符串。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_file, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("get_file_hash 路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 必须是文件
        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 在 spawn_blocking 中读取文件并计算哈希（避免阻塞异步运行时）
        let hash_result = tokio::task::spawn_blocking(move || {
            use std::io::Read;
            let mut file = std::fs::File::open(&canonical_file)?;
            let mut hasher = Sha256::new();
            // 分块读取，避免大文件一次性加载到内存
            let mut buffer = [0u8; 8192];
            loop {
                let n = file.read(&mut buffer)?;
                if n == 0 {
                    break;
                }
                hasher.update(&buffer[..n]);
            }
            let hash_bytes = hasher.finalize();
            Ok::<String, std::io::Error>(format!("{:x}", hash_bytes))
        })
        .await;

        match hash_result {
            Ok(Ok(hash)) => {
                log::info!("文件哈希计算完成: {}, sha256={}", file_path, &hash[..16]);
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "algorithm": "sha256",
                        "hash": hash,
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("计算文件哈希失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("计算文件哈希失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("计算文件哈希任务失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("计算文件哈希任务失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// read_file_lines - 按行读取文件
// ============================================================

struct ReadFileLinesTool;

#[async_trait]
impl Tool for ReadFileLinesTool {
    fn tool_name(&self) -> &str { "read_file_lines" }
    fn description(&self) -> &str { "按行读取纯文本文件，支持偏移和行数限制。使用场景：读取大文件的指定部分、分页读取、查看日志文件尾部。推荐用于大文件分页读取。" }
    fn category(&self) -> &str { "filesystem" }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "文件路径（相对于工作区）"
                },
                "offset": {
                    "type": "integer",
                    "description": "起始行偏移（0-based），默认 0",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "读取行数限制，默认 100，最大 1000",
                    "default": 100
                },
                "encoding": {
                    "type": "string",
                    "description": "文件编码，默认 utf-8。支持 gbk/gb2312/big5/shift_jis/latin1",
                    "default": "utf-8"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let offset = params["offset"].as_u64().unwrap_or(0) as usize;
        let limit = params["limit"].as_u64().unwrap_or(100) as usize;
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("缺少文件路径".to_string()),
                duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 限制最大读取行数，防止 LLM 请求过大导致内存压力
        let safe_limit = limit.min(1000);

        let resolved_path = resolve_path(file_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_file, _) = match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
            Ok(paths) => paths,
            Err(e) => {
                log::warn!("read_file_lines 路径校验失败: {}", e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        };

        // 必须是文件
        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("路径不是文件: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64, error_code: None,
            };
        }

        // 在 spawn_blocking 中读取文件（避免阻塞异步运行时）
        let path_for_task = canonical_file.clone();
        let encoding_label_owned = encoding_label.to_string();
        let read_result = tokio::task::spawn_blocking(move || {
            // 读取文件字节
            let bytes = std::fs::read(&path_for_task)?;

            // 根据编码参数解码
            let encoding = encoding_rs::Encoding::for_label(encoding_label_owned.as_bytes())
                .unwrap_or(encoding_rs::UTF_8);
            let (decoded, _actual_encoding, _had_errors) = encoding.decode(&bytes);
            let content = decoded.into_owned();

            // 按行分割（兼容 \n 和 \r\n）
            let lines: Vec<&str> = content.lines().collect();
            let total_lines = lines.len();

            // 应用 offset 和 limit
            let end = offset.saturating_add(safe_limit).min(total_lines);
            let selected: Vec<String> = if offset < total_lines {
                lines[offset..end].iter().map(|s| s.to_string()).collect()
            } else {
                Vec::new()
            };

            Ok::<(Vec<String>, usize), std::io::Error>((selected, total_lines))
        })
        .await;

        match read_result {
            Ok(Ok((lines, total_lines))) => {
                let returned_lines = lines.len();
                log::debug!(
                    "按行读取文件完成: {}, offset={}, limit={}, 返回 {} 行（总 {} 行）",
                    file_path, offset, safe_limit, returned_lines, total_lines
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "offset": offset,
                        "limit": safe_limit,
                        "total_lines": total_lines,
                        "returned_lines": returned_lines,
                        "lines": lines,
                        "has_more": offset + returned_lines < total_lines,
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("按行读取文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("按行读取文件失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
            Err(e) => {
                log::error!("按行读取文件任务失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("按行读取文件任务失败: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                }
            }
        }
    }
}

// ============================================================
// update_notes - 智能体草稿本（Scratchpad）
// ============================================================
//
// 设计依据：Anthropic《Effective Context Engineering for AI Agents》(2025-09-29)
// 的 "Structured Note-taking" 模式。Agent 在长程任务中自主调用本工具记录关键进度、
// 决策点、待办事项，避免外部硬编码迭代元数据（如"迭代轮次 3/100"、"当前步骤"）
// 注入消息列表，从而：
//   1. 避免角色混淆（Role Confusion）——伪 user 消息注入元数据是反模式
//   2. 节省注意力预算——笔记内容是 agent 主动写的，信噪比高于外部猜测
//   3. 培养 agent 自我规划能力——由 agent 决定记录什么、何时记录
//
// 状态隔离：通过 session_id 在 HashMap 中隔离不同会话的笔记
// 注入方式：executor 每轮迭代开始时读取当前 session 的笔记，刷新到
//           AgentContext::scratchpad_summary，由 get_messages_for_iteration
//           追加到消息列表末尾（保留前缀稳定性以最大化缓存命中）

/// Scratchpad 工具：智能体草稿本
/// 持有全局共享状态 Arc，按 session_id 隔离不同会话
pub struct ScratchpadTool {
    pub states: SharedScratchpadStates,
}

/// Scratchpad 工具的 action 枚举
const ACTION_ADD: &str = "add";
const ACTION_READ: &str = "read";
const ACTION_CLEAR: &str = "clear";

#[async_trait]
impl Tool for ScratchpadTool {
    fn tool_name(&self) -> &str { "update_notes" }

    fn description(&self) -> &str {
        "智能体草稿本：记录或读取任务笔记，用于跨迭代轮次保持上下文。\
         适用场景：复杂多步骤任务中记录关键决策、待办事项、文件路径、中间结果。\
         建议在完成关键步骤后调用 action=add 记录要点；当任务上下文变长时，\
         action=read 可回顾已有笔记；任务完成后 action=clear 清理。\
         笔记内容会在后续迭代中自动注入到你的上下文，无需重复读取。"
    }

    fn category(&self) -> &str { "memory" }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "read", "clear"],
                    "description": "操作类型：add=追加笔记；read=读取所有笔记；clear=清空笔记",
                    "default": "add"
                },
                "content": {
                    "type": "string",
                    "description": "笔记内容（action=add 时必填）。建议简明扼要，每条不超过200字"
                }
            },
            "required": ["action"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        // 从 params 中取出 _session_id（由 executor 在调用前注入）
        // _session_id 以下划线开头，表示是系统注入参数，不暴露给 LLM
        let session_id = params["_session_id"].as_str().unwrap_or("").to_string();
        if session_id.is_empty() {
            log::warn!("update_notes 调用缺少 _session_id 参数");
            return ToolResult {
                success: false,
                output: None,
                error: Some("内部错误：缺少会话标识".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let action = params["action"].as_str().unwrap_or(ACTION_ADD);
        let content = params["content"].as_str().unwrap_or("").to_string();
        let iteration = params["_iteration"].as_u64().unwrap_or(0) as u32;

        match action {
            ACTION_ADD => {
                if content.is_empty() {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some("action=add 时 content 不能为空".to_string()),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                    };
                }

                // 限制单条笔记长度，防止滥用
                let safe_content: String = content.chars().take(500).collect();
                let entry = ScratchpadEntry {
                    content: safe_content,
                    iteration,
                    timestamp_ms: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0),
                };

                let entry_count = {
                    let mut states = self.states.write().expect("scratchpad states 锁中毒");
                    let state = states.entry(session_id.clone()).or_default();
                    state.push(entry);
                    state.len()
                };

                log::info!(
                    "update_notes 追加笔记: session_id={}, 当前笔记数={}",
                    session_id, entry_count
                );

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "action": "add",
                        "total_notes": entry_count,
                        "message": format!("笔记已记录（共 {} 条）", entry_count),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            ACTION_READ => {
                let states = self.states.read().expect("scratchpad states 锁中毒");
                let notes: Vec<&ScratchpadEntry> = states
                    .get(&session_id)
                    .map(|s| s.iter().collect())
                    .unwrap_or_default();

                log::info!(
                    "update_notes 读取笔记: session_id={}, 笔记数={}",
                    session_id, notes.len()
                );

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "action": "read",
                        "total_notes": notes.len(),
                        "notes": notes.iter().map(|e| &e.content).collect::<Vec<_>>(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            ACTION_CLEAR => {
                let cleared_count = {
                    let mut states = self.states.write().expect("scratchpad states 锁中毒");
                    states.remove(&session_id)
                        .map(|s| s.len())
                        .unwrap_or(0)
                };

                log::info!(
                    "update_notes 清空笔记: session_id={}, 已清除 {} 条",
                    session_id, cleared_count
                );

                ToolResult {
                    success: true,
                    output: Some(json!({
                        "action": "clear",
                        "cleared_notes": cleared_count,
                        "message": format!("已清空 {} 条笔记", cleared_count),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            _ => {
                log::warn!("update_notes 未知 action: {}", action);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("未知 action: {}（支持 add/read/clear）", action)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                }
            }
        }
    }
}

/// 格式化 Scratchpad 笔记列表为摘要字符串（供 AgentContext 注入消息列表）
/// 返回 None 表示无笔记，调用方应跳过注入
pub fn format_scratchpad_summary(
    states: &SharedScratchpadStates,
    session_id: &str,
) -> Option<String> {
    let states = states.read().ok()?;
    let state = states.get(session_id)?;
    if state.is_empty() {
        return None;
    }

    let mut summary = String::from("<scratchpad>\n## 你的任务笔记\n\n以下是你之前记录的任务笔记，请基于这些笔记继续工作（无需重复读取）：\n\n");
    for (i, entry) in state.iter().enumerate() {
        summary.push_str(&format!("{}. {}\n", i + 1, entry.content));
    }
    summary.push_str("\n如需更新笔记，请调用 update_notes 工具。\n</scratchpad>");
    Some(summary)
}
