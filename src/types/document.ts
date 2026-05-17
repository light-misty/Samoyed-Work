// ===== 文档类型定义 - 与 Rust 后端对齐 =====

export interface PreviewContent {
  path: string;
  fileType: string;
  content: string;
  pageCount?: number;
  sheetNames?: string[];
  metadata?: DocumentMetadata;
}

export interface DocumentMetadata {
  title?: string;
  author?: string;
  created?: string;
  modified?: string;
  wordCount?: number;
}

export interface VersionInfo {
  versionId: string;
  path: string;
  timestamp: string;
  operation: string;
  description: string;
  size: number;
  sessionId?: string;
}
