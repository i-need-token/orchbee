# 桌面

沙盒化 Electron 产品客户端和 Orchbee 的本机 Terminal Host 的本地代理。

## 当前范围

桌面与主机支持的投影终端组成共享工作台。 Workbench拥有Project、Workspace、Tab、Panel、`TerminalPresentationId`状态； Electron Main拥有持久的技术绑定和Electron独有的特权效果； `apps/terminal-host` 拥有 PTYs、进程、Ghostty VT 状态、历史记录和真实终端身份。浏览器客户端在其自己的页面中提交剪贴板和白名单链接效果。共享工作台文档均不存储在主机中。

Main 恢复持久的 `TerminalPresentationId` 到 `TerminalBindingId` 注册表，拥有一个规范 Workbench 共享文档协调器，并通过沙箱 preload 传递一个原子的能力白名单包。启动时不会创建固定终端或附件。十一个端口具有固定顺序：

- 客户端会话控制提供一个原子文档/终端观察引导程序，接受完整的精确下一个共享文档提案，并发布一个有序的规范事件流；
- 投影控件仅为显示的终端视口打开并释放范围内的主机附件；
- Agent-observation 控制接受一个有关联的完整 snapshot 请求，并在状态更新后发布完整、带 revision、provider-neutral 的 snapshot 与精确 semantic event；它不携带 provider payload、prompt、transcript、command、tool、correlation capability 或 decision channel；
- Agent-integration-management 控制投影一个有界的十 entry 只读本地 inventory，并且只接受针对 Orchbee 精确托管 entry 的 correlated refresh、enable 与 disable command。它不携带 executable/resource path、command output、通用 process/filesystem authority 或 remote-host mutation authority；
- 应用程序生命周期控制仅包含退出原因、用户选择、静默确认、共享文档修订、聚合进度、类型化故障以及恢复正在进行的开发主机重启所需的工作台表示标识；
- Project 文件夹获取选择一个原生现有目录，或在选定父目录下明确创建一个原本不存在的子目录；关联结果区分无效果、已创建、不可用及未知结果；
- Project Worktree 控制只从源 Workspace 资源 Directory 发现 repository，并根据 repository、base、branch 与 Workspace 选项创建一个托管 Worktree；renderer 命令绝不携带目标路径；
- Terminal-launch-profile 控制投影并改变系统 shell 或显式可执行启动选择的一个有界设备本地目录。它不携带环境、CWD、机密、项目有效负载或通用文件系统功能；
- Terminal-default-settings 控制只投影并改变有界的设备本地默认回滚缓冲区字节数。它不携带正在运行的终端、Host 或文件系统权限；
- 剪贴板读取控制只接受相关的纯文本读取请求，并返回一项受终端输入边界限制的文本值或类型化失败。它不公开写入、其他格式、图像、历史记录或通用 Electron 能力；

- user-preferences 控制通过匹配原生桥接读取及更新共享设备偏好，承载绑定来源的补丁与确认快照，不提供路径或通用 I/O。

Client-session 与 projection 数据来自 `@orchbee/workbench-client-protocol`，规范 Agent 快照与事件来自 `@orchbee/agent`。`@orchbee/desktop-client-protocol` 负责十一端口 bootstrap，以及 Agent 观察、集成管理、应用生命周期、本地 Project Folder 和 Worktree、终端启动配置、终端默认值与剪贴板契约。Main 与 Web 将这些契约适配到 `MessagePort`，共享 Workbench runtime 不承担传输细节。

Projection target 选择仍由 Main 负责。请求只携带 `TerminalPresentationId` 与 viewport geometry。Main 解析规范归属并要求本地 Directory placement；旧 Environment placement 会在打开 runtime 前被拒绝。打开后 Main 再次检查规范权威，若目标已失效则先释放临时 attachment，不发布它。

渲染器没有 Node 集成、通用 Electron IPC、filesystem read capability、Host inventory 或原始 terminal-lifetime 权限。它不能直接调用 raw provision、restore、close 或 storage CAS effect。Preload 只接受一个 bootstrap，同时验证全部十一个 port，保留固定的 client-session/projection/Agent-observation/Agent-integration-management/application-lifetime/Project-Folder-acquisition/Project-Worktree/terminal-launch-profiles/terminal-default-settings/clipboard-read/user-preferences 顺序，并证明 context isolation 与 sandboxing。Project Folder port 不接受 renderer path，也不公开 enumeration 或 mutation primitive。Project Worktree port 不接受源或目标 path：discovery 从规范 Project state 推导 repository，创建只接受返回的不透明 identity 与 Git 值。单独的 context-isolated file-drop adapter 只公开当前 platform 与 Electron 的 `File`-scoped backing-path lookup；它不能接受 arbitrary path string、读取 file content、枚举 directory 或执行 IPC request。一个拖入的 disk `File` 可以携带 file 或 directory backing path，但不会授予 child enumeration。Web renderer 只为 user drop 准入该 adapter，应用有界且兼容 Ghostty 的 POSIX live-buffer escaping，直接发布 prepared text，并且绝不追加 Enter。Partial 或 malformed bootstrap 不会启动 renderer runtime。

Main 从应用构造到 shutdown 拥有一个 volatile、provider-neutral 的 Agent runtime。Renderer replacement 或 crash 只 retire 自己的 broker；每个替代 renderer 都会从同一 runtime 获得一份新的完整 snapshot。Desktop restart 会有意从空 snapshot 开始，且不持久化任何 Agent observation。一个准确的 private APC `orchbee.agent.v1` launch envelope 只能创建 capability-free、status unknown 的 heuristic presence session；它会在 30 秒后 stale、120 秒后 detach，不发 semantic event，也没有 notification authority。它还只会触发对该 closed integration ID 的 lazy read-only inspection。Claude Code、Codex、Grok、OpenCode、Pi、OMP、Gemini CLI、GitHub Copilot CLI、Cline 与 Qwen Code 的 rich observation 使用用户显式安装的捆绑 [Agent observation adapter](resources/agent-observation-adapters/README.zh-CN.md)。每个 Agent-bound adapter 校验自己的 upstream contract，丢弃 prompt、tool input/output、transcript path、working directory、error text 与未知 field，只向 private Unix-domain socket 发送共享 normalized v1 allowlist。由应用拥有的 closed registry，而不是 adapter input，授予 adapter provenance、accepted event 与 maximum capability。Visibility 位于 socket envelope 内。只有一个已准入 local 或 reverse-forwarded socket 返回准确 v1 ACK 后，adapter 才会独立地把 private APC correlation half 直接写入每个解析出的 outer TTY，因此覆盖 direct、tmux、Zellij 与 nested-mux session。只有两半针对 live presentation generation 精确 join 后，Desktop 才准入 rich semantic；APC 单独不能制造 idle、attention、outcome、action 或 notification authority。OSC 9、OSC 777、OSC 99 与 OSC 1337 等公开 notification channel 从不用作 observation。[Agent observation ingress contract](docs/agent-observation-ingress.zh-CN.md)负责 join state model、timer 与 generation invariant、observation design 和 correctness evidence。

Desktop 只通过非持久化 `CreateTerminal.environmentOverlay` 提供 `ORCHBEE_AGENT_OBSERVATION_SOCKET`，绝不写入 durable `TerminalLaunch`。显式 sourced 的 Bash、Zsh 或 Fish integration 会为符合条件的 interactive `ssh` call 增加 `StreamLocalBindMask=0177`、`StreamLocalBindUnlink=yes`，以及从远端随机 `/tmp/orchbee-agent-v1-*.sock` 到本地 socket 的 reverse Unix-socket forward。既有 alias/function、noninteractive 与 forwarding-only SSH mode、显式 argument 和 OpenSSH configuration 仍是 authority。不使用 HTTP listener、TCP port、Bearer credential、继承的 terminal secret、remote file installation 或 durable remote cache。Hook、socket、mux resolver、APC 或 SSH-forward failure 都不会阻塞 Agent、SSH 或 terminal。Adapter identifier 只作为 ingress provenance；产品 status、navigation 与 attention behavior 只消费 canonical Agent model，不按 adapter 分支。由 [Agent package guide](../../packages/agent/README.zh-CN.md) 与 [proposal](../../proposals/agent/agent-runtime-observation.zh-CN.md) 定义已实现边界和仍保留的产品决策。

Desktop 另外拥有显式本地 [Agent integration management](docs/agent-integrations.zh-CN.md)。Startup 不检查任何 provider，也不安装任何内容。显式 Refresh 会检查全部 provider；识别到一次 direct launch 时只检查对应 provider。如果它的 integration 尚未启用，安静的右下角 reminder 只能打开 Agent 集成设置，不能修改配置。只有用户在 Settings 中显式操作，才会发布带版本的 durable Capsule 并更新 Orchbee 的准确 guarded provider entry；Main 会重新 inspect 后才反映成功。持久化 hook 会 guard 缺失 executable 与 payload，provider-local module 会捕获 Capsule import 缺失，self-contained extension 不依赖 application-bundle path，已识别的 legacy path 会成为显式 `update-required` migration。Conflict 保持不变，remote host 仍只能手动配置，disable 会保留无关 Agent configuration。

Desktop 上下文菜单和自定义快捷键 Paste 使用 Main 拥有的剪贴板读取端口。空闲的平台默认粘贴组合键会保留原生可编辑元素投递，使共享终端适配器可以区分纯文本与仅图片剪贴板，并在不读取或传输图片字节的情况下发送兼容 Agent 的 `Control+V` 事件对。普通终端选择仍是 Host/Canvas 状态且不占用文档全局 DOM Range，因此隐藏 textarea 会继续作为唯一的原生输入与 IME 所有者。有界的终端 root 后备只准入来自 package 自有 Presentation 后代的粘贴；查找输入框等应用自有控件保留原生编辑行为。当手动选择 Copy 或另一个 Paste Action 仍在提交时，默认组合键会使用 Action 路径；Paste 会等待该精确 Copy 操作结算后再请求当前纯文本，然后复用共享终端的粘贴清理、审阅、回到底部和清除选择路径。Copy 失败时不会合成文本或重试：Paste 会在结算后读取平台剪贴板中的实际纯文本。Browser Preview 页面不接收 Desktop 剪贴板读取端口，并保留现有本地 Browser 策略。

OSC 52 剪贴板写入使用现有投影端口，而不是通用 preload 剪贴板 API。Main 只记住转发到附件的第一个 Host 请求；渲染器自动提交决定必须精确重复其附件、序列与有界文本。匹配的肯定决定会执行 Desktop 拥有的纯文本写入并消费待处理请求；不匹配、重复、Broker 背压、页面退役或附件处置均不写入。

