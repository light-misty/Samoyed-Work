import { useSettingsStore } from "../../stores/useSettingsStore";

export function GeneralTab() {
  const { settings, updateSettings } = useSettingsStore();

  return (
    <div>
      {/* 基本设置 */}
      <div className="mb-6">
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">基本设置</div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">作者名（全局默认）</div>
            <div className="text-[11px] text-text-tertiary mt-[2px]">生成文档时自动填充的作者元数据</div>
          </div>
          <input
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] w-[160px] transition-colors focus:border-accent"
            value={settings.general.authorName}
            onChange={(e) => updateSettings({ general: { authorName: e.target.value } })}
          />
        </div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">操作确认级别</div>
            <div className="text-[11px] text-text-tertiary mt-[2px]">Agent执行文件操作时的确认策略</div>
          </div>
          <select
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg cursor-pointer"
            value={settings.general.confirmationLevel}
            onChange={(e) => updateSettings({ general: { confirmationLevel: e.target.value as typeof settings.general.confirmationLevel } })}
          >
            <option value="always">全部需确认</option>
            <option value="editOnly">仅编辑操作确认</option>
            <option value="never">全部自动确认</option>
          </select>
        </div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">语言</div>
          </div>
          <select
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg cursor-pointer"
            value={settings.general.language}
            onChange={(e) => updateSettings({ general: { language: e.target.value } })}
          >
            <option value="zh-CN">简体中文</option>
            <option value="en-US">English</option>
          </select>
        </div>
      </div>

      {/* Token 预算 */}
      <div className="mb-6">
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">Token 预算</div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">日预算上限</div>
            <div className="text-[11px] text-text-tertiary mt-[2px]">超出时触发提醒</div>
          </div>
          <input
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] w-[120px] transition-colors focus:border-accent"
            placeholder="不限制"
            value={settings.tokenBudget.dailyLimit || ""}
            onChange={(e) => updateSettings({ tokenBudget: { dailyLimit: Number(e.target.value) || 0 } })}
          />
        </div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">月预算上限</div>
          </div>
          <input
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] w-[120px] transition-colors focus:border-accent"
            placeholder="不限制"
            value={settings.tokenBudget.monthlyLimit || ""}
            onChange={(e) => updateSettings({ tokenBudget: { monthlyLimit: Number(e.target.value) || 0 } })}
          />
        </div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">超出预算行为</div>
          </div>
          <select
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg cursor-pointer"
            value={settings.tokenBudget.exceedAction}
            onChange={(e) => updateSettings({ tokenBudget: { exceedAction: e.target.value as typeof settings.tokenBudget.exceedAction } })}
          >
            <option value="warn">仅提醒</option>
            <option value="block">暂停Agent</option>
            <option value="fallback">切换到更便宜的模型</option>
          </select>
        </div>
      </div>

      {/* 版本快照 */}
      <div className="mb-6">
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">版本快照</div>

        <div className="flex items-center justify-between py-[10px] border-b border-border-light">
          <div>
            <div className="text-[13px] text-text-primary">保留策略</div>
          </div>
          <select
            className="px-[10px] py-[6px] border border-border rounded-[var(--radius-sm)] text-[13px] bg-bg cursor-pointer"
            value={settings.versionSnapshot.retentionPolicy}
            onChange={(e) => updateSettings({ versionSnapshot: { retentionPolicy: e.target.value as typeof settings.versionSnapshot.retentionPolicy } })}
          >
            <option value="byCount">按数量（最近{settings.versionSnapshot.maxCount}个）</option>
            <option value="byDays">按时间（最近{settings.versionSnapshot.maxDays}天）</option>
            <option value="both">两者都满足</option>
          </select>
        </div>

        <div className="flex items-center justify-between py-[10px]">
          <div>
            <div className="text-[13px] text-text-primary">应用数据目录</div>
            <div className="text-[11px] text-text-tertiary mt-[2px]">快照和配置的存储位置</div>
          </div>
          <span className="text-[12px] text-text-tertiary font-mono">%APPDATA%/DocAgent</span>
        </div>
      </div>
    </div>
  );
}
