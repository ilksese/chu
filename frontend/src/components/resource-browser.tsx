import { useRef, useState } from "react";
import { tv } from "tailwind-variants";
import { Bot, Download, KeyRound, Network, Plus, RefreshCw, Sparkles, Trash2, Zap } from "lucide-react";
import { Switch } from "@/components/host-controls";
import type { Host, MCP, Skill, SkillUpdate } from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Popover } from "@/components/ui/popover";
import { resourceMeta, type Resource, type ResourceKind } from "@/lib/resources";

const resourceIcon = tv({
  base: "grid size-9 shrink-0 place-items-center rounded-md border border-border [&_svg]:size-4",
  variants: { kind: { skills: "bg-primary", mcps: "bg-info-surface text-info", agents: "bg-muted text-muted-foreground" } },
});
const rowAction = tv({
  base: "grid size-[30px] cursor-pointer place-items-center rounded border-0 bg-transparent p-0 text-muted-foreground hover:bg-primary hover:text-foreground disabled:cursor-not-allowed disabled:opacity-45 [&_svg]:size-4",
  variants: { danger: { true: "hover:bg-destructive hover:text-destructive-foreground" } },
});

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
  const deleteAnchors = useRef(new Map<string, HTMLButtonElement>());
  const visibleHosts = hosts.filter((host) => host.installed);
  if (items.length === 0) {
    return (
      <div className="grid min-h-[280px] place-items-center p-8 text-center text-muted-foreground">
        <span className="grid size-[42px] place-items-center rounded-md border border-border bg-primary [&_svg]:size-5">
          <ResourceIcon kind={kind} />
        </span>
        <strong className="mt-2.5 text-sm text-foreground">还没有资源</strong>
        <p className="mt-1 text-[11px]">使用右上角的新建按钮添加第一个条目。</p>
      </div>
    );
  }

  return (
    <div className="min-w-[660px]">
      <div className="grid min-h-[38px] grid-cols-[40px_minmax(180px,1fr)_max-content_auto] items-center gap-3 border-b border-border bg-muted px-3.5 text-[9px] font-bold text-muted-foreground uppercase">
        <span className="col-span-2">资源</span>
        <div className="grid grid-flow-col justify-end gap-2 text-center auto-cols-[minmax(72px,max-content)]" aria-label="Agent 宿主">
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
          className="relative grid min-h-[84px] grid-cols-[40px_minmax(180px,1fr)_max-content_auto] items-center gap-3 overflow-hidden border-b border-muted-foreground bg-card px-3.5 py-3 text-left last:border-b-0 hover:bg-primary-muted"
        >
          {update?.status === "update" ? <span className="pointer-events-none absolute top-3.5 -left-7 z-4 h-4 w-21 -rotate-45 border-y border-border bg-primary text-center text-[9px] leading-[13px] font-extrabold tracking-widest uppercase shadow-neo-xs">new</span> : null}
          <span className={resourceIcon({ kind, className: "relative z-2" })}>
            <ResourceIcon kind={kind} />
          </span>
          <span className="relative z-2 grid min-w-0 gap-1">
            <span className="flex min-w-0 items-center gap-1.5">
              <strong className="truncate text-[13px]">{item.name}</strong>
              {!item.managed ? (
                <>
                  <Badge tone="warning" size="sm">待导入</Badge>
                  <Badge tone="info" size="sm">{hosts.find((host) => item.enabledOn[host.id])?.name || "未知宿主"}</Badge>
                </>
              ) : null}
              {kind === "skills" && item.managed && Object.values((item as Skill).modeByHost).includes("copy") ? (
                <Badge tone="neutral" size="sm">copy</Badge>
              ) : null}
              {kind === "mcps" && (item as MCP).hasCredentials ? (
                <KeyRound className="size-3 text-warning" aria-label="包含敏感值" />
              ) : null}
            </span>
            <span className="truncate text-[11px] text-muted-foreground">{item.description || "未填写描述"}</span>
            <span className="truncate text-[9px] text-neutral-400">{resourceMeta(item, kind)}</span>
          </span>
          {!item.managed && kind === "skills" ? (
            <Button type="button" variant="primary" size="sm" className="relative z-3 justify-self-end" onClick={() => onImport(item)}>
              <Download />
              导入
            </Button>
          ) : (
            <span className="relative z-3 grid grid-flow-col items-center justify-end gap-2 auto-cols-[minmax(72px,max-content)]">
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
            <span className="relative z-3 flex justify-end gap-0.5">
              {(item as Skill).repository ? (
                <button
                  type="button"
                  className={rowAction()}
                  aria-label={`安装 ${item.name} 同仓库的其他 Skill`}
                  title="安装同仓库的其他 Skill"
                  onClick={() => onMore?.(item as Skill)}
                >
                  <Plus />
                </button>
              ) : null}
              <button
                ref={(node) => {
                  if (node) deleteAnchors.current.set(item.id, node);
                  else deleteAnchors.current.delete(item.id);
                }}
                type="button"
                className={rowAction({ danger: true })}
                aria-label={`删除 ${item.name}`}
                title="删除 Skill"
                disabled={busy === `delete:${item.id}`}
                onClick={() => setPendingDelete(item.id)}
              >
                <Trash2 />
              </button>
              <Popover open={pendingDelete === item.id} anchor={{ current: pendingDelete ? deleteAnchors.current.get(pendingDelete) ?? null : null }} onClose={() => setPendingDelete(undefined)}>
                <strong className="block">删除 {item.name}？</strong>
                <p className="my-1.5 block text-xs text-muted-foreground">宿主上的部署和中央副本都会移除。</p>
                <span className="flex justify-end gap-2">
                  <Button type="button" variant="secondary" size="sm" onClick={() => setPendingDelete(undefined)}>取消</Button>
                  <Button type="button" variant="danger" size="sm" disabled={busy === `delete:${item.id}`} onClick={() => onDelete?.(item.id)}>删除</Button>
                </span>
              </Popover>
            </span>
          ) : null}
          {kind === "mcps" && item.managed ? (
            <button
              type="button"
              className={rowAction()}
              aria-label={`测试 ${item.name}`}
              title="测试配置"
              onClick={() => onTest(item as MCP)}
            >
              <Zap />
            </button>
          ) : null}
          {update?.status === "update" ? (
            <Button type="button" variant="info" size="sm" className="relative z-3 justify-self-end" onClick={() => onUpdate?.(item.id)}>
              <RefreshCw />
              更新
            </Button>
          ) : null}
          {update?.status === "deleted" ? (
            <Button type="button" variant="danger" size="sm" className="relative z-3 justify-self-end" onClick={() => onRemove?.(item.id)}>
              <Trash2 />
              确认移除
            </Button>
          ) : null}
        </div>
        );
      })}
    </div>
  );
}