Main 在允许客户端之前加载并验证共享文档一次。可用的文档可恢复项目、工作区、选项卡、组、持久后备/自定义选项卡标题、面板/拆分结构、Presentation 布局和生命周期意图；每个页面都会在内存中派生一个新的 Client View。Desktop Web 组合可以从该 Electron 页面会话存储的一个精确且仍存活的 `TerminalPresentationId` 中，只恢复焦点 Panel 与显示的子 Presentation。主动导航、披露、焦点、最大化、主动子项、自然终端标题、当前 CWD 和 Split 权重不进入共享文件。丢失的文档将每个恢复的绑定投影为独立的 `retain` 选项卡，并为每个加入页面提供相同的 revision-`0` 初始化种子。第一个接受的提案会在 Main 执行恢复/配置效果之前持久化该种子。不可用的加载或失败的首次提交不会创建替换文档或终端效果。未引用的技术绑定仍然保留，但不会默默加入共享文档。

Coordinator 保持传输中立，Electron 使用直接的 `MessagePort` adapter。`apps/web` 拥有共享 Workbench renderer 和独立的内存 Browser Preview。开发版 Electron 通过 loopback Web origin 上的 User-Agent 发现 adapter；打包 Desktop 通过标准安全 `orchbee:` origin 提供 Web build。Renderer document 导航和会话释放不会结束保留的 Host 终端。

Desktop 严格要求唯一的当前 Host 协议 `0.1.0`；预发布版本既不协商，也不记录历史协议变体。Electron 使用 `42.8.0`，即紧邻本仓库所选当前 Electron 主版本的前一主版本中的最终发布。

`HostConnection` 在每个代理调用者上拥有一个连接全局出站窗口：最多 64 个未应答请求记录和 4 个 MiB 编码请求字节。它会保留该许可，直到相关响应或连接失败为止。已返回 `false` 的完整窗口或节点可写在切换之前拒绝工作，并可重试 `host-transport-backpressure`；在 `drain` 之前没有请求写入恢复。如果移交的请求达到其截止日期，Desktop 将终止连接，并使用不可重试的 `host-request-outcome-unknown` 拒绝所有待处理和未来的请求，因此在出现不明确的结果后，永远不会重播输入。

每个投影终端 Broker 最多保留 64 个已交给 Host 的命令，其中输入专用子窗口为 16，避免一个终端的滚轮或按键突发在通常情况下耗尽连接全局窗口。它通过一个按到达顺序排列的待发窗口吸收渲染器突发；该窗口最多容纳 1,024 个已验证的非 resize 命令及其总计 4 MiB 的序列化命令数据。请求完成后会立即交付最早的可调度待发命令，并且不允许后来的命令越过受输入子窗口阻挡的队首；resize 继续使用独立的最新值合并槽。在这些边界内获准的命令既不会报告渲染器背压，也不会废弃附件。真正填满的待发窗口仍会以关闭方式失败，丢弃从未交给 Host 的工作，并且只废弃该附件。

入站 Host 记录跨 JavaScript Promise 结算保持线协议顺序。`HostConnection` 在解码时认领每个相关响应、完成该响应，并将后续记录延迟到一个事件循环屏障之后，使响应延续先于稍后的 Host 事件运行。先于响应到达的事件仍保持在前；连续事件/帧交付保持同步；每条记录解码后，传输尾部都会跟进，而无需重新开放请求准入。

在 POSIX 上构造 `HostConnection` 之前，Desktop 的进程内本机端点适配器会验证运行时目录和实际套接字、建立连接，并根据内核对端凭据验证服务器的有效 UID。只有经过验证的同 UID 连接才能抑制 Host 启动，并且每次重试或稍后 Desktop 重连都会重复检查。适配器把已验证描述符直接移交给 Node 套接字且不启动帮助进程，因此 `serve` 是唯一的 Terminal Host 产品进程。其所有权、失败、证据和回滚合同见 [`docs/posix-host-endpoint.md`](docs/posix-host-endpoint.zh-CN.md)。

Desktop 将一个 Terminal Host 端点扩展到其产品 `userData` 身份，该身份将 OS 用户、产品配置文件和发布通道数据根分开。短 POSIX 运行时路径是操作系统受信任的每用户运行时位置（Linux 上的 `/run/user/<effective-uid>` 与 macOS 上 Electron 的每用户临时位置）的稳定产品范围子代，从来都不是确定性的共享 `/tmp` 的子级。 Windows 命名管道标识源自相同的产品范围。显式端点和运行时覆盖仅适用于受控开发和测试； POSIX 覆盖仍然通过本机无跟随所有者、私有模式和 ACL 准备和验证。每次成功握手后，Desktop 都会通过 `SetTerminalDefaults` 发送当前终端默认值，因此重新连接到已运行的单例与新启动 CLI 具有相同的结果。新终端使用与 Workbench canvas 对齐的轻量（light）profile；保留的终端保留与其现有主机会话一起存储的配置。通用偏好共享 Native Site 的配置作用域存储；宿主字体与像素展示保持渲染端本地。滚动历史更新未来终端的 Host 默认值。

运行时配置统一按照声明的平台规则解析资源与存储路径，包括 CLI 启动器。模拟平台的检查使用对应平台的路径 fixture；访问真实文件系统的测试使用实际执行平台。Windows 盘符和 UNC 资源路径保持 Windows 路径语义，不受测试宿主影响。

Desktop Main 还拥有单个本地 Electron 窗口生命周期。它恢复严格的设备本地正常边界和最大化记录，将保存的几何图形重新投影到当前显示工作区域，并通过应用程序菜单及其加速器使用的 Main 拥有的语义操作公开最大化/恢复和全屏。全屏保持短暂状态，窗口状态永远不会进入共享工作台文档或终端恢复。请参阅[`docs/window-lifecycle.md`](docs/window-lifecycle.md)。

在 macOS 上，本机应用程序菜单还显示仅会话的 **安全键盘输入** 复选框。 Main拥有的一个语义Action控制Electron的特权应用程序能力；沙盒渲染器不接收安全输入 IPC 或状态。默认关闭。窗口模糊、应用程序清理、退出禁用功能和清除勾选状态；返回焦点不会默默地重新启用它。此切片故意不推断密码提示、保留选择或在停用后自动重新获取安全键盘输入。

## 渲染器文档生命周期

Electron Main 拥有渲染器文档生成及其客户端会话、投影和应用程序生命周期控件。 Electron `did-start-navigation` 详细信息是可信的导航输入； Main 在改变这一代之前用一个确定性策略对它们进行分类：

| 主机架 | 相同的文件 | 结果               |
| ------ | ---------- | ------------------ |
| 是的   | 否         | 开始生成渲染器文档 |
| 是的   | 是的       | 无操作             |
| 否     | 否         | 无操作             |
| 否     | 是的       | 无操作             |

渲染器文档开始加载时，会停用上一页面的控制对象并记录待处理生成。`did-finish-load` 只为最新生成发布一个新的原子 bootstrap；退役失败会封闭该生成。Main 只允许在配置的渲染器源内进行顶层导航，使开发重新加载与打包后的浏览器 Router 文档请求能够完成；跨源导航和新窗口仍被拒绝。同文档浏览器 Router 更新与子帧导航保留当前生成及全部活动控制对象。渲染器进程丢失与窗口关闭仍是显式退役路径；Host 终端与规范共享文档在页面退役后继续存在。同一窗口重载时，替换后的 Web 组合会在确认精确 Presentation 仍属于 bootstrap 文档后，独立恢复其有界会话焦点提示；Main 不接收 Client View 或浏览器存储能力。该模型不新增投影重试、控制端口超时或 IPC 契约。

## 原生项目文件夹获取

Desktop 是当前唯一注入 Workbench existing-Folder 采集功能的组合。一个相关渲染器请求要求 Electron Main 打开所属窗口的 `dialog.showOpenDialog(..., { properties: ['openDirectory'] })`。 Main 最多接受该渲染器的一个活动请求，只需要一个选定的路径，将其标准化为绝对路径，并检查它是否仍命名为可读、可遍历的目录。验证遵循符号链接，同时保留用户选择的路径拼写。渲染器无法提交验证路径或请求目录内容。

选中结果会进入现有的 Workbench 项目创建或项目目录添加 Action。取消，以及有界对话框、类型、存在性、权限或校验失败，都不会改变 Project 状态。Workbench 会在变更前重新检查最新添加目标与精确重复路径。渲染器文档退役、重新加载、卸载、端口关闭、窗口关闭和应用处置都会撤销交付，防止延迟完成的原生请求改变替代页面。现有选择为只读。`New Folder…` 使用原生所选父目录凭据与单一名称，只在明确 Create 后原子创建一个原本不存在的目录。后续失败会保留已提交文件夹，观察实际 Project Action 与精确规范发布，并通过既有终端生命周期只进入捕获的 Primary Workspace。受维护的行为、证据、限制与回滚契约见 [`docs/project-folder-acquisition.md`](docs/project-folder-acquisition.md)。

## 托管 Project Worktree 创建

Desktop 是当前唯一注入托管 Worktree 创建能力的组合。Main 从源 Workspace 的当前可用资源 Directory
发现 Git repository，按规范 Git common-directory identity 去重，并返回可编辑的 base 与新分支
默认值。Renderer 只读展示固定托管位置，可以请求可选 Workspace，但不能选择或提交文件系统 path。

打包版 Desktop 使用 `~/.orchbee`；每个开发 checkout 使用
`~/.orchbee-dev-<scope-digest>`，稳定 digest 从该 checkout 的规范源码路径推导。托管 Worktree 位于
`<data-root>/worktrees/<opaque-id>`。本期没有应用级或 Project 级位置设置。Main 在 Git mutation 前
记录恢复意图，原子提交 Workbench Directory 与可选 Workspace；产品失败时（包括来源关闭或重新分组）保留每个已知创建的 Worktree，也保留所有权不明确的结果，而不递归删除用户数据。参见 [`docs/data-root.md`](docs/data-root.md) 与
[`docs/project-worktree-creation.md`](docs/project-worktree-creation.md)。

## 生命周期和身份所有权

维护的身份链是：

```text
Workbench TerminalPresentationId
        │ committed shared-document intent
        ▼
Desktop TerminalBindingId ── durable binding store
        │ recovery reconciliation
        ▼
HostEpoch + CreationId + TerminalId ── Terminal Host authority
        │ visible viewport demand
        ▼
AttachmentId ── one disposable projection capability
```

`TerminalPresentationId` 由 Workbench 创建并停用。 Main 保持对 `TerminalBindingId` 的严格内存中双射；它从不通过生命周期控制公开绑定、创建、主机、终端或附件身份。主机仍然是该过程和 VT 的真实来源。

