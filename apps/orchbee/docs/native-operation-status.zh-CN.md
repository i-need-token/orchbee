# 保留操作状态

## 命令模型

异步回执通过所选 Session 中的原始请求身份寻址。
使用非零、小写、32 位十六进制 `<request-id>`，不使用资源标签或选择器。
每个命令族使用 `status <request-id>`；支持的重试使用
`resume <request-id> --generation <n> --confirm`。

| 命令族    | 状态                            | 恢复                                                       | 回执中的身份                                       |
| --------- | ------------------------------- | ---------------------------------------------------------- | -------------------------------------------------- |
| Workspace | `workspace status <request-id>` | `workspace resume <request-id> --generation <n> --confirm` | `closeRequestId`                                   |
| Tab       | `tab status <request-id>`       | `tab resume <request-id> --generation <n> --confirm`       | `closeRequestId`                                   |
| Panel     | `panel status <request-id>`     | `panel resume <request-id> --generation <n> --confirm`     | `closeRequestId`                                   |
| Project   | `project status <request-id>`   | `project resume <request-id> --generation <n> --confirm`   | `operationId`                                      |
| Worktree  | `worktree status <request-id>`  | `worktree resume <request-id> --generation <n> --confirm`  | `operationId`                                      |
| Client    | `client status <request-id>`    | 不支持                                                     | `operationId`                                      |
| Server    | `server status <request-id>`    | 所属模型的显式重试                                         | stop 使用 `requestId`，manifest 使用 `operationId` |

Project 和 Worktree 恢复保留可选的 timeout 和 detach 控制。Generation 是
正十进制整数；Project 和 Worktree 的 attempt 上限为 64。Close generation
保留协议的全宽边界。状态读取不恢复、不重新启动、不获取焦点。
多阶段回执使用具体阶段及其所属命令族的身份。

## Close 查询与恢复

Close 状态按精确请求 ID 读取 Server 保留账本，包括
布局成员消失后的墓碑。命令族不匹配时拒绝，不改选资源。
新增的匹配 cohort 读取不增加持久索引或状态，并保留 Site 权限检查。
缺失、拒绝及不可用观察保持区分；缺少历史不证明
以前没有发生效果。

`orchbee.operation.close` v1 包含原始身份、命令族、相关观察及
可空恢复回执。恢复先观察精确目标，再用新的
传输请求 ID 和调用者的 generation 围栏提交一次。读取与恢复共用五秒
预算。返回、确定未提交和提交结果未知保持区分；未知退出码为
8，再次尝试前必须检查。取消保留已获得事实并停止
本地等待。Generation 检查仍由既有关闭状态机负责。

Server 状态先检查持久化停止回执，因此关闭后仍可使用。只有确定
没有停止记录时才查询 manifest 操作状态。调用者在两个域重复使用同一
身份时，stop 优先。停止观察不可用时保留原结果，不猜测。
Client、Project、Worktree 和 Server 保留既有结果 Schema 与失败语义。

## 迁移与证据

统一路径是推荐接口。现有命令族专属状态和恢复路径
保留为已声明的兼容入口，供已保存脚本和历史恢复提示使用。
其原始参数契约不变；新示例使用统一路径。
不引入自动重试、不支持操作的通用恢复或操作重放。

解析用例覆盖精确 ID、缺少操作数、资源选择器拒绝及确认
围栏。Rust、TypeScript 与 JSON Schema 共享命令一致性语料，包括
仅按请求查询、各目标类型、拒绝／不可用／缺失结果及关闭墓碑。
Server 权威用例覆盖保留身份和权限检查。本地验证遵循仓库受影响宿主门禁。

一次性 Client 提示、原生提示及已结算的设置结果在现有单调时钟 UI tick 上经过四秒后消失。同类提示合并，重绘不延长寿命。进行中的工作、未知结果、需恢复的结果与持久配置警告仍由所属流程管理；提示过期不丢弃回执，也不重试操作。

<!-- documentation-pairing: english-sha256=c1804b50d732b35bcd566c9f1df9b2225f340a71327e64ed31ce184e0a175a31 zh-cn-sha256=f4d4c2d6461fb9db482eb794f52af451025e0f3bf822c7ee8256b19814ef06b2 -->
