# Orchbee 原生 CLI Agent 指引

用 `agent rename <session-id> <name>` 设置唯一运行时别名，`agent names` 列出绑定。`agent get <name>`、`agent explain <name>` 与 `agent wait <name>` 可直接解析名称；等待固定原会话，不因后续名称复用而改变目标。`agent resolve <name>` 也可显式取得完整 ID。名称与会话 ID 冲突时，`--name <alias>` 明确选择别名，`--session <id>` 明确选择 ID。`agent rename <target> --clear` 只释放名称。会话结束或 Site dispose 后名称消失。重命名回复丢失是结果未知，禁止盲目重放。

## 发现当前构建

使用 `orchbee --help`、`orchbee help <family>` 和 `orchbee help <family> <command>` 获取精确语法。`orchbee --version`、`orchbee --skill`、`orchbee completion powershell` 和 `orchbee api schema` 均可离线使用。`orchbee api schema <schema-id>` 输出内嵌结果 schema。原生程序与 Desktop CLI 的能力不同；应读取实际调用程序的帮助。

不带参数的 `orchbee` 会启动交互 TUI。自动化应使用显式命令。帮助和补全不启动 Site。`orchbee site start --format json` 显式启动或连接已配置的本地 Site。现有运行时选择使用 `ORCHBEE_RUNTIME_BASE`；不要猜测端点路径，也不要仅为查看帮助而激活 Site。

## 解析目标和检查状态

使用 `orchbee status --format json` 或 `orchbee api snapshot` 检查运行中的 Site，再使用相应的 `project`、`workspace`、`tab`、`panel`、`terminal`、`worktree` 或 `agent` list/get 命令。遵循各命令声明的过滤参数，使用返回的稳定 ID。仅在帮助声明支持时使用标签和后缀选择器；有歧义的目标必须明确解析。

`orchbee terminal read <terminal-id> --history --rows 50 --lines 50 --unwrap --format json` 读取保留输出。输出可能含有秘密，只转发任务需要的内容。读取/查询命令不会创建缺失资源。使用 `workspace create --project <target> --label <label> --cwd <absolute-path>`，一次请求接纳目录并创建 Workspace 和首个终端。分别检查资源提交和终端启动结果。

`orchbee agent read --name alpha --lines 50 --format json` 读取存活命名会话的可见终端。
添加 `--history --rows 50` 则改为读取保留的滚动历史。结果使用终端 read/history schema，
标识实际读取的终端。

## 单次发送有界输入

`orchbee terminal send <terminal-id> --text "echo ready" --enter --format json` 提交文字和一个回车。文字按 UTF-8 计数，连同可选回车不超过 4096 字节。重复 `--key` 可发送语义按键序列；修饰键可用 `ctrl+shift+a` 等组合。最多接受 64 个按键。

必须恰好选择一种来源：`--text`、重复的 `--key`、`--file <path>` 或 `--stdin`。文件/stdin 原样保留字节，包括非 UTF-8 数据，连同附加回车不超过 1 MiB。`--paste` 仅用于文件/stdin，内容上限为 256 KiB；此时 `--enter` 在 paste 后附加独立的 Enter 记录。完整读取和校验来源后才发送任何内容，空输入被拒绝。读取和传输共用五秒期限。

分别检查结果中的输入与清理结论。部分接受不等于整条请求成功。结果未知（退出码 8）时，输入可能已经送达：采取下一步操作前检查终端，绝不盲目重放请求。发送不会获取 TUI 的 controller 或 resize 所有权。

`orchbee agent send-keys --name alpha enter ctrl+c --format json` 直接定位 Agent 终端。
也支持完整会话 ID。绑定缺失、会话结束、不可导航或绑定有歧义时拒绝输入。沿用相同的
序列边界和发送回执。

## 观察 Agent 和关闭资源

使用 `agent list/get/explain` 获取 canonical session 事实，使用声明了条件、超时及可选 `--after-revision` 的 `agent wait` 等待后续状态。`terminal wait-output` 可能匹配已有保留输出。旧的匹配行或 idle 状态都不能证明新提交的任务完成。使用 `agent prompt <target> <text> --wait` 提交字面 paste 加 Enter，先观察活动，再匹配条件。有待处理请求、状态过期或未知时拒绝。活动停滞、等待超时或取消仍保留已接受回执；再次操作前请检查回执。已在 working 的 Agent 可能结束既有轮次，匹配不保证本次提示自己的轮次完成。`--until` 和 `--timeout` 要求 `--wait`，默认超时 600000 毫秒。使用 `agent start alpha --kind codex --panel <id>` 在空闲 Windows 终端启动并等待就绪验证。默认超时 30000 毫秒，允许 3001–300000。未指定 `--panel` 时要求有效调用者上下文。`--` 后参数按字面传递，也可重复使用 `--arg=<value>`。`agent names` 可查看启动预留。超时或取消不会终止进程；保留输入与预留事实，不要盲目重放。

