// 快捷键组合解析结果
export interface ParsedShortcut {
  key: string;
  ctrlKey: boolean;
  shiftKey: boolean;
  altKey: boolean;
}

// 解析快捷键字符串（如 "Ctrl+Enter"、"Enter"、"Shift+Enter"）为结构化对象
export function parseShortcut(shortcut: string): ParsedShortcut {
  const parts = shortcut.split("+").map((p) => p.trim());
  return {
    ctrlKey: parts.includes("Ctrl"),
    shiftKey: parts.includes("Shift"),
    altKey: parts.includes("Alt"),
    key: parts[parts.length - 1] || "",
  };
}

// 判断键盘事件是否匹配快捷键组合
export function matchesShortcut(e: { key: string; ctrlKey: boolean; shiftKey: boolean; altKey: boolean }, shortcut: string): boolean {
  const parsed = parseShortcut(shortcut);
  return (
    e.key.toLowerCase() === parsed.key.toLowerCase() &&
    e.ctrlKey === parsed.ctrlKey &&
    e.shiftKey === parsed.shiftKey &&
    e.altKey === parsed.altKey
  );
}

// 根据 sendMessage 快捷键推导换行快捷键
// 如果发送是 Enter，则换行是 Shift+Enter；如果发送是 Ctrl+Enter，则换行是 Enter
export function deriveNewLineShortcut(sendShortcut: string): string {
  const parsed = parseShortcut(sendShortcut);
  if (parsed.key.toLowerCase() === "enter") {
    if (parsed.ctrlKey) {
      return "Enter";
    }
    if (parsed.shiftKey) {
      return "Enter";
    }
    return "Shift+Enter";
  }
  return "Enter";
}

export function formatTime(ts: number): string {
  const d = new Date(ts);
  return d.toTimeString().slice(0, 8);
}

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

/**
 * 将相对路径转换为绝对路径
 */
function resolveAbsolutePath(path: string, workspaceRoot: string): string {
  if (!path) return path;
  // Windows 绝对路径：C:\...
  if (/^[a-zA-Z]:[/\\]/.test(path)) return path;
  // Unix 绝对路径：/...
  if (path.startsWith('/')) return path;
  // 相对路径：拼接工作区根目录
  const sep = workspaceRoot.includes('\\') ? '\\' : '/';
  const normalized = path.replace(/[/\\]/g, sep);
  // "." 表示当前目录，直接返回工作区根目录
  if (normalized === '.') return workspaceRoot;
  return workspaceRoot.endsWith(sep)
    ? workspaceRoot + normalized
    : workspaceRoot + sep + normalized;
}

/**
 * 提取工具调用的文件路径（绝对路径），无路径时返回 undefined
 */
export function extractToolPath(
  toolName: string,
  input: Record<string, unknown>,
  workspaceRoot: string
): string | undefined {
  const f = (key: string) => String(input[key] ?? '');

  // rename：源路径（绝对）→ 目标文件名
  if (toolName === 'rename') {
    const source = f('source_path');
    const target = f('target_path');
    if (source) {
      const absSource = resolveAbsolutePath(source, workspaceRoot);
      if (target) {
        const targetName = target.replace(/^.*[/\\]/, '');
        return `${absSource} → ${targetName}`;
      }
      return absSource;
    }
    return undefined;
  }

  // copy：源路径（绝对）→ 目标路径（绝对）
  if (toolName === 'copy') {
    const source = f('source_path');
    const target = f('target_path');
    if (source) {
      const absSource = resolveAbsolutePath(source, workspaceRoot);
      const absTarget = target ? resolveAbsolutePath(target, workspaceRoot) : '';
      return target ? `${absSource} → ${absTarget}` : absSource;
    }
    return undefined;
  }

  // lsp：使用 file_path 参数
  if (toolName === 'lsp') {
    const p = f('file_path');
    return p ? resolveAbsolutePath(p, workspaceRoot) : undefined;
  }

  // 统一使用 path 参数的工具
  const pathTools = new Set([
    'list', 'read', 'write', 'edit', 'edit_lines', 'remove',
    'file_info', 'read_lines', 'remove_dir', 'mkdir', 'exists',
    'hash', 'source_code',
    'docx', 'xlsx', 'pptx', 'pdf',
  ]);
  if (pathTools.has(toolName)) {
    const p = f('path');
    return p ? resolveAbsolutePath(p, workspaceRoot) : undefined;
  }

  return undefined;
}
