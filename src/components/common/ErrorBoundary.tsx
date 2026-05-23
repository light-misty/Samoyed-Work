import { Component, type ReactNode } from "react";
import { Icon } from "./Icon";

interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, errorInfo: React.ErrorInfo) => void;
}

interface ErrorBoundaryState {
  hasError: boolean;
  error: Error | null;
}

/**
 * React 错误边界组件
 * 捕获子组件树中的渲染错误，防止整个应用白屏
 * 可通过 fallback 属性自定义错误展示，或使用默认的错误页面
 * 使用 Tailwind CSS 样式，与应用设计系统一致
 */
export class ErrorBoundary extends Component<ErrorBoundaryProps, ErrorBoundaryState> {
  constructor(props: ErrorBoundaryProps) {
    super(props);
    this.state = { hasError: false, error: null };
  }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { hasError: true, error };
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo): void {
    console.error("[ErrorBoundary] 捕获到渲染错误:", error, errorInfo);
    this.props.onError?.(error, errorInfo);
  }

  handleReload = (): void => {
    this.setState({ hasError: false, error: null });
  };

  handleRestart = (): void => {
    window.location.reload();
  };

  render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      return (
        <div className="flex flex-col items-center justify-center h-screen p-10 bg-bg text-text-primary font-sans">
          <div className="max-w-[480px] text-center">
            {/* 错误图标 */}
            <div className="w-16 h-16 rounded-[16px] bg-error-light flex items-center justify-center mx-auto mb-6">
              <Icon name="error" size={28} className="text-error" />
            </div>

            <h2 className="text-lg font-semibold mb-2">
              页面渲染出错
            </h2>

            <p className="text-sm text-text-secondary leading-relaxed mb-6">
              应用遇到了一个意外错误，部分功能可能无法正常使用。
              你可以尝试恢复页面或重启应用。
            </p>

            {/* 错误详情（可折叠） */}
            {this.state.error && (
              <details className="mb-6 text-left bg-bg-sub rounded-[var(--radius-md)] p-3 text-xs font-mono text-error max-h-40 overflow-auto">
                <summary className="cursor-pointer font-medium mb-2 text-text-secondary font-sans text-[12px]">
                  错误详情
                </summary>
                <pre className="m-0 whitespace-pre-wrap break-words">
                  {this.state.error.message}
                  {this.state.error.stack && `\n\n${this.state.error.stack}`}
                </pre>
              </details>
            )}

            {/* 操作按钮 */}
            <div className="flex gap-3 justify-center">
              <button
                onClick={this.handleReload}
                className="px-5 py-2 rounded-[var(--radius-sm)] border border-border bg-bg text-text-primary text-[13px] font-medium cursor-pointer transition-all hover:bg-bg-sub"
              >
                恢复页面
              </button>
              <button
                onClick={this.handleRestart}
                className="px-5 py-2 rounded-[var(--radius-sm)] border-none bg-accent text-white text-[13px] font-medium cursor-pointer transition-all hover:brightness-90"
              >
                重启应用
              </button>
            </div>
          </div>
        </div>
      );
    }

    return this.props.children;
  }
}
