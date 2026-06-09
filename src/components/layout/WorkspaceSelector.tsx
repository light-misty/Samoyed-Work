import { useTranslation } from 'react-i18next';
import { useState, useRef, useEffect, useCallback } from "react";
import { Icon } from "../common/Icon";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";
import { AddWorkspaceDialog } from "../settings/AddWorkspaceDialog";

export function WorkspaceSelector() {
  const { t } = useTranslation();
  const { currentWorkspaceId, workspaces, switchWorkspace, removeWorkspace } = useWorkspaceStore();
  const [open, setOpen] = useState(false);
  const [showAddDialog, setShowAddDialog] = useState(false);
  const [removingId, setRemovingId] = useState<string | null>(null);
  const [removeError, setRemoveError] = useState<string | null>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const currentWs = workspaces.find((w) => w.id === currentWorkspaceId);

  /* 点击外部关闭下拉框 */
  const handleClickOutside = useCallback((e: MouseEvent) => {
    if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
      setOpen(false);
      setRemovingId(null);
      setRemoveError(null);
    }
  }, []);

  /* 按 Escape 关闭下拉框 */
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === "Escape") {
      setOpen(false);
      setRemovingId(null);
      setRemoveError(null);
    }
  }, []);

  useEffect(() => {
    if (open) {
      /* 延迟添加监听，避免当前点击事件立即触发关闭 */
      const timer = setTimeout(() => {
        document.addEventListener("mousedown", handleClickOutside);
        document.addEventListener("keydown", handleKeyDown);
      }, 0);
      return () => {
        clearTimeout(timer);
        document.removeEventListener("mousedown", handleClickOutside);
        document.removeEventListener("keydown", handleKeyDown);
      };
    }
  }, [open, handleClickOutside, handleKeyDown]);

  /* 切换工作区 */
  const handleSwitch = async (id: string) => {
    if (id === currentWorkspaceId) return;
    await switchWorkspace(id);
    setOpen(false);
  };

  /* 移除工作区 */
  const handleRemove = async (id: string) => {
    setRemoveError(null);
    try {
      await removeWorkspace(id);
      setRemovingId(null);
    } catch (err) {
      setRemoveError(err instanceof Error ? err.message : String(err));
    }
  };

  /* 添加工作区完成后 */
  const handleAddSaved = () => {
    setShowAddDialog(false);
  };

  return (
    <div ref={containerRef} className="ws-selector-container">
      {/* 触发按钮 */}
      <div
        role="button"
        aria-label={t('workspace.selectWorkspace')}
        tabIndex={0}
        className={`ws-selector-trigger ${open ? "ws-selector-trigger-active" : ""}`}
        onClick={() => { setOpen((prev) => !prev); setRemovingId(null); setRemoveError(null); }}
        onKeyDown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); setOpen((prev) => !prev); } }}
      >
        <span className="w-2 h-2 rounded-full bg-accent" />
        <span className="ws-selector-label">{currentWs?.name ?? t('workspace.selectWorkspace')}</span>
        <Icon name={open ? "chevron-up" : "chevron-down"} size={14} />
      </div>

      {/* 下拉面板 */}
      {open && (
        <div className="ws-selector-dropdown">
          {/* 工作区列表 */}
          <div className="ws-selector-list">
            {workspaces.length === 0 && (
              <div className="ws-selector-empty">{t('workspace.noWorkspace')}</div>
            )}
            {workspaces.map((ws) => (
              <div key={ws.id} className="ws-selector-item-wrapper">
                <div
                  className={`ws-selector-item ${ws.id === currentWorkspaceId ? "ws-selector-item-active" : ""} ${!ws.pathExists ? "ws-selector-item-deleted" : ""}`}
                  onClick={() => ws.pathExists && handleSwitch(ws.id)}
                >
                  <div className="ws-selector-item-left">
                    <span className="ws-selector-item-dot">
                      {ws.id === currentWorkspaceId && <span className="ws-selector-item-dot-inner" />}
                    </span>
                    <div className="ws-selector-item-info">
                      <span className="ws-selector-item-name">{ws.name}{!ws.pathExists ? ` (${t('workspace.directoryDeleted')})` : ""}</span>
                      <span className="ws-selector-item-path">{ws.path}</span>
                    </div>
                  </div>
                  {removingId !== ws.id && (
                    <button
                      className="ws-selector-remove-btn"
                      title={t('workspace.removeWorkspace')}
                      onClick={(e) => { e.stopPropagation(); setRemovingId(ws.id); setRemoveError(null); }}
                    >
                      <Icon name="close" size={12} />
                    </button>
                  )}
                </div>

                {/* 移除确认条 */}
                {removingId === ws.id && (
                  <div className="ws-selector-confirm">
                    <div className="ws-selector-confirm-text">
                      {t('workspace.confirmRemove')}
                    </div>
                    {removeError && (
                      <div className="ws-selector-confirm-error">{removeError}</div>
                    )}
                    <div className="ws-selector-confirm-actions">
                      <button
                        className="ws-selector-confirm-btn ws-selector-confirm-btn-danger"
                        onClick={(e) => { e.stopPropagation(); handleRemove(ws.id); }}
                      >
                        {t('workspace.confirmRemoveBtn')}
                      </button>
                      <button
                        className="ws-selector-confirm-btn ws-selector-confirm-btn-ghost"
                        onClick={(e) => { e.stopPropagation(); setRemovingId(null); setRemoveError(null); }}
                      >
                        {t('workspace.cancel')}
                      </button>
                    </div>
                  </div>
                )}
              </div>
            ))}
          </div>

          {/* 添加工作区按钮 */}
          <div className="ws-selector-footer">
            <button
              className="ws-selector-add-btn"
              onClick={() => setShowAddDialog(true)}
            >
              <Icon name="plus" size={14} />
              <span>{t('workspace.addWorkspace')}</span>
            </button>
          </div>
        </div>
      )}

      {/* 添加工作区弹窗 */}
      {showAddDialog && (
        <AddWorkspaceDialog
          onClose={() => setShowAddDialog(false)}
          onSaved={handleAddSaved}
        />
      )}

      <style>{`
        .ws-selector-container {
          position: relative;
        }
        .ws-selector-trigger {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 5px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: background 0.15s;
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-secondary);
          white-space: nowrap;
          user-select: none;
        }
        .ws-selector-trigger:hover {
          background: var(--color-bg-sub);
        }
        .ws-selector-trigger-active {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .ws-selector-label {
          max-width: 160px;
          overflow: hidden;
          text-overflow: ellipsis;
        }
        .ws-selector-dropdown {
          position: absolute;
          top: calc(100% + 6px);
          left: 0;
          min-width: 280px;
          max-width: 360px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 200;
          animation: ws-dropdown-in 0.15s ease-out;
          overflow: hidden;
        }
        @keyframes ws-dropdown-in {
          from {
            opacity: 0;
            transform: scale(0.96) translateY(-4px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .ws-selector-list {
          max-height: 320px;
          overflow-y: auto;
          padding: 4px;
        }
        .ws-selector-empty {
          padding: 20px 16px;
          text-align: center;
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .ws-selector-item-wrapper {
          /* 每个工作区条目的包裹层 */
        }
        .ws-selector-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          padding: 8px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: background 0.12s;
        }
        .ws-selector-item:hover {
          background: var(--color-bg-hover);
        }
        .ws-selector-item-active {
          background: var(--color-accent-bg);
        }
        .ws-selector-item-active:hover {
          background: var(--color-accent-bg);
        }
        .ws-selector-item-deleted {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .ws-selector-item-deleted .ws-selector-item-name {
          color: var(--color-error);
          text-decoration: line-through;
        }
        .ws-selector-item-left {
          display: flex;
          align-items: center;
          gap: 8px;
          min-width: 0;
          flex: 1;
        }
        .ws-selector-item-dot {
          width: 14px;
          height: 14px;
          border-radius: 50%;
          border: 1.5px solid var(--color-border);
          display: flex;
          align-items: center;
          justify-content: center;
          flex-shrink: 0;
          transition: border-color 0.15s;
        }
        .ws-selector-item-active .ws-selector-item-dot {
          border-color: var(--color-accent);
        }
        .ws-selector-item-dot-inner {
          width: 7px;
          height: 7px;
          border-radius: 50%;
          background: var(--color-accent);
        }
        .ws-selector-item-info {
          display: flex;
          flex-direction: column;
          gap: 1px;
          min-width: 0;
          flex: 1;
        }
        .ws-selector-item-name {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .ws-selector-item-active .ws-selector-item-name {
          color: var(--color-accent);
        }
        .ws-selector-item-path {
          font-size: 11px;
          color: var(--color-text-quaternary);
          font-family: var(--font-mono);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .ws-selector-remove-btn {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 22px;
          height: 22px;
          border-radius: var(--radius-xs);
          color: var(--color-text-quaternary);
          flex-shrink: 0;
          transition: all 0.12s;
          opacity: 0;
        }
        .ws-selector-item:hover .ws-selector-remove-btn {
          opacity: 1;
        }
        .ws-selector-remove-btn:hover {
          background: var(--color-error-bg);
          color: var(--color-error);
        }
        .ws-selector-confirm {
          padding: 8px 10px 10px;
          margin: 0 4px 4px;
          border-top: 1px solid var(--color-border-light);
          animation: ws-confirm-in 0.15s ease-out;
        }
        @keyframes ws-confirm-in {
          from {
            opacity: 0;
            max-height: 0;
          }
          to {
            opacity: 1;
            max-height: 120px;
          }
        }
        .ws-selector-confirm-text {
          font-size: 11px;
          color: var(--color-text-secondary);
          margin-bottom: 6px;
          line-height: 1.4;
        }
        .ws-selector-confirm-error {
          font-size: 11px;
          color: var(--color-error);
          margin-bottom: 6px;
        }
        .ws-selector-confirm-actions {
          display: flex;
          gap: 6px;
        }
        .ws-selector-confirm-btn {
          padding: 3px 10px;
          border-radius: var(--radius-xs);
          font-size: 11px;
          font-weight: 500;
          border: none;
          cursor: pointer;
          transition: all 0.12s;
        }
        .ws-selector-confirm-btn-danger {
          background: var(--color-error);
          color: white;
        }
        .ws-selector-confirm-btn-danger:hover {
          filter: brightness(0.9);
        }
        .ws-selector-confirm-btn-ghost {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }
        .ws-selector-confirm-btn-ghost:hover {
          background: var(--color-bg-hover);
        }
        .ws-selector-footer {
          border-top: 1px solid var(--color-border-light);
          padding: 4px;
        }
        .ws-selector-add-btn {
          display: flex;
          align-items: center;
          gap: 6px;
          width: 100%;
          padding: 7px 10px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-accent);
          transition: background 0.12s;
          border: none;
          cursor: pointer;
          background: none;
        }
        .ws-selector-add-btn:hover {
          background: var(--color-accent-bg);
        }
      `}</style>
    </div>
  );
}
