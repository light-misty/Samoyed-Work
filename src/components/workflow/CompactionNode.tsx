import { useTranslation } from "react-i18next";
import type { WorkflowNode, CompactionNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface CompactionNodeProps {
  node: WorkflowNode<"compaction">;
}

/**
 * 上下文压缩节点
 * 在 Agent 执行过程中触发上下文压缩时显示
 * - running: 显示"正在压缩上下文..."
 * - completed: 显示压缩结果 "X -> Y tokens"
 * - failed: 显示压缩失败信息
 */
export function CompactionNode({ node }: CompactionNodeProps) {
  const { t } = useTranslation();
  const data = node.data as CompactionNodeData;
  const isRunning = node.status === "running";
  const isFailed = node.status === "failed" || (!!data.error && !data.compacted);

  // 格式化 token 数，便于阅读
  const formatTokens = (n: number): string => {
    if (n >= 1000) {
      return `${(n / 1000).toFixed(1)}k`;
    }
    return String(n);
  };

  // 文本展示：根据节点状态显示不同内容
  const getText = (): string => {
    if (isRunning) {
      return t("compactionNode.compressing", { tokens: formatTokens(data.tokensBefore) });
    }
    if (isFailed) {
      return data.error
        ? t("compactionNode.failedWithError", { error: data.error })
        : t("compactionNode.failed");
    }
    if (data.tokensAfter !== undefined) {
      return t("compactionNode.completed", {
        before: formatTokens(data.tokensBefore),
        after: formatTokens(data.tokensAfter),
      });
    }
    return t("compactionNode.completed");
  };

  return (
    <div className="wf-node">
      <div className={`wf-compaction-flat${isFailed ? " wf-compaction-failed" : ""}${isRunning ? " wf-compaction-running" : ""}`}>
        <Icon
          name={isRunning ? "refresh" : isFailed ? "warning" : "check-circle"}
          size={12}
          className={isRunning ? "wf-compaction-spin" : undefined}
        />
        <span className="wf-compaction-text">{getText()}</span>
      </div>
      <style>{`
        .wf-compaction-flat {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 2px 0;
          font-size: 12px;
          color: var(--color-text-tertiary);
          line-height: 1.6;
          flex: 1;
          min-width: 0;
        }
        .wf-compaction-running {
          color: var(--color-accent, #3b82f6);
        }
        .wf-compaction-failed {
          color: var(--color-error, #ef4444);
        }
        .wf-compaction-text {
          font-size: 12px;
        }
        .wf-compaction-spin {
          animation: wf-compaction-spin 1s linear infinite;
        }
        @keyframes wf-compaction-spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
