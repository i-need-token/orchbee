# 项目和工作空间模型

## 现状和范围

这是 Workbench 拥有的项目、目录、工作空间、客户端本地活动位置、终端上下文组合和浏览器项目启动的实施合同。 Workbench 在规范共享结构上维护每个客户端的实时投影，并通过 [`shared-workbench-document.md`](shared-workbench-document.md) 公开其所有权分割。文件系统突变、Git 影响、PTY 所有权、存储 I/O 和跨设备同步不在本合同范围内。

## 问题

一个人必须能够保持独立终端工作可用，同时按项目和工作空间组织其他终端上下文。当前的平面工作台投影无法保留多个独立的选项卡和面板上下文，也无法从项目目录确定它们的默认 CWD。

## 输入和验证

| 输入                               | 信任与缺席                                                              |
| ---------------------------------- | ----------------------------------------------------------------------- |
| `ProjectWorkspaceState`            | 仅在构造或转换不变检查后才可信                                          |
| 项目、目录、工作区和终端上下文 IDs | 稳定的不透明字符串；丢失或过时 IDs 拒绝                                 |
| 目录描述符                         | 一种规范 Folder 或 Worktree kind，加显式 local 或 Environment placement |
| 托管 Worktree 提交                 | 一个经 host 验证的已创建 Worktree 与一个当前源 Directory                |
| 已有 Worktree 接纳                 | 一个经 host 重新校验的已发现 Worktree 与一个当前源 Directory            |
| 浏览器输入的文件夹路径             | 修剪后的非空用户输入；否则保留且未经文件系统验证                        |
| 显示名称                           | 用户或本地化产品输入；允许重复名称                                      |
| 启动配置参考                       | 稳定项目-本地ID和显示名称；可执行有效负载仍属于主机所有                 |
| 显式终端目录                       | 必须是目标项目的可用成员                                                |
| 分体式 CWD                         | 主机报告本地CWD；缺席通过后备链继续                                     |
| 系统 CWD                           | 组合主机提供所需的最终后备                                              |

第一次交付接收规范化的目录描述符并且不执行文件系统查找。浏览器预览还接受手动输入的路径，并为其模拟终端投影构建合成文件夹目录。其`available: true`表示当前预览可以使用启动CWD的路径；这并不能证明该路径存在于磁盘上。更高版本的平台适配器必须在构造权威本地描述符之前验证外部文件系统和 Git 数据。

每个 Directory 是当前承载 Checkout 的实体，并精确拥有一种 placement：

```text
placement = local | environment(EnvironmentId)
```

对 Workbench 而言，Environment reference 是 opaque non-empty string；组合应用拥有 registry validation。
Path uniqueness 由 placement 限定，因此同一拼写可以分别存在于本地与不同 Environment。Directory refresh
不能改变 placement。普通 Folder acquisition、Git Worktree、Workspace 创建/编辑/复制与 terminal-CWD flow
全部仅限 local，并要求 `placement: local`。创建或编辑普通 Workspace，或解析其 terminal CWD，还要求该
local Directory 当前 available。若已有 local Workspace 的 Directory 随后刷新为 unavailable，它在结构上
仍然有效，但在 Directory 再次 available 前不能启动。authoritative remote-registration 例外是
`environment-workspace-registered`：它可以引用被强制为 unavailable 的 Environment Directory，并创建一个
不含 terminal presentation 或 Host effect 的 empty terminal context。该 placement durable 后，组合层 Main
进程可以使用下述专用 remote-terminal operation；普通 local creation 与 CWD resolution 保持不变。

## 数据

```text
ProjectWorkspaceState
├── activeLocation: standalone | workspace(ProjectId, WorkspaceId)
├── standaloneTerminalContextId
├── expandedProjectIds
└── projects[] (ordered)
    └── Project
        ├── id, name
        ├── directories[] (ordered, one or more)
        ├── primaryDirectoryId
        ├── launchConfigurations[] (ordered stable references)
        └── workspaces[] (ordered, zero or more)
            └── Workspace
                ├── id, name
                ├── primaryDirectoryId
                ├── launchConfigurationId (optional)
                └── terminalContextId
```

`Directory` 是内部领域术语，产品界面使用“文件夹”。Git worktree 是带有 `kind: worktree` 及必需 worktree Git 元数据的 Directory。`source` 记录 Directory 如何加入 Project，但绝不决定其语义类型。folder Directory 要么具有 `worktree: false` 的仓库元数据，要么不含 Git 元数据。

第一个交付模型将配置作为项目本地稳定参考启动，其中包含 ID 和显示名称。工作空间选择一个参考或 `null`。命令、参数、环境和其他可执行有效负载仍不在此交付范围内，并且需要稍后接受的平台合同。

每个终端上下文 ID 都拥有一个独立的选项卡模型和面板工作区。项目和工作空间状态引用该上下文；它不会复制选项卡、面板或终端会话状态。

## 不变量

- 项目、目录、工作空间和终端上下文 IDs 在其所属状态中是唯一的。
- 项目顺序是当前项目身份的排列；重新排序不会改变成员资格、活动位置或披露意图。
- 每个项目至少拥有一个有序目录。
- 一个 Project 的全部 Directory 最多引用一个 Environment identity；local Directory 可以与来自该
  Environment 的任意多个不同 path 共存。
- 每个目录的可识别类型与其 Git 元数据一致。
- 每个项目 `primaryDirectoryId` 都会命名其目录之一。
- 启动配置 IDs 在他们的项目中是独一无二的。
- 每个 Workspace 都属于一个 Project，其 `primaryDirectoryId` 命名该 Project 中的一个 Directory。普通
  Workspace transition 要求 local placement；只有 authoritative remote-registration transition 可以引入
  Environment-placed primary Directory。
- 每个非空工作空间 `launchConfigurationId` 都会命名一个项目启动配置。
- 每个工作区都有一个终端上下文 ID，并且没有两个工作区或独立终端共享终端上下文。
- `activeLocation` 是独立的或命名一个当前项目中的一个当前工作区。
- 项目扩展仅改变披露状态，从未改变`activeLocation`。
- 显示名称不必是唯一的，也不能用作身份。
- 拒绝或报告未更改的转换会保留完整的输入状态。

删除 Directory 会保持引用完整性。对于被删除的 Project primary Directory，按照 Project 先前的稳定顺序
选择其后的第一个幸存 Directory，并在必要时环回第一个幸存项。Workspace primary 使用相同环形顺序，但会
跳过所有 remote Directory；若没有幸存的 local fallback，则拒绝删除。删除 Project 的最后一个 Directory
也会被拒绝。

## 输出

`transitionProjectWorkspace` 返回完整的下一个状态、`applied`、`unchanged` 或 `rejected` 结果、命名拒绝
（如果适用）以及 terminal-context command。Command 可以创建 empty 或 populated context、克隆 context，
或处置内存中的 context；纯 transition 不执行 React、DOM、storage、process、filesystem、Git、time、random
或 network effect。

