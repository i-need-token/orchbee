# Orchbee 原生 CLI Agent 指南

本指南描述 Windows 原生 CLI 的用户流程。操作前，通过已安装构建确认精确语法与平台支持。

## 发现当前构建

从 `orchbee --help` 开始，再查看 `orchbee <family> --help` 和 `orchbee <family> <command> --help`。`--` 前任意位置的单个帮助开关，也支持已填写的操作数和选项，且不会执行它们。未知选项、缺值和重复选项仍会失败。`help <path>` 接受精确帮助路径。裸 `orchbee` 会启动 TUI。

`--version`、`--skill`、`completion <shell>`、`api schema` 和 Windows `--default-config` 均可离线使用。补全支持 Bash、Zsh、Fish、PowerShell 和 Elvish。`api schema` 列出 schema ID；用 `api schema <schema-id>` 查看同一构建的机器契约，或添加 `--output <path>` 导出。读取输出中的 `schema` 与 `version`；本指南不固定目录数量或历史版本。

PowerShell 用户可用 `orchbee completion powershell` 输出补全脚本。

## 选择并启动 Session

用 `config check` 检查本地配置，`status client` 检查本可执行文件，`status server` 检查所选 Server。这些命令不启动 Session。`server diagnose` 提供只读运行时事实，`server start` 执行无界面启动。

`--session <name>` 选择本地命名 Session，可位于命令之前或之后，默认值为 `default`。后续操作与诊断应保持相同选择。`session list` 显示本地清单；`session attach <name>` 进入其 TUI。退出 TUI 不会停止 Session。

```text
orchbee --session work server start
orchbee --session work status --format json
orchbee --session work tui
```

运行时根目录缺失属于环境检查失败；重试前查看 `config check`。Recovery-required 表示启动无法确认安全的所有权。诊断该精确 Session，或使用另一个名称。不要手动删除运行时目录。

## 解析目标并检查状态

Workspace 拥有目录，可属于 Project；Project 分组是可选的。Workspace 包含 Tab，Tab 包含 Panel，Panel 有主终端且可包含子终端。TUI 客户端各自保存选择与视图。

先用 `api snapshot`，再使用相关的 `project`、`workspace`、`tab`、`panel`、`terminal`、`launch-profile`、`client` 或 `agent` list/get 命令。复用返回的 ID。帮助允许时，可使用标签和 `@panel:<suffix>` 等 selector；匹配有歧义时必须使用精确 ID。ID 属于所选 Session 的作用域。

在 Orchbee 终端内，调用者元数据可以选择其 Panel；在外部，`panel current` 可使用唯一当前 TUI 客户端。存在多个客户端时传入 `--client <id>`；无界面查找可使用 `--panel <id>`。`agent start` 省略 `--panel` 时明确要求有效调用者元数据。部分或过期元数据不会授权回退。

## 创建和安排终端

```text
orchbee workspace create --cwd . --label work
orchbee tab create --workspace work --label shell
orchbee panel list --format json
orchbee panel split --panel <panel-id> --direction right
```

检查每份创建回执：布局成员已提交不代表进程已启动。需要客户端的命令会拒绝缺失、有歧义或尚未就绪的客户端；使用 `client list` 并显式传入 `--client`。

`tab create`、`panel split` 和 `panel create-terminal` 支持其声明的启动选择、cwd 和字面环境条目。保存的 Launch Profile 使用 `--profile`；显式程序使用 `--exec` 和可重复的 `--arg`。目录必须已存在。查看帮助中的 `--cwd home|current|workspace`、绝对路径覆盖、完整终端几何参数及 `--env NAME=VALUE` 边界。这些覆盖不修改保存的 profile。

`panel create-terminal --child` 创建子终端。使用 `workspace`/`tab` move、`panel move`/`swap`、`layout`、`panel resize`、`panel zoom` 和 focus 命令安排布局。客户端 focus 控制 TUI 选择，不激活操作系统窗口。选择当前目标或布局路径前，先阅读命令帮助。

## 发送输入并读取输出

`terminal run <terminal-id> "git status"` 提交一条字面命令并发送 Enter。`terminal send` 接受 `--text`、`--file`、`--stdin` 之一，可添加 `--paste`/`--enter`，或使用声明的按键。按键有序，最多接受 64 个。文件与 stdin 在本地读取，路由到已保存机器时也一样。来源读取上限为 1 MiB；形成的 paste 与 argv 仍受其声明的更小边界约束。

回执证明输入记录被接受，不证明 Shell 就绪、命令完成或退出状态。不要重放部分或结果未知的输入。检查回执与终端后，再决定下一步发送内容。

Windows `terminal read` 和 `agent read` 默认读取最近输出。通过 `--source` 选择 `visible`、`recent`、`recent-unwrapped` 或 `detection`；`--plain` 直接输出 UTF-8 文本，排除 JSON。旧 `--history --rows` 使用独立历史契约。不要混用不兼容的输出模式；边界、软换行和 ANSI 规则见帮助。

