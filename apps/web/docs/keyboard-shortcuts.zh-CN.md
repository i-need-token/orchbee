# 键盘快捷键

`@orchbee/web` 拥有一组精选常见语义快捷键的客户端本地覆盖。Workbench 和投影终端功能继续拥有各自的 Action 输入与平台默认值。Web 组合层汇总这些声明，验证完整的有效绑定表，并把同一个来源注入已挂载的 Workbench 与投影终端。菜单、工具提示、Action 搜索界面和快捷键设置都从该有效绑定表投影标签；这些界面都不拥有另一份绑定副本。

## 初始目录

| 快捷方式 ID                                   | 适用范围               | 苹果默认                 | 标准默认值             |
| --------------------------------------------- | ---------------------- | ------------------------ | ---------------------- |
| `workbench.search.actions.open`               | Workbench              | `Cmd+Shift+P`            | `Ctrl+Shift+P`         |
| `workbench.focus.next-region`                 | Workbench              | `Cmd+F6`                 | `Ctrl+F6`              |
| `workbench.focus.previous-region`             | Workbench              | `Cmd+Shift+F6`           | `Ctrl+Shift+F6`        |
| `workbench.terminal.create`                   | Workbench              | `Cmd+T`                  | `Ctrl+Shift+T`         |
| `workbench.terminal.create-current-directory` | 活动终端               | `Option+Cmd+Enter`       | `Ctrl+Alt+Enter`       |
| `workbench.terminal.font-size.increase`       | Workbench              | `Cmd+=`                  | `Ctrl+=`               |
| `workbench.terminal.font-size.decrease`       | Workbench              | `Cmd+-`                  | `Ctrl+-`               |
| `workbench.terminal.font-size.reset`          | Workbench              | `Cmd+0`                  | `Ctrl+0`               |
| `workbench.tab.select.1` … `.9`               | Workbench              | `Cmd+1` … `Cmd+9`        | `Ctrl+1` … `Ctrl+9`    |
| `workbench.tab.select-previous`               | Workbench              | `Cmd+Shift+[`            | `Ctrl+PageUp`          |
| `workbench.tab.select-next`                   | Workbench              | `Cmd+Shift+]`            | `Ctrl+PageDown`        |
| `workbench.tab.active.move-backward`          | Workbench              | `Ctrl+Shift+,`           | `Ctrl+Shift+,`         |
| `workbench.tab.active.move-forward`           | Workbench              | `Ctrl+Shift+.`           | `Ctrl+Shift+.`         |
| `workbench.tab.title.set`                     | Workbench              | `Cmd+Shift+I`            | `Ctrl+Alt+Shift+T`     |
| `workbench.terminal.active.close`             | 活动终端               | `Cmd+W`                  | `Ctrl+Shift+W`         |
| `workbench.panel.active.focus-down`           | 活动面板               | `Cmd+Option+Down`        | `Ctrl+Alt+Down`        |
| `workbench.panel.active.focus-left`           | 活动面板               | `Cmd+Option+Left`        | `Ctrl+Alt+Left`        |
| `workbench.panel.active.focus-next`           | 活动面板               | `Ctrl+Shift+]`           | `Ctrl+Shift+]`         |
| `workbench.panel.active.focus-previous`       | 活动面板               | `Ctrl+Shift+[`           | `Ctrl+Shift+[`         |
| `workbench.panel.active.focus-right`          | 活动面板               | `Cmd+Option+Right`       | `Ctrl+Alt+Right`       |
| `workbench.panel.active.focus-up`             | 活动面板               | `Cmd+Option+Up`          | `Ctrl+Alt+Up`          |
| `workbench.panel.active.move-down`            | 活动面板               | `Cmd+Option+Shift+Down`  | `Ctrl+Alt+Shift+Down`  |
| `workbench.panel.active.move-left`            | 活动面板               | `Cmd+Option+Shift+Left`  | `Ctrl+Alt+Shift+Left`  |
| `workbench.panel.active.move-right`           | 活动面板               | `Cmd+Option+Shift+Right` | `Ctrl+Alt+Shift+Right` |
| `workbench.panel.active.move-up`              | 活动面板               | `Cmd+Option+Shift+Up`    | `Ctrl+Alt+Shift+Up`    |
| `workbench.panel.active.equalize`             | 活动面板               | `Ctrl+Cmd+=`             | `Ctrl+Alt+=`           |
| `workbench.panel.active.maximization.toggle`  | 活动面板               | `Cmd+Shift+Enter`        | `Ctrl+Shift+Enter`     |
| `workbench.panel.active.resize`               | 活动面板               | `Cmd+R`                  | `Ctrl+Shift+R`         |
| `workbench.panel.active.split-right`          | 活动面板               | `Cmd+D`                  | `Ctrl+Shift+D`         |
| `workbench.panel.active.split-below`          | 活动面板               | `Cmd+Shift+D`            | `Ctrl+Shift+E`         |
| `workbench.search.all.open`                   | Workbench              | `Cmd+Shift+F`            | `Ctrl+Shift+K`         |
| `workbench.settings.open`                     | Workbench              | `Cmd+,`                  | `Ctrl+,`               |
| `workbench.sidebar.toggle`                    | Workbench              | `Cmd+B`                  | `Ctrl+Shift+B`         |
| `terminal.search.open`                        | 聚焦终端或查找         | `Cmd+F`                  | `Ctrl+Shift+F`         |
| `terminal.hints.open`                         | 聚焦终端               | `Cmd+Shift+O`            | `Ctrl+Shift+O`         |
| `terminal.output.show-last`                   | 聚焦终端               | `Ctrl+Shift+G`           | `Ctrl+Shift+G`         |
| `terminal.history.show`                       | 聚焦终端               | `Ctrl+Shift+H`           | `Ctrl+Shift+H`         |
| `terminal.selection.copy`                     | 具有选择功能的聚焦终端 | `Cmd+C`                  | `Ctrl+Shift+C`         |
| `terminal.clipboard.paste`                    | 聚焦终端               | `Cmd+V`                  | `Ctrl+Shift+V`         |
| `terminal.history.prompt-previous`            | 聚焦终端               | `Ctrl+Shift+Z`           | `Ctrl+Shift+Z`         |
| `terminal.history.prompt-next`                | 聚焦终端               | `Ctrl+Shift+X`           | `Ctrl+Shift+X`         |
| `terminal.history.scroll-line-up`             | 聚焦终端               | `Ctrl+Shift+Up`          | `Ctrl+Shift+Up`        |
| `terminal.history.scroll-line-down`           | 聚焦终端               | `Ctrl+Shift+Down`        | `Ctrl+Shift+Down`      |
| `terminal.history.scroll-page-up`             | 聚焦终端               | `Ctrl+Shift+PageUp`      | `Ctrl+Shift+PageUp`    |
| `terminal.history.scroll-page-down`           | 聚焦终端               | `Ctrl+Shift+PageDown`    | `Ctrl+Shift+PageDown`  |
| `terminal.history.scroll-top`                 | 聚焦终端               | `Ctrl+Shift+Home`        | `Ctrl+Shift+Home`      |
| `terminal.history.scroll-bottom`              | 聚焦终端               | `Ctrl+Shift+End`         | `Ctrl+Shift+End`       |
| `terminal.selection.select-all`               | 聚焦终端               | `Cmd+A`                  | `Ctrl+Shift+A`         |
| `terminal.clear`                              | 聚焦终端               | `Cmd+K`                  | `Ctrl+Shift+L`         |

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

