import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { WorkflowNode, UserNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { formatSize } from "../../utils/format";

interface UserNodeProps {
  node: WorkflowNode<"user">;
  hideCopy?: boolean;
}

export function UserNode({ node, hideCopy }: UserNodeProps) {
  const { t } = useTranslation();
  const data = node.data as UserNodeData;
  const hasAttachments = data.attachments && data.attachments.length > 0;
  const [copied, setCopied] = useState(false);

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
    <div className="wf-node wf-user-node">
      <div className="wf-user-msg-wrapper">
        <div className="wf-node-card">
          <div className="wf-node-body">
            <div className="wf-user-text">{data.content}</div>
            {hasAttachments && (
              <div className="wf-user-attachments">
                {data.attachments.map((att) => (
                  <span key={att.id} className="wf-attachment-tag" title={att.name}>
                    <Icon name={att.mimeType.startsWith("image/") ? "image" : "file"} size={10} />
                    <span className="wf-attachment-name">{att.name}</span>
                    <span className="wf-attachment-size">{formatSize(att.size)}</span>
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
        {!hideCopy && (
          <div className={`wf-msg-copy-btn${copied ? " wf-copy-visible" : ""}`}>
            <button
              className="wf-copy-button"
              onClick={handleCopy}
              title={copied ? t('common.copied') : t('common.copy')}
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
    </div>
  );
}