每个终端呈现记录不可变的 `launchCwd` 与主机报告的 `currentCwd` 分开。 Workbench拥有不透明的呈现ID和可区分的生命周期，而应用程序拥有不可重用的身份分配和命令调度。主机终端身份永远不会进入该模型。 `resolveTerminalCwd` 返回一条启动路径及其来源：

```text
explicit Project Directory
→ split terminal CWD
→ available Workspace primary Directory
→ available Project primary Directory
→ system CWD
```

在创建 terminal 前，Workbench runtime 会拒绝无效的 explicit Directory。Unavailable local default
Directory 会被跳过。Environment-placed Workspace primary 会在 Project 或 system fallback 前以
`workspace-directory-remote` 拒绝，因此 local runtime 不会意外把 remote Workspace 变成 local shell。
Host 可以报告 terminal 的 current CWD，而不改变其 launch CWD。Panel split 与 child-terminal creation
继承 current CWD；Workspace duplicate 从记录的 launch CWD 启动新 shell。更改 Project 或 Workspace
primary Directory 永远不会改变已有 terminal CWD。

`WorkbenchProjectWorkspaceRuntime#createEnvironmentTerminalAtLocation` 是 Main-admitted remote Workspace
中首个 terminal 的窄例外。调用方提供精确 Project、Workspace 与 Environment identity。Workbench 校验其
containment 与 placement，要求目标 Terminal Context 精确为空，并把 canonical remote primary-Directory
path 记录为 `launchCwd`。成功调用会精确创建一个顶层 Tab、一个 primary Panel 与一个 `provision`
Presentation，且不改变 active location、Project disclosure 或 Client View。local Workspace、Environment
mismatch 或任何非空 context 都会被拒绝；该操作绝不 fallback 到 local system CWD。它只记录 durable
intent，不执行 process 或 network effect。

`resolveActiveTerminalCreationRoute` 是产品 Action boundary。Standalone 与 local Workspace location
保持既有 local semantic command。Environment-placed Workspace 只有在其精确 Terminal Context 为空、
placement 是 default top-level，且 Electron host 提供 remote-terminal-creation capability 时才 eligible；该
capability 只接收 Project 与 Workspace identity。Action 会等待 canonical client-session publication 包含
精确 created 或 existing terminal identity，不执行 optimistic state、local fallback、navigation、selection
或 focus effect。Action、shortcut、New Terminal button、New Group with Tab 与 Duplicate Tab 都重新校验同一
availability policy，因此其他 Tab-producing path 不能绕过 remote singleton boundary。

### 语义命令投影与终端创建

`@orchbee/workbench/semantic-commands` 是不依赖 DOM、React、transport 与 storage 的已批准
Workbench 检查及精确目标终端创建入口。读取操作消费已验证的 `WorkbenchSharedDocument`，mutation
消费权威的 `WorkbenchProjectWorkspaceRuntime`；它绝不会直接编辑持久文档或自行调度应用提交。

| 建模字段 | 合约                                                                                                                                                                                                                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | 非可视调用方需要稳定的 Project、Directory、Workspace 与 terminal identity，以及一项不会漂移到已聚焦客户端对象的终端创建操作。                                                                                                                                                                  |
| 输入     | 读取投影接收已验证的共享文档。公共终端创建契约只接收精确 `WorkbenchLocationRef` 与可选 Project Directory。Owner operation 还接收 owner 提供的后备标题、固定顶层位置与当前 Workbench runtime。缺失或陈旧 identity 会被拒绝。                                                                    |
| 数据     | Project、Directory、Workspace、terminal-context、Tab、Panel、Presentation 与持久 intent identity 继续由现有 Workbench 模型拥有。该操作不创建平行 Store。                                                                                                                                       |
| 输出     | 读取返回带输入文档 revision 的完整规范顺序快照。Owner operation 返回精确 terminal-context、Tab、Panel、Presentation 与 preview/provision intent identity，或一项类型化拒绝。公共 committed output 会加入组合应用已存储的 revision。                                                            |
| 不变量   | 读取绝不查询 Client View。创建只更改一个已解析 terminal context，保留 active location 与每项已有 active Tab selection，创建一个含单一主 Panel Presentation 的顶层 Tab，并保持其他 context 不变。空目标 context 会把唯一新建的 Tab 作为模型有效性所要求的后备项。拒绝保留完整 Workbench state。 |
| 决策     | Project、Directory 与 Workspace 顺序跟随共享文档。Terminal 顺序依次为 Tab 顺序、Panel 深度优先顺序，再在每个 Panel 内先主 Presentation 后子 Presentation。精确 Workspace 解析先检查 Project membership，再检查 Workspace identity。                                                            |
| 模型     | 读取操作是确定性 projector。`executeWorkbenchTerminalCreate` 是主要同步操作，并把唯一 mutation 委托给 `WorkbenchProjectWorkspaceRuntime#createTerminalAtLocation`。                                                                                                                            |
| 转换     | 解析精确 location → 解析可选或默认 CWD → 验证 Tab 位置并分配 identity → 调用 commit callback → 已有其他 Tab 时添加一个非 active Tab，否则安装首个 Tab 的有效性后备项 → 协调其 Panel Presentation。产品 Action 随后可以把仅选择返回的精确 Tab 作为单独 Client View 投影。                       |
| 依赖     | `@orchbee/tab-strip` 拥有 Tab 插入不变量；Workbench 拥有 Project/Workspace 解析、Panel 构造、Presentation lifecycle 与 command 投影；组合应用拥有 identity 分配、revision 指派、持久提交与进程协调。                                                                                           |
| 验证     | 确定性示例覆盖每项读取投影、拒绝类别及空 context 的首个 Tab 后备项。固定 seed 的有状态属性覆盖有效、陈旧、Directory 不可用及 cross-Project trace、精确 context mutation、identity 唯一性、共享文档有效性与已有 active Client View selection 不变。                                             |

Workbench 返回完整读取快照而非分页结果。Desktop semantic-command coordinator 会对快照分页，并把不透明
HMAC cursor 绑定到 operation、target、coordinator generation 与 revision，而无需把 cursor policy 移入
Workbench。托管创建结果表示 Workbench 投影中已存在一个持久 `provision` intent；只有应用 coordinator 在存储其
canonical revision 后才能把该结果标记为已持久提交。Preview 创建保持明确区分，且不能进入共享
文档。该操作不执行 terminal process、network、filesystem 或 Client View navigation effect。

### 投影的终端元数据和选项卡标题

