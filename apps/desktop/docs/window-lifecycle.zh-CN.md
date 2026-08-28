# Desktop 窗口生命周期

## 问题与范围

Desktop 终端用户需要原生窗口，而不是一个包含所有共享标签页的应用窗口。因此 Electron
Main 拥有一个设备本地窗口布局，把未改变的 Workbench 共享文档投影到一个或多个原生窗口。
`Mod+N` 会创建一个原生窗口和一个使用默认当前目录的终端；标签页与组菜单以及跨窗口拖动可以
移动现有标签页，而不重启其终端。

Workbench 共享文档仍然是项目、工作区、终端上下文、标签页、组、面板和终端展示身份的权威。
浏览器和移动客户端继续观察所有共享标签页。Desktop 布局只拥有原生窗口身份、每窗口标签页
分配、激活新近度、普通边界和最大化状态。Quick Terminal、始终置顶、原生标题栏复刻、
每窗口工作区策略、由指针推导目标标签栏内插入位置，以及渲染器或 Terminal Host 对原生窗口
的权威仍不在范围内。

## 输入与规范数据

Electron Main 校验所有渲染器消息，以及 Electron 产品 `userData` 目录下可恢复的
`window-layout-v1.json` 文件。Workbench 协调器提供可信且已校验的共享文档快照。Electron
提供原生窗口事件、显示器工作区和屏幕坐标拖动点。

严格的持久记录为：

```ts
interface DesktopWindowLayoutV1 {
	readonly version: 1;
	readonly primaryWindowId: string;
	readonly nextWindowSequence: number;
	readonly activationOrder: readonly string[];
	readonly windows: readonly {
		readonly windowId: string;
		readonly ordinal: number;
		readonly assignments: readonly {
			readonly terminalContextId: string;
			readonly tabIds: readonly string[];
		}[];
		readonly bounds: {
			readonly x: number;
			readonly y: number;
			readonly width: number;
			readonly height: number;
		};
		readonly maximized: boolean;
	}[];
}
```

窗口 ID 与序号在记录生命周期内保持稳定，并且只由 Main 生成。`activationOrder` 以从最早到
最新的顺序恰好包含每个逻辑窗口一次。分配数组按共享文档的标签页顺序规范化；它们不会创建
第二套共享排序模型。即使 macOS 没有打开的原生窗口，有效布局也至少包含一个逻辑窗口。

坐标、尺寸、序列值、序号、集合大小与字符串都有边界。协调时移除未知终端上下文与标签页，
随后每个当前共享标签页恰好分配一次：未分配标签页进入最近激活的逻辑窗口；若无法解析近期
目标，则进入主窗口。重复分配、未知主窗口、不完整激活顺序、畸形输入、不可读文件或过时版本
都会使完整记录失效，并把所有当前标签页安全收拢到一个新主布局。恢复绝不改变共享文档或删除
终端。有效的旧版单窗口边界记录可以为主窗口矩形提供初始值，但不提供身份或分配状态。

## 输出与效果

确定性布局转换返回规范下一状态、语义结果与有序命令。结果区分 `completed`、`unchanged`、
`blocked` 与 `failed`；过时的标签页、组、窗口、渲染器代次或拖动会话身份绝不会回退到另一
目标。命令可以持久化候选记录、创建或关闭一个原生窗口、发布渲染器投影、聚焦窗口，或供应
一个共享终端标签页。

Main 串行化布局意图。用户请求的重新分配在成为已发布内存分配之前，会先完成准备与原子持久化。
持久化或原生窗口准备失败会保持来源分配不变。文档协调是故障受限的：即使可恢复首选项写入
失败，新的或先前未分配的共享标签页也会发布到最近激活的窗口，因此任何当前标签页都不会在
所有 Desktop 窗口中不可见。后续成功写入或进程重启会使记录收敛。

