import { useState, useMemo } from "react";
import { useTranslation } from "react-i18next";
import { Icon } from "../common/Icon";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { TemplateEditDialog } from "./TemplateEditDialog";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import type { PromptTemplate } from "../../types";

export function TemplatesTab() {
  const { t } = useTranslation();
  const { templates, deleteTemplate, closeSettings, setPendingInsertTemplate } = useSettingsStore();
  const [editOpen, setEditOpen] = useState(false);
  const [editingTemplate, setEditingTemplate] = useState<PromptTemplate | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<PromptTemplate | null>(null);
  const [activeCategory, setActiveCategory] = useState<string>("all");
  const [searchQuery, setSearchQuery] = useState("");

  // 分类标签映射
  const CATEGORY_LABELS: Record<string, string> = {
    document: t('settings.templates.categoryDocument'),
    analysis: t('settings.templates.categoryAnalysis'),
    conversion: t('settings.templates.categoryConversion'),
    custom: t('settings.templates.categoryCustom'),
  };

  // 按分类和搜索过滤模板
  const filteredTemplates = useMemo(() => {
    let result = templates;
    if (activeCategory !== "all") {
      result = result.filter((tpl) => tpl.category === activeCategory);
    }
    if (searchQuery.trim()) {
      const q = searchQuery.toLowerCase();
      result = result.filter(
        (tpl) =>
          tpl.name.toLowerCase().includes(q) ||
          tpl.description.toLowerCase().includes(q) ||
          tpl.content.toLowerCase().includes(q)
      );
    }
    return result;
  }, [templates, activeCategory, searchQuery]);

  // 内置模板和自定义模板分组
  const builtinTemplates = filteredTemplates.filter((tpl) => tpl.isBuiltin);
  const customTemplates = filteredTemplates.filter((tpl) => !tpl.isBuiltin);

  // 打开创建对话框
  const handleCreate = () => {
    setEditingTemplate(null);
    setEditOpen(true);
  };

  // 打开编辑对话框
  const handleEdit = (template: PromptTemplate) => {
    setEditingTemplate(template);
    setEditOpen(true);
  };

  // 删除模板
  const handleDelete = async () => {
    if (!deleteTarget) return;
    await deleteTemplate(deleteTarget.id);
    setDeleteTarget(null);
  };

  // 使用模板：插入到输入框并关闭设置
  const handleUse = (tpl: PromptTemplate) => {
    setPendingInsertTemplate(tpl.content);
    closeSettings();
  };

  return (
    <div>
      {/* 搜索栏 */}
      <div className="template-search">
        <Icon name="search" size={14} />
        <input
          className="template-search-input"
          placeholder={t('settings.templates.searchPlaceholder')}
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
      </div>

      {/* 分类标签 */}
      <div className="template-categories">
        <button
          className={`category-tag ${activeCategory === "all" ? "active" : ""}`}
          onClick={() => setActiveCategory("all")}
        >
          {t('settings.templates.all')}
        </button>
        {Object.entries(CATEGORY_LABELS).map(([key, label]) => (
          <button
            key={key}
            className={`category-tag ${activeCategory === key ? "active" : ""}`}
            onClick={() => setActiveCategory(key)}
          >
            {label}
          </button>
        ))}
      </div>

      {/* 自定义模板区域 */}
      <div className="template-section">
        <div className="section-header">
          <span className="section-title">{t('settings.templates.customTemplates')}</span>
          <span className="section-badge">{customTemplates.length}</span>
          <button className="add-btn" onClick={handleCreate}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
            {t('settings.templates.createTemplate')}
          </button>
        </div>

        {customTemplates.length > 0 ? (
          <div className="template-list">
            {customTemplates.map((tpl) => (
              <div key={tpl.id} className="template-card">
                <div className="template-card-main">
                  <div className="template-name">{tpl.name}</div>
                  <div className="template-desc">{tpl.description}</div>
                  <div className="template-meta">
                    <span className="template-category-badge">
                      {CATEGORY_LABELS[tpl.category] ?? tpl.category}
                    </span>
                    {tpl.variables && tpl.variables.length > 0 && (
                      <span className="template-vars-badge">
                        {t('settings.templates.variableCount', { count: tpl.variables.length })}
                      </span>
                    )}
                  </div>
                </div>
                <div className="template-card-actions">
                  <button
                    className="action-btn action-btn-use"
                    title={t('settings.templates.use')}
                    onClick={() => handleUse(tpl)}
                  >
                    {t('settings.templates.use')}
                  </button>
                  <button
                    className="action-btn"
                    title={t('common.edit')}
                    onClick={() => handleEdit(tpl)}
                  >
                    <Icon name="edit" size={14} />
                  </button>
                  <button
                    className="action-btn action-btn-danger"
                    title={t('common.delete')}
                    onClick={() => setDeleteTarget(tpl)}
                  >
                    <Icon name="trash" size={14} />
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-state">{t('settings.templates.noCustomTemplates')}</div>
        )}

      </div>

      {/* 内置模板区域 */}
      <div className="template-section">
        <div className="section-header">
          <span className="section-title">{t('settings.templates.builtinTemplates')}</span>
          <span className="section-badge">{builtinTemplates.length}</span>
        </div>

        {builtinTemplates.length > 0 ? (
          <div className="template-list">
            {builtinTemplates.map((tpl) => (
              <div key={tpl.id} className="template-card template-card-builtin">
                <div className="template-card-main">
                  <div className="template-name">
                    {tpl.name}
                    <span className="builtin-tag">{t('settings.templates.builtin')}</span>
                  </div>
                  <div className="template-desc">{tpl.description}</div>
                  <div className="template-meta">
                    <span className="template-category-badge">
                      {CATEGORY_LABELS[tpl.category] ?? tpl.category}
                    </span>
                  </div>
                </div>
                <div className="template-card-actions">
                  <button
                    className="action-btn action-btn-use"
                    title={t('settings.templates.use')}
                    onClick={() => handleUse(tpl)}
                  >
                    {t('settings.templates.use')}
                  </button>
                </div>
              </div>
            ))}
          </div>
        ) : (
          <div className="empty-state">{t('settings.templates.noBuiltinTemplates')}</div>
        )}
      </div>

      {/* 编辑/创建对话框 */}
      <TemplateEditDialog
        open={editOpen}
        onClose={() => setEditOpen(false)}
        template={editingTemplate}
      />

      {/* 删除确认对话框 */}
      {deleteTarget && (
        <DeleteConfirmDialog
          name={deleteTarget.name}
          isDir={false}
          onConfirm={handleDelete}
          onCancel={() => setDeleteTarget(null)}
        />
      )}

      <style>{`
        .template-search {
          display: flex;
          align-items: center;
          gap: 8px;
          padding: 8px 12px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          margin-bottom: 16px;
          color: var(--color-text-quaternary);
          transition: all 0.2s;
        }
        .template-search:focus-within {
          border-color: var(--color-accent);
          box-shadow: 0 0 0 2px var(--color-accent-lighter);
        }
        .template-search-input {
          flex: 1;
          border: none;
          outline: none;
          font-size: 13px;
          background: transparent;
          color: var(--color-text-primary);
        }
        .template-search-input::placeholder {
          color: var(--color-text-quaternary);
        }
        .template-categories {
          display: flex;
          gap: 6px;
          margin-bottom: 20px;
          flex-wrap: wrap;
        }
        .category-tag {
          padding: 4px 12px;
          border-radius: 20px;
          font-size: 12px;
          font-weight: 500;
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
          border: 1px solid transparent;
          cursor: pointer;
          transition: all 0.15s;
        }
        .category-tag:hover {
          background: var(--color-bg-hover);
        }
        .category-tag.active {
          background: var(--color-accent-light);
          color: var(--color-accent);
          border-color: var(--color-accent);
        }
        .template-section {
          margin-bottom: 24px;
        }
        .template-section:last-child {
          margin-bottom: 0;
        }
        .section-header .add-btn {
          margin-left: auto;
        }
        .template-list {
          display: grid;
          grid-template-columns: repeat(2, 1fr);
          gap: 8px;
          margin-bottom: 12px;
        }
        .template-card {
          position: relative;
          padding: 10px 80px 10px 12px;
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          transition: all 0.15s;
          text-align: left;
        }
        .template-card:hover {
          border-color: var(--color-border-strong);
          background: var(--color-bg-sub);
        }
        .template-card-builtin {
          background: var(--color-bg-sub);
          padding-right: 58px;
        }
        .template-card-main {
          min-width: 0;
        }
        .template-name {
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
          margin-bottom: 4px;
          display: flex;
          align-items: center;
          gap: 6px;
          text-align: left;
        }
        .builtin-tag {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 3px;
          background: var(--color-accent-light);
          color: var(--color-accent);
        }
        .template-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-bottom: 6px;
          line-height: 1.4;
        }
        .template-meta {
          display: flex;
          gap: 6px;
        }
        .template-category-badge {
          font-size: 10px;
          padding: 1px 6px;
          border-radius: 3px;
          background: var(--color-bg-sub);
          color: var(--color-text-tertiary);
        }
        .template-vars-badge {
          font-size: 10px;
          padding: 1px 6px;
          border-radius: 3px;
          background: var(--color-bg-sub);
          color: var(--color-text-tertiary);
        }
        .template-card-actions {
          position: absolute;
          top: 8px;
          right: 10px;
          display: flex;
          gap: 4px;
          flex-shrink: 0;
        }
        .action-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-xs);
          color: var(--color-text-quaternary);
          transition: all 0.15s;
        }
        .action-btn:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-secondary);
        }
        .action-btn-danger:hover {
          background: var(--color-error-light, rgba(239,68,68,0.1));
          color: var(--color-error, #ef4444);
        }
        .action-btn-use {
          width: auto;
          padding: 0 10px;
          font-size: 11px;
          font-weight: 500;
          color: var(--color-accent);
          background: var(--color-accent-light);
          border-radius: var(--radius-xs);
        }
        .action-btn-use:hover {
          background: var(--color-accent);
          color: white;
        }
        .empty-state {
          font-size: 13px;
          color: var(--color-text-quaternary);
          text-align: center;
          padding: 20px 16px;
        }
        .add-btn {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          padding: 6px 14px;
          border-radius: var(--radius-sm);
          font-size: 12px;
          font-weight: 500;
          background: var(--color-accent);
          color: white;
          border: none;
          cursor: pointer;
          transition: all 0.15s;
        }
        .add-btn:hover {
          background: var(--color-accent-hover);
        }
      `}</style>
    </div>
  );
}
