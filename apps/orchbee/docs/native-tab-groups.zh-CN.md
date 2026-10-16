# 原生 Tab 分组与能力覆盖

Windows 原生 TUI 投影 canonical 分组身份、名称、颜色、成员及固定状态。紧凑标签栏绘制独立组标题、
展开标记和成员数量。Tab 菜单按 GUI 的创建／分组、复制／固定和关闭动作分段；
选择与移动位于子菜单。组标题和新建 Tab 按钮各自拥有独立菜单。
键盘菜单导航和已绘制指针目标调用相同 owner。
配置后台 Tab 时，保持当前 Tab、Panel、Presentation、终端绑定及 viewer 选择。

## 实际 Host 能力比较

此表比较 Workbench 组合真正接入的路径，不把可复用 Tab Strip 的全部导出能力视为产品能力。
GUI owner 为 `WorkbenchTabBar`、`useWorkbench`、`getWorkbenchTabCreationAvailability`、
`WorkbenchProjectWorkspaceRuntime` 和 `TabModel`。原生列指 Windows TUI。这是一部分能力覆盖，
不代表 GUI／TUI 已完全对等。

| 核心操作                         | GUI 实际接入路径                                                                                 | 原生覆盖及剩余缺口                                                                                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 功能菜单                         | Tab、分组、Panel 及 Workbench Action 菜单                                                        | Tab、分组、新建 Tab 各有独立菜单；分组选择和选择／移动使用带分隔线及键盘返回的子菜单。                                                                    |
| 创建、重命名、关闭单个 Tab       | 终端 owner 创建／关闭；标题 action 修改 canonical 名称                                           | 既有 canonical 原生操作；分组／固定元数据仍可完整表达。                                                                                                   |
| 多选                             | Client 本地所选 ID 与锚点；切换、区间及全选                                                      | 选择子菜单提供加入／移出、区间、全选及清空；Ctrl 点击标记已绘制 Tab。Shift 保留给宿主。原生标记保持当前终端活动。                                         |
| 固定／取消固定                   | 选中 Tab 集合经 `TabModel.setTabsPinned`                                                         | 精确选中集合，遵循规范顺序；在固定前缀边界移动，固定时移除组归属。                                                                                        |
| 用已有 Tab 建组                  | 选中集合经 `createGroup`                                                                         | 精确选中集合、Server 分配组身份、名称编辑器及颜色菜单。                                                                                                   |
| 加入／移出组                     | 选中集合经 `moveTabsToGroup`                                                                     | 精确选中集合；加入时取消固定，清理源空组，保持存活分组连续。                                                                                              |
| 重命名／改色／解散组             | 分组菜单经 `updateGroup`／`ungroup`                                                              | 支持完整 canonical 组及全部九种颜色。解散保留终端。                                                                                                       |
| 移动组                           | `shiftGroupRelative`、已接受的拖动 arrangement                                                   | 跨过前／后相邻组或未分组 Tab；不越过固定边界或循环。指针拖拽将完整分组作为一个集合移动。                                                                  |
| 折叠／展开                       | Client view 的 `collapsedGroupIds` 投影入 `TabModel`                                             | 标题点击或菜单；按身份保存于 Client 本地。原生展开状态不持久化，一次只保留一个 context。                                                                  |
| 组内新建 Tab／新建带 Tab 的组    | 本地创建路由可用；远端拒绝组内定位，并禁用新建组                                                 | 通过一次原子 `tab.create` 放置实现；校验创建后激活新 Tab。GUI 使用两次模型提交。                                                                          |
| 在右侧／下方新建 Tab             | 本地 owner 保留目标组及 index + 1，拒绝不兼容的固定边界放置                                      | **在右侧新建标签页** 直接针对精确菜单 Tab，不先导航。原子放置保留目标组及 index + 1；固定前缀内部插入在运行时分配前拒绝。组菜单仍使用组尾位置。           |
| 复制                             | 菜单目标已选中时扩展到所选集合；本地创建允许单源和多源，远端均禁用                               | 选中目标扩展为初始规范顺序的所选 ID；未选中目标仍为精确单个。每个源复用一次 Create 生命周期，有界保留部分／未知结果及原选择。不克隆运行中的拆分树或 PTY。 |
| 关闭组／选中集合／其他或后续 Tab | `onCloseTabs` 将精确 ID 集合交给 `requestActiveTerminalTabsClose`；终端 owner 提供运行中资源确认 | 已通过既有 Close 生命周期实现精确目标规则与一次完整集合确认；逐项结算并保留恢复。详见[关闭集合](native-tab-close-sets.zh-CN.md)。                         |
| 单个 Tab 跨组边界移动            | GUI model 同时更新 arrangement 与分组                                                            | Tab 菜单向左／右／开头／末尾移动时使用 GUI 分组语义，通过本地视图提示跨过折叠的相邻组。既有方向／索引快捷键保留原排序语义。                               |
| 移到其他窗口                     | 库回调／runtime factory 可选；当前应用 Host 未安装新窗口回调                                     | 不将其算作当前 Host 已接入的对等能力。未新增原生窗口迁移操作。                                                                                            |

