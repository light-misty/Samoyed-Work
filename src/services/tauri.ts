/**
 * Tauri 命令调用封装
 * 为每个后端 Tauri 命令提供对应的 TypeScript 异步函数
 * 函数名使用 camelCase，调用 invoke 时使用 snake_case 命令名
 */
import { invoke } from "@tauri-apps/api/core";

// ================================================================
// 类型定义 - 与 Rust 端 serde camelCase 输出一致
// ================================================================

/** 连接测试结果 */
export interface ConnectionResult {
  success: boolean;
  latencyMs: number;
  modelInfo?: ModelInfo;
  errorMessage?: string;
}

/** 模型信息 */
export interface ModelInfo {
  modelName: string;
  maxTokens: number;
  supportsStreaming: boolean;
  supportsToolCall: boolean;
}

/** Provider 配置（用于添加/更新） */
export interface ProviderConfig {
  name: string;
  providerType: string;
  apiBase: string;
  apiKey: string;
  model: string;
  extraParams?: Record<string, unknown>;
}

/** Provider 信息 */
export interface ProviderInfo {
  id: string;
  name: string;
  providerType: string;
  apiBase: string;
  model: string;
  isDefault: boolean;
  isAvailable: boolean;
  createdAt: string;
}

/** 创建会话参数 */
export interface CreateSessionParams {
  title?: string;
  workspaceId?: string;
  providerId?: string;
  templateId?: string;
}

/** 会话筛选条件 */
export interface SessionFilter {
  workspaceId?: string;
  status?: string;
  search?: string;
  limit?: number;
  offset?: number;
}

/** 会话信息 */
export interface Session {
  id: string;
  title: string;
  workspaceId?: string;
  providerId: string;
  templateId?: string;
  createdAt: string;
  updatedAt: string;
  status: string;
}

/** 会话摘要 */
export interface SessionSummary {
  id: string;
  title: string;
  status: string;
  messageCount: number;
  lastMessagePreview?: string;
  createdAt: string;
  updatedAt: string;
}

/** 会话详情 */
export interface SessionDetail {
  session: Session;
  messages: Message[];
  tokenUsage: TokenUsage;
}

/** 消息 */
export interface Message {
  id: string;
  role: string;
  content: string;
  toolCalls?: ToolCall[];
  createdAt: string;
}

/** 工具调用 */
export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  result?: unknown;
}

/** Token 用量 */
export interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

/** 工作区信息 */
export interface WorkspaceInfo {
  id: string;
  name: string;
  path: string;
  isActive: boolean;
  fileCount: number;
  createdAt: string;
  lastAccessed: string;
}

/** 文件树节点 */
export interface FileNode {
  name: string;
  path: string;
  isDir: boolean;
  size?: number;
  modified?: string;
  extension?: string;
  children?: FileNode[];
}

/** 搜索选项 */
export interface SearchOptions {
  extensions?: string[];
  maxResults?: number;
  includeContent?: boolean;
}

/** 搜索结果 */
export interface SearchResult {
  path: string;
  name: string;
  extension: string;
  size: number;
  modified: string;
  matchType: string;
  matchPreview?: string;
  lineNumber?: number;
}

/** 文档预览内容 */
export interface PreviewContent {
  path: string;
  fileType: string;
  content: string;
  pageCount?: number;
  sheetNames?: string[];
  metadata?: DocumentMetadata;
}

/** 文档元数据 */
export interface DocumentMetadata {
  title?: string;
  author?: string;
  created?: string;
  modified?: string;
  wordCount?: number;
}

/** 版本信息 */
export interface VersionInfo {
  versionId: string;
  path: string;
  timestamp: string;
  operation: string;
  description: string;
  size: number;
  sessionId?: string;
}

/** Skill 信息 */
export interface SkillInfo {
  id: string;
  name: string;
  description: string;
  category: string;
  isBuiltin: boolean;
  isEnabled: boolean;
  version: string;
  paramsSchema?: unknown;
  supportedTypes: string[];
}

/** 自定义 Skill 配置 */
export interface CustomSkillConfig {
  name: string;
  description: string;
  category: string;
  promptTemplate: string;
  supportedTypes: string[];
  paramsSchema?: unknown;
}

/** 应用设置 */
export interface AppSettings {
  general: GeneralSettings;
  tokenBudget: TokenBudget;
  versionSnapshot: VersionSnapshot;
  workspace: WorkspaceDefaults;
  shortcuts: Shortcuts;
}

/** 通用设置 */
export interface GeneralSettings {
  authorName: string;
  confirmationLevel: string;
  language: string;
}

/** Token 预算设置 */
export interface TokenBudget {
  dailyLimit: number;
  monthlyLimit: number;
  exceedAction: string;
}

