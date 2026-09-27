# 原生 CLI 工作流

Windows 目录工作区快速开始及已有 Linux 原生 `orchbee` CLI 的无头序列：启动
本地 Site、建立 Project/Workspace/目录、拉起并驱动终端、查看
worktree、观察 Agent，以及干净地关闭。命令清单见
[README](../README.zh-CN.md)；完整语法、target 规则与 failure matrix
由 [native CLI query](native-cli-query.zh-CN.md) 持有。本指南只覆盖受
支持的端到端路径——选项与 target 措辞与 `orchbee help` 的输出一致，
shell 片段仅作示意。

## Windows 目录工作区

使用 [Windows 快速开始](../README.zh-CN.md#windows-原生快速开始)中的完整 Windows 开发 cohort。替换下方可执行文件路径和现有目录路径。PowerShell 5.1 与 7 可直接读取 JSON，无需 jq。

```powershell
$cli = 'C:\path\to\cohort\orchbee.exe'
function Invoke-OrchbeeJson([string[]]$Words) {
    $output = & $cli @Words --format json
    if ($LASTEXITCODE -ne 0) { throw ($output -join "`n") }
    ($output -join "`n") | ConvertFrom-Json
}
Invoke-OrchbeeJson -Words @('site', 'start')
$project = Invoke-OrchbeeJson -Words @('project', 'create', '--label', 'demo')
$created = Invoke-OrchbeeJson -Words @('workspace', 'create', '--project', $project.result.projectId,
    '--label', 'main', '--cwd', 'C:\existing\repo', '--env', 'DEMO=hello')
$created.workspace.result
& $cli workspace get $created.workspace.result.workspaceId --format json
& $cli tui
# Close this Workspace when finished:
& $cli workspace close $created.workspace.result.workspaceId --confirm --format json
# Or stop the whole Windows Site and all its terminals:
$stopped = Invoke-OrchbeeJson -Words @('site', 'stop', '--confirm')
& $cli site stop-status $stopped.result.requestId --format json
```

使用终端前检查 `workspace.result.terminal.outcome`：`started` 确认启动；pending、failed、unavailable 或 unknown 需要根据返回 ID 查询资源。退出 TUI 不会关闭 Workspace。命令自动创建首个终端，并保持客户端焦点。Windows Codex 接入使用 `agent integration enable --provider codex`；`site diagnose` 报告 Windows 本地运行时事实，不激活 Site。下方编号的 POSIX 示例描述已有 Linux 路径。

## Windows Agent 诊断

```powershell
& $cli agent doctor --provider codex --format json
& $cli agent doctor --provider codex --asset-root 'C:\path\to\cohort\agent-observation-adapters'
```

这些命令完全离线，不改变配置。它们检查原生用户配置目录（显式配置时使用 `CODEX_HOME`）、
PATH 上的 provider 条目、Node 及声明的胶囊完整性。第二条命令增加一个显式胶囊检查根，
不会安装它。版本 3 JSON 和本地化文本报告 `windows-endpoint-not-inspected`。Capsule 0.4.16
新增 Windows Codex hook，Windows x64 cohort 包含其所需的固定 ConPTY 运行时。
Doctor 不执行该传输。已安装 hook 或 `complete:true` 不代表 Agent 看板已可用。
已支持托管 enable/disable 和 `agent start`/`prompt`；参见 [Windows 接入](windows-agent-integrations.zh-CN.md)与[启动](windows-agent-start.zh-CN.md)。

## 前提条件

- 在 Linux 上从仓库根目录构建的**开发 cohort**（未打包）——见
  [Linux local startup](../README.zh-CN.md#linux-local-startup)：

  ```sh
  node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
  ```

  然后运行 `/tmp/orchbee-native-dev/orchbee`。staged cohort 是四个必须
  出自同一次构建的可执行文件（CLI、Server、Worker、Host），外加观察适配器
  负载目录，全部由清单钉住；这些前提适用于下方 Linux 示例，Windows 流程见上文。
  这不是已安装产品或系统服务。

- 若按下方片段提取 ID，需要 `PATH` 上有 `jq`；否则请手动从
  `--format json` 输出中读取相同字段。
- 角色区分：`config check`、`site diagnose`、`api schema`、
  `agent doctor` 与整个 `agent integration` 命令族纯属本地（不连接、
  不加锁、无 Site 副作用）；`help`、`completion` 与 `--version` 输出
  固定文本，不触碰运行时状态；`site start` 执行唯一一次有副作用的
  激活；其余每条命令——包括裸 orchbee/`tui` 入口的
  connect-or-activate——都与本地 Site 通信。
- 只有当运行中的 Site 从已启用的受支持 Agent 集成中观察到会话时，
  Agent 行才会出现。provider 接入即第 6 节的 `agent integration` 命令
  族——本地、有界、绝不接触 Site——且没有 `agent launch`；已启用的
  provider 可执行文件与其他程序一样，经第 3 节的通用 `--exec` 途径
  启动。

## 1. 检查本地输入并启动 Site

```sh
orchbee config check   # 仅检查本地输入；任一检查失败以 1 退出
orchbee site start     # 共享一个 20 秒预算；"started" 或 "already-running"
orchbee status         # 运行中 Site 的 Workbench 快照
```

`site start` 运行与 TUI 相同的连接或激活序列。被中断或超时的启动可能
遗留已提交的激活状态；后续调用将以 `rejected` 加 recovery-required
原因应答。Linux 尚无 Site 停止、修复或删除命令。Windows 支持 `site stop --confirm` 和 `site stop-status <id>`；完成停止后可再次 `site start`。停止会结束全部终端进程并丢弃 Site 内存状态。超时或取消仅结束 CLI 等待：保留请求 ID 并查询状态。仅需显式重试失败停止时使用 `--retry --request-id <id>`。

## 2. 创建 Project、Workspace 与目录

每条命令都会打印回执；继续之前先确认 `outcome` 为 `committed`——被
拒绝或失败的回执意味着停下来阅读失败封套，而不是继续串联 ID。下面的
`jq -e` 在字段缺失时以非零退出，配合 `set -e` 或逐步检查即可中止序
列。

```sh
PROJECT=$(orchbee project create --label demo --format json | jq -er .result.projectId)
WS=$(orchbee workspace create --project "$PROJECT" --label main --format json | jq -er .result.workspaceId)
# 替换为本机上一个已存在的绝对项目目录：
DIR=$(orchbee workspace accept-directory "$WS" --path /absolute/path/to/existing-repo --format json | jq -er .result.directoryId)
```

target 是精确的 32 位十六进制 ID 或精确区分大小写的 label；歧义
label 会被拒绝并报告候选。`--path` 是字面本机绝对路径——CLI 绝不
展开 `~`/`$`、不创建、不探测；由 Server 权威探测，并拒绝已有目录的
Workspace。`accept-directory` 返回 `result.directoryId`（以及
`workerId`）：这是一个 Workbench 身份，而不是路径。

## 3. 拉起终端

```sh
TAB=$(orchbee tab create --workspace "$WS" --label shell \
  --format json | jq -er .result.tabId)
