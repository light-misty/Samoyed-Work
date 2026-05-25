import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { ErrorBoundary } from "./components/common/ErrorBoundary";
import App from "./App";
import "./styles/globals.css";

// 禁用浏览器默认右键菜单，桌面端应用不应显示浏览器原生的右键菜单
window.addEventListener("contextmenu", (e) => e.preventDefault());

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <ErrorBoundary>
      <App />
    </ErrorBoundary>
  </StrictMode>
);
