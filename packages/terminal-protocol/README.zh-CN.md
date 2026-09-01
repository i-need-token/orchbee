# 终端协议

`@orchbee/terminal-protocol` 是 Orchbee 原生 Terminal Host 与本地客户端之间私有且不依赖传输方式的线协议契约。它负责协议标识、当前版本的精确校验、协议安全限制、控制 Schema、带附件前缀的二进制输入、打包终端帧与传输信封编解码器、打包行窗口与选择导出编解码器，以及语言无关的一致性夹具。

它不会打开套接字或命名管道、生成进程、解析 VT、路由 Electron IPC、渲染终端或拥有终端生命周期、交付、恢复、项目、工作区、工作台、选项卡、面板、代理或产品会话状态。

## 公共 API

| 领域           | 公共契约                                                                                                                                              |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| 标识           | 严格非零的 128 位 Host、创建、终端、客户端、附件、关联及终端主题操作标识                                                                              |
| 兼容性         | 精确校验当前协议 `0.0.23`；拒绝不同版本，不做适配                                                                                                     |
| 消息注册表     | 当前版本中从每种 kind 到其合法角色、载荷编码及成功响应 kind 的唯一映射                                                                                |
| 控制           | 由消息 kind 选择的请求编码，以及严格的握手、配置/主题、清单、生命周期/启动 overlay、附件、投影/历史、响铃、剪贴板写入、应用 signal、选择和失败 Schema |
| 传输           | 有界的 64 字节信封；对于 JSON、输入、终端帧和行窗口，拒绝生产方的角色或编码不匹配                                                                     |
| 投影           | OBTF 格式 `0.0.3` 与 OBPB 格式 `0.0.2` 缓冲区，提供严格的外部解码以及显式的不可变本地零重打包视图                                                     |
| 图形           | OBKG 格式 `0.0.2` 的自有解码图像、已解析放置、有符号 z 分层、动画 tick，以及严格的快照/仅放置更新准入                                                 |
| 历史           | OBRW 格式 `0.0.3` 的渲染/文本行窗口、render-detail 图形快照、稳定原点、已应用锚点及显式淘汰术语                                                       |
| 选择           | 手势、命令、固定头部分块路由，以及有界且需确认的纯文本导出线协议结构                                                                                  |
| 输入           | 以 `AttachmentId` 为作用域、由消息 kind 选择的有界原始输入、按键、已提交文本、鼠标、焦点及审核模式粘贴编解码器，以及对应的正数能力修订                |
| 生命周期线协议 | 互斥的终端阶段、正常退出、运行时失败、遏制及失败结果 Schema                                                                                           |
| 正确性证据     | 固定种子的编解码器属性测试、畸形输入语料库、生成文件比对及 Zig 一致性测试                                                                             |

每个解码器都返回 `ProtocolResult`。格式错误或不受支持的对端输入属于预期失败变体，无需捕获抛出的异常。

`TerminalLaunch` 要么携带四个正数几何字段以立即或无头启动，要么携带四个空几何字段以延迟可视启动；混用两类值无效。延迟创建会在 `awaiting-start` 状态返回标识，不会隐式创建网格。`StartTerminal` 提供完整几何信息，并接收含精确标识、几何信息以及 `started` 或 `already-started` 转换的 `TerminalStartAccepted`。协议不定义可视启动超时，也不提供 80x24 回退。

`CreateTerminal.environmentOverlay` 是必填且独立的数组，最多包含 16 个非空值的环境变量，每个值最多
4 KiB。它不属于 `TerminalLaunch.environment`：overlay 名称必须按目标操作系统的环境标识规则保持唯一，
并且不能与持久启动环境名称重叠。空 overlay 与已提供 overlay 保持为可区分的输入，使 Host 能在不把
capability 值放入持久启动标识的前提下实现恢复校验。协议只拥有这个有界结构；其内存生命周期、摘要
比较、撤销与恢复行为由 Host 和应用 owner 定义。

`ApplyTerminalTheme` 带有终端标识、唯一操作标识、预期的规范 uint64 主题修订版和一个完整的终端主题：配色方案、默认前景/背景、可空光标 RGB 以及正好 256 个调色板条目。 `TerminalThemeResult` 报告 `applied`、`already-applied` 或 `stale` 以及完整的当前状态。修订版零是启动基线，并且是唯一具有空操作标识的状态。每个 `TerminalSummary` 都具有相同的完整状态，因此重新连接恢复不依赖于实时附件或客户端本地颜色默认值。

