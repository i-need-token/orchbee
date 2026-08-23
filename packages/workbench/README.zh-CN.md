# @orchbee/workbench

Orchbee 应用程序的共享产品语义 React shell。

## 适用范围

Workbench 在应用程序拥有的 `ActionRuntime` 中注册其产品操作。侧边栏控件、快捷方式、选项卡条命令、面板按钮和菜单调用这些注册；注册的处理程序是其语义结果的唯一编写者。每个安装的终端视口在同一运行时中创建一个子范围，并拥有稳定的最小终端目录：复制、粘贴、全选、查找、查找下一个、查找上一个、滚动到底部、清除和重置。注入的表面将实现注册到该范围中，并仅为准备好的上下文菜单调用提供修订固定的语义上下文。连续分隔符调整大小会随着规范面板布局的变化而更新，并在交互结束或取消时保留最后的有效布局。面板拖动预览保持可逆的交互状态，而最终的拖动结果会调用相应的Action。

该包装拥有一个全高、水平可调整大小的两面板外壳。左侧边栏默认为`250px`，约束为`200px`–`400px`；右侧面板填充剩余宽度并通过 `@orchbee/tab-strip` 渲染活动的独立或工作区终端上下文。侧边栏控件和`Mod+B`快捷方式隐藏或恢复侧边栏。在两种状态下，控制器均保持安装在一个固定的外壳位置。当侧边栏隐藏时，选项卡条表面内的透明引导位置会保留其足迹，因此控件永远不会与第一个选项卡重叠或绘制单独的背景。本地化侧边栏提供独立的 `Terminal` 位置、`Search`、可扩展的项目和工作空间层次结构以及底部锚定的 `Settings` 操作。项目行控制公开，而不更改活动终端上下文。工作区行切换到其独立的选项卡和面板上下文；返回终端将恢复现有的独立上下文。

右键单击空白选项卡条空间会显示本地化的方向操作。工作台在顶部水平条和左侧垂直条之间切换条和内容布局，而选项卡、组和新选项卡控件保留其现有的上下文菜单。

shell 由 `@orchbee/ui` 组成原语，但不拥有 PTY 进程、文件系统或 Git 效果、设置目标、身份验证、路由或代理编排生命周期。 Workbench 拥有统一的搜索状态和表示，而语义所有者仍然负责可搜索的读取模型和操作注册。机器可读范围合约位于 [`project.json`](project.json) 中。

Workbench 组合显式区分为 `preview` 或 `managed`。两者都会接收应用拥有的终端 Presentation 标识分配器与稳定的初始 Presentation ID。托管组合还接收操作/移除 ID 分配器和窄化的命令调度器；Workbench 绝不分配 Host 终端 ID，也不以 Host 终端标识执行拓扑或生命周期变更。终端表面接缝只向下述响铃效果协调器公开不透明的 Host 终端标识。动态 Presentation ID 不透明，且删除 Presentation 后不得复用。

托管应用可以用恢复的 Presentation 初始化独立上下文。每个种子保留应用拥有的 PresentationId 与启动 CWD，并投影成一个 Tab；Workbench 仍不会推断或持久化 Project、Workspace、Group、Split、进程或 Host 标识。运行时按 PresentationId 在所有终端上下文中路由每个 provision、exit 与 close 结果；目标缺失、重复或过时时会拒绝，且不改变其他 Presentation。

应用程序可以注入 `terminalSurface` 组件。Workbench 仅为显示中 Tab 的可见 Panel 布局内的终端 Presentation 挂载它，并传入 Presentation 模型、视口的应用所有 Action runtime/scope、窄化的语义上下文注册接缝、响铃效果准入函数和 `autoFocus` 投影，而绝不传入 Host 能力。非活动 Tab、其他 Workspace、后台 Project 和非活动子终端不会挂载该表面。Workbench 只对活动 Tab 所记忆的聚焦 Panel 中正在显示的终端设置 `autoFocus`。当该投影变为 true、true 目标被挂载，或其恢复后的 renderer 进入 ready 时，注入表面会让真实终端输入取得焦点。因此，新 Tab 会聚焦第一个 Panel 的终端；返回某个 Tab 会聚焦该 Tab 记忆的 Panel 终端；拆分会聚焦新 Panel 的终端。关闭聚焦 Panel 时，按视觉叶节点顺序选择前一个终端；只有关闭第一个叶节点时才选择后一个；关闭非聚焦 Panel 会保留并恢复原聚焦终端。右键菜单会保留打开前的焦点元素，并在关闭时恢复，除非所选结构命令把焦点转移给另一终端。这让 Desktop 可把 attachment 与 renderer 生命周期绑定到真实视口需求，同时保留 Workbench 对逻辑拓扑和产品焦点的所有权。

一台安装式工作台面板-工作区终身拥有一台 `WorkbenchTerminalBellEffectCoordinator`。每个表面仅报告不透明的终端身份和正主机响铃序列。协调器保留每个终端的可合并序列间隔，因此重复的、过时的、重新连接的和重放的附件传送都会被拒绝，而不会丢失乱序到达的不同序列。确切地说，第一个被承认的交付可以创建视觉产品效果并调用应用程序功能。 Workbench 本身不执行音频或操作系统通知；这些仍然是两个独立的、默认不存在的应用程序功能。

