import { useEffect, useLayoutEffect } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router";
import { viewPaths } from "@/app/navigation";
import { AppShell } from "@/components/app-shell";
import { OverviewPage } from "@/pages/overview-page";
import { ResourcePage } from "@/pages/resource-page";
import { SettingsPage } from "@/pages/settings-page";
import { useAppStore } from "@/stores/app-store";
import "@/theme.css";

function log(message: string) {
  const line = `[chu] ${message}`;
  console.info(line);
  window.runtime?.LogInfo?.(line);
}

function App() {
  const location = useLocation();
  const initialize = useAppStore((state) => state.initialize);
  const hosts = useAppStore((state) => state.snapshot.hosts);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  useLayoutEffect(() => {
    const page = document.querySelector(".page");
    const style = page ? getComputedStyle(page) : undefined;
    log(
      `route=${location.pathname} hosts=${hosts.length} page=${page?.childElementCount ?? "missing"} opacity=${style?.opacity ?? "-"} visibility=${style?.visibility ?? "-"} height=${page?.getBoundingClientRect().height ?? 0}`,
    );
  }, [location.pathname, hosts]);

  return (
    <AppShell>
      <div className="page" key={location.pathname}>
        <Routes location={location}>
          <Route path={viewPaths.overview} element={<OverviewPage />} />
          <Route path={viewPaths.skills} element={<ResourcePage kind="skills" />} />
          <Route path={viewPaths.mcps} element={<ResourcePage kind="mcps" />} />
          <Route path={viewPaths.agents} element={<ResourcePage kind="agents" />} />
          <Route path={viewPaths.settings} element={<SettingsPage />} />
          <Route path="*" element={<Navigate to={viewPaths.overview} replace />} />
        </Routes>
      </div>
    </AppShell>
  );
}

export default App;
