# CLI Agent 控制面

| 字段       | 值                                                                                                                                                                                                     |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 状态       | 现有 TypeScript CLI 方向以及只读切片 1 至 3 已实现；后续终端控制切片仍为待决事项                                                                                                                       |
| 决策所有者 | 当前请求者，以 Orchbee 产品与架构决策所有者身份作出决策                                                                                                                                                |
| 语义所有者 | `apps/cli`、Desktop 应用组合、`@orchbee/semantic-command-protocol`、`@orchbee/workbench` 与 Agent 能力                                                                                                 |
| 当前权威   | [`apps/cli/README.md`](../../apps/cli/README.zh-CN.md)、[语义命令执行](../../apps/desktop/docs/semantic-command-execution.zh-CN.md) 与 [Agent 运行时观测](../agent/agent-runtime-observation.zh-CN.md) |
| 参考版本   | Herdr 修订 `c2637dc182ddc5425108824d5ed15d24ce38c4e3` 是固定的交互参考，而不是 Orchbee 状态或架构权威                                                                                                  |
| 退出条件   | 交付每个分别获准的切片，把需维护的行为与证据迁入所属应用和包的指南与测试，解决或移除所有剩余决策，然后删除本提案及其译文                                                                               |

简体中文审阅译文即本文。英文
[agent-control-surface.md](agent-control-surface.md) 始终为规范来源。

## 变更授权

| 字段   | 已接受的授权                                                                                                                                                                                                                                                           |
| ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 等级   | 这是面向 Agent 的进程契约，且将扩展跨工作区产品操作，因此属于 Consequential；切片 2 引入含 subscription、timeout、cancellation、revision 与 coordinator-lifetime transition 的 server-owned Agent wait，因此属于 Modeled                                               |
| 问题   | Orchbee 已经交付安全的本地语义命令 CLI，但只公开通用 operation 与 execution 命令。Agent 必须自行发现操作 ID 并手工构造 JSON，而且没有与发行版本匹配的 Skill 来说明已安装命令面及其安全边界                                                                             |
| 结果   | 把现有 `apps/cli` TypeScript 应用演进为 Orchbee 唯一的非可视化 Agent 控制面。在保留服务端拥有事实、精确身份、结构化输出、显式准入和保守未知结果的前提下，增量加入由所有者支撑的产品名词和版本匹配 Skill                                                                |
| 兼容性 | 保留现有 `status`、`operation` 与 `execution` 命令、`ORCHBEE_CLI` 启动契约、输出版本、退出含义、目标选择、输入上限与不重放行为。高层命令只是已准入语义操作之上的附加投影，绝不是另一条执行路径                                                                         |
| 非目标 | 用 Zig 重写 CLI、创建另一套 CLI 或 daemon、复刻 Herdr 的 TUI 或扁平 Agent 状态、把屏幕文本当成语义权威、直连 Terminal Host、加入任意 shell/文件系统/RPC 权限、全局安装、远程访问，或者在没有各自获准的所有者契约时准入启动、Prompt、输入、关闭、焦点、审批或破坏性操作 |
| 恢复   | 可移除一个高层命令或 Skill 章节而不移除通用 operation 控制面。修改操作响应丢失后仍只协调相同请求身份，绝不重放。所有者操作不受支持或不可用时必须显式失败                                                                                                               |

当前请求者最初选择了 Zig CLI，随后在 2026-08-29 明确取代该选择：扩展现有 CLI，
保留当前 TypeScript 实现。本提案以较新的指令为权威。

## 已接受架构

| 备选方案                                  | 后果                                                             | 决策 |
| ----------------------------------------- | ---------------------------------------------------------------- | ---- |
| 用 Zig 可执行文件替换 `apps/cli`          | 丢弃经过测试的客户端与打包路径，却没有增加产品权限               | 拒绝 |
| 创建第二套类似 Herdr 的 CLI 或状态 daemon | 产生相互竞争的发现、生命周期、状态与授权所有者                   | 拒绝 |
| 只保留通用 `operation invoke`             | 保留安全性，但迫使每个 Agent 与 Skill 重复产品 schema 和调用细节 | 拒绝 |
| 用所有者支撑的名词扩展现有 CLI            | 复用已认证端点和账本，同时让已准入能力可发现、可脚本化           | 接受 |

