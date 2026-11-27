# Orchbee 原生 CLI Agent 指南

本指南描述原生 CLI 在 Windows、Linux 和 macOS 上的工作流。语法、帮助与补全跨平台一致；实际执行能力仍明确区分。操作前，先通过已安装构建的 `--help` 确认精确行为。

## 快速上手示例

```text
# 查看运行中的会话、面板与智能体
orchbee status --format json
orchbee agent list --format json

# 在目标工作目录启动智能体且不抢占客户端焦点，等它空闲后返回
orchbee agent start antigravity --cwd <dir> --no-focus --wait-ready -- <provider-args>

# 按面板或子终端发送提示词、按键、查看进程信息或可见屏幕输出
orchbee agent prompt --panel @panel:0x1 "<task>"
orchbee agent prompt --terminal @terminal:0x2 "<task>" --wait --timeout 60000
orchbee agent send-keys --panel @panel:0x1 escape
orchbee agent read --panel @panel:0x1 --source visible --plain
orchbee panel process-info @panel:0x1 --format json

# 在受管 Shell 中执行单条有界命令并等待完成标记
orchbee terminal run --panel @panel:0x1 "git status" --wait --timeout 30000 --format json
```

- 优先使用紧凑稳定句柄（`@panel:0x1`、`@terminal:0x2`、`@agent:<kind>`、`@agent:<n>`）；PowerShell 中记得给句柄加引号。
- 启动时，`--cwd <dir>` 和 `--focus`/`--no-focus` 仅用于新建标签页或 `--split right|down` 启动，与 `--here` 互斥。`--client <id>` 指定 `--focus` 切换的客户端，且需与 `--focus` 同用。
- 在现有终端内启动时，`--here --panel <panel-id>` 要求目标 shell 处于空闲提示符。若目标就是运行本命令的 Panel，会以 `own-panel` 拒绝。若拒绝信息出现 `stage: "agent-terminal"`，先用 `orchbee panel process-info <panel-id>` 和 `orchbee agent list` 排查。
- 超时、取消或结果未知时绝不重放输入；先用 `agent read` 或 `terminal read` 检查现场。
- 仅在用户当前对话明确要求停止时才停止 Server 或 Session；绝不擅自添加 `--confirm`。

## 发现当前构建

从 `orchbee --help` 开始，再查看 `orchbee <family> --help` 和 `orchbee <family> <command> --help`。`--` 前任意位置的单个帮助开关，也支持已填写的操作数和选项，且不会执行它们。未知选项、缺值和重复选项仍会失败。`help <path>` 接受精确帮助路径。裸 `orchbee` 会启动 TUI。

`--version`、`--skill`、`completion <shell>`、`api schema` 以及 `config default` 均可离线使用。补全支持 Bash、Zsh、Fish、PowerShell 和 Elvish。`api schema` 列出 schema ID；用 `api schema <schema-id>` 查看同一构建的机器契约，或添加 `--output <path>` 导出。读取输出中的 `schema` 与 `version`；本指南不固定目录数量或历史版本。

所有 CLI JSON 文档均使用版本 `1`，每种只有一个当前 schema。不同平台及参数下字段均保留：`{kind:"unsupported"}` 表示平台无此能力，`null` 表示来源说没有，`"unknown"` 表示来源说不清。整条命令不支持时返回 `stage: "platform"`、`reason: "unsupported-platform"`，退出码为 `4`；当前发布版本暂不提供的命令返回 `stage: "release"`、`reason: "withheld-in-release"`，退出码为 `4`。`doctor` 在所有平台上都在 `orchbee.doctor` 中保留全部六项检查；平台无法执行的检查为 `{kind:"unsupported"}`。

PowerShell 用户可用 `orchbee completion powershell` 输出补全脚本。

将分页游标原样传回产出它的同一命令和后端。列表游标与变更游标由不同 owner 管理，绝不相互转换。只有确实需要文档说明的后端默认行为时，才省略等待选项。

## 选择并启动 Session

用 `doctor` 检查 Orchbee 在本机能否正常工作：安装、设置、已保存数据、运行时目录和会话，每项为 `pass`、`warning`、`fail` 或 `not-applicable`，只有检查失败时退出码才为 `1`。用 `config check` 检查本地配置，`status` 读取正在运行的 Session 的数量和进程。`doctor` 与 `config check` 不启动 Session；`status` 需要正在运行的 Session。`server start` 执行无界面启动；`server restart` 可把正在运行的 Session 交给本构建的新 Server，终端保持运行；`workspace create`、`project create`、`worktree create` 和 `worktree open` 在所选 Session 未运行时会先自行启动它，并在标准错误输出 `本地会话已启动。`。

