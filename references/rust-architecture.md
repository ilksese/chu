# Chu 原生架构

## 运行链

`src/main.rs` → `App::system` / `App::open` → `Controller::attach` → 原生 Slint 窗口。

- `App::execute(method, args)` 是 UI 和 JSON 自动化的共享边界，后端业务独立于 Slint。
- `worker.rs` 的唯一后台工作线程独占 `App`；Controller 通过通道发命令，定时器在界面线程接收带任务/来源/弹窗身份的结果。
- `operation.rs` 维护独立取消门闩与阶段；Git 网络阶段共享 120 秒期限，提交前原子裁决取消，Commit/回滚不可中断。任务不锁住只读导航，写命令仍串行。
- `worker_handle.rs` 持有唯一请求发送端和 JoinHandle；原生关闭请求进入 `shutdown.rs`，准备任务取消、提交/回滚等待，worker 释放 App 后自动退出。事件循环提前返回也先 join，不留 detached worker。
- App 强持有预览 Store，同规范 root/home 的 App 共享能力，全局索引仅 Weak；最后 owner 释放自有 TempDir，不扫描删除其他缓存。回复接收端断开时显式清当前预览。
- `ViewState` 与 `session.rs` 保存会话内页面查询、记忆选择、滚动、阅读模式和正文加载状态；中央状态不承担页面草稿。
- `dialog_lifecycle.rs` 管加载/dirty/放弃确认；读取失败不能保存，迟到回复不能回填另一个弹窗。显式关闭清空秘密草稿。
- Snapshot 与安全编辑 DTO 不回填 MCP 秘密；凭证不进入候选列表、Provider 快照或错误详情。

## 原生界面与资源

- `ui/app-window.slint` 组合导航与页面；共享列表、详情、部署矩阵、阅读、MCP 字段、导入和设置按职责拆成组件。
- `markdown_view.rs` 使用标准解析器生成原生块，白名单内联使用 Slint StyledText；建树前限制 48 层嵌套，超限整篇纯文本降级，析构也保持有界。HTML 不执行、远程图片不加载，源码始终可回退及复制。
- `mcp_input.rs` 对高级字段严格类型检查，敏感 DTO 解码只保留固定诊断，不携带 serde 原错误链；创建和编辑共用 Patch 动作求值。
- `ui/theme.slint` 是唯一应用颜色来源，直接参与 Slint 编译，不再需要其他主题生成工具或文件。
- `build.rs` 编译 Slint 并内嵌图像，Windows 目标同时编译 ICO 资源；可执行文件不依赖仓库内素材路径。
- `assets/app-icon.png` 保持现有图形，macOS 打包生成标准与 Retina 尺寸的 `iconfile.icns`；SVG 是可编辑源图。
- `scripts/package-macos.sh` 只负责把已有原生可执行文件打包成应用，不启动其他应用构建链。

## 数据兼容与保护

- serde 接受既有缺省字段与 null 集合；保留来源迁移、中央状态版本和锁文件结构。
- 目录 SHA-256 保持既有排序、相对路径、链接和文件内容算法；提示词继续使用 MD5。
- JSON/TOML 写入保留无关配置字段，并维护同一宿主所有托管 MCP 的配置哈希。
- jcode 保留既有 `mcpServers` 别名，避免同时生成冲突的根节点。
- Skill 优先符号链接，失败回退 copy；移除前验证 copy 哈希和 link 指向。
- 项目始终使用 copy；更新只同步干净副本，修改/缺失/冲突状态分别显示。
- 提示词沿用 `.chubak` 原始备份；宿主配置备份为 `.chu.bak`，恢复前保存 `.chu.before-restore`。
- 凭证解析保留多行引号、重复记录、注释、CRLF、转义和前导零；状态提交失败回滚凭证。
- MCP/Agent 编辑删除使用稳定 ID 与全目标预检，保护实际目标路径和中央 Agent 外部修改；按配置路径合并写入并刷新共享哈希，运行时失败回滚 state、文件和备份，不宣称崩溃原子性或跨进程锁。
- Skill GUI 安装使用后端 token 绑定固定 commit、合法路径和内容指纹，追踪 ref 与 resolvedCommit 分开；旧 JSON 预览/安装契约继续保留。
- 私有权限与原子替换保护文件；已有目录权限不被顺带改写；目录替换先暂存新副本。

## 显式导入

- Skills/MCP 默认页面与统计只投影 `managed` 数据；发现的宿主条目不混入默认列表。
- `ListImportCandidates(kind)` 返回候选 ID、来源、类型与可用性，候选 ID 包含内容指纹。
- `ImportHostResources(kind, ids, hostIds)` 先校验来源和全部目标，再执行中央入库与立即应用。
- `import_scan.rs` 处理读取/归一化，`imports.rs` 处理预检/写入，`import_transaction.rs` 回滚配置、备份、链接及新部署。
- Skill 导入不移动或删除来源；内容相同的目标可接管，不同内容拒绝覆盖。
- `mcp_native.rs` 转换命令数组、环境变量与请求头等宿主差异，并保留选项；既有记录沿用其兼容格式。
- `import_dialog.rs` 管理勾选计数和兼容状态；底部应用区固定可见，HTTP/SSE 不能勾选 jcode。

## 验证入口

- `cargo test --all-targets --locked`：隔离业务、文件保护、格式兼容、网络、图标、主题、原生控件与 example 回归。
- `tests/native_ui.rs`：鼠标事件驱动真实后台工作线程、编辑、部署、Provider、MCP 和 Git 安装/更新。
- `tests/import_ui.rs`：候选多选、目标勾选、导入/立即应用、jcode 兼容与回到默认列表。
- `tests/pointer_controls.rs`：首次点击、焦点切换、输入、勾选、禁用/遮罩及键盘操作。
- `tests/native_layout.rs`：原生主题兼容、旧应用栈回流保护。
- `examples/create_fixture.rs`：只使用本地临时目录和模型服务生成持久 QA 数据，并拒绝覆盖已有目录。
- `--screenshot` / `--import-dialog`：真实原生窗口截图，不使用模拟业务 API。

所有写路径测试提供显式 root/home。清理旧实现后，数据兼容测试仍然保留；历史行为对照结果记录在验收文档，不再要求保留另一套运行时。