每条 OBTF 和 OBPB 记录都携带与其行及元数据一同捕获的完整 Host `themeRevision`。OBRW 渲染详情中嵌套带有相同修订标记的 OBPB 快照。客户端把该修订用作呈现屏障；它不是内容修订，也不能替代控制响应与清单所返回的完整主题状态。

每个附件的能力修订均从 `1` 开始。`AttachmentCapabilities` 返回当前正数修订；kind `24` 的 `UpdateAttachmentInputCapability` 只能在精确的下一个修订中更改输入位。完全相同值的重放是幂等的；相同修订冲突、旧修订、修订缺口，或未改变该位的下一修订都会被拒绝。每项原始或语义输入都在二进制权限头中携带精确修订，`InputAccepted` 会原样返回该值。初始 `AttachTerminal` 请求必须至少包含一项能力；仅输入附件降级后，完整的当前能力值可以全部为 false。

kind `25` 的 `ClaimResizeLease` 携带一个附件标识，并返回其完整的当前
`AttachmentCapabilities`。运行中且具备 resize 能力的附件可以 claim；当前 owner 的重放是幂等的。
一次 transfer 会先发布前 owner 完整的 `resizeLease: false` fact，再发布新 owner 的
`resizeLease: true` fact，最后返回 response。Claim 不改变已准入能力或能力修订，
`ResizeTerminal` 也绝不会隐式 claim。

`SelectionExport` 和 `SelectionExportStarted` 保留显式格式字段，唯一合法值为 `plain`。TypeScript 与 Zig 解码器都会拒绝 HTML 和原始 VT，它们不是产品线协议能力。由于帧格式是二进制的，导出分块仍为有界原始字节；执行平台剪贴板效果前，消费者须将获准的纯文本字节解码为 UTF-8。

OBPB 格式 `0.0.2` 有两个故意分开的入口点。 `decodeTerminalProjectionBuffer` 和 `decodeTerminalRowWindow` 完全验证不受信任或网关字节。 `viewTerminalProjectionBuffer` 和 `viewTerminalRowWindow` 仅针对不可变的、功能范围的本地 Terminal Host 传输跳过每单元重新扫描。结构视图函数不是更快的通用解码器。

`materializeTerminalProjectionFrame` 是一个在校验后使用的本地组合 helper。它从元数据匹配的已解码 OBPB snapshot 与 OBRW render source 中创建一个有界 snapshot，对所选行重新设定基准，并可隐藏 live cursor。缺行，或 Host epoch、generation、revision、theme revision、geometry 不匹配时都会失败关闭。结果不携带 extension，且仅是 render projection；它不会恢复 VT、process、PTY、selection 或 command state。调用方仍须对不受信任字节使用严格 decoder。

鼠标移动报告要么不含已按按钮且活动按钮为空，要么携带完整的 `pressedButtons` 集合及其中一个活动按钮。按下与释放始终标明对应按钮；滚轮报告属于按钮按下，但无需把该按钮标记为持续按住。

语义 `KeyInput` 为每个事件携带一个 Option-as-Alt 位。macOS 客户端在发送事件前解析本地的关闭/左侧/右侧/两侧策略；Terminal Host 则负责 Ghostty 当前的 legacy、modify-other-keys 与 Kitty 编码模式。

```ts
import {
	decodeProtocolEnvelope,
	decodeTerminalFrame,
	decodeTerminalFrameRoute,
	inspectTerminalFrameHeader,
	PayloadEncoding,
} from '@orchbee/terminal-protocol';

const envelope = decodeProtocolEnvelope(record);
if (!envelope.ok) return closePeer(envelope.code);

if (envelope.value.payloadEncoding === PayloadEncoding.terminalFrame) {
	const route = decodeTerminalFrameRoute(envelope.value.payload);
	if (!route.ok) return closePeer(route.code);
	forwardToAttachment(route.value.attachmentId, envelope.value.payload);

	const header = inspectTerminalFrameHeader(envelope.value.payload);
	if (!header.ok) return requestSnapshot(header.code);
	if (isAlreadyCommitted(header.value)) return acknowledgeDuplicate(header.value);

	const frame = decodeTerminalFrame(envelope.value.payload);
	if (!frame.ok) return requestSnapshot(frame.code);
	projectTypedColumns(frame.value.cells, frame.value.rowData);
}
```

## 夹具

该包显式导出：

