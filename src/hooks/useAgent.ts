import { useState, useCallback, useEffect, useRef } from "react";

import * as tauriCmd from "../services/tauri";
import {
  onAgentThinking,
  onAgentDeepThinking,
  onAgentContent,
  onAgentToolCall,
  onAgentToolResult,
  onAgentConfirm,
  onAgentDone,
  onAgentError,
  onAgentStopped,
  onAgentNetworkRetry,
  onAgentCompactionStart,
  onAgentCompactionDone,
  onAgentSnapshotCreated,
  onSubAgentStatus,
  onSubAgentToolCall,
  onSubAgentThinking,
  onSubAgentContent,
  onSubAgentToolResult,
  onQuestion,
  type ThinkingPayload,
  type DeepThinkingPayload,
  type ToolCallPayload,
  type ToolResultPayload,
  type ConfirmPayload,
  type DonePayload,
  type NetworkRetryPayload,
} from "../services/event";
import i18n from "../i18n";
import { useWorkflowStore, setCurrentSessionId, applySnapshotNode, type BackgroundAgentEvent, type ToolEventRefs } from "../stores/useWorkflowStore";
import { useAttachmentStore } from "../stores/useAttachmentStore";
import { useAgentModeStore } from "../stores/useAgentModeStore";
import type { NodeStatus, SubAgentNodeData, WorkflowNode } from "../types";

export interface UseAgentReturn {
  isLoading: boolean;
  error: string | null;
  sessionId: string | null;
  lastThinking: ThinkingPayload | null;
  deepThinking: DeepThinkingPayload | null;
  content: string;
  currentToolCall: ToolCallPayload | null;
  lastToolResult: ToolResultPayload | null;
  pendingConfirmation: ConfirmPayload | null;
  doneResult: DonePayload | null;
  isStopped: boolean;
  networkRetry: NetworkRetryPayload | null;
  sendMessage: (prompt: string, options?: Record<string, unknown>) => Promise<void>;
  stopAgent: () => Promise<void>;
  confirmOperation: (operationId: string, approved: boolean, feedback?: string) => Promise<void>;
  /** 权限审批回复（once/reject 双态） */
  respondPermission: (operationId: string, response: 'once' | 'reject', feedback?: string) => Promise<void>;
  reset: () => void;
  setSessionId: (id: string) => void;
  /** 当前 streaming content 节点 ID */
  streamingNodeIdRef: { current: string | null };
  /** 当前 thinking 节点 ID */
  thinkingNodeIdRef: { current: string | null };
  /** 当前迭代轮次 */
  currentIterationRef: { current: number | undefined };
  /** 被 tool_call 关闭的 streaming 节点 ID */
  lastClosedStreamingNodeIdRef: { current: string | null };
  /** 最后一次 tool_call 的迭代轮次，用于过滤残余 content 事件 */
  lastToolCallIterationRef: { current: number | null };
}

const initialState = {
  isLoading: false,
  error: null as string | null,
  sessionId: null as string | null,
  lastThinking: null as ThinkingPayload | null,
  deepThinking: null as DeepThinkingPayload | null,
  content: "",
  currentToolCall: null as ToolCallPayload | null,
  lastToolResult: null as ToolResultPayload | null,
  pendingConfirmation: null as ConfirmPayload | null,
  doneResult: null as DonePayload | null,
  isStopped: false,
  networkRetry: null as NetworkRetryPayload | null,
};

/** 将后台会话事件路由到 useWorkflowStore 的缓存 */
function routeBackgroundEvent(sessionId: string, event: BackgroundAgentEvent) {
  useWorkflowStore.getState().applyBackgroundEvent(sessionId, event);
}

