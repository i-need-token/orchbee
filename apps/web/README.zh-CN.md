# @orchbee/web

Orchbee 工作台的浏览器组合条目。

## 组合边界

`apps/web` 是普通浏览器页面与 Electron 渲染器共用的唯一可运行产品 UI。环境适配器可以提供不同传输与特权能力，但都组合相同的 Workbench、Workspace、Tab、Panel 与投影终端应用。Desktop 不得维护另一套渲染器 UI。共享 Workbench 客户端运行时负责共享文档协调、投影终端呈现、搜索、Action，以及窄化的客户端会话与投影端口；它不依赖 Electron、`MessagePort`、网络或应用生命周期。运行时为每个投影渲染器会话提供稳定的 `TerminalPresentationConfigurationSource`：初始完整值用于打开表面，后续完整值更新同一会话与附件，不改变 React 功能标识，也不重新打开投影。默认来源保留不平衡内边距，把 `fontSize` 固定为 13 以匹配 Ghostty 默认 13 点设置，并用与 Ghostty 对齐的内置 JetBrains Mono 2.304 variable 字体及 Nerd Fonts Symbols Only 3.4.0 后备字体取代来源中立的 monospace。应用启动时会在打开任何表面前注册这些不可变资产；同一 bundle 与许可证链接服务于浏览器、Gateway 和 Electron 渲染器，不依赖机器已安装字体。每项应用组合都会用客户端本地设置控制器替换该后备来源；控制器恢复严格、完整的偏好记录，公开同一稳定呈现来源，并发布完整运行时更新，无需重新打开投影。

浏览器预览使用其内存中的组合。 Desktop 为共享客户端会话和投影端口提供 `MessagePort` 适配器，然后在通用应用程序旁边分层其仅 Electron 的应用程序生命周期、浏览器访问、本机项目文件夹获取、托管 Project Worktree 和设备本地终端启动配置文件功能。其功能范围的本地投影传输声明 `local-host`，允许 OBPB format-`0.0.1` 渲染缓冲区在结构验证后成为直接类型化视图。网关适配器每页使用一个经过身份验证的控件 WebSocket，每个可见终端附件使用一个二进制 WebSocket，并声明 `external`，保留完整的每单元协议扫描。两个适配器组成相同的`createWorkbenchClientRuntime`；两者都没有实现另一个 Workbench 渲染器。共享的投影终端表面在每个内联边缘保留一个透明像素，同时保持其块边缘齐平，在每次容器大小调整后将完整的单元格放入该边界内，并在整个面板视口中镜像有效的终端默认背景，因此画布永远不会暴露不匹配的应用程序表面；主机模式使用主机投影默认值，而客户端显示覆盖使用其选定的默认颜色和 Alpha。背景不透明度适用于默认背景和渲染器透明，绝不适用于全根 CSS 不透明度或终端文本。支持的直接 Kitty 图形使用相同的严格解码的实时帧和裁剪的、指针透明的同级画布；历史演示和空图形快照会删除该覆盖层，而不是重播陈旧的图像。获准的终端响铃会在整个表面显示短暂且不截获指针的信息色罩层，而不是类似焦点的边缘；减少动态效果时会在同一有限时长内显示无动画提示。连续获准的序列会重新开始这个有界视觉提示，但不改变布局或焦点；声音与通知策略保持独立。连接通知明显地覆盖该表面，而不消耗终端网格空间或将最后一行移到视口之外。提交或重复的帧仅清除暂时的连接故障；已发布的终端退出或运行时故障优先于后来的传输损失，并保留其最终帧呈现状态。 Generic Workbench 客户端负载来自 `@orchbee/workbench-client-protocol`；只有网关身份验证/成帧来自 `@orchbee/gateway-protocol`，并且只有 Electron 引导、应用程序生命周期、浏览器访问、项目文件夹获取、Project Worktree 和终端启动配置文件命令来自 `@orchbee/desktop-client-protocol`。

对于主机支持的视觉终端，共享表面在加载当前配置的字体并测量正安装视口之前不会打开投影。它通过相同的 Electron 或网关投影命令发送生成的精确列、行、物理单元格大小、填充感知网格和文档 DPR，然后固定第一个渲染器的演示配置。调整大小的观察或动画帧会唤醒不可测量的表面；释放中止等待。没有计时器启动子进程，也不存在 80x24 视觉后备。

