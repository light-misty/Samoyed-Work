import { create } from "zustand";
import type { FileNode } from "../types";
import * as tauriCmd from "../services/tauri";
import { onFileChange, type FileChangePayload } from "../services/event";
import type { UnlistenFn } from "@tauri-apps/api/event";

/** 递归过滤文件树，仅保留名称匹配关键词的节点（及其父目录） */
function filterTree(nodes: FileNode[], keyword: string): FileNode[] {
  if (!keyword) return nodes;
  const lower = keyword.toLowerCase();
  return nodes.reduce<FileNode[]>((acc, node) => {
    if (node.isDir) {
      const filteredChildren = filterTree(node.children || [], keyword);
      if (filteredChildren.length > 0) {
        acc.push({ ...node, children: filteredChildren });
      }
    } else if (node.name.toLowerCase().includes(lower)) {
      acc.push(node);
    }
    return acc;
  }, []);
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
  /** 清空文件树数据（用于工作区被删除后） */
  clearTree: () => void;
  /** 获取经过搜索过滤后的文件树 */
  getFilteredTree: () => FileNode[];
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
    set((state) => {
      const next = new Set(state.expandedKeys);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return { expandedKeys: next };
    });
  },

  // 选中节点
  selectNode: (key) => {
    set({ selectedKey: key });
  },

  // 设置搜索关键词
  setSearchKeyword: (keyword) => {
    set({ searchKeyword: keyword });
  },

  // 从后端加载文件树
  loadTree: async (workspaceId) => {
    set({ isLoading: true, activeWorkspaceId: workspaceId });
    try {
      const treeData = await tauriCmd.getFileTree(workspaceId);
      set({ treeData, isLoading: false });
    } catch (error) {
      console.error("[FileTreeStore] 加载文件树失败:", error);
      set({ isLoading: false });
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

  // 获取经过搜索过滤后的文件树
  getFilteredTree: () => {
    const { treeData, searchKeyword } = get();
    return filterTree(treeData, searchKeyword);
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
