// ===== LSP 相关类型定义 - 与 Rust 后端对齐 =====
// 对应 src-tauri/src/models/lsp.rs
// Rust 端使用 #[serde(rename_all = "camelCase")]，前端字段为 camelCase
// LspServerStatus 使用 #[serde(rename_all = "lowercase")]，前端为小写字符串

/** LSP 服务器状态（与 Rust LspServerStatus 枚举对齐，小写序列化） */
export type LspServerStatus = "stopped" | "starting" | "ready" | "error" | "terminated";

/** LSP 服务器运行时信息（与 Rust LspServerInfo 对齐） */
export interface LspServerInfo {
  /** 语言名称 */
  language: string;
  /** 服务器名称（从 initialize 响应获取） */
  serverName?: string;
  /** 服务器版本 */
  serverVersion?: string;
  /** 工作区根目录 */
  workspaceRoot: string;
  /** 当前状态 */
  status: LspServerStatus;
  /** 支持的能力（从 initialize 响应获取） */
  capabilities?: unknown;
  /** 启动时间（UNIX 时间戳，毫秒） */
  startedAt: number;
  /** 最后活动时间（UNIX 时间戳，毫秒） */
  lastActivityAt: number;
  /** 错误信息（状态为 error 时存在） */
  error?: string;
}