稳定的 `terminal-parity/PR-11-bell` 见证把多个生产终端表面与一个 Workbench 协调器组合起来，并证明重复/重连抑制、交错序列准入，以及各可选效果具有独立失败边界。稳定的有界面 `terminal-parity/IN-09-action-scope`、`terminal-parity/IN-11-context` 与 `terminal-parity/IN-12-actions` 见证会在其包含的 Workbench 作用域中组合最小注册表，并证明：未认领或不可用的 Apple Command 组合键交给原生处理；语义上下文在执行时固定并重新校验；同步可用性；类型化结果；无需额外确认；以及本地化反馈。这些受控 Canvas 夹具捕获应用效果；它们不声称原生音频、OS 通知、剪贴板或 URL handler 会实际送达，也不会让 Workbench 成为终端状态权威。

地址为权威 Client View 输入的应用程序可能会注入 `WorkbenchNavigationPort`。然后，在工作台改变活动的 Client View 状态之前，工作区、独立上下文和选项卡选择操作会委派导航；应用程序更新浏览器历史记录并将生成的路由应用回运行时。如果没有此端口，Workbench 将保留其本地内存中导航行为。 Workbench 不解析路径或自己的路由器历史记录。

`@orchbee/workbench/terminal-presentation` 还提供运行时中立的终端表示身份、生命周期契约和边界模式。平台应用程序使用该狭窄的入口点，而不是将 React 工作台图导入到主运行时或预加载运行时中。

## 体验目录

Workbench 在 [`docs/experience`](docs/experience) 中维护已审核的交互与业务组件清单。这些记录说明预期用途与排除用途、不变式、组合依赖、实现根及 Storybook 检查状态，不会因此把私有组件变成公共导出。构建新的复杂 Workbench 界面前，请先从仓库根目录运行 `pnpm experience:find --owner @orchbee/workbench` 查找组件，再用 `pnpm experience:show <id>` 查看选中记录。

## 项目和工作空间

维护的内存模型、所有权、回退、CWD、重复和正确性合约位于 [`docs/project-workspace.md`](docs/project-workspace.md) 中。

`WorkbenchProjectWorkspaceRuntime` 拥有独立终端上下文，并为每个 Workspace 分别拥有一套 Tab 模型和 Panel 工作区。Project 与 Workspace 操作使用稳定的 Action ID。Project 创建控件会打开公共的 Folder 获取菜单。应用可以注入窄化的 `projectFolderAcquisition` 能力，以选择一个已有 Folder。Desktop 通过原生选择器提供该能力，并返回一个经 Host 规范化和验证的路径；Browser 与 Gateway 不提供该能力，继续使用手动路径输入适配，但不声称路径确实存在。选择路径后，会原子创建 Project、首个 Folder、Primary Workspace 和 Workspace 终端投影。Project 管理通过 `Add Folder` 复用同一能力；Workbench 在提交迟到结果前会重新检查最新 Project 标识和完全重复的路径。取消与有界失败不会改变 Project；一个已挂载 Workbench 内也至多只有一项选择处于活动状态。`New Folder…` 仍然可见但不可用，因为当前没有组合执行文件系统变更。创建 Workspace 会在其主要 Project Folder 中启动一个 Shell 投影。预览复制会把 Tab 组、Panel 布局、名称、启动配置选择和记录的启动 CWD 复制到全新身份，同时重置由进程派生的 Tab、Group 与 Presentation 状态。托管组合不开放 Project 或 Workspace 复制，因为它无法静默克隆真实进程。终端投影会把启动 CWD 与 Host 报告的当前 CWD 分开。目录刷新会保留稳定身份、顺序和主要引用。删除 Workspace 只处置其 Orchbee 终端上下文；移除 Project 只处置 Orchbee 元数据与上下文，绝不声称会删除文件夹或 Git 工作树。

该包接受主机规范化的目录描述符和浏览器输入的路径，并且不执行文件系统、Git、PTY 或持久性效果本身。 Desktop拥有当前本机existing-Folder选择和验证适配器；文件系统突变、正在进行的目录观察和 Git 效果仍然是应用程序拥有的未来工作。

## 统一搜索

维护的行为、所有权、转换和正确性模型位于 [`docs/unified-search.md`](docs/unified-search.md) 中。

侧栏搜索控件与平台的全部搜索绑定——Apple 上为 `Mod+Shift+F`，标准平台为 `Mod+Shift+K`——会打开一个上部视口模态界面，其中包含 `All` 过滤器和空查询。`Mod+Shift+P` 会以 `Actions` 过滤器打开同一个界面。每次全局打开都会启动新的内存会话；在 `All`、`Agents`、`Actions` 和 `Settings` 之间切换会保留当前查询，而关闭会丢弃会话。文件搜索有意保持缺失。

