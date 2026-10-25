# Client-Server Protocol Rust Binding

## 临时终端清空

新增的 `clear` 控制动作仅携带已有 viewer 的 `attachmentId`，不获取或转移控制权。`cleared` 结果携带正数十进制 `generation` 和 `revision`，以及 `cleanupConfirmed`；投递未知使用已有 `outcome-unknown` 类别。其他结果保留封闭含义。Server 拥有 Site 暴露、物理绑定和 Worker 代次隔离和新临时 attachment 标识；Worker 拥有仅输入附加 → Host 保留提示行的清空 → 分离。[原生清空](../../apps/orchbee/docs/native-cli-query.zh-CN.md#清空终端输出) 拥有公开命令、预算和回执。需要匹配的开发 cohort。

## 范围

`orchbee-client-server-protocol` 拥有原生 OBCS v1 Rust binding。语言中立 v1 schema 与 JSON/binary corpus
位于 [`schemas/client-server`](../../schemas/client-server/README.zh-CN.md)。本 crate 负责 validated Rust value、严格且区分
方向的 JSON decoding、deterministic encoding 与 bounded frame I/O。它不依赖 OBLC local admission
binding、OBCN Server-Worker authorization binding、Terminal Host type 或 Workbench type。

本 crate 不连接 transport、不 authenticate Client、不 authorize request、不选择 Worker、不存储
request identity、不执行 terminal effect、不 cancel work、不 reconcile unknown outcome，也不 project
Workbench state。

## Public API

| API                                                                                                                                                 | Contract                                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `RequestId`、`SiteId`、`WorkerId`、`TerminalId`、`ViewerId`、`HostEpoch`、`ControllerId`、`PresentationId`、`PanelId`、`TabId`、`TerminalContextId` | 解析独立 typed、nonzero opaque 128-bit 小写十六进制 identity。                                                                                             |
| `WorkingDirectory`                                                                                                                                  | 校验最多 4,096 个 encoded byte、无 NUL 的 absolute local-platform UTF-8 path。                                                                             |
| `Geometry`                                                                                                                                          | 仅当所有 scalar 与 product bound 都成立时构造 complete geometry。cell-pixel width 与 height 必须同正（pixel 已知）或同零（pixel 未知）；混合成对值被拒绝。 |
| `TerminalCreateRequest`、`TerminalStartRequest`                                                                                                     | 构造 validated Client request；start 拒绝相同的 start/create request identity。                                                                            |
| `CreateOutcome`、`StartOutcome` 与 result type                                                                                                      | 只表示 OBCS v1 准入的稳定 Client-visible result combination。                                                                                              |
| `encode_client_frame`、`decode_client_frame`                                                                                                        | Encode 或 decode 恰好一个完整 in-memory Client-to-Server frame。                                                                                           |
| `encode_server_frame`、`decode_server_frame`                                                                                                        | Encode 或 decode 恰好一个完整 in-memory Server-to-Client frame。                                                                                           |
| `read_client_message`、`read_server_message`                                                                                                        | 从支持 partial read 的有序 stream 中精确读取一个 frame，并保留后续 frame 未读。                                                                            |
| `write_client_message`、`write_server_message`                                                                                                      | 通过 partial write 写出完整 frame，且不 flush caller。                                                                                                     |

所有 decoded object 与 enum 都是 closed。缺失显式 nullable create field、未知 field 或 tag、重复
key、BOM、无效 UTF-8、尾随 JSON value、direction mismatch、无效 identity、path 或 geometry，以及
不符合精确长度或 63 KiB payload bound 的 JSON frame 都会被拒绝。Protocol 与 geometry number 使用
schema 所有的有限 binary64 safe-integer refinement，因此等价的 integral decimal 与 exponent
spelling 会 decode 为同一个 value。Canonical encoding 使用带普通 integer token 的 compact
deterministic JSON。

## 查看附件 binding

`TerminalViewerRequest`、`TerminalViewerAction`、`TerminalViewerResult` 和 `TerminalViewerOutcome`
提供 Session scope 内只读 Attach、AttachBinding、Poll、FrameApplied、NeedSnapshot 和 Detach value。
AttachBinding 携带独立 typed 的 OBCS `SiteId`、`WorkerId` 与 `TerminalId` value；这些 domain 之间
相同的 encoded byte 仍然有效。本 crate 校验其 lexical contract，但既不导入 Workbench type，也
不判断 tuple 当前是否已暴露。该 semantic resolution 由 Server 负责。
`ViewerUnavailableReason::WorkbenchUnavailable` 仅用于 viewer result。

Poll、FrameApplied、NeedSnapshot 和 Detach 必须携带 typed `attachment_id: ViewerId`；Attach 与
AttachBinding 不能携带该字段。`TerminalViewerAction::attachment_id()` 返回既有 viewer selector，
对两种附着 action 返回 `None`。Server 校验提交 Session 内的所有权；binding 只校验身份语法。

`ViewerId` 与 `HostEpoch` 表示 Server 签发的附件 scope。Attached 还要求实际
`HostTerminalId`；原生投影结合 epoch 与终端身份，拒绝同一 Host 内其他终端的绘制证明。
要求匹配的开发 cohort。反馈计数器使用规范 u64 十进制字符串；
JSON corpus 测试每个 action、outcome、reason、closed field set 与 identity boundary。记录结果保留
schema owner 定义的 OBSR 二进制 framing 和 `MAXIMUM_VIEWER_RECORD_BYTES` bound；同一组 Server
frame function 可分派 JSON control 或 opaque 二进制记录。Client 方向 reader 拒绝 OBSR。
AttachBinding 不增加 OBCN 或 OBSR type。本 crate 不解码 OBTP 或终端呈现；Server 和后续
renderer 分别负责路由与呈现验证。大记录测试覆盖字节相等、partial read、后继帧对齐、方向，
以及非法 header、length、identity 的拒绝。

### 历史视口 action（未发布）

五个追加 action kind 为原生历史视口扩展 viewer 面：`set-projection-demand`、
`set-history-demand`、`read-viewport-rows`、`rows-applied` 与 `history-observed`。它们仍是
未发布契约，要求精确开发 cohort；不定义 fallback 形态。每个 action 在自身 variant 内携带
`attachmentId`，与既有 attachment 作用域 action 一致。

`set-projection-demand` 携带 `demand`（`"active"` 或 `"paused"`）；`set-history-demand` 携带
`demand`（`"following-bottom"` 或 `"viewing-history"`）。它们的 Host 应答只回显
attachment 与 demand 对，因此调用方按 correlation 加回显 demand 匹配——没有 generation
字段可比。

`read-viewport-rows` 携带 `origin` 对象（`{"kind":"tail","rowsBefore":u32}` 或
`{"kind":"committed","delta":i32}`）、`rowCount`（`1..=4096`）、`revision`、`generation`、
`requestSequence`（正 u64 十进制字符串）与 `maximumBytes`（`112..=8,388,608`，与 `read-rows`
相同的 OBRW payload 边界）。Host 按当前视图服务：返回的 revision 成为该窗口的捕获身份，
因此 binding 不要求响应与请求 revision 相等。`rowsBefore` 与 `delta` 是有界 JSON 整数
（`u32`/`i32` 域，均在 JavaScript 安全整数范围内）；`delta` 原生有符号，允许负滚动差值。
可选 `detail` 为 `text`（默认，规范编码省略）或 `render`。Server 精确转发该类型并拒绝
另一类型的响应。交互历史请求 render；既有 text 请求字节保持不变。升级或回滚完整 cohort。

`rows-applied` 携带 `requestSequence`、`generation` 与 `revision`——Host 将 pending 视口锚
提升为 committed 所需的精确三元组。`history-observed` 携带 `sequence`、`generation` 与
`revision`——对一条已投递 history-changed 事件的精确应答三元组。两者 sequence 与
generation 使用正 u64 十进制字符串，revision 使用规范 u64 十进制字符串。

四个追加 viewer action kind 承载 Host 权威的选区和纯文本导出，而不让 Client 获取屏幕权威：
`selection-gesture`、`selection-export`、`selection-export-chunk-applied` 与
`selection-export-cancel`。每项 action 都指定精确 `attachmentId`。手势 generation 与导出
sequence 使用规范的正 u64 十进制文本，chunk sequence 为 `u32`。手势 action 是闭合的扁平集合
`start`、`update`、`end`、`cancel`。视口点的行列范围为 `0..=4095`，click count 为
`1..=3`，指针两轴为 `-65535..=65535`；update geometry 要求 cell width 与 columns 在
`1..=4096`，screen height 是正 `u16`。导出格式仅允许 `plain`；取消原因仅允许
`clipboard-failed`、`disposed`、`replaced`、`user`。未知字段和开放判别值在 OBCS 边界拒绝。

本 binding 只承载 control 和不透明 record。手势的 `SelectionChanged`、导出的
`SelectionExportStarted` 及取消的 `SelectionExportFinished` 作为 record outcome 原样返回；
精确 chunk-applied 回显映射为 `FeedbackAccepted`。Poll 投递的选区变化、原始导出 chunk 和
export-finished event 继续通过 OBSR，binding 不解释 chunk 字节。剪贴板写入、手势调度、导出
组装及重试策略仍归 Client。既有 action JSON 保持逐字节规范；schema corpus 覆盖四种手势、
三种导出控制、硬边界、闭合枚举及额外字段拒绝。匹配的 Client/Server/Worker cohort 一起交付
或回退。

`ViewerRejectedReason::HostRejected`（`"host-rejected"`）是闭合的 viewer 级 reason；四个历史
控制应答 action 与上述四个选区／导出 control 在 Host 返回明确 typed failure 时共用它。它与
controller 面的 `ControlRejectedReason::HostRejected` 不同，也不适用于旧 action。
`read-viewport-rows` 的失败沿用既有 `RowsRejected` 九值 reason 词表；`read-rows` 逐字节不变。

## Controller binding

`TerminalControlRequest`、`TerminalControlAction`、`TerminalControlResult` 和 `TerminalControlOutcome`
携带显式 Acquire 与 grant scope 内的 Input/Resize。`ControllerId` 是 Server 签发的身份；
三个 action 都必须携带 `attachment_id: ViewerId`，通过 `TerminalControlAction::attachment_id()`
读取。这是尚未发布的 OBCS v1 必需字段变更；Client/Server/Worker 必须使用精确匹配的开发 cohort，
不提供隐式 viewer fallback。
`ControlInput` 验证 1 至 `MAXIMUM_CONTROL_INPUT_BYTES`（4,096）byte，并序列化为规范小写 hex。
共用 number adapter 验证正十进制 u64 sequence。Acquire、Input 与 Resize 使用既有 63 KiB OBCS
JSON framing。

`Paste` 是语义自动模式粘贴 action。它携带 `PasteContent`——1 至
`MAXIMUM_PASTE_CONTENT_BYTES`（262,144）byte 的不透明正文——且从不出现在 OBCS JSON 中：
client frame codec 将其编码为专用 `OBCQ` record——4-byte magic、big-endian version 1（`u16`）、
零 reserved 字段（`u16`）、big-endian body 长度（`u32`），随后是 request ID（16）、attachment
ID（16）、controller ID（16）、sequence（8）与内容。完整 `OBCQ` record 为 68 至 262,212 byte。
codec 在分配 payload 前验证完整 12-byte header 与界限，恰好消费一条 record，并保留后续 frame
边界；server 方向 reader 分发 OBCS JSON 或 `OBCQ`，client 方向 reader 拒绝 `OBCQ`。
`Pasted` outcome 回显 Host 报告的正 `accepted_bytes` 计数。除共享拒绝原因外，paste 还映射
Host 类型化失败 `host-input-invalid`、`host-input-limit-exceeded`、`host-input-backpressure`
与 `host-input-unavailable`。

Schema owner 的 corpus 检查每个 action/outcome/reason
以及畸形、最大和超限 input。本 binding 不授予 controller 权限，绝不重试未知副作用。

## Send binding

`TerminalSendRequest`、`SendRecords`、`TerminalSendResult` 与 `TerminalSendOutcome`
承载一次经 Server 授权、面向已暴露 Workbench 终端的非交互 input 提交。request 是扁平的
`requestId` + `siteId`/`workerId`/`terminalId` 绑定三元组 + 闭合记录序列；三元组是
Server 在准入时解析的声明目标，不是化身锁。本 binding 不授予 controller、viewer、resize
或 lease 权限，不占 attachment 槽位，且绝不跨 restart 重绑：准入后 worker 或 Host 化身已
变化的目标只能失败或报告 unknown，绝不改向。

`SendRecords` 包含 1–64 个有序 `SendRecord` 值：text、语义 stroke、不透明 bytes 或由 Host 加括号的 paste。文本及可选折叠 CR 合计不超过 4,096 字节；不透明 bytes 不超过 1 MiB，paste 内容不超过 256 KiB。序列解码内容合计不超过 1 MiB；按键不贡献解码字节。全部按键都使用 `SendKeyStroke`，包括 Control 修饰的字符。具名身份为 1–13；Insert、F1..F25、非控制 Unicode 标量及 Control/Alt/Shift/Super 修饰键使用相同 stroke 编码。[原生发送契约](../../schemas/client-server/README.zh-CN.md#当前原生契约) 拥有规范二进制布局。序列不具备原子性。

每个 `TerminalSendOutcome` 都携带已确认前缀 `progress` 及正交的 `cleanup`。已接受记录数不超过首个停止索引；accepted 输入要求非零且相等的数量／索引，cleanup 不能为 not-needed。任何非零前缀都排除 not-needed 清理；unknown 输入排除 confirmed 清理。`acceptedEncodedBytes` 汇总 Host 编码后入队的字节数（被当前模式忽略的按键可以为零）。接受表示入队，不表示子进程执行。拒绝和不可用原因是闭集；部分或未知结果绝不授权自动重放。

`TerminalSendResult` 另携带 `target`：Server 实际准入本 send 的 `SendTarget` 物理化身
——worker generation、Host epoch 与 Host 终端身份。`target` 字段在 wire 形态中始终存在
且可为 `null`，但 `null` 仅对 cleanup 为 `not-needed` 的 `rejected`/`unavailable`
合法——即在任何 binding 解析前决定的 pre-admission 拒绝。每个 `accepted`、
`outcome-unknown` 或 cleanup 为 `confirmed`/`unconfirmed` 的结果必须记录实际准入的
target；构造与两个 codec 边界一致执行该规则，而 cleanup 为 `not-needed` 的
`rejected`/`unavailable` 结果仍可携带已钉定的 target。消费方逐字段照录该
target，绝不重新解析。

## Launch read binding

`TerminalLaunchReadRequest`（`terminal.launch.read`）与 `TerminalLaunchReadResult`
（`terminal.launch.read.result`）承载一次针对单个终端的只读 launch-fact 查询。请求为
`requestId` + `terminalId` 加必选 `protocolVersion`；不派发任何 Worker/Host RPC、不挂
viewer、不持有 grant、绝不变更——Server 在 coordinator 锁内按其当前已提交的 Workbench
membership 解析目标，并一趟返回。

`found` outcome 携带当前路由行（`terminalId`、`workerId`、`boundTo` 的
presentation/panel/tab/context identity）、`siteId`、join 观测到的 decimal-string
`workbenchRevision` 以及一个 `launch` fact 对象。若终端经具名 Launch Profile 启动，
`found` 还携带可选成员 `launchProfile` `{profileId, catalogRevision}`——在 launch
prepare 时冻结的 provenance；其他方式启动的终端该成员整体缺席，且即便 profile 在其
选择被捕获后遭删除，该成员仍保持准确。其余 outcome 只有 `not-found` 与
`unavailable{reason:"server-busy"}`；无 Workbench 的 Site 一律回答 `not-found`，包括
仅存的 orphan retained 记录。

显式程序还会在 `found` 中提供可选的 `launchSelection:{exec,args}`。这是创建时捕获且已校验的
不可变选择，包括已解析的命名 profile；删除 profile 或移动 Panel 不会改写它。默认 shell 和
`not-tracked` 事实省略该字段。显式 null、无效选择以及 `not-tracked` 同时携带选择均非法。
沿用最多128个参数/30,720字节的约束。查询不暴露环境或授权数据，不执行 RPC，不新增持久存储。
旧消息可省略该字段。

`launch.state` 逐字呈现 retained launch phase——`creating`、`awaiting-start`、
`starting`、`started`、`unknown` 或 `unavailable`——另加 `not-tracked`：当可见 binding
没有可唯一归属且一致的 coordinator 记录时使用（缺失、歧义或不一致的记录如实报告，绝不
修补）。`not-tracked` 是对 retained fact 的观察，不是 `unknown` phase，也绝非进程存活；
`creating` 同样只记录 phase，不证明 dispatch 已发生。`workerGeneration` 为 positive
decimal string，恰在 `not-tracked` 时为 `null`；`geometry` 恰在 `started` 时非 null；
`createOutcome` 照录 retained `CreateOutcome` 编码，在 `creating` 与 `not-tracked` 时为
`null`。三个成员始终存在：缺 key 即 malformed frame，`null` 才表示缺席。decoder 在
closed-dictionary 与 identity 校验之外一并执行这些 conditional-null 规则。任一方向上都不
出现 lease、fence、key 或 authorization 内部字段。

## Request boundary

本 crate 不建模 replay lookup、历史 duplicate detection 或 runtime idempotency ledger。
`RequestId` 仅用于 request/result correlation；Client 为每次 submitted request 生成新 value，且
不会在同一个 active Server authority 与 authenticated subject 范围内复用。
`terminalCreateRequestId` 在同一 scope 内标识 deferred create，不依赖具体 Client Session
transport。Client 对 mutating intent 至多提交一次，且不得在 timeout、EOF、Session loss 或 caller
abandonment 后自动重发。这些事件都不编码 cancellation，也不能证明 effect 未 commit；ambiguous
completion 留给未来的只读 projection 或 observation boundary。

## 验证

从仓库根目录运行：

```sh
CARGO_TARGET_DIR="$PWD/target" cargo fmt --all -- --check
CARGO_TARGET_DIR="$PWD/target" cargo check -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo test -p orchbee-client-server-protocol --all-targets --all-features --locked
CARGO_TARGET_DIR="$PWD/target" cargo clippy -p orchbee-client-server-protocol --all-targets --all-features --locked -- -D warnings
```

Conformance test 消费 canonical cross-language corpus，并覆盖 frame boundary、strict payload
decoding、construction invariant、partial I/O 与 no-flush behavior。它们不证明 transport、Server
state machine、authorization policy、Worker 或 Host mapping、terminal lifecycle 或 end-to-end
execution。

## 独立终端打开 binding

`TerminalOpenRequest`、`TerminalOpenResult` 与 `OpenOutcome` 实现尚未发布的
[独立终端打开 contract](../../schemas/client-server/README.md#standalone-terminal-opening)。
请求只携带关联 ID 和完整 geometry；Opened 返回供 legacy viewer attachment 保留使用的 Server
独立生成引用。AttachBinding 不改变 Open result。Binding 拥有严格编码和校验，Server 拥有
singleton selection、creation 与 lifetime。
Canonical fixtures 覆盖所有 open outcome，并通过 JSON schema 和独立 Rust decoder
拒绝缺失字段、额外字段、畸形及方向错误的消息。

Open 与 AttachBinding 是增量、尚未发布的 OBCS v1 source contract。它们保留既有 frame version
与 limit。安装后的 exact runtime cohort 是 Client、Server、Worker 与 Host；这个 Rust source
binding 是 contract input，而不是已安装的 cohort artifact。发布前，其 source contract 与 runtime
cohort 一起 rollback，且不涉及 persisted state 或 migration。

## 原生启动环境扩展

原生 OBCS binding 为 `terminal.create` 接受可选 `environment`，编码为有序的
`[name, value]` 对。环境为空时省略该可选成员。
`LaunchEnvironment` 在构造和 JSON 解码时校验：至多 32 对，名称和值合计不超过 1,024
UTF-8 字节，名称为 1..128 字节的 ASCII 标识符，不含 NUL，不允许忽略大小写后重复，
且不能使用 `ORCHBEE_*`、`TERM`、`COLORTERM` 或 `TERM_PROGRAM`。空值和内嵌
`=` 逐字保留。Debug 不输出值。含本覆盖的完整请求参与请求身份判断；Server 将其转换为
独立的 OBCN 类型。

v1 schema/corpus 覆盖 launch selection、environment、profile provenance、send sequence progress 及全部四类 OBSN input record（含语义 stroke）。Conformance 验证独立编写的 canonical bytes 与 negative case。这证明原生 OBCS 一致性，不代表 Desktop 或 Workbench schema 对齐。

原生 v1 帧在分配前限制为 63 KiB。组合最大转义 cwd、executable/argv 与环境的测试覆盖请求预算；旧 32 KiB 上限无法容纳该合法组合。

## 终端进程读取（未发布）

`TerminalViewerAction::ReadProcessInfo`（`read-process-info`）仅携带既有 `attachmentId`。它使用当前 viewer 授权，以 record 结果返回原样、相关联的 Host kind-79 进程样本或 kind-5 typed failure。Server 验证 Host epoch、请求关联、attachment 及物理终端身份，绝不用启动参数替代当前进程事实。原生 `panel process-info` 消费者解释 Host 的类型化结果。读取不创建 controller、输入或 resize 授权，不推进帧反馈，也不把缺失或变化中的进程观察转换为空成功。沿用既有会话期限与临时 viewer 回收规则，要求同一开发版本整体部署。

## 显式控制权接管

`TerminalControlAction::Takeover` 是仅携带 `attachmentId` 的闭合 OBCS v1 `takeover` action。
复用 Acquire 的 Acquired 结果及拒绝/不可用/未知分类。
[语言无关契约](../../schemas/client-server/README.zh-CN.md#session-controller)拥有语义；
[Server 交接模型](../../apps/orchbee-server/docs/terminal-controller-takeover.zh-CN.md)拥有
预留、撤销及清理。此新增 action 要求同一开发版本整体部署。

启动选择另将最坏 JSON 字符串内容限制为 30 KiB：ASCII 控制字符计六字节，引号和反斜杠计两字节，其他 UTF-8 字节计一字节。原有 2 KiB 输入域全部保留，同时约束更大的恢复命令。扩大上限后必须使用匹配 cohort。

## 插件终端展示环境

显式终端启动可携带独立的 `PluginTerminalEnvironment`（OBCS 字段为 `pluginEnvironment`）。只允许插件 ID/root/entrypoint/config/state/context、原生二进制路径及可选 Workspace/Tab/Pane 展示 ID：十个唯一名称，七项必填，合计最多 24 KiB UTF-8。这些是展示数据，不是权限或插件登记。普通调用者环境保持原有上限及保留名称拒绝规则。runtime base/scope 和 Worker/终端身份仍由 Worker 适配器负责。插件环境必须配合显式可执行文件。OBCN 启动种类 3 在 40 KiB 帧上限内传递显式启动选择、普通环境和插件环境。Server 在预留物理操作前校验完整插件启动编码。Worker 先清除继承的已知插件展示字段，再应用本次值。Workbench 插件解析、Pane 归属和 Client 布局仍是独立的集成职责。定向边界及启动事实测试覆盖此载荷，不代表插件 Pane 已完整接通。
