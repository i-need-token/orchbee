# Agent 会话命名侧表

| 字段             | 内容                                                                                                                                                                                                                                                                                                                                 |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status           | 设计候选，未实现、未获接受。OBAP wire 联合、names DTO 与帧编解码先于任何 Server 执行器或 CLI 面合入，本文档为未来的实现切片承载既有设计语义。在产品与架构决策负责人接受、且归属实现落地之前，本文任何内容都不是当前权威                                                                                                                            |
| 决策负责人       | 当前请求方，以 Orchbee 产品与架构决策负责人身份                                                                                                                                                                                                                                                                                       |
| 语义归属         | Server `AgentState` 命名表归属、`AgentService` 执行器、OBAP names 家族、CLI names 命令面                                                                                                                                                                                                                                                  |
| 当前权威         | [`apps/orchbee-server/docs/agent-canonical-model.md`](../../apps/orchbee-server/docs/agent-canonical-model.md)（已实现的读模型与会话解复用的 names 拒绝）、[`crates/agent-observation-protocol/README.md`](../../crates/agent-observation-protocol/README.md)（names 编解码/DTO 的暂存状态）                                                    |
| 退出条件         | names 功能在自身获接受的授权下实现，且归属权威——canonical-model、command-surface 与 `schemas/agent-observation/v1`——收敛为描述已交付行为；届时删除本提案及其译文                                                                                                                                                                          |

英文原文为规范：[session-names.md](session-names.md)。

## 变更授权（提议）

| 字段           | 提议授权                                                                                                                                                                                                                                                                                                                              |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 分级           | Modeled——侧表在 `AgentState` cell 内引入一张新的有状态表、selector 解析次序、原子改名迁移与有界读载荷；Consequential 面限于已合入的 wire kind                                                                                                                                                                                               |
| 问题           | 客户端只能用不透明会话身份寻址会话。已合入协议已能收发封闭的 names 请求/结果家族（resolve、names.list、name.assign、name.release），但不存在任何 Server 状态、执行器、策略门或 CLI 文法来应答它                                                                                                                                              |
| 阶段成果       | 一张按 Site、随运行时存续的别名表置于 `AgentState` 内，通过共享 `with_state` fold 应答全部四种 names 操作；CLI 解析位置 `auto` 词与显式 `--session`/`--name` 逃逸；已发布 schema 与 read 家族并列覆盖 kind 7–14                                                                                                                                |
| 非目标         | 跨运行时 dispose 持久化名字、远端（非托管）解析、改动规范会话文档、写账本或重放语义、会话删除（`delete.request` 仍保留——落地时必须路由经过同一个 fold 内 release），以及任何第二 names 权威                                                                                                                                                    |
| 兼容性         | 在本切片落地之前，names 行为即当前拒绝：良构 names 帧以 `WrongDocumentKind` 终结客户端会话。落地后每个 names 操作经定型联合应答——从不命名会话的客户端无需任何改动                                                                                                                                                                              |
| 交付           | 单一切片：`AgentState` names 映射 + fold 内清扫、`AgentService` 执行器操作、`AgentPolicy.manage` 准入、会话解复用路由到执行器、CLI 命令面、schema kind 7–14 与归属权威收敛——证据落在协议、服务、会话解复用与 CLI 进程各层                                                                                                                        |
| 恢复           | 撤回 names 解复用路由；暂存的编解码/DTO 保持惰性，会话解复用拒绝即缺席行为——绝不伪造 names 结果                                                                                                                                                                              |

## 已核实的当前状态

- `crates/agent-observation-protocol` 携带封闭的 `AgentRequest`/`AgentResult`
  联合、全部 names DTO（`AgentSessionSelector`、`AgentNameBinding`、四对
  请求/结果）、`MAXIMUM_NAME_BINDINGS`（512）以及带边界体的 OBAP 帧
  kind 7–14。`schemas/agent-observation/v1` 只覆盖 read 家族（kind 1–6）；
  names kind 没有已发布 schema。
- `apps/orchbee-server` 的 `AgentState` 不持有 names 表，`AgentService` 只执行
  list/get/explain。`AgentPolicy` 仅有 `read` 门——不存在 `manage` 能力。
  应用会话解复用对每个良构 names 请求以
  `AgentWireError::Contract(WrongDocumentKind)` 应答并终结会话，不调用读执行器、
  不写回结果。
- CLI 不暴露任何 names 命令路径；`docs/cli/command-surface.md` 没有已准入的
  names 文法。
- 下文设计逐字搬移自曾短期置于 `agent-canonical-model.md` 内的 names 章节；
  在此保留为供未来实现切片评审的候选，而非已接受事实。

## 提议设计

names 侧表是按 Site、随运行时存续的别名表，由规范的 `AgentState` cell 持有——
绝不写入规范模型字段、绝不持久化，每次运行时 dispose 后重新为空。它让客户端可以
用短的操作者指派别名寻址会话，而非不透明会话身份；规范会话文档本身不变且从不
携带名字。

### 载体与不变量

- `AgentState` 持有 `names: BTreeMap<AgentName, AgentSessionId>` 与反向索引
  `sessions_named: BTreeMap<AgentSessionId, AgentName>`——一对正/反映射与运行时
  共处于同一把 `Mutex` 之下。`names` 的顺序即发射顺序（按 name 字节序）。
- 名字为 ASCII `[a-z][a-z0-9_-]{0,31}`；`AgentName` 定型构造器是唯一准入点，
  任何路径都无法存入畸形别名。
