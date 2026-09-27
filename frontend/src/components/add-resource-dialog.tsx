import { useEffect, useRef, useState, type FormEvent } from "react";
import { tv } from "tailwind-variants";
import { GitBranch, Plus, Search, X } from "lucide-react";
import type { AgentInput, MCP, MCPInput, SkillCandidate } from "@/lib/api";
import type { ResourceKind } from "@/lib/resources";
import { Button } from "@/components/ui/button";
import { useAppStore } from "@/stores/app-store";

const field = "grid gap-1.5 text-[11px] font-bold text-[#5c3613] [&_input]:min-h-10 [&_input]:rounded-md [&_input]:border-2 [&_input]:border-black [&_input]:px-3 [&_input]:text-xs [&_input]:outline-none [&_input]:focus:border-[#e92929] [&_select]:min-h-10 [&_select]:rounded-md [&_select]:border-2 [&_select]:border-black [&_select]:px-3 [&_select]:text-xs [&_textarea]:min-h-[150px] [&_textarea]:rounded-md [&_textarea]:border-2 [&_textarea]:border-black [&_textarea]:px-3 [&_textarea]:py-2 [&_textarea]:text-xs [&_textarea]:outline-none [&_textarea]:focus:border-[#e92929]";
const skillOption = tv({
  base: "grid grid-cols-[18px_minmax(0,1fr)] items-start gap-1.5 rounded-md border-2 border-black bg-[#f7f5ec] p-2 text-[11px] font-bold text-[#5c3613]",
  variants: { picked: { true: "bg-primary" } },
});

