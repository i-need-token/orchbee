# @orchbee/web

Orchbee 工作台的浏览器组合条目。

## 组合边界

Electron 提供来自 `@orchbee/desktop-client-protocol` 的严格 `agent-integration-management` port。它投影十种 Agent 的只读本地 inventory，并接受显式 Settings refresh、enable 与 disable 请求。Browser Preview 不提供此能力。

`apps/web` 是普通浏览器页面与 Electron 渲染器共用的唯一可运行产品 UI。环境适配器可以提供不同传输与特权能力，但都组合相同的 Workbench、Workspace、Tab、Panel 与投影终端应用。Desktop 不得维护另一套渲染器 UI。共享 Workbench 客户端运行时负责共享文档协调、投影终端呈现、搜索、Action，以及窄化的客户端会话与投影端口；它不依赖 Electron、`MessagePort`、网络或应用生命周期。运行时为每个投影渲染器会话提供稳定的 `TerminalPresentationConfigurationSource`：初始完整值用于打开表面，后续完整值更新同一会话与附件，不改变 React 功能标识，也不重新打开投影。默认来源保留不平衡内边距，把 `fontSize` 固定为 13 以匹配 Ghostty 默认 13 点设置，并用与 Ghostty 对齐的内置 JetBrains Mono 2.304 variable 字体及 Nerd Fonts Symbols Only 3.4.0 后备字体取代来源中立的 monospace。应用启动时会在打开任何表面前注册这些不可变资产；同一 bundle 与许可证链接服务于Browser Preview 和 Electron 渲染器，不依赖机器已安装字体。每项应用组合都会用客户端本地设置控制器替换该后备来源；控制器恢复严格、完整的偏好记录，公开同一稳定呈现来源，并发布完整运行时更新，无需重新打开投影。

共享客户端运行时会在释放资源前关闭本地操作入口。会话退出先结束等待中的 proposal，再调用传输清理；若订阅在同步退出之后才返回，立即释放该订阅。文档初始化的订阅获取失败时，会释放此前已经取得的订阅，且不会在 dispose 后报告 ready。proposal 完成过程抛错或 Promise 被拒绝，并不能证明文档没有提交：协调器会进入 `control-unavailable`，结束等待中的 flush，且不会自动重新提交该 proposal。清理会逐一尝试释放每个独立拥有的资源一次，并保留单个或聚合错误。已退出的 surface capability 不再应用迟到的终端 metadata。确定性的能力接口轨迹覆盖这些本地所有权与失败边界，不宣称远端回滚或文档恰好一次投递。

Browser Preview 与 Desktop 渲染器在清理前先终止本地生命周期。React 卸载失败不会阻止后续路由、设置、传输和应用生命周期资源的清理尝试。这些组合入口与 Workbench 的清理路径共用 `runApplicationCleanup`：重复或重入的销毁不再产生效果，全部独立步骤尝试完毕后，再报告原始单个错误或按顺序聚合的错误。组合测试在 React root 边界注入失败；Desktop 还实际执行 DOM 渲染并检查全部十一个能力端口。

Browser Preview 使用内存 composition。Desktop 为共享 client session 与本地 terminal projection 提供 MessagePort adapter，并提供应用生命周期、Agent 观察与集成管理、本地 Project Folder 与 Worktree、terminal launch profile、terminal default 和 clipboard 能力。其 `local-host` projection transport 保留严格 OBPB 结构验证和直接的 typed render-buffer view。共享终端 surface 保留测量的 cell geometry、完整背景覆盖、裁剪的直接 graphics、有界 bell indication、connection notice 与 terminal outcome 优先级。Workbench payload 来自 `@orchbee/workbench-client-protocol`；Electron 能力契约来自 `@orchbee/desktop-client-protocol`。

Electron Desktop 还会在固定 bootstrap bundle 中提供 Agent-observation 与
Agent-integration-management port；Browser Preview 不提供。Agent-observation adapter
会在 mount application 前请求一份严格的完整 snapshot，拒绝 malformed 或 uncorrelated
publication，并且只应用递增 revision。它会在 Main 发布对应 state revision 后，独立于 snapshot
转发经过严格解析的 canonical semantic event。Ready adapter 向 Workbench 提供窄化的
`AgentSnapshotSource`；adapter 不可用时，既有 terminal application 保持不变。Workbench 为精确
Panel 与 Tab 投影 status，并把 attention、success 与 failure candidate 提供给 terminal-feedback
controller。该 controller 默认关闭 Agent notification，绝不由 event 请求 permission，会抑制可见
destination、按 semantic event identity 去重，并且只在用户已启用设置且 permission 已经 granted 时
显示本地化且不含内容的文案。激活 notification 会聚焦 application，并调用 Workbench 的
navigation-only callback。Renderer 不拥有 canonical Agent transition，也不能提交 Agent event、
response、approval 或 decision。Renderer reload 会创建新 adapter，并从 Desktop Main 的当前完整
snapshot 恢复，而不是 replay delta 或旧 notification event。

