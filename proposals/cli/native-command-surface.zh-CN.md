# 原生 Orchbee 命令面

| 字段     | 值                                                                                                                                                                                                                                                                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 状态     | 替换方向以及目标 TUI、裸激活、首版 CLI 信息架构、结构化输出、mutation 安全、打包和 service lifecycle contract 已接受；增量 Phase 3A 原生入口底座与已检查的 Client-side 本机 activation model 已完成，而 runtime implementation、确切 wire/schema 细节、remote targeting、public release cutover 仍是后续工作；旧源代码退役已按独立授权完成 |
| 负责人   | 产品命令体验，以及 Rust `orchbee` Client、Server 语义命令边界、TUI、打包与发行集成的维护者                                                                                                                                                                                                                                                 |
| 当前权威 | [`apps/orchbee/README.md`](../../apps/orchbee/README.zh-CN.md) 负责已交付原生 CLI/TUI 行为；TypeScript entry 已退役。                                                                                                                                                                                                                      |
| 父设计   | [`native-runtime-topology.md`](../platform/native-runtime-topology.zh-CN.md) 拥有可执行程序拓扑和进程职责；本提案拥有公开 Rust 命令面                                                                                                                                                                                                      |
| 决策权威 | 产品决策人已接受一个公共 Rust `orchbee` 同时承载 CLI 与 TUI；同级 Client 的 TUI 模型与首版全屏交互 contract；下文 resource-first command family；human 与 versioned machine output；有界输入与 at-most-once mutation 规则；一个公共打包入口；以及已接受的裸本机激活和用户 service 界面                                                     |
| 退出条件 | 交付剩余已接受的原生命令 slice 与发布边界，然后退役本提案。旧源代码删除独立于新打包获得授权。                                                                                                                                                                                                                                              |

英文规范原文：
[native-command-surface.md](native-command-surface.md)。
如有冲突，以英文为准。

## 已接受的产品边界

- `orchbee` 是唯一公开的原生可执行程序名称。Server、Worker、Terminal Host 与 Agent Bridge
  仍是私有实现进程。
- 裸 `orchbee` 选择交互式 TUI。显式 `orchbee tui` 选择同一个界面；显式 resource command、
  会话 lifecycle command、help、version 与 completion 选择一次性行为，不初始化 TUI。
- 本机 会话 不存在时，裸 `orchbee` 可以发起确切 compatible installed Server 与本机 Worker cohort
  的有界懒激活并等待 ready。Ready 后由私有进程拥有各自生命周期；`orchbee` 绝不继续监督它们。
- 每次 TUI invocation 都是一个独立 Client Session，与 Web tab、Desktop window 或 mobile Client
  使用同一模型。它拥有本地 focus、viewport、scroll 与 presentation state，但不会因为发起了
  activation 就获得共享 会话 或 terminal authority。
- `--help` 与 `--version` 保持无副作用，绝不发现或激活私有 service。
- 一次性 CLI 执行不会初始化 TUI 渲染，也不会把 Server、Worker、Terminal Host 或 Agent
  engine 加载进公共进程。
- Rust 命令面是新的 Client 实现。它不继承当前 TypeScript parser、command name、flag、
  output schema、exit-code table、locator protocol、`ORCHBEE_CLI` variable、embedded Skill
  layout 或 Electron-as-Node launcher contract。
- 现有 semantic capability 是迁移输入，不是自动兼容要求。只有所属 domain 显式准入新的
  command contract 后，每项 capability 才会出现在新 CLI 中。
- 一项只读 CLI operation 可以向 Server 请求有界且版本化的 Workbench semantic projection
  snapshot。该 snapshot 是结构化产品数据，不是 HTML、React tree、pixel，也不包含另一 Client
  的本地 focus、viewport、selection 或 window state；Web、TUI 与 mobile 基于共享 contract
  各自渲染界面。

本次接受只改变未来设计。现有 TypeScript app 与当前指南在单独接受的替换切片切换已交付
入口前继续保持权威。本提案本身不会删除任何源码或 package。

## 已接受的裸本机交互路径

[`native-runtime-topology.md`](../platform/native-runtime-topology.zh-CN.md) 中的本机 terminal milestone
已经接受裸 invocation 的可观察行为。本提案负责其面向 command 的边界：

