# 项目和工作区模型

旧 Browser ingress 与 POSIX SSH Environment 链路已退役。当前 Desktop 提供本地文件夹与 Worktree 能力，Browser Preview 使用内存状态。下文可选 SSH 接口是保留的库契约，当前没有 provider。

## 现状和范围

Workbench 拥有可独立使用的 Workspace、可选 Project 分组、启动资源命名空间、客户端本地选择及终端上下文组合。文件系统与 Git 效果、PTY、持久化 I/O 和规范提交顺序由应用拥有。GUI 文档契约见 [shared-workbench-document.md](shared-workbench-document.zh-CN.md)。

## 问题

用户可以立即创建工作区，再安排分组。项目只是可选的命名组；普通工作区与基于 Worktree 的工作区处于同级。调整组织方式不得改变执行、资源身份或任意客户端的选择。

## 输入和验证

| 输入                      | 契约                                                                           |
| ------------------------- | ------------------------------------------------------------------------------ |
| Workspace 与终端上下文 ID | 独立于分组的稳定身份；不存在则拒绝                                             |
| Project ID                | Workspace 的可空成员关系；非空 ID 必须可解析                                   |
| Directory 与启动配置 ID   | 限于不可变 `resourceScopeId`；禁止把历史原始 ID 全局扁平化                     |
| 本地 Directory            | 可用的本地文件夹或 Worktree 描述；宿主适配器校验外部路径和 Git 事实            |
| 浏览器路径                | 去除首尾空白的非空合成路径；预览不宣称验证文件系统                             |
| Worktree 来源             | 精确 Workspace、资源命名空间、终端上下文和源 Directory；仅重新分组不会使其失效 |
| 显示名称                  | 用户输入或本地化文本；允许重名                                                 |
| 系统 CWD                  | 宿主提供的最终回退，也是独立创建工作区的初始 CWD                               |

Directory 有 `placement: local` 或 `placement: environment(EnvironmentId)`。可辨识种类必须与 Git 元数据一致。Environment Directory 对本地启动不可用；普通本地操作不能把远程 Workspace 悄然变成本地 shell。应用拥有 Environment 权限。

## 数据

```text
ProjectWorkspaceState
├── activeLocation: standalone | workspace(WorkspaceId)
├── standaloneTerminalContextId, expandedProjectIds, allocation sequences
├── projects[]: { id, name } (ordered optional groups)
├── resourceScopes[]: { id, name, directories[], primaryDirectoryId, launchConfigurations[] }
└── workspaces[] (ordered)
    └── { id, name, projectId: nullable, resourceScopeId,
          primaryDirectoryId, launchConfigurationId: nullable, terminalContextId }
```

`Directory` 是领域术语，产品文案使用“文件夹”。`resourceScopeId` 是启动资源及回退 Directory 的稳定命名空间，不是另一种可见分组。多个 Workspace 可以共享它。移动或解散保留所有资源，包括未使用的 Directory 和启动配置。scope 与 Project 身份的权限与生命周期独立。每个资源键都是 `(resourceScopeId, id)`。

Project 不定义仓库身份，也不拥有终端结构。一个项目可以含多个仓库，同一仓库可以出现在多个项目中。每个 Workspace 拥有独立的终端上下文，内含 Tab、Panel 与 presentation。启动配置只保留稳定引用和显示名称，可执行内容仍由宿主拥有。

## 不变量

- Project、资源 scope 和 Workspace ID 在各自集合中唯一；Directory 与启动配置 ID 仅在资源 scope 内唯一。
- 每个 scope 至少有一个 Directory，回退主 Directory 必须可解析。路径在 placement 与命名空间内唯一。
- Workspace 的 Directory 与可选启动配置在其不可变资源 scope 内解析，与可空 Project 成员关系独立。
- 独立终端及 Workspace 的终端上下文 ID 唯一，且恰好解析一次。
- 活动位置只标识 Workspace，不包含 Project；移动、取消分组和解散保留所有客户端的选择。
- 分组操作保留资源目录、CWD、context/Tab/Panel/presentation 身份、attachment、焦点及放大状态，不发出终端或 Git 命令。
- 最后一个 Workspace 关闭后资源目录仍保留；关闭或解散组织结构不会隐式删除启动资源。
- 名称不是身份。拒绝及不变结果保留完整输入状态。
- 首 shell 结构与可选新分组成员关系一起准备；分配失败时两者都不发布。
- 物理等价性由规范 Git common-directory 身份和已验证的 Worktree 路径决定，绝不使用 Project 名称或成员关系。

