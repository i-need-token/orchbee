# orchbee-agent-observation-protocol

[`schemas/agent-observation/v1`](../../schemas/agent-observation/v1/agent-observation.schema.json)
所治理的 version-1 Agent 观察读取契约的独立 Rust 绑定。本 crate 提供有界类型化 wire DTO、严格
语义校验、OBAP 帧编解码器，以及不透明的 list 分页游标 wire 形状。实际消费方是原生
`orchbee` CLI 与 `orchbee-server` 读取服务。

## 范围

- 请求：仅 `agent.session.list/get/explain` 首读切片——携带非零 32 位十六进制 `requestId`
  的扁平封闭对象；`includeEnded`/`limit` 默认值（`false`/`50`）在解析时应用。
- 结果：`{kind, requestId, result:{outcome}}` 下的封闭 `outcome` 联合
  （`listed`/`found`/`rejected`/`unavailable`）；`runtime-unavailable` 是 v1 唯一的不可用原
  因——在真实属主出现之前不保留任何 draining 变体。
- 实体与 canonical Agent 模型逐字一致；唯一的 wire 层收窄是原生 `presentationId` 轮廓
  （32 位小写十六进制，非零）。
- 帧格式：12 字节大端 `OBAP` 帧头，version `1`，kind `1..=6`；按 kind 与方向的上限在分配
  之前执行；帧头/正文 `kind` 一致性在解析后核验；小型 `rejected`/`unavailable` 体在类型化
  构造后限 1,024 字节。
- 游标：`v1.<59>.<43>` 不透明令牌形状与 44 字节大端负载布局
  （`version|operation|flags|instance|target|revision|position`）。标签的创建与验证是
  Server 的私有职责，绝不进入本 crate。

超出 JSON Schema 的语义不变量（capability 与 request 标识唯一、attachment 键唯一、每个
presentation 至多一个 navigable 代际、activity hint 前置条件、已结束会话为空、turn 作用域
request 隶属、集合上限）由 `contract::validate_*` 在每条解析与序列化路径上执行。这些是
wire 层严格检查，不是 canonical superRefinement 证明。

## 非目标

canonical Agent 状态、会话生命周期、ingress、变更、wait 或变更流、持久化、socket 或超时时
生命周期、认证、授权、游标密钥、期限、渲染、CLI 语法与平台效应。

## 验证

`cargo test -p orchbee-agent-observation-protocol` 覆盖类型化往返、帧头与方向拒绝、按 kind
长度上限、严格 JSON 封闭性、语义不变量、集合上限、safe-integer 边界、游标形状与负载布
局，以及对照 wire 上限断言的推导最坏情况编码尺寸。
