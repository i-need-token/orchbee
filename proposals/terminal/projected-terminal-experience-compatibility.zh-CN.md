# Projected Terminal 体验兼容恢复

| 字段     | 内容                                                                                                                                                                                         |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 已接受的恢复设计，包括 2026-08-18 的能力与协调决策；实现与证据尚未完成                                                                                                                       |
| Owner    | `@orchbee/terminal` 维护者；Terminal Host、协议、Workbench、Desktop 与 Browser adapter 分别拥有下文指定的边界                                                                                |
| 当前权威 | [`packages/terminal/README.md`](../../packages/terminal/README.md)、[`apps/terminal-host/docs/runtime-model.md`](../../apps/terminal-host/docs/runtime-model.md)，以及各链接的协议或应用指南 |
| 参考版本 | Orchbee `be7b4d0d531be6bab4c6cb4fb164addb997c951d`；Host 前加固版本 `44daa5a98f263d146cda6588f22eb6d1325cb97e`；Ghostty `91f66da24527fa02d92b5fd0b41cd020f553a64c`                           |
| 退出条件 | 满足每项声明与已接受的不可用行为，把持续维护的事实迁入所属指南与可执行证据，然后删除本提案及其评审翻译                                                                                       |

英文 [projected-terminal-experience-compatibility.md](projected-terminal-experience-compatibility.md)
是 canonical 版本；本文用于简体中文评审。

## 授权

终端体验必须恢复成熟切换前终端的可观察行为，同时保留 Terminal Host 作为唯一 PTY 与 Ghostty VT
权威。Desktop 与经过认证的局域网 Browser 必须通过 projected client 使用同一份 canonical Host
状态。恢复工作按可独立验证的切片进行，不能从截图或仅仅存在一条实现路径推断已经完成。

