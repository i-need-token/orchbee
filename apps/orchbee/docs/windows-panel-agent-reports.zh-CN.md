# Windows Panel Agent 上报

四个命令向运行中的 Site 提交显式终端上报：

```text
orchbee panel report-agent <panel> --source <id> --agent <label> --state idle|working|blocked|unknown [--message <text>] [--seq <n>] [--agent-session-id <id>] [--agent-session-path <path>]
orchbee panel report-agent-session <panel> --source <id> --agent <label> [--seq <n>] [--agent-session-id <id>] [--agent-session-path <path>] [--session-start-source <reason>]
orchbee panel clear-agent-authority <panel> [--source <id>] [--seq <n>]
orchbee panel release-agent <panel> --source <id> --agent <label> [--seq <n>]
```

全部接受 `--format text|json`、完整 Panel ID 或 `@panel:<suffix>`，以及 Windows 全局
`--site` 选择器。帮助和 PowerShell 补全提供各命令的选项和状态值。
`clear-agent-authority` 为 Herdr 仅 API 提供的操作增加原生命令。

## 输入与上报归属

report/release 来源修剪首尾空白后必须非空，与参考 CLI 一致；不同于元数据来源，内部允许
任意字符。clear 原样保留可选来源：省略表示当前权威，空字符串仅匹配空来源。二进制 API
自身保留字面来源。Agent 标签使用共享上报规范化器：已知别名转为规范上报标签，未知标签
保留修剪后的大小写和内容，显式空标签失败。消息原样保留，不应用展示长度截断。
序号输入解析为完整无符号 64 位整数，在线协议中成为规范十进制字符串。重复选项、越界选项、
非法状态及完整请求超过 16 KiB 均在连接前失败；JSON 转义后的长度计入此上限。

Provider ID、路径和启动原因交给 [Server 上报所有者](../../orchbee-server/docs/agent-report-model.zh-CN.md)。
非法引用或未知启动原因规范化为缺失，不伪造会话。仅官方来源/provider 配对可保留可恢复身份。
原生官方状态上报路由到会话身份；完整生命周期上报需要真实匹配的进程证据。自定义上报是
显式权威，不证明进程存在，也不创建规范 Agent 会话。clear 作用于当前匹配权威；release
保留独立活动进程及其他来源的 provider 引用。官方 release 按规则不产生效果。已有来源序号
水位后的旧序号或无序号上报会被忽略。状态变更与引用接受可能结果不同，因此回执中的 disposition
及 `changed` 均有实际含义。

## 提交与回执

一次已有 Site 连接、快照和提交共享五秒期限及信号所有者。只解析一次主终端，提交预期绑定。
未绑定目标在提交前失败，绑定变化返回相关拒绝。不启动 Site、不改变焦点、不输入终端、
不执行 provider 或自动重试。交接后应答丢失、畸形、不匹配或超时保留请求 ID 并报告结果未知。
迟到中断保留已经收到的结果并设置 `interrupted`。

`orchbee.panel.agent-report` v1 包含请求/Site/Panel ID、Server 回执或封闭的未知原因、
`interrupted` 和 `cleanupFailed`。回执包含独立 Workbench/上报修订号、绑定、
applied/ignored/deferred 处理结果、实际变更标记及有效上报视图，其中可含外部 provider 会话引用。
已应用、已忽略和暂存退出 0；目标、权限、容量拒绝退出 3；不可用退出 4；请求无效退出 5；
所有者修订号、时钟或结果上限退出 1。提交后结果未知退出 8。清理失败退出 1，但已有提交结果
未知时保留 8。本地化文本展示处理结果及请求身份；JSON 携带完整视图且不改写字符串。

目录 v88 发布 90 份 schema，原生语法有 104 条命令路径。OBWP 35/36 使用既有 16 KiB 请求和
512 KiB 结果上限。部署匹配的私有 cohort；旧端拒绝这些 carrier kind。状态是临时的，无持久化迁移。

## 剩余集成与验证

原生命令已经调用真实 Server 所有者。关联名称、provider 恢复/持久化、
TUI 展示仍是独立集成义务。尤其是暂存的生命周期上报，不会因为 CLI
回执成功就变成已确认进程。Panel list/get/current 与 Agent Get 现已展示带来源的 `reportedAgent`，并使用实际上下文应用
元数据守卫；两类修订号共同校验，详见 [元数据投影](windows-panel-metadata.zh-CN.md#查询观察模型)。

解析测试覆盖作用域、来源区别、标签/引用保留及完整编码请求上限。命名管道故障测试覆盖提交前
期限、应答相关性、绑定不匹配、丢失、畸形、迟到应答及不重放。回执测试在迟到取消、双语言及
封闭 JSON schema 下保留三种处理结果。Windows 验收使用匹配 cohort 和隔离 Site，通过这些
公开命令操作；不代表已执行认证 provider 任务或完成最终 TUI/全量回归验收。

## 通过 Host 读取确认进程

Windows Site 在后台采样 Host 进程，无需用户发起查询。每轮按轮转顺序访问最多八个主／子绑定，轮间等待 250 毫秒，目标之间使用一秒预算；carrier I/O 和清理可能延迟一轮，因此不承诺新鲜度。临时只读 viewer 释放后才采样另一终端；清理结果不确定会暂停采样，责任保留至 Site 停止流程。成功的 `panel process-info` 及 Panel／Agent 查询中的终端观察也进入同一可信进程适配器。它再次校验精确的 Worker／Host 代际，识别唯一进程链，并可激活匹配的暂存生命周期上报。后续完整样本可证明保留的 PID／创建时间身份已离开，同时原子清理对应上报及适用元数据。缺失／失败／歧义样本不虚构退出；未改变的身份不会反复推进修订。Panel／Agent 查询先采集 Host 观察，再读取上报／元数据修订，因此自身成功接纳的进程确认可以出现在同一次结果中。之后若并发改变修订，查询返回既有 `metadata-changed` 失败，不发布混合结果。屏幕回退状态仍待完成；进程识别不认证 provider，也不创建规范 Agent 会话。
