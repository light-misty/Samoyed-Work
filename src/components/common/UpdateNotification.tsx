import { useState, useEffect, useCallback } from "react";
import { check, type Update, type DownloadEvent } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

interface UpdateNotificationProps {
  open: boolean;
  onClose: () => void;
}

// 更新状态机
type UpdateState = "idle" | "checking" | "available" | "downloading" | "installing" | "error" | "success";

export function UpdateNotification({ open, onClose }: UpdateNotificationProps) {
  const [state, setState] = useState<UpdateState>("idle");
  const [updateInfo, setUpdateInfo] = useState<Update | null>(null);
  const [errorMessage, setErrorMessage] = useState("");
  // 下载进度
  const [downloadProgress, setDownloadProgress] = useState(0);
  const [downloadedBytes, setDownloadedBytes] = useState(0);
  const [totalBytes, setTotalBytes] = useState(0);

  // 重置所有状态
  const resetState = useCallback(() => {
    setState("idle");
    setUpdateInfo(null);
    setErrorMessage("");
    setDownloadProgress(0);
    setDownloadedBytes(0);
    setTotalBytes(0);
  }, []);

  // 关闭时重置状态
  useEffect(() => {
    if (!open) {
      resetState();
    }
  }, [open, resetState]);

  // 检查更新
  const handleCheck = useCallback(async () => {
    setState("checking");
    setErrorMessage("");
    try {
      const result = await check();
      if (result) {
        setUpdateInfo(result);
        setState("available");
      } else {
        // 没有可用更新，直接关闭通知
        setState("idle");
        onClose();
      }
    } catch (err) {
      console.error("[UpdateNotification] 检查更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [onClose]);

  // 当组件打开时自动检查
  useEffect(() => {
    if (open && state === "idle") {
      handleCheck();
    }
  }, [open, state, handleCheck]);

  // 下载并安装更新
  const handleDownloadAndInstall = useCallback(async () => {
    if (!updateInfo) return;

    setState("downloading");
    setDownloadProgress(0);
    setDownloadedBytes(0);
    setTotalBytes(0);

    try {
      await updateInfo.downloadAndInstall((event: DownloadEvent) => {
        switch (event.event) {
          case "Started":
            // 下载开始，记录总大小
            if (event.data.contentLength) {
              setTotalBytes(event.data.contentLength);
            }
            break;
          case "Progress": {
            // 下载进度更新
            setDownloadedBytes((prev) => {
              const newDownloaded = prev + event.data.chunkLength;
              return newDownloaded;
            });
            break;
          }
          case "Finished":
            // 下载完成，进入安装阶段
            setState("installing");
            break;
        }
      });

      // 下载并安装完成
      setState("success");
    } catch (err) {
      console.error("[UpdateNotification] 下载/安装更新失败:", err);
      setErrorMessage(err instanceof Error ? err.message : String(err));
      setState("error");
    }
  }, [updateInfo]);

  // 重新尝试
  const handleRetry = useCallback(() => {
    if (state === "error" && updateInfo) {
      // 如果之前已经获取到更新信息，直接重新下载
      handleDownloadAndInstall();
    } else {
      // 否则重新检查
      handleCheck();
    }
  }, [state, updateInfo, handleDownloadAndInstall, handleCheck]);

  // 重启应用
  const handleRelaunch = useCallback(async () => {
    try {
      await relaunch();
    } catch (err) {
      console.error("[UpdateNotification] 重启应用失败:", err);
    }
  }, []);

  // 格式化文件大小
  const formatBytes = (bytes: number): string => {
    if (bytes === 0) return "0 B";
    const units = ["B", "KB", "MB", "GB"];
    const i = Math.floor(Math.log(bytes) / Math.log(1024));
    return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`;
  };

  // 计算下载进度百分比
  useEffect(() => {
    if (totalBytes > 0) {
      setDownloadProgress(Math.round((downloadedBytes / totalBytes) * 100));
    }
  }, [downloadedBytes, totalBytes]);

  if (!open) return null;

  return (
    <div className="update-notification">
      {/* 关闭按钮 */}
      {state !== "downloading" && state !== "installing" && (
        <button className="update-close" onClick={onClose}>
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
          <div className="update-title">正在检查更新...</div>
        </div>
      )}

      {/* 发现新版本 */}
      {state === "available" && updateInfo && (
        <div className="update-body">
          <div className="update-icon update-icon-available">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <path d="M10 2L12.5 7.5L18 8.5L14 12.5L15 18L10 15.5L5 18L6 12.5L2 8.5L7.5 7.5L10 2Z" stroke="currentColor" strokeWidth="1.5" strokeLinejoin="round" />
            </svg>
          </div>
          <div className="update-title">发现新版本 v{updateInfo.version}</div>
          <div className="update-version-info">
            当前版本: v{updateInfo.currentVersion}
          </div>
          {updateInfo.body && (
            <div className="update-changelog">
              {updateInfo.body}
            </div>
          )}
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleDownloadAndInstall}>
              立即更新
            </button>
            <button className="update-btn update-btn-ghost" onClick={onClose}>
              稍后提醒
            </button>
          </div>
        </div>
      )}

      {/* 下载中 */}
      {state === "downloading" && (
        <div className="update-body">
          <div className="update-icon update-icon-loading">
            <svg className="update-spin" width="20" height="20" viewBox="0 0 24 24" fill="none">
              <circle cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="2.5" opacity="0.25" />
              <path d="M4 12a8 8 0 018-8" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
            </svg>
          </div>
          <div className="update-title">正在下载更新...</div>
          <div className="update-progress-bar">
            <div
              className="update-progress-fill"
              style={{ width: `${downloadProgress}%` }}
            />
          </div>
          <div className="update-progress-info">
            <span>{downloadProgress}%</span>
            {totalBytes > 0 && (
              <span>{formatBytes(downloadedBytes)} / {formatBytes(totalBytes)}</span>
            )}
          </div>
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
          <div className="update-title">正在安装更新...</div>
          <div className="update-desc">安装完成后将自动重启应用</div>
        </div>
      )}

      {/* 安装成功 */}
      {state === "success" && (
        <div className="update-body">
          <div className="update-icon update-icon-success">
            <svg width="20" height="20" viewBox="0 0 20 20" fill="none">
              <circle cx="10" cy="10" r="9" stroke="currentColor" strokeWidth="1.5" />
              <path d="M6 10.5L8.5 13L14 7" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </div>
          <div className="update-title">更新安装完成</div>
          <div className="update-desc">需要重启应用以完成更新</div>
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleRelaunch}>
              立即重启
            </button>
            <button className="update-btn update-btn-ghost" onClick={onClose}>
              稍后重启
            </button>
          </div>
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
          <div className="update-title">更新失败</div>
          <div className="update-error-msg">{errorMessage}</div>
          <div className="update-actions">
            <button className="update-btn update-btn-primary" onClick={handleRetry}>
              重试
            </button>
            <button className="update-btn update-btn-ghost" onClick={onClose}>
              关闭
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

        .update-icon-loading {
          color: var(--color-accent);
        }

        .update-icon-available {
          color: var(--color-accent);
        }

        .update-icon-success {
          color: var(--color-success);
        }

        .update-icon-error {
          color: var(--color-error);
        }

        .update-spin {
          animation: spin 1s linear infinite;
        }

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
          from {
            opacity: 0;
            transform: translateY(20px);
          }
          to {
            opacity: 1;
            transform: translateY(0);
          }
        }
      `}</style>
    </div>
  );
}