`terminal wait-output` 可以匹配已经存在的输出。Windows 默认没有总时限；自动化可使用 `--timeout 30000`，或显式 `--forever`。`--timeout` 与 `--forever` 冲突。匹配到一行不能证明刚提交的命令已完成。

## 安装并操作 Agent

`agent doctor` 离线检查本地接入。先用 `agent integration list`，再用 `explain --provider <id>` 发现每个提供方的实际可用性与管理模式。`enable`/`disable --provider <id>` 执行显式受保护修改。`list --outdated-only` 显示自有修复候选，并分别报告被阻止或未能确认的提供方；空列表不代表检查完整。

Windows Codex 的检查与安装使用同一配置根：非空 `CODEX_HOME`，否则为原生用户目录中的 `.codex`。无效覆盖值拒绝且不回退。提供方退出成功不证明安装成功；应读取修改回执中的观察状态。

`agent start review --kind codex --panel <panel-id>` 在已观察到空闲的 cmd、PowerShell 或 pwsh 终端启动，并等待验证后的就绪。默认时限 30000 毫秒，允许范围 3001–300000。字面提供方参数放在 `--` 后，或使用可重复的 `--arg=<value>`。超时或取消后提供方可能仍在运行；重试前检查输入回执，并用 `agent names` 查看待完成预留。

`agent list/get/explain` 检查当前 Agent 状态。Windows 上，`agent list` 会合并规范会话与物理终端 Agent，并为每行报告来源、语义状态和位置；`--client <id>` 添加独立的 Client 观察，不改变状态。每页 1–100 行，默认 50 行。Windows JSON 使用 `orchbee.agent.list` v2 和 `orchbee.agent.get` v19。Windows 列表游标使用 `u1.<base64url>`，载荷为编码成 base64url 的规范 JSON，最多 1024 个字符，并绑定 Site、来源修订、筛选条件和 Client 选择。解析器识别出的旧格式游标会在读取实时来源前拒绝；判断 `u1` 游标是否过期则需要读取当前来源。收到 `cursor-restart` 后应从第一页重新开始。每页最多为 64 行添加可选 cwd 信息；无法获取时为 null。

Windows 上，`@agent:<kind>` 选择唯一提供方，`@agent:<n>` 选择当前完整列表中的一基序号，而不是当前页的序号。裸名称仍优先匹配 Server 别名，然后回退到提供方匹配。`agent get`、`read`、`prompt`、`send-keys`、`wait` 和 `focus` 共用此解析器；不完整投影不能证明提供方匹配唯一。v19 `agent get` 将普通 Agent 身份和状态放在 `result.agent`，将各来源分面的详细信息放在 `result.inspection`。

`agent rename <target> <name>` 分配运行时别名；`agent resolve` 与 `agent names` 检查绑定，`agent rename <target> --clear` 释放别名。别名随会话结束而失效。之后复用别名不会改变已开始等待的原始目标。

`agent prompt <target> <text> --wait` 提交字面输入，并在检查条件前观察活动。`agent wait` 只观察。Windows 默认没有总时限，条件为 idle/done/blocked；自动化应指定有限时限。新鲜度、待处理关注和已有工作影响准入。匹配状态不证明某条 prompt 的专属回合已完成。取消等待不会撤销已接受的输入，也不会停止 Agent。

保留已观察的 revision；使用 `--after-revision` 等待后续状态前，先查看 `agent wait --help`。

`--until`、`--timeout` 和 `--forever` 要求 `agent prompt --wait`；`--forever`
与 `--timeout` 互斥。Windows Codex 的输入在 paste 与 Enter 之间包含 Right 键。
Linux 保留 600000 毫秒的默认等待期限。等待失败会保留已接受的输入回执，因此不要
重放结果不确定的输入。

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

`terminal session observe` 输出有界 NDJSON 帧且不取得控制权。`terminal session control` 接受 NDJSON 输入；编写记录前先查看其声明的输入 Schema。`terminal attach` 和 `agent attach` 要求真实控制台 stdin/stdout。`Ctrl+B` 后按 `q` 可脱离；连续两次前缀发送一个字面前缀。Attach 不启动 Session、不重连，也不重放输入。

## 插件与已保存机器

使用 `plugin list`、`plugin action list` 和 `plugin pane open --help` 发现已安装能力。`plugin link` 注册本地清单；`plugin install` 的 `--yes` 用于声明的非交互信任流程。注册表命令不启动 Session；调用操作和 pane 命令需要已有 Session。

`plugin action invoke <action-id> --context <json>` 接受 `{"selectedText":"example"}` 等展示覆盖。输入结构见 `api schema orchbee.plugin.runtime` 的 `$defs/context`；字段使用 camelCase，拒绝未知字段。CLI 退出后，已接纳的插件任务可继续运行。`plugin log list` 查看保留日志。

