use tauri::Manager;
use tauri::{AppHandle, State};

use crate::db::message_repo;
use crate::db::session_repo;
use crate::errors::CommandError;
use crate::events::types;
use crate::events::AgentEmitter;
use crate::models::session::{
    CreateSessionParams, Session, SessionDetail, SessionFilter, SessionSummary,
};
use crate::AppState;

/// 创建新会话
#[tauri::command]
pub async fn create_session(
    params: CreateSessionParams,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Session, CommandError> {
    log::info!(
        "create_session 请求: title={:?}, workspace_id={:?}, provider_id={:?}",
        params.title,
        params.workspace_id,
        params.provider_id
    );
    let id = uuid::Uuid::new_v4().to_string();
    let title = params.title.unwrap_or_else(|| "新会话".to_string());
    let workspace_id = params.workspace_id.unwrap_or_default();
    let provider_id = params.provider_id.unwrap_or_default();

    let conn = state.db.conn()?;
    session_repo::create_session(&conn, &id, &workspace_id, &title, &provider_id, "")?;

    let session = session_repo::get_session(&conn, &id)?;
    log::info!(
        "create_session 成功: session_id={}, title={}",
        session.id,
        session.title
    );

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session.id.clone(),
        change_type: "created".to_string(),
        data: Some(serde_json::to_value(&session).unwrap_or_default()),
    });

    Ok(session)
}

/// 列出会话，支持筛选
#[tauri::command]
pub async fn list_sessions(
    filter: Option<SessionFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, CommandError> {
    log::info!("list_sessions 请求: filter={:?}", filter);
    let conn = state.db.conn()?;

    let workspace_id = filter.as_ref().and_then(|f| f.workspace_id.as_deref());
    let status = filter.as_ref().and_then(|f| f.status.as_deref());
    let search = filter.as_ref().and_then(|f| f.search.as_deref());
    let limit = filter.as_ref().and_then(|f| f.limit).unwrap_or(50);
    let offset = filter.as_ref().and_then(|f| f.offset).unwrap_or(0);

    log::debug!(
        "list_sessions 查询条件: workspace_id={:?}, status={:?}, search={:?}, limit={}, offset={}",
        workspace_id,
        status,
        search,
        limit,
        offset
    );
    let result = session_repo::list_sessions(&conn, workspace_id, status, search, limit, offset);
    log::info!("list_sessions 成功: 返回 {} 条记录", result.len());
    Ok(result)
}

/// 获取会话详情，包含消息历史
#[tauri::command]
pub async fn get_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionDetail, CommandError> {
    log::info!("get_session 请求: session_id={}", session_id);
    let conn = state.db.conn()?;
    let session = session_repo::get_session(&conn, &session_id)?;

    // 获取当前活跃分支 ID（无记录时 branch_repo 兜底返回 main 分支 ID）
    let active_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;

    // 加载当前分支的消息
    let mut messages = message_repo::list_messages(&conn, &session_id, &active_branch_id);

    // 检查是否存在 staged revert（回退后未发送新消息）：
    // 若回退边界属于当前活跃分支，则只返回边界之前的消息，并返回 revert 信息供前端展示横幅
    let revert = resolve_revert_info(&conn, &session_id, &active_branch_id, &mut messages)?;

    log::info!(
        "get_session 成功: session_id={}, 消息数={}, revert={:?}",
        session_id,
        messages.len(),
        revert
    );

    // 加载会话的所有分支列表（供前端渲染切换器）
    let branches = crate::db::branch_repo::list_branches_by_session(&conn, &session_id)?;

    Ok(SessionDetail {
        session,
        messages,
        branches,
        active_branch_id,
        revert,
    })
}

/// 计算 staged revert 信息：存在回退且边界属于当前活跃分支时，
/// 截断 messages 到边界之前（不含边界），并返回 RevertInfo
fn resolve_revert_info(
    conn: &rusqlite::Connection,
    session_id: &str,
    active_branch_id: &str,
    messages: &mut Vec<crate::models::message::Message>,
) -> Result<Option<crate::models::snapshot::RevertInfo>, CommandError> {
    let revert = match crate::db::revert_repo::get_revert(conn, session_id)? {
        Some(r) => r,
        None => return Ok(None),
    };
    // 边界消息属于当前活跃分支时才生效（分支切换后旧分支的 revert 被忽略）
    let belongs = message_repo::get_message(conn, session_id, &revert.revert_message_id)?
        .map(|(_, branch, _)| branch == active_branch_id)
        .unwrap_or(false);
    if !belongs {
        return Ok(None);
    }
    // 在消息列表中定位边界消息，截断到边界之前
    let idx = match messages
        .iter()
        .position(|m| m.id == revert.revert_message_id)
    {
        Some(idx) => idx,
        None => return Ok(None),
    };
    let hidden_count = messages.len() - idx;
    messages.truncate(idx);

    // 边界消息是否有可用快照（决定代码是否已回退）
    let code_reverted =
        crate::db::snapshot_repo::get_snapshot_by_message_id(conn, &revert.revert_message_id)?
            .is_some();

    Ok(Some(crate::models::snapshot::RevertInfo {
        revert_message_id: revert.revert_message_id,
        hidden_count,
        code_reverted,
    }))
}

/// 删除会话
#[tauri::command]
pub async fn delete_session(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_session 请求: session_id={}", session_id);

    // 检查会话是否有 Agent 正在运行，防止数据丢失
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::warn!(
                "delete_session 失败: 会话 '{}' 的 Agent 正在运行",
                session_id
            );
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 的 Agent 正在运行，无法删除", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;
    session_repo::delete_session(&conn, &session_id)?;
    log::info!("delete_session 成功: session_id={}", session_id);

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session_id.clone(),
        change_type: "deleted".to_string(),
        data: None,
    });

    Ok(())
}

