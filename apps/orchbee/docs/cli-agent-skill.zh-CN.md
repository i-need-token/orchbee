# Orchbee 原生 CLI Agent 指引

## 发现当前构建

使用 `orchbee --help`、`orchbee help <family>` 和 `orchbee help <family> <command>` 获取精确语法。`orchbee --version`、`orchbee --skill`、`orchbee completion powershell` 和 `orchbee api schema` 均可离线使用。`orchbee api schema <schema-id>` 输出内嵌结果 schema。原生程序与 Desktop CLI 的能力不同；应读取实际调用程序的帮助。

不带参数的 `orchbee` 会启动交互 TUI。自动化应使用显式命令。帮助和补全不启动 Site。`orchbee site start --format json` 显式启动或连接已配置的本地 Site。现有运行时选择使用 `ORCHBEE_RUNTIME_BASE`；不要猜测端点路径，也不要仅为查看帮助而激活 Site。

## 解析目标和检查状态

使用 `orchbee status --format json` 或 `orchbee api snapshot` 检查运行中的 Site，再使用相应的 `project`、`workspace`、`tab`、`panel`、`terminal`、`worktree` 或 `agent` list/get 命令。遵循各命令声明的过滤参数，使用返回的稳定 ID。仅在帮助声明支持时使用标签和后缀选择器；有歧义的目标必须明确解析。

`orchbee terminal read <terminal-id> --history --lines 50 --unwrap --format json` 读取保留输出。输出可能含有秘密，只转发任务需要的内容。读取/查询命令不会创建缺失资源。当前构建中，创建 Workspace、接纳目录和创建初始 Tab 是独立命令；必填参数以各自帮助为准。

## 单次发送有界输入

`orchbee terminal send <terminal-id> --text "echo ready" --enter --format json` 提交文字和一个回车。文字按 UTF-8 计数，连同可选回车不超过 4096 字节。重复 `--key` 可发送语义按键序列；修饰键可用 `ctrl+shift+a` 等组合。最多接受 64 个按键。

必须恰好选择一种来源：`--text`、重复的 `--key`、`--file <path>` 或 `--stdin`。文件/stdin 原样保留字节，包括非 UTF-8 数据，连同附加回车不超过 1 MiB。`--paste` 仅用于文件/stdin，内容上限为 256 KiB；此时 `--enter` 在 paste 后附加独立的 Enter 记录。完整读取和校验来源后才发送任何内容，空输入被拒绝。读取和传输共用五秒期限。

分别检查结果中的输入与清理结论。部分接受不等于整条请求成功。结果未知（退出码 8）时，输入可能已经送达：采取下一步操作前检查终端，绝不盲目重放请求。发送不会获取 TUI 的 controller 或 resize 所有权。

## 观察 Agent 和关闭资源

使用 `agent list/get/explain` 获取 canonical session 事实，使用声明了条件、超时及可选 `--after-revision` 的 `agent wait` 等待后续状态。`terminal wait-output` 可能匹配已有保留输出。旧的匹配行或 idle 状态都不能证明新提交的任务完成。当前构建没有 `agent start` 或 `agent prompt` 命令；不要虚构这些命令，也不要根据终端创建成功推断 Agent 已就绪。

当前构建在 Windows 上仍不提供 integration 管理、`agent doctor` 和 `site diagnose`。支持 Windows 终端启动/发送/读取，不代表这些命令可用。其他不支持的操作返回类型化失败；不要用猜测的状态替代失败。

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
`--current` 同用。`api schema` 现在返回第 10 版目录 envelope；新增的
`orchbee.panel.current` 文档为第 1 版。
