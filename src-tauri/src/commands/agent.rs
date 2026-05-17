use tauri::{AppHandle, Manager, State};

use crate::errors::{CommandError, AGENT_ALREADY_RUNNING, AGENT_NOT_RUNNING, AGENT_SESSION_NOT_FOUND};
use crate::events::AgentEmitter;
use crate::events::types::{
    ContentPayload, DonePayload, ThinkingPayload, TodoItem, TodoUpdatePayload,
};
use crate::AppState;

/// 启动 Agent 执行，在后台 spawn 一个 tokio task
#[tauri::command]
pub async fn start_agent(
    session_id: String,
    prompt: String,
    options: Option<serde_json::Value>,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    // 检查是否已有 Agent 在该会话中运行
    {
        let active = state.active_agents.lock().await;
        if active.contains_key(&session_id) {
            return Err(CommandError::agent(
                AGENT_ALREADY_RUNNING,
                format!("会话 '{}' 已有 Agent 正在运行", session_id),
            ));
        }
    }

    // 注册为活跃 Agent
    {
        let mut active = state.active_agents.lock().await;
        active.insert(session_id.clone(), true);
    }

    let emitter = AgentEmitter::new(app_handle.clone());
    let sid = session_id.clone();

    // 在后台 spawn Agent 执行循环
    tokio::spawn(async move {
        let _ = run_agent_loop(&sid, &prompt, &emitter).await;

        // 无论成功或失败，都从活跃列表中移除
        let app_state = app_handle.state::<AppState>();
        {
            let mut active = app_state.active_agents.lock().await;
            active.remove(&sid);
        }
    });

    let _ = options;
    Ok(())
}

/// 停止 Agent 执行
#[tauri::command]
pub async fn stop_agent(
    session_id: String,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let mut active = state.active_agents.lock().await;
    if !active.contains_key(&session_id) {
        return Err(CommandError::agent(
            AGENT_NOT_RUNNING,
            format!("会话 '{}' 没有 Agent 在运行", session_id),
        ));
    }

    // 标记为停止
    active.insert(session_id.clone(), false);
    Ok(())
}

/// 确认 Agent 操作
#[tauri::command]
pub async fn confirm_operation(
    session_id: String,
    operation_id: String,
    approved: bool,
    feedback: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    let active = state.active_agents.lock().await;
    if !active.contains_key(&session_id) {
        return Err(CommandError::agent(
            AGENT_SESSION_NOT_FOUND,
            format!("会话 '{}' 没有 Agent 在运行", session_id),
        ));
    }

    // 后续实现：将确认结果写入 channel，通知 Agent 循环继续
    log::info!(
        "操作确认: session={}, operation={}, approved={}, feedback={:?}",
        session_id,
        operation_id,
        approved,
        feedback
    );

    Ok(())
}

/// Agent 执行循环（当前为占位实现，后续接入 LLM 适配器）
async fn run_agent_loop(
    session_id: &str,
    prompt: &str,
    emitter: &AgentEmitter<tauri::Wry>,
) -> Result<(), CommandError> {
    // 发送思考链开始事件
    emitter.emit_thinking(ThinkingPayload {
        session_id: session_id.to_string(),
        step: 1,
        thought: format!("正在分析用户需求: {}", prompt),
    })?;

    // 发送 Todo 列表更新事件
    emitter.emit_todo_update(TodoUpdatePayload {
        session_id: session_id.to_string(),
        todos: vec![
            TodoItem {
                id: "1".to_string(),
                content: "分析用户需求".to_string(),
                status: "completed".to_string(),
            },
            TodoItem {
                id: "2".to_string(),
                content: "生成文档内容".to_string(),
                status: "in_progress".to_string(),
            },
            TodoItem {
                id: "3".to_string(),
                content: "输出结果".to_string(),
                status: "pending".to_string(),
            },
        ],
    })?;

    // 发送内容增量事件
    emitter.emit_content(ContentPayload {
        session_id: session_id.to_string(),
        message_id: uuid::Uuid::new_v4().to_string(),
        content: format!("收到您的请求: {}\n\nAgent 引擎正在开发中，此为占位响应。", prompt),
        is_streaming: false,
    })?;

    // 发送完成事件
    emitter.emit_done(DonePayload {
        session_id: session_id.to_string(),
        summary: "Agent 占位执行完成".to_string(),
        total_steps: 1,
        total_tokens: 0,
        duration_ms: 100,
    })?;

    Ok(())
}
