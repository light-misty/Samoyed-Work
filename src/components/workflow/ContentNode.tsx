import type { WorkflowNode, ContentNodeData } from "../../types";
import { MarkdownPreview } from "../preview/MarkdownPreview";

interface ContentNodeProps {
  node: WorkflowNode<"content">;
}

export function ContentNode({ node }: ContentNodeProps) {
  const data = node.data as ContentNodeData;

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-content-text-wrapper">
        <MarkdownPreview
          content={data.content}
          className="wf-content-markdown"
        />
      </div>
      <style>{`
        .wf-content-text-wrapper {
          min-width: 0;
          flex: 1;
        }
        .wf-content-markdown {
          color: var(--color-text-primary);
          word-break: break-word;
          line-height: 1.6;
        }
        .wf-content-markdown p:last-child {
          margin-bottom: 0;
        }
        .wf-content-markdown h1:first-child,
        .wf-content-markdown h2:first-child,
        .wf-content-markdown h3:first-child {
          margin-top: 0;
        }

      `}</style>
    </div>
  );
}
