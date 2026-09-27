# Windows Panel 元数据上报

`panel report-metadata <panel> --source <id>` 向 Panel 当前主终端写入临时展示元数据。
使用精确 Panel ID 或 `@panel:<suffix>`。选项包括 `--title`、`--display-agent`、可重复的
`--state-label STATE=TEXT`、`--agent`、`--applies-to-source`、`--clear-title`、
`--clear-display-agent`、`--clear-state-labels`、可重复的 `--token KEY=VALUE` 和
`--clear-token KEY`、`--seq`、`--ttl-ms`、`--format text|json`。状态为 `idle`、`working`、
`blocked`、`done`、`unknown`。[Server 模型](../../orchbee-server/docs/workbench-authority.zh-CN.md#panel-展示元数据)
负责绑定生命周期、来源顺序、守卫和过期。

解析器在连接前校验整次上报。来源由 1–80 个 ASCII 字母、数字或 `:._-` 组成；文本移除控制
字符、裁剪首尾空白并保留至多 80 个 Unicode 标量。空 token 值删除对应键，空的可选展示文本
被省略。状态标签要求规范化后文本非空。重复状态／token 选项以后者为准。至少需要一个展示
设置／清除或 token 变更。不能同时设置和清除同一字段。纯展示设置会替换来源记录；包含任一
展示清除时采用选择性补丁语义。可选 Agent／来源守卫用于过滤显示，不赋予权威。

一次已有 Site 准入和快照共享可取消的五秒期限。只解析一次主绑定，并作为 `expectedBinding`
提交；Server 拒绝已变化的绑定。未绑定 Panel 在提交前失败。命令只提交一次，不启动 Site、
导航 Client、修改拓扑、改变 OSC 标题或自动重试。交接后的信号保留回执并设置 `interrupted`。
应答丢失、畸形、不匹配或迟到时保留请求 ID，以未知结果和退出码 8 返回。交接前遵循普通 CLI
失败。已知 applied 或 stale 上报退出 0；目标／权限／容量拒绝退出 3，不可用退出 4，无效请求
退出 5，修订号／期限／结果边界失败退出 1。清理失败退出 1，但提交结果已未知时仍退出 8。

`orchbee.panel.metadata` v1 包含请求／Site／Panel ID、observation（带原始应答的 `server`，
或带封闭原因的 `unknown`）、`interrupted` 和 `cleanupFailed`。目录 v85 包含 89 个 schema。
OBWP 33/34 承载 `workbench.panel-metadata` 及其结果；请求上限 16 KiB，结果上限 512 KiB。
Server 在 Site 锁内按读取权限读取、按导航权限上报。读取返回至多四个有序且唯一 Panel 的来源
记录。元数据与 Workbench 修订号独立。需部署匹配 cohort；旧端拒绝未知通信类型。无持久化
迁移，Site 重启清空上报。

协议／owner 测试覆盖原子变更、重放和 TTL；Rust／TypeScript／JSON 共享用例覆盖边界和畸形
输入；命名管道测试检查包括绑定在内的精确关联、应答丢失、期限和不重放。解析、回执、本地化
及 shell 语法检查覆盖 CLI 投影。完整 Panel／Agent 查询投影、TUI 渲染、通用事件及 provider
权威／会话上报仍是独立集成义务。仅此命令不代表完整 Herdr Panel／Agent 对齐或 Desktop 支持。
