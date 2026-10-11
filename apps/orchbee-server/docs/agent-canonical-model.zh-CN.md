# 规范 Agent 观察模型

本文档是 `src/agent/` 下 Server 侧规范 Agent 观察模型的归属文档，记录已接受范围、
移植必须保持的语义断言，以及权威 TypeScript 模型与其 Rust 镜像之间的边界映射。

## 状态与激活边界

`apps/orchbee-server` 拥有对 `packages/agent` 在基提交
`0d4b685a1b7db9405ca94908385e0534ddd41dc9` 所定义确定性语义的忠实 Rust 实现。
该模块由 `main.rs` 在 `#[cfg(any(unix, windows))]` 下注册：规范运行时存在于 **Server 二进制
crate** 内部，绝不进入库。一致性测试在同一二进制目标中只运行一份；`lib.rs`
既不声明也不导出 Agent 模块。不存在公开 API、新 crate 或第二份规范状态。

生产面现已覆盖托管本地观察路径端到端：

- `model` 值类型与 `AgentRuntimeSnapshot`——定型规范状态；带 presence 语义的
  原始构造/校验层即生产准入路径（envelope 解析为定型快照，读取再镜像回去）；
- `AgentRuntime`——空快照构造、`snapshot`、`dispose`、`is_disposed`，以及经
  规范 reducer 的生产 `dispatch`。二进制私有的 `agent_service` 持有一把
  `Mutex<AgentState>` 读写 cell，每请求在锁内克隆一份一致的有界快照，并在投影
  编码或套接字写之前释放锁。`AgentState` 在同一 cell 内持有唯一的
  `AgentRuntime`、hook 翻译器与原生观察叶子——每个触运行时的翻译器方法都接收
  注入的 `&mut AgentRuntime`，因此读路径、hook 派发与原生应用共享同一份规范
  存储；
- `join`、`hook_contract` 与 `hook_ingress`——移植的
  `AgentObservationJoinStateMachine` 与 `AgentHookObservationIngress` 翻译器，
  对托管端点的 envelope 准入、终端应用信号、presentation 注册/退役与 settlement
  推进均为生产激活；
- `native_observation`——私有原生归约叶子，经 Worker 载体准入/退役与 drain
  页应用/终态栅栏服务缝生产激活；
- `project_agent_session`、`project_agent_session_summary` 与
  `explain_agent_session`——OBAP `list`/`get`/`explain` 操作消费的状态/摘要/
  详情/解释投影；
- `local_site_service/agent_observation_endpoint.rs`——在 Site 就绪之前绑定的
  托管私有 Unix 端点；
- `local_terminal_service/agent_drain_read.rs`——作为 `execute_prepared`
  provider 接线的生产 drain 读泵。

仅测试面保持 `#[cfg(test)]`：种子派发路径（`AgentService::seed`）、语料/JSON
发射器与 `FixtureIngress`、`DispatchTrace` 记录器、f64 域语料 settlement 推进、
仅测试服务桥缝（`admit_hook_envelope`、`observe_hook_signal`、
`hook_environment_overlay`、`retire_hook_presentation`、
`advance_hook_settlement`）、仅测试逐事件原生缝（`native_apply_event`、
`native_apply_gap`、`native_reconcile_bindings`、
`native_reconcile_and_snapshot`、`native_sweep`、`native_held_count`）、仅测试
`environment_overlay_for` overlay 缝（生产 overlay 由 Worker 侧依据已校验的
site runtime 生成，而非此处）、聚合/位置/wait 投影，以及全部一致性线束。
生产构建不含种子标志、环境变量、wire 回放或第二条状态路径。

初始运行时是真的空：revision `0`、会话列表为空、`get`/`explain` 报告 `not-found`；
只有当所属监听器停止服务并调用 `AgentService::shutdown`——在同一把锁下销毁规范
运行时——之后才观察到 `runtime-unavailable`，存活的已准入连接随后看到真实的
不可用。

每一次 `AgentState` 访问都经过唯一的私有 `with_state` 失败边界：互斥锁 guard 在
`catch_unwind` 内创建并销毁，因此一次 panic 的 fold 会永久 poison 该 cell，并以
`AgentFoldUnavailable` 返回而不穿越调用方的外层 guard。该 cell 永不解除
poison——不存在任何 `into_inner`/`clear_poison`——部分变更永不可观察，且
`shutdown` 对已 poison 的 cell 保持幂等空操作。读路径把 dispose 与 poison 都映射到
既有的 `RuntimeUnavailable` wire 臂；各写缝返回
`Result<_, AgentFoldUnavailable>`，fold 失败绝不会与"负载被拒"或"空 overlay"混淆。

## 布局

| 路径                                         | 职责                                                                                                                                                                                                       |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/agent.rs`                               | 模块声明与 `pub(crate)` 生产再导出；语料/测试辅助在各模块内保持门控                                                                                                                                        |
| `src/agent/model.rs`                         | 规范值类型、身份校验、UTF-16 排序、快照不变量；原始构造与校验即生产准入路径                                                                                                                                |
| `src/agent/events.rs`                        | 十四种观察事件与四种语义事件；`#[cfg(test)]` `event_id` 溯源访问器                                                                                                                                         |
| `src/agent/reducer.rs`                       | `transition_agent_runtime` 按类守卫、unchanged/rejected/applied、语义事件顺序；`#[cfg(test)]` transition 访问器                                                                                            |
| `src/agent/changes.rs`                       | 变更记录、保留 512 条且连续的生产追加；生产 `read_agent_runtime_changes` 分页类别                                                                                                                          |
| `src/agent/runtime.rs`                       | `AgentRuntime`——生产：空快照构造、`snapshot`、`dispose`、`is_disposed`、`dispatch`；`#[cfg(test)]`：seed；生产变更读取                                                                                     |
| `src/agent/projection.rs`                    | 语义状态、摘要与解释投影；`#[cfg(test)]` 聚合/位置投影与 wait 条件匹配                                                                                                                                     |
| `src/agent/tests/corpus.rs`                  | `#[cfg(test)]` 一致性语料运行器与私有夹具解码器                                                                                                                                                            |
| `src/agent/tests/targeted.rs`                | `#[cfg(test)]` 具名边界回归测试                                                                                                                                                                            |
| `src/agent/tests/oracle.rs`                  | `#[cfg(test)]` 对已记录 TypeScript oracle 结果断言的具名回归测试                                                                                                                                           |
| `src/agent/join.rs`                          | Desktop `AgentObservationJoinStateMachine` 的纯移植：代际注册/退役、hint/事件 join、TTL 清理、settle 效果、准入边界；`#[cfg(test)]` `reset`                                                                |
| `src/agent/hook_contract.rs`                 | 严格 envelope/关联 hint 边界解析与十二 adapter 事件门注册表（移植 `agent-hook-observation-contract.ts` 与 `resolveAgentHookAdapter.ts`）                                                                   |
| `src/agent/hook_ingress.rs`                  | `AgentHookObservationIngress` 的纯移植，桥切分后不持有运行时：会话绑定、turn/请求/子会话映射、join 命令消费、经注入的 `&mut AgentRuntime` 按序规范派发；`#[cfg(test)]` 语料/trace/f64 settlement/JSON 辅助 |
| `src/agent/hook_ingress/settlement_queue.rs` | 生产 `Instant` 漏斗使用的 `SettlementQueue` 计时原语，以及仅供语料使用的 `#[cfg(test)]` 时钟推进入口                                                                                                       |
| `src/agent/tests/conformance_support.rs`     | 仅供测试使用的 fixture 形状 JSON 发射器、`DispatchTrace` 和 `FixtureIngress` 语料所有者对                                                                                                                  |
| `src/agent/hook_ingress/tests.rs`            | 相邻单元测试，验证派发身份、settlement 时钟顺序和 provider 状态转换                                                                                                                                        |
| `src/agent/tests/hook_conformance.rs`        | `#[cfg(test)]`——冻结 join 与 hook 语料的回防线束及原生联合容量检查                                                                                                                                         |