对于主机支持的视觉终端，共享表面在加载当前配置的字体并测量出尺寸为正（positive）的已挂载视口之前，不会打开投影。它通过相同的 Electron 投影命令发送生成的精确列、行、物理单元格大小、填充感知网格和文档 DPR，然后固定第一个渲染器的演示配置。调整大小的观察或动画帧会唤醒不可测量的表面；释放中止等待。没有计时器启动子进程，也不存在 80x24 视觉后备。

Desktop `MessagePort` 投影适配器会让每项本地已释放请求保持关联，直到 Main 确认清理。如果异步 Main open 完成与该 release 交错，适配器只关闭迟到的 attachment 端口，并让共享投影控制保持可用；无关的 pending 与未来投影继续正常工作。无效 schema、标识、端口数量或重复 open 仍会在共享控制边界按失败关闭（fail closed）处理。

已经绘制的 surface 进入 Workbench 托管的 `closing`、`close-failed` 或 `closed` 生命周期时，会保留为冻结的不可交互帧，而不是 dispose 其 DOM。进入该生命周期会释放 projection request、attachment、transport、输入与 resize ownership，并且不会打开或重试另一项 attachment。只有在存活 Tab 跨过显示屏障后 Workbench 移除该 surface，或正常清除最后一个 Tab 时，才会 dispose 这个冻结 renderer。

自然退出后，只有在 Workbench 的 managed 自然退出 removal 等待处置期间，最终帧仍可读取；投影附件会拒绝后续 PTY 输入、调整大小、clear 与 reset 控制，并且处置完成后不会把已退出 Presentation 保留为 Tab。已停止的 Presentation 仍是独立 Workbench 状态。其 Start New Session 控件通过共享组合返回 Workbench 转换结果；获准结果会经常规持久工作流替换 Presentation，拒绝或抛出异常的启动则保留当前已停止 Presentation，并显示局部失败反馈。

意外的附件丢失会冻结最后完成的画布和可访问的行，同时释放其传输和交互所有权。共享表面会在 250、500 和 1,000 毫秒后重试新的附件，并且仅在候选者绘制其第一帧后才替换冻结的投影。耗尽预算后，诚实的最后一帧可见，并暴露了一个本地化的手动重试操作；每次尝试都拥有并释放一个投影请求。类型为 `renderer-backpressure`、`host-transport-backpressure` 或 `host-connection-unavailable` 故障在保留其原始操作和相关诊断后解决相同的附件本地传输不可用信号； Surface 永远不会重试被拒绝的命令或继续使用因果不完整的附件。传输丢失还会显式地与字体/GPU 启动竞争：它会在这类启动可能拖延恢复之前中止该候选者，而候选晋级（promotion）会在其第一帧等待之前和之后分别检查该候选者当前的传输可用性。

恢复控制器会在通知观察者或申请请求之前，为每次打开尝试建立独占归属。同步发生的保留、恢复或处置不能覆盖替代尝试、泄漏迟到的请求结果，也不能在所属尝试退役后继续晋级。资源归属会在清理回调执行前转移，包括输入退订和冻结画面替换。处置会对每项所拥有的资源执行一次清理，保留清理失败，并尊重在最终通知期间取消的监听器。受控恢复控制器测试覆盖这些重入轨迹、迟到结果、抛错清理以及有界重试策略；它们不会对任意回调程序进行模型检查，也不替代真实渲染器和传输的集成证据。 挂载表面对粘贴审阅、移动输入订阅、广播注册及控制器采用相同的清理纪律：注册器的释放函数抛错也不能遗留渲染器或投影请求。Hook 集成测试覆盖单个及多个清理失败。

每个投影表面都会把一个瞬态广播输入 sink 绑定到其精确 Workbench Presentation。只有恢复控制器拥有活动传输，且投影终端公开匹配的附件能力修订时，该 sink 才报告就绪。能力变化、保留、传输丢失、候选替换、冻结与处置都会在后续输入能够复用该 sink 前通知 Workbench。一次获准发送只会把快照修订作为 compare-and-send 守卫，委托一次投影终端的附件本地注入路径；缺少此能力的旧版或测试渲染器及修订变化都会失败关闭。适配器不协调目标，也不重试、回滚或重放多终端投递。

