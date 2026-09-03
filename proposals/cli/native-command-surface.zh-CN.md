# 原生 Orchbee 命令面

| 字段     | 值                                                                                                                                                                                                                                                                     |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态     | 已接受替换方向，增量 Phase 3A 原生入口底座已经完成；裸本机交互路径与有界懒激活已经接受并等待实现；更广 command grammar、稳定 output 与 exit contract、remote targeting、兼容窗口与 cutover 仍为保留决策                                                                |
| 负责人   | 产品命令体验，以及 Rust `orchbee` Client、Server 语义命令边界、TUI、打包与发行集成的维护者                                                                                                                                                                             |
| 现行权威 | 在切换完成前，[`apps/cli/README.md`](../../apps/cli/README.zh-CN.md) 仍是已交付 TypeScript CLI 的权威                                                                                                                                                                  |
| 父设计   | [`native-runtime-topology.md`](../platform/native-runtime-topology.zh-CN.md) 拥有可执行程序拓扑和进程职责；本提案拥有公开 Rust 命令面                                                                                                                                  |
| 决策权威 | 产品决策人已经接受替换并删除 `apps/cli`、在一个公共 Rust `orchbee` 可执行程序中实现 CLI 与 TUI、重新设计命令和参数而不保留当前实验性语法、允许 CLI 输出有界且 framework-neutral 的 Workbench semantic snapshot、下文确切且尚未发布的 Phase 3A 底座，以及裸本机交互路径 |
| 退出条件 | 交付已接受的 Rust 命令切片，以回滚证据切换打包后的公共入口，删除 `apps/cli` 与过时启动 artifact，把维护行为迁入新 app 指南和测试，然后删除本提案                                                                                                                       |

英文规范原文：
[native-command-surface.md](native-command-surface.md)。
如有冲突，以英文为准。

## 已接受的产品边界

- `orchbee` 是唯一公开的原生可执行程序名称。Server、Worker、Terminal Host 与 Agent Bridge
  仍是私有实现进程。
- 裸 `orchbee` 选择交互式 TUI。显式 command 与 option 选择一次性 CLI、lifecycle、help 或
  version 行为；下文确切 Phase 3A 底座之外的语法仍为保留决策。
- 本机 Site 不存在时，裸 `orchbee` 可以发起确切 compatible installed Server 与本机 Worker cohort
  的有界懒激活并等待 ready。Ready 后由私有进程拥有各自生命周期；`orchbee` 绝不继续监督它们。
- 每次 TUI invocation 都是一个独立 Client Session，与 Web tab、Desktop window 或 mobile Client
  使用同一模型。它拥有本地 focus、viewport、scroll 与 presentation state，但不会因为发起了
  activation 就获得共享 Site 或 terminal authority。
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

| 关注点      | 已接受行为                                                                                                                                                            |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation  | 空 argument vector 选择交互式 TUI 路径。用户不需要另一个 public executable 或 hidden mode。                                                                           |
| 已有 Site   | 连接确切 compatible local Server 并创建新的 Client Session；再次 invocation 会创建另一个同级 session，而不是复用本地 view state。                                     |
| Site 不存在 | 发起确切 installed private Server 与本机 Worker cohort 的有界懒激活，等待 typed ready、unavailable 或 incompatible result，并且只在 ready 后进入 TUI。                |
| 生命周期    | Activation 是 bootstrap effect，不是 parent-child supervision。关闭 TUI 只释放自己的 Client Session 与 attachment；不会停止 Site、service 或 retained terminal。      |
| 边界        | 公共进程既不内嵌 backend engine，也不直连 Terminal Host。它通过已接受且经 Server relay 的 Client path 获得 terminal behavior。                                        |
| 延后界面    | 显式 Site lifecycle command、一次性业务 command、稳定 automation output 与 exit category、remote targeting、completion、Skill、packaging 与 public cutover 继续保留。 |

## 问题

用户应该只安装并记住一个命令，同时可以选择交互式 TUI 或适合自动化的一次性操作。当前
`apps/cli` 是 Desktop 打包的 Node projection，面向实验性本机 coordinator 构建。它的语法、
locator、output、launcher 与 packaging 假设无法描述新的独立 Server/Worker 拓扑；保留这些
假设会迫使新架构兼容一套偶然形成的接口。

## 范围

本提案拥有：

- 公开 `orchbee` 调用模式与命令层级；
- argument、input、output、diagnostic 与 exit contract；
- 本机 Site discovery 与显式准入的 activation command；
- human text 与 versioned automation output 之间的边界；
- 同一个 Rust artifact 内的 CLI/TUI 进程隔离；
- 当前 `apps/cli` 的 compatibility、cutover、rollback 与 deletion；以及
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
- 在选定的替代 capability 与 rollback cohort 验证前授权删除。

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

未来语法必须在实现 parser 前显式决定每个维度：

