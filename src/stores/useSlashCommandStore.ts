import { create } from "zustand";

interface SlashCommandState {
  /** /help 覆盖层是否打开 */
  helpOverlayOpen: boolean;
  /** 打开 help 覆盖层 */
  openHelpOverlay: () => void;
  /** 关闭 help 覆盖层 */
  closeHelpOverlay: () => void;
  /** /stats 覆盖层是否打开 */
  statsOverlayOpen: boolean;
  /** 打开 stats 覆盖层 */
  openStatsOverlay: () => void;
  /** 关闭 stats 覆盖层 */
  closeStatsOverlay: () => void;
}

export const useSlashCommandStore = create<SlashCommandState>((set) => ({
  helpOverlayOpen: false,
  openHelpOverlay: () => set({ helpOverlayOpen: true }),
  closeHelpOverlay: () => set({ helpOverlayOpen: false }),
  statsOverlayOpen: false,
  openStatsOverlay: () => set({ statsOverlayOpen: true }),
  closeStatsOverlay: () => set({ statsOverlayOpen: false }),
}));
