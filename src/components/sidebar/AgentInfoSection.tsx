import { useState } from "react";
import { SidebarSection } from "../layout/Sidebar";
import { useSettingsStore } from "../../stores/useSettingsStore";

const confirmationLevelLabels: Record<string, string> = {
  always: "全部需确认",
  editOnly: "仅编辑操作确认",
  never: "全部自动确认",
};

export function AgentInfoSection() {
  const { settings, llmProviders, activeProviderId, updateSettings } = useSettingsStore();
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
    <SidebarSection title="Agent 信息">
      <div className="flex flex-col gap-[10px]">
        {/* 当前模型 */}
        <div className="flex items-center justify-between">
          <span className="text-[12px] text-text-tertiary">当前模型</span>
          <div className="flex items-center gap-[6px] px-[10px] py-1 bg-bg rounded-[var(--radius-sm)] text-[12px] font-medium">
            <span className={`w-[6px] h-[6px] rounded-full ${activeProvider ? "bg-success" : "bg-text-tertiary"}`} />
            {activeProvider?.model ?? "未配置"}
          </div>
        </div>

        {/* Provider */}
        <div className="flex items-center justify-between">
          <span className="text-[12px] text-text-tertiary">Provider</span>
          <span className="text-[13px] font-medium text-text-primary">
            {activeProvider?.providerType ?? "未配置"}
          </span>
        </div>

        {/* 作者名 */}
        <div className="flex items-center justify-between">
          <span className="text-[12px] text-text-tertiary">作者名</span>
          {editing ? (
            <input
              className="text-[13px] font-medium text-text-primary px-2 py-[2px] border border-border rounded-[4px] w-20"
              value={editValue}
              onChange={(e) => setEditValue(e.target.value)}
              onBlur={handleSave}
              onKeyDown={(e) => { if (e.key === "Enter") handleSave(); }}
              autoFocus
            />
          ) : (
            <span
              className="text-[13px] font-medium text-text-primary px-2 py-[2px] rounded-[4px] cursor-pointer border border-transparent transition-all duration-150 hover:border-border hover:bg-bg"
              onClick={() => { setEditValue(settings.general.authorName); setEditing(true); }}
            >
              {settings.general.authorName || "未设置"}
            </span>
          )}
        </div>

        {/* 确认级别 */}
        <div className="flex items-center justify-between">
          <span className="text-[12px] text-text-tertiary">确认级别</span>
          <span className="text-[13px] font-medium text-text-primary">
            {confirmationLevelLabels[settings.general.confirmationLevel] ?? settings.general.confirmationLevel}
          </span>
        </div>
      </div>
    </SidebarSection>
  );
}