`--session <name>` 选择本地命名 Session，可位于命令之前或之后，默认值为 `default`。后续操作与诊断应保持相同选择。`session list` 显示本地清单；`session attach <name>` 进入其 TUI。退出 TUI 不会停止 Session。

```text
orchbee --session work server start
orchbee --session work status --format json
orchbee --session work tui
```

运行时根目录缺失属于环境检查失败；重试前查看 `config check`。`recovery-required` 表示启动无法确认安全的所有权。其详情带有 `blockingProcess` 时，表示之前启动的某个进程仍在运行；请告知用户，结束该进程或重启电脑即可解除拒绝。`server-not-started` 表示系统拒绝创建 Server，`ioKind` 和 `osError` 说明原因。诊断该精确 Session，或使用另一个名称。不要手动删除运行时目录。

## 解析目标并检查状态

Workspace 拥有目录，可属于 Project；Project 分组是可选的。Workspace 包含 Tab，Tab 包含 Panel，Panel 有主终端且可包含子终端。TUI 客户端各自保存选择与视图。

先用 `api snapshot`，再使用相关的 `project`、`workspace`、`tab`、`panel`、`terminal`、`launch-profile`、`client` 或 `agent` list/get 命令。复用返回的 ID。优先使用打印的 `@panel:0x1` 等稳定句柄：它只省略 ID 的前导零，其他资源增删不会改变其含义。PowerShell 中给句柄加引号。ID 和紧凑句柄受所选 Session 及底层身份寿命约束；Server 重启后重新发现运行时终端 ID。

在 Orchbee 终端内，调用者元数据可以选择其 Panel；在外部，`panel current` 可使用唯一当前 TUI 客户端。存在多个客户端时传入 `--client <id>`；无界面查找可使用位置参数 Panel ID。`agent start` 省略 `--panel` 时明确要求有效调用者元数据。部分或过期元数据不会授权回退。只有一个就绪客户端时，`workspace`、`tab` 与 `panel` 的 list/get 按它给出 `focused`；否则传入 `--client <id>`，不然 `focused` 为 null。

## 创建和安排终端

```text
orchbee workspace create --cwd . --label work
orchbee tab create --workspace work --label shell
orchbee panel list --format json
orchbee panel split <panel-id> --direction right
```

检查每份创建回执：布局成员已提交不代表进程已启动。需要客户端的命令会拒绝缺失、有歧义或尚未就绪的客户端；使用 `client list` 并显式传入 `--client`。

`tab create`、`panel split` 和 `panel create-terminal` 支持其声明的启动选择、cwd 和字面环境条目。保存的 Launch Profile 使用 `--profile`；显式程序使用 `--exec` 和可重复的 `--arg`；一次性任务可加 `--close-on-exit`，程序以退出码 0 结束后关闭其面板。目录必须已存在。查看帮助中的 `--cwd home|current|workspace`、绝对路径覆盖、完整终端几何参数及 `--env NAME=VALUE` 边界。这些覆盖不修改保存的 profile。

`panel create-terminal --child` 创建子终端。使用 `workspace`/`tab` move、`panel move --into-tab <tab>`/`swap`、`layout`、`panel resize`、`panel maximize` 和 focus 命令安排布局。客户端 focus 控制 TUI 选择，不激活操作系统窗口。选择当前目标或布局路径前，先阅读命令帮助。

## 发送输入并读取输出

`terminal run <terminal-id> "git status"` 提交一条字面命令并发送 Enter。`terminal send` 接受 `--text`、`--file`、`--stdin` 之一，可添加 `--paste`/`--enter`，或使用声明的按键。按键有序，最多接受 64 个。文件与 stdin 在本地读取，路由到已保存机器时也一样。来源读取上限为 1 MiB；形成的 paste 与 argv 仍受其声明的更小边界约束。

回执证明输入记录被接受，不证明 Shell 就绪、命令完成或退出状态。不要重放部分或结果未知的输入。检查回执与终端后，再决定下一步发送内容。

`terminal read` 和 `agent read` 默认读取最近输出。通过 `--source` 选择 `visible`、`recent`、`recent-unwrapped` 或 `detection`；`--plain` 直接输出 UTF-8 文本，排除 JSON。用 `--lines` 控制窗口、`--source recent-unwrapped` 拼接软换行。不要混用不兼容的输出模式；边界、软换行和 ANSI 规则见帮助。

