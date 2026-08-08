// 剪贴板写入工具
// 在 Tauri 桌面环境中，WebView2 的 navigator.clipboard API 存在已知问题：
// 写入内容只进入 WebView2 内部延迟剪贴板（应用内可粘贴），不会同步到系统剪贴板。
// 因此优先通过 tauri-plugin-clipboard-manager 的 Rust 命令直接写入系统剪贴板，
// 仅在非 Tauri 环境（浏览器调试）或插件调用失败时降级到 Web Clipboard API / execCommand。
import { writeText as tauriWriteText } from "@tauri-apps/plugin-clipboard-manager";

// 检测当前是否运行在 Tauri 环境中
const isTauriEnv = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/**
 * 复制文本到系统剪贴板
 * 优先级：Tauri 插件（系统剪贴板）→ navigator.clipboard → execCommand 降级
 */
export async function copyToClipboard(text: string): Promise<void> {
  // Tauri 环境：优先通过 Rust 侧写入系统剪贴板，绕过 WebView2 剪贴板限制
  if (isTauriEnv) {
    try {
      await tauriWriteText(text);
      return;
    } catch (err) {
      console.warn("[clipboard] Tauri 剪贴板插件写入失败，降级到 Web API:", err);
    }
  }

  // 降级一：Web Clipboard API（浏览器调试环境或插件不可用）
  try {
    if (navigator.clipboard?.writeText) {
      await navigator.clipboard.writeText(text);
      return;
    }
  } catch (err) {
    console.warn("[clipboard] navigator.clipboard 写入失败，降级到 execCommand:", err);
  }

  // 降级二：textarea + execCommand（兼容旧环境）
  try {
    const ta = document.createElement("textarea");
    ta.value = text;
    ta.style.position = "fixed";
    ta.style.opacity = "0";
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    document.body.removeChild(ta);
  } catch (err) {
    console.error("[clipboard] execCommand 复制失败:", err);
  }
}
