import { useEffect, ViewTransition } from "react";
import { Navigate, Route, Routes, useLocation } from "react-router";
import { viewPaths } from "@/app/navigation";
import { AppShell } from "@/components/app-shell";
import { OverviewPage } from "@/pages/overview-page";
import { ResourcePage } from "@/pages/resource-page";
import { SettingsPage } from "@/pages/settings-page";
import { useAppStore } from "@/stores/app-store";
import "@/theme.css";

function App() {
  const location = useLocation();
  const initialize = useAppStore((state) => state.initialize);

  useEffect(() => {
    void initialize();
  }, [initialize]);

  return (
    <AppShell>
      <ViewTransition default="chu-page">
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
      </ViewTransition>
    </AppShell>
  );
}

export default App;
