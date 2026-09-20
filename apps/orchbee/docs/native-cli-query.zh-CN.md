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
  或文件输入——唯一例外是显式声明的 `terminal send --file`/`--stdin` 源（见
  [Send 输入源](#send-输入源)），在任何运行时副作用之前一次性物化。
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
- `--label <label>`：带值 flag，单次使用，`project create`、`project rename`、`workspace create`、
  `workspace rename`、`tab create`、`tab rename` 与 `panel split` 上必填。值为经协议校验的
  Workbench label
  （非空且在协议 label 上限内）；缺失、空串或超长均为 `invalid-usage`。
- `--path <path>`：带值 flag，单次使用，`workspace accept-directory` 上必填、其他任何命令
  上均非法。值为 `WorkbenchDirectoryPath`：字面绝对 POSIX 路径（以 `/` 开头），至多
  4,096 UTF-8 字节且不含 NUL。CLI 从不展开 `~` 或 `$`，从不创建或修复目录，也从不探测
  文件系统——缺失、相对、含 NUL 或超长的值在任何运行时访问之前即为 `invalid-usage`，
  一切文件系统事实均由 Server 在提交后权威判定。
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
- `--match <text>`：带值 flag，单次使用，`terminal wait-output` 上必填；非空 UTF-8
  字面量，最长 4096 字节，匹配时区分大小写。缺失、空、非 UTF-8 或超长值均为
  `invalid-usage`。
- `--timeout <ms>`：带值 flag，单次使用，`terminal wait-output` 上可选（默认
  `30000`）；取一个 `1..=3600000` 的十进制 `u32`。重复、非数值或越界值均为
  `invalid-usage`。
- `terminal wait-output` 上的 `--rows <n>` 是被搜索的保留行窗口 `1..=1000`（默认
  `1000`），把最近的可见行与历史行作为同一范围覆盖——与同名的几何 flag 无关。
- `--target-panel <target>`：带值 flag，单次使用，`panel move` 上必填；按 `panel` 的
  target 规则解析（仅完整 typed ID），并在被移动 panel 自身所在 tab 内解析。
- `--history`：无值 flag，仅 `terminal read` 受理，且必须与 `--rows <n>` 成对——
  在 `terminal read` 上 `--rows` 拼写携带 `1..=1000` 的保留行数，替代其几何含义，
  该含义在 `tab create` 与 `panel split` 上不变。两者缺一、重复、非数值或越界
  计数、或 `--history` 出现在其他命令上，均在任何运行时访问之前报 `invalid-usage`。
- `--text <utf8>`、`--key <name>`、`--file <path>`、`--stdin`：`terminal send`
  四个互斥输入源，各单次使用且在其他命令上均非法。`--text` 与 `--file` 按标准
  取值规则消费下一个 token 作为值（以减号开头的 token 也会被消费，裸 `--`
  不会）；`--key` 取一个封闭键名；`--stdin` 无值。`--text` 必须是非空 UTF-8
  字面量，其字节长度加上可选折叠 CR 不越过 4,096 字节记录上限；`--file`
  原样存储其操作数——解析器不做任何文件系统校验，一切路径、类型与内容
  检查都归 [Send 输入源](#send-输入源) 的物化器。`--enter` 是文本类源的
  修饰符：`--text`、`--file`、`--stdin` 上合法，`--key` 上或单独出现均非法。
  零个或多于一个源均为 `invalid-usage`；冲突按固定次序
  `--text`、`--key`、`--file`、`--stdin` 命名，与 argv 次序无关。
- `--mouse off|on`：带值 flag，单次使用，仅显式 `tui` 路由受理——宿主机指针捕获
  用于面板命中，默认开启。语法仅两 token 形式；`--mouse=<value>`、缺失或未知值、
  重复、出现在其他命令路由或裸调用，均为 `invalid-usage`。
- `--version`：仅在独占根调用时合法；其他位置或与其他参数并存都是 usage。
- `-h`/`--help`：根级独占 token，或已识别帮助路径（family 或完整命令路径）的单一后缀。
- `--` 之后的位置参数：`--` 之后的 token 只作位置参数，故 `orchbee -- status` 查询 status，
  `orchbee project get -- --help` 解析的是字面 label `--help`。

命令路径（固定词，无缩写、无别名）：

```
orchbee                         # 裸调用 → TUI（不变）
orchbee tui [--mouse off|on]    # TUI
orchbee --version               # 版本行（不变）
orchbee -h|--help | orchbee help   # 根帮助
orchbee status
orchbee project list | project get <target>
orchbee project create --label <label>
orchbee project rename <target> --label <label>
orchbee workspace list [--project <target>] | workspace get <target>
orchbee workspace create --project <target> --label <label>
orchbee workspace rename <target> --label <label>
orchbee workspace accept-directory <target> --path <path>
orchbee tab list [--workspace <target> | --standalone] | tab get <target>
orchbee tab create (--workspace <target> | --standalone) --label <label> <geometry>
orchbee tab rename <target> --label <label>
orchbee tab move <target> --direction previous|next|first|last
orchbee tab close <target> --confirm
orchbee panel list [--tab <target>] | panel get <target>
orchbee panel split <target> --direction above|below|left|right --label <label> <geometry>
orchbee panel close <target> --confirm
orchbee panel move <target> --target-panel <target> --direction above|below|left|right
orchbee terminal list | terminal get <target> | terminal read <target> [--history --rows <1-1000>]
orchbee terminal wait-output <target> --match <text> [--rows <1-1000>] [--timeout <1-3600000>]
orchbee terminal send <target> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>
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

`terminal read --history --rows <n>` 沿用同一 attach → poll 前导——`--history` 与 `--rows`
仅成对准入，`n` 是有界保留行数 `1..=1000` 而非几何维度——然后读取已应用帧背后的滚动
历史：

- 已应用帧的保留滚动行数事实为恰好一次 `ReadRows{attachmentId, startRow,rowCount,revision,generation,requestSequence:1,maximumBytes}` 定价：`rowCount = min(n, scrollback,
262144/columns)` 且 `startRow = scrollback - rowCount`，窗口止于滚动边界。滚动为零则短路
  ——不发送 `ReadRows`，答案为空窗口。
- `record` 应答必须是完整的 OBTP `RowsSnapshot`（kind 46）响应，其 OBRW 文本窗口独立回显
  请求：attachment、Host epoch、sequence、revision、generation、columns、起始行、行数，
  以及 `totalRows = scrollback + screenRows`，且载荷不超过 `maximumBytes`（8 MiB）。任何
  失配都是 `protocol-invalid`、stage `viewer`。
- `rows-rejected` 映射到类型化词汇：`limit-exceeded` 为 `rejected`（3）；Host 状态应答
  （`history-evicted`、`stale-revision`、`stale-generation`、`missing-resource`、
  `resource-exhausted`、`invalid-phase`）为 `service-unavailable`（4），details 为
  `{stage:"viewer",operation:"read-history",reason}`；固定请求形状不可能产生的 reason 为
  `protocol-invalid`。不重试，也绝不为此读取发送 `RowsApplied`/feedback 锚点。

`terminal wait-output` 沿用同一快照准入与 attach 前导，但只读 viewer 在单次帧之后继续
保持存活，进入有界观察循环：

- **期限**——在命令开始时取一个绝对 deadline `T0 + --timeout`（默认 30000ms，准入范围
  `1..=3600000`）。cohort、discovery、connect、snapshot 以及每次 `terminal.viewer` 请求
  都在 `min(5 s local budget, remaining deadline)` 内运行；每个等待步骤返回后都复查 deadline，
  因此迟到完成仍算 `timeout`（退出 7）。该 deadline 是协作式的：同步文件系统调用无法
  在系统调用中途打断，最坏超出量是一次已开始的本地操作。
- **观察**——`Poll` 在 deadline 内重复：`Empty` 先睡 10ms，连续空回退到 100ms，收到
  record 即复位（每次睡眠以剩余 deadline 为上限——安静等待绝不忙等）；`record` 喂给
  projection，对**每个**已应用帧发送真实 `FrameApplied` feedback，并在下次 poll 前确认
  `FeedbackAccepted`；`NeedSnapshot` 按原 feedback 规则；`ProjectionBlocked` 被消费并忽略。
- **检查**——每个已应用帧钉住一次 `ReadRows`，覆盖统一缓冲窗口
  `startRow = max(0, totalRows − K)`、`rowCount = K`，其中
  `K = min(--rows, totalRows, 262144/columns)`、`totalRows = scrollback + screenRows`，
  revision 与 generation 为该帧的 `.exact` 值，`maximumBytes` 为 8 MiB。
  `requestSequence` 每附件严格递增、从 1 开始（`checked_add`；重试也取新值，且绝不与
  `transport requestId`、frame sequence、history-event sequence 混用）；解码窗口必须回显
  所发 sequence 及全部钉住字段，否则为 `protocol-invalid`。
  `stale-revision`/`stale-generation`/`history-evicted` 舍弃本次观察并在同一 deadline 内
  继续轮询——绝不匹配半窗口；其余 `rows-rejected` reason 按与 `read-history` 相同的
  typed 词汇映射（operation `"wait-output"`）。行编码器无法编码的载荷以 typed
  `rows-rejected` 失败——绝不静默截断。
- **匹配**——仅当 `wrap` 与 `wrap_continuation` 两两一致时窗口行才拼成逻辑行；不一致即
  硬边界，窗口边缘的残缺行保持诚实的部分行（绝不拿窗口外文本补齐）。首个包含 `--match`
  字面量（区分大小写）的逻辑行胜出；返回的 `excerpt` 是该行内始终包含完整 pattern 的
  ≤4096 字节片段，附 `excerptTruncated` 及其在逻辑行内的字节偏移。命中后命令返回物理
  `rowStart`/`rowEnd`（仅观察期有效，不是游标）、帧 `revision`/`generation`、已解析的
  terminal/worker/site 身份，以及观察到的 `hostEpoch`/`attachmentId`；
  `observed.completedObservations` 只计一致的窗口+匹配通过次数，
  `observed.windowTruncated` 记录被容量钳制的 `K`。
- **取消**——等待期间的 HUP/INT/QUIT/TERM 是本地 `cancelled` 失败（退出 130）：不伪造
  match，也不向终端发送输入、语义取消或 kill。检查点在网络步骤之间与每次 poll 睡眠
  前后，延迟以一次已开始的本地请求为界。
- **释放**——成功时在产出文档前确认 `Detach` 结果；清理未确认则保留真实失败类别并上报
  `{matchObserved:true,cleanup:"unconfirmed"}`，绝不转换。timeout、载具失败与取消保留其
  主失败及清理状态；session drop 总会执行，当没有剩余 Detach 预算时上报为
  `cleanup:"requested"`——绝不谎称已释放。

`terminal send` 在命令入口采样一个绝对的 5 秒 deadline 并贯穿以下全部——声明型
`--file`/`--stdin` 的源物化（第一个有副作用的步骤，先于 cohort 检查）、cohort 检查在
自身预算与该上限的较早者之下运行、discovery、admission 与快照读取只消耗剩余额度——然后
在同一 deadline 内用同一已准入 session 执行恰好一次 `terminal.send` 交换。
`--text`/`--key` 调用完全没有物化步骤：

0. **物化**（仅声明型源）—— `--file`/`--stdin` 在任何运行时访问之前被完整读入
   与 `--text` 相同的有界文本记录；见 [Send 输入源](#send-输入源)。源失败不拥有
   请求身份，也绝不抵达 Server。
1. **解析** —— 在已验证快照的 presentation 绑定中查找该 typed terminal ID；缺失 →
   `not-found`（3），details 为 `{resource:"terminal",target}`，session 随之 drop。
   绝不查询 label。
2. **发送** —— 用快照自身事实在全新请求身份下构造一条
   `terminal.send{siteId,workerId,terminalId,payload}`。payload 为
   `{kind:"text",text,enter}`——字面字节加上折叠进同一输入记录的 `--enter` 回车，合计
   上限 4096 字节——或 `{kind:"key",key}`，命名十五个语义键之一（`enter`、`escape`、
   `tab`、`backspace`、`delete`、`up`、`down`、`left`、`right`、`home`、`end`、
   `page-up`、`page-down`、`ctrl-c`、`ctrl-d`）。键是协议级名称，绝不是预置转义字节串。
   随后在任何类型化结果映射与输出准备之前完全释放 Client Session；send 绝不重试。
3. **应答** —— Server 的 `terminal.send.result` 拆分为 envelope 逐字报告的两条轴：
   `inputStatus`（`accepted` 表示 Host 已把输入记录入队——绝不表示子进程已读取或执行；
   `rejected`/`not-submitted` 表示该记录可证明从未到达队列；`unknown` 表示其可能
   已经落地）与 `cleanupStatus`（`not-needed`/`confirmed`/`unconfirmed`，其中
   `unconfirmed` 表示临时 input-only attachment 可能存续至 carrier 拆除或 Host-epoch
   结束——绝不宣称已释放）。发送经由共享的 Worker↔Host 连接：当该连接失效、或其
   清理无法确认时，同一 Host 上的其它 viewer 可能被中断——这是底层连接的
   transport 级副作用，并非正常 send 夺取控制权。结果的物理 `target`（`workerGeneration`、`hostEpoch`、
   `hostTerminalId`）是 Server 实际准入的 incarnation，逐字投影；准入前拒绝不携带。
   退出映射：`accepted`+`confirmed` → 0；`accepted`+`unconfirmed` → 5 并附勿重发警告
   （输入可能已生效）；`rejected` → 3；`not-submitted` → 4，唯协议故障 reason
   `host-protocol-fault` 属 transport 类缺陷 → 5；`unknown` → 8 并附
   绝不重试警告。每个非零应答都保持 `inputStatus` 诚实——已接受的输入绝不改写为未发送。

### Send 输入源

`--file` 与 `--stdin` 是*声明型*本地源：解析器原样存储，由源阶段在任何运行时副作用
之前各自物化为恰好一条有界文本记录——完整记录存在之前不发生 cohort、discovery、
admission、快照或 send。物化后的记录在下游与 `--text` 不可区分：一条记录喂给一条
`terminal.send` 请求，成功 schema 不变。

- **纯输入边界。** 语法与 target 校验先于一切源 setup/读取完成。四个源 flag 互斥且
  单次使用；`--enter` 只属于文本类源。源失败不拥有请求身份、不接触任何对端、绝不
  归为 transport loss——Server 对任何准入前失败观察到零副作用。
- **单一共享预算。** 命令入口采样的绝对 5 秒 deadline 覆盖源 setup、物化与整个 send
  生命周期；绝不重置或续期。每次源等待与每次读取之前都检查 deadline——包括恰在
  deadline 处结束的等待之后的读取——成功边界本身同样受仲裁：在预算关闭之后才
  观察到的输入结束是 `timeout`，绝非干净记录；过期之后不再发生任何运行时准入。
- **文件准入。** `--file` 以 `O_RDONLY|O_NONBLOCK|O_CLOEXEC` 打开——open 本身不会在
  FIFO 或设备上阻塞——随后在已打开 fd 上的 `fstat` 必须报告 regular file，这同时闭合
  路径替换竞态：被换掉的路径仍报告其真实类型。打不开/读不了的路径、非 regular 结果
  （目录、FIFO、设备或被替换的项）、空文件或读错误都是源阶段 `invalid-usage`。
- **stdin 准入。** `--stdin` 拒绝终端描述符，再按继承描述符的精确 `S_IFMT` 类型分派：
  regular 文件在借用描述符上原位、无缓冲读取——共享文件位置恰好按返回字节数推进，
  至多 cap+1 探测字节（这正是继承位置契约）；
  FIFO/管道经 `/proc/self/fd/0` 重开为私有非阻塞文件描述，因此绝不改动调用方描述符
  旗标、也无需恢复；重开结果必须验证为 FIFO，其他一切类型（socket、设备等）一律拒绝。
- **有界记录。** 采集上限 4,097 字节——wire 上限 4,096 加一——故恰好 4,096 字节成功、
  多一字节即拒绝且不再继续 drain。字节必须严格 UTF-8 解码；不裁剪、不切分、不去 BOM、
  不做 CRLF 归一。`--enter` 保留为 wire 的 folded-CR 旗标——同一记录内的一个回车，与
  `--text` 完全一致——并计入同一上限，故 4,095 字节
  +CR 是接受边界、4,096+CR 被拒绝。源路径与载荷字节绝不出现在任何诊断中。
- **源信号 guard。** 在 Linux 上源阶段在其存续期内拥有进程信号捕获——单线程、无其他
  信号 owner、交接前不 spawn 子进程。合格集为 {SIGHUP, SIGINT, SIGQUIT, SIGTERM}
  减去进程在源入口已阻塞或忽略的全部信号：忽略集取自严格有界的
  `/proc/self/status` `SigIgn` 字段（最多读 64 KiB，须恰好一个合法十六进制字段——
  缺失、畸形、重复或超限均在任何源 I/O 之前报 internal failure），阻塞集取自当前
  线程 mask。Setup 先为合格集创建 `NONBLOCK|CLOEXEC` signalfd——此时继承 disposition
  仍然生效，故该处 setup 故障无需恢复任何东西——随后原子交换线程 mask 并捕获旧
  mask；捕获区间自交换起、先于任何源读取。绝不触碰 disposition，继承的 blocked/
  ignored 状态原样保留——空合格集是真实的 armed 策略（无可捕获），不是 no-op
  回落；继承的 pending 信号在原 mask 下保持 pending。
- **仲裁与最终化。** 读取循环把信号描述符与源描述符并列 poll，在处理就绪数据之前、
  判定超时之前、以及完成边界再各 drain 一次；首个被消费的合格信号选定源取消，且
  优先于 deadline 与源 I/O 错误。物化收尾以最后一次非阻塞信号读为线性化点：读到即
  取消；`EAGAIN` 表示源不再拥有取消——信号描述符关闭、旧 mask 在运行时准入之前被
  精确恢复。该读之后到达的信号保持 pending，在 unmask 时按原 disposition 交付——
  guard 绝不吞掉它——故迟到信号仍可能在准入前以默认行为终止进程。每个受监视
  syscall 都被检查：描述符创建、mask 交换、每次信号读与恢复；出错即 internal
  failure，绝不当作"无信号"；恢复失败是终态且无运行时准入。取消路径上合格 mask
  保持阻塞直到公共 `main` emit 完成输出尝试，再在同一路径恢复旧 mask——之后的
  信号仍可能在输出完成后终止进程，不对此承诺退出码。
- **失败兼容。** 校验与本地原因留在标准 `invalid-usage` 路径（无 details）。源超时与
  取消携带 `orchbee.cli.failure.source` v1 envelope——`{stage:"input",
inputSource:"file"|"stdin"}`，仅超时另带 `timeoutMs`（配置的共享预算，绝非实测
  时长）——无权威 `source`、无请求身份，因为尚无任何准入。源取消经与其他输出
  相同的公共 emit finalizer 按惯例 `128 + signo` 退出（129/130/131/143）——状态
  覆写仅存在于该 envelope——源超时仍为退出 7。
- **已声明的 OS 边界。** deadline 是每个 syscall 周围的逻辑界限，不进入其内部：内核
  卡死的 regular-file 读不可抢占，故不承诺该处及时终止。mask 交换前的 pre-arm 窗口
  在继承 disposition 下运行——该处默认死亡是诚实且无副作用的，不是 guard 窗口
  漏洞，且任一种 arming 次序都不能闭合全部信号到达竞态。继承的 `SIG_IGN`
  disposition 被尊重而非覆盖。以上均为依托 procfs 的 Linux-only 声明，与 procfd
  输入准入一致。

## Canonical 数据与结果

Canonical 遍历次序：`navigation.projects` 顺序；各 project 的 `workspaces` 顺序；
`terminalContexts` 顺序接 `tabs` 顺序；panels 深度优先且 `children[0]` 先于 `children[1]`；
terminal 绑定按该 panel 顺序且 `primaryPresentation` 先于 `childPresentations`。不新增排序；
过滤后的列表与 `ambiguous` 候选都保持该次序。

成功 envelope 为 `{schema, version, source, result}`。所有业务文档——查询、mutation、
收据与失败——均为 `version:1`。仅 `orchbee.api.schema` 目录 envelope 为 `version:2`：
注册新的 schema id 会改变封闭字典的形状，按版本规则属目录形状变化而非业务变化：

- `source.kind="workbench-snapshot"`：带同一份已验证快照的 `siteId` 与十进制字符串
  `revision`——仅在完整有效读取之后出现。
- `source.kind="workbench-command"`：带 `siteId`、`requestId` 与十进制字符串
  `expectedRevision`——已产出的 mutation 结果，以及有效读取之后 command 阶段的失败。
- `source.kind="embedded"`：随二进制编译内嵌的 schema 字典（`api schema` 结果及其
  `not-found` 失败）。
- `source.kind="local-runtime"`：已产出的 `config check` 结果。
- `source.kind="terminal.send"`：`siteId`、send `requestId`，以及解析出该绑定的十进制
  字符串 `snapshotRevision`——出现在类型化 `terminal send` 应答上。
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
- `orchbee.terminal.history` → `{terminal,history,content}`：`history` 携带已定价窗口的
  事实——`revision`/`generation`、`alternateScreen`、`columns`、`screenRows`、十进制字符串
  `scrollbackRows`/`totalRows`/`startRow`、`requestedRows` 与 `windowRows`（Host 窗口的完整
  行数）；`content` 携带 `rows`（`{index,text,continuation}`，`index` 为"保留区+屏幕"绝对行
  坐标的十进制字符串）、`rowCount`（发出的行数）、`truncated`/`omittedRows` 与 `limits`
  `{rows:1000,textBytes:4194304}`。`omittedRows` 诚实计为 `min(requestedRows,scrollbackRows)` 减去已发出行数——即被 `MAXIMUM_FRAME_CELLS/columns` 上限或文本预算截掉的
  行——而早于 `startRow` 的行从未被请求，不算遗漏。text 输出与 `terminal read` 一致：每个
  发出的行渲染为一行转义文本加标记。
- `orchbee.terminal.send` → `{action:"terminal.send",requestId,siteId,workerId,terminalId,
payloadKind,recordBytes?,key?,inputStatus,cleanupStatus,reason?,acceptedBytes?,target?}`——
  请求自身事实加上 Server 的两轴应答：`payloadKind:"text"` 携带 `recordBytes`（单条输入
  记录的字节长度，含折叠的回车），`payloadKind:"key"` 携带 `key`；`inputStatus`/
  `cleanupStatus` 为逐字结果类别；`reason` 为类型化拒绝/不可用 token；`acceptedBytes`
  仅在 `accepted` 时出现；`target` 为 Server 实际准入的物理 incarnation
  （`workerGeneration`、`hostEpoch`、`hostTerminalId`）——只要应答承认一个（`accepted`、
  `outcome-unknown` 或任何 cleanup 义务）即出现，准入前拒绝则缺席。提交的文本绝不回显到
  文档或诊断的任何位置。
- `orchbee.api.snapshot` → `{snapshot}`：完整 `WorkbenchProtocolDocument::Snapshot` 的语义化
  重编码，含 `kind:"workbench.snapshot"`；envelope 的 `siteId`/`revision` 与内部文档一致。
- `orchbee.api.schema` → 带参 `{schema:{schemaId,document}}`；无参
  `{schemas:{schemaId:document,…}}` 列出全部已交付字典（draft 2020-12）。
  目录保持封闭：`result.schemas` 恰好命名已注册的 id，且每个内嵌文档的 `$id`
  与其键一致。按旧目录校验的 introspection 客户端从同一构建中选择内嵌 v2 文档。
- `orchbee.mutation.result` → `{action,outcome,revision?,acceptedRevision?,closeRequestId?,
<resolved typed IDs>,direction?,label?,terminal?}`，按上文 mutation 矩阵：`committed`/
  `unchanged` 带 `revision`，`accepted` 带 `acceptedRevision`+`closeRequestId`，
  `tab create`/`panel split` 带 `terminal:{outcome,reason?,geometry?}`。
- `orchbee.project.rename` → `{action:"project.rename",outcome,revision,projectId,label}`，
  `outcome` 取值 `committed|unchanged`：`project rename` 收据的专用 v1 文档，
  共享 `orchbee.mutation.result` 的 action 枚举保持封闭、不改动。
- `orchbee.config.check` → `{ok,checks:[{name,status,details}]}`，固定次序
  `runtime-base`、`runtime-directory`、`cohort`、`locale`、`sockets`；五项永远齐全。存在失败
  项时仍产出该成功 schema（`ok:false`）并以 1 退出。
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`；不存在英文 `message` 字段。`source` 只取自已可信来源（读取后失败用已验证快照；
  `api schema` not-found 用 `embedded`；mutation command 阶段失败一律用
  `workbench-command`）；读取之前的失败没有 source。
- `orchbee.cli.failure.source` → `{schema:"orchbee.cli.failure.source",version:1,
failure:{kind,details}}`——send 源阶段专用 envelope，仅 `timeout` 与 `cancelled`。
  `details` 为 `{stage:"input",inputSource:"file"|"stdin"}`，超时另带 `timeoutMs`；
  绝不含权威 `source`、请求身份、路径或载荷字节，因为该失败可证明先于任何运行时
  副作用。

### Config 检查

| 检查项              | 规则                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime-base`      | 解析并观测 `ORCHBEE_RUNTIME_BASE`/默认值：存在且为目录则 pass；路径非法、缺失、不安全项或 I/O 错误则 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `runtime-directory` | 基座失败、或合法基座下 scope 目录不存在时为 `not-applicable`（Site 未运行不是配置错误）；既有目录通过校验为 pass；属主/模式/类型不安全或 I/O 错误为 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `cohort`            | `NativeCohort::current` 在其 5 秒预算内验证通过则 pass（`verified:true`）；缺失、非法、散列不符（`invalid-cohort`）或超时（`deadline`）为 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `locale`            | 信息性 pass，携带 `locale`（`en-US`/`zh-CN`）与 `source`（`LC_ALL`/`LC_MESSAGES`/`LANG`/`default`）。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
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

`terminal send` 修改的是 Host 输入状态而非 Workbench 文档：它走上文描述的已准入 session
上的 OBCS `terminal.send` 交换，不触碰 `expectedRevision`、Workbench 命令通道，或 TUI 的
controller/viewer/resize 所有权。

十二条命令经已 admitted 的 OBWP `workbench.command` 通道修改 Workbench 状态：
`project create`/`rename`、`workspace create`/`rename`/`accept-directory`、
`tab create`/`rename`/`move`/`close`、`panel split`/`close`/`move`。每次调用原样执行查询状态机的第 1–5 阶段——一次 cohort 检查、
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

`project create` 是唯一无 target 操作数、无文件系统副作用的修改命令：动作只携带 label，
由 Server 分配新 `projectId` 并在 `committed` 收据中返回，新建 Project 无目录、无 Workspace。
label 允许重复——label 无法唯一标识新 Project；收据中的 `projectId` 可以。

`workspace accept-directory` 提交既有 `WorkspacePrimaryDirectoryAccept` 动作——与 TUI
目录向导所用操作相同——因此 `project create → workspace create → workspace
accept-directory → tab create` 整条序列无需 TUI。target 按普通 Workspace 规则解析（先
按 typed ID、再按精确 label；歧义 label 拒绝），真正的 `projectId` 取自与已解析
Workspace 相同的快照行。`--path` 在 CLI 边界解析为 `WorkbenchDirectoryPath`（字面绝对
路径，≤4,096 UTF-8 字节，不含 NUL；`~`/`$` 保持字面）——CLI 从不创建、修复、规范化
或探测它，故不存在或非目录路径仍正常提交，经 Server 的 `DirectoryUnavailable`
precommit 呈现为 `service-unavailable`（exit 4）。Server 对导航修改、目录关联、
worker 探测与 generation、乐观修订、资源上限保持权威：已持有目录的 Workspace——包括
相同路径——被拒绝（`workspace-directory-conflict`，exit 3），CLI 既不预检
`primary_directory_id` 也不推断幂等成功。`committed` 收据携带新 `revision`、Server
分配的 `directoryId` 与 `workerId`、已解析的 `projectId` 与 `workspaceId`，以及请求的
字面 `path`（永不为规范化形式）。

几何永远显式：非 TTY 调用无法继承终端尺寸，故 `tab create` 与 `panel split` 要求四个 flag
全部给出。`--cols`/`--rows` 为格数，`--cell-width`/`--cell-height` 为每格像素尺寸；每个
维度都是 `u16`，并在任何运行时访问之前经协议 `WorkbenchTerminalGeometry` 校验。

Mutation 结果使用 `schema:"orchbee.mutation.result"`、`version:1`，
`source:{kind:"workbench-command",siteId,requestId,expectedRevision}`，`result` 携带 `action`
（点分命令名）、`outcome`、该结果的 revision 字段、已解析 typed ID，以及命令具备时的
`direction`/`label`/`terminal`。文本输出按与查询相同的转义规则逐行渲染同样字段。
`project rename` 是唯一例外：其收据使用专用的 `schema:"orchbee.project.rename"`、
`version:1` 文档——`source` 仍为同一 `workbench-command` 形态，`result` 封闭于
`action:"project.rename"`、`outcome`（`committed`/`unchanged`）、`revision`、
`projectId` 与 `label`。

明确不交付：`project close`、`workspace close`/`workspace focus`、`tab focus`、
`panel resize`/`panel focus`、终端 open 操作、Site 生命周期命令，以及一切远端操作。

## 失败矩阵

退出码：0 成功（含空表）；1 本地配置/输出/内部确定失败；2 usage/非法 target；3
not-found/ambiguous/denied/rejected；4 service-unavailable（含 `session-capacity`）；5
transport-loss/protocol-invalid；6 incompatible；7 `timeout`（`terminal wait-output`
期限耗尽未命中，或 send 源物化期间共享 deadline 到期）；8 outcome-unknown；130
`cancelled`（被处理的 SIGINT 类中断——`terminal wait-output` 期间收到
HUP/INT/QUIT/TERM；更一般地，被处理的 `terminal send` 源阶段信号按惯例
`128 + signo` 退出：合格集对应 129/130/131/143）。源 guard 未捕获的合格信号按原
disposition 交付——OS 默认终止遵循 shell 的 `128 + signal` 约定，不是 CLI 合成的结果。

| 来源                                                                                                                         | `failure.kind`                               | 退出      | `details`                                                                                 |
| ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------- | --------- | ----------------------------------------------------------------------------------------- |
| argv 封装/语法/format/target 形状错误                                                                                        | `invalid-usage`                              | 2         | 省略                                                                                      |
| 全零 ID/空/超长 label、非法 panel/terminal ID                                                                                | `invalid-identifier`                         | 2         | `{resource,target}`                                                                       |
| 合法 target 或 schema ID 无匹配                                                                                              | `not-found`                                  | 3         | `{resource,target}`                                                                       |
| 多个精确 label 匹配                                                                                                          | `ambiguous`                                  | 3         | `{resource,target,candidates≤8,totalCount}`                                               |
| `NativeCohort::current` 出错                                                                                                 | `check-failed`                               | 1         | `{stage:"cohort"}`                                                                        |
| 基座非法/缺失/非目录、不安全 runtime/socket、其他校验                                                                        | `check-failed`                               | 1         | `{stage:"runtime-base"                             \| "runtime-directory"   \| "socket"}` |
| 合法基座下 scope 目录或 `client.sock` 不存在                                                                                 | `service-unavailable`                        | 4         | `{stage:"discovery"}`                                                                     |
| `ConnectClientError::ConnectFailed`                                                                                          | `service-unavailable`                        | 4         | `{stage:"admission"}`                                                                     |
| `ConnectClientError::Io`                                                                                                     | `transport-loss`                             | 5         | `{stage:"admission"}`                                                                     |
| OBLC `Rejected(CapacityExceeded)`                                                                                            | `session-capacity`                           | 4         | `{stage:"admission"}`                                                                     |
| OBLC `Rejected(HandshakeTimeout)`                                                                                            | `service-unavailable`                        | 4         | `{stage:"admission"}`                                                                     |
| `Rejected(IncompatibleProtocol                                    \| IncompatibleCohort)`、`Protocol(IncompatibleVersion)`   | `incompatible`                               | 6         | `{stage:"admission"}`                                                                     |
| `Rejected(WrongMessage                                            \| MalformedFrame)`、`UnexpectedResponse`、其余 `Protocol` | `protocol-invalid`                           | 5         | `{stage:"admission"}`                                                                     |
| `RequestIoConfiguration`、`SessionUnavailable`、`RequestEncoding`                                                            | `internal-failure`                           | 1         | 省略                                                                                      |
| `SnapshotReadError::RequestNotSubmitted`（任何原因）                                                                         | `transport-loss`                             | 5         | `{stage:"snapshot"}`                                                                      |
| `Failed(Write                                                     \| ResponseDeadline)`、`Failed(Response(Io))`              | `transport-loss`                             | 5         | `{stage:"snapshot"}`                                                                      |
| `Failed(Response(InvalidHeader                                    \| Contract))`、`UnexpectedResponse`                       | `protocol-invalid`                           | 5         | `{stage:"snapshot"}`                                                                      |
| `Failed(CorrelationMismatch)`                                                                                                | `protocol-invalid`                           | 5         | `{stage:"snapshot",reason:"correlation-mismatch"}`                                        |
| outcome `Denied`                                                                                                             | `denied`                                     | 3         | 省略                                                                                      |
| outcome `Unavailable`                                                                                                        | `service-unavailable`                        | 4         | `{stage:"snapshot"}`                                                                      |
| viewer `RequestNotSubmitted`、`OutcomeUnknown(Write\|Read)`                                                                  | `transport-loss`                             | 5         | `{stage:"viewer"}`                                                                        |
| viewer `OutcomeUnknown(InvalidResult\|UnexpectedResult)`、错误 outcome 类别                                                  | `protocol-invalid`                           | 5         | `{stage:"viewer"}`                                                                        |
| viewer `OutcomeUnknown(CorrelationMismatch)`                                                                                 | `protocol-invalid`                           | 5         | `{stage:"viewer",reason:"correlation-mismatch"}`                                          |
| viewer outcome `Rejected(AlreadyAttached\|TerminalUnavailable)` 或 `Unavailable`                                             | `service-unavailable`                        | 4         | `{stage:"viewer"}`                                                                        |
| viewer outcome `Rejected(FeedbackMismatch)`、record 被投影拒绝                                                               | `protocol-invalid`                           | 5         | `{stage:"viewer"}`                                                                        |
| `ReadRows` outcome `RowsRejected(LimitExceeded)`                                                                             | `rejected`                                   | 3         | `{stage:"viewer",operation:"read-history",reason:"limit-exceeded"}`                       |
| `ReadRows` outcome `RowsRejected`（其他可产生 reason）                                                                       | `service-unavailable`                        | 4         | `{stage:"viewer",operation:"read-history",reason}`                                        |
| `ReadRows` outcome `RowsRejected(InvalidField\|InvalidAcknowledgement)`、行窗口 record 失配/畸形                             | `protocol-invalid`                           | 5         | `{stage:"viewer"}`                                                                        |
| `DefinitelyNotSubmitted::RequestNotSubmitted`（command）                                                                     | `transport-loss`                             | 5         | `{stage:"command"}`                                                                       |
| `DefinitelyNotSubmitted::SessionUnavailable`/`RequestEncoding`                                                               | `internal-failure`                           | 1         | 省略                                                                                      |
| command `Rejected` precommit                                                                                                 | `rejected`                                   | 3         | `{stage:"command",reason,revision}`                                                       |
| command `Denied` precommit                                                                                                   | `denied`                                     | 3         | 省略                                                                                      |
| command `Unavailable` precommit                                                                                              | `service-unavailable`                        | 4         | `{stage:"command",reason}`                                                                |
| `OutcomeUnknown`（任意原因）                                                                                                 | `outcome-unknown`                            | 8         | `{stage:"command",requestId,reason}`                                                      |
| send `RequestNotSubmitted`（请求可证明从未到达线缆）                                                                         | `transport-loss`                             | 5         | `{stage:"send"}`                                                                          |
| send `SessionUnavailable`/`RequestEncoding`                                                                                  | `internal-failure`                           | 1         | 省略                                                                                      |
| send `OutcomeUnknown`（写/读/无效结果/correlation）                                                                          | `outcome-unknown`                            | 8         | `{stage:"send",requestId,reason}`                                                         |
| `terminal.send.result` `Rejected`/`Unavailable`                                                                              | 类型化应答，非失败 envelope                  | 3/4       | send 结果 `reason`、`cleanupStatus`；`host-protocol-fault` 退出 5                         |
| `terminal.send.result` `Accepted` + `Unconfirmed` cleanup                                                                    | 类型化应答，非失败 envelope                  | 5         | send 结果 `acceptedBytes`、`cleanupStatus`                                                |
| send 源校验：打不开/非 regular 路径、终端或不支持的 stdin、空、超限、非 UTF-8                                                | `invalid-usage`                              | 2         | 省略（本地化 stderr 指明所声明选项与具体原因）                                            |
| send 源物化期间共享 deadline 到期                                                                                            | `timeout`，属 `orchbee.cli.failure.source`   | 7         | `{stage:"input",inputSource,timeoutMs}`                                                   |
| send 源物化期间捕获合格信号                                                                                                  | `cancelled`，属 `orchbee.cli.failure.source` | 128+signo | `{stage:"input",inputSource}`                                                             |
| send 源信号 setup/读取/恢复失败                                                                                              | `internal-failure`                           | 1         | 省略                                                                                      |
| 预备输出超 8 MiB                                                                                                             | `output-too-large`                           | 1         | `{limitBytes:8388608}`                                                                    |
| stdout 写/flush 失败                                                                                                         | `output-failed`                              | 1         | 不追加第二份 stdout 文档                                                                  |
| `config check` 完成但存在失败项                                                                                              | 成功 schema `ok:false`                       | 1         | 五项全部保留                                                                              |
| 非 Linux 网络查询、mutation 或 `config check`                                                                                | `service-unavailable`/`check-failed`         | 4/1       | `{stage:"platform",reason:"unsupported-platform"}`                                        |
| `terminal wait-output` 期限耗尽未命中                                                                                        | `timeout`                                    | 7         | `{stage:"wait",timeoutMs,observed}`                                                       |
| `terminal wait-output` 期间收到 HUP/INT/QUIT/TERM                                                                            | `cancelled`                                  | 130       | `{stage:"wait",cleanup}`                                                                  |
| `terminal wait-output` 命中但 `Detach` 未确认                                                                                | 类型化 viewer-stage 失败                     | *         | `{stage:"viewer",matchObserved:true,cleanup:"unconfirmed"}`                               |

`resource` ∈ `project|workspace|tab|panel|terminal|schema`；`stage` ∈
`cohort|runtime-base|runtime-directory|socket|discovery|admission|snapshot|viewer|command|send|platform|wait`。
源阶段的 `input` stage 只存在于 `orchbee.cli.failure.source` envelope——不进入标准
`orchbee.cli.failure` 的 stage 词汇。
stage 与 reason 是有限 schema 值；任何异常文本都不进入机器字段。`details` 是按 kind 封闭的词汇——`{resource,target}`、
`{resource,target,candidates≤8,totalCount}`、`{stage[,reason]}`、
`{stage:"command",reason,revision}`、`{stage:"command",requestId,reason}`、
`{stage:"send",requestId,reason}`、
`{stage:"viewer",operation:"read-history"|"wait-output",reason}`、
`{stage:"wait",timeoutMs,observed}`、`{stage:"wait",cleanup}`、
`{stage:"viewer",matchObserved,cleanup}` 或
`{limitBytes}`——内嵌 schema 拒绝超出这些形状的文档。读路径错误永不归类为
outcome-unknown/failed-effect：只有 command 阶段可能产生不可判定效果；
`rejected`/`denied`/`unavailable` precommit 答案都是确定的——Server 未应用任何效果。
send 阶段对其单条输入记录遵循同一规则：`RequestNotSubmitted` 是确定的未提交，
`OutcomeUnknown` 表示记录可能已经落地，而类型化 `terminal.send.result` 应答是 Server
自己的判定，绝非客户端猜测。声明型 `--file`/`--stdin` 源更严格：其一切失败都发生在
任何运行时访问之前，故绝不可能产生 `outcome-unknown`；被捕获信号的取消或共享
deadline 到期在 wire 请求尚不存在时即退出。

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
  例外是 `terminal read`/`terminal wait-output`，其只读能力 viewer attachment 是唯一被
  认可的读路径副作用，最小化持有并在输出前释放。至多持有一个短暂 Client Session，且绝不
  带入输出阶段。
- Mutation 每次调用在全新 `WorkbenchRequestId` 下至多提交一次 `workbench.command`；绝不
  重试或重放，结果不可判定时报告 `outcome-unknown` 而非猜测。`terminal send` 对其唯一
  一次 OBCS `terminal.send` 交换遵循同一纪律：每次调用在全新请求身份下至多一条输入记录，
  绝不重试——不可判定的应答是 `outcome-unknown`，且本地化诊断会说明输入可能已经落地，
  由操作员据此决定是否重复。
- 声明型 `terminal send` 源在任何运行时访问之前被完整物化：恰好一条有界记录（≤4,096
  字节含折叠 CR，严格 UTF-8）喂给恰好一条 `terminal.send` 请求——绝不切分、重放或
  部分提交。源超时、取消或校验失败产生零运行时副作用且无请求身份；物化之后普通 send
  生命周期权威接管，源阶段结果不能在语义上取消已被准入的 send。
- 源信号 guard 绝不改动 disposition，并把继承的线程 mask 精确恢复——被捕获、继承
  阻塞与继承忽略的信号各自保持原语义；没有任何信号 owner 活过源阶段，也没有任何
  合格信号在输出最终化路径之后仍被阻塞。
- 每次调用至多一次快照读取、一次准入，且读取与命令共用一个网络 deadline；`terminal read`
  的 viewer 交换与 `terminal send` 的唯一 send 复用同一 session 与同一 deadline——绝无
  第二次准入或续期预算。
  `terminal wait-output` 保持同一次准入，并把固定 5 秒预算替换为命令开始时取的用户
  `--timeout` deadline；其每附件 `requestSequence` 在全部 `read-rows` 中从 1 严格递增。
- match 只证明该文本在本次观察的搜索窗口内被找到——绝不证明命令完成、新 turn 或没有
  其他输出；轮询间隙滚过 Host 保留区的输出对任何消费者都不可见。
- 没有可信来源的字段不存在；`null` 表示来源确定地表达"无"，`"unknown"` 表示来源无法回答——
  两者绝不混用。
- 失败 envelope 绝不虚构 `source`：siteId/revision 只来自已验证快照。
- Ambiguous 绝不猜测：报告有界候选与真实总数。
- `api schema`、help 与 completion 只枚举同一次构建中真实交付的命令。
- 两个 5 秒预算都是 CLI 自有的 checked deadline；其构造失败是 `internal-failure`，而 transport
  内部 `RequestNotSubmitted` 一律保持 `transport-loss`。

## 场景矩阵与证据

| 场景                                                                                                                                                                     | 预期                                               | 证据                                                      |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------- | --------------------------------------------------------- |
| 穷举语法：format、位置、`--`、help 形式、重复、非法                                                                                                                      | 精确 parse/失败类别                                | `src/invocation` 测试                                     |
| format envelope 次序：任何位置的缺陷即空 stdout；任何位置的合法 json 决定其后失败的 envelope                                                                             | envelope 优先于语法错误                            | `src/invocation` 测试 + `tests/cli_query.rs`              |
| 过滤器范围：`workspace list` 只接受 `--project`；越界过滤器在任何运行时访问之前即 usage                                                                                  | 完整矩阵，过滤器绝不静默丢弃                       | `src/invocation` 测试 + `tests/cli_query.rs`              |
| 目标规则：ID/label/全零/超长/歧义优先                                                                                                                                    | resolver 判定                                      | `src/cli` resolver 测试                                   |
| 已验证快照 → 各结果形状、空集合、null                                                                                                                                    | 精确 JSON/text fixture                             | `src/cli` 输出测试 + JSON Schema 校验（dev `jsonschema`） |
| Schema 字典 ↔ 实际 envelope                                                                                                                                              | 每个成功/失败 fixture 通过校验；被拒变体不通过     | schema 对比测试                                           |
| API 失败格式：text/默认失败保持 stdout 为空，`--format json` 产出 envelope                                                                                               | `api schema`/`api snapshot` 失败遵守调用方 format  | `tests/cli_query.rs` + `src/cli` 测试                     |
| 无 Site 磁盘状态                                                                                                                                                         | `service-unavailable`，零 fs/进程/锁变更           | `tests/cli_query.rs`                                      |
| 真实 staged-cohort Server：status/list/get/api snapshot、8 个准入加第 9 个被拒加释放、活跃 TUI                                                                           | 成功/`session-capacity`/结构不变                   | `tests/local_startup_process`                             |
| 脚本化 OBLC/OBWP peer：denied、unavailable、畸形、错误 correlation、超时、断连                                                                                           | 按矩阵 typed kind；绝不冒称真实 Server 证据        | `tests/cli_query.rs`                                      |
| 脚本化 OBCS viewer peer：attach-binding 字段、`Empty`→`record` 轮询、detach 释放、reject/unavailable/畸形/correlation/断连                                               | `terminal read` 成功 + typed viewer-stage 失败     | `tests/cli_query.rs`                                      |
| 脚本化 OBCS viewer peer + 手工 OBRW record：滚动定价、容量/滚动钳制、typed `rows-rejected`、窗口失配、超大 record、无重试                                                | `terminal read --history` 成功 + typed 失败        | `tests/cli_query.rs`                                      |
| 脚本化 OBCS send peer：UTF-8 文本与可选折叠回车、语义键均以一条请求到达，target/请求身份，accepted/rejected/unavailable/unknown 应答、停滞应答、无重试                   | `terminal send` 退出 0/3/4/5/8 且两轴结果字段齐全  | `tests/cli_send.rs`                                       |
| send 源语法：四源互斥、固定次序冲突命名、`--enter` 适用范围、flag 范围                                                                                                   | 任何运行时访问前的精确 parse/`invalid-usage`       | `src/invocation` 测试                                     |
| 文件/stdin 物化：非阻塞 open + `fstat` 类型闸、regular 文件共享 offset、私有 FIFO 重开、cap+1、严格 UTF-8、空/非 regular/不支持类型的拒绝                                | 有界记录或类型化源 `invalid-usage`；零运行时副作用 | `src/cli/source` 单元测试 + `tests/cli_send.rs`           |
| 源 deadline/取消：停滞管道在共享 deadline 退出 7、被捕获合格信号按 `128+signo` 携带 `orchbee.cli.failure.source`、继承 IGN/BLK 原样保留、恢复失败即 internal             | 源 envelope 事实、无 wire 请求、精确 mask 恢复     | `src/cli/source` 测试 + 标注的 OS/进程证据                |
| 脚本化 OBCS send peer + 声明型源：物化后的文件/stdin 以一条 `terminal.send` 文本请求到达                                                                                 | 与 `--text` 完全相同的 envelope；恰好一条请求      | `tests/cli_send.rs`                                       |
| 脚本化 OBCS viewer peer + 手工 OBTF/OBRW record：递增 `requestSequence`、可见/历史边界窗口、每帧真实 `FrameApplied`、stale 重钉、行尾 excerpt 命中、含 preamble 的总期限 | `terminal wait-output` 成功 + typed 失败           | `tests/cli_query.rs`                                      |
| 真实 staged-cohort Server + 活跃 TUI 终端：`wait-output` 命中已有/后到/滚出屏/UTF-8 输出，短期限退出 7，取消后槽位可复用                                                 | 公共等待观察与 TUI 自身 viewer 共存                | `tests/local_startup_process`                             |
| 真实 staged-cohort Server + 活跃 TUI 终端：`terminal read` 返回其网格并通过 schema 校验                                                                                  | 真实只读观察与 TUI 自身 viewer 共存                | `tests/local_startup_process`                             |
| Mutation 语法：全部命令、target 规则、`--confirm`、几何界限、flag 范围                                                                                                   | 运行时访问前的精确 parse/失败类别                  | `src/invocation` 测试                                     |
| Mutation 脚本化 peer：全部十二条命令、precommit 判定、不可判定与未提交结果                                                                                               | 每次调用一条命令；`outcome-unknown` 绝不重放       | `tests/cli_mutation.rs`                                   |
| 非 TTY；stdout 阻塞时 session 已释放；各 format 预备 >8 MiB；真实写失败                                                                                                  | 文档化退出；阻塞前 transport 句柄已 drop           | `tests/local_startup_process` + emit 单元测试             |
| `config check` 矩阵：每项 pass/fail/not-applicable、`exists` 语义                                                                                                        | 五项 typed 检查齐全                                | `tests/cli_query.rs` + `src/cli/config` 测试              |
| 补全脚本投影语法（bash/zsh 实执行，fish 结构对齐）                                                                                                                       | 正/负上下文；无运行时枚举                          | `tests/cli_query.rs`                                      |
| 双语文本/失败输出                                                                                                                                                        | 精确 en-US/zh-CN 字符串                            | `tests/process.rs` + `tests/cli_query.rs`                 |

模型义务：本切片不新增 Server 状态机、持久化或重试，复用已注册的 OBLC/OBWP/OBCS/Client
模型——`terminal read` 复用既有 `terminal.viewer` attach/poll/detach 生命周期，`--history`
在其中加一次有界 `read-rows`/`rows-rejected` 交换，`terminal wait-output` 则循环同一
poll 并对每个已应用帧加一次有界 `read-rows`——因此无需新 Server 侧协议模型。该命令自身的
结算门——逐步 flag 采样、cleanup 来源与无条件迟到采样——由
`formal/tla/NativeCliWaitSettlement` 单独建模；见 `native-cli-wait-model.md`。已注册模型中
触发器覆盖 `apps/orchbee` 源码、manifest、project scope、README 与测试路径者仍须实际运行；
清单输出不能替代执行。
