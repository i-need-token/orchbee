# @orchbee/desktop-client-protocol

用于 Orchbee Desktop 与权威 `apps/web` 渲染器之间 Electron 专属能力边界的严格、运行时无关数据契约。

## 适用范围

该包负责一个原子的 Desktop renderer bootstrap、十三个独立 capability port 的固定顺序与安全证明、一个静态 SSH Project-enrollment availability boolean、Agent-observation snapshot 与 semantic-event contract、Agent-integration-management command contract、Browser 访问与个人访问令牌管理契约、类型化的现有 Project Folder 获取与托管 Project Worktree 契约、设备本地终端启动配置文件与终端默认设置命令契约、有界纯文本剪贴板读取契约、相关的默认远程终端创建契约、专用 SSH Project enrollment、existing-Project Folder attachment、可复用 SSH Environment catalog 与显式连接控制契约、preload 暴露的 context-isolated 文件路径查询稳定名称，以及 Electron 应用生命周期控制。bootstrap 内嵌由 `@orchbee/workbench-client-protocol` 所拥有、与 transport 无关的 Workbench client-session bootstrap。Availability boolean 不公开 profile、path、signer、artifact、Environment identity 或 diagnostic；Web 只用它在 Main 没有 admitted release source 时省略 SSH surface。

应用生命周期区分用户退出、更新重启和仅开发环境使用的 Terminal Host 重启。用户退出可以取消、保留或停止；更新重启自动保留。开发 Host 重启是一种显式、非交互模式：它持久地停止当前会话，并在 Main 报告就绪前暂存新的 provision；它不是 updater 的别名。命令只携带请求标识、模式、聚合计数、精确的共享文档 revision、进度、有界类型化失败，以及渲染器替换后恢复进行中开发 Host 重启所需的 Workbench presentation 标识。命令绝不暴露 Host、绑定、终端或文件系统权限。

Agent observation 使用 protocol version `2`：renderer 发送一个 correlated full-snapshot request；Main 发送 correlated response、unsolicited complete `AgentRuntimeSnapshot` publication，以及经过严格解析的 canonical `AgentSemanticEvent` publication。Snapshot revision 仍是 state authority；semantic publication 是在对应 snapshot 更新后交付的精确 effect trigger，不会作为 history replay。它不携带 Agent hook payload、prompt、transcript、command、tool content、transport correlation capability、response channel 或 orchestration authority；canonical Agent semantic 仍属于 `@orchbee/agent`。Agent integration management 携带一份 canonical ten-entry snapshot、有序且有界的本地 direct-launch identity、unsolicited idempotent launch observation，以及 correlated inventory、refresh、enable、disable、completed 与 bounded failure message，除此之外不携带其他信息。Launch observation 不含 command text 或 argument，也不授予 status 或 mutation authority。它不公开 executable path、packaged-resource path、raw Agent output、generic process/filesystem capability 或 remote-host authority。

它不包含 Electron import、UI、broker、coordinator、transport lifecycle、网络 Gateway、鉴权、终端投影、PTY 或 VT 行为。Browser 访问 snapshot 只携带 lifecycle state、净化后的 Origin，以及个人访问令牌的 `configured`、`missing` 或 `unavailable` 状态。一个相关的创建/重置命令可以精确返回一次新签发令牌；任何消息都不携带持久化 verifier、Browser session 签名密钥、Gateway 端口、子进程句柄或含 credential 的 URL。Project Folder 获取只能请求用户选择的一个现有目录，并返回相关的 selected、cancelled 或有界 failure 结果；它不能接受任意 renderer path、枚举目录或改变文件系统。Project Worktree discovery 携带一个 Project identity 与可选首选 Directory identity；它返回 repository 创建 metadata，以及只含显示名称和不透明 identity 的有界已有 Worktree inventory。创建只携带规范 discovery 返回的 identity、base ref、新分支与 Workspace 选项。接纳只携带 discovery 返回的 Project、源 Directory、repository 和 Worktree identity。Renderer 命令不能提交源或目标 path、调用通用 Git，或选择位置覆盖。Discovery 可以投影固定托管根目录，供产品只读展示。Terminal-launch-profile 命令公开有界的设备本地目录以及相关的创建、更新、默认、删除和重置操作。命令可以选择具有 POSIX 登录 shell 决策的系统 shell 或具有文字有序参数的绝对可执行文件；它无法提供环境变量、秘密、工作目录、项目有效负载、一次性启动选择或通用文件系统访问。Terminal-default-settings 命令只公开完整、有界的 `scrollbackBytes` 配置和带持久化状态的严格 snapshot；它不能更新正在运行的终端，也不携带 Host 或文件系统权限。剪贴板读取命令只携带相关请求，随后返回不超过终端输入字节上限的一项纯文本值或一项类型化 failure；它不公开剪贴板写入、其他格式、图像、历史记录或通用 platform capability。远程终端创建只携带相关的规范 Project 与 Workspace identity。每个不同 request 可以追加一个 default top-level Terminal，精确 replay 会收敛到该 request 的 canonical Presentation；它不能选择 path、Group、split 或 insertion index。SSH Project enrollment 初始只接受一个有界 SSH 地址。其相关消息携带 progress；精确 prompt generation 与 handle；有界 prompt 与瞬时 response；精确 directory-listing revision、canonical path、parent、有界 child-directory name、navigation 或 selection；连接 status；cancellation 与类型化 settlement。它们绝不携带 Environment 或 Runtime identity、resolved target、SSH argument vector、Host connection、release artifact、持久 credential、file byte、file entry、filesystem mutation 或通用 Environment authority。文件路径 bridge 名称同样不授予通用文件系统权限或定义 shell quoting；preload 拥有 Electron adapter，Web 应用拥有消费 policy。共享 Workbench 页面客户端契约仍属于 `@orchbee/workbench-client-protocol`；Workbench 语义仍属于 `@orchbee/workbench`。

Folder attachment 只在单地址 input 上增加精确 target Project identity，并返回 Main 分配的 Directory identity
及其 canonical path。它复用相同 progress、prompt、directory-selection、cancellation 与 single-active-request
record。Enrollment 与 Folder attachment record 都不携带 Environment identity、不选择 Directory
identity，也不授予 generic Project mutation authority。

SSH Environment management 会发布有界的 registry-backed row 与具体 `Host` 建议。只有这些
management record 会携带不透明的已登记 Environment identity，且只用于精确 catalog status、
Connect 与 Disconnect 关联。Discovery 绝不携带 configuration body 或 path、已解析 SSH option、
credential 或 filesystem authority。Refresh 不会 activate；Add 只解析一个 destination，并追加或复用
registry intent，不连接、打开 Project、选择 directory 或创建 terminal。

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