活动 Tab 交接复用同一套 frozen-session 规则，同时不让非活动终端 DOM 或 attachment 保持挂载。surface 进入 retained 状态时，只捕获精确的最后已绘制语义呈现、冻结 canvas，并释放 transport、输入、resize、request 与 attachment 所有权。应用级 `TerminalPresentationSnapshotCache` 以按字节计量的 `32 MiB` least-recently-used 预算保留这些仅显示快照，并在共享客户端 runtime 处置时清空。incoming surface 可以在新实时 attachment 产出帧前绘制缓存快照；该缓存绘制只满足 Workbench 的 display-ready 屏障，输入、指针交互与 `autoFocus` 仍保持禁用，直到候选者绘制首个实时帧并满足 activation-ready。缓存 miss 或快照过大时，退出的冻结层会保持可见，直至实时、stopped 或真实失败内容就绪。返回仍被冻结的 surface 时，会复用其 canvas 满足显示屏障，同时在其后方连接新候选者。被取代或不活动的 session 不能确认帧或发布元数据，并且每个 surface 仍会在交接后释放其逐终端 Canvas、atlas、buffer 与 renderer。

初始 active surface 也依据相同的已绘制帧或非 canvas 结果证据报告显示就绪。Workbench 使用这一证据，在终端实际显示后才确认 Agent 未读动态。retained surface 不能提供这一已读证据。

附件能力失败不同于传输丢失与最终结果。共享表面把操作、代码、可重试性、Host 关联 ID 或客户端请求序列保留为有界诊断，并应用匹配的终端结果。后台输入确认、历史与调整大小失败绝不产生全局终端内容通知：被拒绝的输入仍留在有界诊断与必要的传输恢复路径中，历史反馈属于查找表面，调整大小仍留在其有界重试与诊断路径。连接、运行时及显式用户 Action 失败会保留本地可见反馈。应用不会自动重放终端输入、控制、剪贴板或导航效果。

共享适配器将最后完成的绘制检查点公开为 `data-projected-terminal-rendered-generation` 和 `data-projected-terminal-rendered-revision`。这两个值都来自一个渲染器遥测事件，并标识在任何动画帧合并后实际绘制的打包主机提交。该诊断合同在 Electron 客户端中是相同的；这不是对 Terminal Host 的第二次确认。运行时表示诊断还可以在同一表面上公开请求、提交和绘制的纪元。它们将独立排序的字体准备与单个本地提交区分开来，并完成当前的重绘，而无需创建另一个附件生命周期。

挂载投影渲染器永远不会移动 DOM 焦点。Workbench 保留主动产品控件；只有指针激活、显式终端焦点请求以及关闭查找或终端文本查看器这几条适配器路径会将焦点移到隐藏的终端输入框。投影根包含该输入框与两个界面，因此在三者之间移动不会发布终端焦点转换。

### 移动浏览器终端交互

| 字段         | 契约                                                                                                                                                                                                                                                                                                                   |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题与范围   | 粗粒度、无 hover 指针上的浏览器访问必须提供终端输入、滚动和必要按键，且不重新设计完整 Workbench。完整外壳响应式布局、原生选择手柄、多点触摸、捏合缩放、Panel 手势和可配置键条均明确排除。                                                                                                                              |
| 输入与依赖   | 应用消费当前投影附件输入就绪状态、终端 textarea 焦点、本地化消息、`(hover: none) and (pointer: coarse)`、`VisualViewport` 几何、布局视口后备和语义按键意图。应用绝不推断 Host 模式或编码终端字节。                                                                                                                     |
| 状态与所有权 | 一个已挂载投影终端渲染器拥有 `MobileTerminalInputState`：附件可用性、原生键盘焦点和一次性 `control`/`alt` 修饰键子集。React 表面只渲染已准备状态。Web composition 适配器把当前可视视口宽度、高度和偏移作为可逆呈现几何拥有；它不拥有终端或 Workbench 状态。                                                            |
| 修饰键转换   | 轻触 Control 或 Alt 会切换该一次性修饰键。下一项合格键条按键、物理按键或独立单个可打印 ASCII 原生输入，会在发布前消费所有已准备修饰键。组合开始、粘贴、多字符或非 ASCII 提交文本、blur、只读降级、附件替换、恢复及处置会清除修饰键，且不转换或重放输入。被阻止或失败的发送仍消费快照，因此过时修饰键不能影响后续命令。 |
| 键条与焦点   | 规范的 `projected-terminal.mobile-input-bar` 只在粗粒度无 hover 媒体条件下出现，包含显示/隐藏键盘、Escape、Tab、Control、Alt 和四个方向控件。其按键控件保留 textarea 焦点并使用精确的活动附件修订；键盘切换反映真实 textarea 焦点，绝不声称操作系统已显示键盘。输入不可用或只读时，产生输入的控件禁用。                |
| 视口投影     | composition 会在终端测量前，把每个有限正值 `VisualViewport` resize 或 scroll 样本应用到应用根；API 不可用或无效时使用布局视口。根尊重安全区域 inset，既有终端 `ResizeObserver` 仍是投影网格 resize 的唯一写入者。键盘关闭会恢复最新完整可视视口，且不持久化几何。                                                      |
| 不变量与失败 | 触摸滚动绝不准备修饰键；控件绝不向已替换附件发布；移动状态不会进入共享 Workbench document、浏览器存储或 Terminal Host。缺失浏览器焦点或视口能力会降级为不变布局和类型化不可用输入，而不是合成成功。桌面指针与硬件键盘行为保持不变。                                                                                    |
| 验证与限制   | 确定性状态测试覆盖每个修饰键转换和替换。渲染器与 React 测试覆盖原生捕获顺序、修订固定、本地化和焦点保留。仍需受控有头（headed）移动视口轨迹来验证媒体条件可见性、触摸轻触/滚动、视口收缩/恢复、网格 resize 和两种 locale；真实 iOS 与 Android 键盘弹出仍属手动发布证据。                                               |

