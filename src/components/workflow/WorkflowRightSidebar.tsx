import { useState, useEffect, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { useWorkflowStore } from '../../stores/useWorkflowStore';
import { useSessionStore } from '../../stores/useSessionStore';
import { useFileTreeStore } from '../../stores/useFileTreeStore';
import { listAllBranchUserMessages } from '../../services/tauri';
import { Icon } from '../common/Icon';
import { CustomScrollArea } from '../common/CustomScrollArea';
import { FileTreeSection } from '../sidebar/FileTreeSection';
import type { UserNodeData } from '../../types/workflow';
import type { BranchUserMessage } from '../../types/session';

/** 格式化 Token 数量为可读字符串 */
function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) {
    return `${(tokens / 1_000_000).toFixed(1)}M`;
  }
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}K`;
  }
  return String(tokens);
}

/** 根据使用百分比返回对应的显示信息 */
function getUsageInfo(usagePercent: number, t: (key: string) => string): { label: string; color: string } {
  if (usagePercent >= 95) {
    return { label: t('contextWindow.approachingLimit'), color: "var(--color-error)" };
  } else if (usagePercent >= 80) {
    return { label: t('contextWindow.normal'), color: "var(--color-warning)" };
  } else {
    return { label: t('contextWindow.normal'), color: "var(--color-success)" };
  }
}

const CONTEXT_SECTIONS = [
  { key: "system", labelKey: "contextWindow.systemPrompt", colorVar: "--color-context-system" },
  { key: "functions", labelKey: "contextWindow.toolDefinitions", colorVar: "--color-context-functions" },
  { key: "history", labelKey: "contextWindow.conversationHistory", colorVar: "--color-context-history" },
  { key: "response", labelKey: "contextWindow.llmResponse", colorVar: "--color-context-response" },
] as const;

function CacheHitRateDisplay({ hitRate }: { hitRate: number }) {
  const { t } = useTranslation();
  const percent = Math.round(hitRate * 100);
  const color =
    percent >= 70
      ? "var(--color-success)"
      : percent >= 40
        ? "var(--color-warning)"
        : "var(--color-error)";

  return (
    <div className="context-panel-cache">
      <span className="context-panel-cache-label">{t('contextWindow.cacheHitRate')}</span>
      <span className="context-panel-cache-value" style={{ color }}>{percent}%</span>
    </div>
  );
}

function ContextPanel() {
  const { t } = useTranslation();
  const contextUsage = useWorkflowStore((s) => s.contextUsage);

  if (!contextUsage) {
    return (
      <div className="context-panel">
        <div className="context-panel-empty">{t('contextWindow.noData')}</div>
      </div>
    );
  }

  const {
    contextWindow,
    systemPromptTokens,
    functionDefinitionsTokens,
    conversationTokens,
    responseTokens,
    totalUsedTokens,
    totalMessageCount,
    cacheHitRate,
    providerCacheType,
  } = contextUsage;

  const usagePercent = contextWindow > 0 ? Math.round((totalUsedTokens / contextWindow) * 100) : 0;
  const usageInfo = getUsageInfo(usagePercent, t);
  const systemPct = contextWindow > 0 ? (systemPromptTokens / contextWindow) * 100 : 0;
  const funcPct = contextWindow > 0 ? (functionDefinitionsTokens / contextWindow) * 100 : 0;
  const convPct = contextWindow > 0 ? (conversationTokens / contextWindow) * 100 : 0;
  const respPct = contextWindow > 0 ? (responseTokens / contextWindow) * 100 : 0;
  const sectionTokens = [systemPromptTokens, functionDefinitionsTokens, conversationTokens, responseTokens];
  const sectionPcts = [systemPct, funcPct, convPct, respPct];

  return (
    <div className="context-panel">
      <div className="context-panel-header">
        <span className="context-panel-label">{t('contextWindow.sectionTitle')}</span>
        <span className="context-panel-value" style={usagePercent >= 95 ? { color: "var(--color-error)" } : undefined}>
          {formatTokens(totalUsedTokens)} / {formatTokens(contextWindow)}
        </span>
      </div>

      <div className="context-panel-bar-track">
        {CONTEXT_SECTIONS.map((section, i) => (
          <div
            key={section.key}
            className="context-panel-bar-segment"
            style={{ width: `${sectionPcts[i]}%`, background: `var(${section.colorVar})` }}
            title={`${t(section.labelKey)}: ${formatTokens(sectionTokens[i])} (${sectionPcts[i].toFixed(1)}%)`}
          />
        ))}
      </div>

      <div className="context-panel-footer">
        <span className="context-panel-status" style={{ color: usageInfo.color }}>
          {usageInfo.label}
        </span>
        <span className="context-panel-percent">{usagePercent}%</span>
      </div>

      {usagePercent >= 80 && (
        <div className="context-panel-warning">
          <span className="context-panel-dot" />
          <span>
            {t('contextWindow.approachingLimitDetail', { total: totalMessageCount })}
          </span>
        </div>
      )}

      {providerCacheType !== "none" && <CacheHitRateDisplay hitRate={cacheHitRate} />}
    </div>
  );
}

type RightSidebarTab = "branches" | "context" | "files";

interface WorkflowRightSidebarProps {
  /** 是否处于收起状态（由父组件控制，用于触发滑入/滑出动画） */
  collapsed?: boolean;
}

/**
 * 工作流右侧边栏：分支导航 + 上下文窗口
 * - 顶部 Tab 栏在「分支导航」和「上下文窗口」之间切换
 * - 分支导航：展示 user 节点列表、分支组指示按钮、分支切换、搜索功能
 * - 上下文窗口：展示 Token 使用情况、进度条、缓存命中率
 * - 滑入/滑出动画：外层控制 width，内层控制 transform，避免内容被压缩
 */
export function WorkflowRightSidebar({ collapsed = false }: WorkflowRightSidebarProps) {
  const { t } = useTranslation();
  const [activeTab, setActiveTab] = useState<RightSidebarTab>("branches");
  const nodes = useWorkflowStore((s) => s.nodes);
  const currentVisibleNodeId = useWorkflowStore((s) => s.currentVisibleNodeId);
  const setRightSidebarVisible = useWorkflowStore((s) => s.setRightSidebarVisible);
  const jumpToNode = useWorkflowStore((s) => s.jumpToNode);
  const jumpToMessage = useWorkflowStore((s) => s.jumpToMessage);
  const executionStatus = useWorkflowStore((s) => s.executionStatus);
  const branchGroups = useWorkflowStore((s) => s.branchGroups);
  const activeBranchId = useWorkflowStore((s) => s.activeBranchId);
  const tabsRef = useRef<HTMLDivElement>(null);

  const [expandedGroupIds, setExpandedGroupIds] = useState<Set<string>>(new Set());
  const [isSearching, setIsSearching] = useState(false);
  const [searchQuery, setSearchQuery] = useState('');
  const [allBranchMessages, setAllBranchMessages] = useState<BranchUserMessage[]>([]);
  const [isLoadingMessages, setIsLoadingMessages] = useState(false);
  const [showRightFileSearch, setShowRightFileSearch] = useState(false);
  const [moreDropdownOpen, setMoreDropdownOpen] = useState(false);
  const moreBtnRef = useRef<HTMLButtonElement>(null);
  const moreDropdownRef = useRef<HTMLDivElement>(null);
  const fileSearchKeyword = useFileTreeStore((s) => s.searchKeyword);
  const setFileSearchKeyword = useFileTreeStore((s) => s.setSearchKeyword);
  const fileSearchInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!isSearching) return;
    const sessionId = useSessionStore.getState().currentSessionId;
    if (!sessionId) return;
    let cancelled = false;
    setIsLoadingMessages(true);
    listAllBranchUserMessages(sessionId)
      .then((messages) => {
        if (!cancelled) {
          setAllBranchMessages(messages);
        }
      })
      .catch((err) => {
        console.error('[WorkflowRightSidebar] 加载全分支用户消息失败:', err);
      })
      .finally(() => {
        if (!cancelled) setIsLoadingMessages(false);
      });
    return () => {
      cancelled = true;
    };
  }, [isSearching]);

  const toggleGroup = (groupId: string) => {
    setExpandedGroupIds((prev) => {
      const next = new Set(prev);
      if (next.has(groupId)) {
        next.delete(groupId);
      } else {
        next.add(groupId);
      }
      return next;
    });
  };

  const handleSwitchToBranch = async (branchId: string) => {
    if (executionStatus === 'running') return;
    try {
      await useSessionStore.getState().switchBranch(branchId);
    } catch (err) {
      console.error('[WorkflowRightSidebar] 切换分支失败:', err);
    }
  };

  /* 更多下拉菜单：搜索文件 */
  const handleMoreSearch = useCallback(() => {
    setShowRightFileSearch(true);
    setMoreDropdownOpen(false);
  }, []);

  /* 关闭文件搜索框 */
  const handleCloseFileSearch = useCallback(() => {
    setShowRightFileSearch(false);
    setFileSearchKeyword('');
  }, [setFileSearchKeyword]);

  /* 搜索框显示时自动聚焦 */
  useEffect(() => {
    if (showRightFileSearch && fileSearchInputRef.current) {
      fileSearchInputRef.current.focus();
    }
  }, [showRightFileSearch]);

  /* 更多下拉菜单：刷新文件树 */
  const handleMoreRefresh = useCallback(() => {
    const { activeWorkspaceId, loadTree } = useFileTreeStore.getState();
    if (activeWorkspaceId) {
      loadTree(activeWorkspaceId);
    }
    setMoreDropdownOpen(false);
  }, []);

  /* 点击外部关闭更多下拉菜单 */
  useEffect(() => {
    if (!moreDropdownOpen) return;
    const handleClick = (e: MouseEvent) => {
      if (
        moreBtnRef.current?.contains(e.target as Node) ||
        moreDropdownRef.current?.contains(e.target as Node)
      ) {
        return;
      }
      setMoreDropdownOpen(false);
    };
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setMoreDropdownOpen(false);
    };
    const timer = setTimeout(() => {
      document.addEventListener('mousedown', handleClick);
      document.addEventListener('keydown', handleKey);
    }, 0);
    return () => {
      clearTimeout(timer);
      document.removeEventListener('mousedown', handleClick);
      document.removeEventListener('keydown', handleKey);
    };
  }, [moreDropdownOpen]);

  const handleCloseSearch = () => {
    setIsSearching(false);
    setSearchQuery('');
    setAllBranchMessages([]);
  };

  const handleSearchResultClick = async (message: BranchUserMessage) => {
    if (executionStatus === 'running') return;
    if (message.branchId === activeBranchId) {
      jumpToMessage(message.messageId);
      return;
    }
    try {
      await useSessionStore.getState().switchBranch(message.branchId);
      const tryJump = (attempts: number) => {
        if (jumpToMessage(message.messageId)) return;
        if (attempts > 0) {
          requestAnimationFrame(() => tryJump(attempts - 1));
        }
      };
      setTimeout(() => tryJump(30), 100);
    } catch (err) {
      console.error('[WorkflowRightSidebar] 跨分支跳转失败:', err);
    }
  };

  const currentUserNodes = nodes.filter((n) => n.type === 'user');

  const searchResultsFromOtherBranches = searchQuery
    ? allBranchMessages.filter(
        (m) =>
          m.branchId !== activeBranchId &&
          m.content.toLowerCase().includes(searchQuery.toLowerCase()),
      )
    : [];

  const hasCurrentBranchMatches = searchQuery
    ? currentUserNodes.some((n) => {
        const data = n.data as UserNodeData;
        return (data.content || '').toLowerCase().includes(searchQuery.toLowerCase());
      })
    : false;

  return (
    <div className={`workflow-right-sidebar${collapsed ? ' collapsed' : ''}`}>
      <div className="workflow-right-sidebar-inner">
        {/* 顶部 Tab 栏 */}
        <div className="branch-graph-header">
          <div
            className="right-sidebar-tabs"
            ref={tabsRef}
            onWheel={(e) => {
              if (tabsRef.current) {
                tabsRef.current.scrollLeft += e.deltaY;
              }
            }}
          >
            <button
              className={`right-sidebar-tab${activeTab === "branches" ? " active" : ""}`}
              onClick={() => setActiveTab("branches")}
            >
              <Icon name="git-branch" size={12} />
              <span>{t('workflow.branchGraph')}</span>
            </button>
            <button
              className={`right-sidebar-tab${activeTab === "context" ? " active" : ""}`}
              onClick={() => setActiveTab("context")}
            >
              <Icon name="chart" size={12} />
              <span>{t('workflow.contextWindow')}</span>
            </button>
            <button
              className={`right-sidebar-tab${activeTab === "files" ? " active" : ""}`}
              onClick={() => setActiveTab("files")}
            >
              <Icon name="folder" size={12} />
              <span>{t('workflow.workspaceFiles')}</span>
            </button>
          </div>
          <div className="branch-graph-header-actions">
            <button
              className="branch-graph-close-btn"
              onClick={() => setRightSidebarVisible(false)}
              title={t('workflow.hideRightSidebar')}
            >
              <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
                <rect x="3" y="3" width="18" height="18" rx="2" />
                <line x1="9" y1="3" x2="9" y2="21" />
                <polyline points="15 10 18 13 15 16" />
              </svg>
            </button>
          </div>
        </div>

        {/* Tab 内容 */}
        {activeTab === "branches" ? (
          <CustomScrollArea className="branch-graph-content">
            {currentUserNodes.length === 0 && !isSearching ? (
              <div className="branch-graph-empty">{t('workflow.emptyWorkflow')}</div>
            ) : isLoadingMessages && isSearching ? (
              <div className="branch-graph-empty">{t('common.loading')}</div>
            ) : (
              <div className="branch-graph-padding">
                {isSearching ? (
                  <>
                    <div className="branch-graph-search-row" style={{ marginBottom: 8 }}>
                      <Icon name="search" size={12} className="branch-graph-search-icon" />
                      <input
                        type="text"
                        className="branch-graph-search-input"
                        placeholder={t('workflow.searchUserMessages')}
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        autoFocus
                      />
                      <button
                        className="branch-graph-close-btn"
                        onClick={handleCloseSearch}
                        title={t('common.cancel')}
                      >
                        <Icon name="close" size={14} />
                      </button>
                    </div>

                    {searchResultsFromOtherBranches.length > 0 && (
                      searchResultsFromOtherBranches.map((msg) => {
                        const summary = msg.content.length > 40 ? msg.content.slice(0, 40) + '...' : msg.content;
                        const sourceBranch = branchGroups
                          .flatMap((g) => g.branches)
                          .find((b) => b.branchId === msg.branchId);
                        const branchName = sourceBranch?.name || msg.branchId.slice(-8);
                        return (
                          <div
                            key={msg.messageId}
                            className="branch-graph-node other-branch"
                            onClick={() => void handleSearchResultClick(msg)}
                          >
                            <span className="branch-graph-node-index">
                              <Icon name="git-branch" size={11} />
                            </span>
                            <span className="branch-graph-node-content">{summary}</span>
                            <span className="branch-graph-node-branch-tag">{branchName}</span>
                          </div>
                        );
                      })
                    )}

                    {searchQuery &&
                      !hasCurrentBranchMatches &&
                      searchResultsFromOtherBranches.length === 0 && (
                        <div className="branch-graph-empty">{t('workflow.noSearchResults')}</div>
                      )}
                  </>
                ) : (
                  <button
                    className="branch-graph-search-toggle"
                    onClick={() => setIsSearching(true)}
                  >
                    <Icon name="search" size={12} />
                    <span>{t('workflow.searchUserMessages')}</span>
                  </button>
                )}

                <div className="branch-graph-search-spacer" />

                {currentUserNodes.map((node, index) => {
                  const data = node.data as UserNodeData;
                  const content = data.content || '';
                  const summary = content.length > 40 ? content.slice(0, 40) + '...' : (content || t('workflow.attachmentMessage'));
                  const isActive = currentVisibleNodeId === node.id;
                  const hasBranchGroup = !!(data.branchGroupId && data.branchTotal && data.branchTotal > 1);
                  const isExpanded = hasBranchGroup && expandedGroupIds.has(data.branchGroupId!);
                  const group = hasBranchGroup ? branchGroups.find((g) => g.branchGroupId === data.branchGroupId) : undefined;
                  const sortedBranches = group?.branches.slice().sort((a, b) => a.sortOrder - b.sortOrder) || [];
                  const nodeClass = `branch-graph-node${isActive ? ' active' : ''}`;
                  return (
                    <div
                      key={node.id}
                      className={nodeClass}
                      data-node-id={node.id}
                      onClick={() => jumpToNode(node.id)}
                    >
                      <span className="branch-graph-node-index">{index + 1}</span>
                      <span className="branch-graph-node-content">{summary}</span>
                      {hasBranchGroup && (
                        <div
                          className="branch-group-indicator"
                          onClick={(e) => {
                            e.stopPropagation();
                            toggleGroup(data.branchGroupId!);
                          }}
                        >
                          <Icon name="git-branch" size={11} />
                          <span>{t('workflow.branchesCount', { count: data.branchTotal })}</span>
                        </div>
                      )}
                      {isExpanded && sortedBranches.length > 0 && (
                        <div className="branch-group-list">
                          {sortedBranches.map((b) => {
                            const isCurrent = b.branchId === activeBranchId;
                            return (
                              <div
                                key={b.branchId}
                                className={`branch-group-item${isCurrent ? ' active' : ' disabled'}`}
                                onClick={(e) => {
                                  e.stopPropagation();
                                  void handleSwitchToBranch(b.branchId);
                                }}
                              >
                                {b.name}
                              </div>
                            );
                          })}
                        </div>
                      )}
                    </div>
                  );
                })}
              </div>
            )}
          </CustomScrollArea>
        ) : activeTab === "context" ? (
          <CustomScrollArea className="branch-graph-content">
            <ContextPanel />
          </CustomScrollArea>
        ) : (
          <div className="workspace-files-panel">
            <div className="workspace-files-header">
              {showRightFileSearch ? (
                <div className="workspace-files-search-row">
                  <Icon name="search" size={12} className="workspace-files-search-icon" />
                  <input
                    type="text"
                    className="workspace-files-search-input"
                    placeholder={t('fileTree.searchPlaceholder')}
                    aria-label={t('fileTree.searchFile')}
                    value={fileSearchKeyword}
                    onChange={(e) => setFileSearchKeyword(e.target.value)}
                    ref={fileSearchInputRef}
                  />
                  <button
                    className="workspace-files-search-close"
                    onClick={handleCloseFileSearch}
                    title={t('common.close')}
                  >
                    <Icon name="close" size={12} />
                  </button>
                </div>
              ) : (
                <div className="workspace-files-header-spacer" />
              )}
              <div className="workspace-files-actions">
                <button
                  className="workspace-files-more-btn"
                  ref={moreBtnRef}
                  onClick={() => setMoreDropdownOpen((v) => !v)}
                  title={t('sidebar.more')}
                  aria-label={t('sidebar.more')}
                >
                  <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
                    <circle cx="12" cy="5" r="2" />
                    <circle cx="12" cy="12" r="2" />
                    <circle cx="12" cy="19" r="2" />
                  </svg>
                </button>
                {moreDropdownOpen && (
                  <div className="workspace-files-dropdown" ref={moreDropdownRef}>
                    <button className="workspace-files-dropdown-item" onClick={handleMoreSearch}>
                      <Icon name="search" size={12} />
                      <span>{t('fileTree.searchFile')}</span>
                    </button>
                    <button className="workspace-files-dropdown-item" onClick={handleMoreRefresh}>
                      <Icon name="refresh" size={12} />
                      <span>{t('fileTree.refreshTree')}</span>
                    </button>
                  </div>
                )}
              </div>
            </div>
            <CustomScrollArea className="branch-graph-content">
              <FileTreeSection hideSearchBar />
            </CustomScrollArea>
          </div>
        )}
      </div>
    </div>
  );
}
