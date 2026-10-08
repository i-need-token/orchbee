# @orchbee/workbench

Orchbee 应用程序的共享产品语义 React shell。

## 适用范围

Workbench 在应用程序拥有的 `ActionRuntime` 中注册其产品操作。侧边栏控件、快捷方式、选项卡条命令、面板按钮和菜单调用这些注册；注册的处理程序是其语义结果的唯一编写者。每个安装的终端视口在同一运行时中创建一个子范围，并拥有稳定的最小终端目录：复制、粘贴、全选、查找导航、提示符导航、保留历史导航、查看和复制完整保留历史、查看和复制上一条命令输出、可见 Hints、允许的链接激活、清除和重置。注入的表面将实现注册到该范围中，并仅为准备好的上下文菜单调用提供修订固定的语义上下文。连续分隔符调整大小会随着规范面板布局的变化而更新，并在交互结束或取消时保留最后的有效布局。面板拖动预览保持可逆的交互状态，而最终的拖动结果会调用相应的Action。

该包装拥有一个全高、水平可调整大小的两面板外壳。左侧边栏默认为`250px`，约束为`200px`–`400px`；右侧面板填充剩余宽度并通过 `@orchbee/tab-strip` 渲染活动的独立或工作区终端上下文。横向外壳镶边高度为 `36px`：其中的 `28px` 侧边栏控件在该行内居中，侧边栏内容上内边距和本机拖动表面也使用同一边界。侧边栏控件和`Mod+B`快捷方式隐藏或恢复侧边栏。在两种状态下，控制器均保持安装在一个固定的外壳位置。当侧边栏隐藏时，选项卡条表面内的透明引导位置会保留其足迹，因此控件永远不会与第一个选项卡重叠或绘制单独的背景。本地化侧边栏提供独立的 `Terminal` 位置、`Search`、可扩展的项目和工作空间层次结构以及底部锚定的 `Settings` 操作。项目行控制公开，而不更改活动终端上下文。工作区行切换到其独立的选项卡和面板上下文；返回终端将恢复现有的独立上下文。当活动工作区所属的项目折叠时，该项目行会成为可见的选中投影；重新展开后，选中表面会回到工作区行。

右键单击空白选项卡条空间会显示本地化的方向操作。工作台在顶部水平条和左侧垂直条之间切换条和内容布局，而选项卡、组和新选项卡控件保留其现有的上下文菜单。

当显示中的终端上下文没有 Tab 时，Workbench 会渲染空白内容区域，不显示居中的创建控件。Tab Strip 的新建 Tab 控件、快捷键、菜单和 Action 仍然可用。Workbench 绝不会把空上下文视为终端创建意图，因此关闭最后一个 Tab 不会重新创建 Tab；应用启动策略仍归组合应用所有。

shell 用 `@orchbee/ui` 组合（compose）原语，而不拥有 PTY 进程、文件系统或 Git 效果、设置目标、身份验证、路由或代理编排生命周期。 Workbench 拥有统一的搜索状态和表示，而语义所有者仍然负责可搜索的读取模型和操作注册。机器可读范围合约位于 [`project.json`](project.json) 中。

Workbench 组合显式区分为 `preview` 或 `managed`。两者都会接收应用拥有的终端 Presentation 标识分配器与稳定的初始 Presentation ID。托管组合还接收操作/移除 ID 分配器和窄化的命令调度器；Workbench 绝不分配 Host 终端 ID，也不以 Host 终端标识执行拓扑或生命周期变更。终端表面接缝只向下述响铃效果协调器公开不透明的 Host 终端标识。动态 Presentation ID 不透明，且删除 Presentation 后不得复用。

托管应用可以用恢复的 Presentation 初始化独立上下文。每个被植入（seed）的 Presentation 保留应用拥有的 PresentationId 与启动 CWD，并投影成一个 Tab；Workbench 仍不会推断或持久化 Project、Workspace、Group、Split、进程或 Host 标识。运行时按 PresentationId 在所有终端上下文中路由每个 provision、exit 与 close 结果；目标缺失、重复或过时时会拒绝，且不改变其他 Presentation。

应用程序可以注入 `terminalSurface` 组件。Workbench 仅为显示中 Tab 的可见 Panel 布局内的终端 Presentation 挂载它，并传入 Presentation 模型、视口的应用所有 Action runtime/scope、窄化的语义上下文注册接缝、响铃效果准入函数和 `autoFocus` 投影，而绝不传入 Host 能力。非活动 Tab、其他 Workspace、后台 Project 和非活动子终端不会挂载该表面。Workbench 只对活动 Tab 所记忆的聚焦 Panel 中正在显示的终端设置 `autoFocus`。当该投影变为 true、true 目标被挂载，或其恢复后的 renderer 进入 ready 时，注入表面会让真实终端输入取得焦点。因此，新 Tab 会聚焦第一个 Panel 的终端；返回某个 Tab 会聚焦该 Tab 记忆的 Panel 终端；拆分会聚焦新 Panel 的终端。关闭聚焦 Panel 时选择最近聚焦且仍然存活的 Panel；只有历史中没有存活候选时，才依次回退到前一个和后一个视觉叶节点。关闭非聚焦 Panel 会保留并恢复原聚焦终端。右键菜单会保留打开前的焦点元素，并在关闭时恢复，除非所选结构命令把焦点转移给另一终端。这让 Desktop 可把 attachment 与 renderer 生命周期绑定到真实视口需求，同时保留 Workbench 对逻辑拓扑和产品焦点的所有权。

一台安装式工作台面板-工作区终身拥有一台 `WorkbenchTerminalBellEffectCoordinator`。每个表面仅报告不透明的终端身份和正主机响铃序列。协调器保留每个终端的可合并序列间隔，因此重复的、过时的、重新连接的和重放的附件传送都会被拒绝，而不会丢失乱序到达的不同序列。确切地说，第一个被承认的交付可以创建视觉产品效果并调用应用程序功能。 Workbench 本身不执行音频或操作系统通知；这些仍然是两个独立的、默认不存在的应用程序功能。

稳定的 `terminal-parity/PR-11-bell` 见证把多个生产终端表面与一个 Workbench 协调器组合起来，并证明重复/重连抑制、交错序列准入，以及各可选效果具有独立失败边界。稳定的有界面 `terminal-parity/IN-09-action-scope`、`terminal-parity/IN-11-context` 与 `terminal-parity/IN-12-actions` 见证会在其包含的 Workbench 作用域中组合最小注册表，并证明：未认领或不可用的 Apple Command 组合键交给原生处理；语义上下文在执行时固定并重新校验；同步可用性；类型化结果；无需额外确认；以及本地化反馈。这些受控 Canvas 夹具捕获应用效果；它们不声称原生音频、OS 通知、剪贴板或 URL handler 会实际送达，也不会让 Workbench 成为终端状态权威。

地址为权威 Client View 输入的应用程序可能会注入 `WorkbenchNavigationPort`。然后，在工作台改变活动的 Client View 状态之前，工作区、独立上下文和选项卡选择操作会委派导航；应用程序更新浏览器历史记录并将生成的路由应用回运行时。如果没有此端口，Workbench 将保留其本地内存中导航行为。 Workbench 不解析路径或自己的路由器历史记录。

`@orchbee/workbench/terminal-presentation` 还提供运行时中立的终端表示身份、生命周期契约和边界模式。平台应用程序使用该狭窄的入口点，而不是将 React 工作台图导入到主运行时或预加载运行时中。

## 键盘焦点路由

Workbench 拥有其普通产品区域之间的语义焦点路由。`Mod+F6` 与
`Mod+Shift+F6` 调用可配置的下一个和上一个区域 Action；直接 Action 可聚焦活动终端、
Tab Strip 或可见侧边栏。焦点环只包含当前可用区域，顺序为：活动终端、Tab Strip、
可见侧边栏。隐藏侧边栏和缺失终端会被跳过。临时对话框、菜单、搜索、Settings 及其他
独占键盘表面不属于该环，并保留其更窄的焦点作用域。

Tab Strip 是一个复合焦点站点。首次进入时记忆的 roving 项为活动 Tab，再次进入时为
最近仍存活的键盘聚焦项。折叠分组头是其中隐藏活动 Tab 的可见代表；隐藏的分组子项
绝不接收焦点。与方向一致的方向键会在边界回绕，只移动焦点而不激活；Home 和 End 移到第一个和最后
一个可见项；Enter 和 Space 激活聚焦 Tab 或切换聚焦分组；Shift+F10 或 Menu 键打开
精确聚焦项的上下文菜单。新建 Tab 按钮在顺序焦点中位于该复合控件之后。关闭、分组、
折叠或移动聚焦项后，优先把焦点保留在同一可见标识，否则依次选择最近可见项、活动
Tab 或其折叠分组代表，最后选择新建 Tab 按钮。

收到新的正数侧边栏焦点请求时，依次聚焦可见的当前位置、代表已折叠 Workspace 的可见活动
Project，最后才使用独立 Terminal 回退。聚焦绝不激活位置或展开 Project。只有现有 Project
滚动区域可以移动：使用 `clientTop` 和 `clientHeight` 测量其内容可视区域，并将最近边缘的垂直
调整限制在自身滚动范围内。完全可见的行与已覆盖整个可视区域的超高行保持不动；其他超高行
对齐较近边缘。零高度几何与 Terminal 目标不滚动。重复请求 token 与单独发生的数据更新不执行
任何操作。现有侧边栏焦点 Hook 测试覆盖这些决策；导航 Story 为可见焦点、导航不变与外层滚动
不变提供受控浏览器场景。

### 焦点路由建模记录

