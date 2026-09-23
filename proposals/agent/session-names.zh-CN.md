# Agent 会话目标解析

| 字段       | 内容                                                                                                                                                                                                                  |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 状态       | get/explain/wait 直接接受名称尚未实现；运行时别名表、OBAP names 家族及 names/resolve/rename 命令已实现                                                                                                                |
| 决策负责人 | 指挥 Windows CLI 完整对齐 herdr 的当前用户                                                                                                                                                                            |
| 语义所有者 | Server AgentState 与 AgentService；原生 CLI 目标和等待执行                                                                                                                                                            |
| 当前权威   | [Canonical Agent 模型](../../apps/orchbee-server/docs/agent-canonical-model.zh-CN.md)、[原生 CLI](../../apps/orchbee/docs/native-cli-query.zh-CN.md)、[OBAP](../../crates/agent-observation-protocol/README.zh-CN.md) |
| 退出条件   | 直接目标读取和等待使用既有名称权威并具备下述证据；所有者文档收敛后删除本提案对                                                                                                                                        |

简体中文翻译；英文原文：[session-names.md](session-names.md)。

## 剩余行为

扩展 get/explain/wait，使其接受一个位置参数 auto 选择器或显式 `--session`/`--name`。
在持有的同一已接纳 Client Session 内解析，沿用命令既有绝对截止和信号所有权。
保留的精确会话 ID 存在时优先于别名；显式名称选择可避开冲突。
不引入第二套别名权威、持久化或终端文本推断。

读取或轮询前只解析一次并固定完整会话 ID。重命名、释放或复用名称不得重新定位正在等待的
会话。dispose、poison、名称缺失、解析被拒、回复格式错误和传输丢失仍为定型失败。
解析回复只能提供自己的来源身份；不得伪造未执行的 get 回复或来源。

## 证据与边界

新增真实传输/进程证据：同一会话内先解析再 get/explain、共享总截止和取消所有者，
以及等待中名称重新绑定。保持显式会话寻址兼容，验证 ID/名称冲突。
既有名称服务、schema 与编解码证据继续证明别名权威。
Agent start 预留和直接 Agent 输入属于报告中的独立缺口；本提案不声称它们已实现。
