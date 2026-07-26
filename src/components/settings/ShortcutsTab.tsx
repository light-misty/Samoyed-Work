import { useState, useRef, useEffect, useCallback } from "react";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "../../stores/useSettingsStore";
import type { Shortcuts } from "../../types";

export function ShortcutsTab() {
  const { t } = useTranslation();
  const { settings, updateSettings } = useSettingsStore();
  const [editingKey, setEditingKey] = useState<keyof Shortcuts | null>(null);
  const [capturedKeys, setCapturedKeys] = useState("");
  const captureRef = useRef<HTMLDivElement>(null);

  // 快捷键配置项定义
  const shortcutItems: {
    key: keyof Shortcuts;
    label: string;
    description: string;
  }[] = [
    { key: "newSession", label: t('settings.shortcuts.newSession'), description: t('settings.shortcuts.newSessionDesc') },
    { key: "closeSession", label: t('settings.shortcuts.closeSession'), description: t('settings.shortcuts.closeSessionDesc') },
    { key: "sendMessage", label: t('settings.shortcuts.sendMessage'), description: t('settings.shortcuts.sendMessageDesc') },
    { key: "toggleSidebar", label: t('settings.shortcuts.toggleSidebar'), description: t('settings.shortcuts.toggleSidebarDesc') },
    { key: "quickPrompt", label: t('settings.shortcuts.quickPrompt'), description: t('settings.shortcuts.quickPromptDesc') },
    { key: "switchMode", label: t('settings.shortcuts.switchMode'), description: t('settings.shortcuts.switchModeDesc') },
  ];

  // 重置所有快捷键为默认值
  const resetToDefaults = () => {
    updateSettings({
      shortcuts: {
        newSession: "Ctrl+N",
        closeSession: "Ctrl+W",
        sendMessage: "Enter",
        toggleSidebar: "Ctrl+B",
        quickPrompt: "Ctrl+/",
        switchMode: "Tab",
      },
    });
  };

  // 捕获键盘快捷键
  const handleKeyDown = useCallback(
    (e: KeyboardEvent) => {
      if (!editingKey) return;
      e.preventDefault();
      e.stopPropagation();

      // 忽略单独的修饰键
      if (["Control", "Shift", "Alt", "Meta"].includes(e.key)) {
        setCapturedKeys("");
        return;
      }

      const parts: string[] = [];
      if (e.ctrlKey || e.metaKey) parts.push("Ctrl");
      if (e.shiftKey) parts.push("Shift");
      if (e.altKey) parts.push("Alt");

      // 获取按键名称
      let keyName = e.key;
      if (keyName === " ") keyName = "Space";
      else if (keyName === "Escape") {
        // Escape 取消编辑
        setEditingKey(null);
        setCapturedKeys("");
        return;
      }
      // 首字母大写
      keyName = keyName.length === 1 ? keyName.toUpperCase() : keyName;

      parts.push(keyName);
      const combo = parts.join("+");
      setCapturedKeys(combo);

      // 自动确认绑定
      updateSettings({
        shortcuts: {
          [editingKey]: combo,
        },
      });
      setEditingKey(null);
      setCapturedKeys("");
    },
    [editingKey, updateSettings],
  );

  useEffect(() => {
    if (editingKey) {
      window.addEventListener("keydown", handleKeyDown, true);
      // 聚焦到捕获区域
      captureRef.current?.focus();
    }
    return () => {
      window.removeEventListener("keydown", handleKeyDown, true);
    };
  }, [editingKey, handleKeyDown]);

  return (
    <div>
      <div className="settings-section">
        <div className="section-header">
          <span className="section-title">{t('settings.shortcuts.sectionTitle')}</span>
          <button className="sc-reset-btn" onClick={resetToDefaults}>
            {t('settings.shortcuts.resetToDefault')}
          </button>
        </div>

        <div className="sc-list">
          {shortcutItems.map((item) => (
            <div key={item.key} className="sc-item">
              <div className="sc-info">
                <div className="sc-label">{item.label}</div>
                <div className="sc-desc">{item.description}</div>
              </div>
              {editingKey === item.key ? (
                <div
                  ref={captureRef}
                  className="sc-capture"
                  tabIndex={0}
                  onBlur={() => {
                    setEditingKey(null);
                    setCapturedKeys("");
                  }}
                >
                  {capturedKeys || t('settings.shortcuts.pressKeysHint')}
                </div>
              ) : (
                <button
                  className="sc-key"
                  onClick={() => {
                    setEditingKey(item.key);
                    setCapturedKeys("");
                  }}
                >
                  {settings.shortcuts[item.key]}
                </button>
              )}
            </div>
          ))}
        </div>
      </div>

      <style>{`
        .sc-reset-btn {
          padding: 4px 10px;
          font-size: 11px;
          color: var(--color-text-tertiary);
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: all 0.15s;
          margin-left: auto;
        }
        .sc-reset-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-secondary);
        }
        .sc-list {
          display: flex;
          flex-direction: column;
        }
        .sc-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          gap: 16px;
        }
        .sc-item:last-child {
          border-bottom: none;
        }
        .sc-info {
          flex: 1;
          min-width: 0;
        }
        .sc-label {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .sc-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 2px;
        }
        .sc-key {
          padding: 4px 12px;
          font-family: var(--font-mono);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
          background: var(--color-bg-sub);
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: all 0.15s;
          white-space: nowrap;
          flex-shrink: 0;
        }
        .sc-key:hover {
          background: var(--color-bg-hover);
          border-color: var(--color-accent);
          color: var(--color-accent);
        }
        .sc-capture {
          padding: 4px 12px;
          font-family: var(--font-mono);
          font-size: 12px;
          font-weight: 500;
          color: var(--color-accent);
          background: var(--color-accent-lighter);
          border: 2px solid var(--color-accent);
          border-radius: var(--radius-sm);
          outline: none;
          min-width: 100px;
          text-align: center;
          animation: pulse 1.5s ease-in-out infinite;
          flex-shrink: 0;
        }
      `}</style>
    </div>
  );
}
