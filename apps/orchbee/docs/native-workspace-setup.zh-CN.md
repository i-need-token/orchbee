# 原生 Workspace 创建与组织

## 范围与归属

Workspace 可以独立使用。Project 是可选的组织方式，与目录、Git 检出目录、终端上下文和执行生命周期分离。
Server 拥有规范导航和修订号，Client 拥有选择与输入。本参考扩展
[上下文选择](native-context-selection.zh-CN.md)和 Server 的
[目录契约](../../orchbee-server/docs/workspace-terminal-directory.zh-CN.md)。

在 Windows 上，新建工作区、`Ctrl+B a` 和 `Ctrl+B Shift+N` 会立即准备并提交带有首个默认 shell 的
Workspace，不要求先建 Project、填写名称表单或再次确认。默认结果不属于任何 Project。
在明确的 Project 行上新建工作区，则使用该 Project，包括空项目和非当前项目。
创建 Workspace 不会自动启动 AI 提供商。

Windows TUI 接受创建后，立即显示待创建侧栏项和当前顶部 Tab，不等待目录准备或终端启动完成。
本地化的“新建工作区”名称在准备完成后替换为默认名称，进度在内容区显示，不打开模态框。
此反馈改进限定于 Windows TUI。GUI 保留既有激活所有者，并沿用相同的规范创建语义。

普通 Workspace 与基于 worktree 的 Workspace 同级。每个 Workspace 可以包含多个 Agent，并直接在其下列出。
[Worktree 启动](native-worktrees.zh-CN.md)拥有检出目录的创建和打开；
[Workspace 关闭](native-workspace-close.zh-CN.md)拥有终端释放。

## 创建默认值与结果归属

只读准备捕获确切的发起上下文、选中 Panel、主 Terminal 绑定、修订号和令牌。
即使前台是 Agent presentation，也采样主终端根进程的目录。明确缺失 cwd 时，依次回退到 Workspace 目录、
Windows 用户主目录。采样不可用、根进程缺失或路径无效时拒绝准备，Client 不伪造成功采样。
目录 basename 提供初始名称，根路径使用本地化后备名称。生成名称避开所捕获快照中全部 Workspace 标签，
必要时依次追加 ` (2)`、` (3)` 等后缀。截断时预留后缀空间，整体不超过 512 个 Unicode 标量值。
显式名称保持原样。默认创建和指定 Project 的创建使用同一快速路径。

一次带修订号围栏的 `WorkspaceLaunch` 接受目录，并一并提交 Workspace、Directory、Context、Tab、
Panel 和主 Presentation。Project 成员关系是可空属性，不提前发送 Default Project 创建命令。
Worker 验证字面目录并启动配置的默认 shell。准入失败不发布半成品 Workspace。
已提交但 shell 尚在等待、失败或结果未知时，保留结构 ID，不创建替代品。

邮箱拥有 FIFO 准入、有界准备／命令租约与保留证据。任何可能失败的 snapshot 读取或 UI 交接之前，
先保存已知回执。确切的 Session、修订号、目录、上下文与终端成员证明先于清理、连接和首帧就绪。
其他 Client 保持自身选择。更新的显式焦点或界面意图撤销发起 Client 的自动进入／返回权限，
迟到的回执仍须验证已提交资源，并保留检查能力。

只有匹配终端达到已接受的首帧 Ready 且原始返回权限仍有效，才隐藏紧凑侧边栏。
宽屏、已隐藏或其他浮层场景消费该权限而不改变布局。取消和后续导航不会重新授予权限。
屏幕发布、偏好持久化与终端就绪分属不同职责。

