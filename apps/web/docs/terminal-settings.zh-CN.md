# 终端设置

## 所有权和范围

本地 Desktop composition 还可以注入 Agent-integration management inventory 与 command；Web
拥有其设置与 reminder presentation，Main 保留所有 process 与 filesystem effect。

`@orchbee/web` 拥有相互独立的本地终端外观、终端交互、终端反馈和 macOS 终端输入首选项记录、其 Browser 存储编解码器、“设置”覆盖层，以及投影终端附件观察的稳定客户端源。它还拥有可选 Desktop 终端启动配置文件和终端默认设置能力的 Presentation 与非特权请求客户端；Electron Main 拥有这些记录、持久化、Host 提交和启动解析。Browser 支持的首选项记录只属于一个 Browser 源或 Desktop 渲染器用户数据分区。这些记录以及 Main 拥有的配置文件和终端默认记录都不属于共享 Workbench 文档，也不会发送到 Gateway 或远程终端。影响主题的首选项会派生一个完整运行时主题请求，经活动 Desktop 或 Gateway 投影传输到 Terminal Host；每个按键的 Option-as-Alt 同样只作为语义输入决策跨越边界，绝不是存储的首选项。

`@orchbee/terminal` 拥有源中立渲染器合约，该合约消耗派生的完整演示配置。主机框架元数据对于终端调色板、光标、VT 状态、终端颜色方案回复和 OSC 行为仍然具有权威。 Workbench 通过渲染器覆盖、光标呈现和应用程序设计令牌表面来协调一个完整的主机主题修订。 Web/桌面组合将旧的源中立 `defaultColorOverride` 保留为 `null`。

此功能可以分析粘贴到临时迁移报告中的 Ghostty 配置。它不会应用或保留该报告、读取 Ghostty 文件、改变主机配置文件、公开完整的调色板编辑器、使 Electron 窗口透明或跨客户端同步首选项。它还投射只读远程开发诊断。该投影既不拥有 SSH 进程，也不推断当前终端是否是远程的。

## 外观与交互偏好记录

该功能写入密钥 `orchbee:web:terminal-appearance:v2`。它的值是一个完整且没有未知字段的 JSON 对象：

```ts
interface StoredTerminalAppearancePreferencesV2 {
	readonly applicationTheme: 'light' | 'dark';
	readonly backgroundOpacityPercent: number;
	readonly customBackground: `#${string}`;
	readonly customForeground: `#${string}`;
	readonly fontFamily: string;
	readonly fontLigatures: boolean;
	readonly fontSize: number;
	readonly fontWeight: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
	readonly fontWeightBold: 100 | 200 | 300 | 400 | 500 | 600 | 700 | 800 | 900;
	readonly terminalDisplayTheme: 'host' | 'light' | 'dark' | 'custom';
	readonly terminalPadding: number;
}
```

版本 2 缺失时，一个有效的 `orchbee:web:terminal-appearance:v1` 记录只会在内存中迁移，并补入
新的产品默认值。迁移不会立即写入；之后每次语义写入都使用版本 2。版本 2 数据无效时绝不
回退读取版本 1。

字体族会被修剪，长度为 1–256 个字符。字号是 8 到 48 之间的整数。两个字重都是 100 到
900 之间以百为单位的整数。终端内边距是 0 到 32 CSS 像素之间的整数。自定义颜色必须是
`#RRGGBB` 并规范化为小写。背景不透明度是 10 到 100 之间的整数百分比。

独立的严格密钥 `orchbee:web:terminal-interaction:v1` 只包含 `copyOnSelect: boolean`。它拥有
独立控制器、持久化问题和源中性订阅。产品默认值是 `true`。

完整产品默认值为：

| 字段         | 默认                                                               |
| ------------ | ------------------------------------------------------------------ |
| 应用主题     | `light`                                                            |
| 字体族       | `"Orchbee JetBrains Mono", "Orchbee Symbols Nerd Font", monospace` |
| 字号         | `13`                                                               |
| 常规字重     | `400`                                                              |
| 粗体字重     | `700`                                                              |
| 字体连字     | `true`                                                             |
| 终端内边距   | `0`                                                                |
| 终端显示主题 | `host`                                                             |
| 自定义前景色 | `#141414`                                                          |
| 自定义背景色 | `#fcfcfc`                                                          |
| 背景不透明度 | `100`                                                              |
| 选中时复制   | `true`                                                             |

Workbench 通过同一个控制器公开增大、减小和重置字号 Action。增大与减小应用两点步长，并在 `8`
和 `48` 处限制；重置只恢复默认 `13` 字号，并保留其他所有首选项。发生变化的 Action 遵循普通的
完整快照发布与持久化工作流，边界请求或已经是默认值的请求保持不变。该行为作用于整个应用，而
不是单个 Panel。

