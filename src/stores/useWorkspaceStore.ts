import { create } from "zustand";
import type { WorkspaceInfo } from "../types";
import * as tauriCmd from "../services/tauri";

interface WorkspaceState {
  currentWorkspaceId: string | null;
  workspaces: WorkspaceInfo[];
  isLoading: boolean;

  addWorkspace: (path: string, name?: string) => Promise<string>;
  switchWorkspace: (id: string) => Promise<void>;
  removeWorkspace: (id: string) => Promise<void>;
  /** 处理工作区目录被外部删除：从 store 中移除并调用后端清理配置 */
  handleWorkspaceDirectoryDeleted: (workspaceId: string) => Promise<void>;
  loadWorkspaces: () => Promise<void>;
}

export const useWorkspaceStore = create<WorkspaceState>((set, get) => ({
  currentWorkspaceId: null,
  workspaces: [],
  isLoading: false,

  // 添加工作区，调用后端 API
  addWorkspace: async (path, name) => {
    try {
      const workspace = await tauriCmd.addWorkspace(path, name);
      const newCurrentId = get().currentWorkspaceId || workspace.id;
      set((state) => ({
        workspaces: [...state.workspaces, workspace],
        currentWorkspaceId: newCurrentId,
      }));
      // 同步后端活动工作区状态，确保文件监听启动
      if (newCurrentId) {
        await tauriCmd.setActiveWorkspace(newCurrentId).catch((err) => {
          console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
        });
      }
      return workspace.id;
    } catch (error) {
      console.error("[WorkspaceStore] 添加工作区失败:", error);
      throw error;
    }
  },

  // 切换工作区，调用后端 API
  switchWorkspace: async (id) => {
    try {
      await tauriCmd.setActiveWorkspace(id);
      set({ currentWorkspaceId: id });
    } catch (error) {
      console.error("[WorkspaceStore] 切换工作区失败:", error);
    }
  },

  // 移除工作区，调用后端 API
  removeWorkspace: async (id) => {
    try {
      await tauriCmd.removeWorkspace(id);
      let newCurrentId: string | null = null;
      set((state) => {
        // 先过滤得到剩余列表，再从剩余列表中取回退值，避免回退到已删除的工作区
        const remaining = state.workspaces.filter((w) => w.id !== id);
        newCurrentId =
          state.currentWorkspaceId === id
            ? remaining[0]?.id ?? null
            : state.currentWorkspaceId;
        return {
          workspaces: remaining,
          currentWorkspaceId: newCurrentId,
        };
      });
      // 同步后端活动工作区状态，确保文件监听切换
      if (newCurrentId) {
        await tauriCmd.setActiveWorkspace(newCurrentId).catch((err) => {
          console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
        });
      }
    } catch (error) {
      console.error("[WorkspaceStore] 移除工作区失败:", error);
    }
  },

  // 处理工作区目录被外部删除：从 store 中移除并调用后端清理配置
  handleWorkspaceDirectoryDeleted: async (workspaceId) => {
    console.warn("[WorkspaceStore] 工作区目录已被外部删除, id=", workspaceId);
    let newCurrentId: string | null = null;
    set((state) => {
      // 从列表中移除该工作区
      const remaining = state.workspaces.filter((w) => w.id !== workspaceId);
      // 如果被删除的是当前活动工作区，自动切换到第一个可用工作区
      newCurrentId =
        state.currentWorkspaceId === workspaceId
          ? remaining[0]?.id ?? null
          : state.currentWorkspaceId;
      return {
        workspaces: remaining,
        currentWorkspaceId: newCurrentId,
      };
    });

    // 调用后端移除工作区配置（清理 workspaces.json 中的条目）
    try {
      await tauriCmd.removeWorkspace(workspaceId);
    } catch (err) {
      console.warn("[WorkspaceStore] 后端移除工作区配置失败:", err);
    }

    // 同步后端活动工作区状态，确保文件监听切换到新的工作区
    if (newCurrentId) {
      await tauriCmd.setActiveWorkspace(newCurrentId).catch((err) => {
        console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
      });
    }
  },

  // 从后端加载工作区列表
  loadWorkspaces: async () => {
    set({ isLoading: true });
    try {
      const workspaces = await tauriCmd.listWorkspaces();

      // 自动清理目录已不存在的工作区
      const deletedWorkspaces = workspaces.filter((w) => !w.pathExists);
      const validWorkspaces = workspaces.filter((w) => w.pathExists);

      // 对目录已不存在的工作区，调用后端移除配置
      for (const ws of deletedWorkspaces) {
        try {
          await tauriCmd.removeWorkspace(ws.id);
          console.warn("[WorkspaceStore] 已自动清理目录不存在的工作区:", ws.name, ws.path);
        } catch (err) {
          console.warn("[WorkspaceStore] 清理工作区配置失败:", err);
        }
      }

      const activeWorkspace = validWorkspaces.find((w) => w.isActive);
      const currentId = activeWorkspace?.id ?? validWorkspaces[0]?.id ?? null;
      set({
        workspaces: validWorkspaces,
        currentWorkspaceId: currentId,
        isLoading: false,
      });
      // 同步后端活动工作区状态，确保文件监听启动
      if (currentId) {
        await tauriCmd.setActiveWorkspace(currentId).catch((err) => {
          console.warn("[WorkspaceStore] 同步活动工作区失败:", err);
        });
      }
    } catch (error) {
      console.error("[WorkspaceStore] 加载工作区列表失败:", error);
      set({ isLoading: false });
    }
  },
}));
