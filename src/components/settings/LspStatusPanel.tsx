import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "../common/Button";
import { useToastStore } from "../../stores/useToastStore";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { lspGetStatus, lspRestartServer, lspStopAll, lspInitialize } from "../../services/tauri";
import type { LspServerInfo, LspServerStatus } from "../../types";

/** 将 LSP 状态映射为翻译键后缀 */
function statusKey(status: LspServerStatus): string {
  switch (status) {
    case "ready":
      return "statusRunning";
    case "terminated":
      return "statusStopped";
    case "starting":
      return "statusStarting";
    case "error":
      return "statusError";
    case "stopped":
    default:
      return "statusStopped";
  }
}

/** 状态对应的 CSS 颜色类名 */
function statusColorClass(status: LspServerStatus): string {
  switch (status) {
    case "ready":
      return "lsp-status-running";
    case "starting":
      return "lsp-status-starting";
    case "error":
      return "lsp-status-error";
    case "stopped":
    case "terminated":
    default:
      return "lsp-status-stopped";
  }
}

/** 格式化时间戳为可读时间 */
function formatTime(ts: number): string {
  if (!ts) return "-";
  return new Date(ts).toLocaleString();
}

export function LspStatusPanel() {
  const { t } = useTranslation();
  const addToast = useToastStore((s) => s.addToast);
  const { settings, updateSettings } = useSettingsStore();
  const lspEnabled = settings.lsp?.enabled ?? false;

  const [showConfirmDialog, setShowConfirmDialog] = useState(false);
  const [servers, setServers] = useState<LspServerInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [restartingLangs, setRestartingLangs] = useState<Set<string>>(new Set());
  const [stoppingAll, setStoppingAll] = useState(false);

  /** 加载 LSP 服务器状态 */
  const loadStatus = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await lspGetStatus();
      setServers(list);
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      addToast("error", `${t("settings.lsp.loadError")}: ${msg}`);
    } finally {
      setLoading(false);
    }
  }, [addToast, t]);

  useEffect(() => {
    if (lspEnabled) {
      loadStatus();
    }
  }, [lspEnabled, loadStatus]);

  /** 处理总开关切换 */
  const handleToggle = useCallback(() => {
    if (lspEnabled) {
      // 关闭时停止所有服务器并持久化状态
      updateSettings({ lsp: { enabled: false } });
      lspStopAll().catch((err) => {
        console.error("[LspStatusPanel] 停止 LSP 服务器失败:", err);
      });
    } else {
      setShowConfirmDialog(true);
    }
  }, [lspEnabled, updateSettings]);

  /** 确认开启 LSP */
  const handleConfirmEnable = useCallback(() => {
    updateSettings({ lsp: { enabled: true } });
    setShowConfirmDialog(false);
    lspInitialize()
      .catch((err) => addToast("error", `初始化 LSP 失败: ${err instanceof Error ? err.message : String(err)}`))
      .then(() => loadStatus());
  }, [updateSettings, addToast, loadStatus]);

  /** 取消开启 */
  const handleCancelEnable = useCallback(() => {
    setShowConfirmDialog(false);
  }, []);

  /** 重启单个服务器 */
  const handleRestart = useCallback(async (language: string) => {
    setRestartingLangs((prev) => {
      const next = new Set(prev);
      next.add(language);
      return next;
    });
    try {
      await lspRestartServer(language);
      addToast("success", t("settings.lsp.restartSuccess"));
      await loadStatus();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast("error", `${t("settings.lsp.restartError")}: ${msg}`);
    } finally {
      setRestartingLangs((prev) => {
        const next = new Set(prev);
        next.delete(language);
        return next;
      });
    }
  }, [addToast, loadStatus, t]);

  /** 停止所有服务器 */
  const handleStopAll = useCallback(async () => {
    setStoppingAll(true);
    try {
      await lspStopAll();
      addToast("success", t("settings.lsp.stopAllSuccess"));
      await loadStatus();
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      addToast("error", `${t("settings.lsp.stopAllError")}: ${msg}`);
    } finally {
      setStoppingAll(false);
    }
  }, [addToast, loadStatus, t]);

  return (
    <div>
      {/* 总开关行 */}
      <div className="setting-row">
        <div className="setting-info">
          <div className="setting-label">{t("settings.lsp.enableLsp")}</div>
          <div className="setting-desc">{t("settings.lsp.enableLspDesc")}</div>
        </div>
        <label className="lsp-toggle">
          <input
            type="checkbox"
            checked={lspEnabled}
            onChange={handleToggle}
          />
          <span className="lsp-toggle-slider" />
        </label>
      </div>

      {lspEnabled && (
        <>
          {/* 标题区 */}
          <div className="section-header lsp-section-header">
            <span className="section-title">{t("settings.lsp.title")}</span>
            <span className="section-badge lsp-experimental-badge">{t("settings.lsp.experimental")}</span>
          </div>
          <div className="lsp-description">{t("settings.lsp.description")}</div>

          {/* 操作按钮区 */}
          <div className="lsp-actions">
            <Button
              variant="ghost"
              size="sm"
              onClick={loadStatus}
              disabled={loading}
              className="lsp-action-btn"
            >
              <span>{t("settings.lsp.refresh")}</span>
            </Button>
            <Button
              variant="ghost"
              size="sm"
              onClick={handleStopAll}
              disabled={stoppingAll || loading || servers.length === 0}
              className="lsp-action-btn"
            >
              <span>{t("settings.lsp.stopAll")}</span>
            </Button>
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="lsp-error-banner">{t("settings.lsp.loadError")}: {error}</div>
          )}

          {/* 服务器列表 */}
          {servers.length === 0 ? (
            <div className="lsp-empty">{t("settings.lsp.noServers")}</div>
          ) : (
            <div className="lsp-table">
              <div className="lsp-table-header">
                <div className="lsp-col-language">{t("settings.lsp.language")}</div>
                <div className="lsp-col-status">{t("settings.lsp.status")}</div>
                <div className="lsp-col-server">{t("settings.lsp.serverName")}</div>
                <div className="lsp-col-workspace">{t("settings.lsp.workspaceRoot")}</div>
                <div className="lsp-col-started">{t("settings.lsp.startedAt")}</div>
                <div className="lsp-col-action"></div>
              </div>
              {servers.map((srv) => {
                const restarting = restartingLangs.has(srv.language);
                return (
                  <div key={srv.language} className="lsp-table-row">
                    <div className="lsp-col-language lsp-language-cell">{srv.language}</div>
                    <div className="lsp-col-status">
                      <span className={`lsp-status-badge ${statusColorClass(srv.status)}`}>
                        {t(`settings.lsp.${statusKey(srv.status)}`)}
                      </span>
                    </div>
                    <div className="lsp-col-server lsp-mono-text">
                      {srv.serverName ? `${srv.serverName}${srv.serverVersion ? `@${srv.serverVersion}` : ""}` : "-"}
                    </div>
                    <div className="lsp-col-workspace lsp-mono-text" title={srv.workspaceRoot}>
                      {srv.workspaceRoot}
                    </div>
                    <div className="lsp-col-started lsp-mono-text">{formatTime(srv.startedAt)}</div>
                    <div className="lsp-col-action">
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => handleRestart(srv.language)}
                        disabled={restarting || loading}
                        className="lsp-action-btn"
                      >
                        <span>{t("settings.lsp.restart")}</span>
                      </Button>
                    </div>
                    {srv.status === "error" && srv.error && (
                      <div className="lsp-error-detail lsp-mono-text">{srv.error}</div>
                    )}
                  </div>
                );
              })}
            </div>
          )}
        </>
      )}

      {/* 实验性功能确认弹窗 */}
      {showConfirmDialog && (
        <div className="lsp-confirm-overlay" onClick={handleCancelEnable}>
          <div className="lsp-confirm-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="lsp-confirm-header">
              <h3>{t("settings.lsp.confirmTitle")}</h3>
              <button className="lsp-confirm-close" onClick={handleCancelEnable}>x</button>
            </div>
            <div className="lsp-confirm-body">
              <p>{t("settings.lsp.confirmContent")}</p>
            </div>
            <div className="lsp-confirm-actions">
              <button className="lsp-confirm-btn lsp-confirm-btn-primary" onClick={handleConfirmEnable}>
                {t("settings.lsp.confirmEnable")}
              </button>
              <button className="lsp-confirm-btn lsp-confirm-btn-ghost" onClick={handleCancelEnable}>
                {t("settings.lsp.confirmCancel")}
              </button>
            </div>
          </div>
        </div>
      )}

      <style>{`
        .lsp-toggle {
          position: relative;
          display: inline-block;
          width: 36px;
          height: 20px;
          flex-shrink: 0;
          cursor: pointer;
        }
        .lsp-toggle input {
          opacity: 0;
          width: 0;
          height: 0;
          position: absolute;
        }
        .lsp-toggle-slider {
          position: absolute;
          inset: 0;
          background: var(--color-border-strong);
          border-radius: 10px;
          transition: all 0.2s;
        }
        .lsp-toggle-slider::before {
          content: '';
          position: absolute;
          width: 16px;
          height: 16px;
          left: 2px;
          top: 2px;
          background: var(--color-bg-elevated);
          border-radius: 50%;
          transition: all 0.2s;
        }
        .lsp-toggle input:checked + .lsp-toggle-slider {
          background: var(--color-accent);
        }
        .lsp-toggle input:checked + .lsp-toggle-slider::before {
          transform: translateX(16px);
        }

        .lsp-section-header {
          margin-top: 24px;
        }
        .lsp-experimental-badge {
          background: var(--color-purple-light);
          color: var(--color-purple);
        }
        .lsp-description {
          font-size: 12px;
          color: var(--color-text-quaternary);
          margin-bottom: 16px;
        }
        .lsp-actions {
          display: flex;
          gap: 8px;
          margin-bottom: 16px;
        }
        .lsp-action-btn {
          display: inline-flex;
          align-items: center;
          gap: 4px;
        }
        .lsp-error-banner {
          padding: 8px 12px;
          margin-bottom: 12px;
          border-radius: var(--radius-sm);
          background: var(--color-error-bg);
          color: var(--color-error);
          font-size: 12px;
        }
        .lsp-empty {
          padding: 32px 12px;
          text-align: center;
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .lsp-table {
          display: flex;
          flex-direction: column;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-sm);
          overflow: hidden;
        }
        .lsp-table-header {
          display: grid;
          grid-template-columns: 120px 100px 160px 1fr 160px 120px;
          gap: 8px;
          padding: 8px 12px;
          background: var(--color-bg-sub);
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
          text-transform: uppercase;
          letter-spacing: 0.3px;
        }
        .lsp-table-row {
          display: grid;
          grid-template-columns: 120px 100px 160px 1fr 160px 120px;
          gap: 8px;
          padding: 10px 12px;
          border-top: 1px solid var(--color-border-light);
          font-size: 12px;
          color: var(--color-text-primary);
          position: relative;
          transition: background 0.15s;
        }
        .lsp-table-row:hover {
          background: var(--color-accent-bg);
        }
        .lsp-language-cell {
          font-weight: 500;
        }
        .lsp-mono-text {
          font-family: monospace;
          font-size: 11px;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .lsp-status-badge {
          display: inline-block;
          padding: 2px 8px;
          border-radius: 10px;
          font-size: 10px;
          font-weight: 500;
        }
        .lsp-status-running {
          background: var(--color-success-bg);
          color: var(--color-success);
        }
        .lsp-status-starting {
          background: var(--color-warning-bg);
          color: var(--color-warning);
        }
        .lsp-status-error {
          background: var(--color-error-bg);
          color: var(--color-error);
        }
        .lsp-status-stopped {
          background: var(--color-bg-hover);
          color: var(--color-text-quaternary);
        }
        .lsp-error-detail {
          grid-column: 1 / -1;
          margin-top: 6px;
          padding: 6px 8px;
          border-radius: var(--radius-sm);
          background: var(--color-error-bg);
          color: var(--color-error);
          font-size: 11px;
          word-break: break-all;
        }

        /* 确认弹窗样式 */
        .lsp-confirm-overlay {
          position: fixed;
          inset: 0;
          background: rgba(0,0,0,0.4);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1000;
        }
        .lsp-confirm-dialog {
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border);
          border-radius: var(--radius-lg);
          padding: 24px;
          width: 420px;
          max-width: 90vw;
          box-shadow: 0 8px 32px rgba(0,0,0,0.15);
        }
        .lsp-confirm-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 16px;
        }
        .lsp-confirm-header h3 {
          font-size: 16px;
          font-weight: 600;
          color: var(--color-text-primary);
          margin: 0;
        }
        .lsp-confirm-close {
          background: none;
          border: none;
          color: var(--color-text-tertiary);
          cursor: pointer;
          font-size: 16px;
          padding: 4px;
        }
        .lsp-confirm-close:hover {
          color: var(--color-text-primary);
        }
        .lsp-confirm-body {
          margin-bottom: 20px;
        }
        .lsp-confirm-body p {
          font-size: 14px;
          line-height: 1.6;
          color: var(--color-text-primary);
          margin: 0;
        }
        .lsp-confirm-actions {
          display: flex;
          justify-content: flex-end;
          gap: 8px;
        }
        .lsp-confirm-btn {
          padding: 8px 16px;
          border-radius: var(--radius-sm);
          font-size: 13px;
          cursor: pointer;
          font-weight: 500;
          border: none;
        }
        .lsp-confirm-btn-primary {
          background: var(--color-accent);
          color: white;
        }
        .lsp-confirm-btn-primary:hover {
          background: var(--color-accent-hover);
        }
        .lsp-confirm-btn-ghost {
          border: 1px solid var(--color-border);
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }
        .lsp-confirm-btn-ghost:hover {
          background: var(--color-bg-hover);
        }
      `}</style>
    </div>
  );
}
