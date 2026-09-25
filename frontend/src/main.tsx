import React, { Component, type ErrorInfo, type ReactNode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router";
import App from "@/App";
import { startDebugBridge } from "@/lib/debug-bridge";

function log(level: "info" | "error", message: string) {
  const line = `[chu] ${message}`;
  console[level](line);
  const runtime = (window as Window & { runtime?: Record<string, (value: string) => void> }).runtime;
  runtime?.[level === "error" ? "LogError" : "LogInfo"]?.(line);
}

window.addEventListener("error", (event) => {
  log("error", `window.error ${event.message} @ ${event.filename}:${event.lineno}:${event.colno}`);
});
window.addEventListener("unhandledrejection", (event) => {
  log("error", `unhandledrejection ${String(event.reason)}`);
});

class RootErrorBoundary extends Component<{ children: ReactNode }, { error?: string }> {
  state: { error?: string } = {};

  static getDerivedStateFromError(error: Error) {
    return { error: error.stack || error.message };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    log("error", `react ${error.stack || error.message} ${info.componentStack}`);
  }

  render() {
    if (!this.state.error) return this.props.children;
    return (
      <pre style={{ margin: 24, whiteSpace: "pre-wrap", color: "#111", fontSize: 12 }}>
        {this.state.error}
      </pre>
    );
  }
}

const container = document.getElementById("root");

const root = createRoot(container!);

log("info", `boot href=${location.href} go=${Boolean(window.go?.main?.App)} debug=${import.meta.env.VITE_CHU_DEBUG_WS || "off"}`);
startDebugBridge();

root.render(
  <React.StrictMode>
    <RootErrorBoundary>
      <BrowserRouter>
        <App />
      </BrowserRouter>
    </RootErrorBoundary>
  </React.StrictMode>
);
