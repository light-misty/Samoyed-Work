use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::db::session_repo;
use crate::errors::{CommandError, FS_NOT_A_DIRECTORY, FS_PATH_NOT_FOUND};
use crate::events::types;
use crate::events::AgentEmitter;
use crate::models::workspace::{
    FileNode, SearchOptions, SearchResult, WorkspaceGitStatus, WorkspaceInfo,
};
use crate::AppState;

/// 列出所有工作区
#[tauri::command]
pub async fn list_workspaces(
    state: State<'_, AppState>,
) -> Result<Vec<WorkspaceInfo>, CommandError> {
    log::info!("list_workspaces: 查询所有工作区");
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    // 从应用设置中读取默认工作区 ID，用于判断 is_active
    let default_workspace_id = config
        .load_app_settings()
        .map(|s| s.workspace.default_workspace_id)
        .unwrap_or_default();

    let result: Vec<WorkspaceInfo> = ws_config
        .workspaces
        .iter()
        .map(|w| {
            let path = PathBuf::from(&w.path);
            let path_exists = path.exists() && path.is_dir();
            let file_count = if path_exists {
                count_files_in_dir(&path).unwrap_or(0)
            } else {
                0
            };
            WorkspaceInfo {
                id: w.id.clone(),
                name: w.name.clone(),
                path: w.path.clone(),
                is_active: w.id == default_workspace_id,
                path_exists,
                file_count,
                created_at: w.created_at.clone(),
                last_accessed: w.created_at.clone(),
            }
        })
        .collect();

    log::info!("list_workspaces: 查询完成, 共 {} 个工作区", result.len());
    Ok(result)
}

/// 添加工作区
#[tauri::command]
pub async fn add_workspace(
    path: String,
    name: Option<String>,
    state: State<'_, AppState>,
) -> Result<WorkspaceInfo, CommandError> {
    log::info!("add_workspace: 添加工作区, path={}", path);
    let dir_path = PathBuf::from(&path);
    if !dir_path.exists() {
        log::error!("add_workspace: 路径不存在: {}", path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("路径不存在: {}", path),
        ));
    }
    if !dir_path.is_dir() {
        log::error!("add_workspace: 路径不是目录: {}", path);
        return Err(CommandError::fs(
            FS_NOT_A_DIRECTORY,
            format!("路径不是目录: {}", path),
        ));
    }

    let cfg_manager = state.config.lock().await;
    let mut ws_config = cfg_manager.load_workspaces()?;

    let display_name = name.unwrap_or_else(|| {
        dir_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "未命名工作区".to_string())
    });

    let entry = cfg_manager.add_workspace(&mut ws_config, &path, &display_name)?;
    cfg_manager.save_workspaces(&ws_config)?;

    let file_count = count_files_in_dir(&dir_path).unwrap_or(0);
    log::info!(
        "add_workspace: 工作区添加成功, name={}, id={}",
        display_name,
        entry.id
    );

    Ok(WorkspaceInfo {
        id: entry.id,
        name: entry.name,
        path: entry.path,
        is_active: false,
        path_exists: true, // 刚添加的工作区目录一定存在
        file_count,
        created_at: entry.created_at.clone(),
        last_accessed: entry.created_at,
    })
}

/// 移除工作区
/// 同时删除该工作区下的所有会话（包括消息），避免出现孤儿会话导致前端分组错乱
#[tauri::command]
pub async fn remove_workspace(
    workspace_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("remove_workspace: 移除工作区, id={}", workspace_id);
    let cfg_manager = state.config.lock().await;
    let mut ws_config = cfg_manager.load_workspaces()?;

    cfg_manager.remove_workspace(&mut ws_config, &workspace_id)?;
    cfg_manager.save_workspaces(&ws_config)?;
    log::info!("remove_workspace: 工作区移除成功, id={}", workspace_id);

    // 删除该工作区下的所有会话（包括消息），避免孤儿会话
    // 孤儿会话会被前端 SessionListSection 的兜底逻辑错误归入第一个工作区，造成"会话转移"
    let deleted_session_ids = {
        let conn = state.db.conn()?;
        session_repo::delete_sessions_by_workspace(&conn, &workspace_id)?
    };
    if !deleted_session_ids.is_empty() {
        log::info!(
            "remove_workspace: 已清理工作区 {} 下的 {} 条关联会话",
            workspace_id,
            deleted_session_ids.len()
        );
        // 通知前端这些会话已被删除，使其清理本地状态
        let emitter = AgentEmitter::new(app_handle.clone());
        for session_id in &deleted_session_ids {
            let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
                session_id: session_id.clone(),
                change_type: "deleted".to_string(),
                data: None,
            });
        }
    }

    // 如果被移除的工作区是当前活动工作区，清除默认工作区设置
    let mut settings = cfg_manager.load_app_settings()?;
    if settings.workspace.default_workspace_id == workspace_id {
        settings.workspace.default_workspace_id = String::new();
        cfg_manager.save_app_settings(&settings)?;
        log::info!("remove_workspace: 已清除默认工作区设置（被移除的工作区是当前活动工作区）");
    }

    Ok(())
}

