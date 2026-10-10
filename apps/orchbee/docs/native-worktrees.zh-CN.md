# 原生 Worktree 启动

## 交互与范围

Windows 的 `Ctrl+B W` 根据分支和基线创建 checkout（空基线表示 `HEAD`）。
`Ctrl+B O` 从仓库列表选择准确 checkout 打开。F1 帮助以及
`[keys].new_worktree` / `open_worktree` 使用现有快捷键注册表。
目录遵循 [CLI Worktree 策略](windows-worktrees.zh-CN.md)；已有分支使用该分支，
不会使用输入的基线。关闭整个 Workspace 组沿用
[现有交互](native-workspace-close.zh-CN.md)。此界面不包含 checkout 删除。

表单捕获键盘、粘贴、鼠标和包含决策的完整输入包。Tab 切换字段，Enter 确认，
Esc 在提交前取消。分支和基线各限制为 1024 个 UTF-8 字节，拒绝控制字符和开头的
`-`，完整引用语法交给 Git 校验。列表使用稳定的绝对路径，显示主 checkout 和
linked checkout，滚动保持选中行可见。重名分支或 Workspace 标签不能决定身份。
界面放不下时关闭，并显示本地化提示。

提交后 Esc 隐藏观察界面，不取消 Server 工作。W 或 O 重新显示保留的操作；
`r` 只读取原操作。失败后仍可查看操作编号以及已知 Project、checkout 和
Workspace 结果。结果未知的 Start 或 Project 创建不会重放；不会隐式 Resume、
删除、放宽信任或回滚。终态结果可关闭，再显式开始新操作。Server 重启遵循
[现有保留期限](windows-worktrees.zh-CN.md#保留续执行)。

## 模型与所有权

Client 需要创建或打开 Worktree 并进入首个 shell；不必运行 CLI 命令，
也不能因 Git 忙碌而丢失当前任务。

| 记录字段 | 合约                                                                                                                                                                                                                          |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 输入     | 准确的来源 context/member、规范 snapshot、选定 Project ID、primary source directory 或准确 primary root cwd、有界 Git 列表、可编辑分支/基线、几何、配置、完整解码输入包和成功绘制 token                                       |
| 数据     | 一个打开 token、不可变确认计划、一个保留操作 ID、可选已知 Default Project 收据、规范 checkout/Workspace 结果；与本地焦点分离                                                                                                  |
| 输出     | 只读准备、可选 Default Project 创建、一次 Start、后续 Read、本地化拒绝/等待/失败/未知/打开状态，或有条件的导航                                                                                                                |
| 决策     | Workspace 所属 Project 优先；standalone 使用唯一 Project、多个时准确选择、零个时仅确认后创建 Default。Source 优先 Workspace primary directory；否则只使用来源 Panel primary root cwd。缺失或不可用采样不回退 home 或其他 pane |
| 不变式   | 未绘制、过期、不完整、变尺寸或来源变化的 draft 不能提交。确认冻结身份和意图。Start 只发一次，Read 不改 Git。已知收据在后续观察失败后保留。隐藏/导航阻止晚完成抢焦点；确认前取消无 Project 或 Git 效果                         |
| 模型     | 确定性交互转换和类型化效果，由有界 carrier 与现有 membership 选择细化                                                                                                                                                         |
| 依赖     | 单调时间；现有 settings owner；有界规范 snapshot/Worktree API；准确借用或临时 Viewer 的进程采样；现有 native membership、控制、几何和首帧 owner                                                                               |
| 所有权   | Client interaction 拥有 draft/observation 及须 join 的 lane。Server 拥有 retained operation/规范 Project/Workspace。Worker 独占 Git。现有 viewer 与 membership owner 独占终端 attach/detach/control                           |
| 参考范围 | 复用当前 Windows CLI 合约，不承诺完整 Herdr 兼容；没有新增 wire、持久格式、依赖、信任或自动重试策略                                                                                                                           |

Git 观察 lane 只有一个命令槽和一个结果槽，不在 IPC 中持锁；关闭先停止接收，
唤醒被阻塞的 I/O，再 join 线程。慢 Git 工作归 Server，不占普通终端 Viewer FIFO。
只有准确 root cwd 准备使用该 FIFO 和现有有界 deadline/cleanup。借用的 primary
Viewer 不会被 detach；临时 Viewer 在准备结束前释放。

配置目录的纯策略归应用，由 CLI 和 TUI 共用。未配置目录继续归 Worker。
TUI 在准备时读取当前已准入配置，不新增 configuration publication 字段，
也不把变化后的来源作为同一次确认的延续。

创建目的地校验和规范路径观察仍由 Server/Worker 负责；Client 不把配置操作数的
字面拼写与规范回执作比较。打开仍匹配列表中选定的准确规范路径。只有 Project
结果时显示其请求/已知项目，不显示尚未提交的 Worktree 状态命令；项目创建结果
未知时，须先检查项目，再决定是否显式创建另一项。

本地进入等待已在执行的 publication refresh，结束后重新核对准确来源。
membership 缺失时，只允许在固定进入 deadline 内补一次只读刷新。
无法完整绘制的界面隐藏时取消进入，保留规范回执。目录拼写使用与 Server
关联一致的纯比较规则，不另造文件系统身份，也不取消 Worker/路径守卫。

进入从规范回执 snapshot 冻结目标的首个 Tab、Panel、primary presentation、
实际终端 binding 和 primary directory ID。每次已接受的来源刷新仍须证明这个
准确目标；context-selection snapshot 在清理旧 Viewer 或安装选择前再次核对，
由现有 membership owner 选择该准确成员，合法 Tab 重排不改变目标。
守卫只属于本次成功准入的 context refresh generation；其他导航请求保留
自己的语义。另一 Client 删除或替换目标时拒绝
进入，保留当前 context 和已知操作结果。
准确 primary presentation 在 cleanup 和 attach 中也优先于记忆的子终端。
在当前 context 内，由现有 presentation switch owner 先选择该 primary，
再检查首帧成功。

## 转换与失败

| 状态 / 事件                | 条件与结果                                                                   |
| -------------------------- | ---------------------------------------------------------------------------- |
| 隐藏 → 准备                | 当前语义来源，没有其他 modal/mutation；读取规范来源和列表                    |
| 准备完成 → 表单/列表       | 准确来源、Site 与非过期 revision；Project 策略和稳定 checkout 路径           |
| 编辑 / 选择                | 更换绘制 token；完整解码并重新成功绘制后才可确认                             |
| 确认前取消                 | 仅丢弃 draft，没有 Project/Git 效果                                          |
| 确认 → 可选 Project        | 冻结计划、来源和操作 ID；仅必要时创建一次 Default                            |
| 已知 Project → Start       | 先保留收据；新 snapshot 和不变语义来源才允许继续                             |
| Start → 观察               | 只进行一次 handoff，独立保留操作身份；不确定时只允许 Read                    |
| 等待 → Read                | 每个观察步骤一个有界请求，普通终端输出继续                                   |
| 已打开 → 导航              | 首个终端 Started 或复用已有 Workspace，准确新 membership，界面可见且来源未变 |
| 导航 → Ready               | 现有 cleanup/attach/control/geometry/首帧流程，只改变发起 Client             |
| 隐藏 / 来源变化 / 读取丢失 | 保留收据，抑制焦点并提供只读恢复                                             |
| 失败 / 未知终态            | 保留 checkout/Workspace，不重新创建或补偿                                    |

规范提交和进入窗口是独立结果。提交后的 snapshot、attach 或绘制失败不会撤销 Git、
删除 Workspace 或重新发送 Start。Server 保留期间也可以使用
`orchbee worktree status <id> --format json` 查看该操作编号。

## 正确性与验证

所属实现测试覆盖来源/ref 校验、确认和绘制接纳、不明交接、保留 Project 回执、Start/Read 分离、安装前目标替换及有界 viewer 释放。

在 `src/terminal_session/state/worktree_launch/tests.rs`、`src/terminal_session/connection/native/worktree_launch/tests.rs`、`src/terminal_session/ui/worktree_launch_tests.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。
