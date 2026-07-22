import { useState, useEffect } from "react";
import { useTranslation } from 'react-i18next';
import type { ProviderInfo, LLMProviderType, ConnectionResult } from "../../types";
import * as tauriCmd from "../../services/tauri";

/** 将人类可读格式(如 "128K", "1M")解析为数字 */
function parseContextWindow(value: string): number | undefined {
  const trimmed = value.trim().toUpperCase();
  if (!trimmed) return undefined;
  // 支持纯数字
  if (/^\d+$/.test(trimmed)) {
    const num = parseInt(trimmed, 10);
    return num > 0 ? num : undefined;
  }
  // 支持 K 后缀 (千)
  const kMatch = trimmed.match(/^(\d+(?:\.\d+)?)K$/);
  if (kMatch) {
    const num = parseFloat(kMatch[1]) * 1000;
    return num > 0 ? Math.round(num) : undefined;
  }
  // 支持 M 后缀 (百万)
  const mMatch = trimmed.match(/^(\d+(?:\.\d+)?)M$/);
  if (mMatch) {
    const num = parseFloat(mMatch[1]) * 1_000_000;
    return num > 0 ? Math.round(num) : undefined;
  }
  return undefined;
}

/** 将数字格式化为人类可读字符串 */
function formatContextWindow(value: number | undefined): string {
  if (value === undefined || value === 0) return "";
  if (value >= 1_000_000 && value % 1_000_000 === 0) {
    return `${value / 1_000_000}M`;
  }
  if (value >= 1_000_000) {
    return `${(value / 1_000_000).toFixed(1).replace(/\.0$/, "")}M`;
  }
  if (value >= 1000 && value % 1000 === 0) {
    return `${value / 1000}K`;
  }
  if (value >= 1000) {
    return `${(value / 1000).toFixed(1).replace(/\.0$/, "")}K`;
  }
  return String(value);
}

/** 常用上下文窗口大小预设 */
const CONTEXT_PRESETS = [
  { label: "8K", value: 8192 },
  { label: "32K", value: 32768 },
  { label: "128K", value: 128000 },
  { label: "200K", value: 200000 },
  { label: "400K", value: 400000 },
  { label: "600K", value: 600000 },
  { label: "1M", value: 1000000 },
];

interface ProviderFormDialogProps {
  mode: "add" | "edit";
  provider?: ProviderInfo | null;
  onClose: () => void;
  onSaved: () => void;
}

// 服务商类型选项（不含中文标签，中文标签在组件内通过 t() 获取）
const providerTypeValues: { value: LLMProviderType; defaultBase: string }[] = [
  { value: "openai", defaultBase: "https://api.openai.com/v1" },
  { value: "anthropic", defaultBase: "https://api.anthropic.com" },
  { value: "gemini", defaultBase: "https://generativelanguage.googleapis.com/v1beta" },
  { value: "ollama", defaultBase: "http://localhost:11434/v1" },
  { value: "custom", defaultBase: "" },
];

// 服务商类型标签映射
const providerTypeLabels: Record<LLMProviderType, string> = {
  openai: "OpenAI",
  anthropic: "Anthropic",
  gemini: "Google Gemini",
  ollama: "Ollama",
  custom: "", // 自定义标签通过 t() 获取
};