/// 设置活动工作区
#[tauri::command]
pub async fn set_active_workspace(
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("set_active_workspace: 设置活动工作区, id={}", workspace_id);
    let cfg_manager = state.config.lock().await;
    let ws_config = cfg_manager.load_workspaces()?;

    let workspace = ws_config.workspaces.iter().find(|w| w.id == workspace_id);
    if workspace.is_none() {
        log::error!("set_active_workspace: 工作区 '{}' 不存在", workspace_id);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("工作区 '{}' 不存在", workspace_id),
        ));
    }

    let ws = workspace.unwrap();

    // 检查工作区目录是否存在于文件系统
    let ws_path = PathBuf::from(&ws.path);
    if !ws_path.exists() || !ws_path.is_dir() {
        log::error!("set_active_workspace: 工作区目录已被删除: {}", ws.path);
        return Err(CommandError::fs(
            FS_PATH_NOT_FOUND,
            format!("工作区目录已被删除: {}，请移除该工作区后重新选择", ws.path),
        ));
    }

    // 更新应用设置中的默认工作区
    let mut settings = cfg_manager.load_app_settings()?;
    settings.workspace.default_workspace_id = workspace_id.clone();
    cfg_manager.save_app_settings(&settings)?;

    // 启动文件监听（传入工作区名称以便 FsWatcher 在目录删除时使用）
    drop(cfg_manager);
    state
        .fs_watcher
        .watch_with_name(workspace_id, ws.path.clone(), ws.name.clone())
        .await;

    log::info!(
        "set_active_workspace: 活动工作区设置成功, id={}",
        settings.workspace.default_workspace_id
    );
    Ok(())
}