应用程序使用拥有终端的计算机的可信主机名转发一个完整的已提交终端元数据观察。 Workbench 通过修剪外部空白然后删除 NUL 字符来标准化自然的 OSC 0/2 标题。 `null` 清除自然标题。持久后备标题和可选的持久自定义选项卡标题保持独立。显示优先级依次为自定义标题、聚焦终端的最新自然标题、其当前 CWD 或启动 CWD 的最后一个非空路径段，然后是后备标题。产品创建的后备值使用本地化且不带编号的“终端”标签，因此身份分配绝不会泄漏到可见标题中。设置自定义标题可以修剪它；空白或已显示的输入不变。较晚的自然标题仍然隐藏在自定义标题下方，清除自定义标题会立即显示最新的自然标题。

当 `<host>` 与 ASCII 不区分大小写的比较下的可信终端机主机名匹配时，仅接受 OSC 7 当前目录值 `file://<host>/<percent-encoded-absolute-path>`。空主机、`localhost`、其他主机、非文件方案、格式错误的转义百分比和 non-UTF-8 解码字节将被忽略。接受的路径在替换 `currentCwd` 之前进行百分比解码；忽略的值保留先前的当前 CWD，而报告的 `null` 则将其清除。元数据永远不会改变 `launchCwd`。

该策略是从 Warp 修订版 `086dade63ff7327426d6276d57f85b3f2f313af0` 提炼出的窄行为对等投影。标题状态、覆盖、重置与持久化已在 `app/src/terminal/model/terminal_model.rs`、`app/src/pane_group/mod.rs` 和 `app/src/workspace/view.rs` 中审查；OSC 标题与 CWD 规范化已在 `app/src/terminal/model/ansi/mod.rs` 中审查。确定性场景覆盖自然标题规范化、自定义覆盖、隐藏的自然更新、重置、持久化、本地主机名匹配、URI 解码和拒绝类别。Warp 的带内命令标题抑制与 SSH 块 CWD 抑制不在对等范围内，因为 Orchbee 的投影元数据目前没有公开等价的可信命令或 SSH 事实；本策略不会猜测任一状态。

## 决策和转变

| 意图                | 守卫                                                        | 结果                                                 | 终端上下文命令                                                     |
| ------------------- | ----------------------------------------------------------- | ---------------------------------------------------- | ------------------------------------------------------------------ |
| 激活独立终端        | 无                                                          | 独立状态变为活动状态                                 | 无                                                                 |
| 切换项目扩展        | 已知项目                                                    | 仅披露变更                                           | 无                                                                 |
| 重新订购项目        | 已知项目和可选的已知后续项目                                | 项目移至该项目之前，或在缺席时移至末尾               | 无                                                                 |
| 重新排序工作区      | 已知项目、源工作区和可选的同级目标                          | 工作空间在该项目内移动，或者在不存在时移动到末尾     | 无                                                                 |
| 激活工作区          | 已知的项目和工作空间成员资格                                | 工作区变得活跃并且项目扩展                           | 无                                                                 |
| 创建项目            | 一个标准化的可用目录                                        | 项目、主工作区和活动位置已创建                       | 使用一个选项卡、面板和 shell 投影创建一个上下文                    |
| 注册远程 Project    | 一个 unavailable Environment Folder；placement 与 path 唯一 | 追加无 Workspace 的 Project；active location 不变    | 无                                                                 |
| 注册远程 Directory  | 已知 Project；位于其唯一 Environment 的 unavailable Folder  | 追加 Directory；Project 与 Workspace default 不变    | 无                                                                 |
| 注册远程 Workspace  | 已知 Environment Directory，且尚无其 admitted Workspace     | 追加一个 inactive Workspace；Project disclosure 不变 | 创建一个 empty context；无 Tab、Panel、Presentation 或 Host effect |
| 添加目录            | 已知项目；位置尚未存在                                      | 目录附加到稳定的顺序                                 | 无                                                                 |
| 注册已创建 Worktree | 已知 Project 与可用源；唯一有效 Worktree                    | 追加 Directory；可选追加、展开并激活 Workspace       | 仅在启用 Workspace 选项时创建一个上下文                            |
| 接纳已有 Worktree   | 已知 Project 与可用源；唯一已发现 Worktree                  | 追加 Directory 与 Workspace，并展开和激活            | 创建一个以接纳 Worktree 为根目录的上下文                           |
| 刷新目录            | 已知身份；种类和来源不变；独特的路径                        | 主机观察更新到位；顺序和参考文献保留                 | 无                                                                 |
| 删除目录            | 至少有一个目录幸存；仅要求文件夹类型                        | 引用初选使用循环下一个幸存者选择                     | 无                                                                 |
| 协调启动参考        | 已知项目；唯一非空参考号 IDs                                | 目录替换；删除的工作区选择变为 `null`                | 无                                                                 |
| 将项目设置为主      | 可用项目目录                                                | 未来项目默认值更改                                   | 无                                                                 |
| 创建工作空间        | 可用项目目录                                                | 工作区追加、项目扩展、工作区变为活动状态             | 使用一个选项卡、面板和 shell 投影创建一个上下文                    |
| 编辑工作区          | 已知的工作空间和可用的项目目录                              | 名称或将来的默认更改                                 | 无                                                                 |
| 重复工作区          | 已知且 primary Directory 为 local 的 Workspace              | 复制追加并变为 active                                | 将 layout 与 name 克隆到全新 Tab、Panel 与 Terminal identity       |
| 删除工作区          | 已知工作空间                                                | 工作区被删除；主动删除回退到独立状态                 | 处理其上下文                                                       |
| 删除项目            | 已知项目                                                    | 项目及其工作区元数据被删除；磁盘位置保留             | 处置每个项目工作区上下文                                           |

Remote Project、Directory 与 Workspace registration 是只供 authoritative application owner 使用的纯
Workbench transition。Project registration 通过现有 sequence 分配 identity，接受 owner 提供的 Directory
identity，且不发出 command。Directory registration 仅在精确 existing Project 没有其他 Environment identity
时，向其中追加一个被强制为 unavailable 的 Environment Folder；精确 duplicate 保持 unchanged，cross-Project
location 或第二个 Environment 会 reject。它不改变 Project name、primary Directory、Workspace、active
location、identity sequence 或 terminal context。Workspace registration 为该 Environment Directory 分配一个
Workspace 与 terminal-context identity，但保持 active location 与 Project disclosure 不变；其唯一 command
会 materialize 一个 empty context。Desktop 拥有 remote inspection、registry authorization、随机 Directory
identity、durable idempotency 与 proposal-origin policy。

Remote terminal provision 是作用于该已准入 placement 的独立 Main-owned operation。它不是通用 Project
transition，也不通过普通 local terminal-create semantic command 暴露。Desktop 拥有 Environment
connectivity、target-specific Host selection、commit-before-effect ordering、retry reconciliation 与 process
execution；Workbench 只拥有精确 structural mutation 及其 rejection rule。

