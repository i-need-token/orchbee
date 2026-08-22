# @orchbee/workbench

Orchbee 应用程序的共享产品语义 React shell。

## 适用范围

Workbench 在应用程序拥有的 `ActionRuntime` 中注册其产品操作。侧边栏控件、快捷方式、选项卡条命令、面板按钮和菜单调用这些注册；注册的处理程序是其语义结果的唯一编写者。每个安装的终端视口在同一运行时中创建一个子范围，并拥有稳定的最小终端目录：复制、粘贴、全选、查找、查找下一个、查找上一个、滚动到底部、清除和重置。注入的表面将实现注册到该范围中，并仅为准备好的上下文菜单调用提供修订固定的语义上下文。连续分隔符调整大小会随着规范面板布局的变化而更新，并在交互结束或取消时保留最后的有效布局。面板拖动预览保持可逆的交互状态，而最终的拖动结果会调用相应的Action。

该包装拥有一个全高、水平可调整大小的两面板外壳。左侧边栏默认为`250px`，约束为`200px`–`400px`；右侧面板填充剩余宽度并通过 `@orchbee/tab-strip` 渲染活动的独立或工作区终端上下文。侧边栏控件和`Mod+B`快捷方式隐藏或恢复侧边栏。在两种状态下，控制器均保持安装在一个固定的外壳位置。当侧边栏隐藏时，选项卡条表面内的透明引导位置会保留其足迹，因此控件永远不会与第一个选项卡重叠或绘制单独的背景。本地化侧边栏提供独立的 `Terminal` 位置、`Search`、可扩展的项目和工作空间层次结构以及底部锚定的 `Settings` 操作。项目行控制公开，而不更改活动终端上下文。工作区行切换到其独立的选项卡和面板上下文；返回终端将恢复现有的独立上下文。

右键单击空白选项卡条空间会显示本地化的方向操作。工作台在顶部水平条和左侧垂直条之间切换条和内容布局，而选项卡、组和新选项卡控件保留其现有的上下文菜单。

shell 由 `@orchbee/ui` 组成原语，但不拥有 PTY 进程、文件系统或 Git 效果、设置目标、身份验证、路由或代理编排生命周期。 Workbench 拥有统一的搜索状态和表示，而语义所有者仍然负责可搜索的读取模型和操作注册。机器可读范围合约位于 [`project.json`](project.json) 中。

Workbench 组合显式区分为 `preview` 或 `managed`。两者都会接收应用拥有的终端 Presentation 标识分配器与稳定的初始 Presentation ID。托管组合还接收操作/移除 ID 分配器和窄化的命令调度器；Workbench 绝不分配 Host 终端 ID，也不以 Host 终端标识执行拓扑或生命周期变更。终端表面接缝只向下述响铃效果协调器公开不透明的 Host 终端标识。动态 Presentation ID 不透明，且删除 Presentation 后不得复用。

托管应用可以用恢复的 Presentation 初始化独立上下文。每个种子保留应用拥有的 PresentationId 与启动 CWD，并投影成一个 Tab；Workbench 仍不会推断或持久化 Project、Workspace、Group、Split、进程或 Host 标识。运行时按 PresentationId 在所有终端上下文中路由每个 provision、exit 与 close 结果；目标缺失、重复或过时时会拒绝，且不改变其他 Presentation。

应用程序可以注入 `terminalSurface` 组件。 Workbench 仅在显示的选项卡和可见的面板布局中为终端演示安装它，传递演示模型以及视口的应用程序拥有的操作运行时/范围、狭窄的语义上下文注册缝和响铃效应准入功能，而不是主机功能。非活动选项卡、其他工作区、后台项目和非活动子终端不会安装表面。安装表面永远不会转移 DOM 焦点；面板焦点继续遵循显式主指针或焦点捕获输入，并且注入的终端仅在显式激活后才可以声明其隐藏输入。右键单击会保留菜单打开之前获得焦点的元素，并在菜单关闭时恢复它，除非选定的结构命令明确地将焦点转移到新拆分的面板。这使得 Desktop 将附件和渲染器生命周期绑定到实际视口需求，同时保留 Workbench 对逻辑拓扑和产品焦点的所有权。

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

侧栏搜索控件和 `Mod+K` 打开一个上部视口模式，其中包含 `All` 过滤器和一个空查询。 `Mod+Shift+P` 与 `Actions` 过滤器打开相同的表面。每个全局打开都会启动一个新的内存会话；在 `All`、`Agents`、`Actions` 和 `Settings` 之间切换会保留当前查询，而关闭会丢弃会话。故意不进行文件搜索。

