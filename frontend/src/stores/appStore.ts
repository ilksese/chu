import { create } from "zustand";
import {
  addAgent as addAgentAPI,
  addMCP as addMCPAPI,
  getSnapshot,
  checkSkillUpdates,
  importSkill as importSkillAPI,
  installSkills,
  previewSkills,
  refreshSnapshot,
  deleteSkill,
  removeSkill,
  updateSkill,
  createPrompt,
  createReference,
  deleteReference,
  deletePrompt,
  restoreBackup,
  testMCP as testMCPAPI,
  togglePrompt,
  updateReference,
  updatePrompt,
  toggleAgent,
  toggleMCP,
  toggleSkill,
  updateHostPaths,
  addProject,
  deleteProject as deleteProjectAPI,
  relocateProject,
  renameProject,
  resetProjectSkill,
  toggleProjectSkill,
  type AgentInput,
  type Host,
  type MCPInput,
  type Prompt,
  type Reference,
  type SkillCandidate,
  type SkillUpdate,
  type Snapshot,
} from "@/lib/api";
import type { Resource, ResourceKind } from "@/lib/resources";

const emptySnapshot: Snapshot = {
  root: "~/.chu",
  hosts: [],
  skills: [],
  mcps: [],
  agents: [],
  prompts: [],
  references: [],
  projects: [],
  lastScan: "",
};

const hiddenHostsStorageKey = "chu:hidden-hosts:v1";

function readHiddenHostIDs() {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(hiddenHostsStorageKey) ?? "[]");
    return Array.isArray(value)
      ? [...new Set(value.filter((hostID): hostID is string => typeof hostID === "string"))]
      : [];
  } catch {
    return [];
  }
}

function writeHiddenHostIDs(hostIDs: string[]) {
  try {
    if (hostIDs.length) localStorage.setItem(hiddenHostsStorageKey, JSON.stringify(hostIDs));
    else localStorage.removeItem(hiddenHostsStorageKey);
  } catch {
    // UI state still works for the current session when storage is unavailable.
  }
}

export function filterVisibleHosts(hosts: Host[], hiddenHostIDs: string[]) {
  return hosts.filter((host) => !hiddenHostIDs.includes(host.id));
}

type Notice = { message: string; error?: boolean };
type HostPaths = Pick<Host, "configPath" | "skillPath" | "agentPath">;

type AppStore = {
  snapshot: Snapshot;
  hiddenHostIDs: string[];
  skillUpdates: SkillUpdate[];
  busy: string;
  notice?: Notice;
  demo: boolean;
  toggleHostVisibility: (hostID: string, visible: boolean) => void;
  initialize: () => Promise<void>;
  clearNotice: () => void;
  refresh: () => Promise<boolean>;
  toggleResource: (
    kind: ResourceKind,
    itemID: string,
    hostID: string,
    enabled: boolean,
  ) => Promise<boolean>;
  importSkill: (item: Resource) => Promise<boolean>;
  testMCP: (itemID: string) => Promise<void>;
  previewSkills: (repository: string) => Promise<SkillCandidate[]>;
  installSkills: (repository: string, paths: string[]) => Promise<boolean>;
  checkSkillUpdates: () => Promise<SkillUpdate[]>;
  updateSkill: (skillID: string) => Promise<boolean>;
  deleteSkill: (skillID: string) => Promise<boolean>;
  removeSkill: (skillID: string) => Promise<boolean>;
  addMCP: (input: MCPInput) => Promise<boolean>;
  addAgent: (input: AgentInput) => Promise<boolean>;
  restoreHost: (host: Host) => Promise<boolean>;
  updateHost: (host: Host, paths: HostPaths) => Promise<boolean>;
  createPrompt: (name: string, content: string) => Promise<boolean>;
  updatePrompt: (item: Prompt, name: string, content: string) => Promise<boolean>;
  deletePrompt: (id: string) => Promise<boolean>;
  togglePrompt: (id: string, hostID: string, enabled: boolean) => Promise<boolean>;
  createReference: (name: string, content: string) => Promise<boolean>;
  updateReference: (item: Reference, name: string, content: string) => Promise<boolean>;
  deleteReference: (id: string) => Promise<boolean>;
  addProject: () => Promise<boolean>;
  renameProject: (projectID: string, name: string) => Promise<boolean>;
  relocateProject: (projectID: string) => Promise<boolean>;
  toggleProjectSkill: (
    projectID: string,
    skillID: string,
    hostID: string,
    enabled: boolean,
  ) => Promise<boolean>;
  resetProjectSkill: (projectID: string, skillID: string, hostID: string) => Promise<boolean>;
  deleteProject: (projectID: string, cleanup: boolean) => Promise<boolean>;
};

