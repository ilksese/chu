import type { Agent, MCP, Skill, Snapshot } from "@/lib/api";

export type ResourceKind = "skills" | "mcps" | "agents";
export type Resource = Skill | MCP | Agent;

export const resourceKinds: ResourceKind[] = ["skills", "mcps", "agents"];

export function resourcesFor(snapshot: Snapshot, kind: ResourceKind): Resource[] {
  if (kind === "skills") return snapshot.skills;
  if (kind === "mcps") return snapshot.mcps;
  return snapshot.agents;
}

export function resourceMeta(item: Resource, kind: ResourceKind) {
  if (kind === "skills") {
    const skill = item as Skill;
    return skill.managed
      ? `${skill.version || "本地"} · ${skill.repository ? "Git" : "导入"}`
      : "宿主中发现";
  }
  if (kind === "mcps") {
    const mcp = item as MCP;
    return mcp.type === "stdio"
      ? `stdio · ${mcp.command}`
      : `${mcp.type.toUpperCase()} · ${mcp.endpoint}`;
  }
  const agent = item as Agent;
  return `${agent.source === "chu" ? "Chu 创建" : "宿主导入"} · ${agent.model || "继承模型"}`;
}
