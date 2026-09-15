import { type CSSProperties, FormEvent, startTransition, useDeferredValue, useEffect, useRef, useState, ViewTransition } from "react"
import {
  Activity,
  ArrowLeft,
  ArrowRight,
  Bot,
  Box,
  Check,
  ChevronRight,
  CircleAlert,
  Code2,
  Command,
  Download,
  FolderCog,
  GitBranch,
  KeyRound,
  Link2,
  MoveHorizontal,
  Network,
  Plus,
  RefreshCw,
  RotateCcw,
  Search,
  Settings,
  ShieldCheck,
  Sparkles,
  TerminalSquare,
  X,
  Zap,
} from "lucide-react"
import { Dock, DockIcon } from "@/components/ui/dock"
import {
  addAgent,
  addMCP,
  Agent,
  AgentInput,
  getSnapshot,
  Host,
  importSkill,
  installSkill,
  MCP,
  MCPInput,
  refreshSnapshot,
  restoreBackup,
  Skill,
  Snapshot,
  testMCP,
  toggleAgent,
  toggleMCP,
  toggleSkill,
  updateHostPaths,
} from "@/lib/api"
import "@/theme.css"

type View = "overview" | "skills" | "mcps" | "agents" | "settings"
type ResourceKind = "skills" | "mcps" | "agents"
type Resource = Skill | MCP | Agent

const navigation: { id: View; label: string; icon: typeof Activity }[] = [
  { id: "overview", label: "总览", icon: Activity },
  { id: "skills", label: "Skills", icon: Sparkles },
  { id: "mcps", label: "MCP 服务", icon: Network },
  { id: "agents", label: "自定义 Agent", icon: Bot },
  { id: "settings", label: "设置", icon: Settings },
]

const showcaseKinds: ResourceKind[] = ["skills", "mcps", "agents"]

const emptySnapshot: Snapshot = { root: "~/.chu", hosts: [], skills: [], mcps: [], agents: [], lastScan: "" }

function HostMark({ host, compact = false }: { host: Host; compact?: boolean }) {
  const initials = host.id === "opencode" ? "OC" : host.id === "claude" ? "CC" : "CX"
  return (
    <span className={`host-mark host-mark-${host.id} ${compact ? "host-mark-compact" : ""}`} aria-hidden="true">
      {initials}
    </span>
  )
}

function Switch({ checked, disabled, label, onChange }: { checked: boolean; disabled?: boolean; label: string; onChange: (checked: boolean) => void }) {
  return (
    <label className="switch" title={label}>
      <input type="checkbox" checked={checked} disabled={disabled} aria-label={label} onChange={(event) => onChange(event.target.checked)} />
      <span className="switch-track"><span className="switch-thumb" /></span>
    </label>
  )
}

function StatusDot({ ready }: { ready: boolean }) {
  return <span className={`status-dot ${ready ? "status-ready" : "status-offline"}`} aria-hidden="true" />
}

function ResourceIcon({ kind }: { kind: ResourceKind }) {
  if (kind === "skills") return <Sparkles />
  if (kind === "mcps") return <Network />
  return <Bot />
}

function resourceMeta(item: Resource, kind: ResourceKind) {
  if (kind === "skills") {
    const skill = item as Skill
    return skill.managed ? `${skill.version || "本地"} · ${skill.repository ? "Git" : "导入"}` : "宿主中发现"
  }
  if (kind === "mcps") {
    const mcp = item as MCP
    return mcp.type === "stdio" ? `stdio · ${mcp.command}` : `${mcp.type.toUpperCase()} · ${mcp.endpoint}`
  }
  const agent = item as Agent
  return `${agent.source === "chu" ? "Chu 创建" : "宿主导入"} · ${agent.model || "继承模型"}`
}

