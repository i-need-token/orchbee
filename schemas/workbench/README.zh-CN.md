# Workbench 跨语言 Contract

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
