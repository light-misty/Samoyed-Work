import type { WorkflowNode, ContentNodeData } from "../../types";

interface ContentNodeProps {
  node: WorkflowNode<"content">;
}

function renderSafeContent(text: string): string {
  return text
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/`([^`]+)`/g, "<code>$1</code>")
    .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
    .replace(/\*([^*]+)\*/g, "<em>$1</em>")
    .replace(/\n/g, "<br />");
}

export function ContentNode({ node }: ContentNodeProps) {
  const data = node.data as ContentNodeData;
  const isStreaming = data.isStreaming || node.status === "running";

  return (
    <div className="wf-node animate-node-in">
      <div className="wf-content-dot" />
      <div className="wf-content-text-wrapper">
        <div
          className="wf-content-text"
          dangerouslySetInnerHTML={{ __html: renderSafeContent(data.content) }}
        />
        {isStreaming && <span className="wf-content-cursor" />}
      </div>
      <style>{`
        .wf-content-dot {
          width: 6px;
          height: 6px;
          border-radius: 50%;
          background: var(--color-text-quaternary);
          flex-shrink: 0;
          margin-top: 7px;
        }
        .wf-content-text-wrapper {
          display: flex;
          align-items: baseline;
          gap: 0;
          min-width: 0;
          flex: 1;
        }
        .wf-content-text {
          font-size: 14px;
          line-height: 1.6;
          color: var(--color-text-primary);
          word-break: break-word;
        }
        .wf-content-text code {
          padding: 1px 5px;
          border-radius: var(--radius-sm);
          background: var(--color-bg-sub);
          font-family: var(--font-mono);
          font-size: 13px;
          color: var(--color-accent);
        }
        .wf-content-text strong {
          font-weight: 600;
        }
        .wf-content-text em {
          font-style: italic;
        }
        .wf-content-cursor {
          display: inline-block;
          width: 2px;
          height: 16px;
          background: var(--color-accent);
          margin-left: 1px;
          vertical-align: middle;
          animation: wf-cursor-blink 1s step-end infinite;
        }
        @keyframes wf-cursor-blink {
          0%, 100% { opacity: 1; }
          50% { opacity: 0; }
        }
      `}</style>
    </div>
  );
}