export function AddResourceDialog({
  kind,
  onClose,
  returnFocus,
  initialRepository = "",
}: {
  kind: ResourceKind;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
  initialRepository?: string;
}) {
  const [mcpType, setMCPType] = useState<MCP["type"]>("stdio");
  const [repository, setRepository] = useState(initialRepository);
  const repositoryRef = useRef<HTMLInputElement>(null);
  const [candidates, setCandidates] = useState<SkillCandidate[]>();
  const [selected, setSelected] = useState<string[]>([]);
  const dialogRef = useRef<HTMLElement>(null);
  const onCloseRef = useRef(onClose);
  const busy = useAppStore((state) => state.busy);
  const previewSkills = useAppStore((state) => state.previewSkills);
  const installSkills = useAppStore((state) => state.installSkills);
  const addMCP = useAppStore((state) => state.addMCP);
  const addAgent = useAppStore((state) => state.addAgent);
  onCloseRef.current = onClose;

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>("input, select, textarea")?.focus();
    if (kind === "skills" && initialRepository) {
      void previewSkills(initialRepository).then(setCandidates);
    }
  }, [initialRepository, kind, previewSkills]);

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const dialog = dialogRef.current;
      if (!dialog) return;
      if (event.key === "Escape") {
        event.preventDefault();
        onCloseRef.current();
        return;
      }
      if (event.key !== "Tab") return;

      const focusable = Array.from(
        dialog.querySelectorAll<HTMLElement>(
          "button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex='-1'])",
        ),
      );
      const first = focusable[0];
      const last = focusable.at(-1);
      if (!first || !last) return;
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      returnFocus?.focus();
    };
  }, [returnFocus]);

  function repositoryValue() {
    return (repositoryRef.current?.value || repository).trim();
  }

  async function findSkills() {
    const found = await previewSkills(repositoryValue());
    setCandidates(found);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (kind === "skills" && !candidates) {
      await findSkills();
      return;
    }
    const values = Object.fromEntries(new FormData(event.currentTarget).entries()) as Record<
      string,
      string
    >;
    const saved =
      kind === "skills"
        ? await installSkills(repositoryValue(), selected)
        : kind === "mcps"
          ? await addMCP({
              name: values.name,
              description: values.description || "",
              type: values.type as MCP["type"],
              endpoint: values.endpoint || "",
              command: values.command || "",
              args: (values.args || "").split(/\s+/).filter(Boolean),
              env: {},
              headers: {},
              secret: values.secret || "",
            } satisfies MCPInput)
          : await addAgent({
              name: values.name,
              description: values.description || "",
              prompt: values.prompt,
              model: values.model || "inherit",
            } satisfies AgentInput);
    if (saved) onClose();
  }

  const title =
    kind === "skills"
      ? "从 Git 安装 Skill"
      : kind === "mcps"
        ? "添加 MCP 服务"
        : "创建自定义 Agent";
  return (
    <div className="fixed inset-0 z-100 grid place-items-center bg-black/58 p-6" role="presentation" onMouseDown={onClose}>
      <section
        ref={dialogRef}
        className="max-h-[calc(100vh-48px)] w-[min(560px,100%)] overflow-auto rounded-lg border-2 border-black bg-white shadow-[8px_8px_0_#000]"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="flex items-center justify-between border-b-2 border-black p-5">
          <div>
            <h2 id="dialog-title" className="m-0 text-[21px] leading-tight font-extrabold">{title}</h2>
          </div>
          <button
            type="button"
            className="inline-grid size-10 cursor-pointer place-items-center rounded-md border-2 border-black bg-white shadow-[2px_2px_0_#000] [&_svg]:size-4"
            aria-label="关闭"
            title="关闭"
            onClick={onClose}
          >
            <X />
          </button>
        </header>
        <form className="grid gap-4 p-5" onSubmit={submit}>
          {kind === "skills" ? (
            <>
              <label className={field}>
                仓库
                <input
                  ref={repositoryRef}
                  name="repository"
                  type="text"
                  required
                  autoFocus
                  defaultValue={initialRepository}
                  onInput={(event) => {
                    setRepository(event.currentTarget.value);
                    setCandidates(undefined);
                    setSelected([]);
                  }}
                  placeholder="owner/repo 或 https://github.com/org/skills"
                />
              </label>
              {candidates ? (
                <div className="grid max-h-[280px] gap-2 overflow-auto">
                  {candidates.length === 0 ? <p>没有发现可安装的 skill。</p> : null}
                  {candidates.map((item) => (
                    <label key={item.path} className={skillOption({ picked: selected.includes(item.path) })}>
                      <input
                        type="checkbox"
                        checked={selected.includes(item.path)}
                        disabled={item.installed}
                        onChange={(event) =>
                          setSelected((current) =>
                            event.target.checked
                              ? [...current, item.path]
                              : current.filter((path) => path !== item.path),
                          )
                        }
                      />
                      <span>
                        <strong>{item.name}</strong>
                        <small>{item.description}</small>
                      </span>
                    </label>
                  ))}
                </div>
              ) : null}
              <p className="m-0 flex items-center gap-2 rounded border border-dashed border-[#5c3613] bg-[#f7f5ec] p-2.5 text-[10px] text-[#5c3613] [&_svg]:size-4 [&_code]:text-[9px]">
                <GitBranch />
                默认不勾选。安装后写入 <code>~/.chu/chu-lock.json</code>。
              </p>
            </>
          ) : null}
          {kind === "mcps" ? (
            <>
              <div className="grid grid-cols-2 gap-3">
                <label className={field}>
                  名称
                  <input name="name" required autoFocus placeholder="filesystem" />
                </label>
                <label className={field}>
                  连接类型
                  <select
                    name="type"
                    value={mcpType}
                    onChange={(event) => setMCPType(event.target.value as MCP["type"])}
                  >
                    <option value="stdio">stdio</option>
                    <option value="http">HTTP</option>
                    <option value="sse">SSE</option>
                  </select>
                </label>
              </div>
              <label className={field}>
                描述
                <input name="description" placeholder="这个 MCP 提供什么能力" />
              </label>
              {mcpType === "stdio" ? (
                <div className="grid grid-cols-2 gap-3">
                  <label className={field}>
                    可执行文件
                    <input name="command" required placeholder="npx" />
                  </label>
                  <label className={field}>
                    参数（空格分隔）
                    <input name="args" placeholder="-y @modelcontextprotocol/server-filesystem" />
                  </label>
                </div>
              ) : (
                <label className={field}>
                  服务 URL
                  <input
                    name="endpoint"
                    type="url"
                    required
                    placeholder="https://service.example/mcp"
                  />
                </label>
              )}
              <label className={field}>
                Token / API Key
                <input
                  name="secret"
                  type="password"
                  autoComplete="off"
                  placeholder="明文写入本地配置"
                />
              </label>
            </>
          ) : null}
          {kind === "agents" ? (
            <>
              <div className="grid grid-cols-2 gap-3">
                <label className={field}>
                  名称
                  <input name="name" required autoFocus placeholder="researcher" />
                </label>
                <label className={field}>
                  模型
                  <input name="model" placeholder="inherit" />
                </label>
              </div>
              <label className={field}>
                描述
                <input name="description" placeholder="什么时候使用这个 Agent" />
              </label>
              <label className={field}>
                系统提示词
                <textarea
                  name="prompt"
                  required
                  rows={8}
                  placeholder="定义职责、边界和输出要求..."
                />
              </label>
            </>
          ) : null}
          <footer className="-mx-5 -mb-5 flex justify-end gap-2.5 border-t-2 border-black bg-[#f7f5ec] px-5 py-3.5">
            <Button type="button" variant="secondary" onClick={onClose}>
              取消
            </Button>
            {kind === "skills" && !candidates ? (
              <Button
                type="submit"
                variant="primary"
                disabled={busy === "preview:skills" || repositoryValue() === ""}
              >
                <Search />
                {busy === "preview:skills" ? "正在查找…" : "查找 Skill"}
              </Button>
            ) : (
              <Button
                type="submit"
                variant="primary"
                disabled={busy === `add:${kind}` || (kind === "skills" && selected.length === 0)}
              >
                <Plus />
                {kind === "skills" ? `安装 ${selected.length || ""}`.trim() : "创建"}
              </Button>
            )}
          </footer>
        </form>
      </section>
    </div>
  );
}
