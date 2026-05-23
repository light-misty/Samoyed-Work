import type { WorkflowNode, ConfirmNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";

interface ConfirmNodeProps {
  node: WorkflowNode<"confirm">;
  onToggle: () => void;
}

export function ConfirmNode({ node }: ConfirmNodeProps) {
  const data = node.data as ConfirmNodeData;
  const confirmHandler = useWorkflowStore((s) => s.confirmHandler);
  const isPending = data.confirmed === null && node.status === "running";

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-node-dot bg-warning-light text-warning">
        <Icon name="warning" size={12} />
      </div>

      <div className="wf-confirm-flat">
        <div className="wf-confirm-title">
          <Icon name="warning" size={14} />
          {data.title}
        </div>
        <div className="wf-confirm-desc">{data.description}</div>
        {isPending ? (
          <div className="wf-confirm-actions">
            <button
              className="btn btn-danger"
              onClick={async (e) => {
                e.stopPropagation();
                await confirmHandler?.(true);
              }}
            >
              {data.confirmLabel}
            </button>
            <button
              className="btn btn-ghost"
              onClick={async (e) => {
                e.stopPropagation();
                await confirmHandler?.(false);
              }}
            >
              {data.cancelLabel}
            </button>
          </div>
        ) : (
          <div className={`wf-confirm-result ${data.confirmed ? "confirmed" : "cancelled"}`}>
            {data.confirmed ? "✓ 用户已确认执行" : "✗ 用户已取消操作"}
          </div>
        )}
      </div>
    </div>
  );
}
