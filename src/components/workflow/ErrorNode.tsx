import type { WorkflowNode, ErrorNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface ErrorNodeProps {
  node: WorkflowNode<"error">;
  onToggle: () => void;
  onRetry?: () => void;
}

export function ErrorNode({ node, onRetry }: ErrorNodeProps) {
  const data = node.data as ErrorNodeData;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-node-dot bg-error-light text-error">
        <Icon name="error" size={12} />
      </div>

      <div className="wf-error-flat">
        <div className="wf-error-message">{data.message}</div>
        <details className="wf-error-details">
          <summary>错误详情</summary>
          <div className="wf-error-detail-content">
            <div>错误码: E{data.code}</div>
            <div>模块: {data.module}</div>
          </div>
        </details>
        {data.recoverable && onRetry && (
          <button className="wf-error-retry-btn" onClick={onRetry}>
            <Icon name="refresh" size={14} />
            重试
          </button>
        )}
      </div>
    </div>
  );
}
