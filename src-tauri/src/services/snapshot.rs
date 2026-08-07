//! 版本快照服务
//!
//! 为「回退消息」功能提供代码快照的创建与恢复，支持两种引擎：
//! - git 引擎：`git stash create` 捕获已跟踪修改（只读，不污染 index/分支），
//!   未跟踪文件复制备份到 `{backup_base}/git_{sha}/`，保证新建文件也能回退
//! - files 引擎：工作区文件全量复制备份，用于非 git 仓库
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use crate::errors::CommandError;
use crate::models::message::{Message, ToolCall};
use crate::utils::git_utils::create_git_command;

/// 快照类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapshotKind {
    /// git 仓库：stash create 快照
    Git,
    /// 非 git 仓库：文件复制备份
    Files,
}

impl SnapshotKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SnapshotKind::Git => "git",
            SnapshotKind::Files => "files",
        }
    }
}

/// 字符串快照类型转枚举（未知类型按 files 处理）
pub fn kind_from_str(kind: &str) -> SnapshotKind {
    if kind == "git" {
        SnapshotKind::Git
    } else {
        SnapshotKind::Files
    }
}

/// 备份时需要跳过的目录名。
/// files 引擎任意层级命中即跳过；git 引擎仅跳过未跟踪文件路径的第一段
const SKIP_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "out",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".idea",
    ".vscode",
];

/// 统一路径分隔符为 `/`，并去除首尾空白
fn normalize_path(p: &str) -> String {
    p.trim().replace('\\', "/")
}

/// 将相对路径安全地拼接到工作区根，拒绝绝对路径与 `..` 逃逸
fn safe_join(workspace: &Path, rel: &str) -> Option<PathBuf> {
    let rel = normalize_path(rel);
    let rel_path = Path::new(&rel);
    if rel_path.is_absolute() {
        return None;
    }
    let mut result = workspace.to_path_buf();
    for comp in rel_path.components() {
        match comp {
            Component::Normal(c) => result.push(c),
            _ => return None, // 拒绝 RootDir / Prefix / CurDir / ParentDir
        }
    }
    Some(result)
}

/// 运行 git 命令（工作区内），返回 Output
fn run_git(workspace: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    create_git_command()
        .args(args)
        .current_dir(workspace)
        .output()
}

/// 判断工作区是否为 git 仓库
fn is_git_repo(workspace: &Path) -> bool {
    run_git(workspace, &["rev-parse", "--is-inside-work-tree"])
        .map(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
        .unwrap_or(false)
}

/// 创建快照，优先使用 git 引擎，失败时降级为文件备份
///
/// 返回 (快照类型, 快照引用)：
/// - git：引用为 commit SHA，未跟踪文件备份在 `{backup_base}/git_{sha}/`
/// - files：引用为备份目录绝对路径
pub fn create_snapshot(
    workspace_path: &str,
    backup_base_dir: &Path,
) -> Result<(SnapshotKind, String), CommandError> {
    let workspace = Path::new(workspace_path);
    if is_git_repo(workspace) {
        match create_git_snapshot(workspace, backup_base_dir) {
            Ok(snapshot_ref) => return Ok((SnapshotKind::Git, snapshot_ref)),
            Err(_) => {
                // 降级：git 快照失败时使用文件备份
                log::warn!(
                    "[snapshot] git 快照创建失败，降级为文件备份: {:?}",
                    workspace_path
                );
            }
        }
    }
    let dir = create_files_snapshot(workspace, backup_base_dir)?;
    Ok((SnapshotKind::Files, dir))
}

/// 创建 git 快照：
/// 1. `git stash create` 捕获已跟踪修改（空输出表示无修改，回退到 HEAD）
/// 2. 未跟踪文件复制备份到 `{backup_base}/git_{sha}_{uuid}/`
///
/// 返回快照引用字符串：`{sha}:{备份目录名}`（备份目录按快照唯一，避免同 SHA 的
/// 多个快照共享目录导致未跟踪文件备份互相污染）
fn create_git_snapshot(workspace: &Path, backup_base_dir: &Path) -> Result<String, CommandError> {
    // 1. 收集未跟踪文件（?? 行）
    let untracked = list_untracked_files(workspace)?;
    // 2. 创建 stash 快照
    let output = run_git(workspace, &["stash", "create"]).map_err(|e| {
        CommandError::fs(
            crate::errors::FS_IO_ERROR,
            format!("git stash create 失败: {e}"),
        )
    })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CommandError::fs(
            crate::errors::FS_IO_ERROR,
            format!("git stash create 失败: {stderr}"),
        ));
    }
    let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
    // 无修改时 stash create 输出为空，回退到 HEAD
    let sha = if sha.is_empty() {
        let head = run_git(workspace, &["rev-parse", "HEAD"]).map_err(|e| {
            CommandError::fs(
                crate::errors::FS_IO_ERROR,
                format!("git rev-parse HEAD 失败: {e}"),
            )
        })?;
        if !head.status.success() {
            let stderr = String::from_utf8_lossy(&head.stderr).to_string();
            return Err(CommandError::fs(
                crate::errors::FS_IO_ERROR,
                format!("git rev-parse HEAD 失败: {stderr}"),
            ));
        }
        String::from_utf8_lossy(&head.stdout).trim().to_string()
    } else {
        sha
    };
    // 3. 备份未跟踪文件（快照中不包含它们，需单独复制）。
    //    备份目录名按快照唯一：git_{sha}_{uuid}，同一工作区的多个快照互不污染
    let dir_name = format!("git_{sha}_{}", uuid::Uuid::new_v4());
    if !untracked.is_empty() {
        let backup_dir = backup_base_dir.join(&dir_name);
        copy_paths(workspace, &backup_dir, &untracked)?;
    }
    Ok(format!("{sha}:{dir_name}"))
}

