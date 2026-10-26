# 水平组拖动契约

## 适用范围

该合约拥有一种行为：在一个选项卡条内拖动水平选项卡组标题。它涵盖激活、临时支持模型更改、整个组重新排序、选择、取消、拖动视觉、落定以及相邻组之间的边界。

行为基线是提交 `e4f73f6` 时的只读上游实现以及迁移到此包中的测试和正式工件。该合约记录了包保留的行为；它没有对以后的浏览器版本做出一般性的平价声明。

以下行为不属于该模型：

- 单个或多选项卡拖动；
- 固定选项卡拖动；
- 垂直条；
- 跨窗口分离和附加；
- 触摸长按和上下文菜单行为；和
- 精确的颜色、排版和非拖动悬停样式。

## 上游参考事实

以下事实来自 Chromium 源和用作迁移基线的测试：

| 事实                                                                       | 上游来源证据                                                                                      |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| 组标题拖动会记录组内活动选项卡的索引，或者当活动选项卡位于组外时索引为零。 | `dragging/tab_drag_controller.cc`，`GroupHeaderDragData` 的构造                                   |
| Attach 选择每个拖动的选项卡并使记录的组选项卡保持活动状态。                | `dragging/tab_drag_controller.cc`、`AttachImpl` 和 `ResetSelection`                               |
| 对于 `kCollapseTabGroupDuringDrag`，最初扩展的组在支持模型中暂时折叠。     | `common/tab_drag_handler.cc`, `StartedDragging`                                                   |
| 拖拽前已折叠的组保持折叠。                                                 | `dragging/tab_drag_controller_interactive_uitest.cc`，`DragCollapsedGroupHeaderRemainsCollapsed`  |
| 头部（header）拖拽通过 `MoveGroupTo` 将组作为一个整块移动。                | `dragging/dragging_tabs_session.cc`，附加移动路径                                                 |
| 一个组不能停在另一个组内部；在拖拽块越过相邻组的远边界之前保持不动。       | `DragGroupHeaderRightAvoidsOtherGroups` 和 `DragGroupHeaderLeftAvoidsOtherGroups`                 |
| 停止组标题拖动会恢复仅因拖动而折叠的展开组。                               | `common/tab_drag_handler.cc`, `StoppedDragging`                                                   |
| 停止组标题拖动会将选择缩减为组中记录的活动选项卡。                         | `common/tab_drag_handler.cc`，`StoppedDragging`； `dragging/tab_drag_controller.cc`，最终选择复位 |

迁移后的 Presentation 还保留这些已记录观察：

- 最初展开的组仍然是包含其标题和选项卡的完整复合拖动视觉效果；
- 未拖动的选项卡和组移出拖动的视觉对象的目标空间，而不是绘制在其下方；
- 无法提交组的位置不会出现插入指示符；和
- release 将完整拖动的视觉对象放置到最终的组边界中，而没有重复的标题、下划线或活动轮廓。

## 状态

语义状态包含：

| 领域             | 含义                                                           |
| ---------------- | -------------------------------------------------------------- |
| `tabs`           | 带有固定和组成员身份事实的有序选项卡身份                       |
| `groups`         | 已知组及其用户可见的折叠意图                                   |
| `activeTabId`    | 只有一个活动选项卡，或者仅当条带为空时没有选项卡               |
| `selectedTabIds` | 选定的身份                                                     |
| `drag`           | `idle`、`armed`、`dragging` 或 `settling` 加上该阶段所需的数据 |

可见的活动表面是派生（derived）出来的。活动的未分组选项卡或扩展组的成员将投影到该选项卡。折叠组的活动成员会投影到其组标题，而该成员的 `activeTabId` 仍然具有权威性。标题使用与普通活动选项卡相同的活动选项卡外壳；有色组色块（chip）包含在该外壳内。

armed（已预备）或活动的组拖拽记录：

- 组身份和有序的成员身份；
- 组中活动的选项卡，或当活动选项卡位于组外时的第一个成员；
- 取消所需的完整预拖动权威状态；
- 拖拽前该组是否已折叠；
- 支撑模型是否仅因拖动而折叠；
- 拖动的视觉对象是 `expanded-group` 还是 `collapsed-header`；
- 最新有效的移除前目标索引；和
- 逻辑上的落定目的地。

指针坐标、DOM 节点、计时器和动画对象不是语义状态。适配器将它们转换为目标观察、逻辑时钟事件和渲染命令。

## 事件