可见键盘 Hints 在两个平台都使用 `Mod+Shift+O`。这个统一绑定避开已由向下拆分拥有的标准平台
`Ctrl+Shift+E`，同时为 Kitty 常用的可见 URL、路径、文件位置与 hash 选择工作流保留一个可发现
Action。检测、效果、取消与排除项由终端包的
[Kitty 键盘 Hints 对齐](../../../packages/terminal/docs/kitty-keyboard-hints-alignment.zh-CN.md)所有。

“显示上一条命令输出”在两个平台都保留 Kitty 修订版
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `Ctrl+Shift+G` 默认键。它会为最新的有界
shell 集成输出打开不可变查看器；提取、搜索、复制、取消与排除契约由终端包的
[Kitty 命令输出查看器对齐](../../../packages/terminal/docs/kitty-command-output-viewer-alignment.zh-CN.md)所有。

“显示回滚历史”在两个平台都保留 Kitty 同一修订版的 `Ctrl+Shift+H` 默认键。它会打开一份包含完整
主屏幕保留历史的不可变 document，并从终端捕获的可见顶部开始呈现。精确提取、换行投影、搜索、
按来源复制、取消与排除项由终端包的
[Kitty 回滚历史查看器对齐](../../../packages/terminal/docs/kitty-scrollback-viewer-alignment.zh-CN.md)所有。

