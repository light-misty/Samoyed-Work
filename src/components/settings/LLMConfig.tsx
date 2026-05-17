import { useSettingsStore } from "../../stores/useSettingsStore";

export function LLMConfigTab() {
  const { llmProviders } = useSettingsStore();

  return (
    <div>
      <div className="mb-6">
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">已配置的 Provider</div>

        {llmProviders.map((p) => (
          <div key={p.id} className="px-3 py-3 border border-border rounded-[var(--radius-md)] mb-2 transition-colors duration-150 hover:border-[#D0D3D9]">
            <div className="flex items-center gap-2 mb-2">
              <span className="font-semibold text-[13px]">{p.name}</span>
              <span className="text-[10px] font-semibold px-[6px] py-[2px] rounded-[3px] bg-accent-light text-accent uppercase">{p.providerType}</span>
              {p.isDefault && (
                <span className="text-[11px] text-success ml-1">默认</span>
              )}
              <div className="ml-auto flex gap-1">
                <button className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all">编辑</button>
                <button className="px-2 py-[3px] rounded-[var(--radius-sm)] text-[11px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all">测试</button>
              </div>
            </div>
            <div className="font-mono text-[11px] text-text-tertiary">
              {p.model} &nbsp;|&nbsp; {p.apiBase} &nbsp;|&nbsp; {p.isAvailable ? "可用" : "不可用"}
            </div>
          </div>
        ))}

        <button className="mt-2 px-[14px] py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-accent text-white hover:bg-accent-hover transition-all">
          + 添加 Provider
        </button>
      </div>

      <div>
        <div className="text-[13px] font-semibold text-text-secondary uppercase tracking-[.3px] mb-3">Fallback 顺序</div>
        <div className="text-[12px] text-text-secondary leading-[1.8]">
          {llmProviders.map((p, i) => (
            <div key={p.id} className="flex items-center gap-2 py-[6px]">
              <span className="text-accent font-semibold">{i + 1}.</span>
              {p.name}
              {p.isDefault && (
                <span className="text-[10px] px-[6px] py-[2px] bg-success-light text-success rounded-[3px]">默认</span>
              )}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
