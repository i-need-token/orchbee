# 键盘快捷键

`@orchbee/web` 拥有一组精选常见语义快捷键的客户端本地覆盖。Workbench 和投影终端功能继续拥有各自的 Action 输入与平台默认值。Web 组合层汇总这些声明，验证完整的有效绑定表，并把同一个来源注入已挂载的 Workbench 与投影终端。菜单、工具提示、Action 搜索界面和快捷键设置都从该有效绑定表投影标签；这些界面都不拥有另一份绑定副本。

## 初始目录

| 快捷方式 ID                                  | 适用范围               | 苹果默认              | 标准默认值            |
| -------------------------------------------- | ---------------------- | --------------------- | --------------------- |
| `workbench.search.actions.open`              | Workbench              | `Cmd+Shift+P`         | `Ctrl+Shift+P`        |
| `workbench.terminal.create`                  | Workbench              | `Cmd+T`               | `Ctrl+Shift+T`        |
| `workbench.terminal.font-size.increase`      | Workbench              | `Cmd+=`               | `Ctrl+=`              |
| `workbench.terminal.font-size.decrease`      | Workbench              | `Cmd+-`               | `Ctrl+-`              |
| `workbench.terminal.font-size.reset`         | Workbench              | `Cmd+0`               | `Ctrl+0`              |
| `workbench.tab.select.1` … `.9`              | Workbench              | `Cmd+1` … `Cmd+9`     | `Ctrl+1` … `Ctrl+9`   |
| `workbench.tab.select-previous`              | Workbench              | `Cmd+Shift+[`         | `Ctrl+PageUp`         |
| `workbench.tab.select-next`                  | Workbench              | `Cmd+Shift+]`         | `Ctrl+PageDown`       |
| `workbench.terminal.active.close`            | 活动终端               | `Cmd+W`               | `Ctrl+Shift+W`        |
| `workbench.panel.active.focus-down`          | 活动面板               | `Cmd+Option+Down`     | `Ctrl+Alt+Down`       |
| `workbench.panel.active.focus-left`          | 活动面板               | `Cmd+Option+Left`     | `Ctrl+Alt+Left`       |
| `workbench.panel.active.focus-right`         | 活动面板               | `Cmd+Option+Right`    | `Ctrl+Alt+Right`      |
| `workbench.panel.active.focus-up`            | 活动面板               | `Cmd+Option+Up`       | `Ctrl+Alt+Up`         |
| `workbench.panel.active.maximization.toggle` | 活动面板               | `Cmd+Shift+Enter`     | `Ctrl+Shift+Enter`    |
| `workbench.panel.active.split-right`         | 活动面板               | `Cmd+D`               | `Ctrl+Shift+D`        |
| `workbench.panel.active.split-below`         | 活动面板               | `Cmd+Shift+D`         | `Ctrl+Shift+E`        |
| `workbench.search.all.open`                  | Workbench              | `Cmd+Shift+F`         | `Ctrl+Shift+K`        |
| `workbench.settings.open`                    | Workbench              | `Cmd+,`               | `Ctrl+,`              |
| `workbench.sidebar.toggle`                   | Workbench              | `Cmd+B`               | `Ctrl+Shift+B`        |
| `terminal.search.open`                       | 聚焦终端或查找         | `Cmd+F`               | `Ctrl+Shift+F`        |
| `terminal.selection.copy`                    | 具有选择功能的聚焦终端 | `Cmd+C`               | `Ctrl+Shift+C`        |
| `terminal.clipboard.paste`                   | 聚焦终端               | `Cmd+V`               | `Ctrl+Shift+V`        |
| `terminal.history.prompt-previous`           | 聚焦终端               | `Ctrl+Shift+Z`        | `Ctrl+Shift+Z`        |
| `terminal.history.prompt-next`               | 聚焦终端               | `Ctrl+Shift+X`        | `Ctrl+Shift+X`        |
| `terminal.history.scroll-line-up`            | 聚焦终端               | `Ctrl+Shift+Up`       | `Ctrl+Shift+Up`       |
| `terminal.history.scroll-line-down`          | 聚焦终端               | `Ctrl+Shift+Down`     | `Ctrl+Shift+Down`     |
| `terminal.history.scroll-page-up`            | 聚焦终端               | `Ctrl+Shift+PageUp`   | `Ctrl+Shift+PageUp`   |
| `terminal.history.scroll-page-down`          | 聚焦终端               | `Ctrl+Shift+PageDown` | `Ctrl+Shift+PageDown` |
| `terminal.history.scroll-top`                | 聚焦终端               | `Ctrl+Shift+Home`     | `Ctrl+Shift+Home`     |
| `terminal.history.scroll-bottom`             | 聚焦终端               | `Ctrl+Shift+End`      | `Ctrl+Shift+End`      |
| `terminal.selection.select-all`              | 聚焦终端               | `Cmd+A`               | `Ctrl+Shift+A`        |
| `terminal.clear`                             | 聚焦终端               | `Cmd+K`               | `Ctrl+Shift+L`        |

