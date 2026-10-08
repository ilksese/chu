# 原生代码规范

## 适用范围与命名

- Rust 模块、函数与变量使用 `snake_case`；类型和 trait 使用 `PascalCase`，常量使用 `SCREAMING_SNAKE_CASE`。
- Slint 文件和属性使用 `kebab-case`；组件、结构和全局主题使用 `PascalCase`。
- Shell 脚本使用 `kebab-case.sh`；可运行的 Rust 示例放在 `examples/`。
- 导入路径、素材引用和文件名大小写必须与磁盘一致，保证跨平台构建。

## 目录职责

| 路径 | 职责 |
| --- | --- |
| `src/main.rs` | 原生启动和 CLI 参数 |
| `src/app.rs` / `src/models.rs` | 命令边界、状态、序列化与兼容 |
| `src/{skills,mcp,...}.rs` | 资源业务、宿主配置与文件操作 |
| `src/controller.rs` / `src/callbacks.rs` | 串行工作线程、UI 事件桥接 |
| `src/presenter.rs` / `src/search.rs` | 视图投影、搜索与选中项 |
| `src/editor.rs` / `src/import_dialog.rs` | 表单草稿与导入选择 |
| `ui/` | 原生界面和共享控件 |
| `ui/theme.slint` | 唯一应用颜色来源 |
| `assets/` | 实际使用的品牌素材及其源图 |
| `tests/` | 隔离业务、交互、主题和打包回归 |
| `examples/create_fixture.rs` | 基于真实命令的隔离验收数据生成 |
| `scripts/` | 原生构建、打包、验证 |

## 边界规则

- 网络与资源文件操作在后台工作线程或独立后端 API 中执行，不放入 Slint 回调。
- 持久化资源状态与页面瞬时状态分离；UI 仅消费安全视图，不消费原始凭证。
- 测试使用显式的临时 home/root；不能改变进程 HOME 使测试访问用户目录。
- 批量导入先检查全部冲突，失败时使用文件日志回滚；不要绕过现有改动保护。
- 保留已有 JSON 字段名、缺省/null 兼容及哈希语义；清理架构不等于删除数据迁移逻辑。
- 不手改 Slint 编译输出或依赖锁条目。新增依赖前确认现有 crate 不能完成需求。
- 手写文件接近 400 行时评估职责，原则上小于 500 行；仅在职责边界清晰时拆分。

## 测试约定

- 后端测试通过 `App::open` 或隔离 fixture 调用真实业务逻辑。
- 控件测试通过无障碍名称定位，然后发送鼠标/键盘事件；只调用回调不能代替首击测试。
- mock 时间只用于无事件循环测试；后台线程联动采用实时事件，不混用两种时钟。
- 主题和应用图标均有兼容性回归；界面调整必须检查最小窗口与弹窗底部动作。