| 事件                         | 所需数据             | 含义                                                                       |
| ---------------------------- | -------------------- | -------------------------------------------------------------------------- |
| `ArmGroup`                   | 组身份               | 记录可能的标题拖动而不更改权威状态                                         |
| `ActivateDrag`               | 无                   | 越过激活阈值并进入拖动循环                                                 |
| `MoveTo`                     | 有效的移除前目标索引 | 预览并应用整个组重新排序                                                   |
| `Commit`                     | 无                   | 保持最终顺序并恢复拖动后折叠和选择语义                                     |
| `Cancel`                     | 无                   | 恢复完整拖拽前的权威状态                                                   |
| `Settle`                     | 无                   | 在最终逻辑帧后结束释放动画                                                 |
| `ToggleGroup`                | 组身份               | 折叠或展开组而不更改活跃成员的身份；折叠将其可见表面从子选项卡更改为组标题 |
| `InvalidateSourceMembership` | 捕获的成员           | 表示 arm 之后、activation 之前一次有界的 owner-state 变化                  |
| `RejectStaleActivation`      | 无                   | 当捕获成员不再组成源组时退役 armed gesture，且不产生权威命令               |
| `RejectMissingGroup`         | 组 identity          | 接收并拒绝未知组，不改变权威状态，也不发出命令                             |
| `RejectOutOfPhaseEvent`      | Event                | 接收并拒绝错相（out-of-phase）的 event，不改变状态，也不发出命令           |
| `RejectRepeatedEvent`        | Event                | 把重复 event 作为精确 no-op 接收                                           |

重复或错相（out-of-phase）的事件是明确的无操作。引用缺失组、缺失成员或无效目标的事件不得部分改变状态。

## 不变量

这些名称是稳定的证据标识符：

| 声明                                         | 必需事实                                                                        |
| -------------------------------------------- | ------------------------------------------------------------------------------- |
| `UniqueTabIdentity`                          | 没有两个选项卡具有相同的标识                                                    |
| `KnownGroupMembership`                       | 每个非空选项卡组引用一个已知组                                                  |
| `PinnedPrefix`                               | 固定选项卡形成一个连续的前缀并且不分组                                          |
| `GroupContiguous`                            | 每个组的成员占据一个连续的范围                                                  |
| `ActiveTabExists`                            | `tabs` 中存在非空活动标识                                                       |
| `StableActiveSurfaceExists`                  | 在临时 `dragging` 之外，活动标识具有可见选项卡或折叠的组标题表面                |
| `CollapsedActiveUsesGroupHeader`             | 如果折叠组包含活动选项卡，则其标题是活动表面，而子标识仍然具有权威性            |
| `SelectionExists`                            | 每个选定的身份都存在于 `tabs` 中                                                |
| `DragSourceExists`                           | 每个 armed（已预备）、正在拖拽或正在落定（settling）的组和成员仍然存在          |
| `GroupMovePermutation`                       | 移动组可保留完整有序的选项卡标识集直至排列                                      |
| `GroupMoveAtomic`                            | 组头移动保留成员身份并将整个组作为一个块移动                                    |
| `GroupTargetBoundary`                        | 已提交的组边界永远不会分裂另一个组                                              |
| `CollapsedDragStaysCompact`                  | 在激活之前折叠的组仍保持折叠状态并使用一个紧凑的标题视觉效果                    |
| `ExpandedDragKeepsFullVisual`                | 激活前展开的组使用完整的标题加选项卡视觉效果，而其支持模型暂时折叠              |
| `CompletionRestoresCollapsedIntent`          | 提交恢复精确的拖拽前折叠意图                                                    |
| `CompletionSelectsRecordedActive`            | 提交仅使记录的组成员处于选中状态并处于活动状态；折叠的结果通过标题将其投影      |
| `CancelRestoresAuthoritativeState`           | 取消恢复顺序、成员关系、折叠、活动、选择和 selection anchor                     |
| `NoInvalidDropFeedback`                      | 可观察的投影没有暴露 `MoveTo` 会拒绝的掉落位置                                  |
| `SettledVisualsDoNotOverlap`                 | 落定时，不同的收集槽没有未声明的占用边界重叠                                    |
| `RejectedInputPreservesAuthorityAndCommands` | 缺失、过时、重复、错相（out-of-phase）和无效目标输入都不产生权威命令或 mutation |
| `StaleActivationRetiresWithoutMutation`      | 源成员关系不匹配时退役 armed gesture，并保留较新的权威状态                      |

`NoInvalidDropFeedback` 与 `SettledVisualsDoNotOverlap` 需要确定性几何轨迹。其余声明属于语义转换，无需浏览器即可检查。

## 转换契约