## 读服务与传输集成

首读由二进制私有的 `agent_service` 提供，集成进既有已准入本地客户端路径——不新增
套接字、监听器、线程或主动推送帧：

| 路径                                                      | 职责                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/agent_service.rs`                                    | `AgentPolicy`、`AgentService`（一把 `Mutex<AgentState>` cell + 按启动期游标权威）、list/get/explain 共享 prepared-read 执行器 `execute_prepared`、`open_observing` 构造期套接字绑定、`shutdown`、`AgentReadFault` 响亮失败类型、`PreparedUnavailable` provider 拒绝标记、生产 `Instant` 时钟缝（`admit_envelope`、`tick`、`next_deadline`）、生产原生载体缝（`native_admit_carrier`、`native_retire_carrier`）与 drain/栅栏缝（`native_apply_drain_page`、`native_fenced_final_snapshot`）、`#[cfg(test)]` `seed`、仅测试桥缝（`admit_hook_envelope`、`observe_hook_signal`、`hook_environment_overlay`、`retire_hook_presentation`、`advance_hook_settlement`）与仅测试原生缝（`native_apply_event`、`native_apply_gap`、`native_reconcile_bindings`、`native_reconcile_and_snapshot`、`native_sweep`、`native_held_count`、`native_poison_after_mutation_for_test`） |
| `src/agent_service/map.rs`                                | 显式定型的规范→wire 映射器；任何不可表示的规范值响亮失败，绝不过滤或捏造                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `src/agent_service/cursor.rs`                             | 私有按启动期 HMAC 列表游标权威：铸造、规范 base64url 形状、MAC/形状/语义校验顺序                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `src/agent_service/tests.rs` + `cursor/tests.rs`          | 策略、排序、分页、游标 invalid/stale/启动期边界、已结束过滤、销毁与映射保真覆盖                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `local_client_service.rs`                                 | 与 workbench 服务并列的一个 `Arc<AgentService>`；`AgentRuntimeOwner` 守卫在监听循环结束时销毁运行时                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| `local_client_service/application_session.rs`             | 同一顺序记录循环内的第三条 `OBAP` 解复用臂——读取、执行、恰好写回一条关联结果                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `local_client_service/agent_harness.rs`（`#[cfg(test)]`） | 可复用的种子服务传输线束：真实事件派发种子、`admitted` 套接字对会话、`listen` 真实监听夹具                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| `local_client_service/tests/agent.rs`                     | 真实监听套接字与已准入对的 OBAP 证据：种子读、空状态、owner 落下后不可用、畸形记录终止                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `local_client_service/application_session/tests/agent.rs` | OBAP 解复用覆盖：路由、与 OBCS 交错的顺序、畸形记录、执行器故障、结果写失败                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `local_terminal_service/agent_drain_read.rs`              | 生产 Agent drain 读泵——每读恰好调用一次的 `execute_prepared` provider：typed 载体三态（`NoCarrier` 是唯一真无值读）、准备锁定前一次性铸造的私有四秒窗口下至多一个在飞 exchange、仅 Backpressure 的 25 ms 准入重试、整页一次 fold、屏障由首个载体匹配回复钉住一次、终态栅栏快照在按序持有的 registry -> coordinator 双守卫内完成                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `local_terminal_service/agent_drain_read/tests.rs`        | 真实线路组合证据：canonical hint + 富观察经真实 registry/`UnixStream`/coordinator 投影、`NoCarrier`/pending/`Unavailable` 区分、载体轮换与页栅栏、deadline 拒绝、poison/销毁、以及保留快照的 `NoCarrier` 读                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| `local_site_service/agent_observation_endpoint.rs`        | 托管观察端点——hook ingress 的私有 Unix socket 传输，在 Site 就绪前绑定于 Site 运行时目录内：每站点一个绑定路径、单 poll 线程上的 nonblocking 监听器与停止管道、有界 accept 排空、共享 500 ms deadline 下的逐连接读取/admission/ACK 状态机、drop 时身份门控解除链接                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `local_site_service/agent_observation_endpoint/tests.rs`  | 真实 socket 端点证据：即时与结算应用、未关联/零关联拒绝、deadline 清扫、连接上限与交错连接、poison/销毁 cell、脚本化写者缝、身份失配、活连接 drop 回收与有界 accept 批                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

读投影绝不改变规范状态：每个请求都经过同一个共享执行器——先应用不可变的策略门，
然后恰好调用一次该请求的必需快照 provider（在任何 `AgentState` 锁之外、panic
catch 之下），释放所有守卫后再投影与映射。`execute_prepared` 是生产入口：
`local_terminal_service/agent_drain_read.rs` 是调用方自有的 provider，它在
provider 自身的终态 fold 全程持有外层 registry -> coordinator 守卫。旧式
`execute` 测试缝为夹具保留原始快照克隆——逐字保留，并非新鲜度证明。provider 拒绝或 panic 都映射到该操作的关联
`runtime-unavailable` 臂——绝不是 `AgentReadFault`，也绝不是捏造或 last-good
快照。`list` 保持规范 UTF-16 存储序，遵守有界 `limit` 与 `include_ended`，铸造并
校验不透明的按启动期游标（`cursor-invalid`/`cursor-stale`）；`get`/`explain` 对
缺失会话回答 `not-found`；策略拒绝时全部操作报告 `read-denied`，销毁后报告
`runtime-unavailable`。