function ResourceList({
  kind,
  items,
  hosts,
  busy,
  selectedID,
  onSelect,
  onToggle,
  onImport,
  onTest,
}: {
  kind: ResourceKind
  items: Resource[]
  hosts: Host[]
  busy: string
  selectedID?: string
  onSelect: (item: Resource) => void
  onToggle: (item: Resource, hostID: string, enabled: boolean) => void
  onImport: (item: Resource) => void
  onTest: (item: MCP) => void
}) {
  if (items.length === 0) {
    return (
      <div className="empty-state">
        <span className="empty-icon"><ResourceIcon kind={kind} /></span>
        <strong>还没有资源</strong>
        <p>使用右上角的新建按钮添加第一个条目。</p>
      </div>
    )
  }

  return (
    <div className="resource-table">
      <div className="resource-table-head">
        <span>资源</span>
        <div className="host-columns" aria-label="Agent 宿主">
          {hosts.map((host) => <span key={host.id} title={host.name}>{host.id === "opencode" ? "OC" : host.id === "claude" ? "CC" : "CX"}</span>)}
        </div>
      </div>
      {items.map((item) => (
        <div
          key={item.id}
          className={`resource-row ${selectedID === item.id ? "resource-row-selected" : ""}`}
        >
          <button type="button" className="resource-select" aria-label={`查看 ${item.name}`} onClick={() => onSelect(item)} />
          <span className={`resource-icon resource-icon-${kind}`}><ResourceIcon kind={kind} /></span>
          <span className="resource-copy">
            <span className="resource-title-line">
              <strong>{item.name}</strong>
              {!item.managed ? <><span className="badge badge-warning">待导入</span><span className="badge badge-source">{hosts.find((host) => item.enabledOn[host.id])?.name || "未知宿主"}</span></> : null}
              {kind === "skills" && item.managed && Object.values((item as Skill).modeByHost).includes("copy") ? <span className="badge">copy</span> : null}
              {kind === "mcps" && (item as MCP).hasCredentials ? <KeyRound className="credential-icon" aria-label="包含敏感值" /> : null}
            </span>
            <span className="resource-description">{item.description || "未填写描述"}</span>
            <span className="resource-meta">{resourceMeta(item, kind)}</span>
          </span>
          {!item.managed && kind === "skills" ? (
            <button type="button" className="import-button" onClick={() => onImport(item)}><Download />导入</button>
          ) : (
            <span className="host-columns host-switches">
              {hosts.map((host) => {
                const operation = `${kind}:${item.id}:${host.id}`
                return (
                  <Switch
                    key={host.id}
                    checked={Boolean(item.enabledOn[host.id])}
                    disabled={!host.installed || busy === operation || !item.managed}
                    label={`${item.enabledOn[host.id] ? "关闭" : "部署到"} ${host.name}`}
                    onChange={(enabled) => onToggle(item, host.id, enabled)}
                  />
                )
              })}
            </span>
          )}
          {kind === "mcps" && item.managed ? (
            <button type="button" className="row-action" aria-label={`测试 ${item.name}`} title="测试配置" onClick={() => onTest(item as MCP)}><Zap /></button>
          ) : <ChevronRight className="row-chevron" aria-hidden="true" />}
        </div>
      ))}
    </div>
  )
}

