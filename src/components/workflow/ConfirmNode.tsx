import type { WorkflowNode, ConfirmNodeData } from "../../types";
import { formatTime } from "../../utils/format";
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";

interface ConfirmNodeProps {
  node: WorkflowNode<"confirm">;
  onToggle: () => void;
}

export function ConfirmNode({ node, onToggle }: ConfirmNodeProps) {
  const data = node.data as ConfirmNodeData;
  const confirmHandler = useWorkflowStore((s) => s.confirmHandler);

  const isPending = data.confirmed === null && node.status === "running";

  return (
    <div className={`relative mb-1 animate-node-in ${!node.isExpanded ? "collapsed" : ""}`}>
      <div className="absolute -left-[28px] top-[14px] w-[22px] h-[22px] rounded-full flex items-center justify-center z-[2] bg-warning-light text-warning">
        <Icon name="warning" size={12} />
      </div>

      <div className="rounded-[var(--radius-md)] border border-warning bg-bg overflow-hidden transition-colors duration-150">
        <div className="flex items-center gap-2 px-[14px] py-[10px] cursor-pointer select-none" onClick={onToggle}>
          <span className="text-[12px] font-semibold uppercase tracking-[.3px]" style={{ color: "var(--color-warning)" }}>
            操作确认
          </span>
          <span className="text-[11px] text-text-tertiary font-mono ml-auto">{formatTime(node.timestamp)}</span>
          {data.confirmed !== null && (
            <span className={`text-[11px] font-medium ${data.confirmed ? "text-success" : "text-error"}`}>
              {data.confirmed ? "已确认" : "已取消"}
            </span>
          )}
          <span className="w-5 h-5 flex items-center justify-center rounded-[4px] transition-colors duration-150 text-text-tertiary hover:bg-bg-sub">
            <Icon name="chevron-down" size={14} style={{ transform: node.isExpanded ? "rotate(0deg)" : "rotate(-90deg)", transition: "transform 0.2s" }} />
          </span>
        </div>
        {node.isExpanded && (
          <div className="px-[14px] pb-3">
            <div>
              <div className="flex items-center gap-[6px] text-[13px] font-semibold text-warning mb-[6px]">
                <Icon name="warning" size={16} />
                {data.title}
              </div>
              <div className="text-[12px] text-text-secondary leading-[1.5] mb-[10px]">
                {data.description}
              </div>
              {isPending ? (
                <div className="flex gap-2">
                  <button
                    className="px-[14px] py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-error text-white hover:bg-[#D63D39] transition-all duration-150"
                    onClick={(e) => {
                      e.stopPropagation();
                      confirmHandler?.(true);
                    }}
                  >
                    {data.confirmLabel}
                  </button>
                  <button
                    className="px-[14px] py-[6px] rounded-[var(--radius-sm)] text-[12px] font-medium bg-bg-sub text-text-secondary hover:bg-bg-hover transition-all duration-150"
                    onClick={(e) => {
                      e.stopPropagation();
                      confirmHandler?.(false);
                    }}
                  >
                    {data.cancelLabel}
                  </button>
                </div>
              ) : (
                <div className={`text-[12px] font-medium ${data.confirmed ? "text-success" : "text-error"}`}>
                  {data.confirmed ? "✓ 用户已确认执行" : "✗ 用户已取消操作"}
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
