export type Host = {
  id: string
  name: string
  description: string
  installed: boolean
  status: string
  configPath: string
  skillPath: string
  agentPath: string
  format: string
}

export type Skill = {
  id: string
  name: string
  description: string
  tracked: boolean
  source: string
  managed: boolean
  enabledOn: Record<string, boolean>
  modeByHost: Record<string, string>
}

export type SkillCandidate = {
  name: string
  description: string
  path: string
  installed: boolean
}

export type SkillUpdate = {
  id: string
  name: string
  status: "update" | "deleted" | "ambiguous"
}

export type MCP = {
  id: string
  name: string
  description: string
  type: "stdio" | "http" | "sse"
  endpoint: string
  command: string
  hasCredentials: boolean
  managed: boolean
  enabledOn: Record<string, boolean>
}

export type Agent = {
  id: string
  name: string
  description: string
  model: string
  source: string
  managed: boolean
  enabledOn: Record<string, boolean>
}

export type Snapshot = {
  root: string
  hosts: Host[]
  skills: Skill[]
  mcps: MCP[]
  agents: Agent[]
  lastScan: string
}

export type MCPInput = {
  name: string
  description: string
  type: MCP["type"]
  endpoint: string
  command: string
  args: string[]
  env: Record<string, string>
  headers: Record<string, string>
  secret: string
}

export type AgentInput = {
  name: string
  description: string
  prompt: string
  model: string
}

type AppAPI = {
  GetSnapshot: () => Promise<Snapshot>
  Refresh: () => Promise<Snapshot>
  PreviewSkills: (repository: string) => Promise<SkillCandidate[]>
  InstallSkills: (repository: string, paths: string[]) => Promise<Snapshot>
  CheckSkillUpdates: () => Promise<SkillUpdate[]>
  UpdateSkill: (skillID: string) => Promise<Snapshot>
  RemoveSkill: (skillID: string) => Promise<Snapshot>
  ImportSkill: (hostID: string, name: string) => Promise<Snapshot>
  ToggleSkill: (skillID: string, hostID: string, enabled: boolean) => Promise<Snapshot>
  AddMCP: (input: MCPInput) => Promise<Snapshot>
  ToggleMCP: (mcpID: string, hostID: string, enabled: boolean) => Promise<Snapshot>
  TestMCP: (mcpID: string) => Promise<string>
  AddAgent: (input: AgentInput) => Promise<Snapshot>
  ToggleAgent: (agentID: string, hostID: string, enabled: boolean) => Promise<Snapshot>
  RestoreBackup: (hostID: string) => Promise<Snapshot>
  UpdateHostPaths: (hostID: string, configPath: string, skillPath: string, agentPath: string) => Promise<Snapshot>
}

declare global {
  interface Window {
    go?: { main?: { App?: AppAPI } }
  }
}

let demoSnapshot: Snapshot = {
  root: "~/.chu",
  lastScan: new Date().toISOString(),
  hosts: [
    { id: "opencode", name: "OpenCode", description: "本地开发 Agent", installed: true, status: "ready", configPath: "~/.config/opencode/opencode.json", skillPath: "~/.config/opencode/skills", agentPath: "~/.config/opencode/agents", format: "json" },
    { id: "claude", name: "Claude Code", description: "Anthropic coding Agent", installed: true, status: "ready", configPath: "~/.claude/settings.json", skillPath: "~/.claude/skills", agentPath: "~/.claude/agents", format: "json" },
    { id: "codex", name: "Codex", description: "OpenAI coding Agent", installed: false, status: "not-found", configPath: "~/.codex/config.toml", skillPath: "~/.codex/skills", agentPath: "~/.codex/agents", format: "toml" },
  ],
  skills: [
    { id: "code-review", name: "code-review", description: "聚焦风险、回归与测试缺口的代码审查", tracked: true, source: "~/.chu/skills/code-review", managed: true, enabledOn: { opencode: true, claude: true, codex: false }, modeByHost: { opencode: "link", claude: "link" } },
    { id: "release-notes", name: "release-notes", description: "从提交历史生成可发布的变更说明", tracked: true, source: "~/.chu/skills/release-notes", managed: true, enabledOn: { opencode: true, claude: false, codex: false }, modeByHost: { opencode: "copy" } },
    { id: "existing-skill", name: "frontend-audit", description: "在 Claude Code 中发现，尚未纳入 Chu", tracked: false, source: "~/.claude/skills/frontend-audit", managed: false, enabledOn: { claude: true }, modeByHost: { claude: "external" } },
  ],
  mcps: [
    { id: "filesystem", name: "filesystem", description: "受控访问本地项目文件", type: "stdio", endpoint: "", command: "npx", hasCredentials: false, managed: true, enabledOn: { opencode: true, claude: true, codex: false } },
    { id: "linear", name: "linear", description: "同步项目事项和交付状态", type: "http", endpoint: "https://mcp.linear.app/mcp", command: "", hasCredentials: true, managed: true, enabledOn: { opencode: false, claude: true, codex: false } },
  ],
  agents: [
    { id: "researcher", name: "researcher", description: "先调研依赖与调用路径，再输出实现建议", model: "inherit", source: "chu", managed: true, enabledOn: { opencode: true, claude: true, codex: false } },
    { id: "release-manager", name: "release-manager", description: "负责发布检查、版本说明和回滚提示", model: "inherit", source: "chu", managed: true, enabledOn: { opencode: true, claude: false, codex: false } },
  ],
}