/** 版本快照设置 */
export interface VersionSnapshot {
  retentionPolicy: string;
  maxCount: number;
  maxDays: number;
}

/** 工作区默认设置 */
export interface WorkspaceDefaults {
  defaultWorkspaceId: string;
}

/** 快捷键设置 */
export interface Shortcuts {
  newSession: string;
  closeSession: string;
  sendMessage: string;
  toggleSidebar: string;
  quickPrompt: string;
}

/** 命令错误 */
export interface CommandError {
  code: number;
  message: string;
}

// ================================================================
// LLM 命令
// ================================================================

/** 测试 LLM Provider 连接 */
export async function testConnection(providerId: string): Promise<ConnectionResult> {
  try {
    return await invoke<ConnectionResult>("test_connection", { providerId });
  } catch (error) {
    console.error("[tauri] testConnection 失败:", error);
    throw error;
  }
}

/** 列出所有 LLM Provider */
export async function listProviders(): Promise<ProviderInfo[]> {
  try {
    return await invoke<ProviderInfo[]>("list_providers");
  } catch (error) {
    console.error("[tauri] listProviders 失败:", error);
    throw error;
  }
}

/** 添加 LLM Provider */
export async function addProvider(config: ProviderConfig): Promise<void> {
  try {
    await invoke("add_provider", { config });
  } catch (error) {
    console.error("[tauri] addProvider 失败:", error);
    throw error;
  }
}

/** 更新 LLM Provider */
export async function updateProvider(providerId: string, config: ProviderConfig): Promise<void> {
  try {
    await invoke("update_provider", { providerId, config });
  } catch (error) {
    console.error("[tauri] updateProvider 失败:", error);
    throw error;
  }
}

/** 删除 LLM Provider */
export async function deleteProvider(providerId: string): Promise<void> {
  try {
    await invoke("delete_provider", { providerId });
  } catch (error) {
    console.error("[tauri] deleteProvider 失败:", error);
    throw error;
  }
}

/** 设置默认 LLM Provider */
export async function setDefaultProvider(providerId: string): Promise<void> {
  try {
    await invoke("set_default_provider", { providerId });
  } catch (error) {
    console.error("[tauri] setDefaultProvider 失败:", error);
    throw error;
  }
}

// ================================================================
// 会话命令
// ================================================================

/** 创建新会话 */
export async function createSession(params: CreateSessionParams): Promise<Session> {
  try {
    return await invoke<Session>("create_session", { params });
  } catch (error) {
    console.error("[tauri] createSession 失败:", error);
    throw error;
  }
}

/** 列出会话 */
export async function listSessions(filter?: SessionFilter): Promise<SessionSummary[]> {
  try {
    return await invoke<SessionSummary[]>("list_sessions", { filter: filter ?? null });
  } catch (error) {
    console.error("[tauri] listSessions 失败:", error);
    throw error;
  }
}

/** 获取会话详情 */
export async function getSession(sessionId: string): Promise<SessionDetail> {
  try {
    return await invoke<SessionDetail>("get_session", { sessionId });
  } catch (error) {
    console.error("[tauri] getSession 失败:", error);
    throw error;
  }
}

/** 删除会话 */
export async function deleteSession(sessionId: string): Promise<void> {
  try {
    await invoke("delete_session", { sessionId });
  } catch (error) {
    console.error("[tauri] deleteSession 失败:", error);
    throw error;
  }
}

/** 更新会话标题 */
export async function updateSessionTitle(sessionId: string, title: string): Promise<void> {
  try {
    await invoke("update_session_title", { sessionId, title });
  } catch (error) {
    console.error("[tauri] updateSessionTitle 失败:", error);
    throw error;
  }
}

// ================================================================
// 工作区命令
// ================================================================

/** 列出所有工作区 */
export async function listWorkspaces(): Promise<WorkspaceInfo[]> {
  try {
    return await invoke<WorkspaceInfo[]>("list_workspaces");
  } catch (error) {
    console.error("[tauri] listWorkspaces 失败:", error);
    throw error;
  }
}

/** 添加工作区 */
export async function addWorkspace(path: string, name?: string): Promise<WorkspaceInfo> {
  try {
    return await invoke<WorkspaceInfo>("add_workspace", { path, name: name ?? null });
  } catch (error) {
    console.error("[tauri] addWorkspace 失败:", error);
    throw error;
  }
}

/** 移除工作区 */
export async function removeWorkspace(workspaceId: string): Promise<void> {
  try {
    await invoke("remove_workspace", { workspaceId });
  } catch (error) {
    console.error("[tauri] removeWorkspace 失败:", error);
    throw error;
  }
}

/** 设置活动工作区 */
export async function setActiveWorkspace(workspaceId: string): Promise<void> {
  try {
    await invoke("set_active_workspace", { workspaceId });
  } catch (error) {
    console.error("[tauri] setActiveWorkspace 失败:", error);
    throw error;
  }
}