终端显示主题从最新协调的主机状态中派生出一个完整的候选主题，而无需修改存储的自定义对：

| 主题     | 候选主机终端                                                                                      |
| -------- | ------------------------------------------------------------------------------------------------- |
| `host`   | 保留完整的当前主机配色方案、默认值、可为空的光标颜色和 256 色调色板。                             |
| `light`  | 设置方案`light`，默认为`#141414` / `#fcfcfc`；保留当前主机光标颜色和完整调色板。                  |
| `dark`   | 设置方案`dark`，默认为`#d4d4d4` / `#1e1e1e`；保留当前主机光标颜色和完整调色板。                   |
| `custom` | 从 `customForeground` / `customBackground` 设置默认值；保留当前的主机方案、光标颜色和完整调色板。 |

在主机、浅色和深色之间进行更改会保留自定义值。仅当选择“自定义”时，自定义颜色控件才可编辑。应用程序主题、终端显示主题、完整的固定叠加颜色以及主机指定的主题修订形成一个发布事务。成功的屏障会在同一 JavaScript 任务中更新 `data-theme` 之前提交每个已安装的终端。

## macOS Option-as-Alt 偏好

macOS 上的桌面渲染器还拥有 `orchbee:web:macos-terminal-input:v1`。严格记录仅包含`optionAsAlt: 'off' | 'left' | 'right' | 'both'`；产品默认为`off`。浏览器、网关、Windows 和 Linux 组合不会公开 Option-as-Alt 行并保留默认值；光标和输入在每个平台上仍然是稳定的目录目的地。无效或不可读的记录会失败到 `off`，而失败的写入会保留已接受的会话值并显示现有的持久性警告。

客户端跟踪物理 `AltLeft` 和 `AltRight` 所有权并解析每个已承认的关键事件的选定方。它不会重写提交的文本或根据 macOS 布局生成的字符进行猜测。完整的输入配置源会立即更新每个已安装的投影终端，而无需更换其附件。 `KeyInput.optionAsAlt` 然后仅携带该事件的语义决策； Terminal Host 仍然负责 Ghostty 的模式相关遗留、modifyOtherKeys 和 Kitty 键盘编码。

## 桌面本地终端启动配置文件

仅本地 Electron 组合物注入 `DesktopTerminalLaunchProfileControl`。其初始快照公开了一个有界规范目录、当前默认身份、目录是否来自内置后备存储或持久存储、可选的持久性问题以及当前平台的登录 shell 功能。浏览器预览和经过身份验证的网关组合不受控制，仅显示剩余的计划配置文件行。渲染器永远不会接收存储路径、节点错误、文件系统原语或启动功能。

每个配置文件都有一个稳定的身份、一个用户可见的名称以及一个命令选择：

- 系统shell用login-shell决定。仅当主报告支持时才启用登录模式，并且在 Windows 上不可用。
- 具有一个绝对平台路径和文字参数的有序向量的自定义可执行文件。参数不会被标记、引用、内插或连接到命令字符串中。

稳定的内置配置文件使用本地化显示标签，而不是持久化产品文案。用户自定义名称会作为精确用户数据保留；重名不会合并身份。编辑器可以复制所选命令来创建新配置文件、重命名、更改命令与参数、保存草稿、设为默认项，或在确认后删除非默认配置文件。默认配置文件和最后一个配置文件都不能删除。Reset 需要确认，并且只有在 Main 持久提交后才会把完整目录替换为内置默认值。操作待处理时会禁用相互竞争的变更，并显示有界的本地化失败；无效草稿绝不会发送变更。提交失败会保留用户草稿，以便修正或重试，而无需重新输入。

创建、编辑、默认选择、删除和重置使用显式控件。它们不会改变正在运行的终端。新配置的终端仅捕获默认配置文件一次，并且其持久绑定在重试和恢复期间保留具体的启动。配置文件解析仅更改可执行文件和参数；环境、机密、演示文稿、CWD、项目或工作区数据、一次性配置文件选择、远程连接和 shell 启动文件突变仍不在此功能范围内。不可用的自定义可执行文件在终端生命周期中会明显失败，而无需系统 shell 回退。完整的Main模型在[`../../desktop/docs/terminal-launch-profiles.md`](../../desktop/docs/terminal-launch-profiles.md)中具有权威性。

## Desktop 终端默认值