`terminal wait-output` 可以匹配已经存在的输出。默认总时限为 30000 毫秒；可用 `--timeout` 覆盖，或显式使用 `--forever`。`--timeout` 与 `--forever` 冲突。匹配到一行不能证明刚提交的命令已完成。

## 安装并操作 Agent

本版本不提供智能体集成：没有命令可以启用、停用、列出或解释集成。早期版本写入的条目保持不变，卸载时会删除它们。

`agent start <provider>` 在你自己的面板旁的新标签页中启动提供方；`--split right|down` 改为打开拆分面板，`--cwd <dir>`、`--focus` 与 `--no-focus` 仅适用于此类新标签页或拆分面板启动，并与 `--here` 互斥。`--client <id>` 指定 `--focus` 切换的客户端，且需与 `--focus` 同用。`--here --panel <panel-id>` 在该已观察到空闲的 cmd、PowerShell、pwsh 或 macOS zsh、bash 终端已确认的提示符处输入一次（不能是你自己的面板：`own-panel`；交互启动时若提示符尚未标记会短暂等待稳定）。`--panel <panel-id>` 或 `--current` 指定参照面板。不带参数时，新标签页或拆分面板的 shell 以提供方作为首个命令运行，面板创建后即返回：`confirmation` 为 `launched`，`submission` 为 null。否则输入命令并在 3 秒检查后返回：`confirmation` 为 `sent`。两种情况下 `place` 和 `panelId` 都指明启动位置；`interactiveReady` 为 false，除非 `--wait-ready` 随后在同一时限内等到智能体空闲或完成首轮：此时为 true；智能体被阻塞时以 `agent-blocked` 失败，未能及时就绪时以 `start-deadline` 失败。不用 `--wait-ready` 时，发送提示前先用 `agent wait --panel <panel-id> --until idle` 等待。shell 回到提示符时以 `command-not-found` 或 `agent-exited` 失败。未经规范会话报告时，就绪由终端观察确认：`confirmation` 为 `observed`，没有 `sessionId`，请用 `@panel:` 句柄经 `--panel` 或 `--terminal` 指定该智能体。后续步骤失败时，已创建的新标签页或拆分面板保留，失败结果的 `createdPanelId` 指向它。聚焦未切换时不会使启动失败；`result.focus`（失败时为 `details.focus`）给出阶段与原因。默认时限 30000 毫秒，允许范围 3001–300000。字面提供方参数放在 `--` 后，或使用可重复的 `--arg=<value>`。超时或取消后提供方可能仍在运行；重试前检查输入回执，并在 `agent list` 中查看该智能体的行。

`agent list/get` 检查当前 Agent 状态；`agent get --format json` 还会给出 `result.explanation` 或 `result.detection`，另一项为 null。`agent list` 会合并规范会话与物理终端 Agent，并为每行报告来源、语义状态和位置；`--client <id>` 添加独立的 Client 观察，不改变状态。每页 1–100 行，默认 50 行。JSON 使用 `orchbee.agent.list` v1 和 `orchbee.agent.get` v1。分页游标应原样传回；畸形游标在读取实时来源前作为用法错误拒绝，收到 `cursor-restart` 时须从第一页重新开始；可选 cwd 信息每页最多为 64 行，无法获取时为 null。

`@agent:<kind>` 选择唯一提供方，`@agent:<n>` 选择当前完整列表中的一基序号，而不是当前页的序号。`agent get`、`read`、`prompt`、`send-keys`、`wait` 和 `focus` 共用此解析器；`agent prompt`、`send-keys`、`read` 和 `wait` 也可直接传入 `--panel` 或 `--terminal`（仅通过终端识别的智能体，例如 `antigravity`，以及子终端，应通过 `--panel` 或 `--terminal` 指定；若在 `stage: "agent-terminal"` 被拒绝，可用 `orchbee panel process-info <panel-id>` 检查进程归属）。不完整投影不能证明提供方匹配唯一。v1 `agent get` 将普通 Agent 身份和状态放在 `result.agent`，将各来源分面的详细信息放在 `result.inspection`。

`agent prompt <target> <text> --wait` 提交字面输入，并在检查条件前观察活动。`agent wait` 只观察。默认没有总时限，条件为 idle/done/blocked；自动化应指定有限时限。新鲜度、待处理关注和已有工作影响准入。匹配状态不证明某条 prompt 的专属回合已完成。取消等待不会撤销已接受的输入，也不会停止 Agent。

