import { useDeferredValue, useEffect, useRef, useState, type FormEvent } from "react";
import { Plus, ScrollText, Search, Trash2 } from "lucide-react";
import { Switch } from "@/components/host-controls";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Popover } from "@/components/ui/popover";
import { readPrompt, type Prompt } from "@/lib/api";
import { useAppStore } from "@/stores/app-store";

export function PromptsPage() {
  const snapshot = useAppStore((state) => state.snapshot);
  const busy = useAppStore((state) => state.busy);
  const createPrompt = useAppStore((state) => state.createPrompt);
  const deletePrompt = useAppStore((state) => state.deletePrompt);
  const togglePrompt = useAppStore((state) => state.togglePrompt);
  const [search, setSearch] = useState("");
  const [editing, setEditing] = useState<Prompt | null>();
  const [pendingDelete, setPendingDelete] = useState<string>();
  const dialogTrigger = useRef<HTMLButtonElement>(null);
  const deleteAnchors = useRef(new Map<string, HTMLButtonElement>());
  const deferredSearch = useDeferredValue(search);
  const query = deferredSearch.trim().toLowerCase();
  const prompts = (snapshot.prompts ?? [])
    .filter((item) => !query || `${item.name} ${item.preview}`.toLowerCase().includes(query))
    .toSorted((left, right) => left.name.localeCompare(right.name));
  const hosts = snapshot.hosts.filter((host) => host.installed);

  return (
    <>
      <section className="mb-6 flex items-center justify-end">
        <Button ref={dialogTrigger} type="button" variant="primary" onClick={() => setEditing(null)}>
          <Plus />
          新建提示词
        </Button>
      </section>
      <div className="flex min-h-14 items-center justify-between rounded-t-lg border-2 border-b-0 border-black bg-white px-3 py-2">
        <label className="flex h-[38px] w-[min(380px,68%)] items-center gap-2 rounded-md border-2 border-black bg-[#f7f5ec] px-3 text-[#5c3613] focus-within:border-[#e92929] focus-within:shadow-[0_0_0_3px_rgb(233_41_41/15%)] [&_svg]:size-4">
          <Search />
          <Input size="bare" aria-label="搜索提示词" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索名称或内容" />
        </label>
        <span className="pr-1.5 text-[11px] text-[#5c3613] tabular-nums">{prompts.length} 个条目</span>
      </div>
      <section className="min-w-0 overflow-x-auto rounded-b-lg border-2 border-black bg-white">
        {prompts.length === 0 ? (
          <div className="grid min-h-[280px] place-items-center p-8 text-center text-[#5c3613]">
            <span className="grid size-[42px] place-items-center rounded-md border border-black bg-primary [&_svg]:size-5"><ScrollText /></span>
            <strong className="mt-2.5 text-sm text-black">还没有提示词</strong>
            <p className="mt-1 text-[11px]">新建后保存在 Chu，再分发到各个宿主。</p>
          </div>
        ) : (
          <div className="min-w-[660px]">
            <div className="grid min-h-[38px] grid-cols-[40px_minmax(180px,1fr)_max-content_auto] items-center gap-3 border-b border-black bg-[#f7f5ec] px-3.5 text-[9px] font-bold text-[#5c3613] uppercase">
              <span className="col-span-2">提示词</span>
              <div className="grid grid-flow-col justify-end gap-2 text-center auto-cols-[minmax(72px,max-content)]">
                {hosts.map((host) => <span key={host.id}>{host.name}</span>)}
              </div>
              <span />
            </div>
            {prompts.map((item) => (
              <div key={item.id} className="grid min-h-[84px] grid-cols-[40px_minmax(180px,1fr)_max-content_auto] items-center gap-3 border-b border-[#5c3613] bg-white px-3.5 py-3 last:border-b-0 hover:bg-[#fffbe8]">
                <span className="grid size-9 place-items-center rounded-md border border-black bg-primary [&_svg]:size-4"><ScrollText /></span>
                <button type="button" className="grid min-w-0 cursor-pointer gap-1 border-0 bg-transparent p-0 text-left" onClick={() => setEditing(item)}>
                  <strong className="truncate text-[13px]">{item.name}</strong>
                  <span className="truncate text-[11px] text-[#5c3613]">{item.preview || "空提示词"}</span>
                  <span className="truncate text-[9px] text-neutral-400">{item.source}</span>
                </button>
                <span className="grid grid-flow-col items-center justify-end gap-2 auto-cols-[minmax(72px,max-content)]">
                  {hosts.map((host) => (
                    <span key={host.id} className="grid justify-items-center">
                      <Switch
                        checked={Boolean(item.enabledOn[host.id])}
                        disabled={busy === `prompts:${item.id}:${host.id}`}
                        label={`${item.enabledOn[host.id] ? "撤下" : "分发到"} ${host.name}`}
                        onChange={(enabled) => void togglePrompt(item.id, host.id, enabled)}
                      />
                      {item.modeByHost[host.id] === "copy" ? <Badge tone="neutral" size="sm">copy</Badge> : null}
                    </span>
                  ))}
                </span>
                <button
                  ref={(node) => {
                    if (node) deleteAnchors.current.set(item.id, node);
                    else deleteAnchors.current.delete(item.id);
                  }}
                  type="button"
                  className="grid size-[30px] cursor-pointer place-items-center rounded border-0 bg-transparent p-0 text-[#5c3613] hover:bg-[#e92929] hover:text-white [&_svg]:size-4"
                  aria-label={`删除 ${item.name}`}
                  onClick={() => setPendingDelete(item.id)}
                >
                  <Trash2 />
                </button>
                <Popover open={pendingDelete === item.id} anchor={{ current: pendingDelete ? deleteAnchors.current.get(pendingDelete) ?? null : null }} onClose={() => setPendingDelete(undefined)}>
                  <strong className="block">删除 {item.name}？</strong>
                  <p className="my-1.5 block text-xs text-[#5c3613]">已分发的宿主会恢复备份。</p>
                  <span className="flex justify-end gap-2">
                    <Button type="button" variant="secondary" size="sm" onClick={() => setPendingDelete(undefined)}>取消</Button>
                    <Button type="button" variant="danger" size="sm" disabled={busy === `delete:${item.id}`} onClick={() => void deletePrompt(item.id)}>删除</Button>
                  </span>
                </Popover>
              </div>
            ))}
          </div>
        )}
      </section>
      {editing !== undefined ? (
        <PromptDialog
          prompt={editing}
          busy={busy === "add:prompts" || busy.startsWith("update:")}
          onClose={() => setEditing(undefined)}
          returnFocus={dialogTrigger.current}
          onCreate={createPrompt}
          onUpdate={(item, name, content) => useAppStore.getState().updatePrompt(item, name, content)}
        />
      ) : null}
    </>
  );
}