创建新窗口会先在共享文档中供应一个使用默认当前目录的标签页。这个已提交标签页最初遵循普通
未分配标签页协调。随后 Main 准备一个隐藏原生窗口和持久重新分配。若原生创建或分配持久化
失败，标签页会留在来源窗口或主窗口，并且不会删除任何终端。只有分配提交后，窗口才发布并显示。

移动单个标签页只改变其设备本地分配；其共享组成员关系、面板布局、终端展示身份与 PTY 保持
不变。移动组会解析当前共享组成员关系，并原子重新分配其所有标签页，包括其他窗口中的片段。
目标窗口按共享文档顺序投影已分配标签页。放下操作不会从 OS z-order 或指针几何推断插入索引。

## Action 与交互投影

Main 拥有不可自动化的 `desktop.window.create` Action。应用的 File 菜单及其 `Mod+N` 快捷键
调用相同的已准备 Action 路径。现有最大化与全屏 Action 以当前活动原生窗口为目标：

| Action ID                           | 效果                                       | 菜单快捷键                         |
| ----------------------------------- | ------------------------------------------ | ---------------------------------- |
| `desktop.window.create`             | 创建一个包含默认当前目录终端的原生窗口     | `CmdOrCtrl+N`                      |
| `desktop.window.toggle-maximized`   | 最大化普通窗口或恢复最大化窗口；全屏时禁用 | `CmdOrCtrl+Shift+M`                |
| `desktop.window.toggle-full-screen` | 请求进入或退出 Electron 原生全屏           | macOS `Ctrl+Cmd+F`；其他平台 `F11` |

标签栏以移动标签页和移动组子菜单扩展其现有已评审上下文菜单组合；子菜单包含每个符合条件的现有窗口
以及新窗口目标。渲染器只发送稳定身份；Main 会在提交前立即重新校验当前文档成员关系与布局分配。
选择当前窗口是有意无操作。成功移动后，空来源窗口会关闭。

跨窗口拖动在释放前只拥有可逆的来源预览。Main 使用当前原生边界和激活顺序对屏幕坐标点进行命中
测试。在另一窗口上释放会请求与菜单相同的重新分配操作；在所有符合条件窗口之外释放会请求准备
一个新窗口移动。Escape、丢失捕获、渲染器停用、过时来源、过时目标与准备失败都会恢复来源预览，
且不提交分配。

## 窗口生命周期与几何

关闭非最后一个原生窗口时，会阻止立即原生关闭，把它的所有标签页原子迁回最近激活的剩余窗口，
持久化不含来源的布局，然后关闭来源外壳。移动操作使一个或多个来源窗口变空时，也遵循相同的
提交后关闭路径。这些关闭绝不会进入应用全局静默流程或停止终端。

最后一个逻辑窗口的外壳能够关闭前，始终会进入现有退出与终端确认门。取消或不可用的决策会在
每个平台保留状态与外壳。在 macOS 上，如果操作系统让应用继续运行但不再有原生外壳，应用激活
会打开保留的主逻辑窗口。第二实例请求会聚焦最近激活的原生窗口。

只有 `BrowserWindow.getNormalBounds()` 会进入窗口记录。最大化记录可恢复普通边界并加上
`maximized: true`。全屏是瞬态的，进程启动时绝不恢复。每个恢复矩形都会投影到当前显示器工作区：
选择交集最大者；否则使用主工作区；缩小过大尺寸，并限制两个轴，使完整窗口可见。几何捕获与
分配转换共用串行化原子持久化队列。

## 状态模型与转换

`DesktopWindowLayout` 是唯一状态所有者。它包含一个已提交规范布局、至多一个已准备转换、
按逻辑窗口 ID 索引的零或多个原生窗口附件，以及每个附件单调递增的渲染器代次。