/// 获取文件树，实际遍历文件系统目录
#[tauri::command]
pub async fn get_file_tree(
    workspace_id: String,
    path: Option<String>,
    depth: Option<u32>,
    state: State<'_, AppState>,
) -> Result<Vec<FileNode>, CommandError> {
    log::info!(
        "get_file_tree: 获取文件树, workspace_id={}, path={:?}, depth={:?}",
        workspace_id,
        path,
        depth
    );
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("get_file_tree: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let root = PathBuf::from(&workspace.path);
    let base = match &path {
        Some(p) => root.join(p),
        None => root.clone(),
    };

    // 默认只返回第一层，深层目录由前端展开时按需懒加载（传 path + depth 获取指定目录子节点）
    let max_depth = depth.unwrap_or(1);
    let result = build_file_tree(&base, &root, max_depth, 0);
    log::info!("get_file_tree: 文件树构建完成, 节点数={}", result.len());
    Ok(result)
}

/// 搜索文件（基于内存文件名索引，文件变更时索引自动失效重建）
#[tauri::command]
pub async fn search_files(
    workspace_id: String,
    query: String,
    options: Option<SearchOptions>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, CommandError> {
    log::info!(
        "search_files: 搜索文件, workspace_id={}, query={}",
        workspace_id,
        query
    );
    let config = state.config.lock().await;
    let ws_config = config.load_workspaces()?;

    let workspace = ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .ok_or_else(|| {
            log::error!("search_files: 工作区 '{}' 不存在", workspace_id);
            CommandError::fs(
                FS_PATH_NOT_FOUND,
                format!("工作区 '{}' 不存在", workspace_id),
            )
        })?;

    let max_results = options.as_ref().and_then(|o| o.max_results).unwrap_or(50) as usize;

    let extensions: Vec<String> = options
        .as_ref()
        .and_then(|o| o.extensions.clone())
        .unwrap_or_default();

    let root = PathBuf::from(&workspace.path);

    // 从内存索引检索（未构建时自动扫描构建），避免每次搜索全盘遍历
    let index = state.file_index_cache.get_or_build(&workspace_id, &root);
    let matches = index.search(&query, max_results);

    // 仅对命中的文件读取元数据，避免全盘 stat
    let mut results = Vec::with_capacity(matches.len());
    for m in matches {
        if !extensions.is_empty() && !extensions.iter().any(|e| e.to_lowercase() == m.extension) {
            continue;
        }
        let (size, modified) = match std::fs::metadata(root.join(&m.path)) {
            Ok(meta) => (
                meta.len(),
                meta.modified()
                    .ok()
                    .map(|t| {
                        let dt: chrono::DateTime<chrono::Utc> = t.into();
                        dt.to_rfc3339()
                    })
                    .unwrap_or_default(),
            ),
            Err(_) => (0, String::new()),
        };
        results.push(SearchResult {
            path: m.path.clone(),
            name: m.name.clone(),
            extension: m.extension.clone(),
            size,
            modified,
            match_type: "name".to_string(),
            match_preview: None,
            line_number: None,
        });
        if results.len() >= max_results {
            break;
        }
    }

    log::info!("search_files: 搜索完成, 结果数={}", results.len());
    Ok(results)
}

#[tauri::command]
pub async fn get_workspace_git_status(
    workspace_path: String,
) -> Result<WorkspaceGitStatus, CommandError> {
    log::info!(
        "get_workspace_git_status: 检查 Git 状态, path={}",
        workspace_path
    );

    use crate::utils::git_utils::create_git_command;

    let is_git_repo = create_git_command()
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&workspace_path)
        .output()
        .is_ok_and(|o| o.status.success());

    if !is_git_repo {
        return Ok(WorkspaceGitStatus {
            is_git_repo: false,
            branch_name: None,
            changed_file_count: 0,
        });
    }

    let branch = create_git_command()
        .args(["branch", "--show-current"])
        .current_dir(&workspace_path)
        .output()
        .ok()
        .and_then(|o| {
            o.status
                .success()
                .then(|| String::from_utf8_lossy(&o.stdout).trim().to_string())
        })
        .unwrap_or_else(|| "HEAD".to_string());

    let changed_file_count = create_git_command()
        .args(["status", "--porcelain"])
        .current_dir(&workspace_path)
        .output()
        .ok()
        .map(|o| {
            std::str::from_utf8(&o.stdout)
                .unwrap_or("")
                .lines()
                .filter(|l| !l.is_empty())
                .count() as u32
        })
        .unwrap_or(0);

    log::info!(
        "get_workspace_git_status: Git 仓库检测完成, branch={}, changed={}",
        branch,
        changed_file_count
    );

    Ok(WorkspaceGitStatus {
        is_git_repo: true,
        branch_name: Some(branch),
        changed_file_count,
    })
}

/// 递归构建文件树
fn build_file_tree(
    dir: &PathBuf,
    root: &PathBuf,
    max_depth: u32,
    current_depth: u32,
) -> Vec<FileNode> {
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

        // 跳过隐藏文件和目录（保留 .agent 以便智能体感知工作区 Skill）
        if name.starts_with('.') && name != ".agent" {
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
        let size = if is_dir { None } else { Some(metadata.len()) };
        let modified = metadata.modified().ok().map(|t| {
            let dt: chrono::DateTime<chrono::Utc> = t.into();
            dt.to_rfc3339()
        });
        let extension = path.extension().map(|e| e.to_string_lossy().to_string());

        let children = if is_dir && current_depth < max_depth - 1 {
            Some(build_file_tree(&path, root, max_depth, current_depth + 1))
        } else {
            None
        };

        nodes.push(FileNode {
            name,
            path: relative,
            is_dir,
            size,
            modified,
            extension,
            children,
        });
    }

    nodes
}

/// 统计目录中的文件数量
fn count_files_in_dir(dir: &PathBuf) -> Result<u32, CommandError> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut count = 0u32;
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_file() {
                count += 1;
            }
        }
    }
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建临时目录结构：a/root.txt、a/b/mid.txt、a/b/c/deep.txt
    fn build_temp_tree() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "samoyed_file_tree_test_{}_{}",
            std::process::id(),
            std::thread::current()
                .name()
                .unwrap_or("main")
                .replace(':', "_")
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("a/b/c")).expect("创建临时目录失败");
        std::fs::write(dir.join("a/root.txt"), "root").expect("写入临时文件失败");
        std::fs::write(dir.join("a/b/mid.txt"), "mid").expect("写入临时文件失败");
        std::fs::write(dir.join("a/b/c/deep.txt"), "deep").expect("写入临时文件失败");
        dir
    }

    /// 默认 depth=1 时仅返回第一层，目录的 children 为 None（深层由前端懒加载）
    #[test]
    fn test_build_file_tree_depth_one_only_first_level() {
        let dir = build_temp_tree();
        let nodes = build_file_tree(&dir, &dir, 1, 0);
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "a");
        assert!(nodes[0].is_dir);
        assert!(nodes[0].children.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// depth=3 时第三层目录的 children 为 None（深度截断边界）
    #[test]
    fn test_build_file_tree_depth_three_cutoff_at_third_level() {
        let dir = build_temp_tree();
        let nodes = build_file_tree(&dir, &dir, 3, 0);
        assert_eq!(nodes.len(), 1);
        let a = &nodes[0];
        let a_children = a.children.as_ref().expect("第一层目录应有 children");
        let b = a_children
            .iter()
            .find(|n| n.name == "b")
            .expect("缺少目录 b");
        let b_children = b.children.as_ref().expect("第二层目录应有 children");
        let c = b_children
            .iter()
            .find(|n| n.name == "c")
            .expect("缺少目录 c");
        assert!(c.children.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
