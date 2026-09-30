import { useRef, useState } from "react";
import {
  CircleAlert,
  Database,
  LoaderCircle,
  Pencil,
  Plus,
  RefreshCw,
  Server,
  Trash2,
} from "lucide-react";
import { ProviderDialog } from "@/components/ProviderDialog";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import type { Provider, ProviderInput } from "@/lib/api";
import { useAppStore } from "@/stores/appStore";

export function ProvidersPage() {
  const providers = useAppStore((state) => state.snapshot.providers);
  const busy = useAppStore((state) => state.busy);
  const createProvider = useAppStore((state) => state.createProvider);
  const updateProvider = useAppStore((state) => state.updateProvider);
  const deleteProvider = useAppStore((state) => state.deleteProvider);
  const refreshProviderModels = useAppStore((state) => state.refreshProviderModels);
  const [selectedID, setSelectedID] = useState<string>();
  const [editing, setEditing] = useState<Provider | null>();
  const [pendingDelete, setPendingDelete] = useState<Provider>();
  const [returnFocus, setReturnFocus] = useState<HTMLElement | null>(null);
  const [deleteError, setDeleteError] = useState("");
  const [deleting, setDeleting] = useState(false);
  const operationRef = useRef(false);
  const addButtonRef = useRef<HTMLButtonElement>(null);
  const providerListRef = useRef<HTMLElement>(null);
  const selected = providers.find((item) => item.id === selectedID) ?? providers[0];
  const refreshing = selected ? busy === `refresh:provider:${selected.id}` : false;
  const locked = Boolean(busy) || deleting;

  async function save(input: ProviderInput) {
    if (useAppStore.getState().busy) return false;
    if (editing) {
      const saved = await updateProvider(editing.id, input);
      if (saved) setSelectedID(editing.id);
      return saved;
    }
    const previousIDs = new Set(useAppStore.getState().snapshot.providers.map((item) => item.id));
    const saved = await createProvider(input);
    if (saved) {
      const added = useAppStore
        .getState()
        .snapshot.providers.find((item) => !previousIDs.has(item.id));
      if (added) setSelectedID(added.id);
    }
    return saved;
  }

  async function remove() {
    if (!pendingDelete || useAppStore.getState().busy || operationRef.current) return;
    const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    operationRef.current = true;
    setDeleting(true);
    setDeleteError("");
    let removed = false;
    try {
      removed = await deleteProvider(pendingDelete.id);
      if (removed) {
        const remaining = useAppStore.getState().snapshot.providers;
        const index = providers.findIndex((item) => item.id === pendingDelete.id);
        setSelectedID(remaining[Math.min(index, remaining.length - 1)]?.id);
        setPendingDelete(undefined);
      } else setDeleteError("删除失败，请重试");
    } catch {
      setDeleteError("删除失败，请重试");
    } finally {
      operationRef.current = false;
      setDeleting(false);
      requestAnimationFrame(() => {
        if (removed) {
          const next = providerListRef.current?.querySelector<HTMLButtonElement>(
            'button[aria-current="true"]',
          );
          (next ?? addButtonRef.current)?.focus();
        } else if (focused?.isConnected) focused.focus();
      });
    }
  }

  return (
    <>
      <section className="mb-4 flex items-center justify-between gap-4" aria-label="Providers">
        <span className="text-xs text-muted-foreground tabular-nums">
          {providers.length} 个供应商
        </span>
        <Button
          ref={addButtonRef}
          type="button"
          variant="primary"
          className="rounded-md"
          disabled={locked}
          onClick={(event) => {
            setReturnFocus(event.currentTarget);
            setEditing(null);
          }}
        >
          <Plus />
          新增供应商
        </Button>
      </section>
      <div className="grid h-[calc(100vh-212px)] min-h-[420px] min-w-0 grid-cols-[260px_minmax(0,1fr)] border-y-2 border-border bg-card">
        <nav
          ref={providerListRef}
          className="min-h-0 overflow-y-auto border-r-2 border-border py-2"
          aria-label="供应商列表"
        >
          {providers.length ? (
            providers.map((provider) => (
              <button
                key={provider.id}
                type="button"
                className="grid w-full min-w-0 cursor-pointer gap-1 border-0 border-b border-border/15 bg-transparent px-4 py-3 text-left hover:bg-muted aria-[current=true]:bg-muted aria-[current=true]:shadow-[inset_3px_0_0_var(--color-primary)]"
                aria-current={selected?.id === provider.id ? "true" : undefined}
                onClick={() => setSelectedID(provider.id)}
              >
                <span className="flex min-w-0 items-start justify-between gap-2">
                  <strong className="min-w-0 text-sm [overflow-wrap:anywhere]">
                    {provider.name}
                  </strong>
                  <Badge tone="neutral" className="shrink-0 rounded-sm tabular-nums">
                    {provider.models.length}
                  </Badge>
                </span>
                <span className="min-w-0 text-[11px] leading-4 text-muted-foreground [overflow-wrap:anywhere]">
                  {provider.baseUrl}
                </span>
                {provider.modelsError ? (
                  <span className="flex items-center gap-1 text-[11px] text-error-foreground">
                    <CircleAlert className="size-3" />
                    获取失败
                  </span>
                ) : null}
              </button>
            ))
          ) : (
            <p className="px-4 text-xs text-muted-foreground">暂无供应商</p>
          )}
        </nav>
        {selected ? (
          <section
            className="flex min-h-0 min-w-0 flex-col"
            aria-labelledby="provider-detail-title"
          >
            <header className="flex max-h-[45%] shrink-0 items-start justify-between gap-4 overflow-y-auto border-b border-border/20 p-5">
              <div className="min-w-0 flex-1">
                <h2
                  id="provider-detail-title"
                  className="m-0 text-lg font-bold [overflow-wrap:anywhere]"
                >
                  {selected.name}
                </h2>
                <dl className="mt-3 mb-0 grid grid-cols-[94px_minmax(0,1fr)] gap-x-3 gap-y-2 text-xs leading-5">
                  <dt className="text-muted-foreground">BASE_URL</dt>
                  <dd className="m-0 [overflow-wrap:anywhere]">{selected.baseUrl}</dd>
                  <dt className="text-muted-foreground">ENV_API_KEY</dt>
                  <dd className="m-0 font-mono [overflow-wrap:anywhere]">
                    {selected.envApiKey || "无需鉴权"}
                  </dd>
                </dl>
              </div>
              <div className="flex shrink-0 gap-2">
                <Button
                  type="button"
                  size="icon"
                  disabled={locked}
                  title="刷新模型"
                  aria-label="刷新模型"
                  onClick={() => {
                    if (!useAppStore.getState().busy) void refreshProviderModels(selected.id);
                  }}
                >
                  <RefreshCw className={refreshing ? "animate-spin" : ""} />
                </Button>
                <Button
                  type="button"
                  size="icon"
                  disabled={locked}
                  title="编辑供应商"
                  aria-label="编辑供应商"
                  onClick={(event) => {
                    setReturnFocus(event.currentTarget);
                    setEditing(selected);
                  }}
                >
                  <Pencil />
                </Button>
                <Button
                  type="button"
                  size="icon"
                  disabled={locked}
                  title="删除供应商"
                  aria-label="删除供应商"
                  onClick={(event) => {
                    setReturnFocus(event.currentTarget);
                    setDeleteError("");
                    setPendingDelete(selected);
                  }}
                >
                  <Trash2 />
                </Button>
              </div>
            </header>
            <div
              className="min-h-0 flex-1 overflow-y-auto p-5"
              aria-busy={refreshing}
              tabIndex={0}
              aria-label="模型列表"
            >
              <div className="mb-3 flex flex-wrap items-center justify-between gap-2">
                <h3 className="m-0 flex items-center gap-2 text-sm font-bold">
                  <Database className="size-4" />
                  模型{" "}
                  <span className="font-normal text-muted-foreground tabular-nums">
                    {selected.models.length}
                  </span>
                </h3>
                {selected.modelsFetchedAt ? (
                  <span className="text-[11px] text-muted-foreground [overflow-wrap:anywhere]">
                    上次获取：
                    {Number.isNaN(Date.parse(selected.modelsFetchedAt))
                      ? selected.modelsFetchedAt
                      : new Date(selected.modelsFetchedAt).toLocaleString()}
                  </span>
                ) : null}
              </div>
              {refreshing ? (
                <p role="status" className="flex items-center gap-2 text-xs text-muted-foreground">
                  <LoaderCircle className="size-4 animate-spin" />
                  正在获取模型{selected.models.length ? "，显示缓存" : ""}
                </p>
              ) : null}
              {selected.modelsError ? (
                <div
                  role="status"
                  className="mb-3 rounded-sm border border-destructive bg-destructive-surface p-3 text-xs leading-5 text-error-foreground [overflow-wrap:anywhere]"
                >
                  <strong className="flex items-center gap-2">
                    <CircleAlert className="size-4 shrink-0" />
                    模型获取失败{selected.models.length ? "，已保留缓存" : ""}
                  </strong>
                  <p className="mt-1 mb-0 whitespace-pre-wrap">{selected.modelsError}</p>
                </div>
              ) : null}
              {selected.models.length ? (
                <ul className="m-0 list-none p-0">
                  {selected.models.map((model, index) => (
                    <li
                      key={`${model}:${index}`}
                      className="border-b border-border/15 py-2.5 font-mono text-xs leading-5 [content-visibility:auto] [contain-intrinsic-size:auto_40px] [overflow-wrap:anywhere]"
                    >
                      {model}
                    </li>
                  ))}
                </ul>
              ) : !refreshing ? (
                <p className="py-8 text-center text-xs text-muted-foreground">
                  {selected.modelsError
                    ? "暂无可用缓存"
                    : selected.modelsFetchedAt
                      ? "未返回模型"
                      : "尚未获取模型"}
                </p>
              ) : null}
            </div>
          </section>
        ) : (
          <section className="grid place-content-center justify-items-center gap-3 p-8 text-muted-foreground">
            <Server className="size-8" />
            <p className="m-0 text-sm">暂无供应商</p>
          </section>
        )}
      </div>
      {editing !== undefined ? (
        <ProviderDialog
          provider={editing}
          providers={providers}
          busy={Boolean(busy)}
          returnFocus={returnFocus}
          onClose={() => setEditing(undefined)}
          onSave={save}
        />
      ) : null}
      {pendingDelete ? (
        <Dialog
          className="w-[420px] rounded-md"
          aria-labelledby="delete-provider-title"
          aria-describedby="delete-provider-description"
          returnFocus={returnFocus}
          onClose={() => {
            if (!locked && !operationRef.current) setPendingDelete(undefined);
          }}
        >
          <div className="grid gap-4 p-5" aria-busy={deleting}>
            <h2 id="delete-provider-title" className="m-0 text-lg font-bold">
              删除供应商？
            </h2>
            <p
              id="delete-provider-description"
              className="m-0 text-sm leading-6 [overflow-wrap:anywhere]"
            >
              确认删除「{pendingDelete.name}」及其模型缓存？
            </p>
            {deleteError ? (
              <p className="m-0 text-xs text-error-foreground" role="alert">
                {deleteError}
              </p>
            ) : null}
            <div className="flex justify-end gap-2">
              <Button
                type="button"
                className="rounded-md"
                disabled={locked}
                onClick={() => setPendingDelete(undefined)}
              >
                取消
              </Button>
              <Button
                type="button"
                variant="danger"
                className="min-w-24 rounded-md"
                disabled={locked}
                onClick={() => void remove()}
              >
                {deleting ? <LoaderCircle className="animate-spin" /> : <Trash2 />}
                {deleting ? "删除中" : "删除"}
              </Button>
            </div>
          </div>
        </Dialog>
      ) : null}
    </>
  );
}
