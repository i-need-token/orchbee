# 终端输入自动化

| 字段     | 值                                                                                                                                                                                                                                                                                                                                                                                |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 评审中的提案。设计 `terminal send-text`、`terminal send-keys` 与 `terminal wait-output`——已接受信息架构中标注 "later explicitly authorized slice" 的三条命令——以及已接受但未实现的 first-cohort 命令 `terminal open`。在本切片被接受前，本文任何内容均未获准或实现；以下所有选项拼写均为提案                                                                                      |
| 负责人   | 产品命令体验，以及 Rust `orchbee` Client、Server 语义命令边界与终端投影数据面的维护者                                                                                                                                                                                                                                                                                             |
| 现行权威 | [`native-command-surface.md`](native-command-surface.md) 持有已接受的信息架构与 automation/input/mutation 契约；[`apps/orchbee/docs/native-cli-query.md`](../../apps/orchbee/docs/native-cli-query.zh-CN.md) 拥有本切片所扩展的已交付只读语法、envelope、失败矩阵与预算约定；[`schemas/client-server/README.md`](../../schemas/client-server/README.zh-CN.md) 拥有 OBCS wire 模型 |
| 父设计   | [`native-command-surface.md`](native-command-surface.md)——本提案仅拥有下述四条命令契约；不改变任何已接受决策                                                                                                                                                                                                                                                                      |
| 决策权威 | 切片负责人接受：四条语法、viewer/controller wire 映射、键名词汇及其客户端编码、有界观察模型、输出 schema，以及下述失败矩阵扩展。最终选项拼写、完整 `wait-output` 谓词集、agent 控制域归属与确切边界值仍为保留决策                                                                                                                                                                 |
| 退出条件 | 全部四条动词获得已接受的命令契约，并在已交付的 CLI 约定下以场景证据实现，之后本提案被消费进归属的实现记录                                                                                                                                                                                                                                                                         |

英文规范原文：
[terminal-input-automation.md](terminal-input-automation.md)。
如有冲突，以英文为准。

## 定位与范围

这四条命令是 Orchbee 终端自动化的 agent 可驱动语义面：相当于控制 socket API 在 terminal
multiplexer 上为 agent 提供的能力（发送输入、等待输出、挂接实时视图）在 CLI 层的对应物。它们
落在 Orchbee 自有模型上——本地同用户 Server 准入、Workbench 暴露的终端绑定、viewer 附件、单一
的合并 input-and-resize controller 授权，以及有界的拉取式 frame 数据面——而不是裸 PTY 字节或
第二条控制通道。agent 可以用 `terminal open` 或 `terminal wait-output` 观察终端，用
`terminal send-text` 或 `terminal send-keys` 操作终端，且全部处于与人工 TUI Client 相同的
准入、controller 仲裁、请求身份与 at-most-once 规则之下。

- `terminal send-text` 把调用方提供的文本作为键入输入写入终端，或作为一次语义粘贴。它需要
  该终端的 controller 授权。
- `terminal send-keys` 把封闭键名词汇编译为字节并作为键入输入写入。它需要 controller
  授权。
- `terminal wait-output` 执行有界观察：挂接一个只读 viewer，等待某个针对可见屏幕的谓词被满足
  或强制 deadline 到达。
- `terminal open` 挂接一个只读 viewer，并把每个送达的屏幕 frame 作为有界输出流式发出，直到
  流结束。

每条命令都是经已接受的 Server-relayed 路径的一次性 Client Session。没有任何命令要求 TTY、
直连 Terminal Host、把终端输出当作 Agent 或 Workbench 权威、创建或终止终端、resize，或跨调用
持有状态。

本切片的 non-goals：实现这些命令；Site 生命周期操作；remote targeting；Agent 启动或控制命令；
resize 动词（controller 授权已覆盖它，但本提案不提出 CLI 动词）；semantic-key wire 扩展；
scrollback 或字节流谓词；`--detach` 支持；close/kill 动词。

## 模型映射

全部四条命令复用已交付的查询状态机完成 cohort 校验、runtime discovery、socket 校验、OBLC
准入，以及一次有界 Workbench snapshot 读取。随后终端定向在同一 Client Session 上继续走 OBCS
viewer/control 契约：

