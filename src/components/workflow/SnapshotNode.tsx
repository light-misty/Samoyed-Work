import type { WorkflowNode, SnapshotNodeData } from "../../types";
import { Icon } from "../common/Icon";
import i18n from "../../i18n";

interface SnapshotNodeProps {
  node: WorkflowNode<"snapshot">;
}

/** 文件快照节点：提示"文件快照已创建"（版本快照/回退功能） */
export function SnapshotNode({ node }: SnapshotNodeProps) {
  const data = node.data as SnapshotNodeData;
  const kindLabel = data.kind === "git" ? "git" : "files";

  return (
    <div className="wf-node wf-node-snapshot">
      <div className="wf-snapshot-notice">
        <Icon name="history" size={12} />
        <span>{i18n.t("workflow.snapshotCreated")}</span>
        <span className="wf-snapshot-kind">({kindLabel})</span>
      </div>
    </div>
  );
}