新建标签页、关闭活动终端、活动面板最大化、向右分屏、向下分屏和粘贴的默认键采用 2026-08-22 观察到的 Warp [桌面端文档约定](https://docs.warp.dev/getting-started/keyboard-shortcuts)；这里只固定选择默认按键，不主张更广泛的行为或视觉一致性。上一个和下一个标签页的默认键使用 Warp 修订版 `086dade63ff7327426d6276d57f85b3f2f313af0` 中的对应绑定。Apple 的全部搜索使用 `Cmd+Shift+F`，把熟悉的 `Cmd+K` 留给且只留给已聚焦终端的清除操作。标准平台的清除使用 `Ctrl+Shift+L`；标准默认值特意保留未加 Shift 的 `Ctrl+B/T/W/K/F/D/E/V/A/L` 与 `Ctrl+C`，供 shell、终端应用和多路复用器使用。用户可以主动录入其他受支持的自定义组合键。即使某个 Action 暂时不可用，产品内冲突仍然不允许出现。

上一个和下一个提示符导航保留 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的精确默认键 `Ctrl+Shift+Z` 与 `Ctrl+Shift+X`。Apple 与标准平台都使用显式 Control，而不是平台 `Mod` 抽象。行为与有界 Host 行契约由终端包的 [Kitty 提示符导航对齐](../../../packages/terminal/docs/kitty-prompt-navigation-alignment.zh-CN.md)所有。

逐行、逐页、顶部和底部历史导航同样保留该修订版中 Kitty 的主要
`kitty_mod+Arrow/Page/Home/End` 绑定。两个平台都使用显式 `Ctrl+Shift`；因为本目录为每个 Action
拥有一个绑定，Kitty 额外的 macOS Command 和 `J`/`K` 别名只能通过替换有效自定义绑定来使用。翻页
保留 Kitty 的一行重叠。行为与有界 Host 行契约由终端包的
[Kitty 键盘回滚对齐](../../../packages/terminal/docs/kitty-keyboard-scrollback-alignment.zh-CN.md)所有。

三个应用范围字号默认值投影自 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44`
中的 macOS `cmd+equal`、`cmd+minus` 和 `cmd+0` 入口。标准平台通过 `Mod` 使用相同物理按键，
得到 `Ctrl+=`、`Ctrl+-` 和 `Ctrl+0`。这是有意限定的常用按键投影：在快捷键模型能够为每个
Action 保留多个绑定之前，不包含 Kitty 的小键盘、Shift 加号和 Backspace 别名。

## 终端粘贴路由

平台默认的 `terminal.clipboard.paste` 组合键具有原生事件回退，因为 `@orchbee/terminal`
拥有感知 MIME 的 DOM `paste` 契约：存在纯文本时以文本为准；仅含图片的事件则为正在
运行的 Agent 精确发送一对合成 `Control+V` 按下/释放事件，并且不读取图片字节。自定义
粘贴组合键无法触发该浏览器事件，因此会调用 Paste Action。

普通 Host 选择处于活动状态时，终端的隐藏 textarea 仍是唯一的浏览器编辑与 IME 所有者；
Canvas 与无障碍描述会投影该选择，但不会创建文档全局 DOM Range。原生粘贴可能投递到
textarea，也可能投递到 package 自有的终端 Presentation 后代，因此终端 root 提供有界后备，
精确执行一次共享粘贴路径。查找输入框等应用自有后代保留自己的原生编辑行为，该后备绝不
消费其事件。

投影终端剪贴板 Action 协调器负责记录是否有一个精确的选择 Copy 或 Paste Action 正在
等待。快捷键解析把规范化组合键与该同步状态结合起来：

| 有效粘贴组合键 | 选择 Copy | Paste Action | 路由                                          |
| -------------- | --------- | ------------ | --------------------------------------------- |
| 平台默认       | 空闲      | 空闲         | 保留按键默认行为，以触发一个原生 `paste` 事件 |
| 平台默认       | 等待中    | 任意         | 调用 Paste Action，并等待该精确 Copy          |
| 平台默认       | 空闲      | 等待中       | 调用 Paste Action，由其并发策略拒绝重复请求   |
| 自定义         | 任意      | 任意         | 调用 Paste Action                             |

Action 路由只在 Action 启用时消费按键；否则仍允许浏览器或终端行为处理。选择 Copy 在其精确
Promise 结算前始终处于等待状态。Paste Action 会快照并等待该 Promise，再读取当前剪贴板
文本，并重新检查取消状态，且不执行重试。Copy 失败仍保留自己的结果；Paste 会在 Copy
结算后读取当前剪贴板。Paste Action 在准备和终端发布期间始终处于等待状态，因此第二次
默认组合键不能逃逸到原生路径并重复输入。上下文菜单 Paste 始终使用 Action 路由。

每个获准的快捷键只采用一条路由。原生路由不调用 Action、不读取 Main 拥有的剪贴板
文本、不产生 Action 状态，并把文本/图片优先级委托给终端适配器。Action 路由绝不读取
图片字节，也不会把 Desktop 剪贴板读取契约扩展到有界纯文本之外。

方向面板聚焦采用 Ghostty 在修订版 `91f66da24527fa02d92b5fd0b41cd020f553a64c` 中的 Apple `Command+Option+Arrow` 与标准平台 `Control+Alt+Arrow` 约定，并与 Warp 修订版 `086dade63ff7327426d6276d57f85b3f2f313af0` 交叉核对。此处只负责应用层绑定选择；空间邻居解析由 Workbench 所有。

没有有效绑定的命令不渲染空的快捷键提示。“重置”“新建子终端”“向左分屏”和“向上分屏”在本目录中仍然有意保持未绑定。允许用户为初始未绑定命令创建可空绑定不在本模型范围内。

## Workbench 分发作用域

Web 发布 Workbench 是否为当前活动产品表面。已挂载的 Workbench 在所属 document 上拥有一个捕获监听器，并且只在该语义作用域活动时参与。焦点位于终端、Tab 条、边栏控件、Workbench 交互表面或 document body 都不会改变 Workbench 绑定的适用性。打开全屏设置表面时，仍挂载的 Workbench 变为非活动；关闭设置后重新活动。卸载 Workbench 会移除监听器。

| Workbench 表面状态     | 绑定决定                                           |
| ---------------------- | -------------------------------------------------- |
| 活动且有一个或多个 Tab | 解析 Workbench 绑定，不依赖聚焦的 Workbench 区域   |
| 活动且没有 Tab         | 新建 Tab 仍启用；依赖目标的关闭与 Panel 命令不可用 |
| 设置已打开             | 跳过所有 Workbench 绑定并保留事件默认行为          |
| Workbench 已卸载       | 没有 Workbench 监听器或绑定参与                    |

解析出的绑定仍然只在对应 Action 启用时运行。成功执行新建 Tab 后，新 Tab 成为活动 Tab，其第一个 Panel 终端被投影为唯一的 `autoFocus` 目标；注入的终端表面会在就绪时取得真实输入焦点。活动终端与活动 Panel 命令从活动 Tab 记住的聚焦 Panel 解析，而不是从键盘事件目标解析，因此边栏或 document body 的焦点不会把命令重定向。不可用 Action 会保留状态且不调用 `preventDefault()`。

`Mod+1` 到 `Mod+9` 会用相应的一基位置调用 `workbench.tab.select-by-position`。位置取自活动终端上下文的规范 `TabModel.tabs` 顺序：固定 Tab 前缀在最前，之后是当前的分组与未分组顺序。折叠组中的 Tab 仍会占据位置；委托的 `tabs.tab.select` Action 会通过普通选择转换展开所选 Tab 所在的组。缺少对应位置时 Action 不可用，按键事件保持不变。`Mod+9` 表示第九个 Tab，绝不表示最后一个 Tab。

### 相邻标签页选择模型

- **问题与输入：** 用户需要在不知道标签页位置的情况下遍历活动终端上下文。可信输入是最新的规范 `TabModel` 快照以及语义化的 `previous` 或 `next` 方向；Action 不接受动态输入。
- **数据、模型与所有权：** Workbench 从有序的 `TabModel.tabs` 投影中解析一个相邻标签页 ID。`@orchbee/tab-strip` 仍是选择状态的唯一所有者，已有的预备 `tabs.tab.select` Action 仍是唯一写入者。
- **输出与转换：** 零个或一个标签页、缺失的活动标签页或缺失的下层 Action 都不可用，并保留按键事件。至少有两个标签页时，previous 选择索引 `i - 1`，next 选择索引 `i + 1`；越过任一边界时回绕到另一端。选择使用 `replace`，因此分组、折叠、固定、焦点与导航行为仍归普通标签页选择转换所有。
- **不变量与重新校验：** 目标不同于活动标签页，属于同一个活动终端上下文，并遵循规范顺序而非 MRU 顺序。可用性与执行都会从当前快照解析；目标被移除或下层注册被替换时会阻止操作，不执行直接修改。
- **验证与排除项：** 纯解析器测试覆盖空、单例、中间位置、两个回绕边界、反向方向及缺失的活动标识。Action 测试覆盖分组与固定顺序、下层注册移除及执行时重新解析。快捷键测试覆盖两个平台、额外修饰键、重复、输入法组合、禁用时继续传递和自定义绑定。重排、跨 Workspace 遍历、跨窗口遍历及 MRU 遍历不在范围内。

### 终端字号转换模型

- **问题与输入：** 用户需要在不打开设置的情况下调整每个终端的大小。输入是在 Workbench 快捷键作用域活动时触发的一个语义 `increase`、`decrease` 或 `reset` Action；不要求终端焦点或 Panel 身份。
- **数据、模型与所有权：** Web 的 `TerminalSettingsController` 仍是设备本地外观快照的唯一写入者。增大和减小使用两点步长并限制在现有的 `8`–`48` 闭区间内；重置只恢复产品默认字号。
- **输出与转换：** 发生变化的值会向每个已挂载终端发布一份完整呈现配置，并持久化到现有外观记录。持久化失败时保留会话值。边界请求或已经是默认值的请求返回 `unchanged`。
- **不变量与重新验证：** 所有已挂载终端观察同一个应用范围值；无关外观首选项绝不改变。即使边界结果未变化，获准的精确组合键也会被消费，因此不会转化为浏览器页面缩放。设置界面会停用 Workbench 快捷键作用域并保留自己的编辑按键。
- **验证与排除项：** 纯测试覆盖步长、限制、重置、发布、持久化和处置。Action 与应用测试覆盖三个意图、物理按键分发和设置界面继续传递。每个 Panel 的缩放、浏览器页面缩放、小键盘别名、Shift 加号别名及多个同时生效的绑定不在范围内。

## 活动目标命令模型

八个 `workbench.*.active.*` Action 是上下文命令。菜单发起的 Action 调用可以携带显式面板目标；否则使用活动标签页中聚焦的面板。执行开始时会重新解析目标，因此陈旧或缺失的目标不可用，也不会吞掉键盘事件。

`workbench.terminal.active.close` 精确解析为一个下层关闭操作：

| 活动目标状态                                     | 准备的操作                     |
| ------------------------------------------------ | ------------------------------ |
| 当前显示的是子终端                               | 关闭该子终端                   |
| 当前显示的是主终端，且活动标签页有两个或更多面板 | 关闭该面板                     |
| 当前显示的是主终端，且活动标签页只有一个面板     | 关闭活动标签页                 |
| 不存在有效活动标签页或目标面板                   | 不可用；保留状态与事件默认行为 |

上下文 Action 会把已经注册的面板或标签页关闭 Action 准备成调用并执行。下层所有者继续负责可用性、运行中进程确认、取消、串行化与权威提交。上下文 Action 只报告下层执行结果，绝不直接修改面板或标签页状态。取消确认后，拓扑与终端会话保持不变。取消上下文执行会取消其准备好的下层执行；如果下层操作已经提交，则其完成结果仍是权威结果。下层注册缺失或陈旧时，上下文 Action 会被阻止，且不会通过直接修改来兜底。

最大化只在活动标签页至少有两个面板时可解析。向右和向下分屏 Action 在存在有效目标面板时可解析。每个命令都准备并执行现有面板 Action，因此菜单、键盘分发和 Action 搜索界面共享同一个状态转换与可用性结果。不可用命令既不会执行，也不会触发 `preventDefault()`。

四个方向聚焦 Action 仅在没有面板最大化时根据加权拆分布局解析。Workbench 要求所请求方向存在一个在垂直于移动方向的轴上具有正重叠的相邻面板，在外边缘绝不循环，并通过现有面板聚焦 Action 委托结果。方向上没有邻居时，快捷键事件仍可交给终端或浏览器处理。

## 和弦与冲突模型

`@orchbee/keyboard-shortcuts` 只提供纯物理和弦机制。和弦包含一个受支持的 `KeyboardEvent.code` 和标准化的精确修饰符集。 `Mod` 映射到 Apple 上的 Command 和标准平台上的 Control。匹配拒绝额外的修饰符、组合（包括旧键代码 229）、重复、不支持的代码和仅修饰符事件。格式和冲突身份来自相同的标准化和弦。

目录中的所有声明都可能重叠：Workbench 快捷键及其聚焦终端的快捷键可能同时适用。因此，其有效组合键必须在产品内全局唯一。发生冲突的编辑既不改变内存也不改变存储，并会标识冲突的 Action、范围、绑定来源和可用性条件。分发期间，所属 Action 适配器会在执行或调用 `preventDefault()` 前解析当前可用性；不可用绑定会继续传递给终端或浏览器。

Workbench 负责格式化自身绑定。投影终端负责其绑定默认值与分发，而 Web 只通过只读 Workbench 端口投影当前的 Action 到标签映射。这样可保持依赖方向：Workbench 永远不导入应用拥有的快捷键目录。命令菜单项保存可选的有效标签，两个共享菜单渲染器都使用 UI primitive 的尾部快捷键槽位。Action 搜索接收同一个投影器，其中也包括从后代作用域发现的终端 Action。

## 持久化与恢复

新写入使用 `orchbee:web:keyboard-shortcuts:v8` 严格记录：

```json
{
	"version": 8,
	"overrides": [
		{
			"shortcutId": "workbench.search.all.open",
			"chord": { "code": "KeyG", "modifiers": ["mod", "shift"] }
		}
	]
}
```

只存储自定义值，并按快捷键 ID 排序。未知字段或 ID、重复项、未排序条目、非规范修饰符、不支持的代码、已存储默认值、过多条目以及有效冲突都会使完整 v8 记录无效，并恢复平台默认值。写入失败时，不可变的会话绑定仍保持活动，同时报告尚未保存，并在下一次合法修改时重试。恢复单项会移除其覆盖。

当 v8 不存在时，Web 会先按准确的旧目录与旧默认值读取 `orchbee:web:keyboard-shortcuts:v7`；如果 v7 也不存在，则依次尝试 v6、v5、v4、v3、v2 和 v1。每次迁移都会把显式覆盖带入扩展后的目录，移除与当前默认值等价的覆盖，拒绝迁移后产生的冲突，并写入规范 v8 记录。v1、v2、v3、v4、v5、v6 与 v7 值原样保留，作为回滚快照。无效当前记录绝不回退到更旧记录，无效迁移源也绝不继续读取下一个更旧版本，因为任一回退都会掩盖数据损坏。回滚时会恢复旧版本能够理解的最新记录；只在 v8 中做出的编辑有意不对旧版本可用。

Browser Preview、Gateway Browser 和 Electron 各自针对自己的源本地存储创建并销毁一个控制器。Gateway 与 Electron 通过 `ProjectedTerminalSurfaceCapability` 传递控制器的窄绑定来源。事件处理器在分发时读取当前表，因此修改不会重新挂载 Workbench、替换附件，也不会重新打开 Host 或 Gateway 连接。

## 声明与证据

| 声明                                                       | 确定性证据                                                                                 |
| ---------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| 一个有效目标只产生一个子终端、面板或标签页关闭操作         | 活动目标解析器决策表测试                                                                   |
| 确认、取消与提交继续由准备好的下层 Action 所有             | 同时注册面板与标签页 Action 的 Workbench Action 集成测试                                   |
| 活动 Workbench 绑定不依赖仍然存在的聚焦后代                | document 捕获生命周期与 Web 应用集成测试                                                   |
| 空 Tab 状态下新建 Tab 会创建并聚焦新终端，设置会抑制它     | Web 应用焦点与设置作用域集成轨迹                                                           |
| Apple 与标准平台默认值符合目录，且自定义编辑会影响分发     | 快捷键定义、解析器与控制器测试                                                             |
| 空闲的平台默认终端 Paste 保留感知 MIME 的原生投递          | 快捷键路由与安装器测试、终端图片粘贴测试及精确 Desktop `terminal-parity/IN-03-image-paste` |
| 活动的普通终端选择会保留原生粘贴与输入所有权               | 投影终端选择/粘贴测试及精确 Desktop `terminal-parity/IN-18-native-selection-paste`         |
| 等待中的 Copy 与 Paste 工作不会接纳陈旧或重复快捷键输入    | 剪贴板协调器与 Action 测试及精确 Desktop `terminal-parity/IN-13-selection-copy-paste`      |
| 方向聚焦会选择空间邻居，并在外边缘继续传递事件             | 四面板解析器、Action 与快捷键分发测试                                                      |
| 按位置选择遵循规范 Tab 顺序并在越界时继续传递事件          | 分组与固定 Action 集成测试及快捷键分发测试                                                 |
| 相邻选择遵循规范 Tab 顺序，且仅在存在多个 Tab 时回绕       | 纯解析器、预备 Action 集成与快捷键继续传递测试                                             |
| 菜单、Action 搜索和设置显示同一个当前绑定                  | 投影与渲染界面测试                                                                         |
| 有效 v1、v2、v3、v4、v5 与 v6 记录迁移时不会丢失显式旧覆盖 | 存储迁移测试，包括与新默认值的冲突                                                         |
| 无效当前记录或迁移源数据不会读取更旧记录                   | 存储损坏与优先级测试                                                                       |

测试使用合成的平台标志、Action 作用域、存储、document 捕获、注入终端焦点与键盘事件。它们不主张操作系统全局快捷键行为或完整 Warp 兼容性。手动 Desktop 原生窗口焦点测试不包含在自动验证中。

## 维护检查

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