/// 更新会话标题
#[tauri::command]
pub async fn update_session_title(
    session_id: String,
    title: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "update_session_title 请求: session_id={}, title={}",
        session_id,
        title
    );
    let conn = state.db.conn()?;
    session_repo::update_session_title(&conn, &session_id, &title)?;
    log::info!(
        "update_session_title 成功: session_id={}, title={}",
        session_id,
        title
    );

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session_id.clone(),
        change_type: "updated".to_string(),
        data: Some(serde_json::json!({ "title": title })),
    });

    Ok(())
}

/// 清除指定工作区下的所有会话
#[tauri::command]
pub async fn clear_workspace_sessions(
    workspace_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, CommandError> {
    log::info!(
        "clear_workspace_sessions 请求: workspace_id={}",
        workspace_id
    );
    let conn = state.db.conn()?;
    let session_ids = session_repo::delete_sessions_by_workspace(&conn, &workspace_id)?;
    let count = session_ids.len() as u64;
    log::info!(
        "clear_workspace_sessions 成功: workspace_id={}, 已删除 {} 条会话",
        workspace_id,
        count
    );

    // 发射会话更新事件，通知前端刷新列表
    let emitter = AgentEmitter::new(app_handle);
    for sid in &session_ids {
        let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
            session_id: sid.clone(),
            change_type: "deleted".to_string(),
            data: None,
        });
    }

    Ok(count)
}

/// 清除所有会话数据
#[tauri::command]
pub async fn clear_all_sessions(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<u64, CommandError> {
    log::info!("clear_all_sessions 请求");
    let conn = state.db.conn()?;
    let count = session_repo::clear_all_sessions(&conn)?;
    log::info!("clear_all_sessions 成功: 已删除 {} 条会话", count);

    // 发射会话更新事件，通知前端刷新列表
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: String::new(),
        change_type: "cleared".to_string(),
        data: None,
    });

    Ok(count)
}