- **问题与范围：** 真实 PTY 输入拥有焦点时，键盘优先用户需要一条有界路径离开终端、操作 Tab Strip 或侧边栏，并在不使用指针的情况下返回精确的活动终端。用户用指针点击侧边栏“终端”入口或任一 Workspace 行时，都期望导航完成后由目标终端输入获得焦点。本变更包括五个公开 Workbench 焦点 Action、两个可配置区域循环绑定、指针点击终端位置激活、Tab Strip roving focus 与键盘上下文菜单入口、terminal-surface 焦点请求投影、本地化、Browser 绑定持久化以及受控 Browser/Desktop 证据。通过键盘激活“终端”或 Workspace 按钮、快捷键与程序化位置变更、侧边栏树展开/折叠、键盘重排、快捷键规则 v2、Settings 深链、屏幕阅读器一致性和 manual-only 原生窗口焦点测试都不会通过该指针点击工作流请求焦点。
- **输入与数据：** 可信输入为已挂载的 Workbench 区域注册表、侧边栏可见状态、活动位置、精确目标位置、活动 Tab 与显示中终端标识、Tab Strip 方向与可见项顺序、单调递增的终端焦点请求 token、规范化后的语义键盘命令、规范化后的指针/非指针终端位置激活来源、匹配的激活 Action 结果和当前终端投影。DOM 焦点与 click 事件是不可信的展示输入；只有验证目标与指针来源后才可更新焦点意图。`WorkbenchFocusRegion` 为 `terminal`、`tab-strip` 或 `sidebar`；`WorkbenchFocusState` 拥有当前普通区域与最近仍存活的 Tab Strip 项。`WorkbenchTerminalLocationActivationState` 至多拥有一项发起客户端的指针意图及其精确目标位置，而 `TabModel` 与 Project/Workspace state 仍对活动 Tab 和位置标识具有权威性。
- **模型与转换：** `resolveWorkbenchFocusTarget` 是一个同步纯解析器，输入方向、当前区域和可用的已注册区域。前进遵循终端 → Tab Strip → 侧边栏 → 终端；后退为其逆序；两者跳过不可用区域，并拒绝没有不同目标的循环。直接聚焦只解析一个请求区域。未知或陈旧的当前焦点在任一方向上都先以活动终端为锚点，其次为 Tab Strip，最后为侧边栏。`transitionWorkbenchTerminalLocationActivation` 仅为规范化后的指针激活启动请求，接纳乱序到达的 Action settlement 与精确目标终端投影观察，只在两者都 ready 后发出一条焦点 command，以最新点击取代旧指针意图，并在失败、dispose、非指针激活或出现不同的非目标位置时取消。Tab Strip roving 在可见 Tab 与分组头之间采用手动激活：与方向一致的方向键会回绕，且与 Home/End 一样只改变记忆的焦点项；Enter 或 Space 把 Tab 委托给 `tabs.tab.select` 或切换分组；上下文菜单按键委托精确聚焦项；结构变更依次按同一可见标识、隐藏活动 Tab 的折叠分组代表、最近可见邻项、活动 Tab、新建 Tab 控件协调记忆项。
- **输出、失败与空操作：** 成功的区域解析为一个已注册目标发出一项类型化焦点请求，并消费其快捷键。指针终端位置激活会调用现有 standalone 或 Workspace-location Action，并且只在该调用完成或 unchanged 且精确目标的活动 Presentation 可用后发出一条终端焦点请求。聚焦终端会递增请求 token；注入的 terminal surface 会为精确活动 Presentation 保留更新请求，并在该 Presentation 挂载且 ready 后执行。键盘 Button 激活、快捷键、Workspace 创建后的激活和程序化位置变更继续执行原有操作，不发出这条焦点请求。缺失、隐藏、陈旧、禁用、失败、取消、被取代、独占表面或不存在的目标都不发出点击拥有的焦点 command。存在但尚未 ready 的终端保持 pending；对已经聚焦目标重复直接聚焦是刻意的 unchanged 结果，不会虚构另一次区域转换。
- **不变量与所有权：** 至多一个普通 Workbench 区域拥有语义焦点。指针点击焦点绝不早于精确位置激活成功或精确目标终端投影，绝不指向目标活动 Tab 所记忆聚焦 Panel 以外的终端，并且只会为最新存活点击发出至多一次。非指针激活绝不创建指针焦点意图，陈旧 Action 完成也不能恢复已替换或已 dispose 的意图。恰有一个可见 Tab Strip 的 Tab 或分组头拥有 `tabIndex=0`，其他复合项全部为 `-1`；折叠分组中隐藏的子项绝不接收焦点；仅移动 Tab 焦点绝不改变活动 Tab 标识；激活使用既有位置或 Tab Action；Tab Strip 拥有键盘焦点时，终端 `autoFocus` 不能抢走焦点。Workbench 拥有激活来源规范化、目标位置验证、区域选择、请求序列和终端目标；`@orchbee/tab-strip` 拥有其复合焦点投影；Project/Workspace state 与 `TabModel` 拥有位置和 Tab 选择/顺序；注入的 terminal-surface 适配器拥有真实输入焦点调用但不能更换目标；Web 拥有有效绑定和持久覆盖。
- **兼容性、交付与回滚：** 已接受的默认键为 `Mod+F6` 与 `Mod+Shift+F6`，保留未修饰功能键供 PTY/TUI 输入。Browser 覆盖从 v20 迁移至 v21：复制每项有效旧覆盖、移除与当前默认值相等的条目，并拒绝迁移后的冲突；v20 记录保持不变以支持回滚，旧构建有意无法读取仅存在于 v21 的编辑。交付顺序为本契约、终端位置指针激活状态模型、纯解析器与协调测试、Tab Strip 语义、Workbench/terminal-surface 集成，最后是 Web 持久化和受控系统证据。从指针来源适配器中移除 Workspace 行，可独立恢复其原有焦点中立行为；一起回退 v21 目录与迁移、焦点 Action/状态、Tab Strip 投影、终端位置状态模型和终端焦点请求字段，则可恢复更早的键盘行为且不删除持久数据。
- **验证与边界：** 确定性解析器测试覆盖两个方向、隐藏/缺失区域、未知与陈旧焦点、单例拒绝、直接聚焦、重复空操作及独占表面回落。终端位置指针激活模型与 Hook 测试覆盖 standalone 和 Workspace 目标、结果先于投影、投影先于结果、目标不可用、失败、取代、非指针激活、dispose 及恰好一次焦点。Tab Strip 组件测试从唯一自然 Tab 入口进入，并覆盖两个方向、Home/End、手动激活、上下文菜单按键、新建 Tab 顺序、移除/分组折叠协调和焦点保持。Workbench 测试覆盖路由发布后的 standalone 与 Workspace 指针点击焦点、焦点中立的键盘激活、Action 可用性、仅在解析到目标后消费快捷键、焦点恢复，以及 Tab Strip 拥有焦点时抑制终端自动聚焦。Web 迁移/目录/本地化测试覆盖 v20→v21 保留、冲突、Settings 投影和回滚。已登记的 `WorkbenchTerminalLocationActivation` TLA+ 模型会在声明边界内检查乱序 acceptance/projection、最新意图安全性、非指针排除、失败、dispose 与公平焦点进展。其 `ObserveReady` 输入是确定性精确位置/当前 Presentation 解析器的规范化输出；TLA+ 不重复结构化标识解析。受控 Browser 与 Desktop 真实 PTY trace 执行终端 → Tab Strip → 菜单 → 终端及反向路由，并覆盖指针 Workspace 导航进入其精确终端输入；原生窗口抢焦点系统测试仍明确排除，除非另行授权。

纯 Project、Directory、Workspace、placement、CWD 与 transition contract 可从
`@orchbee/workbench/project-workspace` 获取。应用 owner 可以通过这个非 React subpath 执行一项权威
mutation，而无需导入可视 Workbench 或其 runtime graph。

经过验证的共享文档读取投影、有界 Terminal-output read contract 与精确目标终端意图创建可从
`@orchbee/workbench/semantic-commands` 获得。这个不依赖 DOM、React、存储和传输的入口会列出规范的
Project、Directory、Workspace、Tab、Panel 与终端标识，并把一次终端创建委托给现有 Workbench runtime。
Tab 读取会保留规范 Tab collection 顺序，并汇总 Panel 与 Terminal-presentation 成员关系。精确目标的 Panel
读取会保留规范的深度优先、从左到右叶节点顺序，并发布从一开始的位置；它绝不替换成 active Tab 或 focused
Panel。两种读取都排除 Client View 的 focus、selection、maximization、active-child、split-weight 与 geometry
状态。

同一入口拥有 `workbench.terminal.output.read` 的 input、output 与 descriptor 含义。Input 固定一个
canonical Terminal presentation identity，在 1–1,000 的范围内默认使用 80 行。Output 是一个不超过
128 KiB 的纯 UTF-8 tail，并携带精确 Host generation、revision、screen、总行数与返回行范围，以及
truncation 状态；soft-wrapped row 会被拼接。Contract 不包含 ANSI style、graphic、hyperlink、raw VT byte、
input、resize、focus、wait、matcher 或 durable-history authority。Workbench 定义并验证这个可序列化含义，
但不读取 Terminal Host state。组合 application 必须从一个精确 capture-only frame 取得成功 result，dispose
其 lease，并以不含文本的方式表示 stale、retired、unavailable、cancelled 与 cleanup-failed outcome。

创建会保留活动 Client View 中已有的选择；空目标会把唯一新建的 Tab 作为 Tab model 所要求的有效性后备项。
产品 Action 可以另行只选择精确返回的 Tab。该入口还发布严格且带版本的 location input 与
committed-result schema；组合应用提供已提交 revision，并继续负责持久提交、进程协调、调用方授权和签名的
revision-fenced 分页。

## 体验目录

Workbench 在 [`docs/experience`](docs/experience) 中维护已审核的交互与业务组件清单。这些记录说明预期用途与排除用途、不变式、组合依赖、实现根及 Storybook 检查状态，不会因此把私有组件变成公共导出。构建新的复杂 Workbench 界面前，请先从仓库根目录运行 `pnpm experience:find --owner @orchbee/workbench` 查找组件，再用 `pnpm experience:show <id>` 查看选中记录。

## 项目和工作空间

维护的内存模型、所有权、回退、CWD、重复和正确性合约位于 [`docs/project-workspace.md`](docs/project-workspace.md) 中。

`WorkbenchProjectWorkspaceRuntime` 拥有独立终端上下文，并为每个 Workspace 分别拥有一套 Tab 模型和 Panel 工作区。Project 与 Workspace 操作使用稳定的 Action ID。Project 创建控件会打开公共的 Folder 获取菜单。应用可以注入窄化的 `projectFolderAcquisition` 能力，以选择一个已有 Folder。Desktop 通过原生选择器提供该能力，并返回一个经 Host 规范化和验证的路径；Browser 与 Gateway 不提供该能力，继续使用手动路径输入适配，但不声称路径确实存在。选择路径后，会原子创建 Project、首个 Folder、Primary Workspace 和 Workspace 终端投影。Project 管理通过 `Add Folder` 复用同一能力；当 Desktop 还提供 SSH attachment 时，该行会给出明确的 local 与 SSH 选项，每个已发布 Environment Folder 都带有本地化且可见的 SSH 标识。权威 remote transition 只追加一个 unavailable Folder，允许来自 Project 唯一 Environment 的多个 path，并且不改变 Project name、primary Directory、Workspace、active location 或 terminal context。Workbench 在提交迟到的 local 结果前会重新检查最新 Project 标识和完全重复的路径。取消与有界失败不会改变 Project；一个已挂载 Workbench 内也至多只有一项选择处于活动状态。`New Folder…` 仍然可见但不可用，因为当前没有组合执行文件系统变更。创建 Workspace 会在其主要 Project Folder 中启动一个 Shell 投影，并且只在当前 canonical 投影仍包含目标时为发起客户端激活该精确 Workspace；普通 Folder 选项会同步提交并激活，应用拥有的 Worktree 结果则会在发布较晚时等待。后续投影可以移除此前成员或变为空集合，绝不会根据历史发布授权激活。预览复制会把 Tab 组、Panel 布局、名称、启动配置选择和记录的启动 CWD 复制到全新身份，同时重置由进程派生的 Tab、Group 与 Presentation 状态。托管组合不开放 Project 或 Workspace 复制，因为它无法静默克隆真实进程。终端投影会把启动 CWD 与 Host 报告的当前 CWD 分开。目录刷新会保留稳定身份、顺序和主要引用。删除 Workspace 只处置其 Orchbee 终端上下文；移除 Project 只处置 Orchbee 元数据与上下文，绝不声称会删除文件夹或 Git 工作树。

该包接受主机规范化的目录描述符和浏览器输入的路径，并且不执行文件系统、Git、PTY 或持久性效果本身。 Desktop拥有当前本机existing-Folder选择和验证适配器；文件系统变更、正在进行的目录观察和 Git 效果仍然是应用程序拥有的未来工作。

## 统一搜索

维护的行为、所有权、转换和正确性模型位于 [`docs/unified-search.md`](docs/unified-search.md) 中。

侧栏搜索控件与平台的全部搜索绑定——Apple 上为 `Mod+Shift+F`，标准平台为 `Mod+Shift+K`——会打开一个上部视口模态界面，其中包含 `All` 过滤器和空查询。`Mod+Shift+P` 会以 `Actions` 过滤器打开同一个界面。每次全局打开都会启动新的内存会话；在 `All`、`Agents`、`Actions` 和 `Settings` 之间切换会保留当前查询，而关闭会丢弃会话。文件搜索有意保持缺失。

当前本地提供程序通过从已安装的应用程序 `ActionRuntime` 请求子树发现来投影操作。它保持每个运行时拥有的已准备或未绑定调用完整，按本地化的 Action 类别对结果进行分组，解释禁用能力，显示组合应用当前提供的 Workbench 与后代终端 Action 快捷键标签，并通过 `executePrepared(...)` 调用选定的就绪结果。Tab 条方向 Action 演示了嵌套输入：选择命令会打开表面内水平/垂直选取器，通过 `prepareInvocation(...)` 绑定所选值，然后执行相同的 scope 绑定令牌。设置提供程序通过 `workbench.settings.open` 投影稳定的根设置目标。

`All` 在查询之前显示精选的推荐操作和设置，然后在键入后搜索完整的本地目录。匹配规范化 Unicode、大小写和重复空格，并对精确的本地化标题、标题前缀、标题标记前缀、本地化关键字、规范英语标题和确定性层中的模糊子序列进行排名。`Agents` 过滤器作为已接受的导航槽存在，但仍然为空，因为尚未组合 Agent-owned searchable read model；Workspace 概览的精确导航 Action 需要内部 Session 行输入，因此刻意不作为 searchable catalog item。Provider coordinator 接受有界同步或异步 provider，并拒绝过时 completion。File search、Agent fallback、持久 MRU 与用户可编辑 keybinding 仍不属于当前实现。

该对话框将 DOM 焦点保持在其组合框中。向上和向下移动 `aria-activedescendant` 通过可检查的结果，Enter 打开或运行突出显示的行，Escape 从嵌套选取器返回或关闭表面，指针移动更新相同的视觉突出显示。输入、过滤器和页脚保持固定，而分组结果滚动。关闭发生在选定的操作开始之前；它不会取消或承担该 Action 的异步生命周期的所有权。

### 搜索建模记录