边界说明：仅有门控面或缺席远端消费者的规范携带负载逐字保留——删除或收窄它们
会破坏已接受的 TypeScript 对等模型。按 docs/RUST.md 的 lint 政策，它们携带
逐项 `#[cfg_attr(not(test), expect(dead_code, reason = "..."))]` 例外——每条
注明规范不变量与缺席的生产消费者，且 `expect` 在真实读取者出现时自动失效：
仅语料构造的事件变体（`LimitedActivityObserved`、`ObservationBecameStale`、
`ObservationReconnected`、`RawPresence::Other`）、仅被门控 trace/读取路径消费的
事件 `event_id` 字段、`AgentSemanticEvent` 负载、`AgentTransition` 负载、
`NativeTransition` 的 `class`/`ack_w`（仅被仅测试逐事件服务缝读取）、fold 结果
的诊断字段（`FinalFence::BarrierUnmet`/`Rejected`）、变更馈送投影字段
（`AgentRuntimeChange`/`AgentRuntimeChangeSession`）、为仅测试 overlay 缝保留的
ingress `socket_path`、`RegistrationFact::binding`，以及 drain exchange 的 `Lost`
负载。`agent.rs` 的重导出在二进制的生产代码或测试代码中都有真实消费者，
不需要未使用导入抑制。无模块级抑制、无虚假使用、无常量表、无变体收窄。

## 会话命名侧表

OBAP names 家族由 `agent_service/names.rs` 在同一个 `AgentState` 状态单元中实现。
canonical 索引持有名称 → 会话和会话 → 名称；下述终端扩展共享同一名称空间。名称只属于当前运行时，不进入 canonical
会话文档，尚未持久化。名称格式为 `[a-z][a-z0-9_-]{0,31}`。每个名称和每个会话至多一个
绑定，上限 512。新增绑定检查上限；满额时原子重命名仍可成功。重名拒绝携带已有占用者的会话身份。

每次 `with_state` fold 在释放叶子互斥锁前协调两张映射，移除指向缺席或 ended 会话的绑定。
dispose 清空映射。poison 和 dispose 回答 `runtime-unavailable`，不伪造空表成功。
精确会话选择器接受任意生命周期的保留身份；赋名还要求 live。名称选择器只查询别名表。
auto 优先检查精确会话是否存在（包括 ended 历史），再查询名称；显式名称选择可避开冲突。
已有但未命名的会话释放名称是 unchanged；缺席的显式名称被拒绝。

read 门控制 resolve/list，独立 manage 门控制 assign/release。策略门先于观察准备。
调用者的既有 drain provider 在状态单元外执行一次；成功后，单次 fold 重查当前运行时，
原子完成定位与修改。准备所得克隆只证明新鲜度，不作为过期的存活性依据。叶子 fold 内不执行
socket I/O、不获取外层锁。顺序应用会话解复用路由两个 OBAP 家族并写回一个相关结果。
修改回复丢失时结果未知，不设重放账本。

`schemas/agent-observation/v1` 覆盖 kind 1–14。原生 `agent names`、`agent resolve`
和 `agent rename` 消费 names 家族；get/explain/wait 及终端控制命令在同一个已准入会话上解析名称，并固定解析后的会话 ID。CLI 组合与期限由 [原生 CLI 契约](../../orchbee/docs/native-cli-query.zh-CN.md) 负责。
确定性服务测试覆盖重名、原子重命名、ID 优先、满额、生命周期清理、策略分离、过期准备、
dispose、poison 和并发抢占。wire 与 Windows 进程测试覆盖上限、schema 合规回复、
错误操作类型和修改结果未知。

## 基于终端的名称

Windows 对具有实际上报、进程识别或保留 provider 身份而没有 canonical Agent 会话的终端，
也允许赋名。同一 Names 所有者将别名绑定到精确的 Presentation、创建代次、Host epoch 和
Host terminal 身份。封闭的终端结果变体将其与 canonical 会话区分，不伪造会话 ID。
512 项容量及重名检查同时覆盖 canonical 名称、终端名称和待定启动预留；满额原子重命名仍有效。

未托管的终端名称保留其 provider 标签和可选外部会话引用。首个引用升级归属；provider 改变或
已知引用改变则清名。已证明的释放／退出、终端移除或物理代次替换也清名；观察缺失不能证明退出。
上报转换在 coordinator 提交锁仍持有时同步通知 Names 叶子，包括两次查询之间的中间变化。
一个弱引用将该组合接到现有 AgentState，不引入第二注册表、事件日志或进程观察器。
Workbench 拥有纯上报归属投影。

Names 请求先检查策略、执行一次 drain 准备，再以已有 registry → coordinator → AgentState
锁序持锁完成最终解析／修改。最终 fold 不做 I/O；修改提交后即使期限随后过去，也返回相关结果。
dispose／poison 所有者仍回答 unavailable。保留既有 canonical ID 优先、已脱离会话名称和
请求不确定性规则。扩展协议随完整匹配 cohort 交付；回退替换 cohort。托管名称持久化遵循
下述新绑定重启契约；单有终端别名不能证明托管启动。

证据须覆盖混合名称／预留冲突、满额、引用升级／替换、两次读取之间 provider A → B → A、
已证明及不确定退出、物理替换、策略／dispose、严格协议边界，以及真实 Windows
重命名／解析／列举／清名和具名终端访问。协议 schema 和 CLI 信封版本须与实际目标一致。

## 托管名称重启

私有 Windows 检查点 version 4 仅保留已完成的托管名称占用，按 Presentation 索引，内容为
名称和 provider ID。每条占用必须有同一规范化 provider 的有效官方恢复引用。占用共用
512 名称上限，名称和 presentation 均唯一。待完成启动、canonical 会话 ID、旧物理身份及
预留 ID 不构成重启权威。继续读取 version 1–3；旧二进制拒绝 version 4，不静默丢弃。
匹配 cohort 回滚必须保留兼容恢复数据，或显式转换／使用此前有效检查点；失败时不得覆盖
保留的 version 4。

