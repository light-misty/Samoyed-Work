import { useState } from "react";
import { useTranslation } from 'react-i18next';
import { SidebarSection } from "../layout/Sidebar";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Icon } from "../common/Icon";

export function AgentInfoSection() {
  const { t } = useTranslation();
  const { settings, llmProviders, activeProviderId, updateSettings, openSettings } = useSettingsStore();

  // 确认级别选项（移入组件内部以使用 t() 翻译）
  const confirmationLevelOptions: { value: string; label: string }[] = [
    { value: "always", label: t('agentInfo.confirmAlways') },
    { value: "editOnly", label: t('agentInfo.confirmEditOnly') },
    { value: "never", label: t('agentInfo.confirmNever') },
  ];
  const activeProvider = llmProviders.find((p) => p.id === activeProviderId);

  const [editing, setEditing] = useState(false);
  const [editValue, setEditValue] = useState(settings.general.authorName);

  const handleSave = () => {
    if (editValue.trim()) {
      updateSettings({ general: { authorName: editValue.trim() } });
    }
    setEditing(false);
  };

  return (
    <SidebarSection title={t('agentInfo.sectionTitle')}>
      <div className="ai-grid" role="region" aria-label={t('agentInfo.sectionTitle')}>
        {/* 当前模型 */}
        <div className="ai-field">
          <span className="ai-field-label">{t('agentInfo.currentModel')}</span>
          <div className={`ai-model-badge ${activeProvider ? "online" : "offline"}`} aria-label={activeProvider ? t('agentInfo.modelConnected') : t('agentInfo.modelDisconnected')}>
            <span className="ai-status-dot" />
            <span className="ai-model-name">
              {activeProvider?.model ?? t('agentInfo.notConfigured')}
            </span>
          </div>
        </div>

        {/* 未配置 Provider 时的引导提示 */}
        {!activeProvider && (
          <button className="ai-setup-hint" onClick={() => openSettings("llm")}>
            <Icon name="settings" size={12} />
            <span>{t('agentInfo.configureLLM')}</span>
          </button>
        )}

        {/* 作者名 */}
        <div className="ai-field">
          <span className="ai-field-label">{t('agentInfo.authorName')}</span>
          {editing ? (
            <input
              className="ai-field-edit"
              aria-label={t('agentInfo.authorName')}
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              onBlur={handleSave}
              onKeyDown={(e) => { if (e.key === "Enter") handleSave(); }}
              autoFocus
            />
          ) : (
            <button
              className="ai-field-value-btn"
              aria-label={t('agentInfo.editAuthorName')}
              onClick={() => { setEditValue(settings.general.authorName); setEditing(true); }}
            >
              <span>{settings.general.authorName || t('agentInfo.notSet')}</span>
            </button>
          )}
        </div>

        {/* 确认级别 */}
        <div className="ai-field">
          <span className="ai-field-label">{t('agentInfo.confirmLevel')}</span>
          <select
            className="ai-field-select"
            aria-label={t('agentInfo.confirmLevel')}
            value={settings.general.confirmationLevel}
            onChange={(e) => updateSettings({ general: { confirmationLevel: e.target.value as "always" | "editOnly" | "never" } })}
          >
            {confirmationLevelOptions.map((opt) => (
              <option key={opt.value} value={opt.value}>{opt.label}</option>
            ))}
          </select>
        </div>
      </div>

      <style>{`
        .ai-grid {
          display: flex;
          flex-direction: column;
          gap: 2px;
        }
        .ai-field {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          padding: 3px 0;
        }
        .ai-field-label {
          font-size: 12px;
          color: var(--color-text-quaternary);
          flex-shrink: 0;
        }
        .ai-model-badge {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 3px 10px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          transition: background 0.2s;
        }

        .ai-status-dot {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          flex-shrink: 0;
          background: var(--color-text-quaternary);
          transition: background 0.3s, box-shadow 0.3s;
        }
        .ai-model-badge.online .ai-status-dot {
          background: var(--color-success);
          box-shadow: 0 0 4px rgba(52, 199, 36, 0.4);
        }
        .ai-model-name {
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .ai-field-value {
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .ai-field-value-btn {
          display: inline-flex;
          align-items: center;
          gap: 4px;
          padding: 2px 8px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-primary);
          border: 1px solid transparent;
          transition: all 0.15s;
          cursor: pointer;
          background: none;
        }
        .ai-field-value-btn:hover {
          border-color: var(--color-border);
          background: var(--color-bg);
          color: var(--color-accent);
        }
        .ai-field-edit {
          font-size: 12px;
          font-weight: 500;
          padding: 2px 8px;
          border: 1.5px solid var(--color-accent);
          border-radius: var(--radius-sm);
          width: 120px;
          background: var(--color-bg);
          box-shadow: 0 0 0 3px var(--color-accent-lighter);
          transition: all 0.2s;
          outline: none;
          color: var(--color-text-primary);
        }
        .ai-field-edit:focus-visible {
          outline: none;
        }
        .ai-field-select {
          font-size: 12px;
          font-weight: 500;
          padding: 2px 8px;
          border: 1px solid transparent;
          border-radius: var(--radius-sm);
          background: none;
          color: var(--color-text-primary);
          cursor: pointer;
          transition: all 0.15s;
          outline: none;
          -webkit-appearance: none;
          appearance: none;
        }
        .ai-field-select:hover {
          border-color: var(--color-border);
          background: var(--color-bg);
          color: var(--color-accent);
        }
        .ai-field-select option {
          color: var(--color-text-primary);
          background: var(--color-bg);
        }
        .ai-field-select:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
        }
        .ai-setup-hint {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 6px 10px;
          border-radius: var(--radius-sm);
          background: var(--color-accent-bg);
          border: 1px solid var(--color-accent-light);
          font-size: 12px;
          color: var(--color-accent);
          cursor: pointer;
          transition: all 0.2s;
          width: 100%;
        }
        .ai-setup-hint:hover {
          background: var(--color-accent-light);
          border-color: var(--color-accent);
        }
      `}</style>
    </SidebarSection>
  );
}
