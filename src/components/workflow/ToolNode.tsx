import type { WorkflowNode, ToolNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface ToolNodeProps {
  node: WorkflowNode<"tool">;
}

export function ToolNode({ node }: ToolNodeProps) {
  const data = node.data as ToolNodeData;
  const hasError = data.success === false;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-node-dot bg-bg-sub text-text-secondary">
        <Icon name="tool" size={12} />
      </div>

      <div className="wf-tool-brief">
        <span className="font-mono">{data.toolName}</span>
        <span> · </span>
        <span>{data.briefDescription}</span>
        {hasError && data.error && (
          <span className="wf-tool-error"> — {data.error}</span>
        )}
      </div>
    </div>
  );
}