正常停止在 registry → coordinator → AgentState 守卫内按精确当前成员捕获。启动只选择
已启用且去重后的 provider 恢复计划。在新的 Host 创建及引用播种完成后、Site 发布前，
Names 将选中占用恢复到新物理绑定。禁用恢复、重复抑制和物理创建缺失均不恢复名称。
恢复的托管状态只表示归属，不表示 provider 就绪，也不伪造 canonical 观察。同 provider
的 reference 改变保留托管名称；provider 变化、确认释放／退出和物理重建／移除使其失效。
改名保留托管状态。

既有 launch 协议族负责精确终端托管状态读取，物理 Agent Get 消费该状态；canonical
启动状态保持独立。读取策略先于准备，registry／coordinator 锁保持到 Names 状态 fold 结束，
避免最终成员校验与状态采样之间发生物理替换。清除恢复名称还会清除它拥有且仍匹配的持久恢复 seed，保留后续独立
上报引用与权威。coordinator 在 Names 修改前准备有界报告更新，只对匹配的成功名称释放
提交该准备结果，提交期间始终持有守卫。两个所有者仅执行 CPU 工作，提交守卫内无传输。
这避免可观察的半释放，也避免删除较新的 provider 会话。

证据须覆盖旧／新检查点严格接纳、有界唯一占用、待完成状态排除、provider／引用一致、
新物理身份、禁用／去重恢复、同 provider reference 替换、分阶段清除，以及真实 Windows
托管重启后的按名查询／输入／清除。仅编码或存储测试不能证明这条运行链路。终端语义
wait／prompt／focus 和完整 provider 就绪仍是独立义务。

## 托管启动预留

Agent 启动复用现有 `AgentState` 单元和名称所有者。私有启动预留临时占用一个名称
及一个原生 presentation 绑定；它不是 canonical 会话，也不凭空产生 idle 观察。
Windows CLI 是首个消费者。OBAP 在现有已接入 Client Session 上承载独立的封闭
启动请求族（reserve、inspect、complete、cancel）。read 策略控制查询，manage
策略控制三个修改操作；策略检查先于叶锁外现有的观察准备。

输入是类型化名称、provider ID、presentation ID、采样 Host epoch 与 Terminal
身份、非零请求身份，以及 1–300000 ms 相对 TTL。预留时将当前原生绑定与两个
采样 Host 身份比较，固定 carrier 和创建 generation，并记录匹配会话的观察
generation/sequence 基线。候选必须是存活的根会话、精确附着在固定 generation、provider
匹配，而且观察晚于基线（或基线中尚不存在）。零候选表示 pending，多个候选
明确返回歧义。这是本地观察证据，不是 provider 进程的因果认证。

名称和预留共享 512 项容量。名称、预留 presentation、已提交的会话别名各自唯一。
普通名称赋值不能占用预留名称，也不能改名其预留 presentation 上的会话；释放和
解析在适用时明确报告 pending-launch。`agent names` 分别投影已提交绑定和待定
预留。过期占用不能阻塞后续请求。预留仅属于当前运行时，不是持久会话恢复数据。

| 事件                 | 守卫与转换                                                                                                    | 可观察结果                       |
| -------------------- | ------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| Reserve              | 运行时和绑定存活；名称／presentation 空闲；容量足够；记录基线和到期点                                         | 预留身份与固定绑定；不输入终端   |
| Inspect              | 预留仍有效；按当前 canonical 状态重算候选                                                                     | 零／一候选的 pending，或明确歧义 |
| Complete             | 绑定不变；指定候选唯一；至少经过 3000 ms；fresh 的 idle／完成状态，且没有注意事项或待处理请求；候选无其他别名 | 在一个 fold 中赋名并删除预留     |
| Cancel               | 匹配有效预留                                                                                                  | 只释放该预留，绝不终止进程       |
| 到期或绑定替换／退役 | 到期，或精确原生绑定已不存在                                                                                  | 释放名称／presentation 占用      |
| Dispose              | 运行时已释放                                                                                                  | 清空别名与预留，回答 unavailable |

服务在 fold 边界采样单调时间；名称所有者将注入时间夹定为单调递增，只接收整数
毫秒。决策前和 fold 后都执行对账。现有端点 deadline/tick 所有者同时处理预留
到期，因此客户端放弃后无需每次启动另建线程。3000 ms 稳定间隔从接受预留开始；
CLI 还会从实际输入后等待。不存在续期或自动重放。

Reserve/complete/cancel 的提交点在叶锁内，序列化和发布在之后进行。修改回执
丢失表示 outcome-unknown。调用方保留预留身份和输入回执，条件允许时查询名称／
预留，否则依靠有界到期恢复。相对服务端 TTL 可能因传输延迟而晚于 CLI 截止点；
取消未确认时绝不能声称已经释放。到期释放名称，不会终止可能仍运行的 Agent。
CLI 负责采样 Shell／进程预检及一次终端输入；名称 fold 内不执行 Socket、进程
查询、外层锁获取或终端写入。

确定性状态证据覆盖并发占用、共享容量、陈旧／歧义候选、blocked／working／stale
状态、精确绑定替换、到期、取消、释放和别名干扰。线路证据覆盖封闭请求族、操作
关联和边界；Windows 进程及 ConPTY 证据覆盖字面参数、一次输入及失败回执保留。
此流程采用实现测试。这些预留规则不证明付费 provider 任务、任意
provider UI 就绪、纯进程会话发现或持久名称恢复。

## 权威语义（断言）

以下断言镜像 `packages/agent` 源码；TypeScript 源码仍是权威。每条断言都由持久语料库
和/或具名边界测试检验。

- applied 转换使 `revision` 加一；`unchanged` 与 `rejected` 结果保留输入快照且不发出
  语义事件。仅 applied 转换追加日志。
- 非 `session-observed` 事件在查找前因 `generation`/`sequence` 非非负安全整数被拒为
  `invalid-event`；然后 `session-not-found`；然后 `generation-mismatch`
  （`observation-reconnected` 且 generation 严格更大时除外）；然后 `sequence` 不大于
  存储值时为 `unchanged`。
- `session-observed` 把数值/来源/保真度/能力检查合入 `source-capability-exceeded`，
  且**先于**身份检查；完全一致的再观察（provider、执行位置、附件集合）为
  `unchanged`，冲突的为 `identity-conflict`。重复能力 token 先由 reducer 去重再校验，
  故"含重复但均有效"的能力列表仍会 applied；未知 token 在快照校验中失败为
  `invalid-event`。
