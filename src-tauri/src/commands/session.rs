use tauri::{AppHandle, State};

use crate::db::session_repo;
use crate::db::message_repo;
use crate::db::token_repo;
use crate::errors::CommandError;
use crate::events::AgentEmitter;
use crate::events::types;
use crate::models::session::{
    CreateSessionParams, Session, SessionDetail, SessionFilter, SessionSummary, TokenUsage,
};
use crate::AppState;

/// 创建新会话
#[tauri::command]
pub async fn create_session(
    params: CreateSessionParams,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<Session, CommandError> {
    log::info!("create_session 请求: title={:?}, workspace_id={:?}, provider_id={:?}", params.title, params.workspace_id, params.provider_id);
    let id = uuid::Uuid::new_v4().to_string();
    let title = params.title.unwrap_or_else(|| "新会话".to_string());
    let workspace_id = params.workspace_id.unwrap_or_default();
    let provider_id = params.provider_id.unwrap_or_default();

    let conn = state.db.conn()?;
    session_repo::create_session(
        &conn,
        &id,
        &workspace_id,
        &title,
        &provider_id,
        "",
    )?;

    let session = session_repo::get_session(&conn, &id)?;
    log::info!("create_session 成功: session_id={}, title={}", session.id, session.title);

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

    log::debug!("list_sessions 查询条件: workspace_id={:?}, status={:?}, search={:?}, limit={}, offset={}", workspace_id, status, search, limit, offset);
    let result = session_repo::list_sessions(&conn, workspace_id, status, search, limit, offset);
    log::info!("list_sessions 成功: 返回 {} 条记录", result.len());
    Ok(result)
}

/// 获取会话详情，包含消息历史和 Token 用量
#[tauri::command]
pub async fn get_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionDetail, CommandError> {
    log::info!("get_session 请求: session_id={}", session_id);
    let conn = state.db.conn()?;
    let session = session_repo::get_session(&conn, &session_id)?;
    let messages = message_repo::list_messages(&conn, &session_id);

    let (input_tokens, output_tokens) = token_repo::get_session_usage(&conn, &session_id);
    let token_usage = TokenUsage {
        prompt_tokens: input_tokens as u64,
        completion_tokens: output_tokens as u64,
        total_tokens: (input_tokens + output_tokens) as u64,
    };

    log::info!("get_session 成功: session_id={}, 消息数={}, tokens={}", session_id, messages.len(), token_usage.total_tokens);
    Ok(SessionDetail {
        session,
        messages,
        token_usage,
    })
}

/// 删除会话
#[tauri::command]
pub async fn delete_session(
    session_id: String,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    log::info!("delete_session 请求: session_id={}", session_id);
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
    log::info!("update_session_title 请求: session_id={}, title={}", session_id, title);
    let conn = state.db.conn()?;
    session_repo::update_session_title(&conn, &session_id, &title)?;
    log::info!("update_session_title 成功: session_id={}, title={}", session_id, title);

    // 发射会话更新事件
    let emitter = AgentEmitter::new(app_handle);
    let _ = emitter.emit_session_updated(types::SessionUpdatePayload {
        session_id: session_id.clone(),
        change_type: "updated".to_string(),
        data: Some(serde_json::json!({ "title": title })),
    });

    Ok(())
}