每个附件只发布自己的焦点声明和投影需求。Terminal Host 会聚合共享终端的合格声明；只要仍有另一项合格声明，失去焦点或可见性的附件就无法发出终端范围的 `FocusOut`，而重新激活需要新的声明。每次打开或显式再次打开紧凑的查找界面时，只要当前存在活动 Host 选择，就会用当前活动 Host 选择的有界预览替换空查询或保留（retained）的查询；没有活动选择时则保留当前查询。搜索所拥有的活动范围仍以 Host 选择为权威，而不会形成第二套应用程序选择模型。每次这样的调用都会聚焦并全选完整查询；从任一查找控件按 Escape 会关闭界面并恢复终端焦点。匹配选项位于查询输入框内，普通搜索进度使用紧凑的本地化计数，左右方向键在匹配选项间移动，指针改变匹配选项后则把焦点还给查询输入框。只有可操作的失败、超时或保留历史通知才会展开工具栏并显示完整的本地化反馈。

`Ctrl+Shift+G` 会在应用拥有的模态查看器中打开上一条带 Shell 标记的命令输出。一个 replace-latest Action 通过有界通用缓冲区 lane 准备精确 generation/revision 的纯文本 document；过时、淘汰、备用屏幕、不可用或超限读取绝不发布局部输出。已准备 document 在打开期间保持不可变，字面量不区分大小写搜索、Enter/Shift+Enter 导航、原生文本选择与独立的 `terminal.output.copy` Action 都在本地运行。查看器会约束焦点循环并在关闭时恢复终端焦点，且有意不启动外部 pager 或重新渲染 ANSI 样式。参见 [Kitty 对齐契约](../../packages/terminal/docs/kitty-command-output-viewer-alignment.zh-CN.md) 与 `projected-terminal.text-viewer` experience component。

`Ctrl+Shift+H` 会通过同一个私有查看器打开完整的主屏幕保留历史。精确快照投影器会连接软换行、保留硬行边界、记录捕获的可见顶部 offset，并且只发布一份完整 document 或类型化的空/不可用结果。命令输出与回滚历史准备会在发布前相互替换，其不可变 document 保留按来源区分的复制 Action。参见 [Kitty 回滚历史查看器契约](../../packages/terminal/docs/kitty-scrollback-viewer-alignment.zh-CN.md)。

投影帧提交后，渲染器会转发完整的可空标题、当前目录元数据、瞬态 `idle`/`active`/`unknown` 关闭活动，以及精确 `TerminalPresentationId`。共享客户端运行时使用客户端会话 bootstrap 中受信任的终端机器主机名规范化该观察，并应用到匹配的 Workbench Presentation。自然标题与已接受的当前 CWD 仍属瞬态；持久后备/自定义 Tab 标题策略及共享文档投影仍归 Workbench 所有。因此 Electron 浏览器客户端使用同一元数据策略，页面绝不会从自身 URL 主机名取得终端信任。

可疑的粘贴文本会打开一个由共享投影终端表面拥有的本地化审阅对话框。它提供了保留的原始文本的有界的、控件可见的预览，并标识多行输入、将被替换的控件以及嵌入的括号粘贴终止符。取消不会发布任何内容；批准将清理后的文本与已审查的模式一起发送，而 Terminal Host 在编码或队列准入之前拒绝模式更改。

