import { useState } from "react";
import type { WorkflowNode, ContentNodeData } from "../../types";
import { MarkdownPreview } from "../preview/MarkdownPreview";
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";

interface ContentNodeProps {
  node: WorkflowNode<"content">;
}

export function ContentNode({ node }: ContentNodeProps) {
  const data = node.data as ContentNodeData;
  const isCompleted = node.status === "completed" && !data.isStreaming;
  const [copied, setCopied] = useState(false);

  // 判断当前 content 节点是否为其所在助手回复片段的最后一个 content 节点
  // 仅在最后一个 content 节点显示复制按钮，避免在工具调用前的中间内容后错误出现按钮
  const nodes = useWorkflowStore((state) => state.nodes);
  const isLastContentInTurn = (() => {
    const idx = nodes.findIndex((n) => n.id === node.id);
    if (idx === -1) return false;
    // 向后扫描到下一个 user 节点（片段边界），期间若遇到 content 节点则非最后一个
    for (let i = idx + 1; i < nodes.length; i++) {
      if (nodes[i].type === "user") break;
      if (nodes[i].type === "content") return false;
    }
    return true;
  })();

  // 复制内容到剪贴板：优先使用现代 Clipboard API，失败时降级为 execCommand
  const handleCopy = async () => {
    try {
      await navigator.clipboard.writeText(data.content);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = data.content;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand("copy");
      document.body.removeChild(ta);
    }
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  return (
    <div className="wf-node">
      <div className="wf-content-text-wrapper">
        <MarkdownPreview
          content={data.content}
          className="wf-content-markdown"
        />
        {isCompleted && isLastContentInTurn && (
          <div className="wf-content-copy-btn">
            <button
              className="wf-copy-button"
              onClick={handleCopy}
              title={copied ? "已复制" : "复制"}
            >
              {copied ? (
                <Icon name="check" size={12} />
              ) : (
                <Icon name="copy" size={12} />
              )}
            </button>
          </div>
        )}
      </div>
      <style>{`
        .wf-content-text-wrapper {
          min-width: 0;
          flex: 1;
          flex-direction: column;
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

        /* 工作流区域表格：小圆角容器，表头深色背景，body 行无背景 */
        .wf-content-markdown .md-table-wrap {
          border-radius: var(--radius-sm);
          overflow: hidden;
          border: 1px solid var(--color-border);
        }
        .wf-content-markdown .md-table {
          margin: 0;
        }
        /* 单元格只保留右、下边框作为内部分隔线，外边框由容器提供 */
        .wf-content-markdown .md-table th,
        .wf-content-markdown .md-table td {
          border-top: none;
          border-left: none;
        }
        .wf-content-markdown .md-table th:last-child,
        .wf-content-markdown .md-table td:last-child {
          border-right: none;
        }
        .wf-content-markdown .md-table tbody tr:last-child td {
          border-bottom: none;
        }
        /* 表头深色背景，body 行背景透明（覆盖 tr 和 td 两处斑马纹来源） */
        .wf-content-markdown .md-table thead th {
          background: var(--color-bg-hover) !important;
        }
        .wf-content-markdown .md-table tbody tr,
        .wf-content-markdown .md-table tbody tr td {
          background: transparent !important;
        }

      `}</style>
    </div>
  );
}
