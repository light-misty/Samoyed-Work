import { useEffect, useState, useCallback } from "react";
import { Icon, type IconName } from "../common/Icon";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import * as tauriCmd from "../../services/tauri";
import type { VersionInfo } from "../../types";

interface VersionHistoryPanelProps {
  open: boolean;
  onClose: () => void;
  workspaceId: string;
  filePath: string;
  fileName: string;
  /** 版本对比回调，传入旧版本和新版本的内容 */
  onCompareVersions?: (oldContent: string, newContent: string, fileType: string) => void;
  /** 版本回滚完成回调 */
  onRollbackComplete?: () => void;
}

/* 操作类型的中文映射和图标 */
const OPERATION_MAP: Record<string, { label: string; icon: IconName; color: string }> = {
  create: { label: "创建", icon: "file-plus", color: "var(--color-success)" },
  modify: { label: "修改", icon: "edit", color: "var(--color-accent)" },
  convert: { label: "转换", icon: "refresh", color: "var(--color-warning)" },
  rollback: { label: "回滚", icon: "undo", color: "var(--color-error)" },
};

/* 格式化时间戳为可读字符串 */
function formatTimestamp(isoStr: string): string {
  try {
    const date = new Date(isoStr);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffMins = Math.floor(diffMs / 60000);
    const diffHours = Math.floor(diffMs / 3600000);
    const diffDays = Math.floor(diffMs / 86400000);

    if (diffMins < 1) return "刚刚";
    if (diffMins < 60) return `${diffMins} 分钟前`;
    if (diffHours < 24) return `${diffHours} 小时前`;
    if (diffDays < 7) return `${diffDays} 天前`;

    const year = date.getFullYear();
    const month = String(date.getMonth() + 1).padStart(2, "0");
    const day = String(date.getDate()).padStart(2, "0");
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");
    return `${year}-${month}-${day} ${hours}:${minutes}`;
  } catch {
    return isoStr;
  }
}

