import { create } from "zustand";

interface SlashCommandState {
  /** /help 覆盖层是否打开 */
  helpOverlayOpen: boolean;
  /** 打开 help 覆盖层 */
  openHelpOverlay: () => void;
  /** 关闭 help 覆盖层 */
  closeHelpOverlay: () => void;
  /** 切换 help 覆盖层 */
  toggleHelpOverlay: () => void;
}

export const useSlashCommandStore = create<SlashCommandState>((set) => ({
  helpOverlayOpen: false,
  openHelpOverlay: () => set({ helpOverlayOpen: true }),
  closeHelpOverlay: () => set({ helpOverlayOpen: false }),
  toggleHelpOverlay: () => set((state) => ({ helpOverlayOpen: !state.helpOverlayOpen })),
}));
