# @orchbee/desktop-client-protocol

用于 Orchbee Desktop 与权威 `apps/web` 渲染器之间 Electron 专属能力边界的严格、运行时无关数据契约。

## 适用范围

该包负责原子 Desktop renderer bootstrap、固定十一端口顺序与安全声明、Agent 观察与集成管理、应用生命周期、本地 Project Folder 获取与托管 Worktree 创建、终端启动配置、终端默认值、有界剪贴板读取与绑定来源的共享用户偏好。偏好端口复用可移植 workbench 偏好 schema，公开确认快照、有界拒绝／结果不明及独立应用问题，不提供路径或通用文件系统能力。

应用生命周期区分用户退出、更新重启和仅开发环境使用的 Terminal Host 重启。用户退出可以取消、保留或停止；更新重启自动保留。开发 Host 重启是一种显式、非交互模式：它持久地停止当前会话，并在 Main 报告就绪前暂存新的 provision；它不是 updater 的别名。命令只携带请求标识、模式、聚合计数、精确的共享文档 revision、进度、有界类型化失败，以及渲染器替换后恢复进行中开发 Host 重启所需的 Workbench presentation 标识。命令绝不暴露 Host、绑定、终端或文件系统权限。

Agent observation 使用 protocol version `2`：renderer 发送一个 correlated full-snapshot request；Main 发送 correlated response、unsolicited complete `AgentRuntimeSnapshot` publication，以及经过严格解析的 canonical `AgentSemanticEvent` publication。Snapshot revision 仍是 state authority；semantic publication 是在对应 snapshot 更新后交付的精确 effect trigger，不会作为 history replay。它不携带 Agent hook payload、prompt、transcript、command、tool content、transport correlation capability、response channel 或 orchestration authority；canonical Agent semantic 仍属于 `@orchbee/agent`。Agent integration management 携带一份 canonical ten-entry snapshot、有序且有界的本地 direct-launch identity、unsolicited idempotent launch observation，以及 correlated inventory、refresh、enable、disable、completed 与 bounded failure message，除此之外不携带其他信息。Launch observation 不含 command text 或 argument，也不授予 status 或 mutation authority。它不公开 executable path、packaged-resource path、raw Agent output、generic process/filesystem capability 或 remote-host authority。

它不包含 Electron import、UI、broker、coordinator、传输生命周期、认证、终端投影、PTY 或 VT 行为。契约只授予各端口所表示的范围内能力。

Folder acquisition 返回已验证路径，不携带分组成员关系。Workbench 通过不可变资源命名空间解析添加目标。Worktree discovery/create/adopt 命令标识源 Workspace 和资源 Directory；Main 重新验证源执行身份，在提交时使用当前可空 Project 成员关系。类型化 `worktree-retained` 失败保留已知 Git 结果，不重放创建。命令不授予通用 Project、文件系统或进程权限。

本地 Project Folder 获取区分现有文件夹选择、原生父目录选择和明确创建子目录。创建命令只携带父目录选择标识、单调递增尝试编号与单一有界名称，不携带渲染器指定的路径。已知无效果失败允许在同一父目录明确修正重试。创建成功或结果未知会消费父目录凭据；后续操作前必须确认精确创建请求。退役后的保留回执仅在 Main 生命周期内报告。参见[所属契约](../../apps/desktop/docs/project-folder-acquisition.zh-CN.md)。

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