```

四个几何 flag 为全有或全无组：全部省略时请求 80x24 逻辑网格且单元像素
为未知 (0,0)——这是请求策略而非实测显示事实；提供 `--cols`/`--rows`/
`--cell-width`/`--cell-height` 中任何一个就必须提供全部四个，例如用
`--cols 80 --rows 24 --cell-width 8 --cell-height 16` 覆盖默认值。
加 `--exec /abs/path [--arg v]...` 可以为这一次
请求替换默认 shell——每个逐字参数重复一次 `--arg`，不做 shell 分词；
已安装的 provider 可执行文件也经由此途径启动，参数以该程序自身接受
的为准。用 `--standalone` 代替 `--workspace` 可以选择独立上下文，或
用 `panel split <panel-id> --direction right ...` 创建相邻 Panel；两
者接受相同的 `--exec`/`--arg` 选择。

## 4. 读取、等待并发送输入

找到绑定在新 Tab 下的终端——`terminal list` 行携带
`boundTo.tabId`、`boundTo.panelId` 与 `boundTo.presentationId`：

```sh
TERM_ID=$(orchbee terminal list --format json | jq -er --arg tab "$TAB" \
  '.result.terminals[] | select(.boundTo.tabId == $tab) | .terminalId')
```

一对确定性的「发送后等待」——`wait-output` 的文本正是上一条 `send`
确实产生的：

```sh
orchbee terminal send "$TERM_ID" --text 'echo READY-MARKER' --enter
orchbee terminal wait-output "$TERM_ID" --match 'READY-MARKER' --timeout 30000
```

其余读取与发送形式各自独立：

```sh
orchbee terminal read "$TERM_ID"                          # 当前可见帧
orchbee terminal read "$TERM_ID" --history --rows 50      # 保留的回滚历史
orchbee terminal read "$TERM_ID" --unwrap --lines 20      # 最后 20 个逻辑行
orchbee terminal wait-output "$TERM_ID" --regex 'build (ok|FAILED)'  # 另一种搜索输入
orchbee terminal send "$TERM_ID" --key ctrl-c
printf 'summarize this\n' | orchbee terminal send "$TERM_ID" --stdin --enter
orchbee terminal send "$TERM_ID" --file prompt.txt --enter
```

`send` 从恰好一种来源提交一组输入：字面 `--text`、可重复的 `--key`、
普通 `--file` 或 `--stdin`。Stdin 接受重定向普通文件和管道，包括 Windows
管道。文字上限为 4096 个 UTF-8 字节，文件/stdin 原样保留不超过 1 MiB
的字节；这些上限包含 `--enter` 并入的回车。文件/stdin 搭配 `--paste`
可接收 256 KiB 内容和可选的独立 Enter 记录。按键支持具名键及修饰键组合，
每组最多 64 条记录。来源读取和递交共用一个五秒绝对期限。
`wait-output` 匹配时退出 `0`，超时为 `7`，取消为 `130`。
结果未知的 send（退出码 `8`）可能已送达；下一步操作前先检查终端。
部分输入与清理结果独立报告。使用 `orchbee --skill` 查看内嵌 Agent 指引。

## 5. 查看 worktree

```sh
orchbee worktree list --project "$PROJECT" --directory "$DIR"
```

`--directory` 接受来自 `accept-directory` 或 `workspace get` 的严格
非零 32 位小写十六进制 Workbench 目录 ID——绝不接受路径或 label。该
列举为只读操作：绝不创建、移动、锁定或清理任何 worktree。分支名保留
完整的 `refs/heads/…` 形式；全零 `head` 表示未出生的仓库，`null`
标记裸仓库条目。

## 6. 接入 provider 并观察 Agent

`agent integration` 管理受支持 provider 的本地接入——检查其状态、解
释 enable 计划，以及启用或禁用受管集成。这些命令只在调用用户的机器
与 PATH 上运行：绝不接触 Site，且每次变更都保持有界、非交互。

```sh
orchbee agent integration list                     # 每个 provider 的本地状态
orchbee agent integration explain --provider codex # enable 将执行的计划
orchbee agent integration enable --provider codex
orchbee agent integration disable --provider codex
```

只有经核实归 Orchbee 所有的条目才会被安装或移除；外来或更新版本的
条目在任何效应前被拒绝。

在受支持的集成已启用、且 Site 正在观察会话的前提下——以下任何命令
都不执行 provider 生命周期或控制操作：

```sh
orchbee agent list                          # 仅存活会话
orchbee agent list --include-ended --limit 20
orchbee agent get <session-id>
orchbee agent explain <session-id>          # 状态与新鲜度的推导依据
orchbee agent wait <session-id>             # 轮询直到条件命中
orchbee agent wait <session-id> --until ended --until failed
```

会话 ID 是 `agent list` 打印的不透明身份，绝不是 label；某页的
`nextCursor` 令牌用 `--cursor` 继续。这些读取不提交任何 Workbench 修
改命令或 provider 命令——不创建、不激活、不修改任何状态——已结束会
话只在 Site 保留期间可查询。已关闭的终端绑定不可导航。
`agent wait` 在一个持有的 Client Session 上轮询单个会话，直到某个
`--until` 条件命中或 `--timeout` 到达（默认 600000 毫秒）：状态条件
要求 `fresh` 新鲜度，`fresh`/`stale`/`disconnected` 直接匹配观察，
`--after-revision` 以首个返回的全局修订为栅栏。它在每次轮询采样当
前状态——在两次已完成观察之间开始又结束的跃迁会被错过。

## 7. 干净地关闭

```sh
orchbee tab close "$TAB" --confirm          # accepted；Site 异步完成关闭
orchbee panel close <panel-id> --confirm
orchbee workspace close "$WS" --confirm     # 关闭整个 context 的终端
```

`--confirm` 为必填。`accepted` 回执携带 `closeRequestId`，由 Site
异步完成关闭。`workspace close` 在 Workspace 的 context scope 上复用同一
lifecycle，清理完成后移除 Workspace 及其 terminal context，保留
checkout、branch 与磁盘目录。保留的操作即使在 Workspace 离开 active projection 之后仍可
凭严格稳定 ID 查询：

```sh
orchbee workspace close-status --project <project-id> --workspace <workspace-id>
orchbee workspace close-resume --project <project-id> --workspace <workspace-id> \
  --request <close-request-id> --generation <n> --confirm
```

`close-status` 报告 scope lifecycle 及每个被捕获 member 的状态，最终以
`closed` tombstone revision 落定；`close-resume` 仅在可恢复的保留操作上开启
一个新的尝试窗口——陈旧的 `--generation` 以 `rejected`/`stale-generation` 作答，
未知结果不可恢复。关闭 Tab 不会停止 Site，本指南中也没有任何删除运行时
状态的步骤。

## JSON 与自动化说明

- `--format json` 在接受它的命令上选择机器封套——上述各 family 以及
  `status`、`site start`、`config check`；`help`、`completion` 与
  `--version` 不接受它。失败封套的 stdout 文档同样需要显式的
  `--format json`。
- u64 字段序列化为十进制字符串（例如 `"revision": "9"`）。
- 所有 target 规则、来源上限、截止时间预算与退出分类的权威出处是
  [native CLI query](native-cli-query.zh-CN.md) 与
  `orchbee help <family> <command>`。

原生 Windows Cursor 使用 `agent integration enable --provider cursor`，再运行 `agent doctor --provider cursor`。这会配置会话身份 Hook；工作和空闲识别属于独立能力。
