import { useState, useCallback, useEffect } from "react";
import { TopBar } from "./components/layout/TopBar";
import { MainLayout } from "./components/layout/MainLayout";
import { MainArea } from "./components/layout/MainArea";
import { InputArea } from "./components/layout/InputArea";
import { WorkflowTimeline } from "./components/workflow/WorkflowTimeline";
import { FileTreeSection } from "./components/sidebar/FileTreeSection";
import { AgentInfoSection } from "./components/sidebar/AgentInfoSection";
import { TodoSection } from "./components/sidebar/TodoSection";
import { TokenSection } from "./components/sidebar/TokenSection";
import { PreviewOverlay } from "./components/preview/PreviewOverlay";
import { SettingsDialog } from "./components/settings/SettingsDialog";
import { HistoryPanel } from "./components/session/HistoryPanel";
import { useWorkflowStore } from "./stores/useWorkflowStore";
import { useSessionStore } from "./stores/useSessionStore";
import { useSettingsStore } from "./stores/useSettingsStore";
import { useWorkspaceStore } from "./stores/useWorkspaceStore";
import { useTokenStore } from "./stores/useTokenStore";
import { useAgent } from "./hooks/useAgent";
import { onTokenUpdate } from "./services/event";

export default function App() {
  const [historyOpen, setHistoryOpen] = useState(false);
  const [previewOpen, setPreviewOpen] = useState(false);
  const [templateLabel, setTemplateLabel] = useState<string | undefined>(undefined);

  const { addNode, setExecutionStatus, clearNodes } = useWorkflowStore();
  const { loadSessions } = useSessionStore();
  const { loadSettings } = useSettingsStore();
  const { loadWorkspaces } = useWorkspaceStore();
  const { addTokenUsage } = useTokenStore();

  const {
    error: agentError,
    lastThinking,
    content,
    currentToolCall,
    lastToolResult,
    pendingConfirmation,
    todos,
    doneResult,
    sendMessage,
    reset: resetAgent,
  } = useAgent();

  // 应用初始化：加载后端数据
  useEffect(() => {
    loadSettings();
    loadWorkspaces();
    loadSessions();
  }, []);

  // 监听 Token 用量更新事件
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    onTokenUpdate((payload) => {
      addTokenUsage(payload.promptTokens, payload.completionTokens);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [addTokenUsage]);

  // Agent 事件 -> WorkflowStore 节点映射：思考过程
  useEffect(() => {
    if (lastThinking) {
      addNode("thinking", {
        content: lastThinking.thought,
        duration: 0,
      }, "running");
    }
  }, [lastThinking, addNode]);

  // Agent 事件 -> WorkflowStore 节点映射：Tool 调用开始
  useEffect(() => {
    if (currentToolCall) {
      addNode("tool", {
        toolName: currentToolCall.toolName,
        input: currentToolCall.arguments,
      }, "running");
    }
  }, [currentToolCall, addNode]);

  // Agent 事件 -> WorkflowStore 节点映射：Tool 执行结果
  useEffect(() => {
    if (lastToolResult) {
      addNode("result", {
        content: lastToolResult.success
          ? JSON.stringify(lastToolResult.result)
          : lastToolResult.error || "执行失败",
        success: lastToolResult.success,
        filePaths: [],
      });
    }
  }, [lastToolResult, addNode]);

  // Agent 事件 -> WorkflowStore 节点映射：执行完成
  useEffect(() => {
    if (doneResult) {
      addNode("reply", {
        content: doneResult.summary || content,
      });
      setExecutionStatus("completed");
    }
  }, [doneResult, content, addNode, setExecutionStatus]);

  // Agent 事件 -> WorkflowStore 节点映射：执行错误
  useEffect(() => {
    if (agentError) {
      setExecutionStatus("failed");
    }
  }, [agentError, setExecutionStatus]);

  // Agent 事件 -> WorkflowStore 节点映射：需要用户确认
  useEffect(() => {
    if (pendingConfirmation) {
      addNode("confirm", {
        title: pendingConfirmation.operationType,
        description: pendingConfirmation.description,
        confirmLabel: "确认执行",
        cancelLabel: "取消操作",
        confirmed: null,
      }, "running");
    }
  }, [pendingConfirmation, addNode]);

  // 发送用户消息
  const handleSend = useCallback(async (text: string) => {
    if (!text.trim()) return;

    // 添加用户节点
    addNode("user", { content: text, attachments: [] });
    setExecutionStatus("running");

    try {
      await sendMessage(text);
    } catch (err) {
      console.error("[App] 发送消息失败:", err);
      setExecutionStatus("failed");
    }
  }, [addNode, setExecutionStatus, sendMessage]);

  // 新建会话
  const handleNewSession = useCallback(() => {
    clearNodes();
    resetAgent();
  }, [clearNodes, resetAgent]);

  // 监听键盘快捷键
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        setPreviewOpen(false);
      }
      if (e.ctrlKey && e.key === "n") {
        e.preventDefault();
        handleNewSession();
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, [handleNewSession]);

  return (
    <div className="app flex flex-col h-screen">
      <TopBar
        onToggleHistory={() => setHistoryOpen(!historyOpen)}
        onNewSession={handleNewSession}
      />

      <MainLayout
        mainArea={
          <MainArea
            workflow={<WorkflowTimeline />}
            inputArea={
              <InputArea
                onSend={handleSend}
                templateLabel={templateLabel}
                onToggleTemplate={() => setTemplateLabel(templateLabel ? undefined : "生成周报")}
              />
            }
          />
        }
        sidebar={
          <>
            <FileTreeSection />
            <AgentInfoSection />
            <TodoSection
              items={todos?.todos.map((t) => ({
                id: t.id,
                text: t.content,
                done: t.status === "completed",
                active: t.status === "running",
              }))}
            />
            <TokenSection />
          </>
        }
      />

      {/* 浮层面板 */}
      <PreviewOverlay open={previewOpen} onClose={() => setPreviewOpen(false)} />
      <SettingsDialog />
      <HistoryPanel open={historyOpen} onClose={() => setHistoryOpen(false)} />

      <style>{`
        .app { display: flex; flex-direction: column; height: 100vh; }
        .topbar-btn {
          width: 34px; height: 34px; border-radius: var(--radius-sm);
          display: flex; align-items: center; justify-content: center;
          transition: background 0.15s; color: var(--color-text-secondary);
        }
        .topbar-btn:hover { background: var(--color-bg-sub); color: var(--color-text-primary); }
        .input-btn {
          width: 32px; height: 32px; border-radius: var(--radius-sm);
          display: flex; align-items: center; justify-content: center;
          transition: background 0.15s; color: var(--color-text-tertiary);
        }
        .input-btn:hover { background: var(--color-bg-sub); color: var(--color-text-secondary); }
      `}</style>
    </div>
  );
}
