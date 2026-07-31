import type { WorkflowNode, PausedNodeData } from "../../types";
import { Icon } from "../common/Icon";

interface PausedNodeProps {
  node: WorkflowNode<"paused">;
}

/** 用户手动停止提示节点：灰色停止图标（圆圈内一条横线）+ 灰色文字 */
export function PausedNode({ node }: PausedNodeProps) {
  const data = node.data as PausedNodeData;

  return (
    <div className="wf-node wf-node-paused">
      <div className="wf-paused-notice">
        <Icon name="stop-circle" size={12} />
        <span>{data.message}</span>
      </div>
    </div>
  );
}
