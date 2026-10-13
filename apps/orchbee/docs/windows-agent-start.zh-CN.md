# Windows Agent 启动

`agent start` 将 Server 的有界名称预留与既有终端输入、进程检查和 canonical Agent 观察组合，
作用于运行中的 Windows Session。它不激活 Session、不安装提供方，也不从进程创建推断就绪。

## 调用与前提

```text
orchbee agent start alpha --kind codex --panel <id> --timeout 30000 --format json -- <args>...
```

名称符合 `[a-z][a-z0-9_-]{0,31}`。`--kind` 从运行时识别注册表中选择规范提供方 ID；
可执行文件别名不成为额外 kind。目标是 Panel 的主终端。省略 `--panel` 或使用 `--current`
要求有效调用者上下文。`--panel` 与 `--current` 冲突。重复的 `--arg=<value>` 可替代尾随参数，
两种形式混用会被拒绝。保留空参数，拒绝控制字符，编码后命令不能超过 8000 个 ASCII 字符。

终端必须有已观察到的空闲 cmd、PowerShell 或 pwsh 进程。运行中的命令、缺失的进程详情，
或已知控制台基础设施以外的额外进程均拒绝启动。提供方 hook 必须报告就绪。
[Windows 集成](windows-agent-integrations.zh-CN.md)定义托管安装及各提供方的身份／状态边界。
识别可执行文件不代表具有 hook 覆盖，也不验证提供方自身的交互界面。

规范 `cursor` 和 `qodercli` 在 Windows 上固定启动 `agent` 和 `qoder`，与当前原生安装一致。
历史名称保留为识别别名；启动不回退到它们。当前可执行文件缺失时沿用既有启动失败流程。
基于名称的进程识别只是启发式事实，不能替代所要求的规范 Hook 观察。自有 Windows 子进程测试
使用字面参数覆盖两个规范选择；不证明经过认证的提供方已经就绪。
参考：[Cursor 安装](https://cursor.com/docs/cli/installation)和
[Qoder 安装](https://docs.qoder.com/cli/installation)。

## 流程与不变量

1. 在准入前校验并引用所有参数。解析并固定主终端，观察 shell 身份，按精确原生绑定预留名称。
2. 重新核对物理终端、Host epoch 与 shell PID/创建身份。提交一次字面 paste 加 Enter；
   PowerShell 在 Enter 前增加 Right，以完成 paste 处理。
3. 检查同一预留与终端。Server 排除未变化的旧观察和歧义候选；CLI 还要求运行中的提供方匹配。
4. 仅对固定的 live 会话完成启动：状态新鲜、idle/已完成且无待处理关注，距离提交至少
   3000 毫秒，并经过 Server 的预留稳定期。Server 重新校验后原子绑定名称并移除预留。

默认总期限为 30000 毫秒，接受 3001–300000。每步 I/O 同时受五秒及原始剩余预算限制。
取消停止观察；名称释放最多使用原始预算中的一秒。任何操作都不续期。
Server TTL 从 reserve 接受时起算，传输延迟可能使其晚于 CLI 期限。
精确到期、绑定替换、退休与 dispose 均移除预留。

Host 进程样本变化或暂时不可用时，在同一原始期限内重新采样。仅重试这两种类型化
采样结果；永久 Host 拒绝与传输失败停止流程。不重放输入或预留变更。
读取耗尽启动总期限时，即使读取传输报告连接关闭，结果仍为超时。保留其原因与已接受的
输入回执；未确认的变更仍报告结果未知。

进程样本是预检，不是原子输入租约。用户输入与独立 send 可能竞争。Canonical hook、
匹配的进程身份和观察基线提供有界本机证据，不是对提供方或新启动会话的因果认证。

## Windows 参数边界

外层 shell 收到 ASCII 的 PowerShell UTF-16LE 编码命令。脚本参数用单引号值和数组 splatting；
原生可执行文件使用 Windows argv 引用。批处理文件显式调用系统 cmd，启用命令扩展，
关闭延迟展开与 AutoRun。内部引号加倍，百分号使用固定 Rust 标准库也采用的空 CD 子串
技术打断展开。进程继承终端控制台、环境和工作目录。不引入执行策略绕过或分离的原生窗口。
提供方脚本仍对其内部的后续解释负责。

## 结果与恢复

成功为 `orchbee.agent.start` v2，含名称、提供方、会话、预留身份、耗时及原始终端发送回执。
失败为 `orchbee.cli.failure.agent-start` v1，独立保留预留处置、最后观察到的预留、有界上游原因，以及输入
回执或提交失败。最后观察中的剩余时间是历史事实。已知部分输入保留退出码 9；输入未知
或变更响应丢失保留退出码 8。已接受输入但清理未确认，不算启动成功。

超时、取消和释放都不终止 Agent。释放未确认时，重试前检查 `agent names` 与终端。
不自动重放结果不确定的 reserve、complete 或 send。`agent names` v2 分别列出已绑定名称
和待完成预留，Server 合计容量为 512。Resolve/rename/clear 对预留名称或终端拒绝为
`launch-pending`（名称失败 v3）。冷启动丢弃普通名称和待完成预留。只有启用 Agent 恢复时，带已接纳 provider 引用的
已完成托管名称才会恢复到新终端。
关闭终端会退休其待完成预留。已绑定名称遵循 canonical 会话生命周期：仍为 live 的会话
解除终端绑定后保留名称，直到会话结束／移除，或显式执行 `agent rename <name> --clear`；
仅解除绑定不能证明提供方会话已结束。

## 证据与归属

`src/cli/agent_start` 拥有组合逻辑；Server 的
[canonical 模型](../../orchbee-server/docs/agent-canonical-model.zh-CN.md)拥有预留状态。
共享 OBAP 协议拥有 wire 校验。测试覆盖真实 Windows exe/ps1/cmd 参数边界、解析与补全范围、
竞争预留、精确到期、旧绑定、canonical 就绪、输入中断及响应丢失。Windows CLI 命名管道测试
保留回执并证明没有自动重放。真实 Session/Worker/Host 验证单独检查组合运行时。
本 Windows 切片不要求执行 TLA 模型。

## 检查已完成启动

Windows Agent Get v9（指定 Client 时为 v10）包含 `management`，携带自己的读取来源、采样规范修订及 `state`。Server 仅在接受 Complete 后保留活动启动身份；手动命名仍为非托管。Active 表示该启动已确认交互就绪，与后续工作、待处理事项、新鲜度或 OS 焦点相互独立。清名、结束／移除／销毁、provider 不符及精确物理绑定退役／替换会清除托管记录。重命名保留它。已完成托管名称及 provider 意图通过
[检查点版本 4](../../orchbee-server/docs/agent-canonical-model.zh-CN.md#托管名称重启)跨正常重启保留。
恢复后的物理 Get v14 使用独立读取来源报告 active provider 归属，不携带旧 Session、预留或
新的交互就绪声明。清名同时仅清除仍匹配的自有持久恢复引用；保留后来的独立报告。

Pending 状态携带原始预留及当前 blocked 标志。读取不延长期限、不完成启动。Pending 时 Get 将 promptReady 置为 false；prompt 在规范状态／进程预检后读取 management，并在发送前拒绝待启动状态。沿用原始准入／总期限及信号所有者。Status 读取失败保留 Agent 失败族，并在 v3 中标记 operation management。成功的 Start v2 在现有完成和发送证据上增加必需的 interactiveReady true；v1 schema 仍供旧调用者使用。这不改变现有阻塞启动取消策略，也不增加屏幕识别规则引擎。
