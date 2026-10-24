# Agent 观察跨语言契约

## 权威

契约变更遵循仓库的[发布前版本规则](../../docs/zh-CN/ARCHITECTURE.md#数据边界)。

[`v1/agent-observation.schema.json`](v1/agent-observation.schema.json) 是 OBAP 帧内 version-1
Agent 观察读取体的语言中立权威。`orchbee-agent-observation-protocol` 中的 Rust 绑定与任何未
来绑定是相互独立的实现。任何绑定都不得新增可接受取值、省略不变量或重定义字段含义。

Version 1 使用由 JSON Schema 2020-12 约束的解码 JSON 值。每个对象都是封闭的，每个集合与字
符串都有界，revision/generation/sequence 数值均为非负安全整数。会话、轮次、请求、附着与解
释实体与 canonical Agent 模型逐字一致；唯一的 wire 层收窄是原生 `presentationId` 轮廓：恰
为 32 个小写十六进制字符，且绝不使用全零哨兵。

## OBAP 帧格式

一次 Agent 读取在已准入的本地 Client Session 上恰好交换一个请求与一个结果；帧族由既有顺
序应用记录循环内的 4 字节记录前缀选择——OBAP 绝不自行开启监听器、连接类型或认证面。

帧头为 12 字节大端序：4 字节 magic `OBAP`、2 字节 version `1`、2 字节 kind、4 字节负载长
度。kind 取值：`1` list 请求、`2` list 结果、`3` get 请求、`4` get 结果、`5` explain 请
求、`6` explain 结果。读取方在分派前拒绝未知 kind、方向错误的 kind、不受支持的版本、无效
帧头或帧头/正文 `kind` 不匹配，并在分配或读取正文前拒绝超出该 kind 上限的声明长度。

编码上限：每个请求体至多 16,384 字节；list 结果体至多 1,048,576 字节；get 与 explain 结
果体至多 131,072 字节。`rejected` 或 `unavailable` 结果体至多 1,024 字节，由生产方在类型
化构造之后、序列化之前检查——帧头本身不标识结果种类，因此任何读取方都不得按结果种类预分
配。

## 请求与结果

请求是扁平封闭对象：`kind`、非零 32 位十六进制 `requestId`，以及操作字段（`sessionId`；
或 `includeEnded`、`limit` 与可选 `cursor`）。缺失的 `includeEnded` 在解析时默认 `false`，
缺失的 `limit` 默认 `50`。结果为 `{kind, requestId, result: {outcome}}`，其中 `outcome`
是封闭联合：成功的页或查找、`rejected` 带封闭原因集合，或 `unavailable`——v1 唯一原因是
`runtime-unavailable`。客户端在信任结果前必须要求 `requestId` 与操作完全匹配；请求标识是唯
一的相关性键。

超出 JSON Schema 校验之外的语义不变量与 canonical 模型逐字一致：capabilities 唯一；
presentation attachment `(presentationId, generation)` 键唯一且每个 `presentationId` 至多有
一个 navigable 附着；`activityHint` 要求 `activity` capability 且 observation source 为
`terminal`；pending request 标识唯一；已结束会话不携带 activity hint、active turn 或
pending request；turn 作用域请求必须命名活跃 turn。每个绑定自行执行这些检查——wire 形状校
验不是 canonical superRefinement 证明，生产方只能由已验证的 canonical 状态构造结果。

## 分页游标

list 分页游标是服务端铸造的不透明令牌：`v1.` + 44 字节负载的无填充 base64url（59 字符） +
`.` + 32 字节标签的无填充 base64url（43 字符），在 107 上限内恰为 106 字符。负载为大端序：

| 字节 | 字段             | 规则                                            |
| ---- | ---------------- | ----------------------------------------------- |
| 1    | version          | `1`                                             |
| 1    | operation        | `1` = list                                      |
| 2    | flags            | bit 0 = `includeEnded`；其余位全零              |
| 8    | runtime instance | 每个 canonical runtime 实例铸造的随机非零 `u64` |
| 16   | target           | v1 全零                                         |
| 8    | revision         | canonical runtime revision，`0..2^53-1`         |
| 8    | position         | 指向过滤后、按 canonical 序排列会话序列的下标   |

标签是对 `b"orchbee.agent.list.cursor.v1\0"` 拼接 44 负载字节计算的 HMAC-SHA256，使用 Server
启动时生成、仅存于内存的 32 字节密钥；没有持久化 epoch。验证顺序：长度与字符集、canonical
负载与标签重编码相等性、恒定时间标签验证，然后是 version、operation、instance、flag、保留
位为零与 position 检查。认证失败、非 canonical 形态或不可能 position 的游标为
`cursor-invalid`；来自同一 runtime 实例、其记录的 list revision 与当前 revision 不再相等的
可信游标为 `cursor-stale`。position 索引按 canonical `sessionId` 比较排序的过滤后会话序列
——UTF-16 码元序，对 ASCII 标识字符集而言与字节序一致——绝不是插入序，也不按 wire 顺序
重整。`nextCursor` 仅在还有过滤后条目时非 null，精确末尾为 null。

## 非目标

本契约不拥有 canonical Agent 状态、会话生命周期、ingress、变更、wait 或变更流、socket 策
略、准入、权限、游标密钥、期限、CLI 语法或渲染。Server 拥有 canonical runtime 与读取策
略；CLI 拥有调用与呈现。

Launch Status 使用只读 kind 25/26、精确会话身份及规范修订。其闭合 management 阶段为 unmanaged、携带原预留及 blocked 标志的 pending，或携带原预留／provider／物理绑定的 active。结果仍限 1,024 字节；只有 status 操作接受 status outcome。[Rust 绑定契约](../../crates/agent-observation-protocol/README.zh-CN.md#托管启动状态) 及 Server 名称所有者定义验证和生命周期。

TerminalStatus 使用只读请求／结果 kind 27/28，沿用请求和小结果上限。它要求完整的新物理启动绑定，返回该绑定及 unmanaged 或 active provider 归属，不携带 canonical 会话、修订、预留或就绪声明；只有 TerminalStatus 结果允许此 outcome。物理成员不符返回 binding-changed。Server Names 所有者持有恢复后的托管状态；客户端关联精确绑定及请求。