当前本地提供程序通过从已安装的应用程序 `ActionRuntime` 请求子树发现来投影操作。它保持每个运行时拥有的已准备或未绑定调用完整，按本地化的 Action 类别对结果进行分组，解释禁用能力，显示组合应用当前提供的 Workbench 与后代终端 Action 快捷键标签，并通过 `executePrepared(...)` 调用选定的就绪结果。Tab 条方向 Action 演示了嵌套输入：选择命令会打开表面内水平/垂直选取器，通过 `prepareInvocation(...)` 绑定所选值，然后执行相同的 scope 绑定令牌。设置提供程序通过 `workbench.settings.open` 投影稳定的根设置目标。

`All` 在查询之前显示精选的推荐操作和设置，然后在键入后搜索完整的本地目录。匹配规范化 Unicode、大小写和重复空格，并对精确的本地化标题、标题前缀、标题标记前缀、本地化关键字、规范英语标题和确定性层中的模糊子序列进行排名。 `Agents` 过滤器作为接受的导航槽出现，但仍然为空，因为当前没有组成代理拥有的可搜索读取模型或导航操作。提供者协调器接受有界同步或异步提供者并拒绝过时的完成。文件搜索、代理回退、持久 MRU 和用户可编辑的键绑定仍然在当前实现之外。

该对话框将 DOM 焦点保持在其组合框中。向上和向下移动 `aria-activedescendant` 通过可检查的结果，Enter 打开或运行突出显示的行，Escape 从嵌套选取器返回或关闭表面，指针移动更新相同的视觉突出显示。输入、过滤器和页脚保持固定，而分组结果滚动。关闭发生在选定的操作开始之前；它不会取消或承担该 Action 的异步生命周期的所有权。

### 搜索建模记录

- **问题和输入：** 一个人需要在不知道菜单位置或快捷方式的情况下发现和调用产品功能。可信输入是经过审查的开放过滤器、已安装的操作范围、本地化的操作描述符以及指针或键盘事件。查询文本是本地用户输入。缺少的操作输入仍然是未绑定的运行时令牌并且永远不会被猜测。
- **数据和所有权：** `WorkbenchSearchState` 是打开会话、筛选器、查询修订、查询、突出显示和嵌套父快照的唯一所有者。语义所有者对于操作的可用性和效果仍然具有权威性。预计结果携带稳定的提供者、组、结果、操作和范围标识以及运行时拥有的准备好的或未绑定的调用。
- **输出：** 打开操作会渲染并聚焦一个模态会话；筛选器与查询变化产生确定性的局部投影；选择已禁用项时只宣告原因，不执行调用；未绑定但受支持的结果会进入嵌套选择器；最终启用的选择至多触发一次 Action 执行。Escape 与外部 dismiss 均不构成任何 Action。
- **过渡：** 打开重置为请求的预设。查询更改会增加修订版本。过滤器更改保留查询文本。结果更改保留仍然可见的突出显示，否则选择第一个启用的结果，回退到第一个可检查行。 Nested Back 可恢复完整的父筛选器、查询、修订和突出显示快照。关闭仅保留下一个单调会话标识。
- **不变量：** 渲染的突出显示始终命名一个当前可见行；隐藏的动作永远不会渲染；禁用或不受支持的未绑定行永远不会执行；准备好的输入仅通过原始运行时令牌绑定；一个接受的行最多达到一个 `executePrepared(...)` 调用；并且关闭或嵌套取消不会执行域突变。
- **依赖项和排除：** React 拥有已安装的对话框生命周期和焦点适配器； `@orchbee/actions` 拥有注册、发现、准备、可用性、确认和执行； `@orchbee/i18n` 拥有产品副本；并设计代币自己的几何形状和外观。网络、存储、遥测、进程状态、代理数据和异步提供者协调都被排除在外，直到它们的语义所有者公开所需的合约。
- **验证：** 纯测试涵盖打开/关闭、查询和过滤器转换、突出显示移动、嵌套恢复、匹配层、确定性分组、设置重复数据删除和参数过滤。运行时投影测试涵盖准备好的和未绑定的结果。组件测试涵盖组合框焦点、操作过滤、嵌套方向输入、操作执行和关闭。 Storybook 提供开放的 All 和 Chinese Action 状态；浏览器验证使用快捷键、四个过滤器、键盘和指针选择、嵌套输入、取消以及两个选项卡条方向。

## 面板工作区

Panel 分隔符使用与轴向对应的局部 CSS 光标，同时关闭 resize primitive 的文档级全局光标注入。穿过分隔符只会在该分隔符是指针目标时改变光标，因此离开目标、取消指针或丢失捕获都不会让 resize 光标滞留在无关终端内容上。

产品终端右键菜单会在打开前捕获精确的聚焦元素。执行包括“清除”在内的非结构命令并关闭菜单后，焦点会返回该元素；成功拆分则抑制恢复，让新 Panel 的终端自动聚焦转移优先。