每个投影表面将工作台拥有的最小终端操作目录注册到视口的应用程序运行时和范围中：复制、粘贴、全选、查找、查找下一个、查找上一个、显示上一条命令输出、复制命令输出、显示回滚历史、复制回滚历史、滚动到底部、清除和重置。可用性是同步的，并且在执行准备的调用时保持权威。上下文菜单和自定义快捷键 Paste 仅通过注入的客户端能力读取文本。空闲的平台默认粘贴组合键会保留原生 DOM 投递，使终端适配器可以应用纯文本优先和仅图片 `Control+V` 策略，并且不读取图片字节。当手动选择 Copy 或另一个 Paste Action 仍在进行时，默认组合键会使用 Action 路径；Paste 会等待该精确 Copy 操作结算后再读取当前剪贴板，然后重用相同的终端粘贴清理和审阅路径。每次执行都会以键入的 Action 结果进行结算；需要可感知反馈的已完成、未更改、阻止和失败结果使用共享的本地化终端状态界面。这些直接终端操作不添加确认步骤。不可用或未绑定的 Apple Command 快捷方式仍保持为原生状态，而不是被抑制。

稳定的有头（headed）Canvas 2D 见证 `terminal-parity/IN-07-pre-input`、`terminal-parity/IN-09-action-scope`、`terminal-parity/IN-11-context` 和 `terminal-parity/IN-12-actions` 与其真实包含的 Workbench 运行时组成了生产投影表面。它们断言无按键应用鼠标移动在继续上报的同时保留当前历史位置与 Host 选择，并覆盖已声明/未声明和可用/不可用快捷方式处理、应用程序鼠标所有权、精确固定上下文和陈旧链接拒绝、完整的最小目录、同步可用性、键入结果、无新确认以及 `en-US` 和 `zh-CN` 反馈。它们的剪贴板和外部打开效果被捕获；它们不要求操作系统菜单、剪贴板或 URL 处理程序。

只有在环境所有者向 Terminal Host 提交准确的下一能力修订后，投影附件流才能实时替换输入权限。共享渲染器会立即将现有文本区域设为只读，使输入法组合、本机结算、粘贴审查以及按住的按键、鼠标和焦点状态失效，并根据实时终端重新计算终端操作的可用性。查找、历史记录、选择、复制、保留输出和已批准链接仍然可用。之后的升级只接受携带新修订号的未来输入；它不会重放降级前的 DOM 状态或排队工作。Electron 使用同一个渲染器契约，任何 renderer 命令都不能请求自身的能力升级（capability upgrade）。

稳定的有头（headed）Canvas 2D `terminal-parity/IN-10-read-only` 见证控制精确的附件能力修订，并观察即时本地修复、保留的搜索/选择/历史/复制、零陈旧输入命令，以及升级后只接受全新输入。Terminal Host 的所有者本地测试会证明权威拒绝与修复顺序；该 Browser 夹具不会授予渲染器改变能力的权限，也不声称原生权限行为。

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
stale preparation 与 unavailable active attachment 均不发布任何内容。Browser Preview client
不获得 backing-path capability；在 active shell dialect 具备同样有界的 escaping contract 前，
Windows 仍不受支持。

稳定的有头（headed）`terminal-parity/IN-08-drop` 见证 在当前 POSIX Desktop platform 使用真实 Electron
`File` 与 backing-path bridge。它的 protected `dragover` fixture 会公开 `Files` marker，但不提供
可读 `FileList`；最终 `drop` 再提供 disk-backed object。它观察直接插入既有 shell line 中间、没有
Paste Review、不自动执行，以及只有单独显式 Enter 后才执行。Owner-local test 覆盖 FileList
顺序与 file-kind item fallback、Ghostty-compatible escaping、32-file 与 4,096-code-point boundary、
Unicode、全部 control range、missing/malformed bridge value、unsupported platform、stale
preparation 与 Browser Preview capability absence。该 witness 不自动操作 Finder，也不声明覆盖每一种
native OS payload variant、Windows support、Browser/LAN path access、directory enumeration、
file-content read、upload 或 filesystem-link activation。

打包的 `orchbee:` origin 和 Electron User-Agent token 选择 Desktop 模式。User-Agent 用于 loopback 开发中的 adapter 发现；Desktop 权限只来自已验证的沙箱 bootstrap。普通 HTTP(S) origin 选择内存 Browser Preview。不支持的 runtime marker 会直接拒绝。

