# Windows Panel 元数据上报

`panel report-metadata <panel> --source <id>` 向 Panel 当前主终端写入临时展示元数据。
使用精确 Panel ID 或 `@panel:<suffix>`。选项包括 `--title`、`--display-agent`、可重复的
`--state-label STATE=TEXT`、`--agent`、`--applies-to-source`、`--clear-title`、
`--clear-display-agent`、`--clear-state-labels`、可重复的 `--token KEY=VALUE` 和
`--clear-token KEY`、`--seq`、`--ttl-ms`、`--format text|json`。状态为 `idle`、`working`、
`blocked`、`done`、`unknown`。[Server 模型](../../orchbee-server/docs/workbench-authority.zh-CN.md#panel-展示元数据)
负责绑定生命周期、来源顺序、守卫和过期。

解析器在连接前校验整次上报。来源由 1–80 个 ASCII 字母、数字或 `:._-` 组成；展示文本移除控制
字符、裁剪首尾空白并保留至多 80 个 Unicode 标量。空 token 值删除对应键，空的可选展示文本
被省略。状态标签要求规范化后文本非空。重复状态／token 选项以后者为准。至少需要一个展示
设置／清除或 token 变更。不能同时设置和清除同一字段，包括显式传入的空文本。纯展示设置会替换来源记录；包含任一
展示清除时采用选择性补丁语义。可选 Agent／来源守卫用于过滤显示，不赋予权威。
显式提供的 `--agent` 裁剪首尾空白后必须非空。已识别的上报别名和可执行路径规范化为上报
标签（例如 `claude`、`agy`、`copilot`、`qwen`）；未知标签保留裁剪后的大小写和内容。
Agent 守卫不会被截断或移除内部控制字符：超出现有 80 标量且无控制字符的通信契约时，
在连接前失败。自定义标签的这一限制仍与 Herdr 的上报输入有差异，需在完整 Agent 上报
能力中继续补齐。原生进程 provider ID 保留独立的识别契约。

一次已有 Site 准入和快照共享可取消的五秒期限。只解析一次主绑定，并作为 `expectedBinding`
提交；Server 拒绝已变化的绑定。未绑定 Panel 在提交前失败。命令只提交一次，不启动 Site、
导航 Client、修改拓扑、改变 OSC 标题或自动重试。交接后的信号保留回执并设置 `interrupted`。
应答丢失、畸形、不匹配或迟到时保留请求 ID，以未知结果和退出码 8 返回。交接前遵循普通 CLI
失败。已知 applied 或 stale 上报退出 0；目标／权限／容量拒绝退出 3，不可用退出 4，无效请求
退出 5，修订号／期限／结果边界失败退出 1。清理失败退出 1，但提交结果已未知时仍退出 8。

`orchbee.panel.metadata` v1 包含请求／Site／Panel ID、observation（带原始应答的 `server`，
或带封闭原因的 `unknown`）、`interrupted` 和 `cleanupFailed`。目录 v86 包含 89 个 schema。
OBWP 33/34 承载 `workbench.panel-metadata` 及其结果；请求上限 16 KiB，结果上限 512 KiB。
Server 在 Site 锁内按读取权限读取、按导航权限上报。读取返回至多四个有序且唯一 Panel 的来源
记录。元数据与 Workbench 修订号独立。需部署匹配 cohort；旧端拒绝未知通信类型。无持久化
迁移，Site 重启清空上报。

协议／owner 测试覆盖原子变更、重放和 TTL；Rust／TypeScript／JSON 共享用例覆盖边界和畸形
输入；命名管道测试检查包括绑定在内的精确关联、应答丢失、期限和不重放。解析、回执、本地化
及 shell 语法检查覆盖 CLI 投影。TUI 渲染、通用事件及 provider 权威／会话上报仍是独立集成义务；
下述查询投影已消费当前 Server 上报。仅此命令不代表完整 Herdr Panel／Agent 对齐或 Desktop 支持。

## 查询观察模型

Panel list/get/current 与 Agent Get 通过同一个有界查询收集器消费 Server 元数据。输入为
已准入连接、原始 Snapshot、有序 Panel ID、绝对期限和取消信号。每批读取四个 Panel，要求
所有批次使用同一元数据修订号，将每条返回绑定与原始主成员关系比较，最后空读校验修订号。
外层查询仍校验规范状态及所选 Client 状态。失败不得发布混合或部分元数据投影。

各投影的 `metadata` 对象包含 `source`（Site、Workbench、元数据修订号及所有读取请求 ID）、
当前 `binding`、`tokens`、原始来源 `presentations`、守卫 `context`（`agent`、
`authoritySource`）及计算出的 `presentation`（title、displayAgent、stateLabels）。
缺失上下文显式为 null，永不满足守卫。实际 Agent 权威集成之前，两项上下文都缺失；带守卫
的记录仍作为原始上报可见，但不覆盖显示。不能用 provider ID、用户名称或通用观察类别替代
缺失权威。这项依赖仍属于完整对齐验收范围。

收集器是读取工作流，不是第二个元数据负责人。读取 Server 元数据不要求 Host 可用。
Agent Get 仅使用唯一且存活的主绑定；Agent 已结束、未绑定、多义或成员关系随后变化时，
连同终端样本一起丢弃元数据。上报元数据不会替换独立观察到的 OSC 标题、规范 label、状态或
就绪性。收集器测试需覆盖批间和最终校验修订变化、缺失／不匹配行和绑定、不匹配守卫与保留
的全局 tokens、取消、原始期限及不重放。

Windows Panel list/get 输出 v8，current 输出 v5，Agent Get 输出 v9（带 `--client` 时为 v10）。
每个 Panel 行或非 null 的 Agent terminal 必须包含 metadata 对象。旧查询版本拒绝该字段，
同时仍接受原有形状。文本输出转义后的有效展示字段和全局 token。目录 v86 包含 89 个 schema
和 100 条命令路径。

收集器在接纳批次前，将所有已编码元数据行的总大小限制在现有 CLI 的 8 MiB 输出预算内；
最终封套仍单独受准备阶段的上限约束。超出预算的批次原子失败，返回 output-too-large，
不发布部分结果。