- Turn 生命周期（`turn-started`、`turn-activity-changed`、`turn-finished`、
  `session-idle-observed`）、attention 请求与 `session-ended` 受已声明能力与 `live`
  生命周期门控；`turn-not-found`/`request-not-found`/`request-conflict` 遵循权威守卫
  顺序。`session-idle-observed` 仅清除被中断 turn 的 turn 域 pending 请求。
- `observation-became-stale` 作用于 `disconnected` 会话时为 `unchanged`；其余任何
  applied 事件恢复 `freshness: fresh`，但 `presentation-detached` 例外：只要仍有
  可导航附件残留就保留原存储的 freshness，分离最后一个可导航附件则强制
  `disconnected`。`presentation-visibility-changed` 没有生命周期守卫，对已结束
  会话仍会 applied。
- `session-ended` 在 turn 活跃时依次发出 `turn-outcome{interrupted}` 与
  `session-ended`；纯启发式存在（`heuristic` 来源 + `heuristic` 保真度 + 零能力）
  以零语义事件退场并强制 `disconnected` freshness。
- `observation-reconnected` 要求 generation 严格更大才进入 reconnect 处理；此后
  sequence 不大于存储值时为 `unchanged`。其携带声明按已声明能力与运行时不变量
  校验（`capability-not-declared` 然后 `invalid-event`）。
- 边界：512 会话、128 pending 请求、32 展示附件、10 能力、变更页 limit 1..=100、
  日志保留 512 条、revision 封顶 `MAX_SAFE_INTEGER`（触顶为 `capacity-exceeded`）。
- 排序按 UTF-16 码元而非 Unicode 标量或字节序——包括可含合法非 ASCII 文本的
  `presentationId` 排序键。
- 语义状态优先级：`needs-attention`（pending 请求）→ 最近结果
  （`failed`/`interrupted`/`canceled`/`succeeded`）→ `working`（活跃 turn）→ `idle`
  → `unknown`（含 `activityHint`）→ `ended`。聚合与位置投影排序时，存活会话优先于
  已结束会话，并按状态优先级、freshness、activity-hint 有无及平票时 generation
  降序排列。
- Wait 匹配：`fresh`/`stale`/`disconnected` 条件直接匹配 freshness；状态条件额外
  要求 `fresh` freshness。
- 日志读取返回 `changes`/`current`/`reset-required`/`revision-ahead`，页内 revision
  连续，`observedThroughRevision` 等于页尾 revision，`hasMore` 当且仅当游标未达头部。
  深度过期游标得到 `reset-required` 并把 `observedThroughRevision` 跳到头部；
  `afterRevision` 超出运行时 revision 得到 `revision-ahead`。非法读取输入在私有边界
  返回定型 `Err`，不是转换结果。
- `dispose` 幂等；之后所有派发均为 `runtime-disposed`，不触碰状态或日志。

## TypeScript 到 Rust 的边界映射

- 事件字段镜像无 schema 的 TypeScript 联合：原始字符串与 `f64` 数字进入 reducer，
  使守卫按相同顺序保持可达。存储的快照字段为已校验定型值（安全整数数值用 `u64`、
  枚举、身份 newtype）。
- `presentationId` 保持按 UTF-16 码元计数的 1..=256 边界。Rust `String` 无法持有孤立
  代理项，故良构性检查在构造层面成立；码元计数仍被强制。
- 会话/运行时构造点映射到 TypeScript 的 parse 点：`session-observed` 与
  `observation-reconnected` 的候选校验产生 `invalid-event`；而只会在 TypeScript
  `applied()` schema parse 中暴露的违例则以 panic 呈现，镜像其抛出行为（例如携带
  超长 `presentationId` 的 `presentation-attached`）。这是可达行为的等价，而非新的
  公共策略。
- 日志输入校验返回 `Result<_, AgentChangeReadError>`；TypeScript 对非法私有输入抛出
  `ZodError` 映射为该定型错误。
- 严格对象 presence 在私有原始边界保留：`RawPresence` 区分 `Absent`、`Null`、
  `Value` 与畸形 `Other`，使必需可空键（活跃 turn 的 `activity`、attention 请求的
  `turnId`、`activityHint`、`lastOutcome`、`parentSessionId`）保持 TypeScript 对
  "键缺失"与"显式 `null`"的区分，且 `unknown`/`idle` turn 拒绝任何残余键——包括
  值为 `null` 的键。必需且不可空的字段在定型域保持裸字段；表达其缺失是私有夹具
  解码器的职责，未来的公共 ingress 是独立分层，拥有自己的畸形输入策略。
- 确定性：模块内不存在环境时间、随机性、IO 或全局状态；freshness 转换是显式事件
  输入。

## 纯 ingress 移植（join + hook 翻译器）

位于同一二进制私有边界之后，对托管路径生产激活。`join.rs` 精确保留 Desktop
`AgentObservationJoinStateMachine` 语义：`pendingHints` 以 correlation id 为键、
`adapterId\0eventId` 事件键、仅在 `receiveHint`/`admitEvent`/`settle` 开头做惰性过期
（注册、退役与 `reset` 不取时钟）、全 correlation 匹配即 `apply-observation`、settle 时
部分匹配应用、已应用事件的迟到 hint 立即 `attach-presentations`、代际退役时静默移除已
匹配附件、hidden 优先否则 `unknown` 的可见度合并、未知代际 hint 直接丢弃（held-hint 队列
明确推迟）。`hook_ingress.rs` 移植 `AgentHookObservationIngress` 翻译器：
带命名空间的会话绑定、adapter 门控的 18 事件翻译、execution/session/turn/request/event
标识分配顺序、turn-key 围栏与采纳、请求与子会话映射（各 128、会话 128）、结束后绑定保
留，以及经注入的 `&mut AgentRuntime` 的按序规范派发——翻译器不持有运行时，无第二
reducer、无恒 applied 假汇。

唯一有意的语义差量是原生准入边界 `pending + recent <= 4096`（已接受的有界资源策略）：
既有键的重放不占新槽、applied 过期把占用从 pending 转入 recent 而总量不变、未应用与
recent 过期各自释放槽位、联合容量满时新准入被拒且不做任何规范变异。字节上限不是堆
边界——字符串与容器开销仍在。