export function useAgent(): UseAgentReturn {
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [sessionId, setSessionId] = useState<string | null>(null);
  const [lastThinking, setLastThinking] = useState<ThinkingPayload | null>(null);
  const [deepThinking, setDeepThinking] = useState<DeepThinkingPayload | null>(null);
  const [content, setContent] = useState("");
  const [currentToolCall, setCurrentToolCall] = useState<ToolCallPayload | null>(null);
  const [lastToolResult, setLastToolResult] = useState<ToolResultPayload | null>(null);
  const [pendingConfirmation, setPendingConfirmation] = useState<ConfirmPayload | null>(null);
  const [doneResult, setDoneResult] = useState<DonePayload | null>(null);
  const [isStopped, setIsStopped] = useState(false);
  const [networkRetry, setNetworkRetry] = useState<NetworkRetryPayload | null>(null);

  const unlistenRefs = useRef<(() => void)[]>([]);
  const sessionIdRef = useRef<string | null>(null);
  const contentEpochRef = useRef(0);
  const lastContentEpochRef = useRef(0);
  // 深度思考链内容累积：后端每次发送增量 delta，在 ref 中同步累积避免 React 批量渲染丢失
  const deepThinkingContentRef = useRef("");
  // 追踪上一次深度思考的 step，用于检测新一轮思考开始
  const lastDeepThinkingStepRef = useRef(0);
  // 追踪当前迭代已发射的 tool_call callId，避免流式阶段提前发射后流式结束重新发射时 epoch 多余递增
  const seenToolCallIdsRef = useRef<Set<string>>(new Set());
  // 追踪最后一次 tool_call 的 iteration，用于忽略同一迭代中 tool_call 之后的残余 content 事件
  const lastToolCallIterationRef = useRef<number | null>(null);
  // 追踪当前压缩节点 ID，compaction_start 创建节点，compaction_done 更新该节点
  const compactionNodeIdRef = useRef<string | null>(null);
  // 追踪子 Agent agentId → nodeId 映射，用于更新对应的 sub_agent 节点
  const subAgentNodeIdsRef = useRef<Map<string, string>>(new Map());
  // 当前 streaming content 节点 ID（用于 addToolNodeFromEvent 关闭流式节点）
  const streamingNodeIdRef = useRef<string | null>(null);
  // 当前 thinking 节点 ID（用于 addToolNodeFromEvent 关闭思考节点）
  const thinkingNodeIdRef = useRef<string | null>(null);
  // 当前迭代轮次（用作 payload.iteration 缺失时的回退值）
  const currentIterationRef = useRef<number | undefined>(undefined);
  // 被 tool_call 关闭的 streaming content 节点 ID（用于后续 content 事件更新该节点）
  const lastClosedStreamingNodeIdRef = useRef<string | null>(null);

  useEffect(() => {
    sessionIdRef.current = sessionId;
    // 同步更新 useWorkflowStore 的当前会话 ID 追踪，供 contextUsage 事件区分当前/后台会话
    setCurrentSessionId(sessionId);
  }, [sessionId]);

  useEffect(() => {
    let cancelled = false;

    const registerListeners = async () => {
      const unlisteners = await Promise.all([
        onAgentThinking((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setLastThinking(payload);
        }),
        onAgentDeepThinking((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "deep_thinking",
              step: payload.step,
              thought: payload.thought,
              isStreaming: payload.isStreaming,
              iteration: payload.iteration,
            });
            return;
          }
          // 同步更新当前迭代轮次，确保后续 onAgentToolCall 能读到最新值
          if (payload.iteration !== undefined) {
            currentIterationRef.current = payload.iteration;
          }
          if (payload.isStreaming) {
            // step 变化表示新一轮思考开始，重置累积内容并递增 epoch
            if (payload.step !== lastDeepThinkingStepRef.current) {
              lastDeepThinkingStepRef.current = payload.step;
              deepThinkingContentRef.current = "";
              contentEpochRef.current += 1;
            }
            // 同步累积增量 delta，避免 React 批量渲染导致中间 chunk 丢失
            deepThinkingContentRef.current += payload.thought;
          }
          // 将累积的完整内容传递给状态，而非仅传递当前 delta
          setDeepThinking({
            ...payload,
            thought: deepThinkingContentRef.current,
          });
        }),
        onAgentContent((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "content",
              content: payload.content,
              isStreaming: payload.isStreaming,
              iteration: payload.iteration,
            });
            return;
          }
          // 忽略同一迭代中 tool_call 之后的残余 content 事件
          // 当 tool_call 事件已到达（前端已关闭 streaming 节点），
          // 但流式阶段还有残余的 content chunk 到达时，这些 chunk 属于已结束的迭代，
          // 不应创建新的 content 节点（否则会出现重复内容节点）
          // 但 is_streaming=false 的最终内容事件需要放行：
          // 后端在流式结束后会发射完整内容（修复 LLM 在 tool_use 块后继续输出文本导致的截断），
          // 前端需要用完整内容更新已有的 content 节点
          if (lastToolCallIterationRef.current !== null
            && payload.iteration !== undefined
            && payload.iteration <= lastToolCallIterationRef.current
            && payload.isStreaming) {
            return;
          }
          if (contentEpochRef.current !== lastContentEpochRef.current) {
            lastContentEpochRef.current = contentEpochRef.current;
            setContent(payload.content);
          } else if (!payload.isStreaming) {
            // 流式结束时收到完整内容（后端可能清理了 XML 标签/特殊 token），
            // 用完整内容替换之前累积的流式内容
            // 即使内容为空也要替换，因为之前的流式内容可能是 XML 标签片段
            setContent(payload.content);
          } else {
            setContent((prev) => prev + payload.content);
          }
        }),
        onAgentToolCall((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "tool_call",
              callId: payload.callId,
              toolName: payload.toolName,
              arguments: payload.arguments,
              iteration: payload.iteration,
            });
            return;
          }
          // 仅在首次见到此 callId 时递增 epoch，避免流式结束后重新发射导致多余递增
          // 多余递增会使下一个迭代的 content 事件被错误地替换而非追加
          if (!seenToolCallIdsRef.current.has(payload.callId)) {
            seenToolCallIdsRef.current.add(payload.callId);
            contentEpochRef.current += 1;
          }
          // 记录最后一次 tool_call 的 iteration，用于忽略残余 content 事件
          if (payload.iteration !== undefined) {
            lastToolCallIterationRef.current = payload.iteration;
          }
          // 直接调用 store 方法创建/更新工具节点，绕过 React state 批处理
          // 避免 React 18 自动批处理导致同一 tick 内多个 tool_call 事件丢失
          const toolEventRefs: ToolEventRefs = {
            thinkingNodeId: thinkingNodeIdRef.current,
            streamingNodeId: streamingNodeIdRef.current,
            currentIteration: currentIterationRef.current,
          };
          const result = useWorkflowStore.getState().addToolNodeFromEvent(payload, toolEventRefs);
          // 如果有关闭的 streaming 节点，记录其 ID 供后续 content 事件更新
          // （修复 LLM 在 tool_use 块后继续输出文本内容导致的截断问题）
          if (result?.closedStreamingNodeId) {
            lastClosedStreamingNodeIdRef.current = result.closedStreamingNodeId;
          }
          // 保留 setCurrentToolCall 供其他 UI 消费者使用（如右侧边栏）
          setCurrentToolCall(payload);
        }),
        onAgentToolResult((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "tool_result",
              callId: payload.callId,
              success: payload.success,
              result: payload.result,
              error: payload.error,
              durationMs: payload.durationMs,
            });
            return;
          }
          // 直接调用 store 方法更新工具节点状态，绕过 React state 批处理
          useWorkflowStore.getState().updateToolResultFromEvent(payload);
          // 保留 setLastToolResult 供其他 UI 消费者使用（如右侧边栏）
          setLastToolResult(payload);
        }),
        onAgentConfirm((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setPendingConfirmation(payload);
        }),
        onAgentNetworkRetry((payload) => {
          if (payload.sessionId !== sessionIdRef.current) return;
          setNetworkRetry(payload);
        }),
        onAgentDone((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "done",
              summary: payload.summary,
              totalSteps: payload.totalSteps,
              durationMs: payload.durationMs,
            });
            return;
          }
          setIsLoading(false);
          setDoneResult(payload);
        }),
        onAgentError((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "error",
              code: payload.code,
              message: payload.message,
              recoverable: payload.recoverable,
            });
            return;
          }
          setIsLoading(false);
          setError(payload.message);
        }),
        onAgentStopped((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "stopped",
              completedSteps: payload.completedSteps,
              reason: payload.reason,
            });
            return;
          }
          setIsLoading(false);
          setIsStopped(true);
        }),
        onAgentCompactionStart((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "compaction_start",
              tokensBefore: payload.tokensBefore,
            });
            return;
          }
          // 当前会话：创建压缩节点，状态为 running
          const nodeId = useWorkflowStore.getState().addNode("compaction", {
            tokensBefore: payload.tokensBefore,
          }, "running");
          compactionNodeIdRef.current = nodeId;
        }),
        onAgentCompactionDone((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "compaction_done",
              tokensBefore: payload.tokensBefore,
              tokensAfter: payload.tokensAfter,
              compacted: payload.compacted,
              ...(payload.error ? { error: payload.error } : {}),
            });
            return;
          }
          // 当前会话：更新压缩节点结果
          const isFailed = !payload.compacted || !!payload.error;
          const existingNodeId = compactionNodeIdRef.current;
          if (existingNodeId) {
            useWorkflowStore.getState().updateNode(existingNodeId, {
              status: isFailed ? "failed" : "completed",
              data: {
                tokensBefore: payload.tokensBefore,
                tokensAfter: payload.tokensAfter,
                compacted: payload.compacted,
                ...(payload.error ? { error: payload.error } : {}),
              },
            });
            compactionNodeIdRef.current = null;
          } else {
            // 未找到压缩开始节点，直接创建一个已完成节点
            useWorkflowStore.getState().addNode("compaction", {
              tokensBefore: payload.tokensBefore,
              tokensAfter: payload.tokensAfter,
              compacted: payload.compacted,
              ...(payload.error ? { error: payload.error } : {}),
            }, isFailed ? "failed" : "completed");
          }
        }),
        onAgentSnapshotCreated((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "snapshot_created",
              messageId: payload.messageId,
              kind: payload.kind,
              createdAt: payload.createdAt,
            });
            return;
          }
          // 当前会话：快照节点插入到对应 user 节点之后（回填后 messageId 已匹配），
          // 未匹配则插入最新 user 节点之后（公共函数内已处理去重）
          const next = applySnapshotNode(
            useWorkflowStore.getState().nodes,
            { messageId: payload.messageId, kind: payload.kind, createdAt: payload.createdAt },
            () =>
              ({
                id: `snapshot_${Date.now()}`,
                type: "snapshot",
                status: "completed",
                timestamp: new Date(payload.createdAt).getTime(),
                data: { kind: payload.kind },
                isExpanded: true,
              }) as WorkflowNode<"snapshot">,
          );
          useWorkflowStore.setState({ nodes: next });
        }),
        onSubAgentStatus((payload) => {
          // 后台会话：路由到缓存
          if (payload.parentSessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.parentSessionId, {
              type: "sub_agent_status",
              agentId: payload.agentId,
              status: payload.status,
              message: payload.message,
              iteration: payload.iteration,
              taskDescription: payload.taskDescription,
            });
            return;
          }
          // 当前会话：创建或更新 sub_agent 节点
          const existingNodeId = subAgentNodeIdsRef.current.get(payload.agentId);
          if (existingNodeId) {
            // 更新已有节点：保留 taskDescription 和 toolCalls，更新状态相关字段
            const existingNode = useWorkflowStore.getState().nodes.find((n) => n.id === existingNodeId);
            const existingData = existingNode?.data as SubAgentNodeData | undefined;
            if (existingData) {
              useWorkflowStore.getState().updateNode(existingNodeId, {
                status: payload.status as NodeStatus,
                data: {
                  ...existingData,
                  status: payload.status,
                  iteration: payload.iteration,
                  message: payload.message,
                },
              });
            }
          } else {
            // 首次事件：创建节点，使用 payload.taskDescription 作为任务描述
            const nodeId = useWorkflowStore.getState().addNode("sub_agent", {
              agentId: payload.agentId,
              taskDescription: payload.taskDescription,
              status: payload.status,
              iteration: payload.iteration,
              toolCalls: [],
              message: payload.message,
            }, payload.status as NodeStatus);
            subAgentNodeIdsRef.current.set(payload.agentId, nodeId);
          }
        }),
        onSubAgentToolCall((payload) => {
          // 后台会话：路由到缓存
          if (payload.parentSessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.parentSessionId, {
              type: "sub_agent_tool_call",
              agentId: payload.agentId,
              toolName: payload.toolName,
              arguments: payload.arguments,
              iteration: payload.iteration,
            });
            return;
          }
          // 当前会话：在对应的 sub_agent 节点的 toolCalls 数组中追加工具调用记录
          const existingNodeId = subAgentNodeIdsRef.current.get(payload.agentId);
          if (existingNodeId) {
            const existingNode = useWorkflowStore.getState().nodes.find((n) => n.id === existingNodeId);
            const existingData = existingNode?.data as SubAgentNodeData | undefined;
            if (existingData) {
              useWorkflowStore.getState().updateNode(existingNodeId, {
                data: {
                  ...existingData,
                  toolCalls: [
                    ...existingData.toolCalls,
                    { toolName: payload.toolName, arguments: payload.arguments },
                  ],
                  iteration: payload.iteration,
                },
              });
            }
          }
          // 若用户正在查看该子 Agent 的详情页，在子 Agent 工作流中添加 tool 节点
          const currentSubAgentId = useWorkflowStore.getState().currentSubAgentId;
          if (currentSubAgentId === payload.agentId) {
            useWorkflowStore.getState().addSubAgentToolNode(
              payload.agentId,
              payload.toolCallId,
              payload.toolName,
              payload.arguments,
              payload.iteration,
            );
          }
        }),
        onSubAgentThinking((payload) => {
          // 仅处理当前会话的子 Agent 事件
          if (payload.parentSessionId !== sessionIdRef.current) return;
          // 仅当用户正在查看该子 Agent 的详情页时更新
          const currentSubAgentId = useWorkflowStore.getState().currentSubAgentId;
          if (currentSubAgentId === payload.agentId) {
            useWorkflowStore.getState().appendSubAgentThinking(
              payload.agentId,
              payload.content,
              payload.isStreaming,
              payload.iteration,
            );
          }
        }),
        onSubAgentContent((payload) => {
          if (payload.parentSessionId !== sessionIdRef.current) return;
          const currentSubAgentId = useWorkflowStore.getState().currentSubAgentId;
          if (currentSubAgentId === payload.agentId) {
            useWorkflowStore.getState().appendSubAgentContent(
              payload.agentId,
              payload.content,
              payload.isStreaming,
              payload.iteration,
            );
          }
        }),
        onSubAgentToolResult((payload) => {
          if (payload.parentSessionId !== sessionIdRef.current) return;
          const currentSubAgentId = useWorkflowStore.getState().currentSubAgentId;
          if (currentSubAgentId === payload.agentId) {
            useWorkflowStore.getState().updateSubAgentToolResult(
              payload.agentId,
              payload.toolCallId,
              payload.result,
              payload.error,
              payload.success,
            );
          }
        }),
        onQuestion((payload) => {
          // 后台会话：路由到缓存
          if (payload.sessionId !== sessionIdRef.current) {
            routeBackgroundEvent(payload.sessionId, {
              type: "question",
              questionId: payload.questionId,
              questions: payload.questions,
            });
            return;
          }
          // 当前会话：创建 question 节点（status="running"）
          useWorkflowStore.getState().addNode("question", {
            questionId: payload.questionId,
            questions: payload.questions,
            answered: false,
          }, "running");
        }),
      ]);

      if (cancelled) {
        unlisteners.forEach((fn) => fn());
        return;
      }

      unlistenRefs.current = unlisteners;
    };

    registerListeners();

    return () => {
      cancelled = true;
      unlistenRefs.current.forEach((unlisten) => unlisten());
      unlistenRefs.current = [];
    };
  }, []);

  const sendMessage = useCallback(
    async (prompt: string, options?: Record<string, unknown>) => {
      setError(null);
      setContent("");
      setLastThinking(null);
      setDeepThinking(null);
      setCurrentToolCall(null);
      setLastToolResult(null);
      setPendingConfirmation(null);
      setDoneResult(null);
      setIsStopped(false);
      setNetworkRetry(null);
      setIsLoading(true);
      contentEpochRef.current += 1;
      lastContentEpochRef.current = contentEpochRef.current;
      deepThinkingContentRef.current = "";
      lastDeepThinkingStepRef.current = 0;
      seenToolCallIdsRef.current.clear();
      lastToolCallIterationRef.current = null;
      compactionNodeIdRef.current = null;
      subAgentNodeIdsRef.current.clear();
      streamingNodeIdRef.current = null;
      thinkingNodeIdRef.current = null;
      currentIterationRef.current = undefined;
      lastClosedStreamingNodeIdRef.current = null;
      // 新消息发送后后端将清理 staged revert，同步清除前端回退状态（横幅消失）
      useWorkflowStore.getState().setRevertInfo(null);

      // 从附件 store 获取当前待发送的附件
      const currentAttachments = useAttachmentStore.getState().attachments;

      try {
        let sid = sessionId;
        if (!sid) {
          // 创建会话时携带当前工作区 ID，使会话可以按工作区分组
          const workspaceId = options?.workspaceId as string | undefined;
          const session = await tauriCmd.createSession({
            title: `${i18n.t('session.newSession')} ${new Date().toLocaleTimeString()}`,
            workspaceId,
          });
          sid = session.id;
          setSessionId(sid);
          sessionIdRef.current = sid;
        }

        // 将附件信息和当前 Agent 模式合并到 options 中
        const agentMode = useAgentModeStore.getState().mode;
        const agentOptions = {
          ...options,
          agentMode,
          ...(currentAttachments.length > 0 ? { attachments: currentAttachments } : {}),
        };

        await tauriCmd.startAgent(sid, prompt, agentOptions);

        // 发送成功后清空附件
        useAttachmentStore.getState().clearAttachments();
      } catch (err) {
        setIsLoading(false);
        // 从 Tauri invoke 错误对象中提取 message，避免 `[object Object]`
        let errorMessage: string;
        if (err instanceof Error) {
          errorMessage = err.message;
        } else if (err && typeof err === "object") {
          errorMessage = (err as Record<string, unknown>)?.message as string ?? JSON.stringify(err);
        } else {
          errorMessage = String(err);
        }
        setError(errorMessage);
      }
    },
    [sessionId],
  );

  const stopAgent = useCallback(async () => {
    if (!sessionId) return;

    try {
      await tauriCmd.stopAgent(sessionId);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [sessionId]);

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

  // 权限审批回复（双态权限系统），与 confirmOperation 并存以保持向后兼容
  const respondPermission = useCallback(
    async (operationId: string, response: 'once' | 'reject', feedback?: string) => {
      if (!sessionId) return;

      try {
        await tauriCmd.permissionRespond(sessionId, operationId, response, feedback);
        setPendingConfirmation(null);
      } catch (err) {
        setError(err instanceof Error ? err.message : String(err));
      }
    },
    [sessionId],
  );

  const reset = useCallback(() => {
    setIsLoading(initialState.isLoading);
    setError(initialState.error);
    setSessionId(initialState.sessionId);
    setLastThinking(initialState.lastThinking);
    setDeepThinking(initialState.deepThinking);
    setContent(initialState.content);
    setCurrentToolCall(initialState.currentToolCall);
    setLastToolResult(initialState.lastToolResult);
    setPendingConfirmation(initialState.pendingConfirmation);
    setDoneResult(initialState.doneResult);
    setIsStopped(initialState.isStopped);
    setNetworkRetry(initialState.networkRetry);
    deepThinkingContentRef.current = "";
    lastDeepThinkingStepRef.current = 0;
    seenToolCallIdsRef.current.clear();
    lastToolCallIterationRef.current = null;
    compactionNodeIdRef.current = null;
    subAgentNodeIdsRef.current.clear();
    streamingNodeIdRef.current = null;
    thinkingNodeIdRef.current = null;
    currentIterationRef.current = undefined;
    lastClosedStreamingNodeIdRef.current = null;
  }, []);

  const setSessionIdExternal = useCallback((id: string) => {
    setSessionId(id);
    sessionIdRef.current = id;
  }, []);

  return {
    isLoading,
    error,
    sessionId,
    lastThinking,
    deepThinking,
    content,
    currentToolCall,
    lastToolResult,
    pendingConfirmation,
    doneResult,
    isStopped,
    networkRetry,
    sendMessage,
    stopAgent,
    confirmOperation,
    respondPermission,
    reset,
    setSessionId: setSessionIdExternal,
    streamingNodeIdRef,
    thinkingNodeIdRef,
    currentIterationRef,
    lastClosedStreamingNodeIdRef,
    lastToolCallIterationRef,
  };
}