运行时的可选转换提交回调仅在任何终端上下文命令或权威状态替换之前针对 `applied` 结果运行。动作适配器使用它来记录它们的提交点。替换后，运行时同步发布新的快照；观察者故障是孤立的，不能重新标记转换或阻止后来的观察者接收它。

删除工作区和移除项目仍然是由 Action 拥有的破坏性转换。无论从下拉菜单、上下文菜单、快捷键还是其他 Action 来源调用，它们都使用在视口中居中并带有全页遮罩的共享模态 Alert Dialog 进行确认。`Escape` 取消，`Enter` 确认；确认被接受前不会提交任何破坏性转换。

预览模式下复制 Workspace 时，会复制主 Directory、启动配置选择、Tab Group、Panel 布局、Presentation 名称，以及每项终端记录的启动 CWD。复制采用允许列表：Tab 与 Group 的焦点、加载、网络、崩溃、权限、丢弃、捕获、内存及其他运行时观察均不复制；Presentation 会以新标识重置为 preview 生命周期。它绝不复制进程标识、输出、命令历史、worktree 或源文件。内存预览会创建具有新标识的普通 shell 投影。托管组合拒绝克隆终端上下文；在当前集成阶段，Desktop 不公开 Project 或 Workspace 复制功能。

## 项目文件夹获取

浏览器应用程序仅以独立终端上下文启动。项目创建控件首先打开平台应用程序使用的相同项目创建菜单。选择 `Select Folder…` 是窄浏览器适应点：工作台预览将打开一个接受一个文件夹路径的对话框，而不是本机文件夹选择器。 `New Folder…` 仍然可见但被禁用，因为此交付不执行文件系统突变。输入处理会修剪外部空白，拒绝空结果，否则保留路径。默认的项目和文件夹显示名称是由 `/` 或 `\\` 分隔的最终非空段；没有普通最终段的类似根的路径会回落到完整的修剪路径。

有效的提交会调用一个项目创建操作。该操作使用 `git: null`、`kind: folder` 和 `source: selected` 构建合成文件夹目录，然后使用现有的原子项目转换。取消和无效输入使状态保持不变。该流程不执行文件系统或 Git 调用，也不提供持久性或真正的终端进程声明。

项目管理对话框使用相同的窄浏览器适配来添加现有文件夹。 其 `Add Folder` 控件拥有文件夹列表的最后一行。 激活它只会用路径字段和紧凑的取消和添加图标操作替换该行； 它们的工具提示显示操作标签和键盘提示，而文件夹行、项目名称字段以及外部取消和保存操作保持稳定。 `Enter` 或添加图标操作修剪并提交非空路径； `Escape`或者本地取消图标操作仅丢弃该内联草稿并保持对话框打开。 项目中已有的确切路径不会提交，并会收到内联说明。 有效路径会调用一个 Project-directory-add 操作，该操作会构造相同的合成文件夹目录并通过现有的 `directory-added` 转换将其附加。 项目名称草稿、主目录、工作区默认值、活动位置和现有终端上下文不会更改。 文件夹添加独立于“项目名称保存”操作提交，与现有的直接“设置主文件夹”和“删除文件夹”操作相匹配。

文件夹列表省略了主要指定和设置主要控件，而项目仅拥有一个文件夹，因为该选择是隐式的。对于多个文件夹，主行带有一个安静的名称，而每个可用的非主行仅当该行悬停或键盘焦点位于其中时才会显示其凸起的“设置主行”操作。将鼠标悬停或聚焦文件夹名称会通过共享工具提示显示完整路径；显示的名称保持紧凑，并且可能会被截断。

当 Desktop 同时提供 local 与 SSH acquisition 时，`Add Folder` 会打开明确的 **添加本地文件夹…** 与
**添加 SSH 文件夹…** 选项。SSH callback 只携带精确的当前 Project identity，并在 application-owned overlay
连接和浏览远端 host 时保持 manager 打开。每个通过 canonical publication 出现的 Environment Folder 都带有
可见且已本地化的 **SSH** 标识及可访问 remote-folder label；local Folder 不显示标识，现有 path Tooltip 继续
提供完整 canonical path。

Desktop 在同一适配点注入窄 existing-Folder 采集能力。它的 Electron Main 所有者打开一个本机单目录选择器，标准化所选的绝对路径，并在返回之前检查该路径是否仍命名为可读、可遍历的目录。检查遵循符号链接，同时保留所选路径拼写；它不执行目录创建、枚举、Git 发现或其他文件系统突变。浏览器和网关客户端不接收本机采集端口，并继续使用上面的手动路径输入演示。

一台安装的工作台最多允许一种本地选择。成功的选择将调用与浏览器条目相同的项目创建或项目目录添加操作。取消是无效操作。有界选择器或验证失败会打开一个本地化诊断并且不执行任何项目变更。在添加选定的文件夹之前，Workbench 会重新检查最新的项目快照：已删除的项目或已添加的另一个操作拒绝的确切路径，而不改变另一个目标。卸载、渲染器文档替换、端口退役、窗口关闭和应用程序处置撤销请求；稍后的本机完成无法提交。维护的桌面策略和证据限制位于 [`apps/desktop/docs/project-folder-acquisition.md`](../../../apps/desktop/docs/project-folder-acquisition.md) 中。

## SSH Project enrollment 与连接控件

Workbench 接受一项用于 Environment-placed Project 的可选平台 capability。Host 提供该 capability 时，
Project-create menu 会增加 **通过 SSH 连接…**；capability 缺失时，该入口会完全消失。Callback 只打开
application-owned workflow，不会通过 Workbench 携带 Project、Environment、path、SSH 或 credential
payload。因此 Browser Preview 与 Gateway 保留共享 Project menu，但不会宣称拥有这项 effect。

另一项可选 callback 只扩展 Project manager 的 `Add Folder` row。它把 canonical target Project identity
传给 application-owned SSH workflow，但绝不通过 Workbench 接受 Environment identity、Directory identity、
path、credential 或 remote listing。最终 Folder 只会通过普通 canonical document publication 出现。

每个已准入 Environment Project 可以拥有一项进程内 connection status：`blocked`、`connected`、
`connecting`、`disconnected`、`interaction-required`、`retrying` 或 `unavailable`。Workbench 把该 status
投影为 Project row 上的安静 indicator，并把其本地化含义加入 row 的 accessible name。Project action menu
仅在 connected 时提供 Disconnect；在 disconnected 时提供 Connect；在 blocked 或 unavailable 时提供
Retry；在 connecting、retrying 或等待 interaction 时提供 disabled status item。调用只向注入的 capability
携带 canonical Project identity。Workbench 不会因这些 control 而在 mount 时启动 SSH、持久化 status、回答
prompt、改变 Project document、导航或创建 terminal。

