---
name: orchbee
description: 通过注入的 CLI 检查或控制当前 Orchbee 应用。当用户明确要求检查 Orchbee Project、Directory、Workspace 或 Terminal，创建 Orchbee Terminal，或调用已准入的 Orchbee 操作时使用。需要 ORCHBEE_CLI。
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
"$ORCHBEE_CLI" terminal list --standalone
"$ORCHBEE_CLI" terminal list --project <project-id> --workspace <workspace-id>
```

使用 `--limit <1-200>` 和返回的 `nextCursor` 分页。只有在发现报告多个协调器且已知一个精确可信
endpoint ID 时，才使用 `--target <endpoint-id>`。

默认输出是一个紧凑、带版本的 JSON 文档。高层命令完成后，所有者结果位于
`.result.execution.result.output`。从该输出读取 ID；绝不根据名称、列表顺序、路径、示例或另一客户端
聚焦的 UI 推导 ID。`--output text` 是供人阅读的美化 JSON，不是稳定的自动化格式。

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

已安装命令面尚不提供 Agent list、read、wait、prompt、launch、approve、resume 或按键输入；
Terminal 输出读取或输入；Tab 或 Panel split、move、focus 或 close；以及远程控制。
不要通过屏幕推断或私有协议模拟这些缺失能力。