- **问题和输入：** 一个人需要在不知道菜单位置或快捷方式的情况下发现和调用产品功能。可信输入是经过审查的开放过滤器、已安装的操作范围、本地化的操作描述符以及指针或键盘事件。查询文本是本地用户输入。缺少的操作输入仍然是未绑定的运行时令牌并且永远不会被猜测。
- **数据和所有权：** `WorkbenchSearchState` 是打开会话、筛选器、查询修订、查询、突出显示和嵌套父快照的唯一所有者。语义所有者对于操作的可用性和效果仍然具有权威性。投影结果（Projected results）携带稳定的提供者、组、结果、操作和范围标识以及运行时拥有的准备好的或未绑定的调用。
- **输出：** 打开操作会渲染并聚焦一个模态会话；筛选器与查询变化产生确定性的局部投影；选择已禁用项时只宣告原因，不执行调用；未绑定但受支持的结果会进入嵌套选择器；最终启用的选择至多触发一次 Action 执行。Escape 与外部 dismiss 均不构成任何 Action。
- **过渡：** 打开重置为请求的预设。查询更改会增加修订版本。过滤器更改保留查询文本。结果更改保留仍然可见的突出显示，否则选择第一个启用的结果，回退到第一个可检查行。 Nested Back 可恢复完整的父筛选器、查询、修订和突出显示快照。关闭仅保留下一个单调会话标识。
- **不变量：** 渲染的突出显示始终命名一个当前可见行；隐藏的动作永远不会渲染；禁用或不受支持的未绑定行永远不会执行；准备好的输入仅通过原始运行时令牌绑定；一个接受的行最多达到一个 `executePrepared(...)` 调用；并且关闭或嵌套取消不会执行域变更。
- **依赖项和排除：** React 拥有已安装的对话框生命周期和焦点适配器； `@orchbee/actions` 拥有注册、发现、准备、可用性、确认和执行； `@orchbee/i18n` 拥有产品副本；并设计代币自己的几何形状和外观。网络、存储、遥测、进程状态、代理数据和异步提供者协调都被排除在外，直到它们的语义所有者公开所需的合约。
- **验证：** 纯测试涵盖打开/关闭、查询和过滤器转换、突出显示移动、嵌套恢复、匹配层、确定性分组、设置重复数据删除和参数过滤。运行时投影测试涵盖准备好的和未绑定的结果。组件测试涵盖组合框焦点、操作过滤、嵌套方向输入、操作执行和关闭。 Storybook 提供开放的 All 和 Chinese Action 状态；浏览器验证使用快捷键、四个过滤器、键盘和指针选择、嵌套输入、取消以及两个选项卡条方向。

## 面板工作区

Panel 分隔符使用与轴向对应的局部 CSS 光标，同时关闭 resize primitive 的文档级全局光标注入。穿过分隔符只会在该分隔符是指针目标时改变光标，因此离开目标、取消指针或丢失捕获都不会让 resize 光标滞留在无关终端内容上。

产品终端右键菜单会在打开前捕获精确的聚焦元素。执行包括“清除”在内的非结构命令并关闭菜单后，焦点会返回该元素；成功拆分则抑制恢复，让新 Panel 的终端自动聚焦转移优先。

每个终端标签页都在标签栏下拥有一个内存中的面板工作区。新标签页从一个全尺寸面板和一个特殊的主终端开始。只有单个面板的主视图会省略面板边框。当终端应用程序不占有鼠标输入时，右键单击终端视口会先显示经过产品取舍的终端操作：适用时打开链接，以及复制、粘贴、全选、查找、显示上一条命令输出、显示回滚历史和清除；随后显示新建 Tab、在当前目录新建 Tab、新建子终端、面板最大化、包含四个方向的拆分子菜单和关闭终端。“查找下一个”“查找上一个”“滚动到底部”和“重置”仍可在 Action 搜索中发现，不再拉长高频指针菜单。只有当界面在菜单打开时捕获到精确链接目标，链接 Action 才会出现。只有组合应用提供了当前实际生效的绑定，命令才会渲染尾部快捷键。菜单关闭和当前目录创建会把被点击面板作为显式 Action 目标；键盘变体则使用活动 Tab 的聚焦面板，二者分别通过对应的上下文 Action 重新校验。子终端与其主终端占用同一个视口；子终端会显示在真实的 `40px` 底栏中，而主终端永远不会出现在该栏。激活子终端后会增加一个 `32px` 标题栏，其中包含返回操作、几何居中的终端标题和关闭操作。

组合应用可以注入由 `@orchbee/agent` 拥有的狭窄 `AgentSnapshotSource`。Workbench 会订阅但不复制或修改 canonical Agent state，解析每个精确 `TerminalPresentationId`，并在 panel header 的 leading region 放置 curated provider identity 与本地化 semantic status。它会对 attach 到多个 pane 的 Session 去重，并把一个 Tab 内的全部 presentation 聚合为稳定 identity icon 与一个仅用于非 ended 状态的 `TabStatusAdornment`。Project sidebar 会另外跨每个 Workspace terminal context 投影 live Session，同时不挂载非活动 surface。存在 live Session 的 Workspace 会把 layout mark 替换为一个 provider mark、按固定顺序重叠四分之一的两个 mark，或在三种及以上 provider 时使用 provider-neutral Agent mark；旁边数字始终统计去重后的 live Session，而不是 provider 种类。独立 disclosure 默认关闭，全局最多展开一个，详情只平铺 Agent 行，不形成第二套 Tab 或 terminal tree。每行显示 curated provider、状态文本以及所在 Tab、Panel 与 primary/child terminal；精确导航会先用最新 Session 重新校验，再选择并聚焦该目标。保留的 ended state 会在所有位置被排除，普通 Tab 或 Workspace identity 会立即恢复。通用 Tab Strip 只接收 host-owned React presentation，不感知 Agent provider。非 ended Agent 会让 panel header 即使在原本无 chrome 的 single-panel primary view 中也可见；empty source 或只有 ended 的 projection 会保留既有 panel layout。Freshness 与 semantic status 会通过本地化文本或 accessible label 加 glyph 表达，绝不只靠颜色。居中的 terminal title 保持独立。完整侧栏契约见 [`docs/project-workspace.md`](docs/project-workspace.md)。

可选的 `WorkbenchAgentNotificationObserver` 只会在 canonical snapshot 更新后接收精确 attention、successful outcome 与 failed outcome event。Workbench 会把所有 navigable attachment 对照当前 Tab/Panel tree 解析；只有 attached local Tab、split pane 或选中的 child presentation 及其 inner mux pane 都可见时，才把 event 标记为 visible。Hidden event 携带 curated、本地化、不含内容的 notification copy，以及一个只导航到精确 Tab 与 pane 的 activation callback。注入的 observer 拥有用户偏好、既有 platform permission、去重与最终 system effect；Workbench 绝不请求 permission，也不会把 notification click 变成 Agent response 或 approval。Activity、Search result、provider event、Agent action 与 transport 断开期间的 delivery 仍不属于这一切片。

一旦 Tab 拥有多个 Panel，每个 Panel 都会显示标题栏。其 Panel 选项菜单包含“最大化 Panel”或“最小化 Panel”，并投影与视口菜单及 Action 搜索相同的当前绑定。最大化会沿目标 Panel 的 split 祖先链投影 `100/0` 布局并隐藏同级分支，同时让完整 Panel React 树与所有终端表面保持挂载；最小化恢复规范权重，且不会替换终端 DOM 标识。在任一转换期间，每个受影响的 split 都会暂时撤回聚焦终端的 `autoFocus` 投影，直到其 resizable primitive 已同步投影布局；随后，同一个已挂载终端重新取得输入焦点，并在接受后续输入前重新测量其表面。拆分已最大化 Panel 时会先恢复整棵树，再执行拆分。始终恰有一个 Panel 处于聚焦状态；该 Panel 保留正常终端表面，其他 Panel 都覆盖非交互式 inactive mask。Panel 内的主指针或键盘焦点会把焦点与 mask 转移到该 Panel；仅有辅助指针上下文意图则不会。活动 Tab 会把其记忆的聚焦 Panel 中正在显示的主终端或子终端投影为唯一的 terminal `autoFocus` 目标。因此，Tab 激活、初始 Tab 创建、子终端激活、Panel 拆分、Panel 移动及聚焦 Panel 关闭，都会通过注入表面恢复或转移真实终端输入焦点，而无需让 Workbench 耦合到表面的 DOM。拖动主终端 Panel 标题栏会抬起从当前 Panel DOM 克隆的缩放视觉快照，从可逆布局投影中移除源，并立即重排其余网格。快照保留当前 Panel chrome 与终端 Presentation，且不会挂载第二个 React 终端实例。显示活动子终端的标题栏不可拖动。移到另一 Panel 的方向边缘时，会投影完整最终网格，并在目标处显示空框占位 Panel。放下后以完整源 Panel 替换占位符，并在目标上方、下方、左侧或右侧执行拆分，同时不替换任何终端标识或状态。目标中心刻意不激活，因此没有方向占位符时释放会取消并恢复规范网格。上下文关闭只解析一个目标：当前显示的子终端；否则在仍有其他 Panel 时解析为主 Panel；否则解析为活动 Tab。在 preview 组合中，关闭活动子终端只影响该子终端并返回主终端；关闭主视图会影响该 Panel 及全部子终端。在 managed 组合中，同一意图会先把每项受影响 Presentation 标记为 closing，并为每项 Presentation 发出一条精确 close 命令；所有关闭完成前拓扑保持存在，但已接受的活动 Tab 关闭会立即选择尚未等待关闭的最近后续 Tab，再回退到最近前序 Tab。因此，重复键盘关闭无需等待 Host 处置即可命中不同 Tab。preview、restoring 或 reconciling Presentation，以及瞬态 close activity 为 `active` 或 `unknown` 的 provisioning 或 `running` Presentation，都需要本地化确认；分类为 `idle` 的 provisioning 或 `running` Presentation 会直接关闭。Panel 关闭会统计受影响的主 Presentation 与子 Presentation。Tab 关闭会遍历每个请求 Tab 的每个 Panel，并在 Tab Strip Action 提交前统计全部主/子 Presentation。应用生命周期会跨每个 context 和 Tab 聚合同一谓词；它不会从更宽泛的未停止 session 数量推断保护。确认数量只包含需要保护的受影响 Presentation。部分失败会保留失败 Presentation 与已关闭同级，供显式重试且不回滚。Tab 中最后一个 Panel 不能通过 Panel 生命周期关闭，因为该情况由 Tab 生命周期负责。

活动 Tab 标识变化会在浏览器绘制前取消任何 presenter-owned Panel 拖动，并恢复源 Tab 的规范布局，因此源预览不会泄漏到新激活的 Tab。

当每个显示中的终端都处于运行且写入就绪状态时，活动标签页可以通过 Action 搜索或任一 Panel 菜单打开专用实时广播输入条。它把每个规范 Panel 显示中的 Presentation 捕获为一个不可变目标集合，让目标在指针 shield 后保持可见，拥有独占键盘焦点，并在该快照过时时于接受后续输入前停止。文本和物理按键敲击会通过每个注入终端表面按 FIFO 顺序投递；多 PTY fan-out 明确不是原子的，部分失败会停止且不重试或回滚。粘贴、文件拖放、隐藏 Presentation、最大化布局、其他标签页或窗口及远程控制均不在范围内。维护中的行为、所有权、转换、Kitty 参考与正确性契约位于 [Panel 广播输入记录](docs/panel-broadcast-input.zh-CN.md)。

managed 终端经过标识校验的自然退出是一项无需确认的终端级移除意图。退出的子终端只移除自身；若它正在显示，Panel 会返回主终端。带有子终端的主终端退出时只关闭已退出的 Presentation；Host 处置完成后，活动子终端会被提升为主终端，没有活动子项时则提升第一个子终端，且不会停止任何存活子终端。没有子终端的主终端退出时，若仍有其他 Panel 则关闭所属 Panel，否则关闭所属 Tab；关闭最后一个 Tab 不会关闭原生应用窗口。退出码和信号不改变该策略，而 `terminal-failed` 仍保留供检查。退出转换会原子进入现有 managed removal 协议，因此已在进行的显式 removal 优先，所有必需 close 完成前拓扑保持存在，处置失败会保留精确目标供重试。自然退出不会新增可配置的保留或重启模式。

终端进程渲染和保留输出搜索执行仍然是终端表面功能。 Find 通过终端 Action 范围及其可用快捷方式公开，而不占用面板标题镶边；仅当安装的查找控制器具有非空查询时，“查找下一个”和“查找上一个”才会同步启用。

已提交的投影终端元数据通过 `applyTerminalMetadataObservation(...)` 进入 Workbench。终端的自然 OSC 标题与持久启动后备标题及可选持久自定义 Tab 标题保持分离。显示优先级依次为自定义标题、聚焦终端的最新自然标题、其当前 CWD 或启动 CWD 的最后一个非空路径段，然后是后备标题。产品创建的后备值是本地化且不带编号的“终端”标签，因此新 Tab 会立即显示启动目录上下文，而不会把标题与身份序号耦合。后续自然标题变化仍记录在自定义标题下；清除自定义标题会立即显示最新自然标题。Host 报告的 OSC 7 CWD 只有在应用提供可信终端机器主机名，且共享策略校验并解码 URI 后才会接受；无效观察不能替换最近一次已接受的当前 CWD。同一观察还携带瞬态关闭活动 `idle`、`active` 或 `unknown`。产品创建的 managed Presentation 从 `idle` 开始，但这只表示 Orchbee 尚未为该新 shell 接受本地输入；水合及 preview Presentation 从 `unknown` 开始。已接受的本地非焦点活动会在评估关闭 Action 前立即把应用新建或 Host 观测到的 `idle` 失效为 `unknown`。失效时已经在途的 `idle` 帧会一直保持为 `unknown`，直至终端客户端完成其因果快照屏障。该屏障只证明替换帧发生在失效之后；既不证明 Host 已接受，也不证明 shell 已消费。后续因果上合格的当前状态投影可以报告 `active`、`idle` 或 `unknown`，包括在 Host 接受输入后恢复新鲜的 `idle`。观测到的 `active` 或 `unknown`（包括平台查询失败）始终恢复保守确认。该值既不持久化，也不作为终端生命周期权威。

