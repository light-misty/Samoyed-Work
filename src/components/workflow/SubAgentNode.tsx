import { useTranslation } from "react-i18next";
import type { WorkflowNode, SubAgentNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";

interface SubAgentNodeProps {
  node: WorkflowNode<"sub_agent">;
}

/**
 * 子 Agent 节点
 * 在 Agent 执行过程中触发子 Agent 时显示
 * - running: 显示"子 Agent 执行中"
 * - completed: 显示"子 Agent 完成"
 * - failed: 显示"子 Agent 失败" + 错误消息
 * - cancelled: 显示"子 Agent 已取消"
 * - 可点击跳转到子 Agent 工作流详情页
 */
export function SubAgentNode({ node }: SubAgentNodeProps) {
  const { t } = useTranslation();
  const setCurrentSubAgentId = useWorkflowStore((s) => s.setCurrentSubAgentId);
  const data = node.data as SubAgentNodeData;
  const isRunning = data.status === "running";
  const isFailed = data.status === "failed";
  const isCompleted = data.status === "completed";
  const isCancelled = data.status === "cancelled";

  // 根据状态选择图标
  const iconName = isRunning
    ? "refresh"
    : isFailed
      ? "warning"
      : isCompleted
        ? "check-circle"
        : "stop";

  // 主文本：根据状态显示不同内容（不在父 Agent 页面显示任务指令）
  const getText = (): string => {
    if (isRunning) {
      return t("subAgentNode.running");
    }
    if (isFailed) {
      return t("subAgentNode.failed");
    }
    if (isCompleted) {
      return t("subAgentNode.completed");
    }
    if (isCancelled) {
      return t("subAgentNode.cancelled");
    }
    return "";
  };

  // 状态相关的 CSS 类名
  const stateClass = isRunning
    ? " wf-subagent-running"
    : isFailed
      ? " wf-subagent-failed"
      : isCompleted
        ? " wf-subagent-completed"
        : " wf-subagent-cancelled";

  return (
    <div className="wf-node">
      <div
        className={`wf-subagent-flat${stateClass}`}
        onClick={() => setCurrentSubAgentId(data.agentId)}
        style={{ cursor: 'pointer' }}
      >
        <Icon
          name={iconName}
          size={12}
          className={isRunning ? "wf-subagent-spin" : undefined}
        />
        <span className="wf-subagent-text">{getText()}</span>
      </div>
      {/* 失败时显示错误消息 */}
      {isFailed && data.message && (
        <div className="wf-subagent-error">
          {t("subAgentNode.error", { message: data.message })}
        </div>
      )}
      <style>{`
        .wf-subagent-flat {
          display: flex;
          align-items: center;
          gap: 6px;
          padding: 2px 0;
          font-size: 12px;
          color: var(--color-text-tertiary);
          line-height: 1.6;
          flex: 1;
          min-width: 0;
          flex-wrap: wrap;
        }
        .wf-subagent-running {
          color: var(--color-accent, #3b82f6);
        }
        .wf-subagent-failed {
          color: var(--color-error, #ef4444);
        }
        .wf-subagent-completed {
          color: var(--color-success, #22c55e);
        }
        .wf-subagent-cancelled {
          color: var(--color-text-tertiary);
        }
        .wf-subagent-text {
          font-size: 12px;
        }
        .wf-subagent-error {
          margin-top: 2px;
          padding-left: 18px;
          font-size: 11px;
          color: var(--color-error, #ef4444);
          line-height: 1.5;
        }
        .wf-subagent-spin {
          animation: wf-subagent-spin 1s linear infinite;
        }
        @keyframes wf-subagent-spin {
          from { transform: rotate(0deg); }
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