- 每个会话至多一个名字、每个名字至多绑定一个会话；正/反映射在每个 fold 边界
  保持一致。
- 绑定永远只指向 live 会话：每个 `with_state` fold 末尾做 reconcile 清扫，丢弃其
  会话缺席或不再 `live` 的绑定。hook `session-ended`、原生 presentation 退役、
  settlement/tick 过期 fold 以及任何未来的移除路径都收敛到同一个 fold 内清理，
  因此绑定绝不会比结束其会话的 fold 多存活一步——也绝不会在那之后被观察到。
- 表以 `MAX_AGENT_SESSIONS`（512）为上界。该上界只约束新增绑定：给已命名会话
  改名在同一 fold 内原子地释放旧位并占新位，因此满载时改名成功。
- dispose 或 poison 的 cell 应答 `runtime-unavailable`，绝不伪造空表或陈旧绑定。

### Selector 解析

每个带目标的操作携带 `AgentSessionSelector`——`session`（定型
`AgentSessionId`）、`name`（定型 `AgentName`）或 `auto`（CLI 位置词）。解析在读取
或变更所在的同一 `with_state` fold 内进行：

- `session`——仅做精确会话身份存在性检查（任意生命周期），绝不回落到 names。
- `name`——只查完整 names 表。
- `auto`——先查精确会话身份存在性，再查 names 表。会话身份可以恰好是合法名字，
  因此决定因素是存在性而非字符集：等于某个 live 会话 id 的名字解析到该 id；
  `--name` 是进入 names 表的唯一逃逸通道。

`resolve` 报告 `resolved{sessionId, resolvedBy}`，`resolvedBy` 记录命中的是哪条
规则（`session-id` 或 `name`）；未命中为 `not-found`。`assign` 与 `release` 以同样
方式在变更 fold 内解析 `target`——在请求与 fold 之间结束的会话得到
`session-ended`，绝不产生幽灵绑定。

### 有限迁移

| Wire 请求 kind | 权限 | Fold 语义 | 结果 |
| --- | --- | --- | --- |
| `agent.session.resolve.request` | `policy.read` | 仅 selector 解析，零变更 | `resolved{sessionId,resolvedBy}` / `rejected{not-found\|read-denied}` / `unavailable` |
| `agent.session.names.list.request` | `policy.read` | fold 内清扫后枚举全表 | `listed{bindings:[{name,sessionId,lifecycle}]}` / `rejected{read-denied}` / `unavailable` |
| `agent.session.name.assign.request` | `policy.manage` | 解析目标；活性与冲突检查；提交绑定（改名时原子换绑） | `assigned{sessionId,name,previousName}` / `unchanged{sessionId,name}` / `rejected{name-in-use\|session-not-found\|session-ended\|name-not-found\|capacity-exceeded\|manage-denied}` / `unavailable` |
| `agent.session.name.release.request` | `policy.manage` | 解析目标；存在则丢弃绑定 | `released{sessionId,name}` / `unchanged{sessionId}` / `rejected{name-not-found\|session-not-found\|manage-denied}` / `unavailable` |

- 会话已持有别名时 `assigned` 携带 `previousName`——旧绑定在同一 fold 内释放、
  新绑定提交，并发 `names.list` 绝不可能观察到两个都缺或两个并存。
- `unchanged` 是状态幂等（`assign` 已绑定的同名对、`release` 无别名的会话），
  不是 requestId 重放承诺——不存在写入账本；回复丢失对调用方是
  `outcome-unknown`，传输层绝不重试。
- `name-in-use` 保留在册绑定；拒绝携带在册方的 `conflictSessionId`，调用方可见
  真实属主。
- `names.list` 是有界读取载荷（≤512 条定型绑定），不属于小结果——与 `listed`
  会话同等地有界并校验；`is_small_outcome` 只收纳 `rejected`/`unavailable` 与
  一次性 manage 应答（`resolved`、`assigned`、`unchanged`、`released`）。

### 有界证伪要点

- 与 live 会话 id 撞名的 `name`：位置 `auto` 解析选中会话；`--name` 是唯一指向
  别名的路径。绑定随其会话结束而释放，因此释放的名字之后可以绑定别的会话。
- 对 ended 或缺席会话的 `assign` 在 fold 内得到 `session-ended`/`session-not-found`
  ——任何 fold 前读取都不被当作活性证明。
- 对未绑定 `--name` 的 `release` 为 `name-not-found`；对无别名会话为 `unchanged`；
  对缺席会话为 `session-not-found`。
- 另一个客户端持有 `wait` 轮询时提交的 `rename` 绝不使 wait 换目标：等待方已钉住
  解析出的完整会话 id。
- `dispose` 随运行时清空该表；此后所有 names 操作应答
  `unavailable{runtime-unavailable}`。
- 未来 `delete.request` 落地时，会话移除必须路由经过同一个 fold 内 release——
  记录于此以免未来 owner 绕过清扫。

## 实现切片的未决证据缺口

- names 小结果体在产生侧限 1,024 字节；评审标记的读取侧是否按该上限预分配
  的问题留给实现切片（评审项 C8）。
- 目前没有任何 names 编解码往返语料、Server 执行器证据、selector 规则覆盖或
  CLI 进程证据；暂存的 DTO 除协议 crate 类型级校验外未经测试，在本切片自身
  证据落地之前不得对外宣称可用。