每个终端标签页都在标签栏下拥有一个内存中的面板工作区。新标签页从一个全尺寸面板和一个特殊的主终端开始。只有单个面板的主视图会省略面板边框。当终端应用程序不占有鼠标输入时，右键单击终端视口会先显示经过产品取舍的终端操作：适用时打开链接，以及复制、粘贴、全选、查找和清除；随后显示新建 Tab、新建子终端、面板最大化、包含四个方向的拆分子菜单和关闭终端。“查找下一个”“查找上一个”“滚动到底部”和“重置”仍可在 Action 搜索中发现，不再拉长高频指针菜单。只有当界面在菜单打开时捕获到精确链接目标，链接 Action 才会出现。只有组合应用提供了当前实际生效的绑定，命令才会渲染尾部快捷键。菜单关闭会把被点击面板作为显式 Action 目标；键盘关闭则使用活动 Tab 的聚焦面板，二者都通过同一个上下文关闭 Action 解析。子终端与其主终端占用同一个视口；子终端会显示在真实的 `40px` 底栏中，而主终端永远不会出现在该栏。激活子终端后会增加一个 `32px` 标题栏，其中包含返回操作、几何居中的终端标题和关闭操作。

一旦 Tab 拥有多个 Panel，每个 Panel 都会显示标题栏。其 Panel 选项菜单包含“最大化 Panel”或“最小化 Panel”，并投影与视口菜单及 Action 搜索相同的当前绑定。最大化会沿目标 Panel 的 split 祖先链投影 `100/0` 布局并隐藏同级分支，同时让完整 Panel React 树与所有终端表面保持挂载；最小化恢复规范权重，且不会替换终端 DOM 标识。拆分已最大化 Panel 时会先恢复整棵树，再执行拆分。始终恰有一个 Panel 处于聚焦状态；该 Panel 保留正常终端表面，其他 Panel 都覆盖非交互式 inactive mask。Panel 内的主指针或键盘焦点会把焦点与 mask 转移到该 Panel；仅有辅助指针上下文意图则不会。活动 Tab 会把其记忆的聚焦 Panel 中正在显示的主终端或子终端投影为唯一的 terminal `autoFocus` 目标。因此，Tab 激活、初始 Tab 创建、子终端激活、Panel 拆分、Panel 移动及聚焦 Panel 关闭，都会通过注入表面恢复或转移真实终端输入焦点，而无需让 Workbench 耦合到表面的 DOM。拖动主终端 Panel 标题栏会抬起从当前 Panel DOM 克隆的缩放视觉快照，从可逆布局投影中移除源，并立即重排其余网格。快照保留当前 Panel chrome 与终端 Presentation，且不会挂载第二个 React 终端实例。显示活动子终端的标题栏不可拖动。移到另一 Panel 的方向边缘时，会投影完整最终网格，并在目标处显示空框占位 Panel。放下后以完整源 Panel 替换占位符，并在目标上方、下方、左侧或右侧执行拆分，同时不替换任何终端标识或状态。目标中心刻意不激活，因此没有方向占位符时释放会取消并恢复规范网格。上下文关闭只解析一个目标：当前显示的子终端；否则在仍有其他 Panel 时解析为主 Panel；否则解析为活动 Tab。在 preview 组合中，关闭活动子终端只影响该子终端并返回主终端；关闭主视图会影响该 Panel 及全部子终端。在 managed 组合中，同一意图会先把每项受影响 Presentation 标记为 closing，并为每项 Presentation 发出一条精确 close 命令；所有关闭完成前拓扑保持存在。preview、provisioning、restoring 或 reconciling Presentation，以及瞬态 close activity 为 `active` 或 `unknown` 的 `running` Presentation，都需要本地化确认；观测为 `idle` 的 `running` Presentation 会直接关闭。Panel 关闭会统计受影响的主 Presentation 与子 Presentation。Tab 关闭会遍历每个请求 Tab 的每个 Panel，并在 Tab Strip Action 提交前统计全部主/子 Presentation。应用生命周期会跨每个 context 和 Tab 聚合同一谓词；它不会从更宽泛的未停止 session 数量推断保护。确认数量只包含需要保护的受影响 Presentation。部分失败会保留失败 Presentation 与已关闭同级，供显式重试且不回滚。Tab 中最后一个 Panel 不能通过 Panel 生命周期关闭，因为该情况由 Tab 生命周期负责。

managed 终端经过标识校验的自然退出是一项无需确认的终端级移除意图。退出的子终端只移除自身；若它正在显示，Panel 会返回主终端。带有子终端的主终端退出时只关闭已退出的 Presentation；Host 处置完成后，活动子终端会被提升为主终端，没有活动子项时则提升第一个子终端，且不会停止任何存活子终端。没有子终端的主终端退出时，若仍有其他 Panel 则关闭所属 Panel，否则关闭所属 Tab；关闭最后一个 Tab 不会关闭原生应用窗口。退出码和信号不改变该策略，而 `terminal-failed` 仍保留供检查。退出转换会原子进入现有 managed removal 协议，因此已在进行的显式 removal 优先，所有必需 close 完成前拓扑保持存在，处置失败会保留精确目标供重试。自然退出不会新增可配置的保留或重启模式。

终端进程渲染和保留输出搜索执行仍然是终端表面功能。 Find 通过终端 Action 范围及其可用快捷方式公开，而不占用面板标题镶边；仅当安装的查找控制器具有非空查询时，“查找下一个”和“查找上一个”才会同步启用。

