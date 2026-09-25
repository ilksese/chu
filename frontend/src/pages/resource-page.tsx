import { useDeferredValue, useRef, useState } from "react";
import { Plus, Search } from "lucide-react";
import { AddResourceDialog } from "@/components/add-resource-dialog";
import { ResourceList } from "@/components/resource-browser";
import type { MCP } from "@/lib/api";
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
  const [search, setSearch] = useState("");
  const [dialogOpen, setDialogOpen] = useState(false);
  const dialogTrigger = useRef<HTMLButtonElement>(null);
  const deferredSearch = useDeferredValue(search);
  const resources = resourcesFor(snapshot, kind);
  const query = deferredSearch.trim().toLowerCase();
  const filtered = query
    ? resources.filter((item) => `${item.name} ${item.description}`.toLowerCase().includes(query))
    : resources;
  const copy = pageCopy[kind];

  return (
    <>
      <section className="page-heading">
        <button
          ref={dialogTrigger}
          type="button"
          className="button button-primary"
          onClick={() => setDialogOpen(true)}
        >
          <Plus />
          {copy.action}
        </button>
      </section>
      <div className="library-toolbar">
        <label className="search-field">
          <Search />
          <input
            aria-label="搜索资源"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            placeholder="搜索名称或描述"
          />
        </label>
        <span>{filtered.length} 个条目</span>
      </div>
      <section className="library-layout">
        <div className="resource-panel resource-panel-full">
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
          />
        </div>
      </section>
      {dialogOpen ? (
        <AddResourceDialog
          kind={kind}
          onClose={() => setDialogOpen(false)}
          returnFocus={dialogTrigger.current}
        />
      ) : null}
    </>
  );
}