当前本地提供程序通过从已安装的应用程序 `ActionRuntime` 请求子树发现来项目操作。它保持每个运行时拥有的准备好的或未绑定的调用完整，按本地化的操作类别对结果进行分组，解释禁用的功能，显示已解析的工作台快捷方式，并通过 `executePrepared(...)` 调用选定的准备结果。 tab-strip-orientation 操作演示了嵌套输入：选择命令会打开表面内水平/垂直选取器，通过 `prepareInvocation(...)` 绑定所选值，然后执行相同的范围绑定标记。设置提供程序通过 `workbench.settings.open` 投影稳定的根设置目标。

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

每个终端标签页都在标签栏下拥有一个内存中的面板工作区。新标签页从一个全尺寸面板和一个特殊的主终端开始。只有单个面板的主视图会省略面板边框。当终端应用程序不占有鼠标输入时，右键单击终端视口会先显示可用或禁用的最小终端操作，再列出用于创建一级子终端，或在当前面板上方、下方、左侧或右侧拆分面板的结构命令。只有当界面在菜单打开时捕获到精确链接目标，链接操作才会出现。子终端与其主终端占用同一个视口；子终端会显示在真实的 `40px` 底栏中，而主终端永远不会出现在该栏。激活子终端后会增加一个 `32px` 标题栏，其中包含返回操作、几何居中的终端标题和关闭操作。

一旦选项卡具有多个面板，每个面板都会显示标题。 其面板选项菜单当前仅包含最大化面板或最小化面板。 最大化用一个面板替换可见的分割投影，而不会破坏底层的树。 分割最大化面板首先恢复树，然后应用分割。 仅聚焦一个面板； 该面板保留正常的终端表面，而其他每个面板都接收非交互式非活动掩模。 面板内的主指针或键盘焦点将焦点和遮罩转移到该面板； 仅辅助指针上下文意图则不然。 拖动主终端面板标题会提升从该面板的当前 DOM 克隆的缩放视觉快照，从可逆布局投影中删除源，并立即回流剩余网格。 该快照保留当前面板镶边和终端演示，而无需安装第二个 React 终端实例。 显示活动子终端的标题不可拖动。 移动到另一个面板的方向边缘上会投影完整的最终网格，并在目标处显示一个空的框架面板。 删除会用完整的源面板替换该占位符，并将目标拆分为上方、下方、左侧或右侧，而不替换任何终端标识或状态。 目标的中心故意处于不活动状态，因此没有方向占位符的释放会取消并恢复规范网格。 在预览合成中，关闭活动子项仅影响该子项并返回到主终端； 关闭主视图会影响面板及其所有子视图。 在托管合成中，相同的意图首先将每个受影响的演示文稿标记为关闭，并为每个演示文稿发出一个精确的关闭命令。 拓扑保持存在，直到每次关闭完成。 预览、配置、恢复或协调演示文稿以及瞬时关闭活动为 `active` 或 `unknown` 的 `running` 演示文稿需要本地化确认。 一个`running`观察到的演示文稿为`idle`直接关闭。 面板关闭会计算其影响的主要和子演示文稿。 选项卡关闭会遍历每个请求的选项卡中的每个面板，并在选项卡条操作提交之前对所有主要和子演示进行计数。 应用程序生命周期在每个上下文和选项卡上聚合相同的谓词； 它并不能推断出对更广泛的不间断会话计数的保护。 确认计数仅包括需要保护的受影响的演示文稿。 部分失败会保留失败的呈现和已关闭的同级以进行显式重试，而无需回滚。 选项卡中的最后一个面板在面板生命周期中不可关闭，因为选项卡生命周期拥有该情况。

终端进程渲染和保留输出搜索执行仍然是终端表面功能。 Find 通过终端 Action 范围及其可用快捷方式公开，而不占用面板标题镶边；仅当安装的查找控制器具有非空查询时，“查找下一个”和“查找上一个”才会同步启用。

已提交的投影终端元数据通过 `applyTerminalMetadataObservation(...)` 进入 Workbench。终端的自然 OSC 标题与持久启动后备标题及可选持久自定义 Tab 标题保持分离。显示优先级依次为自定义标题、聚焦终端的最新自然标题、后备标题。后续自然标题变化仍记录在自定义标题下；清除自定义标题会立即显示最新自然标题。Host 报告的 OSC 7 CWD 只有在应用提供可信终端机器主机名，且共享策略校验并解码 URI 后才会接受；无效观察不能替换最近一次已接受的当前 CWD。同一观察还携带瞬态关闭活动 `idle`、`active` 或 `unknown`。新建、刚启动、水合及预览 Presentation 都从 `unknown` 开始。已提交的实时投影可以报告当前新鲜值，而已接受的本地非焦点活动会在评估关闭 Action 前立即把该观察失效为 `unknown`。失效时已经在途的 `idle` 帧会一直保持为 `unknown`，直至终端客户端完成其因果快照屏障。该屏障只证明替换帧发生在失效之后；既不证明 Host 已接受，也不证明 shell 已消费。Host 接受后，当前 Host 在该终端生命周期的剩余时间内只能报告 `active` 或 `unknown`。如果 Host 拒绝或以其他方式没有接受输入，因果替换帧可以合法恢复新鲜的 `idle`。该值既不持久化，也不作为终端生命周期权威。