`workbench.tab.title.set` Action 通过默认快捷键、Action 搜索和被调用 Tab 的上下文菜单公开该标题策略。打开编辑器时会捕获精确的终端上下文与 Tab 标识，并预填当前生效标题；提交前会重新校验同一标识对。去除首尾空白后的非空值会成为持久自定义标题，即使它与当前自然标题相同；空值或仅含空白的值会清除自定义标题，并立即显示最新的自然标题、CWD 或后备投影。取消、陈旧目标及缺失下层状态都不会修改模型，关闭对话框后焦点会恢复到此前活动的终端表面。这里采用 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 Apple `Shift+Cmd+I` 默认键、标准平台 `Ctrl+Shift+Alt+T` 默认键、预填编辑与空值重置行为；Kitty 的窗口标题命令、远程控制修改及原生 Cocoa 对话框不在范围内。

### Panel 可视选择建模记录

- **问题和输入：** 拥有多个 Panel 的用户需要精确聚焦或交换一个目标，而不必反复遍历空间或循环焦点。可信输入包括活动 Tab ID、其规范深度优先 Panel 叶节点顺序、精确的聚焦源 Panel ID、最大化状态、请求的 `focus` 或 `swap` 模式、规范化后的物理数字/字母键、Backspace 或 Escape 键，以及精确的主指针目标标识。DOM 事件是不可信的展示输入，在进入确定性模型前会转换为语义按键或 Panel ID。缺失活动 Tab、陈旧源、最大化布局、少于两个 Panel，或另一个所属 Panel 交互都会使启动不可用。
- **数据和输出：** `PanelVisualSelectionSession` 是 Workbench 展示器的瞬态状态，绝不属于共享文档或 Client View 状态。它捕获一个模式、Tab ID、源 Panel ID、完整有序 Panel-ID 快照、有序可选目标、等宽且无前缀冲突的标签，以及已接受的按键前缀。目标排除源，并按规范顺序使用 `1234567890ABCDEFGHIJKLMNOPQRSTUVWXYZ`；至多 36 个目标使用单字符标签，更大的受支持集合会整体增加等宽长度，而不会留下不可达 Panel。启动会返回不可用、双 Panel Tab 的精确直接目标，或一个会话。会话转换返回下一会话或关闭、一个可观察结果，以及至多一条精确聚焦或交换命令；它绝不写入 Panel 状态。

| 状态/事件           | 守卫与结果                                                                                                                                                                                     |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 启动                | 少于两个 Panel、陈旧源、最大化或另一个 Panel 交互时不可用。恰好两个 Panel 时直接返回唯一其他目标且不渲染标签。三个或更多 Panel 时以空前缀进入选择。                                            |
| 数字或字母          | 物理 `Digit0`-`Digit9` 与 `KeyA`-`KeyZ` 无视仍按住的修饰键，追加其规范字符。完整标签会关闭并产生一条精确命令；匹配前缀会收窄标签；无匹配、输入法组合或按键重复是归属本模式但不改变状态的事件。 |
| Backspace 或 Escape | Backspace 删除一个前缀字符并保持活动。Escape 关闭且不修改 Panel，并请求恢复源终端焦点。                                                                                                        |
| 主指针落在目标上    | 使用与其标签相同的精确命令关闭。工作区内但目标外的指针按下会取消并被消费；外部按下会取消但不抑制目标位置的交互。                                                                               |
| 工作区重新校验      | 只有活动 Tab、聚焦源、最大化状态和完整有序 Panel-ID 快照仍完全匹配时会话才继续。Tab、焦点、成员关系、顺序、最大化、所属模式、窗口失焦或卸载导致的失效会无命令关闭，且绝不抢夺替代焦点。        |

- **不变量和决策：** 候选 ID 与标签唯一，每个实时目标均可达，标签无前缀冲突，目标顺序是捕获规范顺序排除源后的投影。在精确的现有 Panel Action 提交前，不改变任何权威布局或焦点。活动期间，工作区根节点拥有 DOM 焦点和每个按键事件，包括仍按修饰键的标签键和无效键，因此 Workbench 快捷键与终端输入都观察不到选择按键。聚焦模式提交后恢复所选目标终端；交换模式在源叶节点移动后恢复源终端；显式 Escape 或工作区内源位置取消会恢复源；陈旧或外部取消不会覆盖新的焦点所有者。可视选择、调整大小、关闭确认与拖动是互斥的展示器交互。Web 把严格快捷键记录从 `v18` 扩展到 `v19`；有效且无冲突的 `v18` override 会迁移，而原记录保留为回滚快照；若与任一新默认值冲突，则迁移记录整体失效，不会静默遮蔽其中一条命令。
- **所有权和依赖关系：** `@orchbee/workbench` 拥有纯会话模型、瞬态展示器状态、工作区独占键盘作用域、标签、可访问性投影，以及对现有 `workbench.panel.focus` 或 `workbench.panel.swap` Action 的委托。`PanelWorkspaceState` 仍是唯一的权威 Panel 写入者。terminal-surface 适配器继续拥有真实终端焦点调用，且不会接收选择策略。组合应用拥有有效绑定，Web 拥有 Settings 投影和版本化 Browser 存储迁移。现有规范 `workbench.panel-workspace` 业务组件被扩展；不新增 package、依赖、Host、PTY、进程、网络或远程控制边界。
- **参考范围：** 这里采用 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `Ctrl+Shift+F7` 可视聚焦、`Ctrl+Shift+F8` 可视交换、排除源、双 Panel 立即选择、可见字母数字标签、允许修饰键的物理选择、Escape 取消和主指针选择。Orchbee 特意在超过 36 个目标时使用等宽多字符标签、取消精确陈旧会话，并拒绝最大化布局，而不采用 Kitty 不可达的溢出目标或隐藏窗口选择器。可配置字母表、直接编号聚焦 Action、铃声、子终端目标、跨 Tab/窗口目标、远程控制与广播输入均不在范围内。
- **正确性和验证：** 确定性示例覆盖不可用、直接、带标签、前缀、Backspace、Escape、指针、陈旧和重新进入路径。固定种子的生成式嵌套布局覆盖规范目标顺序、标签唯一与无前缀冲突、每个目标的可达性，以及跨单字符到多字符边界的源排除。React 集成检查独占焦点、仍按修饰键时的选择、无效键隔离、精确 Action 委托、指针提交/取消、陈旧取消、保留层排除、可访问性文案和终端焦点恢复。自动 Desktop Browser 轨迹覆盖真实快捷键路由以及指针/焦点检查点；Storybook 暴露两种模式。Action/目录、本地化、Settings、严格存储迁移、package 测试、构建及仓库合并门禁完成证据。由于会话没有异步完成或独立排序写入者，不选择 TLA+；由于不存在超出生成式确定性模型的关键全称变换声明，不选择 Lean。

### Panel 按位置聚焦建模记录

- **问题、输入与范围：** 已知 Panel 位置且主要使用键盘的用户，需要一条固定长度命令，而不是方向遍历、循环遍历或 overlay。公开 `workbench.panel.active.focus-by-position` Action 接受不可信 `WorkbenchPanelPositionInput`；其校验器只接纳并复制 `1` 到 `10` 中的一个精确位置。可信解析输入是当前活动终端上下文与 Tab 标识、一个精确的实时源 Panel ID、规范 `PanelLayoutNode` 树及可空最大化状态。十个持久快捷键标识在两个平台都把物理 `Ctrl+Shift+Digit1` 到 `Ctrl+Shift+Digit9` 及 `Ctrl+Shift+Digit0` 映射到语义位置 `1` 到 `10`。Panel 重排、钳制、十以上位置、overlay、子终端选择、跨 Tab 或跨窗口目标、远程控制及新的指针界面均不在范围内。
- **模型与输出：** `resolvePanelFocusByPosition` 是同步纯解析器。它校验实时源与唯一的规范深度优先叶节点顺序，拒绝最大化，并返回 `position - 1` 处的叶节点或一个具名不可用原因。活动 Action 遵循现有 active-terminal 策略：可用性解析当前上下文，执行时再次解析，然后用所选 Panel ID 准备精确的现有 `workbench.panel.focus` Action。只有下层 Action 提交焦点与终端 `autoFocus`；不同的实时目标会完成操作，选择已经聚焦的 Panel 会得到刻意的 `unchanged` 结果，任何不可用或陈旧结果都不改变 Panel 状态。

| 情况                       | 结果                                                                                                     |
| -------------------------- | -------------------------------------------------------------------------------------------------------- |
| 精确位置存在且目标不同     | 准备并执行一项精确下层聚焦 Action；消费快捷键。                                                          |
| 精确位置存在且为当前目标   | 执行同一下层 Action，保留全部状态，报告 `unchanged` 并消费快捷键，包括单例位置 `1`。                     |
| 位置不存在                 | 报告 `panel-position-unavailable`；不准备下层 Action，快捷键路由继续传递且不受抑制。                     |
| 已最大化、源陈旧或顺序无效 | 报告匹配的类型化不可用结果，不执行修改，快捷键路由继续传递。                                             |
| 执行前活动上下文发生变化   | 只解析执行时的当前活动上下文与精确目标；此前观察到的 Tab 或 Panel 无法通过这条同步路径收到迟到聚焦命令。 |