| 事件                                           | 守卫                       | 提交与命令                                            | 失败或无操作                           |
| ---------------------------------------------- | -------------------------- | ----------------------------------------------------- | -------------------------------------- |
| `Load(document, storedLayout)`                 | 应用组合启动               | 规范化，附加/打开主窗口，并在需要时持久化恢复         | 无效存储收拢到一个主窗口               |
| `ReconcileDocument(document)`                  | 规范文档修订变化           | 丢弃过时 ID，把缺失标签页分配到近期目标，发布后持久化 | 存储失败保留安全内存投影               |
| `FocusWindow(windowId)`                        | 窗口为当前且已附加         | 把 ID 移到激活顺序末尾并排队持久化                    | 过时 ID 不变                           |
| `RequestNewWindow(sourceId)`                   | 来源代次与活动上下文为当前 | 命令供应共享默认当前目录标签页                        | 无效来源被阻止                         |
| `TerminalProvisioned(tabId)`                   | 匹配请求仍为当前           | 准备隐藏窗口与重新分配                                | 过时完成不变                           |
| `NativeWindowPrepared(windowId)`               | 匹配准备仍为当前           | 持久化候选布局                                        | 准备失败保留已协调的来源分配           |
| `AssignmentPersisted(requestId)`               | 匹配候选仍为当前           | 发布布局、附加/显示目标、关闭新近变空的来源           | 过时完成不变                           |
| `MoveTabs(tabIds, destination)`                | 每个标签页与目标仍为当前   | 持久化一个原子重新分配候选                            | 相同目标不变；校验或持久化失败保留来源 |
| `MoveGroup(groupId, destination)`              | 组仍可在共享文档中解析     | 解析所有成员并使用 `MoveTabs`                         | 缺失或空组不变                         |
| `CompleteDrag(sessionId, point)`               | 来源代次与拖动会话仍为当前 | 解析现有/新目标并使用 `MoveTabs`                      | 取消或过时状态恢复预览                 |
| `CloseWindow(windowId)`                        | 来源不是最后一个逻辑窗口   | 持久化原子迁回/移除，发布后关闭外壳                   | 持久化失败保持窗口打开                 |
| `CloseLastWindow(windowId)`                    | 来源是最后一个逻辑窗口     | 进入平台应用生命周期路径                              | 取消退出保留状态与外壳                 |
| `CaptureGeometry(windowId, bounds, maximized)` | 已附加窗口为当前且非全屏   | 规范化并排队有序持久化                                | 无效或过时捕获不变                     |
| `RetireRenderer(windowId, generation)`         | 代次匹配                   | 只撤销该附件的 broker 与拖动会话                      | 其他窗口保持附加                       |

已准备转换 ID 使延迟、重复和乱序效果完成无害。渲染器只能通过现有多客户端协调器写共享文档；
布局所有者绝不会成为第二个 Workbench 文档写入者。渲染器投影按分配过滤完整规范运行时；活动
终端上下文没有标签页的窗口渲染空 Workbench 面板表面，而不是另一窗口的活动终端。

## 不变量

- 协调后，每个当前共享标签页恰好属于一个逻辑 Desktop 窗口。
- 每个分配引用当前终端上下文和标签页；主窗口与激活顺序恰好各引用每个逻辑窗口一次。
- 失败或过时的移动、关闭、拖动、持久化或原生准备不能移除来源分配或删除终端。
- 标签页与组移动绝不改变共享组成员关系、面板拓扑、展示身份、终端绑定、Host 会话或 PTY 身份。
- 关闭非最后窗口会迁回其完整分配，并且绝不进入应用全局静默流程；只有最后窗口路径可以请求应用退出。
- 渲染器停用只撤销匹配窗口代次的 broker、文档附件与拖动会话。
- 持久记录不含全屏状态；每个持久矩形都来自普通边界，并在恢复时完整位于一个当前显示工作区内。
- 浏览器与移动客户端继续接收不含 Desktop 分配字段的完整共享文档。

## 正确性与验证