/// 列出工作区中所有未跟踪文件（非黑名单）
fn list_untracked_files(workspace: &Path) -> Result<Vec<String>, CommandError> {
    let output = run_git(
        workspace,
        &["status", "--porcelain", "--untracked-files=all"],
    )
    .map_err(|e| CommandError::fs(crate::errors::FS_IO_ERROR, format!("git status 失败: {e}")))?;
    if !output.status.success() {
        return Err(CommandError::fs(
            crate::errors::FS_IO_ERROR,
            "git status 失败".to_string(),
        ));
    }
    let mut files = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        // ?? 前缀 + 空格，其后为相对路径（含空格/中文等特殊文件名时带引号与八进制转义）
        let Some(rest) = line.strip_prefix("?? ") else {
            continue;
        };
        let rel = normalize_path(&decode_git_path(rest.trim()));
        // 跳过黑名单目录（含 / 分隔）
        let first_seg = rel.split('/').next().unwrap_or("");
        if SKIP_DIRS.contains(&first_seg) {
            continue;
        }
        files.push(rel);
    }
    Ok(files)
}

/// 解码 git status --porcelain 输出的路径：
/// core.quotePath 默认开启，特殊字符文件名输出为 `"..."`，其中非 ASCII 字符按
/// 字节以 `\ooo` 八进制转义（如 `"\346\265\213\350\257\225.txt"` = "测试.txt"）
fn decode_git_path(raw: &str) -> String {
    if raw.len() < 2 || !raw.starts_with('"') || !raw.ends_with('"') {
        return raw.to_string();
    }
    let bytes = &raw.as_bytes()[1..raw.len() - 1];
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            let next = bytes[i + 1];
            if (b'0'..=b'7').contains(&next) {
                // \ooo 八进制转义（最多 3 位）
                let mut val = 0u8;
                let mut j = 0;
                while j < 3 && i + 1 + j < bytes.len() {
                    let c = bytes[i + 1 + j];
                    if !(b'0'..=b'7').contains(&c) {
                        break;
                    }
                    val = val * 8 + (c - b'0');
                    j += 1;
                }
                out.push(val);
                i += 1 + j;
                continue;
            }
            let ch = match next {
                b'\\' => b'\\',
                b'"' => b'"',
                b't' => b'\t',
                b'n' => b'\n',
                _ => b'\\',
            };
            out.push(ch);
            i += 2;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| raw.to_string())
}

/// 创建文件备份快照：全量复制工作区（跳过黑名单目录）
fn create_files_snapshot(workspace: &Path, backup_base_dir: &Path) -> Result<String, CommandError> {
    let id = uuid::Uuid::new_v4().to_string();
    let backup_dir = backup_base_dir.join(&id);
    std::fs::create_dir_all(&backup_dir).map_err(|e| {
        CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建备份目录失败: {e}"))
    })?;
    copy_tree(workspace, workspace, &backup_dir)?;
    Ok(backup_dir.to_string_lossy().to_string())
}