- `@orchbee/terminal-protocol/fixtures/canonical.json`
- `@orchbee/terminal-protocol/fixtures/control-conformance-v0.0.23.json`
- `@orchbee/terminal-protocol/fixtures/focus-input-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/malformed-frame-v0.0.3.json`
- `@orchbee/terminal-protocol/fixtures/malformed-row-window-v0.0.3.json`
- `@orchbee/terminal-protocol/fixtures/packed-projection-conformance-v0.0.3.json`
- `@orchbee/terminal-protocol/fixtures/terminal-frame-v0.0.3.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-failed-v0.0.23.json`
- `@orchbee/terminal-protocol/fixtures/terminal-projection-buffer-v0.0.2.bin`
- `@orchbee/terminal-protocol/fixtures/terminal-row-window-v0.0.3.bin`
- `@orchbee/terminal-protocol/fixtures/transport-envelope-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/write-input-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/key-input-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/text-input-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/mouse-input-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/paste-input-v0.0.23.bin`
- `@orchbee/terminal-protocol/fixtures/utf8-bom-conformance-v0.0.23.json`
- `@orchbee/terminal-protocol/fixtures/ghostty-alternate-surface-v0.0.3.bin`
- `@orchbee/terminal-protocol/fixtures/ghostty-render-row-window-v0.0.3.bin`
- `@orchbee/terminal-protocol/fixtures/ghostty-surface-v0.0.3.bin`

`canonical.json` 只是当前二进制格式的语言无关生成器输入，因此没有兼容版本。`control-conformance-v0.0.23.json` 包含由 Schema 生成的命令、回复、事件、可辨识联合、失败，以及原生类型适配器使用的完整 kind/role/encoding/success-response 契约。`fixtures:generate` 使用构建后的 TypeScript codec 和 Schema 生成这些已跟踪记录、权威 `TerminalFailed` 控制记录、当前 OBTF format-`0.0.3`、OBPB format-`0.0.2`、OBRW format-`0.0.3` 与 OBKG format-`0.0.2` 夹具、其严格一致性语料库，以及帧和行窗口畸形语料库。`test:fixtures` 会拒绝任何缺失或字节已过时的生成输出。每个兼容性夹具都携带其所属线协议或二进制格式的精确语义版本；不使用通配符或仅含主版本号的夹具名。

`packed-projection-conformance-v0.0.3.json` 是 OBPB format `0.0.2` 与 OBRW format `0.0.3` 的语言无关严格边界判定语料。每个命名用例都保存完整规范字节、负责该用例的严格边界以及依据契约编写的结果；被接受的用例还保存语料 Schema 要求的全部观察值。预期结果不由任一解码器生成。TypeScript 严格解码器会消费每个用例；`apps/terminal-host` 则解析已声明的包导出，把有界 JSON 字节传入 Zig 一致性测试，并通过匹配的原生严格验证器运行每个用例。

`TerminalProfile` 携带显式的 `defaultCursorShape` 和 `defaultCursorBlinking` 值。缺失字段会规范化为 `block` 和 `false`，使先前持久化的配置对象保留产品默认值；当前编码器始终发出这两个字段。运行时转义序列可以覆盖活动光标；Terminal Host 负责在 RIS 或应用重置时重新应用这些配置默认值。

`ghostty-surface-v0.0.3.bin`、`ghostty-alternate-surface-v0.0.3.bin` 和 `ghostty-render-row-window-v0.0.3.bin` 是 TypeScript 夹具生成流程的例外。Terminal Host 使用同一个固定版本的原生 Ghostty VT 及真实投影编码器生成 OBTF 夹具对；另一个原生终端会重放主语料，加入真实 DECSCA 受保护/未受保护单元，并通过 `row_encoding.encode` 和 `projection_buffer.encodeRenderSnapshot` 生成 OBRW 夹具。Host 一致性检查会重新生成并逐字节比较全部三个夹具。`@orchbee/terminal` 通过两个渲染器后端和打包存储消费 OBTF 夹具对，并通过严格解码器、可信结构视图和打包存储消费 OBRW 字节。其依据契约编写的 6x8 判定会检查每个行与单元列、原始字符串范围、解码字符串、颜色、样式标志、下划线类型、宽度、语义值、受保护标志和提交元数据；线协议有效的语义变异证明仅有严格准入无法满足该判定。协议包负责稳定字节及导出，Terminal Host 负责原生生产，terminal 包负责消费者语义。该证据不向协议包授予 VT、生命周期、历史或信任边界权威。

实时 OBTF 快照和增量可能携带可选扩展类型 `1`，其一字节有效负载将关闭活动报告为 `idle=1` 或 `active=2`。当观察结果未知时，生产者会省略扩展。该扩展仅是瞬态投影元数据；生命周期和关闭策略仍然由消费者拥有。`NeedSnapshot` 原因 `activity-stale` 用于消费者已在本地使关闭活动观察失效、且无法证明在途帧晚于失效输入时，放弃精确的在途帧；替换帧为完整快照。这一顺序只证明替换帧反映失效后的 Host 缓存，不证明 Host 已接受输入，也不证明终端进程已消费输入。在协议 `0.0.23` 中，Host 接受的非焦点输入会立即使缓存观察失效。后续有界当前状态采样可以发布 `active`、`idle` 或 `unknown`，因此因果替换帧可以在同一终端生命周期内合法恢复新鲜的 `idle`。这不会改变任何线协议值或 codec。

