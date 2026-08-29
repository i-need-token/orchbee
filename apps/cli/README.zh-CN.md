# Orchbee CLI

用于调用显式准入的 Orchbee semantic command 的产品所属 Node client。该 workspace 包含
transport-independent client core、严格的通用、Workbench 与 Agent command projection、版本匹配的内嵌
Agent Skill、lifecycle runner、可信本地 locator 和已认证 HTTP adapter，以及 Desktop distribution 使用的
真实 process entry。Desktop 创建的 Terminal 会获得一个精确 launcher path，而不需要全局安装。

## Command 表面

Application runner 接受以下通用 command family：

```text
orchbee status
orchbee operation list
orchbee operation describe <operation-id>
orchbee operation invoke <operation-id> --input-file <file|-> [--detach] [--request-id <request-id>]
orchbee execution get <request-or-execution-id>
orchbee execution wait <request-or-execution-id>
orchbee execution cancel <request-or-execution-id>
```

它还在生产 Desktop catalog 已准入的五个 Workbench operation 之上提供高层 projection：

```text
orchbee project list [--limit <n>] [--cursor <cursor>]
orchbee directory list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee workspace list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee terminal list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee terminal create (--standalone | --project <project-id> --workspace <workspace-id>) [--directory <directory-id>] [--detach] [--request-id <request-id>]
```

生产 Desktop catalog 还准入四个只读 Agent observation operation。其高层 projection 为：

```text
orchbee agent list [--all] [--limit <n>] [--cursor <cursor>]
orchbee agent get <session-id>
orchbee agent explain <session-id>
orchbee agent wait <session-id> [--until <condition>]... [--after-revision <revision>] [--timeout <milliseconds>] [--detach] [--request-id <request-id>]
```

`orchbee --skill` 是用于打印内嵌 Skill 的独立 coordinator-free command。

每条连接 coordinator 的 command 都接受 `--target <endpoint-id>` 与 `--output json|text`。只有可信本地
discovery 精确解析到一个 live coordinator 时，省略 target 才能成功。重复、未知或跨 command 的 option
会在连接前失败。通用 invocation 从指定文件或 stdin 读取一个严格 UTF-8 JSON value。高层 command 通过
Workbench 或 Agent 所有者导出的 schema 校验有界 argument，并恰好映射一个现有 operation。Workbench
分页接受 1-200 项；Agent 分页接受 1-100 项且默认只包含 live Session，`--all` 会包含保留的 ended
Session。无效 input 会在分配 request identity 或联系 Desktop 前被拒绝。

默认输出恰好是 stdout 上一个紧凑、带版本的 JSON document。Invoke result 始终包含 request identity。
`--output text` 是同一 document 的明确不稳定缩进 projection；automation 不得解析它。当前 runner 不会向
stderr 发出带 payload 的 diagnostic。`--skill` 是有意设置的例外：它打印同一 build 中捆绑的规范 Agent
Skill，不在运行时读取文件，也不连接 coordinator。

| Exit | 含义                                                                              |
| ---- | --------------------------------------------------------------------------------- |
| `0`  | Completed、unchanged、active、已接受并 detached observation，或 query 成功        |
| `2`  | CLI usage、option、input source、JSON 或 input capacity failure                   |
| `3`  | Blocked、unsupported、policy rejection、operation unavailable 或 request conflict |
| `4`  | Cancelled terminal result                                                         |
| `5`  | Failed terminal result                                                            |
| `6`  | Target、transport、compatibility、receipt、internal 或 unresolved-outcome failure |

## 产品启动契约

Desktop 与 launcher composition 使用以下值。Desktop 会把前两项注入其拥有的每次 Terminal launch；
第三项由 launcher 拥有：

