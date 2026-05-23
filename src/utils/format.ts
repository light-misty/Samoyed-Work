export function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toTimeString().slice(0, 8);
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function formatTokens(n: number): string {
  return n.toLocaleString();
}

export function generateToolBrief(toolName: string, input: Record<string, unknown>): string {
  const f = (key: string) => String(input[key] ?? "");
  switch (toolName) {
    case "generate_document":
      return `生成 ${f("file_name") || "文档"}`;
    case "read_document":
      return `读取 ${f("file_name") || "文档"}`;
    case "modify_document":
      return `修改 ${f("file_name") || f("path") || "文档"}`;
    case "delete_document":
      return `删除 ${f("file_name") || f("path") || "文件"}`;
    case "convert_format":
      return `转换 ${f("file_name") || f("source_path") || "文档"} 格式`;
    case "search_documents":
      return `搜索 ${f("query") ? `"${f("query")}"` : "文件"}`;
    case "analyze_document":
      return `分析 ${f("file_name") || "文档"}`;
    case "list_workspace":
      return "列出工作区目录";
    case "batch_process":
      return `批量处理 ${f("operation") || "文档"}`;
    default:
      return toolName;
  }
}