版本 1 绑定存储包含规范的 BindingId-ordered 集合。每条记录都有唯一的 BindingId、PresentationId、CreationId、有界启动配置文件以及一个持久状态：`planned`、`create-requested`、`retained`、`exited`、`closing` 或 `lost`。保留的身份是唯一的，并且必须重复其所属的 CreationId。该文件被解析为不受信任的输入，通过仅所有者可用的原子替换进行更新，并且在无效或不可读时绝不自动重写。启动环境值绝不进入诊断。每个默认启动都声明 `TERM=xterm-256color`、`COLORTERM=truecolor` 和 Ghostty 兼容身份 `TERM_PROGRAM=ghostty`，并保留 Host 继承的 `NO_COLOR`、`FORCE_COLOR` 和 `NODE_DISABLE_COLORS` 策略。能力声明不覆盖用户的颜色选择；显式 Launch Profile 环境仍可覆盖继承值。受控 UI 夹具在启动 Desktop 及其全新 Host 前清除自动化注入的颜色策略，而不改变生产启动语义。Desktop 不安装 `xterm-ghostty` terminfo，因此 `TERM` 仍保持可移植基线；该兼容身份使应用可以选择 Orchbee 通过固定 Ghostty VT 实现的 Kitty graphics 和 Kitty keyboard 协议，而不声称 Ghostty 应用版本。新的 POSIX 终端还从代码解析的应用资源路径接收 `ORCHBEE_SHELL_INTEGRATION_DIR`：开发环境相对构建后的 Main 入口解析它，打包应用则在 `process.resourcesPath` 下解析它。环境中的值无法替换该路径。Desktop 将自己的 Zsh、Bash、Fish 脚本及一次性 loader 作为包资源提供。profile 选择后，新建且受支持的本地 Zsh、非登录 Bash 和 Fish launch 会自动加载共享集成，而 Main 不读取或修改启动文件；不受支持的启动形式保持不变，Settings 仍公开精确的手动回退命令。运行中、恢复的、Windows 和远程终端不在该细化范围内。参见 [`docs/automatic-shell-integration.md`](docs/automatic-shell-integration.zh-CN.md)。

启动 locale resolver 是一个确定性策略，其输入为目标平台、Desktop 继承的 `LC_ALL`、`LANG` 和 `LC_CTYPE`、Darwin 系统 locale 以及 Darwin 已安装 locale inventory。它只生成由新终端启动记录捕获的环境 delta。在 Darwin 上，该 delta 会显式设置或删除全部三个 locale 变量，因此 PTY locale 不会依赖单独受监督 Host 的环境。已保留的启动记录和正在运行的 PTY 永远不会被追溯修改。

| 输入条件                         | 启动环境决策                                                                                                                                                 |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Windows                          | 不生成 locale delta。                                                                                                                                        |
| 具有有效 UTF-8 locale 的 POSIX   | 保留有效 locale；Darwin 会跨 Host 边界投影全部三个 locale 变量，并在缺少 `LANG` 时补充它。                                                                   |
| 缺少 `LANG` 的 Darwin            | 通过 Electron 解析 `[NSLocale currentLocale]`，仅当 `/usr/bin/locale -a` 报告该精确 locale 时才生成 `<language>_<country>.UTF-8`；否则回退到 `en_US.UTF-8`。 |
| 存在无效高优先级 locale 的 POSIX | 删除冲突的 `LC_ALL`，修复无效的 `LANG`，并替换显式无效的 `LC_CTYPE`；Linux 使用 `C.UTF-8` 作为回退。                                                         |

所有生成的 locale 名称均为 UTF-8；Desktop 显式继承的有效 locale 保持权威，Host 中冲突的高优先级值绝不会进入子进程。Electron Main 在解析启动配置前通过绝对路径的 macOS 系统工具读取一次有界 locale inventory；inventory 不可用、失败或超限时不会接受任何派生候选，因此选择回退值。Darwin 缺少 `LANG` 时的决策只采用固定 Ghostty revision `9c3ec931d64561a8407dde7ac984ce156ae91539` 的 `src/os/locale.zig`；Desktop 用只读 inventory 替代 Ghostty 的进程全局 `setlocale` 验证，并用显式的 Desktop-to-Host locale 投影替代其同进程环境假设。它不采用 Ghostty 的 `LANGUAGE` 翻译偏好。

## 本地终端默认值

Desktop 与 TUI 共享确认的设备偏好。旧终端默认文件只作为一次性迁移输入。先提交共享持久化，再由 Main 向 Host 应用滚动历史；应用失败可见，重试应用不会重放写入。新终端捕获最新 Host 默认值；运行中和恢复终端保持已有配置。参见 [Desktop 设置](docs/terminal-settings.zh-CN.md)。

## 本地终端启动配置文件

共享设备目录在所有平台默认使用关闭登录模式的系统 Shell。
POSIX 用户可以开启登录模式，Windows 不可。自定义可执行文件使用绝对路径与字面参数。
共享存储校验目录边界，具体 Host 启动准入在创建之前进行。旧 userData 目录仅为迁移输入。

每个新终端捕获选定配置，需要时通过匹配辅助程序解析系统 Shell，随后冻结持久具体启动信息。
自定义配置无需系统 Shell。并发创建共享待处理发现；关闭或释放防止后续创建。
重试和恢复绑定保留捕获的启动信息。存储无效与写入失败可见，不覆盖来源，也不回退到其他配置。
参见[启动配置](docs/terminal-launch-profiles.zh-CN.md)。

普通远程 shell 仍然由用户在 PTY 内部调用的系统 SSH 客户端拥有。terminal-launch-profile 路径不会
包装 `ssh`、保留远程凭据或主机记录、添加 OpenSSH 环境转发规则、安装 terminfo 或写入远程缓存。
请求的 SSH PTY 可以携带标准 `TERM=xterm-256color`；仅当用户的 OpenSSH 客户端配置和服务器都允许时，
`COLORTERM`、`TERM_PROGRAM` 和其他额外变量才会跨连接。共享 Settings 表面提供一个常量只读远程
诊断命令，报告观察到的变量，并区分未设置的 TERM、缺少 `infocmp` 程序和缺少 terminfo 条目。只有用户
显式选择在 shell 中运行该命令时，它才会执行；命令本身不包含网络、安装、文件系统或 terminal-input
effect。

终端、绑定或附件都没有产品级数量上限。活动集合随共享产品结构和可见需求增长；分配、文件系统、Host 或内存资源耗尽会返回操作失败，而不会驱逐或静默截断另一个终端。Host 清单通过限定代次的协议分页读取，而不是一个有界全局列表。关联前的 `TerminalExited` 交付使用有界的按身份索引准入缓冲区：已准入身份永不驱逐；尚未获得绑定关联的身份会在适配器释放缓冲区前重试。Host 信封一经接受，Desktop Main 就会在一次性 Host 监听器返回前，把事件同步记录到所有者专用的持久待处理日志。独立的 64 项按身份索引内存队列只是调度器/缓存，而非崩溃恢复的权威所有者。待处理日志有独立的 4096 项容量；可安全跨重启的发布账本与持久拒绝日志也各有独立容量。日志已满或写入失败时会尽可能记录可观察的持久拒绝；若无法记录，准入路径会以失败关闭。未知身份保持持久待处理所有权，不进入定时循环；观察到确切身份后会唤醒晋升。队列已满、关机和已观察的待处理条目可由重试所有者晋升。只有持久运行时记录、适配器发布提交、发布账本准入、协调器接受与实时发布全部成功后，才会移除待处理条目；因此，压缩失败会保留持久恢复状态。关机在完成其余应用拆卸前，会把尚未解决的内存条目转移到待处理日志。

终端退出溢出存储只接受 version-1 的完整 `pending` 与 `rejected` 数组，按身份保留最新修订。旧 `events` 数组及 `published` 标记被严格拒绝，不迁移、不重写。当前发布分类账继续独立证明已发布身份；恢复不依据旧记录猜测发布状态。

`TerminalFailed` 是不同的结果，而不是合成退出。仅当绑定仍在获取其主机身份时，桌面才会保留身份键控关联缓冲区，然后在公开另一个投影之前记录该绑定的失败。发生故障的终端在幸存的主机中仍然保留并具有权威性，因此桌面重新启动可以从主机清单中恢复相同类型的故障；它不会复制到特定出口的溢出存储或发布分类帐中。这并不保证主机或操作系统崩溃后的恢复。

单独的 Workbench 共享文档存储仅通过一个 Main 拥有的序列化 writer 接受规范 version-1 文档。缺失的存储从 revision `0` 开始；之后每次 commit 都必须精确命名当前 revision 并将其推进 1。Conflict、非下一 revision、无效文件与读取失败都会保留当前数据。写入使用 owner-only 临时文件、文件同步、原子替换以及受支持平台上的父目录同步；可能已经越过 rename 的失败会报告为 indeterminate。客户端只接收原子 bootstrap、类型化 proposal result 与有序 publication；它永远不会接收路径、Node error、store capability 或通用 filesystem capability。参见 [`docs/workbench-shared-document-storage.md`](docs/workbench-shared-document-storage.md)。当前 store 只使用 `workbench-shared-document-v1.json` 并严格校验完整当前形状；不读取其他路径，不接纳历史 DTO，也不执行迁移。

在创建终端之前，配置会分配并保留新的绑定。恢复仅接受精确的 HostEpoch/TerminalId/CreationId 匹配，并且绝不会用新的 shell 来替换丢失的连续性。丢失的创建响应可能仅重播相同的 CreationId 并在记录的主机纪元中启动。孤儿状态、冲突状态、腐败账本状态和传输状态无法关闭。

保留协调是一项单独的仅恢复操作。仅当绑定已持久保留或退出，或在同一主生命周期中成功配置后可恢复时，它才会成功。丢失、计划、关闭或丢失的绑定永远不会通过恢复路径分配替换。恢复失败仍然保持持久保留意图。

每个 Presentation 的生命周期工作由一个操作尾部串行化；无关终端可独立推进。Main 协调器会先持久化并发布包含 provision、close 或 stop 意图的完整共享文档，再取得相应效果。FIFO 提案队列对内容不同的并发完整文档采用先写者胜出，并让等价竞态收敛。随后，close 会持久化技术状态 `closing`，只退役该 Presentation 的活动投影，并把 graceful/force/dispose 行为委托给现有绑定关闭生命周期。Panel 或 Tab 会留在 Workbench 中，直至每个精确终端关闭结果完成；随后提交清理。若清理失败，先前已存储的关闭意图仍可重放。渲染器重新加载、渲染器崩溃、正常 macOS 窗口关闭及不受控 Main 丢失都会释放该页面的投影附件，但保留 Host 终端。受控应用退出遵循以下策略。