| 名称                            | 契约                                                                                                                |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `ORCHBEE_CLI`                   | 精确的当前平台 launcher path。Agent 与 Skill 直接调用该路径；它绝不会被加入 `PATH`。                                |
| `ORCHBEE_CLI_RUNTIME_DIRECTORY` | Desktop 选择的精确 product-scoped rendezvous directory。它不从 current directory、Project 或 repository file 推导。 |
| `ORCHBEE_CLI_NATIVE_ADDON_PATH` | 由可信 launcher 解析并导出的内部 process value；Terminal caller 不选择它。                                          |

在 POSIX 上，`ORCHBEE_CLI` 指向 `resources/orchbee-cli`；在 Windows 上，它指向
`resources/orchbee-cli.cmd`。在源码 checkout 中，launcher 会解析同级
`dist/main/orchbee-cli.cjs` entry 与 Desktop native addon，并使用仓库要求的 Node runtime。在打包
application 中，launcher 会解析 colocated entry 与 addon，并通过
`ELECTRON_RUN_AS_NODE=1` 使用已打包的 Electron executable。Desktop packaging 会把这三项 artifact
放在 `app.asar` 外，验证它们存在，并验证 POSIX launcher 的 executable mode。Launcher 不会启动
coordinator，也不会回退到 TCP 或 project-local discovery。

例如，在 Desktop 创建 POSIX Terminal 后：

```sh
"$ORCHBEE_CLI" status
"$ORCHBEE_CLI" --skill
```

PowerShell 通过 `& $env:ORCHBEE_CLI status` 使用同一个注入路径。这些是
distribution-internal、version-locked launch artifact，不是全局 executable、package-manager 安装或稳定的
第三方 embedding API。

### Process 模型

| 字段         | 契约                                                                                                                                                                                                              |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | 已在 Desktop-owned Terminal 中运行的人、Agent 或 Skill 需要一条非视觉 invocation path，连接同一个 live local coordinator，且不进行全局安装，也不增加另一名 state owner。                                          |
| Inputs       | Argument、高层 owner-backed input、file path、file/stdin byte 与 signal 均不受信。由代码位置确定的 launcher、内嵌 Skill、native addon location 与 Desktop 选择的 runtime directory 属于可信 product composition。 |
| Data         | Process 不拥有 durable state。它只持有一个有界 input buffer、invoke 时的一个 caller-generated request identity，以及一个瞬态 client connection。                                                                  |
| Outputs      | 一个带版本的 stdout document 或精确内嵌 Skill，以及一个 exit code。当前 stderr 为空；operation input 与 result payload 绝不进入 diagnostic。                                                                      |
| Invariants   | 先 parse 后连接；input 在 JSON decode 前受容量限制；一次 invocation 最多提交一次；一个 process 只写一次 result；launch path 不改变 operation authority。                                                          |
| Decisions    | Command grammar 选择 runner branch；`--target` 选择一个精确且经过验证的 endpoint；`--input-file -` 选择有界 stdin 而非有界 file；output option 只选择 serialization。                                             |
| Model        | `runSemanticCommandCliProcess` 用 filesystem、randomness、可信 locator 与 native-transport capability 组合 deterministic runner。`startSemanticCommandCliProcess` 拥有 signal registration 与最终 write。         |
| Cancellation | `SIGINT` 与 `SIGTERM` 只 abort CLI-owned execution observation。它们绝不会发出 semantic cancellation；business cancellation 仍然需要 `execution cancel`。                                                         |
| Verification | Unit test 控制 input、configuration、signal 与 write。Desktop native integration 使用 Node、Electron Node mode 和精确 product launcher 启动 built entry，并连接一个真实 private endpoint。                        |

## Client 边界

`SemanticCommandClient` 依赖一个注入的 `SemanticCommandClientPort`。它使用
`@orchbee/semantic-command-protocol` 验证每个 unknown response，拒绝不匹配的 coordinator
generation，并针对无效 input、格式错误的 response、transport failure 或本地取消的 observation，
返回不带 payload 的 client error。