```text
Agent + 版本匹配 Skill
             |
      apps/cli 命令投影
             |
  语义命令协议与本地传输
             |
 Desktop 已准入目录与执行账本
             |
 Workbench / Agent / Terminal 能力所有者
```

- CLI 仍是无状态客户端。Desktop 或未来的 Hub 拥有权威状态、操作准入、执行回执、授权与提交边界。
- 产品名词只能构造并校验其语义所有者导出的输入与输出契约。它们绝不分派 UI Action，
  也不连接所有者的私有传输。
- 稳定的不透明 ID，而不是标签、顺序、聚焦 UI 状态、进程 ID、路径或猜测的拓扑，
  用于选择修改目标。
- 默认输出仍是一个带版本的 JSON 文档。Skill 以已安装 CLI 输出和精确返回的 ID 为权威。
- `orchbee --skill` 打印与可执行文件来自相同源码修订的指令。它无需连接协调器，
  也不授予额外操作权限。

## Herdr 对齐边界

| Herdr 交互思想                       | Orchbee 决策                                                                                                                             |
| ------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------- |
| 单一 CLI 与版本匹配 Skill            | 通过现有 `apps/cli` 与 `orchbee --skill` 采纳                                                                                            |
| JSON 响应与精确返回的标识符          | 保留当前带版本的 JSON 信封以及不透明的 Workbench 和 Agent 身份                                                                           |
| 服务端拥有实时拓扑                   | 保留 Desktop 或未来 Hub 的所有权；绝不从 renderer 状态重建拓扑                                                                           |
| Workspace、Tab、Pane 与 Agent 命令组 | 仅当各所有者发布所需语义读取或命令契约后，才适配为 Project、Directory、Workspace、Tab、Terminal 与 Agent 名词                            |
| Agent 识别与等待                     | 使用 Agent 所有的 Session、Turn、Attention、Outcome、attachment、provenance 与 freshness 模型；绝不复制 Herdr 的扁平状态或提升屏幕启发式 |
| Pane 输出与输入                      | 保留 Terminal Host 权威并要求显式、有界的语义契约；不通过 CLI 暴露其私有协议                                                             |
| Prompt、按键输入、焦点、关闭与停止   | 保留为待决事项，因为它们可能提交工作、回答提示、抢夺焦点、终止工作，或产生提交结果未知                                                   |

参考范围是命令组织、机器可读输出、精确目标、安全上下文和版本匹配 Skill 指引。
本提案不声称完整行为等价。Herdr 测试可以启发场景，但 Orchbee 证据细化的是 Orchbee 自身所有者契约。

## 已交付切片 1：已准入 Workbench 门面与 Skill

切片 1 仅在生产 Desktop 目录中已经存在的五个操作之上增加以下高层命令：

```text
orchbee project list [--limit <n>] [--cursor <cursor>]
orchbee directory list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee workspace list --project <project-id> [--limit <n>] [--cursor <cursor>]
orchbee terminal list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee terminal create (--standalone | --project <project-id> --workspace <workspace-id>) [--directory <directory-id>] [--detach] [--request-id <request-id>]
orchbee --skill
```

所有连接协调器的命令保留 `--target` 与 `--output`。分页上限保持在 Workbench 所有的边界内。
Workspace 目标必须同时提供精确 Project 和 Workspace 身份。`terminal create` 只创建所有者定义的
Terminal presentation，并保留现有提交、协调、重试与不聚焦语义；它不启动或 Prompt Agent。

内嵌 Skill：

- 仅在明确的 Orchbee 检查或控制意图下激活，并要求注入的 `ORCHBEE_CLI` 路径；
- 首先用 `"$ORCHBEE_CLI" --skill` 作为已安装语法权威；
- 只使用 JSON 返回的精确 ID，绝不猜测聚焦目标；
- 区分只读发现、Terminal 创建、通用 operation 访问、execution 观测与业务取消；
- 对不可用能力如实说明，而不是改用 shell 注入、私有协议或屏幕推断；以及
- 修改前要求明确用户意图，且绝不声称 execution 取消会逆转所有者已经提交的效果。

## 已交付切片 2：规范 Agent 读取与服务端拥有的等待

2026-08-29，请求者委托系统沿已经选定的 Herdr 对齐方向持续完成工作。切片 2 在该授权内采取
保守边界，只交付只读 Agent 观测，不增加任何 provider、Terminal 或 presentation 权限：

