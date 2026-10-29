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

Windows Names 组合还接纳精确终端选择器与有类型的 `nameTarget`：canonical 会话或终端启动
绑定（presentation、创建 generation、Host epoch、终端 ID）。`resolved-target`、
`assigned-target`、`unchanged-target`、`released-target`、`listed-targets` 保留身份区别。
混合列表共用 512 条上限、全局唯一名称及排序规则；物理 presentation 占用不能与待完成
启动重叠。`rejected-target` 只表示 name-in-use，并要求非空占用目标。小结果仍限 1 KiB；
两个列表变体都使用既有 256 KiB 上限。名称／provider 归属核对与受锁保护的组合由 Server
负责，不生成虚假的 canonical ID。

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
settleRemainingMs 为 0–3000。Inspect 的 candidateSessionId 必须出现且可为 null。Reserved 必须携带非零
runToken；Inspect 的 pending 结果必须携带 receipt，其 phase 为 awaiting、started，
或带布尔值 succeeded 的 finished。
成功结果必须匹配对应操作。名称 resolve/assign/release 显式拒绝 launch-pending。
期限、就绪、观察基线、精确绑定核对和名称原子提交仍由 Server 的
[canonical 模型](../../apps/orchbee-server/docs/agent-canonical-model.zh-CN.md)负责。

## 有界 changes 契约

Changes 使用请求/结果 kind 23/24 与惯用的关联 outcome 信封。省略 cursor 获得当前头部检查点；显式 null 无效。分页 limit 默认 50，接受 1–100。结果为 changes/current/reset-required/rejected/unavailable。页内 revision 为正且连续，尾部等于 observedThroughRevision，采样的 runtimeRevision 不小于尾部，只有尾部低于头部时 hasMore 才为真。每条记录包含 canonical 日志的会话投影，不编造附件或执行位置。结果在分配前限为 1 MiB；非分页结果仍最多 1,024 字节。