保留已观察的 revision；使用 `--after-revision` 等待后续状态前，先查看 `agent wait --help`。

`--until`、`--timeout` 和 `--forever` 要求 `agent prompt --wait`；`--forever` 与 `--timeout` 互斥。Windows Codex 的输入在 paste 与 Enter 之间包含 Right 键。等待失败会保留已接受的输入回执，因此不要重放结果不确定的输入。

## 事件与终端流

`events subscribe --event workspace.created --timeout 30000` 每个事件输出一行；加 `--format json` 输出 NDJSON 记录。`orchbee help events subscribe` 列出全部事件名称。`events wait` 等待一个筛选条件，默认 30000 毫秒；timeout `0` 检查一次已接纳基线。保留游标包含 epoch 和十进制字符串序号；出现缺口时必须重建基线。

结构化筛选输入见 `api schema orchbee.events.stream` 的 `$defs/filter`。最小有效输入为：

```json
{
	"kind": "event",
	"event": "workspace.created",
	"scope": { "kind": "all" },
	"condition": { "kind": "any" }
}
```

结构化筛选独占作用域与条件，不能混用简写 selector。只有需要结构化能力时才使用它，普通发现可用简单的 `--event`。

`terminal session observe` 输出有界 NDJSON 帧且不取得控制权。`terminal session control` 接受 NDJSON 输入；编写记录前先查看其声明的输入 Schema。`terminal attach` 和 `agent attach` 要求真实终端 stdin/stdout。按 `Ctrl+b q` 可脱离；连续两次前缀发送一个字面前缀。Attach 不启动 Session、不重连，也不重放输入。

<!-- unreleased -->

## 插件

使用 `plugin list`、`plugin action list` 和 `plugin panel open --help` 发现已安装能力。`plugin link` 注册本地清单；`plugin install` 和 `plugin update` 的 `--yes` 用于声明的非交互信任流程。已启用插件的启动命令和事件命令会在之后的 Session 中自动运行。注册表命令不启动 Session；调用操作和 panel 命令需要已有 Session。

`plugin action invoke <action-id> --context <json>` 接受 `{"selectedText":"example"}` 等展示覆盖。输入结构见 `api schema orchbee.plugin.runtime` 的 `$defs/context`；字段使用 camelCase，拒绝未知字段。CLI 退出后，已接纳的插件任务可继续运行。`plugin log list` 查看保留日志。

Popup 放置要求当前已就绪客户端和焦点。请移除 `--no-focus`，或选择 tab/split 放置。`popup close` 释放弹窗终端。普通面板与 popup 的生命周期不同，应查看对应回执。

<!-- /unreleased -->

## 已保存机器

`machine list` 先以 `local` 列出这台电脑，再列出每台已保存机器的颜色、状态及空闲的 CPU、内存和磁盘；机器读不到的数据在 JSON 中为 `"unknown"`。在这台电脑上，不带 `--machine`、`--session`、`--client` 或 `--cursor` 的 `workspace list` 和 `agent list` 还会询问每台已连接的机器：JSON 增加 `result.machines`，每台机器一项 `{machine, label, source, result}`，`source` 和 `result` 是该机器自己的回答（没有应答时都为 null）；文本在后面加上 `@<名称>` 和该机器自己的表格。这些 ID 和句柄属于那台机器，操作它们时加 `--machine <id>`。`machine add/rename/set-color/enable/disable/remove` 管理已保存的远程机器；机器名称以其颜色显示，即预设颜色（`cyan`、`teal`、`sky`、`blue`、`indigo`、`violet`、`purple`、`magenta`、`pink`、`brown`、`slate` 或 `grey`）或自定义的 `#rrggbb`，`add` 不加 `--color` 时自动选择；rename、set-color、remove、enable 和 disable 用 `<id|label>` 指定机器，多台机器同名时须改用 ID。`machine connect <id|label>` 通过后台连接进程连接机器，机器上线后返回，或返回未能上线的原因；命令结束后连接仍保持。登录需要回答而有 Orchbee TUI 在运行时，命令（包括转发的命令）会在标准错误写一行提示，并最多等待 120 秒，由用户在 TUI 中登录；用户选择暂不登录时，命令以 `auth-required` 结束，此后该机器的命令立即结束，直到有人登录这台机器。没有 TUI 在运行时，标准输入和标准错误都是控制台的命令会在控制台提出每个登录问题；其他命令（例如将标准输入重定向自空设备的 Agent）立即以 `auth-required` 结束（请先配置密钥登录）。`machine disconnect <id|label>` 断开连接，不停止远端 Session。使用 `--machine <name-or-id>` 转发命令；它与 `--session` 一样，可放在 `--` 之前的任意位置。它与本地 `--session` 冲突；TUI 和终端流不转发。远端 cwd 指向远端路径，文件与 stdin 输入在本地读取。删除机器不会停止远端 Session。`--machine local` 表示这台电脑。`workspace set-machine <workspace> --to <name-or-id|local> --dry-run` 列出移动将结束的终端，每个终端带有 `activity`（`idle`、`running` 或 `unknown`），只有在都不可能正在运行程序时 `idle` 才为 true；没有 `--confirm` 时，命令会带着这些信息拒绝执行。加上 `--confirm` 后，工作区会以原名称和项目在那台机器上新建，并在原来的机器上关闭，这会结束这些终端。文件不会被移动：那台机器上存在相对主目录的同一文件夹时从该文件夹开始，否则从主目录开始。`--machine` 指定工作区当前所在的机器；只有这条命令在使用 `--machine` 时仍接受 `--session`，用来指定这台电脑上的 Session。仅当用户要求时才移动工作区，不要自行添加 `--confirm`。

