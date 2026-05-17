/**
 * Agent 交互 Hook
 * 使用 React hooks 封装 Agent 调用逻辑
 * 管理 isLoading、error、sessionId 状态
 * 自动监听所有 Agent 事件并更新状态
 * 组件卸载时取消所有事件监听
 */
import { useState, useCallback, useEffect, useRef } from "react";

import * as tauriCmd from "../services/tauri";
import {
  onAgentThinking,
  onAgentContent,
  onAgentToolCall,
  onAgentToolResult,
  onAgentConfirm,
  onAgentTodoUpdate,
  onAgentDone,
  onAgentError,
  onAgentStopped,
  type ThinkingPayload,
  type ToolCallPayload,
  type ToolResultPayload,
  type ConfirmPayload,
  type TodoUpdatePayload,
  type DonePayload,
} from "../services/event";

/** Agent Hook 返回值类型 */
export interface UseAgentReturn {
  /** 是否正在执行 */
  isLoading: boolean;
  /** 错误信息 */
  error: string | null;
  /** 当前会话 ID */
  sessionId: string | null;
  /** 最后一条思考内容 */
  lastThinking: ThinkingPayload | null;
  /** 累积的内容 */
  content: string;
  /** 当前 Tool 调用 */
  currentToolCall: ToolCallPayload | null;
  /** 最后一个 Tool 结果 */
  lastToolResult: ToolResultPayload | null;
  /** 待确认的操作 */
  pendingConfirmation: ConfirmPayload | null;
  /** Todo 列表 */
  todos: TodoUpdatePayload | null;
  /** 执行完成结果 */
  doneResult: DonePayload | null;
  /** 发送消息，启动 Agent */
  sendMessage: (prompt: string, options?: Record<string, unknown>) => Promise<void>;
  /** 停止 Agent */
  stopAgent: () => Promise<void>;
  /** 确认操作 */
  confirmOperation: (operationId: string, approved: boolean, feedback?: string) => Promise<void>;
  /** 重置状态 */
  reset: () => void;
}

/** 初始状态 */
const initialState = {
  isLoading: false,
  error: null as string | null,
  sessionId: null as string | null,
  lastThinking: null as ThinkingPayload | null,
  content: "",
  currentToolCall: null as ToolCallPayload | null,
  lastToolResult: null as ToolResultPayload | null,
  pendingConfirmation: null as ConfirmPayload | null,
  todos: null as TodoUpdatePayload | null,
  doneResult: null as DonePayload | null,
};

/**
 * Agent 交互 Hook
 * 封装 Agent 调用逻辑，自动管理事件监听和状态更新
 */
export function useAgent(): UseAgentReturn {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [lastThinking, setLastThinking] = useState<ThinkingPayload | null>(null);
  const [content, setContent] = useState("");
  const [currentToolCall, setCurrentToolCall] = useState<ToolCallPayload | null>(null);
  const [lastToolResult, setLastToolResult] = useState<ToolResultPayload | null>(null);
  const [pendingConfirmation, setPendingConfirmation] = useState<ConfirmPayload | null>(null);
  const [todos, setTodos] = useState<TodoUpdatePayload | null>(null);
  const [doneResult, setDoneResult] = useState<DonePayload | null>(null);

  // 保存事件取消监听函数的引用
  const unlistenRefs = useRef<(() => void)[]>([]);

  // 注册所有 Agent 事件监听
  useEffect(() => {
    const registerListeners = async () => {
      const unlisteners = await Promise.all([
        onAgentThinking((payload) => {
          setLastThinking(payload);
        }),
        onAgentContent((payload) => {
          setContent((prev) => prev + payload.content);
        }),
        onAgentToolCall((payload) => {
          setCurrentToolCall(payload);
        }),
        onAgentToolResult((payload) => {
          setLastToolResult(payload);
          setCurrentToolCall(null);
        }),
        onAgentConfirm((payload) => {
          setPendingConfirmation(payload);
        }),
        onAgentTodoUpdate((payload) => {
          setTodos(payload);
        }),
        onAgentDone((payload) => {
          setIsLoading(false);
          setDoneResult(payload);
        }),
        onAgentError((payload) => {
          setIsLoading(false);
          setError(payload.message);
        }),
        onAgentStopped((payload) => {
          setIsLoading(false);
          // 中断也视为完成，保留中断原因
          setError(`Agent 已停止: ${payload.reason}`);
        }),
      ]);

      unlistenRefs.current = unlisteners;
    };

    registerListeners();

    // 组件卸载时取消所有事件监听
    return () => {
      unlistenRefs.current.forEach((unlisten) => unlisten());
      unlistenRefs.current = [];
    };
  }, []);

  /** 发送消息，启动 Agent */
  const sendMessage = useCallback(
    async (prompt: string, options?: Record<string, unknown>) => {
      // 重置状态
      setError(null);
      setContent("");
      setLastThinking(null);
      setCurrentToolCall(null);
      setLastToolResult(null);
      setPendingConfirmation(null);
      setTodos(null);
      setDoneResult(null);
      setIsLoading(true);

      try {
        // 如果没有会话 ID，先创建一个新会话
        let sid = sessionId;
        if (!sid) {
          const session = await tauriCmd.createSession({});
          sid = session.id;
          setSessionId(sid);
        }

        await tauriCmd.startAgent(sid, prompt, options);
      } catch (err) {
        setIsLoading(false);
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [sessionId],
  );

  /** 停止 Agent */
  const stopAgent = useCallback(async () => {
    if (!sessionId) return;

    try {
      await tauriCmd.stopAgent(sessionId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [sessionId]);

  /** 确认操作 */
  const confirmOperation = useCallback(
    async (operationId: string, approved: boolean, feedback?: string) => {
      if (!sessionId) return;

      try {
        await tauriCmd.confirmOperation(sessionId, operationId, approved, feedback);
        setPendingConfirmation(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [sessionId],
  );

  /** 重置所有状态 */
  const reset = useCallback(() => {
    setIsLoading(initialState.isLoading);
    setError(initialState.error);
    setSessionId(initialState.sessionId);
    setLastThinking(initialState.lastThinking);
    setContent(initialState.content);
    setCurrentToolCall(initialState.currentToolCall);
    setLastToolResult(initialState.lastToolResult);
    setPendingConfirmation(initialState.pendingConfirmation);
    setTodos(initialState.todos);
    setDoneResult(initialState.doneResult);
  }, []);

  return {
    isLoading,
    error,
    sessionId,
    lastThinking,
    content,
    currentToolCall,
    lastToolResult,
    pendingConfirmation,
    todos,
    doneResult,
    sendMessage,
    stopAgent,
    confirmOperation,
    reset,
  };
}
