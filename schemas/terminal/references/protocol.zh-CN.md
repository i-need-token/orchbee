# 终端协议当前合约和正确性记录

| 领域         | 现任权力                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 状态         | 当前语言中立的线协议契约权威；各绑定不实现生命周期状态机、PTY、IPC 侦听器、渲染器、Electron、恢复或网络行为                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 线主         | `schemas/terminal`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 本土消费者   | `apps/terminal-host` 消费语言中立的参考、注册表及夹具来实现有界本机调度和 Zig 一致性                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Rust binding | 尚未发布的独立 `crates/terminal-protocol` binding 镜像完整 message registry、固定 OBTP envelope、connection-control payload、kind-10 terminal-inventory control、kind 12-19 非创建 lifecycle control、kind 20-25 attachment control、kind-30/31 raw-input 交换、kind-32/33 resize-control 交换、kind 34-39 semantic-input request、kind-43/44 projection-feedback control、kind-45/50 row-access control、kind 47-49 projection demand、kind 51-54 history control、kind 55-58 terminal-effect event 表示、kind 60-62 selection control、kind 63-68 selection-export 表示，以及 kind 70-73 terminal-configuration control；尚未发布的 Rust Worker 只为现有 Host handshake 消费 envelope 与 connection-control 子集 |
| 参考范围     | 精确协议 `0.1.0`；OBTF 格式 `0.1.0`；OBPB 格式 `0.1.0`；OBRW 格式 `0.1.0`；OBKG 格式 `0.1.0`；输入/选择编解码器；失败术语；边界；以及夹具                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 变更规则     | 字段、消息语义、线协议字节、边界或夹具发生变化时，必须更新本文、Rust、Zig 和 TypeScript 实现、每个生产者/消费者及证据                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

## 建模记录

| 领域     | 合同                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题     | 本机主机和 TypeScript 客户端需要一种有界的、版本化的、可检查的线路合约和分配意识的二进制编解码器，而无需共享业务状态。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 输入     | 原始记录和外部记录在其导出的严格解码器或架构接受它们之前是不可信的。相同用户能力范围的本地主机数据平面可以使用投影缓冲区结构视图：固定元数据和节边界检查一次，而主机生成的每单元列受合同信任并且不会重新扫描。本地客户端还可以仅检查帧头以识别已经提交的不可变副本。缺席是明确的。                                                                                                                                                                                                                                                                                                                                                                             |
| 数据     | 协议版本与标识；传输信封；控制记录；终端帧；行窗口；输入、投影、历史、响铃、选择、配置及运行时失败的线协议术语。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 输出     | 成功解码或编码的值，键入 `ProtocolFailure`、有界二进制有效负载和严格的控制值。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 不变量   | 接受的记录使用确切的当前版本、规范标识符和整数、注册的消息角色和编码、有效的判别式以及声明的硬边界。一旦为 `(HostEpoch, AttachmentId, sequence)` 传递一个编码帧，该范围序列的每个字节和所有标头元数据都是不可变的，并且该序列永远不会被重用。                                                                                                                                                                                                                                                                                                                                                                                                                  |
| 决定     | 精确版本握手、消息需求和编码、二进制布局、安全限制、严格的控制形状以及互斥的结果表示。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 模型     | 在外部边界严格规范化；确定性的 JSON 与二进制编解码器；另有一条具名、能力受限的结构视图路径，只用于不可变的本地 Host 投影缓冲区。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| 依赖关系 | TypeScript 路径使用 Zod validation 与 standard typed-array/TextEncoder API；Rust envelope binding 只使用 standard library，其 JSON control binding 使用 pinned `serde` 与 `serde_json`。两条路径都不会获得 clock、randomness、storage、process、network、DOM、Electron、GPU、PTY 或 domain transition capability。                                                                                                                                                                                                                                                                                                                                             |
| 所有权   | `schemas/terminal` 只拥有线协议语义。Terminal Host 拥有 lifecycle、delivery、retry 与 concurrency；Desktop 拥有其 binding、recovery、close escalation、publication 与 update policy；Worker 拥有其 Server-first Host admission order 与耦合的 connection liveness。                                                                                                                                                                                                                                                                                                                                                                                            |
| 验证     | 具名 example、stable-seed codec property、strict-prefix exploration、generated malformed corpus、generated-file comparison、export check、Zig 对 canonical binary input 的 decode、Rust 针对相同语言中立 fixture 与当前 metadata example 的 registry/envelope/connection/inventory/lifecycle/attachment/raw-input/resize/semantic-input/projection-feedback/projection-demand/history/row-access-control/terminal-effect-event/selection-control/selection-export/terminal-configuration-control conformance、Worker real-process Host-handshake 与 peer-loss test，以及区分 immutable duplicate recognition 与新 sequence strict admission 的 consumer test。 |
| 正确性   | 编解码器证据验证线路表示和界限。消费者拥有的状态模型和集成测试验证格式良好的消息是否合法以及在当前状态下是否正确执行。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

## 身份和版本控制

每个协议标识正好是 16 个非零字节。 JSON 将其呈现为 32 个不带分隔符的小写十六进制字符。全零字节保留用于不存在的信封标识，并且在需要标识时被拒绝。

| 身份                       | 所有者和寿命                                                                 |
| -------------------------- | ---------------------------------------------------------------------------- |
| `HostEpoch`                | 每个主机进程随机；范围每个主机拥有的参考                                     |
| `CreationId`               | 应用程序创建的持久身份；一个幂等密钥，同时其终端仍由一个主机纪元保留         |
| `TerminalId`               | 随机主机身份保留直至明确处置                                                 |
| `ClientId`                 | 有线提供的诊断代理身份；它不会独立于活动的物理会话授予权限                   |
| `AttachmentId`             | 一个物理连接拥有的附件承载协商的功能                                         |
| `TerminalThemeOperationId` | 应用程序为一个终端主题比较和设置操作创建的幂等性身份                         |
| `CorrelationId`            | `Hello` 之后的每个请求和响应都需要；全零信封字节意味着仅握手消息或事件不存在 |

