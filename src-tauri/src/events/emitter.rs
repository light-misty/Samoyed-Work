use tauri::{AppHandle, Emitter, Runtime};

use crate::errors::CommandError;
use super::types;

/// Agent 事件发射器，封装 Tauri 事件发送逻辑
pub struct AgentEmitter<R: Runtime> {
    app_handle: AppHandle<R>,
}

impl<R: Runtime> AgentEmitter<R> {
    /// 创建事件发射器实例
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self { app_handle }
    }

    /// 发射 Agent 思考链增量事件
    pub fn emit_thinking(&self, payload: types::ThinkingPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_THINKING, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Agent 回复内容增量事件
    pub fn emit_content(&self, payload: types::ContentPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_CONTENT, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Tool 调用开始事件
    pub fn emit_tool_call(&self, payload: types::ToolCallPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_TOOL_CALL, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Tool 执行结果事件
    pub fn emit_tool_result(&self, payload: types::ToolResultPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_TOOL_RESULT, payload)
            .map_err(CommandError::from)
    }

    /// 发射需要用户确认的事件
    pub fn emit_confirm(&self, payload: types::ConfirmPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_CONFIRM, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Todo 列表更新事件
    pub fn emit_todo_update(&self, payload: types::TodoUpdatePayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_TODO_UPDATE, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Agent 执行完成事件
    pub fn emit_done(&self, payload: types::DonePayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_DONE, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Agent 执行错误事件
    pub fn emit_error(&self, payload: types::ErrorPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_ERROR, payload)
            .map_err(CommandError::from)
    }

    /// 发射 Agent 执行中断事件
    pub fn emit_stopped(&self, payload: types::StoppedPayload) -> Result<(), CommandError> {
        self.app_handle
            .emit(types::AGENT_STOPPED, payload)
            .map_err(CommandError::from)
    }
}