Desktop bootstrap 在调用渲染器工厂前占住唯一的渲染器启动位置。重入的 bootstrap 消息会关闭未获接纳的端口。卸载先终止所有者的生命周期再清理；在这之后返回的渲染器会立即释放，不能发布 bootstrap 安全标记。组合测试覆盖重复启动、延迟取得资源和 bootstrap 失败后的清理。

## 导航

所有组合使用同一个 TanStack Browser Router。规范 Workspace 路由使用 `/workspaces/$workspaceId[/tabs/$tabId]`，与可空 Project 成员关系独立。带 Project 的旧 Workspace 链接按稳定 Workspace 身份重定向，即使已重新分组或解散。只指向组的链接解析当前成员；未知 Workspace/Tab 身份仍不可用。导航推入历史，规范化替换历史。渲染器焦点恢复在显式路由协调前恢复精确存活的 Workspace、Tab、Panel 和子终端。见 [`docs/navigation-routing.md`](docs/navigation-routing.zh-CN.md)。

## 设置

本地 Desktop Settings 会在 capability 存在时包含 Agent Integrations。Startup、search、category
navigation 与 reminder 渲染都不执行 mutation。受支持 Agent 在带 integration 的本地 shell 中被直接
启动后，Main 只对该 provider 执行 lazy inspection，每个 session 一次的 reminder 可以打开 Agent
Integrations Settings。只有其中显式的 switch action 才能修改 provider configuration。Dismiss reminder
只存储 Agent ID；remote setup 仍只能手动完成。

通用设置提供系统、English 和简体中文。Desktop 与 TUI 读取同一确认设备偏好；
Browser Preview 保留 origin 本地记录。系统遵循声明的语言输入，显式语言优先。
确认修改无需重载即可更新已挂载产品文案。

通用 Settings Action 会打开应用拥有的全窗口 overlay，而不是导航到其他路由。Workbench 子树变为 inert，但其 React tree 与每个 projected terminal attachment 保持 mounted。Desktop 通用偏好使用共享设备存储，具有绑定来源的草稿、确认保存、冲突及结果不明恢复。宿主字体、像素内边距和不透明度保持渲染端本地。Browser Preview 保留独立的严格 origin 记录。在 macOS Desktop 上，单独的严格本地记录选择左右 Option 键是否充当 terminal Alt；默认为关闭，接受的更改会更新每个 mounted projected terminal，而不替换 attachment。六十七个常用 Workbench 与聚焦终端快捷键（包括应用范围字号增大/减小/重置、在精确显示终端的当前目录创建终端标签页、四个方向 Panel-focus command、上一个/下一个 Panel 循环聚焦、十个精确按位置 Panel 聚焦命令、可视 Panel 聚焦与交换、四个方向 Panel-movement command、按比例均分 Panel、键盘 Panel 调整大小模式、活动标签页在其固定或未固定分区内的循环移动、可见终端 Hints、显示上一条命令输出与显示回滚历史）使用单独的严格本地 override 记录；接受的更改会更新 mounted Workbench 与每个 projected terminal，而不替换 attachment 或 connection。标准默认值保留未加 Shift 的 terminal control sequence，runtime Action availability 在消费任何按键前仍具有权威性。Application theme 与 terminal display theme 保持分离，terminal override 绝不修改 Host metadata、palette、cursor、VT 或 OSC state。本地 Desktop 还注入语义 shell-profile control。其显式 editor 可以创建、重命名、选择 system 或 custom executable、保留 literal argument order、选择 default、删除和确认 reset；每项接受的更改只影响 共享 catalog 提交后创建的 terminal。Browser Preview composition 不接收 profile capability。Desktop 会为新建且受支持的本地 Zsh、非登录 Bash 和 Fish 终端自动加载 Shell Integration，且不编辑启动文件；Settings 会解释该边界，把精确的内置 source 命令作为手动回退，并拥有单独的默认关闭设备本地 completion-reminder policy，通知绝不包含 command text。仅粘贴的 Ghostty migration dry-run 会报告无损 current target、manual-review case、ignored key 与 invalid line，但不读取文件、应用值或持久化 source text。Remote development category 是另一项只读 projection：它解释 `xterm-256color` baseline，公开一个固定 shell diagnostic，区分 observed environment 与 terminfo outcome，并声明 Orchbee 不会静默转发额外 environment variable 或修改 remote host。打开、搜索或切换到该 category 不启动 process、不写 terminal input，也不持久化 remote data。参见 [`docs/terminal-settings.md`](docs/terminal-settings.md) 与 [`docs/keyboard-shortcuts.md`](docs/keyboard-shortcuts.md)。

