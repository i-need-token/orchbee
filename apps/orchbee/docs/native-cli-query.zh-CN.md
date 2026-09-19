# 原生 CLI 查询

状态：原生 `orchbee` 命令行面已接受的实现与正确性契约，覆盖只读查询批与首个有界
Workbench mutation 批。查询命令是对已在运行的本地 Site 的只读观察；mutation 命令各自在
[Mutations](#mutations) 安全契约下至多提交一次 effectful `workbench.command`。本面绝不创建、
修复、锁定或激活 Site；查询不创建终端、viewer 或控制授权——唯一例外是 `terminal read`，
它对已绑定终端持有一个短暂的只读 viewer attachment 并在输出前释放。任何命令都不猜测
Agent、Worker、进程或 PTY 状态，也不要求 TTY。这是未发布的本地开发面；`apps/cli` 仍是
已交付的 TypeScript CLI 权威；安装、发布、远端连接与 Site 生命周期操作不在范围内。

归属：本记录拥有 `src/invocation.rs` 与 `src/cli/**` 实现的公共语法、结果 envelope、失败矩阵、
预算、单请求只读状态机、单命令 mutation 状态机与输出副作用。它落实已接受的 CLI-Q 设计与两轮
挑战裁决；文字与代码不一致时，行为是要修的缺陷，不是要保留的分叉。

## 输入与信任边界

- `argv`：有界敌意输入。全部参数的 UTF-8 总字节不超过 64 KiB（65,536）。任一非 UTF-8 参数或
  超限即 `invalid-usage`、stdout 为空——即使另一个参数写着 `--format=json`。不隐式读取 stdin
  或文件输入。
- 环境：`ORCHBEE_RUNTIME_BASE`（可选，默认 `/tmp`）选择运行基座；`LC_ALL`/`LC_MESSAGES`/`LANG`
  选择诊断 locale。不读取其他任何环境变量。
- 文件系统：解析出的运行基座、Site scope 目录 `orchbee-native-<euid>` 及其
  `client.sock`/`worker.sock`、activation 标记、当前可执行文件同目录的 `cohort.json` 与四个
  artifact 只被观察、绝不写入。所有目录遍历使用既有 no-follow、属主/模式校验的 rendezvous 归属
  方（`ModeCheckedLocalSiteRendezvous::validate_mode_checked`，仅 `ExistingOnly` 策略）。
- Socket 对端：`client.sock` 可能属于真实 Server、残留 socket 或脚本 peer。CLI 信任类型化
  transport 与协议层（OBLC 准入、OBWP 相关性与契约校验），绝不从 `Display` 文本、`ErrorKind`
  猜测或字节计数重新推导结论。
- 快照数据：用户可控的 label 与目录路径是任意 Unicode，终端输出按敌意文本处理（见转义规则）；
  JSON 输出无损保留。

## 公共语法

选项扫描在 `--` 之前自左向右进行；`--` 仅终止选项识别。

- `--format text|json` 或 `--format=text|json`：至多一次选择，`--` 前任意位置。
  `--format` 缺值、空值、未知值或重复选择都是格式失败：`invalid-usage`、退出 2、stdout 为空。
  完整的 `--` 前 format envelope 先于普通语法校验整体扫描：envelope 内任何位置的格式缺陷都以
  空 stdout 作答；唯一合法的选择无论位于何处都决定其后一切语法失败的 envelope。
- `--project <target>`、`--workspace <target>`、`--tab <target>`：带值的具名过滤器与上下文
  操作数，各只能用一次。下一个 token 即作为值被消费，即使以减号开头；裸 `--` 永不被当作值
  （缺值 → `invalid-usage`）。被消费的过滤值不再充当 format 选择器。每个 flag 只在声明它
  的命令上合法（`--project` 仅 `workspace list`/`workspace create`，`--workspace` 仅
  `tab list`/`tab create`，`--tab` 仅 `panel list`），出现在任何其他位置（包括 `get`
  命令）都是 `invalid-usage`。过滤器绝不静默丢弃：出现在未声明的命令上即在任何运行时访问
  之前报 `invalid-usage`，故 `workspace list` 拒绝 `--workspace`、`--tab` 与
  `--standalone`。
- `--standalone`：无值布尔 flag，单次使用，仅 `tab list` 与 `tab create`；两处均与
  `--workspace` 互斥。
- `--label <label>`：带值 flag，单次使用，`workspace create`、`workspace rename`、
  `tab create`、`tab rename` 与 `panel split` 上必填。值为经协议校验的 Workbench label
  （非空且在协议 label 上限内）；缺失、空串或超长均为 `invalid-usage`。
- `--direction <word>`：带值 flag，单次使用，`tab move`
  （`previous|next|first|last`）、`panel split` 与 `panel move`
  （`above|below|left|right`）上必填。词集各自封闭；`tab move` 上出现 panel 方向词或
  缺失/未知词均为 `invalid-usage`。
- `--cols <n>`、`--rows <n>`、`--cell-width <n>`、`--cell-height <n>`：显式终端几何组；
  四者在 `tab create` 与 `panel split` 上必须同时给出，各取一个十进制 `u16`。组合后的
  格数与像素范围必须满足协议 `WorkbenchTerminalGeometry` 界限；缺项、重复、非数值或越界
  均在任何运行时访问之前报 `invalid-usage`。
- `--confirm`：无值布尔 flag，`tab close` 与 `panel close` 上必填；该处缺失或出现在任何
  其他命令上均为 `invalid-usage`。
- `--target-panel <target>`：带值 flag，单次使用，`panel move` 上必填；按 `panel` 的
  target 规则解析（仅完整 typed ID），并在被移动 panel 自身所在 tab 内解析。
- `--version`：仅在独占根调用时合法；其他位置或与其他参数并存都是 usage。
- `-h`/`--help`：根级独占 token，或已识别帮助路径（family 或完整命令路径）的单一后缀。
- `--` 之后的位置参数：`--` 之后的 token 只作位置参数，故 `orchbee -- status` 查询 status，
  `orchbee project get -- --help` 解析的是字面 label `--help`。

命令路径（固定词，无缩写、无别名）：

```
orchbee                         # 裸调用 → TUI（不变）
orchbee tui                     # TUI（不变）
orchbee --version               # 版本行（不变）
orchbee -h|--help | orchbee help   # 根帮助
orchbee status
orchbee project list | project get <target>
orchbee workspace list [--project <target>] | workspace get <target>
orchbee workspace create --project <target> --label <label>
orchbee workspace rename <target> --label <label>
orchbee tab list [--workspace <target> | --standalone] | tab get <target>
orchbee tab create (--workspace <target> | --standalone) --label <label> <geometry>
orchbee tab rename <target> --label <label>
orchbee tab move <target> --direction previous|next|first|last
orchbee tab close <target> --confirm
orchbee panel list [--tab <target>] | panel get <target>
orchbee panel split <target> --direction above|below|left|right --label <label> <geometry>
orchbee panel close <target> --confirm
orchbee panel move <target> --target-panel <target> --direction above|below|left|right
orchbee terminal list | terminal get <target> | terminal read <target>
orchbee api snapshot
orchbee api schema [<schema-id>]
orchbee config check
orchbee completion bash|zsh|fish
orchbee help <recognized path>
orchbee <recognized path> -h|--help
```

帮助主题：根级；`project`、`workspace`、`tab`、`panel`、`terminal`、`api`、`config`、
`completion` 八个族；以及包括 `tui` 在内的每条完整命令路径。`help` 只接受已识别的路径（不带
过滤器、不带 format）。路径末尾的 `-h` 或 `--help` 是同一路径上的同一请求：`orchbee --help`、
`project --help`、`project get --help` 输出的内容与 `help`、`help project`、`help project get`
完全相同。后缀 help 优先于缺失的业务参数（`project get --help` 描述 `project get`），但不优先于
已给出的业务参数或多余 token（`project get x --help`、`status extra --help` 都是 usage）。未列出的
目标如 `help help`、`help version`、`help --version` 按未知路径 → usage。Help、`--version`、
`completion` 与 `api schema` 绝不触碰平台、运行、cohort 或 Site 状态。

`<geometry>` 代表四个必填 flag：`--cols <n> --rows <n> --cell-width <n>
--cell-height <n>`。

`help`、`completion`、`tui`、`--version` 与裸调用是固定文本输出，不接受 `--format`；一个合法
`--format=json` 会把随后产生的语法失败变成 JSON `invalid-usage` 失败文档，而 text/默认失败仍
保持 stdout 为空。数据命令（`status`、全部 `list`/`get`、`api snapshot`、`api schema`、
`config check`）与全部 mutation 命令接受 `--format`。`api` 命令是机器面：`api snapshot` 或 `api schema` 成功时在
两种 format 下输出同一份 JSON 文档；`api` 失败则与其他命令一样跟随所选 format——text/默认下
只写 stderr、stdout 为空，只有 `--format json` 才产出失败 envelope。

`orchbee completion` 打印本语法的静态投影：动词、完整帮助路径、每条命令已声明的 flag、
`--format` 的两种拼写及其 `text`/`json` 值、`--` 终止与 flag 值消费上下文。不枚举任何 Site
状态、资源名或运行时数据。

### 目标与过滤器

`project`/`workspace`/`tab` 的 `<target>` 先按完整 typed ID 解析（恰好 32 位小写十六进制、
非零）。合法 ID 只按身份匹配。其他 token 视为精确、区分大小写的 label 候选，要求 1–512 个
Unicode scalar；空或过长的 token 与全零 ID 形值都是 `invalid-identifier`（退出 2）。本身恰为
合法 ID 形的 label 永远无法按 label 命中——ID 天然优先；该实体仍可通过其真实 ID 访问。
`panel` 与 `terminal` 只接受完整 typed ID，其余一律 `invalid-identifier`。过滤器
（`--project`、`--workspace`、`--tab`）使用各自资源相同的 target 规则；所有 mutation 位置
操作数与 `--target-panel` 同样遵守（`panel` 规则：仅 typed ID，并在被移动 panel 自身所在
tab 内解析）。`api schema` 接受非空
schema ID；空串是 `invalid-identifier`（`resource="schema"`），无匹配是 `not-found`。

读取成功后的解析：零匹配 → `not-found`（3）；多个精确 label 匹配 → `ambiguous`（3），按
canonical 遍历次序报告至多前 8 个候选 typed ID 与真实 `totalCount`。命名不存在实体的过滤器是
`not-found`；实体存在但结果为空是成功的空表。

## 查询状态机与预算

每个网络查询（`status`、`list`/`get`、`api snapshot`）在 Linux 上严格按下列顺序执行；第一个
失败阶段即决定失败：

1. **Cohort** —— `NativeCohort::current` 在一个 checked 5 秒单调 deadline 下运行
   （`Instant::now().checked_add(5s)`；`checked_add` 失败为 `internal-failure`、退出 1）。
   任何 cohort 错误（缺失、非法、不安全、散列不符、协作式 deadline）都是 `check-failed`、
   stage `cohort`。
2. **Discovery** —— 解析 `ORCHBEE_RUNTIME_BASE`/默认值，然后
   `ModeCheckedLocalSiteRendezvous::validate_mode_checked`（`ExistingOnly`，不 mkdir、不取锁）。
   分类使用 `RendezvousError` 的 typed path 与已解析 `runtime_base`/`runtime_base.join(scope)`
   比较：scope 目录上 source 为 `NotFound` 的 `System` 错误是 `service-unavailable`、stage
   `discovery`；其余一切 rendezvous 或文件系统校验失败都是 `check-failed`——typed path 属于
   基座或基座组件时 stage `runtime-base`，属于 scope 目录时 stage `runtime-directory`。
   前置观测不宣称与后续 open 原子。
3. **Socket** —— `client_socket_exists()` 复用既有 类型+属主+`0600`+no-follow 校验：不存在 →
   `service-unavailable`、stage `discovery`；无效/出错 → `check-failed`、stage `socket`。
4. **Admission** —— 紧邻 `LocalClient::connect_before` 之前建立一个新的 checked 5 秒单调
   deadline；同一 `Instant` 原样沿用到快照读取，绝不续期，绝不回退到 50 秒请求默认值。
5. **Read** —— 恰好一次 `submit_workbench_snapshot_read_before`，使用全新
   `WorkbenchRequestId`，受同一 deadline 约束。一次 OBLC 准入加一次 OBWP 请求；无重连、重试、
   publication 追赶或第二次读取。
6. **Release** —— 在任何映射、格式化、输出之前 RAII 释放全部自有 `LocalClient`/socket 句柄；
   慢 stdout 因此绝不持有 Session。不存在也不宣称 shutdown 交换；Server 可异步观察到 EOF。
7. **Map** —— 把已验证快照纯函数式地投影为命令结果。

`config check` 纯本地：不 connect socket、不取锁、不 mkdir/chmod；其 cohort 检查使用单独的
checked 5 秒预算。非 Linux 平台对网络查询与 mutation 产出 `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}`，对 `config check` 产出同 details 的
`check-failed`；不存在向无界连接 API 的回退。

`terminal read` 沿用第 1–5 步原样不变，然后在同一 deadline 内让已准入的 session 保持存活
——绝无第二次准入——完成一次只读 viewer 观察：

1. **解析** —— 在已验证快照的 presentation 绑定中查找 typed terminal ID；不存在 →
   `not-found`（3）带 `{resource:"terminal",target}`，session 随之释放。绝不咨询 label。
2. **Attach** —— 以快照自身的事实发起一次 `terminal.viewer`
   `AttachBinding{siteId,workerId,terminalId}`。`Attached{attachmentId,hostEpoch}` 用于构造
   限定该 attachment 与 epoch 的 `TerminalProjection`；其他 outcome 类别按失败矩阵映射。
3. **Poll** —— `Poll{attachmentId}` 在同一 deadline 内重复：`Empty` outcome 等待 10 ms 后
   重试；`record` outcome 送入投影。record 未通过 transport 或帧校验是 `protocol-invalid`、
   stage `viewer`；投影以 `NeedSnapshot` 回应的 record 会发送一次该反馈（得到
   `FeedbackAccepted` 应答）并继续轮询；已消费的 `ProjectionBlocked` 被忽略。投影接受的第一条
   record 即结束观察——被应用的帧就是答案；一次性读取绝不发送 `FrameApplied` 反馈。
4. **释放** —— 尽最大努力 `Detach{attachmentId}` 先于无条件的 session drop；两者都完成于
   任何映射、格式化或输出之前。viewer attachment 绝不保留到输出阶段，session drop 本身就是
   拆除保证。

## Canonical 数据与结果

Canonical 遍历次序：`navigation.projects` 顺序；各 project 的 `workspaces` 顺序；
`terminalContexts` 顺序接 `tabs` 顺序；panels 深度优先且 `children[0]` 先于 `children[1]`；
terminal 绑定按该 panel 顺序且 `primaryPresentation` 先于 `childPresentations`。不新增排序；
过滤后的列表与 `ambiguous` 候选都保持该次序。

成功 envelope 为 `{schema, version:1, source, result}`：

- `source.kind="workbench-snapshot"`：带同一份已验证快照的 `siteId` 与十进制字符串
  `revision`——仅在完整有效读取之后出现。
- `source.kind="workbench-command"`：带 `siteId`、`requestId` 与十进制字符串
  `expectedRevision`——已产出的 mutation 结果，以及有效读取之后 command 阶段的失败。
- `source.kind="embedded"`：随二进制编译内嵌的 schema 字典（`api schema` 结果及其
  `not-found` 失败）。
- `source.kind="local-runtime"`：已产出的 `config check` 结果。
- 不存在 freshness 字段；一次一致读取不等于持续活性保证。

结果载荷（schema ID 为稳定机器名）：

- `orchbee.status` → `{site:{reachable:true,siteId,revision},
counts:{projects,workspaces,tabs,panels,boundTerminals},
worker:{status:"unknown",reason:"not-in-workbench-snapshot"}}`。`boundTerminals` 统计带
  `binding` 的 presentation；快照校验已保证 terminalId 全局唯一，故每个绑定恰命名一个不同
  终端。`reachable` 只证明本次读取成功。
- `orchbee.project.list` → `{projects:[{projectId,label,directoryCount,workspaceCount}]}`；
  `orchbee.project.get` → `{project:{projectId,label,directories:[{directoryId,workerId,path}],
workspaces:[{workspaceId,label}]}}`。
- `orchbee.workspace.list` → `{workspaces:[{workspaceId,label,projectId,
primaryDirectoryId|null,terminalContextId}]}`，跨 project 展平；`workspace get` 增加经
  `terminalContextId` 反查的 `tabs:[{tabId,label}]`。
- `orchbee.tab.list` → `{tabs:[{tabId,label,pinned,groupId|null,
context:{kind:"standalone",terminalContextId}|{kind:"workspace",terminalContextId,
workspaceId}}]}`；standalone 上 `workspaceId` 键不存在而非 null。`tab get` 增加 `root`——
  typed `PanelNode` 子树的语义化重编码，不丢字段。
- `orchbee.panel.list` → `{panels:[{panelId,tabId,terminalContextId,primaryPresentationId,
presentationCount,primaryTerminalId|null}]}`，`primaryTerminalId` 仅反映
  `primaryPresentation.binding`；`panel get` → `{panel:{panelId,tabId,context,slotPath,
primaryPresentation,childPresentations}}`，`slotPath` 为自 root 至该叶子的 `PanelSlot` ID
  链（root 本身即 panel 时为 `[]`），每个 presentation 为 `{presentationId,label,
binding:{terminalId,workerId}|null}`。
- `orchbee.terminal.list` → `{terminals:[{terminalId,workerId,boundTo:{presentationId,panelId,
tabId,terminalContextId}}]}`；`terminal get` 在 `terminal` 下返回同一身份事实。只有被
  presentation 绑定的终端可见；绑定存在不证明进程存活。任何位置都没有 lifecycle/process/agent
  字段。
- `orchbee.terminal.read` → `{terminal,frame,content}`：`terminal` 携带同样的
  `{terminalId,workerId,boundTo}` 身份事实；`frame` 携带已应用投影的
  `{revision,generation,sequence}`（十进制字符串，u64 无损）、`alternateScreen`、`geometry`
  `{columns,rows}` 与 `cursor` `{row,column,visible}`；`content` 携带 `rows`（发出的网格行，
  形如 `{index,text,continuation:{fromPrevious,intoNext}}`——`text` 拼接该行各 cell，
  丢弃宽字符占位尾并裁剪网格填充）、`rowCount`（已应用帧持有的全部行数）、
  `truncated`/`omittedRows` 与 `limits` `{rows:4096,textBytes:4194304}`。发出的行受累计 4 MiB
  JSON 编码文本预算约束；超出预算的行计入 `omittedRows` 且 `truncated` 为 true。text 输出下每
  个发出的行渲染为一行转义文本，`omittedRows>0` 时末尾追加本地化截断标记。本次读取是单一
  时间点帧：`revision`/`generation`/`sequence` 是宿主在该观察时刻的计数器，不是 CLI 的
  freshness 声明。
- `orchbee.api.snapshot` → `{snapshot}`：完整 `WorkbenchProtocolDocument::Snapshot` 的语义化
  重编码，含 `kind:"workbench.snapshot"`；envelope 的 `siteId`/`revision` 与内部文档一致。
- `orchbee.api.schema` → 带参 `{schema:{schemaId,document}}`；无参
  `{schemas:{schemaId:document,…}}` 列出全部已交付字典（draft 2020-12）。
- `orchbee.mutation.result` → `{action,outcome,revision?,acceptedRevision?,closeRequestId?,
<resolved typed IDs>,direction?,label?,terminal?}`，按上文 mutation 矩阵：`committed`/
  `unchanged` 带 `revision`，`accepted` 带 `acceptedRevision`+`closeRequestId`，
  `tab create`/`panel split` 带 `terminal:{outcome,reason?,geometry?}`。
- `orchbee.config.check` → `{ok,checks:[{name,status,details}]}`，固定次序
  `runtime-base`、`runtime-directory`、`cohort`、`locale`、`sockets`；五项永远齐全。存在失败
  项时仍产出该成功 schema（`ok:false`）并以 1 退出。
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`；不存在英文 `message` 字段。`source` 只取自已可信来源（读取后失败用已验证快照；
  `api schema` not-found 用 `embedded`；mutation command 阶段失败一律用
  `workbench-command`）；读取之前的失败没有 source。

### Config 检查

| 检查项              | 规则                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime-base`      | 解析并观测 `ORCHBEE_RUNTIME_BASE`/默认值：存在且为目录则 pass；路径非法、缺失、不安全项或 I/O 错误则 fail。                                                                                                                                                                                                                                                                                                            |
| `runtime-directory` | 基座失败、或合法基座下 scope 目录不存在时为 `not-applicable`（Site 未运行不是配置错误）；既有目录通过校验为 pass；属主/模式/类型不安全或 I/O 错误为 fail。                                                                                                                                                                                                                                                             |
| `cohort`            | `NativeCohort::current` 在其 5 秒预算内验证通过则 pass（`verified:true`）；缺失、非法、散列不符（`invalid-cohort`）或超时（`deadline`）为 fail。                                                                                                                                                                                                                                                                       |
| `locale`            | 信息性 pass，携带 `locale`（`en-US`/`zh-CN`）与 `source`（`LC_ALL`/`LC_MESSAGES`/`LANG`/`default`）。                                                                                                                                                                                                                                                                                                                  |
| `sockets`           | `client`/`worker` 在运行目录可安全检查后报告 `presence` 为 `present`/`absent`/`unchecked`，沿用既有 类型+属主+`0600`+no-follow 规则。`clientPending`/`host` 是仅派生路径预算条目：`presence` 恒为 `unchecked`，地址可容纳时为 `not-applicable`/`presence-not-inspected`，超预算时为 `fail`/`address-too-long`；两者始终携带 `path`/`pathBytesHex` 与 `addressBytes`/`addressCapacityBytes`。任一条目 `fail` 则整体 fail；否则至少一个合规存在的 `client`/`worker` 为 pass；其余为 `not-applicable`。存在绝不宣称服务或 Worker 活性，预算可容纳也绝不宣称端点存在——此处的 `not-applicable` 指刻意未检查 presence，而非预算未知。派生 socket 路径的字节长度超出本平台 `sockaddr_un` 容量时，无论 presence 如何都以 `address-too-long` fail——该地址永远无法 bind 或 connect。 |

details 一律类型化：`runtime-base`/`runtime-directory` 携带 `path`（UTF-8）或 `pathBytesHex`
（非 UTF-8，绝不用替换字符）；`exists` 仅在路径状态已确立时出现——已确立的缺失为 `false`、
观测到条目为 `true`，检查本身失败（权限、路径或 I/O 错误）时该字段整体省略——另可加 `reason`
∈ `{invalid-path,absent,unsafe,io-error}`；`sockets` 的 reasons 为 `{unsafe,io-error,
runtime-unavailable,address-too-long,presence-not-inspected}`。`clientPending`/`host` 预算条目与
每个 `address-too-long` 条目都携带 `path`/`pathBytesHex`
以及 `addressBytes`（派生路径的编码字节长度）和 `addressCapacityBytes`（本平台 socket 地址
容量）。不作跨文件原子健康快照声明。

## Mutations

九条命令经已 admitted 的 OBWP `workbench.command` 通道修改 Workbench 状态：
`workspace create`/`rename`、`tab create`/`rename`/`move`/`close`、
`panel split`/`close`/`move`。每次调用原样执行查询状态机的第 1–5 阶段——一次 cohort 检查、
一次 discovery 与 admission、一次有界快照读取——然后在同一 5 秒 deadline 下再执行两个阶段：

6. **Resolve** —— 把已验证快照纯函数式地投影为 wire action：每个 target 以查询 resolver
   完全相同的语义解析（可能 `not-found`/`ambiguous`，此时提交零条命令），快照提供 `siteId`
   与 `expectedRevision`，并生成一个全新的 `WorkbenchRequestId`。
7. **Command** —— 在已准入 session 上恰好提交一次 `workbench.command`。随后在任何结果
   映射与输出准备之前完全释放 Client Session。

`expectedRevision` 的选择是协议的乐观并发契约：Server 将命令的 base revision 与当前
revision 比对，不符即拒绝且不应用任何效果，故"读取一次并按该 revision 提交"就是被 admitted
的交互序列——CLI 绝不为追赶变动的 base 而重复读取，也绝不在无 base 的情况下提交。

安全契约：

- **至多一次 effect。** 每次调用恰好提交一条命令；全部参数与 target 校验在命令构造之前
  完成，故解析缺陷或未解析 target 不提交任何命令。
- **全新身份。** 每次调用生成一个全新的全局唯一 `WorkbenchRequestId`；receipt 与
  `outcome-unknown` 失败都会报告它以便操作员对账。
- **Server 持有 revision。** `expectedRevision` 永远来自解析出全部 target 的同一份已验证
  快照——绝不猜测、绝不由客户端自增、绝不复用。
- **绝不自动重放。** 提交后结果不可判定的情形——写丢失、响应丢失/畸形/correlation 不符、
  或答案未通过校验——会 retire 该 session 并以 `outcome-unknown`（退出 8）呈现。命令可能
  已应用也可能未应用；绝不重发。本地化诊断会明确说明这一点，并指示操作员先检查
  Workbench 状态（`status`、`tab list`、`panel list` 等）再决定是否重复该命令。
- **关闭确认。** `tab close` 与 `panel close` 是破坏性操作；解析器要求显式 `--confirm`
  flag，故非交互调用绝不会默认执行关闭。

结果：`committed` 报告新 `revision`（base + 1）；`unchanged`——未造成变化的 rename 或
move——报告当前 revision；`accepted`——两条 close——报告 `acceptedRevision` 及 Server 为
异步关闭给出的 `closeRequestId`。`tab create` 与 `panel split` 另携带 `terminal`：Server
仍欠 spawn 时为 `pending`，否则为类型化 spawn 结果（`started` 带回显的 `geometry`，
`unavailable`/`failed` 带 reason，`unknown`）。

几何永远显式：非 TTY 调用无法继承终端尺寸，故 `tab create` 与 `panel split` 要求四个 flag
全部给出。`--cols`/`--rows` 为格数，`--cell-width`/`--cell-height` 为每格像素尺寸；每个
维度都是 `u16`，并在任何运行时访问之前经协议 `WorkbenchTerminalGeometry` 校验。

Mutation 结果使用 `schema:"orchbee.mutation.result"`、`version:1`，
`source:{kind:"workbench-command",siteId,requestId,expectedRevision}`，`result` 携带 `action`
（点分命令名）、`outcome`、该结果的 revision 字段、已解析 typed ID，以及命令具备时的
`direction`/`label`/`terminal`。文本输出按与查询相同的转义规则逐行渲染同样字段。

明确不交付：`workspace close`/`workspace focus`、`tab focus`、`panel resize`/`panel focus`、
终端 open/send 操作、Site 生命周期命令，以及一切远端操作。

## 失败矩阵

退出码：0 成功（含空表）；1 本地配置/输出/内部确定失败；2 usage/非法 target；3
not-found/ambiguous/denied/rejected；4 service-unavailable（含 `session-capacity`）；5
transport-loss/protocol-invalid；6 incompatible；8 outcome-unknown；130 默认 SIGINT 语义。
7 保留且在本切片内不可达。

| 来源                                                                                                                         | `failure.kind`                       | 退出 | `details`                                                                                 |
| ---------------------------------------------------------------------------------------------------------------------------- | ------------------------------------ | ---- | ----------------------------------------------------------------------------------------- |
| argv 封装/语法/format/target 形状错误                                                                                        | `invalid-usage`                      | 2    | 省略                                                                                      |
| 全零 ID/空/超长 label、非法 panel/terminal ID                                                                                | `invalid-identifier`                 | 2    | `{resource,target}`                                                                       |
| 合法 target 或 schema ID 无匹配                                                                                              | `not-found`                          | 3    | `{resource,target}`                                                                       |
| 多个精确 label 匹配                                                                                                          | `ambiguous`                          | 3    | `{resource,target,candidates≤8,totalCount}`                                               |
| `NativeCohort::current` 出错                                                                                                 | `check-failed`                       | 1    | `{stage:"cohort"}`                                                                        |
| 基座非法/缺失/非目录、不安全 runtime/socket、其他校验                                                                        | `check-failed`                       | 1    | `{stage:"runtime-base"                             \| "runtime-directory"   \| "socket"}` |
| 合法基座下 scope 目录或 `client.sock` 不存在                                                                                 | `service-unavailable`                | 4    | `{stage:"discovery"}`                                                                     |
| `ConnectClientError::ConnectFailed`                                                                                          | `service-unavailable`                | 4    | `{stage:"admission"}`                                                                     |
| `ConnectClientError::Io`                                                                                                     | `transport-loss`                     | 5    | `{stage:"admission"}`                                                                     |
| OBLC `Rejected(CapacityExceeded)`                                                                                            | `session-capacity`                   | 4    | `{stage:"admission"}`                                                                     |
| OBLC `Rejected(HandshakeTimeout)`                                                                                            | `service-unavailable`                | 4    | `{stage:"admission"}`                                                                     |
| `Rejected(IncompatibleProtocol                                    \| IncompatibleCohort)`、`Protocol(IncompatibleVersion)`   | `incompatible`                       | 6    | `{stage:"admission"}`                                                                     |
| `Rejected(WrongMessage                                            \| MalformedFrame)`、`UnexpectedResponse`、其余 `Protocol` | `protocol-invalid`                   | 5    | `{stage:"admission"}`                                                                     |
| `RequestIoConfiguration`、`SessionUnavailable`、`RequestEncoding`                                                            | `internal-failure`                   | 1    | 省略                                                                                      |
| `SnapshotReadError::RequestNotSubmitted`（任何原因）                                                                         | `transport-loss`                     | 5    | `{stage:"snapshot"}`                                                                      |
| `Failed(Write                                                     \| ResponseDeadline)`、`Failed(Response(Io))`              | `transport-loss`                     | 5    | `{stage:"snapshot"}`                                                                      |
| `Failed(Response(InvalidHeader                                    \| Contract))`、`UnexpectedResponse`                       | `protocol-invalid`                   | 5    | `{stage:"snapshot"}`                                                                      |
| `Failed(CorrelationMismatch)`                                                                                                | `protocol-invalid`                   | 5    | `{stage:"snapshot",reason:"correlation-mismatch"}`                                        |
| outcome `Denied`                                                                                                             | `denied`                             | 3    | 省略                                                                                      |
| outcome `Unavailable`                                                                                                        | `service-unavailable`                | 4    | `{stage:"snapshot"}`                                                                      |
| viewer `RequestNotSubmitted`、`OutcomeUnknown(Write\|Read)`                                                                  | `transport-loss`                     | 5    | `{stage:"viewer"}`                                                                        |
| viewer `OutcomeUnknown(InvalidResult\|UnexpectedResult)`、错误 outcome 类别                                                  | `protocol-invalid`                   | 5    | `{stage:"viewer"}`                                                                        |
| viewer `OutcomeUnknown(CorrelationMismatch)`                                                                                 | `protocol-invalid`                   | 5    | `{stage:"viewer",reason:"correlation-mismatch"}`                                          |
| viewer outcome `Rejected(AlreadyAttached\|TerminalUnavailable)` 或 `Unavailable`                                             | `service-unavailable`                | 4    | `{stage:"viewer"}`                                                                        |
| viewer outcome `Rejected(FeedbackMismatch)`、record 被投影拒绝                                                               | `protocol-invalid`                   | 5    | `{stage:"viewer"}`                                                                        |
| `DefinitelyNotSubmitted::RequestNotSubmitted`（command）                                                                     | `transport-loss`                     | 5    | `{stage:"command"}`                                                                       |
| `DefinitelyNotSubmitted::SessionUnavailable`/`RequestEncoding`                                                               | `internal-failure`                   | 1    | 省略                                                                                      |
| command `Rejected` precommit                                                                                                 | `rejected`                           | 3    | `{stage:"command",reason,revision}`                                                       |
| command `Denied` precommit                                                                                                   | `denied`                             | 3    | 省略                                                                                      |
| command `Unavailable` precommit                                                                                              | `service-unavailable`                | 4    | `{stage:"command",reason}`                                                                |
| `OutcomeUnknown`（任意原因）                                                                                                 | `outcome-unknown`                    | 8    | `{stage:"command",requestId,reason}`                                                      |
| 预备输出超 8 MiB                                                                                                             | `output-too-large`                   | 1    | `{limitBytes:8388608}`                                                                    |
| stdout 写/flush 失败                                                                                                         | `output-failed`                      | 1    | 不追加第二份 stdout 文档                                                                  |
| `config check` 完成但存在失败项                                                                                              | 成功 schema `ok:false`               | 1    | 五项全部保留                                                                              |
| 非 Linux 网络查询、mutation 或 `config check`                                                                                | `service-unavailable`/`check-failed` | 4/1  | `{stage:"platform",reason:"unsupported-platform"}`                                        |

`resource` ∈ `project|workspace|tab|panel|terminal|schema`；`stage` ∈
`cohort|runtime-base|runtime-directory|socket|discovery|admission|snapshot|viewer|command|platform`。
stage 与 reason 是有限 schema 值；任何异常文本都不进入机器字段。`details` 是按 kind 封闭的词汇——`{resource,target}`、
`{resource,target,candidates≤8,totalCount}`、`{stage[,reason]}`、
`{stage:"command",reason,revision}`、`{stage:"command",requestId,reason}` 或
`{limitBytes}`——内嵌 schema 拒绝超出这些形状的文档。读路径错误永不归类为
outcome-unknown/failed-effect：只有 command 阶段可能产生不可判定效果；
`rejected`/`denied`/`unavailable` precommit 答案都是确定的——Server 未应用任何效果。

## 输出契约与副作用

- stdout 只承载契约文档：`--format json` 下为一份带 2 空格缩进、以 `\n` 结尾的 JSON（`api`
  命令恒为 JSON），text/默认下为本地化人类文档。stderr 承载本地化人类诊断；查询路径不打印
  进度。
- `invalid-usage` 失败在 stderr 写一条本地化诊断，指明单一具体原因——未知命令或选项、
  该命令不接受的选项或参数、重复选项、冲突选项、缺少选项值、缺少必需选项或操作数、缺少
  命令、未知帮助路径、错位的 help/version 标志、非法选项值或几何参数组合、非 Unicode 或
  超限参数表——并以帮助提示结尾。提示对可识别的命令或族上下文给出 `orchbee <path>
  --help`，否则回落到 `orchbee --help`；每条发出的提示都是无需 Site 即可退出 0 的真实
  路径。回显的不可信 token 复用下述带引号转义渲染，并先截到 256 个 Unicode 标量前缀、
  仍有剩余标量时显式追加省略号；非 Unicode 或超限参数表绝不回显。机器 envelope 不变——
  诊断只存在于 stderr。
- 文本把用户可控字符串渲染为带引号的转义值：反斜杠与引号用 `\\`/`\"`；C0/C1 控制字符（含
  ESC）、U+2028/U+2029 与双向格式控制符用无歧义 `\uXXXX` 转义；其余 Unicode 原样保留。列表
  表格只把 label 显示截到 80 显示列并加省略号，转义序列与收尾引号保持完整；ID 绝不截断；
  `get` 字段值完整呈现。JSON 用标准无损编码保留源数据——绝不用 TUI 的替换字符清洗器。
- 完整输出在首次 stdout 写入前于有界缓冲区内构造；预备超过 8 MiB 即 `output-too-large`，
  stdout 不被触碰。真实 OS 写失败可能留下部分字节，但绝不追加第二份文档；stderr 仍报告且
  退出码非零。
- 机器输出与 locale 无关且不含英文 `message`；stderr 诊断按 en-US/zh-CN 本地化。机器 `kind`、
  字段名与枚举值绝不本地化；用户 label/path 数据按数据原样保留。

## 不变量

- 语法非法的调用绝不触碰文件系统、cohort 或 socket。
- 离线命令（`help`、`--version`、`completion`、`api schema`）绝不执行平台、运行、cohort 或
  Site 发现。
- 网络查询不创建任何事物：无目录、锁、标记、终端、viewer、控制授权或第二个请求——唯一
  例外是 `terminal read`，其只读能力 viewer attachment 是唯一被认可的读路径副作用，最小化
  持有并在输出前释放。至多持有一个短暂 Client Session，且绝不带入输出阶段。
- Mutation 每次调用在全新 `WorkbenchRequestId` 下至多提交一次 `workbench.command`；绝不
  重试或重放，结果不可判定时报告 `outcome-unknown` 而非猜测。
- 每次调用至多一次快照读取、一次准入，且读取与命令共用一个网络 deadline；`terminal read`
  的 viewer 交换复用同一 session 与同一 deadline——绝无第二次准入或续期预算。
- 没有可信来源的字段不存在；`null` 表示来源确定地表达"无"，`"unknown"` 表示来源无法回答——
  两者绝不混用。
- 失败 envelope 绝不虚构 `source`：siteId/revision 只来自已验证快照。
- Ambiguous 绝不猜测：报告有界候选与真实总数。
- `api schema`、help 与 completion 只枚举同一次构建中真实交付的命令。
- 两个 5 秒预算都是 CLI 自有的 checked deadline；其构造失败是 `internal-failure`，而 transport
  内部 `RequestNotSubmitted` 一律保持 `transport-loss`。

## 场景矩阵与证据

| 场景                                                                                                                       | 预期                                              | 证据                                                      |
| -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- | --------------------------------------------------------- |
| 穷举语法：format、位置、`--`、help 形式、重复、非法                                                                        | 精确 parse/失败类别                               | `src/invocation` 测试                                     |
| format envelope 次序：任何位置的缺陷即空 stdout；任何位置的合法 json 决定其后失败的 envelope                               | envelope 优先于语法错误                           | `src/invocation` 测试 + `tests/cli_query.rs`              |
| 过滤器范围：`workspace list` 只接受 `--project`；越界过滤器在任何运行时访问之前即 usage                                    | 完整矩阵，过滤器绝不静默丢弃                      | `src/invocation` 测试 + `tests/cli_query.rs`              |
| 目标规则：ID/label/全零/超长/歧义优先                                                                                      | resolver 判定                                     | `src/cli` resolver 测试                                   |
| 已验证快照 → 各结果形状、空集合、null                                                                                      | 精确 JSON/text fixture                            | `src/cli` 输出测试 + JSON Schema 校验（dev `jsonschema`） |
| Schema 字典 ↔ 实际 envelope                                                                                                | 每个成功/失败 fixture 通过校验；被拒变体不通过    | schema 对比测试                                           |
| API 失败格式：text/默认失败保持 stdout 为空，`--format json` 产出 envelope                                                 | `api schema`/`api snapshot` 失败遵守调用方 format | `tests/cli_query.rs` + `src/cli` 测试                     |
| 无 Site 磁盘状态                                                                                                           | `service-unavailable`，零 fs/进程/锁变更          | `tests/cli_query.rs`                                      |
| 真实 staged-cohort Server：status/list/get/api snapshot、8 个准入加第 9 个被拒加释放、活跃 TUI                             | 成功/`session-capacity`/结构不变                  | `tests/local_startup_process`                             |
| 脚本化 OBLC/OBWP peer：denied、unavailable、畸形、错误 correlation、超时、断连                                             | 按矩阵 typed kind；绝不冒称真实 Server 证据       | `tests/cli_query.rs`                                      |
| 脚本化 OBCS viewer peer：attach-binding 字段、`Empty`→`record` 轮询、detach 释放、reject/unavailable/畸形/correlation/断连 | `terminal read` 成功 + typed viewer-stage 失败    | `tests/cli_query.rs`                                      |
| 真实 staged-cohort Server + 活跃 TUI 终端：`terminal read` 返回其网格并通过 schema 校验                                    | 真实只读观察与 TUI 自身 viewer 共存               | `tests/local_startup_process`                             |
| Mutation 语法：全部命令、target 规则、`--confirm`、几何界限、flag 范围                                                     | 运行时访问前的精确 parse/失败类别                 | `src/invocation` 测试                                     |
| Mutation 脚本化 peer：全部九条命令、precommit 判定、不可判定与未提交结果                                                   | 每次调用一条命令；`outcome-unknown` 绝不重放      | `tests/cli_mutation.rs`                                   |
| 非 TTY；stdout 阻塞时 session 已释放；各 format 预备 >8 MiB；真实写失败                                                    | 文档化退出；阻塞前 transport 句柄已 drop          | `tests/local_startup_process` + emit 单元测试             |
| `config check` 矩阵：每项 pass/fail/not-applicable、`exists` 语义                                                          | 五项 typed 检查齐全                               | `tests/cli_query.rs` + `src/cli/config` 测试              |
| 补全脚本投影语法（bash/zsh 实执行，fish 结构对齐）                                                                         | 正/负上下文；无运行时枚举                         | `tests/cli_query.rs`                                      |
| 双语文本/失败输出                                                                                                          | 精确 en-US/zh-CN 字符串                           | `tests/process.rs` + `tests/cli_query.rs`                 |

模型义务：本切片不新增 Server 状态机、持久化或重试，复用已注册的 OBLC/OBWP/OBCS/Client
模型——`terminal read` 原样复用既有 `terminal.viewer` attach/poll/detach 协议——故不需要新的
TLA+ 模型。触发范围覆盖 `apps/orchbee` 源码、manifest、project scope、README 与测试路径的
已注册模型仍须真实运行；inventory 输出不替代执行。