自然退出后，只有在 Workbench 的 managed 自然退出 removal 等待处置期间，最终帧仍可读取；投影附件会拒绝后续 PTY 输入、调整大小、clear 与 reset 控制，并且处置完成后不会把已退出 Presentation 保留为 Tab。已停止的 Presentation 仍是独立 Workbench 状态。其 Start New Session 控件通过共享组合返回 Workbench 转换结果；获准结果会经常规持久工作流替换 Presentation，拒绝或抛出异常的启动则保留当前已停止 Presentation，并显示局部失败反馈。

意外的附件丢失会冻结最后完成的画布和可访问的行，同时释放其传输和交互所有权。共享表面会在 250、500 和 1,000 毫秒后重试新的附件，并且仅在候选者绘制其第一帧后才替换冻结的投影。耗尽预算后，诚实的最后一帧可见，并暴露了一个本地化的手动重试操作；每次尝试都拥有并释放一个投影请求。类型为 `renderer-backpressure`、`host-transport-backpressure` 或 `host-connection-unavailable` 故障在保留其原始操作和相关诊断后解决相同的附件本地传输不可用信号； Surface 永远不会重试被拒绝的命令或继续使用因果不完整的附件。传输丢失还会显式地与字体和 GPU 启动竞争：它在启动之前中止该候选者可能会拖延恢复，并且升级会在第一帧等待之前和之后检查候选者当前的传输可用性。

活动 Tab 交接复用同一套 frozen-session 规则，同时不让非活动终端 DOM 或 attachment 保持挂载。surface 进入 retained 状态时，只捕获精确的最后已绘制语义呈现、冻结 canvas，并释放 transport、输入、resize、request 与 attachment 所有权。应用级 `TerminalPresentationSnapshotCache` 以按字节计量的 `32 MiB` least-recently-used 预算保留这些仅显示快照，并在共享客户端 runtime 处置时清空。incoming surface 可以在新实时 attachment 产出帧前绘制缓存快照；该缓存绘制只满足 Workbench 的 display-ready 屏障，输入、指针交互与 `autoFocus` 仍保持禁用，直到候选者绘制首个实时帧并满足 activation-ready。缓存 miss 或快照过大时，退出的冻结层会保持可见，直至实时、stopped 或真实失败内容就绪。返回仍被冻结的 surface 时，会复用其 canvas 满足显示屏障，同时在其后方连接新候选者。被取代或不活动的 session 不能确认帧或发布元数据，并且每个 surface 仍会在交接后释放其逐终端 Canvas、atlas、buffer 与 renderer。

附件能力失败不同于传输丢失与最终结果。共享表面把操作、代码、可重试性、Host 关联 ID 或客户端请求序列保留为有界诊断，并应用匹配的终端结果。后台输入确认、历史与调整大小失败绝不产生全局终端内容通知：被拒绝的输入仍留在有界诊断与必要的传输恢复路径中，历史反馈属于查找表面，调整大小仍留在其有界重试与诊断路径。连接、运行时及显式用户 Action 失败会保留本地可见反馈。应用不会自动重放终端输入、控制、剪贴板或导航效果。

共享适配器将最后完成的绘制检查点公开为 `data-projected-terminal-rendered-generation` 和 `data-projected-terminal-rendered-revision`。这两个值都来自一个渲染器遥测事件，并标识在任何动画帧合并后实际绘制的打包主机提交。该诊断合同在 Electron 和网关浏览器客户端中是相同的；这不是对 Terminal Host 的第二次确认。运行时表示诊断还可以在同一表面上公开请求、提交和绘制的纪元。它们将独立排序的字体准备与单个本地提交区分开来，并完成当前的重绘，而无需创建另一个附件生命周期。

挂载投影渲染器永远不会移动 DOM 焦点。Workbench 保留主动产品控件；只有指针激活、显式终端焦点请求和关闭查找界面这几条适配器路径会将焦点移到隐藏的终端输入框。投影根包含该输入框与查找界面，因此在两者之间移动不会发布终端焦点转换。每个附件只发布自己的焦点声明和投影需求。Terminal Host 会聚合共享终端的合格声明；只要仍有另一项合格声明，失去焦点或可见性的附件就无法发出终端范围的 `FocusOut`，而重新激活需要新的声明。打开紧凑的查找界面时，如果可用，会根据活动 Host 选择的有界预览生成空查询或预填查询；搜索所拥有的活动范围仍以 Host 选择为权威，而不会形成第二套应用程序选择模型。打开或显式再次打开查找都会聚焦并全选完整查询；从任一查找控件按 Escape 会关闭界面并恢复终端焦点。匹配选项位于查询输入框内，普通搜索进度使用紧凑的本地化计数，左右方向键在匹配选项间移动，指针改变匹配选项后则把焦点还给查询输入框。只有可操作的失败、超时或保留历史通知才会展开工具栏并显示完整的本地化反馈。