/// 批量删除会话中的指定消息
#[tauri::command]
pub async fn delete_session_messages(
    session_id: String,
    message_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "delete_session_messages 请求: session_id={}, ids={:?}",
        session_id,
        message_ids
    );

    if message_ids.is_empty() {
        return Ok(());
    }

    // 检查会话是否有 Agent 正在运行，防止数据不一致
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::warn!(
                "delete_session_messages 失败: 会话 '{}' 的 Agent 正在运行",
                session_id
            );
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 的 Agent 正在运行，无法删除消息", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;
    // 获取当前活跃分支 ID，作为防御性过滤条件（确保不会跨分支删除消息）
    let active_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;
    message_repo::delete_messages_by_ids(&conn, &session_id, &message_ids, &active_branch_id)?;
    log::info!(
        "delete_session_messages 成功: session_id={}, 已删除 {} 条消息",
        session_id,
        message_ids.len()
    );

    Ok(())
}

/// 更新会话的工作区 ID（用于修复旧数据中 workspace_id 为空的会话）
#[tauri::command]
pub async fn update_session_workspace(
    session_id: String,
    workspace_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!(
        "update_session_workspace 请求: session_id={}, workspace_id={}",
        session_id,
        workspace_id
    );
    let conn = state.db.conn()?;
    session_repo::update_session_workspace(&conn, &session_id, &workspace_id)?;
    log::info!(
        "update_session_workspace 成功: session_id={}, workspace_id={}",
        session_id,
        workspace_id
    );
    Ok(())
}

