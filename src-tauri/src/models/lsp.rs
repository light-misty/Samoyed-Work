//! LSP 模型定义
//! 定义 LSP 服务器配置、客户端状态等类型

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// LSP 服务器配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerConfig {
    /// 语言名称(如 "rust", "python", "typescript")
    pub language: String,
    /// 启动命令(如 ["rust-analyzer"], ["pylsp"], ["typescript-language-server", "--stdio"])
    pub command: Vec<String>,
    /// 根目录标识文件(如 ["Cargo.toml"], ["pyproject.toml"], ["tsconfig.json"])
    pub root_patterns: Vec<String>,
    /// 初始化选项(可选,传递给 LSP 服务器的 initializationOptions)
    #[serde(default)]
    pub initialization_options: Option<serde_json::Value>,
}

/// LSP 服务器状态
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LspServerStatus {
    /// 未启动
    Stopped,
    /// 启动中
    Starting,
    /// 已就绪(初始化完成)
    Ready,
    /// 错误状态
    Error,
    /// 已停止(手动或崩溃)
    Terminated,
}

/// LSP 服务器运行时信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspServerInfo {
    /// 语言名称
    pub language: String,
    /// 服务器名称(从 initialize 响应获取)
    pub server_name: Option<String>,
    /// 服务器版本
    pub server_version: Option<String>,
    /// 工作区根目录
    pub workspace_root: PathBuf,
    /// 当前状态
    pub status: LspServerStatus,
    /// 支持的能力(从 initialize 响应获取)
    pub capabilities: Option<serde_json::Value>,
    /// 启动时间(UNIX 时间戳,毫秒)
    pub started_at: u64,
    /// 最后活动时间(UNIX 时间戳,毫秒)
    pub last_activity_at: u64,
    /// 错误信息(状态为 Error 时)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// LSP 位置信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspLocation {
    /// 文件 URI(如 "file:///path/to/file.rs")
    pub uri: String,
    /// 文件路径(从 URI 解析)
    pub file_path: String,
    /// 起始位置
    pub start_line: u32,
    pub start_character: u32,
    /// 结束位置
    pub end_line: u32,
    pub end_character: u32,
}

/// LSP 诊断信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspDiagnostic {
    /// 诊断来源(如 "rustc", "pylint")
    pub source: Option<String>,
    /// 严重级别: 1=Error, 2=Warning, 3=Information, 4=Hint
    pub severity: u8,
    /// 诊断消息
    pub message: String,
    /// 位置
    pub location: LspLocation,
    /// 诊断代码(可选)
    pub code: Option<String>,
}

/// LSP 悬停信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspHover {
    /// 悬停内容(Markdown 格式)
    pub content: String,
    /// 内容范围(可选)
    pub range: Option<LspLocation>,
}

/// LSP 符号信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LspSymbol {
    /// 符号名称
    pub name: String,
    /// 符号类型(1=File, 2=Module, 3=Namespace, 4=Package, 5=Class, 6=Method, 7=Property, 8=Field, 9=Constructor, 10=Enum, 11=Interface, 12=Function, 13=Variable, 14=Constant)
    pub kind: u8,
    /// 符号位置
    pub location: LspLocation,
    /// 详细信息(可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 文档(可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub documentation: Option<String>,
}

/// LSP 调用层级项(prepareCallHierarchy 返回的单个项)
/// 参照 LSP 规范 CallHierarchyItem 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyItem {
    /// 符号名称
    pub name: String,
    /// 符号类型(复用 symbol_kind_name 的类型枚举)
    pub kind: u8,
    /// 文件 URI
    pub uri: String,
    /// 文件路径(从 URI 解析)
    pub file_path: String,
    /// 符号的完整范围
    pub range: LspLocation,
    /// 符号被选中时的范围(通常包含在 range 内)
    pub selection_range: LspLocation,
    /// 详细信息(可选)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// 标签(可选,LSP 1=Deprecated)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<u8>>,
}

/// LSP 调用层级调用项(incomingCalls/outgoingCalls 返回的单个项)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallHierarchyCall {
    /// 调用方向("incoming" 或 "outgoing")
    pub direction: String,
    /// 调用方(incoming 时为此项,即谁调用了目标符号)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<CallHierarchyItem>,
    /// 被调用方(outgoing 时为此项,即目标符号调用了谁)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<CallHierarchyItem>,
    /// 调用发生的位置范围列表(incoming: from 中调用目标的位置;outgoing: 目标中调用 to 的位置)
    pub from_ranges: Vec<LspLocation>,
}

/// 严重级别名称
pub fn severity_name(severity: u8) -> &'static str {
    match severity {
        1 => "Error",
        2 => "Warning",
        3 => "Information",
        4 => "Hint",
        _ => "Unknown",
    }
}

/// 符号类型名称
pub fn symbol_kind_name(kind: u8) -> &'static str {
    match kind {
        1 => "File",
        2 => "Module",
        3 => "Namespace",
        4 => "Package",
        5 => "Class",
        6 => "Method",
        7 => "Property",
        8 => "Field",
        9 => "Constructor",
        10 => "Enum",
        11 => "Interface",
        12 => "Function",
        13 => "Variable",
        14 => "Constant",
        _ => "Unknown",
    }
}