/// 递归复制目录树，跳过黑名单目录
fn copy_tree(workspace: &Path, src_dir: &Path, dest_dir: &Path) -> Result<(), CommandError> {
    for entry in std::fs::read_dir(src_dir)
        .map_err(|e| CommandError::fs(crate::errors::FS_IO_ERROR, format!("读取目录失败: {e}")))?
    {
        let entry = entry.map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("读取目录项失败: {e}"))
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if SKIP_DIRS.contains(&name.as_str()) {
            continue;
        }
        let src_path = entry.path();
        let rel = src_path.strip_prefix(workspace).map_err(|_| {
            CommandError::fs(crate::errors::FS_IO_ERROR, "路径前缀计算失败".to_string())
        })?;
        let dest_path = dest_dir.join(rel);
        let file_type = entry.file_type().map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("读取文件类型失败: {e}"))
        })?;
        if file_type.is_dir() {
            std::fs::create_dir_all(&dest_path).map_err(|e| {
                CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建目录失败: {e}"))
            })?;
            copy_tree(workspace, &src_path, dest_dir)?;
        } else if file_type.is_file() {
            std::fs::copy(&src_path, &dest_path).map_err(|e| {
                CommandError::fs(crate::errors::FS_IO_ERROR, format!("复制文件失败: {e}"))
            })?;
        }
        // 符号链接等其他类型跳过
    }
    Ok(())
}

/// 按相对路径列表复制文件到目标目录
fn copy_paths(workspace: &Path, dest_dir: &Path, rel_paths: &[String]) -> Result<(), CommandError> {
    for rel in rel_paths {
        let src = safe_join(workspace, rel).ok_or_else(|| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("非法备份路径: {rel}"))
        })?;
        if !src.is_file() {
            continue;
        }
        let dest = safe_join(dest_dir, rel).ok_or_else(|| {
            CommandError::fs(
                crate::errors::FS_IO_ERROR,
                format!("非法备份目标路径: {rel}"),
            )
        })?;
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建目录失败: {e}"))
            })?;
        }
        std::fs::copy(&src, &dest).map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("复制文件失败: {e}"))
        })?;
    }
    Ok(())
}

/// 恢复快照：仅恢复 paths 指定的文件
///
/// 对每个路径依次尝试：快照中存在（git show / 备份文件）-> 写回；否则删除工作区文件
/// 返回成功处理的文件数
pub fn restore_snapshot(
    kind: SnapshotKind,
    snapshot_ref: &str,
    workspace_path: &str,
    backup_base_dir: &Path,
    paths: &[String],
) -> Result<usize, CommandError> {
    let workspace = Path::new(workspace_path);
    match kind {
        SnapshotKind::Git => restore_git_snapshot(workspace, snapshot_ref, backup_base_dir, paths),
        SnapshotKind::Files => restore_files_snapshot(workspace, Path::new(snapshot_ref), paths),
    }
}

/// 恢复 git 快照
///
/// 两阶段处理：先恢复/删除文件路径，再删除快照后新建的目录（mkdir/remove_dir 调用），
/// 避免目录删除与文件恢复互相干扰；非法路径（逃逸/绝对路径）跳过而非中止整个回退
fn restore_git_snapshot(
    workspace: &Path,
    snapshot_ref: &str,
    backup_base_dir: &Path,
    paths: &[String],
) -> Result<usize, CommandError> {
    let (sha, dir_name) = parse_git_ref(snapshot_ref);
    let backup_dir = backup_base_dir.join(dir_name);
    let mut restored = 0usize;
    // 阶段1：文件路径
    for rel in paths {
        let rel = normalize_path(rel);
        let Some(ws_file) = safe_join(workspace, &rel) else {
            log::warn!("restore: 跳过非法恢复路径: {rel}");
            continue;
        };
        if !ws_file.is_file() {
            continue;
        }
        // 1. 快照中已跟踪该文件 -> git show 写回
        if file_exists_in_git(workspace, &sha, &rel) {
            let content = git_show_file(workspace, &sha, &rel)?;
            if let Some(parent) = ws_file.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建目录失败: {e}"))
                })?;
            }
            std::fs::write(&ws_file, content).map_err(|e| {
                CommandError::fs(crate::errors::FS_IO_ERROR, format!("写回文件失败: {e}"))
            })?;
            restored += 1;
            continue;
        }
        // 2. 未跟踪文件备份中存在 -> 复制回
        let backup_file = safe_join(&backup_dir, &rel);
        if let Some(backup_file) = backup_file {
            if backup_file.is_file() {
                if let Some(parent) = ws_file.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建目录失败: {e}"))
                    })?;
                }
                std::fs::copy(&backup_file, &ws_file).map_err(|e| {
                    CommandError::fs(crate::errors::FS_IO_ERROR, format!("复制文件失败: {e}"))
                })?;
                restored += 1;
                continue;
            }
        }
        // 3. 快照与备份均不存在 -> 该文件为快照后新建，删除
        std::fs::remove_file(&ws_file).map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("删除文件失败: {e}"))
        })?;
        restored += 1;
    }
    // 阶段2：目录路径（快照后新建的目录删除，快照中已有的目录保留）
    for rel in paths {
        let rel = normalize_path(rel);
        let Some(ws_file) = safe_join(workspace, &rel) else {
            continue;
        };
        if !ws_file.is_dir() {
            continue;
        }
        if dir_exists_in_git(workspace, &sha, &rel) {
            continue;
        }
        if let Some(backup_file) = safe_join(&backup_dir, &rel) {
            if backup_file.exists() {
                continue;
            }
        }
        std::fs::remove_dir_all(&ws_file).map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("删除目录失败: {e}"))
        })?;
        restored += 1;
    }
    Ok(restored)
}