## 关闭、停止与恢复

破坏性命令要求其声明的 `--confirm`。关闭 Workspace 不删除 Git 检出或分支。回执要求恢复时，使用 `operation status|resume <request-id>`，并保留请求 ID 与 generation。不要通过创建第二个操作隐式重试。

仅当用户在当前对话中要求停止时，才停止 Server 或 Session；不要自行添加 `--confirm`。`server stop --confirm` 或 `session stop <name> --confirm` 在最终检查点写入后结束 Session。启动会恢复符合条件的布局和启动事实，不恢复实时屏幕历史或活跃 Agent 状态。CLI 等待失败／取消后，停止操作可能仍在进行；应查询保留的请求身份。`orchbee --machine <name-or-id> server stop --confirm` 通过后台连接进程停止该已保存机器的远端 Session 及其中的所有终端；它不接受请求标识、重试、后台执行或超时选项，因此 `unknown` 结果没有可查询的身份。

在 Windows 上，正式版的 `server stop`、`session stop` 和 `session delete` 必须由人执行：只有在交互式控制台中由人输入 Session 名称后才会运行，并拒绝 `--confirm` 以及任何智能体、脚本或管道调用。不要在正式版中尝试执行它们，应把需要亲自运行的命令告诉用户。开发版和测试版保留 `--confirm`。每次请求无论接受或拒绝都会记录。

`session delete <name> --confirm` 仅删除已证明停止且无使用者的自有状态。它拒绝默认 Session、存活或所有权不确定的状态，不删除项目目录。中断删除必须先解决报告的原因，再重复该操作。

## 消费结果并保留 UTF-8

在命令声明支持时使用 `--format json`。普通查询输出一个文档；事件／终端流输出 NDJSON，attach 为交互式。本地化诊断写入 stderr。结合文档和退出码判断结果：2 用法，3 目标／拒绝，4 不可用，5 拒绝授权，6 不兼容／协议，7 超时，8 结果未知，9 集成拒绝／失败，130 取消。读取 `schema`、`version`、完整性及操作自身的 outcome。因视图在执行期间变化而被拒绝的命令（`revision-mismatch`、`stale-view`，或报告为 `changed` 的 Agent 读取），CLI 已在目标仍解析为同一对象时重新读取并最多重新提交三次；遇到带此原因的退出码 3 时，先查看当前状态再决定是否重新运行，不要循环重试。`agent list` 只有在同一截止时间内最多进行三次全新读取后才报告 `changed`。其他位置的变化（例如在另一个 Workspace 中打开 Tab）抢先于 `agent wait` 对目标的解析或对终端 Agent 的一次采样时，`agent wait` 会重新读取，直到它自己的截止时间。人类表格超过 80 个单元格宽度时使用带标签的记录，使完整 ID 可复制。

新建托管 PowerShell 终端将原生管道编码初始化为 UTF-8。外部启动的 PowerShell 消费原生输出前，应先设置其解码和向外发送的管道编码：

```powershell
$cliUtf8 = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $cliUtf8
$OutputEncoding = $cliUtf8
orchbee workspace list --format json | ConvertFrom-Json
```

## 更多能力