组合层 Electron application 负责 enrollment input、prompt correlation 与 secrecy、progress、cancellation、
connection effect、canonical Project/Workspace admission、publication waiting 与 initiating-client navigation。
成功 enrollment 会先通过普通 canonical Workbench document 出现；它不会创建第二套 remote-Project model。
持续维护的平台契约与 evidence 限制位于
[`apps/desktop/docs/ssh-environment-connection.md`](../../../apps/desktop/docs/ssh-environment-connection.zh-CN.md)。

## 托管 Worktree 产品提交

Workbench 把 Git Worktree 与 Workspace 保持为不同的领域对象。平台 owner 只有在验证并创建一个 Git
Worktree 后，才能提交 `project-worktree-created`。纯 transition 会验证当前 Project、可用源
Directory、repository 一致性、已创建的 Worktree descriptor，以及 Directory identity 与 path
唯一性。随后它追加 Worktree Directory；如果用户要求，还会追加并激活一个以该 Worktree 为主
Directory 的 Workspace。可选 Workspace 与其 terminal context 属于同一个 transition；选项关闭时，
active location 与 identity sequence 保持不变。

该 transition 不执行 Git 或文件系统 effect，也不选择物理路径。Desktop 拥有当前 effect adapter、
固定托管位置、规范文档 transaction、补偿与恢复。维护契约见
[`apps/desktop/docs/project-worktree-creation.md`](../../../apps/desktop/docs/project-worktree-creation.md)。
现有 Folder 或 Worktree 仍可通过普通 Workspace-create transition 选择，因此多个 Workspace 可以引用
同一个 Directory。

## 已有 Worktree 接纳

Workbench 通过 `project-worktree-adopted` 接收 host 重新校验的已有 Worktree。纯 transition 要求一个
可用源 Directory、一个有效且可用的 `source: discovered` Worktree descriptor、repository 一致性，
以及唯一的 Directory identity 与 path。它以原子方式追加 Directory 与一个 Workspace、展开 Project、
激活该 Workspace，并创建一个以该 Worktree 为根目录的 terminal context。Detached Worktree 保留
`branch: null` 及其精确 `HEAD`。

Desktop 负责只读 Git discovery、不透明 Worktree 选择、时点重新校验和 canonical document 提交。
如果所选路径在 discovery 后已经被表示，Desktop 会对该等价 Directory 使用普通
`workspace-created` transition，而不会添加重复项。Git 始终不被改变。维护的平台契约与证据限制见
[`apps/desktop/docs/project-worktree-adoption.md`](../../../apps/desktop/docs/project-worktree-adoption.zh-CN.md)。

## 项目导航操作菜单

每个项目行和工作区行都拥有一个有序的操作菜单模型。该行的省略号按钮将该模型作为锚定下拉菜单打开；右键单击或长按该行将打开与指针定位的上下文菜单相同的模型。两种投影都保留项目顺序、分组、图标、标签、破坏性演示、禁用状态和回调。它们是渲染适配器，而不是独立的操作定义。

打开任一菜单都不会激活 Workspace、切换 Project 展开状态或以其他方式改变 Project/Workspace 状态。dismiss 不会触发任何 Action。产品只在拥有该产品上下文菜单的行内抑制浏览器原生上下文菜单。从任一投影中选择条目，都会恰好调用同一回调一次。

新建 Workspace 子菜单先显示 Project Folder 选项。当 host 提供 Worktree discovery 时，每个拥有已有
Worktree 的已表示 Git repository 都会在其后贡献一个以源 Project Directory 命名的分组；多个
repository 保持为独立分组。Project 中已有的 Worktree 和新发现的外部 Worktree 共享其 repository
分组。每次重新打开 Project 菜单都会刷新这个时点投影。选择当前 Directory 会调用普通 Workspace
创建；选择外部 Worktree 会调用 host 接纳，并只在重新校验和 canonical commit 后创建 Workspace。

### 已创建 Workspace 的激活

创建 Workspace 的客户端会在创建提交后选中它。普通 Folder 或已登记 Worktree 选项使用同步的
`workspace-created` transition，在一次 Workbench commit 中追加并激活 Workspace。托管 Worktree
创建和外部 Worktree 接纳由应用 owner 提交，因此其结果会标识 Workspace，但发起操作的 renderer
可能先收到结果、随后才观察到较新的共享文档。Workbench 因而等待当前投影中出现精确的 Project 与
Workspace identity，再调用现有 Workspace 激活 Action。它绝不会根据共享文档中出现了不相关的
Workspace 来推断意图，因此其他客户端无法抢走发起客户端的 active location。

| 建模字段 | 契约                                                                                                                                                                                                                                                        |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | 用户从当前 Workspace 创建新 Workspace 后原本还要手动寻找并选中结果，而应用拥有的 Worktree commit 可能在成功结果到达发起客户端之后才完成发布。                                                                                                               |
| 输入     | 带有非空 `workspaceId` 的可信托管 Worktree 创建成功结果、可信外部 Worktree 接纳成功结果，以及当前已验证的 `ProjectWorkspaceState`。失败、取消、创建结果中的空 `workspaceId` 与卸载都是明确的缺席情况。                                                      |
| 数据     | `WorkspaceCreationActivationState` 为 `idle`、`waiting(ProjectId, WorkspaceId)` 或 `disposed`。等待目标是客户端本地意图，绝不进入共享文档。                                                                                                                 |
| 输出     | 精确目标出现后，一个 `activate-workspace(ProjectId, WorkspaceId)` command 委托给现有 Action 与 navigation port。其他所有 transition 均不发出 command。                                                                                                      |
| 不变量   | 观察到精确 Project membership 与 Workspace identity 之前绝不请求激活；一个等待目标至多拥有一条 command；重复的相同结果保持 unchanged；失败、空结果和 dispose 绝不激活；该适配器绝不修改共享 Project/Workspace membership。                                  |
| 决策     | 当前 Folder 或已表示 Worktree 使用同步创建 transition。应用拥有的成功结果在发布较晚时等待，在发布已经可见时立即激活。较新的成功创建意图会取代更旧且尚未发布的意图，因此以用户最新完成的请求为准。                                                           |
| 模型     | `transitionWorkspaceCreationActivation` 是确定性状态机。sidebar Hook 提供规范化的成功 identity 与当前已发布 identity，并执行返回的激活 command。                                                                                                            |
| 转换     | `mount` 在 `idle` 中开始一次活跃生命周期；`request(target)` 记录或替换等待目标；`observe(publishedTargets)` 仅在包含精确目标时激活并回到 `idle`；`dispose` 在该生命周期内抑制后续请求与观察。                                                               |
| 依赖     | 平台 Worktree capability 拥有 Git、文件系统、canonical commit 与类型化结果。`ProjectWorkspaceState` 拥有 membership。现有 Workspace Action 与应用 navigation port 拥有客户端本地激活和路由。                                                                |
| 所有权   | `@orchbee/workbench` 拥有发起客户端意图和确定性等待；应用拥有 canonical publication ordering；不会引入新的公共 capability、持久状态或文档 writer。                                                                                                          |
| 验证     | 状态机测试覆盖先发布后返回、先返回后发布、精确 identity、替换、重复、失败缺席和 dispose。Hook 与 Workbench 集成测试覆盖托管创建、接纳和普通 Folder 创建。`WorkspaceCreationActivation` 在声明边界内检查发布安全性、最新意图安全性、dispose 与公平观察进展。 |
| 正确性   | refinement 将类型化 capability 成功结果映射为 `request`，把每次渲染的 canonical Project/Workspace 投影映射为 `observe`，并把唯一模型 command 映射到现有激活回调。应用 commit 与文档传输仍是可信依赖；操作系统和 Git effect 不在本声明范围内。               |