Core 支持 status、operation list 与 description、owner-backed Workbench 与 Agent command projection、
invocation admission、execution lookup、cooperative cancellation，以及带边界 backoff 的等待。
`waitForExecution` 只会重复执行只读 execution lookup。与之不同，`agent wait` 会在 canonical Agent
snapshot source 上调用一个 server-owned asynchronous query，再通过同一 ledger 观察该 semantic
execution。Abort CLI caller signal 只停止本地 execution observation；它绝不隐式调用 invocation 或
cancellation。

`agent wait` 固定一个精确 Session ID。默认条件包含 attention、outcome、idle 与 ended；重复的
`--until` 可选择显式 semantic status，或 `fresh`、`stale` 与 `disconnected` 条件。Semantic status 只匹配
fresh observation。`--after-revision` 要求严格更新的全局 Agent runtime revision，`--timeout` 范围为
1 到 3,600,000 毫秒，默认 600,000 毫秒。使用 `--detach` 保留 request 或 execution identity，以便之后
观察或显式执行 `execution cancel`。Coordinator generation 变化会让进行中的 wait 结果未知；CLI 绝不重放。

Runner 最多调用一次 operation。Response 丢失后，它只会重连并观察同一个 request identity。不同的
coordinator generation 会返回 `outcome-unknown-after-restart`，且绝不重放 mutation。位置参数形式的
execution reference 会通过只读 request 与 execution lookup 解析。如果两个 identity 都存在但指向不同
execution，runner 会返回 `execution-reference-ambiguous`，且不会取消其中任何 execution。Receipt 缺失会
显式返回，绝不视为成功完成。

Core 不依赖 HTTP、socket、named-pipe、SSH、Desktop、Browser、文件系统或 process。
`LocalHttpSemanticCommandClientPort` 是独立 Node adapter，它把该 port 映射到 bundled 本地 HTTP
契约，并在 core 观察 response 前验证每个 response。

## 可信本地连接

`connectLocalSemanticCommandCoordinator` 接受一个可信产品 runtime directory，以及 packaged native
client addon 或注入的等价实现。没有 selector 时，它只枚举该目录中严格的 `*.c.json` locator 名称。没有
live 且 generation 匹配的 coordinator 时返回 `coordinator-unavailable`；超过一个时返回
`coordinator-ambiguous`。显式 selector 是一个精确且经过验证的 endpoint identity，绝不表示最新 process、
current directory 或由 Project 提供的 locator。

Native client 会在读取 locator 前验证其 ownership 与 privacy，只连接对应的 Unix-domain socket 或
Windows named pipe，并把 server 认证为当前 effective OS user。它不会打开 TCP connection。Discovery
通过 `/v1/status` 探测，并丢弃 coordinator generation 不匹配的 locator。之后每个 operation 使用一条
有界 request 和一条有界 response connection；transport loss 会报告给 core，且绝不会引起 invocation
replay、cancellation 或隐式启动 Desktop。SSH、Browser access、remote target 与 command presentation
仍然不存在。

## 所有权边界

CLI 绝不读取 Workbench data file、provider 或 screen state、连接 Terminal Host、枚举产品 Action，也不
存储或重建 feature state。高层 command 只构造由 Workbench 或 Agent 所有者导出的 semantic-command schema
验证的 input，并在调用前验证完整的 expected public operation descriptor。Agent query 只返回 canonical
bounded snapshot 与 projection；`agent explain` 描述 provenance、freshness、capability 与 status basis，
但不公开 prompt、transcript、tool content、path、provider error 或 screen evidence。Coordinator 拥有
catalog admission 与 execution；feature package 拥有 operation input、output、targeting 与 effect；
protocol package 只拥有可序列化的通用 envelope。添加 CLI projection 不会授予 operation authority。

## 验证

在仓库根目录运行：

```sh
pnpm --filter @orchbee/cli build
pnpm --filter @orchbee/cli typecheck
pnpm --filter @orchbee/cli test
pnpm --filter @orchbee/desktop test:native
pnpm desktop:package
```
