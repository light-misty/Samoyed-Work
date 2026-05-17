// ===== 会话类型定义 - 与 Rust 后端对齐 =====

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

export interface SessionSummary {
  id: string;
  title: string;
  status: string;
  messageCount: number;
  lastMessagePreview?: string;
  createdAt: string;
  updatedAt: string;
}

export interface SessionDetail {
  session: Session;
  messages: Message[];
  tokenUsage: TokenUsage;
}

export interface Message {
  id: string;
  role: string;
  content: string;
  toolCalls?: ToolCall[];
  createdAt: string;
}

export interface ToolCall {
  id: string;
  name: string;
  arguments: Record<string, unknown>;
  result?: unknown;
}

export interface TokenUsage {
  promptTokens: number;
  completionTokens: number;
  totalTokens: number;
}

export interface CreateSessionParams {
  title?: string;
  workspaceId?: string;
  providerId?: string;
  templateId?: string;
}

export interface SessionFilter {
  workspaceId?: string;
  status?: string;
  search?: string;
  limit?: number;
  offset?: number;
}
