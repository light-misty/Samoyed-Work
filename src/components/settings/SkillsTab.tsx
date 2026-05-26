import { useState, useEffect, useCallback } from "react";
import { useSettingsStore } from "../../stores/useSettingsStore";
import { Icon } from "../common/Icon";
import { CustomSkillDialog } from "./CustomSkillDialog";
import { DeleteConfirmDialog } from "../common/DeleteConfirmDialog";
import { addCustomSkill, updateCustomSkill, deleteCustomSkill, listCustomSkills } from "../../services/tauri";
import type { CustomSkillConfig } from "../../types";

export function SkillsTab() {
  const { skills, tools, toggleSkill, refreshSkills } = useSettingsStore();
  const [customSkills, setCustomSkills] = useState<CustomSkillConfig[]>([]);
  const [dialogOpen, setDialogOpen] = useState(false);
  const [editingSkill, setEditingSkill] = useState<CustomSkillConfig | null>(null);
  const [deleteTarget, setDeleteTarget] = useState<CustomSkillConfig | null>(null);

  // 加载自定义 Skill 列表
  const loadCustomSkills = useCallback(async () => {
    try {
      const configs = await listCustomSkills();
      setCustomSkills(configs);
    } catch (err) {
      console.error("[SkillsTab] 加载自定义 Skill 失败:", err);
    }
  }, []);

  useEffect(() => {
    loadCustomSkills();
  }, [loadCustomSkills]);

  // 打开创建对话框
  const handleCreate = () => {
    setEditingSkill(null);
    setDialogOpen(true);
  };

  // 打开编辑对话框
  const handleEdit = (skill: CustomSkillConfig) => {
    setEditingSkill(skill);
    setDialogOpen(true);
  };

  // 保存自定义 Skill（创建或更新）
  const handleSave = async (config: CustomSkillConfig) => {
    if (editingSkill) {
      await updateCustomSkill(config);
    } else {
      await addCustomSkill(config);
    }
    await loadCustomSkills();
    await refreshSkills();
  };

  // 确认删除
  const handleDeleteConfirm = async () => {
    if (!deleteTarget) return;
    try {
      await deleteCustomSkill(deleteTarget.id);
      await loadCustomSkills();
      await refreshSkills();
    } catch (err) {
      console.error("[SkillsTab] 删除自定义 Skill 失败:", err);
    } finally {
      setDeleteTarget(null);
    }
  };

  // 内置 Skill 列表
  const builtinSkills = skills.filter((s) => s.isBuiltin);
  // 自定义 Skill 列表（从 registry 同步）
  const customSkillInfos = skills.filter((s) => !s.isBuiltin);

  return (
    <div>
      {/* 内置 Tools */}
      <div className="section-header">
        <span className="section-title">内置 Tools</span>
        <span className="section-badge">{tools.length}</span>
      </div>

      <div className="skills-list">
        {tools.map((t) => (
          <div key={t.id} className="skill-item">
            <div className="skill-item-info">
              <div className="skill-name-row">
                <span className="skill-name">{t.name}</span>
                <span className="skill-tool-badge">Tool</span>
              </div>
              <div className="skill-desc">{t.description}</div>
            </div>
            <div className="tool-always-on">
              始终启用
            </div>
          </div>
        ))}
      </div>

      {/* 内置 Skills */}
      <div className="section-header" style={{ marginTop: 24 }}>
        <span className="section-title">内置 Skills</span>
        <span className="section-badge">{builtinSkills.length}</span>
      </div>

      <div className="skills-list">
        {builtinSkills.map((s) => (
          <div key={s.id} className="skill-item">
            <div className="skill-item-info">
              <div className="skill-name-row">
                <span className="skill-name">{s.name}</span>
                <span className="skill-skill-badge">Skill</span>
              </div>
              <div className="skill-desc">{s.description}</div>
            </div>
            <div className="tool-always-on">
              始终启用
            </div>
          </div>
        ))}
      </div>

      {/* 自定义 Skills */}
      <div className="custom-skills-section">
        <div className="section-header">
          <span className="section-title">自定义 Skills</span>
          {customSkills.length > 0 && (
            <span className="section-badge">{customSkills.length}</span>
          )}
          <button className="add-btn" onClick={handleCreate}>
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2"><line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" /></svg>
            创建自定义 Skill
          </button>
        </div>

        {customSkills.length > 0 ? (
          <div className="skills-list">
            {customSkills.map((cs) => {
              const registryInfo = customSkillInfos.find((s) => s.id === cs.id);
              const enabled = registryInfo?.enabled ?? true;
              return (
                <div key={cs.id} className="skill-item custom-skill-item">
                  <div className="skill-item-info">
                    <div className="skill-name-row">
                      <span className="skill-name">{cs.name}</span>
                      <span className="skill-custom-badge">自定义</span>
                    </div>
                    <div className="skill-desc">{cs.description}</div>
                    {cs.supportedTypes.length > 0 && (
                      <div className="skill-types">
                        {cs.supportedTypes.map((t) => (
                          <span key={t} className="skill-type-tag">{t}</span>
                        ))}
                      </div>
                    )}
                  </div>
                  <div className="skill-actions">
                    <label className="toggle-switch">
                      <input
                        type="checkbox"
                        className="toggle-input"
                        checked={enabled}
                        onChange={() => toggleSkill(cs.id)}
                      />
                      <span className="toggle-track" />
                      <span className="toggle-thumb" />
                    </label>
                    <button
                      className="skill-action-btn"
                      title="编辑"
                      onClick={() => handleEdit(cs)}
                    >
                      <Icon name="edit" size={14} />
                    </button>
                    <button
                      className="skill-action-btn skill-action-btn-danger"
                      title="删除"
                      onClick={() => setDeleteTarget(cs)}
                    >
                      <Icon name="trash" size={14} />
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        ) : (
          <div className="empty-state-lg">
            <div className="empty-icon">
              <Icon name="template" size={32} />
            </div>
            <div>暂无自定义 Skill</div>
            <div className="empty-desc">创建自定义 Skill 来扩展 Agent 的能力</div>
          </div>
        )}

      </div>

      {/* 自定义 Skill 编辑对话框 */}
      <CustomSkillDialog
        open={dialogOpen}
        onClose={() => setDialogOpen(false)}
        skill={editingSkill}
        onSave={handleSave}
      />

      {/* 删除确认对话框 */}
      {deleteTarget && (
        <DeleteConfirmDialog
          name={`自定义 Skill "${deleteTarget.name}"`}
          isDir={false}
          onConfirm={handleDeleteConfirm}
          onCancel={() => setDeleteTarget(null)}
        />
      )}

      <style>{`
        .section-header .add-btn {
          margin-left: auto;
        }
        .skills-list {
          display: flex;
          flex-direction: column;
          margin-bottom: 24px;
        }
        .skill-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          padding: 10px 12px;
          border-bottom: 1px solid var(--color-border-light);
          transition: background 0.15s;
        }
        .skill-item:hover {
          background: var(--color-accent-bg);
        }
        .skill-item:last-child {
          border-bottom: none;
        }
        .skill-item-info {
          flex: 1;
          min-width: 0;
        }
        .skill-name {
          font-size: 13px;
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .skill-name-row {
          display: flex;
          align-items: center;
          gap: 6px;
        }
        .skill-tool-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-accent-bg);
          color: var(--color-accent);
        }
        .skill-skill-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-purple-light);
          color: var(--color-purple);
        }
        .skill-custom-badge {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-purple-light);
          color: var(--color-purple);
        }
        .tool-always-on {
          font-size: 11px;
          color: var(--color-text-quaternary);
          flex-shrink: 0;
        }
        .skill-desc {
          font-size: 11px;
          color: var(--color-text-quaternary);
          margin-top: 2px;
        }
        .skill-types {
          display: flex;
          gap: 4px;
          margin-top: 4px;
        }
        .skill-type-tag {
          font-size: 10px;
          font-weight: 500;
          padding: 1px 6px;
          border-radius: 4px;
          background: var(--color-bg-sub);
          color: var(--color-text-quaternary);
        }
        .skill-actions {
          display: flex;
          align-items: center;
          gap: 4px;
          flex-shrink: 0;
        }
        .skill-action-btn {
          width: 28px;
          height: 28px;
          display: flex;
          align-items: center;
          justify-content: center;
          border-radius: var(--radius-sm);
          color: var(--color-text-quaternary);
          transition: all 0.15s;
        }
        .skill-action-btn:hover {
          background: var(--color-bg-sub);
          color: var(--color-text-primary);
        }
        .skill-action-btn-danger:hover {
          background: var(--color-error-light);
          color: var(--color-error);
        }
        .toggle-switch {
          position: relative;
          display: inline-block;
          width: 36px;
          height: 20px;
          cursor: pointer;
          flex-shrink: 0;
        }
        .toggle-input {
          position: absolute;
          opacity: 0;
          width: 0;
          height: 0;
        }
        .toggle-track {
          position: absolute;
          inset: 0;
          background: var(--color-border-strong);
          border-radius: 10px;
          transition: background 0.2s;
        }
        .toggle-input:checked + .toggle-track {
          background: var(--color-accent);
        }
        .toggle-thumb {
          position: absolute;
          top: 2px;
          left: 2px;
          width: 16px;
          height: 16px;
          background: white;
          border-radius: 50%;
          transition: transform 0.2s;
          box-shadow: 0 1px 3px rgba(0,0,0,0.15);
        }
        .toggle-input:checked ~ .toggle-thumb {
          transform: translateX(16px);
        }
        .custom-skills-section {
          margin-top: 24px;
        }
        .empty-state-lg {
          font-size: 13px;
          color: var(--color-text-quaternary);
          text-align: center;
          padding: 24px 16px;
        }
        .empty-icon {
          margin-bottom: 8px;
          opacity: 0.4;
        }
        .empty-desc {
          font-size: 11px;
          margin-top: 4px;
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
