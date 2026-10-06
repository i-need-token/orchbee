# Workbench 跨语言 Contract

## 语义事件

`v1/workbench-events.schema.json` 及共享语料定义封闭 OBWP55／56 形状，覆盖连接持有订阅、
有界等待、真实语义／探测观察与明确历史 gap。Rust 和 TypeScript 额外执行编码字节上限、
准确事件／数据／来源关联、过滤条件、全宽顺序及递归布局准入。可选 Client `splitRevision`
保留旧形状，存在时提供真实比例变化证据。运行时发布和回收属于
[Windows 事件所有者](../../apps/orchbee-server/docs/windows-events.zh-CN.md)。

## 权威

[`v1/workbench.schema.json`](v1/workbench.schema.json) 是 version-1 只读 Workbench projection
kernel 的语言中立权威。`@orchbee/workbench-client-protocol` 中的 TypeScript binding 与
`orchbee-workbench-protocol` 中的 Rust binding 是独立实现。任何 binding 都不能增加可接受值、
省略 invariant 或重新定义字段含义。

Version 1 使用由 JSON Schema 2020-12 约束的已解码 JSON value。每个 object 都是 closed，
每个 collection 与 string 都有边界，identity 是有类型且非零的 128-bit 小写十六进制字符串，
revision 是 canonical unsigned-64 十进制字符串。transport 必须在 JSON decoding 前拒绝超过
16 KiB 的 encoded intent，或超过 1 MiB 的 encoded snapshot 或 delta。下文 Workbench command 扩展拥有 OBWP
framing 与 duplicate-key rejection。鉴权、授权策略和 transport lifecycle 仍归 application owner。

Label 只包含 Unicode scalar value，拒绝未配对的 UTF-16 surrogate。Protocol version 是已解码的
数值 1，包含 JSON 写法 1.0 和 1e0；canonical encoding 输出 1。

[`v1/workbench-conformance-v1.json`](v1/workbench-conformance-v1.json) 是依据 contract 编写的
跨语言 oracle。Valid 与 invalid document 覆盖 strict parsing 和 semantic invariant；transition
case 覆盖确切 delta result 与 typed rejection。实现必须消费每个 case，不能用自己的 parser 或
reducer 重写 expected result。

## Close 扩展

