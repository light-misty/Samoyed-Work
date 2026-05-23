import { useState, useEffect } from "react";
import type { WorkflowNode, ThinkingNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface ThinkingNodeProps {
  node: WorkflowNode<"thinking">;
}

export function ThinkingNode({ node }: ThinkingNodeProps) {
  const data = node.data as ThinkingNodeData;
  const isStreaming = data.isStreaming || node.status === "running";
  const [expanded, setExpanded] = useState(isStreaming);

  useEffect(() => {
    if (isStreaming) {
      setExpanded(true);
    } else if (node.status === "completed") {
      setExpanded(false);
    }
  }, [isStreaming, node.status]);

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-node-dot" style={{ background: "var(--color-purple-light)", color: "var(--color-purple)" }}>
        <Icon name="thinking" size={12} />
      </div>

      <div className="wf-thinking-block">
        <div
          className="wf-thinking-toggle"
          onClick={() => setExpanded((prev) => !prev)}
        >
          <Icon
            name={expanded ? "chevron-down" : "chevron-right"}
            size={12}
          />
          <span>Thinking...</span>
        </div>

        {expanded && (
          <div className="wf-thinking-content">
            {data.content}
            {isStreaming && <span className="cursor-blink" />}
          </div>
        )}
      </div>
    </div>
  );
}