方向、悬停卡和垂直栏偏好属于 Host 表现选择。原生菜单现已覆盖上述已接入的创建、复制、分组和关闭操作。
Close 有意只删除关闭后变空的源组，保留无关空组；GUI `TabModel.closeTabs` 则删除所有空组。
[Close owner 边界](native-tab-close-sets.zh-CN.md#分组清理边界) 记录此有意差异。Host 表现差异仍是明确限制。

## 共享权威与回执

`tabs.configure` 对已有 `terminalContextId` 执行一种封闭操作：`create-group`、`set-group`、
`update-group`、`ungroup`、`set-pinned`、`move-group`、`create-group-for-tabs`、`set-tabs-group`、
`set-tabs-pinned`、`move-tab` 或 `move-tabs-by-drag`。集合操作携带 1–256 个唯一 Tab ID；任一成员缺失即拒绝整个命令。
输入顺序不替代规范顺序。既有认证 command／revision CAS 及
请求账本负责接入、重放及候选的原子发布。Server 仅在创建时分配组身份，并在 `tabs-configured`
中返回；其他成功变更的必填 `groupId` 为 null。无变化返回原版本的 `unchanged`。过期版本、
不存在目标、数量上界及 context 内任何未完成 Close 均在写入前拒绝。该操作没有终端或 Worker 效果。

原生适配器复用 setup 事务的队列、截止时间、已知／未知结果处理和快照恢复。它用 `Arc` 保留有界
canonical 输入快照，而非从渲染后的 Tab 重建可写文档。精确提交版本的回执必须等于预期完整快照，
包括其他 context、导航、树、名称及绑定。新组 ID 必须不存在于基线的任何 context。后续版本遵循
既有 canonical 刷新规则，因为 peer 可能已提交另一合法修改。投递结果未知会退役连接，绝不自动重试变更。

`configure_tabs` 是 authority 规划及回执检查共用的 Rust 纯转换。它保留每个已有
Tab／Panel／Split／Slot／Presentation 身份和终端绑定。组清理遵循 GUI `removeEmptyGroups`；
从仍存活的源组中间成员建新组时，将新组插在源组块之后，避免拆散源组。
`TabModel.moveTabsToGroup` 执行相同边界规则。这是 canonical 元数据，不是 TUI 独有的第二套分组模型或持久化路径。

选择标记按身份保存在 Client 本地。当前标签页始终在集合中；普通标签页导航清除额外标记。
菜单目标属于选中集合时配置整个集合；未选中的后台目标只配置自身。有类型的单目标／所选集合行文案
标明固定、建组、移出及复制的受影响数量。重命名、右侧创建和单个移动仍只针对一个标签页。
菜单绘制证明捕获选中的规范成员，因此选择或成员变化会使该选择失效。
多选显示 `*` 标记，组标题显示所选／总数，包括折叠标题。原生标记刻意保留活动终端；
GUI 选择则可以同时激活被标记的标签页。

中文菜单、路径与拖动文案将每个 Tab 称为**标签页**，组称为**分组**；英文保留 **Tab**。
本地化文案区分所选成员与完整分组，不改变捕获的 ID。

折叠状态仍归 Client 本地，遵循 `packages/workbench/src/workbench/document/workbench-client-view.ts`
的权威边界。组标题展开操作不选中成员、不提交 command、不推进共享版本，也不从导航器隐藏成员。
组颜色使用用户选择的成对前景／背景色块；单色模式保留名称、数量、展开及当前标记。名称经过既有
终端文字清理及单元宽度裁剪。指针区域与标题、成员 token 从同一次绘制产生。

## 指针排列与菜单导航

新建标签页按钮的右键菜单包含新建标签页、在最近分组内新建及新建分组并创建标签页；
全局菜单提供同一子菜单供键盘用户使用。最近分组限定于当前 context：依次使用活动组、
最近保留的组、最后一个现存组。标签页根菜单在分组归属旁直接提供分组操作；选择／移动不包含
分组管理。直接右键组标题仍打开同一个准确分组的菜单。该菜单包含组内创建、重命名、九色子菜单、
展开状态、相邻移动、解散和关闭。解散明确保留标签页；关闭针对该规范组的所有成员。
标签页的分组选择包含新建组和其他现有组。
标签页子菜单保留键盘选择与移动能力；Shift 手势属于宿主。
不支持的原生宿主隐藏移至新窗口。

[菜单投影](native-action-menus.zh-CN.md#标签页范围与当前值)将分组已保存颜色的勾选、各调色板色块及
临时菜单导航分离。打开时选中已保存颜色而不写入；导航和取消保留该颜色。显式激活使用既有
`update-group` 操作，重新打开读取已提交值。跨分组选择标记不改变被重新着色的完整分组。

`tui_pointer::tabs::TabDrag` 仅按最后一次成功绘制的精确 Session／revision、context、
有序元数据和 token 矩形准入无修饰左键按下。横向移动两个单元格或改变行位置后，
显示静态拾取高亮和插入标记，终端输出继续。移动仅影响本地预览。
有效左键释放只发送一次 `move-tabs-by-drag`，携带有界所选 ID 集合、原顺序插入边界
和可空目的组。固定与非固定成员不混合；完整选中组保留身份，部分成员采用目的组。
未移动的分组保持连续。展开成员是组内目标；标题选择集合前／后边界，包括折叠组。
终端单元格几何不使用悬停展开或动画时序。

Escape、外部释放、带修饰键的指针输入、失去焦点、释放捕获、竞争覆盖层、调整尺寸、来源变化或已绘制矩形变化
均取消且不写入。已取消的手势继续吞掉剩余释放。普通点击在释放时激活或切换展开。
与原生标记一致，拖拽保留当前终端；GUI 可能在按下时激活被拖成员。
展开标题显示起始方括号，最后成员显示结束括号；折叠标题保留名称、颜色、展开标记、
已选／成员数量及当前成员标记。

纯配置语料将十个拖拽结果与 GUI `TabModel.moveTabsByDrag` 比较。
Rust 选择集合／边界矩阵检查固定前缀、连续分组和终端数据保留。
`tui_pointer::tabs::tests` 覆盖仅释放写入及取消轨迹；
`ui::tab_groups::tests::interaction` 覆盖精确来源准入、canonical 回执安装、
独立菜单作用域和子菜单返回后的重新绘制。既有 setup 事务拥有过期／未知结果；
不增加拖拽专属重试、账本、持久化或分布式协议。

## 终端创建与复制

组菜单的 **组内新建 Tab**、新建 Tab 菜单的 **新建分组并创建 Tab** 及 Tab 菜单的 **复制 Tab**
使用 `WorkbenchAction::TabCreate` 和既有原生 `CreateOperation`。可选 typed `placement` 为
`in-group { groupId }`、`new-group`、`after-tab { tabId }` 或 `duplicate { tabId }`。省略时仍在末尾追加未分组／未固定 Tab。
放置和所有新成员在同一 revision 提交，不存在先创建再配置导致分组不连续的窗口。

组内创建插入在该组最后成员之后；空组则在末尾。这与 GUI 组菜单相符，但不同于其独立的
**在右侧／下方新建 Tab** 操作。后者经 `executeWorkbenchTerminalCreate` 将目标组及 index + 1
传给 `WorkbenchProjectWorkspaceRuntime.createTerminalInTarget`；`canInsertTerminalTab`
校验精确请求位置，不兼容时拒绝而非静默追加。原生 Tab 菜单通过 `after-tab` 提供
**在右侧新建标签页**：authority 在 revision CAS 下解析目标组与精确后继位置。新 Tab 未固定，
因此在固定前缀内部插入会在身份／运行时分配前拒绝；最后一个固定 Tab 之后可以插入。
GUI action 在执行时拒绝之前仍可能显示可用。全局新建 Tab 仍在末尾追加。
新组创建追加一个未固定 Tab 及全新无名灰色组，
并清理空组，与 `createNewTabGroup` 一致；组数量上限在清理后检查。`WorkbenchTabGroupLabel`
接受 0–512 个 Unicode 标量值，Tab／Presentation 标签仍必须非空。空标题和菜单目标使用本地化
**Unnamed group**／**未命名分组**，保留身份、颜色、数量和展开操作，不将占位文字写入共享状态。

复制将已选中的菜单 Tab 扩展为规范顺序的所选集合；未选中的后台 Tab 仍为精确单个目标。
实际接入的本地 GUI 范围相同：`getWorkbenchTabCreationAvailability` 允许多个源，
`useTabStripActions` 对这些 ID 逐个调用 `TabModel.duplicateTab`。源只捕获一次，绝不包含新副本。
单个复制保留原始 Tab 标签、原生自动／固定命名模式、固定状态及分组，在源后紧邻插入。GUI
`TabModel.duplicateTab` 以 `activate:false` 调用 `addTab`；实际接入的 `PanelWorkspaceStore`
协调创建单个新 Panel／Presentation，以复制的标题为 fallback，不设置 custom-title policy。
原生命名保持自身 canonical 自动／固定契约。两条路径均不克隆运行中的拆分树、子 Presentation、
PTY、启动 profile 或当前目录。既有终端创建 owner 分配全部新身份并启动一个新本地终端。

前台创建仅在安装已验证且仍包含新 Tab 的快照后清除额外选择标记，适用于分组与未分组创建；
随后选中新 Tab 并展开其组。提交请求、安装前收到回执，以及新 Tab 已被后续快照移除时均保留标记。
复制保留当前 Tab／Panel 身份以及本地分组展开／标记。
它复用创建屏障，因此所选 viewer 会清理后以新 generation 重新挂接，并非持续保留 attachment。
是否挂接依据所选终端的 provenance，而非复制终端的 outcome。更晚合法快照中源被删除时走既有后备选择。

Authority 在运行时效果前检查精确源／组、revision、Close 栅栏及结构容量。共享纯
`create_terminal_tab` 转换同时驱动 Server 原子规划和原生精确版本回执检查，比较完整 canonical
快照（包括其他 context）。新 Tab／Panel／Presentation 及可选组 ID 必须为全新身份。
后续快照遵循既有严格身份／绑定规则。畸形回执绝不安装乐观候选。

终端 pending、failed、unavailable、unknown 均保留已提交成员及原始 provenance。后台复制显示
本地化等待中／未启动／结果未知提示，不能仅凭成员提交就报告终端成功。不确定投递绝不自动重放。
已知提交但投影不可用时，通过显式 Snapshot 读取恢复，再走相同精确清理／安装路径。
所选集合复制分别保留各成员的精确操作、已知提交及终端 provenance，与短暂状态提示分离。
它报告已提交请求、元数据提交及终端启动的数量，不声明批量全有或全无，也不新增批量命令。

私有所选复制状态将整个序列限制为 120 秒。每个既有 50 秒 Create 预算受这个绝对期限约束，
保留一次五秒清理预留。每次 Start 前比较完整 Session Snapshot、context、标记及所选具体
PaneRef／Presentation／绑定；revision CAS 仍为准入权威。只有精确后继版本及纯单个复制转换，
且无关 context／分组保持不变，才能继续序列。

成员安装准备在更改缓存、desired 成员或释放旧 pane 记录前校验冻结旧栅栏。不可克隆的证明只属于
该关联操作及安装，在新 pane 分配后消费一次。旧栅栏跨 detach 准备保留。只有该证明才能采用
新 generation；原 Tab／Panel／Presentation 必须重新达到 Ready，且前一个 lease、回复和因果
poll 结算完成后，才能开始下一个 Start。空闲观察与入队共享邮箱准入锁。Busy 不消耗身份，
等待公平 UI tick，当前生命周期所需的 poll／回复继续运行。

导航、标记及焦点取消均锁存，包括离开后返回的变化。对端漂移、容量拒绝、pending／failed／
unavailable／unknown、投影恢复及超时均停止剩余源。当前操作仍遵循既有清理／恢复 owner；
迟到事实可以细化计数，绝不重启后续源。序列提示让位于后续错误，并在完成或停止五秒后消失；
事实保留至新的显式序列替换它们。

## 兼容性与验证

所属实现测试覆盖组成员关系、固定顺序、复制来源身份、顺序回执/安装/Ready 屏障、取消以及拒绝后不自动重放。

在 `src/terminal_session/state/tab_groups.rs`、`src/terminal_session/state/tab_duplication.rs`、`src/terminal_session/mailbox/tests/tab_duplication.rs`、`src/tui_pointer/tests/tab_groups.rs` 及相邻的状态、carrier、输入和渲染套件中维护实际证据。流程具有多种结果时，使用受控完成/时钟顺序及生成轨迹重放。

运行 `cargo test --locked -p orchbee --all-targets --all-features`。此流程不另设 TLA 模型。证据覆盖命名/生成的实现轨迹，不证明全部交错、物理终端外观或未测试平台。共享身份、所有权和提交协议仍由单独注册的核心模型负责。