只有本地 Electron 组合会注入 `DesktopTerminalDefaultSettingsControl`。其快照包含完整且有界的
Host 默认值、持久化问题、来源、待处理状态和有界失败代码。“终端”页面把
`terminal.scrollback` 显示为 `0` 到 `1_000_000_000` 之间的十进制字节数，并说明正在运行的
终端会保留当前上限。Browser Preview 与认证后的 Gateway 组合不会收到此控制，同一行仍显示为计划项。

渲染器会先验证草稿，再发送完整更新。Host 拒绝时保留旧值；Host 接受后若持久化失败，会
保留可见的会话值并说明尚未保存。之后再次提交会重试持久化。此客户端绝不会发送实时终端
配置更新或驱逐历史记录。权威 Main 事务见
[`../../desktop/docs/terminal-settings.md`](../../desktop/docs/terminal-settings.zh-CN.md)。

## 读取、提交和恢复模型

启动时通过注入的 `getItem` 各读取一次当前本地密钥，并把 `JSON.parse(...)` 直接解析为严格 Zod 模式。只有版本 2 缺失时，外观设置才会额外读取版本 1。记录缺失时使用产品默认值且不警告。读取异常、格式错误 JSON、未知字段、不支持的形状或越界值会使用完整默认值并记录可恢复的持久化问题；禁止部分恢复。

一个语义更新遵循以下顺序：

1. 验证完整的候选偏好记录。
2. 派生不可变的完整首选项快照、源中性 Presentation 配置，并在影响主题的字段变化时派生完整运行时主题候选。
3. 提交内存快照。仅当派生值变化时发布 Presentation 配置，并在主题派生值变化时启动一个最新优先的运行时主题屏障。
4. 尝试一次写入完整的偏好记录。

字体族、字号、常规与粗体字重、连字、内边距和不透明度会实时更新已挂载的终端渲染器，
无需替换附件。独立的交互控制器对 `copyOnSelect` 遵循相同的验证、会话提交、发布和写入顺序。

对于运行时主题屏障，每个已挂载投影终端先预检其渲染器覆盖。预检失败不会发送 Host 变更。随后，已就绪参与者完成 Host compare-and-set 或协调，并等待携带确切已提交 `themeRevision` 的暂存帧。每个参与者都返回已准备句柄后，控制器同步检查全部句柄，提交每个终端的存储/覆盖/光标组合，并更改应用主题。任何失败、取消、取代、隐藏/暂停延迟或失去资格，都会保持先前的完整应用呈现可见，并中止其他全部已准备句柄。某个对等方失败前已经提交的 Host 修订仍会被隔离，不会回滚或部分显示；后续主题请求、参与者注册变更或处置会取代该阶段。没有已挂载参与者时，`data-theme` 会立即应用。

视觉事务不包括浏览器存储或内存中首选项快照。因此，写入失败会像以前一样保留已接受的会话首选项，而呈现失败不会重写存储的选择，即使先前的完整主题仍然可见。

同步侦听器重入按先进先出顺序序列化为不可变的呈现快照。较新提交的修订版不能被较旧修订版的后续存储写入覆盖。发布期间的处置会停止剩余的回调并阻止任何挂起的发布或持久性工作。

无效的候选者不会更改内存、主题、存储或订阅者。当持久性健康时，有效的无操作不会执行任何操作。写入异常不会回滚会话值；快照报告该设置已应用于此会话，但未保存。下一个有效更新将重试完整写入并在成功后清除该问题。一次终端准备失败会将每个已安装的终端和应用程序表面保留在其先前提交的完整呈现状态，而不回滚首选项。

重置行为如下：

- 不同的首选项成为完整的产品默认值，编写一次，并在终端派生值发生更改时最多发布一个终端演示快照。
- 存在持久性问题的默认首选项会重写默认记录一次，而无需发布终端演示快照。
- 具有健康持久性的默认首选项保持不变，并且不执行任何写入或发布。

处置会清除订阅者并拒绝处置后的后续语义更新。任何后期更新都无法在处置后更改主题、存储或演示源。

## 有效的终端颜色和不透明度

Host 运行时主题替换是该设置功能使用的唯一颜色变更路径。提交之后，帧元数据和已解析的调色板单元仍由渲染器掌权。渲染器的 `defaultColorOverride` 始终为空：默认 token 使用 Host 默认前景/背景；显式 RGB、投影后的 ANSI/调色板颜色、`none` 和显式黑色保留各自的 Host 投影值。反色会交换已经解析的显示角色。默认下划线使用 Host 前景色；光标使用 Host 光标颜色，缺失时回退到 Host 定义的前景色。

背景不透明度源自`backgroundOpacityPercent`； `allowTransparency` 不是第二个用户偏好。在 100% 时，渲染器清晰且终端根不透明。从 10% 到 99%，渲染器透明地清除默认背景，并且终端根使用所选的 Alpha 呈现有效背景。显式背景、反向单元格、选择和搜索突出显示仍然不透明。该实现不得使用全根 CSS `opacity`，因为终端文本必须保持不透明。

