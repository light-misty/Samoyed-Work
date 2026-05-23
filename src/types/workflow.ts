export type NodeStatus = "pending" | "running" | "completed" | "failed" | "cancelled";

export type ExecutionStatus = "idle" | "running" | "stopping" | "paused" | "completed" | "failed" | "cancelled";

export type WorkflowNodeType = "user" | "thinking" | "content" | "tool" | "confirm" | "error";

export interface Attachment {
  id: string;
  name: string;
  path: string;
  size: number;
  mimeType: string;
}

export interface UserNodeData {
  content: string;
  attachments: Attachment[];
}

export interface ThinkingNodeData {
  content: string;
  duration: number;
  isStreaming?: boolean;
}

export interface ContentNodeData {
  content: string;
  isStreaming?: boolean;
}

export interface ToolNodeData {
  toolName: string;
  briefDescription: string;
  input: Record<string, unknown>;
  success?: boolean;
  error?: string;
}

export interface ConfirmNodeData {
  title: string;
  description: string;
  confirmLabel: string;
  cancelLabel: string;
  confirmed: boolean | null;
}

export interface ErrorNodeData {
  code: number;
  message: string;
  recoverable: boolean;
  module: string;
}

export interface NodeDataMap {
  user: UserNodeData;
  thinking: ThinkingNodeData;
  content: ContentNodeData;
  tool: ToolNodeData;
  confirm: ConfirmNodeData;
  error: ErrorNodeData;
}

export interface WorkflowNode<T extends WorkflowNodeType = WorkflowNodeType> {
  id: string;
  type: T;
  status: NodeStatus;
  timestamp: number;
  data: NodeDataMap[T];
  isExpanded: boolean;
  error?: string;
}

export interface DiffStats {
  additions: number;
  deletions: number;
  files: number;
}
