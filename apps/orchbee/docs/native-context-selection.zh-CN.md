# 原生上下文选择

## 范围与归属

原生 Client 在已连接的本机 Site 中选择独立 Terminal Context 或 Workspace。Server 拥有导航、
成员、revision 和终端生命周期；Client 拥有活动位置与选择。本参考文档维护
[原生 Tab Session](native-tab-session.zh-CN.md) 的上下文选择扩展，随 Session reducer、传输、
导航渲染及其正确性证据一起更新。

`Ctrl+B [` 和 `Ctrl+B ]` 按 Server 顺序选择上一个、下一个位置：独立上下文，然后是 canonical
全局 Workspace 显示顺序。显式顺序缺失或为空时仍按 Project 遍历，排序不改变 Project 归属。边界循环。标签仅用于展示；Project、Workspace 和 context ID 确立身份。
进入其他位置恢复本 Client 上次选择的 Tab 和 Panel，前提是这些身份仍属于该位置；
否则选择首个现存成员，没有 Tab 则进入 Empty。
进入和离开 Empty 都不会创建终端。已有 Tab 控制作用于当前上下文；显式 `c` 使用 Server
已有的目录准入，绝不回退到 home 目录。Windows 裸启动遵循下面的启动偏好；没有偏好时，
[目录启动](native-directory-startup.zh-CN.md)只在符合条件的空 Site 创建首个 Workspace，
其他 Site 保留 standalone Open。两者使用既有首帧就绪契约。

交互式 Project/Workspace 创建和首目录配置属于 [Workspace 设置](native-workspace-setup.zh-CN.md)；
目录启动拥有空 Site 自动创建。两者复用这里的上下文安装 owner。
Panel 布局、远程访问和生产恢复不在范围内。
该未发布 cohort 整体启用和回滚。改变 Client 位置不会修改 canonical 导航或销毁保留的执行。
[位置标签编辑](native-location-labels.zh-CN.md) 修改 canonical Project 或 Workspace 标签，
并保留这里的有类型位置身份。

## 状态与转换契约

输入为已验证的 Workbench snapshot、显式本地方向、当前 Session 状态、确切 attachment 身份
及既有单调操作预算。导航与当前上下文投影必须来自同一 snapshot revision。非活动上下文的
不支持布局不会使当前上下文无效。选择不支持的目标是确定的本地失败，保留原位置。

| 转换              | 守卫与结果                                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------- |
| 准入位置选择      | 稳定 Session 没有未解决的 mutation、Close 确认/收敛或成员操作时，保留确切目标身份及操作 generation；否则报告忙。          |
| 读取目标 snapshot | 改变 attachment 前重新验证目标的 canonical owner、Site 和 revision。确定不可用或不支持的目标保留原位置。                  |
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
每个访问过的 Tab 保存上次 Panel ID。标签、序号和终端 binding 都不是键。Site 由所属 Session
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
OS 前台保持不变。既有上下文模型每个非空上下文只有一个 Tab，把全部记忆选择抽象为这个唯一
成员。下面历史 TLC 结果不能证明多 Tab/Panel 记忆或修剪；上述易失历史增量采用确定性状态与进程证据，
暂缓执行 TLA。

## Windows 启动偏好

Windows 本地裸启动为当前 Windows 用户和本地 Site runtime 记住最后成功显示的位置、Tab、
Panel 和 presentation。这是 Client 拥有的导航提示，与 Server checkpoint 及终端生命周期分离。
有版本、有大小上限的 JSON 记录位于私有 `LOCALAPPDATA/Orchbee/tui-position/<runtime-key>`
缓存；键包含用户隔离的 runtime 目录和 runtime base。结构 ID 随 checkpoint 重启保留；
新 Server 的 Site ID 和 binding 始终取自当前已验证 snapshot。

渲染器只在工作台未被遮挡、可绘制时返回稳定位置和焦点 pane incarnation。Shell 成功绘制和
flush 后再验证并发布。遮挡、失败、过期或切换中画面不发布。Empty 位置不虚构成员。
应用私有 worker 把待保存位置合并为一份记录，在输入和绘制路径外进行私有加锁原子替换，
退出时 join。多个窗口保持各自实时焦点，仅在启动时读取偏好。最后一次原子发布决定下一次
启动偏好；位置未变的重绘不重复写入。存储失败显示本地化提示，不使存活终端退休。

恢复先验证当前 canonical 归属，再进入普通 bootstrap/attachment。成员已删除则选择该位置
首个现存成员；位置已删除则选择首个非空 canonical 位置，全部为空则进入 standalone Empty。
两种回退均提示。没有记录时，目录启动策略选择符合条件的空 Site 创建或既有 standalone Open。
无效或不支持的记录原样保留，本次禁用
偏好写入并提示。显式 context 或 Agent 启动仍使用确切目标。偏好不授予输入、不改变 Create
provenance、不启动或销毁终端、不改 checkpoint，也不移动其他存活 Client。

这份已接受的建模记录区分持久提示、Client 私有选择、待确认绘制证明和异步存储发布。
确切成员和 pane incarnation 守卫发布；重启先解析当前成员再进入已有 bootstrap owner。
失败保留终端状态。`NativeStartupPreference` 建模两个 Client、两个成员、删除、过期/遮挡/
失败绘制、合并保存和原子发布。文件权限及传输继续由私有缓存和原生 carrier 检查覆盖。
所需证据为编解码/解析测试、过期绘制及独立 Client 测试、真实 carrier 顺序、选定 TLC 覆盖，
以及 Windows ConPTY detach/重开和删除成员场景。不声称 Windows Terminal 手工验收，
也不声称完成仓库尚未执行的模型检查。

运行 `node scripts/tla/run-models.mjs --models native-startup-preference` 检查此有界模型。
完整图有 37,560 个不同状态、深度 17 和十项必需动作见证；不使用 state constraint 或
symmetry，也不声称任意文件系统环境都能推进。

## 正确性证据

`NativeContextSelection` 覆盖一个 Site、独立上下文及两个 Workspace、两个保留 binding、
一个空 Workspace、四个操作 epoch、两个导航 revision 和五种已知 Create 结果。38 项必需
动作见证包括双向经过 Empty、边界循环、忙时拒绝、先验证 snapshot 再清理、原子安装、
旧 epoch 和重新进入后的 provenance。TLC 在无 state constraint 或 symmetry 的条件下检查
188,409 个不同状态。Expiry 的弱公平性支持该抽象内最终进入 stable/retired。既有 Switch、
成员移除和 refresh 模型保留详细的传输、首帧、geometry 和 cleanup reserve 义务；新模型
不证明历史修剪或任意运行时的推进。

通过 `node scripts/tla/run-models.mjs --models native-context-selection` 运行已注册模型。
Snapshot 验证细化模型的 read 步骤，成员 owner 和 mailbox 事务细化 cleanup/install，
carrier trace 细化 snapshot/detach/attach 命令。本地位置选择没有 Create、Start、Dispose
或 canonical 导航写入命令。

确定性测试必须覆盖同名不同身份、导航 revision 变化、不支持的非活动成员、目标拒绝、
双向 Empty 转换、provenance 保留、mutation/Close 忙时准入和过期结果。Carrier 测试必须观察
确切 Detach 先于 Attach，且没有导航修改或隐式 Create。Linux 原生进程证据必须执行 Workspace
选择、保留 standalone 重入、已配置的空 Workspace 中的显式创建、其他 Client 的独立选择，
以及 Workspace Close-to-Empty 与确切 shell 退出。渲染测试覆盖双语、终端文本清洗和有界视口。
这些检查证明本地开发行为，不证明打包或跨平台就绪。
