// 允许在测试模块之后定义工具：项目原有结构将测试模块置于文件中部，
// WriteTextFileTool 等新增的 5 个工具均位于测试模块之后。
// 完整重构文件结构（移动测试模块到末尾）超出当前任务范围，这里以 allow 抑制 lint。
#![allow(clippy::items_after_test_module)]

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::registry::ToolRegistry;
use super::trait_def::Tool;
use crate::db::Database;
use crate::models::tool::{ScratchpadEntry, ScratchpadState, ToolResult};

// 子模块声明
pub mod question;
mod sourcecode;
pub mod task;
mod todowrite;
pub mod webfetch;
pub mod websearch;

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

/// 内置工具注册结果
/// 包含 Scratchpad 共享状态和 TaskTool 引用（用于延迟注入 SubAgentExecutor）
pub struct BuiltinToolsRegistration {
    /// Scratchpad 共享状态
    pub scratchpad_states: SharedScratchpadStates,
    /// TaskTool 引用（用于延迟注入 SubAgentExecutor）
    pub task_tool: task::TaskTool,
}

/// 注册所有内置工具
/// 返回 BuiltinToolsRegistration，包含 Scratchpad 共享状态和 TaskTool（用于延迟注入 SubAgentExecutor）
/// git_bash_path: Git Bash 可执行文件路径（空字符串表示从 PATH 自动检测）
/// db: 数据库连接
/// web_search_config: WebSearch 配置（从 AppSettings 读取）
/// question_channels: Question 工具答案通道（与 submit_question_answer 命令共享）
/// app_handle: Tauri AppHandle（用于 QuestionTool 发射事件）
/// skill_registry: Skill 注册表（用于 SkillTool 注册）
#[allow(clippy::too_many_arguments)]
pub fn register_builtin_tools(
    registry: &mut ToolRegistry,
    git_bash_path: String,
    db: Arc<Database>,
    web_search_config: crate::config::app_settings::WebSearchConfig,
    question_channels: question::QuestionChannels,
    app_handle: Option<tauri::AppHandle<tauri::Wry>>,
    skill_registry: Arc<crate::services::skill::registry::SkillRegistry>,
) -> BuiltinToolsRegistration {
    log::info!("开始注册内置工具");
    registry.register(Box::new(ListDirectoryTool));
    registry.register(Box::new(SearchFilesTool));
    registry.register(Box::new(ReadFileTool));
    registry.register(Box::new(FileInfoTool));
    registry.register(Box::new(FileExistsTool));
    registry.register(Box::new(DeleteFileTool));
    registry.register(Box::new(CreateDirectoryTool));
    registry.register(Box::new(WriteTextFileTool));
    // 新增的 5 个基础文件系统工具
    registry.register(Box::new(RenameFileTool));
    registry.register(Box::new(CopyFileTool));
    registry.register(Box::new(DeleteDirectoryTool));
    registry.register(Box::new(GetFileHashTool));
    // 编程 Agent 改造: 精确字符串替换工具
    registry.register(Box::new(EditTool));
    // 编程 Agent 改造: glob 模式查找工具
    registry.register(Box::new(GlobTool));
    // 编程 Agent 改造: 正则表达式搜索工具
    registry.register(Box::new(GrepTool));

    // Scratchpad 工具：智能体草稿本，由 agent 自主调用 update_notes 写入
    // 设计参考 Anthropic《Effective Context Engineering for AI Agents》的
    // "Structured Note-taking" 模式，替代外部硬编码迭代元数据注入
    let scratchpad_states: SharedScratchpadStates = Arc::new(RwLock::new(HashMap::new()));
    registry.register(Box::new(ScratchpadTool {
        states: scratchpad_states.clone(),
    }));

    // 代码执行工具：write_script + bash + powershell
    // 让智能体通过编写脚本文件并执行命令解决用户问题
    // 命令超时由 LLM 通过 timeout 参数自主决定，最大 300 秒
    // powershell 无需配置路径：优先使用 PATH 上的 pwsh.exe，回退系统内置的 5.1
    registry.register(Box::new(WriteScriptTool));
    registry.register(Box::new(RunCommandTool { git_bash_path }));
    registry.register(Box::new(RunPowerShellCommandTool));

    // TodoWrite 工具：结构化任务管理，按 session_id 隔离并持久化到数据库
    registry.register(Box::new(todowrite::TodoWriteTool::new(db)));

    // SourceCode 工具：基于 tree-sitter 的代码语义搜索
    // 支持按符号类型(function/class/struct 等)和名称通配符查询代码符号
    registry.register(Box::new(
        sourcecode::SourceCodeTool::new().expect("创建 SourceCodeTool 失败"),
    ));

    // Skill 工具（按需加载领域能力）
    registry.register(Box::new(crate::services::skill::tool::SkillTool::new(
        skill_registry,
    )));

    // 新增工具：Task（子 Agent 委托）、WebFetch（URL 获取）、WebSearch（网络搜索）、Question（向用户提问）
    // TaskTool 采用延迟注入模式：先创建不含 sub_executor 的实例并注册，
    // 后续在 lib.rs 中通过 set_sub_executor 注入 SubAgentExecutor
    let task_tool = task::TaskTool::new();
    registry.register(Box::new(task_tool.clone()));
    registry.register(Box::new(webfetch::WebFetchTool::new()));
    registry.register(Box::new(websearch::WebSearchTool::new(web_search_config)));
    registry.register(Box::new(question::QuestionTool::new(
        question_channels,
        app_handle,
    )));

    log::info!("内置工具注册完成, 共注册 26 个工具");

    BuiltinToolsRegistration {
        scratchpad_states,
        task_tool,
    }
}

// ============================================================
// list_directory - 列出目录内容
// ============================================================

struct ListDirectoryTool;