function api() {
  return window.go?.main?.App
}

function withDemoToggle<T extends Skill | MCP | Agent>(items: T[], id: string, hostID: string, enabled: boolean): T[] {
  return items.map((item) => item.id === id ? { ...item, enabledOn: { ...item.enabledOn, [hostID]: enabled } } : item)
}

export async function getSnapshot() {
  const backend = api()
  return { snapshot: backend ? await backend.GetSnapshot() : demoSnapshot, demo: !backend }
}

export async function refreshSnapshot() {
  const backend = api()
  demoSnapshot = { ...demoSnapshot, lastScan: new Date().toISOString() }
  return backend ? backend.Refresh() : demoSnapshot
}

export async function previewSkills(repository: string): Promise<SkillCandidate[]> {
  const backend = api()
  if (backend) return backend.PreviewSkills(repository)
  return [
    { name: "grill-me", description: "A relentless interview to sharpen a plan or design.", path: "skills/productivity/grill-me", installed: false },
    { name: "handoff", description: "Compact the current conversation into a handoff document.", path: "skills/productivity/handoff", installed: false },
  ]
}

export async function installSkills(repository: string, paths: string[]) {
  const backend = api()
  if (backend) return backend.InstallSkills(repository, paths)
  const added = paths.map((path) => {
    const name = path.split("/").at(-1) || "new-skill"
    return { id: name, name, description: "新安装的 Git skill", tracked: true, source: `~/.chu/skills/${name}`, managed: true, enabledOn: {}, modeByHost: {} }
  })
  demoSnapshot = { ...demoSnapshot, skills: [...demoSnapshot.skills, ...added] }
  return demoSnapshot
}

export async function checkSkillUpdates(): Promise<SkillUpdate[]> {
  const backend = api()
  return backend ? backend.CheckSkillUpdates() : []
}

export async function updateSkill(skillID: string) {
  const backend = api()
  return backend ? backend.UpdateSkill(skillID) : demoSnapshot
}

export async function removeSkill(skillID: string) {
  const backend = api()
  if (backend) return backend.RemoveSkill(skillID)
  demoSnapshot = { ...demoSnapshot, skills: demoSnapshot.skills.filter((item) => item.id !== skillID) }
  return demoSnapshot
}

export async function importSkill(hostID: string, name: string) {
  const backend = api()
  if (backend) return backend.ImportSkill(hostID, name)
  demoSnapshot = { ...demoSnapshot, skills: demoSnapshot.skills.map((item) => item.name === name ? { ...item, id: item.name, managed: true, tracked: false, source: `~/.chu/skills/${item.name}`, modeByHost: { [hostID]: "link" } } : item) }
  return demoSnapshot
}

export async function toggleSkill(id: string, hostID: string, enabled: boolean) {
  const backend = api()
  if (backend) return backend.ToggleSkill(id, hostID, enabled)
  demoSnapshot = { ...demoSnapshot, skills: withDemoToggle(demoSnapshot.skills, id, hostID, enabled) }
  return demoSnapshot
}

export async function addMCP(input: MCPInput) {
  const backend = api()
  if (backend) return backend.AddMCP(input)
  demoSnapshot = { ...demoSnapshot, mcps: [...demoSnapshot.mcps, { id: input.name.toLowerCase().replace(/\s+/g, "-"), name: input.name, description: input.description, type: input.type, endpoint: input.endpoint, command: input.command, hasCredentials: Boolean(input.secret || Object.keys(input.env).length || Object.keys(input.headers).length), managed: true, enabledOn: {} }] }
  return demoSnapshot
}

export async function toggleMCP(id: string, hostID: string, enabled: boolean) {
  const backend = api()
  if (backend) return backend.ToggleMCP(id, hostID, enabled)
  demoSnapshot = { ...demoSnapshot, mcps: withDemoToggle(demoSnapshot.mcps, id, hostID, enabled) }
  return demoSnapshot
}

export async function testMCP(id: string) {
  const backend = api()
  return backend ? backend.TestMCP(id) : "预览模式：配置格式检查通过。"
}

export async function addAgent(input: AgentInput) {
  const backend = api()
  if (backend) return backend.AddAgent(input)
  demoSnapshot = { ...demoSnapshot, agents: [...demoSnapshot.agents, { id: input.name.toLowerCase().replace(/\s+/g, "-"), name: input.name, description: input.description, model: input.model || "inherit", source: "chu", managed: true, enabledOn: {} }] }
  return demoSnapshot
}

export async function toggleAgent(id: string, hostID: string, enabled: boolean) {
  const backend = api()
  if (backend) return backend.ToggleAgent(id, hostID, enabled)
  demoSnapshot = { ...demoSnapshot, agents: withDemoToggle(demoSnapshot.agents, id, hostID, enabled) }
  return demoSnapshot
}

export async function restoreBackup(hostID: string) {
  const backend = api()
  return backend ? backend.RestoreBackup(hostID) : demoSnapshot
}

export async function updateHostPaths(hostID: string, configPath: string, skillPath: string, agentPath: string) {
  const backend = api()
  if (backend) return backend.UpdateHostPaths(hostID, configPath, skillPath, agentPath)
  demoSnapshot = { ...demoSnapshot, hosts: demoSnapshot.hosts.map((host) => host.id === hostID ? { ...host, configPath, skillPath, agentPath } : host) }
  return demoSnapshot
}
