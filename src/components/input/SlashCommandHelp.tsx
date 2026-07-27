import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useSlashCommandStore } from "@/stores/useSlashCommandStore";
import { SLASH_COMMANDS } from "@/commands/slashCommands";
import { Icon } from "@/components/common/Icon";
import { Button } from "@/components/common/Button";

/**
 * 斜杠命令帮助覆盖层
 * 通过 useSlashCommandStore 控制 helpOverlayOpen 状态
 * 展示所有斜杠命令的用法、描述和 Agent 运行时可用性
 */
export function SlashCommandHelp() {
  const { t } = useTranslation();
  const { helpOverlayOpen, closeHelpOverlay } = useSlashCommandStore();

  // 监听 Esc 键关闭覆盖层
  useEffect(() => {
    if (!helpOverlayOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        closeHelpOverlay();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [helpOverlayOpen, closeHelpOverlay]);

  if (!helpOverlayOpen) return null;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
      onClick={closeHelpOverlay}
    >
      <div
        className="relative flex max-h-[80vh] w-full max-w-2xl flex-col overflow-hidden rounded-lg border border-border-light bg-bg-elevated shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        {/* 标题栏 */}
        <div className="flex items-center justify-between border-b border-border-light px-6 py-4">
          <h2 className="text-base font-bold text-text-primary">
            {t("slash.help.title")}
          </h2>
          <button
            type="button"
            className="flex h-8 w-8 items-center justify-center rounded-sm text-text-tertiary transition-colors hover:bg-bg-hover hover:text-text-primary"
            onClick={closeHelpOverlay}
            aria-label={t("slash.help.close")}
          >
            <Icon name="close" size={18} />
          </button>
        </div>

        {/* 表格区域：可滚动，表头固定 */}
        <div className="flex-1 overflow-y-auto">
          <table className="w-full border-collapse">
            <thead className="sticky top-0 z-10 bg-bg-elevated">
              <tr>
                <th className="border-b border-border-light px-4 py-2 text-left text-xs font-semibold text-text-secondary">
                  {t("slash.help.command")}
                </th>
                <th className="border-b border-border-light px-4 py-2 text-left text-xs font-semibold text-text-secondary">
                  {t("slash.help.usage")}
                </th>
                <th className="border-b border-border-light px-4 py-2 text-left text-xs font-semibold text-text-secondary">
                  {t("slash.help.description")}
                </th>
                <th className="border-b border-border-light px-4 py-2 text-left text-xs font-semibold text-text-secondary">
                  {t("slash.help.availability")}
                </th>
              </tr>
            </thead>
            <tbody>
              {SLASH_COMMANDS.map((cmd) => (
                <tr
                  key={cmd.name}
                  className="border-b border-border-light last:border-b-0 hover:bg-bg-sub"
                >
                  <td className="whitespace-nowrap px-4 py-2 font-mono text-xs text-text-primary">
                    /{cmd.name}
                  </td>
                  <td className="whitespace-nowrap px-4 py-2 font-mono text-xs text-text-tertiary">
                    {cmd.usage}
                  </td>
                  <td className="px-4 py-2 text-xs text-text-primary">
                    {t(cmd.description)}
                  </td>
                  <td className="whitespace-nowrap px-4 py-2 text-xs">
                    {cmd.allowedInAgent ? (
                      <span className="text-text-tertiary">
                        {t("slash.help.allowed")}
                      </span>
                    ) : (
                      <span className="text-error">
                        {t("slash.help.disabled")}
                      </span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        {/* 底部操作栏 */}
        <div className="flex justify-end border-t border-border-light px-6 py-3">
          <Button variant="ghost" size="md" onClick={closeHelpOverlay}>
            {t("slash.help.close")}
          </Button>
        </div>
      </div>
    </div>
  );
}
