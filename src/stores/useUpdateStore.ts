import { create } from 'zustand'

/** 更新 store 的状态接口 */
interface UpdateState {
  /** 待安装的更新安装包路径（下载后保存的临时文件路径） */
  pendingUpdatePath: string | null
  /** 设置待安装的更新路径 */
  setPendingUpdatePath: (path: string | null) => void
  /** 清除待安装的更新路径 */
  clearPendingUpdatePath: () => void
}

/** 应用更新全局状态 store，管理"稍后重启"场景下的待安装更新文件路径 */
export const useUpdateStore = create<UpdateState>((set) => ({
  pendingUpdatePath: null,

  // 设置待安装的更新路径
  setPendingUpdatePath: (path: string | null) => {
    set({ pendingUpdatePath: path })
  },

  // 清除待安装的更新路径
  clearPendingUpdatePath: () => {
    set({ pendingUpdatePath: null })
  },
}))