投影帧提交后，渲染器会转发完整的可空标题、当前目录元数据、瞬态 `idle`/`active`/`unknown` 关闭活动，以及精确 `TerminalPresentationId`。共享客户端运行时使用客户端会话 bootstrap 中受信任的终端机器主机名规范化该观察，并应用到匹配的 Workbench Presentation。自然标题与已接受的当前 CWD 仍属瞬态；持久后备/自定义 Tab 标题策略及共享文档投影仍归 Workbench 所有。因此 Electron 与 Gateway 浏览器客户端使用同一元数据策略，页面绝不会从自身 URL 主机名取得终端信任。

可疑的粘贴文本会打开一个由共享投影终端表面拥有的本地化审阅对话框。它提供了保留的原始文本的有界的、控件可见的预览，并标识多行输入、将被替换的控件以及嵌入的括号粘贴终止符。取消不会发布任何内容；批准将清理后的文本与已审查的模式一起发送，而 Terminal Host 在编码或队列准入之前拒绝模式更改。

每个投影表面将工作台拥有的最小终端操作目录注册到视口的应用程序运行时和范围中：复制、粘贴、全选、查找、查找下一个、查找上一个、滚动到底部、清除和重置。可用性是同步的，并且在执行准备的调用时保持权威。粘贴仅通过注入的客户端功能读取文本，然后重用相同的终端粘贴清理和审阅路径。每次执行都会以键入的 Action 结果进行结算；需要可感知反馈的已完成、未更改、阻止和失败结果使用共享的本地化终端状态界面。这些直接终端操作不添加确认步骤。不可用或未绑定的 Apple Command 快捷方式仍保持为原生状态，而不是被抑制。

稳定的 Canvas 2D 见证者 `terminal-parity/IN-07-pre-input`、`terminal-parity/IN-09-action-scope`、`terminal-parity/IN-11-context` 和 `terminal-parity/IN-12-actions` 与其真实包含的 Workbench 运行时组成了生产投影表面。它们断言无按键应用鼠标移动在继续上报的同时保留当前历史位置与 Host 选择，并覆盖已声明/未声明和可用/不可用快捷方式处理、应用程序鼠标所有权、精确固定上下文和陈旧链接拒绝、完整的最小目录、同步可用性、键入结果、无新确认以及 `en-US` 和 `zh-CN` 反馈。它们的剪贴板和外部打开效果被捕获；它们不要求操作系统菜单、剪贴板或 URL 处理程序。

只有在环境所有者向 Terminal Host 提交准确的下一能力修订后，投影附件流才能实时替换输入权限。共享渲染器会立即将现有文本区域设为只读，使输入法组合、本机结算、粘贴审查以及按住的按键、鼠标和焦点状态失效，并根据实时终端重新计算终端操作的可用性。查找、历史记录、选择、复制、保留输出和已批准链接仍然可用。之后的升级只接受携带新修订号的未来输入；它不会重放降级前的 DOM 状态或排队工作。Electron 与 Gateway 使用同一个渲染器契约，任何渲染器命令都不能请求提升自身权限。

稳定的有界面 Canvas 2D `terminal-parity/IN-10-read-only` 见证控制精确的附件能力修订，并观察即时本地修复、保留的搜索/选择/历史/复制、零陈旧输入命令，以及升级后只接受全新输入。Terminal Host 的所有者本地测试会证明权威拒绝与修复顺序；该 Browser 夹具不会授予渲染器改变能力的权限，也不声称原生权限行为。

