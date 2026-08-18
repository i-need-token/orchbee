# Unified Search 剩余集成

| 字段     | 内容                                                                                           |
| -------- | ---------------------------------------------------------------------------------------------- |
| 状态     | 已接受；搜索界面和有界异步 Provider 协调已经实现；Agent 与 Terminal 集成仍是未来工作           |
| Owner    | `@orchbee/workbench`、Agent capability、Terminal capability 与应用组合层维护者                 |
| 当前权威 | [`packages/workbench/docs/unified-search.md`](../../packages/workbench/docs/unified-search.md) |
| 退出条件 | 集成剩余的 owner-owned Provider，更新当前权威和测试，然后删除本提案及其译本                    |

英文 [unified-search-surface.md](unified-search-surface.md) 是 canonical 版本；本文用于简体中文评审。

## 当前基础

Workbench 已经拥有唯一的搜索界面。侧边栏、`Mod+K` 和 `Mod+Shift+P` 通过有类型的筛选输入打开同一个
界面。它目前提供本地 Actions 和 Settings 结果，以及带有取消、重试、过期完成拒绝、确定性排序、
无障碍投影、测试和 TLA+ 证据的有界异步 Provider coordinator。

本提案不再重复定义这些已经实现的事实，只负责下面的剩余集成。

## Agent Provider

Agent capability owner 提供可搜索 read model 和导航 Action。Workbench 可以通过现有 Provider
契约投影最近和匹配的 Agent，但不拥有 Agent session、composer、提交或远程任务生命周期。

- `Agents` 显示 owner 提供的最近和匹配结果。
- 选择具体 Agent 时，调用 owner 提供的 prepared navigation Action。
- 没有具体结果匹配时，经过明确评审的 fallback 可以把查询预填入 Agent-owned composer。
- 预填绝不自动提交、启动工作、启动进程或产生外部成本。
- Provider 取消和过期结果拒绝继续使用当前 Workbench coordinator。
- Query 和 result content 只存在于已挂载内存中，Workbench 不持久化，也不写入 telemetry。

组合之前，Agent owner 必须定义稳定结果身份、可用性、导航语义、本地化呈现和预填边界。

## Terminal Actions

应用组合层可以把精确活动 Terminal instance 的 Actions 适配到现有搜索 Provider 和 Action
Runtime 契约中。

- Result 始终绑定到产生其 prepared invocation 的 instance registration。
- Instance 被销毁或活动实例被替换后，旧 Result 变为不可用，绝不重新指向另一个 terminal。
- `@orchbee/terminal` 保留 terminal-specific action 语义，不依赖 Workbench 呈现或本地化。
- Workbench 不获得 Terminal process、session、attachment 或 projection 生命周期。
- 应用组合层映射兼容的定义和结果，不创建第二条搜索路径。

## 排除项

本提案不增加文件搜索、持久化 query 或 action history、用户自定义快捷键、Action chaining、
不透明或 AI 生成的排序、Workbench-owned Agent 或 Terminal runtime state，也不自动向远端暴露
本地 Actions。

Workspaces、Automations、Skills 等其他功能组必须先拥有稳定的 searchable read model 和真实使用，
才能进入搜索。它们未来可能出现，不代表本提案已经接受对应 Filter 或交付承诺。

## 完成条件

1. Agent 公共契约被接受后，组合 Agent-owned Provider 和禁止自动提交的 fallback。
2. Terminal adapter 契约被接受后，在应用边界组合活动 Terminal-instance Actions。
3. 删除这些集成暴露出来的重复 callback 或 metadata。
4. 把维护行为和证据迁入对应的当前指南与测试。
5. 删除本提案及其译本。
