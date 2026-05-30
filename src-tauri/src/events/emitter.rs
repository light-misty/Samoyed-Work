use tauri::{AppHandle, Emitter, Runtime};

use crate::errors::CommandError;
use super::types;

/// Agent 事件发射器，封装 Tauri 事件发送逻辑
/// 所有 emit 方法返回 Result，调用方可根据事件重要性决定是否忽略错误
/// 关键事件（error/done/stopped）失败时会额外记录 warn 级别日志
pub struct AgentEmitter<R: Runtime> {
    app_handle: AppHandle<R>,
}

impl<R: Runtime> Clone for AgentEmitter<R> {
    fn clone(&self) -> Self {
        Self {
            app_handle: self.app_handle.clone(),
        }
    }
}

impl<R: Runtime> AgentEmitter<R> {
    /// 创建事件发射器实例
    pub fn new(app_handle: AppHandle<R>) -> Self {
        Self { app_handle }
    }

    /// 内部统一发射方法，根据事件重要性选择日志级别
    fn emit_event<T: Clone + serde::Serialize + std::fmt::Debug>(
        &self,
        event: &str,
        payload: T,
        critical: bool,
    ) -> Result<(), CommandError> {
        log::debug!("发射事件: {}", event);
        self.app_handle
            .emit(event, payload)
            .map_err(|e| {
                // 关键事件发射失败用 warn 级别，非关键用 debug 级别
                if critical {
                    log::warn!("关键事件 {} 发射失败: {}", event, e);
                } else {
                    log::debug!("事件 {} 发射失败（非关键）: {}", event, e);
                }
                CommandError::from(e)
            })
    }

    /// 发射 Agent 思考链增量事件（非关键）
    pub fn emit_thinking(&self, payload: types::ThinkingPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_THINKING, payload, false)
    }

    /// 发射 Agent 深度思考链增量事件（非关键）
    pub fn emit_deep_thinking(&self, payload: types::DeepThinkingPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_DEEP_THINKING, payload, false)
    }

    /// 发射 Agent 回复内容增量事件（非关键）
    pub fn emit_content(&self, payload: types::ContentPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_CONTENT, payload, false)
    }

    /// 发射 Tool 调用开始事件（非关键）
    pub fn emit_tool_call(&self, payload: types::ToolCallPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_TOOL_CALL, payload, false)
    }

    /// 发射 Tool 执行结果事件（非关键）
    pub fn emit_tool_result(&self, payload: types::ToolResultPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_TOOL_RESULT, payload, false)
    }

    /// 发射需要用户确认的事件（关键 - 用户必须收到确认请求）
    pub fn emit_confirm(&self, payload: types::ConfirmPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_CONFIRM, payload, true)
    }

    /// 发射 Todo 列表更新事件（非关键）
    pub fn emit_todo_update(&self, payload: types::TodoUpdatePayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_TODO_UPDATE, payload, false)
    }

    /// 发射 Agent 执行完成事件（关键 - 前端依赖此事件更新状态）
    pub fn emit_done(&self, payload: types::DonePayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_DONE, payload, true)
    }

    /// 发射 Agent 执行错误事件（关键 - 前端依赖此事件显示错误信息）
    pub fn emit_error(&self, payload: types::ErrorPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_ERROR, payload, true)
    }

    /// 发射 Agent 执行中断事件（关键 - 前端依赖此事件更新状态）
    pub fn emit_stopped(&self, payload: types::StoppedPayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_STOPPED, payload, true)
    }

    /// 发射会话更新事件（关键 - 前端依赖此事件刷新会话列表）
    pub fn emit_session_updated(&self, payload: types::SessionUpdatePayload) -> Result<(), CommandError> {
        self.emit_event(types::SESSION_UPDATED, payload, true)
    }

    /// 发射工作区变更事件（关键 - 前端依赖此事件刷新文件树）
    pub fn emit_workspace_change(&self, payload: types::WorkspaceChangePayload) -> Result<(), CommandError> {
        self.emit_event(types::WORKSPACE_CHANGE, payload, true)
    }

    /// 发射 LLM Provider 切换通知事件（非关键）
    pub fn emit_provider_switch(&self, payload: types::ProviderSwitchPayload) -> Result<(), CommandError> {
        self.emit_event(types::LLM_PROVIDER_SWITCH, payload, false)
    }

    /// 发射上下文窗口使用情况更新事件（非关键）
    pub fn emit_context_usage(&self, payload: types::ContextUsagePayload) -> Result<(), CommandError> {
        self.emit_event(types::AGENT_CONTEXT_UPDATE, payload, false)
    }
}
