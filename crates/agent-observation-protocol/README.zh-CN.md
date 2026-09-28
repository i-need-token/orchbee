# orchbee-agent-observation-protocol

[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json)
所治理的 version-1 Agent 观察契约的独立 Rust 绑定。本 crate 提供有界类型化 wire DTO、严格
语义校验、OBAP 帧编解码器，以及不透明的 list/change 游标 wire 形状。实际消费方是原生
`orchbee` CLI 与 `orchbee-server` Agent 服务——消费 read、names 和 launch reservation 家族，并为原生观察者提供有界 changes。

## 范围

- 请求：封闭的 `AgentRequest` 联合——`agent.session.list/get/explain` 首读家族（携带非零
  32 位十六进制 `requestId` 的扁平封闭对象；`includeEnded`/`limit` 默认值 `false`/`50`
  在解析时应用），外加 `AgentNamesRequest` 家族（经 `AgentSessionSelector` 的
  `resolve`/`names.list`/`name.assign`/`name.release`），以及 `AgentLaunchRequest`
  （`reserve`/`inspect`/`complete`/`cancel`/`status`）与 `AgentChangesRequest`（`agent.session.changes`）。
- 结果：封闭的 `AgentResult` 联合——`{kind, requestId, result:{outcome}}` 下的 read
  `outcome` 联合（`listed`/`found`/`rejected`/`unavailable`）；`runtime-unavailable` 是
  v1 唯一的不可用原因——外加 names 与 launch 结果信封。
- 实体与 canonical Agent 模型逐字一致；唯一的 wire 层收窄是原生 `presentationId` 轮廓
  （32 位小写十六进制，非零）。
- 帧格式：12 字节大端 `OBAP` 帧头，version `1`，请求 kind 为奇数 `1..=23`（read
  `1/3/5`，names `7/9/11/13`，launch `15/17/19/21`，changes `23`），结果为对应偶数 kind `2..=24`；按 kind 与方向的上限在
  分配之前执行；帧头/正文 `kind` 一致性在解析后核验；小型 `rejected`/`unavailable` 体在
  类型化构造后限 1,024 字节。
- 游标：`v1.<59>.<43>` 不透明令牌形状与 44 字节大端负载布局
  （`version|operation|flags|instance|target|revision|position`）。标签的创建与验证是
  Server 的私有职责，绝不进入本 crate。

## 交付状态

三个家族均已接通：原生 Agent 读取、`agent names/resolve/rename` 与 Windows `agent start` 通过 Server 的
`application_session` 解复用和 `AgentService` 执行。已发布 schema 覆盖 kind 1–26。
名称是由 Server 状态单元持有的运行时别名；协议只拥有有界 DTO 与校验。
get/explain/wait 先在同一已接纳会话上解析名称，再按固定会话 ID 读取或轮询。CLI 拥有组合与预算。

名称列表包含 live `bindings` 和独立的 `pending` 启动预留，两者合计最多 512 条。
各列表按唯一名称排序；已绑定会话身份、预留 ID 和预留 presentation 各自唯一，名称不交叉。
name-in-use 拒绝必须标明占用者；其他赋名拒绝携带 null。结果 kind 8/12/14
在分配或读取正文前拒绝超过 1,024 字节的正文；kind 10 维持 256 KiB 的有界列表上限。
canonical 读取实体的语义不变量仍在每次解析与序列化时执行，独立于 JSON Schema。

## 非目标

canonical Agent 状态、会话生命周期、ingress、变更、wait 或变更流、持久化、socket 或超时时
生命周期、认证、授权、游标密钥、期限、渲染、CLI 语法与平台效应。

## 验证

`cargo test -p orchbee-agent-observation-protocol` 覆盖类型化往返、帧头与方向拒绝、按 kind
长度上限、严格 JSON 封闭性、语义不变量、集合上限、safe-integer 边界、游标形状与负载布
局，以及对照 wire 上限断言的推导最坏情况编码尺寸。

names 家族还接受 `{kind:"presentation", presentationId}`。解析、分配与释放在同一次 Agent fold 内检查当前 canonical 快照。只匹配存活且具有对应可导航附件的会话，包括隐藏附件；附件 generation 仍由 canonical 模型管理。零匹配拒绝为 not-found（变更为 session-not-found），多个会话拒绝为 ambiguous-target，一个会话以 resolvedBy presentation 解析。重复附件不会把一个会话计数两次。拒绝保持名称绑定不变，结果仍受 1,024 字节上限约束。Workbench Panel ID 与主 presentation 选择属于 CLI 组合，不进入 Agent 状态单元，也不在其锁内读取 socket。相同 cohort 的交付约束新增 wire 词汇；回滚替换 cohort，无持久状态迁移。

Launch 结果在分配前限为 1,024 字节。预留固定原生 presentation、creation generation、
Host epoch 和物理终端。Reserve 请求 ID 同时标识预留。TTL 为 1–300000 毫秒，
settleRemainingMs 为 0–3000。Inspect 的 candidateSessionId 必须出现且可为 null。
成功结果必须匹配对应操作。名称 resolve/assign/release 显式拒绝 launch-pending。
期限、就绪、观察基线、精确绑定核对和名称原子提交仍由 Server 的
[canonical 模型](../../apps/orchbee-server/docs/agent-canonical-model.zh-CN.md)负责。

## 有界 changes 契约