## 应用生命周期

Desktop 应用构造期间的致命失败会在 Electron 退出前把进程退出状态设为 `1`。普通的单实例拒绝以及正常窗口或受管生命周期退出仍保留成功的进程状态。

启动回滚在清理失败时报告第一个清理失败值，包括 `undefined` 或 `null`；否则保留原始启动错误。
独立清理仍会完成，失败的 owner 保留到后续应用关闭时重试。投影控制退役也在尝试每个独立租约
清理时保留第一个失败值，并保留失败租约供显式重试。这也包括被最终权威检查拒绝的未发布租约：
broker 保留其请求身份直到清理成功，使精确释放能重试该租约而不退役无关投影。
[投影 broker 测试](src/main/terminal-projection-control/__tests__/TerminalProjectionControlBroker.unit.test.ts)
覆盖首个失败身份、跨多个租约的完整重试，以及通过释放或 broker 退役恢复未发布租约的清理，
同时保证被拒绝的渲染器端口不会被传递。

投影服务释放会等待所有已启动的独立租约清理和暂存附件分离结束，再返回成功或失败。
某个清理提前拒绝时会保留首个观察到的失败，但不会在其他清理仍未完成时结束关闭屏障。
失败的 owner 保留供显式重试，成功的清理不会重复。[投影服务测试](src/main/terminal-projection-control/__tests__/DesktopTerminalProjectionService.unit.test.ts)
在一个清理拒绝时保持另一个清理未完成，再验证完成顺序和保留的所有权。

应用关闭和渲染器文档退役会在失败后继续执行各自独立保护的清理步骤，并报告第一个原始抛出值，包括 `undefined` 或 `null`。退役失败时不能发布替代渲染器的 bootstrap；后续重试必须先完成保留的清理。[应用组合测试](src/main/composition/__tests__/DesktopApplication.unit.test.ts) 覆盖失败值保留、独立清理，以及重试时不重复已完成的终端运行时释放。

正常关闭 macOS 窗口只会释放渲染器页面。Command+Q、菜单退出，或关闭 Windows/Linux 上最后一个窗口时，会在渲染器仍可用期间进入统一的用户退出工作流。若一个或多个 Presentation 依据 Workbench 的 Panel 关闭谓词需要保护，应用级对话框会以“保留会话并退出”为默认选项，以“停止会话并退出”为破坏性选项，也可在执行任何停止效果前取消。若观察到每个运行中 Presentation 都处于空闲状态，或已无 Presentation，Desktop 会保留会话、刷新共享文档并直接退出，不显示提示。未来的 updater 会静默进入相同的 `update-restart` 保留准备；updater 安装、签名与发布分发不在本包范围内。

在保留或停止准备之前，Main 确认应用程序范围的协调器已进入静默状态。此确认对生命周期和客户端会话端口进行排序：其他页面无法将新的共享更改竞争到准备窗口中。在前退出所有者断开连接后，替换渲染器只能恢复相同的静止模式。

开发 Host 重启使用独立的非交互静默模式。执行任何停止效果前，渲染器只捕获当前可重启的 Presentation；Main 则跨渲染器文档替换持久化这一精确目标集合。即使共享文档已把它们记录为 stopping 或 stopped，替代渲染器也会从 Main 拥有的目标恢复。该工作流遵循相同的持久 stop 与 Host 排空屏障，以新 provision 标识替换每个已捕获的停止 Presentation，不把该 provision 派发给退役 Host，并刷新暂存文档。只有在精确的暂存修订已加载，且旧 Host 不再拥有任何绑定或未知终端后，Main 才写入 generation-bound ready plan。普通更新重启仍走保留路径，绝不会使用这种破坏性的开发模式。

stop 会封闭退出所有者页面上的终端创建，把每个可见或隐藏的 Workbench Presentation 投影为持久 `stop`，对完整共享文档执行 compare-and-swap，随后才释放关闭命令。成功的关闭结果会把 Presentation 变为 `stopped`，同时保留 Project、Workspace、Tab、Panel、Split、标题及启动 CWD。水合绝不会启动已停止的 Presentation。可见的“开始新会话”Action 会用新标识替换已停止的 PresentationId，并只 provision 所选 Presentation。

当持久 stop 到达最终 Host revision 时，Main 会在终端结果之后、Host Dispose 之前打开一个临时只读附件。它为该 Presentation 归档最新 5,000 行最终渲染行或 10 MiB，以先达到的边界为准。归档格式 `1` 会存储最终 viewport 投影及其完整 OBKG 资源快照，并在每个 render-detail OBRW `0.1.0` 分块中保留独立图形快照；解码器只接受格式 `1` 与精确的当前嵌套格式，不会静默回退。Desktop 本地存储使用带内容校验、私有权限的原子文件，并施加 250 MiB 的全局 least-recently-used 上限。有效归档会在本地 Electron 的 stopped surface 中供现有终端 renderer 使用，保留图形、滚动、搜索、选择与复制，并永久拒绝输入和调整尺寸。归档缺失、损坏、超时、不可写或被淘汰时，会回退到空的 stopped 状态，且绝不阻塞 Host Dispose 或应用退出。归档不会进入共享文档、renderer bootstrap、Browser storage 或 cloud。普通关闭和“开始新会话”会立即移除 archive association；它们的 best-effort store deletion 与 shared-document orphan reconciliation 可能留下一个无法访问的 physical file，供以后 reconcile。归档行与图像绝不会恢复到新 VT 或与其拼接。

被引用的 Presentation 停止后，Main 会加载精确的已提交共享文档修订，并关闭由它拥有但文档未引用的每项绑定。随后要求绑定注册表为空，并扫描 Host 清单。未知 Host 终端会被报告为未解决，绝不会静默终止。部分失败会让 Desktop 进入不可逆恢复对话框，并在受支持时重试，同时显示明确的“仍然退出”警告。“仍然退出”可能让进程继续运行；持久 `stop` 意图会保留到下次启动时的重放权限。本工作流不声明 OS 关机、注销或强制终止行为。

## 需求驱动的投影

Workbench 仅在显示的面板视口内安装应用程序拥有的终端表面。仅当实时 surface 的生命周期为 `running`、`exited` 或 `failed` 时才请求投影；卸载、切换 Tab、隐藏另一个终端上下文或进入 `closing` 会释放请求与 Host attachment。已经绘制的 surface 进入托管 `closing` 时，只会把该 renderer 保留为冻结的不可交互帧，不再拥有 transport、输入、resize、frame stream、metadata 或搜索权。它不会打开替代 attachment，并会在存活 Tab 跨过显示屏障后 dispose；若没有 Tab 存活则立即 dispose。因此，后台 Project、Workspace、Tab、非活动子终端和其他不可见 Panel 不持有实时 renderer、GPU surface、frame stream、history cache、search Worker 或 Host attachment；交接期间至多有一个瞬态退出 Tab 投影保持仅显示状态。

默认可视启动携带空几何。provision 会持久化绑定 Presentation 并在 `awaiting-start` 中创建 Host 标识，但不生成子进程或 PTY。可见的共享 Web 表面会等待配置字体与正挂载的内容框，再依据该精确呈现配置、内边距和文档 DPR 计算完整网格与物理单元格几何。Electron 的投影打开命令保持几何不变。Main 在 `AttachTerminal` 前串行化 `StartTerminal`，校验接受结果中的标识与几何，并且只有精确 Host 转换完成后才把绑定标为已启动。因此，子 PTY、Host Core、首个 Host 帧及渲染器固定的首次呈现均从相同几何开始。不存在超时或 80x24 可视回退。释放尚未挂载或仍在等待的 Presentation 会中止测量；若关闭的 Host 终端仍处于 `awaiting-start`，则直接将其处置，无需 graceful 或 force 终止。立即无头启动与测试组合仍在创建时提供完整几何。

在渲染器文档绘制之前，Electron 使用发送到 Terminal Host 的同一启动配置文件中的默认背景填充本机窗口。该值只是一个启动桥：提交的主机框架元数据拥有 Electron 和浏览器客户端中的实时终端背景，因此 Desktop Main 永远不会维护第二个运行时终端调色板。

每个投影引导程序都包含由生命周期适配器保留的完整的最新主机主题状态。投影代理将附件范围的 `apply-terminal-theme` 命令转发为一个主机比较和设置，并仅发布经过身份检查的完整结果。在渲染器可以使用稍后的引导程序之前，它将结果记录回生命周期恢复状态。协调读取主机清单，需要准确的创建/主机/终端身份，刷新相同的完整状态，并返回专用的协调事件。已知的未改变的故障和未知的传输结果使用特定于主题的结果通道；未知的结果必须在重试之前进行协调，并且永远不会演变成一般的投影失败。 Desktop Main 不渲染或独立选择终端颜色。

Desktop 隐藏本机标题栏，同时保留操作系统的窗口控件。在 macOS 上，Workbench 独占
`Command+W` 作为关闭终端快捷键；Main 保留可点击的本机“关闭窗口”菜单项，但不注册它的
快捷键，因此重复的终端关闭事件或终端关闭动作暂时不可用时，事件不会向下落入并关闭本机
窗口。Main 同时禁用窗口控制覆盖，将本机交通信号灯定位在 `(8, 10)`，渲染器将侧边栏切换
置于保留本机控制空间后的同一 `36px` 水平 Tab 行中。

### 本机窗口拖动区域投影

- **问题与范围：** Desktop 用户必须能从无边框本机窗口 `36px` 顶部镶边中的每个未占用点
  移动窗口，同时不能失去 Tab、组、侧边栏切换、new-Tab 控件或操作系统窗口控件。本投影
  覆盖侧边栏顶行和水平 Tab 栏。垂直 Tab Strip、顶部镶边下方的内容、Panel 与侧边栏交互、
  调整大小分隔符、Tab 跨窗口拖动以及本机移动生命周期均不在范围内。
- **输入、数据与所有权：** 可信的 Workbench 布局提供侧边栏可见性、Tab Strip 方向、水平
  镶边边界和已渲染控件几何；`windowControls` 提供操作系统控件所在的一侧。不存在持久的拖动
  区域状态，也不会推断缺失输入。Workbench 拥有外壳几何，Desktop renderer 独占 CSS app
  region 投影，Electron 与操作系统拥有本机控件优先级以及指针激活、移动、释放和取消。
