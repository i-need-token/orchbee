# orchbee-agent-observation-protocol

[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json)
所治理的 version-1 Agent 观察契约的独立 Rust 绑定。本 crate 提供有界类型化 wire DTO、严格
语义校验、OBAP 帧编解码器，以及不透明的 list 分页游标 wire 形状。实际消费方是原生
`orchbee` CLI 与 `orchbee-server` Agent 服务——消费 read、names 和 launch reservation 三个家族。

## 范围

- 请求：封闭的 `AgentRequest` 联合——`agent.session.list/get/explain` 首读家族（携带非零
  32 位十六进制 `requestId` 的扁平封闭对象；`includeEnded`/`limit` 默认值 `false`/`50`
  在解析时应用），外加 `AgentNamesRequest` 家族（经 `AgentSessionSelector` 的
  `resolve`/`names.list`/`name.assign`/`name.release`），以及 `AgentLaunchRequest`
  （`reserve`/`inspect`/`complete`/`cancel`）。
- 结果：封闭的 `AgentResult` 联合——`{kind, requestId, result:{outcome}}` 下的 read
  `outcome` 联合（`listed`/`found`/`rejected`/`unavailable`）；`runtime-unavailable` 是
  v1 唯一的不可用原因——外加 names 与 launch 结果信封。
- 实体与 canonical Agent 模型逐字一致；唯一的 wire 层收窄是原生 `presentationId` 轮廓
  （32 位小写十六进制，非零）。
- 帧格式：12 字节大端 `OBAP` 帧头，version `1`，请求 kind 为奇数 `1..=21`（read
  `1/3/5`，names `7/9/11/13`，launch `15/17/19/21`），结果为对应偶数 kind `2..=22`；按 kind 与方向的上限在
  分配之前执行；帧头/正文 `kind` 一致性在解析后核验；小型 `rejected`/`unavailable` 体在
  类型化构造后限 1,024 字节。
- 游标：`v1.<59>.<43>` 不透明令牌形状与 44 字节大端负载布局
  （`version|operation|flags|instance|target|revision|position`）。标签的创建与验证是
  Server 的私有职责，绝不进入本 crate。

## 交付状态

三个家族均已接通：原生 Agent 读取、`agent names/resolve/rename` 与 Windows `agent start` 通过 Server 的
`application_session` 解复用和 `AgentService` 执行。已发布 schema 覆盖 kind 1–22。
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
