import { useEffect, useRef, useState } from "react";
import { Bot, Download, KeyRound, Network, Plus, RefreshCw, Sparkles, Trash2, Zap } from "lucide-react";
import { Switch } from "@/components/host-controls";
import type { Host, MCP, Skill, SkillUpdate } from "@/lib/api";
import { resourceMeta, type Resource, type ResourceKind } from "@/lib/resources";

function DeleteConfirm({
  name,
  anchor,
  busy,
  onCancel,
  onConfirm,
}: {
  name: string;
  anchor: string;
  busy: boolean;
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    ref.current?.showPopover();
  }, []);
  return (
    <div
      ref={ref}
      className="delete-popover"
      popover="auto"
      style={{ positionAnchor: anchor } as React.CSSProperties}
      onToggle={(event) => {
        if (event.newState === "closed") onCancel();
      }}
    >
      <strong>删除 {name}？</strong>
      <p>宿主上的部署和中央副本都会移除。</p>
      <span>
        <button type="button" className="button button-secondary" onClick={onCancel}>
          取消
        </button>
        <button type="button" className="button remove-button" disabled={busy} onClick={onConfirm}>
          删除
        </button>
      </span>
    </div>
  );
}

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
  updates = [],
  onUpdate,
  onRemove,
  onMore,
  onDelete,
}: {
  kind: ResourceKind;
  items: Resource[];
  hosts: Host[];
  busy: string;
  onToggle: (item: Resource, hostID: string, enabled: boolean) => void;
  onImport: (item: Resource) => void;
  onTest: (item: MCP) => void;
  updates?: SkillUpdate[];
  onUpdate?: (id: string) => void;
  onRemove?: (id: string) => void;
  onMore?: (item: Skill) => void;
  onDelete?: (id: string) => void;
}) {
  const [pendingDelete, setPendingDelete] = useState<string>();
  const visibleHosts = hosts.filter((host) => host.installed);
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
          {visibleHosts.map((host) => (
            <span key={host.id}>{host.name}</span>
          ))}
        </div>
        <span />
      </div>
      {items.map((item) => {
        const update = updates.find((entry) => entry.id === item.id);
        return (
        <div
          key={item.id}
          className="resource-row"
        >
          {update?.status === "update" ? <span className="update-ribbon">new</span> : null}
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
              {visibleHosts.map((host) => {
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
          {kind === "skills" && item.managed ? (
            <span className="row-actions">
              {(item as Skill).repository ? (
                <button
                  type="button"
                  className="row-action"
                  aria-label={`安装 ${item.name} 同仓库的其他 Skill`}
                  title="安装同仓库的其他 Skill"
                  onClick={() => onMore?.(item as Skill)}
                >
                  <Plus />
                </button>
              ) : null}
              <button
                type="button"
                className="row-action row-action-danger"
                style={{ anchorName: `--delete-${item.id}` } as React.CSSProperties}
                aria-label={`删除 ${item.name}`}
                title="删除 Skill"
                disabled={busy === `delete:${item.id}`}
                onClick={() => setPendingDelete(item.id)}
              >
                <Trash2 />
              </button>
              {pendingDelete === item.id ? (
                <DeleteConfirm
                  name={item.name}
                  anchor={`--delete-${item.id}`}
                  busy={busy === `delete:${item.id}`}
                  onCancel={() => setPendingDelete(undefined)}
                  onConfirm={() => onDelete?.(item.id)}
                />
              ) : null}
            </span>
          ) : null}
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
          {update?.status === "update" ? (
            <button type="button" className="import-button update-button" onClick={() => onUpdate?.(item.id)}>
              <RefreshCw />
              更新
            </button>
          ) : null}
          {update?.status === "deleted" ? (
            <button type="button" className="import-button remove-button" onClick={() => onRemove?.(item.id)}>
              <Trash2 />
              确认移除
            </button>
          ) : null}
        </div>
        );
      })}
    </div>
  );
}
