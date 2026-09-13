import { useState } from "react";
import { useTranslation } from 'react-i18next';
import type { ProviderInfo, LLMProviderType } from "../../types";
import * as tauriCmd from "../../services/tauri";
import { useToastStore } from "../../stores/useToastStore";

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

// 模型模板：DeepSeek 官方仅开放 OpenAI 与 Anthropic 两种兼容格式
const MODEL_TEMPLATES: Record<string, Partial<Record<LLMProviderType, string>>> = {
  deepseek: {
    openai: "https://api.deepseek.com",
    anthropic: "https://api.deepseek.com/anthropic",
  },
};

export function ProviderFormDialog({ mode, provider, onClose, onSaved }: ProviderFormDialogProps) {
  const { t } = useTranslation();
  const [name, setName] = useState(provider?.name ?? "");
  const [providerType, setProviderType] = useState<LLMProviderType>(provider?.providerType ?? "openai");
  const [apiBase, setApiBase] = useState(provider?.apiBase ?? "");
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
  // 字段级验证错误（显示在对应输入框下方）
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  // 后端/非字段级错误（显示在表单底部）
  const [error, setError] = useState<string | null>(null);
  // 当前激活的模型模板（激活后切换服务商类型时自动联动 API Base URL）
  const [activeTemplate, setActiveTemplate] = useState<string | null>(null);
  // 模型下拉列表状态
  const [modelListOpen, setModelListOpen] = useState(false);
  const [modelList, setModelList] = useState<string[] | null>(null);
  const [modelListLoading, setModelListLoading] = useState(false);
  const [modelListError, setModelListError] = useState<string | null>(null);
  // 已获取模型列表对应的请求指纹（API 未变化时复用缓存，避免重复请求）
  const [modelListFetchedKey, setModelListFetchedKey] = useState("");

  // 校验必填字段，收集所有错误返回（测试连接不要求上下文窗口；服务商名称非必填，为空时保存使用模型名称兜底）
  // Ollama 为本地部署，不需要 API Key
  const isOllama = providerType === "ollama";
  const validateRequired = (includeContextWindow = true): Record<string, string> => {
    const errors: Record<string, string> = {};
    if (!apiBase.trim()) errors.apiBase = t('settings.providerForm.enterApiBase');
    if (!model.trim()) errors.model = t('settings.providerForm.enterModelName');
    // 添加模式下 API Key 必填；编辑模式下可留空，后端会从已保存 Provider 查找；Ollama 不需要 API Key
    if (mode === "add" && !isOllama && !apiKey.trim()) errors.apiKey = t('settings.providerForm.enterApiKey');
    if (includeContextWindow && !contextWindow.trim()) errors.contextWindow = t('settings.providerForm.enterContextWindow');
    return errors;
  };

  // 输入时清除对应字段的错误提示
  const clearFieldError = (field: string) => {
    setFieldErrors((prev) => {
      if (!prev[field]) return prev;
      const next = { ...prev };
      delete next[field];
      return next;
    });
  };

  // 应用模型模板：根据服务商类型填充对应的 API Base URL
  const applyModelTemplate = (template: string) => {
    const urls = MODEL_TEMPLATES[template];
    if (!urls) return;
    setActiveTemplate(template);
    // DeepSeek 模板支持 OpenAI / Anthropic 两种格式，其他类型默认使用 OpenAI 格式
    const nextType: LLMProviderType = providerType === "anthropic" ? "anthropic" : "openai";
    setProviderType(nextType);
    const url = urls[nextType];
    if (url) setApiBase(url);
    clearFieldError("apiBase");
  };

  // 切换服务商类型：模板激活时自动联动 API Base URL；选择 Ollama 时自动填充默认地址
  const handleProviderTypeChange = (value: LLMProviderType) => {
    setProviderType(value);
    if (value === "ollama") {
      // Ollama 默认本地地址
      setApiBase("http://localhost:11434/v1");
      clearFieldError("apiBase");
      return;
    }
    if (activeTemplate) {
      const url = MODEL_TEMPLATES[activeTemplate]?.[value];
      if (url) {
        setApiBase(url);
      } else {
        // 模板不支持当前类型：清空 API Base URL，保留模板联动，切回支持类型时自动恢复
        setApiBase("");
      }
    }
    clearFieldError("apiBase");
  };

  // 点击模型名称输入框：根据 API Key 与 API Base URL 获取可用模型列表
  // Ollama 等本地部署可省略 API Key
  const handleModelInputFocus = async () => {
    if (modelListLoading) return;
    setModelListOpen(true);
    if (!apiBase.trim()) { setModelListError(t('settings.providerForm.enterApiBase')); setModelList(null); return; }
    // 非 Ollama 类型需要 API Key 才能获取模型列表
    if (!isOllama && !apiKey.trim()) { setModelListError(t('settings.providerForm.enterApiKey')); setModelList(null); return; }
    const fetchKey = `${providerType}|${apiBase.trim()}|${apiKey.trim()}`;
    // API Key / Base URL / 类型未变化时复用已获取的列表
    if (modelListFetchedKey === fetchKey && modelList) return;
    setModelListLoading(true);
    setModelListError(null);
    try {
      const models = await tauriCmd.listModels(apiBase.trim(), apiKey.trim(), providerType);
      setModelList(models);
      setModelListFetchedKey(fetchKey);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : t('settings.providerForm.fetchModelsFailed');
      setModelListError(msg);
      setModelList(null);
    } finally {
      setModelListLoading(false);
    }
  };

  // 获取服务商类型选项（含 i18n 标签）
  const providerTypeOptions = providerTypeValues.map((opt) => ({
    ...opt,
    label: opt.value === "custom" ? t('settings.providerForm.typeCustom') : providerTypeLabels[opt.value],
  }));

  // 根据服务商类型显示对应的默认地址占位符（灰色提示，不自动填充）
  const apiBasePlaceholder = providerTypeValues.find((o) => o.value === providerType)?.defaultBase;

  // 服务商名称非必填：为空时使用模型名称作为默认名称
  const effectiveName = name.trim() || model.trim();

  const handleSave = async () => {
    const errors = validateRequired();
    if (Object.keys(errors).length > 0) {
      setFieldErrors(errors);
      return;
    }

    setSaving(true);
    setFieldErrors({});
    setError(null);
    try {
      const config = {
        name: effectiveName,
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
    // 验证必要参数（添加和编辑模式通用，测试连接不要求上下文窗口）
    const errors = validateRequired(false);
    if (Object.keys(errors).length > 0) {
      setFieldErrors(errors);
      return;
    }

    setTesting(true);
    setFieldErrors({});
    setError(null);
    try {
      // 始终使用 testConnectionWithConfig 传递当前表单值
      // 编辑模式下传入 providerId，后端在 API Key 为空时自动从已保存 Provider 查找
      const config = {
        name: effectiveName,
        providerType,
        apiBase: apiBase.trim(),
        apiKey: apiKey.trim(),
        model: model.trim(),
        contextWindow: parseContextWindow(contextWindow),
        supportsVision: supportsVision,
      };
      const providerId = mode === "edit" ? provider?.id : undefined;
      const result = await tauriCmd.testConnectionWithConfig(config, providerId);
      // 测试连接结果通过右上角 Toast 展示
      if (result.success) {
        const msg = result.model
          ? t('settings.providerForm.testConnectionSuccessWithModel', { latency: result.latencyMs, model: result.model })
          : t('settings.providerForm.testConnectionSuccess', { latency: result.latencyMs });
        useToastStore.getState().addToast("success", msg);
      } else {
        const msg = t('settings.providerForm.testConnectionFailed', { error: result.errorMessage || result.error || t('settings.providerForm.unknownError') });
        useToastStore.getState().addToast("error", msg);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : typeof err === "string" ? err : t('settings.providerForm.connectionTestFailed');
      useToastStore.getState().addToast("error", t('settings.providerForm.testConnectionFailed', { error: msg }));
    } finally {
      setTesting(false);
    }
  };

  return (
    <div className="fixed inset-0 bg-overlay z-[400] flex items-center justify-center animate-fade-in">
      <div className="dialog-modal">
        <div className="dialog-header">
          <h3 className="dialog-title">
            {mode === "add" ? t('settings.providerForm.addProvider') : t('settings.providerForm.editProvider')}
          </h3>
          <button className="dialog-close-btn" onClick={onClose}>x</button>
        </div>

        <div className="dialog-body">
          {mode === "add" && (
            <div className="form-group">
              <label className="form-label">{t('settings.providerForm.modelTemplate')}</label>
              <div className="template-buttons">
                {Object.keys(MODEL_TEMPLATES).map((key) => (
                  <button
                    key={key}
                    type="button"
                    className={`template-btn ${activeTemplate === key ? "active" : ""}`}
                    onClick={() => applyModelTemplate(key)}
                  >
                    {key === "deepseek" ? "DeepSeek" : key}
                  </button>
                ))}
              </div>
            </div>
          )}

          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.providerName')}</label>
            <input
              className="form-input"
              placeholder={t('settings.providerForm.providerNamePlaceholder')}
              value={name}
              onChange={(e) => { setName(e.target.value); clearFieldError("name"); }}
            />
          </div>

          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.providerType')}</label>
            <select
              className="form-select"
              value={providerType}
              onChange={(e) => handleProviderTypeChange(e.target.value as LLMProviderType)}
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
              placeholder={apiBasePlaceholder}
              value={apiBase}
              onChange={(e) => { setApiBase(e.target.value); setActiveTemplate(null); clearFieldError("apiBase"); }}
            />
            {fieldErrors.apiBase && (
              <div className="form-field-error">{fieldErrors.apiBase}</div>
            )}
          </div>

          {/* Ollama 为本地部署，不需要 API Key，隐藏输入框 */}
          {!isOllama && (
            <div className="form-group">
              <label className="form-label">
                {t('settings.providerForm.apiKey')}{mode === "edit" ? t('settings.providerForm.apiKeyEditHint') : ""}
              </label>
              <input
                type="password"
                className="form-input form-input-mono"
                placeholder="sk-..."
                value={apiKey}
                onChange={(e) => { setApiKey(e.target.value); clearFieldError("apiKey"); }}
              />
              {fieldErrors.apiKey && (
                <div className="form-field-error">{fieldErrors.apiKey}</div>
              )}
            </div>
          )}

          <div className="form-group">
            <label className="form-label">{t('settings.providerForm.modelName')}</label>
            <input
              className="form-input form-input-mono"
              placeholder={t('settings.providerForm.modelNamePlaceholder')}
              value={model}
              onFocus={handleModelInputFocus}
              onBlur={() => setModelListOpen(false)}
              onChange={(e) => { setModel(e.target.value); setModelListOpen(false); clearFieldError("model"); }}
            />
            {fieldErrors.model && (
              <div className="form-field-error">{fieldErrors.model}</div>
            )}
            {modelListOpen && (modelList || modelListLoading || modelListError) && (
              <div className="model-dropdown">
                {modelListLoading && (
                  <div className="model-dropdown-item model-dropdown-hint">{t('settings.providerForm.fetchModelsLoading')}</div>
                )}
                {modelListError && (
                  <div className="model-dropdown-item model-dropdown-error">{modelListError}</div>
                )}
                {modelList && modelList.length === 0 && !modelListLoading && (
                  <div className="model-dropdown-item model-dropdown-hint">{t('settings.providerForm.modelListEmpty')}</div>
                )}
                {modelList?.map((m) => (
                  <div
                    key={m}
                    className="model-dropdown-item"
                    // 阻止 mousedown 触发输入框 blur，保证点击时下拉列表不提前关闭
                    onMouseDown={(e) => e.preventDefault()}
                    onClick={() => { setModel(m); setModelListOpen(false); clearFieldError("model"); }}
                  >
                    {m}
                  </div>
                ))}
              </div>
            )}
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
              onChange={(e) => { setContextWindow(e.target.value); clearFieldError("contextWindow"); }}
            />
            {fieldErrors.contextWindow && (
              <div className="form-field-error">{fieldErrors.contextWindow}</div>
            )}
            <div className="context-presets">
              {CONTEXT_PRESETS.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className={`context-preset-btn ${contextWindow === preset.label ? "active" : ""}`}
                  onClick={() => { setContextWindow(preset.label); clearFieldError("contextWindow"); }}
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
          max-height: 90vh;
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
          min-height: 0;
          overflow-y: auto;
          scrollbar-width: none;
          padding: 20px 24px;
          display: flex;
          flex-direction: column;
          gap: 16px;
        }
        .dialog-body::-webkit-scrollbar {
          display: none;
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
        .form-field-error {
          font-size: 11px;
          color: var(--color-error);
        }
        .template-buttons {
          display: flex;
          gap: 8px;
        }
        .template-btn {
          padding: 6px 14px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          border: 1px solid var(--color-border-light);
          cursor: pointer;
          transition: all 0.15s;
        }
        .template-btn:hover {
          background: var(--color-bg-hover);
          color: var(--color-text-primary);
          border-color: var(--color-border-strong);
        }
        .template-btn.active {
          background: var(--color-accent-light);
          color: var(--color-accent);
          border-color: var(--color-accent);
        }
        .model-dropdown {
          max-height: 180px;
          overflow-y: auto;
          scrollbar-width: none;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-sm);
          background: var(--color-bg-elevated);
          display: flex;
          flex-direction: column;
          gap: 2px;
          padding: 4px;
        }
        .model-dropdown::-webkit-scrollbar {
          display: none;
        }
        .model-dropdown-item {
          padding: 6px 10px;
          border-radius: var(--radius-xs);
          font-size: 12px;
          font-family: var(--font-mono);
          color: var(--color-text-primary);
          cursor: pointer;
          transition: all 0.12s;
        }
        .model-dropdown-item:hover {
          background: var(--color-bg-hover);
        }
        .model-dropdown-hint {
          color: var(--color-text-tertiary);
          cursor: default;
        }
        .model-dropdown-hint:hover {
          background: transparent;
        }
        .model-dropdown-error {
          color: var(--color-error);
          cursor: default;
        }
        .model-dropdown-error:hover {
          background: transparent;
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