function AddResourceDialog({ kind, onClose, onSubmit, returnFocus }: { kind: ResourceKind; onClose: () => void; onSubmit: (values: Record<string, string>) => void; returnFocus?: HTMLElement | null }) {
  const [mcpType, setMCPType] = useState<MCP["type"]>("stdio")
  const dialogRef = useRef<HTMLElement>(null)
  const onCloseRef = useRef(onClose)
  onCloseRef.current = onClose

  useEffect(() => {
    dialogRef.current?.querySelector<HTMLElement>("input, select, textarea")?.focus()
  }, [])

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      const dialog = dialogRef.current
      if (!dialog) return
      if (event.key === "Escape") {
        event.preventDefault()
        onCloseRef.current()
        return
      }
      if (event.key !== "Tab") return

      const focusable = Array.from(dialog.querySelectorAll<HTMLElement>("button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex='-1'])"))
      const first = focusable[0]
      const last = focusable.at(-1)
      if (!first || !last) return
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault()
        first.focus()
      }
    }

    document.addEventListener("keydown", handleKeyDown)
    return () => {
      document.removeEventListener("keydown", handleKeyDown)
      returnFocus?.focus()
    }
  }, [returnFocus])

  function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault()
    const values = Object.fromEntries(new FormData(event.currentTarget).entries()) as Record<string, string>
    onSubmit(values)
  }

  const title = kind === "skills" ? "从 Git 安装 Skill" : kind === "mcps" ? "添加 MCP 服务" : "创建自定义 Agent"
  return (
    <div className="dialog-backdrop" role="presentation" onMouseDown={onClose}>
      <section ref={dialogRef} className="dialog" role="dialog" aria-modal="true" aria-labelledby="dialog-title" onMouseDown={(event) => event.stopPropagation()}>
        <header className="dialog-header">
          <div>
            <h2 id="dialog-title">{title}</h2>
          </div>
          <button type="button" className="icon-button" aria-label="关闭" title="关闭" onClick={onClose}><X /></button>
        </header>
        <form className="dialog-form" onSubmit={submit}>
          {kind === "skills" ? (
            <>
              <label>公开 HTTPS 仓库<input name="repository" type="url" required autoFocus placeholder="https://github.com/org/skills.git" /></label>
              <div className="form-grid">
                <label>仓库子目录<input name="subdir" placeholder="skills/code-review" /></label>
                <label>本地名称<input name="name" placeholder="自动识别" /></label>
              </div>
              <p className="form-note"><GitBranch />只读安装到 <code>~/.chu/skills</code>，更新由你手动触发。</p>
            </>
          ) : null}
          {kind === "mcps" ? (
            <>
              <div className="form-grid">
                <label>名称<input name="name" required autoFocus placeholder="filesystem" /></label>
                <label>连接类型<select name="type" value={mcpType} onChange={(event) => setMCPType(event.target.value as MCP["type"])}><option value="stdio">stdio</option><option value="http">HTTP</option><option value="sse">SSE</option></select></label>
              </div>
              <label>描述<input name="description" placeholder="这个 MCP 提供什么能力" /></label>
              {mcpType === "stdio" ? <div className="form-grid"><label>可执行文件<input name="command" required placeholder="npx" /></label><label>参数（空格分隔）<input name="args" placeholder="-y @modelcontextprotocol/server-filesystem" /></label></div> : <label>服务 URL<input name="endpoint" type="url" required placeholder="https://service.example/mcp" /></label>}
              <label>Token / API Key<input name="secret" type="password" autoComplete="off" placeholder="明文写入本地配置" /></label>
            </>
          ) : null}
          {kind === "agents" ? (
            <>
              <div className="form-grid">
                <label>名称<input name="name" required autoFocus placeholder="researcher" /></label>
                <label>模型<input name="model" placeholder="inherit" /></label>
              </div>
              <label>描述<input name="description" placeholder="什么时候使用这个 Agent" /></label>
              <label>系统提示词<textarea name="prompt" required rows={8} placeholder="定义职责、边界和输出要求..." /></label>
            </>
          ) : null}
          <footer className="dialog-footer">
            <button type="button" className="button button-secondary" onClick={onClose}>取消</button>
            <button type="submit" className="button button-primary"><Plus />{kind === "skills" ? "安装" : "创建"}</button>
          </footer>
        </form>
      </section>
    </div>
  )
}

type ShowcaseItem = {
  id: ResourceKind
  label: string
  description: string
  detail: string
  count: number
  icon: typeof Sparkles
}