- **模型与输出：** renderer 在每次布局投影后应用以下命中测试策略：

  | 水平镶边目标                                      | CSS app region | 可观察结果               |
  | ------------------------------------------------- | -------------- | ------------------------ |
  | 未占用的侧边栏顶行背景                            | `drag`         | 开始本机窗口移动         |
  | 未占用的水平 Tab 栏背景，包括保留的前导与尾随空间 | `drag`         | 开始本机窗口移动         |
  | 侧边栏切换位置、Tab、组或按钮几何                 | `no-drag`      | 保留控件的普通指针行为   |
  | 垂直 Tab Strip 或水平镶边以外的内容               | 未设置         | 保留现有的非窗口拖动交互 |

- **不变量与转换：** 本机窗口控件和已渲染 Workbench 控件始终优先于背景拖动区域；Desktop
  拖动投影绝不延伸到 `36px` 边界以下。显示或隐藏侧边栏以及切换 Tab Strip 方向会重新计算投影，
  但不会开始、提交或取消本机拖动。不支持或不存在水平镶边时是有意的空操作。
- **依赖与验证：** Electron 的 CSS app-region 契约是唯一的平台能力。受控 Desktop 系统轨迹
  检查完整侧边栏行几何、水平 Tab 栏默认值、每项交互排除、侧边栏可见性转换以及真实控件激活。
  自动化证据不会移动操作系统窗口或改变其焦点；这仍处于聚合套件范围之外，并位于需要另行授权的
  手动本机窗口边界内。

Desktop 在受信任的主要组合边界处读取一次终端计算机的主机名，将其验证为运行时配置，并将其包含在每个 Electron 客户端会话引导程序中。共享 Web 渲染器将该主机名与提交的主机标题/CWD 元数据相结合，并应用 Workbench 拥有的规范化策略。渲染器输入、浏览器位置和传输对等名称无法替换此主机名。自然标题和当前 CWD 保留每页瞬时观察结果；只有后备和显式自定义选项卡标题才会进入共享文档协调器。

每个成功请求都会获得新的 AttachmentId、完整能力 bootstrap、正数能力修订和私有 `MessagePort`。Desktop 通过 Workbench bootstrap 转发该修订；每条投影的原始或语义输入命令也会把它带回 Host。对于运行中的 Presentation，每个可写附件都获得 read/input/resize 能力；串行化 open 只有在不存在活动 owner 时才请求初始 resize lease。接纳的 `focused: true`、key press/repeat、已提交 text 或 paste、application mouse press 或 terminal-selection start 会把该可写 projection 记录为最新 desired owner；attachment、background state、viewport resize、pointer motion、release/repair input 与 `focused: false` 不会。只有在运行中 terminal 上仍具有 input 与 resize 能力的 attachment 才能产生租约 activity 或发起 claim；input 降级会使其排队的 ownership intent 失效。Desktop 合并并发 intent，一次串行一个显式 Host `ClaimResizeLease`，且不会延迟 input。已提交 transfer 先以旧 owner 的 revoke 到达 Main，再以新 owner 的 grant 到达；晋升的 renderer 立即测量当前 surface 并提交该 geometry。Owner detach 时，Host 晋升 successful claim 最新的剩余 resize-capable attachment，从未 claim 的候选以最早 admission 打破平局；若 Desktop 记录了更新的剩余 activity，则会再次 claim。open、输入能力变更、release 与 retirement 操作按 Presentation 串行化。只有 Main 拥有的投影租约方能请求实时输入变更；Broker 会向 Host 提交精确的下一修订，校验返回的完整附件标识、不可变 read/resize 状态、resize 租约、新输入值与修订，然后才向下游发布状态。重复的目标状态为空操作。渲染器命令不能提升自身能力；无效或不可发布的 Host 结果会废弃附件，而非授予客户端权限。Main 只有在待处理能力工作和分离都完成后才返回对应释放确认。Renderer 本地 release 可能在 Main 观察到该 release 前，与已经发布的异步 open 交错；携带一个 transferred port 且精确关联的迟到 open 只会关闭该 port，并保持共享控制与无关请求可用，而无效标识、port 数量或重复 open 仍会退役共享控制。Host 请求传输仍可用时，清理失败会保留同一请求以便稍后精确重试，且不能使另一个投影请求失效。一旦确认连接终止，Host 连接 teardown 就构成该连接附件的清理证据：Desktop 不再发送 Host 清理请求，并结算连接作用域内的本地租约/分离所有者，而不改变保留的终端。已退出或失败的终端以只读方式附加保留最终修订及类型化结果。多个可见 Split Panel 与页面客户端可以分别持有独立附件，不设全局附件阈值。

每个 `ProjectedTerminalBroker` 还为其附件拥有一个有界调整大小命令序列化器。串行器处于空闲状态或只有一个主机调整大小请求处于活动状态；当处于活动状态时，每个较新的经过验证的渲染器测量都会替换单个待处理的测量，而不是启动竞争的 PTY 调整大小。活动请求的成功或类型化失败使串行器返回空闲状态，并在附件保持活动状态时立即接受最新的待处理测量。活动失败仍然通过现有的类型化渲染器失败路径发布，并且永远不会静默重播；只有在开始后到达的测量才能驱动下一个请求。处置或附件清理会删除挂起的测量，其他渲染器命令保留其现有并发性，并且挂起的调整大小在调度之前不占用主机请求窗口槽。因此，容器测量的突发使用恒定内存，一次最多提交一个调整大小，并使最新的挂起几何图形成为下一个主机请求，而不允许中间请求超越它。

`HostConnection` 只读取并验证一次固定终端帧标头，然后按 AttachmentId 直接路由二进制记录。Main 不执行 VT 解析、行重建、Kitty 图片加载、JSON/base64 转换或逐 cell 处理。在 Electron capability 边界，broker 会在 structured clone 前只把路由后的 payload 复制到精确大小的 `ArrayBuffer`，因此信封头与合并的相邻记录绝不会穿过 renderer port。Renderer 严格解码 OBTF 中的有界 OBKG raw 像素，并通过 Canvas2D 或 WebGPU 绘制全部三个有符号 z 层；该 frame path 不携带 URL、临时文件句柄或共享内存 capability。OBRW format `0.1.0` 本地历史 render buffer 携带自己的 OBKG 快照，并使用 OBPB structural-view 边界。历史、选择、复制、链接和辅助功能行为仍属于附件作用域。正常 bottom-following 使用不会创建历史缓存或 search Worker；这些资源只由既有交互合同激活。

相关主机 `TypedFailure` 仍为类型化 `HostRequestError`； `HostConnection` 保留了响应相关性 ID，而 `ProjectedTerminalBroker` 添加了确切的渲染器命令操作，而无需替换主机代码、消息或可重试性。失败的行读取还会保留其渲染器请求序列，以便只有历史记录飞行才能解决。客户端拥有的复制和链接效果使用其请求序列。 Electron 转发相同的有界故障合约，并且代理不会重试该诊断身份的用户效果。

Electron 附件只请求协议字面量 `plain` selection export，并通过 Main 的剪贴板提交解码后的 UTF-8 文本。投影命令与平台 effect 都不公开 HTML 或 raw VT format selector。所有 composition 都通过 Electron 或本地 Browser gesture 打开 allowlisted `http`、`https` 或 `mailto` 目标。Desktop-effect 附件还会获得一项可选 local-file capability；Browser 永远不会收到通用 Electron effect port。附件传输背压只会使该附件退役；可见表面会请求新 projection 与完整 snapshot。

该 local-file capability 只接收已经从 generation-pinned 终端文本派生出的 typed file target。Main 把路径视为不可信输入：展开开头的 home marker，要求路径为不含 control character 的绝对路径，并在委托给 Electron 默认应用前验证显式 open target 当前确实是普通文件或目录。显式打开失败会返回有界 typed link failure，供 renderer 显示本地化结果。该 capability 不枚举目录、不解析 Browser 路径，也不会成为通用 preload API。

图片预览边界更窄。Main 会以 read-only、nonblocking 方式打开请求路径，验证已打开句柄是非空、大小不超过 8 MiB 的普通文件，精确读取这段有界内容，并要求 PNG、JPEG、GIF、WebP、AVIF 或 BMP 的扩展名与 magic signature 一致。SVG、HEIC、TIFF、未知、签名不匹配、超限、缺失与非普通目标只能由外部程序打开或返回 unavailable。每个 broker 最多保留一个 current preview request；replacement、显式 cancellation、attachment cleanup 与 disposal 都会让迟到 completion 失效。准入 bytes 只会以精确 `ArrayBuffer` 穿过该 attachment port，不缓存、不持久化，也绝不会进入 archive。预览 unavailable 保持静默，而用户主动打开失败仍会显示本地化反馈。移除可选 attachment capability 即可同时回滚文件激活与预览，且不改变 URL 行为。

意外的共享主机连接关闭会在请求清理之前向每个仍处于活动状态的附件发布一个可重试的 `host-connection-unavailable` 故障。每个可见表面仅冻结其最后提交的投影并进入相同的附件本地恢复路径；它不会等待另一个用户命令来发现主机传输已经结束。

主机承认的 OSC 52 纯文本写入与用户启动的选择复制不同。 Main 路由有界附件事件而不解释其文本。共享渲染器会自动处理确切的请求。桌面效果附件将单一特权写入委托给主程序；客户端效果附件首先调用其已经限定范围的平台剪贴板效果并报告本地写入是否完成。附件停用、渲染器替换或不可用的客户端本地效果不会产生延迟写入或重复写入。 Main 和渲染器都不会保留请求文本或信任决策；主机传送到一个附件意味着另一桌面页面在切换后无法接收相同的请求。在允许一个本地视觉表面提示之前，通过共享工作台生命周期中的不透明终端身份和主机序列来消除重复的响铃传送。 Desktop 目前既不注册单独的选择加入声音效果，也不注册单独的选择加入操作系统通知效果，因此默认情况下，Bell 既不请求窗口焦点、音频也不请求系统通知。 OBTF 进展仍然安静的演示元数据。语义命令完成是分开的：在用户启用默认关闭设备本地策略后，渲染器可以呈现本地化的终端状态并使用其页面本地通知功能。 PTY 输出无法请求通知权限，并且完成负载不包含命令文本。

已知主机 `TerminalExited` 事件进入 Main 拥有的重试/协调路径，然后通过权威运行时注册表进行解析，保留到确切的绑定，并由 PresentationId 发布到工作台。仍然无法解析的身份将保留为持久的、可观察的协调记录，并且仅当观察到其确切身份（或通过可重试的已知条目路径）时才会升级；它不会在无限的未知身份计时器循环中旋转。恢复的退出绑定只会恢复到 Workbench，直至其经过身份检查的 bootstrap 事件进入 managed 自然退出 removal 协议。在此期间最终修订仍可读取；随后产生的 close intent 会在不终止进程的情况下处置 Host 终端，再由 Workbench 按 Panel 拓扑移除或提升精确的 Presentation。协调器预留、适配器提交、发布分类账记录和可见事件是单独的步骤：只有在所有持久和表示所有权步骤成功后，引导程序才能看到事件。在第一个退出发布被消耗之前确认耐久性。