精确的有头（headed）`terminal-parity/SU-09-theme` 见证会在一次发布（release）前持有已准备的 Host 结果及带 revision 标记的帧，随后观察终端默认值、调色板、光标、覆盖层、像素与应用表面共同发布同一匹配修订。它使用受控运行时呈现 Canvas 夹具，不对持久设置迁移、原生 OS 主题事件或渲染器拥有的终端调色板作出声明。

## 本地开发预览

Locale 偏好、自动后备解析和 shortcut-platform setup 属于 Browser Preview 与 Desktop 共用的
Web composition。

`pnpm web` 使用本地化的侧边栏、独立终端上下文、项目和工作区导航以及每个选项卡面板工作区来呈现工作台外壳。浏览器组合创建一个 `ActionRuntime` 和一个内存 `WorkbenchProjectWorkspaceRuntime`。它仅以独立的终端上下文开始。一个人可以通过输入文件夹路径来创建内存中的项目；项目添加控件首先保留通用创建菜单，然后 `Select Folder…` 打开仅浏览器的路径条目来代替本机选择器。浏览器使用该路径作为初始项目文件夹和终端 CWD，而不声明该路径存在。 `New Folder…` 保持可见但被禁用，因为该组合不执行文件系统、Git、PTY 或持久存储效果。

Electron Desktop 将原生 existing-Folder 获取与 managed-Worktree 能力注入共享应用。`Select Folder…` 使用操作系统 picker，本地 Project Folder attachment 使用同一能力，`New Worktree…` 打开固定位置的 Git 表单。Browser Preview 保留手动 Folder path 输入，不获得原生 filesystem 或 Worktree effect。`New Folder…` 选择原生父目录并明确创建一个原本不存在的子目录，然后观察既有 Project Action、精确规范发布与 Workspace 激活。既有路由协调器分别报告当前已接纳目标与待处理导航，创建流程不会把无关路由确认成成功。已创建或结果未知的文件系统效果会保留，供 Select Folder 恢复；不引入新的 provisioning 或文件系统 API。

OSC 52 写入从不共享选择复制快速路径。共享渲染器自动处理每个精确有界的 UTF-8 请求，而无需呈现确认表面。它的决定重复了确切的附件、主机序列和文本； Desktop Main 执行特权 Electron 效果，可复用的 Browser platform-effects adapter 只在本地 Clipboard API 成功后报告成功。失败的客户端本地效果报告失败并消耗请求，而不要求 Main 重复写入；Browser adapter 保留其安全上下文和瞬时用户激活要求。附件更换、终端退役和渲染器处置抑制了后期决策；请求文本和决策永远不会进入浏览器存储或共享工作台文档。这有意允许主机允许的终端程序替换当前的桌面剪贴板，而无需其他用户手势；恢复每个请求确认是回滚边界。每个附件贝尔向工作台生命周期协调器报告不透明的终端身份和主机序列。只有该终端序列的第一个被承认的交付才会产生简短的标记化视觉表面提示；重复、陈旧、重新连接和重放的传送不会产生任何效果，包括当不同的附件流交错时。可选的声音和操作系统通知回调是单独的显式应用程序选择加入，默认情况下不存在，并且在准入后独立失败。这两种效果都不需要焦点。当前的 OBTF 进度样本仍然是一个安静的底部边缘确定或不确定指示器，并且两种视觉呈现都尊重减少的运动。

终端链接悬停仅读取 generation/revision-pinned Host 行。当 application mouse tracking 拥有表面时，普通 context intent 仍归终端；Shift 会明确把该 intent 交还产品。Workbench 菜单打开时，投影表面只捕获 typed link target 及其精确 generation、revision、link epoch、row 与 column。所有 composition 都把 URL activation 限制在 `http`/`https`/`mailto` 策略内。检测到的文件或目录目标只有在该精确附件 bootstrap 携带可选的 Desktop-local file-effects capability 时才可交互；Browser Preview、archive 与来源中立的附件都会保持 inert。准备好的 Workbench Action 会在选择环境拥有的 effect 前立即重新校验完整 context。context 变化会以本地化 blocked 结果结算，不打开 stale target。Apple 平台上的 Command-click 或其他平台上的 Control-click 使用同一 typed request 与重校验路径。Host 投影的文本与 hyperlink metadata 始终只是数据，而不是 effect capability。

具备该 Desktop-local capability 时，fine pointer 悬停在检测到的 PNG、JPEG、GIF、WebP、AVIF 或 BMP 路径上会启动 300 ms dwell，且不会探测无关路径。随后产生的请求仍绑定 attachment、target、generation、revision、link epoch、row 与 column。target replacement、pointer leave 或 press、wheel input、geometry 或 revision 变化、attachment retirement 与 disposal 都会取消待处理 presentation；迟到 completion 无法覆盖最新 hover。表面只会在源 cell 旁渲染一个 pointer-transparent 的有界卡片，并保持终端 focus 与 selection 不变。缺失、不支持、无效或不可读的图片会静默失败。Browser Preview 永远不会请求或接收本地图片 bytes。

