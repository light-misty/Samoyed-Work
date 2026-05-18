import { create } from "zustand";
import type { WorkflowNode, WorkflowNodeType, NodeStatus, ExecutionStatus, NodeDataMap } from "../types";

interface WorkflowState {
  nodes: WorkflowNode[];
  executionStatus: ExecutionStatus;
  error: string | null;
  autoScroll: boolean;
  confirmHandler: ((approved: boolean) => Promise<void>) | null;

  addNode: <T extends WorkflowNodeType>(type: T, data: NodeDataMap[T], status?: NodeStatus) => string;
  updateNode: (id: string, updates: Partial<WorkflowNode>) => void;
  removeNode: (id: string) => void;
  clearNodes: () => void;
  setExecutionStatus: (status: ExecutionStatus) => void;
  setError: (error: string | null) => void;
  toggleNode: (id: string) => void;
  setAutoScroll: (autoScroll: boolean) => void;
  setConfirmHandler: (handler: ((approved: boolean) => Promise<void>) | null) => void;
}

let nodeCounter = 0;

export const useWorkflowStore = create<WorkflowState>((set) => ({
  nodes: [],
  executionStatus: "idle",
  error: null,
  autoScroll: true,
  confirmHandler: null,

  addNode: (type, data, status = "completed") => {
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
}));