| 阶段    | Wire 语义                                                                                                                                                                                                                                                       |
| ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 定向    | `<target>` 只接受完整的类型化终端 ID（恰好 32 个小写十六进制字符，非零），规则同 `terminal get`；不提供 label 匹配。解析把 snapshot 绑定映射为 `{siteId, workerId, terminalId}`；未绑定或不存在的终端是 `not-found`                                             |
| Attach  | `terminal.viewer` `attach-binding` `{siteId, workerId, terminalId}` → `attached` `{attachmentId, hostEpoch}`——一个由本 Session 持有的新只读 viewer                                                                                                              |
| Acquire | `terminal.control` `acquire` `{attachmentId}` → `acquired` `{controllerId}`——请求该终端唯一的合并 input-and-resize 授权。竞争返回 `rejected` `{reason:"controller-busy"}`；Server 没有等待队列，因此 CLI 报告拒绝而不是阻塞                                     |
| Input   | `terminal.control` `input` `{attachmentId, controllerId, sequence, bytes}`，1–4,096 个裸字节 → `input-accepted`；或一条 `paste` `OBCQ` 记录，1–262,144 字节 → `pasted` `{acceptedBytes}`。Acceptance 的含义是进入 Host 的有界输入队列，而不是应用已消费或执行它 |
| Observe | `terminal.viewer` `poll` `{attachmentId}` → `empty` 或一条二进制 `OBSR` 记录；客户端应用该记录后用 `frame-applied` `{sequence, generation, revision}` 确认；sequence 或解码分歧经 `need-snapshot` 恢复                                                          |
| Release | `terminal.viewer` `detach` `{attachmentId}` → `detached`——在 viewer 退役前释放 controller 授权。Session 丢失执行同样的清理，因此崩溃的命令绝不会持有授权或 viewer                                                                                               |

该映射的两条性质驱动了命令设计：

- 输入权威在 Server 侧以附件与 revision 为域、在 wire 上以授权为域：命令必须先 attach 一个
  viewer 才能 acquire controller，且每个 input 或 paste action 都同时命名两个身份外加每条
  授权上严格递增的正 `sequence`。
- frame 数据面对每个 viewer 有序且无损：Worker 每个 viewer 至多缓冲一条记录，若第一条尚未被
  消费就来第二条则关闭附件；而 `poll` 在无记录就绪或有未决 feedback 时立即返回 `empty`。因此
  只要 CLI 观察者对每个 frame 都做了确认，它就能看到每个送达的 revision；等待则以有界的
  客户端间隔轮询——不存在 server 端长轮询。

## 身份、sequence 与 at-most-once 提交

- 每个 wire 请求携带新的 `requestId`——由新鲜随机字节生成的 32 个小写十六进制字符——生成与
  关联方式同已交付命令完全一致。
- 每条新 acquire 的授权上 control `sequence` 从 `1` 开始，随每个 `input` 或 `paste` action
  严格递增。Input action 串行提交：每个 action 只在前一个的结果返回后才提交，因此最后一个
  action 的 acceptance 即证明整个已提交前缀。
- 任何请求绝不重新提交。唯一获准的重试是 wire 的 `server-busy` 有界重试——针对已知未产生任何
  效果的请求，action 值不变、使用新的 `requestId`、处于同一逻辑 deadline 内——可用于
  `attach-binding`、`acquire`、`poll`、`frame-applied`、`need-snapshot` 与 `detach`，绝不用于
  `input`、`resize` 或 `paste`。确定性拒绝不授予任何重试。
- 歧义结果——wire 的 `outcome-unknown`，或写入、读取、deadline、关联或结果形态失败后的
  `SubmissionError::OutcomeUnknown`——使 Session 退役。命令停止，以 `outcome-unknown` 报告并
  携带受影响的 `requestId`，绝不把该效果作为新请求重试。这些命令所走的
  `terminal.viewer`/`terminal.control` 请求族不保留请求账本：`requestId` 只把存活请求与其
  结果关联，并非 Server 账本身份。（Server 确实为 `workbench.command` 保留有界账本——相同的
  已结算重提交会重放原结果，同一 `requestId` 下的不同命令按身份冲突拒绝——但它不覆盖该请求
  族。）因此 reconcile 是观察式的：操作者通过 `terminal read`、`terminal wait-output` 或
  `terminal open` 核实终端，再决定是否重新提交意图。