### 建模记录

- **问题和输入：** 一个人或未来的命令适配器需要确定的本地面板组成，而不需要将几何图形耦合到终端进程所有权。可信输入是不可变的 `TabModel` 快照和语义 UI 事件。缺少活动选项卡不会呈现任何面板工作区。共享文档投影保留层次结构和终端生命周期意图，而焦点、最大化、活动子项和权重投影到 Client View 中；格式错误的存储处理仍然是应用程序边界问题。
- **数据：** `PanelWorkspaceState` 为每个 Tab 保存一个有序 Workspace ID。每个 Workspace 都有一棵规范 `PanelLayoutNode` 树、一个聚焦 Panel ID，以及至多一个最大化 Panel ID。每个叶节点恰好拥有一个主 `TerminalPresentation`、一个有序的一级子项列表，以及零个或一个活动子 Presentation ID。Split 子节点带有稳定、全局唯一且独立于 Panel/Split 标识的布局槽 ID，以及非负百分比权重。每个终端 Presentation 记录 Workbench 拥有的不透明 ID、持久后备标题、瞬态可空自然标题、可空启动/当前 CWD、瞬态 `idle`/`active`/`unknown` 关闭活动，以及可辨识联合形式的活动生命周期。每项 Tab 标题策略记录持久后备及可选自定义标题。异步关闭结果未完成期间，`pendingRemovals` 记录精确的子项、Panel 或 Tab 移除集合。
- **输出：** `transitionPanelWorkspace` 返回完整下一状态、`applied`、`unchanged` 或具名拒绝，以及零条或多条类型化 provision、close 或 Tab-ready 命令。它不执行进程、DOM、存储、时间、随机或网络效果。未知标识、无效关闭集合、冲突移除与过时操作结果都会被拒绝且不变更状态；重复选择和等价调整大小事件刻意作为空操作。provision 失败会保留 Presentation 及所属拓扑，供检查或重试。
- **转换：** 选项卡协调按选项卡 ID 保留工作区，并仅创建或删除已更改的选项卡投影。四向分割、相邻调整大小、焦点、主/子选择、子创建/关闭、面板移动/关闭、最大化/最小化、自定义标题设置/清除以及完整的终端元数据观察是维护的语义事件。托管供应完成/失败/重试、进程退出、关闭完成/失败/重试和选项卡关闭请求是显式 operation-ID-correlated 事件。同轴分割通过仅将源权重减半来本地插入；交叉轴分割创建一个嵌套组。面板移动会在目标周围插入相同的叶子之前删除并标准化源位置，并且从不分配面板或终端标识。拖动投影应用相同的纯树变换而不提交它，允许取消以恢复未触及的规范树。将已经相邻的面板移回到其当前一侧是有意的无操作，它保留了分割标识、槽标识和调整大小的比例。关闭非焦点小组可以保留焦点；关闭聚焦面板会选择下一个可视面板，并返回到上一个面板。仅当每个目标达到 `closed` 后，托管删除才会更改成员资格； Tab 最后发出 `tab-close-ready` 并保留直到 `TabModel` 协调确认删除。新的主终端和子终端继承显示的源终端的可为空 CWD。
- **不变量：** 每个选项卡至少有一个面板；面板、分体和演示 IDs 都是独一无二的；在其选项卡中集中并最大化 IDs 解决方案；活跃子 IDs 在其面板内解析；每个分割至少有两个子项，布局槽 IDs 保持全局唯一，子项权重总计在数字容差内恰好为 `100`，重复的分割方向被展平，并且调整大小仅更改与其分隔符相邻的两个子项。关闭或抬起面板可以保留父插槽标识，同时标准化折叠的分割并保留确定性的下一个视觉邻居。待处理的删除是非空的，从不重叠，解析为实时拓扑，并且仅匹配包含该删除 ID 的 `closing`、`close-failed` 或 `closed` 生命周期状态。
- **决策：** 分割 `[50, 50]` 的右半部分产生 `[50, 25, 25]`，而不是三分之一。即使生成的视口比 `30px` 窄，分割创建仍然有效；这使语义分割命令保持确定性，而不是让渲染约束拒绝模型。在调整分隔符大小期间，两个相邻子项沿活动轴至少被限制为 `30px`；因此，嵌套的正交分割可以保护面板的宽度和高度。每个接受的调整大小候选者都会立即更新规范布局；完成、指针取消和丢失捕获保留最后接受的布局而不是恢复起始布局。分隔符通过交叉点保持单一不透明 one-CSS-pixel 颜色。面板标题和子条空间永远不会覆盖在终端视口上。
- **所有权和依赖关系：** Workbench 是面板布局、选项卡标题投影和逻辑终端表示标识/生命周期状态的唯一编写者。 `TabModel` 仍然对选项卡顺序和活动选项卡具有权威性。应用程序注入身份分配、OSC 7 规范化的可信终端机器主机名，并返回托管命令调度；它无法通过此模型提供可执行文件、环境、主机终端或附件身份。可调整大小的基元拥有原始指针输入、本机键盘行为、分隔符焦点和可访问性以及候选布局生成。 Workbench 不会重新解释这些输入绑定；它拥有拓扑验证、相邻对投影、`30px` 策略和唯一的语义布局写入。调整大小未注册为操作。主机对 PTY/进程生命周期、真正的 CWD 发现、终端渲染和最终的 CLI/代理命令传输保持权威。未来的标头元数据（例如面板状态、节点版本、Git 分支和更改计数）会被故意排除，直到拥有数据合约存在为止；当前的三区域标头为其保留了一个合成点。
- **正确性和验证：** 示例测试涵盖方向、局部权重、嵌套树规范化、CWD 继承、Warp 范围标题/CWD 规范化、自定义标题优先级和恢复、子项选择、最大化、最后面板拒绝、空闲/活动/未知关闭确认以及每个选项卡隔离。移动示例还涵盖所有四个方向、身份保留、折叠源插槽重新插入、等效位置无操作和拒绝行为。固定种子有状态属性测试生成混合层次结构/操作序列，包括每个维护的单选项卡操作，并在每个接受的转换后断言每个不变量。有界探索涵盖所有 `384` four-Panel 分割布局、所有 `18,432` 可能的移动以及 `25,728` 聚焦、最大化、终止、关闭和调整大小操作。选项卡协调分别涵盖三个选项卡 IDs 的每个有序子集以及更深层次的生成协调序列。移动安全还具有固定的嵌套树回归和更深入的生成布局探索。调整大小证据分别涵盖拓扑验证、相邻对和最小尺寸属性、基元到模型同步、本机键盘布局更改、注册替换、取消和不相关的基元故障。托管生命周期示例包括配置失败、批量关闭、部分失败、重试、过时配置/关闭完成和选项卡就绪排序。 `TerminalPresentationRemoval` TLA+ 模型检查两个并发呈现、原子拓扑删除、部分失败保留和过时结果不变性；确切的状态计数属于共享运行器的摘要绑定结果。 React 集成测试涵盖最初的单面板投影。托管应用程序集成还涵盖恢复的选项卡引导、精确的生命周期结果路由、委托选项卡关闭以及在可见选项卡之间移动的附件需求。故事书和浏览器审查练习真实的上下文菜单、调整手柄、标题、面板下拉和对话框的大小。

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

