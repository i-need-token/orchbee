# 原生 Workspace 设置

## 范围与归属

原生 Client 在已有或新建 Project 中创建 Workspace，并显式配置首个目录。Server 仍是导航和
Directory 的唯一权威；Worker 验证字面路径。本参考文档维护 Client 的交互与命令消费扩展，
依托[上下文选择](native-context-selection.zh-CN.md) 和 Server 的
[目录契约](../../orchbee-server/docs/workspace-terminal-directory.zh-CN.md)。

`Ctrl+B a` 打开 Workspace 创建流程：按 Server 顺序选择已有 Project 或“新建 Project”，
必要时输入并提交 Project 名称，再输入并提交 Workspace 名称。可用时默认选择当前 Workspace
所属 Project，否则选择“新建 Project”。选择器使用 `[` / `]` 和 Enter。每一步完整可见地绘制后，
Enter 才能提交。新 Project 已被接受后，即使取消后续 Workspace 步骤，它仍然保留。

`Ctrl+B e` 仅为上下文为空且没有主 Directory 的当前 Workspace 配置目录。输入为字面的绝对
UTF-8 路径，最多 4,096 字节且不能含 NUL；不回退 home、不做 shell 展开、不创建目录，也没有
目录浏览器。标签保留既有 512 个 Unicode scalar 契约。所有提示、拒绝和已提交结果通知均提供
双语。Windows 裸命令[目录启动](native-directory-startup.zh-CN.md)属于独立流程；
既有 `q` / `d` 退出快捷键保留其含义。
setup owner 暴露目录准入的纯拒绝查询，由草稿打开及动作菜单共用。它区分
已有目录、非空 context、缺少 Workspace 及冲突的本地操作，不改变首个目录
准入，也不调用副作用。

成功创建 Workspace 后，验证其确切提交身份，再通过既有上下文清理与安装 owner 选入。
在本向导中，目录接受不启动 shell；显式 `c` 仍是启动意图。该未发布 cohort 整体启用与回滚。回滚或
取消不撤销已经提交的 Project、Workspace 或 Directory 事实。远程访问、重新绑定、删除、
通用 Directory CRUD、持久化与发布打包不在范围内。
[位置标签编辑](native-location-labels.zh-CN.md) 使用既有 Project 和 Workspace 重命名命令，
扩展这里的准入和结果消费 owner。

## 状态与命令契约

Client 保留完整 Project 身份/标签摘要（包括空 Project），以及活动 Workspace 的目录元数据，
它们与当前上下文来自同一 snapshot revision。选择器的标签和位置不能替代捕获的 Project/Workspace
ID。草稿捕获确切 Site、revision 和目标；并发本地结构操作、Close 和上下文变化不得悄悄
重新定向草稿。Server 仍拒绝旧 revision 和不可用目标。

| 状态或事件             | 结果                                                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------ |
| 打开草稿               | 稳定 Session 准入与新步骤 token；不发送线协议命令。                                        |
| 编辑草稿               | 在同一步骤 token 下编辑有界文本；普通编辑不要求新的首次绘制确认。                          |
| 改变选择器选项         | 分配新 token 并撤销显示授权，直到选中的 Project 目标完整绘制。                             |
| 取消草稿               | 释放 modal 输入所有权；保留 canonical 状态及更早的已知提交。                               |
| 完整绘制               | 仅确认确切当前草稿 token 和完整渲染内容。                                                  |
| 提交                   | 冻结一个有类型命令和一个请求身份；通过既有单 carrier 在原操作预算内排空更早接受的工作。    |
| 确定拒绝               | 保留成员与当前终端，显示失败；不重放命令或自动 rebase。                                    |
| 已知提交               | 在任何可失败的 snapshot 读取或 UI 交接前保留不可变结果。                                   |
| 提交后 snapshot 可用   | 验证确切 Site、revision 和返回身份；推进向导，或通过既有精确 Viewer 清理安装新 Workspace。 |
| 提交后 snapshot 不可用 | 显示并保留已知结果，要求显式 snapshot 恢复；绝不重新提交 mutation。                        |
| 不确定交接             | 以有类型的 outcome unknown 退役 carrier；不重试、不虚构不存在、不运行后续向导命令。        |

原始 UTF-8、括号粘贴、不完整序列和提交后的输入抑制复用同一个有界 modal decoder。
Rename 保留自身有类型的草稿和操作契约。目录字节上限不能用 Unicode scalar 数量近似。
包含 Enter 和后续输入的数据包，不能在下一步完整绘制前提交下一步。尺寸变化、旧 token、
销毁和迟到结果不能为其他草稿授权 mutation。