/// 判断目录路径在 git 快照中是否存在（文件路径用 cat-file，目录用 ls-tree）
fn dir_exists_in_git(workspace: &Path, sha: &str, rel: &str) -> bool {
    let rel = rel.trim_end_matches('/');
    run_git(
        workspace,
        &["ls-tree", "-d", "--name-only", sha, &format!("{rel}/")],
    )
    .map(|out| out.status.success() && !String::from_utf8_lossy(&out.stdout).trim().is_empty())
    .unwrap_or(false)
}

/// 判断路径在 git 快照中是否存在
fn file_exists_in_git(workspace: &Path, sha: &str, rel: &str) -> bool {
    let spec = format!("{sha}:{rel}");
    run_git(workspace, &["cat-file", "-e", &spec])
        .map(|out| out.status.success())
        .unwrap_or(false)
}

/// 读取 git 快照中的文件内容（原始字节）
fn git_show_file(workspace: &Path, sha: &str, rel: &str) -> Result<Vec<u8>, CommandError> {
    let spec = format!("{sha}:{rel}");
    let output = run_git(workspace, &["show", &spec])
        .map_err(|e| CommandError::fs(crate::errors::FS_IO_ERROR, format!("git show 失败: {e}")))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        return Err(CommandError::fs(
            crate::errors::FS_IO_ERROR,
            format!("git show 失败: {stderr}"),
        ));
    }
    Ok(output.stdout)
}

/// 恢复文件备份快照（两阶段：先文件路径，再目录路径；非法路径跳过）
fn restore_files_snapshot(
    workspace: &Path,
    backup_dir: &Path,
    paths: &[String],
) -> Result<usize, CommandError> {
    let mut restored = 0usize;
    // 阶段1：文件路径
    for rel in paths {
        let rel = normalize_path(rel);
        let Some(ws_file) = safe_join(workspace, &rel) else {
            log::warn!("restore: 跳过非法恢复路径: {rel}");
            continue;
        };
        if !ws_file.is_file() {
            continue;
        }
        let backup_file = safe_join(backup_dir, &rel);
        if let Some(backup_file) = backup_file {
            if backup_file.is_file() {
                if let Some(parent) = ws_file.parent() {
                    std::fs::create_dir_all(parent).map_err(|e| {
                        CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建目录失败: {e}"))
                    })?;
                }
                std::fs::copy(&backup_file, &ws_file).map_err(|e| {
                    CommandError::fs(crate::errors::FS_IO_ERROR, format!("复制文件失败: {e}"))
                })?;
                restored += 1;
                continue;
            }
        }
        // 备份中不存在 -> 快照后新建的文件，删除
        std::fs::remove_file(&ws_file).map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("删除文件失败: {e}"))
        })?;
        restored += 1;
    }
    // 阶段2：目录路径（备份中不存在的目录视为快照后新建，删除）
    for rel in paths {
        let rel = normalize_path(rel);
        let Some(ws_file) = safe_join(workspace, &rel) else {
            continue;
        };
        if !ws_file.is_dir() {
            continue;
        }
        let backup_exists = safe_join(backup_dir, &rel)
            .map(|p| p.exists())
            .unwrap_or(false);
        if backup_exists {
            continue;
        }
        std::fs::remove_dir_all(&ws_file).map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("删除目录失败: {e}"))
        })?;
        restored += 1;
    }
    Ok(restored)
}