核心只发出 `schedule-settlement`/`cancel-settlement` 效果；端口内不存在定时器或套接字
IO。`hook_ingress/settlement_queue.rs` 中的 `SettlementQueue` 计时原语与生产 `Instant` 漏斗
共享（admission、`tick` 与 native-effects 漏斗头都经服务唯一 `clock_origin` 同步
调度器）。Fixture JSON 发射器、按步 `DispatchTrace` 派发记录器与 `FixtureIngress`
所有者对位于 `tests/conformance_support.rs`；它们与 f64 域语料时钟推进入口均保持
`#[cfg(test)]`。
translator 核心对注入的 `runtime` 原样调用
`dispatch` 并写入 trace——核心不依赖任何 fixture JSON。一致性线束驱动独立的调
度时钟：`settleWait` 步推进调度时间以触发到期的 settle 效果，而注入 `now` 保持该步记
录的 `t`——与语料的双时钟记录一致：记录用 `f64` 缝无法触达生产 `Instant`
sync，`Instant` 绝不跨入翻译器。

夹具 `schemas/agent/v1/agent-observation-join-conformance-v1.json` 与
`schemas/agent/v1/agent-hook-observation-conformance-v1.json` 是蒸馏后的回放载荷加期望
值，附生成所用 Desktop 源文件的 SHA-256 指纹；不含生成器路径、运行历史与主机细节。来源
与刷新契约由本文档归属，不归 `schemas/agent/README.md`（其仅覆盖规范语料）：再生成时
重新运行已记录的外部线束执行真实 Desktop 源码，并在替换前重新核验指纹。

## 一致性证据与来源

`schemas/agent/v1/agent-canonical-conformance-v1.json` 是持久执行语料：由运行权威
TypeScript 源码产生的 29 个向量 / 665 条已记录转换，另含逐向量日志读取、投影与具名
quirk。它是等价性**证据**，不是公共 schema 权威，并始终保持私有夹具格式。夹具的
`provenance.sourceFingerprints` 记录生成时 `packages/agent/src` 每个文件的 SHA-256；
再生成由 `packages/agent` 负责（`conformance:generate`，漂移检测用 `--check`），并
导入所属 TypeScript 构建产物——绝不 vendored 源码拷贝。再生成契约见
[`schemas/agent/README.md`](../../../schemas/agent/README.zh-CN.md)。

其余具名边界用例在 `src/agent/tests/targeted.rs` 中对同一 TypeScript 源码的执行记录结果
断言：守卫优先级、结束后可见性、容量与 `MAX_SAFE_INTEGER` 边界、UTF-16 长度/排序
（含已执行的精确 256/257 码元边界）、日志输入错误、投影平票、stale 会话部分分离与
最后可导航附件分离，以及上述 presence 区分（`unknown`/`idle` turn 残余键、必需可空键
缺失与显式 `null`、缺失的 `parentSessionId`/`turnId`/`activityHint`/`lastOutcome`/
`turn` 声明）。第二组具名用例位于 `src/agent/tests/oracle.rs`，断言更多已记录结果：聚合
attention 子集 freshness 折叠、provider 去重/排序、同优先级 freshness 与
activity-hint 平票、跨 BMP/astral 边界的 UTF-16 `presentationId` 排序、按位置的
navigable/freshness/status 偏好及非获胜 generation 的分组排除、非法种子快照、
非零头空日志的 `reset-required`，以及双重 dispose 幂等。全部一致性线束仅在
`#[cfg(test)]` 内编译，现以 Server 二进制 crate 测试运行。

## 原生观察叶子

`agent/native_observation.rs` 是原生观察线路的私有纯归约器，经托管 drain 路径
生产激活。它持有暂挂提示（至多 1024 条记录、每条原始载荷至多 4096 字节）、唯一的当前载体——
从权威 `TerminalOperationAuthorization` 逐字节复制的 CNP `WorkerId` + `WorkerGeneration`
对——及其标量连续水位线 `W`，以及当前已提交的绑定 `presentation -> (carrier, host)`。
该 fold 为纯函数：它对类型化事实分类并返回自持的效果数据
（`route-hint`、`register-presentation`、`retire-presentation`）；从不接触
`AgentRuntime`、翻译器、IO、注册表或协调器。`AgentState` 在同一单元内持有该叶子，
并在同一个 `with_state` 守卫内应用每一批返回的效果，因此 join 栅栏与提示重放绝不会
比产生它们的 fold 活得更久，整个单元共享既有的永久毒化边界。

序列语义遵循选定的标量 W 契约：`seq <= W` 去重并 ACK 当前 W；`seq == W+1` 先分类再原子
提交 W（无效载荷与容量拒绝同样消费该序号）；`seq > W+1` 为洞并 ACK 当前 W；gap 事实先把
前缀裁剪到 `W+1`，再必须连续覆盖——完全覆盖的 gap 是去重等价的空操作 ACK。陈旧或不存在
的载体不铸 ACK；没有墓碑，也没有 Worker 连接代际之外的 drain epoch。暂挂记录在首次分类
十秒后过期；每个入口点先清扫，`reconcile_complete_bindings` 也不例外，它还会在任何变更
之前校验整个快照（上界、presentation 唯一、身份一一对应），歧义快照被整体拒绝而不产生
部分状态。载体退役为恰好属于该载体的每个绑定返回一个 `retire-presentation` 效果，使
join 栅栏在同一 fold 内落下。

翻译器为本切片新增一个缝：`register_presentation_binding`，因为
`environment_overlay_for` 在未绑定 socket 路径时提前返回，会完全跳过注册；既有 overlay
行为不变。载体生命周期缝 `native_admit_carrier` 与 `native_retire_carrier` 为生产：
Worker 注册表在活动载体记录可见之前、于注册守卫内准入叶子，并在载体释放前于同一
守卫内退役叶子，携带已准入的确切 `WorkerId` + `WorkerGeneration` 身份。逐事件缝
（`native_apply_event`、`native_apply_gap`、`native_reconcile_bindings`、
`native_sweep`）保持 `#[cfg(test)]` 仅测试：生产路径仅经下述整页 fold 应用事实，计时器归属
保持在托管端点 poll 线程内。

生产读准备形态是 `AgentService::execute_prepared`——由必需的调用方自有快照
provider 驱动的共享执行器，不存在缺缝回落。
`local_terminal_service/agent_drain_read.rs` 泵正是该 provider：
`LocalTerminalService::prepare_agent_drain_read` 在策略准入之后、首个准备锁之前
一次性铸造私有四秒窗口，仅以私有 25 ms 间隔重试 `WorkerRequestError::Backpressure`
准入，并在终态 fold 全程持有 registry -> coordinator 守卫。`recv_deadline` 耗尽对泵读
是终态并映射为 `PreparedUnavailable`；空页或无进展页绝不重试，也不存在捏造的
last-good 快照。