/* 格式化文件大小 */
function formatSize(bytes: number): string {
  if (bytes === 0) return "-";
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

export function VersionHistoryPanel({
  open,
  onClose,
  workspaceId,
  filePath,
  fileName,
  onCompareVersions,
  onRollbackComplete,
}: VersionHistoryPanelProps) {
  const [versions, setVersions] = useState<VersionInfo[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  /* 选中用于对比的版本（最多两个） */
  const [selectedForCompare, setSelectedForCompare] = useState<Set<string>>(new Set());
  /* 正在加载内容的版本 ID */
  const [loadingVersionId, setLoadingVersionId] = useState<string | null>(null);
  /* 回滚确认状态 */
  const [rollbackTarget, setRollbackTarget] = useState<VersionInfo | null>(null);

  /* 加载版本历史 */
  const loadVersions = useCallback(async () => {
    if (!workspaceId || !filePath) return;
    setIsLoading(true);
    setError(null);
    setSelectedForCompare(new Set());
    try {
      const result = await tauriCmd.getDocumentVersions(workspaceId, filePath);
      setVersions(result);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, [workspaceId, filePath]);

  /* 打开时自动加载 */
  useEffect(() => {
    if (open) {
      loadVersions();
    } else {
      setVersions([]);
      setError(null);
      setSelectedForCompare(new Set());
      setRollbackTarget(null);
    }
  }, [open, loadVersions]);

  /* ESC 关闭 */
  useEffect(() => {
    if (!open) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [open, onClose]);

  /* 切换对比选中状态 */
  const toggleCompareSelect = useCallback((versionId: string) => {
    setSelectedForCompare((prev) => {
      const next = new Set(prev);
      if (next.has(versionId)) {
        next.delete(versionId);
      } else if (next.size < 2) {
        next.add(versionId);
      }
      return next;
    });
  }, []);

  /* 执行版本对比 */
  const handleCompare = useCallback(async () => {
    if (selectedForCompare.size !== 2 || !onCompareVersions) return;
    const [firstId, secondId] = Array.from(selectedForCompare);
    setLoadingVersionId("compare");
    try {
      /* 并行获取两个版本的内容 */
      const [content1, content2] = await Promise.all([
        tauriCmd.getVersionContent(workspaceId, filePath, firstId),
        tauriCmd.getVersionContent(workspaceId, filePath, secondId),
      ]);
      /* 按时间排序：旧版本在前，新版本在后 */
      const v1 = versions.find((v) => v.versionId === firstId);
      const v2 = versions.find((v) => v.versionId === secondId);
      const v1Time = v1 ? new Date(v1.timestamp).getTime() : 0;
      const v2Time = v2 ? new Date(v2.timestamp).getTime() : 0;
      if (v1Time <= v2Time) {
        onCompareVersions(content1.content, content2.content, content1.fileType);
      } else {
        onCompareVersions(content2.content, content1.content, content1.fileType);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "获取版本内容失败");
    } finally {
      setLoadingVersionId(null);
    }
  }, [selectedForCompare, onCompareVersions, workspaceId, filePath, versions]);

  /* 与当前版本对比 */
  const handleCompareWithCurrent = useCallback(async (version: VersionInfo) => {
    if (!onCompareVersions) return;
    setLoadingVersionId(version.versionId);
    try {
      /* 获取历史版本内容 */
      const versionContent = await tauriCmd.getVersionContent(workspaceId, filePath, version.versionId);
      /* 获取当前文件内容 */
      const currentContent = await tauriCmd.previewDocument(workspaceId, filePath);
      onCompareVersions(versionContent.content, currentContent.content, versionContent.fileType);
    } catch (err) {
      setError(err instanceof Error ? err.message : "获取版本内容失败");
    } finally {
      setLoadingVersionId(null);
    }
  }, [onCompareVersions, workspaceId, filePath]);

  /* 执行回滚 */
  const handleRollbackConfirm = useCallback(async () => {
    if (!rollbackTarget) return;
    try {
      await tauriCmd.rollbackVersion(workspaceId, filePath, rollbackTarget.versionId);
      setRollbackTarget(null);
      onRollbackComplete?.();
      /* 回滚后重新加载版本列表 */
      await loadVersions();
    } catch (err) {
      setError(err instanceof Error ? err.message : "回滚失败");
    }
  }, [rollbackTarget, workspaceId, filePath, onRollbackComplete, loadVersions]);

  if (!open) return null;

  return (
    <div
      className="vh-overlay"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div className="vh-dialog">
        {/* 顶部栏 */}
        <div className="vh-header">
          <div className="vh-header-left">
            <button className="vh-close-btn" onClick={onClose}>
              <Icon name="close" size={16} />
            </button>
            <div className="vh-title-group">
              <span className="vh-title">版本历史</span>
              <span className="vh-filename">{fileName}</span>
            </div>
          </div>
          <div className="vh-header-actions">
            {selectedForCompare.size === 2 && (
              <button
                className="vh-action-btn vh-compare-btn"
                onClick={handleCompare}
                disabled={loadingVersionId !== null}
              >
                <Icon name="git-compare" size={14} />
                对比选中版本
              </button>
            )}
            <button className="vh-refresh-btn" onClick={loadVersions} disabled={isLoading}>
              <Icon name="refresh" size={14} />
            </button>
          </div>
        </div>

        {/* 内容区 */}
        <div className="vh-body">
          {isLoading ? (
            <div className="vh-loading">
              <svg className="vh-spinner" viewBox="0 0 24 24" fill="none">
                <circle className="vh-spinner-track" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
                <path className="vh-spinner-head" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
              </svg>
              <span>加载版本历史...</span>
            </div>
          ) : error ? (
            <div className="vh-error">
              <Icon name="warning" size={20} />
              <span>{error}</span>
              <button className="vh-retry-btn" onClick={loadVersions}>重试</button>
            </div>
          ) : versions.length === 0 ? (
            <div className="vh-empty">
              <Icon name="file" size={24} />
              <span>暂无版本历史</span>
              <span className="vh-empty-hint">文档修改后将自动创建版本快照</span>
            </div>
          ) : (
            <div className="vh-list">
              {versions.map((version, idx) => {
                const op = OPERATION_MAP[version.operation] || { label: version.operation, icon: "file", color: "var(--color-text-tertiary)" };
                const isSelected = selectedForCompare.has(version.versionId);
                const isLoadingThis = loadingVersionId === version.versionId;
                const isLatest = idx === 0;

                return (
                  <div
                    key={version.versionId}
                    className={`vh-item ${isSelected ? "vh-item-selected" : ""} ${isLatest ? "vh-item-latest" : ""}`}
                  >
                    {/* 选择框 */}
                    <button
                      className={`vh-checkbox ${isSelected ? "vh-checkbox-checked" : ""}`}
                      onClick={() => toggleCompareSelect(version.versionId)}
                      title="选择用于对比"
                    >
                      {isSelected && <Icon name="check" size={10} />}
                    </button>

                    {/* 版本信息 */}
                    <div className="vh-item-content">
                      <div className="vh-item-main">
                        <span className="vh-op-badge" style={{ color: op.color, borderColor: op.color }}>
                          <Icon name={op.icon} size={11} />
                          {op.label}
                        </span>
                        {isLatest && <span className="vh-latest-badge">当前</span>}
                        <span className="vh-time">{formatTimestamp(version.timestamp)}</span>
                      </div>
                      <div className="vh-item-meta">
                        {version.description && <span className="vh-desc">{version.description}</span>}
                        {version.size > 0 && <span className="vh-size">{formatSize(version.size)}</span>}
                      </div>
                    </div>

                    {/* 操作按钮 */}
                    <div className="vh-item-actions">
                      {!isLatest && onCompareVersions && (
                        <button
                          className="vh-item-action"
                          onClick={() => handleCompareWithCurrent(version)}
                          disabled={isLoadingThis}
                          title="与当前版本对比"
                        >
                          {isLoadingThis ? (
                            <svg className="vh-mini-spinner" viewBox="0 0 24 24" fill="none">
                              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" opacity="0.25" />
                              <path fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" opacity="0.75" />
                            </svg>
                          ) : (
                            <Icon name="git-compare" size={13} />
                          )}
                        </button>
                      )}
                      {!isLatest && (
                        <button
                          className="vh-item-action vh-rollback-action"
                          onClick={() => setRollbackTarget(version)}
                          title="回滚到此版本"
                        >
                          <Icon name="undo" size={13} />
                        </button>
                      )}
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>

        {/* 底部提示 */}
        {versions.length > 0 && (
          <div className="vh-footer">
            <span className="vh-footer-hint">
              勾选两个版本后可进行差异对比，共 {versions.length} 个版本
            </span>
          </div>
        )}
      </div>

      {/* 回滚确认对话框 */}
      {rollbackTarget && (
        <DeleteConfirmDialog
          name={`版本 ${formatTimestamp(rollbackTarget.timestamp)}`}
          isDir={false}
          onConfirm={handleRollbackConfirm}
          onCancel={() => setRollbackTarget(null)}
        />
      )}

      {/* 全局加载遮罩（对比加载中） */}
      {loadingVersionId === "compare" && (
        <div className="vh-loading-overlay">
          <div className="vh-loading-card">
            <svg className="vh-spinner" viewBox="0 0 24 24" fill="none">
              <circle className="vh-spinner-track" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="3" />
              <path className="vh-spinner-head" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
            </svg>
            <span>正在加载版本内容...</span>
          </div>
        </div>
      )}

      <style>{`
        .vh-overlay {
          position: fixed;
          inset: 0;
          z-index: 200;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--color-overlay);
          animation: vh-fade-in 0.15s ease-out;
        }
        @keyframes vh-fade-in {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        .vh-dialog {
          width: 560px;
          max-width: 90vw;
          max-height: 80vh;
          background: var(--color-bg-elevated, #fff);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-lg, 12px);
          box-shadow: var(--shadow-lg);
          display: flex;
          flex-direction: column;
          overflow: hidden;
          animation: vh-dialog-in 0.2s ease-out;
        }
        @keyframes vh-dialog-in {
          from { opacity: 0; transform: scale(0.95) translateY(-8px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }

        /* 顶部栏 */
        .vh-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 12px 16px;
          border-bottom: 1px solid var(--color-border-light);
          flex-shrink: 0;
        }
        .vh-header-left {
          display: flex;
          align-items: center;
          gap: 10px;
        }
        .vh-close-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          border: none;
          background: none;
          color: var(--color-text-secondary);
          cursor: pointer;
          transition: all 0.15s;
        }
        .vh-close-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .vh-title-group {
          display: flex;
          align-items: baseline;
          gap: 8px;
        }
        .vh-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .vh-filename {
          font-size: 12px;
          color: var(--color-text-tertiary);
          max-width: 200px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .vh-header-actions {
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .vh-action-btn {
          display: flex;
          align-items: center;
          gap: 5px;
          padding: 5px 10px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm);
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .vh-compare-btn {
          background: var(--color-accent);
          color: #fff;
        }
        .vh-compare-btn:hover { opacity: 0.9; }
        .vh-compare-btn:disabled { opacity: 0.5; cursor: not-allowed; }
        .vh-refresh-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          border: none;
          background: none;
          color: var(--color-text-tertiary);
          cursor: pointer;
          transition: all 0.15s;
        }
        .vh-refresh-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }
        .vh-refresh-btn:disabled { opacity: 0.4; cursor: not-allowed; }

        /* 内容区 */
        .vh-body {
          flex: 1;
          overflow-y: auto;
          padding: 12px;
        }

        /* 加载状态 */
        .vh-loading {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 10px;
          padding: 40px 16px;
          color: var(--color-text-tertiary);
          font-size: 13px;
        }
        .vh-spinner {
          width: 24px;
          height: 24px;
          animation: vh-spin 0.8s linear infinite;
        }
        .vh-spinner-track { opacity: 0.25; }
        .vh-spinner-head { opacity: 0.75; }
        @keyframes vh-spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }

        /* 错误状态 */
        .vh-error {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 8px;
          padding: 32px 16px;
          color: var(--color-error);
          font-size: 13px;
          text-align: center;
        }
        .vh-retry-btn {
          padding: 4px 12px;
          font-size: 12px;
          border-radius: var(--radius-sm);
          border: 1px solid var(--color-error);
          background: none;
          color: var(--color-error);
          cursor: pointer;
          transition: all 0.15s;
        }
        .vh-retry-btn:hover { background: var(--color-error-bg); }

        /* 空状态 */
        .vh-empty {
          display: flex;
          flex-direction: column;
          align-items: center;
          justify-content: center;
          gap: 8px;
          padding: 40px 16px;
          color: var(--color-text-quaternary);
          font-size: 13px;
        }
        .vh-empty-hint {
          font-size: 11px;
          color: var(--color-text-quaternary);
        }

        /* 版本列表 */
        .vh-list {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .vh-item {
          display: flex;
          align-items: center;
          gap: 10px;
          padding: 10px 12px;
          border-radius: var(--radius-md);
          border: 1px solid transparent;
          transition: all 0.15s;
          cursor: default;
        }
        .vh-item:hover {
          background: var(--color-bg-hover);
        }
        .vh-item-selected {
          background: var(--color-accent-bg);
          border-color: var(--color-accent-light);
        }
        .vh-item-latest {
          border-left: 3px solid var(--color-accent);
        }

        /* 选择框 */
        .vh-checkbox {
          width: 18px;
          height: 18px;
          border-radius: var(--radius-xs);
          border: 1.5px solid var(--color-border);
          background: var(--color-bg);
          display: flex;
          align-items: center;
          justify-content: center;
          cursor: pointer;
          transition: all 0.15s;
          flex-shrink: 0;
          padding: 0;
        }
        .vh-checkbox:hover {
          border-color: var(--color-accent);
        }
        .vh-checkbox-checked {
          background: var(--color-accent);
          border-color: var(--color-accent);
          color: #fff;
        }

        /* 版本信息 */
        .vh-item-content {
          flex: 1;
          min-width: 0;
        }
        .vh-item-main {
          display: flex;
          align-items: center;
          gap: 8px;
          margin-bottom: 3px;
        }
        .vh-op-badge {
          display: inline-flex;
          align-items: center;
          gap: 3px;
          font-size: 11px;
          font-weight: 600;
          padding: 1px 6px;
          border-radius: var(--radius-xs);
          border: 1px solid;
          line-height: 1.5;
        }
        .vh-latest-badge {
          font-size: 10px;
          font-weight: 600;
          padding: 0 5px;
          border-radius: var(--radius-xs);
          background: var(--color-accent-bg);
          color: var(--color-accent);
          line-height: 1.6;
        }
        .vh-time {
          font-size: 12px;
          color: var(--color-text-secondary);
        }
        .vh-item-meta {
          display: flex;
          align-items: center;
          gap: 10px;
          font-size: 11px;
          color: var(--color-text-tertiary);
        }
        .vh-desc {
          max-width: 250px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .vh-size {
          flex-shrink: 0;
        }

        /* 操作按钮 */
        .vh-item-actions {
          display: flex;
          align-items: center;
          gap: 4px;
          opacity: 0;
          transition: opacity 0.15s;
        }
        .vh-item:hover .vh-item-actions {
          opacity: 1;
        }
        .vh-item-action {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          border: none;
          background: none;
          color: var(--color-text-tertiary);
          cursor: pointer;
          transition: all 0.15s;
          padding: 0;
        }
        .vh-item-action:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .vh-rollback-action:hover {
          color: var(--color-error);
          background: var(--color-error-bg);
        }
        .vh-mini-spinner {
          width: 14px;
          height: 14px;
          animation: vh-spin 0.8s linear infinite;
        }

        /* 底部 */
        .vh-footer {
          padding: 8px 16px;
          border-top: 1px solid var(--color-border-light);
          flex-shrink: 0;
        }
        .vh-footer-hint {
          font-size: 11px;
          color: var(--color-text-quaternary);
        }

        /* 全局加载遮罩 */
        .vh-loading-overlay {
          position: fixed;
          inset: 0;
          z-index: 210;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--color-overlay);
        }
        .vh-loading-card {
          display: flex;
          align-items: center;
          gap: 10px;
          padding: 14px 20px;
          background: var(--color-bg-elevated, #fff);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          font-size: 13px;
          color: var(--color-text-secondary);
        }
      `}</style>
    </div>
  );
}
