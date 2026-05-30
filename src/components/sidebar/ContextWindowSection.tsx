import { SidebarSection } from "../layout/Sidebar";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { useSettingsStore } from "../../stores/useSettingsStore";

/** 格式化 Token 数量为可读字符串 */
function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}M`;
  }
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}K`;
  }
  return String(tokens);
}

/** 根据压缩状态返回对应的显示信息 */
function getCompressionInfo(status: string): { label: string; color: string } {
  switch (status) {
    case "critical":
      return { label: "接近上限", color: "var(--color-error)" };
    case "compressed":
      return { label: "已压缩", color: "var(--color-warning)" };
    default:
      return { label: "正常", color: "var(--color-success)" };
  }
}

/** 各部分定义：标签、颜色变量名、对应字段 */
const SECTIONS = [
  { key: "system", label: "系统提示词", colorVar: "--color-context-system" },
  { key: "functions", label: "工具定义", colorVar: "--color-context-functions" },
  { key: "history", label: "对话历史", colorVar: "--color-context-history" },
  { key: "response", label: "LLM 响应", colorVar: "--color-context-response" },
] as const;

export function ContextWindowSection() {
  // Agent 运行时从 useWorkflowStore 获取实时上下文使用数据
  const { contextUsage } = useWorkflowStore();
  // Agent 未运行时从 useSettingsStore 获取静态 Provider 信息
  const providers = useSettingsStore((s) => s.llmProviders);

  // 无 Provider 时不渲染
  const defaultProvider = providers.find((p) => p.isDefault);
  if (!contextUsage && !defaultProvider) return null;

  // 有实时数据时显示详细使用情况
  if (contextUsage) {
    const {
      contextWindow,
      systemPromptTokens,
      functionDefinitionsTokens,
      conversationTokens,
      responseTokens,
      totalUsedTokens,
      compressionStatus,
      totalMessageCount,
      retainedMessageCount,
    } = contextUsage;

    const usagePercent = contextWindow > 0 ? Math.round((totalUsedTokens / contextWindow) * 100) : 0;
    const compressionInfo = getCompressionInfo(compressionStatus);

    // 计算各部分占比（用于总览分段横条）
    const systemPct = contextWindow > 0 ? (systemPromptTokens / contextWindow) * 100 : 0;
    const funcPct = contextWindow > 0 ? (functionDefinitionsTokens / contextWindow) * 100 : 0;
    const convPct = contextWindow > 0 ? (conversationTokens / contextWindow) * 100 : 0;
    const respPct = contextWindow > 0 ? (responseTokens / contextWindow) * 100 : 0;

    // 各部分的 token 数（用于总览横条 title）
    const sectionTokens = [systemPromptTokens, functionDefinitionsTokens, conversationTokens, responseTokens];

    return (
      <SidebarSection title="上下文窗口">
        <div className="cw-grid" role="region" aria-label="上下文窗口使用信息">
          {/* 总览分段横条：系统/工具/历史/响应 四色横条 */}
          <div className="cw-bar-container">
            <div className="cw-bar-track">
              {SECTIONS.map((section, i) => (
                <div
                  key={section.key}
                  className="cw-bar-segment"
                  style={{ width: `${[systemPct, funcPct, convPct, respPct][i]}%`, background: `var(${section.colorVar})` }}
                  title={`${section.label}: ${formatTokens(sectionTokens[i])} (${[systemPct, funcPct, convPct, respPct][i].toFixed(1)}%)`}
                />
              ))}
            </div>
            <div className="cw-bar-labels">
              <span className="cw-usage-label" style={{ color: compressionInfo.color }}>
                {compressionInfo.label}
              </span>
              <span className="cw-usage-percent">{usagePercent}%</span>
            </div>
          </div>

          {/* 各部分：圆点 + 名称 + 使用量 */}
          <div className="cw-sections">
            {SECTIONS.map((section, i) => (
              <div className="cw-section-row" key={section.key}>
                <span className="cw-section-label">
                  <span className="cw-dot" style={{ background: `var(${section.colorVar})` }} />
                  {section.label}
                </span>
                <span className="cw-section-value">{formatTokens(sectionTokens[i])}</span>
              </div>
            ))}
          </div>

          {/* 总计行 */}
          <div className="cw-token-total">
            <span className="cw-total-label">已使用 / 总窗口</span>
            <span className="cw-total-value" style={compressionStatus === "critical" ? { color: "var(--color-error)" } : undefined}>
              {formatTokens(totalUsedTokens)} / {formatTokens(contextWindow)}
            </span>
          </div>

          {/* 压缩状态标记 */}
          {(compressionStatus === "compressed" || compressionStatus === "critical") && (
            <div className="cw-compressed-badge">
              <span className="cw-compressed-dot" />
              <span>
                {compressionStatus === "critical"
                  ? `上下文接近上限 (${retainedMessageCount}/${totalMessageCount} 消息)`
                  : `已压缩历史 (保留 ${retainedMessageCount}/${totalMessageCount} 消息)`}
              </span>
            </div>
          )}
        </div>

        <CWStyles />
      </SidebarSection>
    );
  }

  // Agent 未运行时，显示静态 Provider 信息（仅窗口大小，无模型名称）
  return (
    <SidebarSection title="上下文窗口">
      <div className="cw-grid">
        {/* 空状态总览横条 */}
        <div className="cw-bar-container">
          <div className="cw-bar-track">
            <div className="cw-bar-segment" style={{ width: 0 }} />
          </div>
          <div className="cw-bar-labels">
            <span className="cw-usage-label" style={{ color: "var(--color-text-quaternary)" }}>
              未使用
            </span>
            <span className="cw-usage-percent">0%</span>
          </div>
        </div>

        {/* 窗口大小 */}
        <div className="cw-token-total">
          <span className="cw-total-label">总窗口</span>
          <span className="cw-total-value">{formatTokens(defaultProvider!.contextWindow)} tokens</span>
        </div>
      </div>

      <CWStyles />
    </SidebarSection>
  );
}

