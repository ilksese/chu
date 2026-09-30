import { useEffect, useLayoutEffect } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router";
import { viewPaths } from "@/app/navigation";
import { AppShell } from "@/components/AppShell";
import { OverviewPage } from "@/pages/overview-page";
import { PromptsPage } from "@/pages/prompts-page";
import { ProjectsPage } from "@/pages/projects-page";
import { ProvidersPage } from "@/pages/providers-page";
import { ReferencesPage } from "@/pages/references-page";
import { ResourcePage } from "@/pages/resource-page";
import { SettingsPage } from "@/pages/settings-page";
import { useAppStore } from "@/stores/appStore";
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
      <div className="mx-auto w-[min(1280px,calc(100%-64px))] py-8 pb-14" key={location.pathname}>
        <Routes location={location}>
          <Route path={viewPaths.overview} element={<OverviewPage />} />
          <Route path={viewPaths.projects} element={<ProjectsPage />} />
          <Route path={viewPaths.skills} element={<ResourcePage kind="skills" />} />
          <Route path={viewPaths.mcps} element={<ResourcePage kind="mcps" />} />
          <Route path={viewPaths.providers} element={<ProvidersPage />} />
          <Route path={viewPaths.agents} element={<ResourcePage kind="agents" />} />
          <Route path={viewPaths.prompts} element={<PromptsPage />} />
          <Route path={viewPaths.references} element={<ReferencesPage />} />
          <Route path={viewPaths.settings} element={<SettingsPage />} />
          <Route path="*" element={<Navigate to={viewPaths.overview} replace />} />
        </Routes>
      </div>
    </AppShell>
  );
}

export default App;
