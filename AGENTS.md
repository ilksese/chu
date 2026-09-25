# Chu Agent Notes

## 沟通与界面

- 使用中文回复。
- Chu 是最小窗口 `1080x680` 的桌面 GUI，不做移动端适配。
- UI 优先复用 `frontend/src/components/ui` 中的 Magic UI 组件，并遵循 `DESIGN.md` 与 `frontend/src/theme.css`；黄色只作强调色，红色只用于危险/错误状态。

## 架构边界

- `main.go` 是 Wails 入口并嵌入 `frontend/dist`；`app.go` 集中负责状态、宿主发现和配置写入，公开的 `App` 方法是前后端绑定边界。
- `frontend/src/lib/api.ts` 直接调用 `window.go.main.App`；普通 Vite 浏览器中不存在该对象，因此会自动使用内存模拟数据，不能据此验证 Go 后端或文件写入。
- `frontend/src/main.tsx` 只挂载 `BrowserRouter`；`frontend/src/App.tsx` 负责初始化和路由，页面在 `frontend/src/pages`，跨页面组件在 `frontend/src/components`，路径与导航配置在 `frontend/src/app/navigation.ts`。
- `frontend/src/stores/app-store.ts` 用 Zustand 管理 Snapshot、全局操作状态、通知和后端动作；搜索、选中项、弹窗和表单草稿等页面瞬时状态留在所属页面。
- 真实后端以 `~/.chu/state.json` 为中央状态，并可能修改 OpenCode、Claude Code、Codex 的真实配置；写路径测试必须使用 `t.TempDir()`，不要让测试调用用户目录。

## 代码组织

- 手写源代码文件接近 400 行时应评估职责边界，原则上控制在 500 行以内。
- 行数不是硬性拆分指标；只在页面、职责或复用边界清晰且有长期维护收益时拆分，禁止为满足行数机械拆文件。
- 生成文件、依赖锁文件和确需维持级联顺序的集中样式文件不受上述行数目标约束。

## 工具链陷阱

- 使用 Go 1.25+、pnpm 和 Node `>=22.22.0`；这是 React Router 8 的最低引擎要求。
- Wails CLI 必须与 `go.mod` 的 `github.com/wailsapp/wails/v2 v2.16.0` 一致。安装命令：`go install github.com/wailsapp/wails/v2/cmd/wails@v2.16.0`。
- `wails dev` 会把 `go.mod` 同步到 CLI 自身版本并执行 `go mod tidy`；不要用旧 CLI，否则会降级 Wails 并改写 `go.sum`。
- `frontend/wailsjs/**` 和 `frontend/package.json.md5` 是已跟踪的生成文件，不要手改；Go 绑定变更后通过 `wails dev` 或 `wails build` 重新生成并审查差异。

## 常用命令

- 完整桌面开发：`pm2 start 'wails dev' --name 'dev:chu' --cwd "$PWD"`。
- 仅浏览器 UI：`pm2 start 'pnpm dev -- --host 0.0.0.0' --name 'dev:chu-web' --cwd "$PWD/frontend"`；此模式只有模拟 API。
- 后端验证：`go test ./... && go vet ./...`；单测：`go test . -run '^TestName$'`。
- 前端验证：`pnpm --dir frontend lint && pnpm --dir frontend build`；当前没有前端测试套件。
- 桌面集成/打包：`wails build`，产物位于忽略目录 `build/bin/`。直接 `go build` 前必须先运行 `pnpm --dir frontend build`，否则嵌入的是旧前端产物。
- `scripts/test-windows-build.sh` 测试的是“把仓库当 Wails 模板生成新项目”，不是本应用的常规测试。

## 桌面调试

- 调试通道默认关闭，只在设置 `VITE_CHU_DEBUG_WS` 后打进页面。命令和抓手见 `references/debug.md`。
- Linux/X11 不要恢复 WebKit DMA-BUF 合成或页面级 `ViewTransition`，否则会黑屏或切页崩溃。Debian 13 构建必须带 `-tags webkit2_41`。
