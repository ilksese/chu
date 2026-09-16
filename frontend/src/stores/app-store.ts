import { create } from "zustand";
import {
  addAgent as addAgentAPI,
  addMCP as addMCPAPI,
  getSnapshot,
  importSkill as importSkillAPI,
  installSkill as installSkillAPI,
  refreshSnapshot,
  restoreBackup,
  testMCP as testMCPAPI,
  toggleAgent,
  toggleMCP,
  toggleSkill,
  updateHostPaths,
  type AgentInput,
  type Host,
  type MCPInput,
  type Snapshot,
} from "@/lib/api";
import type { Resource, ResourceKind } from "@/lib/resources";

const emptySnapshot: Snapshot = {
  root: "~/.chu",
  hosts: [],
  skills: [],
  mcps: [],
  agents: [],
  lastScan: "",
};

type Notice = { message: string; error?: boolean };
type HostPaths = Pick<Host, "configPath" | "skillPath" | "agentPath">;

type AppStore = {
  snapshot: Snapshot;
  busy: string;
  notice?: Notice;
  demo: boolean;
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
  installSkill: (repository: string, subdir: string, name: string) => Promise<boolean>;
  addMCP: (input: MCPInput) => Promise<boolean>;
  addAgent: (input: AgentInput) => Promise<boolean>;
  restoreHost: (host: Host) => Promise<boolean>;
  updateHost: (host: Host, paths: HostPaths) => Promise<boolean>;
};

export const useAppStore = create<AppStore>((set) => {
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
    busy: "",
    demo: false,

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

    installSkill: (repository, subdir, name) =>
      run("add:skills", () => installSkillAPI(repository, subdir, name), "Skill 安装完成"),
    addMCP: (input) => run("add:mcps", () => addMCPAPI(input), "资源已保存到 Chu"),
    addAgent: (input) => run("add:agents", () => addAgentAPI(input), "资源已保存到 Chu"),
    restoreHost: (host) =>
      run(`restore:${host.id}`, () => restoreBackup(host.id), `${host.name} 已恢复上次备份`),
    updateHost: (host, paths) =>
      run(
        `paths:${host.id}`,
        () => updateHostPaths(host.id, paths.configPath, paths.skillPath, paths.agentPath),
        `${host.name} 路径已更新`,
      ),
  };
});