Popup 放置要求当前已就绪客户端和焦点。请移除 `--no-focus`，或选择 tab/split 放置。`popup close` 释放弹窗终端。普通 pane 与 popup 的生命周期不同，应查看对应回执。

`machine list/add/rename/enable/disable/remove` 管理已保存 SSH 配置。路由选项必须位于第一个位置：`orchbee --machine <name-or-id> <command>`。它与本地 `--session` 冲突；TUI 和终端流不转发。远端 cwd 指向远端路径，文件与 stdin 输入在本地读取。删除配置不会停止远端 Session。

## 关闭、停止与恢复

破坏性命令要求其声明的 `--confirm`。关闭 Workspace 不删除 Git 检出或分支。回执要求恢复时，使用 close-status/close-resume 或 worktree status/resume，并保留请求 ID 与 generation。不要通过创建第二个操作隐式重试。

`server stop --confirm` 或 `session stop <name> --confirm` 在最终检查点写入后结束 Session。启动会恢复符合条件的布局和启动事实，不恢复实时屏幕历史或活跃 Agent 状态。CLI 等待失败／取消后，停止操作可能仍在进行；应查询保留的请求身份。

`session delete <name> --confirm` 仅删除已证明停止且无使用者的自有状态。它拒绝默认 Session、存活或所有权不确定的状态，不删除项目目录。中断删除必须先解决报告的原因，再重复该操作。

## 消费结果并保留 UTF-8

在命令声明支持时使用 `--format json`。普通查询输出一个文档；事件／终端流输出 NDJSON，attach 为交互式。本地化诊断写入 stderr。结合文档和退出码判断结果：2 用法，3 目标／拒绝，4 不可用，5 拒绝授权，6 不兼容／协议，7 超时，8 结果未知，9 集成拒绝／失败，130 取消。读取 `schema`、`version`、完整性及操作自身的 outcome。人类表格超过 80 个单元格宽度时使用带标签的记录，使完整 ID 可复制。

新建托管 PowerShell 终端将原生管道编码初始化为 UTF-8。外部启动的 PowerShell 消费原生输出前，应先设置其解码和向外发送的管道编码：

```powershell
$cliUtf8 = [System.Text.UTF8Encoding]::new($false)
[Console]::OutputEncoding = $cliUtf8
$OutputEncoding = $cliUtf8
orchbee workspace list --format json | ConvertFrom-Json
```

## 更多能力

元数据报告、Agent 视图筛选、关注／确认、终端选择／复制／链接／图形、布局导出／应用、通知、配置命令、清单维护、配置重载／导入／重置、更新和 channel 选择均可通过帮助发现。部分能力仅支持 Windows；准入与效果细节由已安装语法规定。`channel set` 保存选择并发起更新，因此应同时检查保存的设置和更新结果。

详细行为与维护契约见[原生 CLI 参考](native-cli-query.zh-CN.md)和[应用 README](../README.zh-CN.md)。可执行能力发现以已安装构建的帮助和 Schema 为准。

## 等待 Shell 命令或新输出

在新初始化的受管 PowerShell 终端中，确认命令行空闲且为空后，使用 `terminal run <target> <command> --wait --timeout 30000 --format json`。它只提交一次，并等待本次调用的 Shell 标记。分别检查 `submission`、`completion`、`failure` 和 `cleanup`。`powershell-success` 表示 Shell 成功状态（0/1），不是原生进程的数值退出码。超时／取消不会撤回已接受输入，不要重放。

要排除保留输出，保存 `terminal read --format json` 的 `result.cursor`，向 `terminal wait-output` 传入 `--after <cursor>`，或在新工作前使用 `--after now`。只匹配后续物理输出行，排除当前行和提示符／输入行。重置、淘汰和窗口间隙会明确拒绝游标；提前增大 `--lines`，自动化使用有限 `--timeout`。参见[命令提交](native-terminal-run.zh-CN.md)。

## 无头布局操作

显式目标使共享布局操作无需 TUI。省略 `--client`，使用 `panel layout <panel>`、
`layout export --tab <tab>`、`layout set-split-ratio --tab <tab> --path root --ratio 0.6`
以及 `panel swap <panel> --with <other-panel>`。共享回执保留规范身份及结果，不声明视口或焦点。
`layout apply --file layout.json --standalone` 也可无头运行；支持显式 Workspace 和已确认的 Tab 目的地。
添加 `--client` 可获取 Client 几何／观察，应用时添加 `--focus` 可请求 Client 聚焦。
方向邻居、resize 和 zoom 需要 Client 几何。只读命令不接受 `--request-id` 或 `--detach`。
修改结果未知时，先检查回执再重新提交。