Workbench `terminal-presentation-event-published` 消息是 Main 拥有的单一生命周期发布。类型化主机故障变成不可重试的 `terminal-failed` 呈现结果；协调器保留它以供以后的客户端引导，而工作台使终端保持可投影和只读状态，直到显式关闭为止。 `ProjectedTerminalBroker` 还可以转发身份检查、附件本地原始 `terminal-exited` 或 `terminal-failed` 通知，以便实时投影可以与主机状态同步；该渲染器传输有自己的单调修订过滤器，位于协调器序列之外，并且不是第二个工作台发布。因此，这两个通道不提供跨通道的一次性声明。

以下稳定生命周期见证把跨所有者兼容性名称映射到现有所有者测试和模型。单元/模型行控制注入传输、时钟、存储、标识和故障。精确有界面行使用普通 Desktop Playwright 套件并声明平台限制；任何一行都不授权单独的会抢焦点的手动原生窗口测试。

| 稳定见证                                                                                                         | 可执行所有者证据                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `terminal-parity/LC-01-open-lifecycle`                                                                           | [`../../packages/terminal/src/projected/__tests__/projected-terminal.unit.test.ts`](../../packages/terminal/src/projected/__tests__/projected-terminal.unit.test.ts)、[`../web/src/workbench-client/projected-terminal-renderer/__tests__/startProjectedTerminalRenderer.unit.test.ts`](../web/src/workbench-client/projected-terminal-renderer/__tests__/startProjectedTerminalRenderer.unit.test.ts) 及 Desktop 生命周期适配器测试执行打开、回滚、同实例重试、重复打开、处置和 DOM 所有权。                                                           |
| `terminal-parity/LC-02-failure-presentation`、`terminal-parity/LC-10-outcomes`、`terminal-parity/LC-11-recovery` | Web 投影表面/渲染器测试和 [`../web/src/workbench-client/projected-terminal-surface/model/__tests__/ProjectedTerminalRecoveryController.unit.test.ts`](../web/src/workbench-client/projected-terminal-surface/model/__tests__/ProjectedTerminalRecoveryController.unit.test.ts)，加上 Desktop 恢复、自然退出、渲染器重载和 Main 重启轨迹，执行失败/退出/停止/重连优先级、末帧保留、有界自动重试、手动重试和本地化可操作状态。                                                                                                                            |
| `terminal-parity/LC-03-dual-client`、`terminal-parity/LC-09-client-isolation`                                    | 受控的[会话协调器](src/main/client-session/__tests__/DesktopClientSessionCoordinator.unit.test.ts)和[投影服务](src/main/terminal-projection-control/__tests__/DesktopTerminalProjectionService.unit.test.ts)测试覆盖独立会话、规范发布、限定作用域的附件及处置。这些名称目前没有对应的 Electron 加认证 Browser 有界面测试；Browser Preview 是独立内存组合。                                                                                                                                                                                             |
| `terminal-parity/LC-04-environment`                                                                              | [`src/main/composition/__tests__/resolve-desktop-runtime-configuration.unit.test.ts`](src/main/composition/__tests__/resolve-desktop-runtime-configuration.unit.test.ts) 覆盖每个平台分支和继承 locale 决策；精确 POSIX 有界面见证先为受监督 Host 提供冲突的环境 locale，再通过真实 Desktop → Host → PTY 路径观察导出的 UTF-8 `LANG`、有效字符集、`TERM`、`COLORTERM`、`TERM_PROGRAM`。跨平台 `terminal-parity/LC-04-color-policy` 见证提供显式继承的颜色策略，并验证三个值均原样到达 PTY。这些有界见证不声明 Windows locale shell 或自动远端环境转发。 |
| `terminal-parity/LC-06-metadata-bridge`                                                                          | 精确有界面 OSC 标题/当前目录见证、Workbench 元数据解析测试、Desktop 客户端会话测试及 受控实现测试 执行帧关联、自然/自定义标题策略、后台交付、过时拒绝及新会话清理。                                                                                                                                                                                                                                                                                                                                                                                     |
| `terminal-parity/LC-12-feature-failures`                                                                         | Desktop Host/Broker 错误测试、Workbench 客户端合约测试及投影渲染器/表面测试执行精确操作/关联标识、可重试性、传输保留、终端控制封闭和本地化用户后果。                                                                                                                                                                                                                                                                                                                                                                                                    |

重点跟踪范围包括：

- 严格的客户端会话和投影边界验证、格式错误的消息退出、精确的提案关联以及页面附件处置；
- 原子引导、FIFO 完整文档提案、第一作者获胜、使用 Client View 保存进行过时回滚、无效文件保存和一个有序发布流；
- 首次提交和每命令预写排序、Main 拥有的仅恢复恢复、失败存储效应抑制以及效应后托管容器清理；
- 新配置、多绑定存储保存、恢复、退出恢复到处置、孤儿和冲突处理、持久关闭和表示注册表重建；
- 不相关的终端并发、精确关闭定位、保留原生窗口的自然退出 Tab 移除、Tab 关闭结果排序和过时结果拒绝；
- 两个恢复的 Workbench Tab，其中两个生命周期记录一致，但只有活动可见 Tab 具有 attachment，随后执行 Tab 切换时的精确 release/open 行为；单独的受控 trace 会驱动 80 次快速第一/第二 Tab 快捷键、通过新 attachment 证明后续输入、关闭活动第二 Tab，并逐帧采样 presentation，断言请求局部的控制存活且顶层从不为空；
- 确切的 `terminal-parity/SU-16-startup-geometry` 真实 Electron/主机证据：受控 POSIX shell 在将控制权交给 `/bin/sh` 之前记录其 PTY 大小，并且第一个子几何体与独立派生的挂载网格和唯一的第一个客户端框架匹配，无需中间`80x24` 框架；所有者本地跟踪涵盖无子延迟创建、附加前启动排序、重复/冲突启动、失败启动和待处理处理。系统见证仅是 POSIX，不会声明 Windows ConPTY 启动或以后的大小调整；
- 精确的 `terminal-parity/SU-18-font-size-shortcuts` 真实 Electron/Host 证据：平台 `Mod` 增大、减小和重置组合键会按产品两点步长更新两个已挂载 Panel 终端、恢复默认值，并保留两个 attachment 身份。该见证只覆盖当前运行器平台，不声明已排除的小键盘或 Shift 加号别名；
- 明确的块/非闪烁配置文件默认值、闪烁条 DECSCUSR 覆盖和 RIS 恢复的精确 `terminal-parity/SU-13-cursor-defaults` real-PTY 投影证据。主机运行时测试还涵盖应用程序重置和非默认配置文件；系统见证人观察规范的投影元数据，而不是将平台屏幕截图视为光标权限；
- 并发查看者只有一个调整大小所有者、last-meaningful-activity handoff、recency fallback、动态渲染器所有权以及序列化发布/开放排序；
- deferred-Host 调整轨迹大小，证明每个附件有一个活动请求，中间测量的恒定空间替换，以及结算后立即发送最新的几何图形；
- 精确的 `terminal-parity/IN-10-read-only` 能力证据，覆盖 Host 提交的实时降级/升级、即时客户端修复、只读本地输出交互、不重放陈旧输入，以及分离前串行化变更；
- 精确标题为 `terminal-parity/IN-14-shared-focus` 的证据，包含两个投影附件各自的焦点声明、一个共享终端聚合焦点区间、只修复目标附件的可见性，以及激活时不重放声明。该夹具控制应用层焦点，不要求操作系统多窗口焦点；
- 精确标题 `terminal-parity/PR-11-bell`、`terminal-parity/IN-07-pre-input`、`terminal-parity/IN-09-action-scope`、`terminal-parity/IN-11-context` 和 `terminal-parity/IN-12-actions` 提供 Canvas 证据，覆盖共享铃声协调器、无按键应用鼠标移动上报期间的历史与选择保留、窗口失焦折叠显式辅助技术来源无障碍 Range 时的 Host 选择保留、containing-Workbench 操作范围、固定/重新验证的语义上下文、同步可用性、类型化执行和本地化反馈。捕获的效果不声明本机音频、通知、URL-handler 或剪贴板传送；
- 精确的 `terminal-parity/IN-08-drop` protected file-marker admission、Desktop backing-path
  resolution、有界 Ghostty-compatible POSIX escaping、直接插入 line middle，以及不打开 review
  或追加隐式 Enter；
- 精确的 `terminal-parity/FS-05-file-links`、`terminal-parity/FS-06-url-policy` 与
  `terminal-parity/FS-07-keyboard-hints` 浏览器组合见证，覆盖禁用文件系统激活、共享的
  `http`/`https`/`mailto` 检测/悬停/上下文/激活边界，以及可见 URL/路径/文件位置/hash 标签的
  精确插入、捕获 URL 打开、取消与标签按键零终端输入；
- 精确的 `terminal-parity/FS-19-local-file-links` 与 `terminal-parity/FS-20-image-preview`
  local-capability 见证，覆盖 relative-path resolution、modifier activation、300 ms dwell、
  replacement cancellation、stale-completion rejection 与生产 pointer-transparent preview card。
  它们使用 fake opener 与合成图片 bytes，不会自动操作操作系统应用；
- 精确的 `terminal-parity/FS-08-command-output-viewer` 与
  `terminal-parity/FS-09-scrollback-viewer` 浏览器组合见证，覆盖生产共享不可变 terminal-text viewer、
  精确保留快照、可见顶部锚定、有界搜索、按来源复制 Action、焦点恢复与不变的 attachment identity；
- 精确的 `terminal-parity/FS-10-active-tab-movement` 与
  `terminal-parity/FS-11-tab-title` 浏览器组合见证，覆盖 固定分区内活动 Tab 循环移动、
  稳定显式 Tab 标题定位、预填自定义标题编辑、在自定义值下保留动态标题、空值重置为最新自然标题、
  Action 搜索发现、焦点恢复与不变的终端输入标识；
- 精确的 `terminal-parity/FS-12-panel-resize` 浏览器组合见证，覆盖 调整模式入口键与
  `W`/`N`/`T`/`S`/`R` 词汇、Control 修饰的大步幅、最近匹配分割轴变更、整个标签页的等权重重置、
  菜单与 Action 搜索发现、陈旧目标关闭、焦点恢复、调整期间不变的 attachment identity，以及标签页
  交接前后稳定的 terminal/creation identity；
