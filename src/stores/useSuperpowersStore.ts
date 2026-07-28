import { create } from "zustand";
import { persist } from "zustand/middleware";

interface SuperpowersState {
  enabled: boolean;
  toggle: () => void;
  setEnabled: (enabled: boolean) => void;
}

export const useSuperpowersStore = create<SuperpowersState>()(
  persist(
    (set) => ({
      enabled: false,
      toggle: () => set((state) => ({ enabled: !state.enabled })),
      setEnabled: (enabled: boolean) => set({ enabled }),
    }),
    {
      name: "samoyed-work-superpowers",
    },
  ),
);
