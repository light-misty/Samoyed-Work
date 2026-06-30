import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "./Icon";

interface DeleteConfirmDialogProps {
  name: string;
  isDir: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export function DeleteConfirmDialog({ name, isDir, onConfirm, onCancel }: DeleteConfirmDialogProps) {
  const { t } = useTranslation();
  const confirmBtnRef = useRef<HTMLButtonElement>(null);

  /* 打开时自动聚焦确认按钮 */
  useEffect(() => {
    confirmBtnRef.current?.focus();
  }, []);

  return (
    <div className="del-overlay" onClick={(e) => { if (e.target === e.currentTarget) onCancel(); }}>
      <div className="del-dialog">
        <div className="del-header">
          <span className="del-icon">
            <Icon name="warning" size={20} />
          </span>
          <span className="del-title">{t("deleteConfirm.title")}</span>
        </div>
        <div className="del-body">
          <p className="del-message">
            {isDir ? t("deleteConfirm.folderMessage", { name }) : t("deleteConfirm.fileMessage", { name })}
          </p>
          <p className="del-warning">{t("deleteConfirm.permanentWarning")}</p>
        </div>
        <div className="del-footer">
          <button className="del-btn del-btn-danger" ref={confirmBtnRef} onClick={onConfirm}>
            {t("common.delete")}
          </button>
          <button className="del-btn del-btn-cancel" onClick={onCancel}>
            {t("deleteConfirm.cancel")}
          </button>
        </div>
      </div>

      <style>{`
        .del-overlay {
          position: fixed;
          inset: 0;
          z-index: 10001;
          display: flex;
          align-items: center;
          justify-content: center;
          background: var(--color-overlay);
          animation: del-fade-in 0.15s ease-out;
        }
        @keyframes del-fade-in {
          from { opacity: 0; }
          to { opacity: 1; }
        }
        .del-dialog {
          min-width: 340px;
          max-width: 420px;
          background: var(--color-bg-elevated, #fff);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-lg, 12px);
          box-shadow: var(--shadow-lg);
          padding: 20px;
          animation: del-dialog-in 0.2s ease-out;
        }
        @keyframes del-dialog-in {
          from {
            opacity: 0;
            transform: scale(0.95) translateY(-8px);
          }
          to {
            opacity: 1;
            transform: scale(1) translateY(0);
          }
        }
        .del-header {
          display: flex;
          align-items: center;
          gap: 10px;
          margin-bottom: 14px;
        }
        .del-icon {
          display: flex;
          align-items: center;
          justify-content: center;
          width: 32px;
          height: 32px;
          border-radius: 50%;
          background: var(--color-error-bg);
          color: var(--color-error);
          flex-shrink: 0;
        }
        .del-title {
          font-size: 14px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .del-body {
          margin-bottom: 18px;
          padding-left: 42px;
        }
        .del-message {
          font-size: 13px;
          color: var(--color-text-primary);
          margin: 0 0 6px;
          line-height: 1.5;
        }
        .del-message strong {
          color: var(--color-text-primary);
          word-break: break-all;
        }
        .del-warning {
          font-size: 12px;
          color: var(--color-error);
          margin: 0;
          opacity: 0.8;
        }
        .del-footer {
          display: flex;
          justify-content: flex-end;
          gap: 8px;
        }
        .del-btn {
          padding: 6px 14px;
          font-size: 12px;
          font-weight: 500;
          border-radius: var(--radius-sm, 4px);
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .del-btn-cancel {
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .del-btn-cancel:hover {
          background: var(--color-bg-sub);
        }
        .del-btn-danger {
          background: var(--color-error);
          color: #fff;
        }
        .del-btn-danger:hover {
          opacity: 0.9;
        }
      `}</style>
    </div>
  );
}