- 精确的 `terminal-parity/FS-13-directional-panel-movement` 浏览器组合见证，覆盖 方向邻居
  选择与嵌套四 Panel 布局中的叶节点交换、全部四个移动方向、外侧边缘事件透传、移动源焦点恢复、
  不变的 Panel/presentation/terminal 标识，以及 Action 搜索与设置发现；
- 精确的 `terminal-parity/FS-14-panel-equalization` 浏览器组合见证，覆盖三个嵌套列上的
  同轴 Panel 单元比例语义、菜单与 Action 搜索发现、可配置快捷键呈现、可用与不可用状态下的事件
  阻止、稳定的 Panel 顺序、终端标识、attachment 标识及输入焦点；
- 精确的 `terminal-parity/FS-15-current-directory-tab` 真实 PTY/Electron 见证，覆盖可信 OSC 7
  当前目录投递、终端上下文菜单与 Action 搜索发现、可配置快捷键分发、新 Tab 中精确继承的 `pwd`，
  以及不变的源终端标识、session、焦点恢复和 Settings 投影；
- 精确的 `terminal-parity/FS-16-visible-panel-selection` Browser 组合见证，覆盖双 Panel 直接聚焦与
  交换、三 Panel 排除源的标签、无效键隔离、仍按修饰键的物理选择、主指针交换、Action 搜索与
  Settings 发现、源/目标焦点恢复、规范叶节点交换，以及不变的 Panel、终端 Presentation、creation
  与 Host terminal 标识；
- 精确的 `terminal-parity/FS-17-panel-position-focus` Browser 组合见证，覆盖规范深度优先 Panel
  编号、物理 `Ctrl+Shift+1/2/3/0` 分发、当前位置目标消费、缺失位置与最大化布局透传、聚焦
  textbox 恢复、稳定的 Panel/Presentation/document 标识，以及位置一与位置十的 Settings 发现；
- 精确的 `terminal-parity/FS-18-panel-broadcast-input` Browser 组合见证，覆盖一个不可变的三 Panel
  目标集、恰好一次的已提交 IME 文本、粘贴排除、串行 FIFO 文本、未修饰 Escape 投递、可见性与
  结束控件、可见目标 shield，以及会停止且不重试或重放的非原子部分投递；
- 精确的 `terminal-parity/FS-19-workbench-region-focus` 真实 PTY/Electron 见证，覆盖通过修饰键在活动终端、
  Tab Strip 与可见侧边栏之间正向及反向循环、精确的键盘 Tab 菜单进入与焦点返回、稳定的
  presentation 与 attachment 标识，以及往返后仍可用的实时 PTY 输入；
- 用于每次操作安全上下文、瞬时用户激活和 API 检查的精确标题 `terminal-parity/LC-05-lan-capability` 浏览器组合证据；同步复制可用性；一位被录取的作家来电；拒绝而不重试；局部故障；并且没有特权 insecure-LAN 路径；
- 确切的 `terminal-parity/HS-07-selection-export` real-PTY/Electron 证据表明，样式化终端选择以精确可见的纯文本形式到达平台剪贴板，与不公开 HTML 或原始 VT 格式的协议、主机和投影命令回归配对；
- 精确的 `terminal-parity/IN-03-image-paste` 真实 PTY/Electron 证据证明，空闲的默认粘贴组合键会保留原生仅图片事件，并向子进程精确发布一个 `Control+V` 字节，且不产生剪贴板文本 Action 或失败横幅；
- 精确的 `terminal-parity/IN-13-selection-copy-paste` real-PTY/Electron 证据表明，活动选择后立即依次复制和粘贴时，会等待 Main 拥有的复制提交，读取新选择文本而不是故意安装的陈旧剪贴板哨兵，通过共享粘贴路径清除选择，并在不手动恢复焦点的情况下把复制 token 追加到实时终端输入；
- 精确的 `terminal-parity/IN-18-native-selection-paste` real-PTY/Electron 证据表明，活动的普通指针选择会让隐藏 textarea 保持聚焦，且不会创建非折叠的终端 DOM Range；随后空闲的默认原生 Paste 无需手动 Copy 或焦点修复，即可清除 Host 选择并追加预存的剪贴板载荷；
- 固定标头帧仅路由到匹配的 AttachmentId，并具有精确大小的结构化克隆有效负载隔离；
- 完整的主题引导、身份范围的应用/协调转发、生命周期刷新、未知结果保存以及精确的 `terminal-parity/SU-09-theme` 持有/释放证据，不会暴露混合应用程序/终端修订；
- 精确标题为 `terminal-parity/CF-03-visual-environment` 的证据，使用仓库产品字体，在生产渲染器能力边界强制选择 Canvas 2D，并固定 `900x500` 视口、DPR `1`、`en-US`、UTC、浅色配色方案及减少动态效果。它断言精确语义状态，不执行像素容差标准化；它不是 WebGPU、跨浏览器、原生 DPI 或操作系统字体光栅化的黄金基准；
- 沙盒端到端输入、输出、历史记录、搜索、选择、复制、链接、退出、渲染器重新加载和 Electron Main 重新启动；
- 开发和打包的 Shell 资源解析、资源存在和隐私检查、真正的 Zsh OSC 133 输出以及一个 macOS Electron 从手动设置策略到长命令完成提醒的路径，无需替换预计的附件；
- 精确的 `terminal-parity/SI-02-prompt-navigation` real-PTY/Electron 证据表明，三个显式 OSC 133 提示符行会到达生产 Host 投影，并且重复按下 `Ctrl+Shift+Z`、再按下 `Ctrl+Shift+X` 会在两个方向上选择最近的行，而不替换附件。该见证不声明自动注入 Shell Integration；
- 精确的 `terminal-parity/SI-03-automatic-shell-integration` real-PTY/Electron 证据表明，全新的
  本地 Desktop Zsh 会保留用户 `.zshenv`、`.zprofile`、`.zshrc` 与 `ZDOTDIR` 状态，清除一次性
  marker，并在无需手动设置或替换附件的情况下发出命令完成事实；
- 精确的 `terminal-parity/HS-11-keyboard-scrollback` real-PTY/Electron 证据表明，
  `Ctrl+Shift+Arrow`、`Ctrl+Shift+PageUp/PageDown` 和 `Ctrl+Shift+Home/End` 会让一个生产保留历史
  视口逐行、按重叠页面、到顶部和到底部移动，且不会替换附件。该见证不声明 次要别名、
  备用屏幕行为或键盘平滑动画时序；
- 精确的 `terminal-parity/HS-12-sustained-output-interaction` real-PTY/Electron 证据表明，
  在每 2 毫秒原地重绘状态行期间，大、小滚轮增量都能进入并继续移动保留历史；细粒度
  滚轮进度与 raw-TTY 输入确认均保持在 150 毫秒以内；已捕获的拖拽在一次匹配但不带按钮位
  的中间移动后会保持到显式释放，选中单词会到达平台剪贴板，终端 revision 继续推进，且
  不会发布历史失败；
- 受控的独立 ClientSession 与附件轨迹，覆盖规范发布、断开、拒绝和限定作用域的处置；Browser Preview 与 Desktop 运行时保持独立；
- 用户退出保留/取消/停止、空闲运行终端的自动保留、面板关闭保护的精确重用、隐藏终端停止、failed-CAS 抑制、部分停止重试、未引用绑定耗尽、unknown-Host-orphan 拒绝、更新重新启动保留、开发主机停止/阶段/就绪排序以及显式启动停止会话后的新身份。

[生命周期适配器测试](src/main/terminal-document-control/__tests__/DesktopTerminalLifecycleAdapter.unit.test.ts)
覆盖精确重试标识、仅恢复准入，以及 provision/close 串行化。
[投影服务测试](src/main/terminal-projection-control/__tests__/DesktopTerminalProjectionService.unit.test.ts)
覆盖按测量尺寸先 start 后 attach、start 失败时抑制附件，以及并发打开串行化。

Desktop 保留序列化共享文档 CAS 与投影终端 IPC 的形式化核心。文档存储核心保持双 Client、修订至 2 的转换系统与性质，将提交/修订检查表达为前后状态动作性质，不再保留决策历史变量。这消除了冗余历史状态，没有删除提交结果。生命周期、binding 恢复、write-ahead、resize lease 及 Folder/Worktree 流程使用各自参考文档指定的确定性和生成实现测试。这些测试覆盖声明的轨迹，不穷举 Desktop 流程的笛卡尔积。

投影服务套件还控制并发 resize 申领、活动合并、只读降级、detach 后备，以及 release/open 顺序。
[Broker 测试](src/main/broker/__tests__/ProjectedTerminalBroker.unit.test.ts)检查一个活动 resize 加最新待处理测量、
类型化失败结算，以及清理时丢弃待处理 geometry。这些是实现轨迹，不是保留的 resize 工作流模型。

[归档捕获测试](src/main/terminal-archive/__tests__/TerminalArchiveCaptureService.unit.test.ts)覆盖最终历史捕获、
字节限制、截止期限和迟到租约释放。[归档存储测试](src/main/terminal-archive/__tests__/terminal-archive.unit.test.ts)
覆盖损坏、驱逐和孤儿文件协调；[归档投影测试](src/main/terminal-archive/__tests__/ArchivedTerminalProjectionService.unit.test.ts)
覆盖只读历史/搜索/复制及归档缺失后备。[Workbench stop 测试](../../packages/workbench/src/workbench/model/__tests__/terminal-session-stop.unit.test.ts)
覆盖产品结构保留、水合时不隐式重启，以及新 Presentation 标识。这些有限实现用例不穷举归档或应用 stop 的全部交错。

## 开发

打包的 observation Capsule 将 README 文件纳入不可变 payload 摘要。其中对已退役流程模型的引用描述该打包版本；当前 observation 证据由 [Agent 观察入口](docs/agent-observation-ingress.zh-CN.md)维护。更新验证组合不重新发布 Capsule。Desktop 仅保留 document-store 和 projected-terminal IPC 的 TLA 协议；局部流程正确性由所属实现测试套件覆盖。

Windows 单元验证对进程密集的 fixture 最多使用四个 Vitest worker。POSIX socket、login-shell、permission-mode 与 `/bin/sh` 语法见证在 POSIX 主机运行。可移植子进程与 ACK 协议检查也在 Windows 运行，并单独验证 Desktop observation ingress 的禁用边界。托管 Desktop Agent 集成仍仅支持 POSIX。

在 Linux 上，Desktop 的必需 shell 集成测试需要 `/bin/bash` 处的 Bash 和 `/bin/zsh` 处的 Zsh。
运行单元测试或根合并门禁前须安装两者；Linux CI 会显式安装 Zsh。

无需 workspace build artifact 或已安装的 Electron executable，即可检查 Rsbuild 配置。开发依赖与 Electron 解析在开发启动 hook 内加载。