有头（headed）Canvas 2D 见证`terminal-parity/FS-05-file-links`和`terminal-parity/FS-06-url-policy`组成了生产投影表面、工作台动作和浏览器效果。他们证明，`http`、`https` 和 `mailto` 单独暴露悬停、上下文和直接激活，而自动 FTP 文本、不支持的显式元数据、显式文件元数据、检测到的路径和直接文件效果请求保持不活动状态。受控装置使用合成的生成固定主机行窗口和捕获的浏览器开启器；它不执行操作系统 URL 处理程序或声明文件系统存在。

有头（headed）Canvas 2D 见证 `terminal-parity/FS-19-local-file-links` 与 `terminal-parity/FS-20-image-preview` 会在同一真实投影表面上启用可选 local capability，并使用假的 Desktop effect。它们证明 relative-path resolution、modifier opening、300 ms image intent、replacement cancellation、拒绝迟到 completion，以及生产锚定卡片最终的 PNG source 与 pointer-transparent interaction。捕获的 opener 与合成 bytes 不会启动操作系统应用、自动操作 Finder、声明原生图片解码，也不会授予 Browser 文件系统访问。

有头（headed）Canvas 2D 见证 `terminal-parity/FS-07-keyboard-hints` 使用同一个生产界面与有界行夹具。
`Mod+Shift+O` 为可见 URL、路径、文件位置与 hash 目标添加标签；标签按键绝不会到达终端输入，
选择 hash 会插入其精确文本，选择 URL 会到达捕获的 Browser opener，而 Escape 会移除 overlay
且不产生效果。该见证不声明屏幕外发现、文件系统/编辑器激活、Kitty 自定义或操作系统 URL 投递。

有头（headed）Canvas 2D 见证 `terminal-parity/FS-08-command-output-viewer` 使用生产界面与合成 OSC 133
提示符行。`Ctrl+Shift+G` 打开精确保留的命令输出；搜索与可见匹配导航留在不可变 document 内，
复制会把同一内容恰好一次送到捕获的纯文本 writer，而 Escape 会恢复终端输入焦点。该见证不声明
外部 pager 执行、ANSI 保留、完整 scrollback 或操作系统剪贴板变更。

有头（headed）Canvas 2D 见证 `terminal-parity/FS-09-scrollback-viewer` 使用生产界面与有界合成保留历史
快照。`Ctrl+Shift+H` 会从捕获的可见顶部打开全部主屏幕保留文本；搜索、匹配导航、按来源复制、
Escape 焦点恢复与不变的 attachment identity 都使用生产查看器和 Action。该见证不声明超出 Host
保留范围的历史、ANSI 保留、外部 pager 或操作系统剪贴板变更。

有头（headed）Canvas 2D 见证 `terminal-parity/LC-05-lan-capability` 组成了生产投影表面、工作台复制操作、主机选择内容路径和浏览器效果。在环回测试源上，它观察到真正的 Chromium 安全上下文和瞬态用户激活信号：没有激活会在没有主机请求的情况下禁用复制，而受信任的单击会接受一个主机请求和一个捕获的纯文本编写器调用。然后证明被拒绝的写入器不会收到重试，并且动态丢失和受控的不安全功能会禁用操作，既不调用主机导出也不调用写入器，并保留本地化反馈。该夹具捕获而不是改变操作系统剪贴板，并控制能力损失和不安全上下文探测；它不声明浏览器 permission-UI 行为或 LAN 传输安全。

浏览器拥有的请求和表示标识符使用 `crypto.getRandomValues`，该标识符在明确接受的 HTTP LAN-preview 源上仍然可用。运行时代码不依赖于仅安全上下文 `crypto.randomUUID` API。

对于完整的本地桌面流程，请运行：

```sh
pnpm desktop
```

Electron 加载 Web 开发 URL，并通过 User-Agent 发现 adapter。打包 Electron 通过标准安全 `orchbee://app` origin 提供 production build；`apps/desktop` 负责打包和资源交付。Browser Preview 保持为独立的内存开发界面。

## 单元验证

运行 `pnpm --filter @orchbee/web test` 执行单元测试。Windows 上 Vitest 最多使用四个工作进程（可用 CPU 更少时取其数量），以限制其他 workspace 套件并发时的进程创建及文件系统竞争。所有文件、断言和 deadline 保留；其他平台沿用运行器默认值。