- **不变量与所有权：** 解析绝不改变布局拓扑、顺序、split/slot/Panel/终端标识、权重、成员关系、计数器、最大化、PTY 状态或共享文档数据。ready 结果只命名请求规范位置上的一个成员，绝不把不存在的位置别名到最后一个 Panel。`PanelWorkspaceState` 继续是焦点与 MRU 历史的唯一所有者；Workbench 快捷键分发、Web Settings、本地化及存储绝不写入 Panel 状态。现有指针聚焦仍是非键盘路径，而需要输入的按位置 Action 不会被合成为十个零输入 Action 搜索结果。
- **决策、兼容性、交付与恢复：** Orchbee 采用 Kitty 的物理 `kitty_mod+1` 到 `kitty_mod+0` 默认键，但刻意省略其 macOS `Command+1` 到 `Command+9` 别名，因为这些组合键继续作为规范 Tab 选择绑定。不同于 Kitty 会钳制的 `nth_window`，缺少精确位置时继续传递，因此快捷键不能静默选择最后一个 Panel。最大化布局遵循现有方向、循环和可视聚焦门控。Web 把严格快捷键记录从 `v19` 推进到 `v20`；有效且无冲突的 `v19` override 会迁移，原记录保持不变作为回滚快照，与任一新默认键冲突时则按照现有警告策略使完整迁移失效。回退会恢复旧目录并让保留的 `v19` 记录重新成为权威；不存在布局、共享文档、Host、进程或功能开关迁移。
- **参考与证据：** 参考为 Kitty revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`：`Tab.first_window` 到 `Tab.tenth_window` 把零基位置 `0` 到 `9` 委托给 `nth_window`，默认映射为 `kitty_mod+1` 到 `kitty_mod+0`。Orchbee 把 Kitty window-group 顺序映射为规范 Panel 叶节点顺序，并明确声明上述别名、钳制、布局 group、previous-window、远程控制及跨 Tab 差异。确定性示例覆盖校验、首个/第十个/当前/缺失目标、单例、最大化、陈旧源及重复顺序；固定 seed 的生成式嵌套布局验证每个接纳位置的精确成员关系与顺序。Action 测试覆盖精确下层委托、unchanged 传播、执行时重新校验、注册丢失及快捷键继续传递。快捷键、Settings、本地化、v19 迁移、冲突、回滚及 Desktop `terminal-parity/FS-17-panel-position-focus` 证据覆盖各项投影，随后运行根合并门禁。该变更不引入长期状态、独立排序完成、关键全称变换、几何、动画或新手势，因此不选择 TLA+ 与 Lean，也不需要新的视觉审查工件。

### 建模记录

- **问题和输入：** 用户或未来命令适配器需要确定性的本地 Panel 组合、方向键盘移动、终端输入连续性及自然退出清理，同时不能把几何或 DOM 输入内部实现耦合到终端进程所有权。可信输入是不可变 `TabModel` 快照、包括一个精确活动 Panel 方向在内的语义 UI 事件、经过标识校验的终端生命周期结果、生成的 managed close 标识，以及注入表面的 renderer-readiness 生命周期。没有活动 Tab 时不渲染 Panel workspace，也没有终端焦点目标。共享文档投影会持久化层级与终端生命周期意图，而焦点、最大化、活动子项及权重投影到 Client View；格式错误的存储处理仍属于应用边界。
- **数据：** `PanelWorkspaceState` 为每个 Tab 保存一个有序 Workspace ID。每个 Workspace 都有一棵规范 `PanelLayoutNode` 树、一个聚焦 Panel ID、一份从最久到最近排列且末项为该聚焦 ID 的唯一 Panel 焦点历史，以及至多一个最大化 Panel ID。每个叶节点恰好拥有一个主 `TerminalPresentation`、一个有序的一级子项列表，以及零个或一个活动子 Presentation ID。每个 split 恰好拥有两个有序子节点；每个子节点带有稳定、全局唯一且独立于 Panel/Split 标识的布局槽 ID，以及非负百分比权重。每个终端 Presentation 记录 Workbench 拥有的不透明 ID、持久后备标题、瞬态可空自然标题、可空启动/当前 CWD、瞬态 `idle`/`active`/`unknown` 关闭活动，以及可辨识联合形式的活动生命周期。每项 Tab 标题策略记录持久后备及可选自定义标题。异步关闭结果未完成期间，`pendingRemovals` 记录精确的子项、主终端提升、Panel 或 Tab 移除集合；主终端提升 removal 只会在原子提交时选择符合条件的存活子终端。
- **输出：** `transitionPanelWorkspace` 返回完整下一状态、`applied`、`unchanged` 或具名拒绝，以及零条或多条类型化 provision、close 或 Tab-ready 命令。`resolveDirectionalPanelFocus` 根据规范布局、源 Panel 和方向推导一个相邻 Panel ID 或 `null`；它本身绝不修改焦点。`resolvePanelSwap` 推导一份完整且结构共享的布局，交换两个精确 Panel 叶节点，同时保留每个 split 与 slot；它返回已应用布局、同一 Panel 的刻意空操作或陈旧目标拒绝。`resolvePanelResize` 根据一个精确 Panel、尺寸调整意图及普通或双倍步进，推导完整且结构共享的布局；它无需测量 DOM 即可返回已应用投影、具名的刻意空操作或过时/无效拒绝。presenter 只为活动 Tab 的聚焦 Panel 及其中正在显示的主终端或子终端投影 `autoFocus`；注入表面在挂载或 renderer ready 后把该投影落实为真实输入焦点效果。最大化或最小化投影会在任一受影响 split 尚未同步其精确 `100/0` 或规范布局时暂时移除该效果，再在最后一次同步布局提交后重新断言该效果，让表面按已提交的 Panel 尺寸重新测量。自然退出要么原子进入一项 managed removal，要么在同一退出已经进入 removal 时成为等价空操作，或者拒绝过时/冲突结果且不分配第二项 removal。纯转换与解析器均不执行进程、DOM、存储、时间、随机或网络效果。provision 和 runtime 失败会保留 Presentation 及所属拓扑，供检查或重试。
- **转换：** Tab 协调按 Tab ID 保留 workspace，并只创建或删除发生变化的 Tab 投影。四向拆分、相邻分隔符调整、活动 Panel 键盘尺寸调整/重置与方向交换、焦点、主/子选择、子项创建/关闭、Panel 移动/关闭、最大化/最小化、自定义标题设置/清除，以及完整终端元数据观察，都是受维护的语义事件。managed provision 完成/失败/重试、带有 Workbench 生成 close request 的自然进程退出、关闭完成/失败/重试及 Tab 关闭请求，都是显式关联标识的事件。子终端自然退出会开始 child removal。主终端自然退出时，若仍有子终端则开始 primary-promotion removal；若没有子终端但仍有其他 Panel，则开始 Panel removal；否则开始 Tab removal。提升只在已退出主终端的处置到达 `closed` 后提交；它会选择符合条件的活动子终端，回退到第一个符合条件的子终端，并在所有子终端都被其他 pending removal 作为目标时等待。每次 split 都用新的二叉 `50/50` split 替换目标 Panel 叶节点，即使新 split 与父节点方向相同也保持嵌套；父 slot 与父权重不变。关闭或抬起一个叶节点时，只用存活兄弟替换它的直接 split 父节点，因此祖先 split 的标识、slot 与权重保持不变。指针 Panel 移动先应用该局部移除，再把同一叶节点插入目标周围，且绝不分配新的 Panel 或终端标识。拖动投影应用同一纯树变换但不提交，因此取消可恢复未触及的规范树。把已经相邻的 Panel 移回当前一侧是刻意的空操作，并保留 split 标识、slot 标识和已调整比例。方向键盘移动则在一次转换中交换完整的源叶节点与最近邻叶节点，保留源 Panel 的焦点，并且绝不重新挂接任一叶节点。每次语义焦点转移都会把目标 Panel 移到焦点历史末尾。关闭非聚焦 Panel 只会删除该历史成员并保留焦点；关闭聚焦 Panel 时选择历史中最近的存活成员，只有历史为空时才按“前一个优先、后一个兜底”的视觉顺序回退。Tab 激活会复用该 Tab 的聚焦 Panel；新协调、水合或复制出的 Tab 只以其聚焦 Panel 初始化历史；split、指针 move 与键盘 swap 转换会聚焦并记录插入或移动的 Panel；显示子终端的选择与主终端提升都会保留所属 Panel 作为焦点所有者。接受活动 managed Tab 关闭时，会先标记 removal，再把 Client View 选择切到尚未等待关闭的最近后续 Tab，或回退到最近前序 Tab；若所有 Tab 都在等待关闭，则选择保留到完成结果将其移除。managed 移除仅在每个目标都到达 `closed` 后改变成员关系，并在该提交点求值焦点历史；Tab 最后发出 `tab-close-ready`，并一直保留到 `TabModel` 协调确认删除。新的主终端与子终端继承当前显示源终端的可空 CWD。
- **不变量：** 每个 Tab 至少拥有一个 Panel；Panel、split 和 Presentation ID 都唯一；focused 与 maximized ID 在所属 Tab 内可解析；焦点历史非空、只包含唯一且存活的 Panel ID，并以 focused ID 结尾；active child ID 在所属 Panel 内可解析且绝不等于主终端 ID；有活动 Tab 时恰有一个显示中终端成为 `autoFocus` 目标，没有活动 Tab 时则没有目标，唯一例外是最大化或最小化布局提交期间逻辑目标仍然唯一但焦点效果会被暂时撤回；每个 split 恰好有两个子项，布局 slot ID 保持全局唯一，子项权重在数值容差内合计恰为 `100`，且 resize 只改变该 split 的两个子项。重复 split 方向保持嵌套，因为 split 祖先关系决定后续关闭与 resize 行为。键盘 swap 只改变两个具名叶节点的位置：每个 split 标识、方向、子 slot、权重、Panel 对象、终端 Presentation、计数器及成员关系均保持不变。最大化只改变渲染布局、隐藏状态和暂时焦点效果门控；它绝不改变拓扑、规范 split 权重、逻辑焦点、终端表面挂载状态或终端 DOM 标识。只有可见祖先链上的每个 split 都报告请求的 primitive 布局已同步后，焦点效果才重新具备资格。关闭或抬起 Panel 会移除其直接父 split，把存活兄弟提升到不变的祖先 slot，从历史中删除已关闭 ID，并保留确定性的“MRU 优先、视觉顺序兜底”焦点目标。pending removal 非空、互不重叠、可解析到实时拓扑，且只匹配带有该 removal ID 的 `closing`、`close-failed` 或 `closed` 生命周期状态。只要仍有非 pending Tab，接受关闭就绝不会让 pending Tab 保持活动。待处理的主终端提升 removal 会让已退出主终端保留在原位直至原子提交，而且提升绝不会选择被其他 pending removal 作为目标的子终端。
- **决策：** 分割 `[50, 50]` 的右半部分会保留外层 `[50, 50]`，并创建内层 `[50, 50]`；其渲染宽度为 `[50, 25, 25]`，关闭任一内层叶节点后恢复外层两个均等区域。即使生成的视口比 `30px` 窄，分割创建仍然有效；这使语义分割命令保持确定性，而不是让渲染约束拒绝模型。方向焦点与移动会把带权布局树投影为规范化矩形。候选 Panel 必须完全位于请求方向，并与源 Panel 在垂直于移动方向的轴上具有正长度的重叠区间。解析器依次选择方向边缘间距最小、垂直于移动方向的轴上中心距离最小、稳定视觉叶节点顺序最靠前的候选。移动会把源 Panel 与该精确候选交换，而不是重建树；被移动源 Panel 保持聚焦。源不存在、布局只有一个 Panel、没有空间候选，或 Panel 最大化期间的方向聚焦或移动均为不可用空操作；两种方向操作都不循环。循环聚焦则遵循规范深度优先叶节点顺序并在两个边界回绕；单 Panel、源陈旧或 Panel 最大化时不可用。解析器本身不修改历史，但其成功产生的语义焦点或 swap 事件会修改历史。焦点历史是限定在一个实时 Tab 内的唯一 MRU 状态：水合和复制只以当前聚焦 Panel 初始化，共享文档应用会为保留的运行时 Tab 保留仍然有效的历史项，而且共享文档和 Client View 都不序列化它。在调整分隔符大小期间，两个子项沿活动轴至少被限制为 `30px`；因此，嵌套 split 会在各自的局部边界保护 Panel 尺寸。每个接受的调整大小候选者都会立即更新规范布局；完成、指针取消和丢失捕获保留最后接受的布局而不是恢复起始布局。键盘调整模式会捕获一个精确的 Tab 与 Panel，并在每条命令前重新校验二者；目标或活动 Tab 丢失时关闭。`W`/`N` 改变宽度，`T`/`S` 改变高度，`R` 把该 Tab 中每个 split 重置为相等子权重，Control 会把普通的五个百分点步进加倍。方向调整选择匹配轴上最近的 split 祖先，并为每个分支至少保留百分之五；缺少对应轴或到达边界时是刻意空操作。单 Panel 或最大化 Panel 仍可发现并打开该模式，但在存在可调整的规范布局前，尺寸命令保持不可用。最大化与最小化收敛使用现有已挂载表面和 `autoFocus` 接缝；重新挂载、伪造全局 resize 事件及自动重放被 Host 拒绝的 resize 都不在范围内。分隔符通过交叉点保持单一不透明 one-CSS-pixel 颜色。面板标题和子条空间永远不会覆盖在终端视口上。无论退出码或信号为何，自然退出始终移除已退出 Presentation，但绝不会把 `terminal-failed` 重新分类为退出。子终端退出只移除该子终端。主终端退出会提升活动子终端或第一个子终端，而不会终止存活项；只有没有子终端的主终端才会升级为 Panel 或 Tab removal。最后一个 Tab 可以关闭而原生 Workbench 窗口仍保留。Kitty 的 forward/backward/to-top 移动变体、独立的全局方向调整大小绑定、手动关闭确认、开发期拓扑迁移和规划中的可配置保留/重启退出模式都不在范围内。
- **所有权和依赖关系：** Workbench 是 Panel 布局、Tab 标题投影、逻辑终端 Presentation 标识/生命周期状态、记忆的聚焦 Panel 及表面 `autoFocus` 投影的唯一写入者。`TabModel` 仍对 Tab 顺序与活动 Tab 具有权威性。应用注入标识分配、供 OSC 7 规范化使用的可信终端机器主机名、managed 命令回传调度及 terminal-surface 适配器；该适配器拥有 renderer readiness 与真实 input focus 调用，但不能选择不同的语义目标。应用不能通过该模型提供 executable、environment、Host 终端或 attachment 标识。resizable primitive 拥有原始指针输入、分隔符的原生键盘行为、分隔符焦点与可访问性，以及候选布局生成。Workbench 不重新解释这些分隔符绑定；它拥有拓扑校验、相邻对投影、指针/分隔符的 `30px` 策略和唯一语义布局写入。打开键盘调整模式以及每条精确尺寸命令都通过已注册的 Workbench 与 Panel Action；模态界面只拥有瞬态焦点、按键路由及可感知状态。每次方向键盘移动都通过一个活动目标 Workbench Action 与一个精确源/目标 Panel Action；快捷键分发及 Action 搜索绝不直接写入布局。Host 仍对 PTY/进程生命周期、真实 CWD 发现、终端渲染及最终 CLI/agent 命令传输具有权威性。Panel 状态、Node 版本、Git 分支及变更数量等未来标题栏元数据刻意排除，直至存在拥有方数据契约；当前三区标题栏为其保留组合点。
- **参考范围：** 焦点选择、二叉 split 插入与关闭时的兄弟提升、默认自然退出关闭及基于提示符的关闭重新分类遵循 Ghostty revision `91f66da24527fa02d92b5fd0b41cd020f553a64c`；方向焦点与聚焦 Panel 的关闭恢复还对照 Warp revision `086dade63ff7327426d6276d57f85b3f2f313af0`。采纳的 split 拓扑投影来自 Ghostty `macos/Sources/Features/Splits/SplitTree.swift` 中的 `SplitTree` 规则：在叶节点插入时，用比例为 `0.5` 的二叉 split 替换该叶节点；移除一个子项时，用存活兄弟替换直接父节点且不改变祖先比例。采纳的方向焦点投影是共同的 Apple `Command+Option+Arrow` 与标准平台 `Control+Alt+Arrow` 默认键位、外侧边缘不循环、根据带权 split 布局进行空间选择，以及通过现有 Panel Action 聚焦。Ghostty 提供规范化空间槽位与稳定最近候选顺序；Warp 提供受垂直于移动方向的矩形重叠约束的即时 split 树候选。关闭恢复采纳 Warp 每个 `PaneGroup` 独立且唯一的 MRU `pane_history`、聚焦时先保留其他项再追加目标、移除前选择恢复目标，以及反向扫描存活候选的做法。Orchbee 刻意在任意历史长度下都扫描存活历史，而不采用 Warp 在历史至多有两项时的位置顺序回退。方向键盘移动采用 Kitty revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `move_window left/right/top/bottom`、当前布局邻居查找、外侧边缘空操作及 Splits 布局叶节点交换。Orchbee 复用其确定性的带权矩形邻居投影，并在现有焦点键上增加 Shift，形成 Apple `Shift+Command+Option+Arrow` 与标准平台 `Ctrl+Alt+Shift+Arrow`。Kitty 示例中的无修饰 Shift 会占用 shell 选择键，而其 forward/backward 默认键保留给循环聚焦而不是移动；forward/backward/to-top 移动循环、Kitty 的最近焦点邻居决胜规则及任意数值移动均排除。键盘调整模式采用同一 Kitty 修订版的 Apple `Command+R` 与标准平台 `Ctrl+Shift+R` 入口、`W`/`N`/`T`/`S`/`R` 控制、Control 倍增、持续模式及 Escape 退出。由于 Browser 客户端没有权威的本地终端 cell 几何或 Kitty remote-control overlay，Orchbee 使用确定性的百分比步进与应用模态界面。Kitty 的逐 cell 步进、bell、overlay 终端及 remote-control 协议均排除。Ghostty 的对角非重叠回退、保留 zoom 的导航、原生 responder 机制、视觉焦点效果及 undo 都不在一致性范围内。其余采纳的 Ghostty 投影包括：每个 Tab 记忆最后聚焦 surface、找不到记忆时回退第一个 surface、新 split 无条件取得焦点、不等待的子进程退出后立即关闭 surface，以及重新评估当前 `Terminal.cursorIsAtPrompt()` 状态而不永久记忆输入；其实现见 `src/Surface.zig`、`src/terminal/Terminal.zig`、`macos/Sources/Features/Terminal/BaseTerminalController.swift` 与 `TerminalController.swift`。Orchbee 的一级子终端在 Ghostty 中没有对应物。通过确定性提升保留它们、最后一个 Tab 关闭后保留原生 Workbench 窗口、平台进程采样、查询失败处理、Ghostty 的关闭确认配置与只读情况、异常运行时计时、`wait-after-command`、原生 Tab/window 表示及 timing 都明确不在一致性范围内。
- **正确性和验证：** 示例测试覆盖方向、二叉局部权重、同方向嵌套、保留祖先权重的直接兄弟提升、CWD 继承、Warp 范围标题/CWD 规范化、自定义标题优先级与恢复、子项选择、最大化、最后 Panel 拒绝、MRU 关闭焦点及视觉兜底、焦点历史不变量、四 Panel 与不均匀嵌套布局中的方向焦点、方向边缘与决胜规则、嵌套规范顺序与两个回绕边界中的循环聚焦、跨嵌套及兄弟位置且保留身份的叶节点交换、陈旧及相同 Panel swap 目标、idle/active/unknown 关闭确认，以及逐 Tab 隔离。Ghostty 差分场景从一个 Panel 开始，依次向下拆分、把上下两半都向右拆分、把右下 Panel 再次向右拆分、关闭该局部 split 的左叶节点，并比较最终四 Panel 层级及三个 `50/50` 比例。自然退出示例覆盖仅移除子项、活动子项或首个子项提升、聚焦与非聚焦 Panel 移除、最后 Panel 的 Tab 移除、退出码/信号等价、已有 removal 竞态、处置失败、重试及过时结果。固定 seed 的有状态属性测试生成混合层级/Action 序列（包括每项受维护的单 Tab Action），并在每次接受转换后断言全部不变量。有界探索覆盖每个受维护的四 Panel split 布局、指针 move、方向 swap、focus、maximize、terminal、close、分隔符 resize 与键盘 resize Action；精确数量由可执行探索维护。Tab 协调单独覆盖三个 Tab ID 的每个有序子集和更深的生成协调序列。move 安全性另有固定嵌套树回归与更深生成布局探索。方向 swap 证据检查双向对合、完整成员与对象保留、不变的 split/slot/weight 拓扑、活动目标重新校验、全部四个快捷键、外侧边缘透传及焦点连续性。resize 证据单独覆盖最近轴选择、两个子位置、普通/双倍步进、最小可见边界、整 Tab 重置、过时目标、拓扑校验、相邻对和最小尺寸属性、primitive 到 model 同步、最大化/最小化结算、模态键盘重复/退出、注册 Action 重新校验、原生分隔符布局变更、注册替换、取消与无关 primitive 失败。managed 生命周期示例覆盖 provision 失败、自然退出 removal、批量关闭、部分失败、重试、过时 provision/close 完成及 Tab-ready 顺序。`TerminalPresentationRemoval` TLA+ 模型检查两项 Presentation 上一次整拓扑显式 removal 或一次自然 singleton removal、原子拓扑或唯一存活项的主终端提升提交、部分失败保留、重复 removal 排除和过时结果不变性。其抽象 `running` phase 合并 provisioning、restoring、reconciling 与 running，因为它们具有相同的自然退出准入。确定性 lifecycle 测试拥有上下文 child-close 选择、互不相交的并发 removal，以及 active-child 与 first-eligible 提升优先级；精确状态数量属于共享 runner 的 digest-bound 结果。React 集成测试覆盖活动目标方向 Action 的可用性与执行、活动目标的 `autoFocus` 投影、最大化布局门控及结构上下文菜单交接。快捷键测试覆盖精确 Apple 与标准平台按键、Desktop 原生 accelerator 交接、不可用时继续传递和焦点连续性。精确 headed Desktop trace 覆盖 FS-12 键盘调整大小及 FS-13 方向 Panel 移动，包括几何、拓扑、陈旧边界、不变的终端标识及恢复的终端输入焦点。projected-terminal 适配器测试覆盖 ready 前请求、ready 与恢复 session、重复投影及处置。专用 Desktop 系统 trace 检查最大化/恢复后的 Panel、终端根、Canvas 与 PTY 几何，以及焦点、输入和滚动连续性。受控的完整 Project/Workspace/Tab/Panel 系统 trace 检查初始与新 Tab 焦点、返回 Tab 后的记忆焦点、新 split 焦点、方向四 Panel 遍历、外侧边缘空操作、嵌套 split 关闭后恢复均等四宫格、关闭后的 MRU 焦点恢复及自然退出 Tab 移除。managed 应用集成还覆盖恢复 Tab bootstrap、精确生命周期结果路由、委托 Tab 关闭、自然退出处置及可见 Tab 间 attachment demand 转移。Storybook 与浏览器审查会练习真实上下文菜单、resize handle、标题栏、Panel drop 与 dialog。

### Panel 循环聚焦建模记录

- **问题与范围：** 在多 Panel Tab 中工作的人需要一种不依赖几何方向的方式抵达每个 Panel，而不必在布局边缘反向操作。可信输入是活动 Tab ID、一个精确的实时源 Panel ID、规范 `PanelLayoutNode` 树、可空的最大化 Panel ID，以及向前或向后意图。本变更包括两个公开的活动 Panel Action、Action 搜索、Apple 与标准平台默认快捷键、快捷键设置、本地化，以及严格持久 override 迁移。跨 Tab、跨原生窗口、子终端、数字、overlay、MRU 顺序及 Panel 移动遍历均不在范围内。
- **模型与输出：** `resolveCyclicPanelFocus` 是纯解析器。它按照规范的深度优先子节点顺序收集 Panel 叶节点，定位精确源，并针对 `1` 或 `-1` 的 `delta` 返回 `(index + delta + count) % count` 处的相邻 ID。至少含两个 Panel 的有效非最大化布局会返回一个精确目标；缺失源、单例布局或存在最大化 Panel 时返回 `null`。活动 Workbench Action 会重新校验当前 Tab 与 Panel，然后把目标委托给现有 `workbench.panel.focus` Action。不可用的键盘调用会继续传递而不受抑制。
- **不变量与所有权：** 循环绝不改变布局拓扑、子节点顺序、split/slot/Panel/终端标识、权重、成员关系、计数器、最大化、PTY 状态或共享文档数据。成功结果保留在源 Tab 中，指向一个不同的实时 Panel，并且在布局未变化时先向前再向后会回到源。Workbench 仍是规范布局、聚焦 Panel、MRU 焦点历史及 `autoFocus` 的唯一所有者；快捷键分发、Action 搜索、设置及应用适配器绝不直接写入 Panel 状态。现有焦点转换会把选中的 Panel 追加到 Tab 本地 MRU 历史，并把其中显示的终端投影为唯一焦点目标。
- **决策、兼容性与恢复：** 两个平台都默认使用物理 `Ctrl+Shift+]` 切换到下一个、`Ctrl+Shift+[` 切换到上一个，与 Kitty 一致，同时避开 Apple 的 `Shift+Command+[` 和 `Shift+Command+]` Tab 选择绑定。命令按照规范叶节点顺序回绕，但在最大化期间保持不可用，以遵循 Orchbee 现有方向焦点门控，而不是改变隐藏 Panel。Web 把严格快捷键目录从 `v17` 扩展为 `v18`；有效且无冲突的 `v17` override 会迁移并保留为回滚快照，与任一新默认值冲突时沿用现有整条记录失效及警告策略。设置支持重新绑定和恢复默认，不支持未绑定状态。不需要布局或共享文档迁移、功能开关或进程恢复；回退集成变更会恢复旧目录，并让保留的 `v17` 记录重新成为权威。
- **参考与验证：** 采用的参考是 Kitty revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`：`Tab._next_window(...)` 把 `1` 和 `-1` 的 delta 委托给 `Layout.next_window(...)`，后者激活下一个 window group；默认映射为 `Ctrl+Shift+]` 与 `Ctrl+Shift+[`。Orchbee 采用相邻 group 循环语义与物理默认键，把 group 顺序映射为规范 Panel 叶节点顺序，并排除 Kitty overlay、布局特有 window group、远程控制、数字聚焦及跨 Tab 行为。确定性解析器测试覆盖嵌套顺序、两个回绕边界、反向互逆、单例及陈旧源；生成的有效树检查实时成员关系与互逆行为。Action 测试覆盖可用性、精确下层委托、执行时重新校验、最大化及继续传递。快捷键、目录、设置、本地化及 `v17` 迁移测试覆盖两个平台、精确修饰键、自定义绑定、冲突恢复和回滚保留。现有 Panel 焦点集成验证 MRU 与终端输入连续性；完整仓库验证作为集成门。解析器是同步的，且不引入新状态或独立排序结果，因此不选择 TLA+。

### Panel 均分建模记录

- **问题与范围：** 在反复进行同方向拆分或手动调整大小后，用户需要一条可发现的命令，把活动 Tab 的 Panel 沿每个 split 轴重新分配为相等份额。可信输入是活动 Tab 标识、一个精确的实时 Panel 目标、规范 `PanelLayoutNode` 树和 Action 调用来源。本变更包括一项整 Tab Action、Panel 菜单、Action 搜索、Apple 与标准平台快捷键、快捷键设置和持久 override 迁移。关闭后自动均分、双击分隔符、改变键盘调整模式的 `R` 重置、跨 Tab 均分以及进程或终端内容变更均不在范围内。
- **模型与转换：** `resolvePanelEqualization` 递归计算每个子项的轴向单元数：一个 Panel 是一个单元，同方向 split 贡献其后代单元之和，垂直方向 split 在当前轴贡献一个单元。每个 split 按这些单元的比例获得子权重，并递归均分自己的子项。`panels-equalized` 会重新校验精确 Tab 和 Panel，拒绝最大化、单 Panel 或陈旧目标；权重不同时一次性提交完整 root，否则返回刻意的 unchanged 结果。该操作是同步的，没有取消、回滚、命令、进程、DOM 测量、存储、时间、随机或网络阶段。
- **不变量：** 均分会保留每个 Panel、终端 Presentation、split 和 slot 标识；子项顺序、split 方向、拓扑、成员关系、计数器、焦点历史、聚焦 Panel 及最大化状态均不改变。每个子权重保持有限且非负，每个 split 在现有容差内合计为 `100`，每个同轴叶单元得到相同的规范化范围；第二次应用均分是幂等的，并保留第二次应用时的结构标识。垂直方向子树在父轴占一个单元，并在自己的轴上独立均分。
- **决策与所有权：** Workbench 仍是规范布局的唯一写入者。`workbench.panel.active.equalize` 解析活动目标，并下沉到菜单与 Action 搜索共同使用的精确 `workbench.panel.equalize` Action；快捷键与菜单适配器绝不直接写入权重。该 Action 仅对未最大化的实时多 Panel Tab 可用。已经均分的布局会消费可用调用并报告 unchanged；单 Panel、最大化及陈旧调用保持不可用，键盘路由继续向下传递。Apple 默认使用 `Control+Command+=`，标准平台默认使用 `Ctrl+Alt+=`；Kitty 文档中的 `Ctrl+Shift+E` 示例与 Orchbee 的标准平台向下拆分绑定冲突。Browser 快捷键 override 从 `v15` 迁移到 `v16`；有效的 `v15` 自定义会迁移，除非与新默认键冲突。键盘调整模式的 `R` 继续把每个二叉 split 重置为相等子权重，这刻意区别于按比例均分 Panel。
- **参考与证据：** 语义 oracle 是 Kitty revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `kitty/layout/splits.py::equalize_biases`；三个嵌套水平叶节点会产生 `1/3` 与 `1/2` 的 root 和内层比例，垂直方向子树只计一个轴向单元。Ghostty revision `9c3ec931d64561a8407dde7ac984ce156ae91539` 的 `src/datastruct/split_tree.zig::equalize` 独立印证该投影，并提供 Apple 键位。确定性示例镜像固定 Kitty 的同轴、混合轴、移除后、单 Panel 及幂等案例；固定 seed 的生成树会检查比例权重、标识、成员关系、拓扑及重复稳定性；有状态和有界 reducer 探索包含该语义事件；Action 测试覆盖精确目标重新校验及 unchanged 结果；菜单、快捷键、本地化、`v15` 迁移及设置测试覆盖每种调用投影。headed Desktop trace `FS-14` 会检查三列几何、菜单与 Action 搜索发现、仅在可用时阻止快捷键、终端标识及输入焦点连续性。该操作是一次同步的单所有者转换，没有独立排序的完成结果，因此不选择 TLA+。

### 当前目录 Tab 创建建模记录

- **问题与范围：** 在 Workspace 根目录下层工作时，用户需要一条可发现的路径，在实际正在查看的终端所报告目录中新建 Tab。该功能包括 `workbench.terminal.create-current-directory`、Action 搜索、终端上下文菜单、一组可配置的 Apple 与标准平台快捷键、本地化设置投影及持久 override 迁移。普通新建 Tab、split 与子终端、环境复制、前台进程复制、SSH 状态及远程 shell 目录发现均保持不变。
- **目标与转换：** 可用性先解析活动终端上下文和活动 Tab，再选择显式调用的 Panel 或该 Tab 的聚焦 Panel。它捕获正在显示的主 Presentation 或子 Presentation，并携带 `terminalContextId`、`tabId`、`panelId` 和 `presentationId`；启动目录取最新非空可信 `currentCwd`，缺失时回退到该 Presentation 的 `launchCwd`。创建前会同步重新校验每个标识，并从同一 Presentation 解析最新 CWD，再准备新 Tab。缺少 Tab、Panel、CWD、Host capability 或标识变化时 Action 不可用且不创建任何内容；普通创建继续解析 Workspace 或独立终端默认目录。
- **不变量与所有权：** Workbench 仍是 Tab 与终端 Presentation 的唯一写入者，Host 仍对观测 CWD 和进程启动具有权威性。菜单和快捷键绝不复制 environment、executable、command、output、attachment 或生命周期状态。创建成功会分配不同的 Tab、Panel 与 Presentation，通过现有交接聚焦新终端，并保留源终端标识与 session。捕获的 CWD 只是瞬态 Action 输入，绝不进入共享文档或浏览器存储。
- **决策与兼容性：** Apple 默认使用 `Option+Command+Enter`，标准平台默认使用 `Ctrl+Alt+Enter`；现有普通新建 Tab 仍使用 `Command+T` 与 `Ctrl+Shift+T`。Browser 快捷键 override 从 `v16` 迁移到 `v17`；有效旧自定义会迁移，除非与新增组合键冲突。回滚时 `v16` 记录保持原样，旧版本有意无法读取只在 `v17` 中完成的编辑。
- **参考与证据：** 行为采用 Kitty revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `kitty/boss.py::new_tab_with_cwd`，包括选择被调用或活动 window 并从其工作目录启动。Orchbee 使用可信 OSC 7 元数据和启动 CWD 回退，而不是 Kitty 的进程 CWD 请求；session membership、命令参数、remote control 及 Kitty 示例中的 `kitty_mod+t` 映射不在范围内。纯解析器测试覆盖显示中子终端优先级、回退、缺失数据、精确重新校验及同一终端的新元数据；runtime、Action、菜单、快捷键、本地化、设置和 `v16` 迁移测试覆盖所有投影。headed Desktop trace `FS-15` 验证 OSC 7 投递、菜单与 Action 搜索发现、快捷键消费、真实 PTY `pwd`、源终端连续性、焦点及设置。捕获和创建是一次同步重新校验操作，没有独立排序的完成结果，因此不选择 TLA+。

### Tab 呈现（presentation）交接建模记录

- **问题与非目标：** 切换已有内容的 Tab 或 Workspace，或者通过 split、关闭、移动、交换、最大化、恢复或选择 displayed child 离散替换同一 Tab 的 Panel 布局时，新容器可能包裹仍表示旧几何的 canvas。交接会持续保留一份有意义的画面，直到目标几何真正完成绘制，同时不让非活动 Tab 的 React 树、attachment、canvas 或 renderer 常驻。连续窗口或分隔线缩放、动画或淡入淡出、操作系统 compositor 保证、持久输出存储、第二套 VT 权威、同步读取 WebGPU 像素，以及保证从未呈现过的目标预先拥有当前像素，均不在范围内。
- **输入与数据：** 可信输入是活动 presentation scope 与 `TabModel` 标识、确定性的布局 revision、参与视觉呈现的 displayed-terminal 标识、单调递增的交接 epoch，以及携带精确 epoch、目标 Tab 标识和 presentation 标识的 display-ready 或 activation-ready 报告。布局 revision 包含 presentation scope、Tab、Panel 与 split 标识、拓扑与子项顺序、split 方向、最大化 Panel、child-terminal 成员关系和 displayed-terminal 选择；它排除焦点、终端 metadata 与 lifecycle 字段、普通正权重及拖动预览。最大化 workspace 只要求该精确 Panel 的 displayed terminal；正常 workspace 会递归排除零权重分支。`TerminalTabPresentationHandoffState` 拥有一个已稳定显示的 revision、一个带退出 revision 的 preparation，或一个仅含目标的 activation barrier。React 集成按布局 revision 至多保留一个只读退出 workspace 快照，包括规范 Tab 已移除或同 Tab 被替换之后。旧 epoch、错误目标 Tab 或重复标识报告都是空操作。
- **输出与提交：** settled 状态渲染一个可交互 revision。preparation 在一个 inert 的目标 revision 上方渲染冻结的退出 revision，并且不投影任何终端 `autoFocus`。只有所有祖先 Panel group 都报告目标最大化布局已稳定后，incoming terminal 才会挂载。Display readiness 表示有意义的缓存或实时像素、stopped 内容或真实失败内容已填充目标；对于 canvas presentation，还要求已绘制 Host 帧的列、行、物理单元尺寸与当前 surface 测量一致。每个必需目标通过 display readiness 后，目标成为唯一可见但仍 inert 的层。只有几何匹配的实时帧、stopped presentation 或终端失败会贡献 activation readiness；所有必需目标通过该屏障后，Workbench 才让目标可交互并恢复唯一 `autoFocus` 投影。没有必需注入 terminal surface 的目标会立即提交。
- **转换与取消：** 不同 Tab、Workspace scope 或布局 revision 都会开启新 epoch、冻结当前呈现的 terminal surface，并准备最新目标，即使 Tab 标识没有变化。再次变化会取代先前目标、保留最后提交的退出 revision，并拒绝旧 epoch 或旧目标的所有迟到报告。返回该退出 revision 时，会通过相同屏障复用并恢复它的冻结层，不会复制该层或提前授予输入。未改变布局 revision 的变化只更新 settled 快照，不发起交接。托管关闭会先冻结已绘制的活动 surface 并退役交互权，再改变规范拓扑。把退出 Tab 从规范拓扑移除只会分离而不会清除其瞬态呈现层；关闭最后一个 Tab 时没有存活目标，会正常清除呈现。
- **不变量：** 只要退出层仍存在，透明、空白或几何过时的 incoming canvas 就绝不会成为顶层已提交内容；preparation 与 activation barrier 都是 inert；activation readiness 必然蕴含几何安全的 display readiness；只有精确当前 epoch 与当前目标的报告会贡献 readiness；必需标识唯一；被隐藏的最大化兄弟和零权重分支不能阻塞提交；最多挂载退出与最新目标两个 revision；返回同 revision 只使用一层；分离的退出投影只读且不能发布 Action 意图；退出 attachment 与输入权必须先退役，其冻结像素才能用于呈现；显示快照绝不提供 attachment 标识、输入权、resize ownership、transport 命令、frame acknowledgement、metadata 发布或持久 Workbench 状态。
- **终端与应用依赖：** Workbench 拥有布局 revision 投影、交接状态、层级堆叠和交互准入。`@orchbee/terminal` 拥有不可变、仅显示的 packed 快照、最近一次已绘制 metadata 与当前 surface 测量几何之间的精确比较，以及一个引用计数、文档范围的 WebGPU device/pipeline runtime。Web adapter 只根据几何当前的 canvas 帧或非 canvas 终端结果解析 display 与 activation readiness，拥有一个按字节计量、上限为 `32 MiB` 的 least-recently-used 快照缓存，并且绝不虚构 PTY 几何。Terminal Host 仍对 VT 状态、PTY 尺寸、帧几何及内容具有权威性。
- **内存、失败与恢复：** 非活动 Tab 不保留 React terminal DOM 或 GPU surface。一次交接期间，一个冻结的退出 renderer 与最新目标可以共存；取代会 dispose 被放弃的目标。受限缓存只保留 packed presentation 数据。缓存拒绝、淘汰、WebGPU 不可用、attachment 失败、缺少快照或帧几何过时，都会保留退出屏障，直到出现几何当前的帧或真实失败。共享 device 丢失遵循现有类型化恢复路径；Canvas 2D 仍是 fallback。
- **决策与替代项：** 保持全部 Tab surface 挂载会让每个非活动终端继续占用 attachment、DOM、atlas 和 GPU，因此被否决。预测性调整或缩放旧 canvas 会制造第二套布局权威，或者在 Host reflow 前明显扭曲文字，因此被否决。在激活时捕获 raster 会让同步 `drawImage` 或 readback 阻塞交互，并随 viewport 面积增长，因此被否决。只绘制背景仍会呈现内容丢失。选定的布局 revision 事务复用现有语义快照和 freeze/resume 边界，同时在原子换层前要求目标几何证明。
- **正确性与验证：** 纯 revision 示例覆盖同 Tab 拓扑、顺序、最大化及 displayed-child 变化；排除焦点、metadata、普通权重与拖动预览；并区分 Workspace scope。纯转换示例覆盖同 Tab revision 替换、跨 Tab 与跨 scope 切换、几何安全的 display 与 activation readiness、重复、过时和错误目标报告、从任一道屏障取代、返回冻结 revision、无 surface 目标及退出项移除。`TerminalTabPresentationHandoff` TLA+ 模型在非空后继目标仍存活的切片内，检查有界乱序 readiness 和同 Tab revision 变化下的持续有意义呈现、几何门控的视觉提交、过时报告拒绝、activation 晚于 display 及交互排他性。确定性 `projectTerminalTabPresentationLayers` 测试拥有零层清除、至多两个挂载层及同 revision keyed 复用声明。React 集成覆盖同 Tab 最大化/恢复、布局稳定门控、活动 Tab 关闭和 Workspace scope 替换；确定性 revision 案例覆盖 split、关闭、移动、交换及 displayed-child 替换。Terminal 与 Web 测试覆盖已绘制与已测量几何不匹配、缓存快照匹配、延迟实时几何、失败及 freeze/cache/resume 顺序。受控 Desktop trace 会在离散转换周围逐 animation frame 采样，并拒绝 canvas 网格与当前 viewport 不一致的已提交层；它排除手工 native-window focus 测试、连续缩放、其他浏览器引擎、操作系统 compositor 时序和无界 device-memory 断言。
- **参考与回滚：** Ghostty revision `91f66da24527fa02d92b5fd0b41cd020f553a64c` 与 Warp revision `086dade63ff7327426d6276d57f85b3f2f313af0` 只用于协调布局与绘制的比较；Orchbee 不声称源码或像素 parity。本变更没有 schema migration、storage rollout 或持久数据。回退布局 revision handoff 与 geometry-ready adapter 会恢复此前的最终 resize 收敛行为；Canvas fallback 与未缓存的实时首帧恢复在 rollout 全程可用。

## 使用

在应用程序组合边界的 Workbench 样式表之前导入一次 UI 兼容性样式表：

```ts
import '@orchbee/ui/styles.css';
import '@orchbee/tab-strip/styles.css';
import '@orchbee/workbench/styles.css';
```

在同一边界渲染壳：

```tsx
import {
	Workbench,
	type WorkbenchHostActionCallback,
	type WorkbenchLayoutStorage,
	type WorkbenchNavigationPort,
	WorkbenchProjectWorkspaceRuntime,
	type WorkbenchShortcutPlatform,
	type WorkbenchTerminalFontSizeChangeCallback,
	type WorkbenchTerminalSurface,
} from '@orchbee/workbench';
import { ActionRuntime } from '@orchbee/actions';

export function ProductInterface({
	actionRuntime,
	layoutStorage,
	navigation,
	onSettings,
	onTerminalFontSizeChange,
	projectWorkspaceRuntime,
	shortcutPlatform,
	terminalSurface,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly navigation?: WorkbenchNavigationPort;
	readonly onSettings: WorkbenchHostActionCallback;
	readonly onTerminalFontSizeChange?: WorkbenchTerminalFontSizeChangeCallback;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
	readonly terminalSurface?: WorkbenchTerminalSurface;
}) {
	return (
		<Workbench
			actionRuntime={actionRuntime}
			layoutStorage={layoutStorage}
			navigation={navigation}
			onSettings={onSettings}
			onTerminalFontSizeChange={onTerminalFontSizeChange}
			projectWorkspaceRuntime={projectWorkspaceRuntime}
			shortcutPlatform={shortcutPlatform}
			terminalSurface={terminalSurface}
		/>
	);
}
```

为每个已安装的应用程序组合创建一个 `ActionRuntime` 和一个 `WorkbenchProjectWorkspaceRuntime`，并使用该组合处置应用程序拥有的资源。使用系统 CWD 和本地化的第一个终端标题初始化项目/工作区运行时。其内存中终端创建操作保留所请求的选项卡组和可选的插入索引。可选的主机回调（例如设置和应用范围终端字号变更）仍然返回结构化的 `ActionResult`，可以异步返回它，在提交之前观察 `context.signal`，并在权威变更之前立即调用 `context.markCommitted()`。

提供存储端口后，Workbench 会恢复 `orchbee:workbench:left-panel-width:v1`、`orchbee:workbench:left-panel-visibility:v1` 与 `orchbee:workbench:tab-strip-orientation:v1`。它使用功能拥有的严格 Zod Schema 解析完整 JSON 记录；只在用户调整大小完成后保存宽度，只在对应语义切换 Action 后保存可见性或 Tab 条方向。存储缺失、畸形、不可用或超出范围时，会回退到可见的 `250px` 侧栏及水平 Tab 条，不阻塞渲染。仓库范围的所有权与键契约见 [`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md)。

Workbench 拥有内存中的 Tab 与 Panel 上下文投影；组合应用对 PTY 及终端进程生命周期效果保持权威。托管组合调度类型化 Presentation 命令，并通过 `applyTerminalPresentationEvent(...)` 回送其类型化结果。Tab 关闭采用委托方式：Workbench 标记其中每个 Presentation 正在关闭，在结果待定期间保留拓扑，并仅在每项精确关闭完成后从 `TabModel` 移除 Tab。Desktop Host 可通过 `createCrossWindowDragRuntime` 把 Tab 条指针生命周期连接到 Host 拥有的实时窗口发现、兼容性检查、原生窗口移动、捕获及 move-loop 行为。Workbench 与浏览器应用不具备这些 OS 能力。

Workbench 公开了跨 Workbench、Terminal Host 与投影客户端（projected client）的运行时主题修订协调契约。 Workbench 拥有取代（supersession）与发布（publication）资格；该契约将主机拥有的终端默认值/调色板/光标和客户端拥有的像素、覆盖层、光标和应用程序表面保留在一个匹配的完整版本上。其模型、重试规则、有界证据和主机/客户端细化均保存在 [`docs/terminal-theme-revision.md`](docs/terminal-theme-revision.md) 中。实现了主机比较和设置、完全恢复状态、投影终端帧暂存、渲染器预检和应用程序范围同步发布屏障。仅当每个安装的参与者都可以提交精确的主机标记帧和完整的客户端快照后，修订才变得可见。

### 共享文档与 Client View 边界

`@orchbee/workbench/document` 将带规范修订的 `WorkbenchSharedDocument` 与单个页面的瞬态 `WorkbenchClientView` 分离。Project、Workspace、Tab/Group 成员关系、持久后备及自定义 Tab 标题、Panel/Split 结构、终端 Presentation 放置与持久生命周期意图属于共享状态。活动与选中导航、展开状态、焦点、最大化、活动子 Presentation 和 Split 权重则为客户端本地状态。每个 Tab 的 Panel MRU 焦点历史只存在于实时 Workbench runtime，并从 Client View 的聚焦 Panel 初始化；它不属于这两项序列化契约。自然终端标题、当前 CWD、视口需求、滚动、搜索、选择、复制、GPU 状态及 `AttachmentId` 仍不属于这两项契约。

托管的 `WorkbenchProjectWorkspaceRuntime` 通过 `getSharedDocument(...)` 和 `getClientView()` 投影这些合约，通过 `initialDocument` 和 `initialClientView` 进行水合，并且可以通过 `applySharedDocument(...)` 应用广播文档，而无需调度过程影响。 `subscribeDocument(...)` 提供应用耐久性协调器；其共享指纹不会因仅 Client View 的转换而改变。应用程序建立其持久性屏障后，`reconcileDurableTerminalPresentations()` 重播持久性配置、关闭和停止操作，并将保留意图转换为仅恢复命令。

应用生命周期所有者可以在封闭终端创建前捕获 `getRestartableTerminalPresentationIds()`，持久停止这些精确会话，再调用 `stageStoppedTerminalSessionsForRestart(...)`。重启暂存只会用新的 PresentationId 与 provision 操作标识替换已捕获的停止 Presentation，同时保留位置、标题与启动 CWD。它更新共享文档投影，但不会向退役运行时派发 provision 命令；下一应用运行时会通过普通持久协调执行这些效果。已停止或正在终止的 Presentation 不会被捕获或重启。

有界 `TerminalPresentationStop` TLA+ 模型在两项 Presentation 与两次 attempt 内检查先提交后关闭、
一次公开重试重新启动每项当前可重试的失败 Presentation、已停止项没有 binding、精确 close/start 目标
关联，以及替换标识保持新鲜。位置、标题与启动 CWD 不属于形式状态，因此由确定性 document
transition 测试拥有其保留声明。

该包不执行存储或 IPC 效果。组合应用在其边界解析 contract，并拥有私有存储、共享 revision、写入顺序、恢复 diagnostic 与最终 Terminal lifecycle effect。规范 shared document 是 version 2，并要求显式 Directory placement。其导出的单向历史 version-1 migration 会为每个 Directory 增加 local placement，并把缺失的 `customTitle` 默认成 `null`；它不保留未交付 topology schema 的 reader 或 alias。维护模型、不变量与证据见 [`docs/shared-workbench-document.md`](docs/shared-workbench-document.md)。

Workbench 拥有五十三个稳定的快捷键声明：Action 搜索、下一个与上一个 Workbench 区域聚焦、普通新建 Tab、在当前目录新建 Tab、应用范围终端字号增大/减小/重置、九个按位置选择 Tab 的操作、上一个和下一个 Tab 选择、活动 Tab 向后与向前移动、设置 Tab 标题、上下文关闭终端、全部搜索、设置、切换边栏、面板最大化、按比例均分 Panel、键盘 Panel 调整大小、向右拆分、向下拆分、四向、循环、按位置与可视 Panel 聚焦、可视 Panel 交换及四向 Panel 移动。当前目录创建在 Apple 上默认为 `⌥⌘Enter`，在标准平台上默认为 `Ctrl+Alt+Enter`；它解析精确的显示中终端，并在缺少可信 CWD 时继续传递。字号在 Apple 上默认为 `⌘=`、`⌘-` 与 `⌘0`，在标准平台上默认为 `Ctrl+=`、`Ctrl+-` 与 `Ctrl+0`；组合应用通过一个可选主机回调拥有外观变更。按位置选择在 Apple 上默认为 `⌘1` 到 `⌘9`，在标准平台上默认为 `Ctrl+1` 到 `Ctrl+9`。每个绑定都会在活动上下文的规范 `TabModel.tabs` 顺序中解析一基位置，其中包含固定 Tab 前缀与折叠组成员，然后执行准备好的 `tabs.tab.select` Action；缺少对应位置时继续传递事件，位置九绝不是最后一个 Tab 的别名。上一个和下一个选择遵循同一规范顺序，仅在至少存在两个 Tab 时越过边界回绕，并在空、单例或缺失活动 Tab 的快照中不可用。Apple 相邻默认值为 `⇧⌘[` 与 `⇧⌘]`；标准平台相邻默认值为 `Ctrl+PageUp` 与 `Ctrl+PageDown`。活动 Tab 移动在两个平台都使用 Kitty 的 `Ctrl+Shift+,` 与 `Ctrl+Shift+.` 默认键，在活动 Tab 所属的固定或未固定分区内循环，并把权威修改委托给预备好的 `tabs.tab.move` Action。设置 Tab 标题在 Apple 上默认为 `⇧⌘I`，在标准平台上默认为 `Ctrl+Alt+Shift+T`；它打开精确的活动或被调用 Tab，并把持久标题策略委托给 Workbench。键盘 Panel 调整大小在 Apple 上默认为 `⌘R`，在标准平台上默认为 `Ctrl+Shift+R`，并打开与菜单及 Action 搜索相同的精确目标模态框。Panel 均分在 Apple 上默认为 `⌃⌘=`，在标准平台上默认为 `Ctrl+Alt+=`，它按同轴 Panel 单元递归重分配活动 Tab，且不改变拓扑或焦点。Apple 方向焦点默认值为 `⌥⌘←/→/↑/↓`；标准平台方向焦点默认值为 `Ctrl+Alt+←/→/↑/↓`。循环聚焦在两个平台都使用显式 `Ctrl+Shift+]` 与 `Ctrl+Shift+[`，遵循活动 Tab 内的规范 Panel 叶节点顺序，并且仅在多个 Panel 存在时回绕。按位置 Panel 聚焦在两个平台都使用 `Ctrl+Shift+1` 至 `Ctrl+Shift+9` 与 `Ctrl+Shift+0`，其中 `0` 映射为活动 Tab 规范深度优先 Panel 叶节点顺序中的语义位置十；有效的当前位置目标会以 unchanged 结果消费按键，缺失、最大化、陈旧或越界目标则不夹取并继续传递。可视聚焦与交换在两个平台都使用 `Ctrl+Shift+F7` 与 `Ctrl+Shift+F8`；双 Panel 布局会立即选择唯一的另一 Panel，更大的布局会进入独占标签选择器。方向移动在此基础上增加 Shift，Apple 为 `⌥⇧⌘←/→/↑/↓`，标准平台为 `Ctrl+Alt+Shift+←/→/↑/↓`。其他 Apple 默认值依次为 `⇧⌘P`、`⌘F6`、`⇧⌘F6`、`⌘T`、`⌘W`、`⇧⌘F`、`⌘,`、`⌘B`、`⇧⌘Enter`、`⌘R`、`⌘D` 与 `⇧⌘D`。其他标准默认值为 `Ctrl+Shift+P/T/W/K/B/Enter/R/D`、`Ctrl+F6`、`Ctrl+Shift+F6`、`Ctrl+,`，向下拆分则为 `Ctrl+Shift+E`；未加 Shift 的终端控制序列继续传给 PTY。组合应用注入一张以快捷键标识为键的完整当前 Workbench 绑定表、发布 Workbench 是否为活动快捷键作用域，并可为终端拥有的 Action 注入只读的实际生效标签映射。处于活动状态时，Workbench 通过所属 document 上的一个捕获监听器解析自身绑定，不依赖焦点位于哪个 Workbench 区域或 document body。非活动或已卸载的 Workbench 不参与；全屏设置表面打开时，Web 会让它变为非活动。同一组绑定值驱动匹配、菜单、Tooltip 与 Action 搜索标签，包括 `workbench.search.open` 的“全部”和“Action”两种不同输入。终端上下文菜单会投影当前的普通和当前目录新建 Tab 标签；Tab 菜单继续投影普通新建 Tab、设置 Tab 标题与上下文关闭终端。没有绑定的命令会让尾部快捷键槽位保持为空。只有对应 Action 当前启用时快捷键才会被消费；不匹配或不可用的绑定会继续传给聚焦表面而不受抑制。成功执行新建 Tab 后，其第一个 Panel 终端会成为唯一的 `autoFocus` 目标，包括从空 Tab 模型创建时。缺少相邻 Panel 或存在最大化 Panel 时，方向焦点与移动不可用；单例或最大化布局中的循环聚焦与可视选择不可用；单例或最大化布局中的均分也不可用，因此这些组合键都会继续传递而不受抑制。

## 开发

变更触发的统一搜索模型检查要求 Java 17 或更高版本。仓库运行器在工具缺失时下载固定版本的 TLA+ `1.7.4` JAR，并在执行前校验 SHA-256；可设置 `ORCHBEE_TLA2TOOLS_JAR` 以使用已配置且匹配的副本。

Workbench 的 Vitest 配置所调度的文件 worker 不会超过四个，也不会超过 Node.js 进程可用的并行度。加载测试配置时，它会输出一次检测到的并行度与配置的 worker 上限，使受限 CI 运行仍可诊断。此规则只影响执行调度；测试选择、断言、默认五秒超时及现有单项测试的超时覆盖保持不变。

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:tla
pnpm --filter @orchbee/workbench test:exports
```