| 关注点      | 已接受行为                                                                                                                                                                                               |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation  | 空 argument vector 选择交互式 TUI 路径。用户不需要另一个 public executable 或 hidden mode。                                                                                                              |
| 终端设备    | 裸 TUI 与显式 TUI 路径要求 interactive TTY，绝不隐式打开 `/dev/tty`。Help、version 与 machine-readable 一次性输出保持可管道传递。                                                                        |
| 已有 会话   | 连接确切 compatible local Server 并创建新的 Client Session；再次 invocation 会创建另一个同级 session，而不是复用本地 view state。                                                                        |
| 会话 不存在 | 在一个 per-user activation lock 下发起确切 installed private Server 与本机 Worker cohort，让并发 invocation 收敛到该 singleton，并只在 typed ready 后进入 TUI。Terminal Host 在首次使用终端前保持 lazy。 |
| 生命周期    | Activation 是 bootstrap effect，不是 parent-child supervision。关闭 TUI 只释放自己的 Client Session 与 attachment；不会停止 会话、service 或 retained terminal。                                         |
| 边界        | 公共进程既不内嵌 backend engine，也不直连 Terminal Host。它通过已接受且经 Server relay 的 Client path 获得 terminal behavior。                                                                           |
| 反馈        | 快速成功的 bootstrap 保持静默；延迟时显示本地化进度。一个 monotonic 总 deadline 以 typed unavailable 或 incompatible diagnostic 结束；`orchbee` 绝不为恢复而杀死、替换或降级现有 cohort。                |
| 延后细节    | 确切 timeout 和进度阈值、remote targeting、详细 command schema、public release cutover 继续保留。旧源代码退役已获独立接受。                                                                              |

