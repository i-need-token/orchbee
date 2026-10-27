# Windows CLI 配置

## 文件与校验

Windows CLI 创建操作读取 `APPDATA/Orchbee/config.toml`，或 `ORCHBEE_CONFIG_FILE` 指定的绝对文件路径。默认文件不存在时保持默认行为；显式文件不存在则报错。位置变量无效或为空、文件无法读取、目标是目录、文件超过 64 KiB、UTF-8/TOML 无效，均在创建前构成致命错误。成功解析的文档逐字段接纳：非法的受支持叶字段使用已有安全默认值，非法 profile 使用该 profile 的已有安全默认值，未知字段则忽略。读取器将每次回退作为警告报告，且不回显值或命令。读取和检查不会创建配置目录或文件。

```toml
[terminal]
new_cwd = "follow"
focus_new_panel = true

[worktrees]
directory = "~/Projects/worktrees"

[update]
channel = "stable"
manifest_check = true
```

只有本文记录的配置段和键会影响 Orchbee，包括下文的 `[keys]` 与 `[ui]` 配置族。未知配置段、字段和动作会警告并忽略；加载配置绝不会启用插件、安装 Provider 或运行配置命令。

`terminal.new_cwd` 接受 `follow`、`home`、`current`、`workspace` 或路径。空值表示 `follow`。路径使用 Windows CLI 路径适配器：相对路径基于配置文件父目录，波浪线路径使用 Windows 主目录，拒绝有歧义的驱动器相对路径、UNC 和设备路径。非法路径仅让该叶字段回退。缺少 `APPDATA` 时仍可使用默认值；显式值无效时不会静默回退。

警告使用有界的规范原因语法和封闭的处置词汇，只携带有界的规范点分字段路径。警告按字段和原因去重、排序。一份结果最多携带 128 条警告及精确的省略数量，因此截断始终可见。未知路径段会转义并在 UTF-8 边界截短；不同原始字段在显示截短前去重。

### 默认配置输出

`orchbee --default-config` 是独占式离线调用。参数解析会在读取任何环境变量或发现运行时前选中它，且不会激活 Session。它向 stdout 写入确定性的 UTF-8 TOML，并以一个换行结尾。固定英文注释列出已接纳设置的默认值、选项和用途；使用英文可让此可复制配置产物在不同 CLI 语言环境下保持一致。已注册的 Client 动作会以注释按键数组列出，没有默认绑定的动作显示为 `[]`；只有取消注释的行才会成为覆盖，因此不改动生成文件会继续沿用 Client 持续演进的默认值。动作行由 Client 使用的同一默认绑定注册表生成。参考还说明自定义命令规则。Settings 独立于原生配置文件管理共享偏好。`config check` 可无警告地接纳生成结果；Windows CLI 解析后的有效设置和按键绑定与配置文件缺失时相同。共享偏好记录不存在时返回规范默认值，与生成配置文件无关。

## 命名 chrome 主题

共享的[应用外观偏好](../../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)
支持浅色（`light`）、深色（`dark`）和系统（`system`）；系统是通用默认值。在原生 TUI 中，
系统模式跟随宿主默认背景，并在缺少宿主背景观测时使用终端原生对比度。共享偏好拥有应用外观。
[原生主题](native-themes.zh-CN.md)定义极性、颜色能力、所有 chrome 消费界面、共享重载及回退。

原生配置文件没有主题设置：`[theme]` 表属于未知配置段，会警告并忽略；`config check` v1 details
和 Session 配置 v1 快照都不携带主题字段。

## 创建策略

| 输入                                 | 结果                                                                              |
| ------------------------------------ | --------------------------------------------------------------------------------- |
| 显式 CLI cwd 或 worktree 路径        | 覆盖对应的有效配置                                                                |
| Workspace、Tab 或 Panel 创建省略 cwd | 应用 `terminal.new_cwd`，随后执行已有的来源目录策略                               |
| 已配置根目录且省略 worktree 路径     | 在任何 Project 或 Git 修改前查询来源仓库，选择 `root/repository-name/branch-slug` |
| 未配置根目录                         | 保持已有的同级检出路径                                                            |
| Resume                               | 使用操作已保留的路径和分支，不重新加载配置                                        |

分支目录名将 ASCII 字母转为小写，将连续非字母数字字符压缩为一个连字符，去掉首尾连字符，空结果使用 `worktree`。自动分支仍为 `worktree/<operation-id>`，并固定在提交意图中。只读仓库查询与创建使用相同来源和信任策略。Server 仍复核来源并独占所有 Git 和规范 Workspace 副作用；查询不构成文件系统事务。显式路径省去额外查询。每次调用只读一次配置，并在应用显式覆盖前完成校验。