移除文件夹保持引用完整：scope 回退项使用原顺序中循环向后的首个存活 Directory；Workspace 主目录沿该顺序跳过远程目录。移除最后一个 Directory，或无法为受影响 Workspace 留下本地回退时拒绝。普通文件夹移除不允许移除 Worktree Directory。

## 输出

`transitionProjectWorkspace` 返回状态、`applied`/`unchanged`/`rejected`、命名拒绝原因及终端上下文命令。它不执行 React、DOM、存储、进程、文件系统、Git、时钟或网络效果。运行时在 Action 提交回调和权威状态替换前准备上下文命令，然后同步发布并隔离观察者失败。

新建工作区使用可用的本地化默认名称（如“工作区”“工作区 (2)”）、宿主 CWD 和默认 shell，含一个 Tab 与 Panel。无需 Project 或表单，也不会自动启动 Agent。显式文件夹接入流程仍可一起创建分组和初始工作区。

```text
Workspace 资源 scope 内的显式 Directory
→ 被拆分终端的当前 CWD
→ 可用的 Workspace 主 Directory
→ 可用的资源 scope 回退 Directory
→ 系统 CWD
```

无效显式 Directory 被拒绝，不可用本地默认项被跳过。远程 Workspace 主目录在任何回退前拒绝本地启动。默认项或分组改变时，现有终端保留当前 CWD 和不可变启动 CWD。拆分和子终端继承当前 CWD；预览复制使用已记录启动 CWD 与新身份。托管上下文克隆仍不可用。

### 语义命令投影与终端创建

`@orchbee/workbench/semantic-commands` 读取已验证共享文档，通过权威运行时修改。Directory 列表接收 `resourceScopeId`；Workspace 列表接受可空 Project 过滤，返回成员关系和资源 scope。精确终端位置只包含 Workspace 身份。读取快照保留规范顺序和输入修订，不查询 Client View，分页由应用负责。

`executeWorkbenchTerminalCreate` 只改变精确上下文，保留当前 Workspace 及既有 Tab 选择。空上下文的唯一首 Tab 是有效性回退。结果含精确 context/Tab/Panel/presentation 和 preview/provision 身份。应用分配修订，在进程协调前持久提交，并拥有分页、效果及导航。缺失身份或无效启动目标拒绝且不修改状态。

### 投影的终端元数据和选项卡标题

应用程序使用拥有终端的计算机的可信主机名转发一个完整的已提交终端元数据观察。 Workbench 通过修剪外部空白然后删除 NUL 字符来标准化自然的 OSC 0/2 标题。 `null` 清除自然标题。持久后备标题和可选的持久自定义选项卡标题保持独立。显示优先级依次为自定义标题、聚焦终端的最新自然标题、其当前 CWD 或启动 CWD 的最后一个非空路径段，然后是后备标题。产品创建的后备值使用本地化且不带编号的“终端”标签，因此身份分配绝不会泄漏到可见标题中。设置自定义标题可以修剪它；空白或已显示的输入不变。较晚的自然标题仍然隐藏在自定义标题下方，清除自定义标题会立即显示最新的自然标题。

当 `<host>` 与 ASCII 不区分大小写的比较下的可信终端机主机名匹配时，仅接受 OSC 7 当前目录值 `file://<host>/<percent-encoded-absolute-path>`。空主机、`localhost`、其他主机、非文件方案、格式错误的转义百分比和 non-UTF-8 解码字节将被忽略。接受的路径在替换 `currentCwd` 之前进行百分比解码；忽略的值保留先前的当前 CWD，而报告的 `null` 则将其清除。元数据永远不会改变 `launchCwd`。

该策略是从 Warp 修订版 `086dade63ff7327426d6276d57f85b3f2f313af0` 提炼出的窄行为对等投影。标题状态、覆盖、重置与持久化已在 `app/src/terminal/model/terminal_model.rs`、`app/src/pane_group/mod.rs` 和 `app/src/workspace/view.rs` 中审查；OSC 标题与 CWD 规范化已在 `app/src/terminal/model/ansi/mod.rs` 中审查。确定性场景覆盖自然标题规范化、自定义覆盖、隐藏的自然更新、重置、持久化、本地主机名匹配、URI 解码和拒绝类别。Warp 的带内命令标题抑制与 SSH 块 CWD 抑制不在对等范围内，因为 Orchbee 的投影元数据目前没有公开等价的可信命令或 SSH 事实；本策略不会猜测任一状态。