| 阶段                  | 事件           | 守卫                       | 下一阶段   | 语义结果                                               |
| --------------------- | -------------- | -------------------------- | ---------- | ------------------------------------------------------ |
| `idle`                | `ArmGroup`     | 组已存在且有成员           | `armed`    | 捕获拖动前的状态并记录活动成员                         |
| `armed`               | `ActivateDrag` | 被捕获的成员仍然组成该组   | `dragging` | 选择拖动的成员，激活记录的成员，并可选择仅折叠支持模型 |
| `dragging`            | `MoveTo`       | 目标是拖动块之外的集合边界 | `dragging` | 以原子方式移动整个组块                                 |
| `dragging`            | `Commit`       | 结构不变量成立             | `settling` | 恢复折叠的意图并保留记录的活跃成员                     |
| `armed` 或 `dragging` | `Cancel`       | 总是                       | `idle`     | 恢复捕获的权威状态                                     |
| `settling`            | `Settle`       | 逻辑落定持续时间已完成     | `idle`     | 删除拖动的投影而不改变权威状态                         |
| `idle`                | `ToggleGroup`  | 组已存在                   | `idle`     | 切换折叠，同时保留活动身份并仅重新映射其可见表面       |

`ActivateDrag` 可能会暂时更改支持组的折叠值，以从布局中删除原始集合。这是一种投射机制，而不是用户折叠意图的改变，因此这两个事实都保持明确的状态。

模型不对 activation、release、cancellation 或 settling 作公平性假设。指针可以无限期停留在阈值以下或保持拖动；只有收到相应的环境 event 后，transition 才会发生。

## 可观察的检查点

| 检查站                | 需要观察                                                               |
| --------------------- | ---------------------------------------------------------------------- |
| `armed`               | 没有权威的顺序或折叠变化；没有拖拽覆盖层                               |
| `activated-expanded`  | 存在完整的组覆盖层；背衬槽被移除或压实；相邻时隙具有有效的非重叠目的地 |
| `activated-collapsed` | 存在一个紧凑的头部覆盖层；组保持折叠；记录的成员处于活动状态           |
| `invalid-target`      | 顺序与最后有效目标保持不变；无效点处没有出现提交指示符                 |
| `valid-target`        | 整组顺序和移位槽目的地符合相同的集合边界                               |
| `cancelled`           | 恢复拖前权威快照和普通渲染                                             |
| `committed`           | 最终顺序有效；恢复折叠意图；记录的成员保持活动                         |
| `settled`             | 不存在拖拽覆盖层，并且占用的槽边界不包含未声明的重叠                   |

## 证据图

| 声明族                 | Lean               | 属性测试 | 确定性系统跟踪     | 上游证据              |
| ---------------------- | ------------------ | -------- | ------------------ | --------------------- |
| 结构身份和成员资格     | `moveGroup` 保存   | 必填     | 检查站             | 源模型行为            |
| 整体运动和目标边界     | 排列和连续性       | 必填     | 左、右痕迹         | 交互式 UI 测试        |
| 折叠和选择生命周期     | 不适用：状态策略   | 必填     | 展开和折叠的痕迹   | 拖动处理程序/控制器源 |
| 取消                   | 可选的恢复引理     | 必填     | 转义和指针取消痕迹 | 控制器恢复路径        |
| 视觉拖拽和位移         | 不适用：几何适配器 | 几何属性 | 必填               | 源布局加上观察记录    |
| 落定（settle）且不重叠 | 不适用：几何适配器 | 几何属性 | 必填               | 观察记录              |

语义状态拥有 `selectionAnchorId`；确定性取消测试恢复准确捕获的权威状态。DOM 焦点仍是适配器状态。浏览器套件检查获得焦点的折叠组头投影，以及 Escape 后语义与绘制状态的恢复。

## 实施桥梁

`src/tab-strip/dragging/horizontal-group-drag-machine.ts` 拥有组拖拽阶段、选择策略、支持模型的临时折叠、整组移动、取消恢复和视觉类别。 React 集成可以测量几何形状并执行命令，但不得独立选择这些语义。

`src/tab-strip/dragging/horizontal-group-drag-model-adapter.ts` 将转换命令应用于 `TabModel` 并断言其规范快照在每个事件后细化纯状态。活动标识和可见呈现仍然是独立的事实：折叠活动组不会将激活转移到该组之外。

每个反例都被序列化为初始规范状态加上语义事件。 DOM 坐标可以作为系统测试输入附加，但回归标识是违反命名声明的最短语义序列。

## 完成标准

此行为切片仅在以下情况下保持完整：

- 纯转换是范围语义决策的唯一所有者；
- Lean 在不引入未证明义务的情况下检查组移动保持性；
- 固定种子属性测试涵盖方向、相邻组、折叠和展开源、提交、取消、重复事件和无效目标；
- 确定性浏览器跟踪覆盖每个可观察的检查点；
- TypeScript、Lean 和跟踪 Schema 之间的映射始终有文档记录；并且
- 包的单元测试、构建、导出契约与仓库验证继续通过。

<!-- documentation-pairing: english-sha256=9d37d0a030529ffacb78bf88583aa12cc4777d6ada48eb1db1c3633af4e63e37 zh-cn-sha256=9643566c3e359ca0993a27dd570b4ea7527c4f75f33df06bc0cb847944fa2975 -->
