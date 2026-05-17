import { create } from "zustand";
import type { FileNode } from "../types";
import * as tauriCmd from "../services/tauri";

interface FileTreeState {
  treeData: FileNode[];
  expandedKeys: Set<string>;
  selectedKey: string | null;
  searchKeyword: string;
  isLoading: boolean;

  toggleNode: (key: string) => void;
  selectNode: (key: string) => void;
  setSearchKeyword: (keyword: string) => void;
  loadTree: (workspaceId: string) => Promise<void>;
}

export const useFileTreeStore = create<FileTreeState>((set) => ({
  treeData: [],
  expandedKeys: new Set(),
  selectedKey: null,
  searchKeyword: "",
  isLoading: false,

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
    set({ isLoading: true });
    try {
      const treeData = await tauriCmd.getFileTree(workspaceId);
      set({ treeData, isLoading: false });
    } catch (error) {
      console.error("[FileTreeStore] 加载文件树失败:", error);
      set({ isLoading: false });
    }
  },
}));
