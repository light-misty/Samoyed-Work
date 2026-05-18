import type { WorkflowNode, ReplyNodeData } from "../../types";
import { formatTime } from "../../utils/format";
import { Icon } from "../common/Icon";

interface ReplyNodeProps {
  node: WorkflowNode<"reply">;
  onToggle: () => void;
}

export function ReplyNode({ node, onToggle }: ReplyNodeProps) {
  const data = node.data as ReplyNodeData;
  const isStreaming = node.status === "running";

  return (
    <div className={`relative mb-1 animate-node-in ${!node.isExpanded ? "collapsed" : ""}`}>
      <div className="absolute -left-[28px] top-[14px] w-[22px] h-[22px] rounded-full flex items-center justify-center z-[2] bg-accent-light text-accent">
        <Icon name="reply" size={12} />
      </div>

      <div className="rounded-[var(--radius-md)] border border-border bg-bg overflow-hidden transition-colors duration-150 hover:border-[#D0D3D9]">
        <div className="flex items-center gap-2 px-[14px] py-[10px] cursor-pointer select-none" onClick={onToggle}>
          <span className="text-[12px] font-semibold uppercase tracking-[.3px] text-accent">回复</span>
          {isStreaming && (
            <span className="inline-block w-[6px] h-[6px] rounded-full bg-accent animate-pulse" />
          )}
          <span className="text-[11px] text-text-tertiary font-mono ml-auto">{formatTime(node.timestamp)}</span>
          <span className="w-5 h-5 flex items-center justify-center rounded-[4px] transition-colors duration-150 text-text-tertiary hover:bg-bg-sub">
            <Icon name="chevron-down" size={14} style={{ transform: node.isExpanded ? "rotate(0deg)" : "rotate(-90deg)", transition: "transform 0.2s" }} />
          </span>
        </div>
        {node.isExpanded && (
          <div className="px-[14px] pb-3">
            <div
              className="text-[14px] leading-[1.7] text-text-primary py-1"
              dangerouslySetInnerHTML={{ __html: data.content }}
            />
            {isStreaming && (
              <span className="inline-block w-[2px] h-[16px] bg-accent animate-pulse ml-[1px] align-middle" />
            )}
          </div>
        )}
      </div>
    </div>
  );
}