Changes 使用请求/结果 kind 23/24 与惯用的关联 outcome 信封。省略 cursor 获得当前头部检查点；显式 null 无效。分页 limit 默认 50，接受 1–100。结果为 changes/current/reset-required/rejected/unavailable。页内 revision 为正且连续，尾部等于 observedThroughRevision，采样的 runtimeRevision 不小于尾部，只有尾部低于头部时 hasMore 才为真。每条记录包含 canonical 日志的会话投影，不编造附件或执行位置。结果在分配前限为 1 MiB；非分页结果仍最多 1,024 字节。

`AgentChangesCursor` 与 list token 共用有界形状，但使用 operation 2 和独立 Server HMAC 域。认证、启动身份及保留范围校验由 Server 负责。超出保留范围的 token 返回 reset-required 与新头部检查点；无效或先前启动实例的 token 返回 cursor-invalid。两者都不代表连续观察。[Server 模型](../../apps/orchbee-server/docs/agent-canonical-model.zh-CN.md#原生有界变更读取) 拥有准备、快照/日志一致性及恢复语义。此扩展要求匹配的 cohort 二进制，无持久迁移。原生消费流程及最终验收与编解码器覆盖分开处理。

## 托管启动状态

只读 launch Status 使用请求／结果 kind 25/26 及现有 16 KiB 请求／1,024 字节结果上限。status 结果携带精确 sessionId、规范 revision 和闭合的 management 阶段：unmanaged；含已验证预留及 blocked 布尔值的 pending；或含 reservationId、providerId 及精确启动绑定的 active。只有 Status 结果允许此 outcome。会话存在性与生命周期、已完成启动的保留及读取授权属于 Server 名称所有者。消费者必须关联 Session，并区分已完成启动与当前提示就绪。此原生增量扩展需要匹配的 cohort 二进制。


## 上报输入值

`report_values` 负责原生 Agent 上报的规范化与已校验输入值，不持有上报状态、权限、帧编号、
期限、provider 执行或 I/O。原生 CLI 消费其 provider 目录和上报标签规范化器；原生进程名称
预处理仍是独立的 CLI 策略。状态／会话／清除／释放处理器及查询／TUI 消费方是独立的集成
义务，不能因这些输入值已实现而视为交付。

兼容参考为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 Windows 上报标签和
`agent_resume` 行为。已识别标签采用上报形式（`claude`、`agy`、`copilot`、`qwen`），
与原生规范 provider ID 独立。未知标签保留裁剪后的原文；规范化不会移除内部控制字符或截断
身份。元数据 CLI 在扩展通信契约前仍保留现有的更严格守卫上限。目录的精确查找不移除后缀、
裁剪、转小写，也不识别单独的 `agent`；这些输入策略由调用方拥有。上报规范化先移除一个
已知后缀，再取最后一个非空路径分量。同一目录也服务原生进程查找，不改变该调用方现有的
后缀或单独 `agent` 行为。

只有精确的 18 组官方来源／Agent 配对可接纳 provider 引用。来源名称是不可信数据，不是
认证。六组使用完整生命周期上报，四组仅提供身份，其余八组使用原生状态观察。其他配对为
自定义上报，不能声称官方可恢复性。ID 必须非空、不含控制字符且至多 512 个 UTF-8 字节；
路径采用同样的文本规则，上限为 4096 字节，并必须通过调用方提供的绝对路径能力检查。
不裁剪文本、不访问文件系统，也不规范化路径。pi/omp 优先采用有效路径，失败时回退到有效
ID；其他官方配对只使用 ID。无效或不支持的引用返回缺失，匹配上报的无操作输入语义。
引用字段私有，不可信反序列化必须经相同的受检构造器及目标平台路径能力；仅按文本反序列化
不能证明路径有效。

Windows 恢复规划器针对有效来源／Agent／引用组合返回字面参数向量，处理各 provider 的
恢复语法，包括 `cursor-agent.cmd`、omp 和 Copilot 的等号参数，以及 Letta 的
`default:AGENT` 形式。它不执行参数向量、不解释 shell，也不把 Orchbee 规范 Session ID
视为 provider 身份。八个已识别启动来源为 startup/resume/clear/compact/branch/new/fork/select；
未知输入规范化为缺失。上报序号以封闭的十进制字符串承载完整无符号 64 位范围，与 JavaScript
安全整数范围的规范 Agent 修订号独立。序号排序和归属转换仍由 Server 负责。原生状态保留是
独立谓词：Qwen 保留原生状态，同时仅接纳身份上报。同样，`branch` 是有效启动输入，但不具备
完整生命周期重新锚定资格。provider／启动来源的会话替换资格是独立谓词，不能替代 Server
对当前归属、进程、代际及序号的检查。

决策测试覆盖所有官方配对与恢复向量、外部来源／Agent 配对、仅身份／完整生命周期区别、
ID／路径优先级及字节／控制字符边界、无效路径回退、未知启动来源和完整 u64 的 Serde 拒绝。
原生 CLI 的别名／自定义标签和进程识别检查保护已集成消费方。这些检查只证明规范化和规划，
不证明 provider 会话实际运行或完整上报对齐。

封闭的 provider 引用种类枚举还为 Workbench 上报结果绑定解码 `id`／`path`。这不会给
`AgentProviderSessionReference` 添加无上下文反序列化；私有已校验值仍需使用构造器和调用方
提供的平台路径谓词。