`AgentChangesCursor` 与 list token 共用有界形状，但使用 operation 2 和独立 Server HMAC 域。认证、启动身份及保留范围校验由 Server 负责。超出保留范围的 token 返回 reset-required 与新头部检查点；无效或先前启动实例的 token 返回 cursor-invalid。两者都不代表连续观察。[Server 模型](../../apps/orchbee-server/docs/agent-canonical-model.zh-CN.md#原生有界变更读取) 拥有准备、快照/日志一致性及恢复语义。此扩展要求匹配的 cohort 二进制，无持久迁移。原生消费流程及最终验收与编解码器覆盖分开处理。

## 托管启动状态

只读 launch Status 使用请求／结果 kind 25/26 及现有 16 KiB 请求／1,024 字节结果上限。status 结果携带精确 sessionId、规范 revision 和闭合的 management 阶段：unmanaged；含已验证预留及 blocked 布尔值的 pending；或含 reservationId、providerId 及精确启动绑定的 active。只有 Status 结果允许此 outcome。会话存在性与生命周期、已完成启动的保留及读取授权属于 Server 名称所有者。消费者必须关联 Session，并区分已完成启动与当前提示就绪。此原生增量扩展需要匹配的 cohort 二进制。

## 上报输入值

`report_values` 拥有上报标签规范化及完整 u64 来源序号，不包含上报状态、权限、provider 执行或 I/O。
已知标签使用上报形式（`claude`、`agy`、`copilot`、`qwen`），与规范 provider ID 独立。未知标签保留去除
首尾空白后的原文；规范化不删除内部控制字符或截断身份。目录精确查找不裁剪、不转小写、不去除后缀，
也不识别裸 `agent`；这些策略由调用者负责。上报规范化先去除一个已知后缀，再选择最后一个非空路径分量。
原生进程识别使用下文的 provider 注册表，从不使用上报标签规范化。

所有上报来源均为自定义数据，不授予 provider Session 或生命周期权威。序号以规范十进制字符串编码完整
无符号 64 位范围，与 JavaScript 安全整数范围内的规范 Agent 修订分离。顺序由 Server 负责。既有测试覆盖
别名／自定义标签规范化、进程识别及完整 u64 序列化边界。

## 原生进程输入规则

协议 crate 拥有 provider 注册表、逐进程分类器、候选选择器和身份结果词汇。它们不执行 I/O：不采集进程、
不解析 Windows 命令行文本、不授予前台或控制权，也不创建规范 Agent 会话。注册表是进程识别唯一的
provider 词汇；后续的写入方和采样器复用它，而不是各自维护表。

- 注册表：`AGENT_PROVIDER_REGISTRY` 为每个 `AgentReportProvider` 保存一个条目，版本由
  `AGENT_PROVIDER_REGISTRY_VERSION` 标识。每个别名都有冲突分类：`Unique` 名称单独即可确定 provider；
  `Shared` 和 `Generic` 名称要求路径中出现条目列出的某个安装根。单独的 `agent` 对 Cursor 和 Grok
  都是通用名称，因此仅凭名称永远不会识别为 Cursor。与上报标签同名的桌面或 IDE 应用名称（`cursor`、
  `kiro`、`qoder`、`antigravity`）不是进程别名。条目还列出包入口、捆绑运行时和允许的 Python 模块。
  只有真实语料验证后，版本范围才进入条目；在此之前每个条目都是尽力而为。
- 分类器：`classify_agent_process` 接收实际映像和已解码参数，返回 `AgentProcessRecognition`：不是
  Agent；带 `AgentInstallShape` 的交互或非交互；带原因的无法解析；或远程客户端。任意参数中出现的名称
  永远不能确定 provider，伪造的运行时 argv[0] 也不能替代映像。`cmd /c`、PowerShell `-Command` 与
  `-File`、`.cmd`、`.bat` 和 `.ps1` 只解析为它们启动的普通命令；带 shell 语法的文本永远不解析。
- 模式：每个 provider 的 `AgentModeGrammar` 把参数分为交互、非交互或无法分类。语法未列出的内容一律
  无法分类，读不到的命令行永远不算交互。
- 结果：`AgentProcessIdentity` 是带 `confirmed` 或 `recognized` 等级的 `agent`、`none`、带一个
  `AgentIdentityUnknownReason` 的 `unknown`，或带一个 `AgentIdentityUnsupportedReason` 的
  `unsupported`。产生它的状态由 Server 负责。线上格式是以 `result` 区分的封闭对象：`agent` 增加
  `providerId`（原生 provider ID）和 `grade`，`unknown` 与 `unsupported` 增加一个 `reason`，`none` 不增加成员。

`evaluate_agent_sample` 把一个完整样本的 `AgentProcessRow` 值转换为 `AgentSampleVerdict`：带证据（交互、
无法分类的参数、读不到的命令行或脱离的控制台）的最外层候选锚点，或无法指出锚点的原因。它拒绝环、父进程复用的
创建时间倒置和并列歧义；锚点子树之外的不透明行和未决行，以及达到行数上限的样本，使判定保持未决；子树内的行永远
不会是外层 Agent。Server 身份状态把判定稳定为运行和结果；原生 CLI 诊断和 Agent start 通过同一判定读取自己的
样本。各调用方
使用既有 OS 解析器，只有 Server 适配器在身份和时间守卫内接纳经证明的 Host 样本。表驱动夹具覆盖
[进程身份提案](../../proposals/agent/agent-process-identity.zh-CN.md)中 Windows 支持矩阵的每一行和
每个反例；控制台归属和不透明行由 Host 采样器提供。

<!-- documentation-pairing: english-sha256=7899e381988fd119585785bda854e4d6f7ac8ff5183e946f15cc44f48d6d58c6 zh-cn-sha256=0072beafc72b587c3cc792504393378a52a85fe6bf800e47fa9f1d0381dc77fe -->
