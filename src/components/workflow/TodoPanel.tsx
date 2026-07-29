import { useState, useEffect, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useWorkflowStore } from '../../stores/useWorkflowStore';
import { useSessionStore } from '../../stores/useSessionStore';
import { getTodoList } from '../../services/tauri';
import { Icon } from '../common/Icon';
import type { TodoItem } from '../../types/todo';

const PRIORITY_LABEL_KEY: Record<string, string> = {
  high: 'todo.priorityHigh',
  medium: 'todo.priorityMedium',
  low: 'todo.priorityLow',
};

const PRIORITY_COLORS: Record<string, string> = {
  high: 'var(--color-error)',
  medium: 'var(--color-warning)',
  low: 'var(--color-text-tertiary)',
};

function TodoItemRow({ item }: { item: TodoItem }) {
  const { t } = useTranslation();
  const isCompleted = item.status === 'completed';
  const isInProgress = item.status === 'in_progress';
  const priorityColor = PRIORITY_COLORS[item.priority] ?? 'var(--color-text-tertiary)';
  const [tooltipPos, setTooltipPos] = useState<{ x: number; y: number } | null>(null);

  return (
    <div
      className={`todo-item${isCompleted ? ' completed' : ''}${isInProgress ? ' in-progress' : ''}`}
      onMouseEnter={(e) => setTooltipPos({ x: e.clientX, y: e.clientY })}
      onMouseMove={(e) => setTooltipPos({ x: e.clientX, y: e.clientY })}
      onMouseLeave={() => setTooltipPos(null)}
    >
      <span className={`todo-item-status ${item.status}`}>
        {isCompleted ? (
          <Icon name="check-circle" size={14} />
        ) : isInProgress ? (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--color-primary)" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
            <polyline points="12 6 12 12 16 14" />
          </svg>
        ) : (
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--color-text-tertiary)" strokeWidth="2">
            <circle cx="12" cy="12" r="10" />
          </svg>
        )}
      </span>
      <span className="todo-item-content">{item.content}</span>
      <span className="todo-item-priority" style={{ color: priorityColor }}>
        {t(PRIORITY_LABEL_KEY[item.priority] ?? 'todo.priorityMedium')}
      </span>
      {tooltipPos && (
        <div className="todo-tooltip" style={{ left: tooltipPos.x + 12, top: tooltipPos.y - 10 }}>
          {item.content}
        </div>
      )}
    </div>
  );
}

export function TodoPanel() {
  const { t } = useTranslation();
  const [items, setItems] = useState<TodoItem[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const nodes = useWorkflowStore((s) => s.nodes);

  const loadTodos = useCallback(async () => {
    const sessionId = useSessionStore.getState().currentSessionId;
    if (!sessionId) {
      setItems([]);
      return;
    }
    setLoading(true);
    setError(null);
    try {
      const todoList = await getTodoList(sessionId);
      setItems(todoList.items);
    } catch (err) {
      console.error('[TodoPanel] 加载待办任务失败:', err);
      setError(t('todo.loadError'));
    } finally {
      setLoading(false);
    }
  }, [t]);

  useEffect(() => {
    loadTodos();
  }, [loadTodos, nodes]);

  return (
    <div className="todo-panel">
      {loading ? (
        <div className="branch-graph-empty">{t('common.loading')}</div>
      ) : error ? (
        <div className="branch-graph-empty">{error}</div>
      ) : items.length === 0 ? (
        <div className="branch-graph-empty">{t('todo.empty')}</div>
      ) : (
        <div className="todo-item-list">
          {items.map((item) => (
            <TodoItemRow key={item.id} item={item} />
          ))}
        </div>
      )}
    </div>
  );
}
