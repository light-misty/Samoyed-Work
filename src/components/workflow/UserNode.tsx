import type { WorkflowNode, UserNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface UserNodeProps {
  node: WorkflowNode<"user">;
}

export function UserNode({ node }: UserNodeProps) {
  const data = node.data as UserNodeData;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-node-dot bg-accent-light text-accent">
        <Icon name="user" size={12} />
      </div>
      <div className="wf-node-card">
        <div className="wf-node-body">
          <div className="wf-user-text">{data.content}</div>
        </div>
      </div>
    </div>
  );
}