创建设置作用于 Windows CLI 创建。旧的显式空 Workspace 命令、TUI 直接创建、既有终端、Server 配置重载和键位仍由各自契约约束；所有创建入口都从下述共享启动配置获取默认程序。

## 检查与正确性

Windows `config check` 版本 1 为 `user-config` 检查增加已接纳的警告列表和省略数量。带警告的结果使用 `warning` 状态，整体文档仍成功并以 0 退出。致命文件或环境接纳错误使用 `fail` 状态并以 1 退出。检查报告选定文件、存在状态和规范化有效配置，文本输出以一行 `Config file:` 结尾，以可复制路径给出该文件；致命的创建配置继续使用独立的 `orchbee.cli.failure.configuration` 信封。来源发现失败会在首个 Project 创建前停止配置路径创建，不重试修改。

共享 Windows 设置读取器拥有有界文件检查、确定性的逐字段接纳和规范化；创建流程拥有优先级及已有副作用回执。读取没有写入器或可变缓存状态；下述显式恢复操作负责配置修复。核心证据覆盖格式错误和重复 TOML、字段局部回退、稳定且有界的警告、相对及波浪线路径、覆盖优先级和目录名向量。Windows 公开 IPC 轨迹覆盖有效启动 cwd、配置检出意图、发现失败后无修改、显式路径不查询及无文件时行为不变。完整应用验收为独立关卡。

## 规则更新策略

`[update] channel` 仅接受 `stable`（默认）或 `preview`，与 `manifest_check` 相互独立。`orchbee channel show [--format text|json]` 读取完整共享配置，不创建文件、不激活 Session、不访问网络。文本仅为频道标记；`orchbee.channel.show` v1 报告频道、default/file 来源、选定配置路径和文件存在状态。默认文件缺失时使用 stable；显式文件缺失或配置非法时失败。这是期望的发布频道偏好，不代表当前安装 cohort 的事实。Windows `config check` v1 和配置失败 v1 接受 `update.channel` 诊断；旧 schema 保持封闭。

`[update] manifest_check` 是 TOML 布尔值，默认 true，来源为 `default`／`file`。
Windows `config check` v1 增加 `manifestCheck` 和 `manifestCheckSource`。
Client 激活校验文件后只传递配置路径和显式文件标志。托管 Server 在任何进程或持久副作用之前，
于该冻结位置读取一次完整的权威版本 0 候选；直接启动通过同一设置所有者确定位置并读取一次。
cohort 必须匹配。
`read_user_settings_at` 在冻结位置支持有界显式重载，保留默认文件缺失与显式文件缺失的
差别，不创建配置文件。