## 决策和转变

| 意图                    | 条件                                 | 原子结果                                                | 终端效果              |
| ----------------------- | ------------------------------------ | ------------------------------------------------------- | --------------------- |
| 新建工作区              | 有效宿主 CWD                         | 未分组 Workspace、资源 scope、首 shell 及发起客户端选择 | 创建一个上下文        |
| 移动 / 取消分组         | Workspace 与可选目标存在             | 只修改 `projectId`                                      | 无                    |
| 新建项目并移动          | Workspace 存在，组名非空             | 一次转变提交分组和成员关系                              | 无                    |
| 解散项目                | Project 存在                         | 移除分组，将当前成员取消分组                            | 无                    |
| 关闭工作区              | Workspace 存在                       | 只移除该 Workspace；关闭当前选择时回退独立终端          | 只处置其上下文        |
| 关闭全部工作区          | Project 存在，允许空组               | 关闭当前成员，保留组与资源                              | 处置成员上下文        |
| 文件夹接入              | 可用本地文件夹                       | 分组、资源 scope 与初始 Workspace                       | 创建一个上下文        |
| 创建 / 接纳 Worktree    | 精确来源与已验证 Git 描述            | Directory、可选 Workspace、首 shell 和当前分组一起提交  | 必要时创建一个上下文  |
| 重新打开已关联 Worktree | 同一物理 Worktree 已有关联 Workspace | 返回稳定 Workspace，不重复创建                          | 无                    |
| 资源目录编辑            | 有效源命名空间与可解析引用           | 仅更新该命名空间及受影响默认项                          | 无                    |
| 激活 Workspace          | 稳定 Workspace 存在                  | 选中它，若有组则展开当前组                              | 无                    |
| 排序                    | 来源与目标具有相同可空成员关系       | 同级排序，不修改分组                                    | 无                    |
| 复制预览 Workspace      | 本地来源与预览组合                   | 新上下文身份，相同资源命名空间及分组                    | 新 shell 投影         |
| 远程注册                | 权威 Environment 描述                | scope / 非活动 Workspace 与空上下文                     | 无 shell 或 Host 效果 |

关闭工作区和关闭全部工作区是独立的破坏性 Action，使用共享确认对话框。解散项目只是分组操作，不使用破坏性确认。以上操作都不会删除或搬移 checkout。资源编辑使用 scope 身份；仅删除分组不会使进行中的资源操作失效。

## 项目文件夹获取

边栏新建工作区立即执行。可选的创建项目菜单保留选择文件夹与新建文件夹接入。浏览器选择文件夹只收集一个绝对路径，构造合成本地描述；原生 Desktop 选择验证一个目录，不修改磁盘。由文件夹产生的组名使用最后一个路径段，根路径则使用去除首尾空白的完整路径。

管理项目只编辑组名。管理工作区拥有名称、默认文件夹和启动资源文件夹列表。预览中的添加文件夹把末行替换为路径及局部取消/添加控件，或调用宿主选择器。添加文件夹、选择回退目录及移除独立于保存 Workspace 名称提交。命名空间内重复路径被拒绝；移除不改变既有终端 CWD。仅有一个文件夹时省略冗余选择控件。名称 Tooltip 展示完整路径，不可用远程文件夹带本地化 SSH 标记。

一个已挂载 Workbench 只允许一次原生选择。添加所选文件夹前，重新检查同一资源命名空间及重复路径，不受分组变化影响。取消、卸载、端口退役与替换撤销请求。可选 SSH 文件夹回调携带 `resourceScopeId`，不使用可变 Project 身份，也不传递凭据。

新建文件夹先选择父目录，再明确提交一个子名称。文件系统成功和产品成功是独立事实。保留的创建回执标识 Workspace、资源 scope、Directory、context 与首 presentation；可空 Project 仅是信息，重新分组不会使回执失效。工作流等待规范发布和精确路由接纳后才确认。创建成功的文件夹在接入失败或未知时保留，提供基于路径的选择文件夹恢复；未知效果绝不再次 mkdir。Main 生命周期和文件系统规则见 [project-folder-acquisition.md](../../../apps/desktop/docs/project-folder-acquisition.zh-CN.md)。