## 项目和工作区重新排序手势

侧边栏支持对两个或多个顶级项目以及一个展开项目中的两个或多个工作区行进行主要鼠标或笔指针重新排序。项目和工作区省略号控件、cross-Project 工作区移动、横截面移动、项目嵌套、触摸长按和键盘排序仍不在此交付范围内。使用项目创建的初始工作区没有固定的排序角色；所属项目中的每个工作区都参与相同的订购模型。

语义手势状态为 `idle`、`armed` 或 `dragging`。 `armed`记录了指针身份、来源身份、所属重排序范围、激活来源，不改变权威顺序或项目公开。至少四个 CSS 像素的移动会激活 `dragging`。 DOM 适配器将稳定的直接同级行布局偏移量投影到视口坐标中，并将其垂直中点低于拖动的源行的垂直中点的第一个非源同级报告为可为空的 `beforeId`；没有这样的兄弟意味着该列表的末尾。列表之外的偏移父项不得被计算两次，并且指针的抓取位置不能延迟或提前挤压交叉。项目命中测试仅使用顶级项目行。工作区命中测试仅使用源工作区所属项目中的工作区行。

拖动期间，权威顺序与 DOM 同级顺序保持不变。源行作为平移并抬高的表面沿垂直方向跟随指针；原始紧凑行槽则保持列表几何稳定。每当拖动行的中点越过同级行的垂直中点，源行与目标行之间的同级项就会动画移入腾出的槽位，包括移动到末尾时的最后一个同级项。这种挤压动画与水平、垂直 Tab 条使用相同排序语言；Project 与 Workspace 重排都不使用独立插入标记。同级置换依据 token 化的紧凑导航行占位，而非实时变换矩形。若源 Project 已展开，只有被拖动的 Presentation 变为紧凑形式；权威 `expandedProjectIds` 始终不变。拖动 Workspace 绝不会改变 Project 展开状态。在共享 Project 视口顶部或底部附近，按比例自动滚动会重新计算作用域内目标，同时保持源行与指针连接。

指针移动更新可见的挤压目标。指针释放通过一个项目重新排序或工作空间重新排序操作提交最后预览的目标，并返回到 `idle`；释放抖动无法静默地将可见目标替换为未预览的放置。工作空间重新排序转换仅更改已识别项目的 `workspaces` 数组；它保留工作区身份、活动位置、终端上下文、项目顺序、项目披露和所有工作区字段。缺少项目、源工作区或目标工作区标识会自动拒绝。自我定位和已经满意的展示位置保持不变。在 Action 更改 DOM 顺序之前，适配器会同步删除活动的位移转换，因此提交的布局无法重播旧的转换。指针捕获仅在拖动激活后开始，因此武装按下不会重新定位普通的项目显示或工作区激活单击。激活前释放是普通单击，不执行重新排序。转义、`pointercancel`、意外丢失的指针捕获、卸载、过时的源或过时的目标会在没有操作或顺序突变的情况下取消，并恢复相同的披露意图。在激活的拖动的提交、取消或标识无效之后，适配器会抑制浏览器在文档捕获边界处的合成尾随单击，因此当指针捕获不可用时，公开或激活无法移动到另一行。重复的、自我定位的、已经满足的举动不变；缺少源或目标身份的拒绝而没有部分突变。

## Workspace Agent 概览

Project 与 Workspace 导航界面会投影每个 Workspace 终端上下文中的实时 Agent Session，但不会成为
Agent 状态的所有者。规范层级仍为 Project → Workspace → Tab → Panel → 主终端或子终端
Presentation → 零个或一个 Agent Session。顶部 Tab Strip 仍是唯一常驻的 Tab 管理器；侧边栏绝不重复
Tab、纯终端行、Panel 缩略图、prompt、命令、工具活动、provider 错误或内部标识。