let previewSerial = 0;

export const useAppStore = create<AppStore>((set, get) => {
  async function run(operation: string, action: () => Promise<Snapshot>, success: string) {
    set({ busy: operation, notice: undefined });
    try {
      const snapshot = await action();
      set({ snapshot, notice: { message: success } });
      return true;
    } catch (error) {
      set({ notice: { message: String(error), error: true } });
      return false;
    } finally {
      set({ busy: "" });
    }
  }

  return {
    snapshot: emptySnapshot,
    hiddenHostIDs: readHiddenHostIDs(),
    skillUpdates: [],
    busy: "",
    demo: false,

    toggleHostVisibility: (hostID, visible) =>
      set((state) => {
        const hiddenHostIDs = visible
          ? state.hiddenHostIDs.filter((id) => id !== hostID)
          : state.hiddenHostIDs.includes(hostID)
            ? state.hiddenHostIDs
            : [...state.hiddenHostIDs, hostID];
        writeHiddenHostIDs(hiddenHostIDs);
        return { hiddenHostIDs };
      }),

    initialize: async () => {
      try {
        const { snapshot, demo } = await getSnapshot();
        set({ snapshot, demo });
      } catch (error) {
        set({ notice: { message: String(error), error: true } });
      }
    },

    clearNotice: () => set({ notice: undefined }),
    refresh: () => run("refresh", refreshSnapshot, "扫描已完成"),

    toggleResource: (kind, itemID, hostID, enabled) => {
      const action =
        kind === "skills"
          ? () => toggleSkill(itemID, hostID, enabled)
          : kind === "mcps"
            ? () => toggleMCP(itemID, hostID, enabled)
            : () => toggleAgent(itemID, hostID, enabled);
      return run(
        `${kind}:${itemID}:${hostID}`,
        action,
        enabled ? "部署已写入宿主配置" : "已取消该宿主部署",
      );
    },

    importSkill: (item) => {
      const sourceHost = Object.entries(item.enabledOn).find(([, enabled]) => enabled)?.[0];
      if (!sourceHost) return Promise.resolve(false);
      return run(
        `import:${item.id}`,
        () => importSkillAPI(sourceHost, item.name),
        `${item.name} 已纳入 Chu 管理`,
      );
    },

    testMCP: async (itemID) => {
      set({ busy: `test:${itemID}` });
      try {
        set({ notice: { message: await testMCPAPI(itemID) } });
      } catch (error) {
        set({ notice: { message: String(error), error: true } });
      } finally {
        set({ busy: "" });
      }
    },

    previewSkills: async (repository) => {
      const serial = ++previewSerial;
      set({ busy: "preview:skills", notice: undefined });
      try {
        const found = await previewSkills(repository);
        if (serial === previewSerial) set({ busy: "" });
        return found;
      } catch (error) {
        if (serial === previewSerial) {
          set({ busy: "", notice: { message: String(error), error: true } });
        }
        return [];
      }
    },
    installSkills: (repository, paths) =>
      run("add:skills", () => installSkills(repository, paths), "Skill 安装完成"),
    checkSkillUpdates: async () => {
      set({ busy: "updates:skills", notice: undefined });
      try {
        const updates = (await checkSkillUpdates()) ?? [];
        set({
          skillUpdates: updates,
          notice: {
            message: updates.length ? `发现 ${updates.length} 个变化` : "没有可更新的 skill",
          },
        });
        return updates;
      } catch (error) {
        set({ notice: { message: String(error), error: true } });
        return [];
      } finally {
        set({ busy: "" });
      }
    },
    updateSkill: async (skillID) => {
      const ok = await run(`update:${skillID}`, () => updateSkill(skillID), "Skill 已更新");
      if (ok)
        set((state) => ({
          skillUpdates: state.skillUpdates.filter((item) => item.id !== skillID),
        }));
      return ok;
    },
    deleteSkill: (skillID) => run(`delete:${skillID}`, () => deleteSkill(skillID), "Skill 已删除"),
    removeSkill: async (skillID) => {
      const ok = await run(`remove:${skillID}`, () => removeSkill(skillID), "Skill 已移除");
      if (ok)
        set((state) => ({
          skillUpdates: state.skillUpdates.filter((item) => item.id !== skillID),
        }));
      return ok;
    },
    addMCP: (input) => run("add:mcps", () => addMCPAPI(input), "资源已保存到 Chu"),
    addAgent: (input) => run("add:agents", () => addAgentAPI(input), "资源已保存到 Chu"),
    restoreHost: (host) =>
      run(`restore:${host.id}`, () => restoreBackup(host.id), `${host.name} 已恢复上次备份`),
    createPrompt: (name, content) =>
      run("add:prompts", () => createPrompt(name, content), "提示词已保存"),
    updatePrompt: (item, name, content) =>
      run(`update:${item.id}`, () => updatePrompt(item.id, name, content), "提示词已更新"),
    deletePrompt: (id) => run(`delete:${id}`, () => deletePrompt(id), "提示词已删除"),
    togglePrompt: (id, hostID, enabled) =>
      run(
        `prompts:${id}:${hostID}`,
        () => togglePrompt(id, hostID, enabled),
        enabled ? "提示词已分发" : "已恢复宿主原文件",
      ),
    createReference: (name, content) =>
      run("add:references", () => createReference(name, content), "Reference 已保存"),
    updateReference: (item, name, content) =>
      run(
        `update:reference:${item.id}`,
        () => updateReference(item.id, name, content),
        "Reference 已更新",
      ),
    deleteReference: (id) =>
      run(`delete:reference:${id}`, () => deleteReference(id), "Reference 已删除"),
    addProject: async () => {
      const count = get().snapshot.projects.length;
      set({ busy: "add:project", notice: undefined });
      try {
        const snapshot = await addProject();
        const added = snapshot.projects.length > count;
        set({ snapshot, notice: added ? { message: "项目已关联" } : undefined });
        return added;
      } catch (error) {
        set({ notice: { message: String(error), error: true } });
        return false;
      } finally {
        set({ busy: "" });
      }
    },
    renameProject: (projectID, name) =>
      run(`rename:project:${projectID}`, () => renameProject(projectID, name), "项目名称已更新"),
    relocateProject: async (projectID) => {
      const path = get().snapshot.projects.find((project) => project.id === projectID)?.path;
      set({ busy: `relocate:project:${projectID}`, notice: undefined });
      try {
        const snapshot = await relocateProject(projectID);
        const moved = snapshot.projects.find((project) => project.id === projectID)?.path !== path;
        set({ snapshot, notice: moved ? { message: "项目目录已重新关联" } : undefined });
        return moved;
      } catch (error) {
        set({ notice: { message: String(error), error: true } });
        return false;
      } finally {
        set({ busy: "" });
      }
    },
    toggleProjectSkill: (projectID, skillID, hostID, enabled) =>
      run(
        `project:${projectID}:${skillID}:${hostID}`,
        () => toggleProjectSkill(projectID, skillID, hostID, enabled),
        enabled ? "Skill 已复制到项目" : "项目 Skill 已关闭",
      ),
    resetProjectSkill: (projectID, skillID, hostID) =>
      run(
        `reset:project:${projectID}:${skillID}:${hostID}`,
        () => resetProjectSkill(projectID, skillID, hostID),
        "项目 Skill 已重置为中央版本",
      ),
    deleteProject: async (projectID, cleanup) => {
      set({ busy: `delete:project:${projectID}`, notice: undefined });
      try {
        const result = await deleteProjectAPI(projectID, cleanup);
        const retained = result.retained.length
          ? `，已保留修改副本：${result.retained.join("，")}`
          : "";
        set({ snapshot: result.snapshot, notice: { message: `项目关联已移除${retained}` } });
        return true;
      } catch (error) {
        set({ notice: { message: String(error), error: true } });
        return false;
      } finally {
        set({ busy: "" });
      }
    },
    updateHost: (host, paths) =>
      run(
        `paths:${host.id}`,
        () => updateHostPaths(host.id, paths.configPath, paths.skillPath, paths.agentPath),
        `${host.name} 路径已更新`,
      ),
  };
});