/// 解析 git 快照引用：新格式 `{sha}:{备份目录名}`；旧格式（纯 SHA）时备份目录为 `git_{sha}`
fn parse_git_ref(snapshot_ref: &str) -> (String, String) {
    match snapshot_ref.split_once(':') {
        Some((sha, dir_name)) => (sha.to_string(), dir_name.to_string()),
        None => (snapshot_ref.to_string(), format!("git_{snapshot_ref}")),
    }
}

/// 删除快照的备份产物（git 未跟踪备份目录 / files 备份目录）
pub fn delete_backup(
    kind: SnapshotKind,
    snapshot_ref: &str,
    backup_base_dir: &Path,
) -> Result<(), CommandError> {
    let dir = match kind {
        SnapshotKind::Git => {
            let (_, dir_name) = parse_git_ref(snapshot_ref);
            backup_base_dir.join(dir_name)
        }
        SnapshotKind::Files => PathBuf::from(snapshot_ref),
    };
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|e| {
            CommandError::fs(crate::errors::FS_IO_ERROR, format!("删除备份目录失败: {e}"))
        })?;
    }
    Ok(())
}

/// 从消息中提取工具调用涉及的相对文件路径（去重，忽略无效路径）
pub fn collect_tool_paths(messages: &[Message]) -> Vec<String> {
    let mut paths: Vec<String> = Vec::new();
    for msg in messages {
        if let Some(calls) = &msg.tool_calls {
            for call in calls {
                collect_call_paths(call, &mut paths);
            }
        }
    }
    // 去重并保留顺序
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

/// 提取单次工具调用的路径参数
fn collect_call_paths(call: &ToolCall, out: &mut Vec<String>) {
    let args = &call.arguments;
    let mut push = |key: &str| {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            let norm = normalize_path(v);
            if !norm.is_empty() {
                out.push(norm);
            }
        }
    };
    match call.name.as_str() {
        "write" | "edit" | "remove" | "mkdir" | "remove_dir" => push("path"),
        "rename" | "copy" => {
            push("source_path");
            push("target_path");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::message::MessageRole;
    use serde_json::json;
    use tempfile::tempdir;

    /// 构造带工具调用的消息
    fn msg_with_tool_calls(calls: Vec<(&str, serde_json::Value)>) -> Message {
        Message {
            id: "msg".to_string(),
            role: MessageRole::Assistant,
            content: String::new(),
            tool_calls: Some(
                calls
                    .into_iter()
                    .map(|(name, arguments)| ToolCall {
                        id: "tc".to_string(),
                        name: name.to_string(),
                        arguments,
                        result: None,
                    })
                    .collect(),
            ),
            reasoning_content: None,
            attachments: None,
            metadata: None,
            branch_id: None,
            branch_group_id: None,
            created_at: "2026-08-07T00:00:00Z".to_string(),
        }
    }

    /// git 是否可用
    fn git_available() -> bool {
        create_git_command()
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// 解码 git status --porcelain 引号/八进制转义路径
    #[test]
    fn test_decode_git_path() {
        // 无引号：原样返回
        assert_eq!(decode_git_path("hello.txt"), "hello.txt");
        // 含空格文件名
        assert_eq!(decode_git_path(r#""my notes.md""#), "my notes.md");
        // 中文文件名（UTF-8 字节按八进制转义）
        assert_eq!(
            decode_git_path(r#""\346\265\213\350\257\225.txt""#),
            "测试.txt"
        );
        // 转义的引号与反斜杠
        assert_eq!(decode_git_path(r#""a\"b\\c.txt""#), "a\"b\\c.txt");
    }

    #[test]
    fn test_collect_tool_paths_extracts_and_dedups() {
        let msgs = vec![
            msg_with_tool_calls(vec![
                ("write", json!({"path": "src/a.ts"})),
                ("edit", json!({"path": "src/b.ts"})),
                ("bash", json!({"command": "rm src/c.ts"})),
            ]),
            msg_with_tool_calls(vec![
                (
                    "rename",
                    json!({"source_path": "old.ts", "target_path": "new.ts"}),
                ),
                (
                    "copy",
                    json!({"source_path": "a.txt", "target_path": "b.txt"}),
                ),
                // 与前面重复，应去重
                ("write", json!({"path": "src/a.ts"})),
            ]),
        ];
        let paths = collect_tool_paths(&msgs);
        assert_eq!(
            paths,
            vec!["src/a.ts", "src/b.ts", "old.ts", "new.ts", "a.txt", "b.txt"]
        );
        // bash 不提取
        assert!(!paths.contains(&"src/c.ts".to_string()));
    }

    #[test]
    fn test_collect_tool_paths_rejects_escape() {
        let msgs = vec![msg_with_tool_calls(vec![
            ("write", json!({"path": "../../evil.txt"})),
            ("write", json!({"path": "C:/evil.txt"})),
            ("write", json!({"path": "ok.txt"})),
        ])];
        // collect_tool_paths 只做提取，逃逸校验在恢复时进行
        let paths = collect_tool_paths(&msgs);
        assert_eq!(paths.len(), 3);
    }

    #[test]
    fn test_safe_join_rejects_escape() {
        let ws = Path::new("C:/ws");
        assert!(safe_join(ws, "a/b.txt").is_some());
        assert!(safe_join(ws, "../x.txt").is_none());
        assert!(safe_join(ws, "C:/evil.txt").is_none());
        assert!(safe_join(ws, "/abs/x.txt").is_none());
    }

    #[test]
    fn test_create_and_restore_files_snapshot() {
        let tmp = tempdir().unwrap();
        let ws = tmp.path().join("workspace");
        std::fs::create_dir_all(ws.join("sub")).unwrap();
        std::fs::write(ws.join("a.txt"), "v1").unwrap();
        std::fs::write(ws.join("sub/b.txt"), "v1").unwrap();
        // 黑名单目录不备份
        std::fs::create_dir_all(ws.join("node_modules/pkg")).unwrap();
        std::fs::write(ws.join("node_modules/pkg/index.js"), "x").unwrap();

        let backup_base = tmp.path().join("backups");
        // tempdir 不是 git 仓库，应走 files 引擎
        let (kind, snapshot_ref) = create_snapshot(&ws.to_string_lossy(), &backup_base).unwrap();
        assert_eq!(kind, SnapshotKind::Files);
        assert!(snapshot_ref.contains(&backup_base.to_string_lossy().to_string()));

        // 修改文件并新建文件
        std::fs::write(ws.join("a.txt"), "v2").unwrap();
        std::fs::write(ws.join("c.txt"), "new").unwrap();
        std::fs::write(ws.join("sub/b.txt"), "v2").unwrap();

        let restored = restore_snapshot(
            kind,
            &snapshot_ref,
            &ws.to_string_lossy(),
            &backup_base,
            &[
                "a.txt".to_string(),
                "c.txt".to_string(),
                "sub/b.txt".to_string(),
            ],
        )
        .unwrap();
        assert_eq!(restored, 3);
        // a.txt 恢复
        assert_eq!(std::fs::read_to_string(ws.join("a.txt")).unwrap(), "v1");
        // sub/b.txt 恢复
        assert_eq!(std::fs::read_to_string(ws.join("sub/b.txt")).unwrap(), "v1");
        // 新建文件 c.txt 被删除
        assert!(!ws.join("c.txt").exists());
        // 黑名单目录未被备份也不受影响
        assert!(ws.join("node_modules/pkg/index.js").exists());

        // 删除备份
        delete_backup(kind, &snapshot_ref, &backup_base).unwrap();
        assert!(!Path::new(&snapshot_ref).exists());
    }

    #[test]
    fn test_restore_files_removes_missing_path() {
        let tmp = tempdir().unwrap();
        let ws = tmp.path().join("workspace");
        std::fs::create_dir_all(&ws).unwrap();
        std::fs::write(ws.join("a.txt"), "v1").unwrap();
        let backup_base = tmp.path().join("backups");
        let (kind, snapshot_ref) = create_snapshot(&ws.to_string_lossy(), &backup_base).unwrap();
        // 快照后新建的文件
        std::fs::write(ws.join("new.txt"), "new").unwrap();
        let restored = restore_snapshot(
            kind,
            &snapshot_ref,
            &ws.to_string_lossy(),
            &backup_base,
            &["new.txt".to_string()],
        )
        .unwrap();
        assert_eq!(restored, 1);
        assert!(!ws.join("new.txt").exists());
    }

    #[test]
    fn test_git_snapshot_roundtrip() {
        if !git_available() {
            eprintln!("跳过：git 不可用");
            return;
        }
        let tmp = tempdir().unwrap();
        let ws = tmp.path().join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        // 初始化 git 仓库并提交初始内容
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "test"],
        ] {
            let out = create_git_command()
                .args(&args)
                .current_dir(&ws)
                .output()
                .unwrap();
            assert!(out.status.success());
        }
        std::fs::write(ws.join("a.txt"), "v1").unwrap();
        let out = create_git_command()
            .args(["add", "a.txt"])
            .current_dir(&ws)
            .output()
            .unwrap();
        assert!(out.status.success());
        let out = create_git_command()
            .args(["commit", "-m", "init"])
            .current_dir(&ws)
            .output()
            .unwrap();
        assert!(out.status.success());

        let backup_base = tmp.path().join("backups");
        let (kind, snapshot_ref) = create_snapshot(&ws.to_string_lossy(), &backup_base).unwrap();
        assert_eq!(kind, SnapshotKind::Git);

        // 修改已跟踪文件 + 新建未跟踪文件
        std::fs::write(ws.join("a.txt"), "v2").unwrap();
        std::fs::write(ws.join("new.txt"), "new").unwrap();

        let restored = restore_snapshot(
            kind,
            &snapshot_ref,
            &ws.to_string_lossy(),
            &backup_base,
            &["a.txt".to_string(), "new.txt".to_string()],
        )
        .unwrap();
        assert_eq!(restored, 2);
        // 已跟踪文件恢复
        assert_eq!(std::fs::read_to_string(ws.join("a.txt")).unwrap(), "v1");
        // 新建未跟踪文件被删除（依赖备份目录存在与否：
        // 若快照创建时文件已存在则从备份恢复——此处 new.txt 是快照后新建，
        // 备份中不存在，应删除）
        assert!(!ws.join("new.txt").exists());
        // git stash create 不应污染 index / 分支
        let out = create_git_command()
            .args(["status", "--porcelain"])
            .current_dir(&ws)
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).trim().is_empty());

        // 删除备份（备份目录名按快照唯一，含 sha 与随机后缀）
        let (_, dir_name) = parse_git_ref(&snapshot_ref);
        delete_backup(kind, &snapshot_ref, &backup_base).unwrap();
        assert!(!backup_base.join(dir_name).exists());
    }

    #[test]
    fn test_git_snapshot_recovers_untracked_file() {
        if !git_available() {
            eprintln!("跳过：git 不可用");
            return;
        }
        let tmp = tempdir().unwrap();
        let ws = tmp.path().join("repo");
        std::fs::create_dir_all(&ws).unwrap();
        for args in [
            vec!["init"],
            vec!["config", "user.email", "test@test.com"],
            vec!["config", "user.name", "test"],
        ] {
            create_git_command()
                .args(&args)
                .current_dir(&ws)
                .output()
                .unwrap();
        }
        // 需要初始 commit，否则 stash create 失败会降级为 files 引擎
        std::fs::write(ws.join("base.txt"), "base").unwrap();
        let out = create_git_command()
            .args(["add", "base.txt"])
            .current_dir(&ws)
            .output()
            .unwrap();
        assert!(out.status.success());
        let out = create_git_command()
            .args(["commit", "-m", "init"])
            .current_dir(&ws)
            .output()
            .unwrap();
        assert!(out.status.success());
        // 快照创建前已存在的未跟踪文件
        std::fs::write(ws.join("draft.txt"), "draft-v1").unwrap();

        let backup_base = tmp.path().join("backups");
        let (kind, snapshot_ref) = create_snapshot(&ws.to_string_lossy(), &backup_base).unwrap();
        assert_eq!(kind, SnapshotKind::Git);

        // agent 修改了该未跟踪文件
        std::fs::write(ws.join("draft.txt"), "draft-v2").unwrap();

        let restored = restore_snapshot(
            kind,
            &snapshot_ref,
            &ws.to_string_lossy(),
            &backup_base,
            &["draft.txt".to_string()],
        )
        .unwrap();
        assert_eq!(restored, 1);
        // 从未跟踪备份目录恢复原始内容
        assert_eq!(
            std::fs::read_to_string(ws.join("draft.txt")).unwrap(),
            "draft-v1"
        );
    }
}
