# Windows Agent 启动

`agent start` 将 Server 的有界名称预留与既有终端输入、进程检查和 canonical Agent 观察组合，
作用于运行中的 Windows Session。它不激活 Session、不安装提供方，也不从进程创建推断就绪。

## 调用与前提

```text
orchbee agent start alpha --provider codex --panel <id> --timeout 30000 --format json -- <args>...
```

名称符合 `[a-z][a-z0-9_-]{0,31}`。`--provider` 从运行时识别注册表中选择规范提供方 ID；
可执行文件别名不成为额外 kind。目标是 Panel 的主终端。省略 `--panel` 或使用 `--current`
要求有效调用者上下文。`--panel` 与 `--current` 冲突。重复的 `--arg=<value>` 可替代尾随参数，
两种形式混用会被拒绝。保留空参数，拒绝控制字符，编码后命令不能超过 8000 个 ASCII 字符。
帮助与无效值诊断列出完整的 `--provider` 取值集合。

读取 Session 快照之后、准入之前，CLI 在启动期限内（至多 10000 毫秒）请 Session 的 Worker 检查该 kind 的托管集成。缺少可执行文件时以
`provider-not-found` 拒绝；集成为 `not-installed`、`disabled` 或 `update-required`，或适配器
capsule 未发布时，以 `integration-not-enabled` 拒绝；`conflict` 以
`integration-blocked` 拒绝。这些均为 `check-failed`（退出码 1），不预留名称也不发送输入，
诊断指出 `agent integration enable` 或 `agent integration explain`。预检绝不启用集成，因为那会
修改提供方自己的配置。没有托管集成的 kind、仅手动管理、`unknown` 状态、因预算截断的检查以及被中断或失败的检查 job，
继续交由 canonical 就绪判定。成功时，文本结果给出下一步 `agent prompt --name <name>`。

完整读取为空后，[Agent 看板](agent-board.zh-CN.md)通过 `agent_provider::detection_executable()`
运行固定的 System32 `where.exe` 查询。最多四个作用域内 worker 逐项发布候选结果，邮箱容量为
提供方数量加一个最终结论。每项有 900 ms 运行时槽和 600 ms 清理时槽，共享六秒目录期限。
部分或失败结果保留已找到的候选和手动表单。每个 helper 都不持有输出管道，并在固定本机进程
创建与 suspended 分配后进入 kill-on-close Windows Job。所有 worker 加入后目录 owner 才退出；
UI 退休队列只回收已退出的 owner。

