---
name: orchbee
description: 通过注入的 CLI 检查或控制当前 Orchbee 应用。当用户明确要求检查 Orchbee Project、Directory、Workspace、Tab、Panel、Terminal、有界 Terminal 输出或已观测 Agent Session，跟随或等待 Agent 状态变化，创建 Orchbee Terminal，或调用已准入的 Orchbee 操作时使用。需要 ORCHBEE_CLI。
---

# Orchbee

Orchbee 通过注入 Desktop 所创建 Terminal 的精确 CLI 路径公开服务端拥有的产品状态。
已安装 CLI 及其结构化响应是权威。

使用 Orchbee 前，在当前 shell 中验证调用者上下文：

```sh
test -n "${ORCHBEE_CLI:-}"
```

```powershell
if (-not $env:ORCHBEE_CLI) { throw 'Orchbee CLI context is unavailable.' }
```

如果检查失败，说明该 shell 没有 Orchbee CLI 上下文并停止。不要搜索 `PATH`、猜测应用运行时目录、
检查私有 locator 文件或连接 Terminal Host。
如果这些指令是复制而来，而不是当前 binary 打印的，请用 `"$ORCHBEE_CLI" --skill` 刷新；
在 PowerShell 中使用 `& $env:ORCHBEE_CLI --skill`。以下示例使用 POSIX 语法；Windows 上每条命令都使用
相同的 PowerShell call operator。

## 发现产品状态

以下命令是只读的：

```sh
"$ORCHBEE_CLI" status
"$ORCHBEE_CLI" project list
"$ORCHBEE_CLI" directory list --project <project-id>
"$ORCHBEE_CLI" workspace list --project <project-id>
"$ORCHBEE_CLI" tab list --standalone
"$ORCHBEE_CLI" tab list --project <project-id> --workspace <workspace-id>
"$ORCHBEE_CLI" panel list --standalone --tab <tab-id>
"$ORCHBEE_CLI" panel list --project <project-id> --workspace <workspace-id> --tab <tab-id>
"$ORCHBEE_CLI" terminal list --standalone
"$ORCHBEE_CLI" terminal list --project <project-id> --workspace <workspace-id>
"$ORCHBEE_CLI" terminal read <presentation-id> [--rows <1-1000>]
"$ORCHBEE_CLI" agent list
"$ORCHBEE_CLI" agent get <session-id>
"$ORCHBEE_CLI" agent explain <session-id>
"$ORCHBEE_CLI" agent changes
```

Workbench list 使用 `--limit <1-200>`，Agent list 使用 `--limit <1-100>`，然后把返回的
`nextCursor` 传给同一 query 继续分页。`agent list` 默认返回 live Session；`--all` 还会包含保留的
ended Session。只有在发现报告多个协调器且已知一个精确可信 endpoint ID 时，才使用
`--target <endpoint-id>`。

`tab list` 返回一个精确 location 的规范共享 Tab 成员关系。只能把返回的 `tabId` 用于 `panel list`；
该命令返回同一 location 与 Tab 中规范的深度优先、从左到右 Panel 叶节点。Panel `position` 只描述该
revision，不是持久目标。两条命令都不报告 active 或 selected Tab、focused 或 maximized Panel、active
child、split weight、pixel geometry、renderer state 或 screen content。

## 读取有界 Terminal 输出

只使用 `terminal list` 返回的精确 `presentationId`：

```sh
"$ORCHBEE_CLI" terminal read <presentation-id>
"$ORCHBEE_CLI" terminal read <presentation-id> --rows <1-1000>
```

默认读取最新保留的 80 行。`.result.execution.result.output` 中的结果包含最多 128 KiB 的 `text`、
精确 generation 与 revision、row 边界、active-screen identity 和 `truncated`。软换行行会被拼接。
格式只有纯文本：没有 ANSI、style、graphics、hyperlink metadata、raw VT stream 或
screen-matcher evidence。

这是一次 capture-only read。它绝不会 focus、start、resize、write、paste、close 或 stop Terminal。
已连接的 SSH Terminal 可以读取，但命令绝不连接断连的 Environment；保留的本地 archive 也可以读取。
Revision 变化、target 退役或 runtime 不可用都会显式返回，绝不重试或转换成较旧输出。

Terminal 输出可能包含秘密，且不会自动脱敏。只有用户请求或获准任务明确需要时才读取；不要回显、
持久化或转发无关内容。此命令不会等待未来输出。除非另一个单独获准的工作流拥有持续时间、停止条件与
披露边界，否则不要创建轮询循环或模拟输出等待。

默认输出是一个紧凑、带版本的 JSON 文档。高层命令完成后，所有者结果位于
`.result.execution.result.output`。从该输出读取 ID；绝不根据名称、列表顺序、路径、示例或另一客户端
聚焦的 UI 推导 ID。`--output text` 是供人阅读的美化 JSON，不是稳定的自动化格式。

## 观察 Agent

只选择 `agent list` 或其他 canonical Agent result 返回的精确 `sessionId`。绝不根据 provider name、
presentation label、path、process identity、screen text、list position 或 focused UI 解析 Session。使用以下
命令检查一个保留的 Session 或其 status basis：

```sh
"$ORCHBEE_CLI" agent get <session-id>
"$ORCHBEE_CLI" agent explain <session-id>
```

