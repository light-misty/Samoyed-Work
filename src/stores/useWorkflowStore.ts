import { create } from "zustand";
import type { WorkflowNode, WorkflowNodeType, NodeStatus, ExecutionStatus, NodeDataMap } from "../types";
import type { Message } from "../types/session";
import { generateToolBrief } from "../utils/format";

interface WorkflowState {
  nodes: WorkflowNode[];
  executionStatus: ExecutionStatus;
  error: string | null;
  autoScroll: boolean;
  confirmHandler: ((approved: boolean) => Promise<void>) | null;

  addNode: <T extends WorkflowNodeType>(type: T, data: NodeDataMap[T], status?: NodeStatus, iteration?: number) => string;
  updateNode: (id: string, updates: Partial<WorkflowNode>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  toggleNode: (id: string) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  setConfirmHandler: (handler: ((approved: boolean) => Promise<void>) | null) => void;
  loadFromMessages: (messages: Message[]) => void;
}

let nodeCounter = 0;

export const useWorkflowStore = create<WorkflowState>((set) => ({
  nodes: [],
  executionStatus: "idle",
  error: null,
  autoScroll: true,
  confirmHandler: null,

  addNode: (type, data, status = "completed", iteration) => {
    const id = `node_${++nodeCounter}`;
    set((state) => ({
      nodes: [
        ...state.nodes,
        {
          id,
          type,
          status,
          timestamp: Date.now(),
          data: data as NodeDataMap[typeof type],
          isExpanded: true,
          iteration,
        } as WorkflowNode,
      ],
    }));
    return id;
  },

  updateNode: (id, updates) => {
    set((state) => ({
      nodes: state.nodes.map((n) => (n.id === id ? { ...n, ...updates } : n)),
    }));
  },

  removeNode: (id) => {
    set((state) => ({
      nodes: state.nodes.filter((n) => n.id !== id),
    }));
  },

  clearNodes: () => {
    nodeCounter = 0;
    set({ nodes: [], error: null, executionStatus: "idle", confirmHandler: null });
  },

  setExecutionStatus: (status) => {
    set({ executionStatus: status });
  },

  setError: (error) => {
    set({ error });
  },

  toggleNode: (id) => {
    set((state) => ({
      nodes: state.nodes.map((n) =>
        n.id === id ? { ...n, isExpanded: !n.isExpanded } : n
      ),
    }));
  },

  setAutoScroll: (autoScroll) => {
    set({ autoScroll });
  },

  setConfirmHandler: (handler) => {
    set({ confirmHandler: handler });
  },

  loadFromMessages: (messages) => {
    nodeCounter = 0;
    const nodes: WorkflowNode[] = [];
    // 追踪当前迭代轮次：每条 assistant 消息代表一次迭代
    let iterationCounter = 0;

    for (const msg of messages) {
      const msgTimestamp = new Date(msg.createdAt).getTime();

      if (msg.role === "user") {
        nodes.push({
          id: `node_${++nodeCounter}`,
          type: "user",
          status: "completed",
          timestamp: msgTimestamp,
          data: { content: msg.content, attachments: [] },
          isExpanded: true,
        });
      } else if (msg.role === "assistant") {
        // 每条 assistant 消息递增迭代计数
        iterationCounter += 1;
        const currentIteration = iterationCounter;

        if (msg.reasoningContent && msg.reasoningContent.trim()) {
          nodes.push({
            id: `node_${++nodeCounter}`,
            type: "thinking",
            status: "completed",
            timestamp: msgTimestamp,
            data: { content: msg.reasoningContent, duration: 0, isStreaming: false },
            isExpanded: true,
            iteration: currentIteration,
          });
        }
        // LLM 响应中 content 在 tool_calls 之前输出，因此 content 节点应排在 tool 节点之前
        if (msg.content && msg.content.trim()) {
          nodes.push({
            id: `node_${++nodeCounter}`,
            type: "content",
            status: "completed",
            timestamp: msgTimestamp,
            data: { content: msg.content },
            isExpanded: true,
            iteration: currentIteration,
          });
        }
        if (msg.toolCalls && msg.toolCalls.length > 0) {
          for (const tc of msg.toolCalls) {
            nodes.push({
              id: `node_${++nodeCounter}`,
              type: "tool",
              status: "completed",
              timestamp: msgTimestamp,
              data: {
                toolName: tc.name,
                briefDescription: generateToolBrief(tc.name, (tc.arguments ?? {}) as Record<string, unknown>),
                input: (tc.arguments ?? {}) as Record<string, unknown>,
                success: true,
              },
              isExpanded: true,
              iteration: currentIteration,
            });
          }
        }
      }
    }

    set({ nodes, error: null, executionStatus: "idle", confirmHandler: null });
  },
}));