| 建模字段       | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题           | 用户在一个 Workspace 工作时，必须能发现并到达另一个 Workspace 中的实时 Agent，且不挂载非活动终端界面。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 输入           | 一个已验证的 `AgentRuntimeSnapshot`、当前 `ProjectWorkspaceState`，以及每个被引用终端上下文的最新 Tab 与 Panel 拓扑。只表示具有匹配本地 Presentation attachment 的 `lifecycle: live` Session。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 投影           | `projectWorkbenchAgentOverview` 统计并列出 Agent Session，而不是 attachment 或 provider 图标。同一 Session 在一个 Workspace 附着到多个 Presentation 时只显示一次；附着到多个 Workspace 时在每个 Workspace 各显示一次。父 Agent 与子 Agent Session 保持为独立的扁平行。目标顺序依次为可导航优先、当前 Tab 顺序、规范 Panel 叶节点顺序、主 Presentation、子 Presentation 顺序及稳定 Session 标识。状态变化绝不重排行。                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Workspace 摘要 | 没有可表示实时 Session 的 Workspace 保留现有布局图标，不显示 Agent 数量、状态徽标或 Agent 展开控件。一个不同 provider 使用其精选品牌图标；两个 provider 按固定 provider 注册表顺序显示两个完整且重叠的图标，并共享一个组状态徽标；三个或更多 provider 使用 provider-neutral Multi-Agent 图标。可见数量是实时 Session 的精确数量，视觉上最高显示 `99+`，无障碍信息仍保留精确值。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 状态           | 聚合优先级依次为需要处理、失败/取消/中断、成功、工作中、空闲、未知。聚合前先排除 ended Session。Freshness 是独立覆盖层：stale 或 disconnected 使用降级 warning 样式，并在文本中同时包含最后语义状态和健康度。需要处理为橙色、失败为红色、工作中为蓝色、Session 仍 live 的成功为绿色、空闲或未知为中性色、健康降级为 warning 色；每种状态还必须有 glyph 与本地化文本。Provider 图标保留品牌色。状态不使用持续动画，也绝不只依赖颜色。                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 展开           | 每个 Workspace 的 Agent 展开项默认折叠，只存在于进程内，绝不持久化，也不会因激活 Workspace 或 Agent 到来而自动展开。整个侧边栏最多展开一个 Workspace Agent 列表。折叠其 Project 或失去最后一个可表示 Session 会清除展开状态。展开后以扁平行显示所有可表示 Agent，并复用侧边栏现有滚动容器。Workspace 重排手势期间隐藏详细投影，使 Agent 行绝不启动或干扰排序。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Agent 行       | 第一行显示一个 provider identity 与本地化语义状态。第二行显示本地化的 `Tab title · Panel position · primary/child terminal` 上下文。用户派生文本可以在视觉上截断，但必须保留完整无障碍名称。匹配但不可导航的 attachment 可以作为禁用行保留；任何行都不拥有持久选择状态。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| 导航           | `workbench.agent.navigate` 只接受精确的 Project、Workspace 与 Agent Session 标识。执行时，它会在同一 Workspace 的最新拓扑中重新解析该 Session；若目标缺失或不可导航则拒绝；否则选择精确 Tab、显示并聚焦精确 Panel 及主/子 Presentation、移除冲突的最大化，并请求 Host Workspace 路由。侧边栏只调用这一项 Action，绝不直接写入 Tab 或 Panel Store。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 焦点与并发     | 客户端本地导航协调器会同时等待已接受的 Action settlement 与精确的活动 Project/Workspace/Tab/Panel/Presentation 投影，然后才发出一次终端焦点请求。较新的 Agent 点击会取代旧意图。执行被拒、目标消失、dispose 或导航到无关位置时会取消，且不使用 fallback，也不会延迟抢焦点。如果目标消失时路由已经提交，则保留已进入 Workspace 的普通状态，且不聚焦其他终端。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 所有权         | `@orchbee/agent` 仍对 Session 生命周期、provider、状态、attachment 与 freshness 具有权威性。Workbench 拥有只读跨 Workspace 投影、展开交互、语义导航 Action 与终端焦点意图。应用 navigation port 拥有路由发布；terminal-surface adapter 拥有最终 DOM 焦点 effect。不会新增 Agent 状态、展开持久化、历史、prompt 内容、timer、后台终端挂载或通知策略。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 验证           | 确定性投影测试覆盖仅 live 过滤、attachment 去重、跨 Workspace fan-out、稳定拓扑顺序、不随状态变化的顺序、状态优先级及 512 Session 边界。Action 测试覆盖精确重新校验、主/子终端目标、冲突最大化及不可用目标。协调器测试覆盖结果/投影乱序、取代、消失、dispose 与恰好一次焦点。Workbench 集成测试覆盖默认折叠、扁平展开、Project 折叠清理、精确导航与聚焦以及 ended Session 清理。Provider 组件测试覆盖精选单 provider 图标、固定双 provider 合成及三个或更多 provider 的中性合成图标。Presenter 测试覆盖两种 locale 的本地化精确数量与 `99+` 视觉上限。专用 Storybook Story 与浏览器检查覆盖折叠和展开、单/双/三 provider 与无 Agent 摘要、需要处理、失败、工作中及成功状态、唯一展开、精确导航和窄侧边栏裁切；CSS 提供 reduced-motion 后备规则。`WorkbenchAgentNavigation` 为 Action settlement、路由投影、目标消失、取代、dispose 与焦点提供有界顺序及最新意图证据。 |

v1 明确不包含 Project 级汇总、全局 Agent 页面、搜索或筛选、按优先级排序行、拓扑缩略图、行内 Agent
回复或审批、Agent 启动或终止、持久历史、持久展开、自动展开及通知策略变更。因此，折叠的 Project 会隐藏其
Workspace 摘要，直到 Project 再次展开。

## 依赖性和所有权

| 责任                                                                                      | 业主                               |
| ----------------------------------------------------------------------------------------- | ---------------------------------- |
| 项目、目录、工作区、活动位置模型、不变量和产品演示                                        | `@orchbee/workbench`               |
| 选项卡排序、分组、选择和选项卡交互                                                        | `@orchbee/tab-strip`               |
| 面板布局加上终端呈现标识、生命周期和命令投影                                              | `@orchbee/workbench` 面板工作区    |
| 产品描述符和 `en-US` / `zh-CN` 目录                                                       | `@orchbee/i18n`                    |
| 浏览器和网关路径条目呈现和合成文件夹投影                                                  | `@orchbee/workbench`               |
| Existing-Folder 本机选择器、路径规范化和时间点验证                                        | `apps/desktop`                     |
| 类型化平台文件夹获取功能                                                                  | `@orchbee/workbench`               |
| 类型化平台 Worktree 发现、创建与采用 capability                                           | `@orchbee/workbench`               |
| Electron Worktree 发现、创建与采用数据契约                                                | `@orchbee/desktop-client-protocol` |
| 托管 Worktree Git 观察、创建、补偿、恢复与固定路径                                        | `apps/desktop`                     |
| 现有 Worktree 发现、重验证与规范注册                                                      | `apps/desktop`                     |
| SSH Project-create 入口、status indicator 与连接 Action presentation                      | `@orchbee/workbench`               |
| Electron enrollment dialog、prompt UI、publication wait 与发起 client navigation          | `apps/web`                         |
| Electron SSH enrollment 与 connection-control data contract                               | `@orchbee/desktop-client-protocol` |
| Target resolution、interaction lease、connection、inspection、admission 与 reconciliation | `apps/desktop`                     |
| 启动配置可执行有效负载                                                                    | 未来的应用程序或运行时所有者       |
| 其他文件系统突变、Git effect、PTY、同步与恢复                                             | 未来的应用程序或运行时所有者       |
| locale 与平台适配器选择                                                                   | 组合应用                           |

`@orchbee/workbench` 暴露了一种产品能力边界。第一次交付不会创建新的无头程序包，并通过其拥有的操作可用性隐藏或禁用不可用的平台操作。

## 正确性和验证