```text
orchbee agent list [--all] [--limit <n>] [--cursor <cursor>]
orchbee agent get <session-id>
orchbee agent explain <session-id>
orchbee agent wait <session-id> [--until <condition>]... [--after-revision <revision>] [--timeout <milliseconds>] [--detach] [--request-id <request-id>]
```

- `@orchbee/agent` 拥有四个 operation descriptor、严格输入输出 schema、摘要、详情、解释、
  条件匹配和默认等待条件。CLI 只接受精确返回的 Session ID；provider 名称、presentation 标签、
  聚焦状态、路径、进程身份和屏幕内容都不能解析目标。
- `agent list` 返回由 revision 限定并经服务端签名的稳定 Session 摘要页，默认仅包含 live
  Session，只有 `--all` 才包含仍保留的 ended Session，最大页大小为 100。`agent get` 返回精确的
  已保留规范快照及其 Agent 所有的语义投影。这两类查询都不返回 Prompt、逐字稿、命令、工具内容、
  文件系统路径、环境值、provider 错误或 adapter 私有的关联数据。
- `agent explain` 说明哪个规范维度选择了投影状态、已声明的能力上限、source、fidelity、
  freshness、generation 与 sequence，以及当前仅导航且无内容的限制。它不暴露或重建 Herdr
  式屏幕 matcher 证据，因为屏幕检测不是 Orchbee 的语义权威。
- `agent wait` 是一个已准入的异步查询，由 Desktop 应用组合基于规范
  `AgentSnapshotSource` 拥有。它在决定性重查之前先订阅，固定一个 Session 身份，并且只根据精确
  快照、显式超时、取消或协调器退役来结束。它不轮询 renderer、不维持隐藏终端，也不直接消费
  provider 事件。
- 等待条件包括 Agent 语义状态种类以及 `fresh`、`stale` 和 `disconnected`。默认条件为
  `needs-attention`、四种 outcome、`idle` 或 `ended`。语义状态条件仅匹配 fresh 观测；只有显式
  请求时才匹配降级健康状态。`--after-revision` 还要求运行时 revision 严格更新；若基线领先于
  当前运行时则拒绝，而不是含糊等待。
- 超时是显式规范输入，默认为 600,000 毫秒，边界为 1 至 3,600,000 毫秒。超时为
  `agent-wait-timeout`，身份不存在为 `agent-session-not-found`，未来基线为
  `agent-revision-ahead`，execution 取消为 `agent-wait-cancelled`。Desktop 或 Hub 重启会改变
  coordinator generation，因此进行中的等待按现有 CLI 契约成为结果未知；不声称连续性或重放。

切片 2 不添加 Agent 修改或持久状态。除确定性的领域、Desktop operation、CLI parser、runner 与
catalog-composition test 外，其异步订阅、重查、超时、取消、精确身份和协调器退役边界还有一个有界
TLA+ model。

## 已交付切片 3：规范 Tab 与 Panel 成员关系读取

切片 3 添加两个确定性 Workbench query 及其高层 CLI 投影：

```text
orchbee tab list (--standalone | --project <project-id> --workspace <workspace-id>) [--limit <n>] [--cursor <cursor>]
orchbee panel list (--standalone | --project <project-id> --workspace <workspace-id>) --tab <tab-id> [--limit <n>] [--cursor <cursor>]
```

- `@orchbee/workbench` 拥有两个严格的版本 1 operation contract，并从同一个经过验证的规范共享文档
  revision 派生两个投影。Desktop 只添加 catalog 准入、授权、签名分页与已有的有界 execution
  envelope；CLI 仍然只是这些 operation 之上经过 descriptor 检查的投影。
- `tab list` 要求一个精确的 standalone 或 Project-and-Workspace location。它保留规范 Tab collection
  顺序，并返回每个稳定 Tab 身份、terminal-context 身份、group 身份、pinned 状态、自定义与后备标题、
  Panel 数量和 Terminal-presentation 数量。
- `panel list` 还要求该 location 中一个精确的 Tab 身份。它保留规范的深度优先、从左到右 Panel-leaf
  顺序，并返回从一开始的位置、稳定 Panel、Tab 与 terminal-context 身份，以及
  Terminal-presentation 数量。缺失的精确 Tab 返回 `tab-not-found`；不存在 active Tab 或 focused
  Panel 后备。