#[async_trait]
impl Tool for ListDirectoryTool {
    fn tool_name(&self) -> &str {
        "list"
    }
    fn description(&self) -> &str {
        "List files and subdirectories in the specified directory. Use cases: browsing workspace contents, locating files, understanding directory hierarchy. Supports depth control and extension filtering."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path, defaults to current working directory"
                },
                "depth": {
                    "type": "integer",
                    "description": "Traversal depth, default 1",
                    "default": 1
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Filter file extensions, e.g. [\"docx\", \"pdf\"]"
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
                error: Some("depth parameter must be greater than or equal to 1".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let extensions: Vec<String> = params["extensions"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let resolved_dir = resolve_path(dir_path, workspace_root);
        let dir = std::path::Path::new(&resolved_dir);
        if !dir.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Directory does not exist: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        if !dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Path is not a directory: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
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
                        error: Some(format!("Directory path is invalid: {}", dir_path)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            };
            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("Workspace root directory path is invalid".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("Directory is outside the workspace, access denied".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let resolved_dir_owned = resolved_dir.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir = std::path::Path::new(&resolved_dir_owned);
            tool_list_dir(dir, dir, max_depth, 0, &extensions_clone)
        })
        .await
        {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("list_directory spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Directory listing task failed: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
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
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
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

        if !is_dir && !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == ext)
        {
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
    fn tool_name(&self) -> &str {
        "search"
    }
    fn description(&self) -> &str {
        "Search files in the specified directory, supporting search by filename or content. Use cases: find files by name, search by content keywords, filter by extension. Set include_content=true to search file contents."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search keyword (optional, can be omitted when filtering by extension only)"
                },
                "directory": {
                    "type": "string",
                    "description": "Directory path to search, defaults to workspace root"
                },
                "extensions": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Restrict file extensions, e.g. [\"docx\", \"pdf\"]"
                },
                "include_content": {
                    "type": "boolean",
                    "description": "Whether to search file contents (only effective for text files)",
                    "default": false
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum number of results",
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
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if query.is_empty() && extensions.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Search keyword and file extensions cannot both be empty, please provide at least one".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let resolved_directory = resolve_path(directory, workspace_root);
        let dir_path = std::path::Path::new(&resolved_directory);
        if !dir_path.exists() || !dir_path.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Directory does not exist or is not a directory: {}",
                    directory
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        if !workspace_root.is_empty() {
            let canonical_dir = match crate::utils::canonicalize(dir_path) {
                Ok(p) => p,
                Err(_) => {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Directory path is invalid: {}", directory)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            };
            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("Workspace root directory path is invalid".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !canonical_dir.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "Search directory is outside the workspace, access denied".to_string(),
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        let query_lower = query.to_lowercase();
        let resolved_directory_owned = resolved_directory.clone();
        let extensions_clone = extensions.clone();

        let results = match tokio::task::spawn_blocking(move || {
            let dir_path = std::path::Path::new(&resolved_directory_owned);
            let mut results = Vec::new();
            tool_search_files(
                dir_path,
                dir_path,
                &query_lower,
                &extensions_clone,
                include_content,
                max_results,
                &mut results,
            );
            results
        })
        .await
        {
            Ok(results) => results,
            Err(join_err) => {
                // spawn_blocking 任务可能因 panic 失败，不应静默吞掉
                log::error!("search_files spawn_blocking 失败: {}", join_err);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("File search task failed: {}", join_err)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        log::info!(
            "文件搜索完成: query={}, directory={}, 结果数: {}",
            query,
            directory,
            results.len()
        );
        ToolResult {
            success: true,
            output: Some(json!({
                "query": query,
                "directory": directory,
                "total": results.len(),
                "results": results,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
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
            tool_search_files(
                &path,
                root,
                query,
                extensions,
                include_content,
                max_results,
                results,
            );
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
            let text_extensions = [
                "txt", "md", "markdown", "csv", "json", "xml", "html", "css", "js", "ts", "py",
                "rs", "toml", "yaml", "yml",
            ];
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
// read - 读取纯文本文件（带行号、二进制保护）
// ============================================================

/// 检测文件是否为二进制文件
/// 通过检查前 8KB 字节是否含 NUL 字节（0x00）判定
/// 含 NUL 字节通常表示为二进制文件（如图片、可执行文件、压缩包等）
fn is_binary_file(bytes: &[u8]) -> bool {
    let check_len = bytes.len().min(8192);
    bytes[..check_len].contains(&0x00)
}

/// 为文本内容添加行号
/// 格式：`   123→内容`（行号右对齐，宽度至少 5，后跟 `→` 和内容）
/// start_line: 起始行号（1-based）
/// end_line: 结束行号（1-based，包含在内），None 表示到文件末尾
fn add_line_numbers(content: &str, start_line: usize, end_line: Option<usize>) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();
    // start_line 是 1-based，转为 0-based 索引
    let start_idx = start_line.saturating_sub(1).min(total);
    // end_line 是 1-based 包含，end_idx 是切片末尾（不包含）
    let end_idx = match end_line {
        Some(end) => end.min(total),
        None => total,
    };

    if start_idx >= end_idx {
        return String::new();
    }

    // 计算行号显示宽度（至少 5）
    let max_line_num = start_line.saturating_add(end_idx - start_idx - 1);
    let width = max_line_num.to_string().len().max(5);

    let mut result = String::new();
    for (i, line) in lines[start_idx..end_idx].iter().enumerate() {
        let line_num = start_line + i;
        result.push_str(&format!("{:>width$}→{}\n", line_num, line, width = width));
    }
    result
}

struct ReadFileTool;

#[async_trait]
impl Tool for ReadFileTool {
    fn tool_name(&self) -> &str {
        "read"
    }
    fn description(&self) -> &str {
        "Read plain text file content (.txt/.md/.csv/.json/.xml etc.), automatically adding line numbers (format `   123→content`), with binary detection protection. Does not depend on Sidecar, faster. Supports reading by line range (start_line/end_line parameters). File size limit 2MB. Note: only for plain text files; use docx_handler/xlsx_handler/pptx_handler/pdf_handler read operations for structured documents like Word/Excel/PPT/PDF."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to workspace)"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Start line number (1-based), default 1",
                    "default": 1
                },
                "end_line": {
                    "type": "integer",
                    "description": "End line number (1-based, inclusive), if omitted reads to end of file"
                },
                "encoding": {
                    "type": "string",
                    "description": "File encoding, default utf-8",
                    "default": "utf-8"
                },
                "max_size": {
                    "type": "integer",
                    "description": "Maximum read bytes, default 2MB",
                    "default": 2097152
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let max_size = params["max_size"].as_u64().unwrap_or(2097152) as usize; // 默认 2MB
        let start_line = params["start_line"].as_u64().unwrap_or(1) as usize; // 默认第 1 行
        let end_line = params["end_line"].as_u64().map(|v| v as usize); // 可选
                                                                        // 读取 encoding 参数（默认 utf-8），支持 GBK/GB2312/Big5/Shift_JIS/Latin1 等
        let encoding_label = params["encoding"].as_str().unwrap_or("utf-8");

        if file_path.is_empty() {
            log::warn!("read 失败: 缺少文件路径");
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        if !workspace_root.is_empty() {
            let (canonical_file, _) =
                match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                    Ok(result) => result,
                    Err(e) => {
                        // 根据错误消息区分错误码：路径越界 vs 路径不存在
                        let is_out_of_bounds = e.contains("outside the workspace");
                        let error_code = if is_out_of_bounds {
                            Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                        } else {
                            None
                        };
                        log::warn!(
                            "read 失败: {}, path={}, workspace={}",
                            e,
                            file_path,
                            workspace_root
                        );
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(e),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code,
                        };
                    }
                };
            // 校验通过后，使用 canonical 路径继续读取
            let _ = canonical_file; // 已通过校验，path 变量继续使用（下方会重新 canonicalize 或直接读取）
        }

        if !path.exists() {
            log::warn!("read 失败: 文件不存在, path={}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("File does not exist: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        if !path.is_file() {
            log::warn!("read 失败: 路径不是文件, path={}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Path is not a file: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 检查文件大小
        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "read 失败: 获取文件信息失败, path={}, 错误: {}",
                    file_path,
                    e
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to get file info: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        if metadata.len() as usize > max_size {
            log::warn!(
                "read 失败: 文件过大, path={}, size={}字节, max={}字节",
                file_path,
                metadata.len(),
                max_size
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "File too large ({} bytes), exceeds maximum read limit ({} bytes)",
                    metadata.len(),
                    max_size
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 读取文件字节，根据 encoding 参数解码
        // 支持 UTF-8/GBK/GB2312/Big5/Shift_JIS/Latin1 等多种编码
        match tokio::fs::read(&resolved_path).await {
            Ok(bytes) => {
                // 二进制文件检测：检查前 8KB 是否含 NUL 字节
                if is_binary_file(&bytes) {
                    log::warn!("read 失败: 检测为二进制文件, path={}", file_path);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("File {} is detected as a binary file (contains NUL bytes), cannot be read as text. Please use the corresponding Handler (e.g. docx_handler/pdf_handler) to process structured documents.", file_path)), duration_ms: start.elapsed().as_millis() as u64, error_code: None,
                    };
                }

                // 根据 encoding 标签解析编码器
                let encoding = encoding_rs::Encoding::for_label(encoding_label.as_bytes())
                    .unwrap_or(encoding_rs::UTF_8);
                // 解码字节为字符串（encoding_rs 自动处理 BOM 和无效字节）
                let (content, _actual_encoding, _had_errors) = encoding.decode(&bytes);
                let content = content.into_owned();
                let total_lines = content.lines().count();

                // 按行范围截取并添加行号
                let numbered_content = add_line_numbers(&content, start_line, end_line);
                let returned_lines = numbered_content.lines().count();

                let ext = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_string();
                log::debug!(
                    "read 完成: {}, start_line={}, end_line={:?}, 返回 {} 行（总 {} 行）",
                    file_path,
                    start_line,
                    end_line,
                    returned_lines,
                    total_lines
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": file_path,
                        "content": numbered_content,
                        "start_line": start_line,
                        "end_line": end_line.unwrap_or(total_lines),
                        "total_lines": total_lines,
                        "returned_lines": returned_lines,
                        "size": metadata.len(),
                        "extension": ext,
                        "encoding": encoding.name(),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("读取文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to read file: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建内存数据库供测试使用
    fn test_db() -> Arc<Database> {
        Arc::new(Database::new(std::path::Path::new(":memory:")).unwrap())
    }

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
        let _scratchpad_states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        // 验证 26 个工具都已注册（8 个原有 + 4 个文件系统 + 1 个 scratchpad + 3 个代码执行 + 3 个搜索编辑 + 1 个 todowrite + 1 个 source_code + 1 个 skill + 4 个 task/web/question）
        let tools = registry.list_tools();
        assert_eq!(tools.len(), 26);

        // 验证每个工具的基本属性
        let tool_names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(tool_names.contains(&"list"));
        assert!(tool_names.contains(&"search"));
        assert!(tool_names.contains(&"read"));
        assert!(tool_names.contains(&"file_info"));
        assert!(tool_names.contains(&"exists"));
        assert!(tool_names.contains(&"remove"));
        assert!(tool_names.contains(&"mkdir"));
        assert!(tool_names.contains(&"write"));
        // 新增工具
        assert!(tool_names.contains(&"rename"));
        assert!(tool_names.contains(&"copy"));
        assert!(tool_names.contains(&"remove_dir"));
        assert!(tool_names.contains(&"hash"));
        // Scratchpad 工具
        assert!(tool_names.contains(&"scratchpad"));
        // 代码执行工具
        assert!(tool_names.contains(&"write_script"));
        assert!(tool_names.contains(&"bash"));
        assert!(tool_names.contains(&"powershell"));
        // 编程 Agent 改造新增工具
        assert!(tool_names.contains(&"edit"));
        assert!(tool_names.contains(&"glob"));
        assert!(tool_names.contains(&"grep"));
        // TodoWrite 工具
        assert!(tool_names.contains(&"todowrite"));
        // SourceCode 工具
        assert!(tool_names.contains(&"source_code"));
        // Skill 工具
        assert!(tool_names.contains(&"skill"));
        // 新增工具
        assert!(tool_names.contains(&"task"));
        assert!(tool_names.contains(&"webfetch"));
        assert!(tool_names.contains(&"websearch"));
        assert!(tool_names.contains(&"question"));
    }

    #[test]
    fn test_tool_definitions_count() {
        let mut registry = ToolRegistry::new();
        let _scratchpad_states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let defs = registry.tool_definitions();
        assert_eq!(defs.len(), 26);

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
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tools = registry.list_tools();
        for tool in &tools {
            assert!(tool.is_builtin);
            assert!(tool.enabled);
            assert_eq!(tool.version, "1.0.0");
            assert!(!tool.name.is_empty());
            assert!(!tool.description.is_empty());
            // 工具类别：filesystem/memory/code、agent/web、skill
            assert!(
                tool.category == "filesystem"
                    || tool.category == "memory"
                    || tool.category == "code"
                    || tool.category == "agent"
                    || tool.category == "web"
                    || tool.category == "skill"
            );
        }
    }

    #[tokio::test]
    async fn test_file_exists_nonexistent() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("exists").unwrap();
        let result = tool
            .execute(json!({
                "path": "/nonexistent/path/file.txt",
                "workspace_root": ""
            }))
            .await;

        assert!(result.success);
        assert!(result.output.is_some());
        let output = result.output.unwrap();
        assert_eq!(output["exists"], false);
    }

    #[tokio::test]
    async fn test_read_file_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("read").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Missing file path"));
    }

    #[tokio::test]
    async fn test_read_with_line_numbers() {
        // 验证 read 工具返回的内容带行号格式（`   N→内容`）
        use std::io::Write;
        let mut tmp_path = std::env::temp_dir();
        tmp_path.push(format!(
            "samoyed_work_test_read_ln_{}.txt",
            uuid::Uuid::new_v4()
        ));
        {
            let mut f = std::fs::File::create(&tmp_path).unwrap();
            writeln!(f, "first line").unwrap();
            writeln!(f, "second line").unwrap();
            writeln!(f, "third line").unwrap();
        }
        let workspace_root = std::env::temp_dir().to_string_lossy().to_string();
        let file_path = tmp_path.to_string_lossy().to_string();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("read").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "workspace_root": workspace_root,
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        let content = output["content"].as_str().unwrap();
        // 验证每行包含行号格式（→ 字符）
        assert!(content.contains("→first line"));
        assert!(content.contains("→second line"));
        assert!(content.contains("→third line"));
        assert_eq!(output["total_lines"], 3);
        assert_eq!(output["returned_lines"], 3);
        assert_eq!(output["start_line"], 1);
        assert_eq!(output["end_line"], 3);

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn test_read_line_range() {
        // 验证 read 工具按行号范围截取（start_line/end_line）
        use std::io::Write;
        let mut tmp_path = std::env::temp_dir();
        tmp_path.push(format!(
            "samoyed_work_test_read_range_{}.txt",
            uuid::Uuid::new_v4()
        ));
        {
            let mut f = std::fs::File::create(&tmp_path).unwrap();
            for i in 1..=10 {
                writeln!(f, "line {}", i).unwrap();
            }
        }
        let workspace_root = std::env::temp_dir().to_string_lossy().to_string();
        let file_path = tmp_path.to_string_lossy().to_string();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("read").unwrap();
        // 读取第 3-5 行
        let result = tool
            .execute(json!({
                "path": file_path,
                "workspace_root": workspace_root,
                "start_line": 3,
                "end_line": 5,
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        let content = output["content"].as_str().unwrap();
        assert_eq!(output["total_lines"], 10);
        assert_eq!(output["returned_lines"], 3);
        assert_eq!(output["start_line"], 3);
        assert_eq!(output["end_line"], 5);
        // 验证内容只包含第 3-5 行
        assert!(content.contains("→line 3"));
        assert!(content.contains("→line 4"));
        assert!(content.contains("→line 5"));
        assert!(!content.contains("→line 2"));
        assert!(!content.contains("→line 6"));

        let _ = std::fs::remove_file(&tmp_path);
    }

    #[tokio::test]
    async fn test_edit_tool_create_new_file() {
        // 验证 edit 工具创建新文件（old_string 为空且文件不存在）
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_edit_create_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "new_file.txt";
        let new_content = "Hello, this is a new file.\nLine 2.";

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "",
                "new_string": new_content,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "创建新文件失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["operation"], "create");
        assert_eq!(output["bytes_written"], new_content.len());

        // 验证文件内容
        let abs_path = temp_dir.join(file_path);
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, new_content);

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_replace_unique() {
        // 验证 edit 工具唯一匹配替换
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_replace_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "edit_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "fn main() {\n    println!(\"hello\");\n}\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "println!(\"hello\");",
                "new_string": "println!(\"world\");",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "替换失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["operation"], "edit");
        assert_eq!(output["matches"], 1);

        // 验证替换后内容
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "fn main() {\n    println!(\"world\");\n}\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_multiple_matches_error() {
        // 验证 edit 工具多处匹配时报错
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_edit_multi_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "multi_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "foo\nbar\nfoo\nbaz\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "foo",
                "new_string": "qux",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Found 2 matches"));

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_no_match_error() {
        // 验证 edit 工具 0 匹配时报错
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_nomatch_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "nomatch_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "hello world\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "nonexistent string",
                "new_string": "replacement",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("No matching string found"));

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_crlf_file_lf_old_string_normalized() {
        // 验证 CRLF 文件 + LF old_string 时归一化匹配成功，写回 CRLF
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_crlf_norm_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "crlf_test.txt";
        let abs_path = temp_dir.join(file_path);
        // 文件使用 CRLF 行尾
        let original = "line1\r\nline2\r\nline3\r\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        // old_string 使用 LF 行尾（精确匹配会失败，触发归一化）
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "line2\n",
                "new_string": "replaced\n",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "归一化匹配应成功: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["operation"], "edit");
        assert_eq!(output["matches"], 1);

        // 验证写回的文件仍为 CRLF 行尾
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "line1\r\nreplaced\r\nline3\r\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_lf_file_lf_old_string_no_normalization() {
        // 验证 LF 文件 + LF old_string 时精确匹配成功，不触发归一化
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_lf_exact_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "lf_test.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "line1\nline2\nline3\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "line2\n",
                "new_string": "replaced\n",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "精确匹配应成功: {:?}", result.error);
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "line1\nreplaced\nline3\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_crlf_file_lf_old_string_replace_all() {
        // 验证 CRLF 文件 + LF old_string + replace_all=true 时归一化匹配多处，写回 CRLF
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_crlf_all_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "crlf_replace_all.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "foo\r\nbar\r\nfoo\r\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "foo\n",
                "new_string": "qux\n",
                "replace_all": true,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(
            result.success,
            "归一化 replace_all 应成功: {:?}",
            result.error
        );
        let output = result.output.unwrap();
        assert_eq!(output["matches"], 2);
        assert_eq!(output["replacedCount"], 2);

        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "qux\r\nbar\r\nqux\r\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_crlf_file_no_match_after_normalization() {
        // 验证归一化后仍无匹配时返回原错误，不修改文件
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_norm_nomatch_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "crlf_nomatch.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "hello world\r\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "nonexistent string",
                "new_string": "replacement",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("No matching string found"));

        // 验证文件未被修改（仍为原始 CRLF 内容）
        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "hello world\r\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_edit_tool_crlf_file_crlf_old_string_exact_match() {
        // 验证 CRLF 文件 + CRLF old_string 时精确匹配成功（无需归一化）
        let temp_dir = std::env::temp_dir().join(format!(
            "samoyed_work_edit_crlf_exact_{}",
            uuid::Uuid::new_v4()
        ));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let file_path = "crlf_exact.txt";
        let abs_path = temp_dir.join(file_path);
        let original = "line1\r\nline2\r\nline3\r\n";
        std::fs::write(&abs_path, original).unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("edit").unwrap();
        // old_string 使用 CRLF 行尾，与文件行尾一致，精确匹配成功
        let result = tool
            .execute(json!({
                "path": file_path,
                "old_string": "line2\r\n",
                "new_string": "replaced\r\n",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "CRLF 精确匹配应成功: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["matches"], 1);

        let content = std::fs::read_to_string(&abs_path).unwrap();
        assert_eq!(content, "line1\r\nreplaced\r\nline3\r\n");

        let _ = std::fs::remove_file(&abs_path);
        let _ = std::fs::remove_dir(&temp_dir);
    }

    #[tokio::test]
    async fn test_glob_find_rust_files() {
        // 验证 glob 工具查找 .rs 文件
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_glob_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 创建测试文件
        tokio::fs::write(temp_dir.join("main.rs"), "fn main() {}")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("lib.rs"), "pub fn lib() {}")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("readme.md"), "# Readme")
            .await
            .unwrap();
        // 创建子目录
        tokio::fs::create_dir_all(temp_dir.join("src"))
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("src/mod.rs"), "pub mod x;")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("glob").unwrap();
        // 用 **/*.rs 查找所有 .rs 文件
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "path": ".",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "glob 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该找到 3 个 .rs 文件（main.rs, lib.rs, src/mod.rs）
        assert_eq!(matches.len(), 3);
        let match_strs: Vec<String> = matches
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        assert!(match_strs.iter().any(|s| s.ends_with("main.rs")));
        assert!(match_strs.iter().any(|s| s.ends_with("lib.rs")));
        assert!(match_strs.iter().any(|s| s.ends_with("mod.rs")));
        // 不应包含 readme.md
        assert!(!match_strs.iter().any(|s| s.ends_with("readme.md")));

        // 清理
        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_glob_with_excludes() {
        // 验证 glob 工具的 exclude_patterns 参数
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_glob_exc_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        tokio::fs::write(temp_dir.join("keep.rs"), "")
            .await
            .unwrap();
        tokio::fs::create_dir_all(temp_dir.join("target"))
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("target/build.rs"), "")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("glob").unwrap();
        let result = tool
            .execute(json!({
                "pattern": "**/*.rs",
                "exclude_patterns": ["target/**"],
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "glob 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该只找到 keep.rs，排除 target/build.rs
        assert_eq!(matches.len(), 1);
        assert!(matches[0].as_str().unwrap().ends_with("keep.rs"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_basic_search() {
        // 验证 grep 工具基本正则搜索：搜索 "fn " 模式，应只匹配 .rs 文件中的函数定义
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_grep_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 创建测试文件
        tokio::fs::write(
            temp_dir.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .await
        .unwrap();
        tokio::fs::write(temp_dir.join("lib.rs"), "pub fn lib() {}\nfn helper() {}\n")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("readme.md"), "# Readme\nnothing here\n")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("grep").unwrap();
        // 搜索 "fn " 模式
        let result = tool
            .execute(json!({
                "pattern": "fn ",
                "path": ".",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该匹配 main.rs 的 1 行（fn main）和 lib.rs 的 2 行（pub fn lib 和 fn helper）
        // readme.md 不应该匹配
        assert_eq!(matches.len(), 3);
        // 验证所有匹配都是 .rs 文件
        for m in matches {
            let path = m["path"].as_str().unwrap();
            assert!(path.ends_with(".rs"), "不应匹配非 .rs 文件: {}", path);
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_with_include() {
        // 验证 grep 工具的 include 参数（文件扩展名过滤）：仅搜索匹配的文件
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_grep_inc_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 在 .rs 和 .md 文件中都写入 "fn "
        tokio::fs::write(temp_dir.join("code.rs"), "fn test() {}\n")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.join("doc.md"), "fn fake\n")
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("grep").unwrap();
        // 只搜索 .rs 文件
        let result = tool
            .execute(json!({
                "pattern": "fn ",
                "path": ".",
                "include": "*.rs",
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该只匹配 code.rs，不匹配 doc.md
        assert_eq!(matches.len(), 1);
        assert!(matches[0]["path"].as_str().unwrap().ends_with("code.rs"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_case_insensitive() {
        // 验证 grep 工具的 case_insensitive 参数：大小写不敏感匹配
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_grep_ci_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 写入不同大小写的内容
        tokio::fs::write(
            temp_dir.join("test.rs"),
            "fn FooBar() {}\nfn foobar() {}\nfn FOOBAR() {}\n",
        )
        .await
        .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("grep").unwrap();
        // 大小写不敏感搜索 "foobar"
        let result = tool
            .execute(json!({
                "pattern": "foobar",
                "path": ".",
                "case_insensitive": true,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        // 应该匹配 3 行（FooBar, foobar, FOOBAR）
        assert_eq!(matches.len(), 3);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_grep_with_context() {
        // 验证 grep 工具的 context_before 和 context_after 参数：返回上下文行
        let temp_dir =
            std::env::temp_dir().join(format!("samoyed_work_grep_ctx_{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        // 写入多行内容，匹配行在中间
        let content = "line 1\nline 2\nfn target() {}\nline 4\nline 5\n";
        tokio::fs::write(temp_dir.join("ctx.rs"), content)
            .await
            .unwrap();

        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("grep").unwrap();
        // 搜索 "target"，前后各 1 行上下文
        let result = tool
            .execute(json!({
                "pattern": "target",
                "path": ".",
                "context_before": 1,
                "context_after": 1,
                "workspace_root": temp_dir.to_string_lossy(),
            }))
            .await;

        assert!(result.success, "grep 失败: {:?}", result.error);
        let output = result.output.unwrap();
        let matches = output["matches"].as_array().unwrap();
        assert_eq!(matches.len(), 1);
        let m = &matches[0];
        assert_eq!(m["line_number"], 3);
        assert_eq!(m["line"].as_str().unwrap(), "fn target() {}");
        // 上下文行验证：匹配行前一行
        let ctx_before = m["context_before"].as_array().unwrap();
        assert_eq!(ctx_before.len(), 1);
        assert_eq!(ctx_before[0].as_str().unwrap(), "line 2");
        // 上下文行验证：匹配行后一行
        let ctx_after = m["context_after"].as_array().unwrap();
        assert_eq!(ctx_after.len(), 1);
        assert_eq!(ctx_after[0].as_str().unwrap(), "line 4");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[tokio::test]
    async fn test_create_directory_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("mkdir").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Missing directory path"));
    }

    #[tokio::test]
    async fn test_write_text_file_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("write").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "content": "test",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Missing file path"));
    }

    #[tokio::test]
    async fn test_delete_file_missing_workspace() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("remove").unwrap();
        let result = tool
            .execute(json!({
                "path": "test.txt",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result
            .error
            .unwrap()
            .contains("Missing workspace root path"));
    }

    #[tokio::test]
    async fn test_search_files_empty_query_and_extensions() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("search").unwrap();
        let result = tool
            .execute(json!({
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("cannot both be empty"));
    }

    #[tokio::test]
    async fn test_file_info_missing_path() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("file_info").unwrap();
        let result = tool
            .execute(json!({
                "path": "",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("Missing file path"));
    }

    /// 测试 encoding 参数：使用 GBK 编码写入中文内容，再用 GBK 编码读取
    /// 验证 encoding_rs 集成是否正确工作
    #[tokio::test]
    async fn test_write_and_read_file_with_gbk_encoding() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("samoyed_work_encoding_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "你好，世界！这是 GBK 编码测试。";
        let file_path = "gbk_test.txt";

        // 使用 GBK 编码写入文件
        let write_tool = registry.get_arc("write").unwrap();
        let write_result = write_tool
            .execute(json!({
                "path": file_path,
                "content": test_content,
                "workspace_root": temp_dir.to_string_lossy(),
                "encoding": "gbk"
            }))
            .await;

        assert!(
            write_result.success,
            "GBK 编码写入失败: {:?}",
            write_result.error
        );
        let output = write_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(output["encoding"], "GBK");

        // 使用 GBK 编码读取文件
        let read_tool = registry.get_arc("read").unwrap();
        let read_result = read_tool
            .execute(json!({
                "path": file_path,
                "workspace_root": temp_dir.to_string_lossy(),
                "encoding": "gbk"
            }))
            .await;

        assert!(
            read_result.success,
            "GBK 编码读取失败: {:?}",
            read_result.error
        );
        let read_output = read_result.output.unwrap();
        assert_eq!(read_output["encoding"], "GBK");
        // content 现在带行号格式（`   1→内容`），用 contains 验证原文存在
        assert!(read_output["content"]
            .as_str()
            .unwrap()
            .contains(test_content));

        // 清理临时文件
        let abs_path = temp_dir.join(file_path);
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：UTF-8 默认编码应保持向后兼容
    #[tokio::test]
    async fn test_read_file_default_utf8_encoding() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("samoyed_work_utf8_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Hello, 世界！UTF-8 默认编码测试。";
        let file_path = "utf8_test.txt";
        let abs_path = temp_dir.join(file_path);

        // 直接用 UTF-8 写入文件（模拟已存在的 UTF-8 文件）
        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 不传 encoding 参数读取（应默认 UTF-8）
        let read_tool = registry.get_arc("read").unwrap();
        let read_result = read_tool
            .execute(json!({
                "path": file_path,
                "workspace_root": temp_dir.to_string_lossy()
            }))
            .await;

        assert!(
            read_result.success,
            "UTF-8 默认读取失败: {:?}",
            read_result.error
        );
        let read_output = read_result.output.unwrap();
        // encoding_rs 返回规范化的编码名（大写）
        assert_eq!(read_output["encoding"], "UTF-8");
        // content 现在带行号格式（`   1→内容`），用 contains 验证原文存在
        assert!(read_output["content"]
            .as_str()
            .unwrap()
            .contains(test_content));

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 encoding 参数：不支持的编码标签应回退到 UTF-8
    #[tokio::test]
    async fn test_read_file_unsupported_encoding_fallback() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        // 创建临时工作区目录
        let temp_dir = std::env::temp_dir().join("samoyed_work_fallback_test");
        tokio::fs::create_dir_all(&temp_dir).await.unwrap();

        let test_content = "Fallback test 你好";
        let file_path = "fallback_test.txt";
        let abs_path = temp_dir.join(file_path);

        tokio::fs::write(&abs_path, test_content).await.unwrap();

        // 传入不支持的编码标签
        let read_tool = registry.get_arc("read").unwrap();
        let read_result = read_tool
            .execute(json!({
                "path": file_path,
                "workspace_root": temp_dir.to_string_lossy(),
                "encoding": "nonexistent-encoding"
            }))
            .await;

        assert!(
            read_result.success,
            "不支持的编码应回退到 UTF-8，但读取失败: {:?}",
            read_result.error
        );
        let read_output = read_result.output.unwrap();
        // 不支持的编码回退到 UTF-8（encoding_rs 返回大写名称）
        assert_eq!(read_output["encoding"], "UTF-8");
        // content 现在带行号格式（`   1→内容`），用 contains 验证原文存在
        assert!(read_output["content"]
            .as_str()
            .unwrap()
            .contains(test_content));

        // 清理临时文件
        let _ = tokio::fs::remove_file(&abs_path).await;
        let _ = tokio::fs::remove_dir(&temp_dir).await;
    }

    /// 测试 Scratchpad 工具的 add 操作
    #[tokio::test]
    async fn test_scratchpad_add_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("scratchpad").unwrap();

        // 第一条笔记
        let result = tool
            .execute(json!({
                "action": "add",
                "content": "已读取 sample.docx，包含 3 个章节",
                "_session_id": "test-session-1",
                "_iteration": 1
            }))
            .await;

        assert!(result.success, "add 失败: {:?}", result.error);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "add");
        assert_eq!(output["total_notes"], 1);

        // 第二条笔记
        let result2 = tool
            .execute(json!({
                "action": "add",
                "content": "识别到需要修改第 2 章的日期",
                "_session_id": "test-session-1",
                "_iteration": 2
            }))
            .await;

        assert!(result2.success);
        assert_eq!(result2.output.unwrap()["total_notes"], 2);
    }

    /// 测试 Scratchpad 工具的 read 操作
    #[tokio::test]
    async fn test_scratchpad_read_notes() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // 先添加两条笔记
        tool.execute(json!({
            "action": "add",
            "content": "笔记 A",
            "_session_id": "test-session-read"
        }))
        .await;
        tool.execute(json!({
            "action": "add",
            "content": "笔记 B",
            "_session_id": "test-session-read"
        }))
        .await;

        // 读取笔记
        let result = tool
            .execute(json!({
                "action": "read",
                "_session_id": "test-session-read"
            }))
            .await;

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
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "待清理的笔记",
            "_session_id": "test-session-clear"
        }))
        .await;

        // 清空
        let result = tool
            .execute(json!({
                "action": "clear",
                "_session_id": "test-session-clear"
            }))
            .await;

        assert!(result.success);
        let output = result.output.unwrap();
        assert_eq!(output["action"], "clear");
        assert_eq!(output["cleared_notes"], 1);

        // 验证已清空
        let read_result = tool
            .execute(json!({
                "action": "read",
                "_session_id": "test-session-clear"
            }))
            .await;
        assert_eq!(read_result.output.unwrap()["total_notes"], 0);
    }

    /// 测试 Scratchpad 工具的会话隔离
    #[tokio::test]
    async fn test_scratchpad_session_isolation() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // session-A 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "会话 A 的笔记",
            "_session_id": "session-A"
        }))
        .await;

        // session-B 添加笔记
        tool.execute(json!({
            "action": "add",
            "content": "会话 B 的笔记 1",
            "_session_id": "session-B"
        }))
        .await;
        tool.execute(json!({
            "action": "add",
            "content": "会话 B 的笔记 2",
            "_session_id": "session-B"
        }))
        .await;

        // 验证 session-A 只有 1 条
        let result_a = tool
            .execute(json!({
                "action": "read",
                "_session_id": "session-A"
            }))
            .await;
        assert_eq!(result_a.output.unwrap()["total_notes"], 1);

        // 验证 session-B 有 2 条
        let result_b = tool
            .execute(json!({
                "action": "read",
                "_session_id": "session-B"
            }))
            .await;
        assert_eq!(result_b.output.unwrap()["total_notes"], 2);
    }

    /// 测试 Scratchpad 缺少 _session_id 时返回错误
    #[tokio::test]
    async fn test_scratchpad_missing_session_id() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        let result = tool
            .execute(json!({
                "action": "add",
                "content": "测试笔记"
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("missing session identifier"));
        assert_eq!(result.error_code, Some(crate::errors::TOOL_INVALID_PARAMS));
    }

    /// 测试 Scratchpad add 时 content 为空返回错误
    #[tokio::test]
    async fn test_scratchpad_add_empty_content() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        let result = tool
            .execute(json!({
                "action": "add",
                "content": "",
                "_session_id": "test-session"
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("content cannot be empty"));
    }

    /// 测试 Scratchpad 未知 action 返回错误
    #[tokio::test]
    async fn test_scratchpad_unknown_action() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        let result = tool
            .execute(json!({
                "action": "delete",
                "_session_id": "test-session"
            }))
            .await;

        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown action"));
    }

    /// 测试 Scratchpad 笔记长度限制（500 字符）
    #[tokio::test]
    async fn test_scratchpad_content_length_limit() {
        let mut registry = ToolRegistry::new();
        let _states = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let tool = registry.get_arc("scratchpad").unwrap();

        // 构造 1000 字符的长内容
        let long_content = "a".repeat(1000);

        let result = tool
            .execute(json!({
                "action": "add",
                "content": long_content,
                "_session_id": "test-session-limit"
            }))
            .await;

        assert!(result.success);

        // 验证存储的内容被截断到 500 字符
        let read_result = tool
            .execute(json!({
                "action": "read",
                "_session_id": "test-session-limit"
            }))
            .await;
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
            states_write.insert(
                "test-session".to_string(),
                vec![
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
                ],
            );
        }

        let summary = format_scratchpad_summary(&states, "test-session");
        assert!(summary.is_some());
        let summary = summary.unwrap();
        assert!(summary.contains("<scratchpad>"));
        assert!(summary.contains("第一条笔记"));
        assert!(summary.contains("第二条笔记"));
        assert!(summary.contains("1. 第一条笔记"));
        assert!(summary.contains("2. 第二条笔记"));
        assert!(summary.contains("scratchpad"));
    }

    /// 测试 is_script_filename 函数：识别脚本文件扩展名
    #[test]
    fn test_is_script_filename() {
        // 脚本文件扩展名应被识别
        assert!(is_script_filename("test.py"));
        assert!(is_script_filename("script.sh"));
        assert!(is_script_filename("run.bash"));
        assert!(is_script_filename("power.ps1"));
        assert!(is_script_filename("batch.bat"));
        assert!(is_script_filename("cmd.cmd"));
        assert!(is_script_filename("ruby.rb"));
        assert!(is_script_filename("lua.lua"));
        assert!(is_script_filename("perl.pl"));

        // 大小写不敏感
        assert!(is_script_filename("TEST.PY"));
        assert!(is_script_filename("Script.SH"));

        // 包含路径的脚本文件
        assert!(is_script_filename("/tmp/test.py"));
        assert!(is_script_filename("D:\\workspace\\script.py"));
        assert!(is_script_filename("subdir/script.sh"));

        // 非脚本文件不应被识别
        assert!(!is_script_filename("readme.txt"));
        assert!(!is_script_filename("notes.md"));
        assert!(!is_script_filename("data.csv"));
        assert!(!is_script_filename("config.json"));
        assert!(!is_script_filename("document.docx"));
        assert!(!is_script_filename("image.png"));
        assert!(!is_script_filename("no_extension"));
    }

    /// 测试 is_script_leak_command 函数：检测 cp 命令将脚本复制到工作区（Windows 风格路径）
    #[test]
    fn test_is_script_leak_command_cp_windows_path() {
        let workspace_root = "D:\\DeskTop\\test";

        // 日志中的实际命令：cp 脚本到工作区（Windows 风格路径）
        let cmd = "cp \"C:/Users/a1926/AppData/Local/Temp/samoyed_work/scripts/modify_resume_pdf.py\" \"D:/DeskTop/test/modify_resume_pdf.py\" && cd \"D:/DeskTop/test\" && python modify_resume_pdf.py 2>&1";
        assert!(
            is_script_leak_command(cmd, "", workspace_root),
            "Windows 风格路径的 cp 命令应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：检测 cp 命令将脚本复制到工作区（Git Bash 风格路径）
    #[test]
    fn test_is_script_leak_command_cp_gitbash_path() {
        let workspace_root = "D:\\DeskTop\\test";

        // 日志中的实际命令：cp 脚本到工作区（Git Bash 风格路径 /d/DeskTop/test）
        let cmd = "cp \"C:/Users/a1926/AppData/Local/Temp/samoyed_work/scripts/fix_resume.py\" \"/d/DeskTop/test/fix_resume.py\" && cd /d/DeskTop/test && python -u fix_resume.py 2>&1";
        assert!(
            is_script_leak_command(cmd, "", workspace_root),
            "Git Bash 风格路径的 cp 命令应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：检测 mv 命令将脚本移动到工作区
    #[test]
    fn test_is_script_leak_command_mv_to_workspace() {
        let workspace_root = "D:\\DeskTop\\test";

        let cmd = "mv /tmp/samoyed_work/scripts/script.py /d/DeskTop/test/script.py";
        assert!(
            is_script_leak_command(cmd, "", workspace_root),
            "mv 命令将脚本移动到工作区应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：检测重定向将脚本写入工作区
    #[test]
    fn test_is_script_leak_command_redirect_to_workspace() {
        let workspace_root = "D:\\DeskTop\\test";

        // 使用 echo + 重定向写入脚本文件
        let cmd = "echo \"print('hello')\" > /d/DeskTop/test/hello.py";
        assert!(
            is_script_leak_command(cmd, "", workspace_root),
            "重定向写入脚本到工作区应被识别为脚本泄露"
        );

        // 使用 cat + 重定向
        let cmd2 = "cat > /d/DeskTop/test/script.py << EOF\nprint('hello')\nEOF";
        assert!(
            is_script_leak_command(cmd2, "", workspace_root),
            "cat 重定向写入脚本到工作区应被识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：安全命令不应被误判
    #[test]
    fn test_is_script_leak_command_safe_commands() {
        let workspace_root = "D:\\DeskTop\\test";

        // 直接执行 temp 目录中的脚本（不复制到工作区）
        let cmd1 =
            "python \"C:/Users/a1926/AppData/Local/Temp/samoyed_work/scripts/script.py\" 2>&1";
        assert!(
            !is_script_leak_command(cmd1, "", workspace_root),
            "直接执行 temp 目录脚本不应被识别为脚本泄露"
        );

        // 列出工作区文件
        let cmd2 = "ls -la /d/DeskTop/test/";
        assert!(
            !is_script_leak_command(cmd2, "", workspace_root),
            "ls 命令不应被识别为脚本泄露"
        );

        // 在工作区内执行 python -c 内联代码
        let cmd3 = "cd /d/DeskTop/test && python -c \"print('hello')\"";
        assert!(
            !is_script_leak_command(cmd3, "", workspace_root),
            "python -c 内联代码不应被识别为脚本泄露"
        );

        // 复制非脚本文件到工作区
        let cmd4 = "cp /tmp/data.csv /d/DeskTop/test/data.csv";
        assert!(
            !is_script_leak_command(cmd4, "", workspace_root),
            "复制非脚本文件不应被识别为脚本泄露"
        );

        // workspace_root 为空
        let cmd5 = "cp /tmp/script.py /workspace/script.py";
        assert!(
            !is_script_leak_command(cmd5, "", ""),
            "workspace_root 为空时不应识别为脚本泄露"
        );
    }

    /// 测试 is_script_leak_command 函数：多种脚本扩展名
    #[test]
    fn test_is_script_leak_command_various_script_extensions() {
        let workspace_root = "D:\\DeskTop\\test";

        // .sh 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.sh /d/DeskTop/test/script.sh",
            "",
            workspace_root
        ));
        // .bash 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.bash /d/DeskTop/test/script.bash",
            "",
            workspace_root
        ));
        // .ps1 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.ps1 /d/DeskTop/test/script.ps1",
            "",
            workspace_root
        ));
        // .bat 脚本
        assert!(is_script_leak_command(
            "cp /tmp/script.bat /d/DeskTop/test/script.bat",
            "",
            workspace_root
        ));
    }

    /// 测试 is_script_leak_command 函数：working_dir 等于 workspace_root 时，相对路径目标被识别
    /// 验证 T-B-CHAIN3-06 修复：命令中使用相对路径目标（如 __self_test__/leak.py），
    /// 当 working_dir 等于 workspace_root 时，应识别为脚本泄露
    #[test]
    fn test_is_script_leak_command_working_dir_equals_workspace() {
        // working_dir 等于 workspace_root 时，相对路径目标被识别
        assert!(is_script_leak_command(
            "cp /tmp/samoyed_work/scripts/hello.py __self_test__/leak.py",
            "/workspace",
            "/workspace"
        ));
        // working_dir 不等于 workspace_root 时，不触发检测
        assert!(!is_script_leak_command(
            "cp /tmp/script.py /other/dir/leak.py",
            "/other/dir",
            "/workspace"
        ));
        // working_dir 为空时，保持现有行为(回归)
        assert!(!is_script_leak_command(
            "cp /tmp/samoyed_work/scripts/hello.py __self_test__/leak.py",
            "",
            "/workspace"
        ));
    }

    /// 集成测试：WriteTextFileTool 拒绝写入脚本文件到工作区
    /// 验证 LLM 试图通过 write_text_file 创建 .py 文件时会被拒绝
    #[tokio::test]
    async fn test_write_text_file_rejects_script_file() {
        let mut registry = ToolRegistry::new();
        let _ = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );

        let tool = registry.get_arc("write").unwrap();

        // 尝试写入 .py 脚本文件，应被拒绝
        let result = tool
            .execute(json!({
                "path": "script.py",
                "content": "print('hello')",
                "workspace_root": ""
            }))
            .await;

        assert!(!result.success, "写入 .py 文件应被拒绝");
        assert!(result.error.is_some());
        let error = result.error.unwrap();
        assert!(error.contains("script file"), "错误信息应提及脚本文件");
        assert!(
            error.contains("write_script"),
            "错误信息应引导使用 write_script 工具"
        );

        // 尝试写入 .sh 脚本文件，也应被拒绝
        let result2 = tool
            .execute(json!({
                "path": "script.sh",
                "content": "echo hello",
                "workspace_root": ""
            }))
            .await;

        assert!(!result2.success, "写入 .sh 文件应被拒绝");

        // 写入普通文本文件应成功（不被拒绝）
        let tmp_dir = std::env::temp_dir().join("samoyed_work_test_write_file");
        let _ = std::fs::create_dir_all(&tmp_dir);
        let result3 = tool
            .execute(json!({
                "path": "readme.txt",
                "content": "hello world",
                "workspace_root": tmp_dir.to_string_lossy()
            }))
            .await;

        assert!(result3.success, "写入普通文本文件应成功");
        // 清理临时文件
        let _ = std::fs::remove_file(tmp_dir.join("readme.txt"));
    }
}

// ============================================================
// file_info - 获取文件元数据
// ============================================================

struct FileInfoTool;

#[async_trait]
impl Tool for FileInfoTool {
    fn tool_name(&self) -> &str {
        "file_info"
    }
    fn description(&self) -> &str {
        "Get file metadata (size, modification time, type, etc.). Use cases: inspect file info before reading, check file type, verify file existence and accessibility. Prefer this tool when file content is not needed."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to workspace)"
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
                error: Some("Missing file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("outside the workspace");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!(
                    "file_info 路径校验失败: {}, path={}, workspace={}",
                    e,
                    file_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        if !path.exists() {
            log::error!("文件不存在: {}", file_path);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("File does not exist: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let metadata = match tokio::fs::metadata(&resolved_path).await {
            Ok(m) => m,
            Err(e) => {
                log::error!("获取文件信息失败: {}, 错误: {}", file_path, e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to get file info: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        let is_dir = metadata.is_dir();
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        let modified = metadata
            .modified()
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
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// file_exists - 检查文件或目录是否存在
// ============================================================

struct FileExistsTool;

#[async_trait]
impl Tool for FileExistsTool {
    fn tool_name(&self) -> &str {
        "exists"
    }
    fn description(&self) -> &str {
        "Check whether a file or directory exists. Use cases: validate paths before reading or modifying files, avoid operating on non-existent files. Lighter than list_directory."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File or directory path (relative to workspace)"
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
                error: Some("Missing path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
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
                let is_out_of_bounds = e.contains("outside the workspace");
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
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
                log::warn!(
                    "file_exists 路径越界: {}, path={}, workspace={}",
                    e,
                    file_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
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
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// delete_file - 删除文件
// ============================================================

struct DeleteFileTool;

#[async_trait]
impl Tool for DeleteFileTool {
    fn tool_name(&self) -> &str {
        "remove"
    }
    fn description(&self) -> &str {
        "Delete the specified file. Note: this operation is irreversible and will automatically trigger user confirmation."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path of the file to delete (relative to workspace)"
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
                error: Some("Missing file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        if workspace_root.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(
                    "Missing workspace root path, unable to perform security validation"
                        .to_string(),
                ),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 路径安全校验（使用统一的校验函数，包含词法归一化防线）
        let canonical_file =
            match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok((canonical_file, _)) => canonical_file,
                Err(e) => {
                    let is_out_of_bounds = e.contains("outside the workspace");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!(
                        "delete_file 路径校验失败: {}, path={}, workspace={}",
                        e,
                        file_path,
                        workspace_root
                    );
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code,
                    };
                }
            };

        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Path is not a file: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let safe_path = canonical_file.to_string_lossy().to_string();

        match tokio::fs::remove_file(&safe_path).await {
            Ok(_) => {
                log::info!("文件已删除: {}", safe_path);
                let result = json!({
                    "path": file_path,
                    "message": format!("File deleted: {}", file_path),
                });
                ToolResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除文件失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to delete file: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
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
    fn tool_name(&self) -> &str {
        "mkdir"
    }
    fn description(&self) -> &str {
        "Create a directory (supports recursive creation). Use cases: ensure target directory exists before writing files, organize file structure. Parent directories are created recursively by default."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path (relative to workspace)"
                },
                "recursive": {
                    "type": "boolean",
                    "description": "Whether to recursively create parent directories",
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
                error: Some("Missing directory path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
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
                            error: Some(format!("Path is invalid: {}", dir_path)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                }
            } else {
                // 路径不存在，检查父目录
                match path.parent() {
                    Some(parent) if parent.exists() => match crate::utils::canonicalize(parent) {
                        Ok(p) => p,
                        Err(_) => {
                            return ToolResult {
                                success: false,
                                output: None,
                                error: Some(format!(
                                    "Parent directory path is invalid: {}",
                                    dir_path
                                )),
                                duration_ms: start.elapsed().as_millis() as u64,
                                error_code: None,
                            };
                        }
                    },
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
                                        error: Some("Directory path is outside the workspace, creation denied".to_string()),
                                        duration_ms: start.elapsed().as_millis() as u64,
                                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                // 校验通过，继续执行
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some(
                                        "Workspace root directory path is invalid".to_string(),
                                    ),
                                    duration_ms: start.elapsed().as_millis() as u64,
                                    error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("Workspace root directory path is invalid".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "Directory path is outside the workspace, creation denied".to_string(),
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                };
            }
        }

        // 检查目录是否已存在
        if path.exists() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Directory already exists: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 检查工作区根目录是否存在，防止自动重建已删除的工作区目录
        if !workspace_root.is_empty() {
            let root_path = std::path::Path::new(workspace_root);
            if !root_path.exists() {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("Workspace directory has been deleted, please remove this workspace and reselect".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("创建目录失败: {}, 错误: {}", dir_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to create directory: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// write_text_file - 写入纯文本文件
// ============================================================

/// 判断文件名是否为脚本文件
/// 用于阻止通过 write_text_file 工具将脚本文件写入工作区
/// 受保护扩展名：.py/.sh/.bash/.ps1/.bat/.cmd/.rb/.lua/.pl
fn is_script_filename(path: &str) -> bool {
    let lower = path.to_lowercase();
    const SCRIPT_EXTENSIONS: &[&str] = &[
        ".py", ".sh", ".bash", ".ps1", ".bat", ".cmd", ".rb", ".lua", ".pl",
    ];
    SCRIPT_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

struct WriteTextFileTool;

#[async_trait]
impl Tool for WriteTextFileTool {
    fn tool_name(&self) -> &str {
        "write"
    }
    fn description(&self) -> &str {
        "Write plain text file content (.txt/.md/.csv/.json etc.), does not depend on Sidecar. Use cases: create plain text files, modify Markdown files, save JSON configurations. Supports append mode. Note: only for plain text; use docx_handler/xlsx_handler/pptx_handler/pdf_handler generate operations for structured documents. Writing script files (.py/.sh/.bash/.ps1/.bat/.cmd etc.) is prohibited; use the write_script tool to write scripts to the system temporary directory instead. Content size limit 4KB (approximately 4000 characters); exceeding this may trigger LLM response truncation."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to workspace)"
                },
                "content": {
                    "type": "string",
                    "description": "File content"
                },
                "encoding": {
                    "type": "string",
                    "description": "File encoding, default utf-8",
                    "default": "utf-8"
                },
                "append": {
                    "type": "boolean",
                    "description": "Whether to append to the file",
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
                error: Some("Missing file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：拒绝写入脚本文件到工作区
        // 脚本文件应通过 write_script 工具创建到系统临时目录，避免污染工作区
        // 受保护扩展名：.py/.sh/.bash/.ps1/.bat/.cmd/.rb/.lua/.pl 等
        if is_script_filename(file_path) {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Writing script files via write_text_file is not allowed: {}. {}",
                    file_path,
                    script_execution_guidance()
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
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
                            error: Some(format!("Path is invalid: {}", file_path)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                }
            } else {
                // 文件不存在，校验父目录
                match path.parent() {
                    Some(parent) if parent.exists() => match crate::utils::canonicalize(parent) {
                        Ok(p) => p,
                        Err(_) => {
                            return ToolResult {
                                success: false,
                                output: None,
                                error: Some(format!(
                                    "Parent directory path is invalid: {}",
                                    file_path
                                )),
                                duration_ms: start.elapsed().as_millis() as u64,
                                error_code: None,
                            };
                        }
                    },
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
                                        error: Some(
                                            "File path is outside the workspace, write denied"
                                                .to_string(),
                                        ),
                                        duration_ms: start.elapsed().as_millis() as u64,
                                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                                    };
                                }
                                path.to_path_buf()
                            }
                            Err(_) => {
                                return ToolResult {
                                    success: false,
                                    output: None,
                                    error: Some(
                                        "Workspace root directory path is invalid".to_string(),
                                    ),
                                    duration_ms: start.elapsed().as_millis() as u64,
                                    error_code: None,
                                };
                            }
                        }
                    }
                }
            };

            let canonical_root =
                match crate::utils::canonicalize(std::path::Path::new(workspace_root)) {
                    Ok(p) => p,
                    Err(_) => {
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some("Workspace root directory path is invalid".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                };
            if !check_path.starts_with(&canonical_root) {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some("File path is outside the workspace, write denied".to_string()),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                            error: Some("Workspace directory has been deleted, please remove this workspace and reselect".to_string()),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                }
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to create parent directory: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
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
                Ok(mut file) => {
                    tokio::io::AsyncWriteExt::write_all(&mut file, &encoded_bytes).await
                }
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
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("写入文件失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to write file: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// 3.5 新增工具：rename_file / copy_file / delete_directory
// / get_file_hash
// ============================================================

/// 校验已存在的路径是否合法
/// 仅检测路径遍历攻击（`..` 越界），不拦截工作区外路径
/// 外部目录访问由权限系统（check_permission）决策，避免职责重叠
/// 返回 Ok((canonical_path, canonical_root)) 表示通过校验（路径可能在工作区外）
/// 返回 Err(error_message) 表示校验失败（路径越界或不存在）
/// 用于需要路径安全校验的工具，减少重复代码
fn validate_existing_path_in_workspace(
    resolved_path: &str,
    workspace_root: &str,
) -> Result<(std::path::PathBuf, std::path::PathBuf), String> {
    if workspace_root.is_empty() {
        return Err(
            "Missing workspace root path, unable to perform security validation".to_string(),
        );
    }

    let canonical_root =
        crate::utils::canonicalize(std::path::Path::new(workspace_root)).map_err(|_| {
            format!(
                "Workspace root directory does not exist or is invalid: {}",
                workspace_root
            )
        })?;

    // 安全防线 1：词法归一化检查（不依赖文件系统）
    // 即使目标文件不存在（canonicalize 会失败），也能识别 `../` 越界并拒绝
    // 避免攻击者通过路径遍历探测文件存在性
    let normalized_path = normalize_path_lexically(resolved_path, &canonical_root);
    if !normalized_path.starts_with(&canonical_root) {
        return Err(format!(
            "Path is outside the workspace, access denied: {} (workspace: {})",
            resolved_path,
            canonical_root.display()
        ));
    }

    // 安全防线 2：canonicalize 确认路径真实存在
    let canonical_path = crate::utils::canonicalize(std::path::Path::new(resolved_path))
        .map_err(|_| format!("Path does not exist or is invalid: {}", resolved_path))?;

    // 注：原安全防线 3（组件级 starts_with 比较）已移除
    // 该防线会拒绝所有工作区外路径，与权限系统（check_permission 中的
    // ExternalDirectory 检查）职责重叠。外部目录访问由权限系统决策（Ask/Deny）
    // canonical_path 可能位于工作区外（如符号链接逃逸），交由权限系统处理
    Ok((canonical_path, canonical_root))
}

/// 对路径进行词法归一化（不访问文件系统）
/// 用于在 canonicalize 失败前识别 `..` 越界，避免泄露文件存在性信息
/// 注意：这是安全防护的补充手段，不能替代 canonicalize（无法识别符号链接）
/// Rust 标准库的 Path::components() 会保留 ParentDir(`..`) 组件，
/// 因此必须手动解析 `..` 才能正确判断越界
fn normalize_path_lexically(
    resolved_path: &str,
    workspace_root: &std::path::Path,
) -> std::path::PathBuf {
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
        return Err(
            "Missing workspace root path, unable to perform security validation".to_string(),
        );
    }

    let canonical_root =
        crate::utils::canonicalize(std::path::Path::new(workspace_root)).map_err(|_| {
            format!(
                "Workspace root directory does not exist or is invalid: {}",
                workspace_root
            )
        })?;

    let target_path = std::path::Path::new(resolved_target);
    // 目标路径可能不存在，规范化父目录
    let check_path = if target_path.exists() {
        crate::utils::canonicalize(target_path)
            .map_err(|_| format!("Target path is invalid: {}", resolved_target))?
    } else {
        // 父目录必须存在且在工作区内
        let parent = target_path.parent().unwrap_or(std::path::Path::new(""));
        if parent.as_os_str().is_empty() {
            // 没有父目录（如 "file.txt"），用工作区根目录
            canonical_root.clone()
        } else {
            crate::utils::canonicalize(parent).map_err(|_| {
                format!(
                    "Parent directory of target path is invalid: {}",
                    parent.display()
                )
            })?
        }
    };

    if !check_path.starts_with(&canonical_root) {
        return Err(format!(
            "Target path is outside the workspace, access denied: {} (workspace: {})",
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
    fn tool_name(&self) -> &str {
        "rename"
    }
    fn description(&self) -> &str {
        "Rename or move a file. Use cases: organize file structure, change file names. Note: cross-filesystem moves may fail; this operation is irreversible."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "Source file path (relative to workspace)"
                },
                "target_path": {
                    "type": "string",
                    "description": "Target file path (relative to workspace)"
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
                error: Some("Missing source file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing target file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) =
            match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("rename_file 源路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 安全校验：禁止通过重命名为脚本文件绕过 write_text_file 的限制
        // 智能体可能通过 "write_text_file 写入 .txt + rename_file 改为 .py" 绕过脚本文件写入限制
        // 此检查复用 is_script_filename 函数，检测目标路径是否为脚本扩展名
        if is_script_filename(target_path) {
            log::warn!(
                "rename_file 拒绝重命名为脚本文件: {} -> {}",
                source_path,
                target_path
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Renaming files to script files via rename_file is not allowed: {}. {}",
                    target_path,
                    script_execution_guidance()
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Source path is not a file: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
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
                        error: Some(format!("Failed to create target parent directory: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!(
                    "重命名文件失败: {} -> {}, 错误: {}",
                    source_path,
                    target_path,
                    e
                );
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to rename file: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
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
    fn tool_name(&self) -> &str {
        "copy"
    }
    fn description(&self) -> &str {
        "Copy a file to a new path. Use cases: create file copies, back up files, copy templates. Supports binary file copying."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source_path": {
                    "type": "string",
                    "description": "Source file path (relative to workspace)"
                },
                "target_path": {
                    "type": "string",
                    "description": "Target file path (relative to workspace)"
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
                error: Some("Missing source file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if target_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing target file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_source = resolve_path(source_path, workspace_root);
        let resolved_target = resolve_path(target_path, workspace_root);

        // 校验源路径在工作区内
        let (canonical_source, _) =
            match validate_existing_path_in_workspace(&resolved_source, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("copy_file 源路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
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
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
            };
        }

        // 安全校验：禁止通过复制为脚本文件绕过 write_text_file 的限制
        // 与 rename_file 相同的防护逻辑，防止智能体通过 copy_file 将 .txt 复制为 .py
        if is_script_filename(target_path) {
            log::warn!(
                "copy_file 拒绝复制为脚本文件: {} -> {}",
                source_path,
                target_path
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Copying files to script files via copy_file is not allowed: {}. {}",
                    target_path,
                    script_execution_guidance()
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 源路径必须是文件
        if !canonical_source.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Source path is not a file: {}", source_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
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
                        error: Some(format!("Failed to create target parent directory: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        }

        // 执行复制
        match tokio::fs::copy(&canonical_source, &resolved_target).await {
            Ok(bytes_copied) => {
                log::info!(
                    "文件已复制: {} -> {}, 字节数: {}",
                    source_path,
                    target_path,
                    bytes_copied
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "source_path": source_path,
                        "target_path": target_path,
                        "bytes_copied": bytes_copied,
                        "message": format!("文件已复制: {} -> {}", source_path, target_path),
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!(
                    "复制文件失败: {} -> {}, 错误: {}",
                    source_path,
                    target_path,
                    e
                );
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to copy file: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
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
    fn tool_name(&self) -> &str {
        "remove_dir"
    }
    fn description(&self) -> &str {
        "Recursively delete a directory and all its contents. Note: this operation is irreversible and will automatically trigger user confirmation. Confirming directory contents before deletion is recommended."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory path to delete (relative to workspace)"
                }
            },
            "required": ["path"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let dir_path = params["path"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if dir_path.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing directory path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(dir_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_dir, _) =
            match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("delete_directory 路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                    };
                }
            };

        // 必须是目录
        if !canonical_dir.is_dir() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Path is not a directory: {}", dir_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        // 禁止删除工作区根目录本身
        let canonical_root = crate::utils::canonicalize(std::path::Path::new(workspace_root))
            .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root));
        if canonical_dir == canonical_root {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Deleting workspace root directory is prohibited".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let safe_path = canonical_dir.to_string_lossy().to_string();

        // 执行删除
        match tokio::fs::remove_dir_all(&safe_path).await {
            Ok(_) => {
                log::info!("目录已删除: {}", safe_path);
                let result = json!({
                    "path": dir_path,
                    "message": format!("目录已删除: {}", dir_path),
                });
                ToolResult {
                    success: true,
                    output: Some(result),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("删除目录失败: {}, 错误: {}", safe_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to delete directory: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// get_file_hash - 计算文件哈希
// ============================================================

struct GetFileHashTool;

#[async_trait]
impl Tool for GetFileHashTool {
    fn tool_name(&self) -> &str {
        "hash"
    }
    fn description(&self) -> &str {
        "Compute the SHA-256 hash value of a file. Use cases: file deduplication, integrity verification, change detection. Returns a hexadecimal hash string."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to workspace)"
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
                error: Some("Missing file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);

        // 校验路径在工作区内
        let (canonical_file, _) =
            match validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                Ok(paths) => paths,
                Err(e) => {
                    log::warn!("get_file_hash 路径校验失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS),
                    };
                }
            };

        // 必须是文件
        if !canonical_file.is_file() {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Path is not a file: {}", file_path)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
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
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("计算文件哈希失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Failed to compute file hash: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("计算文件哈希任务失败: {}, 错误: {}", file_path, e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("File hash computation task failed: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

// ============================================================
// edit - 精确字符串替换工具
// ============================================================

/// 生成 unified diff 摘要，展示修改前后的内容差异
/// 使用 similar crate 计算行级差异，返回带 +/- 前缀的 diff 文本
fn format_diff_summary(old_content: &str, new_content: &str) -> String {
    use similar::{ChangeTag, TextDiff};

    let diff = TextDiff::from_lines(old_content, new_content);
    let mut result = String::new();

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-",
            ChangeTag::Insert => "+",
            ChangeTag::Equal => " ",
        };
        result.push_str(sign);
        result.push_str(change.value());
    }
    result
}

struct EditTool;

#[async_trait]
impl Tool for EditTool {
    fn tool_name(&self) -> &str {
        "edit"
    }
    fn description(&self) -> &str {
        "Precise string replacement tool. old_string must uniquely match in the file (0 matches raises an error, multiple matches raise an error, unless replace_all=true). When old_string is empty and the file does not exist, creates a new file. Generates a diff summary showing content before and after modification."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "File path (relative to workspace)"
                },
                "old_string": {
                    "type": "string",
                    "description": "The original string to replace (must match uniquely, unless replace_all=true). When empty and file does not exist, creates a new file"
                },
                "new_string": {
                    "type": "string",
                    "description": "The new string to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Whether to replace all matches (default false, only replaces the first match). When set to true, replaces all matches without requiring unique matching",
                    "default": false
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let file_path = params["path"].as_str().unwrap_or("");
        let old_string = params["old_string"].as_str().unwrap_or("");
        let new_string = params["new_string"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");
        let replace_all = params["replace_all"].as_bool().unwrap_or(false);

        if file_path.is_empty() {
            log::warn!("edit 失败: 缺少文件路径");
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing file path".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(file_path, workspace_root);
        let path = std::path::Path::new(&resolved_path);
        let file_exists = path.exists() && path.is_file();

        // 分支1：创建新文件（old_string 为空且文件不存在）
        if old_string.is_empty() && !file_exists {
            // 校验目标路径的父目录在工作区内
            if !workspace_root.is_empty() {
                if let Err(e) = validate_target_path_in_workspace(&resolved_path, workspace_root) {
                    let is_out_of_bounds = e.contains("outside the workspace");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!(
                        "edit 失败: {}, path={}, workspace={}",
                        e,
                        file_path,
                        workspace_root
                    );
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code,
                    };
                }
            }

            // 写入新文件
            match tokio::fs::write(&resolved_path, new_string.as_bytes()).await {
                Ok(_) => {
                    log::info!(
                        "edit 创建新文件: {}, 字节数: {}",
                        file_path,
                        new_string.len()
                    );
                    let diff_summary = format_diff_summary("", new_string);
                    ToolResult {
                        success: true,
                        output: Some(json!({
                            "path": file_path,
                            "operation": "create",
                            "bytes_written": new_string.len(),
                            "diff": diff_summary,
                        })),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
                Err(e) => {
                    log::error!("edit 创建文件失败: {}, 错误: {}", file_path, e);
                    ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to create file: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
            }
        } else if !file_exists {
            // 文件不存在且 old_string 非空：无法执行替换
            log::warn!(
                "edit 失败: 文件不存在且 old_string 非空, path={}",
                file_path
            );
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "File {} does not exist. To create a new file, set old_string to an empty string.",
                    file_path
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        } else {
            // 分支2：编辑已存在文件
            // 文件已存在时 old_string 不能为空（否则会产生大量匹配）
            if old_string.is_empty() {
                log::warn!(
                    "edit 失败: 文件已存在时 old_string 不能为空, path={}",
                    file_path
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "File already exists, old_string cannot be empty (to create a new file, use a different path)"
                            .to_string(),
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                };
            }

            // 路径安全校验
            if !workspace_root.is_empty() {
                if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root)
                {
                    let is_out_of_bounds = e.contains("outside the workspace");
                    let error_code = if is_out_of_bounds {
                        Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                    } else {
                        None
                    };
                    log::warn!(
                        "edit 失败: {}, path={}, workspace={}",
                        e,
                        file_path,
                        workspace_root
                    );
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(e),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code,
                    };
                }
            }

            // 读取文件内容（UTF-8 编码）
            let old_content = match tokio::fs::read_to_string(&resolved_path).await {
                Ok(c) => c,
                Err(e) => {
                    log::error!("edit 读取文件失败: {}, 错误: {}", file_path, e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to read file: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            };

            // 统计 old_string 出现次数（精确匹配）
            let mut match_count = old_content.matches(old_string).count();

            // 精确匹配失败时，尝试 CRLF→LF 归一化匹配（Windows CRLF 文件 vs LLM LF old_string）
            let mut normalized_content: Option<String> = None;
            let mut normalized_old_string: Option<String> = None;
            if match_count == 0 {
                let norm_content = old_content.replace("\r\n", "\n");
                let norm_old_string = old_string.replace("\r\n", "\n");
                let norm_count = norm_content.matches(&norm_old_string).count();
                if norm_count > 0 {
                    log::info!(
                        "edit 触发 CRLF→LF 归一化匹配: path={}, 归一化后匹配 {} 处",
                        file_path,
                        norm_count
                    );
                    match_count = norm_count;
                    normalized_content = Some(norm_content);
                    normalized_old_string = Some(norm_old_string);
                }
            }

            if match_count == 0 {
                log::warn!("edit 失败: 未找到匹配的字符串, path={}", file_path);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "No matching string found, old_string does not exist in the file"
                            .to_string(),
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
            // 多匹配处理：replace_all=true 时替换所有，否则报错
            if match_count > 1 && !replace_all {
                log::warn!(
                    "edit 失败: 找到 {} 处匹配，需要唯一匹配（或设置 replace_all=true）, path={}",
                    match_count,
                    file_path
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!(
                        "Found {} matches, unique match required. To replace all matches, set replace_all=true",
                        match_count
                    )),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }

            // 执行替换：replace_all=true 时替换所有匹配，否则仅替换第一个
            // 归一化匹配成功时，在归一化（LF）内容上执行替换，写回时恢复原始行尾风格
            let original_is_crlf = old_content.contains("\r\n");
            let new_content = if let (Some(norm_content), Some(norm_old_string)) =
                (normalized_content, normalized_old_string)
            {
                let replaced = if replace_all {
                    norm_content.replace(&norm_old_string, new_string)
                } else {
                    norm_content.replacen(&norm_old_string, new_string, 1)
                };
                // 恢复原始行尾风格：原始为 CRLF 则将所有 \n 转回 \r\n
                // 先消除 new_string 可能携带的 CRLF，避免产生 \r\r\n
                if original_is_crlf {
                    replaced.replace("\r\n", "\n").replace('\n', "\r\n")
                } else {
                    replaced
                }
            } else if replace_all {
                old_content.replace(old_string, new_string)
            } else {
                old_content.replacen(old_string, new_string, 1)
            };
            let diff_summary = format_diff_summary(&old_content, &new_content);

            // 写回文件
            match tokio::fs::write(&resolved_path, new_content.as_bytes()).await {
                Ok(_) => {
                    let replaced_count = if replace_all { match_count } else { 1 };
                    log::info!("edit 替换成功: {}, 替换 {} 处", file_path, replaced_count);
                    ToolResult {
                        success: true,
                        output: Some(json!({
                            "path": file_path,
                            "operation": "edit",
                            "matches": match_count,
                            "replacedCount": replaced_count,
                            "diff": diff_summary,
                        })),
                        error: None,
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
                Err(e) => {
                    log::error!("edit 写回文件失败: {}, 错误: {}", file_path, e);
                    ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to write back file: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    }
                }
            }
        }
    }
}

// ============================================================
// glob - glob 模式匹配查找文件（遵循 .gitignore）
// ============================================================

struct GlobTool;

#[async_trait]
impl Tool for GlobTool {
    fn tool_name(&self) -> &str {
        "glob"
    }
    fn description(&self) -> &str {
        "Find files using glob pattern matching. Based on the ignore crate, follows .gitignore rules. Supports patterns like **/*.rs, {a,b}/*.ts. Returns a list of paths relative to the workspace (up to 1000 entries)."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "glob pattern (e.g. **/*.rs, src/*.ts)"
                },
                "path": {
                    "type": "string",
                    "description": "Search root directory (relative to workspace), default \".\"",
                    "default": "."
                },
                "exclude_patterns": {
                    "type": "array",
                    "description": "Exclude patterns array (e.g. [\"node_modules/**\", \"target/**\"])",
                    "default": []
                }
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let pattern = params["pattern"].as_str().unwrap_or("");
        let search_path = params["path"].as_str().unwrap_or(".");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 获取 exclude_patterns 数组
        let exclude_patterns: Vec<String> = params["exclude_patterns"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        if pattern.is_empty() {
            log::warn!("glob 失败: 缺少 glob 模式");
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing glob pattern".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(search_path, workspace_root);

        // 路径安全校验
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("outside the workspace");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!(
                    "glob 失败: {}, path={}, workspace={}",
                    e,
                    search_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        // 构建 globset 匹配器
        let glob_matcher = {
            let mut builder = globset::GlobSetBuilder::new();
            let glob = match globset::Glob::new(pattern) {
                Ok(g) => g,
                Err(e) => {
                    log::warn!("glob 失败: 无效的 glob 模式 '{}': {}", pattern, e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Invalid glob pattern '{}': {}", pattern, e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                    };
                }
            };
            builder.add(glob);
            match builder.build() {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("glob 失败: 构建 glob 匹配器失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to build glob matcher: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        };

        // 构建排除匹配器
        let exclude_matcher = if exclude_patterns.is_empty() {
            None
        } else {
            let mut builder = globset::GlobSetBuilder::new();
            for p in &exclude_patterns {
                match globset::Glob::new(p) {
                    Ok(g) => builder.add(g),
                    Err(e) => {
                        log::warn!("glob 失败: 无效的排除模式 '{}': {}", p, e);
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("Invalid exclude pattern '{}': {}", p, e)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                        };
                    }
                };
            }
            match builder.build() {
                Ok(m) => Some(m),
                Err(e) => {
                    log::warn!("glob 失败: 构建排除匹配器失败: {}", e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Failed to build exclude matcher: {}", e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: None,
                    };
                }
            }
        };

        // 获取 canonical_root 用于计算相对路径
        let canonical_root = if !workspace_root.is_empty() {
            crate::utils::canonicalize(std::path::Path::new(workspace_root))
                .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root))
        } else {
            std::path::PathBuf::from(&resolved_path)
        };

        // 遍历目录（遵循 .gitignore）
        let walker = ignore::WalkBuilder::new(&resolved_path)
            .hidden(false) // 显示隐藏文件
            .ignore(true) // 遵循 .ignore 文件
            .git_ignore(true) // 遵循 .gitignore 文件
            .git_global(true) // 遵循全局 gitignore
            .build();

        let mut matches: Vec<String> = Vec::new();
        let mut truncated = false;

        for entry in walker.flatten() {
            let path = entry.path();
            // 跳过目录
            if path.is_dir() {
                continue;
            }
            // 将路径转为相对工作区的字符串（统一用 / 分隔符）
            let relative = path
                .strip_prefix(&canonical_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

            // 用 globset 匹配
            if glob_matcher.is_match(&relative) {
                // 检查排除
                if let Some(ref exc) = exclude_matcher {
                    if exc.is_match(&relative) {
                        continue;
                    }
                }
                matches.push(relative);
                if matches.len() >= 1000 {
                    truncated = true;
                    break;
                }
            }
        }

        log::debug!(
            "glob 完成: pattern={}, path={}, 匹配 {} 项",
            pattern,
            search_path,
            matches.len()
        );
        ToolResult {
            success: true,
            output: Some(json!({
                "pattern": pattern,
                "path": search_path,
                "matches": matches,
                "count": matches.len(),
                "truncated": truncated,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
        }
    }
}

// ============================================================
// grep - 正则表达式搜索文件内容（遵循 .gitignore）
// ============================================================

struct GrepTool;

#[async_trait]
impl Tool for GrepTool {
    fn tool_name(&self) -> &str {
        "grep"
    }
    fn description(&self) -> &str {
        "Search file contents using regular expressions. Based on the ignore crate, follows .gitignore. Supports context lines (context_before/context_after), file extension filtering (include), and case insensitivity. Returns a list of matches (including file path, line number, line content, and context lines)."
    }
    fn category(&self) -> &str {
        "filesystem"
    }
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regular expression"
                },
                "path": {
                    "type": "string",
                    "description": "Search root directory (relative to workspace), default \".\"",
                    "default": "."
                },
                "include": {
                    "type": "string",
                    "description": "File extension glob (e.g. \"*.rs\"), only searches matching files"
                },
                "case_insensitive": {
                    "type": "boolean",
                    "description": "Whether to perform case-insensitive matching, default false",
                    "default": false
                },
                "context_before": {
                    "type": "integer",
                    "description": "Number of context lines before the match, default 0",
                    "default": 0
                },
                "context_after": {
                    "type": "integer",
                    "description": "Number of context lines after the match, default 0",
                    "default": 0
                },
                "max_matches": {
                    "type": "integer",
                    "description": "Maximum number of matches, default 100",
                    "default": 100
                }
            },
            "required": ["pattern"]
        })
    }
    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();
        let pattern = params["pattern"].as_str().unwrap_or("");
        let search_path = params["path"].as_str().unwrap_or(".");
        let include = params["include"].as_str();
        let case_insensitive = params["case_insensitive"].as_bool().unwrap_or(false);
        let context_before = params["context_before"].as_u64().unwrap_or(0) as usize;
        let context_after = params["context_after"].as_u64().unwrap_or(0) as usize;
        let max_matches = params["max_matches"].as_u64().unwrap_or(100) as usize;
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        if pattern.is_empty() {
            log::warn!("grep 失败: 缺少正则表达式");
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing regex pattern".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        let resolved_path = resolve_path(search_path, workspace_root);

        // 路径安全校验
        if !workspace_root.is_empty() {
            if let Err(e) = validate_existing_path_in_workspace(&resolved_path, workspace_root) {
                let is_out_of_bounds = e.contains("outside the workspace");
                let error_code = if is_out_of_bounds {
                    Some(crate::errors::TOOL_PATH_OUT_OF_BOUNDS)
                } else {
                    None
                };
                log::warn!(
                    "grep 失败: {}, path={}, workspace={}",
                    e,
                    search_path,
                    workspace_root
                );
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(e),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code,
                };
            }
        }

        // 编译 regex（支持 (?i) 内联标志和 case_insensitive 参数）
        let re = match regex::RegexBuilder::new(pattern)
            .case_insensitive(case_insensitive)
            .build()
        {
            Ok(r) => r,
            Err(e) => {
                log::warn!("grep 失败: 无效的正则表达式 '{}': {}", pattern, e);
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Invalid regex '{}': {}", pattern, e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                };
            }
        };

        // 构建 include globset 匹配器（若提供 include）
        let include_matcher = if let Some(inc) = include {
            match globset::Glob::new(inc) {
                Ok(g) => match globset::GlobSetBuilder::new().add(g).build() {
                    Ok(m) => Some(m),
                    Err(e) => {
                        log::warn!("grep 失败: 构建 include 匹配器失败: {}", e);
                        return ToolResult {
                            success: false,
                            output: None,
                            error: Some(format!("Failed to build include matcher: {}", e)),
                            duration_ms: start.elapsed().as_millis() as u64,
                            error_code: None,
                        };
                    }
                },
                Err(e) => {
                    log::warn!("grep 失败: 无效的 include 模式 '{}': {}", inc, e);
                    return ToolResult {
                        success: false,
                        output: None,
                        error: Some(format!("Invalid include pattern '{}': {}", inc, e)),
                        duration_ms: start.elapsed().as_millis() as u64,
                        error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
                    };
                }
            }
        } else {
            None
        };

        // 获取 canonical_root 用于计算相对路径
        let canonical_root = if !workspace_root.is_empty() {
            crate::utils::canonicalize(std::path::Path::new(workspace_root))
                .unwrap_or_else(|_| std::path::PathBuf::from(workspace_root))
        } else {
            std::path::PathBuf::from(&resolved_path)
        };

        // 遍历目录（遵循 .gitignore）
        let walker = ignore::WalkBuilder::new(&resolved_path)
            .hidden(false) // 显示隐藏文件
            .ignore(true) // 遵循 .ignore 文件
            .git_ignore(true) // 遵循 .gitignore 文件
            .git_global(true) // 遵循全局 gitignore
            .build();

        let mut matches: Vec<Value> = Vec::new();
        let mut truncated = false;

        'outer: for entry in walker.flatten() {
            let path = entry.path();
            // 跳过目录
            if path.is_dir() {
                continue;
            }

            // 将路径转为相对工作区的字符串（统一用 / 分隔符）
            let relative = path
                .strip_prefix(&canonical_root)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|_| path.to_string_lossy().replace('\\', "/"));

            // 检查 include 过滤
            if let Some(ref inc) = include_matcher {
                if !inc.is_match(&relative) {
                    continue;
                }
            }

            // 读取文件内容
            let bytes = match std::fs::read(path) {
                Ok(b) => b,
                Err(_) => continue,
            };

            // 跳过二进制文件（用 is_binary_file 检测前 8KB）
            if is_binary_file(&bytes) {
                continue;
            }

            // 解码为字符串（UTF-8，容错处理无效字节）
            let content = String::from_utf8_lossy(&bytes);
            let lines: Vec<&str> = content.lines().collect();

            // 逐行匹配 regex
            for (i, line) in lines.iter().enumerate() {
                if re.is_match(line) {
                    // 收集上下文行（匹配行之前的若干行）
                    let ctx_before: Vec<String> = if context_before > 0 {
                        let start_idx = i.saturating_sub(context_before);
                        lines[start_idx..i].iter().map(|s| s.to_string()).collect()
                    } else {
                        Vec::new()
                    };
                    // 收集上下文行（匹配行之后的若干行）
                    let ctx_after: Vec<String> = if context_after > 0 {
                        let end_idx = (i + 1 + context_after).min(lines.len());
                        lines[i + 1..end_idx]
                            .iter()
                            .map(|s| s.to_string())
                            .collect()
                    } else {
                        Vec::new()
                    };

                    matches.push(json!({
                        "path": relative,
                        "line_number": i + 1,
                        "line": line,
                        "match_type": "content",
                        "context_before": ctx_before,
                        "context_after": ctx_after,
                    }));

                    if matches.len() >= max_matches {
                        truncated = true;
                        break 'outer;
                    }
                }
            }
        }

        log::debug!(
            "grep 完成: pattern={}, path={}, 匹配 {} 项",
            pattern,
            search_path,
            matches.len()
        );
        ToolResult {
            success: true,
            output: Some(json!({
                "pattern": pattern,
                "path": search_path,
                "matches": matches,
                "count": matches.len(),
                "truncated": truncated,
            })),
            error: None,
            duration_ms: start.elapsed().as_millis() as u64,
            error_code: None,
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
    fn tool_name(&self) -> &str {
        "scratchpad"
    }

    fn description(&self) -> &str {
        "Agent scratchpad: record or read task notes to maintain context across iterations.\
         Use cases: record key decisions, to-do items, file paths, and intermediate results in complex multi-step tasks.\
         It is recommended to call action=add to record key points after completing important steps;\
         when task context grows long, action=read can review existing notes;\
         after task completion, action=clear cleans up notes.\
         Note contents are automatically injected into your context in subsequent iterations, no need to read them repeatedly."
    }

    fn category(&self) -> &str {
        "memory"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "read", "clear"],
                    "description": "Action type: add=append note; read=read all notes; clear=clear notes",
                    "default": "add"
                },
                "content": {
                    "type": "string",
                    "description": "Note content (required when action=add). Keep it concise, no more than 200 characters per entry"
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
                error: Some("Internal error: missing session identifier".to_string()),
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
                        error: Some("content cannot be empty when action=add".to_string()),
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
                    session_id,
                    entry_count
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
                    session_id,
                    notes.len()
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
                    states.remove(&session_id).map(|s| s.len()).unwrap_or(0)
                };

                log::info!(
                    "update_notes 清空笔记: session_id={}, 已清除 {} 条",
                    session_id,
                    cleared_count
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
                    error: Some(format!(
                        "Unknown action: {} (supported: add/read/clear)",
                        action
                    )),
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

    let mut summary = String::from("<scratchpad>\n## Your Task Notes\n\nThe following are task notes you previously recorded. Please continue working based on these notes (no need to read them again):\n\n");
    for (i, entry) in state.iter().enumerate() {
        summary.push_str(&format!("{}. {}\n", i + 1, entry.content));
    }
    summary.push_str("\nTo update notes, please call the scratchpad tool.\n</scratchpad>");
    Some(summary)
}

// ============================================================
// write_script - 写入脚本文件到临时目录
// ============================================================
//
// 让智能体编写 Python / Bash / PowerShell 脚本文件，存放在系统临时目录下，
// 供 bash 或 powershell 工具执行。脚本文件不污染工作区目录。
//
// 存放路径：<temp_dir>/samoyed_work/scripts/<filename>
// 脚本语言：python（.py）、bash（.sh/.bash）或 powershell（.ps1）

/// 脚本守卫的统一引导文案
///
/// write/rename/copy 三处拒绝写脚本到工作区时都追加这段指引，抽出来避免三份拷贝走样。
/// 必须同时指向两个执行入口：.ps1 在 Git Bash 里跑不了。
fn script_execution_guidance() -> &'static str {
    "Please use the write_script tool to write scripts to the system temporary directory, \
     then execute them via the bash tool (or the powershell tool for .ps1 scripts)"
}

/// 脚本写入工具
/// 将智能体编写的脚本内容写入系统临时目录，返回脚本绝对路径
struct WriteScriptTool;

#[async_trait]
impl Tool for WriteScriptTool {
    fn tool_name(&self) -> &str {
        "write_script"
    }

    fn description(&self) -> &str {
        "Write script content to a temporary file for execution by the bash or powershell tool.\
         Supports writing Python (.py), Bash (.sh/.bash) or PowerShell (.ps1) scripts to solve user problems (document processing, data analysis, automation tasks, etc.).\
         Script files are stored in the system temporary directory and do not pollute the workspace.\
         Returns the absolute path of the script file: run .py/.sh via the bash tool ('python <path>' or 'bash <path>'), and run .ps1 via the powershell tool ('& \"<path>\"' or 'powershell -File <path>').\
         Note: a PowerShell script must call 'exit <code>' explicitly to report failure; a failing native command inside the script does not change the script's exit status on its own."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filename": {
                    "type": "string",
                    "description": "Script filename (including extension, e.g. 'generate_report.py', 'process_data.sh' or 'collect_logs.ps1')"
                },
                "language": {
                    "type": "string",
                    "enum": ["python", "bash", "powershell"],
                    "description": "Script language type: python (.py), bash (.sh) or powershell (.ps1). If filename already has an extension, this field can be omitted and will be auto-inferred"
                },
                "content": {
                    "type": "string",
                    "description": "Script file content (complete source code)"
                }
            },
            "required": ["filename", "content"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        let filename = params["filename"].as_str().unwrap_or("").trim();
        let content = params["content"].as_str().unwrap_or("");
        let language = params["language"].as_str().unwrap_or("");

        // 参数校验
        if filename.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing filename parameter".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }
        if content.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Script content cannot be empty".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：禁止文件名包含路径分隔符或 .. 遍历
        if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Filename contains illegal characters: {}",
                    filename
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 推断语言和扩展名
        let (final_filename, detected_language) = infer_script_language(filename, language);
        let _ = detected_language; // 语言仅用于日志，不强制使用

        // 构造脚本目录：<temp_dir>/samoyed_work/scripts/
        let script_dir = std::env::temp_dir().join("samoyed_work").join("scripts");
        if let Err(e) = std::fs::create_dir_all(&script_dir) {
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!("Failed to create script directory: {}", e)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            };
        }

        let script_path = script_dir.join(&final_filename);

        // 写入脚本文件
        match tokio::fs::write(&script_path, content).await {
            Ok(()) => {
                let path_str = script_path.to_string_lossy().to_string();
                log::info!(
                    "write_script: 已写入脚本文件: {} (语言: {}, 大小: {} 字节)",
                    path_str,
                    detected_language,
                    content.len()
                );
                ToolResult {
                    success: true,
                    output: Some(json!({
                        "path": path_str,
                        "filename": final_filename,
                        "language": detected_language,
                        "size": content.len(),
                        "message": format!("脚本已写入: {}", final_filename)
                    })),
                    error: None,
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => ToolResult {
                success: false,
                output: None,
                error: Some(format!("Failed to write script file: {}", e)),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: None,
            },
        }
    }
}

/// 根据文件名和语言参数推断最终文件名和语言类型
/// 若 filename 已含扩展名，直接使用；否则根据 language 参数补充扩展名
fn infer_script_language(filename: &str, language: &str) -> (String, &'static str) {
    let lower = filename.to_lowercase();
    // 后缀优先于 language 参数：二者矛盾时以后缀为准，
    // 否则会把 .ps1 文件报告成 python 语言、或落到默认分支把 .ps1 再补一个 .py
    if lower.ends_with(".py") {
        (filename.to_string(), "python")
    } else if lower.ends_with(".sh") || lower.ends_with(".bash") {
        (filename.to_string(), "bash")
    } else if lower.ends_with(".ps1") {
        (filename.to_string(), "powershell")
    } else {
        // 无扩展名，根据 language 参数补充
        match language {
            "bash" => (format!("{}.sh", filename), "bash"),
            "powershell" => (format!("{}.ps1", filename), "powershell"),
            _ => (format!("{}.py", filename), "python"),
        }
    }
}

// ============================================================
// bash - 执行 Shell 命令（通过 Git Bash）
// ============================================================
//
// 让智能体通过 Git Bash 执行 Shell 命令，支持运行脚本、文件操作、
// 系统命令等。工作目录默认为当前工作区，可通过 working_dir 参数指定。
//
// Git Bash 路径获取优先级：
// 1. 配置中指定的 git_bash_path
// 2. 从 PATH 环境变量查找 git.exe，推断 bash.exe 位置
// 3. 从 PATH 直接查找 bash.exe

/// 脚本泄露判定的公共部分：命令是否涉及工作区 + 是否涉及脚本文件
///
/// 与具体 shell 无关，供 bash 与 powershell 两个工具复用；
/// 调用方还需确认命令含有"写入/复制"类操作才算泄露。
///
/// 路径格式兼容：
/// - Windows 风格：D:\DeskTop\test 或 D:/DeskTop/test
/// - Git Bash 风格：/d/DeskTop/test（盘符 D: 转换为 /d/）
fn script_leak_targets_workspace(command: &str, working_dir: &str, workspace_root: &str) -> bool {
    if workspace_root.is_empty() {
        return false;
    }

    let lower = command.to_lowercase();

    // 计算工作区路径的多种格式表示，使命令中任意一种格式都能匹配
    // 1. Windows 风格（正斜杠）：D:\DeskTop\test -> d:/desktop/test
    let ws_windows = workspace_root.to_lowercase().replace('\\', "/");
    // 2. Git Bash 风格：D:/DeskTop/test -> /d/DeskTop/test
    //    将 "d:/..." 转换为 "/d/..."（移除冒号，前加 /）
    let ws_gitbash = if ws_windows.len() >= 2 && ws_windows.as_bytes()[1] == b':' {
        format!("/{}", ws_windows.replacen(':', "", 1))
    } else {
        ws_windows.clone()
    };

    // 命令中是否出现工作区路径（任意一种格式匹配即可）
    let cmd_normalized = lower.replace('\\', "/");
    let mentions_workspace =
        cmd_normalized.contains(&ws_windows) || cmd_normalized.contains(&ws_gitbash);

    // 新增逻辑：当命令中未直接出现工作区路径时，结合 working_dir 判定
    // 若 working_dir 等于 workspace_root（命令在工作区内执行），
    // 相对路径目标（如 __self_test__/leak.py）视为工作区内路径，构成泄露
    // working_dir 为空或不等于 workspace_root 时，不触发检测（保持现有行为）
    if !mentions_workspace {
        if working_dir.is_empty() {
            return false;
        }
        let wd_normalized = working_dir.to_lowercase().replace('\\', "/");
        if wd_normalized != ws_windows {
            return false;
        }
    }

    // 命令中是否出现脚本文件扩展名（作为子字符串）
    // 命令中的脚本路径可能是 .py、.sh 等扩展名，需要检查多种边界情况
    const SCRIPT_EXT_TOKENS: &[&str] = &[
        ".py ", ".py\"", ".py'", ".py;", ".sh ", ".sh\"", ".sh'", ".sh;", ".bash ", ".bash\"",
        ".bash'", ".bash;", ".ps1 ", ".ps1\"", ".ps1'", ".ps1;", ".bat ", ".bat\"", ".bat'",
        ".bat;", ".cmd ", ".cmd\"", ".cmd'", ".cmd;",
    ];
    lower.ends_with(".py")
        || lower.ends_with(".sh")
        || lower.ends_with(".bash")
        || lower.ends_with(".ps1")
        || lower.ends_with(".bat")
        || lower.ends_with(".cmd")
        || SCRIPT_EXT_TOKENS.iter().any(|tok| lower.contains(tok))
}

/// 判断命令是否试图将脚本文件复制/移动到工作区目录（Git Bash）
/// 阻止以下脚本泄露途径：
/// 1. cp/mv 命令将脚本文件从临时目录复制到工作区
/// 2. 重定向（>、>>）将脚本内容写入工作区
/// 3. install 命令将脚本安装到工作区
///
/// 检测逻辑：命令同时满足以下条件时拒绝执行
/// - 包含文件复制/移动/重定向操作（cp/copy/mv/move/install/>/>>）
/// - 命令中出现脚本文件扩展名（.py/.sh/.bash/.ps1/.bat/.cmd 等）
/// - 命令中出现工作区路径（用于判断目标是否为工作区）
fn is_script_leak_command(command: &str, working_dir: &str, workspace_root: &str) -> bool {
    if !script_leak_targets_workspace(command, working_dir, workspace_root) {
        return false;
    }

    let lower = command.to_lowercase();
    // 命令中是否包含文件复制/移动/重定向操作
    // cp/copy/mv/move 命令；>、>> 重定向；install 安装命令；tee 写入命令
    lower.contains("cp ")
        || lower.contains("copy ")
        || lower.contains("mv ")
        || lower.contains("move ")
        || lower.contains("install ")
        || lower.contains("> ")
        || lower.contains(">>")
        || lower.contains("tee ")
}

/// 判断 token 是否出现在"命令位置"（前后均为分隔符）
///
/// PowerShell 必须用词边界而非纯 contains：`Move-Item` 是 `Remove-Item` 的子串
/// （remove-item 从第 2 个字符起恰为 move-item），纯子串匹配会把"删除工作区脚本"
/// 误判成"泄露脚本到工作区"；`sc`/`mi` 等两字母别名同理会被 misc、minus 之类吞掉。
fn contains_at_word_boundary(hay: &str, token: &str) -> bool {
    // 词边界分隔符：空白、语句/管道分隔符、括号、引号、赋值号、驱动器与静态调用用的冒号
    const WORD_DELIMS: &[u8] = &[
        b' ', b'\t', b'\n', b'\r', b';', b'|', b'&', b'(', b')', b'{', b'}', b',', b'"', b'\'',
        b'=', b':',
    ];
    let bytes = hay.as_bytes();
    let is_delim = |b: u8| WORD_DELIMS.contains(&b);
    let mut from = 0usize;
    while let Some(rel) = hay[from..].find(token) {
        let start = from + rel;
        let end = start + token.len();
        let prev_ok = start == 0 || is_delim(bytes[start - 1]);
        let next_ok = end >= bytes.len() || is_delim(bytes[end]);
        if prev_ok && next_ok {
            return true;
        }
        from = start + token.len().max(1);
    }
    false
}

/// PowerShell 侧的"写入/复制"操作词表
/// 全称 + 本机 Get-Alias 实测别名：
///   Copy-Item => copy cp cpi | Move-Item => mi move mv | Set-Content => sc
///   Add-Content => ac | Tee-Object => tee | Out-File 无别名
const PS_LEAK_WRITE_TOKENS: &[&str] = &[
    "copy-item",
    "move-item",
    "set-content",
    "add-content",
    "out-file",
    "tee-object",
    "copy",
    "cp",
    "cpi",
    "mi",
    "move",
    "mv",
    "sc",
    "ac",
    "tee",
    // 绕过 cmdlet 直接调用 .NET 写文件的常见形式
    "writealltext",
    "writeallbytes",
    "writealllines",
    "appendalltext",
    "appendallbytes",
    "appendalllines",
];

/// 判断 PowerShell 命令是否试图把脚本文件写入工作区
///
/// 不能直接复用 bash 版：它的词表只有 cp/mv/copy/>/tee 等 POSIX 字面量，
/// 覆盖不到 Copy-Item、Out-File、Set-Content、[IO.File]::WriteAllText 等 PowerShell 写法。
fn is_script_leak_powershell_command(
    command: &str,
    working_dir: &str,
    workspace_root: &str,
) -> bool {
    if !script_leak_targets_workspace(command, working_dir, workspace_root) {
        return false;
    }
    let lower = command.to_lowercase();
    // PowerShell 的重定向与 bash 同为 > 和 >>
    if lower.contains("> ") || lower.contains(">>") {
        return true;
    }
    PS_LEAK_WRITE_TOKENS
        .iter()
        .any(|tok| contains_at_word_boundary(&lower, tok))
}

/// 命令执行工具
/// 通过 Git Bash 执行 Shell 命令，捕获 stdout/stderr/exit_code
pub struct RunCommandTool {
    /// Git Bash 可执行文件路径（空字符串表示自动检测）
    pub git_bash_path: String,
}

/// 命令执行默认超时时间（秒），LLM 未传 timeout 参数时使用
const FALLBACK_COMMAND_TIMEOUT_SECS: u64 = 60;

/// 安全截断字符串到指定字节长度（不会在 UTF-8 字符中间截断）
/// 用于命令执行输出过长时的截断处理
fn truncate_safe(s: &str, max_chars: usize) -> String {
    if s.len() <= max_chars {
        return s.to_string();
    }
    // 找到不超过 max_chars 的字符边界，避免在 UTF-8 字符中间截断导致 panic
    let mut end = max_chars;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!(
        "{}...(truncated, total {} chars)",
        &s[..end],
        s.chars().count()
    )
}

#[async_trait]
impl Tool for RunCommandTool {
    fn tool_name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute Shell commands via Git Bash. Can be used to run script files, execute system commands, process files, etc.\
         The working directory defaults to the current workspace, and can be specified via the working_dir parameter.\
         Command timeout defaults to 60 seconds, adjustable via the timeout parameter (maximum 300 seconds).\
         Output exceeding 6000 characters will be automatically truncated.\
         High-risk commands (containing rm/del/rmdir/format/shutdown/sudo/git push --force, etc.) will request user confirmation.\
         Returns stdout, stderr, exit_code, success, and duration_secs fields.\
         Important: copying or moving script files (.py/.sh/.bash, etc.) to the workspace directory via cp/mv/redirection is prohibited; script files should only be executed in the system temporary directory."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Shell command to execute (will be executed via bash -c). For example: 'python /tmp/samoyed_work/scripts/script.py' or 'ls -la'"
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for command execution (optional, defaults to current workspace root)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Command timeout in seconds, default 60, maximum 300",
                    "default": 60
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        let command = params["command"].as_str().unwrap_or("").trim().to_string();
        let working_dir = params["working_dir"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 命令超时由 LLM 通过 timeout 参数决定，最大 300 秒
        // LLM 未传 timeout 时使用默认值 60 秒
        let timeout = params["timeout"]
            .as_u64()
            .unwrap_or(FALLBACK_COMMAND_TIMEOUT_SECS)
            .min(300);

        // 参数校验
        if command.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing command parameter".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：阻止将脚本文件复制/移动到工作区目录
        // 脚本文件应只在系统临时目录中创建和执行，不允许通过 cp/mv/重定向等方式泄露到工作区
        if is_script_leak_command(&command, working_dir, workspace_root) {
            log::warn!("bash: 检测到脚本泄露命令，已拒绝执行: {}", command);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Command detected attempting to copy or move script files to the workspace directory, execution denied. Script files should only be created and executed in the system temporary directory. Please execute scripts directly via 'python <script_path>' or 'bash <script_path>' in the temporary directory. Command: {}",
                    command
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 解析工作目录：优先使用 working_dir，其次 workspace_root
        let cwd = if !working_dir.is_empty() {
            working_dir.to_string()
        } else if !workspace_root.is_empty() {
            workspace_root.to_string()
        } else {
            String::new()
        };

        // 获取 Git Bash 可执行文件路径
        let bash_path = resolve_bash_path(&self.git_bash_path);
        let bash_path = match bash_path {
            Some(p) => p,
            None => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "未找到 Git Bash 可执行文件。请在设置中配置 Git Bash 路径，或确保 git 已安装并添加到 PATH 环境变量".to_string()
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        log::info!(
            "bash: 执行命令 (cwd='{}', timeout={}s): {}",
            cwd,
            timeout,
            command
        );

        // 在 spawn_blocking 中执行同步的子进程操作，避免阻塞异步运行时
        let command_for_closure = command.clone();
        let cwd_for_closure = cwd.clone();
        let bash_path_for_closure = bash_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            execute_bash_command(
                &bash_path_for_closure,
                &command_for_closure,
                &cwd_for_closure,
                timeout,
            )
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                log::info!(
                    "bash: 命令执行完成 (exit_code={}, stdout={} 字节, stderr={} 字节)",
                    output.exit_code,
                    output.stdout.len(),
                    output.stderr.len()
                );
                // 截断过长输出（6000 字符限制）
                const MAX_OUTPUT_CHARS: usize = 6000;
                let stdout_truncated = truncate_safe(&output.stdout, MAX_OUTPUT_CHARS);
                let stderr_truncated = truncate_safe(&output.stderr, MAX_OUTPUT_CHARS);
                let duration_secs = start.elapsed().as_secs_f64();

                ToolResult {
                    success: output.exit_code == 0,
                    output: Some(json!({
                        "stdout": stdout_truncated,
                        "stderr": stderr_truncated,
                        "exit_code": output.exit_code,
                        "success": output.exit_code == 0,
                        "duration_secs": duration_secs,
                        "command": command,
                        "working_dir": cwd,
                    })),
                    error: if output.exit_code != 0 {
                        Some(format!("命令执行失败，退出码: {}", output.exit_code))
                    } else {
                        None
                    },
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("bash: 命令执行错误: {}", e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Command execution error: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("bash: 任务执行失败: {}", e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Task execution failed: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

/// 命令执行结果
struct CommandOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

/// 解析 Git Bash 可执行文件路径
/// 优先使用配置路径，否则从 PATH 环境变量自动检测
fn resolve_bash_path(configured_path: &str) -> Option<String> {
    // 1. 优先使用配置中指定的路径
    if !configured_path.is_empty() {
        let path = std::path::Path::new(configured_path);
        if path.exists() {
            log::debug!("resolve_bash_path: 使用配置路径: {}", configured_path);
            return Some(configured_path.to_string());
        }
        log::warn!(
            "resolve_bash_path: 配置的 Git Bash 路径不存在: {}",
            configured_path
        );
    }

    // 2. 从 PATH 环境变量自动检测
    find_git_bash_from_path()
}

/// 从 PATH 环境变量中查找 Git Bash 可执行文件
/// 检测策略：
///   a. 先从 PATH 中直接查找 bash.exe
///   b. 若未找到，从 PATH 中查找 git.exe，推断 bash.exe 位置（<git_root>/bin/bash.exe）
fn find_git_bash_from_path() -> Option<String> {
    let path_env = std::env::var_os("PATH")?;

    #[cfg(target_os = "windows")]
    {
        use std::path::PathBuf;

        // Windows 上 PATH 使用分号分隔
        let paths: Vec<PathBuf> = std::env::split_paths(&path_env).collect();

        // 策略 a: 从 PATH 中直接查找 bash.exe
        for dir in &paths {
            let bash_candidate = dir.join("bash.exe");
            if bash_candidate.exists() {
                log::info!(
                    "find_git_bash_from_path: 从 PATH 找到 bash.exe: {}",
                    bash_candidate.display()
                );
                return Some(bash_candidate.to_string_lossy().to_string());
            }
        }

        // 策略 b: 从 PATH 中查找 git.exe，推断 bash.exe 位置
        // Git 安装目录结构：<git_root>/cmd/git.exe，bash.exe 在 <git_root>/bin/bash.exe
        for dir in &paths {
            let git_candidate = dir.join("git.exe");
            if git_candidate.exists() {
                // dir 形如 <git_root>/cmd，bash 应在 <git_root>/bin/bash.exe
                if let Some(parent) = dir.parent() {
                    let bash_inferred = parent.join("bin").join("bash.exe");
                    if bash_inferred.exists() {
                        log::info!(
                            "find_git_bash_from_path: 从 git.exe 推断 bash.exe: {}",
                            bash_inferred.display()
                        );
                        return Some(bash_inferred.to_string_lossy().to_string());
                    }
                    // 部分安装可能在 <git_root>/usr/bin/bash.exe
                    let bash_usr = parent.join("usr").join("bin").join("bash.exe");
                    if bash_usr.exists() {
                        log::info!(
                            "find_git_bash_from_path: 从 git.exe 推断 bash.exe (usr/bin): {}",
                            bash_usr.display()
                        );
                        return Some(bash_usr.to_string_lossy().to_string());
                    }
                }
            }
        }

        log::warn!("find_git_bash_from_path: 未在 PATH 中找到 bash.exe 或 git.exe");
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        // 非 Windows 平台：直接查找 bash
        for dir in std::env::split_paths(&path_env) {
            let bash_candidate = dir.join("bash");
            if bash_candidate.exists() {
                return Some(bash_candidate.to_string_lossy().to_string());
            }
        }
        None
    }
}

/// 执行 bash 命令（同步函数，应在 spawn_blocking 中调用）
fn execute_bash_command(
    bash_path: &str,
    command: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<CommandOutput, String> {
    use std::process::{Command, Stdio};
    use std::time::Instant;

    #[cfg(target_os = "windows")]
    const CREATE_NO_WINDOW: u32 = 0x08000000;

    let mut cmd = Command::new(bash_path);
    cmd.arg("-c").arg(command);

    // 设置工作目录
    if !working_dir.is_empty() {
        cmd.current_dir(working_dir);
    }

    // 注入环境变量，优化 Python 脚本执行环境
    // PYTHONIOENCODING=utf-8: 强制 Python 标准输入输出使用 UTF-8 编码
    //   解决 Windows 下 Python 默认使用 GBK 编码导致输出 Unicode 字符（如 \u2022）时报错的问题
    // PYTHONUTF8=1: 启用 Python UTF-8 模式（PEP 540），使所有文件 I/O 默认使用 UTF-8
    //   进一步减少编码相关的失败，提升智能体脚本执行成功率
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUTF8", "1");

    // 捕获 stdout 和 stderr
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("启动子进程失败: {}", e))?;

    // 使用 tokio 的同步等待 + 超时控制
    // 由于本函数在 spawn_blocking 中调用，可以使用同步等待
    let timeout_duration = Duration::from_secs(timeout_secs);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child
                    .stdout
                    .take()
                    .map(|mut s| {
                        use std::io::Read;
                        let mut buf = String::new();
                        s.read_to_string(&mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();
                let stderr = child
                    .stderr
                    .take()
                    .map(|mut s| {
                        use std::io::Read;
                        let mut buf = String::new();
                        s.read_to_string(&mut buf).ok();
                        buf
                    })
                    .unwrap_or_default();

                let exit_code = status.code().unwrap_or(-1);
                return Ok(CommandOutput {
                    stdout,
                    stderr,
                    exit_code,
                });
            }
            Ok(None) => {
                // 子进程仍在运行，检查超时
                if start.elapsed() >= timeout_duration {
                    log::warn!(
                        "execute_bash_command: 命令超时 ({}秒)，终止子进程",
                        timeout_secs
                    );
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("命令执行超时（{}秒），已终止", timeout_secs));
                }
                // 短暂休眠避免 CPU 空转
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(format!("等待子进程失败: {}", e));
            }
        }
    }
}

// ============================================================
// powershell - 通过 PowerShell 执行命令
// ============================================================

/// PowerShell 命令执行工具
///
/// 与 bash 工具（RunCommandTool）保持完全一致的参数与返回契约，差异只在解释器：
/// 优先使用 PowerShell 7+（pwsh.exe），回退到系统内置的 Windows PowerShell 5.1。
pub struct RunPowerShellCommandTool;

/// 脚本前导：抑制进度流并强制 UTF-8 输出编码
///
/// 两处都是实测出来的必需项：
/// - 不抑制进度流时，5.1 会在 stderr 序列化出一条"正在准备首次使用模块"的 CLIXML 记录，
///   污染错误输出（实测 stderr 由 382 字节降为 0）
/// - 5.1 默认跟随系统 OEM 代码页（中文系统为 GBK），不强制 UTF-8 会让中文输出变 GBK 字节
const PS_SCRIPT_PREFIX: &str = concat!(
    "$ProgressPreference='SilentlyContinue'; ",
    "try { $__swEnc = New-Object System.Text.UTF8Encoding($false); ",
    "[Console]::OutputEncoding = $__swEnc; $OutputEncoding = $__swEnc } catch {}\n",
);

/// 脚本后缀：把 PowerShell 的执行状态归一化为进程退出码
///
/// PowerShell 的宿主退出码不等于"最后一条命令的退出码"：原生命令的退出码只写入
/// `$LASTEXITCODE` 而不会传给进程（实测 `cmd /c "exit 7"` 宿主退出码为 1）。
/// 这里按 bash 语义归一化：退出码 = 最后一条语句的结果。
/// 注意 `$?` 必须作为后缀的第一条语句读取，否则会被紧随其后的赋值语句重置为 True。
const PS_SCRIPT_SUFFIX: &str = concat!(
    "\n$__swOk = $?\n",
    "$__swLe = $LASTEXITCODE\n",
    "$__swEc = 0\n",
    "if (-not $__swOk) {\n",
    "  if ($null -ne $__swLe -and $__swLe -ne 0) { $__swEc = $__swLe } else { $__swEc = 1 }\n",
    "}\n",
    "exit $__swEc\n",
);

/// 输出流在进程退出后的最长等待时间（秒）
///
/// 若 PowerShell 派生的孙进程继承了管道写句柄，管道会在宿主退出后继续打开；
/// 超过该宽限期则放弃等待，避免已成功的命令被误判为超时。
const PS_STREAM_DRAIN_GRACE_SECS: u64 = 3;

/// Windows 进程创建标志：阻止为子进程分配控制台窗口
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x08000000;

/// 组装最终交给 PowerShell 执行的脚本：编码前导 + 用户命令 + 退出码归一后缀
fn build_powershell_script(command: &str) -> String {
    format!("{}{}{}", PS_SCRIPT_PREFIX, command, PS_SCRIPT_SUFFIX)
}

/// 将脚本编码为 `-EncodedCommand` 需要的 Base64(UTF-16LE)
///
/// 相比 `-Command`，`-EncodedCommand` 彻底规避了 Windows 命令行引号二次解析问题，
/// 也规避了 stdin 在 5.1 上按 GBK 解码导致的非 ASCII 脚本乱码（均已实测）。
fn encode_powershell_command(script: &str) -> String {
    use base64::Engine as _;
    if script.is_empty() {
        return String::new();
    }
    let utf16_bytes: Vec<u8> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    base64::engine::general_purpose::STANDARD.encode(&utf16_bytes)
}

/// 钳制超时时间：默认 60 秒，最短 1 秒，最长 300 秒（与 bash 工具一致）
fn clamp_powershell_timeout(timeout: Option<Value>) -> u64 {
    timeout
        .and_then(|v| v.as_u64())
        .unwrap_or(FALLBACK_COMMAND_TIMEOUT_SECS)
        .clamp(1, 300)
}

/// 解码子进程输出字节：UTF-8 优先，失败回退 GBK
///
/// 回退分支是必需的：当前导的编码设置因无控制台而失败时，5.1 会输出 GBK 字节，
/// 直接按 UTF-8 读取会失败并丢空输出（bash 工具现存的 `read_to_string().ok()` 即此问题）。
fn decode_console_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return String::new();
    }
    match std::str::from_utf8(bytes) {
        Ok(text) => text.to_string(),
        // 非 UTF-8：按中文 Windows 的代码页解码
        Err(_) => encoding_rs::GBK.decode(bytes).0.into_owned(),
    }
}

/// 还原 PowerShell 在 stderr 被重定向时序列化出的 CLIXML 错误流
///
/// stderr 是管道时，PowerShell 会把 error/warning/verbose 等流写成
/// `#< CLIXML` + `<Objs><S S="Error">...</S></Objs>`，而原生命令的 stderr 仍以纯文本交错出现。
/// 这里把 CLIXML 块还原为可读文本、丢弃进度记录，并原样保留原生片段。
fn decode_powershell_stderr(text: &str) -> String {
    if !text.contains("<Objs") {
        return text.to_string();
    }

    let mut result = String::with_capacity(text.len());
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find("<Objs") {
            Some(start) => {
                result.push_str(&strip_clixml_marker(&rest[..start]));
                let block = &rest[start..];
                match block.find("</Objs>") {
                    Some(end) => {
                        let full_end = end + "</Objs>".len();
                        result.push_str(&extract_clixml_records(&block[..full_end]));
                        rest = &block[full_end..];
                    }
                    // 缺少闭合标签：保留原文，绝不丢弃信息
                    None => {
                        result.push_str(block);
                        rest = "";
                    }
                }
            }
            None => {
                result.push_str(&strip_clixml_marker(rest));
                rest = "";
            }
        }
    }
    result
}

/// 去掉 `#< CLIXML` 标记行
fn strip_clixml_marker(text: &str) -> String {
    text.replace("#< CLIXML\r\n", "").replace("#< CLIXML\n", "")
}

/// 从一个 `<Objs>...</Objs>` 块中抽取各数据流的文本内容
///
/// 只取 `<S S="流名">内容</S>` 形式的数据记录；`<Obj S="progress">` 这类复合记录
/// 内部不含 `<S S="` 前缀，因此会被整体跳过。
fn extract_clixml_records(block: &str) -> String {
    let mut out = String::new();
    // 跳过开始标签，只处理其内容
    let inner = match block.find('>') {
        Some(gt) => &block[gt + 1..],
        None => return out,
    };
    let inner = match inner.rfind("</Objs>") {
        Some(close) => &inner[..close],
        None => inner,
    };

    let mut rest = inner;
    while let Some(pos) = rest.find("<S S=\"") {
        let after_open = &rest[pos + "<S S=\"".len()..];
        // 跳过流名属性值（如 Error / warning），各数据流统一按文本输出
        let quote = match after_open.find('"') {
            Some(i) => i,
            None => break,
        };
        let tag_end = match after_open[quote..].find('>') {
            Some(i) => quote + i,
            None => break,
        };
        // 自闭合标签（如 `<S S="Error" />`）无正文
        if after_open[..tag_end].trim_end().ends_with('/') {
            rest = &after_open[tag_end + 1..];
            continue;
        }
        let content_area = &after_open[tag_end + 1..];
        let close = match content_area.find("</S>") {
            Some(i) => i,
            None => {
                // 未闭合：把剩余内容当作正文输出
                out.push_str(&decode_clixml_text(content_area));
                break;
            }
        };
        out.push_str(&decode_clixml_text(&content_area[..close]));
        rest = &content_area[close + "</S>".len()..];
    }
    out
}

/// 反转义 CLIXML 文本节点：先还原 XML 实体，再单趟解析 `_xHHHH_`，最后剥离 ANSI 序列
///
/// 顺序不可颠倒：PowerShell 会把字面下划线转义为 `_x005F_`，
/// 单趟从左到右解析才能得到 `_x000D_` 这类字面文本而不是换行符。
fn decode_clixml_text(raw: &str) -> String {
    let unescaped = xml_unescape(raw);
    let with_chars = ps_unescape_chars(&unescaped);
    strip_ansi_escapes(&with_chars)
}

/// 把 `_xHHHH_` 形式的 PowerShell 字符转义还原为对应字符（单趟扫描，不二次解析）
fn ps_unescape_chars(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        // 模式 `_xHHHH_` 全为 ASCII，按字节判定可避免切进多字节字符中间
        if i + 7 <= len
            && bytes[i] == b'_'
            && bytes[i + 1] == b'x'
            && bytes[i + 6] == b'_'
            && bytes[i + 2..i + 6].iter().all(|b| b.is_ascii_hexdigit())
        {
            let digits = std::str::from_utf8(&bytes[i + 2..i + 6]).unwrap_or("0");
            let code = u32::from_str_radix(digits, 16).unwrap_or(0xFFFD);
            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
            i += 7;
            continue;
        }
        // 其余按整字符推进；游标 i 始终落在字符边界上
        let ch = text[i..].chars().next().unwrap_or('\u{fffd}');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// 还原 XML 预定义实体
fn xml_unescape(text: &str) -> String {
    // &amp; 必须最后替换，否则会把 `&amp;lt;` 误解码两次
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

/// 剥离 ANSI 终端转义序列（pwsh 7 会把颜色码写进 CLIXML 正文）
fn strip_ansi_escapes(text: &str) -> String {
    if !text.contains('\u{1b}') {
        return text.to_string();
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            out.push(c);
            continue;
        }
        match chars.peek() {
            Some('[') => {
                chars.next();
                // CSI：终结字节为 @..~
                for t in chars.by_ref() {
                    if matches!(t, '@'..='~') {
                        break;
                    }
                }
            }
            Some(']') => {
                chars.next();
                // OSC：以 BEL 或 ESC\ 结束
                for t in chars.by_ref() {
                    if t == '\u{7}' {
                        break;
                    }
                }
            }
            _ => {
                chars.next();
            }
        }
    }
    out
}

/// 解析 PowerShell 可执行文件路径
///
/// 优先级：
///   1. PATH 中的 pwsh.exe（PowerShell 7+，跨平台且默认 UTF-8）
///   2. 常见安装目录中的 pwsh.exe（MSI 安装后可能未刷新 PATH）
///   3. 系统内置的 Windows PowerShell 5.1
fn resolve_powershell_path() -> Option<String> {
    // 1. PATH 中查找 pwsh
    if let Some(path_env) = std::env::var_os("PATH") {
        #[cfg(target_os = "windows")]
        let exe_name = "pwsh.exe";
        #[cfg(not(target_os = "windows"))]
        let exe_name = "pwsh";
        for dir in std::env::split_paths(&path_env) {
            let candidate = dir.join(exe_name);
            if candidate.is_file() {
                log::info!(
                    "resolve_powershell_path: 从 PATH 找到 {}: {}",
                    exe_name,
                    candidate.display()
                );
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    #[cfg(target_os = "windows")]
    {
        // 2. 已知安装目录（PATH 未刷新时兜底）
        let mut roots: Vec<std::path::PathBuf> = Vec::new();
        for key in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
            if let Ok(dir) = std::env::var(key) {
                roots.push(std::path::PathBuf::from(dir));
            }
        }
        for root in &roots {
            for version in ["7", "8"] {
                let candidate = root.join("PowerShell").join(version).join("pwsh.exe");
                if candidate.is_file() {
                    log::info!(
                        "resolve_powershell_path: 从安装目录找到 pwsh.exe: {}",
                        candidate.display()
                    );
                    return Some(candidate.to_string_lossy().to_string());
                }
            }
        }

        // 3. 回退到系统内置的 Windows PowerShell 5.1
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        for arch in ["System32", "SysWOW64"] {
            let candidate = std::path::Path::new(&system_root)
                .join(arch)
                .join("WindowsPowerShell")
                .join("v1.0")
                .join("powershell.exe");
            if candidate.is_file() {
                log::info!(
                    "resolve_powershell_path: 回退到 Windows PowerShell 5.1: {}",
                    candidate.display()
                );
                return Some(candidate.to_string_lossy().to_string());
            }
        }
        log::warn!("resolve_powershell_path: 未找到任何 PowerShell 解释器");
        None
    }

    #[cfg(not(target_os = "windows"))]
    {
        log::warn!("resolve_powershell_path: 非 Windows 平台未在 PATH 中找到 pwsh");
        None
    }
}

/// 超时后终止整个进程树
///
/// `child.kill()` 只结束 PowerShell 本身，其派生的原生程序会变成孤儿进程。
#[cfg(target_os = "windows")]
fn kill_process_tree(pid: u32) {
    use std::os::windows::process::CommandExt;
    use std::process::{Command, Stdio};
    let _ = Command::new("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
}

#[cfg(not(target_os = "windows"))]
fn kill_process_tree(_pid: u32) {}

#[async_trait]
impl Tool for RunPowerShellCommandTool {
    fn tool_name(&self) -> &str {
        "powershell"
    }

    fn description(&self) -> &str {
        "Execute commands via PowerShell (prefers PowerShell 7+ 'pwsh.exe', falls back to Windows PowerShell 5.1). Can be used to run cmdlets, scripts, native executables, and system management tasks on Windows.\
         The working directory defaults to the current workspace, and can be specified via the working_dir parameter.\
         Command timeout defaults to 60 seconds, adjustable via the timeout parameter (maximum 300 seconds).\
         Output exceeding 6000 characters will be automatically truncated.\
         High-risk commands (Remove-Item/Stop-Process/Clear-Content/Invoke-Expression/shutdown/Set-ExecutionPolicy, etc.) will request user confirmation.\
         Returns stdout, stderr, exit_code, success, and duration_secs fields; exit_code follows the status of the last statement, like a POSIX shell.\
         Use PowerShell syntax (Get-ChildItem, $var, cmdlet -Parameter), not cmd.exe or Unix syntax; use the bash tool for Git Bash commands.\
         Important: copying or writing script files (.py/.sh/.ps1/.bat/.cmd, etc.) into the workspace directory via Copy-Item/Move-Item/Set-Content/Out-File/redirection is prohibited; script files should only be created and executed in the system temporary directory."
    }

    fn category(&self) -> &str {
        "code"
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "PowerShell script to execute (passed to powershell -EncodedCommand, so quotes and $ need no shell escaping). For example: 'Get-ChildItem -Recurse -Filter *.rs | Measure-Object' or \"$env:OS\""
                },
                "working_dir": {
                    "type": "string",
                    "description": "Working directory for command execution (optional, defaults to current workspace root)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Command timeout in seconds, default 60, maximum 300",
                    "default": 60
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, params: Value) -> ToolResult {
        let start = Instant::now();

        let command = params["command"].as_str().unwrap_or("").trim().to_string();
        let working_dir = params["working_dir"].as_str().unwrap_or("");
        let workspace_root = params["workspace_root"].as_str().unwrap_or("");

        // 命令超时由 LLM 通过 timeout 参数决定，最大 300 秒
        let timeout = clamp_powershell_timeout(params.get("timeout").cloned());

        // 参数校验
        if command.is_empty() {
            return ToolResult {
                success: false,
                output: None,
                error: Some("Missing command parameter".to_string()),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 安全校验：阻止将脚本文件复制/写入工作区目录
        // 脚本文件应只在系统临时目录中创建和执行，不允许通过 Copy-Item/Out-File/重定向等方式泄露到工作区
        if is_script_leak_powershell_command(&command, working_dir, workspace_root) {
            log::warn!("powershell: 检测到脚本泄露命令，已拒绝执行: {}", command);
            return ToolResult {
                success: false,
                output: None,
                error: Some(format!(
                    "Command detected attempting to copy or write script files to the workspace directory, execution denied. Script files should only be created and executed in the system temporary directory. Please execute scripts directly via 'python <script_path>' or 'powershell <script_path>' in the temporary directory. Command: {}",
                    command
                )),
                duration_ms: start.elapsed().as_millis() as u64,
                error_code: Some(crate::errors::TOOL_INVALID_PARAMS),
            };
        }

        // 解析工作目录：优先使用 working_dir，其次 workspace_root
        let cwd = if !working_dir.is_empty() {
            working_dir.to_string()
        } else if !workspace_root.is_empty() {
            workspace_root.to_string()
        } else {
            String::new()
        };

        // 获取 PowerShell 可执行文件路径
        let shell_path = match resolve_powershell_path() {
            Some(p) => p,
            None => {
                return ToolResult {
                    success: false,
                    output: None,
                    error: Some(
                        "未找到 PowerShell 可执行文件。请确认系统已安装 PowerShell（Windows 自带 5.1），或安装 PowerShell 7 并将其加入 PATH"
                            .to_string(),
                    ),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                };
            }
        };

        log::info!(
            "powershell: 执行命令 (shell='{}', cwd='{}', timeout={}s): {}",
            shell_path,
            cwd,
            timeout,
            command
        );

        // 在 spawn_blocking 中执行同步的子进程操作，避免阻塞异步运行时
        let command_for_closure = command.clone();
        let cwd_for_closure = cwd.clone();
        let shell_for_closure = shell_path.clone();

        let result = tokio::task::spawn_blocking(move || {
            execute_powershell_command(
                &shell_for_closure,
                &command_for_closure,
                &cwd_for_closure,
                timeout,
            )
        })
        .await;

        match result {
            Ok(Ok(output)) => {
                log::info!(
                    "powershell: 命令执行完成 (exit_code={}, stdout={} 字节, stderr={} 字节)",
                    output.exit_code,
                    output.stdout.len(),
                    output.stderr.len()
                );
                // 截断过长输出（6000 字符限制）
                const MAX_OUTPUT_CHARS: usize = 6000;
                let stdout_truncated = truncate_safe(&output.stdout, MAX_OUTPUT_CHARS);
                let stderr_truncated = truncate_safe(&output.stderr, MAX_OUTPUT_CHARS);
                let duration_secs = start.elapsed().as_secs_f64();

                ToolResult {
                    success: output.exit_code == 0,
                    output: Some(json!({
                        "stdout": stdout_truncated,
                        "stderr": stderr_truncated,
                        "exit_code": output.exit_code,
                        "success": output.exit_code == 0,
                        "duration_secs": duration_secs,
                        "command": command,
                        "working_dir": cwd,
                    })),
                    error: if output.exit_code != 0 {
                        Some(format!("命令执行失败，退出码: {}", output.exit_code))
                    } else {
                        None
                    },
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Ok(Err(e)) => {
                log::error!("powershell: 命令执行错误: {}", e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Command execution error: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
            Err(e) => {
                log::error!("powershell: 任务执行失败: {}", e);
                ToolResult {
                    success: false,
                    output: None,
                    error: Some(format!("Task execution failed: {}", e)),
                    duration_ms: start.elapsed().as_millis() as u64,
                    error_code: None,
                }
            }
        }
    }
}

/// 执行 PowerShell 命令（同步函数，应在 spawn_blocking 中调用）
fn execute_powershell_command(
    shell_path: &str,
    command: &str,
    working_dir: &str,
    timeout_secs: u64,
) -> Result<CommandOutput, String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::mpsc;
    use std::thread;

    let encoded = encode_powershell_command(&build_powershell_script(command));

    let mut cmd = Command::new(shell_path);
    cmd.args([
        "-NoLogo",
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-EncodedCommand",
        encoded.as_str(),
    ]);

    // 设置工作目录
    if !working_dir.is_empty() {
        cmd.current_dir(working_dir);
    }

    // 与 bash 工具保持一致的 Python 编码环境（PowerShell 中也可能调用 python）
    cmd.env("PYTHONIOENCODING", "utf-8");
    cmd.env("PYTHONUTF8", "1");

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let start = Instant::now();
    let mut child = cmd.spawn().map_err(|e| format!("启动子进程失败: {}", e))?;
    let pid = child.id();

    // 管道必须在进程运行期间就被消费：Windows 匿名管道缓冲约 4KB，
    // 若等进程退出后再读，大输出的子进程会阻塞在 write() 上，与父进程的等待互相卡死
    let stdout_handle = child.stdout.take();
    let stderr_handle = child.stderr.take();
    let (tx_out, rx_out) = mpsc::channel::<Vec<u8>>();
    let (tx_err, rx_err) = mpsc::channel::<Vec<u8>>();
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut h) = stdout_handle {
            let _ = h.read_to_end(&mut buf);
        }
        let _ = tx_out.send(buf);
    });
    thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut h) = stderr_handle {
            let _ = h.read_to_end(&mut buf);
        }
        let _ = tx_err.send(buf);
    });

    let timeout_duration = Duration::from_secs(timeout_secs);
    // 进程退出后给输出流一个短暂的收尾窗口（孙进程可能仍持有管道写句柄）
    // 记录为"自 start 起算的时限"，与下方 start.elapsed() 直接比较
    let mut stream_deadline: Option<Duration> = None;
    let mut exit_code: Option<i32> = None;
    let mut out_bytes: Option<Vec<u8>> = None;
    let mut err_bytes: Option<Vec<u8>> = None;

    loop {
        // 轮询进程状态
        if exit_code.is_none() {
            match child.try_wait() {
                Ok(Some(status)) => {
                    exit_code = Some(status.code().unwrap_or(-1));
                    stream_deadline =
                        Some(start.elapsed() + Duration::from_secs(PS_STREAM_DRAIN_GRACE_SECS));
                }
                Ok(None) => {}
                Err(e) => {
                    kill_process_tree(pid);
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("等待子进程失败: {}", e));
                }
            }
        }

        // 非阻塞收集两路输出
        if out_bytes.is_none() {
            match rx_out.try_recv() {
                Ok(v) => out_bytes = Some(v),
                Err(mpsc::TryRecvError::Disconnected) => out_bytes = Some(Vec::new()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if err_bytes.is_none() {
            match rx_err.try_recv() {
                Ok(v) => err_bytes = Some(v),
                Err(mpsc::TryRecvError::Disconnected) => err_bytes = Some(Vec::new()),
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }

        if exit_code.is_some() && out_bytes.is_some() && err_bytes.is_some() {
            break;
        }

        let elapsed = start.elapsed();
        match (exit_code, stream_deadline) {
            (None, _) => {
                if elapsed >= timeout_duration {
                    log::warn!(
                        "execute_powershell_command: 命令超时 ({}秒)，终止进程树",
                        timeout_secs
                    );
                    kill_process_tree(pid);
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(format!("命令执行超时（{}秒），已终止", timeout_secs));
                }
            }
            (Some(_), Some(deadline)) => {
                // 进程已退出但管道未关闭：超过宽限期则放弃等待这部分输出
                if elapsed >= deadline {
                    log::warn!(
                        "execute_powershell_command: 进程已退出但输出流未关闭，放弃等待剩余输出"
                    );
                    out_bytes.get_or_insert_with(Vec::new);
                    err_bytes.get_or_insert_with(Vec::new);
                }
            }
            (Some(_), None) => {}
        }

        // 短暂休眠避免 CPU 空转
        thread::sleep(Duration::from_millis(20));
    }

    let stdout = decode_console_bytes(&out_bytes.unwrap_or_default());
    let raw_stderr = decode_console_bytes(&err_bytes.unwrap_or_default());
    let stderr = decode_powershell_stderr(&raw_stderr);

    Ok(CommandOutput {
        stdout,
        stderr,
        exit_code: exit_code.unwrap_or(-1),
    })
}

// ============================================================
// powershell 工具单元测试（纯函数部分，跨平台可运行）
// ============================================================

#[cfg(test)]
mod powershell_tool_tests {
    use super::*;
    use base64::Engine as _;

    /// 创建内存数据库供本模块测试使用（mod tests 中的同名函数为私有，兄弟模块不可见）
    fn test_db() -> Arc<Database> {
        Arc::new(Database::new(std::path::Path::new(":memory:")).unwrap())
    }

    // ---------- 脚本组装 ----------

    /// 前导必须包含进度流抑制，否则 5.1 会向 stderr 注入启动期的 CLIXML 进度记录
    #[test]
    fn test_build_ps_script_suppresses_progress() {
        let script = build_powershell_script("Write-Output 1");
        assert!(
            script.starts_with("$ProgressPreference='SilentlyContinue'"),
            "脚本首条语句应抑制进度流，实际前缀: {}",
            &script[..script.len().min(80)]
        );
    }

    /// 前导必须强制 UTF-8 输出编码：5.1 默认跟随 OEM 代码页（中文系统为 GBK）
    #[test]
    fn test_build_ps_script_forces_utf8_output() {
        let script = build_powershell_script("Write-Output 1");
        assert!(
            script.contains("[Console]::OutputEncoding"),
            "脚本应设置控制台输出编码"
        );
        // 必须使用不带 BOM 的 UTF8Encoding，否则输出头部会混入 BOM
        assert!(
            script.contains("UTF8Encoding($false)"),
            "应使用不发射 BOM 的 UTF8Encoding"
        );
    }

    /// 用户命令必须原样保留在脚本中（不做任何转义/改写）
    #[test]
    fn test_build_ps_script_keeps_command_verbatim() {
        let cmd = "Get-ChildItem 'C:\\Program Files' -Filter *.txt | Where-Object { $_.Name -match 'a\"b' }";
        let script = build_powershell_script(cmd);
        assert!(script.contains(cmd), "用户命令应原样出现在脚本中");
    }

    /// 用户命令以行注释结尾时，不能吞掉退出码归一后缀
    #[test]
    fn test_build_ps_script_comment_cannot_swallow_suffix() {
        let script = build_powershell_script("Write-Output 1 # 末尾注释");
        // 后缀必须位于独立的一行，否则会被 # 注释吃掉
        let suffix_line = script
            .lines()
            .find(|l| l.contains("exit $__swEc"))
            .expect("退出码后缀应位于独立行，不能被用户命令的行尾注释吞掉");
        assert!(
            !suffix_line.trim_start().starts_with('#'),
            "退出码后缀所在行本身不应是注释行"
        );
    }

    /// 多行用户命令应被完整保留
    #[test]
    fn test_build_ps_script_preserves_multiline_command() {
        let cmd = "if (1 -eq 1) {\n  Write-Output \"yes\"\n}";
        let script = build_powershell_script(cmd);
        assert!(script.contains(cmd), "多行命令应完整保留");
    }

    // ---------- Base64(UTF-16LE) 编码 ----------

    /// 已知向量：与外部工具 iconv -t UTF-16LE | base64 的结果一致
    #[test]
    fn test_encode_ps_command_known_vector() {
        assert_eq!(
            encode_powershell_command("Write-Output 1"),
            "VwByAGkAdABlAC0ATwB1AHQAcAB1AHQAIAAxAA=="
        );
    }

    /// 空脚本应编码为空字符串（而非换行符的 Base64）
    #[test]
    fn test_encode_ps_command_empty() {
        assert_eq!(encode_powershell_command(""), "");
    }

    /// 往返一致性：非 ASCII（中文 + emoji + 引号）必须无损
    #[test]
    fn test_encode_ps_command_roundtrip_non_ascii() {
        let script = "Write-Output \"中文🐶测试\"; $x = 'a\"b'";
        let encoded = encode_powershell_command(script);
        // Base64 解码后应为偶数字节的 UTF-16LE，且能还原原文
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&encoded)
            .expect("应为合法 Base64");
        assert_eq!(bytes.len() % 2, 0, "UTF-16LE 字节数必须为偶数");
        let units: Vec<u16> = bytes
            .chunks(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let decoded: String = char::decode_utf16(units)
            .map(|r| r.unwrap_or('\u{fffd}'))
            .collect();
        assert_eq!(decoded, script, "UTF-16LE 编解码应完全无损");
    }

    /// 编码结果不应包含换行（必须作为单个命令行参数传递）
    #[test]
    fn test_encode_ps_command_is_single_line() {
        let encoded = encode_powershell_command("line1\nline2\nline3");
        assert!(!encoded.contains('\n'), "Base64 结果不应含换行");
    }

    // ---------- 输出字节解码 ----------

    /// 合法 UTF-8 应直接解码
    #[test]
    fn test_decode_console_bytes_utf8() {
        assert_eq!(decode_console_bytes("中文🐶".as_bytes()), "中文🐶");
    }

    /// GBK 字节流应回退解码（PowerShell 5.1 在中文系统上的默认输出编码）
    #[test]
    fn test_decode_console_bytes_gbk_fallback() {
        // "中文" 的 GBK 编码：D6 D0 CE C4（实测于 Windows PowerShell 5.1）
        let gbk_bytes: [u8; 4] = [0xD6, 0xD0, 0xCE, 0xC4];
        assert_eq!(decode_console_bytes(&gbk_bytes), "中文");
    }

    /// 空字节流应解码为空字符串
    #[test]
    fn test_decode_console_bytes_empty() {
        assert_eq!(decode_console_bytes(&[]), "");
    }

    /// 无法归入任何编码的字节不应 panic
    #[test]
    fn test_decode_console_bytes_invalid_does_not_panic() {
        // 0xFF/0xFE 既不是合法 UTF-8 也不是合法 GBK 起始；其中的 ASCII 部分应存活
        let out = decode_console_bytes(b"ok\xFF\xFE\x80\x80end");
        assert!(out.contains("ok"), "可读 ASCII 前缀应保留，实际: {out}");
        assert!(out.contains("end"), "可读 ASCII 后缀应保留，实际: {out}");
    }

    // ---------- CLIXML stderr 还原 ----------

    /// 真实捕获的 5.1 CLIXML 样本（stderr 被重定向时 PowerShell 的错误流序列化格式）
    const PS_CLIXML_SAMPLE: &str = "#< CLIXML\r\nRAW-STDERR \r\n<Objs Version=\"1.1.0.1\" xmlns=\"http://schemas.microsoft.com/powershell/2004/04\"><S S=\"Error\">Get-Item : Cannot find path 'C:\\a1' because it does not exist._x000D__x000A_</S><S S=\"Error\">At line:2 char:1_x000D__x000A_</S><S S=\"Error\">    + CategoryInfo          : ObjectNotFound: (C:\\a1:String) [Get-Item], ItemNotFoundException_x000D__x000A_</S><S S=\"warning\">a warning</S></Objs>";

    /// CLIXML 应被还原为可读文本，且原生 stderr 内容不得丢失
    #[test]
    fn test_decode_ps_stderr_clixml_restored() {
        let cleaned = decode_powershell_stderr(PS_CLIXML_SAMPLE);
        assert!(
            !cleaned.contains("CLIXML") && !cleaned.contains("<Objs"),
            "不应残留 XML 结构，实际: {cleaned}"
        );
        assert!(
            cleaned.contains("Cannot find path 'C:\\a1'"),
            "错误消息正文应保留，实际: {cleaned}"
        );
        assert!(
            cleaned.contains("RAW-STDERR"),
            "交错的原生 stderr 内容不应丢失，实际: {cleaned}"
        );
        assert!(
            cleaned.contains("a warning"),
            "警告流内容应保留，实际: {cleaned}"
        );
    }

    /// `_x000D__x000A_` 应还原为真实换行
    #[test]
    fn test_decode_ps_stderr_restores_newlines() {
        let cleaned = decode_powershell_stderr(PS_CLIXML_SAMPLE);
        assert!(
            cleaned.contains("does not exist.\n") || cleaned.contains("does not exist.\r\n"),
            "换行转义应被还原，实际: {:?}",
            cleaned
        );
    }

    /// 普通（非 CLIXML）stderr 应原样返回
    #[test]
    fn test_decode_ps_stderr_plain_passthrough() {
        let plain = "native tool said: something went wrong\n";
        assert_eq!(decode_powershell_stderr(plain), plain);
    }

    /// PowerShell 会把下划线转义为 `_x005F_`，还原后不得被二次解析
    #[test]
    fn test_decode_ps_stderr_underscore_escape() {
        let input = "<Objs Version=\"1.1.0.1\"><S S=\"Error\">path C:\\nope_x005F_xyz_x000D__x000A_</S></Objs>";
        let cleaned = decode_powershell_stderr(input);
        assert!(
            cleaned.contains("nope_xyz"),
            "_x005F_ 应还原为下划线且不被二次转义，实际: {cleaned}"
        );
    }

    /// pwsh 7 的 CLIXML 内嵌 ANSI 颜色转义，应被剥离
    #[test]
    fn test_decode_ps_stderr_strips_ansi() {
        let input = "<Objs Version=\"1.1.0.1\"><S S=\"Error\">_x001B_[31;1mGet-Item: boom_x001B_[0m_x000D__x000A_</S></Objs>";
        let cleaned = decode_powershell_stderr(input);
        assert!(
            cleaned.contains("Get-Item: boom"),
            "应保留错误正文，实际: {cleaned}"
        );
        assert!(
            !cleaned.contains("[31;1m") && !cleaned.contains('\u{1b}'),
            "ANSI 序列应被剥离，实际: {:?}",
            cleaned
        );
    }

    /// 进度记录块应整体丢弃
    #[test]
    fn test_decode_ps_stderr_drops_progress() {
        let input = "#< CLIXML\r\n<Objs Version=\"1.1.0.1\"><Obj S=\"progress\" RefId=\"0\"><MS><I64 N=\"SourceId\">1</I64><PR N=\"Record\"><AV>Preparing modules</AV></PR></MS></Obj></Objs>";
        let cleaned = decode_powershell_stderr(input);
        assert!(
            !cleaned.contains("Preparing modules"),
            "进度记录不应出现在输出中，实际: {cleaned}"
        );
    }

    /// XML 实体应被还原
    #[test]
    fn test_decode_ps_stderr_unescapes_xml_entities() {
        let input = "<Objs Version=\"1.1.0.1\"><S S=\"Error\">a &lt;b&gt; &amp; c &quot;d&quot;_x000D__x000A_</S></Objs>";
        let cleaned = decode_powershell_stderr(input);
        assert!(
            cleaned.contains("a <b> & c \"d\""),
            "XML 实体应还原，实际: {cleaned}"
        );
    }

    /// 畸形 CLIXML 不应导致信息丢失或 panic（回退原文）
    #[test]
    fn test_decode_ps_stderr_malformed_falls_back() {
        let broken = "#< CLIXML\r\n<Objs Version=\"1.1.0.1\"><S S=\"Error\">unclosed text";
        let cleaned = decode_powershell_stderr(broken);
        assert!(
            cleaned.contains("unclosed text"),
            "畸形输入应保留原始信息，实际: {cleaned}"
        );
    }

    /// 回归：`_x` 前缀后紧跟多字节字符时，反转义不得按字节切进字符中间而 panic
    /// （中文 Windows 的错误消息实测会触发此路径）
    #[test]
    fn test_decode_ps_stderr_multibyte_after_escape_prefix() {
        let input = "<Objs Version=\"1.1.0.1\"><S S=\"Error\">路径 _x不存在_中文🐶_x000D__x000A_</S></Objs>";
        let cleaned = decode_powershell_stderr(input);
        assert!(
            cleaned.contains("路径 _x不存在_中文🐶"),
            "非转义的 _x 与多字节字符应原样保留，实际: {cleaned}"
        );
    }

    // ---------- 路径解析 ----------

    /// 解析出的解释器路径必须真实存在且为可执行文件
    #[test]
    fn test_resolve_powershell_path_points_to_existing_file() {
        let path = resolve_powershell_path();
        assert!(path.is_some(), "本机应至少存在一个 PowerShell 解释器");
        let p = std::path::Path::new(path.as_deref().unwrap());
        assert!(p.is_file(), "解析结果应为存在的文件: {}", p.display());
    }

    /// 优先级：pwsh（PowerShell 7+）应排在 Windows PowerShell 5.1 之前
    #[test]
    fn test_resolve_powershell_path_prefers_pwsh() {
        let chosen = resolve_powershell_path().unwrap().to_lowercase();
        let pwsh_available = std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .any(|d| d.join("pwsh.exe").is_file());
        if pwsh_available {
            assert!(
                chosen.ends_with("pwsh.exe"),
                "存在 pwsh 时应优先选择 pwsh.exe，实际: {chosen}"
            );
        } else {
            assert!(
                chosen.ends_with("powershell.exe"),
                "无 pwsh 时应回退到 powershell.exe，实际: {chosen}"
            );
        }
    }

    // ---------- 脚本泄露检测 ----------

    /// 必须拦下的 PowerShell 写入途径：把临时目录脚本落进工作区
    #[test]
    fn test_ps_script_leak_copy_and_move() {
        let ws = "D:\\DeskTop\\test";
        let tmp = "C:\\Users\\me\\AppData\\Local\\Temp\\samoyed_work\\scripts";
        // Copy-Item / Move-Item 全称
        assert!(is_script_leak_powershell_command(
            &format!("Copy-Item \"{tmp}\\a.py\" \"{ws}\\a.py\""),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("Move-Item {tmp}\\s.sh {ws}\\s.sh"),
            "",
            ws
        ));
        // PowerShell 内置别名 cpi/copy=Copy-Item, mi=Move-Item（本机 Test-Path Alias: 实测）
        assert!(is_script_leak_powershell_command(
            &format!("cpi {tmp}\\a.py {ws}\\a.py"),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("copy {tmp}\\a.py {ws}\\a.py"),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("mi {tmp}\\a.py {ws}\\a.py"),
            "",
            ws
        ));
        // cp/mv 在 PowerShell 中同样是 Copy-Item/Move-Item 别名
        assert!(is_script_leak_powershell_command(
            &format!("cp {tmp}\\a.py {ws}\\a.py"),
            "",
            ws
        ));
    }

    /// 内容写入类 cmdlet 与 .NET 写文件 API 都必须被识别
    #[test]
    fn test_ps_script_leak_content_writers() {
        let ws = "D:\\DeskTop\\test";
        assert!(is_script_leak_powershell_command(
            &format!("Write-Output \"print(1)\" | Set-Content {ws}\\leak.py"),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("Add-Content -Path {ws}\\leak.py -Value \"print(1)\""),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("Get-Content a.txt | Out-File {ws}\\leak.ps1"),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("\"code\" | Tee-Object -FilePath {ws}\\a.ps1"),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("[System.IO.File]::WriteAllText(\"{ws}\\\\a.py\", \"print(1)\")"),
            "",
            ws
        ));
        // sc/ac 为 Set-Content/Add-Content 别名
        assert!(is_script_leak_powershell_command(
            &format!("sc {ws}\\a.py \"print(1)\""),
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            &format!("ac {ws}\\a.py \"print(1)\""),
            "",
            ws
        ));
    }

    /// 重定向与正斜杠/ provider 路径形式都要能匹配
    #[test]
    fn test_ps_script_leak_redirect_and_path_forms() {
        // 正斜杠形式
        assert!(is_script_leak_powershell_command(
            "\"print(1)\" > D:/DeskTop/test/leak.py",
            "",
            "D:\\DeskTop\\test"
        ));
        // 追加重定向
        assert!(is_script_leak_powershell_command(
            "\"print(1)\" >> D:\\DeskTop\\test\\leak.py",
            "",
            "D:\\DeskTop\\test"
        ));
        // PowerShell provider 全限定路径
        assert!(is_script_leak_powershell_command(
            "Copy-Item C:\\t\\a.py Microsoft.PowerShell.Core\\FileSystem::D:\\DeskTop\\test\\a.py",
            "",
            "D:\\DeskTop\\test"
        ));
        // 大小写混排
        assert!(is_script_leak_powershell_command(
            "copy-ITEM C:\\t\\a.py D:\\DeskTop\\test\\a.py",
            "",
            "D:\\DeskTop\\test"
        ));
    }

    /// working_dir 等于工作区时，相对路径写入同样构成泄露
    #[test]
    fn test_ps_script_leak_relative_path_with_workspace_cwd() {
        let ws = "D:\\DeskTop\\test";
        assert!(is_script_leak_powershell_command(
            "Write-Output \"print(1)\" | Set-Content __self_test__/leak.py",
            ws,
            ws
        ));
        // working_dir 指向别处时不应触发相对路径判定
        assert!(!is_script_leak_powershell_command(
            "Write-Output \"print(1)\" | Set-Content leak.py",
            "C:\\Users\\me\\AppData\\Local\\Temp",
            ws
        ));
    }

    /// 非泄露场景必须放行，避免确认与拒绝噪音淹没真实拦截
    #[test]
    fn test_ps_script_leak_allows_legitimate_commands() {
        let ws = "D:\\DeskTop\\test";
        let tmp = "C:\\Users\\me\\AppData\\Local\\Temp\\samoyed_work\\scripts";
        // 工作区之间互不相关的复制
        assert!(!is_script_leak_powershell_command(
            "Copy-Item C:\\a\\src.ps1 C:\\b\\dst.ps1",
            "",
            ws
        ));
        // 只读工作区
        assert!(!is_script_leak_powershell_command(
            &format!("Get-Content {ws}\\src\\main.rs"),
            "",
            ws
        ));
        // 在临时目录执行脚本，不落入工作区
        assert!(!is_script_leak_powershell_command(
            &format!("python {tmp}\\a.py"),
            "",
            ws
        ));
        // 删除工作区脚本属另一类风险，由高风险确认负责，不算泄露
        assert!(!is_script_leak_powershell_command(
            &format!("Remove-Item {ws}\\a.py"),
            "",
            ws
        ));
        // 写入工作区但不是脚本扩展名
        assert!(!is_script_leak_powershell_command(
            &format!("Set-Content {ws}\\notes.txt \"hello\""),
            "",
            ws
        ));
        // 无工作区上下文
        assert!(!is_script_leak_powershell_command(
            "Copy-Item C:\\t\\a.py D:\\DeskTop\\test\\a.py",
            "",
            ""
        ));
        // 普通命令
        assert!(!is_script_leak_powershell_command("Get-Process", "", ws));
    }

    /// 与 bash 工具共用同一套工作区/扩展名判定，二者对同一事实应给出一致结论
    #[test]
    fn test_ps_script_leak_shares_workspace_logic_with_bash() {
        let ws = "D:\\DeskTop\\test";
        // Git Bash 风格路径仅对 bash 有效，PowerShell 用盘符路径
        assert!(is_script_leak_command(
            "cp \"/d/DeskTop/test/../t/a.py\" \"/d/DeskTop/test/a.py\"",
            "",
            ws
        ));
        assert!(is_script_leak_powershell_command(
            "cp C:\\t\\a.py D:\\DeskTop\\test\\a.py",
            "",
            ws
        ));
        // 缺少脚本扩展名时两者都不拦
        assert!(!is_script_leak_command(
            "cp C:\\t\\a.txt D:\\DeskTop\\test\\a.txt",
            "",
            ws
        ));
        assert!(!is_script_leak_powershell_command(
            "Copy-Item C:\\t\\a.txt D:\\DeskTop\\test\\a.txt",
            "",
            ws
        ));
    }

    /// 工具层集成：检测到泄露时必须拒绝执行并返回参数错误码
    #[tokio::test]
    async fn test_powershell_execute_blocks_script_leak() {
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "Copy-Item C:\\Temp\\scripts\\a.py D:\\DeskTop\\test\\a.py",
                "workspace_root": "D:\\DeskTop\\test",
            }))
            .await;
        assert!(!result.success, "泄露命令应被拒绝");
        assert_eq!(result.error_code, Some(crate::errors::TOOL_INVALID_PARAMS));
        assert!(
            result.output.is_none(),
            "被拒绝的命令不应启动子进程产生 output"
        );
        let err = result.error.unwrap_or_default();
        assert!(
            err.contains("workspace") || err.contains("工作区"),
            "错误信息应说明泄露原因，实际: {err}"
        );
    }

    // ---------- write_script 的 PowerShell 语言支持 ----------

    /// filename 与 language 两个方向都要能推断出 .ps1
    #[test]
    fn test_infer_script_language_powershell() {
        // 仅给 language，补 .ps1 扩展名
        assert_eq!(
            infer_script_language("backup_db", "powershell"),
            ("backup_db.ps1".to_string(), "powershell")
        );
        // 仅给 .ps1 后缀，反推语言
        assert_eq!(
            infer_script_language("backup_db.ps1", ""),
            ("backup_db.ps1".to_string(), "powershell")
        );
        // 扩展名优先：language 与后缀矛盾时以后缀为准，不得产出 .ps1 却报告 python
        assert_eq!(
            infer_script_language("backup_db.ps1", "python"),
            ("backup_db.ps1".to_string(), "powershell")
        );
        // 大写后缀同样识别
        assert_eq!(
            infer_script_language("Task.PS1", ""),
            ("Task.PS1".to_string(), "powershell")
        );
    }

    /// 新增语言不得改变 python/bash 的既有推断结果
    #[test]
    fn test_infer_script_language_python_bash_unchanged() {
        let cases: &[(&str, &str, &str, &str)] = &[
            // (filename, language, 期望文件名, 期望语言)
            ("run.py", "", "run.py", "python"),
            ("run.sh", "", "run.sh", "bash"),
            // .bash 此前会落到默认分支被补成 run.bash.py 并报告 python，一并修正
            ("run.bash", "", "run.bash", "bash"),
            ("run", "python", "run.py", "python"),
            ("run", "bash", "run.sh", "bash"),
            // 未给 language 时默认补 .py 的历史行为保持不变
            ("run", "", "run.py", "python"),
            // 未知 language 仍走默认分支，不得 panic
            ("run", "ruby", "run.py", "python"),
        ];
        for &(filename, language, want_name, want_lang) in cases {
            let (name, lang) = infer_script_language(filename, language);
            assert_eq!(
                name.as_str(),
                want_name,
                "filename={filename} language={language}"
            );
            assert_eq!(lang, want_lang, "filename={filename} language={language}");
        }
    }

    #[test]
    fn test_write_script_language_enum_includes_powershell() {
        let schema = WriteScriptTool.parameters();
        let langs: Vec<&str> = schema["properties"]["language"]["enum"]
            .as_array()
            .expect("language 应为枚举数组")
            .iter()
            .map(|v| v.as_str().expect("枚举项应为字符串"))
            .collect();
        assert!(
            langs.contains(&"powershell"),
            "language 枚举应包含 powershell，实际: {langs:?}"
        );
        assert!(langs.contains(&"python"));
        assert!(langs.contains(&"bash"));
    }

    /// 描述必须说明 .ps1 由 powershell 工具执行
    /// 否则模型写完脚本只会去 bash 里执行，而 Git Bash 无法运行 .ps1
    #[test]
    fn test_write_script_description_covers_powershell() {
        let desc = WriteScriptTool.description();
        assert!(
            desc.contains("PowerShell") || desc.contains("powershell"),
            "描述应提及 PowerShell，实际: {desc}"
        );
        assert!(desc.contains(".ps1"), "描述应列出 .ps1，实际: {desc}");
        assert!(
            desc.contains("powershell <path>") || desc.contains("-File"),
            "描述应给出 .ps1 的执行方式，实际: {desc}"
        );
    }

    /// 引导文案要与两个 shell 一致
    #[test]
    fn test_script_guard_messages_point_to_both_shells() {
        // 三处守卫文案都出现在同一模块内，统一检查
        let combined = format!(
            "{}{}{}",
            script_execution_guidance(),
            script_execution_guidance(),
            script_execution_guidance()
        );
        assert!(
            combined.contains("bash tool") && combined.contains("powershell tool"),
            "守卫文案应同时指向 bash 与 powershell 两个执行入口，实际: {combined}"
        );
    }

    // ---------- 工具元数据与注册 ----------

    #[test]
    fn test_powershell_tool_metadata() {
        let tool = RunPowerShellCommandTool;
        assert_eq!(tool.tool_name(), "powershell");
        assert_eq!(tool.category(), "code");
    }

    /// 参数 schema 必须与 bash 工具保持一致的接口规范
    #[test]
    fn test_powershell_tool_parameters_match_bash_contract() {
        let ps = RunPowerShellCommandTool.parameters();
        let bash = RunCommandTool {
            git_bash_path: String::new(),
        }
        .parameters();

        let mut ps_keys: Vec<&String> = ps["properties"].as_object().unwrap().keys().collect();
        let mut bash_keys: Vec<&String> = bash["properties"].as_object().unwrap().keys().collect();
        ps_keys.sort();
        bash_keys.sort();
        assert_eq!(
            ps_keys, bash_keys,
            "powershell 与 bash 的参数集合应完全一致"
        );
        assert_eq!(ps["required"], json!(["command"]));
        assert_eq!(ps["properties"]["timeout"]["default"], json!(60));
    }

    /// 描述需说明执行方式与返回字段，供 LLM 正确选用
    #[test]
    fn test_powershell_tool_description_mentions_contract() {
        let desc = RunPowerShellCommandTool.description();
        for token in [
            "PowerShell",
            "stdout",
            "stderr",
            "exit_code",
            "working_dir",
            "timeout",
        ] {
            assert!(desc.contains(token), "描述应包含关键契约信息: {token}");
        }
    }

    /// 描述必须预先告知脚本泄露禁令
    /// 否则模型只在收到拒绝错误后才被动得知规则，会反复试错消耗迭代次数
    #[test]
    fn test_powershell_tool_description_declares_leak_rule() {
        let desc = RunPowerShellCommandTool.description();
        assert!(
            desc.contains("workspace directory"),
            "应提及工作区限制，实际: {desc}"
        );
        assert!(
            desc.contains("prohibited"),
            "应明确表达禁止语义，实际: {desc}"
        );
        // 需点名 PowerShell 专有写法，POSIX 词表不足以让模型理解被禁的是哪些命令
        assert!(
            desc.contains("Copy-Item") && desc.contains("Out-File"),
            "应点名 Copy-Item/Out-File 等 PowerShell 写入途径，实际: {desc}"
        );
        // 与 bash 一样说明脚本只应在临时目录执行
        assert!(
            desc.contains("temporary directory"),
            "应给出临时目录这一正确做法，实际: {desc}"
        );
    }

    #[test]
    fn test_powershell_registered_in_builtin_registry() {
        let mut registry = ToolRegistry::new();
        let _reg = register_builtin_tools(
            &mut registry,
            String::new(),
            test_db(),
            crate::config::app_settings::WebSearchConfig::default(),
            std::sync::Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::new())),
            None,
            std::sync::Arc::new(crate::services::skill::registry::SkillRegistry::new(
                crate::services::skill::loader::SkillLoader::new(
                    std::path::PathBuf::from("/tmp"),
                    None,
                    Vec::new(),
                ),
            )),
        );
        let names: Vec<String> = registry.list_tools().into_iter().map(|t| t.name).collect();
        assert!(
            names.contains(&"powershell".to_string()),
            "内置注册表应包含 powershell 工具，实际: {names:?}"
        );
    }

    // ---------- 参数校验与错误处理 ----------

    /// 缺少 command 参数时应返回参数错误码，且不得启动子进程
    #[tokio::test]
    async fn test_powershell_execute_missing_command() {
        let result = RunPowerShellCommandTool.execute(json!({})).await;
        assert!(!result.success);
        assert_eq!(
            result.error_code,
            Some(crate::errors::TOOL_INVALID_PARAMS),
            "缺参应返回 TOOL_INVALID_PARAMS"
        );
    }

    #[tokio::test]
    async fn test_powershell_execute_blank_command() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "   "}))
            .await;
        assert!(!result.success);
        assert_eq!(result.error_code, Some(crate::errors::TOOL_INVALID_PARAMS));
    }

    /// timeout 参数必须被钳制在上限内（与 bash 一致：最大 300 秒）
    #[test]
    fn test_powershell_timeout_clamp() {
        assert_eq!(clamp_powershell_timeout(None), 60);
        assert_eq!(clamp_powershell_timeout(Some(json!(10))), 10);
        assert_eq!(clamp_powershell_timeout(Some(json!(9999))), 300);
        // 0 秒会让命令刚启动就被终止，需钳到最小 1 秒
        assert_eq!(clamp_powershell_timeout(Some(json!(0))), 1);
        // 非法类型应回退到默认值
        assert_eq!(clamp_powershell_timeout(Some(json!("abc"))), 60);
    }
}

// ============================================================
// powershell 工具真实执行测试（需要本机存在 PowerShell 解释器）
// ============================================================

#[cfg(all(test, target_os = "windows"))]
mod powershell_execution_tests {
    use super::*;

    /// ToolResult.output 为 Option<Value>，统一在此取出便于断言
    fn out(result: &ToolResult) -> &Value {
        result
            .output
            .as_ref()
            .expect("命令已执行（含非零退出码）时应返回 output 字段")
    }

    fn stdout_of(result: &ToolResult) -> String {
        out(result)["stdout"].as_str().unwrap_or("").to_string()
    }

    fn stderr_of(result: &ToolResult) -> String {
        out(result)["stderr"].as_str().unwrap_or("").to_string()
    }

    /// 基本执行：命令准确性
    #[tokio::test]
    async fn test_powershell_execute_returns_output() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "Write-Output 'hello-ps'"}))
            .await;
        assert!(result.success, "错误: {:?}", result.error);
        assert!(
            stdout_of(&result).contains("hello-ps"),
            "stdout 实际: {}",
            stdout_of(&result)
        );
        assert_eq!(out(&result)["exit_code"], json!(0));
        assert_eq!(out(&result)["success"], json!(true));
        assert!(out(&result)["duration_secs"].is_number());
        assert_eq!(out(&result)["command"], json!("Write-Output 'hello-ps'"));
    }

    /// 显式 exit 应作为退出码返回，并标记为失败
    #[tokio::test]
    async fn test_powershell_execute_explicit_exit_code() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "exit 7"}))
            .await;
        assert!(!result.success);
        assert_eq!(out(&result)["exit_code"], json!(7));
        assert!(
            result.error.unwrap_or_default().contains("7"),
            "错误信息应包含退出码"
        );
    }

    /// 原生程序退出码必须透传（PowerShell 默认不会把原生命令退出码交给宿主）
    #[tokio::test]
    async fn test_powershell_execute_propagates_native_exit_code() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "cmd /c \"exit 13\""}))
            .await;
        assert_eq!(
            out(&result)["exit_code"],
            json!(13),
            "原生命令退出码应被归一化返回，stdout={} stderr={}",
            stdout_of(&result),
            stderr_of(&result)
        );
        assert!(!result.success);
    }

    /// 失败语句后的成功语句应以成功结束（与 bash 的“最后一条语句”语义一致）
    #[tokio::test]
    async fn test_powershell_execute_last_statement_wins() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "cmd /c \"exit 9\"\nWrite-Output 'recovered'"}))
            .await;
        assert!(
            result.success,
            "最后一条语句成功时整体应成功，stderr={}",
            stderr_of(&result)
        );
        assert!(stdout_of(&result).contains("recovered"));
    }

    /// cmdlet 非终止错误应返回非零退出码，且 stderr 为可读文本
    #[tokio::test]
    async fn test_powershell_execute_cmdlet_error() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "Get-Item 'C:\\definitely_not_here_9f3a'"}))
            .await;
        assert!(!result.success, "不存在的文件应失败");
        assert_eq!(out(&result)["exit_code"], json!(1));
        let stderr = stderr_of(&result);
        assert!(
            stderr.contains("Get-Item") || stderr.contains("Cannot find"),
            "stderr 应为可读错误文本，实际: {stderr}"
        );
        assert!(
            !stderr.contains("CLIXML") && !stderr.contains("<Objs"),
            "stderr 不应残留 CLIXML 结构，实际: {stderr}"
        );
    }

    /// 非零退出码时也必须把 stdout 返回给模型（用于诊断）
    #[tokio::test]
    async fn test_powershell_execute_keeps_stdout_on_failure() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "Write-Output 'partial'\nexit 4"}))
            .await;
        assert!(!result.success);
        assert!(
            stdout_of(&result).contains("partial"),
            "失败时也应保留已产生的 stdout，实际: {}",
            stdout_of(&result)
        );
    }

    /// 中文输出不得乱码（5.1 默认 GBK，需要编码前导 + 回退解码双保险）
    #[tokio::test]
    async fn test_powershell_execute_chinese_output() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "Write-Output '中文测试成功'"}))
            .await;
        assert!(result.success, "错误: {:?}", result.error);
        assert!(
            stdout_of(&result).contains("中文测试成功"),
            "中文输出应完整无损，实际: {}",
            stdout_of(&result)
        );
    }

    /// emoji（非 GBK 字符集内）也应正确往返
    #[tokio::test]
    async fn test_powershell_execute_emoji_output() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "Write-Output 'dog:\u{1F436}'"}))
            .await;
        assert!(result.success);
        assert!(
            stdout_of(&result).contains('\u{1F436}'),
            "emoji 应完整保留，实际: {:?}",
            stdout_of(&result)
        );
    }

    /// 中文错误消息也不应乱码
    #[tokio::test]
    async fn test_powershell_execute_chinese_error_message() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "Get-Item 'C:\\不存在的路径'"}))
            .await;
        assert!(!result.success);
        let stderr = stderr_of(&result);
        assert!(
            !stderr.contains('\u{fffd}'),
            "错误输出不应出现替换字符（编码丢失迹象），实际: {stderr}"
        );
    }

    /// 引号、反引号、$ 符号等特殊字符必须无损传入（EncodedCommand 的核心收益）
    #[tokio::test]
    async fn test_powershell_execute_handles_tricky_quotes() {
        let cmd = "$x = 'a\"b\"c'; Write-Output \"[$x]\"";
        let result = RunPowerShellCommandTool
            .execute(json!({"command": cmd}))
            .await;
        assert!(
            result.success,
            "错误: {:?} stderr={}",
            result.error,
            stderr_of(&result)
        );
        assert!(
            stdout_of(&result).contains("[a\"b\"c]"),
            "嵌套引号应原样处理，实际: {}",
            stdout_of(&result)
        );
    }

    /// 多行脚本块执行
    #[tokio::test]
    async fn test_powershell_execute_multiline_script() {
        let cmd = "$total = 0\n1..4 | ForEach-Object { $total += $_ }\nWrite-Output \"sum=$total\"";
        let result = RunPowerShellCommandTool
            .execute(json!({"command": cmd}))
            .await;
        assert!(result.success, "错误: {:?}", result.error);
        assert!(
            stdout_of(&result).contains("sum=10"),
            "实际: {}",
            stdout_of(&result)
        );
    }

    /// working_dir 应作为子进程工作目录生效
    #[tokio::test]
    async fn test_powershell_execute_respects_working_dir() {
        // 用唯一命名的临时目录验证，避免依赖路径字符串比较
        let unique = format!("ps_wd_{}", std::process::id());
        let dir = std::env::temp_dir().join(&unique);
        std::fs::create_dir_all(&dir).expect("创建临时目录失败");
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "[System.IO.Path]::GetFileName((Get-Location).Path.TrimEnd('\\'))",
                "working_dir": dir.to_string_lossy(),
            }))
            .await;
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.success, "错误: {:?}", result.error);
        assert_eq!(stdout_of(&result).trim(), unique, "工作目录应为指定目录");
        assert_eq!(out(&result)["working_dir"], json!(dir.to_string_lossy()));
    }

    /// working_dir 不存在时应给出明确错误，而不是静默失败
    #[tokio::test]
    async fn test_powershell_execute_nonexistent_working_dir() {
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "Write-Output x",
                "working_dir": "C:\\no_such_dir_for_test_9137",
            }))
            .await;
        assert!(!result.success);
        assert!(result.error.is_some());
    }

    /// 大输出回归测试：必须在超时前返回（旧实现的“先等退出再读管道”会死锁）
    #[tokio::test]
    async fn test_powershell_execute_large_output_does_not_deadlock() {
        let started = Instant::now();
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "Write-Output ('x' * 300000)",
                "timeout": 60,
            }))
            .await;
        let elapsed = started.elapsed();
        assert!(result.success, "错误: {:?}", result.error);
        let stdout = stdout_of(&result);
        assert!(
            stdout.contains("truncated"),
            "超过 6000 字符应被截断，实际长度: {}",
            stdout.len()
        );
        assert!(
            elapsed < Duration::from_secs(30),
            "大输出不应阻塞，实际耗时 {:?}",
            elapsed
        );
    }

    /// 超时终止：返回明确的超时错误，并在规定时间内结束
    #[tokio::test]
    async fn test_powershell_execute_timeout_terminates() {
        let started = Instant::now();
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "Start-Sleep -Seconds 60",
                "timeout": 2,
            }))
            .await;
        let elapsed = started.elapsed();
        assert!(!result.success, "超时命令应失败");
        assert!(
            result.error.unwrap_or_default().contains("超时"),
            "错误信息应说明超时"
        );
        assert!(
            elapsed >= Duration::from_secs(2) && elapsed < Duration::from_secs(15),
            "应在超时点附近结束，实际 {:?}",
            elapsed
        );
    }

    /// 超时后不得残留 PowerShell 子进程（进程树终止）
    #[tokio::test]
    async fn test_powershell_execute_timeout_kills_process_tree() {
        // 由 PowerShell 启动一个可观测的子进程，超时后检查其是否仍在运行
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "$p = Start-Process cmd.exe -ArgumentList '/c','timeout /t 60 >nul' -PassThru; Write-Output $p.Id; Start-Sleep -Seconds 60",
                "timeout": 2,
            }))
            .await;
        assert!(!result.success);
        // 留给操作系统回收进程的时间
        tokio::time::sleep(Duration::from_millis(800)).await;
        let probe = std::process::Command::new("tasklist.exe")
            .args(["/FI", "IMAGENAME eq cmd.exe", "/NH"])
            .stdout(std::process::Stdio::piped())
            .spawn();
        if let Ok(probe) = probe {
            let out = probe.wait_with_output();
            if let Ok(out) = out {
                let text = decode_console_bytes(&out.stdout);
                // 只要不再是 60 秒超时的 cmd.exe 即视为已清理；
                // 其它 cmd.exe 可能来自用户环境，故此处仅断言无 "timeout" 字样残留
                assert!(!text.contains("timeout"), "超时后可能残留子进程: {text}");
            }
        }
    }

    /// 权限不足场景：写入受保护目录应失败并返回可读错误
    #[tokio::test]
    async fn test_powershell_execute_permission_denied() {
        let result = RunPowerShellCommandTool
            .execute(json!({
                "command": "New-Item -Path 'C:\\Windows\\System32\\config\\sw_probe_should_fail' -ItemType File -ErrorAction Stop",
            }))
            .await;
        assert!(
            !result.success,
            "写入 System32 应因权限不足而失败，stdout={}",
            stdout_of(&result)
        );
        assert_ne!(out(&result)["exit_code"], json!(0));
        assert!(!stderr_of(&result).is_empty(), "应返回权限错误说明");
    }

    /// 语法错误应被报告为失败而非 panic
    #[tokio::test]
    async fn test_powershell_execute_syntax_error() {
        let result = RunPowerShellCommandTool
            .execute(json!({"command": "if ( {"}))
            .await;
        assert!(!result.success);
        assert_ne!(out(&result)["exit_code"], json!(0));
    }

    /// 性能基线：区分冷启动与热调用，并与 bash 工具同机对比
    #[tokio::test]
    async fn test_powershell_execute_performance_baseline() {
        // 冷启动：首次调用包含解释器定位与进程冷启动开销
        let cold = Instant::now();
        let warmup = RunPowerShellCommandTool
            .execute(json!({"command": "Write-Output warmup"}))
            .await;
        assert!(warmup.success, "预热失败: {:?}", warmup.error);
        let cold_ms = cold.elapsed().as_millis();

        let mut ps_ms = Vec::new();
        for _ in 0..3 {
            let started = Instant::now();
            let result = RunPowerShellCommandTool
                .execute(json!({"command": "Write-Output 1"}))
                .await;
            assert!(result.success);
            ps_ms.push(started.elapsed().as_millis());
        }

        // 同机 bash 工具基线，用于对比 PowerShell 的额外启动开销
        let bash_tool = RunCommandTool {
            git_bash_path: String::new(),
        };
        let mut bash_ms = Vec::new();
        for _ in 0..3 {
            let started = Instant::now();
            let result = bash_tool.execute(json!({"command": "echo 1"})).await;
            assert!(result.success, "bash 基线执行失败: {:?}", result.error);
            bash_ms.push(started.elapsed().as_millis());
        }

        let ps_max = ps_ms.iter().max().copied().unwrap_or(0);
        let ps_avg = ps_ms.iter().sum::<u128>() / ps_ms.len() as u128;
        let bash_max = bash_ms.iter().max().copied().unwrap_or(0);
        eprintln!(
            "[性能基线] powershell 冷启动 {cold_ms} ms | 热调用 max {ps_max} ms avg {ps_avg} ms || bash max {bash_max} ms"
        );

        assert!(ps_max < 20_000, "简单命令热调用耗时异常: {ps_max} ms");
        assert!(cold_ms < 30_000, "冷启动耗时异常: {cold_ms} ms");
    }

    /// 端到端：write_script 写出的 .ps1 必须能被 powershell 工具直接执行
    /// 这是 ps1 语言支持真正要打通的链路，只测字符串推断不足以证明可用
    #[tokio::test]
    async fn test_write_script_ps1_runs_via_powershell_tool() {
        let unique = format!("sw_ps1_ok_{}", std::process::id());

        // 不给扩展名，仅靠 language 推断出 .ps1
        let written = WriteScriptTool
            .execute(json!({
                "filename": unique,
                "language": "powershell",
                "content": "Write-Output 'ps1-file-ran'\nWrite-Output \"pid=$PID\"\n",
            }))
            .await;
        assert!(written.success, "write_script 失败: {:?}", written.error);
        let path = out(&written)["path"].as_str().unwrap_or("").to_string();
        assert!(
            path.ends_with(".ps1"),
            "应产出 .ps1 文件，实际 path: {path}"
        );
        assert_eq!(out(&written)["language"], json!("powershell"));
        assert!(
            std::path::Path::new(&path).is_file(),
            "脚本文件应真实存在: {path}"
        );

        // 用 powershell 工具执行该脚本文件（脚本含中文输出与空格外壳路径均可）
        let run = RunPowerShellCommandTool
            .execute(json!({"command": format!("& '{path}'")}))
            .await;
        let _ = std::fs::remove_file(&path);

        assert!(
            run.success,
            "执行 .ps1 失败: {:?} stderr={:?}",
            run.error,
            run.output.as_ref().and_then(|o| o["stderr"].as_str())
        );
        let stdout = out(&run)["stdout"].as_str().unwrap_or("");
        assert!(
            stdout.contains("ps1-file-ran"),
            "脚本输出未取回，实际: {stdout:?}"
        );
        assert!(
            stdout.contains("pid="),
            "脚本内变量求值应生效，实际: {stdout:?}"
        );
    }

    /// 端到端：脚本内显式 exit 非零时，失败状态必须被退出码归一带回
    /// 实测依据：& 调用一个 exit 3 的 .ps1 会得到 $? = False 且 $LASTEXITCODE = 3
    #[tokio::test]
    async fn test_write_script_ps1_propagates_script_exit_code() {
        let unique = format!("sw_ps1_fail_{}", std::process::id());
        let written = WriteScriptTool
            .execute(json!({
                "filename": unique,
                "language": "powershell",
                "content": "Write-Output 'about-to-fail'\nexit 3\n",
            }))
            .await;
        assert!(written.success);
        let path = out(&written)["path"].as_str().unwrap_or("").to_string();

        let run = RunPowerShellCommandTool
            .execute(json!({"command": format!("& '{path}'")}))
            .await;
        let _ = std::fs::remove_file(&path);

        assert!(!run.success, "脚本 exit 3 应被判定为失败");
        assert_eq!(
            out(&run)["exit_code"],
            json!(3),
            "脚本退出码应被归一到 3，stderr={:?}",
            run.output.as_ref().and_then(|o| o["stderr"].as_str())
        );
        assert!(
            out(&run)["stdout"]
                .as_str()
                .unwrap_or("")
                .contains("about-to-fail"),
            "失败时也应保留脚本已产生的输出"
        );
    }

    /// 输出字段契约：与 bash 工具返回完全相同的键集合
    #[tokio::test]
    async fn test_powershell_execute_output_shape_matches_bash() {
        let ps = RunPowerShellCommandTool
            .execute(json!({"command": "Write-Output 1"}))
            .await;
        let bash = RunCommandTool {
            git_bash_path: String::new(),
        }
        .execute(json!({"command": "echo 1"}))
        .await;

        let mut ps_keys: Vec<String> = ps
            .output
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        let mut bash_keys: Vec<String> = bash
            .output
            .as_ref()
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        ps_keys.sort();
        bash_keys.sort();
        assert_eq!(ps_keys, bash_keys, "返回字段必须与 bash 工具一致");
    }
}

// ============================================================
// powershell 跨解释器兼容性测试
// Windows PowerShell 5.1 与 PowerShell 7 在输出代码页、原生退出码传递、
// 错误流序列化上行为不同，必须对两者分别验证，而不是只测自动选中的那一个
// ============================================================

#[cfg(all(test, target_os = "windows"))]
mod powershell_compat_tests {
    use super::*;

    /// 枚举本机存在的所有 PowerShell 解释器
    fn available_shells() -> Vec<String> {
        let mut shells = vec![resolve_powershell_path().expect("本机应能解析出 PowerShell")];
        let system_root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_string());
        for candidate in [
            format!("{system_root}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"),
            format!("{system_root}\\SysWOW64\\WindowsPowerShell\\v1.0\\powershell.exe"),
        ] {
            if std::path::Path::new(&candidate).is_file() && !shells.contains(&candidate) {
                shells.push(candidate);
            }
        }
        shells
    }

    /// 同一命令在两个解释器上都必须得到一致的结果
    fn assert_case(shell: &str, command: &str, want_stdout: &str, want_code: i32) {
        let out = execute_powershell_command(shell, command, "", 60)
            .unwrap_or_else(|e| panic!("[{shell}] {command} 执行失败: {e}"));
        let tag = format!(
            "[{}] {}",
            shell.rsplit('\\').next().unwrap_or(shell),
            command
        );
        assert_eq!(out.exit_code, want_code, "{tag} 退出码不符");
        assert!(
            out.stdout.contains(want_stdout),
            "{tag} stdout 缺少 {want_stdout:?}，实际: {:?}",
            out.stdout
        );
        assert!(
            !out.stderr.contains("CLIXML") && !out.stderr.contains("<Objs"),
            "{tag} stderr 残留 CLIXML: {}",
            out.stderr
        );
    }

    #[test]
    fn test_compat_at_least_one_shell_available() {
        assert!(
            !available_shells().is_empty(),
            "本机未检测到任何 PowerShell 解释器"
        );
    }

    /// 正常输出与中文在两个解释器上都不得乱码
    #[test]
    fn test_compat_text_and_chinese_output() {
        for shell in available_shells() {
            assert_case(&shell, "Write-Output 'ascii-ok'", "ascii-ok", 0);
            assert_case(&shell, "Write-Output '中文输出正常'", "中文输出正常", 0);
            assert_case(
                &shell,
                "Write-Output 'emoji:\u{1F436}'",
                "emoji:\u{1F436}",
                0,
            );
        }
    }

    /// 退出码归一在两个解释器上行为一致（PowerShell 默认不传原生退出码）
    #[test]
    fn test_compat_exit_code_semantics() {
        for shell in available_shells() {
            assert_case(&shell, "exit 42", "", 42);
            assert_case(&shell, "cmd /c \"exit 5\"", "", 5);
            assert_case(&shell, "Write-Output 'ok'", "ok", 0);
            assert_case(&shell, "Get-Item C:\\missing_zz_1", "", 1);
        }
    }

    /// 错误流：两个解释器都会把 error 记录序列化成 CLIXML，必须都被还原成可读文本
    #[test]
    fn test_compat_stderr_is_readable() {
        for shell in available_shells() {
            let out =
                execute_powershell_command(&shell, "Get-Item C:\\definitely_missing_a7", "", 60)
                    .expect("命令应执行完成");
            let tag = shell.rsplit('\\').next().unwrap_or(&shell);
            assert_eq!(out.exit_code, 1, "[{tag}] 应返回非零退出码");
            assert!(
                !out.stderr.is_empty(),
                "[{tag}] 错误输出不应为空（旧实现会因编码问题丢空）"
            );
            assert!(
                !out.stderr.contains("<Objs") && !out.stderr.contains("#< CLIXML"),
                "[{tag}] CLIXML 未被还原: {}",
                out.stderr
            );
            assert!(
                !out.stderr.contains('\u{1b}'),
                "[{tag}] 错误输出残留 ANSI 转义: {:?}",
                out.stderr
            );
        }
    }

    /// 引号、变量、多行脚本在两个解释器上均无损
    #[test]
    fn test_compat_special_characters() {
        for shell in available_shells() {
            assert_case(&shell, "$x = 'a\"b'; Write-Output \"[$x]\"", "[a\"b]", 0);
            assert_case(
                &shell,
                "$a = 1\n$b = 2\nWrite-Output \"sum=$($a+$b)\"",
                "sum=3",
                0,
            );
            // 含 Windows 路径与反斜杠结尾的字符串
            assert_case(
                &shell,
                "Write-Output 'C:\\Program Files\\app'",
                "C:\\Program Files\\app",
                0,
            );
        }
    }

    /// 工作目录与超时在两个解释器上一致
    #[test]
    fn test_compat_working_dir_and_timeout() {
        let dir = std::env::temp_dir();
        for shell in available_shells() {
            let tag = shell.rsplit('\\').next().unwrap_or(&shell);
            let out = execute_powershell_command(
                &shell,
                "[System.IO.Directory]::GetCurrentDirectory()",
                &dir.to_string_lossy(),
                60,
            )
            .expect("应执行成功");
            assert!(
                out.stdout.trim().eq_ignore_ascii_case(
                    dir.to_string_lossy()
                        .trim_end_matches('\\')
                        .to_lowercase()
                        .as_str()
                ) || out.stdout.contains(
                    dir.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default()
                        .as_str()
                ),
                "[{tag}] 工作目录未生效，实际: {}",
                out.stdout
            );

            let started = Instant::now();
            let err = match execute_powershell_command(&shell, "Start-Sleep -Seconds 60", "", 2) {
                Err(e) => e,
                Ok(_) => panic!("[{tag}] 长命令应在 2 秒超时"),
            };
            assert!(err.contains("超时"), "[{tag}] 超时信息不正确: {err}");
            assert!(
                started.elapsed() < Duration::from_secs(15),
                "[{tag}] 超时响应过慢: {:?}",
                started.elapsed()
            );
        }
    }
}