决策 owner 于 2026-08-11 接受了该恢复范围与声明清单。范围包括一个 Tab 中的一个主终端
Panel；不包括多 Panel/多 Tab 产品工作、恢复浏览器侧 VT、同步全缓冲区 API、不受支持的图形
协议，以及无关的新产品功能。活跃请求的决策 owner 于 2026-08-18 分别接受了
[已接受的能力与产品边界](#已接受的能力与产品边界)中的选择。这些选择只授权其中记录的精确
边界；在每个所属切片收敛前，其实现与证据仍未完成。

### 已接受的终端交互修订——2026-08-16

活跃请求中明确身份的决策 owner 于 2026-08-16 接受了这项 consequential、modeled 修订。
图片剪贴板仅含图片时的 Agent 粘贴与可见网格内的 preedit 换行有意采用已观察到的 Warp
行为；其他部分继续遵守固定 Ghostty 行为与既有恢复授权。

| 字段     | 已接受决策                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | Projected terminal 获得焦点后，会在原生应用处理前消费尚未认领的 macOS Command chord；纯图片 clipboard paste 在没有 plain text 时被取消；history thumb 的右侧 inset 过大；较长 live preedit 不换行而是截断并向左移动；animated foreground-color boundary 可能重新划分 shaped text，使 glyph ink 产生小数像素移动。                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 范围     | `@orchbee/terminal` 拥有 browser-key fallback、clipboard-event adaptation、preedit/IME geometry、scrollbar projection 与不依赖颜色的 shaping geometry。`apps/web` 保留 semantic terminal 与 Workbench Action 的优先级，Desktop 保留既有 native menu 与受控 application-lifetime workflow。Supporting test 与所属 package guide 投影这些事实。                                                                                                                                                                                                                                                                                                                                                                                                               |
| 声明     | 已接受的 semantic binding 执行后，其他未认领的 Apple Command-modified chord 不发布为 terminal input，也不取消 DOM default，因此不依赖 key-name allowlist 即可保留 Browser/Electron 原生处理。文本粘贴不变；已允许的 clipboard event 在没有 plain text 且至少包含一个 image item 时，保持系统剪贴板不变并精确发布一次 Control+V press/release。8 CSS pixel history thumb 与 Panel viewport 右边缘保持 2 CSS pixel。Live preedit 从 Host cursor 开始，以 row-major 顺序跨可见 grid row 换行，不拆分 wide cell；native IME caret anchor 移至相同换行位置，超过最后一个可见 row 时 clip 而不 mutation terminal state。仅 foreground color 的变化改变 paint segment，但绝不改变 shaping-run geometry；影响 glyph shape 的 font/style 属性仍是 shaping boundary。 |
| 边界     | Terminal Host 仍是唯一 PTY、VT、canonical cursor 与 mode-dependent encoder。不增加 protocol、Host、native clipboard read、filesystem、upload 或 durable-state boundary。Browser clipboard metadata 只检查 image MIME item，不读取或保留 image byte。macOS native handling 进入既有权威 Desktop quit gate。Preedit 是可逆 presentation，composition commit/cancellation 语义不变。                                                                                                                                                                                                                                                                                                                                                                           |
| 决策权威 | 活跃请求的决策 owner 接受：generic Apple Command handoff，而不是 key-name allowlist 或新完成的 Action catalog；图片剪贴板仅含图片时发送一个 synthetic Control+V，而不是 Agent detection 或 temporary-file conversion；超过可见 row 时 clip，而不是 scroll 或 mutation Host viewport；当 reduced regression 确认 animated-color 原因时分离 paint 与 shaping。Private helper 名称、local layout type 与 test-fixture 结构属于 delegated implementation decision。                                                                                                                                                                                                                                                                                             |
| 证据     | Reduced unit regression 覆盖 Apple Command default 保留且不发布 Host key、已认领 shortcut 优先级、纯图片 Control+V 精确发布与文本粘贴兼容、Panel-relative 2 pixel scrollbar geometry、narrow/wide 多行 preedit、底部 clipping 与换行后的 native anchor，以及 foreground boundary 移动时 Canvas/WebGPU shaping geometry 稳定。既有 Desktop 受控退出 system path 与受影响 package verification 仍是必需 integration evidence。                                                                                                                                                                                                                                                                                                                                |
| 交付     | 先持久化本修订，再分别以最低层 regression 优先实现可独立评审的 input fallback/image paste、scrollbar、preedit/anchor 与 shaping 切片。没有 migration 或 persisted rollout state。每个切片均可通过 revert 其 source、test 与所属 guide 投影恢复；不保留 feature flag 或 compatibility shim。                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 未知项   | 报告中的 subpixel marquee 原因无法通过截图捕获。只有 reduced renderer regression 证明 geometry 或 raster identity 仅因 foreground boundary 而改变时，才授权修改 production shaping；否则该切片停止在已证伪的假设并报告剩余诊断缺口。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

本修订本身不包括 Agent process detection、user-customizable keymap、完成最终 terminal Action
catalog 或 context menu、非图片 clipboard fallback、读取 image byte、temporary file、upload、
file drop、Host/protocol/VT 变更、为 preedit 滚动 terminal、composition commit 变更、替换字体、
改变动画颜色，以及 scrollbar width、hit target 或 vertical inset 变更。后续
[2026-08-18 修订](#已接受的能力协调与证据修订2026-08-18)只在其记录边界内取代 file-drop 与最终
Action/context 排除项；其他排除项保持不变。

### 已接受的打包后收敛修订——2026-08-16

活跃请求中的决策 owner 在测试已打包的 macOS 客户端后，接受了这项 modeled、consequential
修订。它收敛 failure presentation、scrollbar geometry、application-exit protection 与 resize，
但不改变 PTY、VT、protocol version 或 retained-session 权威。

| 字段     | 已接受决策                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | Background history 与 resize failure 可能作为永久 overlay 留在 terminal content 上；in-canvas scrollbar inset 忽略未使用的 grid pixel，从 Panel 边缘看形成 4 CSS pixel 间距；Desktop user quit 会统计所有未停止 presentation，即使其 foreground command 已 idle；resize 后的 child output 可能在 Host 同步前推进 runtime revision，违反 exact-equality assertion 并使 Terminal Host abort。                                                                                                                                                                                                                                                                                                                                                                      |
| 范围     | `apps/web` 拥有 operation-scoped failure presentation；`@orchbee/terminal` 拥有 Panel-relative scrollbar projection 与 hit testing；`@orchbee/workbench` 拥有共享 close-confirmation predicate；Desktop application lifetime 使用该 predicate；Terminal Host 拥有 resize commit、后续 canonical observation 与 Core synchronization。所属 guide、deterministic regression、受影响 TLA+ model 与打包后的 macOS path 投影这些事实。                                                                                                                                                                                                                                                                                                                                |
| 声明     | Background history 与 resize failure 继续作为 typed diagnostic 保留，但绝不创建覆盖 terminal content 的 global overlay；history failure 的用户后果只由 find surface 呈现，而 connection、runtime 与直接 user-action failure 继续提供可见且有界的 feedback。8 CSS pixel history thumb 及其 hit projection 无论 grid 剩余 pixel 数量如何，都与 Panel viewport 右边缘精确保持 2 CSS pixel。User quit 只对既有 Panel-close predicate 要求确认的 presentation 提示：`running` 加 `idle` 不受保护，`active`、`unknown` 与不稳定的受保护 lifecycle state 仍受保护。Resize revision `r` commit 后，`r'` 的 synchronized observation 在 `r' >= r` 时有效；revision 相等时必须是请求 geometry，revision 更新时则以更新的 canonical state 为准，不能覆盖它或使 Host abort。 |
| 边界     | Terminal Host 仍是唯一 PTY、VT、geometry 与 revision 权威。不改变 wire schema、protocol version、persisted document、process-observation meaning、stop semantics 或 recovery identity。Scrollbar 仍由 client 拥有且仅为 overlay；terminal grid sizing 与 PTY column 不为 scrollbar 预留空间。当不存在受保护 presentation 时，preserve 仍是默认自动退出 mode。                                                                                                                                                                                                                                                                                                                                                                                                    |
| 决策权威 | 决策 owner 选择 Panel-relative 2 pixel geometry，而不是 grid-canvas inset；选择 operation-scoped background feedback，而不是永久 global banner；精确复用 Panel-close protection，而不是新增第二套 Desktop activity policy；选择 monotonic resize observation，而不是阻塞 child output 或 replay 较旧 geometry。Local helper 位置、DOM element 选择与 fixture 构造属于 delegated decision。                                                                                                                                                                                                                                                                                                                                                                       |
| 证据     | Reduced、backend-independent DOM-overlay regression 覆盖 grid remainder 与精确 Panel-relative thumb/hit geometry。Renderer-composition test 区分 history、resize、direct-action、connection 与 runtime failure presentation。Workbench 与 Desktop coordinator test 覆盖 idle、active、unknown、不稳定、stopped 与 mixed presentation。Deterministic Host regression 在 resize commit 与 synchronization 之间调度 canonical revision advancement；resize TLA+ model 检查 monotonic observation 与 newest-state preservation，application-lifetime model 检查零个与非零个受保护 presentation。受影响的 macOS system path、package check 与 root verification 仍然必需。                                                                                            |
| 交付     | 先持久化本修订；随后把 failure/scrollbar presentation、shared exit protection 与 Host resize convergence 作为独立检查的切片交付。在 focused 与 root verification 后重新构建并操作 macOS client。每个切片均可通过 revert 其 source、test、model 与所属 guide projection 恢复；不保留 feature flag 或 compatibility shim。                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 排除项   | 本修订不包括新 notification system、改变直接 user-action failure copy、改变 `unknown` activity policy、在自动 preserve 时停止 idle session、改变 PTY resize ordering、protocol negotiation、retained-session migration、scrollbar width/vertical geometry/fade timing，以及无关 multi-Panel behavior。                                                                                                                                                                                                                                                                                                                                                                                                                                                           |

### 已接受的能力、协调与证据修订——2026-08-18

活跃请求的决策 owner 在评审实现与证据审计后，于 2026-08-18 接受了这项 modeled、
consequential 修订。它解决本提案此前保留的能力与产品选择，但不宣称对应实现已经完成。

| 字段       | 已接受决策                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | Desktop file drop 已经存在，而提案仍禁止接受其 capability；runtime theme、cursor default、launch geometry、live attachment capability、shared focus、terminal Action、bell aggregation、URL policy、LAN Clipboard 行为与 compatibility evidence 缺少唯一已接受结果或完整实现。继续保留这些选择会阻止受影响声明收敛，并允许 source、test 与 proposal 权威互相冲突。                                                                                                                                                                                                                                                     |
| 范围       | apps/terminal-host 拥有 terminal default、startup geometry、attachment capability enforcement、shared-focus aggregation 与 terminal effect。@orchbee/terminal 拥有 source-neutral projection、input repair、link presentation、selection export 与 capability projection。@orchbee/workbench 与 apps/web 拥有 scoped Action、semantic context、app-surface theme coordination、bell presentation、本地化 feedback 与 Browser capability 使用。Desktop 拥有 file-path bridge 与 visual launch orchestration。受影响声明为 PR-11、SU-09、SU-13、SU-16、IN-08 至 IN-12、IN-14、HS-07、FS-05、FS-06、LC-05 与 CF-03。      |
| 边界       | Terminal Host 仍是唯一 PTY、Ghostty VT、canonical terminal palette、cursor、geometry、input-admission 与 terminal-effect 权威。Workbench 协调产品 intent 与 presentation，但不能在 typed Host transaction 之外 mutation terminal state。File drop 只通过 Desktop bridge 解析有界 path text，绝不读取 file byte。Browser Clipboard 不获得 server relay 或不安全的隐藏 fallback。File drop 不授权 filesystem-link activation。HTML/VT selection export 与 filesystem activation 保持不可用。不授权新的平行 specification framework 或 per-change evidence artifact。                                                     |
| 决策权威   | 活跃请求的决策 owner 明确接受下表中的替代方案：保留既有有界 Desktop file-drop capability，而不是删除它；采用 Host-owned、revision-pinned theme 与 cursor default，而不是 client-only 权威；采用 mounted-geometry start，而不是中间 launch grid；采用 per-attachment capability revision 与 terminal-level focus aggregation，而不是 static 或 per-view terminal state；采用 Workbench-owned 最小 Action 与去重 bell effect；保持 plain-only selection 与禁用 filesystem activation；有意收窄 Web URL allowlist；采用无不安全 fallback 的严格 Browser Clipboard capability check；并使用既有 owner 分阶段闭合精确证据。 |
| 证据       | 每个行为切片先在最早 owner 处加入 failing regression。Host ordering、capability、focus、launch 与跨 owner theme transition 在 dependent production behavior 前更新适用 TLA+ abstraction。Geometry、focus、capability、clipboard、Action 与 multi-client 声明需要 deterministic system trace。每项 compatibility claim 最终从所属 guide 映射其精确 stable witness 与环境边界；较窄 trace 或 source path 本身不能闭合声明。                                                                                                                                                                                              |
| 交付       | 先持久化本修订。随后交付 explicit cursor default、mounted-geometry launch 与 revision-pinned theme workflow；再交付 live attachment capability、shared focus、Action/context/feedback 与 bell aggregation；然后对齐 Desktop file drop、URL scheme、LAN Clipboard、plain-only export 与禁用 filesystem activation；只在 governed semantic 收敛后闭合精确证据。每项任务是一个可独立评审的 semantic slice，并在下一切片前停止。                                                                                                                                                                                           |
| 发布与恢复 | 不存在 persisted-data migration。新 protocol 或 profile field 在所有仓库内 producer/consumer 更新前必须向后兼容。Missing/stale capability 以 fail closed 处理，downgrade 后绝不 replay queued input，失败的 theme 或 launch preparation 保留此前 canonical state 或保持未启动 child。每个切片均可通过 revert 其 source、test、model 与所属 guide projection 恢复；不保留 compatibility shim 或长期 feature flag。                                                                                                                                                                                                      |
| 排除项     | 本修订不授权 Windows file drop、读取 file content、upload、drop 后自动 Enter、LAN Browser file path、filesystem-link opening、HTML/VT export、http/https/mailto 以外的 scheme、clipboard server relay、execCommand、静默 clipboard fallback、browser-side VT state、multi-Panel focus policy、新 terminal settings surface 或新的 repository-wide specification system。既有 quit/close confirmation 不变；已接受的最小 terminal Action 不增加 confirmation surface。                                                                                                                                                  |

| 声明                | 已接受结果                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| IN-08               | macOS/Linux Desktop 可以把最多 32 个文件解析为经过验证的 absolute POSIX path text。每次 file drop 都进入 Paste Review，绝不追加 Enter、绝不读取 file content；缺少 Desktop bridge、处于 unsupported platform 或 LAN Browser client 时不可用。                                                                                                                                         |
| SU-09               | Workbench 发起一个 presentation revision。Host 在该 revision 原子提交 terminal default、256-color palette 与 cursor color；projected client 与 application surface 只呈现匹配的完整 revision。Superseded、stale、failed、hidden 与 suspended 路径不能暴露 mixed revision。                                                                                                            |
| SU-13               | TerminalProfile 拥有明确的 block、non-blinking 产品 cursor default。Terminal escape sequence 可以在 runtime override shape/blink；RIS/reset 恢复 profile default。                                                                                                                                                                                                                    |
| SU-16               | Visual create 分配 identity 与 lifecycle resource，但不 spawn child。Start 需要预期 mounted geometry，且 child、PTY、Core、首个 Host frame 与首个 client frame 使用该 geometry。Headless 与 test caller 显式提供 geometry；visual path 不使用 timeout 或 80x24 fallback。                                                                                                             |
| PR-11               | Workbench 按 terminal identity 与 Host bell sequence 对 attachment delivery 去重。一个 sequence 产生一次 visual product effect。Sound 默认关闭并 opt-in；operating-system notification 独立 opt-in。Reconnect 与 replay 不重复 effect。                                                                                                                                               |
| IN-09、IN-11、IN-12 | 所属 Workbench 拥有最小 catalog：Copy、Paste、Select All、Find、Find Next、Find Previous、Scroll to Bottom、Clear 与 Reset。Availability 同步；execution 返回 typed result 与本地化 feedback。Available terminal Action 保留当前即时执行且不增加 confirmation surface。Context 在打开时 pinned，并在已允许 Web-link effect 前重新验证。未认领的 Apple Command chord 继续交给 native。 |
| IN-14               | Host 对 attached、visible、input-capable attachment 的 focus claim 做 OR aggregation，只在 aggregate 从 zero 变 nonzero 或从 nonzero 变 zero 时发出 terminal FocusIn/FocusOut。Detach、visibility loss、capability downgrade 与 stale focus event 遵循明确 repair 与 ordering。                                                                                                       |
| IN-10               | 每个 attachment 具有单调 capability revision。Host 拒绝 stale 或 downgraded PTY input；client 立即修复 composition、native settlement、held key 与 mouse state。Upgrade 绝不 replay queued input，同时 local history、selection、search 与 copy 仍被允许。                                                                                                                            |
| HS-07               | Plain-text selection export 是唯一支持格式。HTML 与 raw VT export 保持不可用。                                                                                                                                                                                                                                                                                                        |
| FS-05               | Filesystem-link activation 保持不可用。已接受的 file-drop capability 不代表 opener capability。                                                                                                                                                                                                                                                                                       |
| FS-06               | Detection、hover、context 与 activation 共用一个只包含 http、https、mailto 的 allowlist。Unsupported explicit metadata 可以继续 projection，但不呈现为可激活。                                                                                                                                                                                                                        |
| LC-05               | 每次操作都评估 Browser Clipboard capability。Clipboard API 只在 secure context 中由真实 user activation 执行。Missing 或 rejected capability 返回有界本地化 feedback；不允许 execCommand、server relay、静默 downgrade、重复 retry 或读取 image byte。不安全 LAN client 可以使用经过评审的 manual-copy presentation，但不获得 privileged clipboard path。                             |
| CF-03               | Semantic 收敛后再闭合证据。通过扩展既有 evidence owner，为每项声明提供精确 stable witness、owner-guide mapping、受控环境与明确排除项。如果既有 owner 无法强制完整性，新增 evidence manifest 或 specification framework 需要单独 architecture decision。                                                                                                                               |

## 问题与结果

`5677c6f757d2d294dfee6cdb96549bf719d4c2e8` 切换删除了成熟的浏览器终端组合层，而 projected
替代方案尚未拥有全部渲染、几何、输入、选区、搜索、无障碍和生命周期契约。结果是一组边界
能力丢失，并不能证明由 Host 持有 VT 状态的架构是错误的。

当 projected surface 通过 Host 所有的状态、有界传输和 source-neutral 客户端组合重现已采纳的
可观察行为时，恢复才算成功。只有所属当前权威或另行接受的保留决策明确规定差异时，行为才可
不同。

## 输入与输出

| 类型       | 契约                                                                                                                                             |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| Host 输入  | 经过验证的 OBTF frame、OBPB row window、metadata、selection 状态、terminal effect、生命周期事件与 typed failure                                  |
| 应用输入   | Attachment capability、生命周期意图、presentation 配置、本地化 label、clipboard/navigation capability 与 Workbench focus/context                 |
| 浏览器输入 | 测量后的容器几何、DPR、字体可用性、document visibility、keyboard、IME、pointer、wheel、touch、DOM selection 与无障碍交互                         |
| 参考输入   | 固定的 Orchbee revision、Ghostty revision、canonical 环境、semantic action trace 与采纳的可观察 checkpoint                                       |
| 渲染输出   | Packed Canvas 或 WebGPU 像素、平衡后的几何、原生输入 anchor、离屏无障碍树、live announcement 与明确的生命周期呈现                                |
| 语义输出   | Host command、frame acknowledgement、Action request、clipboard/navigation request、metadata update、telemetry 与 typed feature/lifecycle failure |
| No-op      | 无效 identity、过期 revision、不可用 capability、重复 acknowledgement、无 owner 的 gesture、隐藏动画与被拒绝的生命周期 transition 不改变对应权威 |

不可信的进程、网络、binary、JSON、DOM 和持久化输入在第一个边界验证。Host metadata 缺失表示
不存在，绝不表示可以合成终端状态。应用 capability 缺失表示行为不可用，并通过明确的 Action
可用性或反馈呈现，绝不静默采用更高权限 fallback。

## 所有权与依赖边界

| Owner                             | 权威事实                                                                                                                                                                | 排除事实                                                           |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `apps/terminal-host`              | PTY/进程生命周期、Ghostty VT 与 mode、保留 history、selection endpoint、依赖 mode 的输入编码、canonical geometry、terminal effect 与 revision                           | 产品 UI、clipboard、外部导航、Web theme 与浏览器 gesture policy    |
| `@orchbee/terminal-protocol`      | 版本化 Host wire schema、codec、bound、control kind 与 conformance fixture                                                                                              | 产品策略与渲染                                                     |
| `@orchbee/terminal`               | Packed projection commit、renderer 像素、source-neutral 输入/gesture adapter、有界 history presentation、客户端 search/link projection、IME anchor 与 accessible mirror | PTY byte、VT 解析、产品拓扑、特权 effect 与第二份 terminal palette |
| `apps/web`                        | Electron/Browser 共用组合、scoped Action、presentation theme intent、Workbench bridge、本地化 feature feedback 与环境 capability 使用                                   | Electron 或 Gateway 传输内部机制与 Host 状态 mutation              |
| `apps/desktop` 与 Gateway adapter | Capability-scoped transport、attachment routing、应用生命周期、Browser access policy 与平台 effect                                                                      | VT 解析与另一套 renderer UI                                        |
| `@orchbee/workbench`              | Project/Workspace/Tab/Panel presentation identity、focus/context 组合、title/current-directory policy 与 terminal lifecycle placement                                   | PTY、VT、projection decoding 与平台 effect                         |

每项可变事实只有一个 writer。Host projection 是 canonical terminal 状态；浏览器 overlay 与动画是
可逆 presentation。客户端绝不重建 VT、重解析终端转义序列，或在每个 frame 上重建逐 cell object
graph。应用 effect 绝不把 Host metadata 当作打开文件或外部 scheme 的授权。

## 状态与转换

| 状态组            | 状态                                                                                                   | 已接受的 transition 与取消规则                                                                                                                                                                                                                            |
| ----------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Surface lifecycle | `constructed`、`opening`、`live`、`recovering`、`exited`、`failed`、`disposed`                         | Open 以 transaction 方式分配资源。Recovering 保留最后提交的 frame，并使用有界重试与明确的用户恢复。Exit/failure 撤销 live input/resize capability，但不销毁可读的最终内容。Dispose 取消所有 pending workflow，并且只释放一次所属 DOM 与 attachment 状态。 |
| Projection        | `paused`、`snapshot-required`、`in-flight`、`ready`、`blocked`                                         | Host 拥有 generation/revision ordering。只有精确的 applied acknowledgement 才推进 base。过期或无效输入请求 snapshot，且不 mutation 已提交 store。                                                                                                         |
| Viewport          | `following-bottom`、`viewing-history`、`returning-bottom`                                              | 用户 history intent 请求有界 Host window。由输入拥有的恢复返回 bottom；仅 focus repair 不返回。Eviction、clear、screen change 与 reflow 保留或明确失效 Host pin，并把客户端移到匹配 phase。                                                               |
| Resize            | `unmeasured`、`measured-observer`、`measured-owner`、`request-in-flight`、`canonical-committed`        | 一个 attachment 拥有 Host resize lease。每个 view 都测量并呈现 canonical geometry；只有 owner 请求 PTY geometry。丢失或拒绝的请求保持未收敛，直到 canonical Host commit 或 typed failure 解决。                                                           |
| Input             | `idle`、`composing`、`key-owned`、`mouse-owned-by-application`、`mouse-owned-by-terminal`、`repairing` | Press sequence 期间 gesture ownership 固定。Blur、visibility loss、detach、cancel 或 capability loss 在产生 focus-out 结果前修复 attachment-owned 状态。Composition cancel 不提交文本。                                                                   |
| Search            | `idle`、`debouncing`、`scanning`、`ready`、`unavailable`                                               | Query replacement 取消 stale work。Debounce 或 scan 期间的 navigation intent 被保留。Decoration 保持有界，但不会静默限制方向搜索范围。Active match、reveal 与 selection ownership 原子改变。                                                              |
| Clear/reset       | `validated`、`committed`、`published`、`unknown-result`                                                | Host 在一次 canonical mutation 前验证 authority 与 lifecycle。Clear/reset 绝不注入 PTY byte。丢失结果不会自动 replay。下一份 projection 是完整 snapshot，并修复无效的客户端 transient。                                                                   |

每项 async workflow 都返回 typed failure 或 cancellation。任何 retry 都不能复制 terminal input、
clear/reset、clipboard、link activation，或其他结果未知的用户 effect。

## 不变量

- Terminal Host 是唯一 PTY、Ghostty VT、terminal history、terminal selection 与 mode-dependent
  input 权威。
- 一个 projected frame 原子提交 metadata、geometry、row、generation 与 revision。
- Live projection、row window、search、link、accessibility、queue 与 retained revision 都有明确的
  byte、row、item 或 time bound。
- 一个 terminal 最多有一个 resize lease；非 owner view 不发生 resize fight，也不把 clipping
  重新解释成更小的 canonical grid。
- 每个连接 page 有新的 session 与 attachment identity；loss 或 backpressure 只退休该 page 的资源。
- Desktop 与 Browser 命名同一 Host generation/revision 的 checkpoint 时，暴露等价的 semantic
  row、geometry、selection、accessibility、lifecycle 与已批准的 visual projection。
- Terminal color 来自 Host metadata。Application theme 只能通过明确的 composition workflow
  拥有应用 surface 与 overlay。
- Focus、key、mouse、IME、selection、search 与 hover transition 都有明确 owner 和 cancellation；
  一个客户端不能静默修复另一个客户端的 active input。
- 产品文案在 `en-US` 与 `zh-CN` 中本地化。

## 参考精化

| 字段          | 契约                                                                                                                                                                                                   |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Provenance    | 成熟 Orchbee `be7b4d0`；Host 前加固版本 `44daa5a`；切换版本 `5677c6f`；固定 Ghostty `91f66da`；每项 witness 记录精确 package protocol 与 fixture format                                                |
| Scope         | Semantic row、grapheme、width、wrap、cursor、style、color、geometry、focus、input/IME、mouse、history、selection、link、search、accessibility、lifecycle 与必要 paint 细节                             |
| Projection    | 先比较 canonical state，再比较 geometry/accessibility，最后在 paint 属于声明时比较像素；参考路径与 projected 路径捕获相同 semantic checkpoint                                                          |
| Normalization | 仅在声明不观察这些值时规范化 attachment/session identity、临时路径、process identifier 与非 domain timing；绝不规范化 text、width、wrap、color、geometry、focus、selection、ordering 或 lifecycle 差异 |
| Authority     | 已接受的本地声明优先于偶发的参考行为；否则固定的成熟行为是默认兼容基线。当前安全与 capability 权威在保留变更被接受前继续生效。                                                                         |

Canonical visual 环境固定仓库提供的字体、viewport、DPR、locale、theme、motion preference、
renderer backend 与 logical scheduling。平台探索在其输入被固定并由仓库提供前仅属于维护证据。

## 声明追踪

每个 witness 名称都是稳定标识。切片完成时，在所属指南中把它映射到可执行 core test、
deterministic system trace、differential fixture，或有依据的“不适用”方法。

| 声明  | 可观察契约                                                                                                                                                       | Semantic owner                          | 必需 witness                                 |
| ----- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- | -------------------------------------------- |
| PR-01 | CJK、combining、emoji 与 ZWJ grapheme 保留 Ghostty boundary 与 width。                                                                                           | Host / protocol / terminal              | `terminal-parity/PR-01-graphemes`            |
| PR-02 | Narrow、wide、spacer-tail 与 wrapped spacer-head topology 精确一致。                                                                                             | Host / protocol / terminal              | `terminal-parity/PR-02-cell-topology`        |
| PR-03 | Wide tail 上的 cursor 使用 Ghostty presentation geometry，不改变 canonical coordinate。                                                                          | terminal renderer                       | `terminal-parity/PR-03-wide-cursor`          |
| PR-04 | Soft-wrap 与 hard-line boundary 在 paint、search、link 与 accessibility 中一致。                                                                                 | Host / protocol / terminal              | `terminal-parity/PR-04-wrap-boundaries`      |
| PR-05 | Projected style flag、color、decoration 与 blink phase 匹配 Ghostty。                                                                                            | Host / protocol / terminal renderer     | `terminal-parity/PR-05-styles`               |
| PR-06 | Dynamic default、palette、cursor color、overlay 与 application surface 协调解析。                                                                                | Host / terminal / web / desktop         | `terminal-parity/PR-06-colors`               |
| PR-07 | OSC 8 与 OSC 133 metadata 经过 projection 并由策略受控地激活。                                                                                                   | Host / protocol / terminal / web        | `terminal-parity/PR-07-semantic-links`       |
| PR-08 | Title、current directory、screen、cursor 与 interaction mode 随所属 frame 提交。                                                                                 | Host / protocol / terminal              | `terminal-parity/PR-08-metadata`             |
| PR-09 | Snapshot/delta commit 与 completed-draw checkpoint 原子且由 generation/revision 固定。                                                                           | Host / protocol / terminal / web        | `terminal-parity/PR-09-frame-commit`         |
| PR-10 | Terminal query reply 保留配置的 pinned-Ghostty 行为与有序 routing。                                                                                              | Host                                    | `terminal-parity/PR-10-replies`              |
| PR-11 | 每个 Host bell sequence 对每个 attachment 到达一次；Workbench 按 terminal sequence 去重一次 visual effect，sound 与 operating-system notification 分别 opt-in。  | Host / protocol / terminal / app        | `terminal-parity/PR-11-bell`                 |
| PR-12 | Terminal 发起的 column change 关联 Ghostty、PTY、pixel、Core 与 client geometry。                                                                                | Host / terminal                         | `terminal-parity/PR-12-deccolm`              |
| PR-13 | Application clear/reset 协调 mutation Host VT 状态并发布完整新 baseline。                                                                                        | Host / protocol / terminal / web        | `terminal-parity/PR-13-clear-reset`          |
| SU-01 | 配置的 Latin/CJK/emoji face 在 input/canvas 共享测量前加载。                                                                                                     | terminal / web                          | `terminal-parity/SU-01-font-load`            |
| SU-02 | Metric、weight、italic、ligature、bearing 与 constraint 保持 Ghostty 对齐；仅 foreground paint 的变化不重新划分 shaping geometry。                               | terminal renderer                       | `terminal-parity/SU-02-font-metrics`         |
| SU-03 | Cursor shape、blink、focus、accent 与一个物理像素 stroke 稳定。                                                                                                  | terminal renderer                       | `terminal-parity/SU-03-cursor`               |
| SU-04 | IME anchor 与 preedit 使用 Ghostty cell width，以及已接受的 row-major visible-grid wrapping/clipping projection。                                                | terminal input / renderer               | `terminal-parity/SU-04-ime-geometry`         |
| SU-05 | Selection、search 与 link overlay 一致分层与 clip。                                                                                                              | terminal renderer                       | `terminal-parity/SU-05-overlays`             |
| SU-06 | 从 fit 中减去 padding，剩余 pixel 被平衡且无 clipping 或无效 geometry。                                                                                          | terminal geometry                       | `terminal-parity/SU-06-fit`                  |
| SU-07 | Measured、external、rejected、lost 与 re-entered resize 路径收敛到 Host geometry。                                                                               | terminal / Host                         | `terminal-parity/SU-07-resize-convergence`   |
| SU-08 | DPR change 协调重建 surface geometry、raster state 与 PTY pixel dimension。                                                                                      | terminal / browser                      | `terminal-parity/SU-08-dpr`                  |
| SU-09 | 一个 presentation revision 原子更新 Host default/palette 与匹配的 client pixel、overlay、cursor 和 app surface。                                                 | web / Host / terminal                   | `terminal-parity/SU-09-theme`                |
| SU-10 | History 暴露可见、可拖动、会淡出的 8 CSS pixel overlay scrollbar thumb，并与 Panel viewport 右边缘精确保持 2 CSS pixel。                                         | terminal history presentation           | `terminal-parity/SU-10-scrollbar`            |
| SU-11 | Smooth history motion 是有界、可逆的 presentation，并在隐藏时取消。                                                                                              | terminal history presentation           | `terminal-parity/SU-11-smooth-scroll`        |
| SU-12 | WebGPU 与 Canvas 暴露等价 canonical output、health、fallback 与 recovery。                                                                                       | terminal renderer                       | `terminal-parity/SU-12-backends`             |
| SU-13 | 产品 cursor default 是 block、non-blinking，经 reset 保留，同时 terminal escape 可 override。                                                                    | Host profile / terminal renderer        | `terminal-parity/SU-13-cursor-defaults`      |
| SU-14 | Runtime presentation config 原子重新测量并重绘，不 reopen。                                                                                                      | terminal / web                          | `terminal-parity/SU-14-runtime-presentation` |
| SU-15 | Visibility 与 demand pause transient rendering，并以一个 current draw 恢复。                                                                                     | terminal / web                          | `terminal-parity/SU-15-visibility`           |
| SU-16 | Visual creation 在 child spawn 前等待预期 mounted geometry，使 PTY、Core 与 first frame 不暴露中间 launch grid。                                                 | desktop / Host / terminal               | `terminal-parity/SU-16-startup-geometry`     |
| IN-01 | Browser physical key、modifier、keypad、Kitty action 与 mobile backspace 按 Ghostty 编码；未认领的 Apple Command chord 保留给 native application handling。      | terminal input / Host                   | `terminal-parity/IN-01-keyboard`             |
| IN-02 | IME 只 commit 一次，cancel 不 commit，所有 lifecycle repair 清除 transient input。                                                                               | terminal input                          | `terminal-parity/IN-02-composition`          |
| IN-03 | Paste inspection、review、ordering、capability 与 bracketed-mode encoding 构成一个决定；纯图片 clipboard input 在不读取 image byte 时发出一个 Control+V signal。 | terminal / web / Host                   | `terminal-parity/IN-03-paste`                |
| IN-04 | Focus repair ordering 精确，internal movement 不重复，mount 不窃取 focus。                                                                                       | terminal / Workbench                    | `terminal-parity/IN-04-focus`                |
| IN-05 | Gesture 期间 mouse ownership 固定，Shift 安全赋予 terminal selection/link ownership。                                                                            | terminal input / Host                   | `terminal-parity/IN-05-mouse-ownership`      |
| IN-06 | Pointer shape 由 selection、rectangle、application mouse 与 link policy 导出。                                                                                   | terminal presentation                   | `terminal-parity/IN-06-pointer`              |
| IN-07 | Local paging 与每个允许的 input source 执行共用 history/selection transition。                                                                                   | terminal input / history                | `terminal-parity/IN-07-pre-input`            |
| IN-08 | Text drop 遵循 paste；有界 Desktop file drop 总是进入 Paste Review，缺少已接受 Desktop path capability 时不可用。                                                | terminal / app                          | `terminal-parity/IN-08-drop`                 |
| IN-09 | Terminal Action 使用所属 Workbench scope，只消费 available binding，不增加 confirmation surface，否则保留 native handling。                                      | Workbench / web                         | `terminal-parity/IN-09-action-scope`         |
| IN-10 | 单调 attachment capability revision 抑制 stale/downgraded PTY input，同时保留已允许的 local selection/history 行为。                                             | terminal / attachment owner             | `terminal-parity/IN-10-read-only`            |
| IN-11 | Context-menu intent 遵守 mouse ownership，只暴露 pinned semantic context，并在已允许 effect 前重新验证。                                                         | terminal / Workbench                    | `terminal-parity/IN-11-context`              |
| IN-12 | 已接受的最小 terminal Action catalog 具有同步 availability、typed execution 与本地化、可感知的 completion/failure feedback。                                     | Workbench / web                         | `terminal-parity/IN-12-actions`              |
| IN-13 | Wheel policy 保留 delta mode、axis、local history、mouse reporting 与 alternate-screen fallback。                                                                | terminal input / Host                   | `terminal-parity/IN-13-wheel`                |
| IN-14 | Shared-terminal focus 对 attached、visible、input-capable attachment claim 做 OR aggregation，只发出 aggregate zero/nonzero transition。                         | Host / app                              | `terminal-parity/IN-14-shared-focus`         |
| IN-15 | Abrupt attachment loss 不会遗留或跨客户端释放 attachment-owned key/mouse state。                                                                                 | Host / terminal                         | `terminal-parity/IN-15-detach-repair`        |
| IN-16 | Concurrent client 共用一个有界 terminal-global FIFO，不 replay 或 reorder。                                                                                      | Host                                    | `terminal-parity/IN-16-input-fifo`           |
| IN-17 | Mobile activation 打开 virtual keyboard，并保持 exactly-once input 与 selection。                                                                                | terminal browser input                  | `terminal-parity/IN-17-mobile`               |
| HS-01 | Bottom following 与有界 history navigation 保留一个一致 visible window。                                                                                         | Host / terminal history                 | `terminal-parity/HS-01-history-window`       |
| HS-02 | Resize reflow 在查看 history 时保留 Host-pinned logical top。                                                                                                    | Host / terminal history                 | `terminal-parity/HS-02-reflow`               |
| HS-03 | Clear、eviction 与 screen change 产生匹配 viewport 与 live/history state。                                                                                       | Host / terminal history                 | `terminal-parity/HS-03-history-transition`   |
| HS-04 | Click count、word/line/output、threshold、endpoint 与 rectangle semantic 明确且稳定。                                                                            | terminal gesture / Host selection       | `terminal-parity/HS-04-selection-gesture`    |
| HS-05 | Autoscroll、outside release、lost capture、blur、cancel 与 race 具有一个 copy consequence。                                                                      | terminal gesture / Host selection       | `terminal-parity/HS-05-selection-cancel`     |
| HS-06 | Selection eviction 不能 export replacement content，clipboard completion 保持 pinned。                                                                           | Host selection / app                    | `terminal-parity/HS-06-selection-eviction`   |
| HS-07 | Plain export 持续维护；HTML 与 raw VT exposure 保持不可用。                                                                                                      | Host / terminal / app                   | `terminal-parity/HS-07-selection-export`     |
| HS-08 | 已允许 programmatic/accessibility selection workflow 保留 absolute endpoint。                                                                                    | Host selection / terminal               | `terminal-parity/HS-08-selection-commands`   |
| HS-09 | Scrollback 有界，zero 禁用 retention，已允许 update 确定性 eviction。                                                                                            | Host configuration                      | `terminal-parity/HS-09-scrollback`           |
| HS-10 | Copy-on-select、opt-out、manual copy 与 exactly-once 可感知结果保持一致。                                                                                        | terminal / app                          | `terminal-parity/HS-10-copy`                 |
| FS-01 | Search option、mapping、navigation、query seeding、reveal 与 active selection 保持一致。                                                                         | terminal search / Host selection / web  | `terminal-parity/FS-01-search`               |
| FS-02 | Search scheduling、navigation intent、invalidation、cancellation、bound 与 reveal 确定。                                                                         | terminal search                         | `terminal-parity/FS-02-search-scheduling`    |
| FS-03 | Link 以 precedence 与精确 cell mapping 覆盖完整的有界 logical line。                                                                                             | Host rows / terminal links              | `terminal-parity/FS-03-links`                |
| FS-04 | Hover 在 content、viewport、geometry change 时失效；activation 重新验证 cell。                                                                                   | terminal links / geometry               | `terminal-parity/FS-04-link-invalidation`    |
| FS-05 | Filesystem activation 保持不可用，Desktop file-drop capability 不代表 opener capability。                                                                        | app capability                          | `terminal-parity/FS-05-file-links`           |
| FS-06 | Detection、presentation、context 与 activation 共用一个只包含 `http`、`https`、`mailto` 的 allowlist。                                                           | terminal / app capability               | `terminal-parity/FS-06-url-policy`           |
| AC-01 | Accessible row 保留 space、grapheme mapping、position 与有效 navigable structure。                                                                               | terminal accessibility                  | `terminal-parity/AC-01-accessible-rows`      |
| AC-02 | DOM 与 Host selection 双向映射且不拆分 grapheme。                                                                                                                | terminal accessibility / Host selection | `terminal-parity/AC-02-dom-selection`        |
| AC-03 | Cursor 与 changed output 有序 announce，history 不变成虚假 live output。                                                                                         | terminal accessibility                  | `terminal-parity/AC-03-announcements`        |
| AC-04 | Deletion 与 line-kill announcement 原子处理 wide 与 surrogate grapheme。                                                                                         | terminal accessibility / Host output    | `terminal-parity/AC-04-deletion`             |
| LC-01 | Open、rollback、retry、duplicate open、dispose 与 DOM ownership 确定。                                                                                           | terminal lifecycle                      | `terminal-parity/LC-01-open-lifecycle`       |
| LC-02 | Failure、recovery、final frame 与 exit 具有明确 precedence，且不破坏 geometry。                                                                                  | terminal / web lifecycle                | `terminal-parity/LC-02-failure-presentation` |
| LC-03 | Desktop 与 LAN Browser 对一个 Host checkpoint 暴露等价 semantic。                                                                                                | web / desktop / Gateway                 | `terminal-parity/LC-03-dual-client`          |
| LC-04 | PTY launch environment 修复 locale/terminal identity，并移除 parent-only color policy。                                                                          | desktop / Host launch                   | `terminal-parity/LC-04-environment`          |
| LC-05 | 每次 user-activated secure-context operation 都检查 Browser Clipboard；capability loss 改变 Action availability，并给出无不安全 fallback 的有界本地化反馈。      | web / Browser capability                | `terminal-parity/LC-05-lan-capability`       |
| LC-06 | Host title/current-directory metadata 以明确 rename/launch policy 到达 Workbench。                                                                               | terminal / web / Workbench              | `terminal-parity/LC-06-metadata-bridge`      |
| LC-07 | 一个 resize owner 与所有 non-owner 呈现 canonical geometry，并确定性恢复 ownership。                                                                             | Host / desktop / web / terminal         | `terminal-parity/LC-07-resize-lease`         |
| LC-08 | Browser access 默认 loopback，LAN 需明确意图，stop 时撤销 capability。                                                                                           | desktop / Server / Gateway              | `terminal-parity/LC-08-browser-access`       |
| LC-09 | 每个 page 隔离 session/attachment lifetime，并从完整 current snapshot reopen。                                                                                   | desktop / Gateway / Host                | `terminal-parity/LC-09-client-isolation`     |
| LC-10 | Exit、failure、stop 与 reconnect 是不同、本地化、可读且可操作的状态。                                                                                            | Workbench / web / terminal              | `terminal-parity/LC-10-outcomes`             |
| LC-11 | Recovery 保留真实 last frame，具有有界 retry，并暴露 manual recovery。                                                                                           | web / terminal lifecycle                | `terminal-parity/LC-11-recovery`             |
| LC-12 | Typed feature failure 保留 operation、correlation、retryability 与 user consequence。                                                                            | Host / brokers / terminal / web         | `terminal-parity/LC-12-feature-failures`     |
| CF-01 | 恢复不引入 browser VT authority 或逐 frame object-grid 重建。                                                                                                    | all owners                              | `terminal-parity/CF-01-host-authority`       |
| CF-02 | Projection、history、search、link、accessibility 与 revision 保持有界。                                                                                          | Host / protocol / terminal              | `terminal-parity/CF-02-bounds`               |
| CF-03 | Visual evidence 固定所有 domain-relevant 环境输入，不规范化 semantic difference。                                                                                | terminal / system evidence              | `terminal-parity/CF-03-visual-environment`   |

## 证据与交付

每个 defect 先在最早错误 owner 处加入精简的 failing regression。兼容声明使用固定的 differential
corpus 或 versioned golden trace。Geometry、timing、gesture、focus、accessibility 与 platform
声明还需要 deterministic system trace，并控制 font、clock、frame、geometry、capability 与
interaction。Host concurrency 或独立排序事件的变更在影响抽象时更新并运行所属 TLA+ model。

切片按以下依赖顺序推进：

1. projection 与 Host control（`PR-01` 至 `PR-13`）；
2. surface geometry、theme、cursor 与 runtime presentation（`SU-01` 至 `SU-16`）；
3. scroll presentation 与 history（`SU-10`、`SU-11`、`HS-01` 至 `HS-03`、`HS-09`）；
4. input、focus、mouse、context 与 Action（`IN-01` 至 `IN-17`）；
5. selection、search、link 与 accessibility（`HS-04` 至 `AC-04`）；
6. application lifecycle、launch environment 与 dual-client convergence（`LC-01` 至
   `LC-12`）。

一个切片只有在所属当前契约、failing-before regression、focused check 与受影响 package gate
全部通过后才推进。完成还要求 root `pnpm verify` 与所有选中的 change-triggered correctness
command 通过。

## 已接受的能力与产品边界

活跃请求的决策 owner 于 2026-08-18 在
[已接受的修订](#已接受的能力协调与证据修订2026-08-18)精确范围内，接受了此前在此保留的
选择。接受决定授权 governed implementation，但不代表已经完成。

- Capability expansion 在所属 implementation、negative case、typed failure、owner
  documentation 与选定 system evidence 收敛前保持不可用。
- 已接受的 Desktop file-drop path capability 不授权 LAN Browser path、filesystem-link
  opening、file read、upload、Windows support 或自动执行 command。
- Plain-text-only selection export 与 unavailable filesystem activation 是有意产品结果，不是
  临时缺口。
- External URL interaction 只允许 `http`、`https` 与 `mailto`；增加其他 scheme 是新的
  reserved decision。
- Clipboard server relay、`execCommand`、静默 fallback、HTML/VT export 与新的 evidence
  specification framework 仍不在本授权范围内。
- 任何需要 materially different owner、public contract、security boundary、product outcome 或
  evidence limit 的实现，必须在 governed edit 前返回 decision stage。