元数据报告、关注／确认、终端图形、布局导出／应用、通知、配置命令、清单维护、配置重载均可通过帮助发现。部分能力仅支持 Windows；准入与效果细节由已安装语法规定。

`update` 把所在频道的最新版本安装在当前版本旁边；运行中的终端和智能体保持原版本。其 JSON 中 `upToDate` 为 true 表示没有更新的版本，未做任何改动。
<!-- unreleased -->

channel 选择也可通过帮助发现。`channel set` 保存选择并发起更新，因此应同时检查保存的设置和更新结果。
<!-- /unreleased -->

详细行为与维护契约见[原生 CLI 参考](native-cli-query.md)和[应用 README](../README.md)。可执行能力发现以已安装构建的帮助和 Schema 为准。

## 等待 Shell 命令或新输出

在新初始化的受管 Shell 终端中（Windows 上为 PowerShell，macOS 上为 zsh 或 bash），确认命令行空闲且为空后，使用 `terminal run <target> <command> --wait --timeout 30000 --format json`。它只提交一次，并等待本次调用的 Shell 标记。分别检查 `submission`、`completion`、`failure` 和 `cleanup`。`statusKind: powershell-success` 表示 PowerShell 成功状态（0/1），不是原生进程的数值退出码；`shell-exit-status` 表示 zsh 或 bash 的退出状态（`$?`）。超时／取消不会撤回已接受输入，不要重放。

要排除保留输出，保存 `terminal read --format json` 的 `result.cursor`，向 `terminal wait-output` 传入 `--after <cursor>`，或在新工作前使用 `--after now`。只匹配后续物理输出行，排除当前行和提示符／输入行。重置、淘汰和窗口间隙会明确拒绝游标；提前增大 `--lines`，自动化使用有限 `--timeout`。参见[命令提交](native-terminal-run.md)。

## 无头布局操作

显式目标使共享布局操作无需 TUI。省略 `--client`，使用 `panel layout <panel>`、
`layout export --tab <tab>`、`layout set-split-ratio --tab <tab> --path root --ratio 0.6`
以及 `panel swap <panel> --with <other-panel>`。共享回执保留规范身份及结果，不声明视口或焦点。
`layout apply --file layout.json --workspace <workspace>` 也可无头运行；支持显式 Workspace 和已确认的 Tab 目的地。
添加 `--client` 可获取 Client 几何／观察，应用时添加 `--focus` 可请求 Client 聚焦。
方向邻居、resize 和 `panel maximize` 需要 Client 几何。只有 stop、close 和 worktree 命令接受 `--request-id` 或 `--detach`。
修改结果未知时，先检查回执再重新提交。

## 一致的命令词汇

Panel 命令族使用位置目标或 `--current`；Agent 使用 `--panel` 或 `--terminal`，终端可为子终端。
四向方向为 `left|right|up|down`，Tab 顺序使用 `--order previous|next|first|last`。
显示标签的创建／重命名使用 `--label`。Provider 作为操作数传入，`agent start` 中为 `antigravity`。事件使用 `panel.*`，
按键使用 `ctrl+c` 等规范加号修饰符。close 结束活动资源，delete 销毁存储记录，remove 移除
成员关系，dissolve 保留分组中的子项。
<!-- unreleased -->

uninstall 移除已安装插件。
<!-- /unreleased -->

## 恢复异步操作

Workspace、Tab、Panel、Project、Worktree、Client、Session 停止和 Agent 检测规则操作都使用回执 ID 调用 `operation status <request-id>`，无需写出所属类别。`operation resume <request-id> --generation <n> --confirm` 继续可恢复的操作，先检查状态。未完成的 Workspace、Tab 或 Panel 关闭会在 Server 上自行继续，最多约三次、每次 30 秒的尝试，之后才报告一次失败；关闭最终停止时，再用 `operation resume` 继续。[保留操作状态](native-operation-status.md) 负责身份映射、查找顺序和边界。

## 终端输入输出的 Panel 目标

`terminal run/read/send/wait-output` 接受终端 ID 或 `@terminal:`、Panel ID 或 `@panel:`，也接受 `--panel <panel>`。Panel 目标在一次已准入快照中选择主 Presentation，绝不改选子终端或改变焦点。子终端需显式使用其终端 ID。Panel 缺失、无绑定或歧义时在输入前失败。裸 ID 同时指向不同 Panel 和 Terminal 资源时拒绝，需类型化句柄消歧。响应保留实际终端及位置身份。示例：`terminal run --panel @panel:0x1 "git status"`。普通目标与 `--panel` 互斥。
