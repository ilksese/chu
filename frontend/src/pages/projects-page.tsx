import { useState, type FormEvent } from "react";
import { Link } from "react-router";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import {
  Check,
  ChevronDown,
  FolderKanban,
  FolderOpen,
  Pencil,
  Plus,
  RotateCcw,
  Trash2,
  X,
} from "lucide-react";
import { HostMark, Switch } from "@/components/HostControls";
import { AnimatedList } from "@/components/ui/AnimatedList";
import { Badge } from "@/components/ui/Badge";
import { Button } from "@/components/ui/Button";
import { Dialog } from "@/components/ui/Dialog";
import { Input } from "@/components/ui/Input";
import type { Host, Project, ProjectDeployment } from "@/lib/api";
import { useAppStore } from "@/stores/appStore";

const statusCopy: Record<ProjectDeployment["status"], string> = {
  enabled: "已同步",
  modified: "本地已修改",
  missing: "副本缺失",
  conflict: "同名冲突",
};

export function ProjectsPage() {
  const snapshot = useAppStore((state) => state.snapshot);
  const busy = useAppStore((state) => state.busy);
  const addProject = useAppStore((state) => state.addProject);
  const renameProject = useAppStore((state) => state.renameProject);
  const relocateProject = useAppStore((state) => state.relocateProject);
  const toggleProjectSkill = useAppStore((state) => state.toggleProjectSkill);
  const resetProjectSkill = useAppStore((state) => state.resetProjectSkill);
  const deleteProject = useAppStore((state) => state.deleteProject);
  const reducedMotion = useReducedMotion();
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const [editing, setEditing] = useState<string>();
  const [name, setName] = useState("");
  const [pendingDelete, setPendingDelete] = useState<Project>();
  const skills = snapshot.skills
    .filter((skill) => skill.managed)
    .toSorted((a, b) => a.name.localeCompare(b.name));

  function toggleExpanded(projectID: string) {
    setExpanded((current) => {
      const next = new Set(current);
      if (next.has(projectID)) next.delete(projectID);
      else next.add(projectID);
      return next;
    });
  }

  async function submitName(event: FormEvent<HTMLFormElement>, projectID: string) {
    event.preventDefault();
    if (await renameProject(projectID, name)) setEditing(undefined);
  }

  return (
    <>
      <section className="mb-6 flex items-center justify-between gap-6">
        <p className="m-0 text-xs text-muted-foreground">
          {snapshot.projects.length} 个项目 · 项目目录中的修改会被保护
        </p>
        <Button
          type="button"
          variant="primary"
          disabled={busy === "add:project"}
          onClick={() => void addProject()}
        >
          <Plus />
          新增项目
        </Button>
      </section>

      {snapshot.projects.length === 0 ? (
        <section className="grid min-h-[360px] place-items-center rounded-lg border-2 border-dashed border-muted-foreground bg-card p-8 text-center">
          <div>
            <span className="mx-auto grid size-12 place-items-center rounded-lg border-2 border-border bg-primary shadow-neo-sm [&_svg]:size-5">
              <FolderKanban />
            </span>
            <strong className="mt-4 block text-base">还没有关联项目</strong>
            <p className="mt-1.5 text-xs text-muted-foreground">
              选择一个本地文件夹，集中配置项目级 Skill。
            </p>
          </div>
        </section>
      ) : (
        <AnimatedList aria-label="项目列表">
          {snapshot.projects.map((project) => {
            const open = expanded.has(project.id);
            const hosts = visibleHosts(project, snapshot.hosts);
            const deploymentCount = Object.values(project.deployments).reduce(
              (count, byHost) =>
                count + Object.values(byHost).filter((item) => item.enabled).length,
              0,
            );
            return (
              <article
                key={project.id}
                className="overflow-hidden rounded-lg border-2 border-border bg-card shadow-neo-sm"
              >
                <div className="flex min-h-[78px] items-center gap-3 px-4 py-3">
                  {editing === project.id ? (
                    <form
                      className="flex min-w-0 flex-1 items-center gap-3"
                      onSubmit={(event) => void submitName(event, project.id)}
                    >
                      <span className="grid size-10 shrink-0 place-items-center rounded-md border-2 border-border bg-primary [&_svg]:size-[18px]">
                        <FolderKanban />
                      </span>
                      <Input
                        autoFocus
                        className="max-w-[420px]"
                        value={name}
                        onChange={(event) => setName(event.target.value)}
                        aria-label="项目名称"
                      />
                      <Button type="submit" size="icon" variant="primary" aria-label="保存项目名称">
                        <Check />
                      </Button>
                      <Button
                        type="button"
                        size="icon"
                        variant="secondary"
                        aria-label="取消重命名"
                        onClick={() => setEditing(undefined)}
                      >
                        <X />
                      </Button>
                    </form>
                  ) : (
                    <button
                      type="button"
                      className="flex min-w-0 flex-1 cursor-pointer items-center gap-3 border-0 bg-transparent p-0 text-left"
                      aria-expanded={open}
                      aria-controls={`project-content-${project.id}`}
                      onClick={() => toggleExpanded(project.id)}
                    >
                      <span className="grid size-10 shrink-0 place-items-center rounded-md border-2 border-border bg-primary [&_svg]:size-[18px]">
                        <FolderKanban />
                      </span>
                      <span className="grid min-w-0 flex-1 gap-1">
                        <strong className="truncate text-sm">{project.name}</strong>
                        <span
                          className="truncate text-[10px] text-muted-foreground"
                          title={project.path}
                        >
                          {project.path}
                        </span>
                      </span>
                      <span className="flex shrink-0 items-center gap-2">
                        <Badge tone={project.available ? "neutral" : "warning"}>
                          {project.available ? `${deploymentCount} 项部署` : "目录不可用"}
                        </Badge>
                        <ChevronDown
                          className={`size-4 transition-transform ${open ? "rotate-180" : ""}`}
                        />
                      </span>
                    </button>
                  )}
                  {editing !== project.id ? (
                    <Button
                      type="button"
                      size="icon"
                      variant="ghost"
                      aria-label={`重命名 ${project.name}`}
                      onClick={() => {
                        setName(project.name);
                        setEditing(project.id);
                      }}
                    >
                      <Pencil />
                    </Button>
                  ) : null}
                  <Button
                    type="button"
                    size="icon"
                    variant="ghost"
                    aria-label={`删除 ${project.name}`}
                    onClick={() => setPendingDelete(project)}
                  >
                    <Trash2 />
                  </Button>
                </div>

                <AnimatePresence initial={false}>
                  {open ? (
                    <motion.div
                      id={`project-content-${project.id}`}
                      className="overflow-hidden"
                      initial={reducedMotion ? false : { height: 0, opacity: 0 }}
                      animate={{ height: "auto", opacity: 1 }}
                      exit={{ height: 0, opacity: 0 }}
                      transition={
                        reducedMotion
                          ? { duration: 0 }
                          : { duration: 0.18, ease: [0.16, 1, 0.3, 1] }
                      }
                    >
                      <div className="border-t-2 border-border">
                        {!project.available ? (
                          <div className="flex items-center justify-between gap-4 bg-warning-surface px-4 py-3 text-xs text-warning-foreground">
                            <span>
                              找不到项目根目录。重新选择后会按原配置部署，冲突文件不会被覆盖。
                            </span>
                            <Button
                              type="button"
                              size="sm"
                              variant="secondary"
                              disabled={busy === `relocate:project:${project.id}`}
                              onClick={() => void relocateProject(project.id)}
                            >
                              <FolderOpen />
                              重新定位
                            </Button>
                          </div>
                        ) : null}
                        {skills.length === 0 ? (
                          <div className="grid min-h-36 place-items-center p-6 text-center text-xs text-muted-foreground">
                            <span>
                              没有 Chu 管理的 Skill。
                              <Link
                                className="font-bold text-foreground underline underline-offset-4"
                                to="/skills"
                              >
                                前往技能页安装
                              </Link>
                            </span>
                          </div>
                        ) : hosts.length === 0 ? (
                          <div className="grid min-h-36 place-items-center p-6 text-center text-xs text-muted-foreground">
                            未检测到可配置的 Agent 宿主。
                          </div>
                        ) : (
                          <div className="min-w-[680px] overflow-x-auto">
                            <div
                              className="grid min-h-11 items-center gap-3 border-b border-border bg-muted px-4 text-[10px] font-bold text-muted-foreground"
                              style={columns(hosts.length)}
                            >
                              <span>Skill</span>
                              {hosts.map((host) => (
                                <span
                                  key={host.id}
                                  className="flex items-center justify-center gap-1.5"
                                >
                                  <HostMark host={host} compact />
                                  {host.name}
                                </span>
                              ))}
                            </div>
                            {skills.map((skill) => (
                              <div
                                key={skill.id}
                                className="grid min-h-[76px] items-center gap-3 border-b border-muted-foreground px-4 py-2.5 last:border-b-0 hover:bg-primary-muted"
                                style={columns(hosts.length)}
                              >
                                <span className="grid min-w-0 gap-1">
                                  <strong className="truncate text-xs">{skill.name}</strong>
                                  <span className="truncate text-[10px] text-muted-foreground">
                                    {skill.description || "无描述"}
                                  </span>
                                </span>
                                {hosts.map((host) => {
                                  const deployment = project.deployments[skill.id]?.[host.id];
                                  const operation = `project:${project.id}:${skill.id}:${host.id}`;
                                  return (
                                    <span
                                      key={host.id}
                                      className="grid min-h-12 place-items-center content-center gap-0.5"
                                    >
                                      <Switch
                                        checked={Boolean(deployment?.enabled)}
                                        disabled={!project.available || busy === operation}
                                        label={`${deployment?.enabled ? "关闭" : "启用"} ${project.name} 的 ${skill.name}（${host.name}）`}
                                        onChange={(enabled) =>
                                          void toggleProjectSkill(
                                            project.id,
                                            skill.id,
                                            host.id,
                                            enabled,
                                          )
                                        }
                                      />
                                      {deployment ? (
                                        <span className="flex h-[18px] items-center gap-1">
                                          <StatusBadge deployment={deployment} />
                                          {deployment.enabled &&
                                          (deployment.status === "modified" ||
                                            deployment.status === "missing") ? (
                                            <button
                                              type="button"
                                              className="grid size-[18px] cursor-pointer place-items-center rounded border border-border bg-card text-muted-foreground hover:bg-primary disabled:cursor-wait disabled:opacity-50 [&_svg]:size-2.5"
                                              aria-label={`将 ${skill.name} 重置为中央版本`}
                                              title="重置为中央版本"
                                              disabled={
                                                busy ===
                                                `reset:project:${project.id}:${skill.id}:${host.id}`
                                              }
                                              onClick={() =>
                                                void resetProjectSkill(
                                                  project.id,
                                                  skill.id,
                                                  host.id,
                                                )
                                              }
                                            >
                                              <RotateCcw />
                                            </button>
                                          ) : null}
                                        </span>
                                      ) : (
                                        <span className="h-[18px]" />
                                      )}
                                    </span>
                                  );
                                })}
                              </div>
                            ))}
                          </div>
                        )}
                      </div>
                    </motion.div>
                  ) : null}
                </AnimatePresence>
              </article>
            );
          })}
        </AnimatedList>
      )}

      {pendingDelete ? (
        <Dialog
          className="w-[min(520px,100%)] backdrop:bg-foreground/35"
          aria-labelledby="delete-project-title"
          onClose={() => setPendingDelete(undefined)}
        >
          <section className="p-5">
            <strong id="delete-project-title" className="text-base">
              移除 {pendingDelete.name}？
            </strong>
            <p className="mt-2 text-xs leading-5 text-muted-foreground">
              项目根目录不会被删除。你可以只移除 Chu 中的关联，或同时清理 Chu 创建且未被修改的 Skill
              副本。
            </p>
            <div className="mt-5 flex flex-wrap justify-end gap-2">
              <Button
                type="button"
                variant="secondary"
                autoFocus
                onClick={() => setPendingDelete(undefined)}
              >
                取消
              </Button>
              <Button
                type="button"
                variant="danger"
                disabled={busy === `delete:project:${pendingDelete.id}`}
                onClick={async () => {
                  if (await deleteProject(pendingDelete.id, true)) setPendingDelete(undefined);
                }}
              >
                <Trash2 />
                移除并清理
              </Button>
              <Button
                type="button"
                variant="primary"
                disabled={busy === `delete:project:${pendingDelete.id}`}
                onClick={async () => {
                  if (await deleteProject(pendingDelete.id, false)) setPendingDelete(undefined);
                }}
              >
                仅从 Chu 移除
              </Button>
            </div>
          </section>
        </Dialog>
      ) : null}
    </>
  );
}

function visibleHosts(project: Project, hosts: Host[]) {
  return hosts.filter(
    (host) =>
      host.installed ||
      Object.values(project.deployments).some((byHost) => byHost[host.id]?.enabled),
  );
}

function columns(hostCount: number) {
  return { gridTemplateColumns: `minmax(240px, 1fr) repeat(${hostCount}, minmax(92px, 110px))` };
}

function StatusBadge({ deployment }: { deployment: ProjectDeployment }) {
  const warning = deployment.status === "modified" || deployment.status === "missing";
  const conflict = deployment.status === "conflict";
  return (
    <Badge
      size="sm"
      tone={warning ? "warning" : "neutral"}
      className={conflict ? "bg-destructive text-destructive-foreground" : undefined}
    >
      {statusCopy[deployment.status]}
    </Badge>
  );
}