持久托管发布版 Session 在启动时及每三十分钟检查规则。调试／测试构建和独立端点关闭自动网络任务。
`server agent-manifests` 暴露实际策略及最近自动结果。`server reload-agent-manifests` 只重载规则，从不读取
`config.toml`；`server reload-config` 将规则检查开关与其他配置一起发布。致命文件/TOML 错误保留原开关；
字段警告应用已接纳的回退设置。关闭检查停止调度；重新开启立即安排检查。
活动更新期间重载返回忙碌。此命令不热重载其他运行中设置及键位。自动检查关闭时仍可显式更新。
参见[屏幕模型](../../orchbee-server/docs/agent-screen-model.zh-CN.md#自动检查与配置快照)。
发布版验收夹具若不以公开网络访问为测试目标，须显式关闭自动检查。回退到不认识此键的读取器时，
须先移除 update 配置节；用户覆盖和已提交规则缓存予以保留。

## 恢复自定义快捷键

Windows `config reset-keys [--format text|json]` 是离线配置修复操作，使用与 `config check`
相同的文件位置；文件不存在（包括显式路径）或没有顶层 `keys` 时成功返回，不创建或写入
任何内容。它移除 `[keys]` 及其嵌套表、命令数组，或可以安全删除的顶层赋值；其他设置、
被移除表之外的注释和行结束符逐字节保留。UTF-8／TOML 无效、文件超过 64 KiB，或编辑后解析结果不等于
原配置去掉 `keys` 时，在写入前失败。这可用于从当前设置读取器尚不支持的自定义键位配置
中恢复；重置操作不校验或修复其他设置，也不重载正在运行的 Client。

共享 Windows 设置修复所有者固定私有父目录树，拒绝重解析点、非普通文件或多硬链接文件，
独占重新打开文件后对比准备阶段的字节。输入变化或有并发文件持有者时拒绝编辑。相邻的
唯一 `.bak-keys-...` 备份以不覆盖方式创建，通过私有权限准入并刷盘后，才通过独占句柄
改写原文件，保留原文件身份和权限。写入／刷盘失败时，在释放句柄前恢复并刷盘原内容；
恢复失败则保留完整备份并报告 `recoveryRequired: true`。改写开始后的进程崩溃也保留此前
已刷盘的备份。这是可恢复的原地替换，不是跨断电的文件系统事务。

封闭的 `orchbee.config.reset-keys` v1 回执标明 `missing`、`no-keys`、`reset` 或 `failed`、
选定路径、可为 null 的备份路径、有界错误字段／原因及恢复要求。重置或无变更退出 0，
失败退出 1；用法错误在文件访问前退出 2。文本本地化并转义路径，不回显配置值。成功备份
属于用户恢复产物，通过回执明确交付给调用者负责恢复／删除。不再需要的失败尝试备份会
删除；清理失败则暴露准确路径。不自动准入 Session、重载、启动进程或改写无关设置。

纯变换、独占文件／备份检查、注入写入／恢复失败以及真实 Windows CLI 进程测试覆盖本契约。
完整快捷键、完整重载对齐和最终 Windows 替换验收仍是独立待办。

## Windows 自动 Shell

共享的[启动配置偏好](../../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)为每次默认终端启动选择程序。内置系统 Shell 配置优先在绝对 PATH 目录中查找首个兼容的 `pwsh.exe`，然后使用绝对 `SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe`。COMSPEC/SHELL 和当前目录查找不覆盖该策略；选择时不执行探测程序。已有 PowerShell 提示符集成在选择后执行。显式可执行文件和其他启动配置保留原参数。

`terminal.default_shell` 与 `terminal.shell_mode` 已退役：原生配置文件不选择启动程序，已有的这两个键按未知键警告并忽略。私有启动参数、Server 接纳值和 Configuration 快照字段都不携带 Shell。

自动 PATH 候选必须通过有界 DOS/PE/COFF、可选头检查，非空节表必须处于文件边界内，具有可执行映像且非 DLL 标志，x86/x64/ARM64 机器类型兼容。损坏、缺失、不可读或不兼容的候选会被跳过。发现过程解析候选的规范路径，排除祖先目录中存在 `AppxManifest.xml` 文件的映像：Store/AppX 激活拒绝 Host 的原子 Job-list 启动。该选择发生在终端创建之前，继续搜索 PATH 或使用系统回退；不重试创建，也不放松 Job 进程归属约束。显式启动配置程序仍按字面选择。这些检查仅证明结构候选，不能保证依赖可加载。已有固定版本 `winsafe` kernel 安全包装获取系统架构，不添加自写 unsafe。[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo) 对 ARM64 上的 x86/x64 进程返回模拟架构；模拟 Worker 优先本机 ARM64 程序尚未支持。显式启动配置程序仍可选择该程序。真实 x64 选择、合成头部／架构分区，以及直接／嵌套包标记分区覆盖本实现，不声称通过 ARM64 硬件验收。

## Windows Client 按键配置

有界共享设置读取器在 Client 启动前接纳 `[keys]`。每个 Client 在在线发布前持有不可变绑定快照；
编辑文件或重置在下一个 Client 或显式在线重载后生效。Client 将已实现的语义动作映射到默认注册表。
用户覆盖替换该动作的默认绑定，并取代冲突的默认按键。无效的显式动作仍保留为显式禁用覆盖，
不会静默恢复内置默认；其他动作及同一动作里的有效候选仍可使用。未知动作会警告并忽略。多个显式动作占用同一按键时，固定动作注册表顺序选择第一项并
报告后续冲突。诊断不保留原始按键或命令。

`prefix` 接受一个按键。动作值接受字符串或最多 16 个候选；空字符串或空数组保留为显式禁用覆盖，并发出有界警告。
`prefix+` 指定前缀层，其余必须是安全直达按键。`switch_tab` 和 `focus_agent` 支持带修饰键的 `1..9`。Agent 序号按过滤后的机器
Agent 列表计算，`focus_agent` 默认未绑定。
完整修饰键及大写字母在冲突检查前归一化。前缀本身在两层均保留。直达可打印字符、
裸编辑键及 Ctrl+C/D/Z 被拒绝。捕获与模式先于直达分发；粘贴及释放事件不触发绑定。
双前缀优先发送规范 C0，否则发送原始字节，不再硬编码回退为 Ctrl+B。

`copy_mode` 进入[键盘复制模式](native-keyboard-copy.zh-CN.md)，默认 `prefix+U`（大写 U）。它可以使用相同的安全直达组合键，也可以用空数组禁用入口。模式内移动键保持固定。

`new_worktree` 与 `open_worktree` 进入 [Worktree 打开流程](native-worktrees.zh-CN.md)，默认
为 `prefix+W` 与 `prefix+O`。安全直达组合键和禁用沿用相同注册表。

`menu` 打开[原生动作菜单](native-action-menus.zh-CN.md)，默认 `prefix+f2`。
同样支持安全直达组合键及显式禁用。Machines 保留 `prefix+M`。

`resize_mode` 进入[键盘调整模式](native-keyboard-resize.zh-CN.md)，默认 `prefix+P`。
`resize_pane_left`、`resize_pane_right`、`resize_pane_up` 和 `resize_pane_down` 默认
未绑定，接受相同的安全组合键。不进入模式，直接将最近可用分隔线移动五个百分点；
例如 `resize_pane_left = "ctrl+left"`。模式内方向键固定为五个百分点，Shift+方向键为一个，
Esc 退出。待定期间额外调整键被消费，不排队。使用这些新动作名的配置需要匹配当前 cohort；
使用旧 reader 前需删除这些键。其他现有配置保留默认行为。 直接调整绑定要求修饰键或功能键；裸编辑键仍属于 pane。

解析后的注册表同时提供分发、帮助行和主要实时提示。帮助会省略没有有效绑定的动作；需要会话的
动作即使当前未连接 Orchbee 会话，只要有有效绑定，帮助仍会保留该行并提示连接会话。现有原生
默认值仅为没有显式覆盖的动作保留；未知字段发出警告且不改变有效注册表。
自定义命令配置无效时禁用已配置命令，不执行部分或未经验证的命令。导航模式映射、按序
Agent/Workspace 按键及其他未支持动作仍未完成。定向证据覆盖边界、冲突与默认
优先级、输入归属、分发及发现；不宣称全量回归或视觉验收通过。

注册表的显示投影独立于接纳的配置拼写，对规范化绑定进行格式化。帮助行和实时提示中的
ASCII 字母只有在绑定包含 Shift 时才使用大写，其余保持小写。Unicode 字形模式使用方向键
字形，并在 Apple 宿主上使用 Apple 修饰键符号。简化字形模式使用 `Up`、`Down`、`Left`、
`Right` 以及 `Ctrl`、`Alt`、`Shift`、`Super`，Apple 宿主也一样。前缀与其成员之间使用一个
空格。这些显示规则不改变 TOML 拼写、规范化、匹配或分发。

支持的动作字段为 `help`、`menu`、`new_worktree`、`open_worktree`、`new_workspace`、`rename_workspace`、`workspace_picker`、
`detach`、`previous_workspace`、`next_workspace`、`previous_agent`、`next_agent`、`focus_agent`、`new_tab`、`rename_tab`、`previous_tab`、
`next_tab`、`move_tab_previous`、`move_tab_next`、`switch_tab`、`close_tab`、
`copy_mode`, `resize_mode`, `resize_pane_left/down/up/right`，以及 `focus_pane_left/down/up/right`（四个字段）、`split_vertical`、`split_horizontal`、
`close_pane`、`zoom`、`toggle_sidebar` 和 `reload_config`。它们调用现有原生动作；
Workspace 对应原生 Workbench 位置，竖向分屏向右、横向分屏向下。
Agent 上一个/下一个默认使用前缀 `{`/`}`，使用同一临时筛选/排序后的
[机器 Agent 列表](machines.zh-CN.md#agent-元数据视图控制与导航)，并重新验证精确物理目标。
这两个动作不增加按序 Agent 聚焦或自定义视图 JSON DSL。

```toml
[keys]
prefix = "ctrl+a"
help = ["prefix+?", "f1"]
new_tab = "prefix+n"
switch_tab = "alt+1..9"
focus_agent = "prefix+alt+1..9"
zoom = ""
```

Windows `config check` v1 保留 `keyPrefix`、`keyOverrides` 并增加有界警告记录；历史
机器字段 `keyPrefix` 保持稳定且不随平台变化
的报告拼写（本例为 `Ctrl+A`），TUI 快捷键发现则显示实际未按 Shift 的手势（`Ctrl+a`）。
示例让新建 Tab 取代默认的下一 Tab 快捷键；空 `zoom` 禁用放大快捷键。当前绑定只在主
TUI 使用。CLI 终端流 attach 保留独立的 Ctrl+B/q 转义。

功能键使用 xterm 的实际 F1–F12 序列编号（跳过 16 和 22），带修饰符的 F1–F4
以及 kitty F1–F24。修饰符事件尾部保留按下、重复和释放；无效事件或无法表示的
Hyper/Meta 修饰位不会降级为普通快捷键。宿主是否实际提供这些按键仍取决于终端。

## 在线配置重载

侧栏 profile 使用同一接纳/发布转换：共享 Windows 配置所有者验证
`ui.sidebar.agents.rows`、`rows_by_agent`、`row_gap`，再发布有界规范 profile。
Client 在安装任何字段前同时验证键位、通知和两个侧栏；畸形私有线协议 profile 保留全部原配置。
首个快照到达后，启动使用 Server 的完整已接纳基线；连接期间显示的本地投影只是临时值。
配置检查报告支持的布局及边界。token、Provider 选择、
规则、几何与证据见 [Agent 侧栏配置模型](machines.zh-CN.md#agent-侧栏配置模型)。

Windows `server reload-config` 请求运行中 Session 重载；`server config` 读取最后发布的版本。
两者均不激活 Session。Server 冻结启动配置位置，串行接纳重载，验证整个有界文件并准备
一个包含有效设置和警告的接纳候选后才提交。致命读取或解析失败会保留原版本、全部 profile、
诊断和在线 manifest-check 调度。带警告的候选会原子替换有效设置和诊断；Client 绝不会把一个
版本的设置与另一个版本的诊断拼接。版本 0 是权威启动基线。Client 会应用首个完整 Server
快照一次，包括版本 0；之后拒绝重复或更旧版本。首次成功重载发布版本 1，之后只要已接纳
设置或诊断任一变化，版本就递增。这也包括回退值与后来合法显式值
相同但警告已清除的情况。另一重载持有接纳权时读取可返回忙。提交后丢失响应属于结果未知；
Client 不自动重发变更，可通过读取当前状态恢复判断。

64 KiB 源文件限额和各 profile 的独立边界都是接纳输入；组合后的响应还必须满足不变的
256 KiB 配置响应上限。安装版本 0 或应用任何在线策略前，以及每次重载提交前，Server 使用同一个精确
serializer 预检，对包含固定宽度合法关联标识的完整 `ConfigurationResult` 执行序列化（启动时为
`Current`，重载时为 `Reloaded`）。已接纳源文件的组合转义 profile 和诊断如果超过该响应上限，则会被拒绝，
不截断其他合法设置。启动不安装配置状态或在线策略；重载保留完整旧快照和在线策略。

Server 提交已支持的在线 manifest-check 策略，并向连接中的主 TUI Client 发布键位、profile
及有界警告记录。
发布不等于所有 Client 都已确认应用。每个 Client 拥有可取消的配置观察线程，仅在输入包
之间应用完整且已验证的新版本快照，并拒绝过时或重复版本；清除待处理前缀，保留编辑器／
确认归属和草稿。带警告的快照会应用有效设置，并显示本地化摘要，引导用户运行
`orchbee config check`；后续干净快照会清除此警告。配置健康状态与临时提示独立保留：检查点恢复提示
优先用于首次启动显示；该提示消除后，配置警告会重新显示，而且不会阻止 Agent 通知卡片。重载失败保留现有设置和警告记录，并显示
本地化失败提示。重载动作可通过 `reload_config` 配置。原生默认是前缀 F5；前缀 Shift+R 是原生已有的 Project 重命名默认键位。配置 `reload_config = "prefix+shift+r"` 会明确替换该冲突默认。

CLI 目录及 worktree 默认值由下一次调用读取。其他配置族与 Client 专属 profile 策略仍未完成。线协议按键 profile 上限为 64 KiB；每动作最多 144 个展开触发项，保留用户文件中 16 个索引候选项的上限。可取消状态订阅使用共享 Client carrier，发布最新已提交配置。Reload 保留为两秒交换上限的一次性请求；结果未知时不重放，由新订阅恢复当前状态。重载成功表示状态已发布，不代表每个 Client 都已应用。

不引入持久配置迁移或自动重试。确定性测试覆盖临时启动、首次完整应用、致命失败保留、原子 settings/diagnostic 代次、后续单调应用及警告发布/清除。连接测试覆盖传播，不假设交付公平性。授权、有界序列化、无变化重载及退出仍是实现义务。设置提交/发布核心的范围更窄，只涉及 writer/CAS，不证明整个重载流程。

`server config` 与 `server reload-config` 版本 1 为每个成功快照增加必需的 `diagnostics` 对象。
文本输出包含可见数量、精确省略数量，以及每条有界的字段／原因／处置记录。

## 后续 Windows 终端的在线默认值

生产现在在每次新 Host 启动前，从一个确认快照读取
[共享设备启动配置](../../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)和滚动历史。
记录不存在时使用规范启动默认值。配置回执不携带启动程序。

现有 PTY、已准备启动、精确重放和延迟 Start 不接收后续默认值。配置重载先提交注册表，再提交 manifest 调度；候选校验在提交前完成，调度拒绝接纳时发布不变。持锁期间没有 Worker IPC 或分布式确认。重载成功证明配置接纳，不保证程序存在。

启动准备失败记录既有的确定交接前失败，保留 Worker/Server 连接，不创建 Host 终端。同一失败请求重放原失败；修正设置后必须使用新的创建标识。首次 Host 尚不存在时，延迟循环也保持可用。Host 启动/传输不确定性仍保留原退出与不重试策略。证据覆盖失败保留、精确/冲突重放和真实 Windows 管道交换；provider/多 Client/视觉全量验收仍延后。匹配的私有 cohort 整套部署或回退，不迁移持久化数据。

## 外层窗口标题

`[ui] window_title` 默认为 `{hostname}: {workspace}`，可在线重载。[标题所有者](windows-terminal-title.zh-CN.md#配置模板) 定义变量、字面花括号、无效/空策略、有界渲染及 CLI 覆盖优先级。语法错误、类型错误或超过 4096 字节现在只让该叶字段回退，并出现在 `config check` v1 的警告记录中；未知 `[ui]` 键会忽略并单独警告。有效 `windowTitle` 始终可解析，因此 `windowTitleDiagnostic` 保持 null。`server config`/`server reload-config` v1 暴露已发布的有效模板及诊断，

## 自定义通知配置

`ui.toast.delivery` 接受 off（默认）、orchbee、terminal 或 system，用于手动 CLI 通知。`ui.toast.overlay.position` 选择四角默认位置。共享偏好 `agentNotificationMode` 和 `soundEnabled` 管理自动 Agent 通知及声音。`ui.toast.delay_seconds` 提供延迟（默认一秒，范围 0–3600）；`[ui.sound.agents]` 提供逐 Agent 声音覆盖，不改变显式 `notification show` 的声音；剪贴板策略仍然独立。启动配置属于 Client；显式 Session 重载与键位一起发布完整的有界通知 profile，Client 在同时校验两者之后应用。`server config`/重载 v1 携带 16 KiB `notificationProfile`，`config check` v1 暴露手动与自动策略，配置失败 v1 增加自动延迟／声音诊断。详见完整的[自动通知契约](windows-notifications.zh-CN.md#自动物理-agent-通知)。

通用 Agent 提醒模式和全局声音保持权威；手动通知投递与逐 Agent 声音覆盖保持独立归属。

## Windows 版本安装

`orchbee update [--format text|json]` 安装配置的频道。`orchbee channel set <stable|preview> [--format text|json]` 先校验并原子保存频道，保留注释与其他 TOML 值，再运行同一更新器。该写入器可以创建显式指定但不存在的文件。更新失败保留已保存偏好。两个命令均无须 Session；不会启动、停止、替换或重新连接运行中的 Session。

共用更新工作流、来源／清单准入、有界传输策略、验证、发布、激活决策及已安装版本来源在所有宿主平台参与编译。具名原生端口拥有声明过的环境／配置访问、安装根目录准入、不跟随链接的清单打开、系统程序定位、capsule 验证和受管进程执行。能力准入先于信号注册及任何环境、文件、网络或进程效果。Linux/macOS 保留通用 unsupported-platform 结果（退出码 4），既不保存频道也不执行更新。

将 `ORCHBEE_UPDATE_MANIFEST` 设置为显式本地发布清单绝对路径或 HTTPS URL。`{channel}` 替换为所选频道。没有隐式公共发布源。选定的清单是信任依据；逐文件 SHA-256 校验保证与清单一致，不等于独立发布者签名。`ORCHBEE_INSTALL_DIR` 默认为 `LOCALAPPDATA/Programs/Orchbee`。安装路径拒绝 PATH 分隔符、展开变量和控制字符。获取来源共用 600 秒时限并支持 Ctrl+C 取消。HTTPS 使用系统目录中的 curl 8.4 或更新版本，仅允许 HTTPS 有界重定向，不读取 curlrc 或继承代理/凭据环境，并限制下载大小。

本地来源模型独立于编译宿主，保留既有 Windows 驱动器绝对路径语法：`/` 和 `\` 分隔组件，重复分隔符和 `.` 遵循非 verbatim 的 Windows 组件语义，拒绝 `..`、UNC、设备、verbatim 和驱动器相对路径。原始外部来源上限为 4096 字节；解析已准入相对产物时保留既有 parent／join 行为。既有来源夹具覆盖这些边界，并在 Windows 上与原 Windows 标准库解析器及 join 对照。

发布清单为封闭 JSON，包含 `schemaVersion: 1`、`product: "orchbee"`、`platform: "windows-x86_64"`、`channel`、`version`、`buildId` 和 `files`。每文件恰含 `path`、`url`、小写 `sha256` 与字节数 `size`。文件 URL 为 HTTPS 或相对清单目录解析的可移植路径。清单最大 64 KiB，最多 4096 个文件，单文件最大 512 MiB，总量最大 2 GiB。可移植路径拒绝目录穿越、Windows 设备名、替代数据流、大小写别名及文件/目录冲突。必需内容包括四个程序、schema-2 `cohort.json`、适配器 capsule 及三个 ConPTY 分发文件。所有声明文件先复制到私有暂存区并计算哈希；通过 cohort 与 capsule 摘要接纳后，才执行新 Client 的精确版本启动检查。

安装所有者串行化写入器，并以清单 SHA-256 为名发布独立目录。现有版本经过校验后复用，不覆盖。用户 PATH 激活是后续副作用：失败或结果丢失均保留发布路径以供显式重试。现有进程继续使用原文件。旧版本属于安装状态，因可能仍在运行而保留；自动保留策略和在线交接是独立工作。未发布暂存区在子进程结束确认后清理；结束未确认或清理失败通过回执移交确切路径供恢复，不把旧版本当暂存目录删除。

`orchbee.update` v1 区分频道已保存、版本已发布、PATH 已激活、复用、中断、失败阶段/原因及待清理项。完成状态为 0，确定失败为 1，用法错误为 2，超时为 7，物理/激活结果未确认为 8，中断为 130。即使激活需重试，仍可直接使用已发布路径；重新登录可刷新继承的 PATH。完整分发发布、HTTPS 来源验收以及真实用户 PATH/整套 cohort 启动验收属于部署义务，本地暂存测试不代表这些义务已完成。

从已完成的 Windows cohort 准备清单而不执行发布：

```powershell
node scripts/package-native-windows.mjs C:/path/to/cohort stable 0.1.0 build-id
$env:ORCHBEE_UPDATE_MANIFEST = 'C:/path/to/cohort/release.json'
orchbee update --format json
```

打包工具校验现有 cohort、适配器 capsule 和固定 ConPTY 包，清点有界普通文件，再独占写入同目录的 `release.json`，拒绝已有清单。HTTPS 分发需将此目录发布到清单 URL；相对文件 URL 沿用目录布局。版本必须匹配所打包 Client 的 `--version`；buildId 区分同版本号的不同发布。

独立版本目录同时保存经过接纳的原始 `release.json`；该保留文件不属于文件清单条目，复用时核对其未改变。`status client` v1 将清单绑定到目录标识和已验证 cohort 后再投影实际安装来源，不以当前配置偏好替代。

CLI 私有的 `release_store` 模块拥有不可变发布。`ReleaseStore` 获取已有私有缓存写锁并固定安装目录祖先。暂存区借用该锁，只创建经过验证的可移植路径与独占私有文件，再通过不替换目录重命名发布到小写 SHA-256 标识。应用拥有清单校验、下载、cohort 接纳、启动检查、用户 PATH 激活及保留版本。显式清理只删除自身未发布暂存区；子进程结束未确认时使用 `retain_for_recovery` 并报告保留路径。`open_release_file` 拒绝不安全祖先、重解析点、外部 ACL 和硬链接。路径准入、暂存区所有权及发布／恢复决策在所有宿主平台参与编译；暂存区分配与文件系统变更使用具名平台端口，已有私有文件端口拥有锁和句柄准入。Unix 端口在文件系统效果前返回类型化 Unsupported，不能创建成功的存储或暂存区。既有路径准入测试在所有宿主平台运行；原生 Windows 测试覆盖写入互斥、不可变发布和暂存清理。

## Windows Agent 侧栏布局

Windows 主 TUI 在启动及 `server reload-config` 后应用 `ui.sidebar.agents`。例如：

```toml
[ui.sidebar.agents]
rows = [["state_icon", "state_text"], ["machine", "workspace", "tab"], ["pane", "$summary"], ["terminal_title_stripped"]]
row_gap = 1

[ui.sidebar.agents.rows_by_agent]
claude = [["state_icon", "agent"], [{ token = "$model", fg = "#8bc", rules = [{ contains = "fast", ignore_case = true, bold = true }] }]]
```

省略布局使用单行标题默认，显式空布局保留身份及导航行；缺失或隐藏的值不占行。
[侧栏模型](machines.zh-CN.md#agent-侧栏配置模型)定义 Provider 别名、规则、按实际高度
计算的窗口及不变的物理权威。`terminal_title`/`terminal_title_stripped` 现在读取精确物理终端的 OSC 标题。
原始形式保留活动符号；去动画形式移除一个提权前缀和一个已知前导活动符号。
缺失或清除的标题消失；`$terminal_title` 仍是独立自定义值。
`ui.sidebar.spaces` 使用独立 Workspace 布局与 profile，见下文。

`config check` v1 和 Session 配置回执 v1 必需携带 `agentSidebarProfile` 与 `spacesSidebarProfile`（最多 32 KiB
的封闭规范 JSON）。配置失败 v1 增加 `ui.sidebar.agents` 及布局/颜色/token/规则/Provider
诊断。目录列出当前文档与命令路径。
匹配的私有 cohort 一起携带新必需 profile，整套部署/回滚，无持久迁移。

## Windows Workspace 侧栏布局

主侧栏将 Workspace 放在所属 Project 下，可用 Agent 放在精确归属的 Workspace 行下，共用一个滚动窗口。[侧栏导航模型](tui-shell-layout.zh-CN.md#侧栏导航模型)
拥有展开状态和键盘导航。Workspace 选择器仍可通过打开 Machines 后按 Tab，或点击
侧栏范围页脚进入。即使重名，点击行仍保留完整机器／Session/context 身份。
`ui.sidebar.spaces` 控制 Workspace 行，`ui.sidebar.agents` 控制其 Agent 子行。
Agent 的 `row_gap` 用于 Agent 行之前及相邻 Agent 行之间；Workspace 的 `row_gap` 用于下一条 Context
行之前，每个分组之前至少保留一行。Project 的首个 Workspace 紧贴其标题。

```toml
[ui.sidebar.spaces]
rows = [["state_icon", "workspace"], [{ token = "branch", fg = "#8bc", rules = [{ starts_with = "worktree/", dim = true }] }, "git_status"], ["$issue"]]
row_gap = 0
```

默认值为 `[["state_icon", "workspace", "git_status"], ["location"]]`，间隔 0。
`location` 显示分支，否则显示 Workspace 文件夹，本机主目录缩写为 `~`。
显式 `rows = []` 保留身份／导航行。另一个内置值是 `state_text`；
`$name` 读取 Workspace 报告，不替代内置值。样式与首条匹配规则共用 Agent 接纳机制，
但图标／Git 计数拒绝文字规则，Workspace 布局没有 `rows_by_agent`。
最多 16 行、每行 16 个 token、每个文字 token 16 条规则，`row_gap` 范围 0–65535。
缺失／隐藏值折叠。运行状态绘制在行首状态轨道，Git 计数右对齐在配置它的行末端，在身份行上
贴齐行尾；其他文字共享剩余单元格。

状态使用完整物理 Agent 快照，不受 seen 及列表／来源过滤影响。
Git 在所属 Worker 上读取已提交的 Workspace 目录，显示相对本地 upstream 的非零 ahead／behind；
缺少 upstream 时无计数；非仓库无 Git 观测。游离时无分支名，未出生分支保留名字。
不 fetch、不改配置、不执行 hook/fsmonitor、不启动终端，绘制时无 I/O。
自定义 `$branch`／`$git_status` 保持独立。
[Workspace 侧栏模型](machines.zh-CN.md#工作区侧栏模型)定义保护、聚合、预算、精确激活及验收限制。

配置失败 v1 接纳 `ui.sidebar.spaces`。Session／Client 重载先验证键位、通知及两个侧栏 profile，
再统一发布。每个 profile 最多 32 KiB；匹配 cohort 的配置帧包括转义最多 256 KiB。

`terminal.focus_new_panel` 接受布尔值，默认为 `true`。原生 Client 分屏选中刚提交的新窗格；`false` 保留原窗格。Client 在启动时读取该偏好，编辑此字段后需重启。类型无效时产生字段级诊断并使用默认值。CLI `panel split --focus` 保留显式命令策略。

<!-- documentation-pairing: english-sha256=0d416d4d843a6cfeb5c6556ff47ed1927a3fc756f5d783177033e39a8677b363 zh-cn-sha256=c60d771288255b96f8a6a00b8e356425439bbead6f89b2239e9036e1b2277874 -->
