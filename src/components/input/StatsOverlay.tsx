import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSlashCommandStore } from "../../stores/useSlashCommandStore";
import { useSessionStore } from "../../stores/useSessionStore";
import { getContextUsage } from "../../services/tauri";
import type { ContextUsageInfo } from "../../types";
import { Icon } from "../common/Icon";

export function StatsOverlay() {
  const { t } = useTranslation();
  const { statsOverlayOpen, closeStatsOverlay } = useSlashCommandStore();
  const currentSessionId = useSessionStore((s) => s.currentSessionId);
  const [usageInfo, setUsageInfo] = useState<ContextUsageInfo | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!statsOverlayOpen) return;
    if (!currentSessionId) {
      setUsageInfo(null);
      setLoading(false);
      setError(null);
      return;
    }
    setLoading(true);
    setError(null);
    getContextUsage(currentSessionId)
      .then((info) => {
        setUsageInfo(info);
        setLoading(false);
      })
      .catch((err) => {
        setError(String(err));
        setLoading(false);
      });
  }, [statsOverlayOpen, currentSessionId]);

  useEffect(() => {
    if (!statsOverlayOpen) return;
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        closeStatsOverlay();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [statsOverlayOpen, closeStatsOverlay]);

  if (!statsOverlayOpen) return null;

  const formatTokens = (n: number): string => {
    if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
    return String(n);
  };

  const formatPercent = (rate: number): string => {
    if (!Number.isFinite(rate)) return "0.0%";
    return `${(rate * 100).toFixed(1)}%`;
  };

  const usageRatio =
    usageInfo && usageInfo.contextWindow > 0
      ? usageInfo.totalUsedTokens / usageInfo.contextWindow
      : 0;
  const progressWidth = Math.min(Math.max(usageRatio * 100, 0), 100);
  const inputTokens = usageInfo
    ? usageInfo.systemPromptTokens + usageInfo.functionDefinitionsTokens + usageInfo.conversationTokens
    : 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-overlay"
      onClick={closeStatsOverlay}
    >
      <div
        className="relative flex max-h-[80vh] w-full max-w-lg flex-col overflow-hidden rounded-lg border border-border-light bg-bg-elevated shadow-lg"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between border-b border-border-light px-6 py-4">
          <h2 className="text-base font-bold text-text-primary">
            <span className="inline-flex items-center gap-2">
              <Icon name="chart" size={16} />
              {t("slash.stats.title")}
            </span>
          </h2>
          <button
            type="button"
            className="flex h-8 w-8 items-center justify-center rounded-sm text-text-tertiary transition-colors hover:bg-bg-hover hover:text-text-primary"
            onClick={closeStatsOverlay}
            aria-label={t("slash.help.close")}
          >
            <Icon name="close" size={18} />
          </button>
        </div>

        <div className="flex-1 overflow-y-auto px-6 py-5">
          {loading ? (
            <div className="flex items-center justify-center py-12 text-sm text-text-tertiary">
              <span className="stats-overlay-spinner mr-2" />
              {t("common.loading")}
            </div>
          ) : error ? (
            <div className="py-8 text-center text-sm text-error">{error}</div>
          ) : !currentSessionId ? (
            <div className="py-8 text-center text-sm text-text-tertiary">{t("slash.stats.noData")}</div>
          ) : !usageInfo ? (
            <div className="py-8 text-center text-sm text-text-tertiary">{t("slash.stats.noData")}</div>
          ) : (
            <div className="flex flex-col gap-5">
              <div className="grid grid-cols-2 gap-4">
                <div className="flex flex-col gap-1">
                  <span className="text-xs text-text-tertiary">{t("slash.stats.model")}</span>
                  <span className="text-sm font-medium text-text-primary">{usageInfo.modelName}</span>
                </div>
                <div className="flex flex-col gap-1">
                  <span className="text-xs text-text-tertiary">{t("slash.stats.contextWindow")}</span>
                  <span className="text-sm font-medium text-text-primary">{formatTokens(usageInfo.contextWindow)}</span>
                </div>
                <div className="flex flex-col gap-1">
                  <span className="text-xs text-text-tertiary">{t("slash.stats.totalTokens")}</span>
                  <span className="text-sm font-medium text-text-primary">{formatTokens(usageInfo.totalUsedTokens)}</span>
                </div>
                <div className="flex flex-col gap-1">
                  <span className="text-xs text-text-tertiary">{t("slash.stats.messageCount")}</span>
                  <span className="text-sm font-medium text-text-primary">{usageInfo.totalMessageCount}</span>
                </div>
              </div>

              <div className="flex flex-col gap-1.5">
                <span className="text-xs text-text-tertiary">{t("slash.stats.usageRatio")}</span>
                <div className="flex items-center gap-3">
                  <div className="h-2 flex-1 overflow-hidden rounded-full bg-bg-sub">
                    <div
                      className="h-full rounded-full bg-accent transition-all duration-300"
                      style={{ width: `${progressWidth}%` }}
                    />
                  </div>
                  <span className="w-12 text-right text-xs font-medium text-text-primary">{formatPercent(usageRatio)}</span>
                </div>
              </div>

              <div className="flex flex-col gap-3 rounded-lg bg-bg-sub p-4">
                <div className="flex items-center justify-between">
                  <span className="text-xs text-text-tertiary">{t("slash.stats.inputTokens")}</span>
                  <span className="text-sm font-medium text-text-primary">{formatTokens(inputTokens)}</span>
                </div>
                <div className="flex items-center justify-between pl-3">
                  <span className="text-xs text-text-quaternary">{t("slash.stats.systemPrompt")}</span>
                  <span className="text-xs text-text-secondary">{formatTokens(usageInfo.systemPromptTokens)}</span>
                </div>
                <div className="flex items-center justify-between pl-3">
                  <span className="text-xs text-text-quaternary">{t("slash.stats.functionDefs")}</span>
                  <span className="text-xs text-text-secondary">{formatTokens(usageInfo.functionDefinitionsTokens)}</span>
                </div>
                <div className="flex items-center justify-between pl-3">
                  <span className="text-xs text-text-quaternary">{t("slash.stats.conversation")}</span>
                  <span className="text-xs text-text-secondary">{formatTokens(usageInfo.conversationTokens)}</span>
                </div>
                <div className="border-t border-border-light pt-3">
                  <div className="flex items-center justify-between">
                    <span className="text-xs text-text-tertiary">{t("slash.stats.outputTokens")}</span>
                    <span className="text-sm font-medium text-text-primary">{formatTokens(usageInfo.responseTokens)}</span>
                  </div>
                </div>
              </div>

              {usageInfo.lifetimeCacheHitTokens + usageInfo.lifetimeCacheMissTokens > 0 && (
                <div className="flex flex-col gap-1.5">
                  <span className="text-xs text-text-tertiary">{t("slash.stats.cacheHitRate")}</span>
                  <span className="text-sm font-medium text-text-primary">
                    {formatPercent(
                      usageInfo.lifetimeCacheHitTokens /
                        (usageInfo.lifetimeCacheHitTokens + usageInfo.lifetimeCacheMissTokens)
                    )}
                  </span>
                </div>
              )}
            </div>
          )}
        </div>

      </div>

      <style>{`
        .stats-overlay-spinner {
          width: 14px;
          height: 14px;
          border: 2px solid var(--color-border-strong);
          border-top-color: var(--color-text-tertiary);
          border-radius: 50%;
          animation: stats-spin 0.8s linear infinite;
        }
        @keyframes stats-spin {
          to { transform: rotate(360deg); }
        }
      `}</style>
    </div>
  );
}