## SSH Project enrollment 与连接控件

可选 SSH enrollment 和组状态接口当前没有应用 provider。它们不会在挂载时启动 SSH，也不定义执行权限。Environment placement 仍是资源命名空间内的 Directory 属性。未来连接 provider 必须解析这一稳定 placement；可移动 Project 不能代替 Environment、仓库或终端身份。

## 托管 Worktree 产品提交

`project-worktree-created` 接收精确源 Workspace 和 Directory。宿主在异步准备后重新验证其不可变 scope、context 和路径。已移动来源使用当前分组。未分组来源与新 Workspace 在提交 Directory 和首 shell 的同一次转变中新建一个共同 Project。未请求 Workspace 时只改变资源目录。已关闭来源绝不重建。

发布保留本地导航。Git 效果和已知部分结果保留由 [project-worktree-creation.md](../../../apps/desktop/docs/project-worktree-creation.zh-CN.md) 拥有。产品提交失败时保留已知创建的 checkout；重新分组绝不授权补偿或删除。

## 已有 Worktree 接纳

`project-worktree-adopted` 接收宿主重新验证的 `source: discovered` 元数据，包括 detached 的 `branch: null` 和精确 `HEAD`。它与创建共享当前成员关系和首 shell 规则。物理 Git 身份跨所有分组和资源 scope 检查。已关联 Worktree 返回既有 Workspace；等价且未关联的文件夹描述可原位升级，保留 ID。宿主不修改 Git。见 [project-worktree-adoption.md](../../../apps/desktop/docs/project-worktree-adoption.zh-CN.md)。

## 项目导航操作菜单

每个项目行和工作区行都拥有一个有序的操作菜单模型。该行的省略号按钮将该模型作为锚定下拉菜单打开；右键单击或长按该行将打开与指针定位的上下文菜单相同的模型。两种投影都保留项目顺序、分组、图标、标签、破坏性演示、禁用状态和回调。它们是渲染适配器，而不是独立的操作定义。

打开任一菜单都不会激活 Workspace、切换 Project 展开状态或以其他方式改变 Project/Workspace 状态。dismiss 不会触发任何 Action。产品只在拥有该产品上下文菜单的行内抑制浏览器原生上下文菜单。从任一投影中选择条目，都会恰好调用同一回调一次。

行选项与 Project 区域的创建控件在静止时视觉隐藏，但始终保留在正常 Tab 顺序中。悬停、焦点进入
所属行或区域，以及菜单打开时都会立即显示这些控件；隐藏控件不接收指针输入。空区域始终显示
创建控件。这样可在不改变布局的情况下，保留正向与反向键盘发现路径。

行内菜单适配器采用以下决策模型。它不拥有 Project、Workspace 或菜单状态：稳定对象 ID 仍由现有
回调持有，共享菜单原语负责打开、遍历、关闭与焦点交接。
捕获的入口与打开前焦点引用限定在一个 Project 及其精确 Workspace ID 内（`null` 标识 Project 行），
由 `finalFocus` 消费，并随 owner 释放。保持不变的打开前焦点不视为后续交接。

| 输入或转换                                                                                | 结果与不变量                                                                                                                                                                          |
| ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 焦点位于 Project 或 Workspace 行控件时，输入精确的 `Shift+F10` 或无修饰键的 `ContextMenu` | 聚焦并调用本行仍存在、已启用且闭合的省略号入口，恰好打开一次现有 Dropdown Menu。行通过 `aria-keyshortcuts` 声明这些按键。                                                             |
| 多余修饰键、输入法组合输入、重复输入、已被消费的事件，或缺少仍存在且已启用的入口          | 不调用，也不抑制事件。终端输入不附加行监听器。                                                                                                                                        |
| Portal 菜单的 `keydown` 事件通过 React 行冒泡                                             | 实际 DOM 包含关系会拒绝它；已打开的菜单保留键盘所有权。                                                                                                                               |
| 原生 Menu 键释放在 Dropdown Menu 打开后合成 `contextmenu` 事件                            | 本行仍存在、已启用且展开的省略号，只抑制来自本行或其 `aria-controls` 对应已打开 dropdown 的键盘上下文事件。这涵盖 portal 自动聚焦目标，不放宽对象归属，不新增菜单状态，指针行为不变。 |
| 右键点击或长按行                                                                          | 捕获其明确的省略号入口，供 Context Menu 现有的 `finalFocus` 回调使用，保留相同对象目标与操作条目。                                                                                    |
| Escape 或外部关闭                                                                         | 原语关闭菜单且不调用操作；Escape 返回仍有效的明确入口，外部交互沿用原语的焦点策略。                                                                                                   |
| 操作打开对话框或移除其所属行                                                              | 入口消失、对话框拥有焦点，或其他仍存在的控件已取得焦点时，拒绝返回。外部点击、焦点移出和兄弟菜单打开导致的关闭也会抑制显式回焦。适配器不安排延迟回焦，也不会用兄弟对象替代操作目标。  |

