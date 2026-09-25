import { Bot, Download, KeyRound, Network, Sparkles, Zap } from "lucide-react";
import { Switch } from "@/components/host-controls";
import type { Host, MCP, Skill } from "@/lib/api";
import { resourceMeta, type Resource, type ResourceKind } from "@/lib/resources";

function ResourceIcon({ kind }: { kind: ResourceKind }) {
  if (kind === "skills") return <Sparkles />;
  if (kind === "mcps") return <Network />;
  return <Bot />;
}

export function ResourceList({
  kind,
  items,
  hosts,
  busy,
  onToggle,
  onImport,
  onTest,
}: {
  kind: ResourceKind;
  items: Resource[];
  hosts: Host[];
  busy: string;
  onToggle: (item: Resource, hostID: string, enabled: boolean) => void;
  onImport: (item: Resource) => void;
  onTest: (item: MCP) => void;
}) {
  if (items.length === 0) {
    return (
      <div className="empty-state">
        <span className="empty-icon">
          <ResourceIcon kind={kind} />
        </span>
        <strong>还没有资源</strong>
        <p>使用右上角的新建按钮添加第一个条目。</p>
      </div>
    );
  }

  return (
    <div className="resource-table">
      <div className="resource-table-head">
        <span>资源</span>
        <div className="host-columns" aria-label="Agent 宿主">
          {hosts.map((host) => (
            <span key={host.id} title={host.name}>
              {host.id === "opencode" ? "OC" : host.id === "claude" ? "CC" : "CX"}
            </span>
          ))}
        </div>
      </div>
      {items.map((item) => (
        <div
          key={item.id}
          className="resource-row"
        >
          <span className={`resource-icon resource-icon-${kind}`}>
            <ResourceIcon kind={kind} />
          </span>
          <span className="resource-copy">
            <span className="resource-title-line">
              <strong>{item.name}</strong>
              {!item.managed ? (
                <>
                  <span className="badge badge-warning">待导入</span>
                  <span className="badge badge-source">
                    {hosts.find((host) => item.enabledOn[host.id])?.name || "未知宿主"}
                  </span>
                </>
              ) : null}
              {kind === "skills" &&
              item.managed &&
              Object.values((item as Skill).modeByHost).includes("copy") ? (
                <span className="badge">copy</span>
              ) : null}
              {kind === "mcps" && (item as MCP).hasCredentials ? (
                <KeyRound className="credential-icon" aria-label="包含敏感值" />
              ) : null}
            </span>
            <span className="resource-description">{item.description || "未填写描述"}</span>
            <span className="resource-meta">{resourceMeta(item, kind)}</span>
          </span>
          {!item.managed && kind === "skills" ? (
            <button type="button" className="import-button" onClick={() => onImport(item)}>
              <Download />
              导入
            </button>
          ) : (
            <span className="host-columns host-switches">
              {hosts.map((host) => {
                const operation = `${kind}:${item.id}:${host.id}`;
                return (
                  <Switch
                    key={host.id}
                    checked={Boolean(item.enabledOn[host.id])}
                    disabled={!host.installed || busy === operation || !item.managed}
                    label={`${item.enabledOn[host.id] ? "关闭" : "部署到"} ${host.name}`}
                    onChange={(enabled) => onToggle(item, host.id, enabled)}
                  />
                );
              })}
            </span>
          )}
          {kind === "mcps" && item.managed ? (
            <button
              type="button"
              className="row-action"
              aria-label={`测试 ${item.name}`}
              title="测试配置"
              onClick={() => onTest(item as MCP)}
            >
              <Zap />
            </button>
          ) : null}
        </div>
      ))}
    </div>
  );
}