## 叠加和聚焦模型

设置体验是应用程序拥有的全窗口覆盖，而不是路线。保持工作台安装状态可以保留终端表面和附件的特性。首次打开时，它会捕获仍然连接的活动元素并聚焦“设置”搜索。重复打开不变，不取代返回目标。当设置打开时，工作台子树实际上是惰性的，因此它的终端和操作快捷方式无法使用设置输入。滚动内容视口保持连续焦点顺序并绘制插入焦点指示器，因此即使每个设置控件都处于禁用的计划预览状态，类别仍保持键盘可滚动状态。

返回会关闭“设置”。Escape 会先清除非空搜索，只有查询为空时才关闭“设置”；关闭时依次恢复捕获元素、“设置”Action 目标和 Workbench 根。Desktop 可以注入语义 Agent 集成、Browser 访问、本地 shell 配置文件和终端默认值控件。现有 Browser 访问对话框位于“设置”上方；关闭后焦点返回其“设置”入口，同时“设置”保持打开。Browser 和 Gateway 组合不会收到此 Desktop 能力包。

交互结构遵循已接受的 Cursor 与 Coze 参考，但不复制任一品牌：紧凑的设置导航栏负责返回、搜索、以图标为主的产品结果类别与当前选择；居中的滚动内容列负责一个类别标题，以及稳定排列的分区标签、低干扰分组行、说明和右对齐控件。信息密集的诊断、迁移输入与 Shell 设置沿用相同的表面、圆角、字体、间距与语义边界词汇，而不另造类别专用卡片。窄宽度下，导航栏会变成可横向滚动的顶部导航，设置控件堆叠在说明文字下方。由 Browser 支持的外观、输入、快捷键与反馈变更会立即应用；Shell 配置文件草稿需要显式保存，因为 Main 所拥有的持久目录变更是独立事务。参考品牌、文案、类别和像素几何都不是兼容性目标。

## 设置信息架构和计划目录

设置栏对产品结果进行分组，而不是公开实施包。当前的信息架构足够稳定，可以让未来的设置进入，而无需创建另一个顶级表面：

| 集团       | 类别                                     |
| ---------- | ---------------------------------------- |
| 应用       | 一般、外观                               |
| 终端       | 文本、颜色、光标和输入、交互、终端       |
| 工作空间   | Windows 和选项卡、配置文件、键盘快捷键   |
| 集成       | Agent 集成、Shell 集成、远程开发         |
| 数据和访问 | Ghostty 迁移和功能门控桌面浏览器访问条目 |

外观主题、终端字体族与字号、常规与粗体字重、连字、内边距、有效默认颜色、默认背景不透明度、选中时复制、重置、macOS Option-as-Alt、键盘快捷键、Shell 集成、远程诊断、迁移报告、浏览器访问入口、受能力门控的 Desktop 启动配置文件编辑器和受能力门控的 Desktop 回滚缓冲区默认值，均保留现有行为和所有权。其余“常规”“外观”“文本”“颜色”“光标与输入”“交互”“终端”“窗口与标签页”，以及配置文件的外观、连接和自动化各行，共同组成一个已本地化的规划目录；它提炼自 Ghostty、WezTerm、Kitty、Alacritty、Windows Terminal、iTerm2、Tabby、Konsole、GNOME Terminal 和 Foot 中反复出现的设置。该目录覆盖启动与恢复策略、窗口呈现、排版与字形塑造、调色板与对比度、光标与键盘协议、剪贴板与鼠标行为、Shell 与仿真策略、历史记录与反馈、窗口与标签页，以及超出当前获准启动配置文件范围的配置文件功能。缺少 Desktop 终端默认设置能力时，回滚缓冲区行仍属于计划目录，不能改变 Host 状态。

计划的行被显式标记，仅公开禁用的预览控件，并且从不读取或写入首选项。实现一个需要有自己公认的行为权威、模型、持久边界、平台能力和验证。因此，可见行是产品方向，而不是兼容性或可用性声明。

中性分组设置使用一个无外轮廓、带圆角的轻微表面。只有行分隔线、真实输入边界、焦点指示器，以及语义状态或危险提示保留边框。这样既让各类别保持统一、低干扰的视觉系统，又保留有意义的状态边界。

## 设置发现和导航模型

设置搜索是全局发现界面，而不是页内过滤器。其确定性目录包含每个可用类别以及每个当前或明确计划的设置，具有稳定的标识符、所属类别、本地化标签、可选部分、描述和搜索关键字。能力门控类别和设置仅在其组合能力存在时才进入目录。目录永远不会读取首选项值，并且搜索永远不会更改控制器、存储、主机、终端附件、迁移报告或远程环境。

