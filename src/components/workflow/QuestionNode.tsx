import { useState } from "react";
import { useTranslation } from "react-i18next";
import type { WorkflowNode, QuestionNodeData } from "../../types";
import { Icon } from "../common/Icon";
import { useWorkflowStore } from "../../stores/useWorkflowStore";
import { submitQuestionAnswer } from "../../services/tauri";

interface QuestionNodeProps {
  node: WorkflowNode<"question">;
}

/**
 * 提问节点
 * Agent 向用户提问时显示，支持单选/多选
 * - 未回答时(answered=false)：显示问题列表和提交按钮
 * - 已回答时(answered=true)：显示问题和用户选择，禁用交互
 */
export function QuestionNode({ node }: QuestionNodeProps) {
  const { t } = useTranslation();
  const data = node.data as QuestionNodeData;
  const updateNode = useWorkflowStore((s) => s.updateNode);
  // 每个问题的选中选项状态：questionIndex -> 选中选项 label 数组
  const [selections, setSelections] = useState<Record<number, string[]>>({});
  const [submitting, setSubmitting] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // 切换单选项：单选时仅保留当前选中项
  const handleSingleSelect = (questionIndex: number, label: string) => {
    setSelections((prev) => ({ ...prev, [questionIndex]: [label] }));
  };

  // 切换多选项：多选时在已选列表中增删
  const handleMultiToggle = (questionIndex: number, label: string) => {
    setSelections((prev) => {
      const current = prev[questionIndex] ?? [];
      const next = current.includes(label)
        ? current.filter((l) => l !== label)
        : [...current, label];
      return { ...prev, [questionIndex]: next };
    });
  };

  // 提交回答
  const handleSubmit = async () => {
    // 校验：每个问题至少选择一个选项
    for (let i = 0; i < data.questions.length; i++) {
      const sel = selections[i] ?? [];
      if (sel.length === 0) {
        setErrorMsg(t("questionNode.selectRequired"));
        return;
      }
    }
    setErrorMsg(null);
    setSubmitting(true);
    try {
      // 构造回答数组
      const answers = data.questions.map((_, idx) => ({
        questionIndex: idx,
        selectedOptions: selections[idx] ?? [],
      }));
      await submitQuestionAnswer(data.questionId, answers);
      // 提交成功后更新节点数据，标记为已回答
      updateNode(node.id, {
        data: { ...data, answered: true, answers },
      });
    } catch (err) {
      setErrorMsg(err instanceof Error ? err.message : String(err));
    } finally {
      setSubmitting(false);
    }
  };

  // 判断选项是否被选中
  const isSelected = (questionIndex: number, label: string): boolean => {
    if (data.answered && data.answers) {
      const ans = data.answers.find((a) => a.questionIndex === questionIndex);
      return ans?.selectedOptions.includes(label) ?? false;
    }
    return (selections[questionIndex] ?? []).includes(label);
  };

  return (
    <div className="wf-node">
      <div className="wf-question-flat">
        {/* 标题 */}
        <div className="wf-question-title">
          <Icon name="info" size={14} />
          <span>{t("questionNode.title")}</span>
          {data.answered && (
            <span className="wf-question-answered-badge">
              {t("questionNode.answered")}
            </span>
          )}
        </div>

        {/* 问题列表 */}
        <div className="wf-question-list">
          {data.questions.map((q, qIdx) => (
            <div key={qIdx} className="wf-question-item">
              {/* 问题标签和文本 */}
              <div className="wf-question-header">{q.header}</div>
              <div className="wf-question-text">{q.question}</div>
              {/* 选项列表 */}
              <div className="wf-question-options">
                {q.options.map((opt, oIdx) => {
                  const checked = isSelected(qIdx, opt.label);
                  const inputId = `wf-q-${node.id}-${qIdx}-${oIdx}`;
                  return (
                    <label
                      key={oIdx}
                      htmlFor={inputId}
                      className={`wf-question-option${checked ? " wf-question-option-checked" : ""}`}
                    >
                      <input
                        id={inputId}
                        type={q.multiSelect ? "checkbox" : "radio"}
                        name={`wf-q-${node.id}-${qIdx}`}
                        checked={checked}
                        disabled={data.answered}
                        onChange={() => {
                          if (q.multiSelect) {
                            handleMultiToggle(qIdx, opt.label);
                          } else {
                            handleSingleSelect(qIdx, opt.label);
                          }
                        }}
                        className="wf-question-input"
                      />
                      <span className="wf-question-option-label">{opt.label}</span>
                      {opt.description && (
                        <span className="wf-question-option-desc">{opt.description}</span>
                      )}
                    </label>
                  );
                })}
              </div>
            </div>
          ))}
        </div>

        {/* 错误提示 */}
        {errorMsg && (
          <div className="wf-question-error">{errorMsg}</div>
        )}

        {/* 提交按钮（未回答时显示） */}
        {!data.answered && (
          <div className="wf-question-actions">
            <button
              className="wf-question-submit-btn"
              onClick={(e) => {
                e.stopPropagation();
                void handleSubmit();
              }}
              disabled={submitting}
            >
              {submitting ? t("common.loading") : t("questionNode.submit")}
            </button>
          </div>
        )}
      </div>

      <style>{`
        .wf-question-flat {
          display: flex;
          flex-direction: column;
          gap: 8px;
          padding: 8px 0;
        }
        .wf-question-title {
          display: flex;
          align-items: center;
          gap: 6px;
          font-size: 13px;
          font-weight: 600;
          color: var(--color-text-primary);
        }
        .wf-question-answered-badge {
          display: inline-block;
          padding: 1px 6px;
          font-size: 10px;
          font-weight: 600;
          color: var(--color-success, #22c55e);
          border: 1px solid var(--color-success, #22c55e);
          border-radius: 3px;
          margin-left: 4px;
        }
        .wf-question-list {
          display: flex;
          flex-direction: column;
          gap: 10px;
        }
        .wf-question-item {
          display: flex;
          flex-direction: column;
          gap: 4px;
        }
        .wf-question-header {
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-secondary);
          text-transform: uppercase;
        }
        .wf-question-text {
          font-size: 13px;
          color: var(--color-text-primary);
          line-height: 1.5;
        }
        .wf-question-options {
          display: flex;
          flex-direction: column;
          gap: 4px;
          margin-top: 2px;
        }
        .wf-question-option {
          display: flex;
          align-items: flex-start;
          gap: 6px;
          padding: 4px 8px;
          border-radius: 4px;
          cursor: pointer;
          font-size: 12px;
          transition: background 0.15s;
        }
        .wf-question-option:hover {
          background: var(--color-bg-tertiary);
        }
        .wf-question-option-checked {
          background: var(--color-bg-tertiary);
        }
        .wf-question-input {
          margin-top: 2px;
          cursor: pointer;
        }
        .wf-question-input:disabled {
          cursor: not-allowed;
        }
        .wf-question-option-label {
          font-weight: 500;
          color: var(--color-text-primary);
        }
        .wf-question-option-desc {
          color: var(--color-text-tertiary);
          font-size: 11px;
          margin-left: 4px;
        }
        .wf-question-error {
          font-size: 11px;
          color: var(--color-error, #ef4444);
          padding: 2px 0;
        }
        .wf-question-actions {
          display: flex;
          gap: 8px;
          margin-top: 4px;
        }
        .wf-question-submit-btn {
          padding: 4px 16px;
          min-height: 28px;
          font-size: 12px;
          font-weight: 500;
          color: white;
          background: var(--color-accent, #3b82f6);
          border: 1px solid var(--color-accent, #3b82f6);
          border-radius: var(--radius-sm, 6px);
          cursor: pointer;
          transition: all 0.15s;
        }
        .wf-question-submit-btn:hover:not(:disabled) {
          filter: brightness(0.9);
        }
        .wf-question-submit-btn:disabled {
          opacity: 0.6;
          cursor: not-allowed;
        }
      `}</style>
    </div>
  );
}
