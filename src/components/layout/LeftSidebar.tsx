import { useState, useRef, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { AgentInfoSection } from "../sidebar/AgentInfoSection";
import { FileTreeSection } from "../sidebar/FileTreeSection";
import { SessionListSection } from "../sidebar/SessionListSection";
import { Icon } from "../common/Icon";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";

interface LeftSidebarProps {
  /** 文件预览回调 */
  onOpenPreview: (filePath: string, fileName: string) => void;
  /** 版本历史回调 */
  onOpenVersionHistory: (filePath: string, fileName: string) => void;
  /** 切换会话（父组件需同步切换工作区） */
  onSwitchSession: (sessionId: string, workspaceId?: string) => void;
  /** 为指定工作区新建会话 */
  onCreateSession: (workspaceId: string) => void;
  /** 切换工作区并准备展示文件树 */
  onShowFiles: (workspaceId: string) => void;
  /** 删除当前会话后清理工作流 */
  onDeleteCurrentSession: (nextSessionId: string | null) => void;
}

type LeftSidebarView = "sessions" | "files";

/**
 * 左侧栏容器
 * 在「会话列表」与「工作区文件」两种视图之间切换。
 */
export function LeftSidebar({
  onOpenPreview,
  onOpenVersionHistory,
  onSwitchSession,
  onCreateSession,
  onShowFiles,
  onDeleteCurrentSession,
}: LeftSidebarProps) {
  const { t } = useTranslation();
  const [view, setView] = useState<LeftSidebarView>("sessions");
  const { workspaces, currentWorkspaceId } = useWorkspaceStore();
  // 新建会话下拉菜单开关状态
  const [newSessionOpen, setNewSessionOpen] = useState(false);
  const newSessionRef = useRef<HTMLDivElement>(null);

  const currentWorkspace = workspaces.find((w) => w.id === currentWorkspaceId);

  const handleShowFiles = (workspaceId: string) => {
    // 通知父组件切换活动工作区，保证文件树加载正确路径
    onShowFiles(workspaceId);
    setView("files");
  };

  const handleBackToSessions = () => {
    setView("sessions");
  };

  // 点击外部或 Escape 关闭新建会话下拉菜单
  useEffect(() => {
    if (!newSessionOpen) return;
    const handleClickOutside = (e: MouseEvent) => {
      if (newSessionRef.current && !newSessionRef.current.contains(e.target as Node)) {
        setNewSessionOpen(false);
      }
    };
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") setNewSessionOpen(false);
    };
    // 延迟添加监听，避免当前点击事件立即触发关闭
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
      document.addEventListener("keydown", handleKeyDown);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [newSessionOpen]);

  // 选择工作区后创建新会话
  const handlePickWorkspace = (workspaceId: string) => {
    onCreateSession(workspaceId);
    setNewSessionOpen(false);
  };

  return (
    <div className="left-sidebar">
      {view === "files" ? (
        <div className="file-tree-enter">
          <div className="file-tree-header">
            <button
              className="file-tree-back-btn"
              onClick={handleBackToSessions}
              title={t("sessionList.backToSessions")}
              aria-label={t("sessionList.backToSessions")}
            >
              <Icon name="back" size={14} />
              <span>{t("sessionList.backToSessions")}</span>
            </button>
            <span className="file-tree-workspace-name" title={currentWorkspace?.name}>
              {currentWorkspace?.name || ""}
            </span>
          </div>
          <div className="file-tree-wrapper">
            <FileTreeSection
              onOpenPreview={onOpenPreview}
              onOpenVersionHistory={onOpenVersionHistory}
            />
          </div>
        </div>
      ) : (
        <>
          {/* 新建会话按钮 + 工作区选择下拉菜单 */}
          <div ref={newSessionRef} className="new-session-section">
            <button
              type="button"
              className={`new-session-trigger ${newSessionOpen ? "new-session-trigger-active" : ""}`}
              aria-haspopup="listbox"
              aria-expanded={newSessionOpen}
              aria-label={t('topBar.newSession')}
              onClick={() => setNewSessionOpen((prev) => !prev)}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="new-session-icon">
                <line x1="12" y1="5" x2="12" y2="19"/>
                <line x1="5" y1="12" x2="19" y2="12"/>
              </svg>
              <span>{t('topBar.newSession')}</span>
            </button>

            {newSessionOpen && (
              <div className="new-session-dropdown" role="listbox">
                {workspaces.length === 0 ? (
                  <div className="new-session-empty">{t('workspace.noWorkspace')}</div>
                ) : (
                  workspaces.map((ws) => (
                    <div
                      key={ws.id}
                      className="new-session-item"
                      role="option"
                      aria-label={t('sessionList.newSessionForWorkspace', { workspace: ws.name })}
                      onClick={() => handlePickWorkspace(ws.id)}
                    >
                      <Icon name="folder" size={14} className="new-session-item-icon" />
                      <div className="new-session-item-info">
                        <span className="new-session-item-name">{ws.name}</span>
                        <span className="new-session-item-path">{ws.path}</span>
                      </div>
                    </div>
                  ))
                )}
              </div>
            )}
          </div>

          {/* Agent 信息区置于会话列表上方，默认收缩，可点击展开 */}
          <AgentInfoSection />
          <SessionListSection
            onSwitchSession={onSwitchSession}
            onCreateSession={onCreateSession}
            onShowFiles={handleShowFiles}
            onDeleteCurrentSession={onDeleteCurrentSession}
          />
        </>
      )}

      <style>{`
        .left-sidebar {
          display: flex;
          flex-direction: column;
          height: 100%;
          width: 100%;
          overflow: hidden;
        }
        /* 新建会话按钮区: 悬停背景与智能体信息标题栏一致 */
        .new-session-section {
          position: relative;
          flex-shrink: 0;
          margin: 4px 8px 0;
        }
        .new-session-trigger {
          display: flex;
          align-items: center;
          gap: 6px;
          width: 100%;
          padding: 8px 12px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          user-select: none;
          transition: background 0.15s;
          background: transparent;
          border: none;
          font-size: 14px;
          font-weight: 400;
          color: var(--color-text-primary);
        }
        .new-session-icon {
          flex-shrink: 0;
          color: var(--color-text-primary);
        }
        .new-session-trigger:hover,
        .new-session-trigger-active {
          background: var(--color-bg-hover);
        }
        /* 删除全局 button:active 的 scale 缩小动画反馈 */
        .new-session-trigger:active {
          transform: none;
        }
        /* 下拉菜单: 复用 WorkspaceSelector 的视觉语言 */
        .new-session-dropdown {
          position: absolute;
          top: calc(100% + 4px);
          left: 0;
          right: 0;
          min-width: 200px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 200;
          animation: new-session-dropdown-in 0.15s ease-out;
          overflow: hidden;
          padding: 4px;
        }
        @keyframes new-session-dropdown-in {
          from {
            opacity: 0;
            transform: scale(0.96) translateY(-4px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .new-session-empty {
          padding: 14px 12px;
          text-align: center;
          font-size: 13px;
          color: var(--color-text-quaternary);
        }
        .new-session-item {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: background 0.12s;
          min-width: 0;
        }
        .new-session-item:hover {
          background: var(--color-bg-hover);
        }
        .new-session-item-icon {
          color: var(--color-text-tertiary);
          flex-shrink: 0;
        }
        .new-session-item-info {
          display: flex;
          flex-direction: column;
          gap: 1px;
          min-width: 0;
          flex: 1;
        }
        .new-session-item-name {
          font-size: 14px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .new-session-item-path {
          font-size: 12px;
          color: var(--color-text-quaternary);
          font-family: var(--font-mono);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .file-tree-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          flex-shrink: 0;
        }
        .file-tree-back-btn {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 4px 8px;
          border-radius: var(--radius-sm);
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-secondary);
          background: transparent;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .file-tree-back-btn:hover {
          background: var(--color-bg-hover);
        }
        .file-tree-workspace-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
          max-width: 140px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .file-tree-wrapper {
          flex: 1;
          min-height: 0;
          overflow-y: auto;
        }
        .file-tree-enter {
          display: flex;
          flex-direction: column;
          height: 100%;
          width: 100%;
          overflow: hidden;
          animation: file-tree-slide-in 0.28s cubic-bezier(0.4, 0, 0.2, 1);
        }
        @keyframes file-tree-slide-in {
          from {
            opacity: 0;
            transform: translateX(-100%);
          }
          to {
            opacity: 1;
            transform: translateX(0);
          }
        }
      `}</style>
    </div>
  );
}