drain 页应用/终态栅栏服务缝与只读叶子访问器 `NativeObservation::current_carrier`
为生产：`current_carrier` 暴露既有的 `(CarrierIdentity, W)`
元组，不引入新的叶子状态、token、epoch 或第二个水位。一个私有适配器在 fold 之前
从 typed `AgentDrainReply` 逐条提升全部域事实：每个 `NativeEvent` 都以本次读取自己
请求的载体构造，绝不使用调用方提供的载体，因此任意或混合载体的事实向量在类型上
不可表示；typed 回复构造（连续有界覆盖、逐条负载与 gap 边界）仍是唯一校验，构造
失败在任何 fold 之前即拒绝——token echo、floor 与 ACK 相关性仍是未来调用方的前置
条件，而非叶子状态。`native_apply_drain_page` 在同一个 `with_state` 守卫内先检查
完整 `CarrierIdentity`、再应用任何条目，应用全部事实并在同一守卫内消费其效果，随后
读取活的 `W`——空页或全去重页同样返回该值；载体缺失、退役或被替换则返回
`CarrierMismatch` 且零变更。退役或替换只能发生在 fold 之前或之后，绝不能在 fold
内部。`native_fenced_final_snapshot` 保持同样的单 fold 纪律：按既定因果序先做活
`(identity, W)` 栅栏对照钉住的屏障（或 `NoCarrier` 期望的叶子缺席检查），再做销毁
检查、原子整份事实 reconcile、效果消费与规范克隆——陈旧的本地水位永远不能为另一个
载体的快照作证。`NoCarrier` 仅当 registry 的活动载体确实缺席时才是读取的真无值；
活载体证明失败映射到既有的 `RuntimeUnavailable`，绝不静默改标为 `NoCarrier` 提供
服务。`local_terminal_service/agent_drain_read.rs` 泵在生产中拥有窗口、节奏与传输
接线；载体生命周期经托管 Worker 的真实注册表准入运转。

## 托管观察端点

`local_site_service/agent_observation_endpoint.rs` 是为
托管 hook 观察 ingress 供给的生产私有 Unix 传输。`local_site_service::managed`
在站点运行时目录内绑定每站点一个
socket（`agent-observation-v3.sock`，模式 `0600`），在绑定时记录该路径的文件系统身份，
并运行单个 poll 线程，多路复用监听器、停止管道与至多 `MAX_ACTIVE_CONNECTIONS` 条已
接受的 nonblocking 流；`set_nonblocking` 失败的流即弃，accept 排空每轮循环受同一私有
预算约束，因此持续积压的连接无法饿死停止管道、结算 tick 或存活连接。

该传输不拥有 canonical 语义：strict-envelope 正文经 `admit_envelope` 缝折叠进唯一
共享的 `AgentService` cell，30 字节 `ACCEPTED_ENVELOPE_ACK` 仅表示 join-admission 簿记
接受了信封——绝非 canonical 应用，因此被接受但未关联的信封被诚实地静默消费而不产生
会话，零关联信封在解析处即被拒。读取、admission 与 ACK 写共享每条已接受流 500 ms 的
绝对 deadline；超大正文与 admission 后 deadline 过期在无 ACK 下关闭流，销毁状态在 fold
内检查，因此 admit 诚实关闭而非伪造确认。

时钟归属在 cell 边界保持单向。端点把 `Instant` 值传入服务缝，后者经唯一 `clock_origin`
换算为翻译器毫秒，并在每个可能调度的 fold 入口同步结算调度器——admission、`tick` 与
native-effects 漏斗头——使外部调用方调度的定时器（native event-before-hint fold）汇入
同一单调调度器。`Instant` 绝不跨入翻译器；corpus 的双钟契约因记录用 `f64` 缝无法触达
sync 而得以保持。poll 活性是私有 25 ms 上限：poll 超时取连接 deadline、下一个结算
deadline 与该上限的最小值，因此无连接时调度的定时器在一次协作唤醒内结算——无需手动
tick 或第二条连接。该上限约束陈旧度，不构成延迟保证。

失败归属有意切分：逐连接与逐 fold 错误仅销毁该连接（`next_deadline` 错误不产生
deadline，`tick` 错误跳过，admission 错误关闭当前流）；non-`WouldBlock` 监听器 accept
失败与永久 poll 错误终止端点线程，`EINTR` 重试。`BoundEndpoint` 拥有监听
器、停止管道、路径与已记录身份，直到 `start` 把监听器移入 poll 线程并在 spawn 成功后解除
身份武装；`AgentObservationEndpoint::drop` 写停止字节、无条件 join 线程，随后仅在其
当前元数据仍匹配绑定时记录的身份时解除路径链接——被替换或外来路径绝不被删除。

生产激活对已接受的托管本地 Site 路径生效：`local_site_service::managed` 创建或
取得运行时目录，在发布 Site 就绪之前绑定本端点，绑定或端点初始化失败时诚实失败
`site start`——不存在静默的 Agent 不可用模式。托管路径经
`open_observing(policy, socket_path)` 恰好构造一次 canonical `AgentService`——
socket 路径在构造期即绑定进翻译器，随后 `Arc` 才被共享——并把同一 `Arc` 交给
端点、Worker 准入（`register_carrier_with_agents` 在注册守卫内准入原生叶子）与
客户端读取。Worker 从既有 `--local-site <runtime-dir>` 参数推导 socket 路径，
校验 socket 类型与同用户归属，并经既有环境覆盖注入
`ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`；不新增 wire 字段、公共设置或 Worker 对
Server 代码的依赖，远端（非托管）路径保持既有行为。被替换或外来 socket 路径在
清理时绝不解除链接——drop 先停止并 join poll 线程，再做身份门控解除链接。

## 明确超出范围

持久化/持久状态、远端（非托管）Agent 观察与
任何远端 Agent 扩展、wait/watch、观察 fold 之外的规范模型变更、
规范会话删除、TUI 消费者、wire 帧/编解码归属
（`orchbee-agent-observation-protocol` crate）、游标秘密及其轮换策略
（`agent_service`）、CLI 语法、渲染与本地化、TypeScript 行为变更，以及监听器/
wait 并发。面向客户端的读取预算不变；私有泵窗口是服务器内部常量，不是产品设置
或对客户端的承诺。新的语义、schema、依赖或归属问题一律回到 Root，不在此裁决。