function FeatureCarousel({
  items,
  activeIndex,
  onActiveChange,
  onOpen,
}: {
  items: ShowcaseItem[]
  activeIndex: number
  onActiveChange: (index: number) => void
  onOpen: (kind: ResourceKind) => void
}) {
  const dragStart = useRef<number | undefined>(undefined)
  const didDrag = useRef(false)

  function move(direction: -1 | 1) {
    onActiveChange((activeIndex + direction + items.length) % items.length)
  }

  function relativeOffset(index: number) {
    let offset = index - activeIndex
    if (offset > items.length / 2) offset -= items.length
    if (offset < -items.length / 2) offset += items.length
    return offset
  }

  return (
    <section
      className="showcase"
      aria-label="功能浏览"
      aria-roledescription="carousel"
      tabIndex={0}
      onKeyDown={(event) => {
        if (event.key === "ArrowLeft") {
          event.preventDefault()
          move(-1)
        }
        if (event.key === "ArrowRight") {
          event.preventDefault()
          move(1)
        }
      }}
    >
      <div className="showcase-heading">
        <div>
          <h2>能力橱窗</h2>
          <p>横向拖动，浏览并进入你的 Agent 资源。</p>
        </div>
        <div className="carousel-actions">
          <button type="button" className="icon-button" aria-label="上一个功能" onClick={() => move(-1)}><ArrowLeft /></button>
          <button type="button" className="icon-button" aria-label="下一个功能" onClick={() => move(1)}><ArrowRight /></button>
        </div>
      </div>

      <div
        className="carousel-stage"
        onPointerDown={(event) => {
          dragStart.current = event.clientX
          didDrag.current = false
        }}
        onPointerUp={(event) => {
          if (dragStart.current !== undefined) {
            const distance = event.clientX - dragStart.current
            didDrag.current = Math.abs(distance) > 44
            if (didDrag.current) move(distance < 0 ? 1 : -1)
          }
          dragStart.current = undefined
          window.setTimeout(() => { didDrag.current = false }, 0)
        }}
        onPointerCancel={() => {
          dragStart.current = undefined
          didDrag.current = false
        }}
      >
        <div className="carousel-orbit" aria-hidden="true" />
        {items.map((item, index) => {
          const offset = relativeOffset(index)
          const Icon = item.icon
          const active = offset === 0
          const style = {
            "--carousel-x": `${offset * 76}%`,
            "--carousel-rotate": `${offset * -16}deg`,
            "--carousel-scale": active ? 1 : 0.82,
            "--carousel-depth": active ? "0px" : "-120px",
            "--carousel-opacity": active ? 1 : 0.78,
          } as CSSProperties

          return (
            <article
              key={item.id}
              className="showcase-card"
              data-active={active}
              style={style}
            >
              <button
                type="button"
                className="showcase-hit"
                aria-current={active ? "true" : undefined}
                aria-label={active ? `打开 ${item.label}` : `浏览 ${item.label}`}
                onClick={() => {
                  if (didDrag.current) return
                  if (active) onOpen(item.id)
                  else onActiveChange(index)
                }}
              />
              <span className="showcase-card-topline"><span>{item.id.toUpperCase()}</span><small>{String(index + 1).padStart(2, "0")}</small></span>
              <span className="showcase-icon"><Icon /></span>
              <span className="showcase-copy">
                <strong>{item.label}</strong>
                <span>{item.description}</span>
              </span>
              <span className="showcase-card-footer">
                <span><strong>{item.count}</strong><small>{item.detail}</small></span>
                <span className="showcase-open"><ArrowRight /></span>
              </span>
            </article>
          )
        })}
      </div>

      <div className="carousel-footer">
        <span className="drag-hint"><MoveHorizontal />拖动或使用方向键</span>
        <div className="carousel-dots" aria-label="选择功能">
          {items.map((item, index) => (
            <button
              type="button"
              key={item.id}
              aria-label={`浏览 ${item.label}`}
              aria-current={index === activeIndex ? "true" : undefined}
              onClick={() => onActiveChange(index)}
            />
          ))}
        </div>
        <strong>{String(activeIndex + 1).padStart(2, "0")} / {String(items.length).padStart(2, "0")}</strong>
      </div>
    </section>
  )
}