查询被修剪、Unicode 规范化、拆分为非空术语，并在有效区域设置中不区分大小写进行比较。类别结果与其类别标签相符。设置结果要求每个术语与设置标签、部分、描述或显式关键字匹配。直接标签匹配排名在部分、描述和关键词匹配之前；可用的设置比附近的计划匹配具有有限的优先级，每个计划结果都被标记，并且原始信息架构顺序打破了剩余的联系。空查询保留分组的导航栏。非空查询会用一个全局结果列表替换该栏，同时当前内容页面保持完整且不变。零匹配在轨道中呈现局部空状态，而不是清空内容页面。搜索结果仍然是一个带标签的列表框；零匹配投影包含一个禁用的、未选择的选项，并带有礼貌的实时公告，保留组合框关系而不创建可操作的结果。

拥有的交互状态是以下之一：

| 状态       | 查询 | 结果选择                       | 轨道投影     |
| ---------- | ---- | ------------------------------ | ------------ |
| 浏览       | 空   | 无                             | 分组类别     |
| 搜索结果   | 非空 | 一个有界索引，最初是第一个结果 | 全球搜索结果 |
| 搜索无结果 | 非空 | 无                             | 局部空状态   |

转换是确定性的：

| 输入               | 守卫           | 结果                                                   |
| ------------------ | -------------- | ------------------------------------------------------ |
| 编辑查询           | 任意           | 重新计算全局投影并选择其第一个结果（如果存在）。       |
| 向下箭头或向上箭头 | 搜索结果       | 将有界选择移动一；当前的内容页面和每个偏好保持稳定。   |
| 输入               | 搜索结果       | 激活选定的结果。                                       |
| 指针激活           | 可见的结果     | 激活该结果。                                           |
| 激活类别结果       | 任意           | 选择类别，清除查询，恢复分组导航，并将内容滚动到顶部。 |
| 激活设置结果       | 其类别仍然可用 | 选择类别，清除查询，恢复分组导航，并显示其稳定目标。   |
| 逃亡               | 查询非空       | 清除搜索、恢复分组导航、保持设置打开并保留当前类别。   |
| 逃亡               | 查询为空       | 遵循设置覆盖关闭和焦点恢复合同。                       |

移动或激活结果时，搜索输入会保留键盘焦点。结果激活可能仅滚动设置栏或内容视口；它从不聚焦设置控件或调用其操作。通过正常轨道更改类别不会清除任何用户数据，并从该类别的顶部开始。

采用的参考投影是在2026年8月17日在macOS上的光标3.14.7中观察到的。采用的范围是全局类别和设置发现、查询期间的轨道替换、打字时稳定右侧内容、显式空结果、箭头导航、Enter激活、Escape清除、激活后查询清除、类别恢复和目标显示。明确排除光标品牌、副本、目录内容、模糊搜索实现、结果限制、像素几何、动画计时和排名关系。

## Desktop Agent 集成

只有本地 Electron composition 会注入 `AgentIntegrationManagementControl` 与
`AgentIntegrationReminderController`。Browser Preview 与已鉴权 Gateway client 会省略 Agent 集成
category、对应 search entry 与 reminder。Renderer 消费一个规范的十 entry snapshot，并显示 curated
Agent name、bounded state/failure 与共享隐私说明。它不会收到 Agent command output、executable path、
adapter resource path、filesystem capability 或 remote-host authority。

打开设置、选择 Agent 集成、渲染 row 与搜索都是只读的。刷新请求新的 Main 只读 inventory。开启与
关闭使用 button，而不是 optimistic switch；一次点击发送一个 semantic request，row 跟随 Main 重新检查
后的 snapshot。Pending work 会禁用竞争变更。Conflict、unsupported/manual-only entry、ambiguous
inspection 或不可用的本地 Agent 都不能开启。Remote host 会明确说明只能手动设置；renderer 绝不写入
terminal input 或尝试远端设置。

Non-modal reminder 只对本地可用、`managed-local`、没有 issue 且 state 为 `not-installed` 或
`disabled` 的 entry 生效。Canonical order 选择第一个符合条件的 entry，controller 每个 application
session 最多 offer 一次 reminder。渲染以及**查看集成**都不执行 integration mutation；查看会隐藏
reminder 并打开该设置 category。显式关闭会把一个 Agent ID 写入严格且有界的
`orchbee:web:agent-integration-reminders:v1` record。设置中的关闭会在请求 Main 前记录同一 dismissal；
经过验证的成功开启会移除它。缺失或无效 storage 会回退为空 dismissal；写入失败仍会保留当前 session
行为。