Windows 上可用 `agent doctor` 离线检查本机配置。`agent integration list/explain/enable/disable --provider <id>` 管理 Codex、OpenCode、Pi、OMP（`codex`、`opencode`、`pi`、`omp`）；检查回执后再 list 确认观测状态。其他 provider 仍为 manual-only。Windows `site diagnose` 使用 `orchbee.site.diagnose.windows` v2 报告只读运行时、锁和管道名称事实。其他不支持的操作返回类型化失败；不要用猜测的状态替代失败。

破坏性的 close/delete 命令需要 `--confirm`。关闭 Workspace 不会删除其 Git 检出或分支。关闭结果需要恢复时使用 Workspace close-status/close-resume。退出一个 TUI 客户端不等于停止 Site。

## 消费结果

在命令声明支持时使用 `--format json`。标准输出承载一份机器文档，本地化诊断使用标准错误。同时检查文档与进程退出码。退出码 2 表示用法错误，4 表示不可用，7 表示超时，8 表示结果未知，130 表示取消。不要解析人类文案，也不要认为等待失败/取消会取消被观察的操作。保留恢复所需的稳定 ID 和请求身份。

## 在目标目录和环境中启动

`tab create` 和 `panel split` 接受 `--cwd <absolute-path>` 与可重复的 `--env NAME=VALUE`，
可配合默认 shell、显式程序或 `--profile`。值逐字保留，允许空值。
显式 cwd 覆盖已接纳的 Workspace 目录；均未提供时使用 Worker 主目录。
目录必须已存在。这些选项不修改已存储的配置。
环境至多 32 项，名称和值合计不超过 1,024 UTF-8 字节；名称为 ASCII 标识符，
忽略大小写后必须唯一。Orchbee 和终端身份变量保留。
检查结果中的终端 outcome：成员关系已提交并不证明进程已启动。
完整校验规则以原生 CLI 帮助为准。

## 定位调用者 Panel

在新启动的 Windows 终端内，使用 `orchbee panel current --format json` 查询它当前
所在的 Panel、Tab 和 Workspace。`--current` 是同一查询的显式写法。在该进程之外
使用 `panel current --panel <id-or-selector>`。Worker 提供 `ORCHBEE_TERMINAL_ID`
和 `ORCHBEE_WORKER_ID`；查询在一次快照中核实它们的当前绑定。上下文缺失或过期
时返回错误，绝不回退到客户端焦点。显式 `--panel` 忽略继承上下文，不能与
`--current` 同用。`orchbee.panel.current` 文档为第 1 版。

`agent wait` 或 `agent prompt --wait` 可显式加 `--forever`，与 `--timeout` 互斥；默认仍等待最多 600000 毫秒，单次读取始终有界。`--until blocked` 对应 fresh needs-attention，`--until done` 对应 fresh 完成结果集合，不表示未读。等待/prompt 成功为版本 2，无限模式的 timeoutMs 为 null；无限等待读取超时失败也为版本 2。Windows Codex 的提示提交在 paste 与 Enter 间加入 Right，以清除 paste-burst 对 Enter 的抑制。

面向单个 Agent 的命令还支持 `--panel <id|@panel:suffix>` 或位置参数 Panel ID。位置参数优先按 Session ID 和名称解析，再回退到 Panel。Panel 选择主终端且要求唯一存活 Agent；等待固定到选定的 Session。

## 在 Agent 输入前检查进程

`orchbee panel process-info --panel <id|@panel:suffix> --format json` 查询真实 Windows
Job。在 Orchbee 终端内可省略目标，使用经过校验的调用方上下文。v2 文档包含实时 cwd、
精确创建身份、Windows argv 投影、provider 识别和前台估计。null 字段与选择歧义明确
返回；前台估计不证明交互已经就绪。

Windows 的 `agent send-keys` 与 `agent prompt` 会核对实际运行的 provider，并在发送前
重读同一会话／presentation generation。Shell、不同或有歧义的 Agent、未知 provider
以及绑定变化都会拒绝且不输入。prompt 还会复查状态是否新鲜、已知、有待处理请求。
请检查 process-info 与 Agent 事实解决拒绝；只有明确要直接操作终端时才用 `terminal send`。
进程仍可能在预检后退出，因此需要检查发送回执。

## Windows Site 停止