`agent get` 返回 canonical bounded snapshot 与 Agent-owned status projection。`agent explain` 报告
source、fidelity、freshness、generation、sequence、capability ceiling 与 canonical status basis。两条
命令都不返回 prompt、transcript、command、tool content、filesystem path、environment value、provider
error 或 screen-matcher evidence。

使用以下命令等待同一 Session 的 canonical condition：

```sh
"$ORCHBEE_CLI" agent wait <session-id>
"$ORCHBEE_CLI" agent wait <session-id> --until needs-attention --until failed
"$ORCHBEE_CLI" agent wait <session-id> --after-revision <revision> --timeout <milliseconds>
```

默认 condition 是 `needs-attention`、`failed`、`succeeded`、`canceled`、`interrupted`、`idle` 与
`ended`。显式 condition 还可以包含 `working`、`unknown`、`fresh`、`stale` 或 `disconnected`。
Semantic-status condition 只在 observation fresh 时匹配；如果需要 degraded state 作为结果，应显式请求
health condition。`--after-revision` 要求严格更新的全局 Agent runtime revision。Timeout 默认为
600,000 毫秒，范围为 1 到 3,600,000。

该 wait 是 server-owned semantic execution。`SIGINT` 或 `SIGTERM` 只停止该 CLI process 的本地观察，
不会取消 waiter 或 Agent。如果之后可能需要取消，应使用 `--detach` 启动，保留返回的 request 或 execution
ID，再对该精确 ID 使用 `execution cancel`。取消 wait 仍不会取消 Agent 工作。Coordinator restart 会让
进行中的 wait 结果未知；应报告该不确定性且不得重放。

### 跟随全部 canonical Agent 变化

任务需要跨 Session 持续观察时，使用 global change feed：

```sh
"$ORCHBEE_CLI" agent changes
"$ORCHBEE_CLI" agent changes --cursor <next-cursor> --timeout <milliseconds>
```

第一次调用返回空 `baseline`。读取其精确
`.result.execution.result.output.nextCursor`，再把该 cursor 传给下一次调用。每次调用后都用新返回的
`nextCursor` 替换它：

- `changes`：消费有序 `items`；`hasMore` 为 true 时立即重复，否则再次长轮询；
- `current`：timeout 前没有 canonical transition；仅在仍需要观察时继续；
- `reset-required`：cursor 已落后于内存保留的 512 条 change。运行 `agent list --all` 获取新的 canonical
  snapshot，报告连续性已经丢失，再用 reset result 的 cursor 继续。绝不能静默跨过 gap。

每个 item 是一个 applied runtime revision，包含 canonical transition kind 与 content-free Session
summary。它不包含 prompt、transcript、tool、path、terminal output 或 screen evidence。默认 timeout 为
30,000 毫秒，上限为 600,000。Cursor 绑定当前 coordinator generation，并在 restart 后失效。取消或中断
feed wait 绝不会取消 Agent 工作。

## 创建 Terminal

Terminal 创建是产品修改。仅当用户请求新 Terminal，或已授权任务中的工作流明确需要时使用：

```sh
"$ORCHBEE_CLI" terminal create --standalone
"$ORCHBEE_CLI" terminal create \
  --project <project-id> \
  --workspace <workspace-id> \
  [--directory <directory-id>]
```

创建会返回精确的 `tabId`、`panelId`、`presentationId` 与 `terminalContextId` 值。它不会启动 Agent、
提交 Prompt、发送终端输入或授予 Terminal Host 访问权。

只有当调用者会保留并观察返回的请求或执行身份时才使用 `--detach`。修改结果丢失或未知时，
不得用新的请求身份重试。

## 使用通用 operation 控制面

优先使用上面的高层命令。当用户请求的已准入能力没有高层命令时，先检查实时目录再调用：

```sh
"$ORCHBEE_CLI" operation list
"$ORCHBEE_CLI" operation describe <operation-id>
"$ORCHBEE_CLI" operation invoke <operation-id> --input-file <json-file|->
```

只使用精确返回的 operation ID 和符合其所有者契约的输入。目录中存在操作并不会覆盖用户意图、
授权、确认或任务范围。绝不改用任意 shell、文件系统、renderer Action、私有 socket 或 Terminal Host 请求。

## 观察 execution

```sh
"$ORCHBEE_CLI" execution get <request-or-execution-id>
"$ORCHBEE_CLI" execution wait <request-or-execution-id>
"$ORCHBEE_CLI" execution cancel <request-or-execution-id>
```

`execution cancel` 请求协作取消一个仍活跃且尚未提交的语义执行。它不会停止 Terminal 进程、
取消 Agent 工作或逆转所有者已经提交的效果。如果 CLI 报告 `outcome-unknown-after-restart` 或回执不可用，
应报告不确定性且不得重放修改。

## 当前限制

已安装命令面尚不提供 Agent prompt、launch、approve、answer、resume、interrupt、stop、orchestration
或按键输入；Terminal 输出等待、输入、ANSI/raw 导出或持久输出历史；Tab 或 Panel split、move、focus
或 close；以及远程控制。Agent
observation 在 Desktop restart 后仍不连续。不要通过屏幕推断或私有协议模拟缺失能力。
