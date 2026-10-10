# Chu

**Rust + Slint 原生桌面 AI Agent 资源管理器。** 集中管理 Skills、MCP、自定义 Agent、提示词、References、Providers 和项目部署，支持 OpenCode、Claude Code、Codex、jcode 四个宿主。

- 原生窗口：Slint 1.18.1、Winit、软件渲染；界面与业务均由 Rust/Slint 实现。
- 桌面尺寸：默认 `1360×860`，最小 `1080×680`，不做移动端适配。
- 浅色 neo-brutalist 风格，视觉规范见 `DESIGN.md`；语义颜色只定义在 `ui/theme.slint`。
- 应用图标位于 `assets/`，保留既有原图，窗口图像在编译时内嵌。
- 兼容已有中央状态、锁文件、宿主配置、部署记录、备份和凭证格式。

## 功能

| 页面 | 能力 |
| --- | --- |
| 总览 | 可点击资源统计、真实待处理事项、宿主发现及定向配置、快捷入口、中央资源库位置 |
| 项目管理 | 目录选择、添加、重命名、重新定位；Skill 副本部署；修改/缺失/冲突识别、显式重置和保护性清理 |
| Skills | 默认只列 Chu 管理的数据；多选导入宿主技能并立即应用；Git 来源安装、追踪、检查/应用更新、部署和删除 |
| MCP 服务 | managed 资源导入/创建/编辑/删除、stdio/http/sse 类型化表单、安全凭证 patch；编辑同步全部启用宿主，漂移拒绝、失败回滚；JSON/TOML 及分级连接检查 |
| Providers | 创建/编辑/删除、独立凭证变量、模型目录获取与刷新；失败保留缓存，禁止重定向转发凭证 |
| 自定义 Agent | 创建/编辑/删除、稳定 ID、模型/提示词及 Markdown/frontmatter 分发；同步启用宿主并保护中央文件及副本的外部修改 |
| 提示词 | CRUD、重命名、外部文件发现、按宿主切换、原始备份恢复及干净副本同步 |
| References | Markdown 编辑、重命名、外部发现、删除和路径保护 |
| 设置 | 宿主路径、显示/隐藏偏好、配置备份恢复 |

私有权限与原子替换保护文件写入；配置修改前创建备份。已修改的宿主/项目副本不会被静默覆盖或删除。文件和网络操作在独立串行工作线程执行。

资源列表支持整行选择与方向键预览，页面查询/选择/滚动只保留在当前会话。Markdown 详情提供原生预览、源码和复制，不执行 HTML 或加载远程图片；超过 48 层嵌套的文档完整降级为纯文本。原文读取失败禁止更新；脏草稿退出需确认。Git 网络阶段共享 120 秒上限，准备阶段可安全取消，提交/回滚阶段不可中断。任务期间仍可浏览只读资源。正常关闭窗口会安全取消准备工作或等待提交/回滚结束，再自动退出并回收线程与自有预览快照。

GUI 安装使用绑定来源与 commit 的预览，不将旧候选与新仓库输入混用；旧 JSON 自动化命令继续兼容。MCP/Agent 重命名保持资源 ID，编辑和删除先检查全部目标，任何漂移/冲突整项拒绝，运行时写入失败回滚。MCP 现有秘密不回填，保持/替换/移除均需显式操作，不变更现有存储格式。


## 开发与验证

使用 **Rust 1.92+ / Cargo**。Git 用于 Skill 来源仓库；目标平台还需相应 SDK、链接器和原生开发库。应用构建、测试和验收数据生成只使用原生工具链。

```bash
cargo run --locked
./scripts/verify-rust.sh
```

验证脚本执行：

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --all-targets --locked
cargo build --locked
```

可用 `cargo test --locked --test imports` 等命令执行定向回归。`tests/` 验证真实资源操作、数据兼容、鼠标/键盘交互、主题和打包；Rust example 自带隔离数据生成测试。

## 原生构建与打包

```bash
./scripts/build.sh
```

产物：`build/bin/chu`；macOS 同时生成未签名的 `build/bin/Chu.app`，Windows 为 `build/bin/chu.exe`。macOS 打包包含完整标准/Retina 尺寸 `.icns` 和图标元数据，Windows 目标嵌入 `assets/windows/app.ico`。

```bash
./scripts/build-macos-arm.sh
./scripts/build-macos-intel.sh
./scripts/build-macos-universal.sh
./scripts/build-linux.sh
./scripts/build-windows.sh
```

跨编译需要目标 Rust toolchain、平台 SDK 和链接器；脚本失败即停止。Linux 需要系统字体、Fontconfig、X11/Wayland 与 xkbcommon 等原生依赖；中文显示需可用的 CJK 字体。分发签名、公证等发布工作不属于本地构建。

## 隔离验收与截图

**写路径测试禁止使用真实用户配置。** 通过 Rust example 创建新的验收目录：

```bash
cargo run --locked --example create_fixture -- /tmp/chu-native-qa
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home
```

输出目录必须不存在；省略目录参数时生成可保留的临时数据，并打印 `base/root/home` JSON。数据生成器通过真实应用命令操作资源，Provider 使用本地 HTTP 服务，不请求真实供应商。

```bash
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home \
  --request '{"method":"Refresh","args":[]}'
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home \
  --page 2 --screenshot /tmp/chu-native-qa/skills.png
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home \
  --page 3 --import-dialog --screenshot /tmp/chu-native-qa/import-mcp.png
```

`--root` 与 `--home` 必须成对提供。页面编号：0 总览、1 项目、2 Skills、3 MCP、4 Providers、5 Agents、6 提示词、7 References、8 设置。截图来自真实原生窗口，尺寸受系统缩放与窗口限制影响；详见 `references/debug.md`。

## 导入并立即应用

Skills/MCP 页面点击“导入”，在弹窗中多选来源资源，再勾选底部的目标宿主。点击“导入并应用”完成中央入库与部署；未勾选宿主时只入库，不移除原宿主数据。

- 已有同名资源不能重复导入；不同内容、标识冲突、来源变化会阻止整批提交。筛选不丢隐藏项选择，重扫只保留身份/指纹未变且仍有效的勾选；已选来源相互同名时立即禁提交，不自动切换来源。
- MCP 候选列表不展示凭证或敏感参数；jcode 不接受 HTTP/SSE，选择远程服务时会取消其应用勾选并说明原因，恢复兼容后仍需重新勾选。
- 写入失败回滚中央副本、目标部署及配置备份。
- 先只导入后，仍可在详情中接管内容一致的原宿主条目；不同内容继续保护。

## 数据与代码结构

中央目录仍是 `~/.chu/`：`state.json`、`chu-lock.json`、资源目录、Git 缓存和 `preferences.json`。Provider 密钥不进入 Snapshot。宿主路径遵循 `XDG_CONFIG_HOME`、Windows `APPDATA` 和 `JCODE_HOME`；Codex 项目技能使用 `.agents/skills`，jcode 提示词使用 `prompt-overlay.md`。

```text
Cargo.toml / Cargo.lock    原生依赖与固定版本
build.rs                  Slint 编译和 Windows 图标资源
src/                      后端、工作线程、视图、CLI
ui/                       原生界面和唯一主题 theme.slint
assets/                   应用图标及可编辑源图
examples/create_fixture.rs 隔离验收数据生成器
tests/                    原生业务、交互、布局、兼容性测试
scripts/                  构建、打包、验证
references/               架构、代码规范、调试和验收记录
```

架构见 `references/rust-architecture.md`，验收记录见 `references/rust-acceptance.md`。