function App() {
  const [snapshot, setSnapshot] = useState<Snapshot>(emptySnapshot)
  const [view, setView] = useState<View>("overview")
  const [resourceKind, setResourceKind] = useState<ResourceKind>("skills")
  const [carouselIndex, setCarouselIndex] = useState(0)
  const [search, setSearch] = useState("")
  const deferredSearch = useDeferredValue(search)
  const [selected, setSelected] = useState<Resource>()
  const [busy, setBusy] = useState("")
  const [dialog, setDialog] = useState<ResourceKind>()
  const dialogTrigger = useRef<HTMLButtonElement>(null)
  const [notice, setNotice] = useState<{ message: string; error?: boolean }>()
  const [demo, setDemo] = useState(false)

  useEffect(() => {
    getSnapshot().then(({ snapshot: next, demo: preview }) => {
      setSnapshot(next)
      setDemo(preview)
      setSelected(next.skills[0])
    }).catch((error) => setNotice({ message: String(error), error: true }))
  }, [])

  const activeKind: ResourceKind = view === "skills" || view === "mcps" || view === "agents" ? view : resourceKind
  const resources: Resource[] = activeKind === "skills" ? snapshot.skills : activeKind === "mcps" ? snapshot.mcps : snapshot.agents
  const query = deferredSearch.trim().toLowerCase()
  const filtered = query ? resources.filter((item) => `${item.name} ${item.description}`.toLowerCase().includes(query)) : resources
  const inspectedResource = filtered.find((item) => item.id === selected?.id) ?? filtered[0]
  const installedHosts = snapshot.hosts.filter((host) => host.installed).length
  const activeDeployments = [...snapshot.skills, ...snapshot.mcps, ...snapshot.agents].reduce((total, item) => total + Object.values(item.enabledOn).filter(Boolean).length, 0)
  const unmanaged = [...snapshot.skills, ...snapshot.mcps, ...snapshot.agents].filter((item) => !item.managed).length
  const showcaseItems: ShowcaseItem[] = [
    { id: "skills", label: "Skills", description: "安装、导入并向宿主部署可复用能力。", detail: "个技能", count: snapshot.skills.length, icon: Sparkles },
    { id: "mcps", label: "MCP 服务", description: "集中管理工具连接与本地服务配置。", detail: "个连接", count: snapshot.mcps.length, icon: Network },
    { id: "agents", label: "自定义 Agent", description: "共享角色定义，并按宿主独立启用。", detail: "个角色", count: snapshot.agents.length, icon: Bot },
  ]

  function selectView(next: View) {
    startTransition(() => {
      setView(next)
      if (next === "skills" || next === "mcps" || next === "agents") {
        setResourceKind(next)
        setCarouselIndex(showcaseKinds.indexOf(next))
        setSelected((next === "skills" ? snapshot.skills : next === "mcps" ? snapshot.mcps : snapshot.agents)[0])
      }
    })
  }

  async function run(operation: string, action: () => Promise<Snapshot>, success: string) {
    setBusy(operation)
    setNotice(undefined)
    try {
      const next = await action()
      setSnapshot(next)
      setNotice({ message: success })
      return true
    } catch (error) {
      setNotice({ message: String(error), error: true })
      return false
    } finally {
      setBusy("")
    }
  }

  async function handleToggle(item: Resource, hostID: string, enabled: boolean) {
    const operation = `${activeKind}:${item.id}:${hostID}`
    const action = activeKind === "skills" ? () => toggleSkill(item.id, hostID, enabled) : activeKind === "mcps" ? () => toggleMCP(item.id, hostID, enabled) : () => toggleAgent(item.id, hostID, enabled)
    await run(operation, action, enabled ? "部署已写入宿主配置" : "已取消该宿主部署")
  }

  async function handleImport(item: Resource) {
    const sourceHost = Object.entries(item.enabledOn).find(([, enabled]) => enabled)?.[0]
    if (!sourceHost) return
    await run(`import:${item.id}`, () => importSkill(sourceHost, item.name), `${item.name} 已纳入 Chu 管理`)
  }

  async function handleTest(item: MCP) {
    setBusy(`test:${item.id}`)
    try {
      setNotice({ message: await testMCP(item.id) })
    } catch (error) {
      setNotice({ message: String(error), error: true })
    } finally {
      setBusy("")
    }
  }

  async function handleDialogSubmit(values: Record<string, string>) {
    if (!dialog) return
    const action = dialog === "skills"
      ? () => installSkill(values.repository, values.subdir || "", values.name || "")
      : dialog === "mcps"
        ? () => addMCP({ name: values.name, description: values.description || "", type: values.type as MCP["type"], endpoint: values.endpoint || "", command: values.command || "", args: (values.args || "").split(/\s+/).filter(Boolean), env: {}, headers: {}, secret: values.secret || "" } satisfies MCPInput)
        : () => addAgent({ name: values.name, description: values.description || "", prompt: values.prompt, model: values.model || "inherit" } satisfies AgentInput)
    if (await run(`add:${dialog}`, action, dialog === "skills" ? "Skill 安装完成" : "资源已保存到 Chu")) setDialog(undefined)
  }

  function changeShowcase(index: number) {
    setCarouselIndex(index)
    setResourceKind(showcaseKinds[index])
  }

  return (
    <>
      <a className="skip-link" href="#main-content">跳到主要内容</a>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="brand" aria-label="Chu Agent Manager">
            <span className="brand-mark"><Command /></span>
            <strong>CHU</strong>
          </div>
          <nav aria-label="主导航">
            <Dock orientation="vertical" direction="middle" className="side-dock">
              {navigation.map((item) => {
                const Icon = item.icon
                const count = item.id === "skills" ? snapshot.skills.length : item.id === "mcps" ? snapshot.mcps.length : item.id === "agents" ? snapshot.agents.length : undefined
                return (
                  <DockIcon key={item.id}>
                    <button
                      type="button"
                      className="dock-button"
                      aria-current={view === item.id ? "page" : undefined}
                      title={item.label}
                      onClick={() => selectView(item.id)}
                    >
                      <Icon />
                      {count !== undefined ? <span className="nav-count" aria-hidden="true">{count}</span> : null}
                      <span className="dock-label">{item.label}</span>
                    </button>
                  </DockIcon>
                )
              })}
            </Dock>
          </nav>
          <div className="sidebar-footer" title={`中央仓库 ${snapshot.root}`}>
            <Box />
            <span>LOCAL</span>
          </div>
        </aside>

        <main className="workspace" id="main-content">
          <header className="topbar">
            <div className="breadcrumbs"><span>Chu</span><ChevronRight /><strong>{navigation.find((item) => item.id === view)?.label}</strong></div>
            <div className="topbar-actions">
              <span className="sync-state"><StatusDot ready={installedHosts > 0} />{installedHosts} 个宿主在线</span>
              {demo ? <span className="preview-badge">预览模式</span> : null}
              <button type="button" className="icon-button" aria-label="刷新扫描" title="刷新扫描" disabled={busy === "refresh"} onClick={() => run("refresh", refreshSnapshot, "扫描已完成")}><RefreshCw className={busy === "refresh" ? "spin" : ""} /></button>
              <span className="profile" title="本地用户">LD</span>
            </div>
          </header>

          {notice ? <div className={`notice ${notice.error ? "notice-error" : ""}`} role="status">{notice.error ? <CircleAlert /> : <Check />}<span>{notice.message}</span><button type="button" aria-label="关闭提示" onClick={() => setNotice(undefined)}><X /></button></div> : null}

          <ViewTransition default="chu-page">
            <div className="page" key={view}>
              {view === "overview" ? (
                <>
                  <section className="page-heading home-heading">
                    <div><h1>Agent 控制台</h1><p>统一维护资源，按宿主独立交付。</p></div>
                    <button type="button" className="button button-primary" onClick={(event) => { dialogTrigger.current = event.currentTarget; setDialog(resourceKind) }}><Plus />添加资源</button>
                  </section>

                  <FeatureCarousel items={showcaseItems} activeIndex={carouselIndex} onActiveChange={changeShowcase} onOpen={selectView} />

                  <section className="stats-grid" aria-label="资源状态">
                    <article><span className="stat-icon stat-green"><TerminalSquare /></span><div><small>已接入宿主</small><strong>{installedHosts}<span> / {snapshot.hosts.length}</span></strong><em>自动扫描本机</em></div></article>
                    <article><span className="stat-icon stat-yellow"><Link2 /></span><div><small>活动部署</small><strong>{activeDeployments}</strong><em>link 优先</em></div></article>
                    <article><span className="stat-icon stat-blue"><FolderCog /></span><div><small>中央资源</small><strong>{snapshot.skills.filter((item) => item.managed).length + snapshot.mcps.filter((item) => item.managed).length + snapshot.agents.filter((item) => item.managed).length}</strong><em>位于 {snapshot.root}</em></div></article>
                    <article><span className="stat-icon stat-amber"><CircleAlert /></span><div><small>等待处理</small><strong>{unmanaged}</strong><em>{unmanaged ? "发现外部资源" : "没有配置冲突"}</em></div></article>
                  </section>

                  <section className="overview-lower">
                    <div className="host-band">
                      <div className="section-heading"><h2>Agent 宿主</h2><button type="button" className="text-button" onClick={() => selectView("settings")}>查看路径 <ChevronRight /></button></div>
                      <div className="host-grid">
                        {snapshot.hosts.map((host) => (
                          <article key={host.id} className="host-card">
                            <HostMark host={host} />
                            <div><strong>{host.name}</strong><span><StatusDot ready={host.installed} />{host.installed ? "已连接" : "未检测到"}</span></div>
                            <code>{host.format.toUpperCase()}</code>
                          </article>
                        ))}
                      </div>
                    </div>
                    <aside className="insight-panel">
                      <span className="insight-icon"><ShieldCheck /></span>
                      <div><h3>所有写入都有退路</h3><p>宿主配置写入前保留上一份备份。检测到外部改动时，Chu 会停止覆盖。</p></div>
                      <div className="guard-list"><span><Check />同名冲突阻断</span><span><Check />copy 改动保护</span><span><Check />敏感值默认掩码</span></div>
                    </aside>
                  </section>
                </>
              ) : view === "settings" ? (
                <SettingsView snapshot={snapshot} busy={busy} onRestore={(host) => run(`restore:${host.id}`, () => restoreBackup(host.id), `${host.name} 已恢复上次备份`)} onUpdate={(host, paths) => run(`paths:${host.id}`, () => updateHostPaths(host.id, paths.configPath, paths.skillPath, paths.agentPath), `${host.name} 路径已更新`)} />
              ) : (
                <>
                  <section className="page-heading resource-page-heading">
                    <div><h1>{view === "skills" ? "Skills" : view === "mcps" ? "MCP 服务" : "自定义 Agent"}</h1><p>{view === "skills" ? "中央保存，按宿主使用 link 或 copy 部署。" : view === "mcps" ? "保留原生配置结构，只维护 Chu 托管条目。" : "共享公共定义，并允许按宿主独立启用。"}</p></div>
                    <button type="button" className="button button-primary" onClick={(event) => { dialogTrigger.current = event.currentTarget; setDialog(activeKind) }}><Plus />{activeKind === "skills" ? "安装 Skill" : activeKind === "mcps" ? "添加 MCP" : "创建 Agent"}</button>
                  </section>
                  <div className="library-toolbar"><label className="search-field"><Search /><input aria-label="搜索资源" value={search} onChange={(event) => setSearch(event.target.value)} placeholder="搜索名称或描述" /></label><span>{filtered.length} 个条目</span></div>
                  <section className="library-layout">
                    <div className="resource-panel resource-panel-full"><ResourceList kind={activeKind} items={filtered} hosts={snapshot.hosts} busy={busy} selectedID={selected?.id} onSelect={setSelected} onToggle={handleToggle} onImport={handleImport} onTest={handleTest} /></div>
                    <ResourceInspector item={inspectedResource} kind={activeKind} hosts={snapshot.hosts} />
                  </section>
                </>
              )}
            </div>
          </ViewTransition>
        </main>
        {dialog ? <AddResourceDialog kind={dialog} onClose={() => setDialog(undefined)} onSubmit={handleDialogSubmit} returnFocus={dialogTrigger.current} /> : null}
      </div>
    </>
  )
}

