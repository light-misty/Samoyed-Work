use tauri::State;

use crate::db::session_repo;
use crate::db::message_repo;
use crate::db::token_repo;
use crate::errors::CommandError;
use crate::models::session::{
    CreateSessionParams, Session, SessionDetail, SessionFilter, SessionSummary, TokenUsage,
};
use crate::AppState;

/// 创建新会话
#[tauri::command]
pub async fn create_session(
    params: CreateSessionParams,
    state: State<'_, AppState>,
) -> Result<Session, CommandError> {
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
    Ok(session)
}

/// 列出会话，支持筛选
#[tauri::command]
pub async fn list_sessions(
    filter: Option<SessionFilter>,
    state: State<'_, AppState>,
) -> Result<Vec<SessionSummary>, CommandError> {
    let conn = state.db.conn()?;

    let workspace_id = filter.as_ref().and_then(|f| f.workspace_id.as_deref());
    let status = filter.as_ref().and_then(|f| f.status.as_deref());
    let search = filter.as_ref().and_then(|f| f.search.as_deref());
    let limit = filter.as_ref().and_then(|f| f.limit).unwrap_or(50);
    let offset = filter.as_ref().and_then(|f| f.offset).unwrap_or(0);

    Ok(session_repo::list_sessions(&conn, workspace_id, status, search, limit, offset))
}

/// 获取会话详情，包含消息历史和 Token 用量
#[tauri::command]
pub async fn get_session(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<SessionDetail, CommandError> {
    let conn = state.db.conn()?;
    let session = session_repo::get_session(&conn, &session_id)?;
    let messages = message_repo::list_messages(&conn, &session_id);

    let (input_tokens, output_tokens) = token_repo::get_session_usage(&conn, &session_id);
    let token_usage = TokenUsage {
        prompt_tokens: input_tokens as u64,
        completion_tokens: output_tokens as u64,
        total_tokens: (input_tokens + output_tokens) as u64,
    };

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
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let conn = state.db.conn()?;
    session_repo::delete_session(&conn, &session_id)?;
    Ok(())
}

/// 更新会话标题
#[tauri::command]
pub async fn update_session_title(
    session_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let conn = state.db.conn()?;
    session_repo::update_session_title(&conn, &session_id, &title)?;
    Ok(())
}
