import { create } from "zustand";
import { onTokenUpdate, type TokenUpdatePayload } from "../services/event";

interface TokenState {
  sessionTokens: number;
  inputTokens: number;
  outputTokens: number;
  dailyTotal: number;
  monthlyTotal: number;
  dailyBudget: number;
  monthlyBudget: number;
  totalCost: number;

  addTokenUsage: (input: number, output: number) => void;
  resetSession: () => void;
  setDailyBudget: (budget: number) => void;
  setMonthlyBudget: (budget: number) => void;
  /** 初始化 token:update 事件监听，应在应用启动时调用 */
  initTokenListener: () => Promise<void>;
  /** 销毁 token:update 事件监听，应在应用卸载时调用 */
  destroyTokenListener: () => void;
}

// 保存事件取消监听函数
let tokenUnlisten: (() => void) | null = null;

export const useTokenStore = create<TokenState>((set) => ({
  sessionTokens: 0,
  inputTokens: 0,
  outputTokens: 0,
  dailyTotal: 0,
  monthlyTotal: 0,
  dailyBudget: 0,
  monthlyBudget: 0,
  totalCost: 0,

  addTokenUsage: (input, output) => {
    set((state) => ({
      inputTokens: state.inputTokens + input,
      outputTokens: state.outputTokens + output,
      sessionTokens: state.sessionTokens + input + output,
      dailyTotal: state.dailyTotal + input + output,
      monthlyTotal: state.monthlyTotal + input + output,
    }));
  },

  resetSession: () => {
    set({ sessionTokens: 0, inputTokens: 0, outputTokens: 0 });
  },

  setDailyBudget: (budget) => set({ dailyBudget: budget }),
  setMonthlyBudget: (budget) => set({ monthlyBudget: budget }),

  // 初始化 token:update 事件监听
  initTokenListener: async () => {
    // 防止重复注册
    if (tokenUnlisten) return;

    tokenUnlisten = await onTokenUpdate((payload: TokenUpdatePayload) => {
      set((state) => ({
        inputTokens: state.inputTokens + payload.promptTokens,
        outputTokens: state.outputTokens + payload.completionTokens,
        sessionTokens: state.sessionTokens + payload.promptTokens + payload.completionTokens,
        dailyTotal: state.dailyTotal + payload.promptTokens + payload.completionTokens,
        monthlyTotal: state.monthlyTotal + payload.promptTokens + payload.completionTokens,
        totalCost: state.totalCost + payload.totalCost,
      }));
    });
  },

  // 销毁 token:update 事件监听
  destroyTokenListener: () => {
    if (tokenUnlisten) {
      tokenUnlisten();
      tokenUnlisten = null;
    }
  },
}));
