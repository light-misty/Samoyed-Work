import { useState, useRef, useEffect } from "react";
import { Icon } from "../common/Icon";
import { useSettingsStore } from "../../stores/useSettingsStore";
import type { PromptTemplate, TemplateVariable } from "../../types";

interface TemplatePickerProps {
  open: boolean;
  onClose: () => void;
  onInsert: (text: string) => void;
}

const CATEGORY_LABELS: Record<string, string> = {
  document: "文档生成",
  analysis: "文档分析",
  conversion: "格式转换",
  custom: "自定义",
};

export function TemplatePicker({ open, onClose, onInsert }: TemplatePickerProps) {
  const { templates } = useSettingsStore();
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedTemplate, setSelectedTemplate] = useState<PromptTemplate | null>(null);
  const [varValues, setVarValues] = useState<Record<string, string>>({});
  const containerRef = useRef<HTMLDivElement>(null);

  // 点击外部关闭
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };
    // 延迟绑定避免立即触发
    const timer = setTimeout(() => document.addEventListener("mousedown", handler), 100);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handler);
    };
  }, [open, onClose]);

  // 重置状态
  useEffect(() => {
    if (open) {
      setSearchQuery("");
      setSelectedTemplate(null);
      setVarValues({});
    }
  }, [open]);

  if (!open) return null;

  // 过滤模板
  const filtered = searchQuery.trim()
    ? templates.filter((t) => {
        const q = searchQuery.toLowerCase();
        return t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q);
      })
    : templates;

  // 选择模板时初始化变量默认值
  const handleSelect = (tpl: PromptTemplate) => {
    setSelectedTemplate(tpl);
    const defaults: Record<string, string> = {};
    if (tpl.variables) {
      for (const v of tpl.variables) {
        defaults[v.name] = v.defaultValue != null ? String(v.defaultValue) : "";
      }
    }
    setVarValues(defaults);
  };

  // 替换模板中的变量占位符
  const resolveContent = (tpl: PromptTemplate): string => {
    let result = tpl.content;
    for (const [key, value] of Object.entries(varValues)) {
      result = result.split(`{{${key}}}`).join(value || `{{${key}}}`);
    }
    return result;
  };

  // 插入模板内容
  const handleInsert = () => {
    if (!selectedTemplate) return;
    const resolved = resolveContent(selectedTemplate);
    onInsert(resolved);
    onClose();
  };

  return (
    <div ref={containerRef} className="template-picker">
      {/* 搜索栏 */}
      <div className="picker-search">
        <Icon name="search" size={14} />
        <input
          className="picker-search-input"
          placeholder="搜索模板..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
          autoFocus
        />
      </div>

      {/* 模板列表或变量填写 */}
      {!selectedTemplate ? (
        <div className="picker-list">
          {filtered.length === 0 ? (
            <div className="picker-empty">无匹配模板</div>
          ) : (
            filtered.map((tpl) => (
              <button
                key={tpl.id}
                className="picker-item"
                onClick={() => handleSelect(tpl)}
              >
                <div className="picker-item-name">
                  {tpl.name}
                  {tpl.isBuiltin && <span className="picker-builtin-tag">内置</span>}
                </div>
                <div className="picker-item-desc">{tpl.description}</div>
                <span className="picker-item-category">
                  {CATEGORY_LABELS[tpl.category] ?? tpl.category}
                </span>
              </button>
            ))
          )}
        </div>
      ) : (
        <div className="picker-detail">
          {/* 返回按钮 + 模板名称 */}
          <div className="picker-detail-header">
            <button className="picker-back-btn" onClick={() => setSelectedTemplate(null)}>
              <Icon name="back" size={14} />
            </button>
            <span className="picker-detail-name">{selectedTemplate.name}</span>
          </div>

          {/* 变量填写 */}
          {selectedTemplate.variables && selectedTemplate.variables.length > 0 && (
            <div className="picker-vars">
              {selectedTemplate.variables.map((v: TemplateVariable) => (
                <div key={v.name} className="picker-var-group">
                  <label className="picker-var-label">{v.label}</label>
                  {v.type === "select" && v.options ? (
                    <select
                      className="picker-var-select"
                      value={varValues[v.name] ?? ""}
                      onChange={(e) => setVarValues({ ...varValues, [v.name]: e.target.value })}
                    >
                      <option value="">请选择...</option>
                      {v.options.map((opt) => (
                        <option key={opt} value={opt}>{opt}</option>
                      ))}
                    </select>
                  ) : v.type === "boolean" ? (
                    <select
                      className="picker-var-select"
                      value={varValues[v.name] ?? "true"}
                      onChange={(e) => setVarValues({ ...varValues, [v.name]: e.target.value })}
                    >
                      <option value="true">是</option>
                      <option value="false">否</option>
                    </select>
                  ) : (
                    <input
                      className="picker-var-input"
                      placeholder={v.label}
                      value={varValues[v.name] ?? ""}
                      onChange={(e) => setVarValues({ ...varValues, [v.name]: e.target.value })}
                    />
                  )}
                </div>
              ))}
            </div>
          )}

          {/* 预览 */}
          <div className="picker-preview">
            <div className="picker-preview-label">预览</div>
            <div className="picker-preview-content">
              {resolveContent(selectedTemplate)}
            </div>
          </div>

          {/* 插入按钮 */}
          <button className="picker-insert-btn" onClick={handleInsert}>
            插入到输入框
          </button>
        </div>
      )}

      <style>{`
        .template-picker {
          position: absolute;
          bottom: 100%;
          left: 0;
          right: 0;
          margin-bottom: 8px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border);
          border-radius: var(--radius-lg);
          box-shadow: var(--shadow-xl, 0 8px 30px rgba(0,0,0,0.12));
          max-height: 400px;
          display: flex;
          flex-direction: column;
          overflow: hidden;
          z-index: 100;
          animation: slideUp 0.15s ease;
        }
        @keyframes slideUp {
          from { opacity: 0; transform: translateY(8px); }
          to { opacity: 1; transform: translateY(0); }
        }
        .picker-search {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 10px 14px;
          border-bottom: 1px solid var(--color-border-light);
          color: var(--color-text-quaternary);
          flex-shrink: 0;
        }
        .picker-search-input {
          flex: 1;
          border: none;
          outline: none;
          font-size: 13px;
          background: transparent;
          color: var(--color-text-primary);
        }
        .picker-search-input::placeholder {
          color: var(--color-text-quaternary);
        }
        .picker-list {
          flex: 1;
          overflow-y: auto;
          padding: 6px;
        }
        .picker-empty {
          font-size: 13px;
          color: var(--color-text-quaternary);
          text-align: center;
          padding: 24px 16px;
        }
        .picker-item {
          width: 100%;
          text-align: left;
          padding: 10px 12px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: all 0.12s;
          border: none;
          background: transparent;
        }
        .picker-item:hover {
          background: var(--color-bg-sub);
        }
        .picker-item-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
          margin-bottom: 2px;
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .picker-builtin-tag {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 5px;
          border-radius: 3px;
          background: var(--color-accent-light);
          color: var(--color-accent);
        }
        .picker-item-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-bottom: 4px;
        }
        .picker-item-category {
          font-size: 10px;
          padding: 1px 6px;
          border-radius: 3px;
          background: var(--color-bg-sub);
          color: var(--color-text-tertiary);
        }
        .picker-detail {
          flex: 1;
          display: flex;
          flex-direction: column;
          overflow: hidden;
        }
        .picker-detail-header {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 10px 14px;
          border-bottom: 1px solid var(--color-border-light);
          flex-shrink: 0;
        }
        .picker-back-btn {
          width: 24px;
          height: 24px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-xs);
          color: var(--color-text-secondary);
          transition: all 0.12s;
        }
        .picker-back-btn:hover {
          background: var(--color-bg-sub);
        }
        .picker-detail-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .picker-vars {
          padding: 12px 14px;
          display: flex;
          flex-direction: column;
          gap: 10px;
          border-bottom: 1px solid var(--color-border-light);
          max-height: 180px;
          overflow-y: auto;
        }
        .picker-var-group {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .picker-var-label {
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
        }
        .picker-var-input {
          padding: 6px 10px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-xs);
          font-size: 12px;
          background: var(--color-bg);
          color: var(--color-text-primary);
        }
        .picker-var-input:focus {
          border-color: var(--color-accent);
          outline: none;
        }
        .picker-var-select {
          padding: 6px 10px;
          border: 1px solid var(--color-border);
          border-radius: var(--radius-xs);
          font-size: 12px;
          background: var(--color-bg);
          color: var(--color-text-primary);
        }
        .picker-preview {
          padding: 12px 14px;
          flex: 1;
          overflow-y: auto;
          min-height: 60px;
        }
        .picker-preview-label {
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-quaternary);
          margin-bottom: 6px;
        }
        .picker-preview-content {
          font-size: 12px;
          color: var(--color-text-secondary);
          line-height: 1.6;
          white-space: pre-wrap;
          font-family: var(--font-mono);
        }
        .picker-insert-btn {
          margin: 10px 14px 12px;
          padding: 8px 16px;
          border-radius: var(--radius-sm);
          font-size: 13px;
          font-weight: 500;
          background: var(--color-accent);
          color: white;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
          flex-shrink: 0;
        }
        .picker-insert-btn:hover {
          background: var(--color-accent-hover);
        }
      `}</style>
    </div>
  );
}