/// 创建分支命令
/// 在指定用户消息节点处分叉出新分支：
/// 1. 复制原分支截至该消息之前（不含）的所有消息到新分支
/// 2. 为原分叉点消息设置 branch_group_id（用于 UI 切换器定位）
/// 3. 设置会话活跃分支为新分支
/// 不在此处创建 user 消息也不触发 Agent，由前端调用 start_agent 时创建 user 消息
/// 并通过 branchGroupId 参数让 run_agent 在持久化时为新 user 消息设置 branch_group_id
#[tauri::command]
pub async fn create_branch(
    session_id: String,
    fork_message_id: String,
    state: State<'_, AppState>,
) -> Result<crate::models::CreateBranchResult, CommandError> {
    // 1. 检查 Agent 未运行
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 有 Agent 正在运行，无法创建分支", session_id),
            ));
        }
    }

    let mut conn = state.db.conn()?;

    // 2. 获取当前活跃分支（原分支）
    let source_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;

    // 3. 生成新分支 ID 和分支组 ID
    let new_branch_id = format!("branch_{}", uuid::Uuid::new_v4());
    let branch_group_id = format!("bg_{}", uuid::Uuid::new_v4());
    let now = chrono::Utc::now().to_rfc3339();

    // 4. 查询同分支组内的最大 sort_order（如果是首次分叉则为 1）
    // 注意：原 fork_message_id 对应的消息可能已有 branch_group_id（之前已分叉过）
    // 此时新分支应加入同一 branch_group_id，sort_order 在该组内递增
    let (final_branch_group_id, sort_order) = {
        // 检查原消息是否已有 branch_group_id
        let existing_group_id: Option<String> = conn
            .query_row(
                "SELECT branch_group_id FROM session_messages WHERE id = ?1",
                rusqlite::params![fork_message_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();

        if let Some(existing_id) = existing_group_id {
            // 已有分支组，加入该组，sort_order 在组内递增
            let existing_branches =
                crate::db::branch_repo::list_branches_by_group(&conn, &existing_id)?;
            let max_sort = existing_branches
                .iter()
                .map(|b| b.sort_order)
                .max()
                .unwrap_or(0);
            (existing_id, max_sort + 1)
        } else {
            // 首次分叉，使用新生成的 branch_group_id，sort_order = 1
            // 原分支 main 不属于任何 branch_group（它的 fork_message_id 为 NULL）
            (branch_group_id.clone(), 1)
        }
    };

    // 5. 在事务中执行所有数据库操作，保证原子性
    let tx = conn.transaction()?;

    // 5.1 创建 Branch 记录
    let branch = crate::models::Branch {
        id: new_branch_id.clone(),
        session_id: session_id.clone(),
        parent_branch_id: Some(source_branch_id.clone()),
        fork_message_id: Some(fork_message_id.clone()),
        branch_group_id: Some(final_branch_group_id.clone()),
        name: format!("分支 {}", sort_order + 1), // sort_order=1 时显示"分支 2"（main 是"分支 1"）
        sort_order,
        created_at: now.clone(),
    };
    crate::db::branch_repo::create_branch(&tx, &branch)?;

    // 5.2 复制原分支截至 fork_message_id 之前（不含）的消息到新分支
    let copied_count = crate::db::message_repo::copy_messages_to_branch(
        &tx,
        &session_id,
        &source_branch_id,
        &fork_message_id,
        &new_branch_id,
    )?;
    log::info!(
        "创建分支 {}: 从原分支 {} 复制了 {} 条前缀消息",
        new_branch_id,
        source_branch_id,
        copied_count
    );

    // 5.3 若原 fork_message_id 对应消息的 branch_group_id 为空，则更新为新生成的 branch_group_id
    if final_branch_group_id == branch_group_id {
        // 首次分叉，需要为原消息打标
        crate::db::branch_repo::update_message_branch_group_id(
            &tx,
            &fork_message_id,
            &final_branch_group_id,
        )?;
    }

    // 5.4 设置会话活跃分支为新分支
    // 注意：不在此处创建 user 消息，由前端调用 startAgent 时创建
    // 这样避免 user 消息被重复创建（create_branch + startAgent 各创建一次）
    // 新 user 消息的 branch_group_id 由 run_agent 从活跃分支记录中获取并设置
    crate::db::branch_repo::set_session_active_branch(&tx, &session_id, &new_branch_id)?;

    tx.commit()?;

    log::info!(
        "创建分支成功: session_id={}, new_branch_id={}, branch_group_id={}",
        session_id,
        new_branch_id,
        final_branch_group_id
    );

    Ok(crate::models::CreateBranchResult {
        branch_id: new_branch_id,
        branch_group_id: final_branch_group_id,
    })
}

/// 切换会话的活跃分支
#[tauri::command]
pub async fn switch_branch(
    session_id: String,
    branch_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    // 检查 Agent 未运行
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 有 Agent 正在运行，无法切换分支", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;

    // 验证目标分支存在且属于该会话
    let branch = crate::db::branch_repo::get_branch(&conn, &branch_id)?.ok_or_else(|| {
        CommandError::db(
            crate::errors::DB_RECORD_NOT_FOUND,
            format!("分支 '{}' 不存在", branch_id),
        )
    })?;
    if branch.session_id != session_id {
        return Err(CommandError::db(
            crate::errors::DB_CONSTRAINT_VIOLATION,
            format!("分支 '{}' 不属于会话 '{}'", branch_id, session_id),
        ));
    }

    // 设置活跃分支
    crate::db::branch_repo::set_session_active_branch(&conn, &session_id, &branch_id)?;

    log::info!(
        "切换分支: session_id={}, branch_id={}",
        session_id,
        branch_id
    );
    Ok(())
}

/// 列出会话内所有分支组（用于前端渲染切换器）
#[tauri::command]
pub async fn list_branch_groups(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::BranchGroupInfo>, CommandError> {
    let conn = state.db.conn()?;
    let groups = crate::db::branch_repo::list_branch_groups(&conn, &session_id)?;
    Ok(groups)
}

/// 获取指定会话的 Todo 任务列表
#[tauri::command]
pub async fn get_todo_list(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<crate::models::todo::TodoList, CommandError> {
    let conn = state.db.conn()?;
    let todo_list = crate::db::todo_repo::get_todo_list(&conn, &session_id)?;
    Ok(todo_list)
}

/// 列出会话内所有分支的所有 user 消息（用于跨分支搜索）
#[tauri::command]
pub async fn list_all_branch_user_messages(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<crate::models::BranchUserMessage>, CommandError> {
    let conn = state.db.conn()?;
    let messages = crate::db::branch_repo::list_all_branch_user_messages(&conn, &session_id)?;
    Ok(messages)
}

/// 根据会话关联的工作区 ID 解析工作区绝对路径
/// 同步版本：使用 blocking_lock（命令在 async runtime 内，通过 block_in_place 桥接）
fn resolve_workspace_path(
    session_id: &str,
    workspace_id: &str,
    config: &std::sync::Arc<tokio::sync::Mutex<crate::config::ConfigManager>>,
) -> Result<String, CommandError> {
    if workspace_id.is_empty() {
        return Err(CommandError::fs(
            crate::errors::FS_PATH_NOT_FOUND,
            format!("会话 '{}' 未关联工作区，无法回退代码", session_id),
        ));
    }
    let cfg = tokio::task::block_in_place(|| config.blocking_lock());
    let ws_config = cfg.load_workspaces()?;
    ws_config
        .workspaces
        .iter()
        .find(|w| w.id == workspace_id)
        .map(|w| w.path.clone())
        .ok_or_else(|| {
            CommandError::fs(
                crate::errors::FS_PATH_NOT_FOUND,
                format!("工作区 '{workspace_id}' 不存在或已被删除",),
            )
        })
}

/// 获取快照备份根目录（app_data_dir/snapshots），不存在则创建
fn snapshot_backup_base(app_handle: &AppHandle) -> Result<std::path::PathBuf, CommandError> {
    let app_data_dir = app_handle.path().app_data_dir()?;
    let backup_base = app_data_dir.join("snapshots");
    std::fs::create_dir_all(&backup_base).map_err(|e| {
        CommandError::fs(crate::errors::FS_IO_ERROR, format!("创建快照目录失败: {e}"))
    })?;
    Ok(backup_base)
}

/// 回退消息：将工作流回退到指定用户消息节点之前
///
/// 流程（staged 模式，支持 redo）：
/// 1. 校验：Agent 未运行、目标消息存在且是 user 消息且属于当前活跃分支
/// 2. 首次回退时创建 redo 基线快照（当前文件状态）；重复回退保留首次的 redo 基线
/// 3. 用目标消息的快照恢复被回退范围内工具调用涉及的文件
/// 4. 写入 session_reverts 状态（消息隐藏而非物理删除，redo 时恢复）
#[tauri::command]
pub async fn rollback_session_messages(
    session_id: String,
    message_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::models::snapshot::RollbackResult, CommandError> {
    log::info!(
        "rollback_session_messages 请求: session_id={}, message_id={}",
        session_id,
        message_id
    );

    // 1. Agent 运行中拒绝
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::warn!(
                "rollback_session_messages 失败: 会话 '{}' 的 Agent 正在运行",
                session_id
            );
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 的 Agent 正在运行，无法回退消息", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;
    let active_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;

    // 2. 校验目标消息：存在、属于当前活跃分支、是 user 消息
    let (role, msg_branch, _) = message_repo::get_message(&conn, &session_id, &message_id)?
        .ok_or_else(|| {
            CommandError::db(
                crate::errors::DB_RECORD_NOT_FOUND,
                format!("消息 '{message_id}' 不存在，无法回退"),
            )
        })?;
    if msg_branch != active_branch_id {
        return Err(CommandError::db(
            crate::errors::DB_CONSTRAINT_VIOLATION,
            "目标消息不属于当前活跃分支，无法回退".to_string(),
        ));
    }
    if role != "user" {
        return Err(CommandError::db(
            crate::errors::DB_CONSTRAINT_VIOLATION,
            "只能回退用户消息节点".to_string(),
        ));
    }

    // 3. 解析会话工作区路径与快照备份目录
    let session = session_repo::get_session(&conn, &session_id)?;
    let workspace_path = resolve_workspace_path(
        &session_id,
        session.workspace_id.as_deref().unwrap_or(""),
        &state.config,
    )?;
    let backup_base = snapshot_backup_base(&app_handle)?;

    // 4. 已有 staged revert：保留首次的 redo 基线快照（OpenCode 行为），仅更新边界
    let existing_revert = crate::db::revert_repo::get_revert(&conn, &session_id)?;
    let redo_snapshot_id = match existing_revert {
        Some(r) => {
            log::info!(
                "rollback: 会话 '{}' 已有 staged revert（边界 {}），保留原 redo 基线",
                session_id,
                r.revert_message_id
            );
            r.redo_snapshot_id
        }
        None => {
            // 创建 redo 基线快照（当前文件状态，用于撤销回退）
            let (kind, snapshot_ref) =
                crate::services::snapshot::create_snapshot(&workspace_path, &backup_base)?;
            let redo_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            crate::db::snapshot_repo::create_snapshot(
                &conn,
                &crate::models::snapshot::SnapshotRecord {
                    id: redo_id.clone(),
                    session_id: session_id.clone(),
                    message_id: None,
                    kind: kind.as_str().to_string(),
                    snapshot_ref,
                    workspace_path: workspace_path.clone(),
                    created_at: now,
                },
            )?;
            redo_id
        }
    };

    // 5. 查询目标消息的快照并恢复文件
    let target_snapshot = crate::db::snapshot_repo::get_snapshot_by_message_id(&conn, &message_id)?;
    let (restored_file_count, code_reverted, snapshot_kind) = if let Some(snap) = target_snapshot {
        // 提取被回退范围内消息的工具调用路径
        let range_messages =
            message_repo::list_messages_from(&conn, &session_id, &active_branch_id, &message_id);
        let paths = crate::services::snapshot::collect_tool_paths(&range_messages);
        let kind = crate::services::snapshot::kind_from_str(&snap.kind);
        let restored = crate::services::snapshot::restore_snapshot(
            kind,
            &snap.snapshot_ref,
            &workspace_path,
            &backup_base,
            &paths,
        )?;
        log::info!(
            "rollback: 恢复文件完成, session_id={}, restored={}",
            session_id,
            restored
        );
        (restored, true, Some(snap.kind.clone()))
    } else {
        // 无快照的旧消息：仅回退对话，代码不回退
        log::warn!(
            "rollback: 消息 '{}' 无快照，仅回退对话（代码未回退）",
            message_id
        );
        (0, false, None)
    };

    // 6. 统计被隐藏的消息数（含边界消息自身）
    let hidden_count =
        message_repo::count_messages_from(&conn, &session_id, &active_branch_id, &message_id);

    // 7. 回退后会话是否已无任何消息（边界为首条消息且无其他分支消息）
    // 此时整个会话失去意义（文件已恢复），删除会话并清理所有快照备份（含 redo 基线）
    let all_messages = message_repo::count_session_messages(&conn, &session_id);
    let session_deleted = if hidden_count >= all_messages {
        for snap in crate::db::snapshot_repo::list_snapshots_by_session(&conn, &session_id)? {
            let kind = crate::services::snapshot::kind_from_str(&snap.kind);
            let _ =
                crate::services::snapshot::delete_backup(kind, &snap.snapshot_ref, &backup_base);
        }
        crate::db::session_repo::delete_session(&conn, &session_id)?;
        log::info!(
            "rollback: 会话 '{}' 回退后已无任何消息，删除整个会话",
            session_id
        );
        true
    } else {
        // 8. 写入回退状态（消息 staged 隐藏）
        crate::db::revert_repo::set_revert(
            &conn,
            &crate::db::revert_repo::RevertRecord {
                session_id: session_id.clone(),
                revert_message_id: message_id.clone(),
                redo_snapshot_id,
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        )?;

        // 9. 发射会话更新事件，通知前端刷新工作流
        let emitter = AgentEmitter::new(app_handle);
        let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
            session_id: session_id.clone(),
            change_type: "updated".to_string(),
            data: Some(serde_json::json!({ "revert": { "revertMessageId": message_id } })),
        });
        false
    };
    log::info!(
        "rollback_session_messages 成功: session_id={}, 边界={}, 隐藏={}, 恢复文件={}, 删除会话={}",
        session_id,
        message_id,
        hidden_count,
        restored_file_count,
        session_deleted
    );

    Ok(crate::models::snapshot::RollbackResult {
        revert_message_id: message_id,
        hidden_count,
        restored_file_count,
        code_reverted,
        snapshot_kind,
        session_deleted,
    })
}

/// 撤销回退（redo）：恢复 redo 基线快照文件并清除 revert 状态
///
/// 被隐藏的消息采用 staged 模式（物理仍在 DB），清除 revert 状态后自动恢复显示
#[tauri::command]
pub async fn redo_session_messages(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<crate::models::snapshot::RedoResult, CommandError> {
    log::info!("redo_session_messages 请求: session_id={}", session_id);

    // 1. Agent 运行中拒绝
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            log::warn!(
                "redo_session_messages 失败: 会话 '{}' 的 Agent 正在运行",
                session_id
            );
            return Err(CommandError::agent(
                crate::errors::AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 的 Agent 正在运行，无法撤销回退", session_id),
            ));
        }
    }

    let conn = state.db.conn()?;

    // 2. 无 revert 状态时 no-op
    let revert = match crate::db::revert_repo::get_revert(&conn, &session_id)? {
        Some(r) => r,
        None => {
            log::info!("redo: 会话 '{}' 无回退状态，忽略", session_id);
            return Ok(crate::models::snapshot::RedoResult { hidden_count: 0 });
        }
    };
    let active_branch_id = crate::db::branch_repo::get_session_active_branch(&conn, &session_id)?;

    // 3. 边界属于当前活跃分支时才生效（分支切换后忽略）
    let belongs = message_repo::get_message(&conn, &session_id, &revert.revert_message_id)?
        .map(|(_, branch, _)| branch == active_branch_id)
        .unwrap_or(false);
    if !belongs {
        log::warn!(
            "redo: 回退边界 '{}' 不属于当前活跃分支 '{}'，忽略",
            revert.revert_message_id,
            active_branch_id
        );
        return Ok(crate::models::snapshot::RedoResult { hidden_count: 0 });
    }
    let hidden_count = message_repo::count_messages_from(
        &conn,
        &session_id,
        &active_branch_id,
        &revert.revert_message_id,
    );

    // 4. 解析工作区与备份目录
    let session = session_repo::get_session(&conn, &session_id)?;
    let workspace_path = resolve_workspace_path(
        &session_id,
        session.workspace_id.as_deref().unwrap_or(""),
        &state.config,
    )?;
    let backup_base = snapshot_backup_base(&app_handle)?;

    // 5. 用 redo 基线快照恢复文件（回退范围内工具路径）
    let redo_snap = crate::db::snapshot_repo::get_snapshot_by_id(&conn, &revert.redo_snapshot_id)?;
    if let Some(redo_snap) = &redo_snap {
        let range_messages = message_repo::list_messages_from(
            &conn,
            &session_id,
            &active_branch_id,
            &revert.revert_message_id,
        );
        let paths = crate::services::snapshot::collect_tool_paths(&range_messages);
        let kind = crate::services::snapshot::kind_from_str(&redo_snap.kind);
        let restored = crate::services::snapshot::restore_snapshot(
            kind,
            &redo_snap.snapshot_ref,
            &workspace_path,
            &backup_base,
            &paths,
        )?;
        log::info!(
            "redo: 恢复文件完成, session_id={}, restored={}",
            session_id,
            restored
        );
    } else {
        log::warn!(
            "redo: redo 基线快照 '{}' 不存在，仅恢复对话",
            revert.redo_snapshot_id
        );
    }

    // 6. 清除回退状态（消息恢复显示），并清理已使用完毕的 redo 基线快照（记录+备份）
    crate::db::revert_repo::clear_revert(&conn, &session_id)?;
    if let Some(redo_snap) = redo_snap {
        let kind = crate::services::snapshot::kind_from_str(&redo_snap.kind);
        let _ =
            crate::services::snapshot::delete_backup(kind, &redo_snap.snapshot_ref, &backup_base);
        let _ = crate::db::snapshot_repo::delete_snapshots_by_ids(
            &conn,
            std::slice::from_ref(&revert.redo_snapshot_id),
        );
    }
    log::info!(
        "redo_session_messages 成功: session_id={}, 恢复消息数={}",
        session_id,
        hidden_count
    );

    // 7. 发射会话更新事件，通知前端刷新工作流
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session_id.clone(),
        change_type: "updated".to_string(),
        data: Some(serde_json::json!({ "redo": true })),
    });

    Ok(crate::models::snapshot::RedoResult { hidden_count })
}