Desktop file drop 是同一 surface 的可选 capability，而不是 Browser filesystem API。Desktop
adapter 只能把 user-supplied disk `File` object 解析为 backing path；一个 object 可以代表 file
或 directory，但不会枚举 child。Drag data store 受保护时，file drag recognition 使用 `Files`
marker 或 file-kind item；到 `drop` 时再使用有序 `FileList` 或非 null file-kind item。确定性
preparer 最多接受 32 条 absolute POSIX path，保留顺序，应用固定 Ghostty live-buffer backslash
escaping set，并用一个空格连接。任意 path 缺失、为 relative、包含 C0/C1 control character，或使
escaped input 超过 4,096 个 Unicode code point 时，会拒绝整个 drop。Ready drop 直接进入既有
FIFO paste path，因此 simple path 保持不变，shell 会在当前 editing cursor 收到文本，不打开 Paste
Review，也不追加 Enter。Local path 可能不存在于 SSH、container 或其他 foreground environment；
direct insertion 有意不增加 warning 或 remote-path translation。Replacement、lifecycle abort、
stale preparation 与 unavailable active attachment 均不发布任何内容。Browser 与 Gateway client
不获得 backing-path capability；在 active shell dialect 具备同样有界的 escaping contract 前，
Windows 仍不受支持。

稳定的 headed `terminal-parity/IN-08-drop` witness 在当前 POSIX Desktop platform 使用真实 Electron
`File` 与 backing-path bridge。它的 protected `dragover` fixture 会公开 `Files` marker，但不提供
可读 `FileList`；最终 `drop` 再提供 disk-backed object。它观察直接插入既有 shell line 中间、没有
Paste Review、不自动执行，以及只有单独显式 Enter 后才执行。Owner-local test 覆盖 FileList
顺序与 file-kind item fallback、Ghostty-compatible escaping、32-file 与 4,096-code-point boundary、
Unicode、全部 control range、missing/malformed bridge value、unsupported platform、stale
preparation 与 Gateway capability absence。该 witness 不自动操作 Finder，也不声明覆盖每一种
native OS payload variant、Windows support、Browser/LAN path access、directory enumeration、
file-content read、upload 或 filesystem-link activation。

打包的`orchbee:` origin和Electron User-Agent令牌选择桌面模式。 User-Agent 是用于环回开发的适配器发现，而不是身份验证；桌面权限仍然只能通过经过验证的沙盒引导程序到达。显式 `?orchbee-runtime=gateway` 标记选择环回上经过身份验证的进程生命周期网关客户端或显式启用的受信任 LAN。普通的HTTP(S)浏览器源选择浏览器预览。网关端口、版本和生命周期功能必须在片段中恰好出现一次。无效、重复或不完整的值将无法关闭且不会回退。

## 导航

所有三种组合都使用一种TanStack路由器和浏览器的路径契约相同。 项目、工作区和活动选项卡标识显示在地址中。 结构不完整的路线（例如 `/projects` 或 `/projects/$projectId/workspaces/$workspaceId`）将自己替换为第一个订购的可用后代； 显式陈旧标识符保留地址并显示本地不可用状态。 应用程序内导航会推送历史记录，而规范化和协调会取代它，因此“后退”和“前进”会重现显示的 Client View，而无需发布共享文档更改。 请参阅[`docs/navigation-routing.md`](docs/navigation-routing.md)。

## 设置