export function ProviderFormDialog({ mode, provider, onClose, onSaved }: ProviderFormDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(provider?.name ?? "");
  const [providerType, setProviderType] = useState<LLMProviderType>(provider?.providerType ?? "openai");
  const [apiBase, setApiBase] = useState(provider?.apiBase ?? "https://api.openai.com/v1");
  const [apiKey, setApiKey] = useState("");
  const [model, setModel] = useState(provider?.model ?? "");
  const [contextWindow, setContextWindow] = useState<string>(
    formatContextWindow(provider?.contextWindow)
  );
  const [supportsVision, setSupportsVision] = useState<boolean>(
    provider?.supportsVision ?? false
  );
  const [saving, setSaving] = useState(false);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState<ConnectionResult | null>(null);
  const [error, setError] = useState<string | null>(null);

  // 获取服务商类型选项（含 i18n 标签）
  const providerTypeOptions = providerTypeValues.map((opt) => ({
    ...opt,
    label: opt.value === "custom" ? t('settings.providerForm.typeCustom') : providerTypeLabels[opt.value],
  }));

  useEffect(() => {
    const option = providerTypeValues.find((o) => o.value === providerType);
    if (option && mode === "add") {
      setApiBase(option.defaultBase);
    }
  }, [providerType, mode]);

  const handleSave = async () => {
    if (!name.trim()) { setError(t('settings.providerForm.enterProviderName')); return; }
    if (!apiBase.trim()) { setError(t('settings.providerForm.enterApiBase')); return; }
    if (!model.trim()) { setError(t('settings.providerForm.enterModelName')); return; }
    if (mode === "add" && !apiKey.trim()) { setError(t('settings.providerForm.enterApiKey')); return; }

    setSaving(true);
    setError(null);
    try {
      const config = {
        name: name.trim(),
        providerType,
        apiBase: apiBase.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
        contextWindow: parseContextWindow(contextWindow),
        supportsVision: supportsVision,
      };
      if (mode === "add") {
        await tauriCmd.addProvider(config);
      } else if (provider) {
        await tauriCmd.updateProvider(provider.id, config);
      }
      onSaved();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : t('settings.providerForm.saveFailed');
      setError(msg);
    } finally {
      setSaving(false);
    }
  };

  const handleTest = async () => {
    // 验证必要参数（添加和编辑模式通用）
    if (!apiBase.trim()) {
      setError(t('settings.providerForm.enterApiBase'));
      return;
    }
    if (!model.trim()) {
      setError(t('settings.providerForm.enterModelName'));
      return;
    }
    // 添加模式下 API Key 必填；编辑模式下可留空，后端会从已保存 Provider 查找
    if (mode === "add" && !apiKey.trim()) {
      setError(t('settings.providerForm.enterApiKey'));
      return;
    }

    setTesting(true);
    setTestResult(null);
    setError(null);
    try {
      // 始终使用 testConnectionWithConfig 传递当前表单值
      // 编辑模式下传入 providerId，后端在 API Key 为空时自动从已保存 Provider 查找
      const config = {
        name: name.trim(),
        providerType,
        apiBase: apiBase.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
        contextWindow: parseContextWindow(contextWindow),
        supportsVision: supportsVision,
      };
      const providerId = mode === "edit" ? provider?.id : undefined;
      const result = await tauriCmd.testConnectionWithConfig(config, providerId);
      setTestResult(result);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : t('settings.providerForm.connectionTestFailed');
      setTestResult({ success: false, latencyMs: 0, errorMessage: msg, error: msg });
    } finally {
      setTesting(false);
    }
  };

  return (
    <div
      className="fixed inset-0 bg-overlay z-[400] flex items-center justify-center animate-fade-in"
    >
      <div
        className="dialog-modal"
      >
        <div className="dialog-header">
          <h3 className="dialog-title">
            {mode === "add" ? t('settings.providerForm.addProvider') : t('settings.providerForm.editProvider')}
          </h3>
          <button className="dialog-close-btn" onClick={onClose}>x</button>
        </div>

        <div className="dialog-body">
          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.providerName')}</label>
            <input
              className="form-input"
              placeholder={t('settings.providerForm.providerNamePlaceholder')}
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.providerType')}</label>
            <select
              className="form-select"
              value={providerType}
              onChange={(e) => setProviderType(e.target.value as LLMProviderType)}
            >
              {providerTypeOptions.map((opt) => (
                <option key={opt.value} value={opt.value}>{opt.label}</option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.apiBaseUrl')}</label>
            <input
              className="form-input form-input-mono"
              placeholder="https://api.openai.com/v1"
              value={apiBase}
              onChange={(e) => setApiBase(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">
              {t('settings.providerForm.apiKey')}{mode === "edit" ? t('settings.providerForm.apiKeyEditHint') : ""}
            </label>
            <input
              type="password"
              className="form-input form-input-mono"
              placeholder={mode === "edit" ? "sk-..." : "sk-..."}
              value={apiKey}
              onChange={(e) => setApiKey(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.modelName')}</label>
            <input
              className="form-input form-input-mono"
              placeholder={t('settings.providerForm.modelNamePlaceholder')}
              value={model}
              onChange={(e) => setModel(e.target.value)}
            />
          </div>

          <div className="form-group">
            <label className="form-label">
              {t('settings.providerForm.contextWindowSize')}
            </label>
            <input
              className="form-input form-input-mono"
              type="text"
              placeholder={t('settings.providerForm.contextWindowPlaceholder')}
              value={contextWindow}
              onChange={(e) => setContextWindow(e.target.value)}
            />
            <div className="context-presets">
              {CONTEXT_PRESETS.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className={`context-preset-btn ${contextWindow === preset.label ? "active" : ""}`}
                  onClick={() => setContextWindow(preset.label)}
                >
                  {preset.label}
                </button>
              ))}
            </div>
          </div>

          <div className="form-group">
            <label className="form-label">
              {t('settings.providerForm.visionCapability')}
            </label>
            <select
              className="form-select"
              value={supportsVision ? "yes" : "no"}
              onChange={(e) => setSupportsVision(e.target.value === "yes")}
            >
              <option value="no">{t('settings.providerForm.notSupported')}</option>
              <option value="yes">{t('settings.providerForm.supported')}</option>
            </select>
          </div>

          {testResult && (
            <div className={`test-result ${testResult.success ? "test-success" : "test-error"}`}>
              {testResult.success ? (
                <span>{testResult.model ? t('settings.providerForm.testConnectionSuccessWithModel', { latency: testResult.latencyMs, model: testResult.model }) : t('settings.providerForm.testConnectionSuccess', { latency: testResult.latencyMs })}</span>
              ) : (
                <span>{t('settings.providerForm.testConnectionFailed', { error: testResult.errorMessage || testResult.error || t('settings.providerForm.unknownError') })}</span>
              )}
            </div>
          )}

          {error && (
            <div className="test-result test-error">{error}</div>
          )}
        </div>

        <div className="dialog-footer">
          <button
            className="dialog-btn dialog-btn-ghost mr-auto"
            onClick={handleTest}
            disabled={testing}
          >
            {testing ? (
              <span className="test-loading">
                <span className="test-spinner"></span>
                {t('settings.providerForm.testing')}
              </span>
            ) : t('settings.providerForm.testConnection')}
          </button>
          <button className="dialog-btn dialog-btn-primary" onClick={handleSave} disabled={saving}>
            {saving ? t('settings.providerForm.saving') : t('settings.providerForm.save')}
          </button>
          <button className="dialog-btn dialog-btn-ghost" onClick={onClose}>{t('settings.providerForm.cancel')}</button>
        </div>
      </div>

      <style>{`
        .dialog-modal {
          width: 520px;
          background: var(--color-bg-elevated);
          border-radius: var(--radius-xl);
          box-shadow: var(--shadow-xl);
          display: flex;
          flex-direction: column;
          overflow: hidden;
          animation: scaleIn 0.2s ease;
        }
        .dialog-header {
          padding: 18px 24px;
          border-bottom: 1px solid var(--color-border-light);
          display: flex;
          align-items: center;
          gap: 12px;
          flex-shrink: 0;
        }
        .dialog-title {
          font-size: 15px;
          font-weight: 700;
          color: var(--color-text-primary);
          flex: 1;
        }
        .dialog-close-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-secondary);
          transition: all 0.15s;
          font-size: 16px;
        }
        .dialog-close-btn:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .dialog-body {
          flex: 1;
          overflow-y: auto;
          padding: 20px 24px;
          display: flex;
          flex-direction: column;
          gap: 16px;
        }
        .form-group {
          display: flex;
          flex-direction: column;
          gap: 6px;
        }
        .form-label {
          font-size: 12px;
          font-weight: 500;
          color: var(--color-text-secondary);
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .form-input {
          padding: 8px 12px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          transition: all 0.2s;
          background: var(--color-bg);
          color: var(--color-text-primary);
        }
        .form-input:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          outline: none;
        }
        .form-input-mono {
          font-family: var(--font-mono);
        }
        .form-select {
          padding: 8px 12px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          font-size: 13px;
          background: var(--color-bg);
          color: var(--color-text-primary);
          cursor: pointer;
          transition: all 0.2s;
        }
        .form-select:focus {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
          outline: none;
        }
        .test-result {
          padding: 8px 12px;
          border-radius: var(--radius-sm);
          font-size: 12px;
        }
        .test-success {
          background: var(--color-success-light);
          color: var(--color-success);
          border: 1px solid var(--color-success-bg);
        }
        .test-error {
          background: var(--color-error-light);
          color: var(--color-error);
          border: 1px solid var(--color-error-bg);
        }
        .dialog-footer {
          padding: 16px 24px;
          border-top: 1px solid var(--color-border-light);
          display: flex;
          align-items: center;
          gap: 8px;
          flex-shrink: 0;
        }
        .dialog-btn {
          padding: 6px 16px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .dialog-btn-primary {
          background: var(--color-accent);
          color: white;
        }
        .dialog-btn-primary:hover:not(:disabled) {
          background: var(--color-accent-hover);
        }
        .dialog-btn-primary:disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .dialog-btn-ghost {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }
        .dialog-btn-ghost:hover {
          background: var(--color-bg-hover);
        }
        .dialog-btn-ghost:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
        .test-loading {
          display: inline-flex;
          align-items: center;
          gap: 4px;
        }
        .test-spinner {
          width: 10px;
          height: 10px;
          border: 2px solid var(--color-text-quaternary);
          border-top-color: var(--color-text-secondary);
          border-radius: 50%;
          animation: spin 0.8s linear infinite;
        }
        @keyframes spin {
          to { transform: rotate(360deg); }
        }
        .context-presets {
          display: flex;
          gap: 6px;
          margin-top: 2px;
        }
        .context-preset-btn {
          padding: 3px 10px;
          border-radius: var(--radius-xs);
          font-size: 11px;
          font-weight: 500;
          font-family: var(--font-mono);
          background: var(--color-bg-sub);
          color: var(--color-text-tertiary);
          border: 1px solid var(--color-border-light);
          cursor: pointer;
          transition: all 0.15s;
        }
        .context-preset-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
          border-color: var(--color-border-strong);
        }
        .context-preset-btn.active {
          background: var(--color-accent-light);
          color: var(--color-accent);
          border-color: var(--color-accent);
        }
      `}</style>
    </div>
  );
}