- `--detach` 不出现在本切片任何命令上：这些操作都不拥有可供事后观察的持久 Server 侧身份，且
  已接受契约禁止把终端输入变成 fire-and-forget 效果。

## 语法增量

命令路径在不改变已交付语法的前提下扩展——选项扫描、`--` 终止、`--format text|json`、`--help`
后缀与 `help` 路径的行为与今天完全一致；completion 只投影新动词与 flag，绝不枚举内容或运行时
状态：

```
orchbee terminal send-text <target> (--file <path> | --stdin) [--paste]
orchbee terminal send-keys <target> <key> [<key>...]
orchbee terminal wait-output <target> (--text <literal> | --regex <pattern> | --silence <duration>)... [--timeout <duration>]
orchbee terminal open <target> [--timeout <duration>] [--frames <count>]
```

- `<target>` 是终端的类型化 ID；只有 Workbench snapshot 中可见的 presentation-bound 终端
  可达，与 `terminal list`/`terminal get` 一致。
- `<duration>` 是正整数加可选 `ms`/`s`/`m` 单位后缀（裸数字 = 毫秒）。确切拼写与取值范围为
  保留项；下方给出提案值。
- `wait-output` 上的谓词 flag 可重复且以 OR 组合；任一谓词满足即结束等待。
- 本切片没有命令接受位置参数内容，因此 `send-text` 内容在构造上不可能出现在 `argv` 中。

## `terminal send-text`

`orchbee terminal send-text <target> (--file <path> | --stdin) [--paste]` 把调用方提供的文本
如同键入般写入终端输入队列——或在 `--paste` 下作为一次语义粘贴，由 Host 按终端当前
bracketed-paste 模式编码。

输入来源与边界：

- `--file <path>` 与 `--stdin` 恰需其一；同给或都不给是 `invalid-usage`。刻意不提供内联
  文本选项：内容绝不出现在 `argv`、进程列表、人类输出、诊断或 shell completion 中。
- 内容必须是非空合法 UTF-8 且至多 262,144 字节（256 KiB，与 wire 上最大的单次输入边界对齐）。
  空、超限、非 UTF-8、不可读或不存在的来源在任何运行时访问前即为 `invalid-usage`。
- `--stdin` 仅在被显式选择时完整读取 stdin；绝不隐式读取 stdin 或文件输入。

送达映射：

- 默认（`input` 模式）：字节内容被切分为每条至多 4,096 字节的 `terminal.control` `input`
  action——切分点可落在字节流任意位置，因为 Host 按字节精确拼接队列——`sequence` 为 `1..N`。
  成功意味着每个分块都返回 `input-accepted`。
- `--paste`：全部内容作为一条 `paste` `OBCQ` 记录提交；`pasted` `{acceptedBytes}` 报告
  Host 准入的编码字节数。对多行或敏感内容，paste 是更安全的送达方式，因为支持
  bracketed-paste 的应用把它作为一次粘贴事件接收，而不是可执行的按键序列。
- 命令先 attach 再 acquire；若另一 Client 持有 controller，命令以 `controller-busy` 失败——
  这是拒绝，绝不是等待。

结果 `orchbee.terminal.send-text`：
`{terminal:{terminalId,workerId},sent:{mode:"input"|"paste",bytes:<n>,requests:<n>,requestId:"<hex32>"}}`——
`requestId` 是最后一个已确认效果请求的身份，按有序 sequence 规则，它的 acceptance 即证明已
提交前缀；`requests` 计效果 action 数；`bytes` 计已提交来源字节数。人类文本只报告字节数与
终端身份；绝不回显内容。

与 noninteractive/secret 规则的一致性：内容只经显式选择的有界来源进入；输出只报告计数与
身份，绝不报告字节；中途失败的部分发送报告实际已确认的 `bytes`；歧义效果以 `outcome-unknown`
报告并携带该请求的身份以供 reconcile。

## `terminal send-keys`

