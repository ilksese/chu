# Chu

本地 AI 编程 Agent 资源管理桌面应用：集中管理 **Skills、MCP 服务、自定义 Agent**，并部署到 OpenCode、Claude Code、Codex 三个宿主。

- 桌面壳：[Wails v2](https://wails.io)（Go + WebView2）
- 前端：React + TypeScript + Vite + Tailwind CSS + [Magic UI](https://magicui.design) + Zustand
- 包管理：pnpm
- 界面：浅色主题，neo-brutalist 风格，视觉规范见 `DESIGN.md`

## 功能

| 功能 | 说明 |
| --- | --- |
| Skill 安装 | 从公开 HTTPS Git 仓库克隆（需包含 `SKILL.md`），存入中央目录 |
| Skill 导入 | 把宿主目录中已有的 skill 收编为中央管理（优先 link，回退 copy） |
| Skill 部署 | 按宿主启用/禁用，link 或 copy；目标被外部修改时拒绝误删 |
| MCP 管理 | 添加 stdio / http / sse 三类 MCP，支持命令、参数、环境变量、请求头、token |
| MCP 部署 | 直接写入宿主原生配置（`opencode.json` / `settings.json` / `config.toml`），写入前自动备份 |
| MCP 测试 | stdio 检查命令存在性；http/sse 发请求验证连通性 |
| Agent 创建 | 编写自定义 Agent（markdown + frontmatter），部署到各宿主 |
| 备份恢复 | 宿主配置被 Chu 修改后自动生成 `.chu.bak`，可一键恢复 |
| 宿主路径 | 支持修改各宿主的配置 / skills / agents 路径 |

## 构建

依赖：Go 1.25+、Node 22.22+、pnpm；`wails build` / `wails dev` 需要与 `go.mod` 一致的 Wails CLI v2.16.0（`go install github.com/wailsapp/wails/v2/cmd/wails@v2.16.0`）。

### 开发

```bash
pm2 start 'wails dev' --name 'dev:chu' --cwd "$PWD"
```

### 构建

```bash
wails build
```

产物在 `build/bin/chu(.exe)`。

不走 wails CLI 也可以（前端产物通过 `go:embed` 打包）：

```bash
cd frontend
pnpm install
pnpm run build
cd ..
go build -o build/chu.exe .   # Windows；其他平台去掉 .exe
```

## 数据位置

运行时数据在 `~/.chu/`：

- `state.json` — 资源清单与部署状态
- `skills/` — 中央 skill 目录
- `agents/` — 自定义 Agent
- `sources/` — Git 克隆缓存

各宿主默认路径（可在应用内修改）：

| 宿主 | 配置 | skills / agents |
| --- | --- | --- |
| OpenCode | `~/.config/opencode/opencode.json`（Windows 下为 `%APPDATA%`） | `~/.config/opencode/skills`、`agents` |
| Claude Code | `~/.claude/settings.json` | `~/.claude/skills`、`agents` |
| Codex | `~/.codex/config.toml` | `~/.codex/skills`、`agents` |

## 仓库结构

```
main.go                    Wails 入口，embed 前端产物
app.go                     后端：资源存储、部署、宿主配置读写
frontend/src/App.tsx       前端初始化与路由入口
frontend/src/pages/        页面及页面局部状态
frontend/src/components/   跨页面 UI 和功能组件
frontend/src/stores/       Zustand 全局状态与后端操作
frontend/src/lib/api.ts    Wails API 边界与浏览器模拟数据
DESIGN.md                  视觉规范
PRODUCT.md                 产品定义
```

前端只把 Snapshot、全局操作状态、通知和后端动作放入 Zustand。搜索词、轮播位置、当前选中项和表单草稿保留在所属页面，避免无关页面订阅临时 UI 状态。直接运行 Vite 时 `window.go.main.App` 不存在，`api.ts` 会使用内存模拟数据；真实配置写入必须在 Wails 环境验证。
