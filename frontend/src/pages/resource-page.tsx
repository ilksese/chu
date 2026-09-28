import { useDeferredValue, useRef, useState } from "react";
import { Plus, RefreshCw, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { AddResourceDialog } from "@/components/add-resource-dialog";
import { ResourceList } from "@/components/resource-browser";
import type { MCP, Skill } from "@/lib/api";
import { resourcesFor, type ResourceKind } from "@/lib/resources";
import { useAppStore } from "@/stores/app-store";

const pageCopy: Record<ResourceKind, { action: string }> = {
  skills: { action: "安装 Skill" },
  mcps: { action: "添加 MCP" },
  agents: { action: "创建 Agent" },
};

export function ResourcePage({ kind }: { kind: ResourceKind }) {
  const snapshot = useAppStore((state) => state.snapshot);
  const busy = useAppStore((state) => state.busy);
  const toggleResource = useAppStore((state) => state.toggleResource);
  const importSkill = useAppStore((state) => state.importSkill);
  const testMCP = useAppStore((state) => state.testMCP);
  const updates = useAppStore((state) => state.skillUpdates);
  const checkSkillUpdates = useAppStore((state) => state.checkSkillUpdates);
  const updateSkill = useAppStore((state) => state.updateSkill);
  const deleteSkill = useAppStore((state) => state.deleteSkill);
  const removeSkill = useAppStore((state) => state.removeSkill);
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const [repository, setRepository] = useState("");
  const dialogTrigger = useRef<HTMLButtonElement>(null);
  const deferredSearch = useDeferredValue(search);
  const resources = resourcesFor(snapshot, kind);
  const query = deferredSearch.trim().toLowerCase();
  const filtered = (query
    ? resources.filter((item) => `${item.name} ${item.description}`.toLowerCase().includes(query))
    : resources
  ).toSorted((left, right) => {
    if (kind !== "skills") return left.name.localeCompare(right.name);
    const rank = (item: (typeof resources)[number]) => {
      const status = updates.find((update) => update.id === item.id)?.status;
      if (status === "update") return 0;
      return item.managed ? 1 : 2;
    };
    return rank(left) - rank(right) || left.name.localeCompare(right.name);
  });
  const copy = pageCopy[kind];

  return (
    <>
      <section className="mb-6 flex items-center justify-end gap-6">
        {kind === "skills" ? (
          <Button
            type="button"
            variant="secondary"
            disabled={busy === "updates:skills"}
            onClick={() => void checkSkillUpdates()}
          >
            <RefreshCw className={busy === "updates:skills" ? "animate-spin" : ""} />
            检查更新
          </Button>
        ) : null}
        <Button
          ref={dialogTrigger}
          type="button"
          variant="primary"
          onClick={() => {
            setRepository("");
            setDialogOpen(true);
          }}
        >
          <Plus />
          {copy.action}
        </Button>
      </section>
      <div className="flex min-h-14 items-center justify-between rounded-t-lg border-2 border-b-0 border-border bg-card px-3 py-2">
        <label className="flex h-[38px] w-[min(380px,68%)] items-center gap-2 rounded-md border-2 border-input bg-muted px-3 text-muted-foreground focus-within:border-ring focus-within:shadow-focus [&_svg]:size-4">
          <Search />
          <Input size="bare"
            aria-label="搜索资源"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="搜索名称或描述"
          />
        </label>
        <span className="pr-1.5 text-[11px] text-muted-foreground tabular-nums">{filtered.length} 个条目</span>
      </div>
      <section>
        <div className="min-w-0 overflow-x-auto rounded-b-lg border-2 border-border bg-card">
          <ResourceList
            kind={kind}
            items={filtered}
            hosts={snapshot.hosts}
            busy={busy}
            onToggle={(item, hostID, enabled) =>
              void toggleResource(kind, item.id, hostID, enabled)
            }
            onImport={(item) => void importSkill(item)}
            onTest={(item: MCP) => void testMCP(item.id)}
            updates={kind === "skills" ? updates : []}
            onUpdate={(id) => void updateSkill(id)}
            onRemove={(id) => void removeSkill(id)}
            onDelete={(id) => {
              if (!snapshot.skills.find((skill) => skill.id === id)?.managed) return;
              void deleteSkill(id);
            }}
            onMore={(item: Skill) => {
              setRepository(item.repository);
              setDialogOpen(true);
            }}
          />
        </div>
      </section>
      {dialogOpen ? (
        <AddResourceDialog
          kind={kind}
          initialRepository={repository}
          onClose={() => setDialogOpen(false)}
          returnFocus={dialogTrigger.current}
        />
      ) : null}
    </>
  );
}