## 正确性证据

所属实现测试覆盖Project/Workspace 前提、独立目录校验、绘制权限、同包提交、提交后取消、过期结果、保留提交和准确启动路径。

在 `src/terminal_session/state/workspace_setup.rs`、`src/terminal_input/dispatcher/tests/workspace_setup.rs`、`src/terminal_session/connection/native/tests/workspace_setup.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。

## Windows 快速启动 Workspace

`Ctrl+B Shift+N` 打开一个名称／目录表单，确认后创建带有第一个 shell 的 Workspace 并进入
该终端。`Ctrl+B a` 保留高级空 Workspace 向导。快速表单不创建 Git checkout 或分支。
关闭 Workspace 仍是独立动作。
当前机器／Site 上已绘制 Project 的“快速新建工作区”固定该准确 Project，包括空或非当前
Project，无须先选择子 Workspace。打开或取消菜单或表单不导航、不修改规范状态。

### 输入、决策和所有权

Setup 所有者捕获发起 Client 的精确 context、焦点 Panel、主 Terminal binding、revision 和
准备 token。有界只读准备使用新快照和主 Terminal 根进程目录，即使前台是 Agent 展示。
只有明确缺失的根进程 cwd 才回退到 Workspace 主目录，再到 Windows 用户 home；采样不可用、
根进程缺失、路径非法或来源改变均为拒绝。标签、进程采样和可编辑路径仍是不可信输入。
executor 借用确切的存活主 pane Viewer，并校验已记录的 Host epoch 与物理 Terminal 身份；
不会在同一 Session 中再次附着同一 Terminal，也不释放借用的 Viewer。主终端未附着时，
使用临时只读 Viewer，在准备阶段原始清理截止时间内先释放，再接受或拒绝采样。
路径复用既有 Windows 用户路径规范化（包括 `~`）；相对输入基于已准备的来源目录解析。
名称限 512 个 Unicode scalar，路径限 4096 个 UTF-8 字节，不截断。

| Project 情况                     | 选择                                 |
| -------------------------------- | ------------------------------------ |
| 来源是 Workspace                 | 其精确 Project ID                    |
| 没有来源 Workspace；一个 Project | 该精确 Project ID                    |
| 没有来源 Workspace；零个 Project | 确认后创建一个本地化 Default Project |
| 没有来源 Workspace；多个 Project | 要求明确选择已有 Project             |

上表适用于全局快速启动。Project 菜单请求单独携带完整规范目标 Project，以及发起 context、
desired pane 和焦点 pane incarnation。Setup 所有者在接收菜单交接前、只读准备完成后和
表单确认时核对证明；不可变启动计划保留该证明。Project 被删除或替换、来源 context 改变，
或 desired／焦点 pane 改变时拒绝，不回退到其他 Project。仅 context 成员相等不能证明焦点
未变。目录仍来自上述发起来源策略，从不取目标的任意子项。表单显示固定 Project 和可编辑
名称／路径，不显示 Project 选择器，也不创建 Default。菜单原截止时间约束 setup 准入；
准备沿用有界 lease，人工编辑不计时，确认时才开始既有启动操作预算。

目录 basename 提供初始名称，根目录使用本地化回退。Tab 在名称和目录间切换；Project 选择器
使用精确身份，包括重复标签。Setup 草稿拥有原始字段、完整性、绘制回执和 token。Mailbox
拥有 FIFO、截止时间、准备／命令 lease 和保留的 Worker 证据。Server navigation 拥有 revision
及已提交结构身份；Worker／Host 拥有实际 shell 启动。既有 membership 与 attachment 所有者
释放旧 Viewer、attach 新目标并建立首帧。其他 Client 保留其选择。

Project 定向快速启动仅在回执准确终端达到既有已接受首帧 Ready 状态后，从紧凑侧栏返回一次。
Setup 在接受提交时建立本地返回标记，只由同一操作已验证的 membership 提交推进。标记保留
操作 generation 和准确 desired anchor；保留的回执提供 Project、Workspace、context、Tab、
Panel 及 presentation 身份。任一不匹配都使标记退役。ClientView 单独记住该动作是否从已打开
的紧凑侧栏发起。每个更新的显式动作（包括被拒绝的焦点动作）、关闭、鼠标导航或成功的鼠标
菜单进入，都在完成前使相应返回权限退役；其他原生捕获也使其退役。
Project 行及展开标记的鼠标按下会直接取得侧栏导航；shell 适配器在获取 Ready
返回前结算这一更新焦点。之后退出该模式也不恢复权限。返回同一界面或回执目标
不会重新激活。即使可选 client control 不存在，原生 shell 适配器也消费两侧权限，然后仅隐藏
仍有效的原紧凑侧栏并结束其键盘导航。宽屏、隐藏侧栏及其他浮层只消费、不改变布局。
取消、重命名、pending／unknown 回执及全局向导均不发出返回；图形可见性和位置发布不作为
门槛，因为侧栏本身可能覆盖已接受的终端帧。

### 转移和结果

| 状态／事件                      | 守卫及结果                                                                 |
| ------------------------------- | -------------------------------------------------------------------------- |
| 打开快速表单                    | 既有结构权威空闲；捕获来源并排队一次只读准备                               |
| 准备完成                        | 来源和 revision 相同；以新 token 显示预填可编辑字段                        |
| 取消准备／表单                  | 丢弃草稿；不修改 Project 或 Workspace，忽略迟到准备                        |
| 选择 Project／切换字段          | 新 token，并须再次完整绘制才能用 Enter 确认                                |
| 确认                            | 解码完整且已完整绘制；校验两个字段，捕获一个不可变启动计划                 |
| 零个 Project                    | 提交一次 revision-fenced ProjectCreate；必要的新读取前保留其 ID            |
| Project 已知且快照新鲜          | 只继续一次已确认计划；不要求第二次表单确认                                 |
| 启动                            | 提交一次显式规范 cwd、默认 shell 的 WorkspaceLaunch                        |
| 启动已提交                      | 再读取前保留 Project／Workspace／目录／context／tab／Panel／展示 ID        |
| Terminal Started 且证明新鲜成员 | 选择回执准确的 Tab／Panel／presentation，再使用既有清理、attach 及首帧流程 |
| 定向回执终端 Ready              | 仅消费一次匹配的返回；只有原紧凑侧栏权限仍有效时才显示终端                 |
| 更新的界面／焦点意图            | 即使被拒绝也使 submitted／entering 返回权限退役；不关闭重新打开的界面      |
| 明确拒绝                        | 保留原成员及已知先前 Project；不重放或自动 rebase                          |
| 已知 pending／failed 启动       | 保留结构结果；显示可检查结果，不宣称 Ready 或创建替代对象                  |
| 丢失结果或提交后读取／交接失败  | 保留已知证据，必要时退役，显示明确恢复；不重放或补偿                       |

提交已接受后 Escape 不能撤销 Server 工作。迟到输入、旧 token、窄或失败绘制、peer revision
变化及已取消准备不能授权另一启动。Default Project 创建和 Workspace 启动是两个独立提交；
取消或失败绝不删除先前已提交 Project。既有 checkpoint 和位置偏好格式不变；位置发布仍须
目标首帧成功绘制。

### 正确性范围

所属实现测试覆盖当前与非当前 Project 目标、来源/起点保留、一次 Project/Workspace 交接、取消、不明结果不重放、准确新目标和首帧就绪。

在 `src/terminal_session/state/workspace_setup/quick.rs`、`src/terminal_session/connection/native/tests/quick_workspace.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。

## 全局 Workspace 排序

Windows 的 `workspace move <target> --insert-index N` 和 `workspace move-block <targets...> [--before target]` 在同一快照中解析全部精确 ID、标签或 Workspace 后缀，再通过导航权威提交一次。全局显示顺序保留 Project／目录归属、终端身份及各 Client 的选择。索引是原顺序的插入边界；块按调用者顺序移动，允许跨 Project，来源包含 1–1023 个不同的已解析 ID。无变化时保留修订。

导航提交适配器只为这些动作接受专用重排结果，并校验修订；提交后不兼容或关联不符的响应会释放会话并报告结果未知。CLI 输出使用 `orchbee.workspace.move` v1，已提交结果携带完整 `workspaceOrder`，否则明确返回 unchanged；拒绝使用专用失败 schema。没有自动重放或补偿。快照输出为可选 `navigation.workspaceOrder` 升至 v5，列表及原生导航读取同一顺序。Windows 定向验收覆盖跨 Project 单项／块移动、无变化和拒绝不变量、两个 TUI Client 保留选择及存活 shell PID、canonical 键盘导航、创建／关闭的顺序维护，以及正常停止后恢复排序并运行新 shell。完整替换验收仍独立保留。
