import { useState } from "react";
import { useTranslation } from "react-i18next";
import { open } from "@tauri-apps/plugin-dialog";
import { useWorkspaceStore } from "../../stores/useWorkspaceStore";

interface AddWorkspaceDialogProps {
  onClose: () => void;
  onSaved: () => void;
}

export function AddWorkspaceDialog({ onClose, onSaved }: AddWorkspaceDialogProps) {
  const { t } = useTranslation();
  const { addWorkspace } = useWorkspaceStore();
  const [path, setPath] = useState("");
  const [name, setName] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const handleBrowse = async () => {
    try {
      const selected = await open({
        directory: true,
        multiple: false,
        title: t('settings.addWorkspace.selectDirectory'),
      });
      if (selected) {
        setPath(selected);
      }
    } catch (err) {
      console.error(t('settings.addWorkspace.openDialogFailed'), err);
    }
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!path.trim()) {
      setError(t('settings.addWorkspace.pathRequired'));
      return;
    }
    setSaving(true);
    setError(null);
    try {
      await addWorkspace(path.trim(), name.trim() || undefined);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('settings.addWorkspace.addFailed'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="dialog-overlay" onClick={onClose}>
      <div className="dialog-content" onClick={(e) => e.stopPropagation()}>
        <div className="dialog-header">
          <h3>{t('settings.addWorkspace.title')}</h3>
          <button className="dialog-close" onClick={onClose}>x</button>
        </div>
        <form onSubmit={handleSubmit}>
          <div className="form-group">
            <label>{t('settings.addWorkspace.pathRequiredLabel')}</label>
            <div className="input-group">
              <input
                type="text"
                value={path}
                onChange={(e) => setPath(e.target.value)}
                placeholder={t('settings.addWorkspace.clickBrowsePlaceholder')}
                className="form-input"
              />
              <button type="button" className="browse-btn" onClick={handleBrowse}>
                {t('settings.addWorkspace.browse')}
              </button>
            </div>
            <span className="form-hint">{t('settings.addWorkspace.pathHint')}</span>
          </div>
          <div className="form-group">
            <label>{t('settings.addWorkspace.nameOptionalLabel')}</label>
            <input
              type="text"
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('settings.addWorkspace.nameOptionalPlaceholder')}
              className="form-input"
            />
          </div>
          {error && <div className="form-error">{error}</div>}
          <div className="dialog-actions">
            <button type="submit" className="btn-primary" disabled={saving}>
              {saving ? t('settings.addWorkspace.adding') : t('settings.addWorkspace.add')}
            </button>
            <button type="button" className="btn-ghost" onClick={onClose}>{t('settings.addWorkspace.cancel')}</button>
          </div>
        </form>
      </div>

      <style>{`
        .dialog-overlay {
          position: fixed;
          inset: 0;
          background: rgba(0,0,0,0.4);
          display: flex;
          align-items: center;
          justify-content: center;
          z-index: 1000;
        }
        .dialog-content {
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border);
          border-radius: var(--radius-lg);
          padding: 24px;
          width: 480px;
          max-width: 90vw;
          box-shadow: 0 8px 32px rgba(0,0,0,0.15);
        }
        .dialog-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          margin-bottom: 20px;
        }
        .dialog-header h3 {
          font-size: 16px;
          font-weight: 600;
          color: var(--color-text-primary);
          margin: 0;
        }
        .dialog-close {
          background: none;
          border: none;
          color: var(--color-text-tertiary);
          cursor: pointer;
          font-size: 16px;
          padding: 4px;
        }
        .dialog-close:hover {
          color: var(--color-text-primary);
        }
        .form-group {
          margin-bottom: 16px;
        }
        .form-group label {
          display: block;
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-secondary);
          margin-bottom: 6px;
        }
        .input-group {
          display: flex;
          gap: 8px;
        }
        .form-input {
          flex: 1;
          padding: 8px 12px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-bg);
          color: var(--color-text-primary);
          outline: none;
          transition: border-color 0.15s;
        }
        .form-input:focus {
          border-color: var(--color-accent);
        }
        .browse-btn {
          padding: 8px 16px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          cursor: pointer;
          white-space: nowrap;
        }
        .browse-btn:hover {
          background: var(--color-bg-hover);
        }
        .form-hint {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 4px;
          display: block;
        }
        .form-error {
          font-size: 12px;
          color: var(--color-error);
          margin-bottom: 12px;
        }
        .dialog-actions {
          display: flex;
          justify-content: flex-end;
          gap: 8px;
          margin-top: 20px;
        }
        .btn-ghost {
          padding: 8px 16px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          cursor: pointer;
        }
        .btn-ghost:hover {
          background: var(--color-bg-hover);
        }
        .btn-primary {
          padding: 8px 16px;
          border: none;
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-accent);
          color: white;
          cursor: pointer;
          font-weight: 500;
        }
        .btn-primary:hover {
          background: var(--color-accent-hover);
        }
        .btn-primary:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}
