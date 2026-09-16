import { useEffect, useRef, useState, type FormEvent } from "react";
import { GitBranch, Plus, X } from "lucide-react";
import type { AgentInput, MCP, MCPInput } from "@/lib/api";
import type { ResourceKind } from "@/lib/resources";
import { useAppStore } from "@/stores/app-store";

export function AddResourceDialog({
  kind,
  onClose,
  returnFocus,
}: {
  kind: ResourceKind;
  onClose: () => void;
  returnFocus?: HTMLElement | null;
}) {
  const [mcpType, setMCPType] = useState<MCP["type"]>("stdio");
  const dialogRef = useRef<HTMLElement>(null);
  const onCloseRef = useRef(onClose);
  const busy = useAppStore((state) => state.busy);
  const installSkill = useAppStore((state) => state.installSkill);
  const addMCP = useAppStore((state) => state.addMCP);
  const addAgent = useAppStore((state) => state.addAgent);
  onCloseRef.current = onClose;

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>("input, select, textarea")?.focus();
  }, []);

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

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const values = Object.fromEntries(new FormData(event.currentTarget).entries()) as Record<
      string,
      string
    >;
    const saved =
      kind === "skills"
        ? await installSkill(values.repository, values.subdir || "", values.name || "")
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
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section
        ref={dialogRef}
        className="dialog"
        role="dialog"
        aria-modal="true"
        aria-labelledby="dialog-title"
        onMouseDown={(event) => event.stopPropagation()}
      >
        <header className="dialog-header">
          <div>
            <h2 id="dialog-title">{title}</h2>
          </div>
          <button
            type="button"
            className="icon-button"
            aria-label="关闭"
            title="关闭"
            onClick={onClose}
          >
            <X />
          </button>
        </header>
        <form className="dialog-form" onSubmit={submit}>
          {kind === "skills" ? (
            <>
              <label>
                公开 HTTPS 仓库
                <input
                  name="repository"
                  type="url"
                  required
                  autoFocus
                  placeholder="https://github.com/org/skills.git"
                />
              </label>
              <div className="form-grid">
                <label>
                  仓库子目录
                  <input name="subdir" placeholder="skills/code-review" />
                </label>
                <label>
                  本地名称
                  <input name="name" placeholder="自动识别" />
                </label>
              </div>
              <p className="form-note">
                <GitBranch />
                只读安装到 <code>~/.chu/skills</code>，更新由你手动触发。
              </p>
            </>
          ) : null}
          {kind === "mcps" ? (
            <>
              <div className="form-grid">
                <label>
                  名称
                  <input name="name" required autoFocus placeholder="filesystem" />
                </label>
                <label>
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
              <label>
                描述
                <input name="description" placeholder="这个 MCP 提供什么能力" />
              </label>
              {mcpType === "stdio" ? (
                <div className="form-grid">
                  <label>
                    可执行文件
                    <input name="command" required placeholder="npx" />
                  </label>
                  <label>
                    参数（空格分隔）
                    <input name="args" placeholder="-y @modelcontextprotocol/server-filesystem" />
                  </label>
                </div>
              ) : (
                <label>
                  服务 URL
                  <input
                    name="endpoint"
                    type="url"
                    required
                    placeholder="https://service.example/mcp"
                  />
                </label>
              )}
              <label>
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
              <div className="form-grid">
                <label>
                  名称
                  <input name="name" required autoFocus placeholder="researcher" />
                </label>
                <label>
                  模型
                  <input name="model" placeholder="inherit" />
                </label>
              </div>
              <label>
                描述
                <input name="description" placeholder="什么时候使用这个 Agent" />
              </label>
              <label>
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
          <footer className="dialog-footer">
            <button type="button" className="button button-secondary" onClick={onClose}>
              取消
            </button>
            <button
              type="submit"
              className="button button-primary"
              disabled={busy === `add:${kind}`}
            >
              <Plus />
              {kind === "skills" ? "安装" : "创建"}
            </button>
          </footer>
        </form>
      </section>
    </div>
  );
}
