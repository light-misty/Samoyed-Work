import type { WorkflowNode, WorkflowNodeType } from "../../types";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { UserNode } from "./UserNode";
import { ThinkingNode } from "./ThinkingNode";
import { ToolNode } from "./ToolNode";
import { ResultNode } from "./ResultNode";
import { ReplyNode } from "./ReplyNode";
import { ConfirmNode } from "./ConfirmNode";
import { ErrorNode } from "./ErrorNode";

interface WorkflowNodeRendererProps {
  node: WorkflowNode;
  /** 错误节点重试回调 */
  onRetry?: () => void;
}

export function WorkflowNodeRenderer({ node, onRetry }: WorkflowNodeRendererProps) {
  const { toggleNode } = useWorkflowStore();
  const nt = node.type as WorkflowNodeType;

  switch (nt) {
    case "user":
      return <UserNode node={node as WorkflowNode<"user">} onToggle={() => toggleNode(node.id)} />;
    case "thinking":
      return <ThinkingNode node={node as WorkflowNode<"thinking">} onToggle={() => toggleNode(node.id)} />;
    case "tool":
      return <ToolNode node={node as WorkflowNode<"tool">} onToggle={() => toggleNode(node.id)} />;
    case "result":
      return <ResultNode node={node as WorkflowNode<"result">} onToggle={() => toggleNode(node.id)} />;
    case "reply":
      return <ReplyNode node={node as WorkflowNode<"reply">} onToggle={() => toggleNode(node.id)} />;
    case "confirm":
      return <ConfirmNode node={node as WorkflowNode<"confirm">} onToggle={() => toggleNode(node.id)} />;
    case "error":
      return <ErrorNode node={node as WorkflowNode<"error">} onToggle={() => toggleNode(node.id)} onRetry={onRetry} />;
    default:
      return null;
  }
}
