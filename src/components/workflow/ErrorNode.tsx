import type { WorkflowNode, ErrorNodeData } from "../../types";
import { formatTime } from "../../utils/format";
import { Icon } from "../common/Icon";

interface ErrorNodeProps {
  node: WorkflowNode<"error">;
  onToggle: () => void;
  /** 重试回调，仅在 recoverable 为 true 时可用 */
  onRetry?: () => void;
}

export function ErrorNode({ node, onToggle, onRetry }: ErrorNodeProps) {
  const data = node.data as ErrorNodeData;

  return (
    <div className={`wf-node animate-node-in ${!node.isExpanded ? "collapsed" : ""}`}>
      <div className="wf-node-dot bg-error-light text-error">
        <Icon name="error" size={12} />
      </div>

      <div className="wf-node-card">
        <div className="wf-node-header" onClick={onToggle}>
          <span className="wf-node-label error">执行出错</span>
          {data.recoverable && (
            <span className="wf-error-badge">可重试</span>
          )}
          <span className="wf-node-time">{formatTime(node.timestamp)}</span>
          <span className="wf-node-toggle">
            <Icon name="chevron-down" size={14} style={{ transform: node.isExpanded ? "rotate(0deg)" : "rotate(-90deg)", transition: "transform 0.2s" }} />
          </span>
        </div>
        {node.isExpanded && (
          <div className="wf-node-body">
            {/* 错误消息 */}
            <div className="wf-error-message">
              {data.message}
            </div>

            {/* 错误详情（可折叠） */}
            <details className="wf-error-details">
              <summary>错误详情</summary>
              <div className="wf-error-detail-content">
                <div>错误码: E{data.code}</div>
                <div>模块: {data.module}</div>
              </div>
            </details>

            {/* 可恢复时显示重试按钮 */}
            {data.recoverable && onRetry && (
              <button className="wf-error-retry-btn" onClick={onRetry}>
                <Icon name="refresh" size={14} />
                重试
              </button>
            )}
          </div>
        )}
      </div>

      <style>{`
        .wf-error-badge {
          font-size: 10px;
          padding: 1px 6px;
          border-radius: var(--radius-sm);
          background: var(--color-warning-bg, rgba(245, 158, 11, 0.1));
          color: var(--color-warning);
          font-weight: 500;
        }
        .wf-error-message {
          font-size: 13px;
          color: var(--color-error);
          line-height: 1.6;
          padding: 8px 12px;
          background: var(--color-error-bg, rgba(239, 68, 68, 0.06));
          border-radius: var(--radius-sm);
          border-left: 3px solid var(--color-error);
        }
        .wf-error-details {
          margin-top: 8px;
          font-size: 12px;
        }
        .wf-error-details summary {
          cursor: pointer;
          color: var(--color-text-tertiary);
          font-weight: 500;
          padding: 2px 0;
          user-select: none;
        }
        .wf-error-details summary:hover {
          color: var(--color-text-secondary);
        }
        .wf-error-detail-content {
          padding: 8px 12px;
          margin-top: 4px;
          background: var(--color-bg-sub);
          border-radius: var(--radius-sm);
          font-family: var(--font-mono);
          font-size: 11px;
          color: var(--color-text-tertiary);
          line-height: 1.8;
        }
        .wf-error-retry-btn {
          display: inline-flex;
          align-items: center;
          gap: 6px;
          margin-top: 10px;
          padding: 6px 14px;
          border-radius: var(--radius-sm);
          background: var(--color-accent);
          color: white;
          font-size: 12px;
          font-weight: 500;
          cursor: pointer;
          transition: all 0.15s;
          border: none;
        }
        .wf-error-retry-btn:hover {
          filter: brightness(0.9);
        }
      `}</style>
    </div>
  );
}