已提交的投影终端元数据通过 `applyTerminalMetadataObservation(...)` 进入 Workbench。终端的自然 OSC 标题与持久启动后备标题及可选持久自定义 Tab 标题保持分离。显示优先级依次为自定义标题、聚焦终端的最新自然标题、其当前 CWD 或启动 CWD 的最后一个非空路径段，然后是后备标题。产品创建的后备值是本地化且不带编号的“终端”标签，因此新 Tab 会立即显示启动目录上下文，而不会把标题与身份序号耦合。后续自然标题变化仍记录在自定义标题下；清除自定义标题会立即显示最新自然标题。Host 报告的 OSC 7 CWD 只有在应用提供可信终端机器主机名，且共享策略校验并解码 URI 后才会接受；无效观察不能替换最近一次已接受的当前 CWD。同一观察还携带瞬态关闭活动 `idle`、`active` 或 `unknown`。新建、刚启动、水合及预览 Presentation 都从 `unknown` 开始。已提交的实时投影可以报告当前新鲜值，而已接受的本地非焦点活动会在评估关闭 Action 前立即把该观察失效为 `unknown`。失效时已经在途的 `idle` 帧会一直保持为 `unknown`，直至终端客户端完成其因果快照屏障。该屏障只证明替换帧发生在失效之后；既不证明 Host 已接受，也不证明 shell 已消费。后续因果上合格的当前状态投影可以报告 `active`、`idle` 或 `unknown`，包括在 Host 接受输入后恢复新鲜的 `idle`。该值既不持久化，也不作为终端生命周期权威。

### 建模记录

