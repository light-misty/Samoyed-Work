import { useState, useEffect } from "react";
import { Icon } from "../common/Icon";
import { useSettingsStore } from "../../stores/useSettingsStore";
import type { PromptTemplate, TemplateVariable, CreateTemplateParams } from "../../types";

interface TemplateEditDialogProps {
  open: boolean;
  onClose: () => void;
  template?: PromptTemplate | null;
}

const CATEGORIES = [
  { value: "document", label: "文档生成" },
  { value: "analysis", label: "文档分析" },
  { value: "conversion", label: "格式转换" },
  { value: "custom", label: "自定义" },
];

const VAR_TYPES = [
  { value: "string", label: "文本" },
  { value: "number", label: "数字" },
  { value: "boolean", label: "布尔" },
  { value: "select", label: "选择" },
];

export function TemplateEditDialog({ open, onClose, template }: TemplateEditDialogProps) {
  const { createTemplate, updateTemplate } = useSettingsStore();
  const isEdit = !!template;

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [content, setContent] = useState("");
  const [category, setCategory] = useState("custom");
  const [variables, setVariables] = useState<TemplateVariable[]>([]);
  const [saving, setSaving] = useState(false);

  // 编辑模式时填充表单
  useEffect(() => {
    if (template) {
      setName(template.name);
      setDescription(template.description);
      setContent(template.content);
      setCategory(template.category);
      setVariables(template.variables ?? []);
    } else {
      setName("");
      setDescription("");
      setContent("");
      setCategory("custom");
      setVariables([]);
    }
  }, [template, open]);

  // 从模板内容中自动提取 {{变量名}} 占位符
  const extractVariables = () => {
    const matches = content.match(/\{\{(\w+)\}\}/g);
    if (!matches) return;
    const existingNames = new Set(variables.map((v) => v.name));
    const newVars: TemplateVariable[] = [];
    for (const match of matches) {
      const varName = match.slice(2, -2);
      if (!existingNames.has(varName)) {
        newVars.push({
          name: varName,
          type: "string",
          label: varName,
        });
        existingNames.add(varName);
      }
    }
    if (newVars.length > 0) {
      setVariables([...variables, ...newVars]);
    }
  };

  // 添加变量
  const addVariable = () => {
    setVariables([...variables, { name: "", type: "string", label: "" }]);
  };

  // 更新变量
  const updateVariable = (index: number, field: keyof TemplateVariable, value: unknown) => {
    const updated = [...variables];
    updated[index] = { ...updated[index], [field]: value };
    // 如果修改了 name，同步更新 label（仅当 label 为空或与旧 name 相同时）
    if (field === "name" && typeof value === "string") {
      if (!updated[index].label || updated[index].label === updated[index].name) {
        updated[index].label = value;
      }
    }
    setVariables(updated);
  };

  // 删除变量
  const removeVariable = (index: number) => {
    setVariables(variables.filter((_, i) => i !== index));
  };

  // 保存模板
  const handleSave = async () => {
    if (!name.trim() || !content.trim()) return;
    setSaving(true);
    try {
      // 过滤掉未填写 name 的变量
      const validVars = variables.filter((v) => v.name.trim());
      const varsPayload = validVars.length > 0 ? validVars : undefined;

      if (isEdit && template) {
        await updateTemplate(template.id, {
          name,
          description,
          content,
          category,
          variables: varsPayload,
        });
      } else {
        const params: CreateTemplateParams = {
          name,
          description,
          content,
          category,
          variables: varsPayload,
        };
        await createTemplate(params);
      }
      onClose();
    } catch (err) {
      console.error("[TemplateEditDialog] 保存失败:", err);
    } finally {
      setSaving(false);
    }
  };

  if (!open) return null;

  return (
    <div
      className="fixed inset-0 bg-overlay z-[310] flex items-center justify-center animate-fade-in"
      onClick={(e) => { if (e.target === e.currentTarget) onClose(); }}
    >
      <div
        className="template-edit-dialog"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="dialog-header">
          <h3 className="dialog-title">{isEdit ? "编辑模板" : "创建模板"}</h3>
          <button className="dialog-close-btn" onClick={onClose}>
            <Icon name="close" size={16} />
          </button>
        </div>

        {/* 表单内容 */}
        <div className="dialog-body">
          {/* 模板名称 */}
          <div className="form-group">
            <label className="form-label">模板名称 <span className="form-required">*</span></label>
            <input
              className="form-input"
              placeholder="例如：周报生成"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
          </div>

          {/* 模板描述 */}
          <div className="form-group">
            <label className="form-label">描述</label>
            <input
              className="form-input"
              placeholder="简要描述模板用途"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>

          {/* 分类 */}
          <div className="form-group">
            <label className="form-label">分类</label>
            <select
              className="form-select"
              value={category}
              onChange={(e) => setCategory(e.target.value)}
            >
              {CATEGORIES.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>

          {/* 模板内容 */}
          <div className="form-group">
            <div className="form-label-row">
              <label className="form-label">模板内容 <span className="form-required">*</span></label>
              <button className="extract-btn" onClick={extractVariables}>
                <Icon name="template" size={12} />
                提取变量
              </button>
            </div>
            <textarea
              className="form-textarea"
              placeholder="输入模板内容，使用 {{变量名}} 定义变量占位符&#10;例如：请帮我生成一份{{docType}}，主题是{{topic}}"
              rows={6}
              value={content}
              onChange={(e) => setContent(e.target.value)}
            />
            <div className="form-hint">使用 {"{{变量名}}"} 语法定义可替换的变量占位符</div>
          </div>

          {/* 变量定义 */}
          {variables.length > 0 && (
            <div className="form-group">
              <label className="form-label">变量定义</label>
              <div className="variables-list">
                {variables.map((v, i) => (
                  <div key={i} className="variable-row">
                    <input
                      className="var-input var-input-name"
                      placeholder="变量名"
                      value={v.name}
                      onChange={(e) => updateVariable(i, "name", e.target.value)}
                    />
                    <input
                      className="var-input var-input-label"
                      placeholder="显示标签"
                      value={v.label}
                      onChange={(e) => updateVariable(i, "label", e.target.value)}
                    />
                    <select
                      className="var-select"
                      value={v.type}
                      onChange={(e) => updateVariable(i, "type", e.target.value)}
                    >
                      {VAR_TYPES.map((t) => (
                        <option key={t.value} value={t.value}>{t.label}</option>
                      ))}
                    </select>
                    {v.type === "select" && (
                      <input
                        className="var-input var-input-options"
                        placeholder="选项1,选项2,选项3"
                        value={v.options?.join(",") ?? ""}
                        onChange={(e) => updateVariable(i, "options", e.target.value.split(",").map((s) => s.trim()).filter(Boolean))}
                      />
                    )}
                    <button className="var-remove-btn" onClick={() => removeVariable(i)}>
                      <Icon name="close" size={12} />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 添加变量按钮 */}
          <button className="add-var-btn" onClick={addVariable}>
            <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
            添加变量
          </button>
        </div>

        {/* 底部按钮 */}
        <div className="dialog-footer">
          <button className="btn-cancel" onClick={onClose}>取消</button>
          <button
            className="btn-save"
            onClick={handleSave}
            disabled={!name.trim() || !content.trim() || saving}
          >
            {saving ? "保存中..." : isEdit ? "保存修改" : "创建模板"}
          </button>
        </div>

        <style>{`
          .template-edit-dialog {
            width: 600px;
            max-height: 85vh;
            background: var(--color-bg-elevated);
            border-radius: var(--radius-xl);
            box-shadow: var(--shadow-xl);
            display: flex;
            flex-direction: column;
            overflow: hidden;
            animation: scaleIn 0.2s ease;
          }
          .dialog-header {
            padding: 16px 20px;
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
          }
          .dialog-close-btn:hover {
            background: var(--color-bg-sub);
            color: var(--color-text-primary);
          }
          .dialog-body {
            flex: 1;
            overflow-y: auto;
            padding: 20px;
          }
          .form-group {
            margin-bottom: 16px;
          }
          .form-group:last-child {
            margin-bottom: 0;
          }
          .form-label {
            display: block;
            font-size: 12px;
            font-weight: 600;
            color: var(--color-text-secondary);
            margin-bottom: 6px;
          }
          .form-label-row {
            display: flex;
            align-items: center;
            justify-content: space-between;
            margin-bottom: 6px;
          }
          .form-required {
            color: var(--color-error, #ef4444);
          }
          .form-input {
            width: 100%;
            padding: 8px 12px;
            border: 1px solid var(--color-border);
            border-radius: var(--radius-sm);
            font-size: 13px;
            background: var(--color-bg);
            color: var(--color-text-primary);
            transition: all 0.2s;
            box-sizing: border-box;
          }
          .form-input:focus {
            border-color: var(--color-accent);
            box-shadow: 0 0 0 2px var(--color-accent-lighter);
            outline: none;
          }
          .form-select {
            padding: 8px 12px;
            border: 1px solid var(--color-border);
            border-radius: var(--radius-sm);
            font-size: 13px;
            background: var(--color-bg);
            color: var(--color-text-primary);
            cursor: pointer;
          }
          .form-select:focus {
            border-color: var(--color-accent);
            box-shadow: 0 0 0 2px var(--color-accent-lighter);
            outline: none;
          }
          .form-textarea {
            width: 100%;
            padding: 10px 12px;
            border: 1px solid var(--color-border);
            border-radius: var(--radius-sm);
            font-size: 13px;
            font-family: var(--font-mono);
            line-height: 1.6;
            background: var(--color-bg);
            color: var(--color-text-primary);
            resize: vertical;
            min-height: 120px;
            box-sizing: border-box;
          }
          .form-textarea:focus {
            border-color: var(--color-accent);
            box-shadow: 0 0 0 2px var(--color-accent-lighter);
            outline: none;
          }
          .form-hint {
            font-size: 11px;
            color: var(--color-text-quaternary);
            margin-top: 4px;
          }
          .extract-btn {
            display: inline-flex;
            align-items: center;
            gap: 4px;
            padding: 3px 8px;
            border-radius: var(--radius-xs);
            font-size: 11px;
            font-weight: 500;
            background: var(--color-accent-light);
            color: var(--color-accent);
            border: none;
            cursor: pointer;
            transition: all 0.15s;
          }
          .extract-btn:hover {
            background: var(--color-accent);
            color: white;
          }
          .variables-list {
            display: flex;
            flex-direction: column;
            gap: 8px;
          }
          .variable-row {
            display: flex;
            align-items: center;
            gap: 6px;
          }
          .var-input {
            padding: 6px 8px;
            border: 1px solid var(--color-border);
            border-radius: var(--radius-xs);
            font-size: 12px;
            background: var(--color-bg);
            color: var(--color-text-primary);
          }
          .var-input:focus {
            border-color: var(--color-accent);
            outline: none;
          }
          .var-input-name { width: 100px; }
          .var-input-label { width: 100px; }
          .var-input-options { width: 140px; }
          .var-select {
            padding: 6px 8px;
            border: 1px solid var(--color-border);
            border-radius: var(--radius-xs);
            font-size: 12px;
            background: var(--color-bg);
            color: var(--color-text-primary);
          }
          .var-remove-btn {
            width: 24px;
            height: 24px;
            display: flex;
            align-items: center;
            justify-content: center;
            border-radius: var(--radius-xs);
            color: var(--color-text-quaternary);
            transition: all 0.15s;
            flex-shrink: 0;
          }
          .var-remove-btn:hover {
            background: var(--color-error-light, rgba(239,68,68,0.1));
            color: var(--color-error, #ef4444);
          }
          .add-var-btn {
            display: inline-flex;
            align-items: center;
            gap: 4px;
            padding: 6px 12px;
            border-radius: var(--radius-sm);
            font-size: 12px;
            font-weight: 500;
            background: var(--color-bg-sub);
            color: var(--color-text-secondary);
            border: 1px dashed var(--color-border);
            cursor: pointer;
            transition: all 0.15s;
            margin-top: 8px;
          }
          .add-var-btn:hover {
            border-color: var(--color-accent);
            color: var(--color-accent);
            background: var(--color-accent-light);
          }
          .dialog-footer {
            padding: 16px 20px;
            border-top: 1px solid var(--color-border-light);
            display: flex;
            justify-content: flex-end;
            gap: 8px;
            flex-shrink: 0;
          }
          .btn-cancel {
            padding: 8px 16px;
            border-radius: var(--radius-sm);
            font-size: 13px;
            font-weight: 500;
            background: var(--color-bg-sub);
            color: var(--color-text-secondary);
            border: none;
            cursor: pointer;
            transition: all 0.15s;
          }
          .btn-cancel:hover {
            background: var(--color-bg-hover);
          }
          .btn-save {
            padding: 8px 16px;
            border-radius: var(--radius-sm);
            font-size: 13px;
            font-weight: 500;
            background: var(--color-accent);
            color: white;
            border: none;
            cursor: pointer;
            transition: all 0.15s;
          }
          .btn-save:hover:not(:disabled) {
            background: var(--color-accent-hover);
          }
          .btn-save:disabled {
            opacity: 0.5;
            cursor: not-allowed;
          }
        `}</style>
      </div>
    </div>
  );
}
