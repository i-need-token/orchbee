# 原生 CLI 工作流

面向实验性 Linux 原生 `orchbee` CLI 的完整无头（headless）序列：启动
本地 Site、建立 Project/Workspace/目录、拉起并驱动终端、查看
worktree、观察 Agent，以及干净地关闭。命令清单见
[README](../README.zh-CN.md)；完整语法、target 规则与 failure matrix
由 [native CLI query](native-cli-query.zh-CN.md) 持有。本指南只覆盖受
支持的端到端路径——选项与 target 措辞与 `orchbee help` 的输出一致，
shell 片段仅作示意。

## 前提条件

- 在 Linux 上从仓库根目录构建的**开发 cohort**（未打包）——见
  [Linux local startup](../README.zh-CN.md#linux-local-startup)：

  ```sh
  node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
  ```

  然后运行 `/tmp/orchbee-native-dev/orchbee`。staged cohort 是四个必须
  出自同一次构建的可执行文件（CLI、Server、Worker、Host），外加观察适配器
  负载目录，全部由清单钉住；非 Linux 平台不
  被准入（`unsupported-platform`），且这不是已安装产品或系统服务。

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
原因应答。没有任何 CLI 命令可以停止、修复或删除 Site。

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
label 会被拒绝并报告候选。`--path` 是字面绝对 POSIX 路径——CLI 绝不
展开 `~`/`$`、不创建、不探测；由 Server 权威探测，并拒绝已有目录的
Workspace。`accept-directory` 返回 `result.directoryId`（以及
`workerId`）：这是一个 Workbench 身份，而不是路径。

## 3. 拉起终端

```sh
TAB=$(orchbee tab create --workspace "$WS" --label shell \
  --cols 80 --rows 24 --cell-width 8 --cell-height 16 \
  --format json | jq -er .result.tabId)
```

四个几何参数均为必填。加 `--exec /abs/path [--arg v]...` 可以为这一次
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
orchbee terminal send "$TERM_ID" --key ctrl-c
printf 'summarize this\n' | orchbee terminal send "$TERM_ID" --stdin --enter
orchbee terminal send "$TERM_ID" --file prompt.txt --enter
```

`send` 从恰好一个来源递送恰好一条输入记录：字面 `--text`、一个具名
`--key`、常规文件 `--file`，或 `--stdin`——接受重定向的常规文件或
FIFO/管道，拒绝终端、套接字或设备。承载文本的来源连同并入的
`--enter` 回车在内不超过 4096 字节。在命令入口采样的一条绝对 5 秒
deadline 同时覆盖来源物化*和*整个发送生命周期。`wait-output` 匹配成
功退出 `0`，超时退出 `7`，收到信号退出 `130`。结果未知的发送（退
出码 `8`）可能已被送达——重发前先查看终端。

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
```

`--confirm` 为必填。`accepted` 回执携带 `closeRequestId`，由 Site
异步完成关闭。关闭 Tab 不会停止 Site，本指南中也没有任何删除运行时
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
