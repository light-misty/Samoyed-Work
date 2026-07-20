import { useState } from "react";
import { createPortal } from "react-dom";
import { useTranslation } from "react-i18next";
import type { WorkflowNode, UserNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { formatSize } from "../../utils/format";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { useSessionStore } from "../../stores/useSessionStore";
import * as tauriCmd from "../../services/tauri";

interface UserNodeProps {
  node: WorkflowNode<"user">;
  hideCopy?: boolean;
}

export function UserNode({ node, hideCopy }: UserNodeProps) {
  const { t } = useTranslation();
  const data = node.data as UserNodeData;
  const hasAttachments = data.attachments && data.attachments.length > 0;
  const [copied, setCopied] = useState(false);
  const [showDeleteConfirm, setShowDeleteConfirm] = useState(false);

  const executionStatus = useWorkflowStore((s) => s.executionStatus);
  const isAgentRunning = executionStatus === "running";

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

  const handleDelete = async () => {
    const sessionId = useSessionStore.getState().currentSessionId;
    if (!sessionId) return;

    const { nodes, clearSessionCache, loadContextUsage } = useWorkflowStore.getState();

    const currentIdx = nodes.findIndex((n) => n.id === node.id);
    if (currentIdx === -1) return;

    let endIdx = nodes.length;
    for (let i = currentIdx + 1; i < nodes.length; i++) {
      if (nodes[i].type === "user") {
        endIdx = i;
        break;
      }
    }

    // 方案一：从节点数据中收集 messageId（会话从后端加载时已填充）
    const messageIdSet = new Set<string>();
    for (const n of nodes.slice(currentIdx, endIdx)) {
      const mid = (n.data as unknown as Record<string, unknown>).messageId as string | undefined;
      if (mid) messageIdSet.add(mid);
    }

    // 方案二：节点是在实时对话中创建的（无 messageId），从后端拉取消息列表来定位
    if (messageIdSet.size === 0) {
      try {
        const detail = await tauriCmd.getSession(sessionId);
        const msgs = detail.messages;
        // 统计当前用户节点是第几条用户消息
        let userMsgIndex = 0;
        for (let i = 0; i < currentIdx; i++) {
          if (nodes[i].type === "user") userMsgIndex++;
        }
        // 在消息列表中找到对应的用户消息
        let msgStartIdx = -1;
        let userSeen = -1;
        for (let i = 0; i < msgs.length; i++) {
          if (msgs[i].role === "user") {
            userSeen++;
            if (userSeen === userMsgIndex) {
              msgStartIdx = i;
              break;
            }
          }
        }
        if (msgStartIdx === -1) return;
        // 收集从该用户消息到下一用户消息之间的所有消息 ID
        for (let i = msgStartIdx; i < msgs.length; i++) {
          messageIdSet.add(msgs[i].id);
          if (i > msgStartIdx && msgs[i].role === "user") {
            // 不包含下一条用户消息本身
            messageIdSet.delete(msgs[i].id);
            break;
          }
        }
      } catch (err) {
        console.error("[UserNode] 获取会话消息失败:", err);
        return;
      }
    }

    const messageIds = Array.from(messageIdSet);
    if (messageIds.length === 0) return;

    try {
      await tauriCmd.deleteSessionMessages(sessionId, messageIds);

      useWorkflowStore.setState({
        nodes: nodes.filter((_, i) => i < currentIdx || i >= endIdx),
      });

      clearSessionCache(sessionId);
      loadContextUsage(sessionId);
    } catch (err) {
      console.error("[UserNode] 删除消息失败:", err);
    }
  };

  const showActions = !hideCopy;

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
        {showActions && (
          <div className={`wf-msg-actions-row${copied ? " wf-copy-visible" : ""}`}>
            {!isAgentRunning && (
              <button
                className="wf-delete-button"
                onClick={() => setShowDeleteConfirm(true)}
                title={t('workflow.deleteMessage')}
              >
                <Icon name="trash" size={12} />
              </button>
            )}
            <div className="wf-msg-copy-btn">
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
          </div>
        )}
      </div>

      {showDeleteConfirm && createPortal(
        <div className="wf-del-overlay" onClick={() => setShowDeleteConfirm(false)}>
          <div className="wf-del-dialog" onClick={(e) => e.stopPropagation()}>
            <div className="wf-del-header">
              <span className="wf-del-icon">
                <Icon name="warning" size={18} />
              </span>
              <span className="wf-del-title">{t('deleteConfirm.title')}</span>
            </div>
            <div className="wf-del-body">
              <p className="wf-del-message">{t('workflow.deleteMessageConfirm')}</p>
            </div>
            <div className="wf-del-footer">
              <button
                className="wf-del-btn wf-del-btn-danger"
                onClick={() => {
                  setShowDeleteConfirm(false);
                  handleDelete();
                }}
              >
                {t('common.delete')}
              </button>
              <button
                className="wf-del-btn wf-del-btn-cancel"
                onClick={() => setShowDeleteConfirm(false)}
              >
                {t('deleteConfirm.cancel')}
              </button>
            </div>
          </div>
        </div>,
        document.body
      )}
    </div>
  );
}