Workbench 拥有五个稳定的快捷方式声明，用于侧边栏切换、终端创建、所有搜索、操作搜索和设置。 Apple 默认值为 `⌘B`、`⌘N`、`⌘K`、`⇧⌘P` 和 `⌘,`。标准默认值使用终端安全的 `Ctrl+Shift+B`、`Ctrl+Shift+N`、`Ctrl+Shift+K` 和 `Ctrl+Shift+P`，而设置仍为 `Ctrl+,`；未移位的终端控制序列继续到 PTY。组合应用程序可以注入由快捷方式标识键控的完整当前绑定表。同一个表驱动匹配、工具提示和命令表面标签，包括 `workbench.search.open` 的不同“全部”和“操作”输入。仅当当前启用快捷方式的操作时，才会使用快捷方式；不匹配和不可用的结合继续到聚焦表面而不受抑制。

## 开发

变更触发的统一搜索模型检查要求 Java 17 或更高版本。仓库运行器在工具缺失时下载固定版本的 TLA+ `1.7.4` JAR，并在执行前校验 SHA-256；可设置 `ORCHBEE_TLA2TOOLS_JAR` 以使用已配置且匹配的副本。

```sh
pnpm --filter @orchbee/workbench... build
pnpm --filter @orchbee/workbench typecheck
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/workbench test:tla
pnpm --filter @orchbee/workbench test:exports
```