临时显示不伪造资源 ID 或资源动作。校验交接完成前，保留规范选择及源终端。动作获准时就在下一输入事件前
使旧指针映射失效；临时内容为当前视图时，实时面板探测、选择和历史滚动准入均拒绝隐藏的源终端。
键入无法到达它，手势取消仍可清理原有的所有者。更新的焦点或界面意图，即使发生在准备期间，也会让创建继续在后台运行，
并永久撤销自动进入权限。明确拒绝会移除临时显示。结果未知或已提交但尚未完成对账时，沿用已有的请求／资源
恢复事实，不重放，也不创建替代品。
传输通道退役时立即隐藏创建反馈，同时保留这些恢复事实。

已知的 Workspace ID 会替换临时行。仅供创建衔接的规范投影填补侧栏观察器的延迟；同一 Session 的完整
检查点达到或超过提交修订号后即取代它，包括该检查点已经移除 Workspace 的情况。折叠的 Project 仅在检查点
仍将该确切 Workspace 归属于它时显示当前标记。名称从不用于合并创建项，
先前回执也不能为后一次启动提供身份。

## 可选 Project 组织

右键 Workspace 并打开 Project 子菜单。未分组 Workspace 提供新建 Project，已分组的提供离开
Project；随后按规范顺序列出已有 Project，使用准确捕获的 ID。兼容组织选择器仍提供已有 Project、
不属于项目及新建 Project。
新建 Project 只询问名称；`WorkspaceCreateProject` 在一次规范提交中创建 Project 并移入已有 Workspace。
允许重复标签，捕获的 ID 决定目标。Project 可以包含多个仓库的 Workspace。

分组选择器只改变 `workspace.projectId`，保留 Workspace、上下文、Tab、Panel、Presentation、
Directory 和终端身份，以及 cwd、Git 文件、Agent 和发起 Client 的选择。
操作非当前 Workspace 不会进入它。Project 重命名同样针对确切行，不选择子 Workspace。

侧栏拖拽及键盘排序改为经同一非模态 setup 通道提交一次 `navigation.configure`。源捕获 Site、
revision 和完整 Navigation；释放或键盘准入复核该源，并要求已绘制的分组和显示顺序与其一致；
权威层再次检查 CAS。共享纯转换为回执验证
提供精确的预期成员关系与顺序；相同 revision 下的部分变化或无关修改均判为分歧。读取刷新前保留
已知回执，后续恢复绝不重复移动。排序不改变终端选择或物理资源。
[侧栏布局契约](tui-shell-layout.zh-CN.md) 管理落点反馈、取消及局部绑定。

解散 Project 会移除组织关系，并让其 Workspace 保持未分组。关闭全部工作区是单独的显式 Project 操作，
拥有破坏性确认与保留的成员集合。普通 Workspace 关闭不会扩展为 Git 家族或整个 Project。
自动创建的 Project 在只剩一个 Workspace 时继续保留。

| 转换                          | 条件与结果                                             |
| ----------------------------- | ------------------------------------------------------ |
| 打开分组选择器                | 捕获确切 Session、snapshot、目标与当前焦点，不提交变更 |
| 选择已有 Project 或不属于项目 | 提交一次成员关系命令；无变化时修订号不变               |
| 选择新建 Project              | 显示一个有界名称输入框，提交前不创建空 Project         |
| 完整绘制或鼠标选择            | 只有当前令牌和完整呈现的选项能够授权提交               |
| 提交前取消                    | 释放输入捕获，不改变规范状态                           |
| 已知提交                      | 读取提交后的 snapshot 前保留不可变证据                 |
| snapshot 可用                 | 验证结果并保持当前终端与焦点                           |
| 明确拒绝                      | 保留状态，不自动变基或重放                             |
| 交接不确定或后续读取失败      | 保留已知事实并提供恢复，不推断不存在或重复创建         |

选择器支持键盘导航与鼠标选择。选中窗口限制长列表，标签与路径使用安全终端文本。
有界 UTF-8 和括号粘贴复用现有模态解码器。名称允许 512 个 Unicode 标量，
目录路径允许 4,096 个 UTF-8 字节且不得含 NUL。提交包中的剩余输入不能确认下一步或流入终端。
窗口调整、过期绘制令牌和迟到结果不能改变操作目标。