确定性规范化器与 reducer 独立于生产环境。示例测试覆盖缺失、有效、重复、过时、损坏、不可读、
旧版边界、显示器移除和写入失败。带固定种子的有状态生成器会执行创建、聚焦、移动、整组移动、
协调、关闭、几何、取消、重复完成和乱序完成轨迹；它记录种子，并把失败收缩为可重放命令序列。

`DesktopWindowLayout` 在有界双窗口、三个标签页模型中检查独占分配、关闭前迁回、失败保持、
过时完成无操作，以及已准备操作推进到一个终态结果。Main/渲染器协议测试校验不可信消息、
代次停用与精确窗口路由。隐藏窗口 Electron 系统轨迹覆盖原生创建、菜单与快捷键等价、分配投影、
非最后窗口关闭、重启恢复、离屏修正和终端附件身份稳定。

会窃取焦点的原生窗口轨迹仍然只允许在仓库显式批准门下手动运行。自动化证据不声称 OS 焦点、
z-order、全局指针投递、Wayland 绝对定位，或尚未运行相同轨迹的 OS 家族原生行为。Electron 在
显示器枚举、普通边界、原生事件与屏幕坐标报告方面仍受信任。

| 声明                                 | 所有者 | TLA+                                                | 属性或核心测试                                                       | 集成或系统轨迹                                             | 限制                                   |
| ------------------------------------ | ------ | --------------------------------------------------- | -------------------------------------------------------------------- | ---------------------------------------------------------- | -------------------------------------- |
| `AssignmentsPartitionCurrentTabs`    | 本参考 | `AssignmentsPartitionCurrentTabs`                   | `preserves an exclusive complete assignment across generated traces` | `projects each Tab into exactly one hidden native window`  | 有界 TLA+ 身份；抽样固定种子轨迹       |
| `FailedPreparationPreservesSource`   | 本参考 | `FailedPreparationPreservesSource`                  | `keeps the source on every rejected or failed move`                  | `keeps a terminal attached when new-window creation fails` | 信任原子文件替换与 Electron 失败适配器 |
| `NonLastCloseRehomesBeforeEffect`    | 本参考 | `NonLastCloseRehomesBeforeEffect`                   | `rehomes every source Tab before emitting close`                     | `closes one hidden window without application quiescing`   | OS 外壳完成属于适配器证据              |
| `GroupMovePreservesSharedMembership` | 本参考 | 不适用：成员关系是布局模型不可变输入                | `moves all generated Group members without editing the document`     | `moves a fragmented Group through one protocol request`    | 组语义由 Workbench 拥有                |
| `RendererRetirementIsWindowScoped`   | 本参考 | `RendererRetirementIsWindowScoped`                  | `ignores stale generation completions`                               | `retires one renderer while another remains attached`      | 信任进程崩溃投递                       |
| `RestoredBoundsAreVisible`           | 本参考 | 现有 `DesktopLifecycle.WindowFullScreenIsTransient` | 现有显示投影测试                                                     | `restores each hidden window inside a work area`           | 仅当前显示拓扑                         |

## 参考范围、发布与恢复

参考发现固定到 Kitty 源码修订 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`。Orchbee 采用
原生新窗口命令（`new_os_window`）、当前工作目录变体，以及把标签页移动到现有或新 OS 窗口
（`detach_tab`）的语义可用性。Orchbee 不声称兼容 Kitty 配置、窗口放置、焦点、动画、拖动
命中测试或时序。二者不同时，以本所有者合约为准。

发布使用新的可恢复布局文件，不改变共享文档版本 1 或 Terminal Host 持久化。旧版 Desktop 会
忽略新文件并打开一个窗口。损坏、缺少分配或显式回滚会把所有当前标签页收拢到一个主 Desktop
窗口；移除本地布局文件只会丢失窗口放置与分配首选项。共享 Workbench 与 PTY 恢复数据保持可用，
布局恢复绝不会删除它们。
