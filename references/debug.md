# Chu 桌面调试

调试通道默认关闭。只有构建时设置 `VITE_CHU_DEBUG_WS`，页面才会连接本地调试服务。正式包不要设置这个变量。

窗口合成层截图不能代表页面内容。Linux/X11 上 WebKit 可能把窗口画成纯黑，DOM 仍然正常。查看界面用 `screenshot`，不要用 `xwd` 或系统窗口截图判断页面是否渲染。

## 启动

```bash
pm2 start 'go run ./cmd/chu-debug' --name 'dev:chu-debug' --cwd "$PWD"

export VITE_CHU_DEBUG_WS=ws://127.0.0.1:17321/ws
wails build -debug -platform linux/amd64 -tags webkit2_41 -m -o chu-dev
pm2 start './build/bin/chu-dev' --name 'dev:chu-desktop' --cwd "$PWD"
```

Debian 13 构建必须带 `-tags webkit2_41`。不要删除 `main.go` 里的 WebKit DMA-BUF 关闭项，也不要恢复页面级 `ViewTransition`。

调试服务监听 `127.0.0.1:17321`。页面连上后日志会出现 `debug page connected`。

## 命令

CLI 在 `cmd/chu-debug-cli`：

```bash
go run ./cmd/chu-debug-cli snapshot
go run ./cmd/chu-debug-cli click e15
go run ./cmd/chu-debug-cli --raw eval "return document.title"
```

`--raw` 只输出 JSON。`--filename` 把截图存到文件。

交互优先用 snapshot 里的 ref，例如 `e15`。ref 在下一次 `snapshot` 后失效。也可以直接传 CSS 选择器。

### Core

```bash
chu-debug snapshot
chu-debug snapshot ".page"
chu-debug find 设置
chu-debug click e15
chu-debug dblclick e7
chu-debug hover e4
chu-debug fill e5 "/tmp/example"
chu-debug type "hello"
chu-debug press Enter
chu-debug check e12
chu-debug uncheck e12
chu-debug select e9 "option-value"
chu-debug text e5
chu-debug html e5
chu-debug attr e5 data-kind
chu-debug box e5
chu-debug eval "return location.pathname"
```

`click`、`fill`、`press`、`type`、`check`、`select` 和 `goto` 执行后返回新的 snapshot。

### Navigation

```bash
chu-debug goto /settings
chu-debug goto /skills
chu-debug back
chu-debug forward
chu-debug reload
```

路由：`/`、`/skills`、`/mcps`、`/agents`、`/settings`。`goto` 使用 `history.pushState`，不刷新页面。

### Keyboard

```bash
chu-debug press Enter
chu-debug press Escape
chu-debug press ArrowDown
chu-debug type "搜索词"
```

`type` 需要先有焦点输入框。`press` 发到 `document.activeElement`。

### Mouse

```bash
chu-debug scroll 400
chu-debug mousemove 150 300
chu-debug hover e4
```

这些是 DOM 事件，不移动系统鼠标。窗口被合成层遮黑时仍然有效。

### Screenshot

```bash
chu-debug screenshot
chu-debug screenshot .page
chu-debug screenshot e5 --filename=settings.png
```

截图使用 snapdom 从 DOM 出图，宽度最多 1360，不依赖窗口是否可见。默认保存为 `chu-debug.png`。

### Storage

```bash
chu-debug localstorage-list
chu-debug localstorage-get theme
chu-debug localstorage-set theme dark
chu-debug localstorage-delete theme
chu-debug localstorage-clear

chu-debug sessionstorage-list
chu-debug sessionstorage-get step
chu-debug sessionstorage-set step 3
chu-debug sessionstorage-delete step
chu-debug sessionstorage-clear
```

### DevTools

```bash
chu-debug console
chu-debug eval "return document.querySelectorAll('button').length"
```

`console` 返回页面桥启动后捕获的 `console.debug/info/warn/error`，以及 `window.error` 和未处理的 Promise 拒绝。

## 抓手

snapshot 只列出可见的标题、按钮、链接、输入框、文本域、选择框和 `role="button"`。隐藏或零尺寸元素不会获得 ref。

```text
url: wails://wails/settings
title: Chu Agent Manager
viewport: 1360x860
e1 [heading] 宿主与路径 box=128,96 294x43
e8 [button] 设置 box=26,518 42x42
e12 [textbox] 配置文件 value=/home/ryuucode/.config/opencode/opencode.json box=260,283 714x40
```

| 抓手 | 用法 | 何时用 |
| --- | --- | --- |
| ref | `click e12` | 默认。来自最近一次 snapshot |
| CSS | `click "button[title=设置]"` | ref 不稳定，或要命中 snapshot 没列出的元素 |
| 文本 | `find 保存路径` | 不知道 ref，先搜索名称、角色和输入值 |
| 坐标 | `box e12` | 检查是否溢出、重叠或滚出视口 |
| 脚本 | `eval "return ..."` | 上面的抓手不够时读取计算样式或状态 |

ref 格式是 `e` 加数字。选择器只要包含空格、`#`、`.`、`[` 或 `>`，就不会被当成 ref。

## 页面能力

| 能力 | 命令 | 说明 |
| --- | --- | --- |
| 观察 | `snapshot` `find` `text` `html` `attr` `box` | 读取当前 DOM，不改状态 |
| 操作 | `click` `dblclick` `hover` `fill` `type` `press` `check` `uncheck` `select` | 通过 DOM 事件驱动界面 |
| 导航 | `goto` `back` `forward` `reload` | 只切本应用路由 |
| 取证 | `screenshot` `console` | 视觉和运行时错误 |
| 状态 | `localstorage-*` `sessionstorage-*` | 读写页面存储 |

## 协议

CLI 最终向 `POST /command` 发送：

```json
{"id":"click-1","op":"click","ref":"e15"}
```

响应：

```json
{"id":"click-1","ok":true,"data":{},"error":""}
```

没有页面时 `error` 为 `no page connected`。ref 过期时页面返回 `stale ref`，先重新 `snapshot`。超时为 8 秒。

## 增加能力

1. 在 `frontend/src/lib/debug-bridge.ts` 的 `runDebug` 增加一个 `op`。
2. 需要新参数时扩展 `DebugRequest`。
3. 在 `cmd/chu-debug-cli/main.go` 的 `commandPayload` 把命令参数映射到该 `op`。
4. 返回可 JSON 序列化的数据。截图继续返回 data URL，由 CLI 落盘。
5. 把命令补进本文件对应分类。

调试服务只转发 JSON，一般不用改。不要把调试连接放进未设置 `VITE_CHU_DEBUG_WS` 的正式渲染路径。
