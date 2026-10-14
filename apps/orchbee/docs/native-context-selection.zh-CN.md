# 原生上下文选择

## 范围与归属

原生 Client 在已连接的本机 Session 中选择独立 Terminal Context 或 Workspace。Server 拥有导航、
成员、revision 和终端生命周期；Client 拥有活动位置与选择。本参考文档维护
[原生 Tab Session](native-tab-session.zh-CN.md) 的上下文选择扩展，随 Session reducer、传输、
导航渲染及其正确性证据一起更新。

`Ctrl+B [` 和 `Ctrl+B ]` 按 Server 顺序选择上一个、下一个位置：独立上下文，然后是 canonical
全局 Workspace 显示顺序。排序不改变 Project 归属，边界循环。Workspace 和 context ID 确立位置身份；
Project 归属和标签是展示元数据。移动 Workspace 或解散其 Project 保留选择记忆与存活 attachment。
进入其他位置恢复本 Client 上次选择的 Tab 和 Panel，前提是这些身份仍属于该位置；
否则选择首个现存成员，没有 Tab 则进入 Empty。
进入和离开 Empty 都不会创建终端。已有 Tab 控制作用于当前上下文；显式 `c` 使用 Server
已有的目录准入，绝不回退到 home 目录。
Windows 侧栏在当前机器激活 Workspace 时共用此 Session 选择 owner，不打开选择器。
接纳前核验精确 Session／位置身份；先检查 Busy，再判断精确当前位置无需操作。
Session 结算已接受工作期间，机器 UI owner 只保留最后一个尚未接纳的目标。
侧栏当前标识、位置标签及 Tab 一起读取已安装的 Session 选择。
请求生命周期、取消及跨机器交接见[机器导航](machines.zh-CN.md#跨机器工作区导航)。
Windows 裸启动遵循下面的启动偏好；没有偏好时，
[目录启动](native-directory-startup.zh-CN.md)只在符合条件的空 Session 创建首个 Workspace，
其他 Session 保留 standalone Open。两者使用既有首帧就绪契约。

Workspace 创建、可选的 Project 组织和首目录配置属于 [Workspace 设置](native-workspace-setup.zh-CN.md)；
目录启动拥有空 Session 自动创建。两者复用这里的上下文安装 owner。
Panel 布局、远程访问和生产恢复不在范围内。
该未发布 cohort 整体启用和回滚。改变 Client 位置不会修改 canonical 导航或销毁保留的执行。
[位置标签编辑](native-location-labels.zh-CN.md) 修改 canonical Project 或 Workspace 标签，
并保留这里的有类型位置身份。

## Windows 捕获输入的导航器

`Ctrl+B w`、其配置替代键或操作菜单打开捕获输入的导航器。现有入口刷新及已知结果恢复仍归
Session 所有，且只请求一次。关闭不会取消已接受的工作。普通快捷键帮助仍不捕获输入。

两列分别包含规范位置和当前位置的 Tab。ClientView 拥有易失的轮次、精确来源及本地焦点目标，
不拥有导航变更。焦点初始为当前 Tab，否则为当前位置。Up/Down、Home/End 和 PageUp/PageDown
仅在列内移动，不执行激活；Left/Right 选择列，Tab/Shift+Tab 切换列，进入时选择该列当前行。
列表窗口随焦点移动以呈现长列表。活动标记与键盘焦点区分显示。Enter 打开焦点行；无修饰主键
单击打开精确命中行。Esc 返回终端。配置的导航器开关键也可关闭；其本地前缀吞掉其他所有动作，
包括双前缀。可打印输入、信号、粘贴及未知按键在此均不会到达终端。

| 转换       | 守卫、输出与归属                                                                                                                                                                                                                                     |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 进入       | 捕获前取得括号粘贴租约，清除旧前缀，通过现有所有者取消指针手势但不导出，并捕获入口数据包。初始缺失的行仅可首次取得来源与当前焦点。                                                                                                                   |
| 绘制       | 完整绘制成功后，才为每个完整可见行发布轮次／来源／目标凭据。来源为 Session、规范修订和当前位置。关闭携带同一精确 DrawSource。键盘激活还要求已绘焦点；指针激活要求精确行上的匹配主键释放。标签可省略，不完整几何、隐藏行、计数器及页脚无效。          |
| 浏览／协调 | 移动和缩放撤销旧凭据。同一 Session／位置的新规范修订可保留仍存在的焦点身份，但必须重绘。焦点身份丢失或 Session／位置变化会取消；终端输出本身不改变来源或凭据。                                                                                       |
| 激活       | 在 NativeUi 准入处重新验证精确来源与成员关系。现有刷新、变更和冲突选择守卫优先于无操作。返回可区分的 `ExactCurrent`、`Admitted(generation)` 或 `Refused`；仅前两者关闭。已证实的当前目标不提交请求。否则仅解析一次当前序号并使用现有位置／Tab 操作。 |
| 退出       | Esc、外部按下、接受的激活、覆盖捕获或无法呈现的几何清除本地捕获。保留退出数据包、待解码片段和输入屏障，直至成功的新帧及新数据包。已消费的指针决策另行持有匹配释放，即使已重绘；释放、失焦或新的合格按下结束该归属。                                  |

Hover 和 pressed 反馈不移动临时游标或当前上下文。离开已建立手势的行或关闭目标会永久取消激活；移回不会重臂。被覆盖、失败、被替换或未呈现的候选没有新的指针权威。

指针行使用自己的导航器凭据，不复用普通 Tab 栏路径。每个已绘制列有独立滚轮区域：
一格将本地焦点移动三行而不激活，合并工作每次最多处理一个可见列，反向时丢弃旧工作并保留
新的一格。标题关闭控件返回终端。背景滚轮不关闭导航器，也不到达窗格；Shift 保留现有宿主绕过机制。拒绝的激活保留导航器并显示现有通知。不能根据高亮的待完成选择或含糊
的 `None`／`KeepNavigator` 推断目标。现有 Session 状态继续拥有异步选择、失败、记忆的
Tab／Panel 恢复及已接受副作用。

导航器隐藏窗格光标和焦点装饰，呈现本地化列名、焦点、退出及激活提示。紧凑降级时，Esc 和
Enter 的优先级高于可选标题或修订文本。无法呈现一行及退出提示的几何会关闭并显示现有拒绝提示。

Navigator 核心、dispatcher 与 shell 测试覆盖捕获、完整行绘制、一次性转移、释放所有权及鼠标掩码恢复。这些具体输入所有权轨迹替代单独 sidebar TLA 模型。

## 状态与转换契约

输入为已验证的 Workbench snapshot、显式本地方向、当前 Session 状态、确切 attachment 身份
及既有单调操作预算。导航与当前上下文投影必须来自同一 snapshot revision。非活动上下文的
不支持布局不会使当前上下文无效。选择不支持的目标是确定的本地失败，保留原位置。

| 转换              | 守卫与结果                                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 准入位置选择      | 稳定 Session 没有未解决的 mutation、Close 确认/收敛或成员操作时，保留确切目标身份及操作 generation；否则报告忙。          |
| 读取目标 snapshot | 改变 attachment 前重新验证目标的 canonical owner、Session 和 revision。确定不可用或不支持的目标保留原位置。               |
| 清理旧 attachment | 隔离旧输入、帧和 grant，然后在安装不同选择前 detach 确切的原 Viewer。不授权终端 Dispose 或 Start。                        |
| 安装目标          | 通过既有成员 owner 和 mailbox 事务原子发布新上下文、成员、provenance 和可空的 desired attachment。Empty 不伪造 Tab 身份。 |
| Attach 所选 Tab   | 使用确切保留 binding 及既有 setup、首帧和 controller geometry 规则。旧 generation 的迟到结果不能成为当前结果。            |
| 交接后失败        | 保留已知事实并采用既有 deadline/retirement 规则；绝不重试不确定的副作用或合成清理成功。                                   |

已知 Create provenance 必须在离开和返回上下文后保留。Snapshot binding 不能把已知 Pending、
failed、unavailable 或 outcome-unknown Create 变为 attach 许可。保留的 provenance 受协议
context 和 Tab 上限约束，只能依据已验证的成员证据修剪。Pending Close 保留原确切目标，
并在既有结算规则允许前阻止位置变化。当前上下文的刷新及 Create/Move/Rename/Close 结果
处理不得悄悄返回 standalone。

## Client 选择记忆

只读布局查询根据本 Client 的 Panel 记忆及当前 canonical 成员解析未激活 Tab 的 `focusedPanelId`，
无有效记忆时回退首个成员。`selected` 仍为 false；查询不访问目标，不改变导航、附着或终端几何。

`state/navigation_memory.rs` 拥有易失、Client 私有的选择历史。每个确切位置保存上次 Tab ID，
每个访问过的 Tab 保存上次 Panel ID。标签、序号和终端 binding 都不是键。Session 由所属 Session
固定；历史既不是 Server 状态，也不是持久化存储。Herdr 的 Workspace 活动 Tab 和 Tab 布局
焦点是参考行为。

| 输入                    | 守卫与结果                                                                                                                        |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| 准入 Context 离开       | 既有忙碌与目标守卫通过后，记录当前稳定选择。Empty 不添加虚构选择。                                                                |
| 准入 Tab 离开           | 记录离开时逻辑稳定的选择，即使 Ready 帧证明暂时存在空隙；替代进行中的切换不覆盖原记录，绝不记录未访问的 switching 目标。          |
| 解析普通目标            | 先验证 canonical 成员与 provenance；仅当记忆中的 Tab 和 Panel 仍属于确切位置及父 Tab 时使用。没有历史或成员已移除则回退首个成员。 |
| 解析显式 Panel 或 Agent | 显式稳定目标优先于历史；目标不存在时仍按既有规则拒绝，记忆不能替换为其他 Panel。                                                  |
| 安装 canonical 成员     | 修剪不存在的导航位置、Tab 和 Panel。当前上下文成员优先于旧保留条目；离开上下文的历史使用保留成员，直到重新验证。                  |
| 拒绝、替代或取消        | 最多记录先前的稳定离开位置；不把目标标记为已访问，不发出额外命令。清理与 generation 收敛仍由既有规则负责。                        |

记忆受协议中已知 Context 和 Tab 成员上限约束，每个访问过的 Tab 最多保存一个 Panel ID。
本地创建 Tab 或设置 Workspace 的准入也记录原稳定选择，随后仍按既有创建规则选择新成员。
保留不确定 Create provenance 不会为 canonical 已删除位置保留导航记忆。恢复不能修改 Create
许可、选择其他 Client、调整终端尺寸或激活 OS 窗口。普通键盘、鼠标及 CLI 导航共用此 Session
策略。Tab 切换准入后固定目标 Panel ID，候选 snapshot 重排时重新解析序号；
Tab 存在但目标 Panel 在操作中被删除时明确拒绝；整个 Tab 被删除时仍按既有成员规则回退 Tab 或 Empty。
准备好的成员候选同时决定状态与 mailbox 的 Panel 身份，避免两处选择不一致。显式创建、成员修复和 Close 保留既有选择规则。
重启丢弃逐位置历史；Windows 仅保留下面描述的上次显示位置。cohort 回滚不迁移 checkpoint。

确定性轨迹覆盖非首个成员记忆、按身份重排、删除后回退首个成员或 Empty、显式覆盖、被替代或
拒绝的切换、过期保留成员、独立 Client 和已知不可 attach 的 Create provenance。Windows 进程
证据通过 CLI Tab/Workspace/Panel focus 驱动两个真实 ConPTY Client，检查其他 Client 选择及
OS 前台保持不变。这些是命名实现轨迹，不是 context-selection 的穷举模型检查。

## Windows 启动偏好

Windows 本地裸启动为当前 Windows 用户和本地 Session runtime 记住最后成功显示的位置、Tab、
Panel 和 presentation。这是 Client 拥有的导航提示，与 Server checkpoint 及终端生命周期分离。
有版本、有大小上限的 JSON 记录位于私有 `LOCALAPPDATA/Orchbee/tui-position/<runtime-key>`
缓存；键包含用户隔离的 runtime 目录和 runtime base。版本 2 保存 Workspace/context 身份，
不包含可变的 Project 归属。有效的版本 1 记录按严格的历史结构读取，迁移时去除 Project 身份；
无效和不支持的记录保持原样。结构 ID 随 checkpoint 重启保留；新 Server 的 Session ID 和 binding
始终取自当前已验证 snapshot。

渲染器只在工作台未被遮挡、可绘制时返回稳定位置和焦点 pane incarnation。Shell 成功绘制和
flush 后再验证并发布。遮挡、失败、过期或切换中画面不发布。Empty 位置不虚构成员。
应用私有 worker 把待保存位置合并为一份记录，在输入和绘制路径外进行私有加锁原子替换，
退出时 join。多个窗口保持各自实时焦点，仅在启动时读取偏好。最后一次原子发布决定下一次
启动偏好；位置未变的重绘不重复写入。存储失败显示本地化提示，不使存活终端退休。

恢复先验证当前 canonical 归属，再进入普通 bootstrap/attachment。成员已删除则选择该位置
首个现存成员；位置已删除则选择首个非空 canonical 位置，全部为空则进入 standalone Empty。
两种回退均提示。没有记录时，目录启动策略选择符合条件的空 Session 创建或既有 standalone Open。
无效或不支持的记录原样保留，本次禁用
偏好写入并提示。显式 context 或 Agent 启动仍使用确切目标。偏好不授予输入、不改变 Create
provenance、不启动或销毁终端、不改 checkpoint，也不移动其他存活 Client。

此契约分离持久建议、Client 本地选择、待定绘制证明及异步存储发布。准确成员关系与 pane incarnation 保护发布；重启在引导前解析当前成员。失败保留终端状态。编解码/解析、私有缓存与 carrier 测试覆盖过期/被覆盖/失败绘制、独立 Client、合并及原子发布。Windows ConPTY 分离/重开及成员移除轨迹将这些案例连接到 carrier。这些命名实现轨迹不证明全部交错或物理 Windows Terminal 外观。

## 正确性证据

所属实现测试覆盖选择记忆、准确 Workspace/Tab/Panel 身份、取消、过期完成、安装前清理、首帧接纳、启动偏好和导航器输入所有权。

在 `src/terminal_session/state/context_selection.rs`、`src/terminal_session/mailbox/tests/context_selection.rs`、`src/terminal_session/connection/native/tests/context_selection.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。
