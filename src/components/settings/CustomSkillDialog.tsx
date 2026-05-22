import { useState, useEffect } from "react";
import { Icon } from "../common/Icon";
import type { CustomSkillConfig } from "../../types";

const SKILL_CATEGORIES = [
  { value: "document", label: "文档处理" },
  { value: "data", label: "数据处理" },
  { value: "format", label: "格式转换" },
  { value: "custom", label: "自定义" },
];

const DOC_TYPES = [
  { value: "docx", label: "Word" },
  { value: "xlsx", label: "Excel" },
  { value: "pptx", label: "PPT" },
  { value: "pdf", label: "PDF" },
  { value: "md", label: "Markdown" },
];

interface CustomSkillDialogProps {
  open: boolean;
  onClose: () => void;
  /** 编辑模式时传入已有的自定义 Skill 配置 */
  skill?: CustomSkillConfig | null;
  /** 保存回调 */
  onSave: (config: CustomSkillConfig) => Promise<void>;
}

export function CustomSkillDialog({ open, onClose, skill, onSave }: CustomSkillDialogProps) {
  const isEdit = !!skill;

  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [category, setCategory] = useState("custom");
  const [promptTemplate, setPromptTemplate] = useState("");
  const [supportedTypes, setSupportedTypes] = useState<string[]>([]);
  const [saving, setSaving] = useState(false);

  // 编辑模式时填充表单
  useEffect(() => {
    if (skill) {
      setName(skill.name);
      setDescription(skill.description);
      setCategory(skill.category);
      setPromptTemplate(skill.promptTemplate);
      setSupportedTypes(skill.supportedTypes);
    } else {
      setName("");
      setDescription("");
      setCategory("custom");
      setPromptTemplate("");
      setSupportedTypes([]);
    }
  }, [skill, open]);

  // 切换文档类型选择
  const toggleDocType = (docType: string) => {
    setSupportedTypes((prev) =>
      prev.includes(docType)
        ? prev.filter((t) => t !== docType)
        : [...prev, docType]
    );
  };

  // 保存自定义 Skill
  const handleSave = async () => {
    if (!name.trim() || !promptTemplate.trim()) return;
    setSaving(true);
    try {
      const now = new Date().toISOString();
      const config: CustomSkillConfig = {
        id: skill?.id ?? "",
        name: name.trim(),
        description: description.trim(),
        category,
        promptTemplate: promptTemplate.trim(),
        supportedTypes,
        paramsSchema: skill?.paramsSchema ?? undefined,
        version: skill?.version ?? "1.0.0",
        createdAt: skill?.createdAt ?? now,
        updatedAt: now,
      };
      await onSave(config);
      onClose();
    } catch (err) {
      console.error("[CustomSkillDialog] 保存失败:", err);
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
      <div className="custom-skill-dialog" onClick={(e) => e.stopPropagation()}>
        {/* 标题栏 */}
        <div className="dialog-header">
          <h3 className="dialog-title">{isEdit ? "编辑自定义 Skill" : "创建自定义 Skill"}</h3>
          <button className="dialog-close-btn" onClick={onClose}>
            <Icon name="close" size={16} />
          </button>
        </div>

        {/* 表单内容 */}
        <div className="dialog-body">
          {/* Skill 名称 */}
          <div className="form-group">
            <label className="form-label">Skill 名称 <span className="form-required">*</span></label>
            <input
              className="form-input"
              placeholder="例如：合同生成器"
              value={name}
              onChange={(e) => setName(e.target.value)}
            />
            <div className="form-hint">名称将作为 LLM 调用此 Skill 时的工具标识</div>
          </div>

          {/* 描述 */}
          <div className="form-group">
            <label className="form-label">描述</label>
            <input
              className="form-input"
              placeholder="简要描述 Skill 的功能，LLM 根据此描述决定是否调用"
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
              {SKILL_CATEGORIES.map((c) => (
                <option key={c.value} value={c.value}>{c.label}</option>
              ))}
            </select>
          </div>

          {/* 支持的文档类型 */}
          <div className="form-group">
            <label className="form-label">支持的文档类型</label>
            <div className="doc-types-row">
              {DOC_TYPES.map((dt) => (
                <button
                  key={dt.value}
                  className={`doc-type-chip ${supportedTypes.includes(dt.value) ? "active" : ""}`}
                  onClick={() => toggleDocType(dt.value)}
                >
                  {dt.label}
                </button>
              ))}
            </div>
            <div className="form-hint">选择此 Skill 可以处理的文档格式，不选则表示不限格式</div>
          </div>

          {/* 提示词模板 */}
          <div className="form-group">
            <label className="form-label">提示词模板 <span className="form-required">*</span></label>
            <textarea
              className="form-textarea"
              placeholder={"输入提示词模板，使用 {{参数名}} 定义可替换的占位符\n\n例如：\n你是一位专业的{{domain}}专家。请根据以下要求生成一份{{docType}}文档：\n主题：{{topic}}\n要求：{{requirements}}\n请确保内容专业、结构清晰、语言规范。"}
              rows={8}
              value={promptTemplate}
              onChange={(e) => setPromptTemplate(e.target.value)}
            />
            <div className="form-hint">
              使用 {"{{参数名}}"} 语法定义参数占位符。LLM 调用此 Skill 时，参数会被替换到模板中，
              渲染后的提示词作为工具结果返回给 LLM，指导其后续行为。
            </div>
          </div>

          {/* 参数预览 */}
          {promptTemplate && (
            <div className="form-group">
              <label className="form-label">自动识别的参数</label>
              <div className="params-preview">
                {extractParams(promptTemplate).length > 0 ? (
                  extractParams(promptTemplate).map((p) => (
                    <span key={p} className="param-tag">{p}</span>
                  ))
                ) : (
                  <span className="no-params">模板中未检测到参数占位符</span>
                )}
              </div>
            </div>
          )}
        </div>

        {/* 底部按钮 */}
        <div className="dialog-footer">
          <button className="btn-cancel" onClick={onClose}>取消</button>
          <button
            className="btn-save"
            onClick={handleSave}
            disabled={!name.trim() || !promptTemplate.trim() || saving}
          >
            {saving ? "保存中..." : isEdit ? "保存修改" : "创建 Skill"}
          </button>
        </div>

        <style>{`
          .custom-skill-dialog {
            width: 620px;
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
            min-height: 160px;
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
          .doc-types-row {
            display: flex;
            gap: 6px;
            flex-wrap: wrap;
          }
          .doc-type-chip {
            padding: 4px 12px;
            border-radius: var(--radius-sm);
            font-size: 12px;
            font-weight: 500;
            background: var(--color-bg-sub);
            color: var(--color-text-secondary);
            border: 1px solid var(--color-border);
            cursor: pointer;
            transition: all 0.15s;
          }
          .doc-type-chip:hover {
            border-color: var(--color-accent);
            color: var(--color-accent);
          }
          .doc-type-chip.active {
            background: var(--color-accent-light);
            color: var(--color-accent);
            border-color: var(--color-accent);
          }
          .params-preview {
            display: flex;
            gap: 6px;
            flex-wrap: wrap;
            min-height: 28px;
          }
          .param-tag {
            padding: 2px 10px;
            border-radius: 10px;
            font-size: 11px;
            font-weight: 500;
            font-family: var(--font-mono);
            background: var(--color-accent-light);
            color: var(--color-accent);
          }
          .no-params {
            font-size: 12px;
            color: var(--color-text-quaternary);
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

/** 从提示词模板中提取 {{param}} 占位符参数名 */
function extractParams(template: string): string[] {
  const matches = template.match(/\{\{(\w+)\}\}/g);
  if (!matches) return [];
  // 去重并保持顺序
  const seen = new Set<string>();
  return matches
    .map((m) => m.slice(2, -2))
    .filter((p) => {
      if (seen.has(p)) return false;
      seen.add(p);
      return true;
    });
}
