import { useEffect, useRef, useState, type FormEvent } from "react";
import { useBeforeUnload } from "react-router";
import { BookOpenText, FilePenLine, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input } from "@/components/ui/Input";
import { Popover } from "@/components/ui/Popover";
import { readReference, type Reference } from "@/lib/api";
import { useAppStore } from "@/stores/appStore";

export function ReferencesPage() {
  const references = useAppStore((state) => state.snapshot.references).toSorted((left, right) =>
    left.name.localeCompare(right.name),
  );
  const busy = useAppStore((state) => state.busy);
  const createReference = useAppStore((state) => state.createReference);
  const deleteReference = useAppStore((state) => state.deleteReference);
  const [editing, setEditing] = useState<Reference | null>();
  const [returnFocus, setReturnFocus] = useState<HTMLElement | null>(null);
  const [pendingDelete, setPendingDelete] = useState<string>();
  const deleteAnchors = useRef(new Map<string, HTMLButtonElement>());

  return (
    <>
      <section className="mb-6 flex items-center justify-between">
        <span className="text-[11px] text-muted-foreground tabular-nums">
          {references.length} 个条目
        </span>
        <Button
          type="button"
          variant="primary"
          onClick={(event) => {
            setReturnFocus(event.currentTarget);
            setEditing(null);
          }}
        >
          <Plus />
          添加 Reference
        </Button>
      </section>

      <section className="min-w-0 overflow-x-auto rounded-lg border-2 border-border bg-card">
        {references.length === 0 ? (
          <div className="grid min-h-[280px] place-items-center p-8 text-center text-muted-foreground">
            <span className="grid size-[42px] place-items-center rounded-md border border-border bg-primary [&_svg]:size-5">
              <BookOpenText />
            </span>
            <strong className="mt-2.5 text-sm text-foreground">还没有参考文档</strong>
            <p className="mt-1 text-[11px]">添加后保存在 ~/.chu/references。</p>
          </div>
        ) : (
          <div className="min-w-[560px]">
            <div className="grid min-h-[38px] grid-cols-[40px_minmax(240px,1fr)_76px] items-center gap-3 border-b border-border bg-muted px-3.5 text-[9px] font-bold text-muted-foreground uppercase">
              <span className="col-span-2">Reference</span>
              <span className="text-center">操作</span>
            </div>
            {references.map((item) => (
              <div
                key={item.id}
                className="grid min-h-[84px] grid-cols-[40px_minmax(240px,1fr)_76px] items-center gap-3 border-b border-muted-foreground bg-card px-3.5 py-3 last:border-b-0 hover:bg-primary-muted"
              >
                <span className="grid size-9 place-items-center rounded-md border border-border bg-primary [&_svg]:size-4">
                  <BookOpenText />
                </span>
                <button
                  type="button"
                  className="grid min-w-0 cursor-pointer gap-1 border-0 bg-transparent p-0 text-left"
                  onClick={(event) => {
                    setReturnFocus(event.currentTarget);
                    setEditing(item);
                  }}
                >
                  <strong className="truncate text-[13px]">{item.name}</strong>
                  <span className="truncate text-[11px] text-muted-foreground">
                    {item.preview || "空文档"}
                  </span>
                  <span className="truncate text-[9px] text-neutral-400">{item.source}</span>
                </button>
                <span className="flex justify-end gap-1">
                  <button
                    type="button"
                    className="grid size-[30px] cursor-pointer place-items-center rounded border-0 bg-transparent p-0 text-muted-foreground hover:bg-primary hover:text-foreground [&_svg]:size-4"
                    aria-label={`编辑 ${item.name}`}
                    title="编辑"
                    onClick={(event) => {
                      setReturnFocus(event.currentTarget);
                      setEditing(item);
                    }}
                  >
                    <FilePenLine />
                  </button>
                  <button
                    ref={(node) => {
                      if (node) deleteAnchors.current.set(item.id, node);
                      else deleteAnchors.current.delete(item.id);
                    }}
                    type="button"
                    className="grid size-[30px] cursor-pointer place-items-center rounded border-0 bg-transparent p-0 text-muted-foreground hover:bg-destructive hover:text-destructive-foreground [&_svg]:size-4"
                    aria-label={`删除 ${item.name}`}
                    title="删除"
                    onClick={() => setPendingDelete(item.id)}
                  >
                    <Trash2 />
                  </button>
                  <Popover
                    open={pendingDelete === item.id}
                    anchor={{
                      current: pendingDelete
                        ? (deleteAnchors.current.get(pendingDelete) ?? null)
                        : null,
                    }}
                    onClose={() => setPendingDelete(undefined)}
                  >
                    <strong className="block">删除 {item.name}？</strong>
                    <p className="my-1.5 block text-xs text-muted-foreground">
                      文件将从本机永久删除。
                    </p>
                    <span className="flex justify-end gap-2">
                      <Button
                        type="button"
                        variant="secondary"
                        size="sm"
                        onClick={() => setPendingDelete(undefined)}
                      >
                        取消
                      </Button>
                      <Button
                        type="button"
                        variant="danger"
                        size="sm"
                        disabled={busy === `delete:reference:${item.id}`}
                        onClick={() => void deleteReference(item.id)}
                      >
                        删除
                      </Button>
                    </span>
                  </Popover>
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      {editing !== undefined ? (
        <ReferenceDialog
          reference={editing}
          busy={busy === "add:references" || busy === `update:reference:${editing?.id}`}
          onClose={() => setEditing(undefined)}
          returnFocus={returnFocus}
          onCreate={createReference}
          onUpdate={(item, name, content) =>
            useAppStore.getState().updateReference(item, name, content)
          }
        />
      ) : null}
    </>
  );
}

function ReferenceDialog({
  reference,
  busy,
  onClose,
  returnFocus,
  onCreate,
  onUpdate,
}: {
  reference: Reference | null;
  busy: boolean;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
  onCreate: (name: string, content: string) => Promise<boolean>;
  onUpdate: (item: Reference, name: string, content: string) => Promise<boolean>;
}) {
  const initialName = reference?.name ?? "";
  const [name, setName] = useState(initialName);
  const [content, setContent] = useState("");
  const [initialContent, setInitialContent] = useState<string | null>(reference ? null : "");
  const [loadError, setLoadError] = useState("");
  const dirty = name !== initialName || (initialContent !== null && content !== initialContent);

  useEffect(() => {
    if (!reference) return;
    let active = true;
    void readReference(reference.id)
      .then((value) => {
        if (!active) return;
        setContent(value);
        setInitialContent(value);
      })
      .catch((error) => {
        if (active) setLoadError(String(error));
      });
    return () => {
      active = false;
    };
  }, [reference]);

  useBeforeUnload((event) => {
    if (!dirty) return;
    event.preventDefault();
    event.returnValue = "";
  });

  function requestClose() {
    if (dirty && !window.confirm("有未保存的修改，确认放弃吗？")) return;
    onClose();
  }

  return (
    <Dialog
      className="w-[min(640px,100%)] backdrop:bg-foreground/35"
      aria-labelledby="reference-dialog-title"
      onClose={requestClose}
      returnFocus={returnFocus}
    >
      <form
        className="grid gap-4 p-5"
        aria-busy={busy || initialContent === null}
        onSubmit={async (event: FormEvent) => {
          event.preventDefault();
          const ok = reference
            ? await onUpdate(reference, name, content)
            : await onCreate(name, content);
          if (ok) {
            onClose();
          }
        }}
      >
        <strong id="reference-dialog-title">
          {reference ? "编辑 Reference" : "添加 Reference"}
        </strong>
        <label className="grid gap-1.5 text-[11px] font-bold text-muted-foreground">
          名称
          <Input
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            autoFocus
          />
        </label>
        <label className="grid gap-1.5 text-[11px] font-bold text-muted-foreground">
          Markdown 内容
          <textarea
            className="min-h-[240px] resize-y rounded-md border-2 border-input px-3 py-2 text-xs leading-5 outline-none focus:border-ring focus:shadow-focus disabled:bg-neutral-100"
            value={content}
            onChange={(event) => setContent(event.target.value)}
            disabled={initialContent === null}
          />
        </label>
        {loadError ? (
          <p
            className="rounded border-2 border-destructive bg-destructive-surface p-2.5 text-xs text-error-foreground"
            role="alert"
          >
            读取失败：{loadError}
          </p>
        ) : null}
        <span className="flex justify-end gap-2">
          <Button type="button" variant="secondary" onClick={requestClose}>
            取消
          </Button>
          <Button
            type="submit"
            variant="primary"
            disabled={busy || initialContent === null || Boolean(loadError)}
          >
            {reference ? "保存" : "创建"}
          </Button>
        </span>
      </form>
    </Dialog>
  );
}