- **问题和输入：** 用户或未来命令适配器需要确定性的本地 Panel 组合、终端输入连续性及自然退出清理，同时不能把几何或 DOM 输入内部实现耦合到终端进程所有权。可信输入是不可变 `TabModel` 快照、语义 UI 事件、经过标识校验的终端生命周期结果、生成的 managed close 标识，以及注入表面的 renderer-readiness 生命周期。没有活动 Tab 时不渲染 Panel workspace，也没有终端焦点目标。共享文档投影会持久化层级与终端生命周期意图，而焦点、最大化、活动子项及权重投影到 Client View；格式错误的存储处理仍属于应用边界。
- **数据：** `PanelWorkspaceState` 为每个 Tab 保存一个有序 Workspace ID。每个 Workspace 都有一棵规范 `PanelLayoutNode` 树、一个聚焦 Panel ID，以及至多一个最大化 Panel ID。每个叶节点恰好拥有一个主 `TerminalPresentation`、一个有序的一级子项列表，以及零个或一个活动子 Presentation ID。Split 子节点带有稳定、全局唯一且独立于 Panel/Split 标识的布局槽 ID，以及非负百分比权重。每个终端 Presentation 记录 Workbench 拥有的不透明 ID、持久后备标题、瞬态可空自然标题、可空启动/当前 CWD、瞬态 `idle`/`active`/`unknown` 关闭活动，以及可辨识联合形式的活动生命周期。每项 Tab 标题策略记录持久后备及可选自定义标题。异步关闭结果未完成期间，`pendingRemovals` 记录精确的子项、主终端提升、Panel 或 Tab 移除集合；主终端提升 removal 只会在原子提交时选择符合条件的存活子终端。
- **输出：** `transitionPanelWorkspace` 返回完整下一状态、`applied`、`unchanged` 或具名拒绝，以及零条或多条类型化 provision、close 或 Tab-ready 命令。presenter 只为活动 Tab 的聚焦 Panel 及其中正在显示的主终端或子终端投影 `autoFocus`；注入表面在挂载或 renderer ready 后把该投影落实为真实输入焦点效果。自然退出要么原子进入一项 managed removal，要么在同一退出已经进入 removal 时成为等价空操作，或者拒绝过时/冲突结果且不分配第二项 removal。纯转换不执行进程、DOM、存储、时间、随机或网络效果。provision 和 runtime 失败会保留 Presentation 及所属拓扑，供检查或重试。
- **转换：** Tab 协调按 Tab ID 保留 workspace，并只创建或删除发生变化的 Tab 投影。四向拆分、相邻调整大小、焦点、主/子选择、子项创建/关闭、Panel 移动/关闭、最大化/最小化、自定义标题设置/清除，以及完整终端元数据观察，都是受维护的语义事件。managed provision 完成/失败/重试、带有 Workbench 生成 close request 的自然进程退出、关闭完成/失败/重试及 Tab 关闭请求，都是显式关联标识的事件。子终端自然退出会开始 child removal。主终端自然退出时，若仍有子终端则开始 primary-promotion removal；若没有子终端但仍有其他 Panel，则开始 Panel removal；否则开始 Tab removal。提升只在已退出主终端的处置到达 `closed` 后提交；它会选择符合条件的活动子终端，回退到第一个符合条件的子终端，并在所有子终端都被其他 pending removal 作为目标时等待。同轴拆分仅把源权重减半并在本地插入；跨轴拆分会创建嵌套 group。Panel 移动先移除并规范化源位置，再把同一叶节点插入目标周围，且绝不分配新的 Panel 或终端标识。拖动投影应用同一纯树变换但不提交，因此取消可恢复未触及的规范树。把已经相邻的 Panel 移回当前一侧是刻意的空操作，并保留 split 标识、slot 标识和已调整比例。关闭非聚焦 Panel 会保留焦点；关闭聚焦 Panel 时选择前一个视觉叶节点，只有它位于第一位时才选择后一个。Tab 激活会复用该 Tab 的聚焦 Panel；新协调出的 Tab 从其唯一的第一个 Panel 开始；split 与 move 转换会聚焦插入或移动的 Panel；显示子终端的选择与主终端提升都会保留所属 Panel 作为焦点所有者。managed 移除仅在每个目标都到达 `closed` 后改变成员关系；Tab 最后发出 `tab-close-ready`，并一直保留到 `TabModel` 协调确认删除。新的主终端与子终端继承当前显示源终端的可空 CWD。
- **不变量：** 每个 Tab 至少拥有一个 Panel；Panel、split 和 Presentation ID 都唯一；focused 与 maximized ID 在所属 Tab 内可解析；active child ID 在所属 Panel 内可解析且绝不等于主终端 ID；有活动 Tab 时恰有一个显示中终端成为 `autoFocus` 目标，没有活动 Tab 时则没有目标；每个 split 至少有两个子项，布局 slot ID 保持全局唯一，子项权重在数值容差内合计恰为 `100`，重复 split 方向会被展平，且 resize 只改变分隔符两侧的相邻子项。最大化只改变渲染布局与隐藏状态；它绝不改变拓扑、规范 split 权重、终端表面挂载状态或终端 DOM 标识。关闭或抬起 Panel 会在规范化折叠 split 时保留父 slot 标识，并保留确定性的“前一个优先、后一个兜底”视觉邻居。pending removal 非空、互不重叠、可解析到实时拓扑，且只匹配带有该 removal ID 的 `closing`、`close-failed` 或 `closed` 生命周期状态。待处理的主终端提升 removal 会让已退出主终端保留在原位直至原子提交，而且提升绝不会选择被其他 pending removal 作为目标的子终端。
- **决策：** 分割 `[50, 50]` 的右半部分产生 `[50, 25, 25]`，而不是三分之一。即使生成的视口比 `30px` 窄，分割创建仍然有效；这使语义分割命令保持确定性，而不是让渲染约束拒绝模型。在调整分隔符大小期间，两个相邻子项沿活动轴至少被限制为 `30px`；因此，嵌套的正交分割可以保护面板的宽度和高度。每个接受的调整大小候选者都会立即更新规范布局；完成、指针取消和丢失捕获保留最后接受的布局而不是恢复起始布局。分隔符通过交叉点保持单一不透明 one-CSS-pixel 颜色。面板标题和子条空间永远不会覆盖在终端视口上。无论退出码或信号为何，自然退出始终移除已退出 Presentation，但绝不会把 `terminal-failed` 重新分类为退出。子终端退出只移除该子终端。主终端退出会提升活动子终端或第一个子终端，而不会终止存活项；只有没有子终端的主终端才会升级为 Panel 或 Tab removal。最后一个 Tab 可以关闭而原生 Workbench 窗口仍保留。手动关闭确认和规划中的可配置保留/重启退出模式不在范围内。
- **所有权和依赖关系：** Workbench 是 Panel 布局、Tab 标题投影、逻辑终端 Presentation 标识/生命周期状态、记忆的聚焦 Panel 及表面 `autoFocus` 投影的唯一写入者。`TabModel` 仍对 Tab 顺序与活动 Tab 具有权威性。应用注入标识分配、供 OSC 7 规范化使用的可信终端机器主机名、managed 命令回传调度及 terminal-surface 适配器；该适配器拥有 renderer readiness 与真实 input focus 调用，但不能选择不同的语义目标。应用不能通过该模型提供 executable、environment、Host 终端或 attachment 标识。resizable primitive 拥有原始指针输入、原生键盘行为、分隔符焦点与可访问性，以及候选布局生成。Workbench 不重新解释这些输入绑定；它拥有拓扑校验、相邻对投影、`30px` 策略和唯一语义布局写入。resize 不注册为 Action。Host 仍对 PTY/进程生命周期、真实 CWD 发现、终端渲染及最终 CLI/agent 命令传输具有权威性。Panel 状态、Node 版本、Git 分支及变更数量等未来标题栏元数据刻意排除，直至存在拥有方数据契约；当前三区标题栏为其保留组合点。
- **参考范围：** 焦点选择、默认自然退出关闭与基于提示符的关闭重新分类遵循 Ghostty revision `91f66da24527fa02d92b5fd0b41cd020f553a64c`。采纳的投影包括：每个 Tab 记忆最后聚焦 surface、找不到记忆时回退第一个 surface、新 split 无条件取得焦点、关闭时按“前一个优先、后一个兜底”选择叶节点、不等待的子进程退出后立即关闭 surface，以及重新评估当前 `Terminal.cursorIsAtPrompt()` 状态而不永久记忆输入；其实现见 `src/Surface.zig`、`src/terminal/Terminal.zig`、`macos/Sources/Features/Terminal/BaseTerminalController.swift` 与 `TerminalController.swift`。Orchbee 的一级子终端在 Ghostty 中没有对应物。通过确定性提升保留它们、最后一个 Tab 关闭后保留原生 Workbench 窗口、平台进程采样、查询失败处理、Ghostty 的关闭确认配置与只读情况、异常运行时计时、`wait-after-command`、原生 Tab/window 表示、responder 机制、undo 与 timing 都明确不在一致性范围内。
- **正确性和验证：** 示例测试覆盖方向、局部权重、嵌套树规范化、CWD 继承、Warp 范围标题/CWD 规范化、自定义标题优先级与恢复、子项选择、最大化、最后 Panel 拒绝、“前一个优先、后一个兜底”的关闭焦点、idle/active/unknown 关闭确认，以及逐 Tab 隔离。自然退出示例覆盖仅移除子项、活动子项或首个子项提升、聚焦与非聚焦 Panel 移除、最后 Panel 的 Tab 移除、退出码/信号等价、已有 removal 竞态、处置失败、重试及过时结果。固定 seed 的有状态属性测试生成混合层级/Action 序列（包括每项受维护的单 Tab Action），并在每次接受转换后断言全部不变量。有界探索覆盖全部 `384` 个四 Panel split 布局、全部 `18,432` 种可能 move，以及 `25,728` 次 focus、maximize、terminal、close 与 resize Action。Tab 协调单独覆盖三个 Tab ID 的每个有序子集和更深的生成协调序列。move 安全性另有固定嵌套树回归与更深生成布局探索。resize 证据单独覆盖拓扑校验、相邻对和最小尺寸属性、primitive 到 model 同步、原生键盘布局变更、registration 替换、取消与无关 primitive 失败。managed 生命周期示例覆盖 provision 失败、自然退出 removal、批量关闭、部分失败、重试、过时 provision/close 完成及 Tab-ready 顺序。`TerminalPresentationRemoval` TLA+ 模型检查自然退出准入、两项并发 Presentation、原子拓扑或主终端提升提交、部分失败保留、重复 removal 排除和过时结果不变性；精确状态数量属于共享 runner 的 digest-bound 结果。React 集成测试覆盖活动目标的 `autoFocus` 投影与结构上下文菜单交接。projected-terminal 适配器测试覆盖 ready 前请求、ready 与恢复 session、重复投影及处置。受控的完整 Project/Workspace/Tab/Panel 系统 trace 检查初始与新 Tab 焦点、返回 Tab 后的记忆焦点、新 split 焦点、关闭后的邻居焦点及自然退出 Tab 移除。managed 应用集成还覆盖恢复 Tab bootstrap、精确生命周期结果路由、委托 Tab 关闭、自然退出处置及可见 Tab 间 attachment demand 转移。Storybook 与浏览器审查会练习真实上下文菜单、resize handle、标题栏、Panel drop 与 dialog。

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
	type WorkbenchTerminalSurface,
} from '@orchbee/workbench';
import { ActionRuntime } from '@orchbee/actions';

