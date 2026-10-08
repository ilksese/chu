# 原生桌面调试与隔离验收

## 创建隔离数据

```bash
cargo run --locked --example create_fixture -- /tmp/chu-native-qa
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home
```

输出目录必须不存在。省略 example 的目录参数时会生成可保留的临时目录，并输出包含 `base`、`root`、`home` 的 JSON。示例通过真实应用命令创建 Skills、MCP、Agent、提示词、References、项目和本地模型目录；不调用联网供应商，不操作真实宿主配置。

`--root` / `--home` 必须成对使用；不要为了方便改写进程 HOME，也不要以真实配置做写路径测试。

## JSON 命令

```bash
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home   --request '{"method":"Refresh","args":[]}'
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home   --request '{"method":"ListImportCandidates","args":["skills"]}'
```

命令与 UI 共用 `App::execute`。候选 ID 含内容指纹，不能手工拼造；导入使用 `ImportHostResources(kind, ids, hostIds)`。

## 原生截图

```bash
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home   --page 2 --screenshot /tmp/chu-native-qa/skills.png
cargo run --locked -- --root /tmp/chu-native-qa/chu --home /tmp/chu-native-qa/home   --page 3 --import-dialog --screenshot /tmp/chu-native-qa/import-mcp.png
```

窗口使用原生软件渲染器，截图来自真实窗口。`--width` / `--height` 设置启动时请求的物理尺寸，实际尺寸还受窗口最小限制、系统缩放和显示器可用区域影响；不要把 Retina PNG 像素数直接当成逻辑布局尺寸。

页面编号：0 总览、1 项目、2 Skills、3 MCP、4 Providers、5 Agents、6 提示词、7 References、8 设置。`--import-dialog` 只支持页面 2/3。

## 定向回归

```bash
cargo test --locked --test imports
cargo test --locked --test import_ui
cargo test --locked --test pointer_controls
cargo test --locked --test native_layout
cargo test --locked --example create_fixture
```

UI/后端测试发送实际鼠标按下/抬起事件。若首次点击无效，检查 `TouchArea` 上方是否有获取焦点并消费事件的覆盖层。不要通过额外点击、延迟或双击绕过问题。

完整验证运行 `./scripts/verify-rust.sh`；发布产物运行 `./scripts/build.sh`。测试与验收产物放在临时目录或忽略目录，不进入源码。
