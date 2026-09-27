import { startTransition, useEffect, type ReactNode } from "react";
import { tv } from "tailwind-variants";
import { Box, Check, ChevronRight, CircleAlert, Command, RefreshCw, X } from "lucide-react";
import { useLocation, useNavigate } from "react-router";
import { navigation, viewFromPath } from "@/app/navigation";
import { StatusDot } from "@/components/host-controls";
import { Dock, DockIcon } from "@/components/ui/dock";
import { useAppStore } from "@/stores/app-store";

const noticeStyle = tv({
  base: "fixed top-[76px] right-6 z-80 grid w-[min(420px,calc(100vw-32px))] grid-cols-[18px_minmax(0,1fr)_24px] items-center gap-2 rounded-md border-2 p-3 text-xs shadow-[4px_4px_0_#000] [&_svg]:size-4",
  variants: { error: { true: "border-[#e92929] bg-[#fff3f3] text-[#781818]", false: "border-[#229948] bg-[#eefaf1] text-[#145c2b]" } },
});

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

  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(clearNotice, 2000);
    return () => window.clearTimeout(timer);
  }, [notice, clearNotice]);

  function selectView(path: string) {
    startTransition(() => {
      if (location.pathname !== path) navigate(path);
    });
  }

  return (
    <>
      <a className="fixed top-2 left-2 z-200 -translate-y-[160%] rounded border-2 border-black bg-primary px-3 py-2 text-xs font-bold text-black no-underline shadow-[2px_2px_0_#000] focus:translate-y-0" href="#main-content">
        跳到主要内容
      </a>
      <div className="grid min-h-screen grid-cols-[96px_minmax(0,1fr)] bg-[#fffdf4]">
        <aside className="sticky top-0 z-40 flex h-screen flex-col items-center border-r-2 border-black bg-[#fffdf4] px-2.5 py-4">
          <div className="grid w-full justify-items-center gap-1.5" aria-label="Chu Agent Manager">
            <span className="grid size-10 place-items-center rounded-lg border-2 border-black bg-primary shadow-[2px_2px_0_#000] [&_svg]:size-5">
              <Command />
            </span>
            <strong className="text-[11px] leading-none">CHU</strong>
          </div>
          <nav className="grid w-full flex-1 place-items-center" aria-label="主导航">
            <Dock orientation="vertical" direction="middle" className="w-16 rounded-lg">
              {navigation.map((item) => {
                const Icon = item.icon;
                const count =
                  item.id === "skills"
                    ? snapshot.skills.length
                    : item.id === "mcps"
                      ? snapshot.mcps.length
                      : item.id === "agents"
                        ? snapshot.agents.length
                        : item.id === "prompts"
                          ? snapshot.prompts.length
                          : undefined;
                return (
                  <DockIcon key={item.id}>
                    <button
                      type="button"
                      className="group relative grid size-full cursor-pointer place-items-center rounded-md border-2 border-transparent bg-transparent p-0 transition hover:border-black hover:bg-[#f7f5ec] aria-[current=page]:border-black aria-[current=page]:bg-primary [&_svg]:size-[19px]"
                      aria-current={view === item.id ? "page" : undefined}
                      title={item.label}
                      onClick={() => selectView(item.path)}
                    >
                      <Icon />
                      {count !== undefined ? (
                        <span className="absolute -top-0.5 -right-0.5 h-[17px] min-w-[17px] rounded-full border border-black bg-white px-1 text-center text-[9px] leading-[15px] font-extrabold tabular-nums" aria-hidden="true">
                          {count}
                        </span>
                      ) : null}
                      <span className="pointer-events-none absolute top-1/2 left-[calc(100%+13px)] z-5 w-max -translate-x-1 -translate-y-1/2 rounded bg-black px-2 py-1.5 text-[11px] leading-none font-bold text-white opacity-0 transition group-hover:translate-x-0 group-hover:opacity-100 group-focus-visible:translate-x-0 group-focus-visible:opacity-100">{item.label}</span>
                    </button>
                  </DockIcon>
                );
              })}
            </Dock>
          </nav>
          <div className="grid justify-items-center gap-1 text-[9px] font-extrabold text-[#5c3613] [&_svg]:size-4" title={`中央仓库 ${snapshot.root}`}>
            <Box />
            <span>LOCAL</span>
          </div>
        </aside>

        <main className="min-w-0" id="main-content">
          <header className="sticky top-0 z-30 flex h-16 items-center justify-between border-b-2 border-black bg-white/94 px-7">
            <div className="flex items-center gap-1.5 text-xs text-[#5c3613] [&_svg]:size-3">
              <span>Chu</span>
              <ChevronRight />
              <strong className="text-black">{navigation.find((item) => item.id === view)?.label}</strong>
            </div>
            <div className="flex items-center gap-2.5">
              <span className="flex items-center gap-1.5 text-[11px] font-semibold text-[#5c3613]">
                <StatusDot ready={installedHosts > 0} />
                {installedHosts} 个宿主在线
              </span>
              {demo ? <span className="inline-flex min-h-[22px] items-center rounded-full border border-black bg-primary px-2 text-[10px] font-bold">预览模式</span> : null}
              <button
                type="button"
                className="inline-grid size-10 cursor-pointer place-items-center rounded-md border-2 border-black bg-white shadow-[2px_2px_0_#000] transition hover:-translate-x-px hover:-translate-y-px hover:bg-[#ffe62d] hover:shadow-[4px_4px_0_#000] disabled:cursor-wait disabled:border-neutral-300 disabled:bg-neutral-200 disabled:shadow-none [&_svg]:size-4"
                aria-label="刷新扫描"
                title="刷新扫描"
                disabled={busy === "refresh"}
                onClick={() => void refresh()}
              >
                <RefreshCw className={busy === "refresh" ? "animate-spin" : ""} />
              </button>
            </div>
          </header>

          {notice ? (
            <div className={noticeStyle({ error: Boolean(notice.error) })} role="status">
              {notice.error ? <CircleAlert /> : <Check />}
              <span>{notice.message}</span>
              <button type="button" className="grid size-6 cursor-pointer place-items-center rounded border-0 bg-transparent p-0" aria-label="关闭提示" onClick={clearNotice}>
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
