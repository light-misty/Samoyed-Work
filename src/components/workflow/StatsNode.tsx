import { useTranslation } from "react-i18next";
import type { WorkflowNode, StatsNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface StatsNodeProps {
  node: WorkflowNode<"stats">;
}

/**
 * Token 用量统计节点
 * 在 Agent 执行过程中展示当前上下文 Token 使用情况
 * - usageInfo 为 null：显示"暂无统计数据"
 * - usageInfo 有值：展示模型、上下文窗口、各类 Token 分项及占用比例
 */
export function StatsNode({ node }: StatsNodeProps) {
  const { t } = useTranslation();
  const data = node.data as StatsNodeData;
  const info = data.usageInfo;

  // 格式化 token 数，便于阅读：< 1000 直接显示，>= 1000 以 k 为单位保留 1 位小数
  const formatTokens = (n: number): string => {
    if (n >= 1000) {
      return `${(n / 1000).toFixed(1)}k`;
    }
    return String(n);
  };

  // 格式化百分比：0.0 - 1.0 的浮点数转为 "xx.x%"
  const formatPercent = (rate: number): string => {
    if (!Number.isFinite(rate)) {
      return "0.0%";
    }
    return `${(rate * 100).toFixed(1)}%`;
  };

  // 计算上下文占用比例，避免除以零
  const usageRatio =
    info && info.contextWindow > 0
      ? info.totalUsedTokens / info.contextWindow
      : 0;
  // 进度条宽度上限 100%，避免溢出
  const progressWidth = Math.min(Math.max(usageRatio * 100, 0), 100);
  // 输入 Token = 系统提示 + 工具定义 + 对话历史
  const inputTokens = info
    ? info.systemPromptTokens + info.functionDefinitionsTokens + info.conversationTokens
    : 0;

  return (
    <div className="wf-node">
      <div className="wf-stats-flat">
        {/* 标题 */}
        <div className="wf-stats-title">
          <Icon name="chart" size={14} />
          <span>{t("slash.stats.title")}</span>
        </div>

        {info === null ? (
          /* 暂无统计数据 */
          <div className="wf-stats-empty">{t("slash.stats.noData")}</div>
        ) : (
          <>
            {/* 概览信息：模型名 + 消息数 */}
            <div className="wf-stats-overview">
              <div className="wf-stats-overview-item">
                <span className="wf-stats-label">{t("slash.stats.model")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {info.modelName || "-"}
                </span>
              </div>
              <div className="wf-stats-overview-item">
                <span className="wf-stats-label">{t("slash.stats.messageCount")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {info.totalMessageCount}
                </span>
              </div>
            </div>

            {/* 上下文窗口与总 Token */}
            <div className="wf-stats-grid">
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.contextWindow")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatTokens(info.contextWindow)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.totalTokens")}</span>
                <span className="wf-stats-value wf-stats-value-mono wf-stats-value-accent">
                  {formatTokens(info.totalUsedTokens)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.inputTokens")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatTokens(inputTokens)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.outputTokens")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatTokens(info.responseTokens)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.systemPrompt")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatTokens(info.systemPromptTokens)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.functionDefs")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatTokens(info.functionDefinitionsTokens)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.conversation")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatTokens(info.conversationTokens)}
                </span>
              </div>
              <div className="wf-stats-cell">
                <span className="wf-stats-label">{t("slash.stats.cacheHitRate")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatPercent(info.cacheHitRate)}
                </span>
              </div>
            </div>

            {/* 占用比例 + 进度条 */}
            <div className="wf-stats-progress-wrap">
              <div className="wf-stats-progress-header">
                <span className="wf-stats-label">{t("slash.stats.usageRatio")}</span>
                <span className="wf-stats-value wf-stats-value-mono">
                  {formatPercent(usageRatio)}
                </span>
              </div>
              <div
                className="wf-stats-progress-track"
                role="progressbar"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(progressWidth)}
              >
                <div
                  className={`wf-stats-progress-fill${progressWidth >= 90 ? " wf-stats-progress-danger" : ""}`}
                  style={{ width: `${progressWidth}%` }}
                />
              </div>
            </div>
          </>
        )}
      </div>

      <style>{`
        .wf-stats-flat {
          display: flex;
          flex-direction: column;
          gap: 8px;
          padding: 8px 10px;
          font-size: 12px;
          color: var(--color-text-primary);
          background: var(--color-bg-secondary, rgba(0, 0, 0, 0.02));
          border: 1px solid var(--color-border, rgba(0, 0, 0, 0.06));
          border-radius: 6px;
          line-height: 1.5;
        }
        .wf-stats-title {
          display: flex;
          align-items: center;
          gap: 6px;
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .wf-stats-empty {
          font-size: 12px;
          color: var(--color-text-tertiary);
          padding: 4px 0;
        }
        .wf-stats-overview {
          display: flex;
          flex-direction: column;
          gap: 4px;
          padding-bottom: 6px;
          border-bottom: 1px dashed var(--color-border, rgba(0, 0, 0, 0.06));
        }
        .wf-stats-overview-item {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
        }
        .wf-stats-grid {
          display: grid;
          grid-template-columns: repeat(2, minmax(0, 1fr));
          gap: 4px 12px;
        }
        .wf-stats-cell {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          min-width: 0;
        }
        .wf-stats-label {
          font-size: 11px;
          color: var(--color-text-secondary);
          flex-shrink: 0;
        }
        .wf-stats-value {
          font-size: 12px;
          color: var(--color-text-primary);
          text-align: right;
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .wf-stats-value-mono {
          font-family: var(--font-mono, ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace);
          font-variant-numeric: tabular-nums;
        }
        .wf-stats-value-accent {
          color: var(--color-accent, #3b82f6);
          font-weight: 600;
        }
        .wf-stats-progress-wrap {
          display: flex;
          flex-direction: column;
          gap: 4px;
          padding-top: 6px;
          border-top: 1px dashed var(--color-border, rgba(0, 0, 0, 0.06));
        }
        .wf-stats-progress-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
        }
        .wf-stats-progress-track {
          width: 100%;
          height: 6px;
          background: var(--color-bg-muted, rgba(0, 0, 0, 0.08));
          border-radius: 3px;
          overflow: hidden;
        }
        .wf-stats-progress-fill {
          height: 100%;
          background: var(--color-accent, #3b82f6);
          border-radius: 3px;
          transition: width 0.3s ease;
        }
        .wf-stats-progress-danger {
          background: var(--color-error, #ef4444);
        }
      `}</style>
    </div>
  );
}
