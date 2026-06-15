import { useState, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { useUpdateStore } from "../../stores/useUpdateStore";
import {
  checkUpdate,
  downloadUpdate,
  installDownloadedUpdate,
  type UpdateInfo,
  type DownloadUpdateEvent,
} from "../../services/tauri";

interface UpdateNotificationProps {
  open: boolean;
  onClose: () => void;
}

// 更新状态机：idle → checking → available → downloading → downloaded → installing
type UpdateState = "idle" | "checking" | "available" | "downloading" | "downloaded" | "installing" | "error";

export function UpdateNotification({ open, onClose }: UpdateNotificationProps) {
  const { t } = useTranslation();
  const [state, setState] = useState<UpdateState>("idle");
  const [updateInfo, setUpdateInfo] = useState<UpdateInfo | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);
  // 已下载的安装包路径
  const [installerPath, setInstallerPath] = useState<string | null>(null);

  const executionStatus = useWorkflowStore((s) => s.executionStatus);
  const isAgentWorking = executionStatus === "running" || executionStatus === "stopping";
  const setPendingUpdatePath = useUpdateStore((s) => s.setPendingUpdatePath);

  // 关闭通知：如果在 downloaded 状态关闭，自动保存待安装更新路径
  const handleClose = useCallback(() => {
    if (state === "downloaded" && installerPath) {
      setPendingUpdatePath(installerPath);
    }
    onClose();
  }, [state, installerPath, setPendingUpdatePath, onClose]);

  // 重置所有状态
  const resetState = useCallback(() => {
    setState("idle");
    setUpdateInfo(null);
    setErrorMessage("");
    setDownloadProgress(0);
    setDownloadedBytes(0);
    setTotalBytes(0);
    setInstallerPath(null);
  }, []);

  useEffect(() => {
    if (!open) resetState();
  }, [open, resetState]);

  // 检查更新
  const handleCheck = useCallback(async () => {
    setState("checking");
    setErrorMessage("");
    try {
      const result = await checkUpdate();
      if (result) {
        setUpdateInfo(result);
        setState("available");
      } else {
        setState("idle");
        handleClose();
      }
    } catch (err) {
      console.error("[UpdateNotification] 检查更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [onClose, handleClose]);

  // 当组件打开时自动检查
  useEffect(() => {
    if (open && state === "idle") handleCheck();
  }, [open, state, handleCheck]);

  // 下载更新（保存到临时文件，不安装）
  const handleDownload = useCallback(async () => {
    if (!updateInfo) return;
    setState("downloading");
    setDownloadProgress(0);
    setDownloadedBytes(0);
    setTotalBytes(0);

    try {
      const result = await downloadUpdate((event: DownloadUpdateEvent) => {
        if (event.event === "progress") {
          setDownloadedBytes(event.data.downloaded);
          if (event.data.contentLength) {
            setTotalBytes(event.data.contentLength);
            setDownloadProgress(Math.round((event.data.downloaded / event.data.contentLength) * 100));
          }
        } else {
          setState("downloaded");
        }
      });
      setInstallerPath(result.installerPath);
    } catch (err) {
      console.error("[UpdateNotification] 下载更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [updateInfo]);

  // 立即更新：安装已下载的更新（带自动重启），然后退出当前进程
  const handleRestartNow = useCallback(async () => {
    if (!installerPath || isAgentWorking) return;
    setState("installing");
    try {
      // restart=true，NSIS 传 /R 参数，安装完成后自动重启
      await installDownloadedUpdate(installerPath, true);
      // install_downloaded_update 内部调用 std::process::exit(0)，正常不会执行到这里
    } catch (err) {
      console.error("[UpdateNotification] 安装更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [installerPath, isAgentWorking]);

  // 稍后更新：将安装包路径保存到全局 store，关闭程序时安装
  const handleRestartLater = useCallback(() => {
    if (installerPath) {
      setPendingUpdatePath(installerPath);
    }
    onClose();
  }, [installerPath, setPendingUpdatePath, onClose]);

  // 重新尝试
  const handleRetry = useCallback(() => {
    if (state === "error" && updateInfo) {
      handleDownload();
    } else {
      handleCheck();
    }
  }, [state, updateInfo, handleDownload, handleCheck]);

  // 格式化文件大小
  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
  };

  if (!open) return null;

  return (
    <div className="update-notification">
      {/* 关闭按钮：下载中和安装中不允许关闭 */}
      {state !== "downloading" && state !== "installing" && (
        <button className="update-close" onClick={handleClose}>
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <path d="M2 2L12 12M12 2L2 12" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
          </svg>
        </button>
      )}

      {/* 检查中 */}
      {state === "checking" && (
        <div className="update-body">
          <div className="update-icon update-icon-loading">
            <svg className="update-spin" width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2.5" opacity="0.25" />
              <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.checking")}</div>
        </div>
      )}

      {/* 发现新版本 */}
      {state === "available" && updateInfo && (
        <div className="update-body">
          <div className="update-title">{t("update.newVersionFound", { version: updateInfo.version })}</div>
          <div className="update-version-info">
            {t("update.currentVersionLabel", { version: updateInfo.currentVersion })}
          </div>
          {updateInfo.body && (
            <div className="update-changelog">{updateInfo.body}</div>
          )}
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleDownload}>
              {t("update.updateNow")}
            </button>
            <button className="update-btn update-btn-ghost" onClick={handleClose}>
              {t("update.later")}
            </button>
          </div>
        </div>
      )}

      {/* 下载中 */}
      {state === "downloading" && (
        <div className="update-body">
          <div className="update-title">{t("update.downloadingUpdate")}</div>
          <div className="update-progress-bar">
            <div className="update-progress-fill" style={{ width: `${downloadProgress}%` }} />
          </div>
          <div className="update-progress-info">
            <span>{downloadProgress}%</span>
            {totalBytes > 0 && (
              <span>{formatBytes(downloadedBytes)} / {formatBytes(totalBytes)}</span>
            )}
          </div>
        </div>
      )}

      {/* 下载完成，等待用户选择更新时机 */}
      {state === "downloaded" && updateInfo && (
        <div className="update-body">
          <div className="update-title">{t("update.installComplete")}</div>
          <div className="update-desc">{t("update.needRestart")}</div>
          {isAgentWorking && (
            <div className="update-agent-warning">
              <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
                <path d="M7 1L13 12H1L7 1Z" stroke="currentColor" strokeWidth="1.2" strokeLinejoin="round" />
                <path d="M7 5.5V8" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" />
                <circle cx="7" cy="10" r="0.5" fill="currentColor" />
              </svg>
              <span>{t("update.agentRunning")}</span>
            </div>
          )}
          <div className="update-actions">
            <button
              className={`update-btn update-btn-primary ${isAgentWorking ? "update-btn-disabled" : ""}`}
              onClick={handleRestartNow}
              disabled={isAgentWorking}
              title={isAgentWorking ? t("update.agentRunningDesc") : undefined}
            >
              {t("update.restartNow")}
            </button>
            <button className="update-btn update-btn-ghost" onClick={handleRestartLater}>
              {t("update.restartLater")}
            </button>
          </div>
          <div className="update-later-hint">{t("update.restartLaterDesc")}</div>
        </div>
      )}

      {/* 安装中 */}
      {state === "installing" && (
        <div className="update-body">
          <div className="update-icon update-icon-loading">
            <svg className="update-spin" width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2.5" opacity="0.25" />
              <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.installingUpdate")}</div>
          <div className="update-desc">{t("update.installCompleteRestart")}</div>
        </div>
      )}

      {/* 错误状态 */}
      {state === "error" && (
        <div className="update-body">
          <div className="update-icon update-icon-error">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="1.5" />
              <path d="M7 7L13 13M13 7L7 13" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">{t("update.updateFailed")}</div>
          <div className="update-error-msg">{errorMessage}</div>
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleRetry}>
              {t("common.retry")}
            </button>
            <button className="update-btn update-btn-ghost" onClick={handleClose}>
              {t("common.close")}
            </button>
          </div>
        </div>
      )}

      <style>{`
        .update-notification {
          position: fixed;
          bottom: 20px;
          right: 20px;
          width: 360px;
          z-index: 9998;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border);
          border-radius: var(--radius-lg);
          box-shadow: var(--shadow-xl);
          animation: updateSlideUp 0.3s ease forwards;
          overflow: hidden;
        }

        .update-body {
          padding: 20px;
          display: flex;
          flex-direction: column;
          gap: 10px;
        }

        .update-close {
          position: absolute;
          top: 12px;
          right: 12px;
          width: 24px;
          height: 24px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: 4px;
          color: var(--color-text-quaternary);
          transition: all 0.15s;
          z-index: 1;
        }

        .update-close:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }

        .update-icon {
          flex-shrink: 0;
          display: flex;
          align-items: center;
          justify-content: center;
        }

        .update-icon-loading { color: var(--color-accent); }
        .update-icon-error { color: var(--color-error); }

        .update-spin { animation: spin 1s linear infinite; }

        .update-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--color-text-primary);
        }

        .update-desc {
          font-size: 12px;
          color: var(--color-text-tertiary);
        }

        .update-version-info {
          font-size: 12px;
          color: var(--color-text-tertiary);
          font-family: var(--font-mono);
        }

        .update-changelog {
          font-size: 12px;
          line-height: 1.6;
          color: var(--color-text-secondary);
          max-height: 120px;
          overflow-y: auto;
          padding: 8px 10px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          border: 1px solid var(--color-border-light);
          white-space: pre-wrap;
          word-break: break-word;
        }

        .update-progress-bar {
          width: 100%;
          height: 6px;
          background: var(--color-bg-sub);
          border-radius: 3px;
          overflow: hidden;
        }

        .update-progress-fill {
          height: 100%;
          background: var(--color-accent);
          border-radius: 3px;
          transition: width 0.2s ease;
        }

        .update-progress-info {
          display: flex;
          justify-content: space-between;
          font-size: 11px;
          color: var(--color-text-tertiary);
          font-family: var(--font-mono);
        }

        .update-error-msg {
          font-size: 12px;
          color: var(--color-error);
          line-height: 1.5;
          word-break: break-word;
        }

        .update-agent-warning {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 8px 10px;
          background: var(--color-warning-bg, rgba(250, 173, 20, 0.1));
          border-radius: var(--radius-sm);
          font-size: 12px;
          color: var(--color-warning, #faad14);
          line-height: 1.4;
        }

        .update-agent-warning svg {
          flex-shrink: 0;
          color: var(--color-warning, #faad14);
        }

        .update-later-hint {
          font-size: 11px;
          color: var(--color-text-quaternary);
          line-height: 1.4;
        }

        .update-actions {
          display: flex;
          gap: 8px;
          margin-top: 4px;
        }

        .update-btn {
          padding: 6px 16px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm);
          transition: all 0.15s;
          cursor: pointer;
        }

        .update-btn-primary {
          background: var(--color-accent);
          color: #fff;
        }

        .update-btn-primary:hover {
          background: var(--color-accent-hover);
        }

        .update-btn-disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }

        .update-btn-disabled:hover {
          background: var(--color-accent);
        }

        .update-btn-ghost {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          border: 1px solid var(--color-border);
        }

        .update-btn-ghost:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
        }

        @keyframes updateSlideUp {
          from { opacity: 0; transform: translateY(20px); }
          to { opacity: 1; transform: translateY(0); }
        }
      `}</style>
    </div>
  );
}
