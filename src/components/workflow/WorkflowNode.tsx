import type { WorkflowNode, WorkflowNodeType } from "../../types";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { UserNode } from "./UserNode";
import { ThinkingNode } from "./ThinkingNode";
import { ContentNode } from "./ContentNode";
import { ToolNode } from "./ToolNode";
import { ConfirmNode } from "./ConfirmNode";
import { ErrorNode } from "./ErrorNode";
import { CompactionNode } from "./CompactionNode";
import { SubAgentNode } from "./SubAgentNode";
import { QuestionNode } from "./QuestionNode";

interface WorkflowNodeRendererProps {
  node: WorkflowNode;
  onRetry?: () => void;
  hideCopy?: boolean;
}

export function WorkflowNodeRenderer({ node, onRetry, hideCopy }: WorkflowNodeRendererProps) {
  const { toggleNode } = useWorkflowStore();
  const nt = node.type as WorkflowNodeType;

  switch (nt) {
    case "user":
      return <UserNode node={node as WorkflowNode<"user">} hideCopy={hideCopy} />;
    case "thinking":
      return <ThinkingNode node={node as WorkflowNode<"thinking">} />;
    case "content":
      return <ContentNode node={node as WorkflowNode<"content">} hideCopy={hideCopy} />;
    case "tool":
      return <ToolNode node={node as WorkflowNode<"tool">} />;
    case "confirm":
      return <ConfirmNode node={node as WorkflowNode<"confirm">} />;
    case "error":
      return <ErrorNode node={node as WorkflowNode<"error">} onToggle={() => toggleNode(node.id)} onRetry={onRetry} />;
    case "compaction":
      return <CompactionNode node={node as WorkflowNode<"compaction">} />;
    case "sub_agent":
      return <SubAgentNode node={node as WorkflowNode<"sub_agent">} />;
    case "question":
      return <QuestionNode node={node as WorkflowNode<"question">} />;
    default:
      return null;
  }
}