活动标签页移动在两个平台都保留 Kitty 修订版
`6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `Ctrl+Shift+,` 与 `Ctrl+Shift+.` 默认键。
Workbench 在 Apple 平台仍使用显式 Control，因为 Kitty 的 `kitty_mod` 默认值在该平台同样是
Control+Shift；按键选择以外采用的行为由下方移动模型定义。

Panel 循环聚焦在两个平台都保留该修订版中“下一个”的 `Ctrl+Shift+]` 与“上一个”的
`Ctrl+Shift+[` 默认键。Workbench 在 Apple 平台仍使用显式 Control，只遍历当前 Tab 的规范深度优先
Panel 叶节点顺序，并且仅在至少存在两个 Panel 时回绕。

Panel 按位置聚焦在两个平台都保留该修订版的物理 `Ctrl+Shift+1` 到 `Ctrl+Shift+0` 默认键，其中
`0` 表示位置十。Workbench 把它们映射到当前 Tab 内规范深度优先 Panel 叶节点顺序的一基精确位置。
它刻意省略 Kitty 的 macOS `Command+digit` 别名，因为这些组合键继续用于选择 Tab，也不会把缺失位置
钳制到最后一个 Panel。

Panel 可视聚焦与交换在两个平台都保留该修订版的 `Ctrl+Shift+F7` 与 `Ctrl+Shift+F8` 默认键。
双 Panel Tab 会立即选择唯一的另一 Panel。三个或更多 Panel 会进入 Workbench 的精确、排除源的
字母数字选择器；目标顺序、多字符标签、陈旧会话取消，以及对现有聚焦与交换 Action 的委托均由
Workbench 拥有。

设置 Tab 标题保留该修订版中 Apple 的 `Shift+Cmd+I` 默认键与标准平台的
`Ctrl+Shift+Alt+T` 默认键。Workbench 使用同一个语义 Action 处理快捷键分发、Action 搜索和被调用
Tab 的上下文菜单；下方标题模型定义采纳的编辑器与重置行为。

“调整活动面板大小”保留 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 中 macOS
的 `cmd+r` 调整模式入口；标准平台使用 `Ctrl+Shift+R`，从而为 shell 保留未加 Shift 的组合键。
共享模态框采用 Kitty 的 `W` 变宽、`N` 变窄、`T` 变高、`S` 变矮、`R` 重置、Control 修饰的大步幅
以及 Escape 退出词汇。Orchbee 使用不依赖几何尺寸的五个百分点步幅，按住 Control 时翻倍，为每个
分支至少保留百分之五，并把捕获标签页中的每个分割重置为等权重。

“均分面板”采用 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 中
`splits.py::equalize_biases` 的语义：同轴嵌套 Panel 获得相等的规范化范围；垂直子树在父轴上计为一个
单元，并在自身轴上递归均分。Apple 使用 Ghostty 修订版
`9c3ec931d64561a8407dde7ac984ce156ae91539` 的 `Control+Command+=` 约定；标准平台使用
`Ctrl+Alt+=`，因为 Kitty 示例中的 `Ctrl+Shift+E` 已属于“向下拆分”。这一单次操作不同于调整大小
模式的 `R`；后者会把每个二叉 split 重置为子节点等权重。

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

方向 Panel 移动采用 Kitty 修订版 `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的当前布局邻居选择、外侧边缘空操作和 Splits 叶节点交换。Orchbee 在现有焦点组合键上增加 Shift，形成 Apple `Command+Option+Shift+Arrow` 与标准平台 `Ctrl+Alt+Shift+Arrow`。无修饰的 `Shift+Arrow` 仍供 shell 选择使用。Kitty 的 forward/backward 默认键保留给循环聚焦，而不是移动；forward/backward/to-top 移动循环与数值移动不在范围内。

没有有效绑定的命令不渲染空的快捷键提示。“重置”“新建子终端”“向左分屏”和“向上分屏”在本目录中仍然有意保持未绑定。允许用户为初始未绑定命令创建可空绑定不在本模型范围内。

## Workbench 分发作用域

Web 发布 Workbench 是否为当前活动产品表面。已挂载的 Workbench 在所属 document 上拥有一个捕获监听器，并且只在该语义作用域活动时参与。焦点位于终端、Tab 条、边栏控件、Workbench 交互表面或 document body 都不会改变 Workbench 绑定的适用性。打开全屏设置表面时，仍挂载的 Workbench 变为非活动；关闭设置后重新活动。卸载 Workbench 会移除监听器。

| Workbench 表面状态     | 绑定决定                                               |
| ---------------------- | ------------------------------------------------------ |
| 活动且有一个或多个 Tab | 解析 Workbench 绑定，不依赖聚焦的 Workbench 区域       |
| 活动且没有 Tab         | 新建 Tab 仍启用；当前目录创建、关闭与 Panel 命令不可用 |
| 设置已打开             | 跳过所有 Workbench 绑定并保留事件默认行为              |
| Workbench 已卸载       | 没有 Workbench 监听器或绑定参与                        |

解析出的绑定仍然只在对应 Action 启用时运行。成功执行新建 Tab 后，新 Tab 成为活动 Tab，其第一个 Panel 终端被投影为唯一的 `autoFocus` 目标；注入的终端表面会在就绪时取得真实输入焦点。活动终端与活动 Panel 命令从活动 Tab 记住的聚焦 Panel 解析，而不是从键盘事件目标解析，因此边栏或 document body 的焦点不会把命令重定向。不可用 Action 会保留状态且不调用 `preventDefault()`。

Panel 可视选择器活动期间，其工作区根节点拥有 DOM 焦点，并发布独占键盘捕获标记。document 级
Workbench 快捷键监听器会跳过该标记内的事件，随后工作区会消费每个物理标签、Backspace、Escape、
输入法组合、重复、仍按修饰键与无效按键事件。这样既防止递归快捷键分发与终端输入，也让工作区外的
指针取消继续到达预期目标。

`Mod+1` 到 `Mod+9` 会用相应的一基位置调用 `workbench.tab.select-by-position`。位置取自活动终端上下文的规范 `TabModel.tabs` 顺序：固定 Tab 前缀在最前，之后是当前的分组与未分组顺序。折叠组中的 Tab 仍会占据位置；委托的 `tabs.tab.select` Action 会通过普通选择转换展开所选 Tab 所在的组。缺少对应位置时 Action 不可用，按键事件保持不变。`Mod+9` 表示第九个 Tab，绝不表示最后一个 Tab。

`Ctrl+Shift+1` 到 `Ctrl+Shift+9`，以及 `Ctrl+Shift+0`，会调用
`workbench.panel.active.focus-by-position`，并传入一到十的精确语义位置。位置根据活动 Tab 的规范
深度优先 Panel 叶节点顺序解析。目标存在时委托普通 Panel 聚焦 Action；目标已经聚焦时是刻意可用的
`unchanged` 结果，仍会消费事件。位置缺失、布局最大化、源陈旧、顺序无效或下层 Action 缺失时不可用，
事件保持不变。

### 当前目录 Tab 创建模型

- **问题与输入：** 用户需要一个以实际正在查看的终端目录为根的新 Tab。调用可以不携带目标，从而解析活动 Tab 的聚焦 Panel；也可以携带终端上下文菜单捕获的一个精确 Panel 目标。
- **数据与所有权：** Workbench 捕获正在显示的主 Presentation 或子 Presentation 标识，并读取其最新可信非空 `currentCwd`，缺失时回退到同一 Presentation 的 `launchCwd`。Workbench 仍是 Tab 与 Presentation 的唯一写入者；Host 仍对观测 CWD 和进程启动具有权威性。
- **转换与重新校验：** 执行会重新校验终端上下文、Tab、Panel 与 Presentation，再解析同一终端的最新 CWD，并用显式启动目录创建一个普通终端 Tab。缺少 CWD、标识陈旧或 Host capability 不可用时，该 Action 不可用、不创建任何内容，并保留按键事件。
- **不变量与排除项：** 成功会分配不同的 Tab、Panel 与 Presentation 标识，通过现有交接聚焦新终端，并保留源 session。普通新建 Tab 继续使用其 Workspace/默认 CWD。绝不复制 environment、executable、前台进程、output、attachment、SSH 状态或远程 shell 目录发现结果。
- **参考与证据：** 这一有界行为遵循 Kitty revision `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的 `new_tab_with_cwd` 目标选择，但使用可信终端元数据而不是进程 CWD 请求。解析器、runtime、Action、菜单、快捷键、Settings、v16 迁移及精确 Desktop `terminal-parity/FS-15-current-directory-tab` 测试覆盖采纳路径；Kitty session membership、命令参数、remote control 及其示例映射均排除。

### 相邻标签页选择模型

- **问题与输入：** 用户需要在不知道标签页位置的情况下遍历活动终端上下文。可信输入是最新的规范 `TabModel` 快照以及语义化的 `previous` 或 `next` 方向；Action 不接受动态输入。
- **数据、模型与所有权：** Workbench 从有序的 `TabModel.tabs` 投影中解析一个相邻标签页 ID。`@orchbee/tab-strip` 仍是选择状态的唯一所有者，已有的预备 `tabs.tab.select` Action 仍是唯一写入者。
- **输出与转换：** 零个或一个标签页、缺失的活动标签页或缺失的下层 Action 都不可用，并保留按键事件。至少有两个标签页时，previous 选择索引 `i - 1`，next 选择索引 `i + 1`；越过任一边界时回绕到另一端。选择使用 `replace`，因此分组、折叠、固定、焦点与导航行为仍归普通标签页选择转换所有。
- **不变量与重新校验：** 目标不同于活动标签页，属于同一个活动终端上下文，并遵循规范顺序而非 MRU 顺序。可用性与执行都会从当前快照解析；目标被移除或下层注册被替换时会阻止操作，不执行直接修改。
- **验证与排除项：** 纯解析器测试覆盖空、单例、中间位置、两个回绕边界、反向方向及缺失的活动标识。Action 测试覆盖分组与固定顺序、下层注册移除及执行时重新解析。快捷键测试覆盖两个平台、额外修饰键、重复、输入法组合、禁用时继续传递和自定义绑定。重排、跨 Workspace 遍历、跨窗口遍历及 MRU 遍历不在范围内。

### 活动标签页移动模型

- **问题与输入：** 用户需要在不离开终端输入的情况下重排活动标签页。可信输入是最新的规范
  `TabModel` 快照以及语义化的 `backward` 或 `forward` 意图；两个 Workbench Action 都不接受动态输入。
- **数据、模型与所有权：** Workbench 解析活动标签页 ID 和一个下层 `tabs.tab.move` 方向。
  `@orchbee/tab-strip` 仍是标签页顺序、固定分区、组成员关系、选择与活动标识的唯一所有者和写入者。
- **输出与转换：** 在活动标签页所属的固定或未固定分区内，backward 与 forward 委托一次相对移动。
  越过该分区的第一个或最后一个边界时会委托 `last` 或 `first`，从而在不破坏固定前缀不变量的前提下
  匹配 Kitty 的循环移动。缺少活动标签页、分区内少于两个标签页或缺少下层 Action 时不可用，并保留
  按键事件。预备好的下层 Action 拥有同步提交与结果。
- **不变量与重新校验：** 同一个活动标签页保持活动和选中；每个标签页标识仍精确存在一次；固定标签页
  仍组成连续前缀；所有分组副作用遵循现有 `TabModel` 移动转换。可用性与执行都会从当前快照解析，
  因此陈旧标识或注册绝不会触发直接兜底修改。
- **决策与参考范围：** 分区内部的标签页委托 `previous` 或 `next`；多标签页固定分区中的第一个或
  最后一个标签页委托 `last` 或 `first`；所有不可用状态都不产生决策。这里采用 Kitty 修订版
  `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的活动标签页方向、循环边界和精确默认键。
  Kitty 不具备的固定分区与分组、跨窗口移动、拖动行为、动画及视觉一致性不在范围内。
- **验证：** 纯解析器测试覆盖缺失、单例、中间位置、两个边界、两个固定分区及缺失活动标识。
  预备 Action 测试覆盖执行时重新解析、下层注册移除、活动标识不变与精确下层输入。快捷键、设置、
  Action 搜索及 Desktop 轨迹覆盖两个默认键、禁用时继续传递、自定义绑定、可见发现、最终顺序、焦点
  与终端输入连续性。

### 标签页标题模型

- **问题与输入：** 用户需要为活动或指针调用的 Tab 设置稳定名称，同时不丢失终端更新的动态标题。
  打开操作可以不带输入并解析活动 Tab，也可以接收菜单捕获的可信显式终端上下文/Tab 标识。提交携带
  同一标识对以及字符串或重置意图。
- **数据、模型与所有权：** Workbench 拥有精确的 `terminalContextId`/`tabId` 目标，
  `PanelWorkspaceState` 仍是 Tab 持久可空自定义标题的唯一写入者。自然 OSC 标题、当前/启动 CWD 与
  本地化后备值继续在其下独立更新。
- **输出与转换：** 打开时预填并选中当前生效标题。去除首尾空白后的非空提交会设置自定义标题，包括
  该值与当前自然标题相同时；空值或仅含空白的提交会清除它，并显示最新的自然标题、CWD 或后备投影。
  保存和取消都会关闭模态框，并恢复此前的终端焦点。
- **不变量与重新校验：** 打开与提交都会分别从当前快照解析精确上下文和 Tab。编辑器打开期间切换活动
  Tab 不会重定向写入；任一标识被移除都会阻止操作。自定义文本可见时仍会记录动态元数据，重置绝不会
  制造后备值或写入终端拥有的元数据。
- **决策与参考范围：** 这里采用 Kitty 修订版
  `6d5d0c440603ad9bdf6dcd599f73f6dde21acb44` 的两个精确默认键、以当前标题初始化编辑及空值重置。
  Orchbee 使用共享本地化模态框与持久共享文档，而不是 Kitty 原生对话框；窗口标题编辑、远程控制、
  多个同时生效的绑定及视觉一致性不在范围内。
- **验证：** 模型测试覆盖自定义/自然标题优先级、等值自定义锁定、重置与共享文档持久化。Action 与
  React 测试覆盖活动和显式目标、陈旧拒绝、快捷键/上下文菜单/Action 搜索入口、空值重置、焦点恢复及
  终端输入不变。设置、存储迁移和精确 Desktop `terminal-parity/FS-11-tab-title` 轨迹覆盖两个默认键与
  用户覆盖。

### 终端字号转换模型

- **问题与输入：** 用户需要在不打开设置的情况下调整每个终端的大小。输入是在 Workbench 快捷键作用域活动时触发的一个语义 `increase`、`decrease` 或 `reset` Action；不要求终端焦点或 Panel 身份。
- **数据、模型与所有权：** Web 的 `TerminalSettingsController` 仍是设备本地外观快照的唯一写入者。增大和减小使用两点步长并限制在现有的 `8`–`48` 闭区间内；重置只恢复产品默认字号。
- **输出与转换：** 发生变化的值会向每个已挂载终端发布一份完整呈现配置，并持久化到现有外观记录。持久化失败时保留会话值。边界请求或已经是默认值的请求返回 `unchanged`。
- **不变量与重新验证：** 所有已挂载终端观察同一个应用范围值；无关外观首选项绝不改变。即使边界结果未变化，获准的精确组合键也会被消费，因此不会转化为浏览器页面缩放。设置界面会停用 Workbench 快捷键作用域并保留自己的编辑按键。
- **验证与排除项：** 纯测试覆盖步长、限制、重置、发布、持久化和处置。Action 与应用测试覆盖三个意图、物理按键分发和设置界面继续传递。每个 Panel 的缩放、浏览器页面缩放、小键盘别名、Shift 加号别名及多个同时生效的绑定不在范围内。

## 活动目标命令模型

十九个 `workbench.*.active.*` Action 是上下文命令。菜单发起的 Action 调用可以携带显式面板目标；否则使用活动标签页中聚焦的面板。执行开始时会重新解析目标，因此陈旧或缺失的目标不可用，也不会吞掉键盘事件。

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

上一个与下一个聚焦 Action 只把当前 Tab 的规范布局展平为深度优先叶节点顺序，选择相邻 Panel，在两个
边界回绕，并通过同一个 Panel 聚焦 Action 委托该精确目标。单 Panel、陈旧源或 Panel 最大化时不可用，
因此组合键会继续传递，且不会跨 Tab、终端上下文或窗口。

按位置聚焦 Action 校验一个 `1` 到 `10` 的精确输入，只把当前 Tab 展平为规范深度优先 Panel 叶节点
顺序，并为 `position - 1` 处的叶节点准备普通 Panel 聚焦 Action。它绝不钳制。选择当前 Panel 仍然可用，
并返回下层 Action 的 `unchanged` 结果，包括单例中的位置一；位置缺失、源陈旧、Panel 标识重复、布局
最大化或下层注册缺失时，状态与按键事件都保持不变。

四个方向移动 Action 复用完全相同的邻居解析器，再准备一个精确源/目标 `workbench.panel.swap` Action。底层转换交换完整 Panel 叶节点，同时保留 split 拓扑、权重、slot ID、终端标识、计数器与 Panel 成员关系；移动源保持聚焦。缺少邻居、单 Panel 布局、Panel 最大化、目标陈旧或底层注册缺失时不会发生变更，并保留按键事件。

Panel 均分仅在活动 Tab 存在多个 Panel、没有最大化 Panel 且目标精确存活时解析，然后准备下层
`workbench.panel.equalize` Action。其递归单元模型会在每个 split 分配比例权重，同时保留拓扑、顺序、
Panel、Split、slot 与终端标识、成员关系、焦点、计数器及最大化状态。已均分布局是有意的可用
`unchanged` 结果，仍会消费已接纳的调用；单例、最大化、陈旧或缺少下层目标时不可用，并保留按键事件。

调整大小模式在打开时捕获精确的活动标签页和面板。每个调整按键都会重新验证该标识，通过底层面板
Action 委托一次变更，并调整分割轴与请求方向匹配的最近祖先。缺少对应轴或到达边界时保持不变；
最大化面板拒绝变更；切换或移除捕获的标签页或面板会关闭该模式。面板菜单、终端上下文菜单、Action
搜索和当前可配置快捷键都会打开同一模式。Electron 保持原生“重新加载”和“强制重新加载”菜单命令
可点击，但不注册其加速键，让渲染器拥有的当前绑定优先解析。

## 快捷键设置发现模型

- **问题与输入：** 设置目录中的快捷键数量已经很多，逐行浏览不再是可靠的发现路径。本地发现界面
  接受普通文本、输入的快捷键表示或一次捕获的物理 `KeyboardEvent`；当前本地化行投影和完整有效
  绑定表是可信输入，输入文本与按键事件是瞬时、不可信输入。缺失或仅含空白的查询表示完整目录。
- **数据、模型与所有权：** Web 拥有一个瞬时发现状态，由模式（`text` 或 `recording`）和查询
  （`text` 或一个标准化和弦）组成。纯解析器保留目录顺序并投影匹配的快捷键 ID。
  `KeyboardShortcutSettingsController` 仍是唯一的绑定写入者；发现功能只读其快照，不拥有存储、
  Action 或快捷键变更能力。
- **输出与不变量：** 空文本返回全部行。普通文本通过不区分大小写的合取词项搜索本地化 Action 名称、
  可见范围、可用条件与来源、稳定 Action 与快捷键 ID，以及格式化后的当前和弦。修饰键名称、常见
  Apple 符号、物理按键代码和常见按键标签都作为可搜索别名。捕获的和弦使用与冲突检测相同的规范
  和弦标识进行比较，因此只返回当前的精确匹配。零匹配产生本地化空状态；所有非空结果保留目录顺序。
  搜索、捕获、清除、取消与卸载绝不会改变绑定或持久化记录。

| 当前模式 | 事件                       | 下一模式 | 查询与可观察结果                                  |
| -------- | -------------------------- | -------- | ------------------------------------------------- |
| 文本     | 文本变化                   | 文本     | 替换为文本查询并立即投影匹配行                    |
| 文本     | 开始录制                   | 录制     | 保留当前查询、聚焦搜索框并等待一个受支持和弦      |
| 录制     | 受支持和弦                 | 录制     | 替换为精确和弦查询并显示其当前匹配 Action 行      |
| 录制     | 仅修饰键、重复或输入法组合 | 录制     | 无操作；保留先前查询                              |
| 录制     | 不支持或不含修饰键的按键   | 录制     | 保留先前查询并显示本地化的无效和弦指引            |
| 录制     | Escape、Tab 或显式停止     | 文本     | 停止捕获、保留当前查询，并让 Tab 继续普通焦点移动 |
| 任一     | 清除                       | 不变     | 将查询替换为空文本并显示完整目录                  |

普通模式下，非空查询会先被 Escape 清除，之后外层设置界面才可处理 Escape；空查询时 Escape 仍可交给
设置界面。开始现有的逐行快捷键编辑器会停止发现录制，开始发现录制则会取消任何逐行编辑器，因此同一
时刻只有一个已聚焦控件拥有物理按键捕获。搜索框通过礼貌状态报告结果数量，其可访问标签不依赖占位文案。

该交互采用 VS Code 修订版 `cd429513258458bcbe37b17fe714874197fe2adf` 对普通快捷键搜索与显式
“录制按键”模式的区分。Orchbee 有意只支持一个物理和弦而非 VS Code 的和弦序列，使用自身的有效
目录与本地化元数据，并将录制路径限定为发现功能而不是编辑命令。

## 和弦与冲突模型

`@orchbee/keyboard-shortcuts` 只提供纯物理和弦机制。和弦包含一个受支持的 `KeyboardEvent.code` 和标准化的精确修饰符集。 `Mod` 映射到 Apple 上的 Command 和标准平台上的 Control。匹配拒绝额外的修饰符、组合（包括旧键代码 229）、重复、不支持的代码和仅修饰符事件。格式和冲突身份来自相同的标准化和弦。

目录中的所有声明都可能重叠：Workbench 快捷键及其聚焦终端的快捷键可能同时适用。因此，其有效组合键必须在产品内全局唯一。发生冲突的编辑既不改变内存也不改变存储，并会标识冲突的 Action、范围、绑定来源和可用性条件。分发期间，所属 Action 适配器会在执行或调用 `preventDefault()` 前解析当前可用性；不可用绑定会继续传递给终端或浏览器。

Workbench 负责格式化自身绑定。投影终端负责其绑定默认值与分发，而 Web 只通过只读 Workbench 端口投影当前的 Action 到标签映射。这样可保持依赖方向：Workbench 永远不导入应用拥有的快捷键目录。命令菜单项保存可选的有效标签，两个共享菜单渲染器都使用 UI primitive 的尾部快捷键槽位。Action 搜索接收同一个投影器，其中也包括从后代作用域发现的终端 Action。

## 持久化与恢复

新写入使用 `orchbee:web:keyboard-shortcuts:v21` 严格记录：

```json
{
	"version": 21,
	"overrides": [
		{
			"shortcutId": "workbench.search.all.open",
			"chord": { "code": "KeyY", "modifiers": ["mod", "shift"] }
		}
	]
}
```

只存储自定义值，并按快捷键 ID 排序。未知字段或 ID、重复项、未排序条目、非规范修饰符、不支持的代码、已存储默认值、过多条目以及有效冲突都会使完整 v21 记录无效，并恢复平台默认值。写入失败时，不可变的会话绑定仍保持活动，同时报告尚未保存，并在下一次合法修改时重试。恢复单项会移除其覆盖。

当 v21 不存在时，Web 会先按准确的旧目录与旧默认值读取 `orchbee:web:keyboard-shortcuts:v20`；如果 v20 也不存在，则依次尝试 v19、v18、v17、v16、v15、v14、v13、v12、v11、v10、v9、v8、v7、v6、v5、v4、v3、v2 和 v1。每次迁移都会把显式覆盖带入扩展后的目录，移除与当前默认值等价的覆盖，拒绝迁移后产生的冲突，并写入规范 v21 记录。v1 到 v20 的值原样保留，作为回滚快照。无效当前记录绝不回退到更旧记录，无效迁移源也绝不继续读取下一个更旧版本，因为任一回退都会掩盖数据损坏。回滚时会恢复旧版本能够理解的最新记录；只在 v21 中做出的编辑有意不对旧版本可用。

Browser Preview、Gateway Browser 和 Electron 各自针对自己的源本地存储创建并销毁一个控制器。Gateway 与 Electron 通过 `ProjectedTerminalSurfaceCapability` 传递控制器的窄绑定来源。事件处理器在分发时读取当前表，因此修改不会重新挂载 Workbench、替换附件，也不会重新打开 Host 或 Gateway 连接。

## 声明与证据

| 声明                                                                     | 确定性证据                                                                                                            |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------- |
| 一个有效目标只产生一个子终端、面板或标签页关闭操作                       | 活动目标解析器决策表测试                                                                                              |
| 确认、取消与提交继续由准备好的下层 Action 所有                           | 同时注册面板与标签页 Action 的 Workbench Action 集成测试                                                              |
| 活动 Workbench 绑定不依赖仍然存在的聚焦后代                              | document 捕获生命周期与 Web 应用集成测试                                                                              |
| 空 Tab 状态下新建 Tab 会创建并聚焦新终端，设置会抑制它                   | Web 应用焦点与设置作用域集成轨迹                                                                                      |
| 当前目录 Tab 创建使用精确的显示中可信 CWD 并保留源终端                   | 解析器、runtime、Action、菜单、快捷键、Settings、v16 迁移及精确 Desktop `terminal-parity/FS-15-current-directory-tab` |
| Apple 与标准平台默认值符合目录，且自定义编辑会影响分发                   | 快捷键定义、解析器与控制器测试                                                                                        |
| 空闲的平台默认终端 Paste 保留感知 MIME 的原生投递                        | 快捷键路由与安装器测试、终端图片粘贴测试及精确 Desktop `terminal-parity/IN-03-image-paste`                            |
| 活动的普通终端选择会保留原生粘贴与输入所有权                             | 投影终端选择/粘贴测试及精确 Desktop `terminal-parity/IN-18-native-selection-paste`                                    |
| 等待中的 Copy 与 Paste 工作不会接纳陈旧或重复快捷键输入                  | 剪贴板协调器与 Action 测试及精确 Desktop `terminal-parity/IN-13-selection-copy-paste`                                 |
| 方向聚焦会选择空间邻居，并在外边缘继续传递事件                           | 四面板解析器、Action 与快捷键分发测试                                                                                 |
| 循环聚焦遵循规范 Panel 顺序、回绕，并在不可用时继续传递                  | 生成式解析器、预备 Action、快捷键分发、Settings 与 v17 迁移测试                                                       |
| 按位置聚焦选择一个精确规范 Panel，且绝不钳制缺失位置                     | 生成式解析器、预备 Action、快捷键/Settings、v19 迁移及精确 Desktop `terminal-parity/FS-17-panel-position-focus`       |
| 可视聚焦与交换会选中一个精确的其他 Panel，且不泄漏已接管的输入           | 模型、Action、React、快捷键、Settings、v19 迁移及精确 Desktop `terminal-parity/FS-16-visible-panel-selection`         |
| 方向移动会交换精确相邻叶节点并保留源焦点                                 | 模型、有界探索、Action、快捷键、设置及精确 Desktop `terminal-parity/FS-13-directional-panel-movement`                 |
| 均分按同轴 Panel 比例分配范围且不改变标识                                | 模型、生成布局、Action、菜单、快捷键、设置、v15 迁移及精确 Desktop `terminal-parity/FS-14-panel-equalization`         |
| 键盘调整大小定位最近匹配分割并重新验证捕获标识                           | 解析器、状态、Action、React、设置、迁移、菜单与 Desktop 系统测试                                                      |
| 按位置选择遵循规范 Tab 顺序并在越界时继续传递事件                        | 分组与固定 Action 集成测试及快捷键分发测试                                                                            |
| 相邻选择遵循规范 Tab 顺序，且仅在存在多个 Tab 时回绕                     | 纯解析器、预备 Action 集成与快捷键继续传递测试                                                                        |
| 活动 Tab 移动通过一个下层 Action 在其固定分区内循环                      | 纯解析器、预备 Action 集成、快捷键/设置测试及 Desktop 系统轨迹                                                        |
| Tab 标题编辑保留动态元数据，并重新校验精确的被调用 Tab                   | Workbench 模型/Action/React 测试、存储迁移及精确 Desktop `terminal-parity/FS-11-tab-title`                            |
| 可见 Hints 消费一个聚焦绑定，且标签按键绝不会泄漏到终端输入              | 终端、Action、快捷键、Settings 与精确 Desktop `terminal-parity/FS-07-keyboard-hints`                                  |
| 显示回滚历史消费一个聚焦绑定并打开精确不可变查看器                       | 终端、Action、快捷键、Settings 与精确 Desktop `terminal-parity/FS-09-scrollback-viewer`                               |
| 菜单、Action 搜索和设置显示同一个当前绑定                                | 投影与渲染界面测试                                                                                                    |
| 快捷键设置可查找本地化文本、输入的按键别名与精确录制和弦，且不会编辑绑定 | 纯发现解析器与转换测试，以及渲染后的搜索/捕获集成测试                                                                 |
| 有效 v1 到 v19 记录迁移时不会丢失显式旧覆盖                              | 存储迁移测试，包括与新默认值的冲突                                                                                    |
| 无效当前记录或迁移源数据不会读取更旧记录                                 | 存储损坏与优先级测试                                                                                                  |

测试使用合成的平台标志、Action 作用域、存储、document 捕获、注入终端焦点与键盘事件。它们不主张操作系统全局快捷键行为或完整 Warp 兼容性。手动 Desktop 原生窗口焦点测试不包含在自动验证中。

## 维护检查

```sh
pnpm --filter @orchbee/keyboard-shortcuts typecheck
pnpm --filter @orchbee/keyboard-shortcuts test
pnpm --filter @orchbee/workbench test
pnpm --filter @orchbee/web typecheck
pnpm --filter @orchbee/web test
```
