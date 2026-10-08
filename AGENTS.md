# Chu Agent Notes

## 沟通与界面

- 使用中文回复。
- Chu 是 Rust + Slint 原生桌面 GUI，默认 `1360x860`，最小窗口 `1080x680`，不做移动端适配。
- 遵循 `DESIGN.md`；优先复用 `ui/components.slint` 中的共享组件。
- 颜色集中定义于 `ui/theme.slint`，UI 组件必须使用 `Theme` 语义令牌，不得硬编码主题颜色或阴影颜色。黄色仅作强调，红色用于危险、错误和既定焦点提示。
- 缺少颜色时先补充原生主题；只有官方 Logo、品牌图形和不可主题化素材可以保留原色。
- 应用图标共用 `assets/app-icon.png`；Windows 使用 `assets/windows/app.ico`；SVG 是可编辑源图，不要用占位图替换品牌资源。

## 架构边界

- `src/main.rs` 是原生入口，提供桌面启动、隔离模式、JSON 自动化和截图。
- `src/app.rs` 管理中央状态、兼容迁移和 `App::execute` 命令边界，资源操作按职责分布在 `src/`。
- `src/controller.rs` 的串行工作线程独占 `App`；禁止在 Slint 回调中做资源文件操作或网络请求。OS 原生目录选择除外。
- `src/presenter.rs` / `src/search.rs` 管理页面投影和瞬时搜索/选中项；表单草稿与导入选项在 `src/editor.rs` / `src/import_dialog.rs`。
- Skills/MCP 默认列表只显示 `managed` 数据。宿主资源通过 `ListImportCandidates` 和 `ImportHostResources` 显式导入并应用；不要重新混入默认列表。
- 导入写入使用预检和 `src/import_transaction.rs` 的回滚；禁止覆盖已修改副本或不同内容的同名资源。
- 真实状态位于 `~/.chu/state.json`，可能修改四宿主真实配置。所有写路径测试必须使用 `tempfile::TempDir` 与显式隔离 home，不能访问真实用户目录。
- 保留现有状态/锁文件、来源迁移、JSON/TOML 配置、凭证格式、备份和哈希兼容，不因代码清理丢弃用户数据。

## 代码组织

- 命名和目录职责见 `references/code-conventions.md`。
- 手写源码接近 400 行时评估职责边界，原则上控制在 500 行以内；禁止为满足行数机械拆分。
- 生成文件和锁文件不受行数目标约束；不手改 Slint 编译产物或 `Cargo.lock` 内的依赖条目。
- `ui/theme.slint` 是唯一应用主题来源；图像在编译时内嵌，不能依赖运行时仓库路径。

## 工具链与验证

- 使用 Rust 1.92+ 和 Cargo；Git 用于 Skill 来源仓库。平台构建需要对应 SDK、链接器和原生资源工具。
- 开发：`cargo run --locked`。
- 完整验证：`./scripts/verify-rust.sh`；包含格式、Clippy、所有目标的 Rust 测试及主程序构建。
- 定向测试：`cargo test --locked --test imports`。
- 本机打包：`./scripts/build.sh`，产物在忽略目录 `build/bin/`；macOS 应用为 `build/bin/Chu.app`。
- 隔离验收数据：`cargo run --locked --example create_fixture -- <新的输出目录>`，禁止覆盖已有目录。
- 调试与截图见 `references/debug.md`。`--root` 和 `--home` 必须同时提供，不允许把真实宿主配置作为写路径测试。

## 交互回归

- 使用命中测试和鼠标按下/抬起事件验证首次点击，不能只调用回调替代交互测试。
- 仅处理键盘的 `FocusScope` 不能覆盖 `TouchArea`，应保持零尺寸，防止吞掉首次鼠标点击。
- 实时工作线程联动测试不混入 mock 时间；需要可见焦点、无障碍名称和 Tab/Enter/Space/Escape 支持。
