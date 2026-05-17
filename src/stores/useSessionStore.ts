import { create } from "zustand";
import type { SessionSummary } from "../types";
import * as tauriCmd from "../services/tauri";

interface SessionState {
  currentSessionId: string | null;
  sessions: SessionSummary[];
  isLoading: boolean;

  createSession: (title?: string, workspaceId?: string) => Promise<string>;
  switchSession: (sessionId: string) => void;
  deleteSession: (sessionId: string) => Promise<void>;
  updateSessionTitle: (sessionId: string, title: string) => Promise<void>;
  loadSessions: (workspaceId?: string) => Promise<void>;
}

export const useSessionStore = create<SessionState>((set) => ({
  currentSessionId: null,
  sessions: [],
  isLoading: false,

  // 创建新会话，调用后端 API
  createSession: async (title, workspaceId) => {
    try {
      const session = await tauriCmd.createSession({
        title: title || `新会话 ${new Date().toLocaleTimeString()}`,
        workspaceId,
      });
      set((state) => ({
        sessions: [
          {
            id: session.id,
            title: session.title,
            status: session.status,
            messageCount: 0,
            createdAt: session.createdAt,
            updatedAt: session.updatedAt,
          },
          ...state.sessions,
        ],
        currentSessionId: session.id,
      }));
      return session.id;
    } catch (error) {
      console.error("[SessionStore] 创建会话失败:", error);
      throw error;
    }
  },

  // 切换当前会话
  switchSession: (sessionId) => {
    set({ currentSessionId: sessionId });
  },

  // 删除会话，调用后端 API
  deleteSession: async (sessionId) => {
    try {
      await tauriCmd.deleteSession(sessionId);
      set((state) => ({
        sessions: state.sessions.filter((s) => s.id !== sessionId),
        currentSessionId:
          state.currentSessionId === sessionId
            ? state.sessions[0]?.id ?? null
            : state.currentSessionId,
      }));
    } catch (error) {
      console.error("[SessionStore] 删除会话失败:", error);
      throw error;
    }
  },

  // 更新会话标题，调用后端 API
  updateSessionTitle: async (sessionId, title) => {
    try {
      await tauriCmd.updateSessionTitle(sessionId, title);
      set((state) => ({
        sessions: state.sessions.map((s) =>
          s.id === sessionId ? { ...s, title } : s
        ),
      }));
    } catch (error) {
      console.error("[SessionStore] 更新会话标题失败:", error);
      throw error;
    }
  },

  // 从后端加载会话列表
  loadSessions: async (workspaceId) => {
    set({ isLoading: true });
    try {
      const sessions = await tauriCmd.listSessions(
        workspaceId ? { workspaceId } : undefined
      );
      set({ sessions, isLoading: false });
    } catch (error) {
      console.error("[SessionStore] 加载会话列表失败:", error);
      set({ isLoading: false });
    }
  },
}));