`.exe`、`.com`、`.cmd`、`.bat` 与 `.ps1` 匹配只是 Client PATH 提示，不是目标可用性证明。
自动检测排除通用命令名 `agent`：[Cursor 使用该命令](https://prod.cursor.com/docs/cli/installation)，
但其他提供方也可能安装同名文件。Cursor 仍可手动选择，沿用已有启动映射。远程 origin 不查询
Client PATH，同样提供手动选择。表单提供可编辑的自动名称，并在当前绘制准入后注入精确聚焦
的 Panel ID。手动选项复用预留、前置检查、单次提交和就绪归属者，不引入调用者上下文环境变量
或子进程适配器。

固定本机 System32 进程创建及 suspended Job 分配是同步 Windows 所有权原语，应用取消无法中断。
取得自有 child 后，每次等待都会轮询取消与绝对期限；Job 终止加 kill-on-close 覆盖整个进程树，
owner 只在
确认退出或固定 teardown 时槽耗尽后结束 join。
关闭或取代目录时会把已取消的 owner 移入退休队列。输入线程只会 drop 并 join worker 已退出的
owner。Session teardown 负责最终 join；它仍可能等待已开始的初始进程创建或 Job 分配调用；取得
所有权后，期限与 teardown 时槽会约束它。

看板的后台任务在单一 300000 毫秒期限下直接调用应用私有的类型化流程，包含处理信任和登录提示的时间。接受启动后立即关闭看板并恢复终端输入。关闭看板不会取消任务；
`c` 与可见取消操作才是显式路径。Session teardown 会唤醒并加入有界 owner。每个完成携带代际与
Panel 目标，因此旧结果无法落定到替代草稿或目标。未知的预留或输入效果保持 unknown，绝不
重放。命令已完整接受但等待就绪超时，会显示尚未确认并提示检查终端，仍不可重试。固定的规范关注请求同时更新不捕获输入的通知和重开的看板，后台流程继续在原始期限内观察。关注解除后可完成同一预留，绝不重新提交。CLI 保留既有启动受阻拒绝。看板不会安装 Agent，也不会改变集成／默认配置。发现结果为空时仍可手动选择，但不会声称已检测到任何 provider。取消会唤醒进程采样与单次 send 交换，但进行中的
Windows 进程创建／Job 分配调用以及最长一秒的尽力预留释放仍可能延迟 teardown；取消并非瞬时。

终端必须有已观察到的空闲 cmd、PowerShell 或 pwsh 进程。运行中的命令、缺失的进程详情，
或已知控制台基础设施以外的额外进程均拒绝启动。运行时的完整 PowerShell 初始化脚本
仅在带有 `-NoExit` 和常规交互开关时被接受。任意命令脚本、附加参数和非交互启动仍被拒绝。
提供方 hook 必须报告就绪。
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
3. 检查同一预留与终端。Server 排除未变化的旧观察和歧义候选；CLI 还要求运行中的提供方匹配，
   [启动回执](#启动回执)和 Server 识别结果可提前否定这一点。
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

## 启动回执

Reserve 同时返回 Server 生成的运行令牌：非零、永不复用的 32 位小写十六进制值。Shell 运行
Worker 的受管 PowerShell 初始化时，Agent 启动在唯一一次提交的行前加 `<#orchbee-run:<token>#> `。
经过校验的读取包装器剥离该前缀，只为这条命令导出 `ORCHBEE_RUN_TOKEN`，并在开始和完成标记中
回传令牌；命令返回时 prompt 删除该变量。其他 PowerShell 把前缀当作注释，命令照常执行；cmd
不加前缀。

Server 依据其采样器对同一 Host 终端的最新帧锁存每个预留的回执：`awaiting`、`started`，
然后是带 PowerShell `$?` 的 `finished`。Inspect 以 `receipt` 返回。收到 finished 回执时启动
立即结束：`$?` 为 false 报告 `launch-command-failed`（例如找不到提供方程序），为 true 报告
`agent-exited`。没有回执时，3000 毫秒后的空闲 shell 检查仍报告 `agent-exited`。

每次进程采样后，启动流程读取 Server 对所预留终端的识别结果和运行锚点，其前台遵循
[process-info 规则](native-cli-query.zh-CN.md#windows-解释与-agent-输入预检)；启动流程不从自己的样本中
做选择。已发布的运行若是另一提供方，报告 `agent-provider-mismatch`，例如 Cursor 的通用名 `agent`
实际启动了 Grok；歧义报告 `agent-runtime-ambiguous`，根进程缺失报告 `agent-exited`。就绪还要求已发布
的运行是所请求的提供方。Server 仍在稳定、读取失败或终端不符时，该步不选择前台，启动流程在预算内继续轮询。

- 每个预留只有一个运行令牌；只有本 Host 终端回传该令牌才能推进其回执，回执阶段不会回退。
- 前缀不增加输入：仍只提交一行一次，且不重放。
- 令牌只进入受管包装器执行的那条命令。
- 回执只能让启动提前结束，不能完成启动。完成启动需要 Server 已发布的运行，因此 Server 仍在稳定时
  启动会推迟到运行稳定，但这不会单独让启动失败。
- 识别结果按[手动输入运行边界规则](native-terminal-run.zh-CN.md#手动输入运行边界)成为 `confirmed`，该规则同样适用于这条带前缀的命令；cmd 启动保持 `recognized`。

## Windows 参数边界

无参数的原生表单使用简短的提供方命令：cmd 调用固定命令名，PowerShell 在调用前明确解析 Application/ExternalScript，排除函数和别名。无需临时启动文件或执行策略更改。CLI 保留字面参数包装器，其外层 shell 收到 ASCII 的 PowerShell UTF-16LE 编码命令。脚本参数用单引号值和数组 splatting；
原生可执行文件使用 Windows argv 引用。批处理文件显式调用系统 cmd，启用命令扩展，
关闭延迟展开与 AutoRun。内部引号加倍，百分号使用固定 Rust 标准库也采用的空 CD 子串
技术打断展开。进程继承终端控制台、环境和工作目录。不引入执行策略绕过或分离的原生窗口。
提供方脚本仍对其内部的后续解释负责。

## 结果与恢复

成功为 `orchbee.agent.start` v1，含名称、提供方、会话、预留身份、耗时及原始终端发送回执。
失败为 `orchbee.cli.failure.agent-start` v1，独立保留预留处置、最后观察到的预留、有界上游原因，以及输入
回执或提交失败。最后观察中的剩余时间是历史事实。已知部分输入保留退出码 9；输入未知
或变更响应丢失保留退出码 8。已接受输入但清理未确认，不算启动成功。

对任何 Pending 或 Unknown 预留、已接受或部分接受的输入、未知投递前沿，或已接纳后未返回
类型化完成便失联的 worker，看板都会保留不可重试的 recovery 结果。关闭并重开不会清除该结果。
只有流程证明零输入且预留已落定后才提供重试。即使 worker 已退出，已入队的完成也以 channel
为权威，因此 worker 退出观察不能丢弃 Started 结果。

超时、取消和释放都不终止 Agent。释放未确认时，重试前检查 `agent names` 与终端。
不自动重放结果不确定的 reserve、complete 或 send。`agent names` v1 分别列出已绑定名称
和待完成预留，Server 合计容量为 512。Resolve/rename/clear 对预留名称或终端拒绝为
`launch-pending`（名称失败 v1）。冷启动丢弃全部名称和待完成预留。
关闭终端会使待完成预留退役。已提交名称遵循规范会话生命周期：脱离仍活动的会话会保留名称，直到会话结束／移除或显式执行
`agent rename <name> --clear`；脱离本身不能证明 provider 会话结束。

## 证据与归属

`crates/workbench-client/src/agent_start.rs` 与 `crates/workbench-client/src/agent_start/` 拥有类型化流程和报告，独立于最终结论保留
`ReservationState` 与 `DeliveryDisposition`。`crates/workbench-client/src/agent_runtime/startup/` 拥有共享的
Snapshot／OBAP 读取、精确绑定解析、进程 attach/read/detach 和单次 prompt 提交。
这些操作保留已准入的 Client、原始绝对期限与 wake；进程读取失败仍汇入一次尽力 detach，
Paste／可选 Codex Right／Enter 序列只提交一次。

`src/cli/agent_start.rs` 保留 CLI 选择和信号收束；`src/cli/agent_start/report.rs` 把领域事实
投影到既有 CLI failure、status 和 JSON 字段。Workbench 直接消费类型化的预留、投递和
失败事实。Server 的 [canonical 模型](../../orchbee-server/docs/agent-canonical-model.zh-CN.md)
拥有预留状态，共享 OBAP 协议拥有 wire 校验。

Host envelope 与进程 payload 校验证据位于 `crates/workbench-client/src/agent_runtime/startup/processes/tests.rs`。
`src/cli/agent_start/tests.rs` 让既有回执矩阵经过真实的领域到 CLI 转换；Workbench 恢复
优先级仍由 `crates/workbench-client/src/agent_board/start/tests.rs` 覆盖。既有测试还覆盖真实
Windows exe/ps1/cmd 参数边界、解析与补全范围、竞争预留、精确到期、旧绑定、canonical
就绪、输入中断及响应丢失。Windows CLI 命名管道测试保留回执并证明没有自动重放。
真实 Session/Worker/Host 验证单独检查组合运行时。本 Windows 切片不要求执行 TLA 模型。

## 检查已完成启动

Windows Agent Get v1（指定 Client 时为 v1）包含 `management`，携带自己的读取来源、采样规范修订及 `state`。Server 仅在接受 Complete 后保留活动启动身份；手动命名仍为非托管。Active 表示该启动已确认交互就绪，与后续工作、待处理事项、新鲜度或 OS 焦点相互独立。清名、结束／移除／销毁、provider 不符及精确物理绑定退役／替换会清除托管记录。重命名保留它。托管名称是临时状态，不从检查点恢复。

Pending 状态携带原始预留及当前 blocked 标志。读取不延长期限、不完成启动。Pending 时 Get 将 promptReady 置为 false；prompt 在规范状态／进程预检后读取 management，并在发送前拒绝待启动状态。沿用原始准入／总期限及信号所有者。Status 读取失败保留 Agent 失败族，并在 v1 中标记 operation management。成功的 Start v1 在现有完成和发送证据上增加必需的 interactiveReady true。CLI 的启动受阻取消策略保持不变。交互看板可在原始预算内等待规范关注解除；两个路径均不通过屏幕文本猜测信任提示。

原生表单在检测尚无结果时不预选提供商，也不生成自动名称。已发现的提供商排在手动选项之前；显式选择按提供商身份固定，不随目录更新改变。没有显式选择时，首个发现的提供商成为默认项；检测结束且未发现候选时选择首个手动提供商。启动反馈仅在看板状态行显示一次，结果未确认时引导用户查看终端。只有固定身份的规范关注观察才声明需要输入；进程存在本身不能证明就绪。

<!-- documentation-pairing: english-sha256=142b251a397174ac191dc368d05640359f339d7fb77edaec1dd5c470af679a75 zh-cn-sha256=62fdfe8ee5f5f8facc20532fee478e4e26a05fa068536f661bd0ae80b2d8e6d2 -->
