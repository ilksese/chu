import { startTransition, type ReactNode } from "react";
import { Box, Check, ChevronRight, CircleAlert, Command, RefreshCw, X } from "lucide-react";
import { useLocation, useNavigate } from "react-router";
import { navigation, viewFromPath } from "@/app/navigation";
import { StatusDot } from "@/components/host-controls";
import { Dock, DockIcon } from "@/components/ui/dock";
import { useAppStore } from "@/stores/app-store";

export function AppShell({ children }: { children: ReactNode }) {
  const location = useLocation();
  const navigate = useNavigate();
  const view = viewFromPath(location.pathname);
  const snapshot = useAppStore((state) => state.snapshot);
  const busy = useAppStore((state) => state.busy);
  const notice = useAppStore((state) => state.notice);
  const demo = useAppStore((state) => state.demo);
  const refresh = useAppStore((state) => state.refresh);
  const clearNotice = useAppStore((state) => state.clearNotice);
  const installedHosts = snapshot.hosts.filter((host) => host.installed).length;

  function selectView(path: string) {
    startTransition(() => {
      if (location.pathname !== path) navigate(path);
    });
  }

  return (
    <>
      <a className="skip-link" href="#main-content">
        跳到主要内容
      </a>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand" aria-label="Chu Agent Manager">
            <span className="brand-mark">
              <Command />
            </span>
            <strong>CHU</strong>
          </div>
          <nav aria-label="主导航">
            <Dock orientation="vertical" direction="middle" className="side-dock">
              {navigation.map((item) => {
                const Icon = item.icon;
                const count =
                  item.id === "skills"
                    ? snapshot.skills.length
                    : item.id === "mcps"
                      ? snapshot.mcps.length
                      : item.id === "agents"
                        ? snapshot.agents.length
                        : undefined;
                return (
                  <DockIcon key={item.id}>
                    <button
                      type="button"
                      className="dock-button"
                      aria-current={view === item.id ? "page" : undefined}
                      title={item.label}
                      onClick={() => selectView(item.path)}
                    >
                      <Icon />
                      {count !== undefined ? (
                        <span className="nav-count" aria-hidden="true">
                          {count}
                        </span>
                      ) : null}
                      <span className="dock-label">{item.label}</span>
                    </button>
                  </DockIcon>
                );
              })}
            </Dock>
          </nav>
          <div className="sidebar-footer" title={`中央仓库 ${snapshot.root}`}>
            <Box />
            <span>LOCAL</span>
          </div>
        </aside>

        <main className="workspace" id="main-content">
          <header className="topbar">
            <div className="breadcrumbs">
              <span>Chu</span>
              <ChevronRight />
              <strong>{navigation.find((item) => item.id === view)?.label}</strong>
            </div>
            <div className="topbar-actions">
              <span className="sync-state">
                <StatusDot ready={installedHosts > 0} />
                {installedHosts} 个宿主在线
              </span>
              {demo ? <span className="preview-badge">预览模式</span> : null}
              <button
                type="button"
                className="icon-button"
                aria-label="刷新扫描"
                title="刷新扫描"
                disabled={busy === "refresh"}
                onClick={() => void refresh()}
              >
                <RefreshCw className={busy === "refresh" ? "spin" : ""} />
              </button>
              <span className="profile" title="本地用户">
                LD
              </span>
            </div>
          </header>

          {notice ? (
            <div className={`notice ${notice.error ? "notice-error" : ""}`} role="status">
              {notice.error ? <CircleAlert /> : <Check />}
              <span>{notice.message}</span>
              <button type="button" aria-label="关闭提示" onClick={clearNotice}>
                <X />
              </button>
            </div>
          ) : null}
          {children}
        </main>
      </div>
    </>
  );
}