常见的“设置”操作会打开应用程序拥有的全窗口叠加层，而不是导航到另一条路线。 Workbench 子树变为惰性，但其 React 树和每个投影终端附件仍保持安装状态。客户端本地应用程序主题、终端字体、有效的默认终端颜色和默认背景不透明度立即应用，并作为当前浏览器源的一项严格版本记录保留。在macOS桌面上，单独的严格本地记录选择是否左、右或两个Option键都充当终端Alt；默认设置为关闭，接受的更改会更新每个已安装的投影终端，而无需替换其附件。十八个常用的工作台和聚焦终端快捷方式（包括四个方向面板聚焦命令）使用单独的严格本地覆盖记录；接受的更改会更新已安装的工作台和每个投影终端，而无需更换附件或连接。标准默认值保留未移动的终端控制序列，而运行时操作可用性在使用任何按键之前仍然具有权威性。应用程序主题与终端显示主题保持分离，终端覆盖永远不会改变主机元数据、调色板、光标、VT 或 OSC 状态。本地桌面还注入了语义 shell 配置文件控件。其显式编辑器可以创建、重命名、选择系统或自定义可执行文件、保留文字参数顺序、选择默认值、删除和确认重置；每个接受的更改仅影响主拥有的目录提交后创建的终端。网关和浏览器组合不接收配置文件功能。 Shell 集成仅手动：设置显示准确的 Zsh、Bash 和 Fish 源命令，从不编辑启动文件，并拥有单独的默认关闭设备本地完成提醒策略，其通知从不包含命令文本。仅粘贴 Ghostty 迁移试运行会报告无损当前目标、手动审核案例、忽略的键和无效行，而无需读取文件、应用值或保留源文本。远程开发类别是另一种只读投影：它解释了 `xterm-256color` 基线，公开一个恒定的 shell 诊断，区分观察到的环境和 terminfo 结果，并声明 Orchbee 不会默默转发额外的环境变量或修改远程主机。打开、搜索或切换到此类别不会启动任何进程，不会写入终端输入，也不会保留任何远程数据。 Desktop 还注入语义浏览器访问条目，而无需将 Electron 端口或控制器暴露给通用设置模块。请参阅[`docs/terminal-settings.md`](docs/terminal-settings.md)和[`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md)。

精确的 `terminal-parity/SU-09-theme` 见证会在一次原子发布前持有已准备 Host 结果及带修订标记的帧，随后观察终端默认值、调色板、光标、覆盖层、像素与应用表面共同发布同一匹配修订。它使用受控运行时呈现 Canvas 夹具，不对持久设置迁移、原生 OS 主题事件或渲染器拥有的终端调色板作出声明。

## 本地开发预览

`pnpm web` 使用本地化的侧边栏、独立终端上下文、项目和工作区导航以及每个选项卡面板工作区来呈现工作台外壳。浏览器组合创建一个 `ActionRuntime` 和一个内存 `WorkbenchProjectWorkspaceRuntime`。它仅以独立的终端上下文开始。一个人可以通过输入文件夹路径来创建内存中的项目；项目添加控件首先保留通用创建菜单，然后 `Select Folder…` 打开仅浏览器的路径条目来代替本机选择器。浏览器使用该路径作为初始项目文件夹和终端 CWD，而不声明该路径存在。 `New Folder…` 保持可见但被禁用，因为该组合不执行文件系统、Git、PTY 或持久存储效果。

该组合还负责 locale 与快捷键平台设置。Electron Desktop 把原生 existing-Folder 获取与托管 Worktree 能力注入同一应用，因此 `Select Folder…` 会打开 OS 选择器，Project Manager 的 `Add Folder` 使用同一能力而不显示手动路径字段，`New Worktree…` 则打开固定位置的 Git 表单。普通浏览器预览与已认证 Gateway 页面不接收这两项原生能力，继续使用手动 Folder 路径输入，且不显示 New Worktree。新建 Folder 仍需等待环境 adapter 拥有对应 effect。没有 Gateway marker 的普通 HTTP(S) URL 仍是内存预览，不会连接 Desktop Main 系统。由 Desktop Main 打开或复制的 capability-bearing URL，会在 Electron 及其他每个已认证页面共用的规范 Workbench 文档与终端 Presentation 上创建新页面会话和 Client View。关闭页面只释放其附件；重新打开 URL 会创建新会话与完整投影快照。

Gateway 复制选择时会从 Terminal Host 获取有界 UTF-8 选择字节，并可用浏览器剪贴板 API 提交。每次写入都动态解析当前 API，并要求 `window.isSecureContext` 与活动的瞬态 `navigator.userActivation`；复制 Action 可用性使用同一实时检查，最终效果在 Host 往返后再次检查。API 缺失、激活过期、上下文不安全、能力探测失败或写入被拒绝时，操作会失败并以有界局部表面结果结束。浏览器路径不执行重试、`execCommand`、服务器中继、隐藏降级、剪贴板读取或图像字节读取。当前 insecure-LAN 组合未安装手动复制呈现，因此不公开特权复制路径，Copy 保持不可用；未来经审核的手动复制呈现必须保持明确的非剪贴板交互。剪贴板效果归属明确后，自动与手动准入的复制尝试共享一个本地化表面级成功/失败公告；复制反馈不依赖查找表面是否打开。Gateway 链接使用仅允许 `http`、`https` 与 `mailto` 的本地浏览器效果；自动检测、投影悬停/上下文、Workbench Action 可用性及最终效果都消费终端包中的同一规范化策略。页面不获得 Electron Main 效果能力。Electron 继续使用 Main 拥有的剪贴板，并在特权边界独立重新校验同一窄化 scheme 集合。

OSC 52 写入从不共享选择复制快速路径。共享渲染器自动处理每个精确有界的 UTF-8 请求，而无需呈现确认表面。它的决定重复了确切的附件、主机序列和文本； Desktop Main 执行特权 Electron 效果，而网关浏览器仅在其自己的本地剪贴板 API 成功后报告成功。失败的客户端本地效果报告失败并消耗请求，而不要求 Main 重复写入；网关路径保留其安全上下文和瞬时用户激活要求。附件更换、终端退役和渲染器处置抑制了后期决策；请求文本和决策永远不会进入浏览器存储或共享工作台文档。这有意允许主机允许的终端程序替换当前的桌面剪贴板，而无需其他用户手势；恢复每个请求确认是回滚边界。每个附件贝尔向工作台生命周期协调器报告不透明的终端身份和主机序列。只有该终端序列的第一个被承认的交付才会产生简短的标记化视觉表面提示；重复、陈旧、重新连接和重放的传送不会产生任何效果，包括当不同的附件流交错时。可选的声音和操作系统通知回调是单独的显式应用程序选择加入，默认情况下不存在，并且在准入后独立失败。这两种效果都不需要焦点。当前的 OBTF 进度样本仍然是一个安静的底部边缘确定或不确定指示器，并且两种视觉呈现都尊重减少的运动。

终端链接悬停仅读取生成/修订固定的主机行。当应用程序鼠标跟踪拥有该表面时，普通上下文意图仍由终端拥有； Shift 明确地将这一意图返回给产品。当工作台菜单打开时，投影表面仅捕获链接目标及其确切的生成、修订、链接纪元、行和列。准备好的工作台操作应用 `http`/`https`/`mailto` 允许列表，并在选择浏览器本地或 Electron Main 拥有的效果之前立即重新验证完整上下文。更改的上下文将作为本地化的阻止结果解决，而无需打开过时的目标。修改器辅助的直接点击使用相同的请求和重新验证路径。主机投影的超链接是数据，而不是效果功能。

头部Canvas 2D见证`terminal-parity/FS-05-file-links`和`terminal-parity/FS-06-url-policy`组成了生产投影表面、工作台动作和浏览器效果。他们证明，`http`、`https` 和 `mailto` 单独暴露悬停、上下文和直接激活，而自动 FTP 文本、不支持的显式元数据、显式文件元数据、检测到的路径和直接文件效果请求保持不活动状态。受控装置使用合成的生成固定主机行窗口和捕获的浏览器开启器；它不执行操作系统 URL 处理程序或声明文件系统存在。

头部 Canvas 2D 见证人 `terminal-parity/LC-05-lan-capability` 组成了生产投影表面、工作台复制操作、主机选择内容路径和浏览器效果。在环回测试源上，它观察到真正的 Chromium 安全上下文和瞬态用户激活信号：没有激活会在没有主机请求的情况下禁用复制，而受信任的单击会接受一个主机请求和一个捕获的纯文本编写器调用。然后证明被拒绝的写入器不会收到重试，并且动态丢失和受控的不安全功能会禁用操作，既不调用主机导出也不调用写入器，并保留本地化反馈。该夹具捕获而不是改变操作系统剪贴板，并控制能力损失和不安全上下文探测；它不声明浏览器 permission-UI 行为或 LAN 传输安全。

浏览器拥有的请求和表示标识符使用 `crypto.getRandomValues`，该标识符在明确接受的 HTTP LAN-preview 源上仍然可用。运行时代码不依赖于仅安全上下文 `crypto.randomUUID` API。

对于完整的本地桌面流程，请运行：

```sh
pnpm desktop
```

Electron加载Web开发URL并从其User-Agent中检测到。打包的 Electron 从标准安全 `orchbee://app` 源提供此应用程序的生产构建； `apps/desktop` 拥有打包和资源交付，但没有第二个渲染器实现。在桌面中，打开 **设置 → 浏览器访问**。默认操作启动仅环回访问并打开系统浏览器。必须明确启用 Trusted-LAN 预览，并始终显示 HTTP 未加密，并且拥有该链接将授予完全终端控制权。渲染器仅接收经过清理的起源； Desktop Main拥有秘密链接复制和开放效果。