schema、规范一致性语料和独立 Rust/TypeScript binding 已实现严格 Close 命令、状态及
Resume 语法。[Close 线协议契约](#已接受的-tab-close-线协议契约) 定义此分阶段基础的边界。
[Server Close 权威](../../apps/orchbee-server/docs/terminal-tab-close.zh-CN.md) 拥有生命周期
行为及已纳入维护的 Server/Worker 有限模型。原生模型及运行时集成尚待完成；解析器及模型
证据不证明实际 Close 效果。

## Projection

`workbench.snapshot` 是一个 Site revision 的完整 authorized product projection。它包含：

- 有序 Project，及各自的有序 Directory 与有序 Workspace；
- 恰好一个 standalone Terminal Context，以及每个 Workspace 各一个 Terminal Context；
- 有序 Tab group 与 Tab；
- 每个 Tab 的完整 binary Panel/Split topology；
- 每个 Panel 的一个 primary Terminal Presentation 与零个或多个 child Terminal Presentation；以及
- Presentation 可选的 opaque Worker/Terminal binding。

每个 Directory 携带自身的 typed identity、确切 Worker identity 与 absolute POSIX UTF-8 path。
每个 Workspace 携带一个必填且可为空的 primary Directory reference，指向自身 Project。
Projection 包含 label，但不包含 directory discovery 或 mutation policy、terminal content、terminal lifecycle policy、
Environment、Checkout、Worktree、Agent、Launch Profile、Archive、SSH、lease、fence、Host epoch、
attachment 或 persistence fact。它也不包含 Client-local active location、active Tab、focus、
disclosure、selection、maximization、split weight、viewport、geometry、search、window、DOM、React
或 pixel state。Presentation 缺少 binding 只表示本 projection 没有暴露可 attach 的 Terminal
identity；Client 不能据此推断 lifecycle reason。

Project、Directory、Workspace 与 Terminal Context array 使用 identity 升序。Directory identity
在整个 Site projection 中唯一，Workspace 的非空 reference 必须在其所在 Project 内解析。Tab group 与 Tab 保留显式产品
顺序。Pinned Tab 构成一个前置且不分组的 prefix，每个 Tab group 的 member 都连续。每个被引用
context 恰好存在一次，每个 context 也恰好被引用一次。除 JSON Schema validation 外，identity
uniqueness、tree depth、node quantity、membership 与 reference invariant 还作用于完整 snapshot。

### Workspace 显示顺序

Workspace 移动命令的块输入和重排输出使用非空、无重复且最多 1023 个 Workspace ID 的列表，专用请求／结果上限为 48 KiB，其他命令保留既有上限。完整重排输出携带已提交修订，无变化移动使用既有 `unchanged` 结果。块的可选 before 目标允许省略或 null；位于来源块内的目标在变更前拒绝。

Navigation 可选携带 `workspaceOrder`，上限 1023 个 Workspace 身份。缺省或空列表沿用 Project 优先的身份顺序遍历。
非空顺序必须是该 Navigation 中全部 Workspace 身份的完整排列：重复、缺少成员或外来身份均无效。
完整 Navigation 替换与快照都检查该不变量。Project、Directory 与 Workspace 成员数组仍按身份排序；
显示顺序不改变归属、Directory 引用或终端绑定。筛选视图保持对应子序列，Client 本地选中身份仍由 Client 拥有。

原生排序权威在显式顺序末尾添加新建 Workspace，关闭时移除对应身份。私有检查点保留该字段；旧检查点缺少
该字段时保持原有遍历。所有参与者使用同一兼容 cohort；旧严格解码器可能拒绝新增字段，绝不能通过丢弃字段来修复。

## Intent 与 delta

`workbench.snapshot.read` 请求完整 authorized projection。原始 projection document union 仍然只
准入该 intent、snapshot 和 delta。Workbench command 请求/结果 envelope 是独立增量契约；两份契约均不定义命令行语法。

`workbench.delta` 命名一个确切 Site、base revision、更大的 result revision，以及一组无序且原子
的 aggregate change。一个 change 可以替换完整 navigation、upsert 一个完整 Terminal Context，
或移除一个完整 Terminal Context。至多允许一次 navigation replacement，且每个 Terminal Context
identity 至多一个 change。当 authorized Site fact 的变化位于本 projection 外时，空 change set
可以推进 projection revision。

`applyWorkbenchDelta` 无副作用地执行以下步骤：

1. 拒绝 Site mismatch、base-revision mismatch 或 non-increasing result revision；
2. 拒绝重复 navigation 或 Terminal Context target；
3. 暂存每个 aggregate replacement，并按 identity 排序 Terminal Context；
4. 校验完整 candidate snapshot；以及
5. 只有所有检查成功后才返回完整 next snapshot。

任何 rejection 都保持 caller 的 snapshot 不变。恢复时获取完整 snapshot；本切片不定义 replay
buffer、retry、persistence 或 transport request。

## 命令与 OBWP

[`v1/workbench-command.schema.json`](v1/workbench-command.schema.json) 拥有严格 Workbench 请求和结果。
[`v1/workbench-command-conformance-v1.json`](v1/workbench-command-conformance-v1.json) 是其共享
Rust/TypeScript oracle；原始 duplicate-key case 另行验证 Rust wire parser。TypeScript 验证已解码值；
它的 transport 必须在解码前拒绝重复 key。

`workbench.command` 携带 protocolVersion、requestId、siteId、expectedRevision 及一项 action：
project.create、project.rename、workspace.create、workspace.rename、
workspace.primary-directory.accept、tab.rename、tab.create、panel.split、tab.move、tab.close、
panel.close 或 panel.move。Label 在既有上限内保留包括
空白在内的确切文本。新产品身份归 Server 所有。`workbench.command.result` 关联 project-created、
workspace-created、workspace-primary-directory-accepted、renamed、unchanged、tab-create-pending、tab-created、panel-split-pending、panel-split-created、tab-moved、tab-close-accepted、panel-close-accepted、panel-moved、rejected、denied 或 unavailable outcome。Revision 与新身份
仅出现于声明它们的变体；denied 不泄露两者。`workbench.snapshot.result` 包含携带完整嵌套
`workbench.snapshot` 的 available，或 denied、unavailable。Schema 拥有封闭 reason；
[Server 权威](../../apps/orchbee-server/docs/workbench-authority.zh-CN.md) 拥有决策顺序、原子提交、
授权及其有界进程内 ledger。契约不隐含自动重放。

`workbench.publication.read` 携带 protocolVersion、requestId 及调用方已提交的
`knownRevision`。`workbench.publication.result` 关联携带仍为当前修订号的 `current`、携带 1 至 64
条按序已提交 publication（每条为完整 `workbench.snapshot` 或 `workbench.delta`）的
`published`、`denied` 或 `unavailable`。将这组有序 publication 应用到 `knownRevision` 处的
snapshot 即复现已提交的 projection。当 Server journal 无法将 `knownRevision` 衔接到其保留的
最旧 publication 时，可以回答携带一条完整 snapshot 边界 publication 的 `published`。Denied 与
unavailable 不泄露修订号或内容。

OBWP 使用 12 字节 header：magic `OBWP`、big-endian u16 version 1、big-endian u16 kind 及
big-endian u32 payload length。Kind 1/2/9 为 snapshot.read/command/publication.read 请求，3/4/10 为
snapshot.result/command.result/publication.result 响应。JSON kind 必须匹配 header。Reader 在分配 body 前拒绝错误
version、方向、kind 或超限 payload。请求与命令结果最多 16 KiB；原始 snapshot 与 publication 结果最多
1 MiB + 1,024 字节，每条已发布文档保留其独立 canonical 上限。完整嵌套 snapshot 另受 canonical JSON
1 MiB 上限约束。Envelope 内的空白与替代 escape
计入原始外层上限。独立 snapshot 保留原始 1 MiB 上限。有效 record 保留后续 record 未读；EOF 截断失败。
Rust binding 提供 Read/Write codec，不拥有 socket、timeout、准入或重试。OBCS 继续为独立终端协议，
OBLC 准入位于两种 application carrier 之前。

## Workspace primary Directory 契约

`workspace.primary-directory.accept` 恰好携带 `projectId`、`workspaceId` 与 `path`。
`path` 是 1 至 4,096 个编码字节的 absolute POSIX UTF-8 文本，不含 NUL，并保留原始拼写。
Client 不提供 Directory 或 Worker identity。成功的 `workspace-primary-directory-accepted`
outcome 携带已提交的 `revision`、Server 分配的 `directoryId` 与选定的 `workerId`。

Projection 通过向 Project 追加一项有序的
`Directory { directoryId, workerId, path }`，并由 Workspace 必填且可为空的
`primaryDirectoryId` 选择它，来表示原子结果。每个 Project 至多接纳 256 个 Directory。
Directory identity 在整个 Site projection 中唯一；Workspace reference 必须在自身 Project 内解析。
缺失必填可空字段、悬空或跨 Project reference、重复或无序 Directory identity、非法 path、
未知字段及 collection overflow 均为无效输入。

Rejection reason domain 增加 `workspace-directory-unavailable` 与
`workspace-directory-conflict`；command-unavailable domain 增加 `directory-unavailable`。
[Server Directory 权威](../../apps/orchbee-server/docs/workspace-terminal-directory.zh-CN.md)
拥有授权、Worker 校验、set-once/empty-context 准入、原子提交、重放及 runtime 行为。
这些 schema 与 binding contract 不校验文件系统，也不选择 Worker。

## 规范 Tab 重命名契约

`tab.rename` 恰好携带 `terminalContextId`、`tabId` 和 `label`。Label 非空，最多 512 个
Unicode code point，并原样保留，包括空白。Server 先解析指定 Terminal Context，再解析该
context 内的 Tab。Context 不可用时返回 `terminal-context-unavailable`；Tab 缺失时返回
`tab-unavailable`，包括使用了另一 context 的 Tab identity。

Label 改变时返回下一修订号的 `renamed`；文本相等时返回当前修订号的 `unchanged`。该操作只
改变 `Tab.label`：`Presentation.label`、所有 identity、顺序、拓扑、binding、terminal state
和 Server identity cursor 均保持精确不变。它不分配 identity、不启动 Worker effect，并且只
需要普通 Workbench mutation grant。

请求 fingerprint 包含 expected revision、context、Tab 和确切 label。精确保留重放即使发生
后续 mutation，也返回原结果和原 result revision；同一 request identity 下内容改变时返回
`request-identity-conflict`。目标的 `tab.create` 结果仍 pending 时可提交 rename。之后 create
completion 只结算该保留 create 结果，不改变 projection 或任一 label。

### Tab 重命名细化记录

| 字段         | 契约                                                                                                                                                                                                        |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题与范围   | 独立实现的边界需要一种不会改变终端 identity 或 presentation 来源的规范语义重命名。本记录准入未发布 wire 含义；Server 权威拥有授权、重放、容量与提交。                                                       |
| 输入与信任   | 不可信的有界 JSON，携带确切 Site、request identity、expected revision、Terminal Context、Tab 和 label。解码值在校验和 Server 准入前不具有权威。                                                             |
| 数据与所有权 | Schema 拥有封闭字段和 `tab-unavailable`；Server 拥有规范 projection、revision 和 ledger。目标 `Tab.label` 是唯一可变产品事实。                                                                              |
| 输出与决策   | `renamed`、`unchanged` 或既有 typed denied/rejected/unavailable 结果。Context 解析先于 Tab 解析；任何结果均不提供新 identity 或 terminal outcome。                                                          |
| 模型与转换   | 一次同步纯 projection 转换或 no-op。精确重放使用保留的 fingerprint/result revision。Pending create settlement 相互独立，不能回写 projection。                                                               |
| 不变量       | 提交后只能改变目标 `Tab.label`；`Presentation.label`、顺序、拓扑、binding、terminal state、所有 identity 和 cursor 均保持精确。改变标签时修订恰好增加一，相等文本不增加修订。                               |
| 依赖         | 既有 OBWP framing、Workbench label/identity 校验和纯 Server Workbench grant。该操作不引入 Worker、Host、OBCS、terminal-create grant、clock、entropy、persistence、retry、renderer 或 selection 权威。       |
| 验证与限制   | 共享 Rust/TypeScript corpus 与 codec 测试建立 wire 一致性。Server 有状态测试、有界并发/重放模型、projection 测试及 create/rename/completion 进程见证，在单进程及有界 ledger 范围内建立 runtime refinement。 |

## 独立 Tab 创建契约

`tab.create` 恰好携带 `terminalContextId`、`label` 和 `geometry`。目标 context 必须是
canonical standalone Terminal Context。该语义操作追加一个未分组、未固定的 Tab、一个根 Panel，
以及一个具有 binding 且没有 child 的 primary Presentation。提交的 label 原样写入 `Tab.label`
和 primary `Presentation.label`。允许重复 label；label 绝不用于选择身份。Server 拥有所有新身份
及 Worker/Terminal binding。请求不携带 placement、generation、create reference、cwd、executable
或结构身份。

独立的 `WorkbenchTerminalGeometry` 值包含 `columns`、`rows`、`cellPixelWidth` 和
`cellPixelHeight`。每项都是 1..4096 范围内的已解码有限整数，接受等值的整数型十进制小数和指数
JSON 写法——cell-pixel 成对字段例外：取值范围 0..4096，且必须同正（pixel 已知）或同零
（pixel 未知），混合成对值非法。Canonical encoding 输出整数 token。语义校验还要求 `columns*rows <= 262144`、
`columns*cellPixelWidth <= 65535` 和 `rows*cellPixelHeight <= 65535`。JSON Schema 拥有单值
边界和封闭结构；binding 拥有这三项乘积约束，由 schema comment 和 semantic corpus case 说明。
OBWP 独立于 OBCS 拥有该值；runtime adapter 必须验证转换一致性，不能引入协议依赖。

两种已提交 outcome 均恰好携带 `revision`、`terminalContextId`、`tabId`、`panelId` 和
`presentationId`。`tab-create-pending` 表示 membership 和确切 logical binding 已提交，
terminal completion 仍待完成。`tab-created` 额外携带封闭的 `terminalOutcome`：

| Kind              | 字段与含义                                                  |
| ----------------- | ----------------------------------------------------------- |
| `started`         | `geometry`，恰好等于接受的请求 geometry。                   |
| `unavailable`     | `reason`：`terminal-runtime-unavailable` 或 `server-busy`。 |
| `failed`          | `reason`：`terminal-create-failed`。                        |
| `outcome-unknown` | 没有额外字段；无法确定 completion。                         |

每种已提交变体均保留相同的结构身份和 binding。Terminal outcome 为 unavailable、failed 或
unknown 时，不回滚 membership，也不授权另一次 launch。Pending 期间的相同请求重放返回固定
pending tuple；结算后的相同请求重放返回保留的最终结果。两者均不再次派发 effect。进程内准入、
retirement、ledger reservation 和响应交付顺序归 Server 权威所有。

对于新 request identity，顶层 `denied`、`rejected` 和 `unavailable` 均不提交新 Tab。
`WorkbenchRejectionReason` 额外准入 `terminal-context-unavailable` 和 `tab-unavailable`。Reason domain 分离：
`WorkbenchCommandUnavailableReason` 准入 `request-ledger-full`、`no-compatible-worker`、
`server-busy`、`service-unavailable`、`terminal-busy` 和 `terminal-runtime-unavailable`；`WorkbenchSnapshotUnavailableReason` 仅准入
`service-unavailable`；`WorkbenchTerminalUnavailableReason` 与 `WorkbenchTerminalFailureReason`
拥有上述嵌套 reason。两种语言 binding 的 action union 均名为 `WorkbenchAction`。既有四项 action
保持确切拼写与行为。

### 契约细化记录

| 字段         | 契约                                                                                                                                                                                                                                                                                                                            |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题与范围   | 独立实现的 Rust 与 TypeScript 边界必须对同一个共享 terminal-Tab intent 达成一致，并区分已提交 membership 和 terminal completion。本记录准入 contract type、schema、corpus 和 codec；不证明 Server 命令或 Client 交互已可用。                                                                                                    |
| 输入与信任   | 不可信的有界 JSON 或 framed byte；解码后的 identity、label、revision 和 geometry 通过校验前不具有权威。必填字段没有默认值或推断的缺省含义。                                                                                                                                                                                     |
| 数据与所有权 | Schema 拥有封闭 wire variant 和字段含义；每个 binding 独立校验。仅 runtime owner 可以授权命令、分配身份、提交 membership、派发 Worker effect 和选择 Client-local Tab。                                                                                                                                                          |
| 输出与决策   | 完整 typed request/result 或 rejection。决策覆盖 shape、direction、kind、identity、numeric value、geometry product、label、encoded limit 和 action-specific pending/final variant。解析和序列化没有业务 effect。                                                                                                                |
| 模型与转换   | Pure parser/validator/encoder；没有本地业务状态转换。Pending 和 final 是语义结果值。Server 状态机拥有两者之间的转换、确切重放和有界保留。                                                                                                                                                                                       |
| 不变量       | 封闭字段和 reason；确切文本和有类型的 identity domain；等值的已解码数字；保持 framing/byte limit；提交后的失败保留 membership；不隐含新请求或 effect。                                                                                                                                                                          |
| 依赖         | 既有 JSON/serde、TypeScript Zod 边界和有界 Read/Write codec。Contract 不引入 OBCS 依赖，也不引入 process、socket、clock、permission、retry 或 renderer capability。                                                                                                                                                             |
| 验证与限制   | 依据 contract 编写的 corpus 驱动独立 Rust/TypeScript 接受与拒绝校验，包含原始数字写法、wire 边界的 duplicate key、区分 schema/semantic 的 geometry failure、每种 result variant 和确切 canonical round trip。Geometry 转换一致性与 action/result correlation 还需要 runtime adapter 测试；仅 corpus 一致不能证明 runtime 行为。 |

这些 Tab 扩展仍为本地、未发布边界。Schema、corpus、Rust/TypeScript binding 和消费它们的 runtime
binary 在同一 activation cohort 中一起更新；OBWP 保持 version 1。旧的 closed decoder 拒绝新增
action/result。回退时一起恢复整组 source/runtime cohort；不提供混合版本 rollout、公开兼容承诺、
持久迁移或 replay recovery。

## 已接受的 Tab 移动契约

本地未发布 Move 扩展已在 schema、语料、两个 binding 及 Server 中实现；原生 standalone Client 交互已集成，
其有界 Linux cohort 证据见[原生 Session 契约](../../apps/orchbee/docs/native-tab-session.zh-CN.md)。它使用现有 OBWP v1
命令 envelope 和 navigation-mutation 授权。action 为
`tab.move { terminalContextId, tabId, direction }`，direction 严格为
`previous | next | first | last`。Previous/next 与一个相邻 Tab 交换，不循环；
first/last 将完整 Tab 移到该边界。单成员或目标已在请求边界时产生现有
`Unchanged { revision }`。实际置换产生 `TabMoved { revision, terminalContextId, tabId }`
（线上 outcome 为 `tab-moved`），恰好推进一个修订。不允许新产品/终端身份或终端/查看器效果。

现有 `TabUnavailable` / `tab-unavailable` 仅在精确 standalone 或 Workspace 上下文及完整支持布局
通过后表示目标缺失；仅 standalone 要求 Site 组合。Workspace Move 不要求 Directory 或在线 Worker。
schema 有效但原生不支持的布局属于内部权威分歧，不是产品拒绝。完整已验证命令
相等性，包括 Site、预期修订、上下文、Tab 和方向，是重放指纹。保留的精确重放返回固定原结果/
修订；同 ID 内容改变则冲突。
[Server 优先级及提交](../../apps/orchbee-server/docs/workbench-authority.zh-CN.md#已接受的规范-tab-移动模型)
拥有授权、退役、容量和发布。严格语料在两个语言 binding 中覆盖每个方向及错误/额外字段。
消费 adapter 单独校验请求/结果关联及 action 兼容性。OBCS Open 不变。此扩展和两种二进制及两个 binding 作为一个未发布批次激活
及回退，不提供旧 action 别名或混合版本兼容性。

## 已接受的 Panel Move 线协议契约

`panel.move { terminalContextId, tabId, panelId, targetPanelId, direction }` 在 Tab 内重定位一个叶子
Panel，direction 严格为 `above | below | left | right` —— 即 `panel.split` 使用的同一
`WorkbenchPanelSplitDirection` 文法。above/left 在目标叶子之前插入源叶子，below/right 在其后插入，
两者都建立一个新的垂直/水平 Split。源叶子与目标同名，或这对叶子已是匹配方向 Split 按隐含次序排列的
直接孩子（即布局已满足）时，产生现有 `Unchanged { revision }`。实际重定位产生
`PanelMoved { revision, terminalContextId, tabId, panelId, targetPanelId }`（线上 outcome 为
`panel-moved`），恰好推进一个修订。源叶子保留其完整 Panel 节点——身份、Presentation 与绑定——新
Split 是唯一新增结构：一个 fresh Split 身份加两个 fresh Slot 身份，同时源叶子空出的父 Split 折叠为
兄弟子树。不允许任何终端或查看器效果。指定 Tab 中不存在的源/目标叶子为 `PanelUnavailable`；缺失
Tab 为 `TabUnavailable`；未知上下文为 `TerminalContextUnavailable`。Server 优先级、重放保留、身份
分配与发布遵循共享命令规则；严格语料在两个语言 binding 中覆盖每个方向、no-op 及错误/额外字段。

## 已接受的 Tab Close 线协议契约

[Server Close 契约](../../apps/orchbee-server/docs/terminal-tab-close.zh-CN.md) 拥有生命周期
准入、不可变接受、状态、显式 Resume 和发布。其严格线协议投影独立于 Server/Worker/原生
运行时激活分阶段交付。`tab.close` action 仅增加 `terminalContextId` 和 `tabId`；
`panel.close` 额外携带 `panelId`。
`tab-close-accepted` 携带 `closeRequestId`、`terminalContextId`、`tabId` 和
`acceptedRevision`；`panel-close-accepted` 回显相同字段并附加 `panelId`。别名保留主操作最初
准入修订，该值不是成员移除修订。叶子级操作共用同一份保留的 Close 记录、
`workbench.tab-close.read` 状态查询及 `workbench.tab-close.resume` 控制；这些线协议结构
不携带 `panelId`。

无副作用的 `workbench.tab-close.read` 与显式 `workbench.tab-close.resume` 请求、结果、
每个状态及原因和必需目标回显遵循该所有者记录。所有对象保持封闭。`attemptGeneration` 与
`expectedAttemptGeneration` 共用独立于修订的 `WorkbenchTabCloseAttemptGeneration`
值域，采用 `1..=18446744073709551615` 的规范十进制文本。递增经过检查；零、溢出及非规范
文本失败。OutcomeUnknown phase 精确为 `terminate | force | dispose`；Closing phase
独立为 `queued | waiting-grace | waiting-outcome | disposing`。

OBWP 保留 kind 1/2/3/4，增加 5 表示 Close 读取、6 表示 Close 读取结果、7 表示 Resume、
8 表示 Resume 结果。kind 5..8 保留 16 KiB 编码限制及精确方向/kind 检查；仅快照结果保留
更大限制。保持 version 1 及未发布批次部署方式。

Command Unavailable 额外准入 `terminal-busy` 和 `terminal-runtime-unavailable`。
规范案例 `command-uses-terminal-unavailable-reason` 在这一有意的未发布语法扩展中有效。
其他既有预期判定不变。解析验证语法；消费 Move/Create adapter 仍须拒绝不适用于 action 的
原因及结果。规范夹具和独立 binding 证明语法一致性，不证明生命周期效果、重放不可变性、
授权、状态转移或最终清理。

## 限制

| 值                        | Version-1 限制                 |
| ------------------------- | ------------------------------ |
| Encoded read intent       | 16 KiB                         |
| Encoded snapshot 或 delta | 1 MiB                          |
| Product label             | 512 个 Unicode code point      |
| Project                   | 每个 snapshot 256 个           |
| Directory                 | 每个 Project 256 个            |
| Directory path            | 4,096 个编码 UTF-8 字节        |
| Workspace                 | 每个 Project 256 个            |
| Terminal Context          | 每个 snapshot 1,024 个         |
| Tab group                 | 每个 Terminal Context 64 个    |
| Tab                       | 每个 Terminal Context 256 个   |
| Child Presentation        | 每个 Panel 16 个               |
| Panel/Split depth         | 从 Tab root 开始 16 个 node    |
| Panel/Split node          | 每个 Terminal Context 4,096 个 |
| Delta change              | 每个 delta 1,024 个            |

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/workbench-client-protocol test
cargo test -p orchbee-workbench-protocol --locked
pnpm verify
```

这些检查只证明 schema/binding conformance 与 pure delta refinement。它们不证明 Server、
transport、authorization policy、persistence model、renderer、CLI command 或 mobile
implementation。

## 原生 Windows 路径细化

线协议中的绝对路径接受 POSIX 前导斜杠，或 ASCII 盘符后跟 `:` 及 `/` 或 `\`；Windows 逐字盘符路径可以带 `\\?\` 前缀。词法规则不依赖解码器所在平台；字节保持原样，不展开、不规范化。本次 Windows 增量不接受反斜杠根路径、盘符相对路径、UNC 或设备路径。既有字节上限和 NUL／Unicode 校验不变。执行方仍须在产生效果前依据本地文件系统验证路径。

## 保留 Worktree 操作

[`workbench-worktree.schema.json`](v1/workbench-worktree.schema.json) 拥有独立的 `workbench.worktree.operation` 请求与 `workbench.worktree.operation.result` 回答。OBWP kind 19/20 对应这两个方向，各有独立 96 KiB 分配前上限。操作 ID 与交换请求 ID 分离；Start 保留 Project 及意图，Read 观察，Resume 约束前次尝试（1–64）。结果独立保留 Git 事实与 Workspace 状态，包括关闭后移除失败。[共享语料](v1/workbench-worktree-conformance-v1.json) 由 Rust 与 TypeScript 双方消费，包含 JSON Schema 结构检查及明确的语义字节／跨字段检查。线协议本身不授予变更权限，也不暗示 Desktop 支持。运行时及 CLI 生命周期归 [Windows Worktree 所有者](../../apps/orchbee/docs/windows-worktrees.zh-CN.md)。

## 按来源查询 Worktree

[`workbench-worktree-query.schema.json`](v1/workbench-worktree-query.schema.json) 拥有 `workbench.worktree.query` 及其结果，使用 OBWP kinds 21/22。请求保留 16 KiB intent 上限；响应使用独立的 32 KiB 分配前上限。Workspace 或字面目录来源无需变更操作。结果包含 Git 检出事实及零个或多个规范 Workspace 关联。Rust 和 TypeScript 校验必填可空来源／退出码字段、严格闭合失败变体、UTF-8 路径／引用上限、唯一检出／Workspace 身份及来源／根目录／linked 关系。两者消费[共享查询语料](v1/workbench-worktree-query-conformance-v1.json)；JSON Schema 覆盖结构约束，绑定额外校验字节上限与跨条目关系。运行时行为属于 [Windows Worktree 所有者](../../apps/orchbee/docs/windows-worktrees.zh-CN.md)。

## Workspace 组关闭线上合同

[`workbench-workspace-group-close.schema.json`](v1/workbench-workspace-group-close.schema.json) 定义 `workbench.workspace-group-close` 及其结果，OBWP kind 为 23/24。请求沿用 16 KiB intent 上限；结果具有独立的 512 KiB 分配前上限。Start 捕获作用域，Read 在源移除后仍使用操作 ID，Resume 对 1–64 次尝试设置围栏。Available 应答携带 1–128 个唯一有序成员，选定源排在最后。子记录为 null 表示尚未提交；子记录即使必须存在、可为 null 的观测不可用，也保留 Close 请求 ID。现有 Close 拥有状态与代次定义。绑定约束前序已关闭前缀、子 ID 唯一性、完整 Closed 声明及未知观测不可恢复。[共享样例](v1/workbench-workspace-group-close-conformance-v1.json) 覆盖 Rust、TypeScript 和结构 schema 校验。[运行时归属](../../apps/orchbee/docs/windows-worktrees.zh-CN.md#workspace-组关闭) 提供授权、物理清理和单调保留观测。

## 易失 Client 视图控制

`presentation` 被动读取一个 Panel 的精确 Client 本地成员。`presentation-read` 回执包含
规范修订、Panel ID 及必须存在但可为 null 的选择：Presentation ID、绑定、本地附着代次／
实例编号与已安装视口。载荷限制为 4 KiB。绑定验证精确请求 Panel，以及不早于最低修订的
Ready/Busy 视图；Applied 不得确认此读取。不改变 Client 选择字段或规范数据。

`viewports` 读取 1–16 个有序唯一 Panel ID；`viewport-read` 返回规范修订号、可空主绑定以及
不可用、待处理或实时/历史事实。位置计数为十进制 uint64，偏移由总行数、起始行和实际 Client
视口高度计算。来源保留 Host epoch/attachment/generation/revision 及几何。
独立绑定验证指标公式、精确请求成员、非零来源身份及 Ready/Busy 成功；待处理状态不能伪造位置。
载荷限制为现有帧内的 12 KiB，包括所有者完成消息。Windows 所有者预留内存、允许 Busy 期间
只读观察历史，并保留变更准入限制。查询不创建 viewer，不请求历史数据或确认。

[`workbench-client-view.schema.json`](v1/workbench-client-view.schema.json) 拥有 Client 发现、仅所有者可执行的同步及经确认的导航，与规范 Workbench 快照分离。OBWP kind 25／26 将请求限制为 16 KiB、结果限制为 64 KiB。独立 Rust／TypeScript 绑定及[共享语料](v1/workbench-client-view-conformance-v1.json) 校验必填可空字段、非零 u64 序列、最多 32 个唯一 Client、同步命令版本约束，以及已观测 Applied 或单 Panel zoom-unchanged 结果。[Windows 运行时契约](../../apps/orchbee/docs/windows-client-control.zh-CN.md) 拥有新鲜期、回执保留、路由、绘制后确认和停机。`ClientViewControl` 是登记的有界生命周期模型；模型执行是单独的证据。

## Client 布局观测

Client 视图契约还拥有 `layout` 意图及 `layout-read` 完成／状态。OBWP 帧类型 27/28 使用专用的 1 MiB 分配前上限；25/26 保留 16 KiB／64 KiB 限制。载荷保留必填但可为 null 的焦点与几何、规范 Panel／split 身份、有限比例、源可用性及四个重新计算的邻居。Rust 与 TypeScript 独立依据扩展共享语料校验边界、引用、实际单元格包含关系及精确请求／视图对应关系。[Windows 布局所有者](../../apps/orchbee/docs/windows-client-layout.zh-CN.md)定义几何、内存预留及只读行为。

方向控制新增 `focus-direction` 意图、`direction-focused` 和 `direction-unchanged` 回执，沿用普通 25/26 帧。成功聚焦校验区别于来源的精确目标和已观测视图；无邻居回执校验来源布局版本，支持当前选择位于其他上下文。共享语料覆盖闭合形状、版本、目标、意图不匹配和无效方向。

分割调整在 25/26 帧新增严格的 `resize`、`set-split-ratio` 意图及 `layout-adjusted` 回执。数量为有限的 0–0.5，比例为有限的 0.1–0.9，二进制分割路径最多 16 个索引。绑定独立校验目标／版本／视图关系、互不相同的有序 slot、精确 setter 数值或夹取后的有符号 resize 增量，以及仅 resize 可用的 no-split 结果。普通 Applied 不能确认这两种意图。共享语料包含 64 个有效、115 个无效文档；结构 JSON Schema 检查补充独立语义校验器。运行时生命周期、绘制顺序和非活动保存归 Windows 布局所有者管理。

## 窗格自定义标题

[窗格标题契约](../../apps/orchbee/docs/native-panel-labels.zh-CN.md) 增加可选的规范 `Presentation.customLabel` 及携带精确 context/Tab/Panel ID 的 `panel.rename`。存储的自定义标题必须非空、已裁剪、最多 512 个 Unicode 码点且不能为 null；省略该字段保持旧快照形状。更新的 `label` 可省略或为 null，先限制原文长度，再按 Unicode White_Space 裁剪，空白输入规范化为 null。Rust 与 TypeScript 独立实现该规则（NEL 是空白，BOM 不是）。创建时的 `label` 及所有身份和绑定保持不变。此扩展属于统一升级的实验性 OBWP v1 cohort，旧的严格读取器可能拒绝新字段或动作。共享命令一致性语料覆盖规范设置、清除及规范化输入。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。Windows CLI 通过原 Client 单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](../../apps/orchbee/docs/native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

`set-split-ratio` 接受可选 `expectedSplit:{splitId,firstSlotId,secondSlotId}`。提供时三者均必填，两侧槽位必须不同；显式 null 非法。Client 在写入前比较精确身份，`layout-adjusted` 回执也必须匹配三者。省略时保留原路径设置行为。共享语料覆盖替换身份、缺字段及槽位重复；该附加字段要求匹配 cohort，未增加 Desktop CLI 命令。

[Panel 输入契约](../../apps/orchbee/docs/native-panel-input.zh-CN.md) 新增归 Panel 所有的可选 `rightClick`（默认 `orchbee`，或 `panel`）、`panel.input.set` 的必填策略、`panel.split` 的可选策略及 `panel-input-configured` 回执。null 与未知值无效。纯修改保持成员关系；分屏只为新 Panel 设置策略。Rust、TypeScript 和 JSON Schema 消费同一组一致性样例。不增加 Desktop CLI 命令，要求匹配的开发 cohort。

## Tab 插入边界契约

`tab.move-index { terminalContextId, tabId, insertIndex }` 扩展共享规范 Tab 重排；原 `tab.move`
方向契约不变。`insertIndex` 是 0..256 内的整数，在原始顺序中表示零基插入边界。Server 在精确
上下文/目标和树成员组合验证后拒绝超过当前 Tab 数量的值，原因是
`tab-move-index-out-of-bounds`。源边界及其后一边界均无变化；总数量边界表示末尾。
方向和索引移动均保留固定／分组元数据。精确位置若破坏固定前缀或打散连续分组，则返回保留的
`tab-move-order-conflict`，不修改状态、退役或隐式重新分组；合法位置保留完整 Tab 记录。
复用 `tab-moved` 和 `unchanged`、同一 revision、保留结果及原子提交 owner，不引入终端或焦点效果。
请求指纹包含 action 类型和插入边界。语料覆盖上下界、整数小数表示、缺失/多余/冲突字段、
负数、分数、字符串、布尔和空值拒绝。匹配实验批次一同发布和回退；不支持混合旧严格解码器，
无持久化迁移。可执行证据见拥有该行为的 Server/原生 CLI 测试，TLA 模型本次不执行。

## 自动 Tab 命名

[Tab 标签契约](../../apps/orchbee/docs/native-tab-labels.zh-CN.md) 拥有可选 `automaticLabel` 元数据。省略标签的新建 Tab 按当前上下文位置编号；显式数字名称及旧 Tab 保留固定名称。移动或关闭后 CLI 查询、标签选择器和 TUI 显示保持一致，重命名切换为固定名称。原生 `api snapshot` v4 暴露规范元数据；目录 v34 包含 68 份 schema。共享协议需要匹配 cohort。

Client 所有的 Agent 提示状态读取指定 1–100 个唯一规范 Agent 会话 ID。`agent-attention` 提交及 `attention-read` 完成使用类型 29，分配前限制为 64 KiB；响应仍使用类型 26。条目保留请求顺序、可空的不存在会话状态、观察 generation/sequence、语义变化版本及 seen，同时包含可认证启动周期的 changes 游标和可空宿主焦点。Workbench 绑定轻量 Agent 协议 DTO，不拥有 Agent 状态。[Client 提示状态所有者](../../apps/orchbee/docs/native-client-attention.zh-CN.md) 定义读取可用性、不改变确认、有界回执存储及 CLI 游标认证。

物理 `terminal-agent-attention` 读取指定 1–100 个唯一完整展示／创建／Host 绑定；
`terminal-attention-read` 保留有序的可空序号／seen 条目、报告修订及可空外层焦点。
它们复用类型 29／64 KiB 的提交／完成封套及读取生命周期。意图／结果关联要求精确绑定
和顺序，不合成规范 Session。[物理确认所有权](../../apps/orchbee/docs/native-client-attention.zh-CN.md#物理终端观察与绘制)
定义 Server／Client 合并及绘制门槛。

## 保留终端的 Panel 迁移

`panel.relocate` 保留精确的源上下文／Tab／Panel 元组。`tab` 目标接受可选 `terminalContextId` 和
`targetPanelId`：上下文缺省使用源上下文，Panel 缺省保留根部嫁接。`new-tab` 接受可选
`terminalContextId` 和 `label`，标签缺省创建自动编号 Tab。`new-workspace` 要求 `projectId` 和
`label`，可选 `primaryDirectoryId`、`primaryDirectoryPath` 与 `tabLabel`。目录 ID 与路径互斥；路径要求 Worker 验证，在迁移时原子创建 Directory，并由可选 `transfer.createdDirectoryId` 返回。可选字段缺省时省略，显式 null 非法；目录必须
属于所选 Project。现有编码目标继续有效。

`panel-relocated` 和 `panel-relocated-to-new-tab` 保留原有身份字段，可选携带 `transfer`：
必填 `targetTerminalContextId`、`closedSourceTab`，可选 `closedSourceWorkspaceId`、
`createdWorkspaceId`。跨上下文移动或移除源 Tab 时提供该对象；新 Workspace 迁移使用 new-Tab
结果并提供新 Workspace 身份。原有同上下文且不移除源 Tab 的结果省略 transfer。全部终端绑定保留。
[Server 迁移模型](../../apps/orchbee-server/docs/workbench-authority.zh-CN.md#已接受的-panel-迁移模型)
拥有源移除、候选容量、发布和 standalone 组合；Client 比例与焦点不属于此次原子结构结果。
共享命令夹具覆盖新旧形式、缺省标签及非法／null 可选值。

## Workspace 元数据

[`workbench-workspace-metadata.schema.json`](v1/workbench-workspace-metadata.schema.json) 定义 `workbench.workspace-metadata` 及其结果，OBWP 类型为 31/32。Read 接受 0–8 个有序且唯一的 Workspace ID；Report 接受 1–16 个唯一 token 补丁，可附 uint64 序号及有界 TTL。结果具有独立的 128 KiB 分配前上限；请求保留 16 KiB 意图上限。[共享语料](v1/workbench-workspace-metadata-conformance-v1.json) 检查两种语言绑定。绑定还执行 uint64 范围、结果身份唯一性及编码字节限制；Rust 原始解码拒绝重复 token 键。[所属模型](../../apps/orchbee/docs/windows-workspace-metadata.zh-CN.md) 定义权限、过期、容量及查询一致性。

[Panel 元数据 schema](v1/workbench-panel-metadata.schema.json) 与 [一致性用例](v1/workbench-panel-metadata-conformance-v1.json) 定义 OBWP 33/34 上报和读取。Rust 与 TypeScript 还校验 JSON Schema 之外的行／来源／顺序语义不变量；Rust 原始解码也拒绝重复键。

[Panel Agent schema](v1/workbench-panel-agent.schema.json) 还覆盖固定代际的物理
`observe`／`observed` 读取。共享语料检查必填可空游标、非零 Host 身份和语义变化形状；
语言绑定额外约束连续序号、头部和当前状态一致。精确游标覆盖由请求／结果关联负责。

物理 `explain` 使用相同的精确绑定和读取权限。`explained` 结果分别保留当前上报视图与原始
屏幕规则判断，清单、胜出规则、跳过/回退原因和采样字段均必填且可为空。Rust/TypeScript
校验最高优先级首条胜出规则、状态/标记一致性、证据上限及完整生命周期跳过/来源一致性。
未知 provider 拒绝；没有清单的已知 provider 明确回退为空闲。沿用 512 KiB 结果上限。
这份未发布 cohort 一起变更；规范 OBAP 解释保持独立。

生效规则来源为物理 Explain 增加可选 `manifest` 对象：来源（`bundled`、`local-override`、
`remote-cache`）、必填且可空的实际路径／缓存版本、遮蔽情况及有界类型化告警。
省略信息保留旧形状，显式 null 拒绝。两个 binding 都校验远程来源／版本及告警来源唯一性。
CLI 携带来源信息时输出 v3，并保留封闭的 v1/v2 封套。

精简物理 `states` 读取接受 0–64 个唯一展示 ID，按请求顺序返回条目、Workbench／报告修订，
以及必填可空的完整绑定／序号／Agent／状态快照。成员不存在则拒绝整批；没有当前物理观察
则返回 null。空读取用于报告修订门槛。Rust／TypeScript 共用严格形状、唯一性／顺序检查和
一致性语料；Server 继续负责当前 Worker 及物理身份准入。

`agent-focus` ClientView 意图保留精确 context／Tab／Panel／Presentation，以及规范 Session
或完整物理 Agent 目标。绑定验证非零物理身份及展示对应；应用回执要求精确 Ready Client
选择。它复用既有导航帧及保留操作生命周期。[Agent 导航](../../apps/orchbee/docs/native-agent-navigation.zh-CN.md)
负责显式确认及身份丢失后的拒绝。

## 已安装的 Agent 规则操作

`workbench.agent-manifests` / `.result` 使用 OBWP 37/38，结果上限 512 KiB。
严格的 Rust、TypeScript 和 JSON 契约覆盖已安装快照及 pending/applied/failed 重载回执。
当前快照及操作查询只读；重载使用明确的 Site 生命周期操作 ID。
共享 `workbench-agent-manifests-conformance-v1.json` 语料覆盖接纳与拒绝的形状、可空字段、
数字版本、来源、修订及重复项。语义字节／版本及跨字段校验补充 JSON Schema。
这些 DTO 描述实际已安装状态，不声称支持远程下载／更新。Server 屏幕模型拥有安装和回执上限。

两个 Panel 载体还支持 `read-presentations` 和 `presentations-read`，用于精确主项或子项报告／元数据观察：四个有序唯一 Presentation ID、空读取修订校验、必需的所属 `panel` 行，以及不同成员共享 Panel ID。Rust／TypeScript 与主项读取同样校验载荷不变量及元数据字段顺序归属。既有主项读写形状不变。共享用例覆盖精确成员数量界限、严格包装、同 Panel 成员和未绑定载荷；原生调用者还关联 Presentation、Panel 和 binding。协议应与其 Server 及原生 TUI 消费者作为匹配 cohort 交付。

## Panel 终端创建扩展

`panel.create-terminal` 指定 Context／Tab／Panel、必填 geometry 及一个封闭目标：
`child { label }` 或 `unbound { presentationId }`。可选 `launch { exec, args }` 与 `profile`
在非 null 时互斥。可选 `options` 提供绝对 `cwd` 及字面值 `environment` 名称／值二元组。
共享 Rust／TypeScript／JSON 语料覆盖结构、身份、程序选择、字节上限、环境限制及规范化。
null 选择器和空 options 规范化为省略字段。Launch 最多 128 个参数，UTF-8 总量及最坏转义内容均限
30 KiB；profile 限 512 UTF-8 字节；环境变量限 32 个 ASCII 大小写不重复项、1024 总字节，
并排除托管名称。请求使用既有 96 KiB 原生启动信封。JSON Schema 标明由绑定语义层执行的
聚合字节及唯一性规则。

`tab.create` 和 `panel.split` 使用与 `panel.create-terminal` 相同的可选 launch／profile／options
字段、封闭嵌套结构、规范化和 96 KiB 命令信封。三者均保留字面参数和环境变量值；
同时提供非 null 的 launch 和 profile 无效。Tab 创建的默认 `automaticLabel:false` 与 Split
的默认 `rightClick:orchbee` 规范化为省略，与 Rust 序列化一致。null `options` 和 null 环境数组
无效；null cwd 和空环境数组省略。共享语料跨 Rust、TypeScript 和 JSON Schema 检查这些
既有原生启动能力，包括超过普通意图限额的扩展请求。本扩展不增加 Desktop 命令，
也不改变 Server 启动、授权、重放或回滚归属。仅原生提供的 Workspace 启动和 profile 目录
操作仍不在此次绑定扩展范围内。

`panel-terminal-pending` 和 `panel-terminal-created` 携带修订、Context／Tab／Panel／Presentation；
最终变体包含既有 terminal outcome，均拒绝分屏专用字段。
[Server 所有者](../../apps/orchbee-server/docs/workbench-authority.zh-CN.md#在既有-panel-内创建终端)
要求导航及终端创建权限，并负责预备、效果及结算。既有分屏语法／结果字段不变。匹配的私有 cohort
一起上线；扩展自身不增加持久化迁移或公共 CLI 命令。

## 无身份布局结构创建

命令绑定还接受 `tab.create-layout` 及其最终 `tab-layout-created` 回执。
经过验证的递归模板携带标签、手工覆盖、Panel 输入策略及二叉 Split 方向，不包含调用方提供的身份或绑定。
除封闭的 JSON 结构外，Rust 和 TypeScript 还限制最多 24 个叶节点、16 层和每个叶节点的 16 个子成员。
整个命令上限为 1 MiB；虽然共享命令帧可以承载较大模板，其他命令仍保留各自的限额。
Server 负责原子分配和发布；物理成员创建仍是独立的保留命令。
此私有契约本身不代表公共 CLI 导出与应用往返流程已完成。

## Site 配置发布

`workbench.configuration`（OBWP 类型 39）携带 `current` 或 `reload`；`workbench.configuration.result`（类型 40）携带封闭的 current/reloaded/failed 结果。每帧上限 256 KiB。[配置 schema](v1/workbench-configuration.schema.json) 负责封闭的结构形状与码点数量上限；共享语料检查结构边界与 UTF-8 语义边界，独立 Rust/TypeScript 绑定另行执行所述 UTF-8 字节上限。快照版本 0 表示启动基线，正版本表示显式发布。每个成功快照都包含必需且封闭的 `diagnostics` 记录；它至多含 128 个条目，按 Unicode 标量值将 `(field, reason)` 严格升序排列且二元组不重复，`omittedCount` 范围为 0–65536。每个非空 field 不含控制字符且至多 256 个 UTF-8 字节；每个 reason 是至多 64 字节的小写 ASCII kebab token；disposition 只能是 `defaulted`、`disabled`、`ignored` 或 `unsupported`。仅含键位的规范配置上限 64 KiB；重启字段不可重复；可选失败诊断为必需但可空的成对字段（128 个 UTF-8 字节、无控制字符，仅载入失败允许）。请求和 Site 标识必须匹配，成功结果必须对应请求动作。

[Windows 配置归属契约](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md#在线配置重载) 负责授权、冻结路径、候选校验、发布、Client 应用和关闭。发布不表示所有 Client 已确认。已提交重载丢失响应仍为未知，不自动重试。不新增 Desktop 命令；匹配的私有版本整套交付，不迁移持久化格式。

必需 `defaultShell` 和 `shellMode` 暴露后续终端准备所用的已接纳默认值，不是 Worker 解析后的事实。可执行文件为空表示平台默认，否则为最多 4096 UTF-8 字节、无控制字符的字面量；模式为 `auto`、`login` 或 `non_login`。匹配的私有 cohort 一同升级。公开 CLI configuration v2 映射这两个字段，历史 v1 schema 保持封闭。

## Client 临时通知

`workbench.notification`（OBWP kind 41）承载 show/listen；`workbench.notification.result`（kind 42）承载 show/listening/failed。帧保留 16 KiB intent 上限。[通知 schema](v1/workbench-notification.schema.json) 及共享语料约束 Rust 与 TypeScript 的封闭结构、必需可空的 body/pending 值、四角位置和 none/done/request 声音。规范化标题/正文至多包含 80/240 个 Unicode 标量，只保留合并后的 ASCII 空格且无控制字符。关联必须匹配两个身份及请求动作；shown 仅在 shown 原因时为 true。

[Windows 通知归属](../../apps/orchbee/docs/windows-notifications.zh-CN.md) 定义授权、连接拥有的有界收件箱、过期、重放与 UI 交接。这是临时、匹配 cohort 的传输，不新增持久化迁移或 Desktop 命令。公开的 `orchbee.notification.show` v2 保留 v1 并暴露提交事实，包括不确定提交后的 null 回应；不保证显示确认。

## 外层 Client 标题发布

`workbench.window-title`（OBWP 43）承载 set/clear/listen；`workbench.window-title.result`（44）承载 published/listening/failed，保持 16 KiB intent 上限。[封闭 schema](v1/workbench-window-title.schema.json) 及共享语料约束 Rust 和 TypeScript：必需可空标题、正十进制 u64 revision、无控制字符且 trim 后至多 200 个标量的标题，以及 reason/changed/title 一致性。请求与回应的身份、Site、动作必须匹配。

[Windows 标题归属](../../apps/orchbee/docs/windows-terminal-title.zh-CN.md) 定义有界重放、连接焦点路由及恢复。公开的 `orchbee.terminal.title` v1 回执区分发布和实际显示。这项私有 cohort 增量不改变持久化文档。

## 窗口标题模板输入

匹配 cohort 的 `workbench.window-title` listen 动作要求 `context`，包含 workspace/tab/pane/terminalTitle；每项为无控制字符且至多 200 个 Unicode 标量的字符串，允许为空。仅前台渲染使用这些展示值，不授予选择权限。`ConfigurationSnapshot.windowTitle` 是必需原始字符串，限制为 4096 个 UTF-8 字节，按文档中的建议性策略允许空值和无效模板。Rust/TypeScript 绑定执行字节/标量上限；schema 与语料保证字段存在及封闭结构。公开配置回执升级为 v3、检查为 v7、配置失败为 v4、目录为 v107；旧版本继续保持封闭。

## 自定义通知策略投影

匹配的私有通知消息要求可空 `position`，null 表示由接收 Client 策略决定。Show 结果新增 `disabled`，用于 Server 关闭且没有 Client 的情况；连接中的 Client 可以独立关闭视觉显示。配置快照要求非空且最多 16 KiB UTF-8 的 `notificationProfile`；Client 仅在通知配置和键位均有效时共同应用。公开通知回执升至 v2，Site 配置 v4，检查 v8，配置失败 v5，目录 v108。旧公开版本保持封闭。既有原生 Windows [通知归属](../../apps/orchbee/docs/windows-notifications.zh-CN.md) 管理效果与关闭。

命令启动选项 schema 仅在 `tab.create` 和 `panel.split` 上受理有界 `pluginPane` 选择器，与显式程序／profile 字段互斥。Rust 与 TypeScript 通过命令一致性语料共享选择器及冲突案例。这是匹配 cohort 的扩展，不增加线协议种类，也不受理子终端／Workspace 插件启动。

插件 PaneGet／PaneFound 复用 OBWP 45／46，承载封闭成员事实，不新增修改权限。Rust 额外校验响应 Panel 与请求的关联；Pane 选择器沿用启动选项的标识规则。

Agent 侧栏发布增加必需的非空 `agentSidebarProfile`，上限为 32 KiB UTF-8。它是共享 Windows 配置所有者接纳的封闭规范 JSON；传输绑定约束字符串和整帧边界，Client 安装时验证 token/布局/规则语义。键位、通知、侧栏原子安装；无效侧栏 profile 保留全部原 Client 设置。匹配 cohort 一起更新。公开 `config check` v10、配置失败 v7、Site 配置回执 v5 保持旧版本封闭；目录 v122 仍含 105 个 schema 和 141 条命令路径。

物理标题读取以 `titles` 扩展封闭 Panel Agent 契约：至多四个精确物理
绑定，每个绑定对应一个有序观察。观察为不可用，或携带正元数据版本及
必需可空标题的已观察值，标题最多 64 KiB UTF-8。
除 JSON Schema 外，Rust 和 TypeScript 校验展示 ID 唯一性及字节上限。
Site 在 Worker 读取前后保护保留的物理身份。标题不改变报告版本、
Agent 序号、源过滤或 Client 已读状态；公开 CLI 输出版本不变。
匹配的私有 cohort 一起更新。共享一致性案例覆盖原始 Unicode、清除、
缺失、可空、版本、封闭字段及上限。

## Workspace Git 状态读取

[`workbench-workspace-git.schema.json`](v1/workbench-workspace-git.schema.json)
拥有 `workbench.workspace-git` 及 `.result`，OBWP kind 49/50，分别在分配前限制 8 KiB。
观测回执保留 Workbench 修订及必需 nullable branch/divergence；
失败为 denied/unavailable/not-found/source-changed/timeout/malformed。
分支最多 1024 个 UTF-8 字节，计数为 uint32；null 分支禁止计数。
[共享语料](v1/workbench-workspace-git-conformance-v1.json)检查封闭形状、nullable 字段、字节／计数边界及精确身份；
绑定还验证结构 JSON Schema 之外的语义约束。
[Workspace 侧栏所有者](../../apps/orchbee/docs/machines.zh-CN.md#工作区侧栏模型)
定义规范目录／Worker 保护、Git 生命周期及可见消费。

Workspace 侧栏发布增加必需非空 `spacesSidebarProfile`，独立于 `agentSidebarProfile` 限制 32 KiB。
配置帧包含 JSON 转义限制 256 KiB；Client 安装前先验证全部四个 profile。
公开 config check v11、配置失败 v8 和 Site 配置回执 v6 保持所有旧变体封闭。
目录 v123 仍有 105 个 schema／141 条路径。

## 配置命令线协议契约

[配置命令](v1/workbench-configured-command.schema.json)封闭原生已安装命令目录、调用及私有独立popup生命周期。原生CLI的 `orchbee.command` 内嵌同一组嵌套回执，Client popup后续操作使用control v11。ID标识规范化已安装配置，不授权调用者提供可执行文本。必须出现的可空source与selection字段区分全局shell／plugin动作及当前呈现绑定调用；plugin选区文本从准确终端版本捕获。除JSON形状外，Rust和TypeScript检查ID／来源／上下文关联、行跨度、共享字节上限及嵌套回执身份。[原生模型](../../apps/orchbee/docs/windows-custom-commands.zh-CN.md)拥有执行、不确定副作用、进程保留与回收语义。

规范分割 `ratio` 可省略为等比例默认值，有限数值限制为 0.1–0.9。`layout.set-split-ratio` 携带确切有序身份、修订及原／新比例，结果为 `split-ratio-configured`。cohort 及持久兼容性见[共享比例归属](../../apps/orchbee-server/docs/shared-split-ratios.zh-CN.md)。

## 命名 chrome 主题发布

匹配 cohort 的配置快照必需 `themeName`，精确取值为 `terminal-native`、`one-light` 或 `one-dark`。
Rust、TypeScript 和 JSON Schema 在缺失、类型错误或未知值时拒绝整份快照。
Client 先准入全部四个 profile 及命名选择再安装任何字段；重复、过期或错误快照保留已安装调色板。
仅主题变化沿用完整设置发布的既有修订。[原生主题所有者](../../apps/orchbee/docs/native-themes.zh-CN.md)
定义启动投影、颜色能力、整帧应用及回退。此匹配 cohort 线协议增量不迁移 checkpoint。
公开 Site 配置 v9 和 Windows config check v14 保持所有旧版封闭；目录 v142 保持既有 schema 和命令清单。

## 原生设置协议契约

[设置](v1/workbench-settings.schema.json) 定义匹配 Windows cohort 的闭合 Open/Save 请求及结果。三个必需但可空的字段区分未修改设置与明确的主题／提醒／声音修改；全部为空的补丁非法。来源令牌区分文件不存在与规范 SHA-256 内容摘要。Save 同时约束发布 revision；结果区分磁盘持久化、在线应用与写入前拒绝。共享语料在 Rust、TypeScript 和 JSON Schema 中检查额外字段拒绝、全宽 revision、完整快照及独立的 4 KiB 请求／256 KiB 结果限制。[原生设置所有者](../../apps/orchbee/docs/native-settings.zh-CN.md) 负责写入、输入捕获、不确定结果和恢复。