当前协议版本精确为 `0.1.0`。`Hello`、`HelloAccepted` 与每个传输信封都携带该精确版本。不同的主版本、次版本或补丁版本均被拒绝；该预发布契约不包含任何协商范围、兼容模式或历史消息结构。打包 OBTF 帧格式精确为 `0.1.0`；OBPB 投影缓冲区格式精确为 `0.1.0`；OBRW 行窗口格式精确为 `0.1.0`；OBKG 图形扩展格式精确为 `0.1.0`。对应解码器只接受这些精确格式，并遵循仓库的[发布前版本规则](../../../docs/zh-CN/ARCHITECTURE.md#数据边界)。首次公开发布时契约版本变为 `1.0.0`；此后每项外部可观察契约变化都要更新所属语义版本，并在 [`../docs/version-history.md`](../docs/version-history.md) 记录。Host 构建版本是用于标识可执行文件的独立语义版本；`HelloAccepted.hostVersion` 仅将其用于诊断，不要求构建版本与协议版本一同递增。

## 控制合约

低速率控制有效负载是 UTF-8 JSON，由信封界定，并使用消息的 Zod 模式进行解析。未知键和重复的 JSON 对象成员名称（包括转义等效名称）将被拒绝。无效的 UTF-8、无效的 JSON、JSON 字符串中未配对的 UTF-16 代理、重复的环境名称、无效的身份和超出范围的大小在状态突变之前失败。 TypeScript 请求生产者使用消息种类选择的 `encodeControlRequest` 边界；调用者无法将请求类型与不同的有效控制模式配对。本机请求使用者在分派之前将相同的有效负载解码为键入的主机命令。

`TerminalProfile` 携带显式 `defaultCursorShape`（`block`、`underline` 或 `bar`）和 `defaultCursorBlinking` 值。当解码较旧的持久配置文件形状时，这两个字段都具有兼容性默认值（`block` 和 `false`）；当前的编码器实现了它们。这些是启动默认值，而不是不可变的光标状态：终端转义序列可能会覆盖两者，直到 RIS 或应用程序重置恢复配置文件值。

`TerminalTheme` 是一个完整的运行时颜色值，包含 `colorScheme`、无符号 24 位 `defaultForegroundRgb` 和 `defaultBackgroundRgb`、可空无符号 24 位 `defaultCursorRgb` 以及恰好 256 个无符号 24 位`palette` 条目。 `TerminalThemeState` 添加了规范的十进制文本 uint64 `revision` 和可为空的 `operationId`。修订版 `0` 是启动基线，是唯一具有空操作标识的合法状态；每个正数（positive）revision 都携带提交它的 identity。运行时主题不包括光标形状/闪烁、覆盖、应用程序令牌、内容修订或生成。

消息注册表还拥有每种类型是否可以是请求、响应、事件或声明的组合；每个合法角色中的有效负载编码；以及每个请求的成功响应类型。 TypeScript 连接代码派生这些事实，而不是接受它们作为调用站点参数。本机生产镜像注册表并在记录传递之前拒绝错误的角色或编码。三种语言的注册表测试会针对 `v1/constants.json` 比较每个镜像消息字段，而不仅仅是数字类型集。

`Hello` 和 `HelloAccepted` 最多携带 64 个 `supportedOptionalFeatures` 名称，每个名称非空，最多 64 个 UTF-8 字节。可选功能名称与其解码后的 JSON 字符串值进行比较；因此，Unicode 代码点可以使用多个字节，同时保持在字节范围内。

`TerminalLaunch` 包含一个文字可执行文件和一个参数数组； 它从不包含 shell 连接的命令字符串。 `currentDirectory: null` 表示继承 Host 启动上下文。 环境条目是一个增量：`value: null` 删除变量或使变量不存在，而 `""` 设置显式空值。 名称不能包含`=`. 启动字符串不能包含 U+0000，因为可执行文件、参数、目录、名称和值字段跨越操作系统进程边界。 产品秘密和值仍然是有效负载数据，并且被排除在正常诊断之外。

`CreateTerminal` 还携带与 `TerminalLaunch` 分离的必填 `environmentOverlay`。它最多包含 16 个条目；每个值都非空值，且最多为 4 KiB UTF-8。名称使用与启动环境名称相同的语法和目标操作系统身份规则，在 overlay 内必须唯一，也不能与任何启动环境名称重叠。Overlay 不是库存数据，也不属于持久启动指纹。Host 仍会为保留的创建 generation 派生经目标规则规范化、与顺序无关的 digest：非空重放必须匹配该 digest；空重放则可以在不重建 capability 的情况下校验一个已经保留的创建。Host runtime contract 拥有这些重放与生命周期语义；本契约只拥有必填且有界的线协议区分。

四个启动几何字段构成封闭选择。`columns`、`rows`、`cellPixelWidth` 与 `cellPixelHeight` 要么全是合法整数，要么全是 `null`；部分缺省属于畸形输入。`columns` 与 `rows` 恒为正；`cellPixelWidth` 与 `cellPixelHeight` 必须同正（pixel 已知）或同零（pixel 未知）——混合 cell-pixel 成对值属于畸形输入。数值几何要求立即启动，无头调用方与测试调用方必须提供。空几何会保留延迟终端标识并准备启动，但不隐式提供网格。后续 `StartTerminal` 携带完整 `TerminalGeometry`；线协议契约不定义超时或 80x24 可视回退。

可选且非 null 的 `CreateTerminal.captureSource` 以全空几何选择 Windows 滚动历史编辑器创建。
其封闭对象包含 `attachmentId`、`terminalId`、正数规范 uint64 文本 `generation`，
以及允许零的规范 uint64 文本 `revision`。普通省略保留旧请求形状和编码字节；
显式 null、未知字段、非法标识／条件和立即启动几何均被拒绝。Host 拥有读取授权、
一次锁内完整捕获、按来源区分的重放身份、字面文档路径追加、延迟 Start 和文档清理。
请求不接收文件系统路径，也不接收从终端文字选取的可执行文件。
选定编辑器的启动仍由上游可信启动授权负责。Rust 在同一个 creation kind 中
将该可选流程投影为 `CreateScrollbackEditor`；普通 `CreateTerminal` 值与确认形状保持不变。

环境名称在线路上是唯一的，与写入的完全相同：名称与写入相同的两个条目将被拒绝，而仅大小写不同的名称（例如 `Path` 和 `PATH`）仍然是不同的线路名称。环境名称是目标操作系统环境标识符，主机在准备启动时应用目标 OS 的身份规则：在 POSIX 目标上，名称区分大小写，因此 `Path` 和 `PATH` 是不同的变量；在 Windows 上，名称不区分大小写，因此相同的两个条目是重复的，并且主机在任何进程或 PTY 效果之前拒绝 `CreateTerminal`，并键入 `invalid-field` 失败。 `CreationId` 启动指纹使用与目标身份规则相同的归一化，因此等效折叠输入会产生稳定的、平台一致的摘要。

`TerminalConfiguration` 当前包含 `scrollbackBytes`。内置默认值为 10,000,000 个十进制字节；零会禁用保留回滚；安全上限为 1,000,000,000 个十进制字节。`TerminalLaunch` 携带显式配置或 `null`；首次创建时，`null` 解析为 Host 当前终端默认值。未解析的启动值参与 `CreationId` 摘要，因此即使 Host 默认值后来变化，幂等重试仍保持稳定。`SetTerminalDefaults` 只影响后续终端。`UpdateTerminalConfiguration` 会变更一个保留终端。增加回滚预算可以保留历史；减少预算会淘汰满足边界所需的最旧行。行被淘汰时，Host 会推进内容修订与生成，并使历史锚点、选择/导出状态与投影基准失效。清单摘要会报告有效配置及其独立配置修订，以及完整的当前运行时主题状态及其独立主题修订。

JSON 内的规范无符号 64 位整数使用十进制文本，除了值 `0` 之外，没有符号、空格、指数或前导零。打包二进制使用小端无符号整数。

### 进程采样

`ReadProcessInfo`（kind 79，control JSON 请求和响应）限定于一个具有读取能力的 `attachmentId`。响应回显 `attachmentId` 及其确切的 `terminalId`，并包含 `kind: "windows-job"`、正 uint32 `rootPid`，以及按 PID 排序、最多 64 行且 PID 唯一的 `processes` 数组。每行包含正 uint32 `pid`、必须存在且可空的正 uint32 `parentPid`、正的规范十进制 uint64 `creationTime`（Windows FILETIME），以及必须存在且可空的 `image`、`commandLine`、`currentDirectory`。文本为不含 NUL 的严格 Unicode，每项最多 4096 个 UTF-16 code unit；完整响应仍受 64 KiB control 限额约束。已退出的根进程可以不在进程行中，但其保留身份仍提供 `rootPid`。

Host 拥有采样与 attachment 授权。未知或不可读 attachment 返回 `missing-resource`；平台不支持返回 `process-sample-unsupported`；成员变化返回 `process-sample-changed`；OS 或预算失败返回 `process-sample-unavailable`；容量或编码长度超限返回 `limit-exceeded`。失败绝不转换为空成功。样本描述一个保留 Windows Job 的顺序观察，不是原子进程树、前台进程组、输入许可或就绪保证。远程检查不可用时，可选字段可以缺失。消费者必须关联实际响应及 attachment/terminal 身份后才能使用这些事实。

## 运行时绑定

每个连接在任何其他请求之前恰好发送一个 `Hello`。 `Hello` 和 `HelloAccepted` 协商 `frameBytes`、`inputBytes` 和 `rowWindowBytes`。每个握手后请求都携带非零的`CorrelationId`和当前的`HostEpoch`；每个响应都保留相关性。事件没有相关性。过时的纪元、重复的 `Hello` 和预握手操作在没有状态突变的情况下失败。

命名响应类型是权威的：`HelloAccepted`、`ProtocolRejected`、`TerminalStartAccepted`、`AttachmentCapabilities`、`InputAccepted`、`ResizeAccepted`、 `ProjectionDemandAccepted`、`TerminalDefaultsAccepted`、`TerminalConfigurationAccepted`、`TerminalControlAccepted`和`TerminalThemeResult`回答其相应的请求类型。其他成功的操作会以其请求类型进行响应。 `TypedFailure`可以回答任何请求并保留其相关性。

| 操作                              | 成功的有效载荷                                                                                                                                                               |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ListTerminals`                   | 有效终端摘要的一代范围页面                                                                                                                                                   |
| `CreateTerminal`                  | `TerminalIdentity`                                                                                                                                                           |
| `StartTerminal`                   | 精确的终端标识、接受的完整几何形状以及 `started` 或 `already-started` 过渡                                                                                                   |
| `SetTerminalDefaults`             | 有效的默认配置和默认修订                                                                                                                                                     |
| `UpdateTerminalConfiguration`     | 终端标识、有效配置和修订、内容修订/生成以及逐出行计数                                                                                                                        |
| `ApplyTerminalTheme`              | 终端和请求操作标识、`applied`、`already-applied` 或 `stale` 转换以及完整的当前操作/修订/主题状态                                                                             |
| `TerminateTerminal`               | 目标 `TerminalId`                                                                                                                                                            |
| `ForceTerminateTerminal`          | 目标 `TerminalId`                                                                                                                                                            |
| `DisposeTerminal`                 | 目标 `TerminalId`                                                                                                                                                            |
| `AttachTerminal`                  | 附件与终端标识、授予的 input/read/resize 能力、初始能力修订 `1`，以及调整大小租约事实                                                                                        |
| `DetachTerminal`                  | 目标 `AttachmentId`                                                                                                                                                          |
| `UpdateAttachmentInputCapability` | 附件与终端标识、完整当前能力、当前正数能力修订，以及调整大小租约事实                                                                                                         |
| `ClaimResizeLease`                | 附件与终端标识、完整且未改变的能力与能力修订，以及已提交的调整大小租约事实                                                                                                   |
| 原始或语义输入                    | 附件身份、准确的 capability revision（能力修订）以及提交（commit）到有序入口的编码终端字节数                                                                                 |
| `ResizeTerminal`                  | 附件身份、接受的规范单元/像素几何形状以及由此产生的修订                                                                                                                      |
| `ClearTerminal` / `ResetTerminal` | 附件和终端身份、精确操作以及最终的修订/生成                                                                                                                                  |
| 投影需求                          | 附件身份加接受`active`或`paused`需求                                                                                                                                         |
| 帧/恢复                           | OBTF 格式 `0.1.0` 快照/增量事件，携带已提交的主机主题修订版，精确 `FrameApplied`，键入 `NeedSnapshot`，或键入 `ProjectionBlocked`                                            |
| 历史/读取行                       | 接受的历史需求，有界渲染/文本 `RowsSnapshot`、精确 `RowsApplied`、`HistoryChanged` 和 `HistoryObserved`                                                                      |
| 选择/导出                         | 当前选择状态加上有界的开始/块/确认/完成或取消流程                                                                                                                            |
| 终端开始/结果                     | `TerminalStarted` 携带子进程存在后的终端标识。正常退出携带最终修订和退出代码、信号或未知结果。运行时故障包含最终修订、类型化故障和过程观察。库存使用互斥的阶段特定汇总字段。 |

`RuntimeFailure.stage` 是主终端故障的判别式； `kind`是其所需的更粗的类别，不是一个独立的字段。其合法配对为`admit-terminal-reply/terminal-reply`、`apply-resize/resize`、`dispatch-termination/process-control`、`observe-process/process-observation`、`read-output/pty-read`、`synchronize-worker/worker-integrity`和`write-input/pty-write`。其他每一对都是畸形的。 `ProcessObservation.kind = unavailable` 始终携带 `observe-process/process-observation` 对。 `FailureContainment.kind = blocked` 相反携带一对上下文清理：`contain-process/process-control`、`release-output/pty-read`、`release-process-identity/process-observation` 或 `release-editor-document/editor-document`。文档释放配对表示 Windows 滚动历史编辑器在进程包含、工作线程加入及进程身份释放之后仍持有的清理，或延迟 Start 失败后的文档清理；它不声称任何子进程结局。清理配对作为主要故障非法，主要配对在任一上下文清理位置同样非法。

`ListTerminals` 接受 `{ cursor, maximumTerminals }`。 `cursor` 是首页的 `null` 或包含来自先前响应的规范十进制 `inventoryGeneration` 和 `offset` 值。 `maximumTerminals` 是 `1-128`。响应重复 `inventoryGeneration`，最多返回请求的摘要数量，并提供下一代范围的游标或 `null`。Create 与 Dispose 会推进 membership（成员资格）generation；生命周期、几何、revision 与退出变化不会。一代不匹配会返回 `stale-inventory-cursor`，因此一次遍历永远不会合并两个成员资格。每个页面都保持在 64 KiB 控制范围内，并且协议没有设置终端总数限制。

`awaiting-start` 库存摘要包含空 `columns` 和 `rows`、无退出、故障、遏制或过程观察以及 `terminateRequested: false`。启动阶段带有正网格几何形状。启动后精确的 `StartTerminal` 重放将返回 `transition: already-started` 的原始标识和几何图形；不同的几何形状是不可重试的 `start-conflict` 类型。spawn（拉起）失败是一种类型化的可重试失败，既不发布成功的启动响应，也不发布 `TerminalStarted` 事件；这些效果的生命周期所有权仍然属于 Terminal Host。

`TerminateTerminal` 和 `ForceTerminateTerminal` 是不同的请求类型。wire（线路协议）允许 Desktop 直接发送 force；Host 拥有的生命周期决定当前阶段是否接受该请求，并保证 force 涵盖 graceful 意图。两种请求都不处置（dispose）保留的 identity。 Desktop 拥有升级计时，并在请求处置之前等待 `TerminalExited` 或 `TerminalFailed`。丢失的身份和无效的当前状态请求将被报告为类型化失败，而不会被编解码器重新解释。

成功的 `TerminateTerminal`、`ForceTerminateTerminal` 和 `DisposeTerminal` 响应保留
请求 kind（`12`、`18` 和 `13`），且仅携带由 `TerminalAcknowledgedSchema` 校验的
`{ terminalId }`。Rust 生命周期接受变体保留该类型化的 `TerminalTarget`；它们拒绝
缺失、格式错误、重复或额外字段。解码确认消息只保留其目标，不决定它是否与待处理请求匹配。
消费者负责 envelope 关联和目标匹配；终止请求被接受不代表进程已退出，也不授予处置权限。
现有 `outputs.terminal_acknowledged` 一致性 fixture 定义了这三类响应的规范字节。
Rust 生命周期 fixture 往返、目标保留和畸形输入用例使用与 Host Zig 编码器一致性测试
相同的 fixture 验证这一有界编解码器声明。它们不证明 Host 运行时转换、异步顺序
或消费者的关闭工作流。

该线路不会暴露主机的物理代理会话身份。主机将附件绑定到其一个活动代理会话，并通过该会话解析每个 `AttachmentId`。 `ClientId`是诊断元数据，不合并、转让、独立授予权限。

每个附件至少请求输入、读取或调整大小权限之一。仅当也请求并授予调整大小权限时，调整大小租约才是合法的。空的请求能力绑定和没有调整大小权限的调整大小租约在附件准入时格式错误。仅输入附件降级后，完整的当前能力值可以全部为 false。

kind `25` 的 `ClaimResizeLease` 只携带一个附件标识。运行中且具有 resize 能力的附件可以
claim terminal 的唯一 lease；当前 owner 的重放是幂等的。一次成功 transfer 会返回完整且未改变的
`AttachmentCapabilities`，先发布前 owner 的 `resizeLease: false` `ResizeLeaseChanged` fact，
再发布 claimant 的 `resizeLease: true` fact；成功 response 位于两个 event 之后。遭拒 claim
保留此前 owner、geometry、capability revision 与已准入 capability value。Non-owner 的
`ResizeTerminal` 会被拒绝，绝不会隐式 claim。

`ResizeLeaseChanged` 是仅事件的完整所有权事实，包含确切的附件和终端身份以及 `resizeLease`；它从来都不是一个切换。当分离调整大小所有者提升现有观察者时，主机会在成功分离响应之前提交该观察者的事件。消费者在更改可能颁发 `ResizeTerminal` 的人员之前，根据其附件范围验证这两个身份；演示测量本身仍然是客户本地的，并且不需要租用。

`ResizeTerminal` 要求完整 geometry 满足单字段、cell-pixel 成对、grid-cell 与 unsigned-16 pixel-extent bound。
`ResizeAccepted` 回显 committed geometry，并增加其 resulting revision。为保持兼容，现行 response
schema 会执行四个单字段 bound 以及 cell-pixel 成对不变量——混合对在变更前的下限之下本就不可
解码，也没有任何有效含义——但不会重复 request schema 的 grid-cell 与 pixel-extent cross-field
refinement。这个 decoder
boundary 不会授权或描述无效 runtime geometry：Terminal Host 只接受并发出完整有效 geometry。若要
收紧 response admission，必须作出显式 compatibility decision，不能把它夹带在 binding 修改中。

`ApplyTerminalTheme` 是终端范围的，不需要附件。它携带 `terminalId`、`operationId`、`expectedThemeRevision` 和一个完整的 `terminal` 主题。主机在生命周期和比较和设置之前检查精确的当前操作重放：相同的操作 ID 和主题返回 `already-applied` 而没有突变，而具有不同主题数据的相同 ID 返回不可重试的 `theme-operation-conflict`。预期修订版本不是当前版本的不同操作会返回 `stale`，而不会发生突变。接受的操作应用完整的延迟或启动运行时状态，然后推进独立主题修订一次；其结果转换为 `applied`。

每个`TerminalThemeResult`都会回显请求操作和终端身份，并包含完整的权威`current`状态。对于`applied`和`already-applied`，`current.operationId`等于回显请求操作；对于 `stale`，则有所不同。运行时应用（该主题）会替换 Host 拥有的动态颜色默认值并清除先前的 OSC 颜色覆盖；后续终端 OSC action 可能覆盖这些默认值，而它们的重置形式（reset forms）、RIS 与应用重置会回到最新运行时主题。丢失的结果是未知的结果：客户端在变基之前首先通过库存进行协调，并且永远不会从客户端本地覆盖推断权限。

`ClearTerminal` 和 `ResetTerminal` 携带可输入的 `AttachmentId`；Clear 另要求必填布尔字段 `preservePrompt`。false 表示擦除活动 Ghostty 屏幕并将光标归位，不重置模式；true 按 [Host 运行时契约](../../../apps/terminal-host/docs/runtime-model.zh-CN.md#保留提示行的清空) 保留主屏幕完整提示行。这些是语义 Host 请求。Reset 不接受提示行策略，执行 Ghostty 完全重置并返回主屏幕。`TerminalControlAccepted` 返回附件、解析后的终端、精确操作及最终代次／修订。丢失响应代表未知结果，不授权重放。

每个附件均以能力修订版 `1` 开头。`AttachmentCapabilities` 在完整的当前能力值旁报告该正数规范 uint64 修订，其中可以包含仅输入附件降级后的全 false 值。`UpdateAttachmentInputCapability` 类型 `24` 携带附件、正数修订和下一个输入位。Host 仅应用更改输入位的精确 `current + 1` 修订；精确的当前修订和值是幂等重放。同修订不同值或下一修订不变值是 `capability-revision-conflict`；旧修订或缺口是 `stale-capability-revision`。读取和调整大小能力在此协议版本中仍是不可变的已准入值。

`WriteInput` 是一个原始字节有效负载，恰好包含 16 个非零 `AttachmentId` 字节、一个正的小端 uint64 capability revision（能力修订），然后是一个协商的 `inputBytes` 终端字节。硬终端字节最大值为 1 MiB，因此最大原始有效负载为 1 MiB 加 24 个字节。附件及具体修改解决当前输入权限； `TerminalId` 从未出现在此有效负载中。 `InputAccepted` 呼应修订版，意味着完整的终端字节后缀已提交到有序有界主机队列一次。它是仅队列准入的，并且不保证字节已到达终端写入器，因此根据主机的有界回复优先级策略，终端生成的回复可以在接受的客户端字节之前传送。在该响应之前断开的连接会使结果未知并且永远不会授权重播。

TypeScript 生产者使用 `encodeInputRequest`；消息类型选择原始或语义二进制编解码器。本地消费者通过一个语义输入解码器路由相同的注册类型，因此双方都不需要维护业务层类型到格式表。

`KeyInput`、`TextInput`、`MouseInput`、`FocusInput`、`PasteInput` 和 `PasteAutoInput` 是有界打包有效负载，以相同的 24 字节权限标头为前缀：附件标识后跟其正小端 uint64 capability revision（能力修订）。它们携带浏览器规范化的语义意图，而不是 DOM 对象或预选的转义序列。 `KeyInput` 字节 `24` 携带按键操作，而字节 `25` 在位 `0` 中携带组合，在位 `1` 中携带每个事件 Option-as-Alt 意图。客户端在发布之前解决任何左/右 macOS 偏好； Terminal Host 将意图应用于权威 Ghostty 编码器模式，而不是接受客户端选择的转义序列。 `PasteInput` 使用 32 字节标头：字节 `24` 在位 `0` 中携带经过审核的括号粘贴期望，字节 `25-27` 保留零，字节 `28-31` 携带小端文本字节长度。未知的标志位或非零保留字节将被拒绝。按下或释放鼠标始终会识别其按钮。没有按下按钮的鼠标运动带有空按钮；按下按钮的运动标识 `pressedButtons` 中存在的一个活动按钮，其位集描述了完整的当前按下按钮状态。滚轮报告仍然是按钮按下情况，不需要描述按住的按钮。主机调度在编码之前保留入口顺序和快照权威 Ghostty 编码器模式，因此输出合并或飞行中的帧不会使键、鼠标或焦点编码过时。对于粘贴，主机将权威的括号粘贴模式与审查的期望进行比较，同时保持用于编码的相同终端锁定。不匹配在队列准入之前返回不可重试的 `input-mode-changed`；客户端必须获取当前的预计状态并请求新的用户决定，而不是重播不明确的粘贴。协商的 `inputBytes` 限制适用于编码的终端字节输出，而不是打包的语义负载； `acceptedBytes` 报告当当前终端模式故意不发出任何内容时，输出长度可能为零。主机必须在队列准入之前预检零输出并粘贴边界，然后仅将编码字节计入有序入口预算。

`PasteAutoInput` 携带相同的 24 字节权限标头和 32 字节标头布局，所有标志与保留字节必须为零；字节 `28-31` 携带小端内容字节长度，其余主体为不透明的粘贴内容。与 `PasteInput` 不同，其内容为原始字节，不要求 UTF-8，也不携带客户端声明的括号粘贴期望：主机在用于编码的同一终端锁下快照权威括号粘贴模式，并自行包裹内容。不透明粘贴内容上限为 256 KiB，低于通用语义输入正文上限。

## 消息注册表

| 范围    | 集团                 | 种类                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------- | -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `1-5`   | 连接方式             | `Hello=1`、`HelloAccepted=2`、`ProtocolRejected=3`、`Ping=4`、`TypedFailure=5`                                                                                                                                                                                                                                                                                                                                                      |
| `10-19` | 库存和生命周期       | `ListTerminals=10`、`CreateTerminal=11`、`TerminateTerminal=12`、`DisposeTerminal=13`、`TerminalStarted=14`、`TerminalExited=15`、`StartTerminal=16`、 `TerminalStartAccepted=17`、`ForceTerminateTerminal=18`、`TerminalFailed=19`                                                                                                                                                                                                 |
| `20-25` | 附件                 | `AttachTerminal=20`、`DetachTerminal=21`、`AttachmentCapabilities=22`、`ResizeLeaseChanged=23`、`UpdateAttachmentInputCapability=24`、`ClaimResizeLease=25`                                                                                                                                                                                                                                                                         |
| `30-39` | 输入和大小           | `WriteInput=30`、`InputAccepted=31`、`ResizeTerminal=32`、`ResizeAccepted=33`、`KeyInput=34`、`TextInput=35`、`MouseInput=36`、 `FocusInput=37`, `PasteInput=38`, `PasteAutoInput=39`                                                                                                                                                                                                                                               |
| `41-58` | 投影与效果           | `SnapshotFrame=41`、`DeltaFrame=42`、`FrameApplied=43`、`NeedSnapshot=44`、`ReadRows=45`、`RowsSnapshot=46`、`SetProjectionDemand=47`、 `ProjectionDemandAccepted=48`、`ProjectionBlocked=49`、`RowsApplied=50`、`SetHistoryDemand=51`、`HistoryDemandAccepted=52`、`HistoryChanged=53`、`HistoryObserved=54`、 `TerminalBell=55`、`TerminalClipboardWriteRequest=56`、`TerminalMetadataChanged=57`、`TerminalApplicationSignal=58` |
| `60-68` | 选择和导出           | `SelectionGesture=60`、`SelectionCommand=61`、`SelectionChanged=62`、`SelectionExport=63`、`SelectionExportStarted=64`、`SelectionExportChunk=65`、`SelectionExportChunkApplied=66`、 `SelectionExportFinished=67`, `SelectionExportCancel=68`                                                                                                                                                                                      |
| `70-78` | 终端配置、控制和主题 | `SetTerminalDefaults=70`、`TerminalDefaultsAccepted=71`、`UpdateTerminalConfiguration=72`、`TerminalConfigurationAccepted=73`、`ClearTerminal=74`、`ResetTerminal=75`、`TerminalControlAccepted=76`、 `ApplyTerminalTheme=77`, `TerminalThemeResult=78`                                                                                                                                                                             |

未列出的号码未分配。标题和当前目录更改通过终端 frame 和带精确 identity 的 metadata event 传播。初始调整大小租约权限通过附件许可返回；稍后的所有权 transfer 使用附件作用域的 claim 与完整 event。

`../v1/constants.json` 拥有该表的语言中立消息契约。Rust、Zig 和 TypeScript 将真实实现（包括 TypeScript 私有的 `MESSAGE_CONTRACTS`）与其对拍。新增或修改 kind 时，必须原子更新注册表、绑定、编解码器、夹具及消费者。

`TerminalBell` 是包含 `attachmentId` 和正规范终端响铃 `sequence` 的仅事件控制记录。主机以终端的当前序列为新的可读取附件设置基线，因此在连接或重新连接时不会重播较早的铃声。当提交其确切的传出记录时，每个后续序列都会针对每个实时可读附件发出一次。该事件没有客户端确认，仅携带语义意图；声音、视觉闪烁、通知和可访问性策略仍然是应用程序效果而不是线路行为。

`TerminalClipboardWriteRequest` 是仅事件控制记录，包含 `attachmentId`、正终端拥有的规范 `sequence` 以及最多 16 个 KiB 纯文本 UTF-8。 NUL、未配对的代理、零序列和过大的文本格式错误。它代表分配给一个可读取附件的一个主机承认的 OSC 52 剪贴板写入；它不代表剪贴板读取或授予效果。 Terminal Host 拥有有界挂起状态、单接收者交付、传出批量回滚以及移交（handoff）后移除。接收应用程序拥有显式的每个请求提交策略和平台剪贴板效果。该文件不会被确认或重播到另一个附件，并且它不包含客户端的信任期限、选择类型选择、丰富的格式或剪贴板内容。

`TerminalApplicationSignal` 是仅事件控制记录，包含一个精确 terminal identity、一个由 terminal 拥有的正数 `sequence`，以及最多 4 KiB UTF-8 `payload`。Terminal Host 从准确 private APC prefix `orchbee.agent.v1;` 准入 decoded payload；record 的 literal channel `orchbee.agent.v1` 只是历史 Host-to-Desktop discriminator，不由 child 发出。该 record 既不认证发出信号的进程，也不授予 Agent、lifecycle、attention、outcome、notification 或 provider 权威。Terminal Host 会按 terminal sequence 的 FIFO 顺序保留最多 64 个已准入 hint，并且只有在 queue head 对应的 outgoing record 精确提交后才移除它。产品 adapter 必须先校验更窄的 payload schema，才能解释该值。

请求和响应使用响应标志和`CorrelationId`； [运行时绑定](#运行时绑定) 中的命名对使用其注册的响应类型，其他响应重用请求类型。事件使用事件标志。除非存在可选消息标志，否则未知类型将被拒绝，在这种情况下，可以跳过有界有效负载而不改变状态。

## 当前传输信封

每条记录都以这个固定的 64 字节小端字节序标头开头：

| 偏移量 | 字节 | 领域                                                  |
| ------ | ---: | ----------------------------------------------------- |
| `0`    |    4 | ASCII `OBTP`                                          |
| `4`    |    2 | 标头长度，恰好为 `64`                                 |
| `6`    |    2 | 旗帜                                                  |
| `8`    |    4 | 总记录字节数                                          |
| `12`   |    4 | 有效负载字节，正好是总数减去标头                      |
| `16`   |    2 | 协议主版本                                            |
| `18`   |    2 | 次版本                                                |
| `20`   |    2 | 留言种类                                              |
| `22`   |    1 | 编码：控制JSON `1`，原始字节`2`，终端帧`3`，行窗口`4` |
| `23`   |    1 | 保留零                                                |
| `24`   |   16 | correlation identity；全零表示不存在                  |
| `40`   |   16 | 主机纪元；仅在带有纪元的语义操作之前允许全零          |
| `56`   |    2 | 协议补丁                                              |
| `58`   |    6 | 保留零                                                |
| `64`   |    N | 载荷                                                  |

标志位 `0` 标记可选消息，位 `1` 标记响应，位 `2` 标记事件。位 `3-7` 是可选的标志分区，报告未知值但忽略。位 `8-15` 是所需的分区，任何未知值都会使用 `unknown-required-flag` 关闭解码。事件和响应不能同时设置。

## 当前打包终端帧

该帧以固定的 160 字节标头开始：

| 偏移量 | 字节 | 领域                                                                   |
| ------ | ---: | ---------------------------------------------------------------------- |
| `0`    |    4 | ASCII `OBTF`                                                           |
| `4`    |    1 | 主要格式 `0`                                                           |
| `5`    |    1 | 次要格式 `1`                                                           |
| `6`    |    1 | 快照 `1` 或增量 `2`                                                    |
| `7`    |    1 | 帧标志；目前为零                                                       |
| `8`    |    4 | 总帧字节数                                                             |
| `12`   |    2 | 标头长度 `160`                                                         |
| `14`   |    2 | 保留零                                                                 |
| `16`   |   16 | Host 纪元                                                              |
| `32`   |   16 | 附件身份                                                               |
| `48`   |    8 | 附件本地帧序列                                                         |
| `56`   |    8 | base revision；全 1 表示无 base                                        |
| `64`   |    8 | 捕获的规范终端修订版                                                   |
| `72`   |    8 | 投影生成                                                               |
| `80`   |    8 | 保留的回滚缓冲区行数                                                   |
| `88`   |    2 | 列                                                                     |
| `90`   |    2 | 行数                                                                   |
| `92`   |    2 | 光标列                                                                 |
| `94`   |    2 | 光标行                                                                 |
| `96`   |    1 | 光标形状：块`1`、条形`2`、下划线`3`                                    |
| `97`   |    1 | 光标标志：可见位`0`、闪烁位`1`、宽尾位`2`                              |
| `98`   |    1 | 活动屏幕：主 `1` 或备用 `2`                                            |
| `99`   |    1 | 保留零                                                                 |
| `100`  |    4 | 应用程序键盘/光标、粘贴、焦点、鼠标、modifyOtherKeys 和 Kitty 交互标志 |
| `104`  |   12 | 已解析的默认前景、背景与光标 RGB；游标默认使用其声明的哨兵             |
| `116`  |    8 | 标题和当前目录字符串索引；全 1 表示缺席                                |
| `124`  |    4 | 字符串计数                                                             |
| `128`  |    2 | 样式（style）数量                                                      |
| `130`  |    2 | 行记录计数                                                             |
| `132`  |    2 | 扩展（extension）数量                                                  |
| `134`  |    2 | 保留零                                                                 |
| `136`  |    4 | 单元格像素宽度和高度为两个 `u16` 值                                    |
| `140`  |    4 | 编码单元格总数                                                         |
| `144`  |    1 | 格式补丁 `0`                                                           |
| `145`  |    8 | 已提交的 Host 终端主题修订                                             |
| `153`  |    7 | 保留零                                                                 |

变量部分严格按照以下顺序排列：

1. 每个字符串都是 `u32 byteLength` 后跟严格的 UTF-8 字节。
2. 每个 20 字节样式包含前景、背景和下划线颜色代码 (`u32`)、超链接字符串索引 `u32`、显式样式标志 `u16`、下划线类型 `u8` 和保留零 `u8`。 RGB 占低 24 位；声明的 default 与 none 哨兵是字段特定的。
3. 每个八字节行标题包含行索引 `u16`、换行标志 `u8`、OSC 133 提示类 `u8`、单元格计数 `u16` 和保留零`u16`。确切地说，`columns` 八字节单元如下：字素字符串索引 `u32`、样式索引 `u16`、压缩宽度/OSC 133 语义/保护标志 `u8` 和保留零`u8`。
4. 每个扩展都有种类 `u16`、标志 `u16`、字节长度 `u32` 和字节。标志位`15`表示需要；未知的所需扩展被拒绝。未知的可选扩展仍然受到限制并作为不透明数据返回。

当前的可选扩展注册表是：

| 种类 |         字节 | 含义                                                                                                                 |
| ---: | -----------: | -------------------------------------------------------------------------------------------------------------------- |
|  `1` |          `1` | 实时关闭活动：`0=unknown`、`1=idle`、`2=active`。 Terminal Host 仅发出 `idle` 或 `active` 并在其观察未知时忽略扩展。 |
|  `2` | `40..33 MiB` | 一份严格解码的 OBKG `0.1.0` Kitty 图形资源快照或仅放置更新。                                                         |
|  `3` |          `2` | 当前终端进度：状态字节加百分比`0..100`或空哨兵`255`。                                                                |
|  `4` |         `25` | 最新语义命令完成：正序列、持续时间毫秒、正挂钟完成毫秒和标准化退出代码。                                             |
|  `5` | `10` or `66` | 行原点 epoch 和最近一次关联 Shell 调用；见下文扩展契约。                                                             |

关闭活动是一项提示性的、对新鲜度敏感的采样，而不是生命周期或遏制权威。消费者将缺失、重复、畸形或无法识别的关闭活动载荷映射到 `unknown`。已接受的非焦点输入会在输入 API 返回前同步将客户端最近提交的观察失效为 `unknown`。仅焦点输入和同步遭拒的输入不会使其失效。历史行、失败的实时快照及已暂存但未提交的帧都不能恢复新鲜度；只有随后提交且具备因果新鲜度证据的实时呈现可以恢复。

Host 在投影脏数据门控前按有界节奏采样活动。已接受的会影响活动的输入会使缓存采样失效，并在终端内容修订未变时仍强制生成实时快照。如果已在途的帧到达已在本地失效的消费者并声称 `idle`，消费者会针对其精确序列发送原因是 `activity-stale` 的 `NeedSnapshot`，而不是 `FrameApplied`。Host 放弃该在途帧并强制生成完整快照；由于该请求在消费者到 Host 的流中位于输入之后，替换帧构成因果证据，证明即便终端修订未变，Host 缓存也是在失效后捕获的。它既不证明 Host 接受了输入，也不证明终端进程消费了输入。Host 接受的非焦点输入会立即使缓存观察失效。后续有界当前状态采样可以发布 `active`、`idle` 或 `unknown`，因此替换帧及同一生命周期内后续帧可以合法恢复新鲜的 `idle`。主题暂存阶段会在声明就绪前应用此围栏。新鲜的 `idle` 可以允许产品不经确认直接关闭；`active`、`unknown` 及任何本地过时的呈现都需要确认。

进度状态字节 `0` 移除指示器，`1` 有效，`2` 错误，`3` 不确定，`4` 暂停。 `remove` 和 `indeterminate` 需要空标记。 `set`、`error` 和 `pause` 接受空标记或百分比。重复的进度扩展、未知状态、高于 `100` 的百分比以及非法状态/值组合会拒绝整个帧。省略的进度扩展解析为 `remove`。进度是当前的呈现元数据，而不是生命周期、流程、通知或完成权限。

命令完成类型 `4` 存储三个小端 `u64` 值 - 正终端本地序列、持续时间毫秒和正 Unix 纪元完成毫秒 - 后跟一个 `u8` 退出代码。重复记录、零序列、零完成时间或非 25 字节有效负载会拒绝完整帧。有效负载故意不携带命令文本、参数、目录、环境或输出。这是最近的陈述事实；每个客户端都拥有新鲜度、焦点、权限、重复数据删除、速率限制和通知策略。

OBKG 以这个固定的 40 字节标头开头：

| 偏移量 | 字节 | 字段                          |
| ------ | ---: | ----------------------------- |
| `0`    |    4 | ASCII `OBKG`                  |
| `4`    |    3 | 格式主、次和补丁 `0.1.0`      |
| `7`    |    1 | 资源快照 `1` 或仅放置更新 `2` |
| `8`    |    4 | 总 OBKG 字节                  |
| `12`   |    8 | 仅放置更新复用的资源修订      |
| `20`   |    8 | 单调 Ghostty animation tick   |
| `28`   |    2 | 图像记录数；仅放置更新时为零  |
| `30`   |    2 | 放置记录数                    |
| `32`   |    4 | 聚合解码像素字节              |
| `36`   |    4 | 保留零                        |

每个快照图像都是一个 32 字节记录，紧接着其原始像素字节：图像 ID `u32`、gray/gray-alpha/RGB/RGBA 格式 `u8`、三个保留零字节、generation `u64`、宽度和高度 `u32`、字节长度 `u32` 以及保留零 `u32`。像素长度必须恰好等于维度乘以通道；图像 ID 唯一且非零；所有当前放置必须引用快照图像。每个 56 字节放置记录图像与放置 ID、带符号网格列和行、像素偏移、源裁剪、目标像素大小、带符号 `z` 和标志。只定义 internal-ID 标志位。每个有符号 32 位 z 值都合法；合成精确使用三个有序层：`z < -1073741824` 位于单元格背景下方，`-1073741824 <= z < 0` 位于单元格背景与文本之间，`z >= 0` 位于文本上方。快照裁剪矩形受其图像边界约束。

客户端在 256 个可见解码图像和 1,024 个可见放置中最多保留 32 MiB。仅放置更新不携带图像字节，并且必须命名精确的已保留资源修订；消费者在提交任何文本行前还会根据已保留图像图验证每个引用与裁剪。携带仅放置扩展数据的终端快照无效。资源集或显示动画 generation 变化、重传、删除、重置、屏幕变化或未确认基线都会把 Host 的整个 OBTF 记录提升为快照。direct、文件、临时文件及受支持的 POSIX 共享内存输入、raw/PNG/zlib/chunk 传输、动画、virtual 与 relative 放置、裁剪、有符号 z 合成和历史相交都在本机边界解析。固定版本加载器最多接纳 400 MiB 编码采集数据，而解码存储与每个投影资源集仍分别限制为 32 MiB。OBKG 只携带自有解码像素和已解析放置；绝不携带路径、URL、句柄、共享内存名称或映射、编码容器、压缩流或未解析 Ghostty pin。

快照没有 base，并按 canonical 顺序包含每个可见行。Delta 命名精确的已确认 base，并按递增 row 顺序替换完整且唯一的 dirty row；它绝不携带 partial cell。没有 grid row 变脏时，delta 可以包含零行，同时仍携带 metadata 与 revision transition。Columns、grid-row count、cell-pixel width、cell-pixel height、active-screen 或 generation 变化都要求 snapshot。TypeScript decoder 会先验证并扫描所有 table 与 row，再分配精确 typed column 并读取 cell，而不为每个 cell 构建 JavaScript object。Projected store 在改变 committed grid 前暂存完整 snapshot 或每个 delta row。Frame sequence 单调发出，但 abandoned frame 可能在 committed client sequence 中留下 gap。Delta 必须是精确 next sequence 并命名精确 committed base；complete snapshot 可以使用任何大于 last committed sequence 的 sequence，并跨 gap 原子重建 client base。Fixed-header inspector 只验证 bounded header，绝不准入 cell、string、style、row 或 extension data。Capability-scoped local client 可以在 full decode 前用该 inspector 仅确认已经 committed 的 sequence；这是可靠的，因为 Host delivery owner 在交付该 scoped sequence 时使完整 encoded record immutable。每个更高的 OBTF format-`0.1.0` sequence，以及 trusted local delivery path 之外到达的每一帧，在暴露或提交任何 payload data 前仍须 full decode。OBPB format `0.1.0` projection buffer 提供下文单独命名的 strict-decode 与 local structural-view boundary。Header commit metadata 不同的 repeated sequence 是需要 recovery 的 protocol-state conflict。显式 trusted local duplicate path 可以依赖 immutable commit metadata 而不 hash body；external 与 relayed consumer 也可以在 acknowledgement 前完整 decode duplicate。若 fixed-header validation 与精确 scope association 成功后 full body decode 失败，`NeedSnapshot(decoder-rejected)` 必须命名该 header 的精确 sequence；若 fixed header 本身无法验证或关联，consumer 会 fail the connection closed，绝不猜测 sequence。对于一帧中存在的每对 adjacent row，前一行的 wrap flag 等于下一行的 wrap-continuation flag。Spacer-head 只允许位于 wrapped row 的最后一列；若下一行存在，则它以对应 wide cell 开始。这些关系在 frame 暴露或提交前验证。Sparse delta 改变 retained state 前，consumer 还须对 complete candidate 校验每个 replaced row 与其 retained predecessor/successor 之间的 wrap 和 spacer-head/wide topology。

原生表面证据集包含导出的 `ghostty-surface.bin`、`ghostty-alternate-surface.bin` 和 `ghostty-render-row-window.bin`。Terminal Host 从一个固定版本的原生 Ghostty VT 和真实 OBTF 编码器依次生成 OBTF 夹具对。另一个固定版本的终端会重放主语料，加入 DECSCA 受保护与未受保护单元，并且只通过 `row_encoding.encode` 和 `projection_buffer.encodeRenderSnapshot` 生成 render-detail OBRW；这些字节不由 TypeScript 编码器构造。生产者一致性检查会重新生成并逐字节比较全部三个夹具。TypeScript OBRW 消费者依据一份按契约编写的完整 6x8 矩阵检查严格视图与可信视图，通过 `replaceProjectionBufferSnapshot` 提交可信帧，并验证颜色节顺序、样式、语义、保护、超链接、行换行和宽度拓扑的线协议有效变异。该证据能检测原生列误赋值或遗漏，但不会改变格式、验证 PTY/历史行为，也不向本包授予 VT、渲染器、生命周期或可信本地路径权威。

语料中的 OSC 10/11 覆盖在单元格颜色中表现为显式前景／背景 RGB，包括空白单元格；帧头仍保留相同的有效值。Host 投影仅对不存在程序覆盖的通道保留默认标记，但 DECSCNM 生效期间会显式输出两个已交换的默认色。显式单元格和调色板 RGB 即使等于默认色也保持确切值。这些夹具使用现有 RGB／默认标记编码，格式不变。

## 当前投影缓冲区

OBPB 格式 `0.1.0` 是打包的主机到渲染数据平面。 `decodeTerminalProjectionBuffer` 是默认边界，在公开类型化视图之前严格扫描行、单元格、颜色、字符串范围、UTF-8 和宽单元格不变量。网络对等点、插件或持久工件必须使用该严格边界。 `viewTerminalProjectionBuffer` 保留用于不可变、同一用户、功能范围的本地主机路径。它检查固定标头、身份、计数、规范节目录、对齐和零填充、总长度以及两个小元数据字符串，然后在相同字节上创建类型化数组视图。它故意不遍历每个单元格来重新验证已从规范 Ghostty 状态生成的值；跨语言一致性和生产者测试建立了这些列不变量。

`materializeTerminalProjectionFrame` 在分配物化行/样式图或解码 UTF-8 之前，验证每个选中的字素与超链接源范围。同一来源的精确别名仅计数一次且仅解码一次；范围键包含来源身份。不同或重叠的范围各自以其完整字节长度计入累计逻辑解码工作量，且总量不得超过 `PROTOCOL_LIMITS.maxFrameBytes`。重复的原始样式元组复用其物化样式，而解码为等价样式的不同范围仍共享一个语义样式表条目。`TerminalFrameSchema` 在一次解析中仅验证一次相同的字符串内容，并在每个无效出现路径上保留问题。

固定标头为 320 字节。字节`0..159`包含`OBPB`，版本`0.1.0`，快照/增量种类，总/标题长度，列，网格行，包含的行/单元格，主机和附件标识，序列/基础/修订/生成，回滚原点，光标和像素几何形状，交互标志，默认颜色，可为空的标题/当前目录blob范围、总字符串字节和提交的主机终端主题修订。字节`160..287`包含`{ u32 offset, u32 byteLength }`的十六个固定目录项；字节 `288..319` 保留为零。可空元数据使用偏移量 `0xffffffff`；所有其他字符串偏移量都相对于最终字符串字节部分。

所有整数都是小尾数。固定标头是：

| 偏移量 | 字节 | 领域                                      |
| -----: | ---: | ----------------------------------------- |
|      0 |    4 | 魔法`OBPB`                                |
|      4 |    1 | 主要格式 `0`                              |
|      5 |    1 | 次要格式 `1`                              |
|      6 |    1 | 种类：快照 `1`、增量 `2`                  |
|      7 |    1 | 可选标志；位 `7` 是必需的/保留的          |
|      8 |    4 | 缓冲区总字节数                            |
|     12 |    2 | 标头字节 `320`                            |
|     14 |    2 | 网格列                                    |
|     16 |    2 | 网格行                                    |
|     18 |    2 | 包含行数                                  |
|     20 |    4 | 包含单元格计数                            |
|     24 |   16 | Host 纪元                                 |
|     40 |   16 | 附件 ID                                   |
|     56 |    8 | 序列                                      |
|     64 |    8 | 基础修订； `0xffffffffffffffff`表示不存在 |
|     72 |    8 | 修订版                                    |
|     80 |    8 | 一代                                      |
|     88 |    8 | 回滚缓冲区行原点                          |
|     96 |    2 | 光标列                                    |
|     98 |    2 | 光标行                                    |
|    100 |    2 | 单元格像素宽度                            |
|    102 |    2 | 单元像素高度                              |
|    104 |    1 | 光标形状                                  |
|    105 |    1 | 光标标志                                  |
|    106 |    1 | 活动屏幕                                  |
|    107 |    1 | 保留零                                    |
|    108 |    4 | 交互标志                                  |
|    112 |    4 | 默认前景色                                |
|    116 |    4 | 默认背景颜色                              |
|    120 |    4 | 默认光标颜色                              |
|    124 |    4 | 可为空的标题字节偏移量                    |
|    128 |    4 | 标题字节长度                              |
|    132 |    4 | 可为空的当前目录字节偏移量                |
|    136 |    4 | 当前目录字节长度                          |
|    140 |    4 | 字符串节字节长度                          |
|    144 |    1 | 格式补丁 `0`                              |
|    145 |    8 | 已提交的 Host 终端主题修订                |
|    153 |    7 | 保留零                                    |
|    160 |  128 | 十六个目录条目                            |
|    288 |   32 | 保留零                                    |

目录槽和元素类型是固定的：

| 插槽 | 列               |  元素 |
| ---: | ---------------- | ----: |
|    0 | 包含行索引       | `u16` |
|    1 | 行标志           |  `u8` |
|    2 | 行提示类         |  `u8` |
|    3 | 字素字节偏移量   | `u32` |
|    4 | 字素字节长度     | `u16` |
|    5 | 超链接字节偏移量 | `u32` |
|    6 | 超链接字节长度   | `u16` |
|    7 | 前景色           | `u32` |
|    8 | 背景颜色         | `u32` |
|    9 | 下划线颜色       | `u32` |
|   10 | 风格标志         | `u16` |
|   11 | 下划线种类       |  `u8` |
|   12 | 单元宽度         |  `u8` |
|   13 | 语义类           |  `u8` |
|   14 | 受保护单元标志   |  `u8` |
|   15 | UTF-8 字符串字节 |  `u8` |

交互字的位 0–17 保留既有键盘／鼠标模式，位 18 表示 Ghostty `mouse_alternate_scroll`
（DEC 1007）。Host 随每个修订捕获完整交互字，包括只有模式变化的修订；增量帧整体替换它。
OBPB 在偏移 108 携带相同交互字，render 行窗口内也保持一致。位 19–31 仍非法。
`alternateScroll` 是必需的布尔类型字段；false 表示实际关闭模式，不是字段缺失时的默认值。
依当前预发布版本策略，更新和回退均使用完整开发 cohort。

部分使用其自然对齐和规范槽顺序，并具有零对齐填充。快照包含按规范顺序排列的每一行。增量包含按升序排列的零个或多个完整脏行，并命名确切的已确认基数。快照消费者保留不可变的缓冲区；增量消费者仅保留仍被脏行范围引用的缓冲区。共享内存不是此版本的一部分。

语言中立的 `v1/fixtures/packed-projection-conformance.json` 是 OBPB 与 OBRW 共享严格准入用例的已生成、已跟踪证据权威。其 23 个 OBPB 用例包含 17 个规范变异类别、被接受的稀疏增量行索引 `[0, 2]` 及重复和降序拒绝，以及可选/必需头部标志分区；其 11 个 OBRW 用例覆盖等价的头部标志分区，以及投影段长度、图形段长度和图形快照 kind 失败，另含 5 个 text-detail 成功见证：identity-ASCII 隐式单元格映射行、总文本超过 16 KiB 而每个单元格跨度均在单格上限内的聚合行、空行、三行换行链，以及以 U+FEFF 开头的行。夹具生成器负责每个完整规范字节串，并直接硬编码依据契约编写的结果与成功观察值，不向任一解码器获取判定结果。`test:fixtures` 会拒绝缺失或字节已过时的输出。`decodeTerminalProjectionBuffer` 与 `decodeTerminalRowWindow` 消费每个匹配用例；Host runner 读取语言中立的夹具路径，提供有界语料字节，并让两个 Zig 严格验证器消费相同用例，同时仅断言其公共摘要公开的观察值。

## 当前打包的终端行窗口

`ReadRows` 的范围包括附件、已观察修订、精确代次、正数请求序列、请求行数、协商字节上限、详情（`render` 或 `text`）、用途（`viewport`、`search` 或 `buffer`）、修订策略（`exact` 或 `current-view`），以及一个来源：

- `absolute` 命名保留的绝对行。
- `oldest` 命名距最旧保留行的无符号偏移量。
- `tail` 指定请求的视口下方剩余的行数。
- `committed` 将有界符号增量应用于附件的最后应用的锚点。

`exact` 要求请求修订等于 Host 当前修订，并可用于所有用途和来源。`current-view` 只允许用于来源为 `tail` 或 `committed` 的 `viewport` 请求；其中修订是观察游标而不是请求标识。Host 在持有终端互斥锁时，根据处理当下的视图解析来源，并返回当时捕获的当前修订、代次和总行数。两种策略都要求精确匹配当前代次，因此 resize 或屏幕代次转换不会重新解释旧视口请求。

Host 解析一次来源并返回连续 `RowsSnapshot`。每个附件的每个 purpose 通道至多有一个行请求在途；`viewport`、`search` 与 `buffer` 通道彼此独立推进。请求序列在附件生命周期内严格递增；安装前失败的请求不消费序列。响应只形成待定锚点；匹配的 `RowsApplied` 才把它提升为已提交锚点，因此较早行请求的迟到确认无法提升较晚锚点。调整大小会使待定且固定到代次的行捕获失效，但已提交跟踪锚点仍可跟随有效 Ghostty reflow pin。屏幕代次变化、分离或 Ghostty 页面淘汰会使受影响锚点失效。等待精确确认期间，`HistoryChanged` 会合并。每个 `HistoryChanged` 事件及其 `HistoryObserved` 确认都携带正数的附件本地事件 `sequence`；该值在附件生命周期内严格递增且从不复用。`HistoryObserved` 会原样返回被确认事件的精确 `sequence`。若确认的 `sequence` 早于当前在途事件，则无论过时还是重复都作为幂等空操作，绝不会清除更新在途事件；修订/代次标识仍按完全匹配的在途事件校验。

每个行窗口都以这个固定的 112 字节小端头开始：

| 偏移量 | 字节 | 领域                                         |
| ------ | ---: | -------------------------------------------- |
| `0`    |    4 | ASCII `OBRW`                                 |
| `4`    |    1 | 主要格式 `0`                                 |
| `5`    |    1 | 次要格式 `1`                                 |
| `6`    |    1 | 详细信息：渲染 `1` 或文本 `2`                |
| `7`    |    1 | 可选标志；所需位 `7` 当前为零                |
| `8`    |    4 | 行窗口总字节数                               |
| `12`   |    2 | 标头长度，恰好为 `112`                       |
| `14`   |    2 | 列                                           |
| `16`   |   16 | Host 纪元                                    |
| `32`   |   16 | 附件身份                                     |
| `48`   |    8 | 正附件本地请求序列                           |
| `56`   |    8 | 捕获的规范修订版                             |
| `64`   |    8 | 捕获的屏幕生成                               |
| `72`   |    8 | 保留行总数                                   |
| `80`   |    8 | 绝对第一行                                   |
| `88`   |    2 | 连续行数                                     |
| `90`   |    2 | 保留零                                       |
| `92`   |    4 | 渲染细节的单元格计数或文本细节的逻辑映射计数 |
| `96`   |    4 | 正文字节                                     |
| `100`  |    1 | 格式补丁 `0`                                 |
| `101`  |    4 | render detail 的 OBPB 投影长度；text 时为零  |
| `105`  |    4 | render detail 的 OBKG 图形长度；text 时为零  |
| `109`  |    3 | 保留零                                       |
| `112`  |    N | render 时先投影再图形，或 text 行            |

渲染详情会依次连接一份 OBPB format-`0.1.0` 快照和一份完整 OBKG format-`0.1.0` 图形快照。OBPB 的身份、序列、修订、generation、列数、行数及回滚行原点都与外层窗口一致。每个图形放置都与所选行区间相交并重定位到窗口坐标；每项被引用图像都包含在内。文本详情要求两个段长度均为零，并把每行存储为 12 字节头部，后接严格 UTF-8 文本，并在需要时附上已排序的 12 字节 UTF-8/UTF-16/列/宽度映射。行标志位 `2` (`0x04`) 要求序列化零个映射，并声明每个文本字节都是窄 ASCII，其 UTF-8 字节偏移、UTF-16 代码单元偏移与终端列均等于字符索引。头部条目数仍然包含这些逻辑映射，因此 262,144 个条目的界限保持不变。Unicode、宽单元格、组合字素和未标记行保留显式映射。非空显式行至少有一个从字节偏移零和 UTF-16 偏移零开始的映射。每个映射都从 UTF-8 代码点边界开始，标明对应的 UTF-16 位置，位于列数范围内，并且不与前一个映射单元重叠。每个映射界定一个逻辑单元，因此单个 UTF-8 字符串上限（16 KiB）适用于每个映射单元：从一个映射起点到下一个映射起点的每段字节，以及最后一个映射起点到行文本结尾的最终字节段，都不得超过 16,384 字节。即使每个映射单元的跨度都有界，一行合计仍可能超过 16 KiB。相邻行的换行标志必须一致。文本行、逻辑映射、字节、UTF-8、排序、标记和每单元跨度关系都会经过严格验证。导出的行窗口解码器复用严格的投影缓冲区解码器。单独命名的原生行窗口视图复用投影缓冲区结构视图，不会再执行第二次逐单元语义扫描。

## 主机拥有的选择和导出

指针选择手势携带附件的精确代次与有界、视口相对的数值行/列端点，不携带修订标识。手势 `start` 携带浏览器判定的点击次数、整数表面位置，以及第三次点击是否选择语义输出；`update` 携带当前表面位置、矩形意图和有界单元几何；`end` 只携带视口点；`cancel` 不携带点或动作专属字段。Host 在终端互斥锁内于处理时解析每个点：查看历史时相对于附件已提交且被跟踪的历史锚点，否则相对于当前活动视口。普通输出导致的修订变化不会取消拖选；代次不匹配、屏幕替换或锚点淘汰则会取消。

Host 使用这些事实和固定的 Ghostty 选择行为来拥有单词、行、输出块、矩形和单元内端点语义。`cancel` 幂等释放固定手势，同时保留最新选择；只有 `end` 表示普通指针释放。发布的选择端点是绝对行/列对；活动状态仅包含端点、矩形模式和可选的 4 KiB 预览。`none` 与 `evicted` 不同，因此客户端不能在回滚丢失或屏幕变化后静默复制替代内容。

选择命令仍保留精确的修订与代次标识。`SelectionCommand.set-range` 携带显式、包含端点的绝对 `start` 和 `end` 单元点及矩形模式。它是辅助技术和其他程序化选择的非指针路径；不会合成点击次数、拖动阈值或指针手势状态。

`SelectionExport` 携带文字 `plain` 格式和严格增加附件生命周期的附件本地导出序列； HTML 和原始 VT 不是允许的请求或响应值。在安装之前失败的导出不会消耗其序列。主机首先返回 `SelectionExportStarted`，然后在飞行中最多发出一个原始 `SelectionExportChunk`。每个块包含附件标识、导出序列、单调 `u32` 块序列、最终位以及最多 1 MiB 字节。其 32-byte header 存储 raw 16-byte attachment identity、正数 little-endian `u64` export sequence、little-endian `u32` chunk sequence、flags byte 0 中的 final bit，以及三个为零的 reserved byte；其余 opaque body 可以为空，并且不隐式视为 UTF-8。`SelectionExportChunkApplied` 仅推进匹配的导出，因此来自较早导出的延迟确认无法推进较晚的流。完成或明确取消会释放所有导出存储。格式化输出总量上限为 16 MiB。选择-导出剪贴板写入和其他平台效果保留在该协议包和主机之外；单独定义的 OSC 52 请求仅携带意图，但仍然不授予任何平台效果。

## 硬界限

握手可以选择较小的传输有效负载范围。它永远不会提高这些包的硬限制。

| 输入或保留单位                     |           硬最大值 |
| ---------------------------------- | -----------------: |
| 控制 JSON                          |             64 KiB |
| 一项 OSC 52 剪贴板文本             |             16 KiB |
| 启动 overlay 条目/单个值           |         16 / 4 KiB |
| 单个 terminal application signal   |              4 KiB |
| 一个库存页面中的终端               |                128 |
| 一个原始、文本或粘贴输入正文       |              1 MiB |
| 一个 `PasteAutoInput` 内容正文     |            256 KiB |
| 一键文字                           |              4 KiB |
| `WriteInput` 原始有效负载          |       1 MiB + 24 B |
| `PasteAutoInput` 原始有效负载      |     256 KiB + 32 B |
| 最大语义输入有效负载               |       1 MiB + 32 B |
| 每个终端待处理的有序输入           |              4 MiB |
| 每个终端的回滚配置                 |    1,000,000,000 B |
| PTY 读取批次                       |            256 KiB |
| 打包帧                             |             36 MiB |
| 所有帧扩展/一条 OBKG 记录          |             33 MiB |
| 解码Kitty资源                      |             32 MiB |
| Kitty 图像/placements              |         256 / 1024 |
| 传输记录                           |      44 MiB + 64 B |
| 行窗口记录                         |             44 MiB |
| 客户端边界处的搜索查询             |             16 KiB |
| 选择预览                           |              4 KiB |
| 选择导出/一个 chunk（分块）        |     16 MiB / 1 MiB |
| `SelectionExportChunk` raw payload |       1 MiB + 32 B |
| 单个 UTF-8 字符串                  |             16 KiB |
| 字符串/样式/行记录                 | 4096 / 1024 / 4096 |
| 单元/框架扩展                      |        262144 / 64 |
| 总扩展字节数                       |             33 MiB |
| 网格列/行                          |        4096 / 4096 |
| 单元格像素宽度/高度                |        4096 / 4096 |
| 终端像素总宽度/高度                |      65535 / 65535 |

`One UTF-8 string` 限制每个序列化字符串，并且对于文本行窗口，每个逻辑单元的字节跨度：从映射开始到下一个映射开始的字节以及到行文本结束的最终跨度必须适合 16,384 个字节。聚合行文本不受此每个单元格边界的限制。

在相关分配之前检查计数和长度。整数溢出、长度不一致、截断、无效的 UTF-8、错误的表引用、重复或超出网格的行以及未知的必需标志都属于类型失败。

终端、附件和连接总数没有协议或产品硬最大值。分配、进程、PTY、标识符空间或操作系统耗尽拒绝当前操作；它不授权截断库存或删除保留状态。

## 消费者拥有的行为

有效的协议值本身并不是合法的域转换。状态所有者决定该值是否可接受、执行效果、提交状态并确定重试行为：

| 消费者        | 权威行为和证据                                                                                                                                                                                       |
| ------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Terminal Host | 终端成员资格和生命周期、进程/PTY 所有权、运行时故障遏制、附件功能状态、有序输入、投影/历史/选择交付、批量提交和主机关闭。请参阅[主机运行时模型](../../../apps/terminal-host/docs/runtime-model.md)。 |
| 桌面          | 产品绑定、持久意图、重新连接、终端关闭升级、结果发布、更新激活和渲染器协调。请参阅 Desktop 拥有的指南、模型和测试。                                                                                  |
| 渲染器        | 呈现并应用已经验证的投影/历史值；它不会获得 Host 生命周期权限。                                                                                                                                      |

消费者模型可以使用协议消息名称和模式作为细化点，但它们与拥有建模状态的消费者一起生活。该包的证据有意仅限于线路形状、精确版本处理、安全边界、确定性编解码器、畸形输入拒绝、生成的夹具完整性和跨语言一致性。它不声明流程、交付、恢复、调度或产品策略的正确性。

OBTF 与 OBPB 的网格几何以 `(0, 0)` 保留未知字符像素尺寸，与 Host 及控制几何契约一致。
否则两个尺寸都必须为正；零值与正值混合的组合非法。网格及像素范围上限不变。
终端快照不会为了导出文本或样式而编造像素尺寸。

## 条件空闲终止

kind 82 `TerminateTerminalIfIdle` 使用控制 JSON 请求／响应，与普通终止共用准确的
`{ terminalId }` schema。成功回显目标且只接受经过空闲门禁的优雅终止请求；不证明退出或已处置。
Host 在派发前立即刷新权威进程／提示符状态。活动、未知、排队输入或尚未得到回应的输入返回
不可重试的 typed `confirmation-required`，不改变终止意图或发送信号。成功的库存预读不能替代
此门禁。旧 cohort 拒绝 kind 82；不会回退到 kind 12 或 Force。

库存摘要可携带 `closeActivity: "idle" | "active" | "unknown"`。
Host 为每个序列化页刷新这项仅供关闭的观察，包括没有 attachment 的终端。缺省表示 unknown，
绝非 idle；显式 null 或其他值非法。采样与内容 revision 及库存 membership generation 独立。
其 idle 状态要求当前语义提示符证据，且此前未接受产生字节的活动输入。
接受活动输入字节后，活动终端的空闲证据保持 unknown，直到终端被替换或已证实结束；即使 OS 写入
全部完成，后续 C/A 提示符周期也不能证明 shell 缓冲已被消费。普通投影关闭活动的回退
不授予破坏性权限。TypeScript／Rust codec 与 Zig registry 共用封闭判别值及既有规范目标 ACK
fixture。

## 关联命令观察扩展

可选 OBTF 扩展 kind `5` 包含正输出原点 epoch（`u64`）、reader-ready（`u8`，0/1）和调用阶段（`u8`，0 缺失、1 运行、2 完成）。没有调用时长度精确为 10 字节；否则 66 字节负载继续包含：非零 16 字节调用 ID；开始行 `u64`、开始列 `u16`、结束行 `u64`、结束列 `u16`、正开始 generation `u64`、正开始输出 epoch `u64` 和有符号 `i32` 状态，整数均为小端。运行中的调用要求结束坐标和状态为零。理解该扩展的消费者遇到重复、非法阶段／布尔值、零身份／epoch／generation 或长度错误时，拒绝整个帧。

该观察代表最近一次协作式 Shell 调用，不含命令文本，也不替代 kind 4 的呈现元数据。只有 generation 和输出 epoch 仍一致时，坐标才属于当前保留物理行。最旧保留行 pin 失去连续性时 Host 改变输出 epoch；客户端绝不能由饱和行数推断连续性。旧消费者跳过这一可选扩展。Windows CLI 将其与终端身份及 Host epoch 结合，为不透明输出游标提供围栏。

<!-- documentation-pairing: english-sha256=13f512bccd9a8cfe09839128641f5f3426595f4e886e8c2ca598590aef0033f9 zh-cn-sha256=875ae809bc7b42739cb57464abc5b027662281186d54897bb4d9d3ebd76a9d23 -->