| 维度          | 必需决策                                                                                                                                                              |
| ------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mode          | 裸 TUI、一次性 resource command、Site lifecycle command、help、version、completion，以及任何 machine-only discovery mode                                              |
| Noun 与 verb  | 稳定产品词汇、层级、alias、缩写策略，以及 unsupported capability 的报告方式                                                                                           |
| Targeting     | 本机 Site 默认值、显式 remote Site 或 Worker selection、identity validation、ambiguity 与 no-target behavior                                                          |
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
| 边界       | `apps/orchbee` 只拥有自己的 process entry、invocation decision、locale projection 与 development presentation。除了 Rust standard library 外，它没有 runtime dependency，也不授予 Server、Worker、Host、filesystem 或 process capability。`apps/cli` 及其 Desktop launcher 继续作为唯一已交付 CLI 权威。新 binary 是 Cargo verification input，但不是 release 或 packaging input。                                                                                                        |
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

1. 盘点 `apps/cli`、其 packaged launcher、`ORCHBEE_CLI` variable、embedded Skill、当前 command
   family、output 与 exit meaning 的每个真实 consumer。
2. 选择第一批一次性 Rust command，并记录其确切 grammar、schema、activation policy、identity、bound
   与排除的 legacy capability。
3. 独立于 TUI rendering 和 backend engine 实现 Rust CLI core，再加入 TUI composition，且不得
   改变 CLI startup dependency。
4. 只针对刻意保留的 semantic behavior 验证其 owner contract。不得把 parser parity 当作
   acceptance oracle。
5. 打包一套兼容的 `orchbee`/Server/Worker/Host cohort，并原子切换公共入口。在任何不兼容
   durable migration 前，rollback 恢复上一套完整 cohort。
6. 替换 cohort 与 rollback path 验证后，在一个已接受的删除切片中删除 `apps/cli`、Desktop
   launcher 与注入变量、过时 protocol/Skill consumer、test、documentation 与 project contract。

任何过渡状态都不会在同一个安装中暴露两个权威公共命令面。开发期间旧实现在源码中可以共存，
但 packaging 必须恰好选择一个。
Phase 3A 是有意保持尚未发布的源码共存状态：它既不完成第一批业务命令的选择，也不完成 packaged-entry
cutover。

## 证据计划

| 主张                                                                         | 所需证据                                                                                                          |
| ---------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| CLI startup 不会初始化 TUI 或 backend engine                                 | Dependency 与 process-start test、cold/warm startup measurement、artifact-size 与 peak-memory measurement         |
| 一次 invocation 无法获得 Server、Worker 或 Host authority                    | Capability-negative test、malformed-input test、authentication test 与 direct-Host connection rejection           |
| 会修改状态的 command 最多提交一次                                            | Request-identity property test、transport-loss trace、restart reconciliation 与 unknown-outcome test              |
| Human output 不是 automation contract                                        | Versioned-schema conformance、stdout/stderr test、redaction test 与 compatibility fixture                         |
| 裸调用与显式 CLI mode 不会选择含糊路径                                       | Parser decision-table test、help/version test、TUI bootstrap test 与 unsupported-mode case                        |
| 裸本机激活会创建同级 Client Session，而不拥有 service 生命周期               | Absent/present/incompatible cohort process test、simultaneous-TUI trace 与 Client-exit retention test             |
| Phase 3A 没有隐藏 backend 或 TUI 初始化，且每条 authored message 均已本地化  | Dependency metadata、两种 locale 的确切 process-output test，以及 release-profile startup/size/memory baseline    |
| Cutover 后只剩一个公共入口与一套兼容 service cohort                          | Packaged installation、upgrade、rollback、partial-installation 与 stale-private-executable test                   |
| 删除 `apps/cli` 不会移除刻意选择的 capability                                | Consumer scan、selected-capability traceability、packaging diff 与 current-authority/documentation cleanup check  |
| CLI Workbench snapshot 暴露产品语义，但不暴露 Web 或另一 Client 的本地 state | Server projection fixture、schema bound、authorization/redaction test 与 negative HTML/DOM/pixel/client-view case |

## 保留决策

1. 定义已接受裸本机路径和确切 `--help`、`--version` vector 之外的完整 command tree、option name、
   alias 与 help organization。
2. 选择 human 与 automation output format、versioning、streaming、pagination、diagnostic 与稳定
   exit category，包括确切 Workbench semantic snapshot command 与 schema。
3. 决定 remote Site targeting 与 authentication、显式 lifecycle command，以及已接受有界本机
   bootstrap 之外的 production service installation 与 activation mechanics。
4. 选择哪些当前 semantic capability（如有）进入第一版 Rust release，哪些在没有 compatibility
   alias 的情况下移除。
5. 独立于旧 embedded-Skill contract 定义 installed Skill 与 shell-completion distribution model。
6. 为支持的平台设置实测 startup、artifact-size 与 peak-memory budget；Phase 3A reference baseline
   不设置 acceptance budget。
7. 定义确切 packaged cutover、update cohort、compatibility rejection、rollback 与 `apps/cli`
   deletion authorization。