`site stop --confirm --format json` 结束托管 Site 及其终端进程。可用 `--detach` 在准入后返回，保留 `requestId`，通过 `site stop-status <request-id> --format json` 查询。超时或 Ctrl+C 只结束等待。使用相同 `--request-id` 重复调用只观察既有尝试；只有 `--retry --request-id <id> --confirm` 才重试失败停止。准确匹配的已停止记录是完成回执，会被后续 `site start` 消费；未记录不能否定先前停止。绝不通过管道不存在推断完成。Linux 和 macOS 尚不支持停止。

## 保留 Worktree 操作身份

Windows 上按命令帮助使用 `worktree create/open/remove`。保留返回的操作 ID，脱离等待、超时或断线后用 `worktree status <id>` 查询；不要盲目重复不确定的 Git 副作用。同时读取检出结果与 Workspace 状态：后续 Git 移除失败时，已关闭的 Workspace 保持关闭。仅对明确可恢复的失败使用 `worktree resume <id> --generation <attempt> --confirm`。Remove 要求 `--confirm`；`--force` 允许丢弃脏检出中的改动。参见 [Worktree 契约](windows-worktrees.zh-CN.md)。

## 从来源查询 Worktree

Windows 上 `worktree list --cwd C:/code/repo --format json` 无需 Project。可改用 `--workspace <target>` 选择已注册 Workspace，或省略两者使用已校验调用者上下文／当前目录。无效调用者身份会被拒绝，绝不回退 cwd。`--trust-repository` 仅作用于本次查询。来源结果使用 `orchbee.worktree.query` v1，明确返回多个 Workspace 关联及类型化失败原因。原 `--project/--directory` 组合保留旧封套，不可混用来源 flag。两种形式均不激活 Site 或变更 Git。

Windows Worktree 的 `--cwd` 与 create/open 的 `--path` 也接受相对调用者目录的路径及前导 `~`。主目录优先级为 `USERPROFILE`、`HOMEDRIVE`+`HOMEPATH`、`HOME`；选中的值无效时会在访问运行时前失败。除此之外路径按字面处理，不做 shell 或变量展开。

## 关闭 Windows worktree 组

主 Workspace 有关联成员时使用 `workspace close <target> --group --confirm`。关联 Workspace 目标只关闭自身。Git 路径和分支保留。保留 `orchbee.workspace.group-close` 应答中的操作 ID；即使源已消失，也可使用 `workspace close-status --group --request <operation-id>`。对可恢复失败使用 `workspace close-resume --group --request <operation-id> --generation <attempt> --confirm`。检查每个捕获成员的 Close 身份和观测；`close` 为 null 表示尚未提交，子操作的 `observation` 为 null 则表示已接纳但尚未观测。绝不重放未知结果。`--detach`、超时或 Ctrl+C 只结束等待。作用域选择器 `--project`/`--workspace` 属于普通 status/resume，不能与组模式混用。

## 控制一个 Windows TUI

使用 `client list --format json` 并保留精确的 current `clientId`。存在多个视图时传入 `--client <id>`，不要选择第一个。`workspace focus <target>`、`tab focus <target>` 和 `panel focus [<target>]` 导航该 TUI。`panel zoom [<target>] --on|--off|--toggle` 改变其 Tab 缩放；默认 toggle，由 TUI 激活目标后只解析一次。不激活操作系统窗口。

保留 `orchbee.client.control` 中的 `operationId`，在 detach、超时、Ctrl+C 或提交结果未知后使用 `client control-status <operation-id>`。`queued` 和 `delivered` 尚未完成；只有 `applied`、`zoom-toggled` 或明确的单 Panel `zoom-unchanged` 确认已绘制。Incomplete 可能保留部分导航效果。查询不会再次应用命令；不要重复结果未知的 toggle。回执易失，完成后 60 秒过期。回执缺失不能证明操作没有效果。

## 离线导出 schema

Windows 上，`orchbee api schema [<schema-id>] --output <path>` 将相同 JSON 写入字面文件路径与 stdout。父目录必须存在。完整准备后才替换已有普通文件；未知 ID 不写文件。若清理失败，应保留返回的临时文件路径。发布后的 stdout 失败不会撤销文件。使用 `--format json` 获取类型化导出失败。

## 检查 Windows TUI 布局

使用 `panel layout [<panel>] --client <id> --format json` 检查单元格，不选择目标 Tab 或调整终端尺寸。`panel neighbor --direction left|right|up|down` 返回可为 null 的邻居 ID；`panel edges` 返回物理边界布尔值。两者接受相同的 Panel/Client 选择器。读取 `projection.sourceAvailable`：隐藏源不作邻居／边缘判断，退出码为 3。待处理或失败观测的 `projection` 为 null。非活动几何明确标为未选中，没有本地焦点时保持 null。三条命令均保留完整布局与操作身份；分离或等待中断后通过 `client control-status` 检查保留结果。