可选扩展类型 `2` 是一个严格解码的 OBKG format-`0.0.2` 记录，用于有界 Kitty 图形投影。资源快照携带 Host 自有的已解码 gray、gray-alpha、RGB 或 RGBA 字节、单调 Ghostty 动画 tick，以及 Host 解析文件、临时文件、共享内存、virtual、relative、裁剪与历史关系后的当前放置。仅放置更新会命名其复用的精确图像修订。有符号 z 值选择背景下方、背景与文本之间或文本上方三个渲染层。重复记录、过时修订、错误像素长度、缺失图像引用、非规范标志或违反限制会拒绝整个 OBTF 帧。OBKG 不携带源路径、句柄、共享内存名称或映射、编码容器、压缩流或未解析 Ghostty pin。

可选扩展类型 `3` 是当前终端进度 sample。其两字节 payload 携带 `remove`、`set`、`error`、`indeterminate` 或 `pause`，并附带从 `0` 到 `100` 的规范百分比或空标记 `255`。`remove` 和 `indeterminate` 不能携带百分比。缺失表示 `remove`，因此快照绝不会凭空创建或重放已完成任务。

可选扩展 kind `4` 表示最近一次语义命令完成。其 25 字节载荷携带正数的终端本地 `u64` 序列、`u64` 持续时长（毫秒）、正数的 `u64` Unix 纪元完成时间（毫秒）以及规范化的 `u8` 退出码。它绝不携带命令文本、参数、目录、环境或输出。重复或畸形记录会导致整帧被拒绝；客户端负责时效性、焦点、权限、去重、速率限制和通知效果。

`TerminalClipboardWriteRequest` 是一条独立且仅用于事件的控制记录，表示一次经 Host 准入的 OSC 52 纯文本写入。它携带精确附件、正数序列，以及最多 16 KiB、不含 NUL 或未配对代理项的有效 UTF-8。该 Schema 不授予剪贴板读取、选择类型、HTML、图像、持久化、信任或效果能力；应用负责提交策略及平台写入。

kind `58` 的 `TerminalApplicationSignal` 是一条独立且仅用于事件的控制记录，携带精确终端标识、字面
channel `orchbee.agent.v1`、正数终端本地序列，以及最多 4 KiB、不含 NUL 或未配对代理项的有效 UTF-8
payload。该记录是 Terminal Host 从 private APC prefix `orchbee.agent.v3;` decode 的不可信 transport
hint；literal channel 只是历史 Host-to-Desktop discriminator，不是 child wire protocol。协议不授予
provider identity、Agent lifecycle、attention、idle、outcome、notification 或 action 语义。应用必须
施加自己单独拥有的更严格 payload Schema。

## 所有权

该包定义值在线协议中的表示与校验方式。它刻意不定义哪些生命周期转换合法、效果何时发生、交付如何提交或产品如何恢复。这些权威分别归属其状态所有者：

- [`apps/terminal-host/docs/runtime-model.md`](../../apps/terminal-host/docs/runtime-model.md) 拥有终端生命周期、运行时遏制、附件传送、重试和主机并发。
- `apps/desktop` 拥有产品绑定、关闭升级、重连、发布和更新策略。

协议 Schema 尽可能使非法的线协议组合无法表示。由消费者拥有的模型与测试负责判断一个有效线协议值在消费者当前状态下是否合法。

完整的线协议布局、消息注册表、边界及证据限制见 [`references/protocol.md`](references/protocol.md)。[`docs/version-history.md`](docs/version-history.md) 负责预发布策略，并从 `1.0.0` 起记录每次已发布线协议或二进制格式版本变更。NPM 包版本是独立的发布元数据，并非线协议兼容版本。

## 开发

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-protocol typecheck
pnpm --filter @orchbee/terminal-protocol build
pnpm --filter @orchbee/terminal-protocol test
pnpm --filter @orchbee/terminal-protocol test:exports
pnpm --filter @orchbee/terminal-host test:protocol
pnpm verify
```

仅在有意审核 TypeScript 所有的夹具更改后运行 `pnpm --filter @orchbee/terminal-protocol fixtures:generate`。对于三个原生表面夹具，运行 `pnpm --filter @orchbee/terminal-host surface-fixtures:generate`。各自的一致性检查只比较已跟踪文件，不会重写它们。