`WorkbenchProjectWorkspace.unit.test.tsx` 覆盖目标标识、按键守卫、取消、模态交接、移除，以及
active location 与展开状态不变。导航 Story 为真实浏览器的 Tab/Shift+Tab、计算后可见性、单一菜单
和焦点返回轨迹提供固定场景；仅 DOM 测试不能证明 CSS 可达性。依赖 capability 的条目与本地化原因仍由现有
操作模型负责。此同步适配器不引入异步 owner 或额外 TLA+ 模型。

Workspace 菜单从其资源文件夹和按仓库区分的 Worktree 选项提供新建工作区。重新打开菜单刷新 discovery。仓库标签描述源文件夹，绝不使用 Project 分组。移动到项目提供已有组、新建项目并移动和移出项目。Project 菜单明确区分管理项目、解散项目和关闭全部工作区。

### 已创建 Workspace 的激活

发起客户端在异步创建/接纳前捕获激活意图。完成返回精确 Workspace，并等待其发布，兼容响应/发布的任意顺序。只有最新且存活的意图可以激活一次。Workspace、Tab 或 Panel 导航、显式同一行导航、较新的创建或卸载会退役旧意图。分组改变及另一上下文发布不算导航。就绪判断不包含 Project 身份，完成不能复活已关闭来源。就绪与焦点使用 React 已提交的投影；挂起或被放弃的渲染不能发布成员关系或改变操作目标。迟到完成仍观察最新已提交投影。

## 项目和工作区重新排序手势

侧边栏支持主鼠标或笔指针排序：两个或更多顶级 Project，以及同一当前分组（包括未分组）中的两个或更多 Workspace。Project 和 Workspace 省略号控件、跨 Project 拖动、跨区域拖动、Project 嵌套、触摸长按和键盘排序仍不在此交付范围内。与 Project 一起创建的初始 Workspace 没有固定位置；同组所有 Workspace 参与相同的排序模型。

语义手势状态为 `idle`、`armed` 或 `dragging`。 `armed`记录了指针身份、来源身份、所属重排序范围、激活来源，不改变权威顺序或项目公开。至少四个 CSS 像素的移动会激活 `dragging`。 DOM 适配器将稳定的直接同级行布局偏移量投影到视口坐标中，并将其垂直中点低于拖动的源行的垂直中点的第一个非源同级报告为可为空的 `beforeId`；没有这样的兄弟意味着该列表的末尾。列表之外的偏移父项不得被计算两次，并且指针的抓取位置不能延迟或提前挤压交叉。项目命中测试仅使用顶级项目行。Workspace 命中测试只使用与源 Workspace 具有相同当前可空 Project 成员关系的行。

拖动期间，权威顺序与 DOM 同级顺序保持不变。源行作为平移并抬高的表面沿垂直方向跟随指针；原始紧凑行槽则保持列表几何稳定。每当拖动行的中点越过同级行的垂直中点，源行与目标行之间的同级项就会动画移入腾出的槽位，包括移动到末尾时的最后一个同级项。这种挤压动画与水平、垂直 Tab 条使用相同排序语言；Project 与 Workspace 重排都不使用独立插入标记。同级置换依据 token 化的紧凑导航行占位，而非实时变换矩形。若源 Project 已展开，只有被拖动的 Presentation 变为紧凑形式；权威 `expandedProjectIds` 始终不变。拖动 Workspace 绝不会改变 Project 展开状态。在共享 Project 视口顶部或底部附近，按比例自动滚动会重新计算作用域内目标，同时保持源行与指针连接。