`orchbee terminal send-keys <target> <key> [<key>...]` 把每个 key token 编译为字节编码，并把
拼接后的字节流作为键入 `input` action 写入，与 `send-text` 一样按 4,096 字节切分。Key token
来自封闭词汇表；不认识的 token 是 `invalid-usage`——不提供字面文本兜底，因此拼写错误不会注入
意外文本，字面内容也保持唯一的 secret 安全归宿（`send-text`）。

提案词汇与编码（最终拼写为保留项）：

| Token                                               | 字节                                                                                                                                                                                                                                            |
| --------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `C-<c>`                                             | 控制字节：`C-a`…`C-z` → `0x01`…`0x1a`（大小写不敏感）；`C-@`/`C-Space` → `0x00`；`C-[` → `0x1b`；`C-\` → `0x1c`；`C-]` → `0x1d`；`C-^` → `0x1e`；`C-_` → `0x1f`；`C-?` → `0x7f`                                                                 |
| `M-<key>`                                           | `0x1b` 加 `<key>` 的编码（meta/alt）                                                                                                                                                                                                            |
| `C-M-<c>`                                           | `0x1b` 加 `C-<c>` 控制字节                                                                                                                                                                                                                      |
| `Enter`, `Escape`, `Tab`, `Space`, `BSpace`, `BTab` | `0x0d`；`0x1b`；`0x09`；`0x20`；`0x7f`；`0x1b 0x5b 0x5a`（shift-tab）                                                                                                                                                                           |
| `Up`, `Down`, `Right`, `Left`                       | `0x1b 0x5b 0x41`/`0x42`/`0x43`/`0x44`（CSI normal cursor 模式）                                                                                                                                                                                 |
| `Home`, `End`                                       | `0x1b 0x5b 0x48`/`0x46`                                                                                                                                                                                                                         |
| `Insert`, `Delete`, `PageUp`, `PageDown`            | `0x1b 0x5b 0x32 0x7e`；`0x1b 0x5b 0x33 0x7e`；`0x1b 0x5b 0x35 0x7e`；`0x1b 0x5b 0x36 0x7e`                                                                                                                                                      |
| `F1`–`F4`                                           | `0x1b 0x4f 0x50`/`0x51`/`0x52`/`0x53`（SS3）                                                                                                                                                                                                    |
| `F5`–`F12`                                          | `0x1b 0x5b <n> 0x7e`，`<n>` = `15`,`17`,`18`,`19`,`20`,`21`,`23`,`24`                                                                                                                                                                           |
| `C-`/`M-`/`S-` + 具名特殊键                         | xterm 修饰形式：方向键/Home/End → `0x1b 0x5b 0x31 0x3b <m> <final>`；`Insert`/`Delete`/`PageUp`/`PageDown`/`F5`–`F12` → `0x1b 0x5b <n> 0x3b <m> 0x7e`；`F1`–`F4` → `0x1b 0x5b 0x31 0x3b <m> P/Q/R/S`，`<m>` = `1 + shift(1) + alt(2) + ctrl(4)` |

与现有输入编码的关系：Host 的语义 `KeyInput` 路径（W3C 风格键码、经 Ghostty VT 的
mode-aware 编码）在 OBCS version 1 下不可达——`terminal.control` `input` 只携带裸字节。因此
CLI 在客户端把键名编译为真实按键在终端默认模式下产生的同一 normal-mode 序列。这对 shell 与
面向行的工具是确定性且足够的，但无法表达 mode-dependent 编码（application cursor keys、
kitty keyboard protocol、`modifyOtherKeys`）：处于这类模式的终端应用看到的字节与物理按键
不同。未来携带语义键的 wire 扩展是保留决策，不属于本切片。

结果 `orchbee.terminal.send-keys`：
`{terminal:{terminalId,workerId},sent:{keys:<n>,bytes:<n>,requests:<n>,requestId:"<hex32>"}}`。
键名按设计是 `argv` 中的命令指定符——像 flag 名一样出现在调用中且可被补全；它们不是内容。
任何可能携带 secret 的内容归属 `send-text`，其设计在构造上不接受 `argv` 内容。

## `terminal wait-output`

`orchbee terminal wait-output <target> <predicate>... [--timeout <duration>]` 挂接一个只读
viewer 并等待，直到某个谓词被满足或 deadline 到达。它绝不 acquire controller，也不参与竞争。

谓词（至少一个必需；组合为 OR）：

| Flag                   | 提案边界                | 语义                                                                                                                         |
| ---------------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `--text <literal>`     | 1–4,096 UTF-8 字节      | 可见屏幕文本包含该字面子串时满足                                                                                             |
| `--regex <pattern>`    | 1–4,096 字节且可编译    | 模式在屏幕文本任意位置匹配时满足；Rust `regex` 线性时间语法（无 backreference/lookaround）；无法编译的模式是 `invalid-usage` |
| `--silence <duration>` | 1–600,000 ms            | 自上一个已应用 frame（或自 attach）起 `<duration>` 内无新应用 frame 时满足                                                   |
| `--timeout <duration>` | 1–600,000 ms，默认 30 s | 对观察阶段的强制总边界；到达时仍无谓词满足则结束等待                                                                         |

求值平面：谓词在可见屏幕文本上求值——frame 可见行拼接成的文本，即 `terminal read` 所暴露的
同一投影平面——在每个已应用 frame 后求值，包括 attach 后的首个快照。本命令刻意不承诺字节流或
scrollback 语义：在一个送达 frame revision 内出现又滚出屏幕的内容在本切片不可观察；流域
谓词为保留项。

为何这是有界观察而非无限轮询：

- deadline 是强制的：`--timeout` 始终界定观察（带提案默认值与硬上限），任何调用都不可能
  永远等待。
- 一个 Session、一次 snapshot 读取、一个附件覆盖整个等待——它不是循环重跑 `terminal read`；
  那样得到的快照是相互独立的采样，可能错过中间屏幕 revision，且每次都要重建 Session 与
  附件。
- 投递对每个 viewer 有序且无损：CLI 在下一条记录产生前用 `frame-applied` 确认每条记录，
  因此 Host 发出的每个 revision 都会被求值——采样不可能跳过它。
- 恢复走既有语义路径（sequence 或解码分歧时 `need-snapshot`），而不是重新 attach。
- 轮询间隔有固定下限（提案 25 ms）且在 deadline 内运行；报告 `blocked`
  （`capture-allocation-failed`/`projection-too-large`）的记录对 `--silence` 计为活动，
  但不携带屏幕内容，绝不参与谓词求值。

结果 `orchbee.terminal.wait`：

- 满足，exit 0：
  `{terminal:{terminalId,workerId},wait:{matched:true,predicate:{kind:"text"|"regex",value:"<pattern>"}|{kind:"silence",durationMs:<n>},revision:"<u64>",sequence:"<u64>",matchedText:"<line>",elapsedMs:<n>}}`——
  `matchedText` 是首个匹配的屏幕行，有界 4,096 字节；`revision` 与 `sequence` 标识观察到
  匹配的 frame。
- deadline 到达，exit 1：
  `{terminal:{terminalId,workerId},wait:{matched:false,reason:"deadline",elapsedMs:<n>}}`——
  以 success schema 承载的确定性否定结果（同 `config check` 报告 `ok:false` 的模式），
  不是 failure envelope。脚本按 exit code 分支；agent 读取类型化的 `matched`/`reason`
  字段。
- 等待中途消失的终端使等待不可满足：按 wire outcome 报 `not-found`
  `{resource:"terminal",target}` 或 `service-unavailable`，而不是 `matched:false` 结果。

`matchedText` 是调用方已被授权读取的终端输出（与 `terminal read` 同一权威）；它有界且只
出现在结果文档中，绝不出现在诊断中。

## `terminal open`

`orchbee terminal open <target> [--timeout <duration>] [--frames <count>]` 挂接一个只读
viewer 并把每个送达的屏幕 frame 流式发出，直到流结束。"open" 是已接受的、在终端上挂接
视图的产品动词；本命令是 viewer 附件加流式输出，不是 wire 的 `terminal.open` 操作——OBCS
`terminal.open` 请求是 bare-TUI bootstrap 使用的 Server-owned standalone-terminal
create-or-rejoin 流程，与本命令无关。本命令绝不 acquire controller，也绝不发送输入。

流边界：

- `--format json` 发出 NDJSON：每行一个 `{schema:"orchbee.terminal.open",version:1,source,event}`
  对象，`source` 是用于定向解析的 `{kind:"workbench-snapshot",siteId,revision}`。事件按流
  序：
  - `attached`——`{kind:"attached",terminal:{terminalId,workerId},attachmentId,hostEpoch}`，
    恰好一次且在最前。
  - `frame`——`{kind:"frame",sequence:"<u64>",generation:"<u64>",revision:"<u64>",cursor:{column:<n>,row:<n>},rows:[{index:<n>,text:"<row>"}],truncated:<bool>}`——
    该 revision 的完整可见屏幕（每个事件自包含；消费者无需维护状态），随每条已应用记录
    发出。每个事件发出的 `rows` 文本上限 256 KiB；溢出丢弃尾部行并置 `truncated:true`。
  - `blocked`——`{kind:"blocked",reason:"capture-allocation-failed"|"projection-too-large",revision:"<u64>"}`，
    投影无法产出 frame 时发出；随后以新快照恢复投递。
  - `ended`——`{kind:"ended",reason:"detached"|"terminal-unavailable"|"timeout"}`，恰好一次
    且在最后，然后 exit 0。被观察的终端消失时 `terminal-unavailable` 干净地结束流；reason
    字段报告流的结束方式。
- `--format text` 在每个已应用 frame 后打印完整可见屏幕文本，以单独一行 form-feed（`\f`）
  分隔，受同样的单记录文本上限约束。它是屏幕状态数据面的忠实呈现——本面不提供字节流——
  且人类输出绝不是自动化 API。
- `--timeout <duration>` 界定总流时长；到达时发出 `ended` `timeout` 并 exit 0。
  `--frames <count>` 界定 `frame` 事件数；到达时发出 `ended` `detached`。两者均可选；
  都不给时，流持续到终端 detach 或调用方中断（`SIGINT` → exit 130，不保证最后事件）。

流在时长上无界，但每个发出事件都有字节边界，且传输本身有界——一个 Session、一个附件、
逐条确认的有序无损记录。想按内容停止的命令应使用 `wait-output`。

## 输出契约

- 机器数据只在 stdout；本地化诊断只在 stderr；text 为默认，`--format json` 选择上述版本化
  机器文档。`open` 的机器文档是 NDJSON，因为该操作是流；其余命令发出一份 pretty JSON 文档。
- 成功 envelope 保持已交付的 `{schema,version:1,source,result}` 形态，`source:{kind:"workbench-snapshot",siteId,revision}`
  来自定向读取。`source` 描述的是定向快照，而不是生效时刻状态。
- 新 schema ID：`orchbee.terminal.send-text`、`orchbee.terminal.send-keys`、
  `orchbee.terminal.wait`、`orchbee.terminal.open`（NDJSON 事件 envelope）。
  `orchbee.cli.failure` 形态不变；仅按下表扩充 `kind`/`details` 词汇。
- 一次性文档在既有有界缓冲中制备，保持 8 MiB `output-too-large` 上限。`open` 事件逐条
  有界（256 KiB 发出文本），不受文档上限约束，因为流是增量的。
- Secret 分类：`send-text` 内容是终端输入负载——绝不出现在 `argv`、stdout、stderr、诊断
  或 completion 中；只报告计数与身份。`send-keys` token 按设计是 `argv` 中的命令指定符，
  如同 flag；封闭词汇表阻止内容夹带。`wait-output` 谓词是调用方在 `argv` 中选的选择器，
  如同 `grep` 模式；`matchedText` 与 `open` frame 文本是调用方既有读取权威下的终端输出。

## 失败矩阵

Exit code 沿用已交付表：`0` 成功；`1` 本地确定性结果（输入来源、内部、输出失败，以及
`wait-output` deadline 未命中结果）；`2` usage/无效 target；`3` 拒绝；`4` 服务不可用；
`5` transport loss/协议无效；`6` 不兼容；`130` `SIGINT`。`7` 继续为 failed-effect 类别
保留（本切片无生产者）；`8` 分配给 unknown-outcome 类别，即 `outcome-unknown`。

新增或扩展词汇：

| Kind                  | Exit | Details                        | 产生者                                                                                                                                                                                                         |
| --------------------- | ---- | ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `controller-busy`     | 3    | `{resource:"terminal",target}` | `acquire` → `rejected{controller-busy}`：另一 Client 持有合并 input/resize 授权。确定性拒绝——Server 没有任何队列                                                                                               |
| `denied`              | 3    | 增加 `{reason}`                | 其他 `rejected` outcome：`not-controller`、`host-rejected`、`host-input-invalid`、`host-input-limit-exceeded`、`host-input-backpressure`、`host-input-unavailable`、`feedback-mismatch`——`reason` 携带 wire 值 |
| `outcome-unknown`     | 8    | `{stage,requestId,sentBytes?}` | wire `outcome-unknown` 或 `SubmissionError::OutcomeUnknown`；输入序列中途死亡时 `sentBytes` 报告已确认输入字节                                                                                                 |
| `not-found`           | 3    | `{resource:"terminal",target}` | 目标在 snapshot 中不可解析、`attach` → `rejected{terminal-unavailable}`，或操作中途 `rejected{viewer-unavailable}`                                                                                             |
| `service-unavailable` | 4    | `{stage,reason}`               | 有界重试耗尽后，任意阶段的 `unavailable{server-busy\|terminal-runtime-unavailable\|workbench-unavailable}`                                                                                                     |
| `internal-failure`    | 1    | 省略                           | `rejected{sequence-mismatch}` 或 `already-attached`（若 sequence/attachment 跟踪正确则不可达）、编码错误、deadline 构造失败                                                                                    |
| `transport-loss`      | 5    | `{stage}`                      | `RequestNotSubmitted`——可证明从未到达 transport 的请求不携带歧义效果                                                                                                                                           |

阶段名在已交付集合（`cohort`、`runtime-base`、`runtime-directory`、`socket`、`discovery`、
`admission`、`snapshot`、`platform`）上扩展 `attach`、`acquire`、`input`、`observe` 与
`stream`。既有 kind——`invalid-usage`、`invalid-identifier`、`ambiguous`、`check-failed`、
`connection-capacity`、`protocol-invalid`、`incompatible`、`output-too-large`、`output-failed`——
不变适用。`ambiguous` 在本切片无生产者，因为终端 target 只接受类型化 ID。

## Session 状态机与预算

每条命令执行已交付的有序阶段——cohort（5 s checked deadline）、discovery、socket、
admission——然后在 admission deadline 下恰好一次 snapshot 读取，再在同一 Session 上进入
命令阶段：

- `send-text` / `send-keys`：`attach-binding` → `acquire` → 效果 action → `detach`，全部
  处于一个 checked 效果阶段 deadline 内（提案 15 s；256 KiB 分块发送至多 64 个串行
  本地 socket action）。成功时尝试一次 `detach`；任何失败时丢弃 Session，执行同样的清理。
- `wait-output`：`attach-binding` → 观察循环（`poll` → 应用 → 求值 → `frame-applied`）
  → `detach`，由覆盖整个阶段的 `--timeout` deadline 界定。
- `open`：`attach-binding` → 流循环 → `detach`，时长无界，但每个请求各自受标准请求
  deadline 约束。

语法上无效的调用——语法、format、target 形态、输入来源缺失或重复、超限或非 UTF-8 内容、
无法编译的 regex——在任何文件系统或网络访问前失败，与已交付命令完全一致。

## 不变量

- `send-text` 内容绝不出现在 `argv`、stdout、stderr、诊断或 completion 中；只报告计数与
  身份。绝不隐式读取 stdin 或文件输入；恰需一个显式来源 flag。
- 每条命令在其生命周期内至多持有一个 Client Session、一次 snapshot 读取、一个 viewer
  附件与一个 controller 授权；`detach` 为尽力而为，Session 丢失执行等价清理。
- `input`/`paste` action 每个至多提交一次、串行、处于严格递增 sequence 下；歧义 outcome
  停止命令并以 `outcome-unknown` 报告且携带请求身份；该效果绝不作为新请求重试。
- Controller 竞争是 `controller-busy` 拒绝，绝不是排队或等待；本切片没有任何等待授权的
  行为。
- `wait-output` 始终在强制 deadline 下运行，对每个送达 revision 求值，并把 `blocked`
  记录当作无屏幕内容的活动。
- `terminal open` 与 `terminal wait-output` 绝不 acquire controller；`send-*` 绝不发出
  终端内容。
- `--detach` 在本切片每条语法中均缺席。
- CLI 向 Server 提交语义意图并校验类型化结果；绝不直连 Terminal Host、绝不把裸 PTY 字节
  当作权威解析、绝不把终端输出当作 Agent 或 Workbench 事实。

## 保留决策与开放问题

为归属阶段保留：最终选项拼写与取值范围；完整 `wait-output` 谓词集（包括任何流域或
scrollback 谓词）；agent 控制域归属与任何独立的 agent 域策略；确切边界值（字节上限、
轮询间隔、默认/上限 timeout、单事件文本上限）；未来 wire 扩展是否携带语义 `KeyInput`；
以及任何 `--frames`/`--timeout` 拼写变更。

本提案已定决策：controller 竞争时拒绝而非排队；客户端把键名编译为 normal-mode 字节；
可见屏幕谓词平面；携带自包含屏幕行的 NDJSON `open` 事件；deadline 未命中作为 exit 1 的
确定性 `matched:false` 结果；exit 8 的 `outcome-unknown` 携带请求身份；无 `--detach`。

## 场景矩阵与证据

| 场景                                                                                                    | 期望                                                            | 证据                                                  |
| ------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- | ----------------------------------------------------- |
| 语法：输入来源互斥、`--`、format、help 形态、谓词要求、未知 key token                                   | 在任何运行时访问前的精确 `invalid-usage`/类型化 ID 失败         | `src/invocation` 测试 + `tests/cli_query.rs` 风格集成 |
| 键名词汇：每个名称/修饰组合                                                                             | 黄金字节编码向量；未知 token 被拒绝                             | key-table 单元测试                                    |
| 切分：0/1/4,096/4,097/262,144/262,145 字节来源、UTF-8 边界切分                                          | 正确分块数、边界处 `invalid-usage`、字节精确流                  | send-path 单元测试                                    |
| 脚本化 OBCS peer：acquire `controller-busy`；input 拒绝；`unavailable` 原因；发送中途 `outcome-unknown` | 按矩阵的类型化 kind；效果绝不重试；报告 `requestId`             | 脚本化 peer 集成测试                                  |
| Viewer 生命周期：`attach-binding` 拒绝/`unavailable` 优先级、poll/`frame-applied`、`need-snapshot`      | 类型化映射；单一附件；有序确认                                  | 脚本化 peer 集成测试                                  |
| `wait-output`：首个快照即匹配、流中途匹配、`--silence`、deadline、`blocked` 记录、终端丢失              | `matched`/`deadline` 语义；每个 revision 被求值；类型化结束状态 | 带构造 frame 序列的脚本化 peer 测试                   |
| `open`：NDJSON 事件顺序、自包含行、截断标记、`ended` 原因、`--timeout`/`--frames`                       | 事件语法与边界在构造流下成立                                    | 脚本化 peer + 黄金 NDJSON 测试                        |
| Secret 不泄漏：内容字节在每种输出/诊断/completion 面上缺席                                              | 两种 locale 下对 stdout/stderr/completion 的扫描断言            | 输出测试                                              |
| 非 TTY 执行、等待/流中途 `SIGINT`、stdout 写失败                                                        | 文档化退出；阻塞前丢弃 Session                                  | 集成测试                                              |
| 真实 staged-cohort Server：在持有 controller 的 TUI 旁对活终端执行 send-*/wait/open                     | `controller-busy` 拒绝；释放后成功路径；无泄漏授权              | `tests/local_startup_process`                         |

模型义务：本切片不新增 Server 状态机、持久化或重试——它组合已注册的 OBCS viewer/control
模型与一个客户端有界观察循环——因此不需要新的 TLA+ 模型。触发范围覆盖 `apps/orchbee` 源与
测试路径的已注册模型仍必须实际运行；inventory 输出不能替代运行。

<!-- documentation-pairing: english-sha256=2918ea125a62b8696adc50cc5a3be531bbfe5c52235f5413c2ea4da1f4419d18 zh-cn-sha256=6fdd03902769a5d6efe8f6c1088a2c21573f691165ea5b6637aeb343e4eee1ef -->