Desktop Main 的准确 effect、reconciliation、conflict、rollback 与远端排除以
[Desktop Agent 集成指南](../../desktop/docs/agent-integrations.zh-CN.md)为准。

## 远程开发诊断

远程开发类别记录了桌面启动基线 `TERM=xterm-256color` 并公开了一个常量 POSIX-shell 诊断命令。该命令报告版本标记、观察到的 `TERM`、`COLORTERM` 和 `TERM_PROGRAM`、TERM 是否与基线匹配，以及一个 terminfo 结果：

| 结果                           | 含义                                      |
| ------------------------------ | ----------------------------------------- |
| `TERMINFO=term-unset`          | 远程 shell 没有 TERM 值。                 |
| `TERMINFO=unsafe-term`         | TERM 无法安全打印或传递到 `infocmp`。     |
| `TERMINFO=infocmp-unavailable` | 远程 shell 找不到 `infocmp` 可执行文件。  |
| `TERMINFO=available`           | `infocmp "$TERM"` 成功。                  |
| `TERMINFO=missing`             | `infocmp` 存在，但无法解析观察到的 TERM。 |

该命令不包含用户插值，并在一个子 shell 内运行其帮助程序和临时值。它仅读取环境值，打印最多 128 个与保守的显示安全字符集匹配的字符的值，检查 `command -v infocmp` 并评估 `command infocmp` 的退出状态。不安全或过大的值会收到稳定的标记，而不是作为潜在的终端控制输入进行回显。它不会调用 SSH、访问网络、安装 terminfo、创建目录、读取配置文件、写入远程状态或在用户的交互式 shell 中留下帮助程序函数。设置不执行或粘贴命令；只读字段使用正常的文本选择，因此用户仍然负责选择在何处以及是否运行它。

只有在 `ssh` 仍是 file、没有更早的 alias 或 function 拥有该名称、存在私有 local Agent-observation socket，且 stdin 与 stdout 都是交互式 terminal 时，显式 source 的 shell integration 才会定义 `ssh` wrapper。Configuration、control、仅转发与 version/query mode 保持不变。符合条件的调用会保留全部用户参数，只增加 `StreamLocalBindMask=0177`、`StreamLocalBindUnlink=yes`，以及从远端随机 `/tmp/orchbee-agent-v2-*.sock` 到 local Desktop socket 的 reverse Unix-socket forward。OpenSSH authentication、显式参数与用户配置仍然是最终依据。Orchbee 不添加 `SendEnv` rule、不持久化 host 或 credential、不安装 remote file，也不维护 remote cache。标准请求的 SSH PTY 仍根据 OpenSSH 行为携带 TERM。OSC 7 hostname、process title 与 terminal output 不是 remote-identity authority。

## Shell 集成与完成提醒

Shell 集成类别说明 Desktop 会为新建且受支持的本地 Zsh、非登录 Bash 和 Fish 终端自动加载
Orchbee 拥有的集成。精确的 `source` 命令仍作为不受支持的启动形式及其他客户端的手动回退。该界面
绝不读取、创建、编辑或备份 Shell 启动文件，也绝不执行显示的命令。Zsh 使用 `preexec` 和
`precmd`；Fish 使用 `fish_preexec` 和 `fish_postexec`；Bash 需要 4.4 或更高版本。macOS 自带的
Bash 3.2 明确不受支持。删除用户添加的 `source` 行并启动新 Shell，只会撤销该手动回退。

Desktop 从可信的应用资源路径为新的 POSIX 终端提供 `ORCHBEE_SHELL_INTEGRATION_DIR`。launch
profile 选择完成后，受支持的本地启动形式会使用一次性 Zsh、Bash 或 Fish loader；loader 会先恢复
Shell 的正常启动路径，再 source 共享集成。不受支持的形式保持不变，并仍可使用手动回退。脚本发出
标准 OSC 133 命令边界和 OSC 7 当前目录状态，但不传输命令文本。环境变量不能替换该资源路径；自动
加载也绝不改变运行中或恢复的终端、Windows 终端、远程连接或用户启动文件。Terminal Host 只投影
最新的正序列、持续时间、完成时间戳和标准化退出代码。

对于显式安装的 Agent observation adapter，shell integration 还会启用上文所述、符合条件的 SSH reverse-socket path；remote adapter 会把有界 OSC 777 correlation hint 直接写到解析出的 direct、tmux、Zellij 或 nested-mux outer TTY。Terminal Host 只投影有界 semantic signal。

