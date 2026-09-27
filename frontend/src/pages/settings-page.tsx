import { useEffect, useState } from "react";
import { Check, KeyRound, RotateCcw } from "lucide-react";
import { HostMark, StatusDot } from "@/components/host-controls";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import type { Host } from "@/lib/api";
import { useAppStore } from "@/stores/app-store";

type HostPaths = Pick<Host, "configPath" | "skillPath" | "agentPath">;

function pathsFromHost(host: Host): HostPaths {
  return { configPath: host.configPath, skillPath: host.skillPath, agentPath: host.agentPath };
}

export function SettingsPage() {
  const snapshot = useAppStore((state) => state.snapshot);
  const busy = useAppStore((state) => state.busy);
  const restoreHost = useAppStore((state) => state.restoreHost);
  const updateHost = useAppStore((state) => state.updateHost);
  const [drafts, setDrafts] = useState<Record<string, HostPaths>>(() =>
    Object.fromEntries(snapshot.hosts.map((host) => [host.id, pathsFromHost(host)])),
  );

  useEffect(() => {
    const summary = snapshot.hosts
      .map((host) => `${host.id}:${typeof host.format}:${host.format ?? "nil"}`)
      .join(",");
    console.info(`[chu] settings hosts=${snapshot.hosts.length} ${summary}`);
    window.runtime?.LogInfo?.(`[chu] settings hosts=${snapshot.hosts.length} ${summary}`);
    setDrafts(Object.fromEntries(snapshot.hosts.map((host) => [host.id, pathsFromHost(host)])));
  }, [snapshot.hosts]);

  return (
    <>
      <section className="grid grid-cols-[minmax(0,1fr)_310px] items-start gap-6">
        <div className="grid gap-3.5">
          {snapshot.hosts.map((host) => {
            const paths = drafts[host.id] ?? pathsFromHost(host);
            return (
              <article key={host.id} className="rounded-lg border-2 border-black bg-white p-[18px]">
                <header className="grid grid-cols-[40px_minmax(0,1fr)_auto] items-center gap-3 border-b border-[#5c3613] pb-4">
                  <HostMark host={host} />
                  <div>
                    <h2 className="m-0 text-base font-extrabold">{host.name}</h2>
                    <span className="mt-1 flex items-center gap-1.5 text-[10px] text-[#5c3613]">
                      <StatusDot ready={host.installed} />
                      {host.installed ? "已检测到安装" : "未检测到安装"}
                    </span>
                  </div>
                  <Badge>{host.format.toUpperCase()}</Badge>
                </header>
                <div className="grid gap-2.5 py-4">
                  {(["configPath", "skillPath", "agentPath"] as const).map((key) => (
                    <label key={key} className="grid grid-cols-[100px_minmax(0,1fr)] items-center gap-3 text-[11px] font-semibold text-[#5c3613]">
                      {key === "configPath"
                        ? "配置文件"
                        : key === "skillPath"
                          ? "Skill 目录"
                          : "Agent 目录"}
                      <Input
                        value={paths[key]}
                        onChange={(event) =>
                          setDrafts((current) => ({
                            ...current,
                            [host.id]: { ...paths, [key]: event.target.value },
                          }))
                        }
                      />
                    </label>
                  ))}
                </div>
                <footer className="flex justify-end gap-2.5 pt-4">
                  <Button
                    type="button"
                    variant="secondary"
                    disabled={busy === `restore:${host.id}`}
                    onClick={() => void restoreHost(host)}
                  >
                    <RotateCcw />
                    恢复上次备份
                  </Button>
                  <Button
                    type="button"
                    variant="primary"
                    disabled={busy === `paths:${host.id}`}
                    onClick={() => void updateHost(host, paths)}
                  >
                    <Check />
                    保存路径
                  </Button>
                </footer>
              </article>
            );
          })}
        </div>
        <aside className="sticky top-22 rounded-lg border-2 border-black bg-white p-[18px] shadow-[2px_2px_0_#000]">
          <h2 className="m-0 text-[19px] font-extrabold">Chu 中央目录</h2>
          <code className="mt-4 block truncate rounded border border-black bg-[#f7f5ec] p-2.5 text-[10px]">{snapshot.root}</code>
          <p className="text-xs leading-relaxed text-[#5c3613]">Skill 本体、MCP 定义、自定义 Agent 和部署状态统一存放在这里。</p>
          <div className="mt-4 flex gap-2.5 rounded-md border border-black bg-[#fde8c8] p-3 text-[#5c3613] [&_svg]:size-4">
            <KeyRound />
            <span>
              <strong>本地凭据</strong>当前版本按明文写入配置文件，界面默认隐藏。
            </span>
          </div>
        </aside>
      </section>
    </>
  );
}
