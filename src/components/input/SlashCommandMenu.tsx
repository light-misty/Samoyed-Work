import { useEffect, useRef, useCallback } from "react";
import { useTranslation } from "react-i18next";
import type { SlashCommand } from "../../commands/slashCommands";

interface SlashCommandMenuProps {
  /** 过滤后的命令列表 */
  commands: SlashCommand[];
  /** 当前高亮项索引 */
  highlightIndex: number;
  /** 选择命令时的回调 */
  onSelect: (command: SlashCommand) => void;
  /** 关闭菜单的回调（Esc 或点击外部） */
  onClose: () => void;
  /** Agent 是否正在运行（用于禁用某些命令的显示） */
  agentRunning: boolean;
  /** 是否从上方弹出（历史会话页面为 true，新建会话页面为 false） */
  dropdownUp?: boolean;
}

/**
 * 斜杠命令选择菜单
 *
 * 在用户输入 / 时弹出，展示匹配的命令列表供用户选择。
 * 键盘导航（上下键/回车/Esc）由父组件 InputArea 处理并更新 highlightIndex，
 * 本组件只负责渲染高亮状态、自动滚动以及鼠标点击交互。
 *
 * 定位：菜单始终从输入框上方弹出（centered 与非 centered 两种布局位置相同）。
 */
export function SlashCommandMenu(props: SlashCommandMenuProps) {
  const { commands, highlightIndex, onSelect, onClose, agentRunning, dropdownUp = true } = props;
  const { t } = useTranslation();
  const containerRef = useRef<HTMLDivElement>(null);
  // 命令项 ref 数组，用于自动滚动到高亮项
  const itemRefs = useRef<Array<HTMLDivElement | null>>([]);

  // 点击菜单外部时关闭菜单
  const handleClickOutside = useCallback((e: MouseEvent) => {
    if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
      onClose();
    }
  }, [onClose]);

  // 延迟添加 mousedown 监听，避免触发当前点击事件立即关闭
  useEffect(() => {
    const timer = setTimeout(() => {
      document.addEventListener("mousedown", handleClickOutside);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, [handleClickOutside]);

  // 高亮项变化时自动滚动到视口
  useEffect(() => {
    const el = itemRefs.current[highlightIndex];
    if (el) {
      el.scrollIntoView({ block: "nearest" });
    }
  }, [highlightIndex]);

  // 点击命令项：禁用项不触发选择
  const handleSelect = (cmd: SlashCommand) => {
    if (agentRunning && !cmd.allowedInAgent) return;
    onSelect(cmd);
  };

  return (
    <div
      ref={containerRef}
      className={`slash-menu-container ${dropdownUp ? "slash-menu-up" : "slash-menu-down"}`}
      role="listbox"
      aria-label={t("slash.menu.title")}
    >
      <div className="slash-menu-header">
        <span className="slash-menu-title">{t("slash.menu.title")}</span>
        <span className="slash-menu-hint">{t("slash.menu.hint")}</span>
      </div>

      <div className="slash-menu-list">
        {commands.length === 0 ? (
          <div className="slash-menu-empty">{t("slash.menu.empty")}</div>
        ) : (
          commands.map((cmd, idx) => {
            const isHighlighted = idx === highlightIndex;
            const isDisabled = agentRunning && !cmd.allowedInAgent;
            return (
              <div
                key={cmd.name}
                ref={(el) => { itemRefs.current[idx] = el; }}
                className={`slash-menu-item ${isHighlighted ? "highlighted" : ""} ${isDisabled ? "disabled" : ""}`}
                role="option"
                aria-selected={isHighlighted}
                aria-disabled={isDisabled}
                // 阻止 mousedown 默认行为，防止点击菜单项时输入框失焦
                onMouseDown={(e) => e.preventDefault()}
                onClick={() => handleSelect(cmd)}
              >
                <div className="slash-menu-item-row">
                  <div className="slash-menu-item-name">/{cmd.name}</div>
                  <div className="slash-menu-item-desc">{t(cmd.description)}</div>
                </div>
                {cmd.requiresArgs && (
                  <div className="slash-menu-item-usage">{cmd.usage}</div>
                )}
                {isDisabled && (
                  <div className="slash-menu-item-disabled">{t("slash.menu.disabledInAgent")}</div>
                )}
              </div>
            );
          })
        )}
      </div>

      <style>{`
        .slash-menu-container {
          position: absolute;
          left: 0;
          width: max-content;
          min-width: 320px;
          max-width: 480px;
          background: var(--color-bg-elevated);
          border: 1px solid var(--color-border-light);
          border-radius: var(--radius-md);
          box-shadow: var(--shadow-lg);
          z-index: 200;
          overflow: hidden;
          display: flex;
          flex-direction: column;
        }
        .slash-menu-container.slash-menu-up {
          bottom: calc(100% + 6px);
          animation: slash-menu-in-up 0.15s ease-out;
        }
        .slash-menu-container.slash-menu-down {
          top: calc(100% + 6px);
          animation: slash-menu-in-down 0.15s ease-out;
        }
        @keyframes slash-menu-in-up {
          from { opacity: 0; transform: scale(0.96) translateY(4px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }
        @keyframes slash-menu-in-down {
          from { opacity: 0; transform: scale(0.96) translateY(-4px); }
          to { opacity: 1; transform: scale(1) translateY(0); }
        }
        .slash-menu-header {
          display: flex;
          align-items: center;
          justify-content: space-between;
          gap: 8px;
          padding: 8px 12px;
          border-bottom: 1px solid var(--color-border-light);
          flex-shrink: 0;
        }
        .slash-menu-title {
          font-size: 11px;
          font-weight: 600;
          color: var(--color-text-quaternary);
          text-transform: uppercase;
          letter-spacing: 0.04em;
        }
        .slash-menu-hint {
          font-size: 10px;
          color: var(--color-text-quaternary);
          white-space: nowrap;
        }
        .slash-menu-list {
          overflow-y: auto;
          padding: 4px;
          display: flex;
          flex-direction: column;
          gap: 1px;
          scrollbar-width: none;
        }
        .slash-menu-list::-webkit-scrollbar {
          display: none;
          width: 0;
        }
        .slash-menu-empty {
          padding: 20px 16px;
          text-align: center;
          font-size: 12px;
          color: var(--color-text-quaternary);
        }
        .slash-menu-item {
          padding: 6px 10px;
          border-radius: var(--radius-sm);
          cursor: pointer;
          transition: background 0.12s;
        }
        .slash-menu-item:hover {
          background: var(--color-bg-hover);
        }
        .slash-menu-item.highlighted {
          background: var(--color-accent-bg);
        }
        .slash-menu-item.disabled {
          opacity: 0.5;
          cursor: not-allowed;
        }
        .slash-menu-item.disabled:hover {
          background: transparent;
        }
        .slash-menu-item-row {
          display: flex;
          align-items: baseline;
          gap: 10px;
        }
        .slash-menu-item-name {
          font-family: var(--font-mono);
          font-size: 12px;
          font-weight: 600;
          color: var(--color-text-primary);
          flex-shrink: 0;
        }
        .slash-menu-item-desc {
          font-size: 11px;
          color: var(--color-text-secondary);
          overflow: hidden;
          text-overflow: ellipsis;
          white-space: nowrap;
        }
        .slash-menu-item-usage {
          font-family: var(--font-mono);
          font-size: 10px;
          color: var(--color-text-quaternary);
        }
        .slash-menu-item-disabled {
          font-size: 10px;
          color: var(--color-error);
          margin-top: 1px;
        }
      `}</style>
    </div>
  );
}