names 家族还接受 `{kind:"presentation", presentationId}`。解析、分配与释放在同一次 Agent fold 内检查当前 canonical 快照。只匹配存活且具有对应可导航附件的会话，包括隐藏附件；附件 generation 仍由 canonical 模型管理。零匹配拒绝为 not-found（变更为 session-not-found），多个会话拒绝为 ambiguous-target，一个会话以 resolvedBy presentation 解析。重复附件不会把一个会话计数两次。拒绝保持名称绑定不变，结果仍受 1,024 字节上限约束。Workbench Panel ID 与主 presentation 选择属于 CLI 组合，不进入 Agent 状态单元，也不在其锁内读取 socket。相同 cohort 的交付约束新增 wire 词汇；回滚替换 cohort，无持久状态迁移。

## 原生有界变更读取

原生 `agent.session.changes` 家族将现有 canonical 日志提供给 Client 提示状态、等待及增量观察。日志保留 512 个已应用转换，不引入第二份事件存储或 Agent 写入者。每条记录包含 revision、十四种 canonical 转换之一，以及日志的精确会话投影（生命周期、观察、父会话、待处理数量、provider、会话 ID 和语义状态）。附件与执行位置需要一致的会话读取，不编造历史字段。

读取准入先于在 Agent 单元外执行恰好一次调用者拥有的 drain 准备。拒绝读取不运行该 provider。准备失败或 panic 返回关联的 runtime-unavailable。准备成功后，在 Agent 单元内一起读取日志与头部 revision；可以包含更晚的 canonical 事件。已释放或毒化的运行时拒绝读取。Agent 锁内不执行 I/O。

| 输入/状态                                 | 结果与义务                                                                            |
| ----------------------------------------- | ------------------------------------------------------------------------------------- |
| 省略 cursor                               | 当前头部检查点；不声称已交付更早的变更。                                              |
| 位于头部的真实 cursor                     | Current，保持相同已观察 revision，并返回可复用 cursor。                               |
| 仍在保留范围内的真实 cursor               | 1–100 条连续变更，cursor 位于最后已交付 revision，只有低于采样头部时 hasMore 才为真。 |
| 早于保留范围的真实 cursor                 | Reset-required 与新头部检查点；消费者必须重读基线并明确连续性丢失。                   |
| 无效、其他启动实例、错误操作或超前 cursor | Cursor-invalid；消费者必须显式重建基线，不能静默视为 current。                        |
| 禁止读取、准备失败、运行时已释放/毒化     | 关联拒绝/不可用；不声称交付进度。                                                     |

变更 token 复用 cursor 载荷布局，operation 为 2，flags/target/position 为零，携带启动实例与已观察 revision。其 HMAC 域为 `orchbee.agent.changes.cursor.v1\0`，与 list token 分离。先校验形状、规范编码和常量时间 MAC，再检查语义。现有启动期权威拥有两个域并随服务释放；token 不跨重启。省略 limit 默认 50，接受 1–100；分页前不筛选。协议所有者在分配前将每页限制为 1 MiB，并验证正数连续 revision、尾部/头部/hasMore 一致性、封闭转换种类及会话边界。

消费者先建立检查点，再获取一致会话基线并协调后续变更。Reset 或 cursor-invalid 使连续性及依赖它的确认失效。Client 本地 seen/unseen 不修改 canonical 状态。此扩展要求匹配的开发 cohort；替换 cohort 即可回滚，无持久状态迁移。初始消费者为原生 Windows 传输。协议、游标域/重启/保留边界、canonical corpus 和准备读取失败测试覆盖此边界；实际展示及最终 CLI 验收仍由消费者负责。

## 托管启动状态

只读 `agent.launch.status` 在读取策略准入和 Agent 单元外的一次准备后检查精确 Session。它一起采样规范修订和名称所有者的启动状态：非托管；包含原有精确预留及当前阻塞标志的待启动；或者包含已接受 Complete 的预留身份、provider 和物理绑定的活动状态。Status 不提交、不续期、不重放、不启动。

只有已接受的 launch Complete 才创建活动状态。手动命名不能证明启动就绪；重命名保留已有托管身份。活动记录受已提交名称容量约束，在显式清名、规范结束／移除／销毁、provider 不符或精确原生绑定替换／退役时移除。待启动状态保留原 TTL。活动表示启动已完成，与之后的工作／阻塞／新鲜度、OS 焦点及输入所有权相互独立。现有 CLI 阻塞启动取消策略仍是独立工作流义务。已完成归属也可通过托管名称重启契约恢复为
物理名称，但不恢复 canonical 会话，也不重复声明原先的就绪事实。

冻结语料保留原始源码指纹。新增 Kilo 注册表边界由 `kilo_hook_boundary_matches_opencode_with_distinct_provider` 和 Desktop 适配器注册表测试单独检查；不将历史语料改标为已重新生成。

Cursor 增加原生和 Desktop 边界的直接证据，仅支持 session-identified（及旧版 session-started）身份；历史语料指纹不变。它授予 sessionLifecycle，不声明回合、活动或权限能力。

Qoder 仅准入具有 sessionLifecycle 的 session-identified。原生注册表和精确入口的直接测试扩展仅识别身份的证据，不修改历史语料。

Antigravity CLI 复用仅识别身份的边界证据，将适配器 antigravity-cli 映射为规范 provider antigravity。PreInvocation 不建立回合状态或就绪事实。

Mastracode 通过封闭观察注册表授予会话／回合生命周期、活动、批准及子 Agent 能力。直接关联入口证据覆盖身份、一个带标识的运行、提示请求／解除及完成；历史语料指纹不变。

Droid 加入仅识别身份的注册表和原生入口见证，在 SessionStart 时保留未知或已有回合状态。

Kimi 仅准入 session-identified 与 session-ended。重复身份保留规范轮次状态；Stop 及其他不支持的生命周期事件仍被拒绝。

Letta 仅准入 session-identified，保留未知或现有轮次状态。默认对话身份包含 Agent ID；内容及推测的完成状态不会进入规范状态。

Devin 六组 hook 仅接收 session-identified。身份刷新保留未知或已有轮次状态，不解决权限请求，也不推断完成。

Hermes 仅准入 session-identified。会话启动／重置和 CLI 恢复观察保留规范轮次状态，不制造空闲或完成。