指针移动更新可见的挤压目标。指针释放通过一个项目重新排序或工作空间重新排序操作提交最后预览的目标，并返回到 `idle`；释放抖动无法静默地将可见目标替换为未预览的放置。Workspace 排序仅修改全局 `workspaces` 顺序中的当前同组顺序；保留 Workspace 身份、活动位置、终端上下文、Project 顺序、展开状态和所有 Workspace 字段。缺少源或目标 Workspace，或目标属于另一组时，原子拒绝。自我定位和已经满意的展示位置保持不变。在 Action 更改 DOM 顺序之前，适配器会同步删除活动的位移转换，因此提交的布局无法重播旧的转换。指针捕获仅在拖动激活后开始，因此武装按下不会重新定位普通的项目显示或工作区激活单击。激活前释放是普通单击，不执行重新排序。转义、`pointercancel`、意外丢失的指针捕获、卸载、过时的源或过时的目标会在没有操作或顺序突变的情况下取消，并恢复相同的披露意图。在激活的拖动的提交、取消或标识无效之后，适配器会抑制浏览器在文档捕获边界处的合成尾随单击，因此当指针捕获不可用时，公开或激活无法移动到另一行。重复的、自我定位的、已经满足的举动不变；缺少源或目标身份的拒绝而没有部分突变。

## Workspace Agent 概览

边栏在每个已分组或未分组 Workspace 下直接展示零到多个活动 Agent 行，不重复 Tab 或 Panel 管理界面。`projectWorkbenchAgentOverview` 在每个 Workspace 的精确上下文解析 Session attachment，在该 Workspace 内按 Session 去重，保留规范 Tab/Panel/presentation 顺序。已结束 Session 被排除。Agent runtime 拥有生命周期、provider、状态与 freshness，Workbench 只做投影。

没有 Agent 的 Workspace 不显示 Agent 计数或展开项。一至两个 provider 使用对应标记，更多使用中性多 Agent 标记。计数视觉上限为 `99+`，无障碍文本保留精确计数。状态优先级为 attention、failed/canceled/interrupted、succeeded、working、idle、unknown；freshness 单独呈现，不只靠颜色，也不使用状态动画。

最多展开一个 Workspace 的 Agent 列表，初始折叠且属于客户端本地状态。重新分组、取消分组、解散及临时折叠 Project 保留该 Workspace 的展开意图；Workspace 移除或所有已表示 Session 消失时清除。拖动时临时隐藏详情。`workbench.agent.navigate` 只用 Workspace 和 Agent Session 身份，重新解析当前拓扑后选中精确 Tab、Panel 与子终端，移除冲突放大。既有 Action/投影协调器拒绝陈旧或已替代焦点意图。Agent 列表和导航绝不启动 Agent。

导航来源与取消判断使用 React 最后提交的位置；挂起或被放弃的渲染不能改变该意图。目标是否就绪仍由权威 Agent 快照和运行时拓扑决定。Hook 测试在导航与完成之间加入真实 Agent 观察事件：被放弃的位置渲染保留焦点，而已提交的无关导航取消焦点。

## 依赖性和所有权

Workbench 拥有纯组织模型、Action 适配器、资源对话框与客户端本地视图。Tab Strip 拥有 Tab 语义，Agent 拥有 Session，UI 拥有菜单/对话框原语，i18n 拥有文案。应用拥有文件系统验证、Git 物理身份、规范修订、存储及进程协调。GUI 资源命名空间有意独立于原生协议中全局唯一的 Directory ID。

## 正确性和验证

确定性与固定种子生成测试覆盖独立创建、scope ID 冲突、移动/新建组/取消分组/解散、同级顺序、关闭与关闭全部、资源和选择保留、不可用回退、物理 Worktree 去重及拒绝转变。当前形状测试覆盖图验证与无效 Client View。双客户端运行时测试保留上下文对象身份、焦点、子终端选择与放大；首 shell 分配失败不会发布结构。

Hook 与组件测试覆盖立即创建、菜单、稳定管理目标、Agent 行及延迟激活抑制。Desktop 拥有异步 Git 竞争与部分结果保留；Web 拥有稳定路由与不可用身份。DOM 测试不能证明原生窗口焦点或实际 shell 进程行为。仓库集成 gate 仍由集成负责人执行。

## 交付与恢复

GUI 共享文档与 Client View 只接受当前 version-1 形状。完整图在使用前校验；无效图不会迁移、重置或修复。旧开发形状不享有兼容承诺。见 [shared-workbench-document.md](shared-workbench-document.zh-CN.md)。