/** 上下文窗口区域样式，提取为独立组件确保所有分支共享 */
function CWStyles() {
  return (
    <style>{`
      .cw-grid {
        display: flex;
        flex-direction: column;
        gap: 4px;
      }

      /* ===== 总览分段横条 ===== */
      .cw-bar-container {
        display: flex;
        flex-direction: column;
        gap: 2px;
      }
      .cw-bar-track {
        height: 4px;
        background: var(--color-context-idle);
        border-radius: 2px;
        overflow: hidden;
        display: flex;
      }
      .cw-bar-segment {
        height: 100%;
        transition: width 0.5s ease;
        min-width: 0;
      }
      .cw-bar-labels {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      .cw-usage-label {
        font-size: 10px;
        font-weight: 500;
      }
      .cw-usage-percent {
        font-size: 10px;
        color: var(--color-text-quaternary);
        font-variant-numeric: tabular-nums;
      }

      /* ===== 各部分行：圆点 + 名称 + 使用量 ===== */
      .cw-sections {
        display: flex;
        flex-direction: column;
        gap: 2px;
      }
      .cw-section-row {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      .cw-section-label {
        font-size: 10px;
        color: var(--color-text-quaternary);
        display: flex;
        align-items: center;
        gap: 4px;
      }
      .cw-section-value {
        font-size: 10px;
        font-weight: 500;
        color: var(--color-text-quaternary);
        font-variant-numeric: tabular-nums;
      }
      .cw-dot {
        width: 5px;
        height: 5px;
        border-radius: 50%;
        flex-shrink: 0;
      }

      /* ===== 总计行 ===== */
      .cw-token-total {
        display: flex;
        justify-content: space-between;
        align-items: center;
      }
      .cw-total-label {
        font-size: 10px;
        color: var(--color-text-quaternary);
        font-weight: 500;
      }
      .cw-total-value {
        font-size: 10px;
        font-weight: 500;
        color: var(--color-text-quaternary);
        font-variant-numeric: tabular-nums;
      }

      /* ===== 压缩状态标记 ===== */
      .cw-compressed-badge {
        display: flex;
        align-items: center;
        gap: 4px;
        padding: 3px 6px;
        background: var(--color-warning-bg, rgba(250, 173, 20, 0.1));
        border-radius: var(--radius-sm);
        font-size: 10px;
        color: var(--color-warning, #faad14);
      }
      .cw-compressed-dot {
        width: 5px;
        height: 5px;
        border-radius: 50%;
        background: var(--color-warning, #faad14);
        flex-shrink: 0;
      }
    `}</style>
  );
}
