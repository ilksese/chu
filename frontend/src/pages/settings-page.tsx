import { useEffect, useState } from "react";
import { Check, KeyRound, RotateCcw } from "lucide-react";
import { HostMark, StatusDot } from "@/components/host-controls";
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
    setDrafts(Object.fromEntries(snapshot.hosts.map((host) => [host.id, pathsFromHost(host)])));
  }, [snapshot.hosts]);

  return (
    <>
      <section className="page-heading">
        <div>
          <h1>宿主与路径</h1>
          <p>自动发现结果可作为后续手动路径配置的基础。</p>
        </div>
      </section>
      <section className="settings-layout">
        <div className="settings-list">
          {snapshot.hosts.map((host) => {
            const paths = drafts[host.id] ?? pathsFromHost(host);
            return (
              <article key={host.id} className="settings-host">
                <header>
                  <HostMark host={host} />
                  <div>
                    <h2>{host.name}</h2>
                    <span>
                      <StatusDot ready={host.installed} />
                      {host.installed ? "已检测到安装" : "未检测到安装"}
                    </span>
                  </div>
                  <span className="format-badge">{host.format.toUpperCase()}</span>
                </header>
                <div className="path-fields">
                  {(["configPath", "skillPath", "agentPath"] as const).map((key) => (
                    <label key={key}>
                      {key === "configPath"
                        ? "配置文件"
                        : key === "skillPath"
                          ? "Skill 目录"
                          : "Agent 目录"}
                      <input
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
                <footer>
                  <button
                    type="button"
                    className="button button-secondary"
                    disabled={busy === `restore:${host.id}`}
                    onClick={() => void restoreHost(host)}
                  >
                    <RotateCcw />
                    恢复上次备份
                  </button>
                  <button
                    type="button"
                    className="button button-primary"
                    disabled={busy === `paths:${host.id}`}
                    onClick={() => void updateHost(host, paths)}
                  >
                    <Check />
                    保存路径
                  </button>
                </footer>
              </article>
            );
          })}
        </div>
        <aside className="settings-aside">
          <h2>Chu 中央目录</h2>
          <code>{snapshot.root}</code>
          <p>Skill 本体、MCP 定义、自定义 Agent 和部署状态统一存放在这里。</p>
          <div className="security-note">
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
