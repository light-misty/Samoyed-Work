import { create } from "zustand";
import type { FileNode } from "../types";
import * as tauriCmd from "../services/tauri";
import { onFileChange, type FileChangePayload } from "../services/event";
import type { UnlistenFn } from "@tauri-apps/api/event";

/** 在树中按路径查找节点（用于展开时判断是否需要懒加载） */
function findNodeInTree(nodes: FileNode[], path: string): FileNode | null {
  for (const node of nodes) {
    if (node.path === path) return node;
    if (node.children) {
      const found = findNodeInTree(node.children, path);
      if (found) return found;
    }
  }
  return null;
}

/** 更新树中指定路径节点的子节点数据（懒加载完成后回填） */
function updateNodeChildren(nodes: FileNode[], path: string, children: FileNode[]): FileNode[] {
  return nodes.map((node) => {
    if (node.path === path) return { ...node, children };
    if (node.children) {
      return { ...node, children: updateNodeChildren(node.children, path, children) };
    }
    return node;
  });
}

interface FileTreeState {
  treeData: FileNode[];
  expandedKeys: Set<string>;
  selectedKey: string | null;
  searchKeyword: string;
  isLoading: boolean;
  /** 当前活动工作区 ID，用于文件变更事件时刷新 */
  activeWorkspaceId: string | null;
  /** 文件变更事件的取消监听函数 */
  unlistenFn: UnlistenFn | null;
  /** 防抖定时器 ID */
  debounceTimer: ReturnType<typeof setTimeout> | null;

  toggleNode: (key: string) => void;
  selectNode: (key: string) => void;
  setSearchKeyword: (keyword: string) => void;
  loadTree: (workspaceId: string) => Promise<void>;
  /** 懒加载指定目录的子节点（后端仅返回一层，深度截断后展开时按需补齐） */
  loadChildren: (path: string) => Promise<void>;
  /** 递归恢复已展开目录的子节点数据（文件变更刷新后保持展开状态） */
  restoreExpanded: (nodes: FileNode[]) => Promise<void>;
  /** 清空文件树数据（用于工作区被删除后） */
  clearTree: () => void;
  /** 初始化文件变更事件监听 */
  initFileChangeListener: () => Promise<void>;
  /** 销毁文件变更事件监听 */
  destroyFileChangeListener: () => void;
}

export const useFileTreeStore = create<FileTreeState>((set, get) => ({
  treeData: [],
  expandedKeys: new Set(),
  selectedKey: null,
  searchKeyword: "",
  isLoading: false,
  activeWorkspaceId: null,
  unlistenFn: null,
  debounceTimer: null,

  // 展开/折叠节点
  toggleNode: (key) => {
    const state = get();
    const node = findNodeInTree(state.treeData, key);
    const isExpanded = state.expandedKeys.has(key);

    set((s) => {
      const next = new Set(s.expandedKeys);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return { expandedKeys: next };
    });

    // 展开且目录子节点数据缺失（深度截断）时按需懒加载
    if (!isExpanded && node?.isDir && !node.children) {
      void get().loadChildren(key);
    }
  },

  // 懒加载指定目录的子节点（深度 1 只返回该目录的直接子项）
  loadChildren: async (path) => {
    const workspaceId = get().activeWorkspaceId;
    if (!workspaceId) return;
    try {
      const children = await tauriCmd.getFileTree(workspaceId, path, 1);
      set((state) => ({
        treeData: updateNodeChildren(state.treeData, path, children),
      }));
    } catch (error) {
      console.error("[FileTreeStore] 懒加载子节点失败:", path, error);
    }
  },

  // 选中节点
  selectNode: (key) => {
    set({ selectedKey: key });
  },

  // 设置搜索关键词
  setSearchKeyword: (keyword) => {
    set({ searchKeyword: keyword });
  },

  // 从后端加载文件树（默认仅第一层，展开时按需懒加载）
  loadTree: async (workspaceId) => {
    set({ isLoading: true, activeWorkspaceId: workspaceId });
    try {
      const treeData = await tauriCmd.getFileTree(workspaceId);
      set({ treeData, isLoading: false });
      // 刷新后恢复已展开目录的子节点，保持用户展开状态
      void get().restoreExpanded(treeData);
    } catch (error) {
      console.error("[FileTreeStore] 加载文件树失败:", error);
      set({ isLoading: false });
    }
  },

  // 递归恢复已展开目录的子节点数据（深度截断后刷新时保持展开状态）
  restoreExpanded: async (nodes) => {
    const expandedKeys = get().expandedKeys;
    for (const node of nodes) {
      if (!node.isDir || !expandedKeys.has(node.path)) continue;
      if (node.children == null) {
        // 子节点数据缺失（未加载），按需加载该层
        await get().loadChildren(node.path);
      }
      const updated = findNodeInTree(get().treeData, node.path);
      if (updated?.children) {
        await get().restoreExpanded(updated.children);
      }
    }
  },

  // 清空文件树数据（用于工作区被删除后）
  clearTree: () => {
    set({
      treeData: [],
      expandedKeys: new Set(),
      selectedKey: null,
      searchKeyword: "",
      activeWorkspaceId: null,
    });
  },

  // 初始化文件变更事件监听
  initFileChangeListener: async () => {
    // 先销毁旧监听
    get().destroyFileChangeListener();

    try {
      const unlisten = await onFileChange((payload: FileChangePayload) => {
        const { activeWorkspaceId, debounceTimer } = get();
        // 只处理当前活动工作区的文件变更
        if (activeWorkspaceId && payload.workspaceId === activeWorkspaceId) {
          // 防抖：2000ms 内的多次变更合并为一次刷新
          if (debounceTimer) {
            clearTimeout(debounceTimer);
          }
          const timer = setTimeout(() => {
            get().loadTree(activeWorkspaceId);
            set({ debounceTimer: null });
          }, 2000);
          set({ debounceTimer: timer });
        }
      });
      set({ unlistenFn: unlisten });
    } catch (error) {
      console.error("[FileTreeStore] 初始化文件变更监听失败:", error);
    }
  },

  // 销毁文件变更事件监听
  destroyFileChangeListener: () => {
    const { unlistenFn, debounceTimer } = get();
    if (unlistenFn) {
      unlistenFn();
      set({ unlistenFn: null });
    }
    if (debounceTimer) {
      clearTimeout(debounceTimer);
      set({ debounceTimer: null });
    }
  },
}));