/** 获取文件树 */
export async function getFileTree(
  workspaceId: string,
  path?: string,
  depth?: number,
): Promise<FileNode[]> {
  try {
    return await invoke<FileNode[]>("get_file_tree", {
      workspaceId,
      path: path ?? null,
      depth: depth ?? null,
    });
  } catch (error) {
    console.error("[tauri] getFileTree 失败:", error);
    throw error;
  }
}

/** 搜索文件 */
export async function searchFiles(
  workspaceId: string,
  query: string,
  options?: SearchOptions,
): Promise<SearchResult[]> {
  try {
    return await invoke<SearchResult[]>("search_files", {
      workspaceId,
      query,
      options: options ?? null,
    });
  } catch (error) {
    console.error("[tauri] searchFiles 失败:", error);
    throw error;
  }
}

// ================================================================
// 文档命令
// ================================================================

/** 预览文档 */
export async function previewDocument(
  workspaceId: string,
  path: string,
): Promise<PreviewContent> {
  try {
    return await invoke<PreviewContent>("preview_document", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] previewDocument 失败:", error);
    throw error;
  }
}

/** 获取文档版本历史 */
export async function getDocumentVersions(
  workspaceId: string,
  path: string,
): Promise<VersionInfo[]> {
  try {
    return await invoke<VersionInfo[]>("get_document_versions", { workspaceId, path });
  } catch (error) {
    console.error("[tauri] getDocumentVersions 失败:", error);
    throw error;
  }
}

/** 回滚到指定版本 */
export async function rollbackVersion(
  workspaceId: string,
  path: string,
  versionId: string,
): Promise<void> {
  try {
    await invoke("rollback_version", { workspaceId, path, versionId });
  } catch (error) {
    console.error("[tauri] rollbackVersion 失败:", error);
    throw error;
  }
}

// ================================================================
// Skill 命令
// ================================================================

/** 列出所有 Skill */
export async function listSkills(): Promise<SkillInfo[]> {
  try {
    return await invoke<SkillInfo[]>("list_skills");
  } catch (error) {
    console.error("[tauri] listSkills 失败:", error);
    throw error;
  }
}

/** 切换 Skill 启用/禁用 */
export async function toggleSkill(skillId: string, enabled: boolean): Promise<void> {
  try {
    await invoke("toggle_skill", { skillId, enabled });
  } catch (error) {
    console.error("[tauri] toggleSkill 失败:", error);
    throw error;
  }
}

/** 添加自定义 Skill */
export async function addCustomSkill(config: CustomSkillConfig): Promise<void> {
  try {
    await invoke("add_custom_skill", { config });
  } catch (error) {
    console.error("[tauri] addCustomSkill 失败:", error);
    throw error;
  }
}

/** 删除自定义 Skill */
export async function deleteCustomSkill(skillId: string): Promise<void> {
  try {
    await invoke("delete_custom_skill", { skillId });
  } catch (error) {
    console.error("[tauri] deleteCustomSkill 失败:", error);
    throw error;
  }
}

// ================================================================
// 设置命令
// ================================================================

/** 获取应用设置 */
export async function getSettings(): Promise<AppSettings> {
  try {
    return await invoke<AppSettings>("get_settings");
  } catch (error) {
    console.error("[tauri] getSettings 失败:", error);
    throw error;
  }
}

/** 更新应用设置 */
export async function updateSettings(settings: Record<string, unknown>): Promise<void> {
  try {
    await invoke("update_settings", { settings });
  } catch (error) {
    console.error("[tauri] updateSettings 失败:", error);
    throw error;
  }
}

// ================================================================
// Agent 命令
// ================================================================

/** 启动 Agent */
export async function startAgent(
  sessionId: string,
  prompt: string,
  options?: Record<string, unknown>,
): Promise<void> {
  try {
    await invoke("start_agent", { sessionId, prompt, options: options ?? null });
  } catch (error) {
    console.error("[tauri] startAgent 失败:", error);
    throw error;
  }
}

/** 停止 Agent */
export async function stopAgent(sessionId: string): Promise<void> {
  try {
    await invoke("stop_agent", { sessionId });
  } catch (error) {
    console.error("[tauri] stopAgent 失败:", error);
    throw error;
  }
}

/** 确认 Agent 操作 */
export async function confirmOperation(
  sessionId: string,
  operationId: string,
  approved: boolean,
  feedback?: string,
): Promise<void> {
  try {
    await invoke("confirm_operation", {
      sessionId,
      operationId,
      approved,
      feedback: feedback ?? null,
    });
  } catch (error) {
    console.error("[tauri] confirmOperation 失败:", error);
    throw error;
  }
}