- 两个 query 都保留 Workbench 的分页上限 200，把每个不透明 cursor 绑定到精确 operation、target、
  coordinator generation 与共享 revision，并在该 revision 变化后返回 `page-cursor-stale`。Panel
  cursor target 还包含精确 Tab 身份。
- 输出有意排除 active 或 selected Tab、focused 或 maximized Panel、最近 focus history、active child
  Terminal、split weight、pixel geometry、renderer state、screen content、process state 和推断的 Agent
  state。它们属于 Client View 或其他语义 owner 的事实，不属于规范共享成员关系。

此切片不添加 mutation、异步 owner transition、持久状态或新的失败排序。确定性 projection、schema、
Desktop operation、HTTP boundary、CLI parser、runner、catalog 与 Skill test 会细化该契约；不选择
TLA+，因为分页复用已有建模的 semantic-command ledger 和现有 revision-fenced cursor 机制。

## 不变量与证据

1. 每个高层命令恰好映射一个已准入操作 ID 和一个所有者有效的有界输入；只有实时完整公共 descriptor
   与内嵌所有者契约匹配时才调用。解析失败不连接协调器，也不分配请求身份。
2. 高层调用与通用调用共享相同的请求身份、调用前 describe、执行账本、等待、取消、退出码与未知结果实现。
3. 仅添加高层 CLI 投影绝不向 Desktop catalog 添加 operation，也不授予调用者权限；每个新的 owner
   operation 都会单独声明、准入和测试。
4. `--skill` 在运行时不做发现、连接、调用、文件读取或产品修改。
5. 现有命令参数与输出文档保持兼容。

确定性 parser 与 runner 测试覆盖每种已接受形态、不兼容 selector、精确操作/输入映射、操作不可用、
响应丢失、输出与 Skill 不连接路径。Desktop 真实端点测试覆盖 Node、打包 Electron Node mode 与精确
launcher，并证明打印 Skill 不增加端点请求。切片 1 不添加新的异步所有者状态，因此复用 semantic-command
ledger model。切片 2 新增 `AgentSemanticWait.tla`；领域与 Desktop operation test 对其 exact-identity、
revision、freshness、subscribe-before-recheck、timeout、cancellation、removal 与
coordinator-retirement boundary 进行 refinement。切片 3 添加确定性 Workbench 成员关系投影与精确
target cursor test，不增加新的状态机。

## 交付顺序

1. 已交付切片 1：Workbench 门面与版本匹配 Skill 不修改生产操作目录。
2. 已交付切片 2：只读 `agent list`、`agent get`、`agent wait` 与 `agent explain` 使用上面的精确契约。
3. 已交付切片 3：只读 `tab list` 与精确目标 `panel list` 使用规范共享 Workbench 文档，且不暴露
   Client View 状态。
4. 逐个所有者操作接受 Workspace、Tab、Panel 与 Terminal 的创建、split、move、rename、focus 和 close 等拓扑修改。
5. 只有具备精确 provider 与 Terminal 权限、确认、审计、取消、重放和未知结果契约后，
   才接受 Agent 启动、Prompt、Resume、Decision、原始终端输入/输出与编排。
6. 把已交付行为与证据迁入当前所有者指南并删除已解决的提案章节；没有未来决策后删除完整提案。

## 待决事项

- 超出所选易失 Desktop 运行时与现有 coordinator-generation 失败边界之外的任何 Agent CLI
  连续性，包括跨 Desktop 或 Hub 重启的持久等待重放或状态保留；
- 调用者上下文环境变量及其精确 Project、Workspace、Tab、Terminal、presentation attachment 与远程拓扑含义；
- 创建、split、move、focus、close、stop 与破坏性生命周期命令；
- Terminal 输出快照、历史边界、敏感内容策略、输入、paste、逻辑按键与输出等待；
- Agent 启动、命名、provider 参数、Prompt/Turn 身份、提交、审批、回答、Resume、中断、编排与外部成本策略；
- 全局安装、shell completion、第三方 API 稳定性、远程/SSH/Browser 调用、多用户、审计保留与持久执行恢复；以及
- 超出上面明确选择的固定交互思想以外的任何完整 Herdr 等价声明。

实现在触及这些决策之一的最早切片处停止，直到其所有者接受精确契约、失败行为、证据边界、
发布与恢复。