Terminal feedback 使用单独的密钥 `orchbee:web:terminal-feedback:v2`，其严格 JSON 值包含 `agentNotificationMode`（`never` 或 `hidden`）、`completionReminderMode`（`never`、`unfocused` 或 `always`）和 `completionReminderMinimumSeconds`（1 到 3,600 之间的整数）。三者默认分别为 `never`、`never` 与 5 秒。有效的 version-1 completion-reminder record 会迁移，并保持 Agent notification 关闭。无效或不可读的存储会回退到完整默认值并显示可恢复警告；写入失败会保留当前 session 值，并报告尚未保存。重置会恢复全部三个默认值。

每个客户端根据终端身份和肯定的完成顺序进行重复数据删除，忽略超过 60 秒或未来超过 5 秒的事实，并将一个终端的系统通知限制为每 30 秒一次。 `unfocused` 要求文档或终端根目录缺少焦点。只能通过专用的“设置”按钮请求浏览器通知权限；终端输出、启动、存储恢复和配置分析从不要求它。当系统能力不可用时，合格事件仍呈现终端内部的本地化状态。通知仅包含持续时间和退出状态，而不包含命令文本、参数、目录、环境、输出或用户身份。

Agent 状态通知使用单独的精确 semantic-event subscription，而不是 terminal screen inference。设置默认为关闭；`hidden` 只在所有 navigable attachment 当前都被 local Tab、split/child presentation 或 inner tmux/Zellij pane 隐藏时，准入 attention、successful outcome 与 failed outcome event。System permission 必须已经 granted；event path 绝不会请求 permission。Copy 只根据有界 Agent identity 与 event kind 生成，是本地化且 curated 的，绝不包含 prompt、command、tool content、output、path 或 error text。每个 semantic event identity 只处理一次。激活 notification 会聚焦 application 并选择其精确 Tab 与 pane，但不能回答 question 或授予 approval。

当前工作台卸载完全属于非活动选项卡的终端表面。已安装但未聚焦的终端可以立即提醒，并且重新连接可以在 60 秒新鲜度窗口内考虑完成事实。这并不是完整的后台监控声明：当 Surface 在该窗口之外保持未安装状态时，完成不会产生立即通知。未来的全 Tab command-completion monitor 需要单独的轻量级 Workbench/Host event subscription，而不是 hidden renderer。Agent observation 已使用这种轻量级 application-level subscription，因此不受此 command-completion 限制；完全断开的 SSH delivery 仍不在范围内。

## Ghostty 迁移报告

设置迁移类别分析用户粘贴的文本，无需读取文件、调用 Ghostty、记录源、更改首选项或写入浏览器存储。仅当安装该类别时，源及其报告才存在；离开类别或关闭设置都会清除两者。没有应用或导入操作。

该分析器是 Ghostty 修订版 `91f66da24527fa02d92b5fd0b41cd020f553a64c` 的有意限制的兼容性投影，而不是 Ghostty 配置解析器。它识别换行符分隔的 `key = value` 赋值、周围的空格、整行注释、CRLF、第一个等号和一对周围的双引号。它不会加载 `config-file`、解析主题、合并文件、运行命令或模型 CLI 优先级和平台默认值。

每一个非空、非注释行都会产生一个按源排序的结果：

| 寻找   | 含义                                                                       |
| ------ | -------------------------------------------------------------------------- |
| 支持   | 归一化值有一个无损电流Orchbee设定目标；它尚未被应用。                      |
| 手册   | 存在附近的用户结果，但无法安全地表示 Ghostty 值或组合语义。                |
| 被忽略 | 该行被取代、重置源默认值或位于迁移白名单之外；未知的密钥也可能是拼写错误。 |
| 无效   | 有界分析器无法解释该行或值。其他有效线路仍会收到调查结果。                 |

仅支持这些最终有效值：

| Ghostty 钥匙          | Orchbee 目标         | 支持的源值                                                                        |
| --------------------- | -------------------- | --------------------------------------------------------------------------------- |
| `font-family`         | 终端字体系列         | 恰好一个活动的非空系列，最多 256 个字符，没有 CSS 列表分隔符                      |
| `font-size`           | 终端字体大小         | 8 到 48 之间的整数的十进制值                                                      |
| `foreground`          | 自定义前景           | 三位或六位十六进制 RGB，带或不带 `#`；需要自定义显示主题                          |
| `background`          | 自定义背景           | 三位或六位十六进制 RGB，带或不带 `#`；需要自定义显示主题                          |
| `background-opacity`  | 背景不透明度         | 从 0.1 到 1 的值，其百分比为整数，除非有效的 `background-opacity-cells` 为 `true` |
| `macos-option-as-alt` | 使用 Option 作为 Alt | `false`、`true`、`left` 或 `right`，映射到关闭、两者、左或右                      |

