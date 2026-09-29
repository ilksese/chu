# 代码规范

## 适用范围

本文档约束 `frontend/src` 下手写 TypeScript 与 React 源码的文件命名和组织方式。框架约定文件、生成文件及第三方代码不受这些命名规则约束。

## 文件命名

| 类型 | 命名方式 | 示例 |
| --- | --- | --- |
| 页面 | `kebab-case` | `projects-page.tsx`、`settings-page.tsx` |
| React 组件 | `PascalCase` | `AppShell.tsx`、`ui/Dialog.tsx` |
| 公共自定义 Hook | `camelCase` | `hooks/useBodyScrollLock.ts` |
| Store | `camelCase` | `stores/appStore.ts` |
| 普通模块 | `camelCase` | `navigation.ts`、`resources.ts` |
| 系统级桥接或辅助工具 | `kebab-case` | `debug-bridge.ts` |

页面组件即使导出 PascalCase React 组件，文件名仍使用 `kebab-case`。系统级辅助工具是连接运行时、调试通道、构建环境或操作系统能力的基础设施模块，不包括普通业务工具函数。

## 目录约定

- 页面放在 `frontend/src/pages`。
- 跨页面业务组件放在 `frontend/src/components`。
- 原子 UI 组件放在 `frontend/src/components/ui`。
- 全局公共自定义 Hook 放在 `frontend/src/lib/hooks`。
- 页面瞬时状态留在页面组件；跨页面状态放在 `frontend/src/stores`。

## 导入约定

- 使用 `@/` 别名导入 `frontend/src` 内模块。
- 导入路径的大小写必须与磁盘文件名完全一致，确保在 Linux 和 CI 环境中可用。
- 重命名文件时必须同步更新全部导入方，不保留旧路径兼容层。

## 例外

- `vite-env.d.ts` 等框架约定文件保持框架要求的名称。
- `frontend/wailsjs` 下的 Wails 绑定是生成文件，不手动重命名或修改。