function PromptDialog({
  prompt,
  busy,
  onClose,
  returnFocus,
  onCreate,
  onUpdate,
}: {
  prompt: Prompt | null;
  busy: boolean;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
  onCreate: (name: string, content: string) => Promise<boolean>;
  onUpdate: (item: Prompt, name: string, content: string) => Promise<boolean>;
}) {
  const dialogRef = useRef<HTMLFormElement>(null);
  const [name, setName] = useState(prompt?.name ?? "");
  const [content, setContent] = useState("");

  useEffect(() => {
    if (!prompt) return;
    void readPrompt(prompt.id).then(setContent);
  }, [prompt]);

  return (
    <div className="fixed inset-0 z-90 grid place-items-center bg-black/35 p-6" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <form
        ref={dialogRef}
        className="grid w-[min(640px,100%)] gap-4 rounded-lg border-2 border-black bg-white p-5 shadow-[6px_6px_0_#000]"
        onSubmit={async (event: FormEvent<HTMLFormElement>) => {
          event.preventDefault();
          const data = new FormData(event.currentTarget);
          const nextName = String(data.get("name") ?? name);
          const nextContent = String(data.get("content") ?? content);
          const ok = prompt ? await onUpdate(prompt, nextName, nextContent) : await onCreate(nextName, nextContent);
          if (ok) {
            returnFocus?.focus();
            onClose();
          }
        }}
      >
        <strong>{prompt ? "编辑提示词" : "新建提示词"}</strong>
        <label className="grid gap-1.5 text-[11px] font-bold text-[#5c3613]">
          名称
          <Input name="name" value={name} onChange={(event) => setName(event.target.value)} required />
        </label>
        <label className="grid gap-1.5 text-[11px] font-bold text-[#5c3613]">
          内容
          <textarea name="content" rows={27} className="min-h-[180px] rounded-md border-2 border-black px-3 py-2 text-xs outline-none focus:border-[#e92929]" value={content} onChange={(event) => setContent(event.target.value)} />
        </label>
        <span className="flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={onClose}>取消</Button>
          <Button type="submit" variant="primary" disabled={busy}>{prompt ? "保存" : "创建"}</Button>
        </span>
      </form>
    </div>
  );
}