标题 X 和取消使用现有取消路径；表单字段可点击，提交与 Enter 使用同一所有者。项目行在匹配的左键释放时提交准确的已绘制选项。仅 Project 列表内的滚轮每格移动三个选项，每次受可见行数限制并在首尾停止。选择变化后必须完整重绘才能确认；同一数据包中滚轮后紧接 Enter，不能确认尚未绘制的选项。

若一次打开尝试无法完整呈现所需外壳，则以本地化状态提示拒绝该次尝试，且不保留隐藏的输入所有者。
外壳一旦完成过一次绘制，尺寸不足就会暂停同一个界面实例：令牌、阶段、选中的 Project、草稿文本和
编辑器光标均原样保留。暂停期间，普通编辑、选择、滚动与提交会被消费但不改变状态；只有当前本地
`Esc` 返回可用。之后一次完整绘制会恢复同一个界面。因其他所有者遮盖而未出现在某一帧，只会撤销
当前绘制证明；这既不是拒绝，也不是取消。

新建 Project 名称编辑器是 Project 选择器的子页面。`Esc`、标题栏返回或对应指针操作只返回一层，
恢复选择器的选择与草稿，并且只丢弃尚未提交的名称编辑。选择器自身的返回才关闭根草稿。
已经越过提交边界的快速 Workspace 工作在界面隐藏后仍以原操作身份继续；呈现生命周期绝不重放它。

## 显式目录与标签编辑

Workspace Directory 动作（默认为 `Ctrl+B e`）只为 Empty 且未配置目录的 Workspace 接受首个目录。
路径必须是现有绝对目录，不进行 shell 展开、文件系统创建或主目录回退。接受目录本身不启动 shell。
Empty 工作台引导把目录保留为本地化事实，并且仅在没有输入捕获所有者时，通过共享按键提示组件呈现
实际配置的 Workspace Directory 或 Create Tab 快捷键。菜单与草稿共享同一个纯准入查询，区分
已配置、非空、不存在和忙碌目标。

[位置标签编辑](native-location-labels.zh-CN.md)拥有 Workspace 与 Project 重命名。
标签是可编辑的呈现信息，不是选择或执行身份。裸 Windows
[目录启动](native-directory-startup.zh-CN.md)使用相同的独立 Workspace 模型。

## 全局 Workspace 顺序

`workspace move` 和 `workspace move-block` 改变全局显示顺序，不改变 Project 成员关系或终端身份。
有序 ID 始终是规范 Workspace 的完整排列。插入边界与按调用方顺序排列的块保留现有 CLI 语义，
无变化时修订号不变。分组依据成员关系决定呈现，不重新定义物理 Git 关联。
参见 [CLI 工作流](cli-workflows.zh-CN.md)。

## 正确性与验证

维护状态、传输、输入、渲染和集成证据，覆盖一步启动、未分组与指定 Project 创建、原子新项目成员关系、
非当前工作区移动、解散、终端绑定和选择保持、迟到完成以及不重放恢复。
归属套件包括 `state/workspace_setup`、`state/tests/workspace_setup`、
`connection/native/tests/quick_workspace` 和 `terminal_input/dispatcher/tests/workspace_setup`。
`ui/tests/workspace_setup/responsive` 控制准备、启动、焦点、拒绝及恢复的完成顺序；
`tui_render/workspace_sidebar/tests/creation` 检查基于确切身份的观察器交接。这些本地实现轨迹不提供穷尽 TLC 保证。

运行 `cargo test --locked -p orchbee --all-targets --all-features` 及适用根门禁。
此工作流没有独立 TLA 模型。实现轨迹与进程测试不能替代真实终端的鼠标／键盘、视口、首帧与交互评审。
共享提交和归属协议继续保留各自注册的模型。