export function ProductInterface({
	actionRuntime,
	layoutStorage,
	navigation,
	onSettings,
	projectWorkspaceRuntime,
	shortcutPlatform,
	terminalSurface,
}: {
	readonly actionRuntime: ActionRuntime;
	readonly layoutStorage: WorkbenchLayoutStorage;
	readonly navigation?: WorkbenchNavigationPort;
	readonly onSettings: WorkbenchHostActionCallback;
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
			projectWorkspaceRuntime={projectWorkspaceRuntime}
			shortcutPlatform={shortcutPlatform}
			terminalSurface={terminalSurface}
		/>
	);
}
```

为每个已安装的应用程序组合创建一个 `ActionRuntime` 和一个 `WorkbenchProjectWorkspaceRuntime`，并使用该组合处置应用程序拥有的资源。使用系统 CWD 和本地化的第一个终端标题初始化项目/工作区运行时。其内存中终端创建操作保留所请求的选项卡组和可选的插入索引。可选的主机回调（例如设置）仍然返回结构化的 `ActionResult`，可以异步返回它，在提交之前观察 `context.signal`，并在权威突变之前立即调用 `context.markCommitted()`。

提供存储端口后，Workbench 会恢复 `orchbee:workbench:left-panel-width:v1`、`orchbee:workbench:left-panel-visibility:v1` 与 `orchbee:workbench:tab-strip-orientation:v1`。它使用功能拥有的严格 Zod Schema 解析完整 JSON 记录；只在用户调整大小完成后保存宽度，只在对应语义切换 Action 后保存可见性或 Tab 条方向。存储缺失、畸形、不可用或超出范围时，会回退到可见的 `250px` 侧栏及水平 Tab 条，不阻塞渲染。仓库范围的所有权与键契约见 [`docs/frontend/browser-storage.md`](../../docs/frontend/browser-storage.md)。

Workbench 拥有内存中的 Tab 与 Panel 上下文投影；组合应用对 PTY 及终端进程生命周期效果保持权威。托管组合调度类型化 Presentation 命令，并通过 `applyTerminalPresentationEvent(...)` 回送其类型化结果。Tab 关闭采用委托方式：Workbench 标记其中每个 Presentation 正在关闭，在结果待定期间保留拓扑，并仅在每项精确关闭完成后从 `TabModel` 移除 Tab。Desktop Host 可通过 `createCrossWindowDragRuntime` 把 Tab 条指针生命周期连接到 Host 拥有的实时窗口发现、兼容性检查、原生窗口移动、捕获及 move-loop 行为。Workbench 与浏览器应用不具备这些 OS 能力。

Workbench 公开了跨 Workbench、Terminal Host 和预计客户端的运行时主题修订协调契约。 Workbench拥有更替和发表资格；该合同将主机拥有的终端默认值/调色板/光标和客户端拥有的像素、覆盖层、光标和应用程序表面保留在一个匹配的完整版本上。其模型、重试规则、有界证据和主机/客户端细化均保存在 [`docs/terminal-theme-revision.md`](docs/terminal-theme-revision.md) 中。实现了主机比较和设置、完全恢复状态、投影终端帧暂存、渲染器预检和应用程序范围同步发布屏障。仅当每个安装的参与者都可以提交精确的主机标记帧和完整的客户端快照后，修订才变得可见。

### 共享文档与 Client View 边界

`@orchbee/workbench/document` 将带规范修订的 `WorkbenchSharedDocument` 与单个页面的瞬态 `WorkbenchClientView` 分离。Project、Workspace、Tab/Group 成员关系、持久后备及自定义 Tab 标题、Panel/Split 结构、终端 Presentation 放置与持久生命周期意图属于共享状态。活动与选中导航、展开状态、焦点、最大化、活动子 Presentation 和 Split 权重则为客户端本地状态。自然终端标题、当前 CWD、视口需求、滚动、搜索、选择、复制、GPU 状态及 `AttachmentId` 仍不属于这两项契约。

托管的 `WorkbenchProjectWorkspaceRuntime` 通过 `getSharedDocument(...)` 和 `getClientView()` 投影这些合约，通过 `initialDocument` 和 `initialClientView` 进行水合，并且可以通过 `applySharedDocument(...)` 应用广播文档，而无需调度过程影响。 `subscribeDocument(...)` 提供应用耐久性协调器；其共享指纹不会因仅 Client View 的转换而改变。应用程序建立其持久性屏障后，`reconcileDurableTerminalPresentations()` 重播持久性配置、关闭和停止操作，并将保留意图转换为仅恢复命令。

应用生命周期所有者可以在封闭终端创建前捕获 `getRestartableTerminalPresentationIds()`，持久停止这些精确会话，再调用 `stageStoppedTerminalSessionsForRestart(...)`。重启暂存只会用新的 PresentationId 与 provision 操作标识替换已捕获的停止 Presentation，同时保留位置、标题与启动 CWD。它更新共享文档投影，但不会向退役运行时派发 provision 命令；下一应用运行时会通过普通持久协调执行这些效果。已停止或正在终止的 Presentation 不会被捕获或重启。

该包不执行存储或 IPC 效果。组合应用程序在其边界解析合约，并拥有私有存储、共享修订、写入顺序、恢复诊断和最终的终端生命周期效果。版本 1 解析器将省略的 `customTitle` 默认为 `null`，同时不保留未交付的拓扑模式的读取器或别名。维护的模型、不变量和证据在 [`docs/shared-workbench-document.md`](docs/shared-workbench-document.md) 中定义。

Workbench 拥有九个稳定的快捷键声明：Action 搜索、新建 Tab、上下文关闭终端、全部搜索、设置、切换边栏、面板最大化、向右拆分与向下拆分。Apple 默认值依次为 `⇧⌘P`、`⌘T`、`⌘W`、`⇧⌘F`、`⌘,`、`⌘B`、`⇧⌘Enter`、`⌘D` 与 `⇧⌘D`。标准默认值为 `Ctrl+Shift+P/T/W/K/B/Enter/D`、`Ctrl+,`，向下拆分则为 `Ctrl+Shift+E`；未加 Shift 的终端控制序列继续传给 PTY。组合应用注入一张以快捷键标识为键的完整当前 Workbench 绑定表、发布 Workbench 是否为活动快捷键作用域，并可为终端拥有的 Action 注入只读的实际生效标签映射。处于活动状态时，Workbench 通过所属 document 上的一个捕获监听器解析自身绑定，不依赖焦点位于哪个 Workbench 区域或 document body。非活动或已卸载的 Workbench 不参与；全屏设置表面打开时，Web 会让它变为非活动。同一组绑定值驱动匹配、菜单、Tooltip 与 Action 搜索标签，包括 `workbench.search.open` 的“全部”和“Action”两种不同输入。Tab 上下文菜单会投影当前的新建 Tab 与上下文关闭终端标签；没有绑定的命令会让尾部快捷键槽位保持为空。只有对应 Action 当前启用时快捷键才会被消费；不匹配或不可用的绑定会继续传给聚焦表面而不受抑制。成功执行新建 Tab 后，其第一个 Panel 终端会成为唯一的 `autoFocus` 目标，包括从空 Tab 模型创建时。

## 开发

变更触发的统一搜索模型检查要求 Java 17 或更高版本。仓库运行器在工具缺失时下载固定版本的 TLA+ `1.7.4` JAR，并在执行前校验 SHA-256；可设置 `ORCHBEE_TLA2TOOLS_JAR` 以使用已配置且匹配的副本。

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:tla
pnpm --filter @orchbee/workbench test:exports
```