后来支持的标量分配取代了早期接受的粘贴文本中相同键的分配。 `font-family` 保留了 Ghostty 的空值重置和可重复后备列表含义；多个活跃系列是手动的，而不是被猜测到 CSS 列表中。命名颜色、命名或浅色/深色主题文件、`window-theme`、键绑定、工作目录、包含的配置、字体样式详细信息和单元格不透明度语义都是手动的。非白名单诊断会忽略其他键。

支持的颜色查找标识一个无损值目标，而不是完整的主题转换。报告告诉用户选择自定义显示主题并保留当前的其他 Orchbee 自定义颜色；它永远不会发明粘贴文本中不存在的对应颜色。

当超过 262,144 JavaScript UTF-16 代码单元、4,096 行或一行上 4,096 个代码单元时，分析会拒绝完整源，而不是截断它。这些是 Orchbee 资源边界，而不是有关 Ghostty 字节限制的声明。

## 所需证据

- 存储测试涵盖缺失、格式错误、未知字段、版本 1 内存迁移、无效版本 2 的优先级、越界、读取异常、写入失败后成功、重置修复和健康无操作行为。
- 源测试证明完整的单次 Presentation 发布、独立交互发布、实时附件更新、应用主题隔离、终端设置隔离、稳定订阅和处置。
- macOS 输入测试涵盖严格的存储恢复、所有四个选项侧值、实时源发布、物理侧分辨率、安装终端更新以及禁用和启用语义标记的主机编码。
- 渲染器测试涵盖主机和覆盖模式、显式黑色/RGB/调色板、反转、下划线、光标、Canvas 2D 和 WebGPU alpha 以及未更改的主机元数据。
- 确定性搜索测试涵盖目录身份和能力门控、本地化类别、部分、标签、描述和关键字匹配、直接匹配排名、多术语标准化、稳定排序和空结果。组件测试涵盖语言环境、全局跨类别结果、查询期间的稳定内容、指针和键盘激活、分层转义、目标显示、禁用的空结果选项、键盘可滚动内容、无效草稿、立即更新、重置、只读远程诊断、迁移分析和处置、惰性、幂等打开、焦点恢复和嵌套浏览器访问行为。故事浏览器审查涵盖了每个类别以及浅色和深色主题中的填充、计划和空搜索投影。
- 终端启动配置文件协议与客户端测试覆盖严格快照、关联、至多一项待处理变更、格式错误记录、失败投影和处置。设置组件测试覆盖原样保留的自定义可执行文件参数、只影响未来终端的边界、创建与默认选择、删除约束、重置确认、平台能力和本地化失败。
- 终端默认设置协议、Main 控制器、存储、broker、渲染器客户端、组合和组件测试覆盖严格边界、十四端口 bootstrap 顺序、Host 优先提交、Host 拒绝、会话可见的写入失败、重试、仅未来终端捕获、能力门控、本地化验证和可见失败。TLA+ 模型检查不实时驱逐。
- 终端反馈模型测试涵盖默认关闭行为、严格的持久性恢复、持续时间和焦点策略、过时的和未来的事实、序列重复数据删除、通知速率限制、显式权限请求和处置。组件测试涵盖两个区域设置、所有三个手动源命令、Bash 边界、提醒控件、权限状态和重置。
- Agent-integration protocol、Main adapter/controller/broker、renderer client、composition、storage、设置与 reminder 测试覆盖 canonical ordering、只读启动、一个 pending mutation、准确 entry conflict 与 disable、reconciliation、malformed message、显式用户操作、per-Agent dismissal、session offer limit、远端排除、本地化以及不自动 mutation。TLA+ model 检查 serialization、precommit preservation、reconciliation、reload 与 disposal safety。
- 迁移分析器测试涵盖可接受的语法、精确目标边界、标量取代、字体重置和后备列表、不透明单元格交互、畸形行、资源限制以及每个分配一个查找的排序，而不依赖于 React 或存储。
- 浏览器、网关和桌面组合测试证明共享源、仅限本地配置文件功能和设置条目；桌面 Electron 见证人证明现有的投影终端在打开和关闭“设置”时保持连接状态。当前的macOS系统见证人额外启动了一个具有隔离密钥和配置的临时环回`sshd`，然后证明普通的SSH PTY接收到`TERM=xterm-256color`，解析其terminfo条目，并且不接收`COLORTERM` 或 `TERM_PROGRAM` 无显式转发。一个单独的 macOS Electron 见证人获取已发送的 Zsh 资源，观察一个真实的语义完成和本地化提醒，并证明预计的附件身份未更改。
