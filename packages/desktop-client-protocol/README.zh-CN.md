# @orchbee/desktop-client-protocol

用于 Orchbee Desktop 与权威 `apps/web` 渲染器之间 Electron 专属能力边界的严格、运行时无关数据契约。

## 适用范围

该包负责一个原子的 Desktop 渲染器 bootstrap、八个独立能力端口的固定顺序与安全证明、类型化的现有 Project Folder 获取与托管 Project Worktree 契约、设备本地终端启动配置文件与终端默认设置命令契约、preload 暴露的上下文隔离文件路径查询稳定名称，以及 Electron 应用生命周期控制。bootstrap 内嵌由 `@orchbee/workbench-client-protocol` 所拥有、与传输无关的 Workbench 客户端会话 bootstrap。

应用生命周期区分用户退出、更新重启和仅开发环境使用的 Terminal Host 重启。用户退出可以取消、保留或停止；更新重启自动保留。开发 Host 重启是一种显式、非交互模式：它持久地停止当前会话，并在 Main 报告就绪前暂存新的 provision；它不是 updater 的别名。命令只携带请求标识、模式、聚合计数、精确的共享文档 revision、进度、有界类型化失败，以及渲染器替换后恢复进行中开发 Host 重启所需的 Workbench presentation 标识。命令绝不暴露 Host、绑定、终端或文件系统权限。

它不包含 Electron 导入、UI、代理、协调器、传输生命周期、网络 Gateway、身份验证、终端投影、PTY 或 VT 行为。 Project 文件夹获取只能请求用户选择的一个现有目录，并返回相关的选择、取消或绑定失败结果；它不能接受任意渲染器路径、枚举目录或改变文件系统。Project Worktree discovery 携带一个 Project identity 与可选首选 Directory identity；创建只携带规范 discovery 返回的 identity、base ref、新分支与 Workspace 选项。Renderer 命令不能提交源或目标 path、调用通用 Git，或选择位置覆盖。Discovery 可以投影固定托管根目录，供产品只读展示。Terminal-launch-profile 命令公开有界的设备本地目录以及相关的创建、更新、默认、删除和重置操作。命令可以选择具有 POSIX 登录 shell 决策的系统 shell 或具有文字有序参数的绝对可执行文件；它无法提供环境变量、秘密、工作目录、项目有效负载、一次性启动选择或通用文件系统访问。Terminal-default-settings 命令只公开完整、有界的 `scrollbackBytes` 配置和带持久化状态的严格快照；它不能更新正在运行的终端，也不携带 Host 或文件系统权限。文件路径桥名称同样不授予通用文件系统权限或定义 shell 引用； preload 拥有 Electron 适配器，Web 应用程序拥有消费策略。共享的 Workbench 页面客户端合约仍保留在 `@orchbee/workbench-client-protocol` 中； Workbench 语义保留为 `@orchbee/workbench`。

## 使用

应用程序从包根导入架构和推断类型。每个进程、IPC、Window 或 MessagePort 边界都会在将不可信值传递给内部所有者之前解析它们。

```ts
import { DesktopRendererBootstrapSchema } from '@orchbee/desktop-client-protocol';

const bootstrap = DesktopRendererBootstrapSchema.parse(untrustedBootstrap);
```

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/desktop-client-protocol build
pnpm --filter @orchbee/desktop-client-protocol typecheck
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop-client-protocol test:exports
```