function ResourceInspector({ item, kind, hosts }: { item?: Resource; kind: ResourceKind; hosts: Host[] }) {
  if (!item) return <aside className="inspector"><div className="empty-inspector"><Code2 /><span>选择一个条目查看部署详情</span></div></aside>
  return (
    <aside className="inspector">
      <header><span className={`resource-icon resource-icon-${kind}`}><ResourceIcon kind={kind} /></span><div><h2>{item.name}</h2></div></header>
      <p>{item.description || "未填写描述"}</p>
      <dl><div><dt>管理状态</dt><dd>{item.managed ? "Chu 托管" : "等待导入"}</dd></div><div><dt>类型</dt><dd>{resourceMeta(item, kind)}</dd></div></dl>
      <h3>宿主部署</h3>
      <div className="inspector-hosts">
        {hosts.map((host) => <div key={host.id}><HostMark host={host} compact /><span><strong>{host.name}</strong><small>{item.enabledOn[host.id] ? kind === "skills" ? (item as Skill).modeByHost?.[host.id] || "已启用" : "已启用" : "未启用"}</small></span><StatusDot ready={Boolean(item.enabledOn[host.id])} /></div>)}
      </div>
    </aside>
  )
}

function SettingsView({ snapshot, busy, onRestore, onUpdate }: { snapshot: Snapshot; busy: string; onRestore: (host: Host) => void; onUpdate: (host: Host, paths: Pick<Host, "configPath" | "skillPath" | "agentPath">) => void }) {
  const [drafts, setDrafts] = useState<Record<string, Pick<Host, "configPath" | "skillPath" | "agentPath">>>(() => Object.fromEntries(snapshot.hosts.map((host) => [host.id, { configPath: host.configPath, skillPath: host.skillPath, agentPath: host.agentPath }])) as Record<string, Pick<Host, "configPath" | "skillPath" | "agentPath">>)

  useEffect(() => {
    setDrafts(Object.fromEntries(snapshot.hosts.map((host) => [host.id, { configPath: host.configPath, skillPath: host.skillPath, agentPath: host.agentPath }])) as Record<string, Pick<Host, "configPath" | "skillPath" | "agentPath">>)
  }, [snapshot.hosts])

  return (
    <>
      <section className="page-heading"><div><h1>宿主与路径</h1><p>自动发现结果可作为后续手动路径配置的基础。</p></div></section>
      <section className="settings-layout">
        <div className="settings-list">
          {snapshot.hosts.map((host) => (
            <article key={host.id} className="settings-host">
              <header><HostMark host={host} /><div><h2>{host.name}</h2><span><StatusDot ready={host.installed} />{host.installed ? "已检测到安装" : "未检测到安装"}</span></div><span className="format-badge">{host.format.toUpperCase()}</span></header>
              <div className="path-fields">
                {(["configPath", "skillPath", "agentPath"] as const).map((key) => <label key={key}>{key === "configPath" ? "配置文件" : key === "skillPath" ? "Skill 目录" : "Agent 目录"}<input value={drafts[host.id]?.[key] || ""} onChange={(event) => setDrafts((current) => ({ ...current, [host.id]: { ...current[host.id], [key]: event.target.value } }))} /></label>)}
              </div>
              <footer><button type="button" className="button button-secondary" disabled={busy === `restore:${host.id}`} onClick={() => onRestore(host)}><RotateCcw />恢复上次备份</button><button type="button" className="button button-primary" disabled={busy === `paths:${host.id}`} onClick={() => onUpdate(host, drafts[host.id])}><Check />保存路径</button></footer>
            </article>
          ))}
        </div>
        <aside className="settings-aside"><h2>Chu 中央目录</h2><code>{snapshot.root}</code><p>Skill 本体、MCP 定义、自定义 Agent 和部署状态统一存放在这里。</p><div className="security-note"><KeyRound /><span><strong>本地凭据</strong>当前版本按明文写入配置文件，界面默认隐藏。</span></div></aside>
      </section>
    </>
  )
}

export default App