| 声明                                                                                                        | 确定性证据                                                              | 补充证据                                                                                      | 限制                                                           |
| ----------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| 会员资格和主要参考资料仍然有效                                                                              | 转换示例和不变断言                                                      | 固定种子状态属性测试                                                                          | 生成有限迹线                                                   |
| Directory 后备选择循环到下一个存续项                                                                        | 边界与移除集合示例                                                      | 生成顺序的属性校验器                                                                          | Host 路径标识可信                                              |
| 目录刷新保留身份、顺序和引用                                                                                | 转换和运行时测试                                                        | 不适用                                                                                        | 主机观察值得信赖                                               |
| Remote registration 收敛到一个 Project、Workspace 与 empty context，且无 terminal effect                    | 确定性 transition、document 与 runtime test                             | Desktop remote-admission TLA+ 与 authority test                                               | 有界 trace；Environment reference 为 opaque                    |
| 首个 remote-terminal provision 只填充精确 empty remote context，且绝不使用 local fallback                   | exact-context、mismatch 与 non-empty rejection test                     | Desktop provision TLA+、commit/effect、target-routing 与 forgery test                         | 一个顶层 terminal；runtime availability 仍由 Desktop 拥有      |
| Electron 首个 remote-terminal Action 等待 canonical publication 并保留 Client View                          | route、Action、availability 与 Tab Strip test                           | Desktop creation-surface TLA+、protocol、broker 与 composition test                           | 仅 default singleton；仅 Electron capability                   |
| Electron SSH enrollment 与 Project connection control 保持 capability-scoped 且 canonical-publication-first | Workbench surface、localized dialog、client、controller 与 routing test | Desktop enrollment TLA+、protocol、broker、owner、composition 与 signed loopback SSH evidence | 仅 POSIX Electron；enrollment 不创建 terminal                  |
| 活动位置始终解析                                                                                            | 转换和删除测试                                                          | 有状态属性测试                                                                                | 仅内存状态                                                     |
| CWD 遵循公认的优先级                                                                                        | 决策表单元测试                                                          | 不适用                                                                                        | 仅本地路径投影                                                 |
| 语义读取会解析精确的规范资源，且不回退到 Client View                                                        | 有序读取投影及过时/cross-Project 测试                                   | 共享文档 Schema 验证                                                                          | 完整内存快照；无分页                                           |
| 精确终端创建只改变一个目标上下文，并保留已有选择或唯一有效后备项                                            | 拒绝矩阵、空 context 与产品 Action 测试                                 | 固定种子状态属性迹线                                                                          | 持久提交和修订仍由应用拥有                                     |
| 预计标题和 CWD 遵循声明的 Warp 范围                                                                         | 标准化和运行时投影场景                                                  | Web 桥和引导适配器测试                                                                        | 无带内命令或 SSH-state 奇偶校验                                |
| 工作区副本重置运行时状态并保留布局投影                                                                      | 上下文运行时和集成测试                                                  | 组件场景                                                                                      | 无过程或输出奇偶校验声明                                       |
| 项目披露不会切换终端上下文                                                                                  | 减速机和部件测试                                                        | 键盘和指针交互                                                                                | 受控的浏览器环境                                               |
| 项目重新排序保留身份、活动位置和披露                                                                        | 转移边界和排列测试                                                      | 确定性指针时间线测试                                                                          | 主要鼠标和笔指针                                               |
| 工作区重新排序保留在一个项目内，并保留身份、活动位置和终端所有权                                            | 转移边界和固定种子排列测试                                              | 确定性作用域指针时间线测试                                                                    | 主要鼠标和笔指针；无 cross-Project 移动                        |
| 拖动完成和取消恢复公开和激活意图                                                                            | 手势机器阶段测试                                                        | 确定性指针时间线测试                                                                          | 受控的指针和滚动几何形状                                       |
| 产品行为在两个区域设置中发布                                                                                | 目录奇偶性和组件测试                                                    | 故事书评论                                                                                    | 仅限 `en-US` 和 `zh-CN`                                        |
| 浏览器路径条目创建一个原子项目或无状态                                                                      | 操作、组件和 Web 集成测试                                               | 不适用                                                                                        | 仅合成内存预览                                                 |
| 浏览器路径条目附加一个唯一的文件夹或保持状态不变                                                            | 操作、组件和 Web 集成测试                                               | 不适用                                                                                        | 精确路径比较；没有 FS 呼叫                                     |
| 桌面选择仅提交一个经过验证的选定结果；取消和失败永远不会改变                                                | 主要运行时、代理、客户端、Hook 和集成测试                               | `DesktopLifecycle` 文件夹场景                                                                 | 注入的对话框适配器；真正的选择器是人工证据                     |
| 已停用的渲染器或陈旧/重复的添加目标无法接受延迟的本机完成                                                   | Broker 生命周期和 Workbench 状态机测试                                  | `DesktopLifecycle` 安全和进度检查                                                             | 有界模型和受控事件循环                                         |
| 托管 Worktree 提交会原子添加 Directory 与可选 Workspace                                                     | Transition 与 managed-runtime 测试                                      | Desktop 创建 transaction 模型                                                                 | Git 与文件系统细化仍由 Desktop 拥有                            |
| 现有 Worktree 采用会按仓库分组清单并原子创建一个 Workspace                                                  | Transition、runtime、协议、Hook 与组件测试                              | 真实仓库 Git adapter 测试                                                                     | 时间点本地清单；不修改 Git                                     |
| 发起客户端只会在发布后激活其精确且成功创建的 Workspace                                                      | 激活状态机、Hook、Action、routing 与 component 测试                     | `WorkspaceCreationActivation` 模型                                                            | 仅一个最新待处理客户端意图；信任 canonical commit 与 transport |
| 省略号和行上下文手势公开一个操作菜单模型                                                                    | 组件奇偶校验和调用测试                                                  | 指针交互回顾                                                                                  | 仅项目和工作区行                                               |

同步内存转换、语义投影与终端创建以及浏览器路径条目集合不需要 TLA+。Remote admission ordering、
cancellation、idempotency 与单向 document migration 由 Desktop 已登记的 admission 与 migration model
覆盖。异步 existing-Folder Desktop adapter 由
`DesktopLifecycle` 的 `project-folder-acquisition` 配置覆盖，包括选择、取消、失败、过时目标、
renderer 退役及延迟原生完成。托管 Worktree 的 Git/产品一致性与恢复由 Desktop 单独注册的
`ProjectWorktreeCreation` 模型覆盖。现有 Worktree 的发现与采用使用 Desktop 文档中的有界
transition 与对抗场景；它们不会增加 Git mutation 或恢复义务。客户端本地的成功结果/发布时序交错由
已注册的 `WorkspaceCreationActivation` 模型覆盖。真实 OS 选择器行为、Git 命令细化、其他文件系统
mutation 与跨设备同步仍不属于这些模型的声明范围。

## 交付与恢复

按依赖顺序集成：模型和不变测试、终端上下文生命周期、操作、演示，然后是 Web 内存中预览。原子地替换实验性单上下文工作台 API 及其唯一的应用程序消费者；不保留兼容性路径。

浏览器交付不执行磁盘或进程 effect。Desktop existing-Folder 切片不写入文件系统数据；只有选定
路径可以通过正常 Workbench Action 与共享文档提交进入规范 Project 文档。Desktop 托管 Worktree
创建是当前唯一的 Git mutation 切片，并通过上面的原子 transition 进入产品状态。现有 Worktree
发现是只读的；采用会重新验证不透明选择，并在不修改 Git 的前提下原子注册 Directory 与
Workspace。回滚任一 adapter 都不会使已提交 Project 记录失效。新建 Folder、Worktree 删除或
移动及其他平台 effect 仍在当前权限之外，直到其语义 owner 单独接受、实现并记录。