`pnpm desktop` 拥有一个开发运行时，其中包含 Web 开发服务器 Electron 和受监督的 Terminal Host。运行时遵循以下更新策略：

| 成功的改变                            | 发展行动                                                                                                          | 保存状态                                              |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------- |
| Web 渲染器或源构建的前端包            | 通过Web开发服务器重新加载渲染器文档                                                                               | Canonical Workbench 文档、桌面绑定、主机终端          |
| Electron Main、预加载或源构建的依赖项 | 刷新渲染器文档，重新启动Electron，并重新连接到同一主机                                                            | Canonical Workbench 文档和主机终端                    |
| Terminal Host Zig 源码                | 将每个正在运行的 Presentation（呈现）作为新的持久 provision 进行暂存，停止旧会话，更换主机，然后重新启动 Electron | 项目、工作区、选项卡、面板、拆分、标题和启动 CWD 拓扑 |

Electron 或 Host 编译失败不会影响最后运行的进程。只有新二进制文件完成，且旧渲染器已持久暂存新的终端 provision 后，才开始替换 Host；暂存路径不可用时会中止替换。重启后的 Presentation 会获得新的 Workbench Presentation、Desktop 绑定、Host、创建、终端和附件标识；其进程、shell 状态、瞬态元数据、选择与回滚缓冲会被刻意丢弃。该策略仅用于开发：打包更新后的重启仍会保留 Host 中的保留终端。

桌面开发运行时拥有一个序列化更新工作流程。替换过程中到达的较新的成功构建将被合并并随后应用；即使连续观察看到相同的源摘要，每个更改的主机二进制文件也会被接纳。捕获的二进制文件仅限于活动代、回滚代和挂起代，并且关闭 Rsbuild 服务器会阻止所有重新启动。确定性运行时测试自己的命令顺序、适配器故障、中止准备、更新合并、工作区清理和捕获的二进制不变性。强制操作系统终止、编译器正确性和跨主机进程的 PTY 迁移仍然不在这些声明范围内。

Host 源摘要除原生源码与构建文件外，还包含两个共享 Local Site Runtime PowerShell 启动资产。
Zig 已登记的复制输入会在任一脚本修改时唤醒既有构建监听；Desktop 监听器观察产生的二进制文件。
缺少共享资产会使观察失败，不会将旧启动字节视为当前代次。

开发 Host 构建监听器会关闭文件监听器、终止受监督的构建，并排空待处理的观测工作，即使前面的步骤已经失败。它报告第一个原始失败值；[构建监听器清理测试](src/development/__tests__/NodeHostBuildWatch.unit.test.ts) 覆盖同步监听器失败和后续异步终止失败。

开发运行时仅从受监督的 Terminal Host 环境中删除继承的 `PAGER` 和 `GIT_PAGER` 覆盖。因此，交互式 shell 选择其配置的或默认的寻呼机，而 Electron、构建过程、打包启动和调用开发人员环境保持不变。

在 POSIX 上，开发 supervisor 会直接在 `/tmp` 下创建一个随机、仅所有者可访问的工作区。该受控路径使完整端点保持在 Darwin 的 Unix 套接字路径名上限以内；进程内本机适配器仍会验证禁止跟随的目录描述符、所有者、模式、ACL、端点和实际对端凭据。打包产品集合点仍位于上述受信任的每用户运行时位置下，绝不使用该开发工作区。

一起启动规范的 Web 开发服务器和桌面：

```sh
pnpm desktop
```

`pnpm desktop` 构建共享依赖与 Web 资源，然后运行 Web 开发服务器及 Desktop 拥有的 native/Electron 开发生命周期。Electron 默认使用 `http://127.0.0.1:3000/`；`ORCHBEE_DESKTOP_RENDERER_URL` 可选择显式 loopback Web server。

在 macOS 上，该命令会在 `apps/desktop/out/development-runtime/` 下物化并临时签名一份
content-addressed `Orchbee Dev.app`，再启动其精确可执行文件。它的应用名称、checkout 专属 bundle
identifier、Electron 默认图标和 checkout 专属数据根目录均与已安装的正式应用隔离。开发诊断必须定位
该精确 bundle path 或 identifier，绝不能回退到 `/Applications/Orchbee.app`。

### 分发 Profile

运行时身份和描述文件解析由 `src/distribution/desktop-distribution-profile.ts` 负责；仅供打包使用的参数解析、描述文件编码和图标生成位于 `src/distribution/tooling/`。两者复用相同的应用名称校验。这些直接执行源码的命令需要 Node 原生类型擦除能力（Node 22.18 及更新版本默认启用）。通过 Node 执行的脚本遵循 [TypeScript 模块契约](../../docs/zh-CN/TYPESCRIPT.md#模块语法)，使用显式 `.ts` 路径导入源码模块；不生成产物的 Desktop 类型检查启用 `allowImportingTsExtensions`，分发测试包含真实 Node 导入见证。`@electron/packager` 和 `node-api-headers` 均为开发依赖。

Desktop 只拥有以下三种显式 distribution profile：

| 命令                           | 应用                | Bundle identifier                          | 数据根目录                      | 图标              | 输出                        |
| ------------------------------ | ------------------- | ------------------------------------------ | ------------------------------- | ----------------- | --------------------------- |
| `pnpm desktop:package:dev`     | `Orchbee Dev.app`   | `com.electron.orchbee.dev.d<scope-digest>` | `~/.orchbee-dev-<scope-digest>` | Electron 默认图标 | `apps/desktop/out/dev/`     |
| `pnpm desktop:package:local`   | `Orchbee Local.app` | `com.electron.orchbee.local`               | `~/.orchbee-local`              | Electron 默认图标 | `apps/desktop/out/local/`   |
| `pnpm desktop:package:release` | `Orchbee.app`       | `com.electron.orchbee`                     | `~/.orchbee`                    | Electron 默认图标 | `apps/desktop/out/release/` |

`ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME` 可以为 `pnpm desktop` 和
`pnpm desktop:package:dev` 覆盖默认的 `Orchbee Dev` 名称。
`ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME` 可以独立为 `pnpm desktop:package:local` 覆盖默认的
`Orchbee Local` 名称。每个值都必须为 1–64 个字符，不能有首尾空白，且不能包含路径分隔符、控制字符或
平台无效的文件名字符。所选名称会持久化到对应的 distribution descriptor，使打包应用在没有该 shell
变量时仍保留该名称。两种覆盖均不会改变所属 profile 的 bundle identifier 或数据根目录；`release`
profile 会忽略这两个变量。

```sh
ORCHBEE_DESKTOP_DEVELOPMENT_APPLICATION_NAME="Orchbee Dev Canary" \
pnpm desktop:package:dev

ORCHBEE_DESKTOP_LOCAL_APPLICATION_NAME="Terminal" \
pnpm desktop:package:local
```

`pnpm desktop` 使用 `dev` identity，但不创建打包应用。打包需明确选择 dev/local/release profile。每个命令构建 Terminal Host、当前平台 endpoint adapter、Web 资源与 Main/preload bundle。Host 与 native endpoint 资源保留在 `app.asar` 之外；Shell integration 和 Agent observation adapter 保持原有归属。打包独立于当前原生 CLI/TUI 的主要功能开发流程。

三条 package command 都写入 `apps/desktop/out/` 下各自的 profile 专属目录。它们都不执行 Electron
code signing、notarization、installer creation、publication 或 update distribution。

`ORCHBEE_TERMINAL_HOST_BINARY` 选择绝对路径的开发/测试 Host binary。`ORCHBEE_TERMINAL_SCROLLBACK_BYTES` 选择启动 scrollback 预算，十进制范围为 `0` 至 `1000000000`，默认 `10000000`。`ORCHBEE_DESKTOP_RUNTIME_DIRECTORY`、`ORCHBEE_DESKTOP_ENDPOINT_ID` 和 `ORCHBEE_DESKTOP_RECOVERY_DIRECTORY` 隔离受控开发与系统测试状态。POSIX runtime 保留产品范围的可信用户位置及 native endpoint-security 验证。

`pnpm desktop` 在内部设置 `ORCHBEE_TERMINAL_HOST_SUPERVISED=1`，以便 Main 仅重试开发运行时拥有的主机进程，并为生成准备握手提供仅所有者的 `ORCHBEE_DESKTOP_DEVELOPMENT_RESTART_PLAN` 路径。这两个变量是私有开发组合控件，不支持产品配置或生产更新机制。

从仓库根目录运行包检查：

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/web build
pnpm --filter @orchbee/desktop build
pnpm --filter @orchbee/desktop test:system
```

### 手动本机窗口测试

`test:system` 从每个聚合、自动、CI 和代理启动的运行中排除 `@manual-native-window` 测试。该测试会改变操作系统焦点，同时验证最小化、激活、最大化、全屏和重新启动行为。仅在人类明确批准该精确测试后才运行它：

```sh
ORCHBEE_DESKTOP_NATIVE_WINDOW_TEST_APPROVAL=I_ACKNOWLEDGE_FOCUS_INTERRUPTION \
  pnpm --filter @orchbee/desktop test:system:manual:native-window
```

AI 代理不得调用命令或设置批准变量，除非当前人工请求明确授权此精确的焦点窃取测试。任何聚合或全套命令都不能调用手动脚本。

可选的吞吐量线束需要绝对大输出的夹具和至少五次运行：

```sh
pnpm --filter @orchbee/desktop benchmark -- \
  --file /absolute/path/to/large-output.txt \
  --runs 5
```

使用基准测试时保持 `pnpm web` 运行，或将 `ORCHBEE_DESKTOP_RENDERER_URL` 设置为另一个环回 Web 开发服务器。

交互目标始终限定在同一个终端 presentation 内。线束根据稳定 presentation root 的几何位置放置指针，并在该 root 上观测滚轮开始，因此瞬态历史快照 Canvas 与 Canvas transform 是被测行为，而不是 actionability 前置条件。它的输出命令会在一个已渲染的完成标记之后保持 shell 打开，直到历史、搜索和复制测量结束；因此，进程退出不会在这些交互期间把实时终端替换成 stopped presentation。捕获 terminal-exit trace 后，线束会终止其隔离的临时 Electron 实例，而不会执行产品的交互式用户退出流程。报告会以 KiB 汇总 `app.getAppMetrics()` 返回的 Browser、renderer（`Tab`）、GPU 与 Electron 合计工作集。这些进程数据支持同机比较；它们不会计入所有由驱动拥有的 GPU 分配，也不定义整个系统的 RSS。

它还可以收集受控的 Ghostty 和仅主机引用。结果是诊断证据；它们不是当前开发阶段的合并或切片完成门。
