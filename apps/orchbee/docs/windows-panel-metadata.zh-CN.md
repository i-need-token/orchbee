# Windows Panel 元数据上报

## 精确成员目标

`--presentation <id>` 选择位置参数 Panel 的一个规范成员，ID 为完整、非零、32 位小写十六
进制。省略时仍指向主项。CLI 在交接前拒绝缺失或未绑定成员；Server 在 Session 门内复核精确
所属及期望绑定。错误所属或过期绑定的写入不能影响其他成员。已有绑定索引元数据所有者
保留来源序列、守卫、资源限制与 TTL。

精确请求／回复使用现有载体中的 `report-presentation` / `presentation-reported`；旧
`report` / `reported` 仍仅针对主项。回复关联请求、Session、Panel、Presentation 和绑定。
所有上报均输出 `orchbee.panel.metadata` 版本 1 并携带 `presentationId`：显式成员 ID 或默认主项的 null，包括未知或失败的已提交结果。文本标识成员。准入前检查请求边界，绝不重放结果不确定的写入。

[Agent 报告目标模型](windows-panel-agent-reports.zh-CN.md#精确成员目标)拥有共用 CLI 解析及
证据边界。元数据轨迹另外保留 token／来源隔离、TTL 与重放语义。不新增载体、依赖或持久化
迁移；配套 cohort 一起发布，通过源码回滚。最终全量验收继续后置。

## 命令与生命周期

`panel report-metadata <panel> --source <id>` 默认向 Panel 当前主终端写入临时展示元数据。
使用精确 Panel ID 或 `@panel:0x<hex>`。选项包括 `--title`、`--display-agent`、可重复的
`--state-label STATE=TEXT`、`--agent`、`--applies-to-source`、`--clear-title`、
`--clear-display-agent`、`--clear-state-labels`、可重复的 `--token KEY=VALUE` 和
`--clear-token KEY`、`--seq`、`--ttl-ms`、`--format text|json`。状态为 `idle`、`working`、
`blocked`、`done`、`error`、`unknown`。[Server 模型](../../orchbee-server/docs/workbench-authority.zh-CN.md#panel-展示元数据)
负责绑定生命周期、来源顺序、守卫和过期。

解析器在连接前校验整次上报。来源由 1–80 个 ASCII 字母、数字或 `:._-` 组成；展示文本移除控制
字符、裁剪首尾空白并保留至多 80 个 Unicode 标量。空 token 值删除对应键，空的可选展示文本
被省略。状态标签要求规范化后文本非空。重复状态／token 选项以后者为准。至少需要一个展示
设置／清除或 token 变更。不能同时设置和清除同一字段，包括显式传入的空文本。纯展示设置会替换来源记录；包含任一
展示清除时采用选择性补丁语义。可选 Agent／来源守卫用于过滤显示，不赋予权威。
显式提供的 `--agent` 裁剪首尾空白后必须非空。已识别的上报别名和可执行路径规范化为上报
标签（例如 `claude`、`agy`、`copilot`、`qwen`）；未知标签保留裁剪后的大小写和内容。
Agent 守卫不会被截断或移除内部控制字符：超出现有 80 标量且无控制字符的通信契约时，
在连接前失败。原生进程 provider ID 保留独立的识别契约。

一次已有 Session 准入和快照共享可取消的五秒期限。只解析一次所请求成员的绑定（默认主项），并作为 `expectedBinding`
提交；Server 拒绝已变化的绑定。未绑定 Panel 在提交前失败。命令只提交一次，不启动 Session、
导航 Client、修改拓扑、改变 OSC 标题或自动重试。交接后的信号保留回执并设置 `interrupted`。
应答丢失、畸形、不匹配或迟到时保留请求 ID，以未知结果和退出码 8 返回。交接前遵循普通 CLI
失败。已知 applied 或 stale 上报退出 0；目标／权限／容量拒绝退出 3，不可用退出 4，无效请求
退出 5，修订号／期限／结果边界失败退出 1。清理失败退出 1，但提交结果已未知时仍退出 8。

`orchbee.panel.metadata` v1 包含请求／Session／Panel ID、observation（带原始应答的 `server`，
或带封闭原因的 `unknown`）、`interrupted` 和 `cleanupFailed`。目录列出当前文档与命令路径。
OBWP 33/34 承载 `workbench.panel-metadata` 及其结果；请求上限 16 KiB，结果上限 512 KiB。
Server 在 Session 锁内按读取权限读取、按导航权限上报。读取返回至多四个有序且唯一 Panel 的来源
记录。元数据与 Workbench 修订号独立。需部署匹配 cohort；旧端拒绝未知通信类型。无持久化
迁移，Session 重启清空上报。

协议／owner 测试覆盖原子变更、重放和 TTL；Rust／TypeScript／JSON 共享用例覆盖边界和畸形
输入；命名管道测试检查包括绑定在内的精确关联、应答丢失、期限和不重放。解析、回执、本地化
及 shell 语法检查覆盖 CLI 投影。TUI 渲染及通用事件仍是独立集成义务；
下述查询投影已消费当前 Server 上报。仅此命令不代表 Desktop 支持。

## 查询观察模型

Panel list/get 与 Agent Get 通过一个有界收集器读取 Server 元数据和显式 Agent 上报。
输入为已准入连接、原始 Snapshot、有序 Panel ID、原始绝对期限及取消信号。两类接口均每批
读取四个 Panel，分别要求自身修订号跨批稳定。每条返回行须匹配原始主绑定及 Workbench 修订号。
`panel current` 则解析一个精确成员，通过 `ReadPresentations` 校验其 Presentation ID、所属
Panel 和可空绑定；见[当前成员信息增强](native-current-panel.zh-CN.md#当前成员信息增强)。
两种读取形式共用相同的全局修订号。
全部数据读完后，先空读校验元数据，再空读校验 Agent 上报；两者稳定区间因此重叠，联合查询
期间任一类都不能静默变化。发生变化以 `metadata-changed` 拒绝查询，包括会改变元数据守卫
上下文的上报变化。外层仍校验规范状态及所选 Client 状态，不自动重试或发布部分投影。

各 `metadata` 对象包含 `source`（Session、Workbench、元数据修订号及其读取请求 ID）、`binding`、
`tokens`、原始来源 `presentations`、守卫 `context`（`agent`、`authoritySource`）及计算出的
`presentation`（title、displayAgent、stateLabels）。同级 `reportedAgent` 包含自身 `source`
（Session、Workbench、上报修订号及上报读取请求 ID）、同一逻辑 `binding`，以及 `view`（Agent 标签、
状态、权威来源及消息）。未绑定行的 binding/view 为 null；已绑定
但没有上报的终端给出明确 unknown 视图。守卫使用真实 Server 上报视图，缺失事实保留 null，
永不满足守卫。不得用通用观察类别、用户命名或规范 provider ID 替代上下文。
自定义上报不会授予规范 Agent 会话身份。

收集器是读取工作流，不是另一个上报负责人。Host 样本不可用时，Server 元数据及上报仍可读取。
Agent Get 使用唯一且存活的主绑定；Agent 已结束、未绑定、多义或成员关系随后变化时，连同终端
样本一起丢弃两者。上报不会替换独立观察到的 OSC 标题、规范 label、状态或就绪性。Host 进程观察、名称与 TUI 发布各自由其真实所有者维护。

Windows Panel list/get 输出 v1，current 输出 v1，Agent Get 输出 v1（带 `--client` 时为 v1）。
每个 Panel 行和非 null 的 Agent terminal 必须包含 metadata 与 reportedAgent。
本地化文本转义有效元数据、上报详情和消息。目录列出当前文档与命令路径。

接纳时将元数据与上报编码行的合计大小限制在既有 CLI 8 MiB 输出预算内。元数据接纳后，上报
仅使用剩余行预算；每批在扩展状态前校验。最终封套仍单独受限。超限或不一致读取不输出部分
结果。测试覆盖两类修订号复核、精确分批、错误／缺失行与绑定、权限、传输丢失、原始共享期限、
封闭当前 schema、带守卫展示、全局 tokens 和转义文本。匹配 Windows cohort 验收实际公开
上报及查询，不代表最终 TUI 或认证 provider 任务证明。

Windows TUI 现通过独立修订号校验的一轮上报／元数据读取，展示当前 Tab 各 Panel 所选 Presentation 的状态。自定义标题保持优先；有效元数据、字面 token 和本地化 Agent 状态绘制在既有单面板、分屏或放大标题栏中。范围、失败与关闭规则见 [Panel 上报展示](windows-panel-report-presentation.zh-CN.md)。

<!-- documentation-pairing: english-sha256=4e0ba2bc086ddc02f5665ed23d000ce309be6c84e8e096640c2f53b9cf62e494 zh-cn-sha256=d09dd4546139e42231fdf78f8fdd97289dd35de1180a466a1fe16de05cb91e56 -->