已检查 model 及其 maintained bound 现由
[`apps/orchbee` 指南](../../apps/orchbee/README.zh-CN.md#已接受的本机-session-activation-建模记录)
维护。它只提供 design evidence；当前 bare route 仍未接线且不可用。

## 已接受的 TUI Client 契约

TUI 是与 Web、Desktop 和 mobile 同一架构层级的 Workbench Client。它不是全局 terminal picker，
也不会引入第二套 Workbench 模型。

| 关注点            | 已接受行为                                                                                                                                                                                                               |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 共享 document     | Server-owned Workbench state 包含 Project、Workspace、terminal context、Tab、Panel 与 Presentation 的 membership 和 ordering。任一 Client 发起并获接受的 mutation 都发布给所有已授权 Client。                            |
| Client view       | 每次 TUI invocation 拥有自己的 active location、active Tab、focus、layout projection、viewport、scroll、search 与 selection presentation。再打开一个 TUI 会像再打开一个 Browser tab 一样创建独立 Client View。           |
| 初始位置          | 裸 TUI 默认进入 standalone terminal context 及其第一个有序 Tab。打开空 context 时，可以通过一次由 Server 协调的 semantic operation 恰好创建一次首个 terminal；启动时不显示 terminal-picker modal。                       |
| 空状态            | 关闭最后一个 Tab 后保留显式空 context；不会关闭 TUI、停止 会话，或终止无关 retained terminal。                                                                                                                           |
| Attachment        | 只有 visible active terminal 需要 transient display attachment。切换会释放旧 attachment 并打开新 attachment；两者都不会创建、克隆、重启或终止 retained terminal。产品文案使用“打开”或“连接”，`attachment` 仅是工程术语。 |
| Controller        | 多个 attachment 可以查看同一个 terminal；同一时间只有一个组合 input-and-resize controller。Attach 只在无人拥有时取得 controller；否则 Client 显式进入 view-only。Release 不会自动晋升另一 viewer，也不会抢占 control。   |
| 首次创建 geometry | 全屏 TUI 在提交 immediate Create 前取得一份完整的真实 cell 和 pixel geometry，绝不伪造 80x24。Deferred Create 加 Start 仍供尚不知道首个真实 geometry 的其他 flow 使用。                                                  |

首版 TUI 使用全屏 alternate screen。宽布局左侧是 Orchbee 自有业务 sidebar，右侧是 Workbench tabs、
panel tree 与真实 terminal；窄布局把 sidebar 收进 overlay。首版 Rust 技术栈选用 Ratatui 加
Crossterm。默认 prefix 是 `Ctrl+B`，进入 prefix mode 时显示可见提示，首版映射如下：

| 按键                   | Action                        |
| ---------------------- | ----------------------------- |
| Prefix 后按 `c`        | 创建 Tab                      |
| Prefix 后按 `n` / `p`  | 选择下一个 / 上一个 Tab       |
| Prefix 后按 `1` … `9`  | 选择对应的有序 Tab            |
| Prefix 后按 `h/j/k/l`  | 在 Panel 之间移动             |
| Prefix 后按 `b`        | 切换业务 sidebar              |
| Prefix 后按 `w`        | 打开 Workbench navigator      |
| Prefix 后按 `?`        | 打开快捷键帮助                |
| Prefix 后按 `q` 或 `d` | Detach 当前 TUI Client        |
| Prefix 后按 `Ctrl+B`   | 向 terminal 发送字面 `Ctrl+B` |

不在 prefix mode 时，`Ctrl+C`、`Ctrl+D` 与普通 terminal input 直接传给 active terminal。
Orchbee 拥有自己的信息架构与视觉设计。

## 已接受的 CLI 信息架构

Command 使用显式 resource-first noun/verb 词汇，不支持 fuzzy abbreviation，也不为私有 executable
名称提供 public alias。已接受的目标 command family 如下：

| Family         | 已接受 command                                                                                                                               | 交付边界                                                                                                                                                                                                                                                                               |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 入口与工具     | 裸 `orchbee`、`tui`、`status`、`completion`、`config check`、`api schema`、`api snapshot`、`--help` 与 `--version`                           | 入口、基础工具、schema discovery 与有界只读 snapshot 进入首个 public command cohort。                                                                                                                                                                                                  |
| Project        | `project list`、`project get`                                                                                                                | 首个 cohort 只读。                                                                                                                                                                                                                                                                     |
| Workspace      | `workspace list`、`workspace get`、`workspace create`、`workspace focus`、`workspace rename`、`workspace close`                              | `list` 与 `get` 优先；mutation 仅在其 Server state machine 获准后加入。                                                                                                                                                                                                                |
| Tab            | `tab list`、`tab get`、`tab create`、`tab focus`、`tab rename`、`tab close`                                                                  | `list` 与 `get` 优先；mutation 仅在其 Server state machine 获准后加入。                                                                                                                                                                                                                |
| Panel          | `panel list`、`panel get`、`panel focus`、`panel split`、`panel resize`、`panel move`、`panel close`                                         | `list` 与 `get` 优先；mutation 仅在其 Server state machine 获准后加入。                                                                                                                                                                                                                |
| Terminal       | `terminal list`、`terminal get`、`terminal read`、`terminal open`；后续 `terminal send-text`、`terminal send-keys` 与 `terminal wait-output` | 首批只做只读与 `open`；active input automation 仍是后续单独授权切片。                                                                                                                                                                                                                  |
| 会话 lifecycle | `session enable`、`server stop` 与 `session restart`                                                                                         | 后续 lifecycle slice；不存在 public `orchbee server` command。**已部分取代：**公共 `server` 命令族现负责管理选定会话的后台服务（见 [Client 命令面](../../apps/orchbee/README.zh-CN.md#命令面)）；[术语表](../../docs/zh-CN/GLOSSARY.md#运行时角色)保留该命令族名称，不改为 `session`。 |
| 后续 domain    | Agent、managed Worktree、Archive、Launch Profile 与 integration command                                                                      | 只有所属 domain 的已接受 contract 才能添加。                                                                                                                                                                                                                                           |

`Tab`、`Panel` 与 `Terminal` 保持不同的产品 noun。不存在 monolithic `--no-session` mode，也不围绕
旧的 raw `session` 术语构建兼容语法。Root help 引导 Agent 使用 `api schema`、`api snapshot` 和
稳定 CLI command，而不是恢复旧 `--skill` interface。Update command 如果获准，必须识别 installation
manager，绝不能绕过 Brew 或 npm manager 覆盖其管理的安装。

## 已接受的 automation、输入与 mutation 契约

- 默认输出本地化 human-readable text。显式选择 machine mode 后输出稳定且版本化的 JSON；streaming
  operation 使用 NDJSON。确切 selection flag、schema identifier、pagination bound 与数字 exit code
  留给 implementation slice 决定。
- Machine data 只写 stdout；本地化 diagnostic 与 progress 只写 stderr。稳定 exit category 区分
  usage、rejection、service unavailable、failed effect、cancellation、transport loss、incompatibility
  与 unknown outcome。
- 来自 argument、file 或 stdin 的 CLI input 必须显式选择并具有 byte bound。Secret 绝不能出现在
  argv、human output、diagnostic 或 shell completion 中。
- 每个 mutation 携带全局唯一 request identity，最多提交一次。结果不明确后，Client 只对账该
  identity，绝不把 effect 当成新请求重试。
- 破坏性 operation 需要显式 noninteractive confirmation contract。`--detach` 只允许用于具备
  durable Server-owned identity 且可在之后观察的 operation；绝不能把 terminal input 变成
  fire-and-forget effect。
- `completion` 根据已接受 grammar 生成 shell completion。Installed Skill 是由暴露它的 feature
  所拥有的单独 versioned product asset，不是当前 embedded Skill 的兼容副本。

## 已接受的打包边界

一套标准安装只暴露 `bin/orchbee`。Compatible private Server、Worker、Terminal Host、manifest，
以及包含 Agent integration 时的 Agent Bridge artifact 位于私有 versioned `libexec` cohort。
Shell installer、Homebrew 与 npm 分发同一套 signed 且经过 digest 验证的 native cohort；npm 使用
platform-specific optional package，执行 Orchbee 时不依赖 Node.js runtime。macOS 与 Linux 是首批
native release target；Windows 属于后续工作。

Release layout 使用 side-by-side version directory 和一个 atomic active pointer。安装或 migration
失败时回滚整套 compatible cohort。Public cutover 要等待选定 vertical command、legacy consumer
migration、packaged upgrade/rollback 证据，以及经过真实 artifact 测量的 startup/size budget；本提案
不凭空指定这些 budget。

## 问题

用户应安装并记住一个原生 `orchbee` 命令，用于交互式 TUI 与一次性自动化。此前 Desktop 内嵌 TypeScript CLI 及其 grammar、locator、launcher 和打包假设已退役；它们不是原生 Server/Worker topology 的兼容要求。

## 范围

本提案拥有：

- 公开 `orchbee` 调用模式与命令层级；
- argument、input、output、diagnostic 与 exit contract；
- 本机 会话 discovery 与显式准入的 activation command；
- human text 与 versioned automation output 之间的边界；
- 同一个 Rust artifact 内的 CLI/TUI 进程隔离；
- 未来原生发布 cohort 的 compatibility、public cutover 与 rollback；以及
- 公共进程不会获得 backend 或 arbitrary-command authority 的证据。

## 非目标

本提案不会：

- 拥有 Server state、operation catalog、Worker placement、PTY/VT state、Agent semantics 或
  Workbench transition；
- 定义 Terminal Host、Server-Worker 或 Agent Bridge wire protocol；
- 把私有可执行程序名称暴露为普通用户命令；
- 默认保留每条当前 CLI command 或添加 compatibility alias；
- 把 terminal-create permission 变成 raw shell、arbitrary argv、filesystem、Git 或 generic
  RPC authority；
- 要求 CLI 使用时初始化 TUI 或运行 Desktop app；或者
- 在选定 capability 与 rollback cohort 验证前授权未来 public release cutover。

## 进程与权威模型

| 关注点                                                                 | 权威                                  |
| ---------------------------------------------------------------------- | ------------------------------------- |
| Command parsing 与 presentation                                        | 一次 `orchbee` invocation             |
| Human session、semantic admission 与 Workbench projection snapshot     | Server                                |
| Worker selection 与 effect authorization                               | Server                                |
| Node-local execution                                                   | 确切 Worker 及其专用 capability owner |
| PTY、VT、history 与 terminal lifecycle                                 | 确切 Terminal Host                    |
| TUI focus、viewport、selection presentation 与 local interaction state | 确切 TUI Client Session               |

除非单独接受的 Client-local preference 拥有一个显式 store，否则 CLI 在调用之间保持无状态。它提交
semantic intent 并校验 typed result。它绝不读取产品 state file，不从 current directory 发现
Project，不直连 Terminal Host，也不把 terminal output 当作 Agent 或 Workbench 权威。

## 命令设计契约

已接受目标已经固定上文产品词汇。每个 implementation slice 在扩展 parser 前仍须显式落实以下维度：

| 维度          | 必需决策                                                                                                                                                              |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mode          | 裸 TUI、一次性 resource command、会话 lifecycle command、help、version、completion，以及任何 machine-only discovery mode                                              |
| Noun 与 verb  | 稳定产品词汇、层级、alias、缩写策略，以及 unsupported capability 的报告方式                                                                                           |
| Targeting     | 本机 会话 默认值、显式 remote 会话 或 Worker selection、identity validation、ambiguity 与 no-target behavior                                                          |
| Input         | Positional value、option、stdin/file、size bound、secret classification、confirmation 与 noninteractive failure                                                       |
| Output        | Human projection、有界 Workbench semantic snapshot、versioned automation schema、stdout/stderr ownership、streaming、pagination、redaction 与 compatibility lifecycle |
| Completion    | Synchronous result、detached request identity、observation、wait、cancellation、signal、timeout 与 unknown-outcome behavior                                           |
| Exit          | Usage、rejection、unavailable service、failed effect、cancellation、transport loss、incompatibility 与 unknown outcome 的稳定类别                                     |
| Activation    | 哪些 command 可以请求 service activation、哪些只做 observation，以及 version mismatch 或 partial cohort installation 如何失败                                         |
| Extensibility | Owner 如何准入新的 semantic command，且不暴露 operation registry、raw RPC、private executable 或另一个 parser authority                                               |

Automation 只消费已接受 command contract 显式选择的 versioned output mode。Human-formatted output
绝不是 automation API。会修改状态的 invocation 携带一个 request identity，最多提交一次，并在
response 不明确后只对账该 identity；它绝不会自动重放 effect。

## 已接受的 Phase 3A 原生入口底座

产品决策人在 2026-09-02 授权了这一增量切片。它创建真实但尚未发布的 Rust application 边界与
可测量 process baseline，同时不会假装 TUI、Server connection 或业务命令已经存在。

| 授权字段   | 已接受决策                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | 目标架构指定一个公共原生 `orchbee` process，但仓库中还没有对应 application artifact、deterministic invocation boundary、本地化原生 diagnostic 或可复现 startup measurement。若先实现业务命令或占位 TUI，就会被迫提前决定尚未接受的 Server、transport、activation 与 interface。                                                                                                                                                                                                           |
| 范围       | 把 `apps/orchbee` 添加为 experimental Rust Cargo application，并产出尚未发布的 `orchbee` binary。唯一准入的 argument vector 是 empty、确切 `--help` 与确切 `--version`。Empty argument 选择 TUI route；由于 TUI composition 不属于本切片，该 route 报告本地化的 development-unavailable result。Help 以及 unavailable/usage diagnostic 同时提供 `en-US` 与 `zh-CN`；version line 与 locale 无关。添加可重复的 release-profile baseline measurement。                                      |
| 非目标     | 不准入 TUI renderer、Workbench command、Server 或 Worker discovery、activation、IPC、network、authentication、persistence、terminal access、child-process supervision、shell completion、Skill、业务命令、package-manager artifact、Desktop packaging change、public-entry cutover、compatibility alias、当前 CLI migration 或 deletion。                                                                                                                                                 |
| 主张       | Parsing 精确且无 side effect。Help 与 version 成功完成，且不会初始化 TUI 或 backend capability。Empty argument 只会到达显式 unavailable TUI presentation。任何其他、额外或非 Unicode argument 都会在产生 effect 前成为 usage failure。Help 只写 stdout；unavailable 与 usage diagnostic 只写 stderr；version 向 stdout 写入一行 `orchbee <version>`。每条 authored human message 都具备两种 locale。                                                                                      |
| 边界       | `apps/orchbee` 只拥有自己的 process entry、invocation decision、locale projection 与 development presentation。除了 Rust standard library 外，它没有 runtime dependency，也不授予 Server、Worker、Host、filesystem 或 process capability。新 binary 是 Cargo verification input，但不是 release 或 packaging input。                                                                                                                                                                      |
| 证据       | Table-driven parser 与 locale test 覆盖每个准入 vector、额外 argument、受支持平台上的非 Unicode input、locale precedence 与 fallback。Process test 在两种 locale 下断言 exit status 与确切 stdout/stderr。Cargo dependency metadata 与 source review 证明不存在 backend/TUI/process dependency。Release measurement 记录 artifact byte、重复 warm `--version` latency，以及 host 可以提供时的 peak resident memory。仍需通过根 format、lint、build、test、project 与 documentation gate。 |
| 交付与回滚 | 先记录这个已接受切片，再添加 application contract 与 source，接入现有 Cargo workspace，收集 baseline，最后收敛 documentation 与 gate。该 application 是增量且尚未发布的，不拥有 durable state，也不改变任何当前 process。回滚只需移除 application member 与其已接受切片记录，不需要 data migration 或 compatibility handling。                                                                                                                                                            |

确切 Phase 3A reference result 与其 maintenance lifecycle 记录在 application guide 的
[测量](../../apps/orchbee/README.zh-CN.md#测量)一节。

### 建模记录

| 字段       | Contract                                                                                                                                                                                                                                                                                                                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 输入       | Argument vector 与 `LC_ALL`、`LC_MESSAGES`、`LANG` value 都是不可信 process input。Locale selection 按该优先级检查这些 variable，识别 `zh` language prefix，其余情况 fallback 到 `en-US`；它绝不会持久化 locale data，也不会据此做 authorization。                                                                                                                                         |
| 输出       | Help 与 version 返回 development exit status `0`；不可用的 bare TUI route 返回 `1`；invalid invocation 返回 `2`。这些数字只描述尚未发布的底座，不接受未来 stable exit table。一个 result 至多包含一个 static stdout payload 与一个 static stderr payload；准入 case 绝不会同时填充两者。                                                                                                   |
| 决策       | 零个 argument 选择 `Tui`；一个确切 `--help` 选择 `Help`；一个确切 `--version` 选择 `Version`；其他所有 vector 都选择 `UsageFailure`。不存在 short flag、alias、abbreviation、combined option、target inference 或 current-directory behavior。                                                                                                                                             |
| 模型       | Pure parser 把 argument 映射为一个 invocation variant 或 usage failure。Pure presenter 把该 decision 与 locale 映射为一个完整 process result。Process adapter 读取有界 input，各写一次所选 stream，并把 development status 映射到 operating-system exit code。                                                                                                                             |
| 依赖与状态 | Production code 只使用 standard-library argument、environment 与 stream capability。它不打开 file 或 socket，不启动 process 或 thread，不初始化 terminal mode，不读取 repository 或 product state，也不拥有跨 invocation 的 durable 或 mutable state。                                                                                                                                     |
| 验证与限制 | Exhaustive example 覆盖有限的准入 grammar 与两种受支持 locale projection。本切片不含 async event、retry、mutation、persistence、resource lifecycle 或 independently ordered transition，因此不选择 TLA+ 或 property model。Startup measurement 只是指定 host 与 profile 上的 reference observation，不是 cross-platform budget、cold-cache guarantee，也不能证明未来 TUI/CLI performance。 |

## 迁移与回滚

1. 未来 installed consumer 应按原生 cohort 盘点。
2. 选择第一批一次性 Rust command，并记录其确切 grammar、schema、activation policy、identity、bound
   与排除的 legacy capability。
3. 独立于 TUI rendering 和 backend engine 实现 Rust CLI core，再加入 TUI composition，且不得
   改变 CLI startup dependency。
4. 只针对刻意保留的 semantic behavior 验证其 owner contract。不得把 parser parity 当作
   acceptance oracle。
5. 打包一套兼容的 `orchbee`/Server/Worker/Host cohort；安装 Agent capability 时还要包含
   Agent Bridge，然后原子切换公共入口。在任何不兼容
   durable migration 前，rollback 恢复上一套完整 cohort。
6. 旧源代码退役按另行接受的明确终止决定推进。未来发布打包与 public-entry activation
   需要独立证据，不阻塞主要功能开发。

任何过渡状态都不会在同一个安装中暴露两个权威公共命令面。旧源代码退役已独立于发布打包完成。未来 packaging 必须恰好选择一个原生公共入口与兼容 cohort。
已经完成且尚未发布的 Phase 3A 底座不提供 packaged-entry cutover 的证据。

## 证据计划

| 主张                                                                          | 所需证据                                                                                                                                                                     |
| ----------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| CLI startup 不会初始化 TUI 或 backend engine                                  | Dependency 与 process-start test、cold/warm startup measurement、artifact-size 与 peak-memory measurement                                                                    |
| 一次 invocation 无法获得 Server、Worker 或 Host authority                     | Capability-negative test、malformed-input test、authentication test 与 direct-Host connection rejection                                                                      |
| 会修改状态的 command 最多提交一次                                             | Request-identity property test、transport-loss trace、restart reconciliation 与 unknown-outcome test                                                                         |
| Human output 不是 automation contract                                         | Versioned-schema conformance、stdout/stderr test、redaction test 与 compatibility fixture                                                                                    |
| 裸调用与显式 CLI mode 不会选择含糊路径                                        | Parser decision-table test、help/version test、TUI bootstrap test 与 unsupported-mode case                                                                                   |
| 裸本机激活会创建同级 Client Session，而不拥有 service 生命周期                | Absent/present/incompatible cohort process test、simultaneous-TUI trace 与 Client-exit retention test                                                                        |
| TUI 共享 Workbench 语义，但不共享另一 Client 的 view                          | Multi-TUI/Web trace、empty-context first-create test、Tab/Panel navigation test 与 attachment/controller fencing                                                             |
| 被管道连接的一次性 command 不会抢占 terminal device 或初始化 alternate screen | TTY/non-TTY process matrix、`/dev/tty` negative test、确切 stdout/stderr fixture 与 signal/terminal restoration test                                                         |
| Phase 3A 没有隐藏 backend 或 TUI 初始化，且每条 authored message 均已本地化   | Dependency metadata、两种 locale 的确切 process-output test，以及 release-profile startup/size/memory baseline                                                               |
| Cutover 后只剩一个公共入口与一套兼容 service cohort                           | Packaged installation、upgrade、rollback、partial-installation 与 stale-private-executable test                                                                              |
| 旧源代码退役后没有保留 consumer 绑定已删除 workspace                          | Consumer scan、保留 workspace 的 dependency/build/test check、native asset check 与 current-authority/documentation cleanup 为明确接受的源代码退役提供证据，不要求发布打包。 |
| CLI Workbench snapshot 暴露产品语义，但不暴露 Web 或另一 Client 的本地 state  | Server projection fixture、schema bound、authorization/redaction test 与 negative HTML/DOM/pixel/client-view case                                                            |

## 保留决策

1. 选择 machine output、input source、pagination、confirmation、targeting、timeout 与 detached
   observation 的确切 option 拼写；定义 versioned JSON/NDJSON schema 与数字 exit code，但不改变
   已接受的语义 category。
2. 从已接受的只读和 `terminal open` family 中选择确切 first-release subset；每项后续 mutation
   只有连同所属 owner state machine 与安全证据才能准入。
3. 定义 remote 会话 targeting 与 authentication。本机 command surface 不暗含 remote enrollment、
   production human/device identity、TLS、Internet reachability、multi-user tenancy 或 mobile
   background behavior。
4. 定义已接受的全屏结构与默认 keyboard map 之外的详细 TUI visual composition、accessibility
   projection、可配置 binding policy、mouse behavior 与非初始 layout operation。
5. 独立于旧 embedded-Skill contract 定义 installed Skill catalog 与 completion-installation
   mechanics；已接受的 `completion` generator 和面向 Agent 的 schema discovery 不决定这些
   distribution detail。
6. 从实测 release candidate 推导支持平台的 startup、artifact-size 与 peak-memory budget；Phase 3A
   reference baseline 不设置 acceptance budget。
7. 未来 packaged public cutover 需要 consumer、compatibility、signed-cohort upgrade、rollback、documentation 与 installed-artifact 证据。本次旧源代码退役已另行获得授权，不以该未来发布为前提。

<!-- documentation-pairing: english-sha256=184618492dc17ee8b216a13b5f1f5bb5c252a573b665de63ffac026d68a7432b zh-cn-sha256=b53d28ee882c1f3428aedfe13db8cd19fb08e458b2288f7a20d46f5cc828aaac -->
