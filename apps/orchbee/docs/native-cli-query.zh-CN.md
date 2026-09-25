# 原生 CLI 查询

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` 在一个输入请求中粘贴命令，再发送 Enter。命令须为一个引号字符串；保持原始文本，目标只接受完整 ID 或 `@terminal:<suffix>`。回执确认输入提交，不保证 Shell 已就绪、命令完成或退出状态；部分或未知结果不得重放。 [命令提交契约](native-terminal-run.zh-CN.md).

状态：原生 `orchbee` 命令行的当前实现与正确性契约，覆盖查询、Workbench 修改、
终端观察与输入、Agent 名称及本地 provider 接入。查询观察已有 Site；Workbench 修改
遵循下文 mutation 契约。只有显式的 `site start` 和 TUI 启动路径可以激活 Site。
Agent 状态来自 Server 的 canonical Agent 模型，不从终端文字推断；名称由 Server
管理，`agent integration` 只管理调用用户的本地 provider 配置。每个命令的效果、
预算和资源释放边界见相应章节。命令无需 TTY；交互式 TUI 有独立的终端准入要求。
原生 CLI 仍为未打包的开发面，Desktop 的 `apps/cli` 是另一交付入口。

归属：本记录拥有 `src/invocation.rs` 与 `src/cli/**` 实现的公共语法、结果信封、失败矩阵、
预算、交换状态机与输出副作用。文字与实现必须在同一变更中收敛。

## Agent 会话名称

`agent names` 列出运行中 Site 的所有存活别名。`agent resolve <target>` 返回完整会话 ID，
优先检查精确 ID 是否存在，再查别名。`agent rename <target> <new-name>` 赋名或原子替换
名称；`agent rename <target> --clear` 只释放名称，不停止会话。可把目标位置参数替换成
`--session <id>`、`--name <alias>` 或 `--panel <id|@panel:suffix>` 明确选择；必须且只能使用一种选择器。
名称格式为 `[a-z][a-z0-9_-]{0,31}`，区分大小写且唯一，与待完成启动预留共用 512 条上限。
会话结束、移除或 Site 运行时 dispose 后释放名称；Site 重启后的别名表为空。

三个命令均接受 `--format text|json`。内嵌成功 schema 为 `orchbee.agent.names`、
`orchbee.agent.resolve` 与 `orchbee.agent.rename`；names 为 version 2，独立列出预留，另两者为 version 1；接纳后的失败使用
`orchbee.cli.failure.agent-names` version 1。presentation 解析与 ambiguous-target 失败使用 version 2；launch-pending 失败使用 version 3；原有结果仍为 version 1。相关语义结果携带 `agent-names` source
和 OBAP request ID。这四份文档与提示契约一起纳入 Schema 目录 version 39。非法名称或冲突选择器在
任何运行时访问前以退出码 2 失败。重名以退出码 3 拒绝并标明占用者，绝不覆盖其名称。

每个命令的单一五秒截止覆盖 cohort 校验、仅查已有 Site、接纳、可选 Panel 解析和最终 OBAP 交换。
信号参与传输等待；会话在信号仲裁与输出前关闭。确认回复与信号竞态时，已确认回复仍为
权威结果。已提交修改若没有可信匹配回复，包括取消时，返回 outcome-unknown（退出码 8），
绝不重放。已知在提交前取消返回 130。不引入别名缓存、不调用 provider、不创建 Site，
也不修改 canonical 会话文档。

`get`、`explain` 和 `wait` 接受相同的位置目标或显式 `--session`/`--name`/`--panel` 选择器。
精确的保留会话 ID 优先于名称；显式 ID 与不可能是名称或 Panel 的不透明 token 直接读取。其余目标在
同一个已接纳的 Client Session 上先发 Resolve。解析与读取共享原来的截止和信号所有者，
不二次接纳或续期。wait 的首次解析受 `min(remaining_ms, 5000)` 约束，固定解析结果后再轮询；
释放、重命名或复用别名不会改变等待目标。成功结果只使用实际读取的 `agent-read` source；
解析拒绝使用自身的 `agent-names` source 与失败 schema，绝不伪造未执行读取的来源。
Windows 进程证据覆盖真实管道接纳、名称解析后读取、固定目标轮询、共享五秒截止、
显式 ID 直读、schema 合规失败、丢失或不匹配的修改回复，以及效果前解析。

## Agent 终端操作

`agent read <target>` 和 `agent send-keys <target> <key>...` 接受与 `agent get`
相同的 Session ID、运行时名称或 Panel 选择器，包括显式 `--session`、`--name` 和 `--panel`。
读取支持 `--history --rows <n>`、`--unwrap` 和 `--lines <n>`，边界和含义与
`terminal read` 相同。发送最多接受 64 个语义按键或修饰键组合，产生任何效果前验证整个序列。

CLI 组合既有责任方：在一个仅连接既有 Site 的 Client Session 中读取 Workbench
快照，按需解析别名，再读取权威 Agent 会话。只有活动会话中可导航且存活的 presentation
附件才能选中终端。关联时比较快照中已绑定终端的完整 presentation ID；不会将 Panel ID
当成 presentation ID。显式或回退得到的 Panel 将关联限制在其主 presentation。
没有匹配返回 not-found，多个匹配返回 ambiguous，两种情况
均不附加 viewer 或提交输入。所有交换共享一个五秒期限。操作固定解析到的目标；输入准入时
Server 仍验证终端当前绑定。

| 状态             | 允许的下一操作                  | 失败效果                       |
| ---------------- | ------------------------------- | ------------------------------ |
| 已解析           | 快照准入和读取                  | 无终端效果                     |
| 已持有快照       | 在同一会话中 Resolve 和权威 Get | 无终端效果                     |
| 唯一绑定终端     | 既有终端读取或发送交换          | 沿用 viewer/输入清理规则       |
| 已收到回复或失败 | 释放会话，投影输出              | 不重放；不确定发送仍为退出码 8 |

成功复用 `orchbee.terminal.read`、`orchbee.terminal.history` 或
`orchbee.terminal.send` schema 及实际终端来源。Agent 解析或读取失败保留自身关联的来源和
失败 schema。不引入 provider 命令、Agent 生命周期转换、新 Site、别名缓存、输入所有权
或逐轮因果保证。信号沿用既有终端操作行为；此组合不宣称新增可取消的 viewer 或输入传输。
证据须覆盖互异的 presentation/Panel ID、非活动及不可导航附件、歧义绑定、效果前解析、
单次会话准入以及发送失败不重放。

嵌入的发送 schema 验证现有序列回执：载荷种类、记录数、已确认前缀、停止位置、编码
字节数、部分接受和清理状态。通用歧义失败包含 Panel 与终端候选项。这些修正过时的 schema
投影，不改变线路协议或回执版本。

## Agent 提示提交

`agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms>]`
与其他 Agent 命令使用相同 ID/名称选择器。文字为非空 UTF-8，受 256 KiB paste 上限和
命令 64 KiB argv 封套限制。`--until` 和 `--timeout` 要求 `--wait`；Windows 默认 idle/done/blocked 且无总超时，
显式有限超时最长一小时。不等待时总预算为五秒。

一个仅连接既有 Site 的 Client Session 负责快照查找、一次 Agent 身份解析、权威预检 Get、
一次 paste 加 Enter 发送以及后续 Get。预检要求会话存活、新鲜、状态已知且唯一绑定可导航
终端。有待处理请求/needs-attention 时，在发送任何输入前以 `agent-blocked` 拒绝。这是
采样预检，不会锁定 provider 后续状态或前台所有权。Server 在输入准入时仍验证终端绑定。

| 状态     | 转换与证据                                                            | 保留效果                     |
| -------- | --------------------------------------------------------------------- | ---------------------------- |
| 已解析   | 在 min(总期限, 五秒) 内准入与解析                                     | 无                           |
| 预检     | 新鲜、存活、已知、未阻塞且唯一绑定                                    | 无                           |
| 发送     | 一次有序 paste 和 Enter 序列，不重放                                  | 精确终端回执                 |
| 活动检查 | 预检已 working，或接受后五秒内观察到更新的 working/attention/轮次结果 | 后续任何错误都保留已接受回执 |
| 条件等待 | 在原总期限内采样固定会话的状态/新鲜度条件                             | 不承诺轮次因果               |
| 完成     | 释放会话，裁定取消，一次输出                                          | 不重放输入或取消 provider    |

从非 working 状态开始等待时，先要求活动证据。若轮询之间错过短暂 working，更新的轮次
结果也可作为证据。旧 idle 或不变的完成结果不能通过活动检查。活动期限耗尽为
`prompt-stalled`；调用方期限先到则为 `wait-deadline`。后续读取错误、结束、超时或取消
不会抹去已确认提交。已在 working 的 Agent 可能完成既有轮次；匹配结果不标识本次提示
自己的轮次。轮询可能漏掉转换，不宣称支持 Herdr 的事件重放。

Windows 中，canonical provider 为 `codex` 时，单次输入序列按字面 paste、Right、Enter 排列。Right 作为非字符边界，促使 Codex 刷出 paste-burst 缓冲并清除 Enter 抑制窗口；不向提示文本拼接转义码，也不以固定延时猜测投递速度。其他 provider 保持 paste、Enter，普通 `terminal send --paste --enter` 不增加按键。回执准确报告三条或两条记录；若在 Right 或 Enter 前停止，只报告实际接受的前缀，绝不补发或开始等待。该边界不证明输入框为空、焦点归属或 provider 已就绪。

Schema 目录 version 39 共 72 份文档，包含提示成功与失败契约。

`orchbee.agent.prompt` 返回会话 ID、完整终端提交回执，以及可选的匹配观察/等待事实。
发送拒绝、部分接受或不确定时沿用既有终端发送/失败契约，不开始等待。预检及提交后失败
使用 `orchbee.cli.failure.agent-prompt`；提交后失败附回执，避免将等待失败误认为尚未提交。
不带 `--wait` 的确认发送在取消与回复竞争时仍为权威结果。等待中的取消为退出码 130，
附已接受回执。已提交输入存在歧义时，即使取消也保持退出码 8。所有会话均先释放，再完成
信号处理和输出。回执不包含提示文字。

证据覆盖阻塞/未就绪拒绝且无终端效果、精确字面 paste 加 Enter、单次准入和不重放、
working/idle 活动检查、更新的完成结果、固定会话读取、总期限/活动期限、部分及不确定发送、
取消裁定和 schema 校验。原生权威观察仍是唯一 Agent 状态来源，不新增屏幕推断或 provider API。

## Agent 等待策略

Windows `agent wait` 和 `agent prompt --wait` 默认无总超时，按顺序采用 `idle`、`done`、`blocked` 条件。`--forever` 仍为显式同义选项，与 `--timeout` 互斥；prompt 要求 `--wait`。有限超时仍为 1–3600000 毫秒。准入、解析及每次读取仍有五秒上限；中断只停止等待，不重新连接。现有 Linux 轮询默认值不变。Prompt 保留非工作状态下的五秒活动门槛。

Windows wait 在首次规范 Get 前建立经认证的变更检查点，随后在后续 Get 前消费连续保留页。全部步骤共用一个已准入会话、可选的单调总期限和取消归属。每次评估最多排空 512 条变更，每页 100 条，最多保留一条命中变更。不会静默恢复游标重置或跨启动周期：历史丢失明确拒绝。部分回包或读取失败不能成为命中观测。

Windows wait 版本 3 新增可空的 `matchedChange`。空表示当前样本命中；非空时历史变更是匹配证据，`revision` 和 `session` 仍为独立的最新 Get 样本。历史记录不会覆盖当前状态。`source.requestId` 标识命中的 Get 或变更读取。保留请求条件顺序。`--after-revision` 仍为全局修订门槛，不代表回合因果。Linux wait 和 prompt 成功封套保持版本 2；Windows prompt 使用版本 3。有限／无限失败保持既有版本及空超时区别。

`blocked` 匹配 fresh needs-attention；`done` 目前匹配 fresh succeeded/failed/canceled/interrupted。Client 相对的未看见状态合并属于独立后续消费者，变更流本身不代表它已实现。证据覆盖瞬态、其他会话、分页顺序、重置拒绝、固定身份、期限和取消清理。

Windows `agent wait --client <id>` 同时固定一个精确 Client 和已解析 Agent。它使用上述 Client attention 合并：idle/done 只匹配该 Client 当前经核实的投影，其他条件保持规范当前／变更页匹配。缺失／过期 Client 版本不能满足 idle/done。历史规范事件不会虚构历史 Client 确认；指定 Client 的 idle/done 采用采样，不重放。所选 Client 丢失时结束等待，不改选其他 Client。

所选 Client 等待使用 v4，要求 `client` 投影并保留可空 `matchedChange`；未选 Client 的 Windows 等待保持 v3。每次 Client 读取、游标证明和规范样本共用五秒评估上限，并受原始总期限约束。Busy 时仍可读取，不确认已看见、不导航。有限超时和取消保持既有等待失败／收尾契约。即使当前 Client 状态不同，显式规范状态条件也可以返回历史证据。

所选 Client 组合查询对缺失或过期 Client 保留 `orchbee.cli.failure.client-control` v1 选择错误。其他 Client 读取错误使用同封套 v2，必须包含 `stage: client-agent-read`；kind 区分拒绝访问、操作拒绝、传输丢失、无效应答、截止和取消。既有 Agent 和准入错误保留原契约。

## 输入与信任边界

- `argv`：有界敌意输入。全部参数的 UTF-8 总字节不超过 64 KiB（65,536）。任一非 UTF-8 参数或
  超限即 `invalid-usage`、stdout 为空——即使另一个参数写着 `--format=json`。不隐式读取 stdin
  或文件输入——唯一例外是显式声明的 `terminal send --file`/`--stdin` 源（见
  [Send 输入源](#send-输入源)），在任何运行时副作用之前一次性物化。
- 环境：`ORCHBEE_RUNTIME_BASE`（可选，默认 `/tmp`）选择运行基座；`LC_ALL`/`LC_MESSAGES`/`LANG`
  选择诊断 locale。`agent doctor` 另外读取 `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`
  （观察端点 override，其 endpoint 事实原样报告）、供有界可执行文件扫描的继承 `PATH`，以及
  Linux 上 provider 目标前缀所用的 `HOME`。Windows 使用原生用户配置目录，
  并读取可选非空 `CODEX_HOME` 选择 Codex 配置。调用者模式的 `panel current` 还读取下文说明的
  `ORCHBEE_TERMINAL_ID` 与 `ORCHBEE_WORKER_ID`。不读取其他任何环境变量。
- 文件系统：解析出的运行基座、Site scope 目录 `orchbee-native-<euid>` 及其
  `client.sock`/`worker.sock`、activation 标记、当前可执行文件同目录的 `cohort.json` 与四个
  artifact 只被观察、绝不写入。清单还声明 `agent-observation-adapters` sidecar 摘要——
  启动绝不打开或遍历该目录。所有目录遍历使用既有 no-follow、属主/模式校验的 rendezvous 归属
  方（`ModeCheckedLocalSiteRendezvous::validate_mode_checked`，仅 `ExistingOnly` 策略）。
  `agent doctor` 检查一个不相交的声明集合——解析出的 home 下的十个 provider 钩子目标、
  提供的 `--asset-root` 路径、这些 provider 文件自身声明的 recorded root、观察 socket 路径的
  末段组件，以及每个 PATH 目录的候选可执行文件——全部在其自身的单文件/计数/聚合字节与条目
  上限下以 no-follow `lstat` 准入。它绝不枚举环境用户目录。
- Socket 对端：`client.sock` 可能属于真实 Server、残留 socket 或脚本 peer。CLI 信任类型化
  transport 与协议层（OBLC 准入、OBWP 相关性与契约校验），绝不从 `Display` 文本、`ErrorKind`
  猜测或字节计数重新推导结论。
- 快照数据：用户可控的 label 与目录路径是任意 Unicode，终端输出按敌意文本处理（见转义规则）；
  JSON 输出无损保留。

## 公共语法

选项扫描在 `--` 之前自左向右进行；`--` 仅终止选项识别。

- `--format text|json` 或 `--format=text|json`：至多一次选择，`--` 前任意位置。
  `--format` 缺值、空值、未知值或重复选择都是格式失败：`invalid-usage`、退出 2、stdout 为空。
  完整的 `--` 前 format envelope 先于普通语法校验整体扫描：envelope 内任何位置的格式缺陷都以
  空 stdout 作答；唯一合法的选择无论位于何处都决定其后一切语法失败的 envelope。
- `--project <target>`、`--workspace <target>`、`--tab <target>`：带值的具名过滤器与上下文
  操作数，各只能用一次。下一个 token 即作为值被消费，即使以减号开头；裸 `--` 永不被当作值
  （缺值 → `invalid-usage`）。被消费的过滤值不再充当 format 选择器。每个 flag 只在声明它
  的命令上合法（`--project` 仅 `workspace list`/`workspace create`/`worktree list`/
  `workspace close-status`/`workspace close-resume`，
  `--workspace` 仅
  `tab list`/`tab create`/`workspace close-status`/`workspace close-resume`，`--tab`
  仅 `panel list`），出现在任何其他位置（包括 `get`
  命令）都是 `invalid-usage`。过滤器绝不静默丢弃：出现在未声明的命令上即在任何运行时访问
  之前报 `invalid-usage`，故 `workspace list` 拒绝 `--workspace`、`--tab` 与
  `--standalone`。在 `workspace close-status` 与 `workspace close-resume` 上，两个 flag
  都是严格稳定 ID 而非 target：只接受完整的非零 32 位小写十六进制 Workbench 标识符——
  绝不接受 label 或 selector——因为保留的 close 记录比 Workspace 的 active-projection
  membership 活得更久，必须按稳定身份重新证明。
- `--directory <directory-id>`：带值 flag，单次使用，`worktree list` 上必填、其他任何
  命令上均非法。值仅为 `WorkbenchDirectoryId`——严格非零 32 位小写十六进制标识符，
  绝非文件系统路径、标签或前缀。缺失、畸形、大写、全零或宽度错误的值在任何运行时访问
  之前即为 `invalid-usage`；CLI 不做目录 membership 预检——membership 由运行中 Site
  权威判定，well-formed 但不存在的目录以 worktree 失败 envelope 下的
  `not-found` `{resource:"directory"}` 作答。
- `--standalone`：无值布尔 flag，单次使用，仅 `tab list` 与 `tab create`；两处均与
  `--workspace` 互斥。
- `--label <label>`：带值 flag，单次使用；`project create`、`project rename`、`workspace create`、`workspace rename` 与 `tab rename` 上必填。Windows `tab create` 与 `panel split` 可省略，采用创建前相应 Tab／Panel 数加一。显式值为非空且在协议上限内的 Workbench label；空串或超长均为 `invalid-usage`。
- `--path <path>`：带值 flag，单次使用，`workspace accept-directory` 上必填，也由 Windows Worktree Create/Open 接受。值为 `WorkbenchDirectoryPath`：字面本机绝对路径（POSIX `/` 或 Windows 盘符绝对路径），至多
  4,096 UTF-8 字节且不含 NUL。CLI 从不展开 `~` 或 `$`，从不创建或修复目录，也从不探测
  文件系统——缺失、相对、含 NUL 或超长的值在任何运行时访问之前即为 `invalid-usage`，
  一切文件系统事实均由 Server 在提交后权威判定。
- `--direction <word>`：带值 flag，单次使用，`tab move` 未使用 `--insert-index` 时需要
  （`previous|next|first|last`）、`panel split` 与 `panel move`
  （`above|below|left|right`）上必填。词集各自封闭；`tab move` 上出现 panel 方向词或
  缺失/未知词均为 `invalid-usage`。
- `--cols <n>`、`--rows <n>`、`--cell-width <n>`、`--cell-height <n>`：`tab create` 与
  `panel split` 上的终端几何组，各取一个十进制 `u16`。该组为全有或全无：四个全部省略
  时命令请求文档化默认值（80x24 逻辑网格、成对零的未知单元像素）；提供任何一个就必须
  提供全部四个。组合后的格数与像素范围必须满足协议 `WorkbenchTerminalGeometry` 界限；
  缺项、重复、非数值或越界均在任何运行时访问之前报 `invalid-usage`。
- `--exec <path>` / `--arg <value>`：临时启动选择，仅 `tab create`、`panel split` 与
  `launch-profile create` 接受（后者必填，前两条 launch-bearing 修改命令上可选）。
  `--exec` 单次使用，取不含 NUL 的非空绝对 UTF-8 可执行文件路径。`--arg` 可重复，
  每个值按顺序原样追加为 `argv[1..]`——空值与 dash 前导值按标准取值规则被消费为值，
  `argv[0]` 由可执行文件路径合成。至多 128 个 `--arg` 值，且可执行文件加全部参数值合计
  至多 2,048 UTF-8 字节；无 `--exec` 而带 `--arg`、重复 `--exec`、或越界均在任何运行时
  访问之前报 `invalid-usage`。省略 `--exec` 保持解析默认 shell；选择属于请求身份，绝不
  持久化为 profile。
- `--name <name>`：带值 flag，单次使用，`launch-profile create` 上必填、其他任何
  命令上非法。值为 `LaunchProfileName`——非空 UTF-8 字面量、至多 512 字节；不要求
  唯一（唯一定位条目的是铸造的 profile ID，而非名称）。缺失、空或超长值均为
  `invalid-usage`。
- `--profile <selector>`：带值 flag，单次使用，仅 `tab create` 与 `panel split`
  受理，与显式 `--exec`/`--arg` 选择互斥——任一组合均为点名冲突选项的
  `invalid-usage`。值为 `LaunchProfileSelector`：非空 UTF-8 字面量、至多 512 字节，
  逐字传给 Server，由其在目录锁下按"先 ID 后名称基数"规则解析——CLI 绝不读目录或
  猜测匹配。解析出的 profile 仅为该次请求提供启动，不在任何其他地方持久化。
- `--request <close-id>`：带值 flag，单次使用，`workspace close-resume` 上必填、其他任何
  命令上均非法。值为该 close 操作自身的 `WorkbenchRequestId`——严格非零 32 位小写十六
  进制标识符，由 Server 重新校验的操作身份，绝非 resume 请求的新鲜身份。缺失、畸形、
  大写或全零的值在任何运行时访问之前即为 `invalid-usage`。
- `--generation <n>`：带值 flag，单次使用，`workspace close-resume` 上必填、其他任何
  命令上均非法。值为显式 attempt-generation 栅栏：不带前导零的非零十进制 `u64`——协议
  `WorkbenchTabCloseAttemptGeneration` 的拼写。陈旧但合法的 generation 是语义请求，由
  Server 以 `stale` 作答，绝非语法问题；缺失、空、零、非数值或溢出值在任何运行时访问
  之前均为 `invalid-usage`。
- `--confirm`：无值布尔 flag，`tab close`、`panel close`、`workspace close` 与
  `workspace close-resume` 上必填；该处缺失或出现在任何
  其他命令上均为 `invalid-usage`。
- `--match <text>`：带值 flag，单次使用，`terminal wait-output` 的两个搜索输入之一
  ——`--match` 与 `--regex` 必须且只能提供一个。非空 UTF-8 字面量，最长 4096 字节，
  匹配时区分大小写。缺失、空、非 UTF-8 或超长值——或两个搜索 flag 同时出现——均为
  `invalid-usage`。
- `--regex <pattern>`：带值 flag，单次使用，`terminal wait-output` 的另一搜索输入；
  非空 UTF-8 正则表达式，至多 1024 字节，在任何运行时访问之前以 1 MiB 程序大小上限
  与 1 MiB 惰性 DFA 缓存上限编译一次。缺失、空、超长或编译失败的值——或与 `--match`
  并存——均为 `invalid-usage`。
- `--timeout <ms>`：带值 flag，单次使用，`terminal wait-output`（默认 `30000`）与
  `agent wait`（Windows 默认无总超时，Linux 默认 `600000`）上可选；取一个 `1..=3600000` 的十进制 `u32`。
  重复、非数值或越界值均为 `invalid-usage`。
- `terminal wait-output` 上的 `--rows <n>` 是被搜索的保留行窗口 `1..=1000`（默认
  `1000`），把最近的可见行与历史行作为同一范围覆盖——与同名的几何 flag 无关。
- `--target-panel <target>`：带值 flag，单次使用，`panel move` 上必填；按 `panel` 的
  target 规则解析（仅完整 typed ID），并在被移动 panel 自身所在 tab 内解析。
- `--history`：无值 flag，仅 `terminal read` 受理，且必须与 `--rows <n>` 成对——
  在 `terminal read` 上 `--rows` 拼写携带 `1..=1000` 的保留行数，替代其几何含义，
  该含义在 `tab create` 与 `panel split` 上不变。两者缺一、重复、非数值或越界
  计数、或 `--history` 出现在其他命令上，均在任何运行时访问之前报 `invalid-usage`。
- `--unwrap`：无值 flag，仅 `terminal read` 受理；只读呈现变换，把经验证的软换行
  行链拼接为逻辑行。硬换行与空行保留；窗口边缘落在换行行内部时以
  `startsMidLine`/`endsMidLine` 如实标记，而非声称完整行。
- `--lines <n>`：带值 flag，单次使用，仅 `terminal read` 受理；仅呈现层的尾部截取，
  保留最后 `n` 条已发出条目 `1..=65535`——`--unwrap` 下为逻辑行，否则为物理行。
  绝不扩大抓取窗口，其 `tail{requested,omitted,entries}` 计数与字节预算的
  `omittedRows` 分开报告。重复、非数值、零或越界值均为 `invalid-usage`。
- `--text <utf8>`、`--key <name>`、`--file <path>`、`--stdin`：`terminal send`
  四个互斥输入源，在其他命令上均非法。`--text` 与 `--file` 按标准
  取值规则消费下一个 token 作为值（以减号开头的 token 也会被消费，裸 `--`
  不会）；每次 `--key` 取一个封闭键名，最多重复 64 次；其他源只能使用一次。`--stdin` 无值。`--text` 必须是非空 UTF-8
  字面量，其字节长度加上可选折叠 CR 不越过 4,096 字节记录上限；`--file`
  原样存储其操作数——解析器不做任何文件系统校验，一切路径、类型与内容
  检查都归 [Send 输入源](#send-输入源) 的物化器。`--enter` 是文本类源的
  修饰符：`--text`、`--file`、`--stdin` 上合法，`--key` 上或单独出现均非法。
  `--paste` 仅能与 `--file` 或 `--stdin` 同用，生成一条粘贴记录及可选的末尾 Enter 记录。
  零个或多于一种源均为 `invalid-usage`；冲突按固定次序
  `--text`、`--key`、`--file`、`--stdin` 命名，与 argv 次序无关。
- `--mouse off|on`：带值 flag，单次使用，仅显式 `tui` 路由受理——宿主机指针捕获
  用于面板命中，默认开启。两种声明写法（`--mouse on` 与 `--mouse=on`）取值同为
  封闭的 `off|on` 词表；缺失或未知值、任一写法下的重复、出现在其他命令路由
  或裸调用，均为 `invalid-usage`。
- `--include-ended`：无值布尔 flag，单次使用，仅 `agent list` 受理；让请求页包含
  lifecycle 为 `ended` 的 session。
- `--limit <n>`：带值 flag，单次使用，仅 `agent list` 受理；页大小为一个
  `1..=100` 的十进制 `u64`，默认取 wire 契约的 50。重复、非数值或越界值均为
  `invalid-usage`。
- `--cursor <c>`：带值 flag，单次使用，仅 `agent list` 受理；铸造的
  `v1.<59>.<43>` 续页 token，逐字携带——真实性由 Server 侧校验，CLI 绝不验证。
  畸形 token 在任何运行时访问之前即 `invalid-usage`。
- `--until <condition>`：可重复带值 flag，由 `agent wait` 和 `agent prompt --wait` 受理；每次出现取
  十四名条件词表中的一个：`blocked`、`done`、`needs-attention`、`failed`、`succeeded`、
  `canceled`、`interrupted`、`working`、`idle`、`unknown`、`ended`、`fresh`、
  `stale`、`disconnected`。重复的值按请求顺序保留用于匹配与输出回显；重复
  同一值或未知名称均为 `invalid-usage`。完全省略 `--until` 时，Windows 依次使用
  `idle`、`done`、`blocked`；Linux 依次保持 `needs-attention`、`failed`、`succeeded`、
  `canceled`、`interrupted`、`idle`、`ended`。
- `--after-revision <n>`：带值 flag，单次使用，仅 `agent wait` 受理；可选的
  全局修订栅栏，取 `0..=9007199254740991` 安全整数范围内的一个十进制 `u64`。
  重复、非数值或越界值均为 `invalid-usage`。
- `--provider <id>`：带值 flag，可重复，仅 `agent doctor` 受理；每个值是一个规范
  provider id（`claude-code`、`codex`、`gemini`、`github-copilot`、`grok`、`cline`、
  `opencode`、`pi`、`omp`、`qwen-code`），按 flag 次序选取并在固定规范行序下去重——
  未知 id 在任何运行时访问之前即 `invalid-usage`。不带该 flag 即选取全部十个。
- `--asset-root <path>`：带值 flag，可重复，仅 `agent doctor` 受理；每个值是声明为
  capsule 候选根的非空绝对无 NUL 路径，按 flag 次序保留。相对拼写、`~`/`$` 形式、空值
  与第五个值在任何运行时访问之前即 `invalid-usage`——至多受理四个 supplied root，
  任何一个都绝不展开、绝不猜测。
- `--provider <id>`：带值 flag，单次使用，仅 `agent integration` 命令受理；
  封闭的受支持 provider id 之一（十个成员，`agent integration list` 报告）。
  在 `explain`/`enable`/`disable` 上必填，在 `list` 上为可选过滤。其他取值均为
  `invalid-usage`。
- `--adapter-source <abs>`：带值 flag，单次使用，仅 `agent integration` 命令受理；
  一个字面绝对路径，指向开发/测试用的 adapter 负载，替换本次调用内打包的 cohort
  来源。相对路径或重复使用均为 `invalid-usage`。
- `--version`：仅在独占根调用时合法；其他位置或与其他参数并存都是 usage。
- `-h`/`--help`：根级独占 token，或已识别帮助路径（family 或完整命令路径）的单一后缀。
- `--` 之后的位置参数：`--` 之后的 token 只作位置参数，故 `orchbee -- status` 查询 status，
  `orchbee project get -- --help` 解析的是字面 label `--help`。

命令路径（固定词，无缩写、无别名）：

```
orchbee                         # 裸调用 → TUI（不变）
orchbee tui [--mouse off|on]    # TUI
orchbee --version               # 版本行（不变）
orchbee -h|--help | orchbee help   # 根帮助
orchbee status
orchbee status server | status client
orchbee project list | project get <target>
orchbee project create --label <label>
orchbee project rename <target> --label <label>
orchbee workspace list [--project <target>] | workspace get <target>
orchbee workspace create --project <target> --label <label>
orchbee workspace rename <target> --label <label>
orchbee workspace accept-directory <target> --path <path>
orchbee workspace close <target> --confirm
orchbee workspace close-status --project <id> --workspace <id>
orchbee workspace close-resume --project <id> --workspace <id> --request <close-id> --generation <n> --confirm
orchbee worktree list --project <target> --directory <directory-id>
orchbee tab list [--workspace <target> | --standalone] | tab get <target>
orchbee tab create [--workspace <target> | --standalone] [--label <label>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee tab rename <target> --label <label>
orchbee tab move <target> (--direction previous|next|first|last | --insert-index N)
orchbee tab close <target> --confirm
orchbee panel list [--tab <target>] | panel get <target>
orchbee panel rename <target> (--label <text> | --clear)
orchbee panel split [<target> | --panel <target> | --current] --direction above|below|left|right [--label <label>] [--ratio <number>] [<geometry>] [--exec <path> [--arg <value>]... | --profile <selector>] [--focus | --no-focus] [--client <id>] [--timeout <ms>]
orchbee panel close <target> --confirm
orchbee panel move <target> --target-panel <target> --direction above|below|left|right
orchbee terminal list | terminal get <target> | terminal read <target> [--history --rows <1-1000>] [--unwrap] [--lines <1-65535>]
orchbee terminal wait-output <target> (--match <text> | --regex <pattern>) [--rows <1-1000>] [--timeout <1-3600000>]
orchbee terminal send <target> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>
orchbee launch-profile list | launch-profile get <selector>
orchbee launch-profile create --name <name> --exec <path> [--arg <value>]...
orchbee launch-profile delete <selector>
orchbee agent list [--include-ended] [--limit <1-100>] [--cursor <v1.…>]
orchbee agent get <target> | agent explain <target>
orchbee agent read <target> [--history --rows <n>] [--unwrap] [--lines <n>]
orchbee agent send-keys <target> <key>...
orchbee agent prompt <target> <text> [--wait] [--until <condition>]... [--timeout <ms>] [--forever]
orchbee agent wait <target> [--until <condition>]... [--after-revision <0-9007199254740991>] [--timeout <1-3600000>] [--forever]
orchbee agent doctor [--provider <id>]... [--asset-root <path>]...
orchbee agent integration list [--provider <id>]
orchbee agent integration explain --provider <id>
orchbee agent integration enable --provider <id> [--adapter-source <abs>]
orchbee agent integration disable --provider <id> [--adapter-source <abs>]
orchbee client list | client get <client-id> | client control-status <operation-id>
orchbee workspace focus <target> [--client <id>]
orchbee tab focus <target> [--client <id>]
orchbee panel focus [<target>] [--panel <target> | --current] [--client <id>]
orchbee panel zoom [<target>] [--panel <target> | --current] [--toggle | --on | --off] [--client <id>]
orchbee api snapshot
orchbee api schema [<schema-id>] [--output <path>]
orchbee site start
orchbee site stop --confirm [--request-id <id>] [--retry] [--detach] [--timeout <ms>]
orchbee site stop-status <request-id>
orchbee site diagnose
orchbee config check
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --skill
orchbee help <recognized path>
orchbee <recognized path> -h|--help
```

帮助主题：根级；`client`、`project`、`workspace`、`worktree`、`tab`、`panel`、`terminal`、`launch-profile`、`agent`、
`agent integration`、`api`、`site`、
`config`、`completion` 等族；以及包括 `tui` 在内的每条完整命令路径。`help` 只接受已识别的路径（不带
过滤器、不带 format）。路径末尾的 `-h` 或 `--help` 是同一路径上的同一请求：`orchbee --help`、
`project --help`、`project get --help` 输出的内容与 `help`、`help project`、`help project get`
完全相同。后缀 help 优先于缺失的业务参数（`project get --help` 描述 `project get`），但不优先于
已给出的业务参数或多余 token（`project get x --help`、`status extra --help` 都是 usage）。未列出的
目标如 `help help`、`help version`、`help --version` 按未知路径 → usage。Help、`--version`、
`--skill`、`completion` 与不带 `--output` 的 `api schema` 绝不触碰平台、运行、cohort 或 Site 状态。显式 Windows schema 导出仅操作调用者选择的文件目标及自有同目录临时文件。
`--skill` 必须是根命令的唯一参数，输出内嵌且本地化的 CLI 使用指南。PowerShell
补全是静态语法投影：绝不执行用户表达式或查询 Site；需显式加载，不修改 shell profile。

Elvish 补全与 PowerShell 使用同一份由 Rust 维护的 JSON 语法。回调接收交互编辑器提供的
字面词语（命令在首位，未完成词在末位）。每次调用的纯解析器记录已完成的位置参数、已见
flag、字面值和待消费值的所属 flag。`--` 与终止选项停止补全；非法作用域、重复或冲突使可行
命令集合为空。可行路径决定下一个命令词、未使用且兼容的 flag、操作数预算和封闭枚举值；
连写值保留 `--flag=` 前缀。自由形式的值没有候选。回调去重输出候选，匹配与引用由 Elvish
负责。它不求值任何传入词语，也不启动进程。只注册 `orchbee` 与 `orchbee.exe`，不修改
profile 或其他补全器。生成脚本遵循共享的 8 MiB 输出上限。独立的固定输出测试要求每个
补全脚本和帮助正文小于 256 KiB，用于提示意外的语法体积增长。在交互式 Elvish 0.21 中显式
执行 `eval (orchbee completion elvish | slurp)` 加载。

共享补全用例检查可接受与失效前缀、帮助深度、目标冲突、枚举所属参数及连写形式。Elvish
解释器测试在可移植验收中明确忽略；手动执行时必须用 `ORCHBEE_TEST_ELVISH` 指定解释器，
缺少解释器会使该次测试失败。测试提供注册命名空间，在无编辑器环境执行未经修改的脚本。
Windows ConPTY 证据单独检查实际交互注册与加载。这证明静态补全，不涵盖运行时目标发现
或其他操作系统。

`[<geometry>]` 代表可选的全有或全无组 `--cols <n> --rows <n> --cell-width <n>
--cell-height <n>`；省略时命令请求默认的 80x24 网格与未知单元像素 (0,0)。

`help`、`completion`、`tui`、`--version`、`--skill` 与裸调用是固定文本输出，不接受 `--format`；一个合法
`--format=json` 会把随后产生的语法失败变成 JSON `invalid-usage` 失败文档，而 text/默认失败仍
保持 stdout 为空。数据命令（`status`、全部 `list`/`get`、`workspace close-status`、`agent explain`、`agent doctor`、`agent wait`、
`api snapshot`、`api schema`、`site start`、`site stop`、`site stop-status`、`site diagnose`、`config check`，以及全部四条 `agent integration` 命令）与全部 mutation 命令接受 `--format`。`api` 命令是机器面：`api snapshot` 或 `api schema` 成功时在
两种 format 下输出同一份 JSON 文档；`api` 失败则与其他命令一样跟随所选 format——text/默认下
只写 stderr、stdout 为空，只有 `--format json` 才产出失败 envelope。

`orchbee completion` 打印本语法的静态投影：动词、完整帮助路径、每条命令已声明的 flag
（分写与 `--flag=value` 连写两种拼写）、`--format` 的两种拼写及其 `text`/`json` 值、
封闭词表值在两种拼写下的补全（`--mouse`、`--direction`、`--key`、`--until`、`--provider`）、
`--` 终止与 flag 值消费上下文。不枚举任何 Site 状态、资源名或运行时数据，也绝不猜测
自由形式的值。

### 目标与过滤器

`<target>` 操作数有三种解析形态。第一种是完整 typed ID（恰好 32 位小写十六进制、非零），
只按身份匹配。第二种是显式 selector `@<kind>:<suffix>`——`<kind>` 为 `project`、
`workspace`、`tab`、`panel`、`terminal` 之一，`<suffix>` 为一至三十一个小写十六进制
字符——在已读快照上解析到该 kind 中标识符以该后缀结尾的唯一成员。已识别 selector
前缀但后缀为空、含大写、为 32 位数字，或 kind 与操作数 resource 不符，在任何运行时访问
之前即 `invalid-identifier`（退出 2）；其余 `@` token 保持 label 解读——只有五个前缀是
selector 语法。其他 token 在 resource 允许 label 时视为精确、区分大小写的 label 候选，
要求 1–512 个 Unicode scalar；空或过长的 token 与全零 ID 形值都是 `invalid-identifier`
（退出 2）。本身恰为合法 ID 或 selector 形状的 label 永远无法按 label 或 selector
命中——该实体仍可通过其真实 ID 访问。`panel` 与 `terminal` 只接受完整 typed ID 及各自的
`@panel:`/`@terminal:` selector，其余一律 `invalid-identifier`。过滤器
（`--project`、`--workspace`、`--tab`）使用各自资源相同的 target 规则；所有 mutation 位置
操作数与 `--target-panel` 同样遵守（`panel` 规则，并在被移动 panel 自身所在
tab 内解析）。`api schema` 接受非空
schema ID；空串是 `invalid-identifier`（`resource="schema"`），无匹配是 `not-found`。
`agent get`、`agent explain` 与 `agent wait` 的位置目标是 `[A-Za-z0-9._:-]` 的
1–128 字符 token（也支持显式 Panel 前缀）；按上述 ID 优先规则解析，或用 `--session`/`--name`/`--panel` 明确选择。
畸形 token 在任何运行时访问前即 `invalid-usage`。
`launch-profile get` 与 `launch-profile delete` 接受一个位置 `<selector>`，
`--profile` 携带同一 token：`LaunchProfileSelector`——非空 UTF-8 字面量、至多 512
字节（即名称上限，使每个受理的 selector 都能触达任一规则）。空或超长 token 为
`invalid-usage`；CLI 绝不把它拆成 id/label 分派——由 Server 在目录锁下解析：先精确
profile ID，再按名称、匹配基数必须恰为一。零匹配应答 `not-found`；名称匹配多于一个
条目应答 `ambiguous`，报告至多前 8 个候选 profile ID 加真实 `totalCount`——两者均走
专用 launch-profile 失败 envelope。

读取成功后的解析：零匹配 → `not-found`（3）；多个精确 label 或 selector 后缀匹配 →
`ambiguous`（3），按 canonical 遍历次序报告至多前 8 个候选 typed ID 与真实 `totalCount`。
selector 候选是命令本已读取的那份快照中可见的成员——对每个操作数声明的作用域都是完整
集合，绝不追加抓取——且 selector 是当下句柄而非持久句柄：删除再创建可能让同一后缀落到
不同成员上，需要稳定身份的命令使用完整 typed ID。命名不存在实体的过滤器是
`not-found`；实体存在但结果为空是成功的空表。

## 查询状态机与预算

每个网络查询（`status`、`list`/`get`、`api snapshot`）在 Linux 上严格按下列顺序执行；第一个
失败阶段即决定失败：

1. **Cohort** —— `NativeCohort::current` 在一个 checked 5 秒单调 deadline 下运行
   （`Instant::now().checked_add(5s)`；`checked_add` 失败为 `internal-failure`、退出 1）。
   任何 cohort 错误（缺失、非法、不安全、散列不符、协作式 deadline）都是 `check-failed`、
   stage `cohort`。本地化 stderr 诊断额外报告观测到的 `io::ErrorKind`——存在原始 OS
   错误码时一并报告——不含路径、OS 消息文本或因果断言；机器记录不变。
2. **Discovery** —— 解析 `ORCHBEE_RUNTIME_BASE`/默认值，然后
   `ModeCheckedLocalSiteRendezvous::validate_mode_checked`（`ExistingOnly`，不 mkdir、不取锁）。
   分类使用 `RendezvousError` 的 typed path 与已解析 `runtime_base`/`runtime_base.join(scope)`
   比较：scope 目录上 source 为 `NotFound` 的 `System` 错误是 `service-unavailable`、stage
   `discovery`；其余一切 rendezvous 或文件系统校验失败都是 `check-failed`——typed path 属于
   基座或基座组件时 stage `runtime-base`，属于 scope 目录时 stage `runtime-directory`。
   前置观测不宣称与后续 open 原子。
3. **Socket** —— `client_socket_exists()` 复用既有 类型+属主+`0600`+no-follow 校验：不存在 →
   `service-unavailable`、stage `discovery`；无效/出错 → `check-failed`、stage `socket`。
4. **Admission** —— 紧邻 `LocalClient::connect_before` 之前建立一个新的 checked 5 秒单调
   deadline；同一 `Instant` 原样沿用到快照读取，绝不续期，绝不回退到 50 秒请求默认值。
5. **Read** —— 恰好一次 `submit_workbench_snapshot_read_before`，使用全新
   `WorkbenchRequestId`，受同一 deadline 约束。一次 OBLC 准入加一次 OBWP 请求；无重连、重试、
   publication 追赶或第二次读取。
6. **Release** —— 在任何映射、格式化、输出之前 RAII 释放全部自有 `LocalClient`/socket 句柄；
   慢 stdout 因此绝不持有 Session。不存在也不宣称 shutdown 交换；Server 可异步观察到 EOF。
7. **Map** —— 把已验证快照纯函数式地投影为命令结果。

`config check` 纯本地：不 connect socket、不取锁、不 mkdir/chmod；其 cohort 检查使用单独的
checked 5 秒预算。`site diagnose` 同属纯本地命令——只做描述符相对的路径与锁观察；绝不
connect、绝不创建锁或标记、绝不修复——共享一个在有界文件系统操作之间检查的协作式 5 秒
预算。Linux 和 Windows 以外的平台对网络查询与 mutation 产出 `service-unavailable`
`{stage:"platform",reason:"unsupported-platform"}`，对 `config check` 产出同 details 的
`check-failed`；不存在向无界连接 API 的回退。`site start` 在 Linux/Windows 以外、
`site diagnose` 在 Linux/Windows 以外的平台以同 details 的 `service-unavailable` 应答。

`agent doctor` 是离线 provider 钩子诊断：仅凭有界文件系统事实解释 Agent 观察为何可能无法
到达。它不执行 provider 命令、不 connect socket、不发现 Site、不取锁、不做任何写入——检查集
即上文声明的集合，每次 lstat/read/list/PATH 扫描都运行在同一个 5 秒协作预算下，至多 64 个探测
项、聚合 16 MiB 读取与单文件/计数上限，项间设取消检查点。Linux 和 Windows 以外的平台以
`check-failed` `{stage:"platform",reason:"unsupported-platform",command:"agent doctor"}`
应答。触及上限的运行仍产出文档并标 `complete:false`——部分事实是显式的，绝不静默丢弃——
`providers[]` 始终按规范次序报告每个所选 provider。`hints[]` 仅作观察：每个
`{id,evidence,text}` 指明其引用的具体事实行，不含因果、修复或信任断言。

### Windows Agent 诊断细化

Windows `agent doctor` 拥有相同的有界离线观察流程，输出 `orchbee.agent.doctor`
版本 3；Linux 保持版本 1，schema 继续保留原版本 2 词汇。Capsule 0.4.1 支持
原生 Windows Codex hook 传输，但 doctor 不连接命名管道，也不写 Console 关联 hint。
端点保持 `not-inspected`，原因改为 `windows-endpoint-not-inspected`；配置和胶囊
完整性不证明事件投递或 Site 存活。`complete` 描述探测覆盖率。
Windows 托管安装由 [Windows 集成契约](windows-agent-integrations.zh-CN.md) 负责；Windows Agent 启动见 [Windows Agent 启动](windows-agent-start.zh-CN.md)；prompt 组合契约见上文。

| 字段         | 契约                                                                                                                                                                                                                                                                               |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 输入与所有者 | 原生 doctor 独占负责只读文件系统、PATH 和环境观测。Windows home 使用原生用户配置目录；非空绝对 `CODEX_HOME` 指向现存非重解析目录时选择该 Codex 配置。空 `CODEX_HOME` 使用默认目录；无效非空根报告不可检查，不静默替换。该只读准入比 Codex 对相对路径或符号链接覆盖做规范化更严格。 |
| 准入         | 打开普通文件时不跟随末级重解析点，在有界读取前根据已打开句柄检查元数据。拒绝设备命名空间和相对路径；重解析条目不检查。祖先路径分量与同步文件系统延迟不属于硬截止时间保证。                                                                                                         |
| 决策         | provider 配置缺失、禁用、冲突或不可读，原生可执行文件或脚本可见或缺失，Node 可见或缺失，胶囊自洽、损坏或预算不足，以及 Windows 端点尚未检查，分别报告。PATH 搜索不执行条目，也不推断插件命令支持。                                                                                 |
| 兼容性       | Windows 接受原生绝对路径及 Windows 路径使用的单行 TOML 字面量字符串。既有 Linux 扫描语义和输出版本不变。嵌入 schema 将新增 Windows endpoint 词汇限定到版本 3；版本 2 保留原有封闭词汇。                                                                                            |
| 副作用与边界 | 不执行 provider、不连接 socket、不激活 Site、不加锁、不写入。沿用 5 秒协作预算、64 项、16 MiB 总读取量及各探测上限。取消返回 130；生成完整或部分诊断文档均返回 0。                                                                                                                 |
| 证据与限制   | Windows 隔离进程测试覆盖本地化及 schema 合法输出、路径和配置隔离、缺失及已安装声明、损坏资源与零安装副作用。文件系统测试覆盖拒绝重解析点及读取预算。doctor 测试不证明 hook 投递；未完成独立评审；按用户要求不执行 TLA。                                                            |

`terminal read` 沿用第 1–5 步原样不变，然后在同一 deadline 内让已准入的 session 保持存活
——绝无第二次准入——完成一次只读 viewer 观察：

1. **解析** —— 在已验证快照的 presentation 绑定中查找 typed terminal ID；不存在 →
   `not-found`（3）带 `{resource:"terminal",target}`，session 随之释放。绝不咨询 label。
2. **Attach** —— 以快照自身的事实发起一次 `terminal.viewer`
   `AttachBinding{siteId,workerId,terminalId}`。`Attached{attachmentId,hostEpoch}` 用于构造
   限定该 attachment 与 epoch 的 `TerminalProjection`；其他 outcome 类别按失败矩阵映射。
3. **Poll** —— `Poll{attachmentId}` 在同一 deadline 内重复：`Empty` outcome 等待 10 ms 后
   重试；`record` outcome 送入投影。record 未通过 transport 或帧校验是 `protocol-invalid`、
   stage `viewer`；投影以 `NeedSnapshot` 回应的 record 会发送一次该反馈（得到
   `FeedbackAccepted` 应答）并继续轮询；已消费的 `ProjectionBlocked` 被忽略。投影接受的第一条
   record 即结束观察——被应用的帧就是答案；一次性读取绝不发送 `FrameApplied` 反馈。
4. **释放** —— 尽最大努力 `Detach{attachmentId}` 先于无条件的 session drop；两者都完成于
   任何映射、格式化或输出之前。viewer attachment 绝不保留到输出阶段，session drop 本身就是
   拆除保证。

`terminal read --history --rows <n>` 沿用同一 attach → poll 前导——`--history` 与 `--rows`
仅成对准入，`n` 是有界保留行数 `1..=1000` 而非几何维度——然后读取已应用帧背后的滚动
历史：

- 已应用帧的保留滚动行数事实为恰好一次 `ReadRows{attachmentId, startRow,rowCount,revision,generation,requestSequence:1,maximumBytes}` 定价：`rowCount = min(n, scrollback,
262144/columns)` 且 `startRow = scrollback - rowCount`，窗口止于滚动边界。滚动为零则短路
  ——不发送 `ReadRows`，答案为空窗口。
- `record` 应答必须是完整的 OBTP `RowsSnapshot`（kind 46）响应，其 OBRW 文本窗口独立回显
  请求：attachment、Host epoch、sequence、revision、generation、columns、起始行、行数，
  以及 `totalRows = scrollback + screenRows`，且载荷不超过 `maximumBytes`（8 MiB）。任何
  失配都是 `protocol-invalid`、stage `viewer`。
- `rows-rejected` 映射到类型化词汇：`limit-exceeded` 为 `rejected`（3）；Host 状态应答
  （`history-evicted`、`stale-revision`、`stale-generation`、`missing-resource`、
  `resource-exhausted`、`invalid-phase`）为 `service-unavailable`（4），details 为
  `{stage:"viewer",operation:"read-history",reason}`；固定请求形状不可能产生的 reason 为
  `protocol-invalid`。不重试，也绝不为此读取发送 `RowsApplied`/feedback 锚点。

`terminal read --unwrap`/`--lines` 是作用于已物化发出行的呈现变换——既不改变抓取
窗口，也不改变读取上限：

- `--unwrap` 只把相邻且经验证软换行相容的行（`wrap` 与 `wrap_continuation` 两两一致）
  拼成逻辑行；JSON `content` 随之发出 `lines[]`——`{firstRow,lastRow,text,startsMidLine,endsMidLine}`——替代 `rows[]`，并附只对观测片段计数的 `lineCount`（绝不是
  保留历史总行数）。`rowCount` 仍计物理行。窗口开头或结尾落在换行行内部时产出的是
  带标记的片段，绝不谎称完整行。
- `--lines <n>` 对已发出条目做尾部截取：无 `--unwrap` 时为物理 `rows[]`，启用时为逻辑 `lines[]`。`tail{requested,omitted,entries}` 成员与
  `omittedRows`（抓取/预算省略）分开报告这次呈现省略。text 输出每条已发出条目打印
  一行，并附一条本地化的"… 未显示前 N 行/逻辑行"标记，与读取上限标记相区分。
- 任一 flag 使封套升为 `version:2`（`orchbee.terminal.read` /
  `orchbee.terminal.history`）；不带 flag 的输出保持 `version:1` 逐字节不变。

`terminal wait-output` 沿用同一快照准入与 attach 前导，但只读 viewer 在单次帧之后继续
保持存活，进入有界观察循环：

- **期限**——在命令开始时取一个绝对 deadline `T0 + --timeout`（默认 30000ms，准入范围
  `1..=3600000`）。cohort、discovery、connect、snapshot 以及每次 `terminal.viewer` 请求
  都在 `min(5 s local budget, remaining deadline)` 内运行；每个等待步骤返回后都复查 deadline，
  因此迟到完成仍算 `timeout`（退出 7）。该 deadline 是协作式的：同步文件系统调用无法
  在系统调用中途打断，最坏超出量是一次已开始的本地操作。
- **观察**——`Poll` 在 deadline 内重复：`Empty` 先睡 10ms，连续空回退到 100ms，收到
  record 即复位（每次睡眠以剩余 deadline 为上限——安静等待绝不忙等）；`record` 喂给
  projection，对**每个**已应用帧发送真实 `FrameApplied` feedback，并在下次 poll 前确认
  `FeedbackAccepted`；`NeedSnapshot` 按原 feedback 规则；`ProjectionBlocked` 被消费并忽略。
- **检查**——每个已应用帧钉住一次 `ReadRows`，覆盖统一缓冲窗口
  `startRow = max(0, totalRows − K)`、`rowCount = K`，其中
  `K = min(--rows, totalRows, 262144/columns)`、`totalRows = scrollback + screenRows`，
  revision 与 generation 为该帧的 `.exact` 值，`maximumBytes` 为 8 MiB。
  `requestSequence` 每附件严格递增、从 1 开始（`checked_add`；重试也取新值，且绝不与
  `transport requestId`、frame sequence、history-event sequence 混用）；解码窗口必须回显
  所发 sequence 及全部钉住字段，否则为 `protocol-invalid`。
  `stale-revision`/`stale-generation`/`history-evicted` 舍弃本次观察并在同一 deadline 内
  继续轮询——绝不匹配半窗口；其余 `rows-rejected` reason 按与 `read-history` 相同的
  typed 词汇映射（operation `"wait-output"`）。行编码器无法编码的载荷以 typed
  `rows-rejected` 失败——绝不静默截断。
- **匹配**——仅当 `wrap` 与 `wrap_continuation` 两两一致时窗口行才拼成逻辑行；不一致即
  硬边界，窗口边缘的残缺行保持诚实的部分行（绝不拿窗口外文本补齐）。每个观测片段
  独立搜索——硬边界或未抓取的缺口两侧绝不互相匹配。首个包含模式的片段胜出：
  `--match` 为字面、区分大小写的子串，`--regex` 为正则引擎的最左匹配。`^`/`$` 只锚定
  观测片段的边缘，合法的零长度匹配（观测空行上的 `^$`）也算命中。返回的 `excerpt`
  是该片段内 ≤4096 字节的片段，始终包含完整的 `--match` 字面量；`--regex` 命中可超过
  该上限，此时 excerpt 以命中起点开头，由扩展成员如实报告——`matchLength` 为完整字节
  长度，`matched`/`matchedTruncated` 为 ≤4096 字节的命中文本，`startsMidLine`/
  `endsMidLine` 为片段边缘事实，均随 `patternKind:"regex"` 出现（封套 `version:2`；
  字面命中保持 `version:1`）。`matchOffset`、`excerptOffset` 是拼接片段内的 UTF-8 字节
  偏移，绝非终端列。命中后命令返回物理
  `rowStart`/`rowEnd`（仅观察期有效，不是游标）、帧 `revision`/`generation`、已解析的
  terminal/worker/site 身份，以及观察到的 `hostEpoch`/`attachmentId`；
  `observed.completedObservations` 只计一致的窗口+匹配通过次数，
  `observed.windowTruncated` 记录被容量钳制的 `K`。
- **取消**——等待期间的 HUP/INT/QUIT/TERM 是本地 `cancelled` 失败（退出 130）：不伪造
  match，也不向终端发送输入、语义取消或 kill。检查点在网络步骤之间与每次 poll 睡眠
  前后，延迟以一次已开始的本地请求为界。
- **释放**——成功时在产出文档前确认 `Detach` 结果；清理未确认则保留真实失败类别并上报
  `{matchObserved:true,cleanup:"unconfirmed"}`，绝不转换。timeout、载具失败与取消保留其
  主失败及清理状态；session drop 总会执行，当没有剩余 Detach 预算时上报为
  `cleanup:"requested"`——绝不谎称已释放。

`terminal send` 在命令入口采样一个绝对的 5 秒 deadline 并贯穿以下全部——声明型
`--file`/`--stdin` 的源物化（第一个有副作用的步骤，先于 cohort 检查）、cohort 检查在
自身预算与该上限的较早者之下运行、discovery、admission 与快照读取只消耗剩余额度——然后
在同一 deadline 内用同一已准入 session 执行恰好一次 `terminal.send` 交换。
`--text`/`--key` 调用完全没有物化步骤：

0. **物化**（仅声明型源）—— `--file`/`--stdin` 在任何运行时访问之前被完整读入
   下文定义的有界字节或粘贴内容（较小的 UTF-8 输入沿用文本记录路径）；见 [Send 输入源](#send-输入源)。源失败不拥有
   请求身份，也绝不抵达 Server。
1. **解析** —— 在已验证快照的 presentation 绑定中查找该 typed terminal ID；缺失 →
   `not-found`（3），details 为 `{resource:"terminal",target}`，session 随之 drop。
   绝不查询 label。
2. **发送** —— 一条 `terminal.send` 请求携带有序记录序列，在新的请求身份下使用
   快照中的 Site、Worker 与终端事实。`--text` 生成一条 UTF-8 记录（含可选 CR 共
   4096 字节）；`--file`/`--stdin` 生成一条不透明字节记录（含可选 CR 共 1 MiB），
   较小的 UTF-8 内容沿用文本路径。`--paste` 改为生成一条粘贴记录（256 KiB）及
   可选的独立 Enter 记录。重复 `--key` 按参数顺序生成最多 64 条语义键记录；键
   使用协议名称，不是预制转义字符串。在映射类型化结果或准备输出之前完整释放
   Client Session。包括部分接受在内，请求绝不重试。
3. **应答** —— Server 的 `terminal.send.result` 拆分为 envelope 逐字报告的两条轴：
   `inputStatus`（`accepted` 表示 Host 已把输入记录入队——绝不表示子进程已读取或执行；
   `rejected`/`not-submitted` 表示该记录可证明从未到达队列；`unknown` 表示其可能
   已经落地）与 `cleanupStatus`（`not-needed`/`confirmed`/`unconfirmed`，其中
   `unconfirmed` 表示临时 input-only attachment 可能存续至 carrier 拆除或 Host-epoch
   结束——绝不宣称已释放）。发送经由共享的 Worker↔Host 连接：当该连接失效、或其
   清理无法确认时，同一 Host 上的其它 viewer 可能被中断——这是底层连接的
   transport 级副作用，并非正常 send 夺取控制权。结果的物理 `target`（`workerGeneration`、`hostEpoch`、
   `hostTerminalId`）是 Server 实际准入的 incarnation，逐字投影；准入前拒绝不携带。
   退出映射：`accepted`+`confirmed` → 0；`accepted`+`unconfirmed` → 5 并附勿重发警告
   （输入可能已生效）；`rejected` → 3；`not-submitted` → 4，唯协议故障 reason
   `host-protocol-fault` 属 transport 类缺陷 → 5；`unknown` → 8 并附
   绝不重试警告。每个非零应答都保持 `inputStatus` 诚实——已接受的输入绝不改写为未发送。

`worktree list` 共享网络前导的第 1–4 步外加一次快照读取，全部处于命令入口一次性采样
的同一个绝对 5 秒 deadline 内（`snapshot_session_until`——cohort、discovery、socket、
admission、快照与 worktree-list 交换各自只消耗剩余额度；不做按阶段续期）。随后：

1. **解析** —— `--project` 目标按普通 project target 规则在已验证快照上解析（精确 ID
   或精确 label；`not-found`/`ambiguous` 保持通用失败 envelope——快照边界即该命令的
   family 边界）。此处绝不检查目录 membership。
2. **读取** —— 在同一已准入 session、同一 deadline 内，以新的 `WorkbenchRequestId`
   提交恰好一次 `workbench.worktree-list.read` 请求 `{projectId,directoryId}`。
   Server 在该时刻权威核验目录对 project 的 membership，并派发一个有界 Worker
   发现 job——先 `git rev-parse --path-format=absolute --git-common-dir` 探测，
   再 `git worktree list --porcelain -z` 列举，二者共享一个 5 秒 job TTL 与一份
   64 KiB 合并 stdout 预算；越过共享预算的 stage 承担 `too-large` 归因。该读取绝不
   重试、重放或重连；不可判定的交换即 `outcome-unknown`（8）。
3. **映射** —— session 在类型化 `worktree-list.result` 投影之前完全释放。`Found`
   产出 `orchbee.worktree.list` 文档：已注册的 `{projectId,directoryId,path}` 身份
   加逐字 `entries`（`worktreePath`、`head`、`branch`、`bare`、`detached`、`locked`、
   `prunable`——经 `*BytesHex` 备选字段字节精确）。该交换产出的每个失败——含
   transport 与 protocol 缺陷——都携带专用的 `orchbee.cli.failure.worktree`
   envelope；`not-found{resource:"directory"}` 是目录缺席的应答，绝不出现部分条目集。

`launch-profile list` 与 `launch-profile get <selector>` 共享网络前导的第 1–4 步外加
一次快照读取，全部处于同一个 5 秒 deadline 下，然后：

1. **读取** —— 在同一已 admitted session 上、同一 deadline 内、一个全新
   `WorkbenchRequestId` 下发出恰好一次 `workbench.launch-profile-catalog.read` 请求
   `{selector|null}`。selector 逐字透传；Server 在目录锁下解析（先精确 profile ID，
   再按名称基数）。该读取绝不重试、重放或重连；不可判定的交换按失败矩阵映射。
2. **映射** —— session 在类型化 `launch-profile-catalog.result` 投影之前完全释放。
   `Catalog` 产出 `orchbee.launch-profile.list` 文档
   `{catalogRevision,profiles:[{profileId,name,launch}]}`（至多 64 条，逐字）；
   `Profile` 产出 `orchbee.launch-profile.get` 文档
   `{catalogRevision,profile:{profileId,name,launch}}`。该交换产出的每个失败——含
   transport 与 protocol 缺陷——都携带专用的
   `orchbee.cli.failure.launch-profile` envelope；
   `not-found{resource:"launch-profile"}` 是 selector 未命中的应答，`ambiguous`
   携带受限候选 ID。两个成功 envelope 都携带
   `source:{kind:"launch-profile-catalog",siteId, catalogRevision}`——目录私有
   revision，与 Workbench 导航 revision 是不同的计数。

### Send 输入源

Windows 管道读取拥有一个隐藏、非交互的 Windows PowerShell 读取进程，
从操作系统系统目录解析。仅传入继承的 stdin 句柄，来源字节不会变成脚本
或参数文本。固定脚本最多向私有输出管道复制来源上限加一个探测字节。
CLI 在现有期限和中断守卫下轮询，并在成功及已处理的失败路径终止和回收子进程。
不修改调用者的管道模式，也不遗留阻塞的读取线程。普通文件 stdin 保留
直接读取方式。进程启动和终止属于操作系统调度边界，不提供硬实时保证。

适配器使用已准入的 `winsafe` 系统目录和 `winapi-util` 句柄类型绑定，
沿用锁定版本，不引入自编写的不安全代码。状态为已接纳 -> 读取中 ->
完成/失败/超时/取消 -> 子进程已回收，仅完成可进入提交。
`source/input.rs` 拥有原始字节/paste 路由，文件/stdin 不限定为 UTF-8。
Windows 测试用真实读取子进程验证 EOF、上限和超时，并注入信号观察验证取消；
这些测试不模拟物理 Ctrl+C。TLA 执行延期。

`--file` 与 `--stdin` 是声明型本地源。语法校验先于源读取；只有完整有界来源
存在后才进行 cohort 发现、准入、快照和提交。较小的合法 UTF-8 内容保留文本
记录路径，其他内容成为原始字节。Paste 内容成为一条 paste 记录，搭配
`--enter` 时再附加独立的尾随 Enter 记录。

- **纯输入边界。** 四种来源互斥，只有 `--key` 可重复，最多 64 条记录。
  `--enter` 不能搭配按键，`--paste` 必须搭配文件/stdin。
  准入前源失败不提交任何内容，也不拥有请求身份。
- **单一共享预算。** 命令入口采样的绝对 5 秒 deadline 覆盖源 setup、物化与整个 send
  生命周期；绝不重置或续期。每次源等待与每次读取之前都检查 deadline——包括恰在
  deadline 处结束的等待之后的读取——成功边界本身同样受仲裁：在预算关闭之后才
  观察到的输入结束是 `timeout`，绝非干净记录；过期之后不再发生任何运行时准入。
- **Linux 文件准入。** `--file` 以 `O_RDONLY|O_NONBLOCK|O_CLOEXEC` 打开——open 本身不会在
  FIFO 或设备上阻塞——随后在已打开 fd 上的 `fstat` 必须报告 regular file，这同时闭合
  路径替换竞态：被换掉的路径仍报告其真实类型。打不开/读不了的路径、非 regular 结果
  （目录、FIFO、设备或被替换的项）、空文件或读错误都是源阶段 `invalid-usage`。
- **Linux stdin 准入。** `--stdin` 拒绝终端描述符，再按继承描述符的精确 `S_IFMT` 类型分派：
  regular 文件在借用描述符上原位、无缓冲读取——共享文件位置恰好按返回字节数推进，
  至多 cap+1 探测字节（这正是继承位置契约）；
  FIFO/管道经 `/proc/self/fd/0` 重开为私有非阻塞文件描述，因此绝不改动调用方描述符
  旗标、也无需恢复；重开结果必须验证为 FIFO，其他一切类型（socket、设备等）一律拒绝。
- **有界记录。** 文件/stdin 最多读取 1 MiB 字节上限加一个探测字节，或
  256 KiB paste 内容上限加一个探测字节。原始字节（含 NUL 和非 UTF-8 数据）
  原样保留，不裁剪、不去 BOM、不归一 CRLF。空输入被拒绝。可选附加回车
  计入非 paste 上限，paste 则使用独立 Enter 记录。字面 `--text` 保留连同
  回车 4096 个 UTF-8 字节的上限。载荷不会出现在诊断中。
- **源信号 guard。** 在 Linux 上源阶段在其存续期内拥有进程信号捕获——单线程、无其他
  信号 owner、交接前不 spawn 子进程。合格集为 {SIGHUP, SIGINT, SIGQUIT, SIGTERM}
  减去进程在源入口已阻塞或忽略的全部信号：忽略集取自严格有界的
  `/proc/self/status` `SigIgn` 字段（最多读 64 KiB，须恰好一个合法十六进制字段——
  缺失、畸形、重复或超限均在任何源 I/O 之前报 internal failure），阻塞集取自当前
  线程 mask。Setup 先为合格集创建 `NONBLOCK|CLOEXEC` signalfd——此时继承 disposition
  仍然生效，故该处 setup 故障无需恢复任何东西——随后原子交换线程 mask 并捕获旧
  mask；捕获区间自交换起、先于任何源读取。绝不触碰 disposition，继承的 blocked/
  ignored 状态原样保留——空合格集是真实的 armed 策略（无可捕获），不是 no-op
  回落；继承的 pending 信号在原 mask 下保持 pending。
- **仲裁与最终化。** 读取循环把信号描述符与源描述符并列 poll，在处理就绪数据之前、
  判定超时之前、以及完成边界再各 drain 一次；首个被消费的合格信号选定源取消，且
  优先于 deadline 与源 I/O 错误。物化收尾以最后一次非阻塞信号读为线性化点：读到即
  取消；`EAGAIN` 表示源不再拥有取消——信号描述符关闭、旧 mask 在运行时准入之前被
  精确恢复。该读之后到达的信号保持 pending，在 unmask 时按原 disposition 交付——
  guard 绝不吞掉它——故迟到信号仍可能在准入前以默认行为终止进程。每个受监视
  syscall 都被检查：描述符创建、mask 交换、每次信号读与恢复；出错即 internal
  failure，绝不当作"无信号"；恢复失败是终态且无运行时准入。取消路径上合格 mask
  保持阻塞直到公共 `main` emit 完成输出尝试，再在同一路径恢复旧 mask——之后的
  信号仍可能在输出完成后终止进程，不对此承诺退出码。
- **失败兼容。** 校验与本地原因留在标准 `invalid-usage` 路径（无 details）。源超时与
  取消携带 `orchbee.cli.failure.source` v1 envelope——`{stage:"input",
inputSource:"file"|"stdin"}`，仅超时另带 `timeoutMs`（配置的共享预算，绝非实测
  时长）——无权威 `source`、无请求身份，因为尚无任何准入。源取消经与其他输出
  相同的公共 emit finalizer 按惯例 `128 + signo` 退出（129/130/131/143）——状态
  覆写仅存在于该 envelope——源超时仍为退出 7。
- **已声明的 OS 边界。** deadline 是每个 syscall 周围的逻辑界限，不进入其内部：内核
  卡死的 regular-file 读不可抢占，故不承诺该处及时终止。mask 交换前的 pre-arm 窗口
  在继承 disposition 下运行——该处默认死亡是诚实且无副作用的，不是 guard 窗口
  漏洞，且任一种 arming 次序都不能闭合全部信号到达竞态。继承的 `SIG_IGN`
  disposition 被尊重而非覆盖。以上均为依托 procfs 的 Linux-only 声明，与 procfd
  输入准入一致。

`agent list`、`agent get` 与 `agent explain` 沿用与每个网络查询相同的
cohort → discovery → socket → admission 前导，但共享一个在调用入口采样一次的绝对
5 秒命令预算：前导、准入、可选名称解析与 OBAP 读取共用同一个 ceiling，阶段之间绝不刷新——
预算耗尽的后续阶段不会启动，在 ceiling 上或其后完成的步骤结算为命令自身的
timeout。它们完全跳过 Workbench 快照读取。它们在唯一已准入的 Client Session 上
必要时先完成上述名称解析，再执行一次 Agent Observation Protocol（OBAP）读取交换：

1. **请求**——一个 OBAP 帧（`OBAP` magic，version 1），使用全新铸造的
   `ObapRequestId`：`agent.session.list.request`（kind 1）携带 `includeEnded`、
   `limit` 与可选 `cursor`（首页省略——绝不序列化为 `null`，线上显式 `null`
   亦被拒绝）；`agent.session.get.request`（kind 3）或
   `agent.session.explain.request`（kind 5）携带已解析的 `sessionId`。编码后的
   请求以 16 KiB 为界。
2. **结果**——恰好一个匹配的结果帧（kind 2/4/6），list 以 1 MiB 为界、
   get/explain 以 128 KiB 为界；小型拒答走 ≤1 KiB 界限。帧的 `requestId` 必须
   等于所发身份，结果 kind 必须匹配所发 operation——任何失配均为
   `protocol-invalid`、stage `agent-read`。
3. **释放**——Client Session 在任何结果映射或输出准备之前完全 drop。提交后
   不可判定的结果会 retire 该 session 且绝不重放：不重连、不二次准入、不重试。

在 Linux 上，Agent 读取命令为整个 armed span 拥有一套独立、更窄的信号
watch——仅 {SIGINT, SIGTERM}，在任何运行时接触之前通过源阶段所用的同一套
私有 arming 机制 arm：合格集排除进程已 block 或 ignore 的每个信号，借用的
信号描述符并入每次准入与交换等待，精确的旧 mask 在输出期间保持屏蔽、随后
由同一个不可克隆的 finalization 令牌恢复。arming 运行之前，进程信号行为
保持继承；SIGHUP 与 SIGQUIT 在整个调用期间绝不 mask、绝不捕获——完全保留
其继承 disposition，那里的默认原始死亡是诚实结果、CLI 不予上报；继承为
ignored 或 blocked 的成员只是离开合格集（空集仍是真实的 armed 策略）。
arming 本身就确立了取消所有权——与准入竞速无关——因此被捕获的信号无论在
准入期还是读取期落地，都将本次调用结算为 `orchbee.cli.failure.agent` 文档
上的 `cancelled`：`{stage:"agent-read", operation}`、无关联 source，任一
成员的退出状态均为 130。130 对 SIGINT 与 SIGTERM 是刻意统一的——不是
`128 + signo`，信号身份不予暴露。在唯一的 finish 仲裁上信号优先于
deadline、transport 故障与准入结果，且该仲裁仍只在已准入 session drop
之后运行。在 clear 仲裁下幸存的临时 wake 观测判为 `internal-failure`，
绝不判为 transport 或 timeout。

语义应答映射到 `orchbee.cli.failure.agent` 文档下的固定失败类别：
`listed`/`found` 是仅有的成功 outcome；`rejected{read-denied}` → `denied`；
`rejected{not-found}` → `not-found`（仅 get/explain，指名该 session）；
`rejected{cursor-invalid|cursor-stale}` → `rejected`（仅 list）；`unavailable` →
`service-unavailable` 且 `reason:"runtime-unavailable"`。交换内 deadline 耗尽为
`timeout` 且 `timeoutMs:5000`；transport 故障为 `transport-loss`；畸形、失配或
correlation 不符的帧为 `protocol-invalid`。准入后的全部失败携带
`{stage:"agent-read",operation:"list"|"get"|"explain"}`（wait 命令各臂上为 `"wait"`）；相关联的 `agent-read`
source 只附在可信语义应答上——transport、timeout、protocol-invalid 与
cancelled 失败均不携带。Agent 读取只观察 Server 的 canonical Agent session；`ended` session
被如实报告、绝不复活，`--include-ended` 放宽的是页过滤器而非生命周期本身。

`agent wait` 持有一个已准入的 Client Session。Windows 按上述 Agent 等待策略读取规范
变更及当前明细；Linux 保留 Get 轮询。每次交换都有新的请求身份，历史字段绝不覆盖
当前明细。在命令开始时采样的可选绝对单调 deadline 覆盖准入、每次读取
与每次退避；`--forever` 只省略该总期限；每轮在 `min(5 s, remaining total)` 下运行。两次未命中观察之间，循环睡一个
可中断的私有退避：起始 50ms、每次静默轮翻倍、封顶 500ms 且受剩余预算约束——
信号 wake 描述符并入该睡眠，持有的 session 在唯一的 finish 仲裁之前 drop，
与单次读取完全一致。retire 或结果不可判定的交换之后绝不重新准入、重放或新开
session。

每份返回的文档在客户端按请求顺序求值。三个新鲜度条件（`fresh`、`stale`、
`disconnected`）直接匹配观察的 freshness；每个状态条件还要求 `fresh` 新鲜度
；原生状态条件匹配 status `kind`，`blocked` 和 `done` 使用上述组合。多个条件可命中同一份文档——`matchedConditions`
按 `--until` 的请求顺序回显。没有基线或 poll-0 变更语义：第一次读取即可立即
命中。`--after-revision` 是调用方给出的全局修订栅栏——不是回合级或事件因果
身份。栅栏大于首个返回 revision 时，命令在恰好一次观察后以退出 3 拒绝
（`rejected`、`revisionAheadDetails`）；revision 单调，故该拒绝不可能在更晚
触发。任何一轮返回 `rejected{not-found}`——首轮或等待中——都是 `not-found`
（3）；`read-denied` 是 `denied`；`unavailable` 是 `service-unavailable`；
各自携带其应答轮的相关联 `agent-read` source。

两类不同的 deadline 都以退出 7 的 `timeout` 落在
`{stage:"agent-read", operation:"wait"}` 下。`wait-deadline` 是共享绝对预算
耗尽——步骤后结算优先于任何迟到的命中，预算之后到达的条件绝不发布。
`read-deadline` 是某一轮自身 `min(5 s, remaining)` 的界限在总量仍有余量时触发：
持有的 session 被该交换 retire 且绝不重新准入，details 记录实际准入的
`readBudgetMs`——文档不声称所求条件在整个等待期间缺席。timeout details 携带
配置的 `timeoutMs`、单调 `elapsedMs`、`completedObservations` 以及最后一次
完成观察的 `lastStatusKind`/`lastFreshness`/`lastRevision`——零次完成观察时
全部省略。armed span 内的取消同样是 `cancelled` 130 且无 source；准入失败
保持通用失败族。

现有 Linux 等待在每次完成的轮询处采样当前投影状态——它不是订阅，也不提供变更通知。
在两次已完成观察之间开始又结束的跃迁会被错过：待处理请求可能已清空，turn
outcome 可能被后来的 turn 替换，freshness 在轮询之间移动。错过窗口不只由
轮询间隔决定——Server 读取时长与调度同样会拉宽它——即便对一旦达成就保持
为真的条件，也不存在 sticky-state 保证。命中时发出的 `orchbee.agent.wait`
envelope（version 2）携带 `matchedConditions`、命中的 `revision`、完整的
`session` 明细逐字段，以及 `wait` 回显 `{until, afterRevision?, timeoutMs, observations, elapsedMs}`；其 `source` 仅为决定性轮询的 `agent-read`
requestId——失败、取消与不可信交换均不发布。text 输出为单行：
`session <id> matched <conditions> at revision <n> after <ms>ms`。

## Site 激活

`site start` 是 TUI 本地启动的无头形态：两者调用同一个共享 connect-or-activate 核
（`src/local_activation.rs`）——序列完全一致，仅失败投影不同。一个 20 秒绝对 deadline 在
cohort 验证之前开始，且绝不按阶段重置：

1. **Cohort** —— 先执行 `NativeCohort::current(deadline)`，先于任何运行时目录工作。其
   typed io kind 分类：`InvalidData` → `incompatible`，`NotFound` → `service-unavailable`，
   `PermissionDenied` → `denied`，`TimedOut` → `timeout`，其余一切 kind →
   `internal-failure`——均带 `{stage:"cohort"}`。
2. **Prepare** —— 解析 `ORCHBEE_RUNTIME_BASE`/默认值，推导 `orchbee-native-<euid>`，然后
   以 create-if-missing 模式执行 `prepare_mode_checked`：即使调用稍后被拒，运行时目录也可能
   已被创建。Rendezvous 错误与查询的 discovery 阶段完全同规则分类。
3. **Socket 预算** —— 每个派生 socket 路径必须适配本平台 `sockaddr_un`；超预算路径在任何
   取锁或准入尝试之前即 `service-unavailable`
   `{stage:"runtime-directory",reason:"address-too-long"}`。
4. **Connect or activate** —— 尝试一个已准入 `LocalClient`（socket 缺失或拒连在 deadline
   内可重试；其他 connect 失败逐字沿用查询的 `admission` 映射）；取激活锁，复查 socket，
   读标记，取 Site 生存期锁；当标记或 socket 显示无存活属主时拒绝
   `rejected`/`{stage:"activation",reason:"recovery-required"}`；提交标记，按已验证 cohort
   spawn `orchbee-server --local-site <runtime-directory>`，随后轮询 connect 加 `try_wait`
   直至就绪。spawn 或 `try_wait` 失败同样是 `recovery-required` 拒绝——标记可能已提交，故
   答案绝不是干净超时声明。
5. **Probe** —— 已准入的探针 `LocalClient` 在输出前释放；成功不意味持有 session、grant 或
   Site 身份收据。

`result.outcome` 报告本次调用实际走的分支：`started` 仅当本次调用拉起了 Server 子进程，
`already-running` 当既有 Site 准入了其探针。`timeout` 答案意为就绪未能确认——标记提交之前
无遗留；提交与 spawn 之间可能残留无存活属主的标记（后续调用随即应答
`recovery-required`）；spawn 之后 Server 可能仍在运行。该命令绝不声称超时证明没有 Server
启动，绝不重试 `recovery-required`，绝不修复或删除运行时状态。信号保持继承：不存在捕获
阶段，中断即进程死亡，按惯例 `128 + signo` 退出，绝不产出 `cancelled` envelope。下文说明 Windows 显式停止和后续启动；没有修复动词。

## Site 诊断

Windows `site diagnose` 输出独立闭合的 `orchbee.site.diagnose.windows` v2，保留
Linux v1 文档。原生目录为 v50/72。复用已有运行时路径／SID scope，不进行 cohort
准入、激活、端点连接或进程发现。有序行描述选定基目录、已准入运行时目录、标记、
激活／生命周期锁及五个命名管道地址。Windows 不输出文件系统 Host socket scope 行，
也不虚构 POSIX mode／UID。`private` 只表示已检查当前用户归属和运行时 DACL 白名单，
不是持续有效的安全保证。

版本 2 区分现有标记的 `activation: active|stopped`；Stopped 另带 `stopRequestId`，
不记作缺失。只有 Active 附加 activation-recorded 通告。这是保留完成记录的观察，
不是新的进程健康检查；后续启动会消费旧停止回执。

每行给出 known/absent/uninspectable，生命周期锁另有 free/held/unsafe。父目录被拒绝、
无效或缺失时，不检查所有后代。绝不获取激活锁；生命周期探针只打开已有且通过准入的
文件，短暂尝试独占锁，可能与启动争用。每个端点最多枚举 2048 个命名空间条目；
管道名称未经身份认证，预算耗尽为 `observation-limit`，不记作不存在。五秒协作式期限
可产生部分覆盖，文件系统停顿除外。`complete` 只表示每行已尝试或由父状态导出，
不表示健康。所有句柄由 RAII 持有，Ctrl+C 终止进程时由系统释放。不修复、创建／删除
文件、启动／停止进程或重试。标记和管道通告只描述当前恢复策略。Windows 公共进程
测试覆盖 absent/held/free、非法标记、中文路径、schema、本地化文本和零创建；
runtime／IPC 测试覆盖准入、真实监听器和有界观察。

`site diagnose` 是激活的只读对偶面：它报告隐式 per-euid 本地 Site 运行时的有界事实，
不 connect 任何东西、不创建任何东西、不修复任何东西。运行时基座与 `site start`
完全同规则解析（`ORCHBEE_RUNTIME_BASE` 或 `/tmp`，scope `orchbee-native-<euid>`），
随后命令经由 rendezvous 自身已验证的目录描述符检查一组有限具名路径——描述符相对的
`fstatat` 加 `AT_SYMLINK_NOFOLLOW`；仅对 Site 生存期锁追加一次"仅存性打开"探针，
其 `try_lock` 结果在同一描述符上立即释放。激活锁仅为元数据（存在性/类别/属主/模式/
链接数），命令绝不获取它。任何端点都不 connect——`client.sock` 尤其如此，一次
connect 就会消耗一个准入槽——也不枚举、发信号或散列任何进程。

每一行都是封闭状态的事实文档，绝非健康判定：`runtimeBase` 与 `runtimeDirectory`
携带 `path`/`pathBytesHex`；`marker`、`locks:{activation,siteLifetime}`、
`endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}` 与
`hostScope` 各自报告 `state`（`known|absent|unsafe|uninspectable`；生存期锁另有
`free` 与 `held`），以及不完全已知时的具体 `reason`——`symlink`、`invalid-kind`、
`foreign-owner`、`invalid-mode`、`invalid-size`、`invalid-link-count`、
`invalid-lock`、`denied`、`io-error`、`invalid-path`、`identity-changed`、
`unsafe-parent`、`parent-unavailable` 或 `deadline`。`Present` 行携带判定所依据的
原始事实（`kind`、`ownerMatch`、`mode`、`size`、`linkCount`）。`held` 生存期锁
仅意为某属主在该观察瞬间持有锁——绝不意为 Site、Worker 或任何进程存活——`absent`
是路径名事实，绝不证明执行已结束。Host 目录先获准入，然后才检查 Host 端点名；父级
不可准入时端点行不穿透遍历地关闭为 `uninspectable`/`unsafe-parent`。`complete` 是
"已尝试或已分类"的覆盖度事实——仅当协作式 5 秒预算在操作之间耗尽时为 `false`——
绝非运行时健康声明。

`result.advisories` 携带现行激活谓词自身事实所支持的条件性通告：观察到完全有效的
标记时 `activation-recorded`；观察到完全有效的 client 或 worker socket 叶时
`socket-residue`——文本中分别注释为 "activation recorded" 或 "socket residue
observed; if no existing Site can be joined, fresh activation is blocked by current
recovery policy"。该通告在已观察事实上引述现行策略；它绝不预测非原子未来
`site start` 的结果——后者在读任何残留之前先尝试认证接入。

命令以完整或部分事实文档退出 0——unsafe 与 uninspectable 行是事实而非命令失败。
调用、引擎与输出准备失败经正常失败面应答（`invalid-usage`、非 Linux 平台的
`service-unavailable` `{stage:"platform",reason:"unsupported-platform"}`、
`output-too-large`、`output-failed`）；中断即按惯例 `128 + signo` 的进程死亡
（SIGINT 为 `130`）。一次临时诊断性生存期锁探针可能与并发激活自身的取锁竞争——
这是调用方应预期的有界瞬时效应；命令记录该效应而非声称零副作用。它绝不获取激活锁、
绝不创建或删除路径、绝不修复、绝不制造存活或终止见证。

## Canonical 数据与结果

Canonical 遍历次序：`navigation.projects` 顺序；各 project 的 `workspaces` 顺序；
`terminalContexts` 顺序接 `tabs` 顺序；panels 深度优先且 `children[0]` 先于 `children[1]`；
terminal 绑定按该 panel 顺序且 `primaryPresentation` 先于 `childPresentations`。不新增排序；
过滤后的列表与 `ambiguous` 候选都保持该次序。

快照与单效果命令封套使用 `{schema, version, source, result}`；分阶段流程有自己的内嵌 schema。每份 schema 定义发出的版本及严格形状。增加 Panel 输入策略后，`orchbee.panel.list` 与 `orchbee.tab.get` 为 v2，`orchbee.panel.get/current` 为 v3；`orchbee.api.snapshot` 加入可选 Tab 自动命名后为 v4。`orchbee.terminal.get` 因启动与来源信息为 v3。`orchbee.terminal.read/history` 带 `--unwrap` 或 `--lines` 时为 v2，`orchbee.terminal.wait-output` 携带正则匹配细节时为 v2；三者不带这些 flag 时仍为 v1。各命令与失败的完整版本契约见 `api schema <id>`。目录为 v50，包含 72 份必需文档：注册 ID 会改变闭合字典形状，与业务文档版本独立。来源保留各所有者的含义：

- `source.kind="workbench-snapshot"`：带同一份已验证快照的 `siteId` 与十进制字符串
  `revision`——仅在完整有效读取之后出现。
- `source.kind="workbench-command"`：带 `siteId`、`requestId` 与十进制字符串
  `expectedRevision`——已产出的 mutation 结果，以及有效读取之后 command 阶段的失败。
- `source.kind="embedded"`：随二进制编译内嵌的 schema 字典（`api schema` 结果及其
  `not-found` 失败）。
- `source.kind="local-runtime"`：已产出的 `config check` 结果、`agent doctor` 结果、`site start` 的每个
  应答，以及每份 `site diagnose` 文档——本地运行时是激活 outcome、拒绝、离线诊断与
  有界运行时观察的应答权威。
- `source.kind="terminal.send"`：`siteId`、send `requestId`，以及解析出该绑定的十进制
  字符串 `snapshotRevision`——出现在类型化 `terminal send` 应答上。
- `source.kind="terminal-launch-read"`：`siteId` 与十进制字符串 `revision`——Server 在
  coordinator 锁内 join 出的修订——出现在 `terminal get` 应答上。
- `source.kind="worktree-list"`：`siteId` 与 Server 权威 membership 核验时刻已提交
  Workbench 的十进制字符串 `revision`——出现在 `worktree list` 结果及其快照后失败上。
- `source.kind="agent-read"`：语义应答带回的相关联 `requestId`——出现在已产出的
  agent 读取结果与准入后语义拒答上。
- `source.kind="launch-profile-catalog"`：`siteId` 与已解析目录的十进制字符串
  `catalogRevision`——出现在 `launch-profile list`/`get` 结果及其会话后失败上。
- `source.kind="workspace-close-resume"`：`siteId`、resume `requestId`、所指
  `closeRequestId`，以及请求携带的十进制字符串 `expectedAttemptGeneration` 栅栏——
  出现在 `workspace close-resume` 应答上：保留 lifecycle 的 resume 没有
  `workbench.command`，resume 请求自身的回显即诚实来源。
- 不存在 freshness 字段；一次一致读取不等于持续活性保证。

结果载荷（schema ID 为稳定机器名）：

- `orchbee.status` → `{site:{reachable:true,siteId,revision},
counts:{projects,workspaces,tabs,panels,boundTerminals},
worker:{status:"unknown",reason:"not-in-workbench-snapshot"}}`。`boundTerminals` 统计带
  `binding` 的 presentation；快照校验已保证 terminalId 全局唯一，故每个绑定恰命名一个不同
  终端。`reachable` 只证明本次读取成功。
- `orchbee.project.list` → `{projects:[{projectId,label,directoryCount,workspaceCount}]}`；
  `orchbee.project.get` → `{project:{projectId,label,directories:[{directoryId,workerId,path}],
workspaces:[{workspaceId,label}]}}`。
- `orchbee.workspace.list` → `{workspaces:[{workspaceId,label,projectId,
primaryDirectoryId|null,terminalContextId}]}`，跨 project 展平；`workspace get` 增加经
  `terminalContextId` 反查的 `tabs:[{tabId,label}]`。
- `orchbee.workspace.close-status` → 逐字的保留 `WorkbenchWorkspaceCloseReadOutcome`：
  无覆盖该 scope 的保留操作时为 `{kind:"not-closing"}`，否则
  `{kind:"available",closeRequestId,projectId,workspaceId,terminalContextId,
attemptGeneration,status,counts:{closing,cleanupBlocked,outcomeUnknown,
disposedAwaitingPublication},members:[{tabId,panelId,presentationId,status}…]}`。`status`
  是 scope lifecycle（`kind` 为 `closing`|`cleanup-blocked`|`outcome-unknown`|
  `disposed-awaiting-publication`|`closed`，后者带 tombstone `revision`），每个 member
  的 `status` 是捕获时的逐成员 lifecycle 事实、不含 `closed`（closed tombstone 仅属
  scope 级），`attemptGeneration` 是十进制字符串的尝试栅栏；Workspace 离开 active
  projection 后该记录仍可应答。
- `orchbee.tab.list` → `{tabs:[{tabId,label,pinned,groupId|null,
context:{kind:"standalone",terminalContextId}|{kind:"workspace",terminalContextId,
workspaceId}}]}`；standalone 上 `workspaceId` 键不存在而非 null。`tab get` 增加 `root`——
  typed `PanelNode` 子树的语义化重编码，不丢字段。
- `orchbee.panel.list` → `{panels:[{panelId,tabId,terminalContextId,primaryPresentationId,
presentationCount,primaryTerminalId|null}]}`，`primaryTerminalId` 仅反映
  `primaryPresentation.binding`；`panel get` → `{panel:{panelId,tabId,context,slotPath,
primaryPresentation,childPresentations}}`，`slotPath` 为自 root 至该叶子的 `PanelSlot` ID
  链（root 本身即 panel 时为 `[]`），每个 presentation 为 `{presentationId,label,
binding:{terminalId,workerId}|null}`。
- `orchbee.terminal.list` → `{terminals:[{terminalId,workerId,boundTo:{presentationId,panelId,
tabId,terminalContextId}}]}`；`terminal get` 在 `terminal` 下返回同一身份事实，其中嵌套一个
  `launch` 成员——文档形状为 `{terminal:{terminalId,workerId,boundTo,launch}}`，绝不出现兄弟
  `result.launch`。只有被 presentation 绑定的终端可见；绑定存在不证明进程存活。其余任何位置
  都没有 lifecycle/process/agent 字段。与 `terminal list` 不同，`terminal get` 不从 Client
  持有的快照作答：它在已准入 session 上发送一次 `terminal.launch.read` 请求，由 Server 在
  coordinator 锁内解析当前绑定、路由与 retained launch fact——快照只提供类型化目标 id。
  其 envelope 为 `version:3`，`source:{kind:"terminal-launch-read",siteId,revision}` 的
  `revision` 是 join 所得 workbench 修订的十进制字符串；旧的 `version:1` 校验器会拒绝它，
  这是刻意的兼容性边界。`launch` 为 `{state,workerGeneration,createOutcome,geometry}`：
  `state` 逐字呈现 retained phase——`creating`、`awaiting-start`、`starting`、`started`、
  `unknown`、`unavailable`——或在可见绑定没有可唯一归属且一致的记录时为 `not-tracked`；
  `workerGeneration` 为正的十进制字符串，仅在 `not-tracked` 时为 `null`；`geometry` 仅在
  `started` 时非空；`createOutcome` 照录 retained create outcome，在 `creating` 与
  `not-tracked` 时为 `null`。当启动命名了一个目录 profile 时，终端还多一个兄弟成员
  `launchProfile:{profileId,catalogRevision}`——逐字呈现的已解析 provenance，显式
  `--exec` 与默认 shell 启动下缺席（绝非 `null`）；envelope 为 `version:3`。state
  是 retained 事实，绝非进程存活：`creating` 只记录
  phase、不证明 dispatch 已发生；deferred-start 失败如实读作 `unavailable`，不虚构 start
  原因。退出码保持查询契约——`not-found` 为 3、`server-busy` unavailable 为
  4——`found` 应答即使 `launch.state` 为 `unavailable` 且 `createOutcome.kind` 为
  `failed` 仍退出 0，因为查询本身成功。该交换承载于已准入 session，与准入及快照读取共享
  同一份 5 秒网络预算：断开或耗尽的交换是 `transport-loss` 5，correlation 失配或无效结果是
  `protocol-invalid` 5——launch read 绝不报告 `timeout`。
- `orchbee.terminal.read` → `{terminal,frame,content}`：`terminal` 携带同样的
  `{terminalId,workerId,boundTo}` 身份事实；`frame` 携带已应用投影的
  `{revision,generation,sequence}`（十进制字符串，u64 无损）、`alternateScreen`、`geometry`
  `{columns,rows}` 与 `cursor` `{row,column,visible}`；`content` 携带 `rows`（发出的网格行，
  形如 `{index,text,continuation:{fromPrevious,intoNext}}`——`text` 拼接该行各 cell，
  丢弃宽字符占位尾并裁剪网格填充）、`rowCount`（已应用帧持有的全部行数）、
  `truncated`/`omittedRows` 与 `limits` `{rows:4096,textBytes:4194304}`。`--unwrap` 下
  `rows` 成员被 `lines` 替代——每条为 `{firstRow,lastRow,text,startsMidLine,endsMidLine}`
  的拼接逻辑行或带标记的窗口边缘片段——并附 `lineCount`；`--lines` 下附 `tail` 成员报告
  `{requested,omitted,entries:"rows"|"lines"}`。发出的行受累计 4 MiB
  JSON 编码文本预算约束；超出预算的行计入 `omittedRows` 且 `truncated` 为 true。text 输出下每
  个发出的行（或逻辑行）渲染为一行转义文本，`tail.omitted>0` 时追加本地化尾部标记，
  `omittedRows>0` 时末尾追加本地化截断标记。本次读取是单一
  时间点帧：`revision`/`generation`/`sequence` 是宿主在该观察时刻的计数器，不是 CLI 的
  freshness 声明。
- `orchbee.terminal.history` → `{terminal,history,content}`：`history` 携带已定价窗口的
  事实——`revision`/`generation`、`alternateScreen`、`columns`、`screenRows`、十进制字符串
  `scrollbackRows`/`totalRows`/`startRow`、`requestedRows` 与 `windowRows`（Host 窗口的完整
  行数）；`content` 携带 `rows`（`{index,text,continuation}`，`index` 为"保留区+屏幕"绝对行
  坐标的十进制字符串）、`rowCount`（发出的行数）、`truncated`/`omittedRows` 与 `limits`
  `{rows:1000,textBytes:4194304}`——并做与 `terminal read` 相同的 `--unwrap`/`--lines`
  替换（`lines` 的 `firstRow`/`lastRow` 为十进制字符串，附 `lineCount` 与 `tail`）。
  `omittedRows` 诚实计为 `min(requestedRows,scrollbackRows)` 减去已发出行数——即被 `MAXIMUM_FRAME_CELLS/columns` 上限或文本预算截掉的
  行——而早于 `startRow` 的行从未被请求，不算遗漏。text 输出与 `terminal read` 一致：每个
  发出的行渲染为一行转义文本加标记。
- `orchbee.terminal.send` → `{action:"terminal.send",requestId,siteId,workerId,terminalId,
payloadKind,records,recordBytes?,recordsAccepted,firstStoppedRecord,acceptedEncodedBytes,
inputStatus,cleanupStatus,reason?,target?}`. `payloadKind` 为 text、key、bytes、paste 或 sequence；
  `records` 为提交记录总数。`recordBytes` 仅表示单条内容记录的字节数（text/bytes 可含折入的 CR），
  不猜测后续语义按键的编码字节数。`recordsAccepted`、可空的零基 `firstStoppedRecord` 和
  `acceptedEncodedBytes` 原样报告已确认进度；回复丢失的失败 envelope 不伪造这些字段。
  `inputStatus` 包括 accepted、rejected、partial-accepted、not-submitted、unknown；
  `cleanupStatus` 独立报告清理结果。`reason` 为类型化拒绝/不可用原因。可选 `target` 是 Server
  实际准入的物理实例（`workerGeneration`、`hostEpoch`、`hostTerminalId`）。回执及诊断绝不回显
  提交内容。旧 `key` 和 `acceptedBytes` 字段不是当前序列回执的一部分。
- `orchbee.agent.list` → 逐字的 `listed` outcome：
  `{kind:"listed",items:[<session summary>…],nextCursor|null,revision}`，每个 summary
  即 wire `AgentSessionSummary`（session/provider/lifecycle/语义状态/freshness/观察
  计数/执行位置/pending 请求计数 ≤128/presentation 附件），`nextCursor` 为 Server
  铸造的续页 token，供 `--cursor` 使用。
- `orchbee.agent.get` → 逐字的 `found` outcome：
  `{kind:"found",revision,session}`，`session` 即 wire `AgentSessionDetail`——
  在 summary 字段之外增加 `capabilities`（≤10 个去重 token）、`lastOutcome` 与
  `pendingRequests`。
- `orchbee.agent.explain` → 逐字的 `found` outcome：
  `{kind:"found",revision,session,explanation}`，`explanation` 为 canonical 11 字段
  对象（`status`、`statusBasis`、`freshnessMeaning`、`capabilityCeiling`、
  `fidelity`、`freshness`、`generation`、`sequence`、`source`、`actionPath`、
  `contentAvailable`）——Server 的派生解读，绝非 CLI 推断。
- `orchbee.agent.wait` → 命中结果 `{matchedConditions,revision,session,wait}`；`matchedConditions` 是
  有效 `until` 的命中子集，按请求顺序返回（1–14 个唯一条件）。`revision` 与 `session`
  是当前 Get 样本的修订及原始 `AgentSessionDetail`。Windows v3 另带可空 `matchedChange`，
  将历史匹配证据与该样本分开；`wait` 回显有效的
  `{until,afterRevision?,timeoutMs,observations,elapsedMs}`，仅在提供时包含 `afterRevision`。
- `orchbee.api.snapshot` → `{snapshot}`：完整 `WorkbenchProtocolDocument::Snapshot` 的语义化
  重编码，含 `kind:"workbench.snapshot"`；envelope 的 `siteId`/`revision` 与内部文档一致。
- `orchbee.api.schema` → 带参 `{schema:{schemaId,document}}`；无参
  `{schemas:{schemaId:document,…}}` 列出全部已交付字典（draft 2020-12）。
  目录保持封闭：`result.schemas` 恰好命名已注册的 id，且每个内嵌文档的 `$id`
  与其键一致。按旧目录校验的 introspection 客户端从同一构建中选择内嵌 v2 文档。
- `orchbee.worktree.list` → `{projectId,directoryId,path,worktrees:[<entry>…]}`：已注册
  目录身份（`path` 为已注册 Workbench 目录——与每条目观察到的 `worktreePath` 相区分）
  加 `git worktree list --porcelain -z` 的逐字投影。每条目恰好携带 `worktreePath`
  （UTF-8）或 `worktreePathBytesHex`（小写十六进制原始字节）之一；`head` 为 40 或 64
  位小写十六进制对象名，bare 仓库为 `null`（全零 HEAD 是 unborn 仓库的真实 Git 值，
  原样保留）；`branch`（逐字完整 ref，如 `refs/heads/main`，绝不剥前缀）、
  `branchBytesHex` 或 `null`（无分支适用时）三者取一；外加 `bare`/`detached`/`locked`/
  `prunable` 标志。
- `orchbee.cli.failure.worktree` → `{schema:"orchbee.cli.failure.worktree",version:1,
failure:{kind,details?},source?}`：`worktree list` 越过快照边界后一切失败的专用
  envelope（边界前的语法、前导与 project 解析失败保持 `orchbee.cli.failure`）。其
  `details` 词汇封闭：`not-found` 为 `{resource:"directory",target}`；
  `denied`/`service-unavailable`（`server-busy`|`worker-unavailable`|`job-dead`|
  `job-unknown`）/`transport-loss`/`protocol-invalid`（`reason:"correlation-mismatch"`）
  为 `{stage:"worktree-list"[,reason]}`；`check-failed` 为 `{stage:"probe"|"list",status}`
  （`status` 为整数或 `null`）或 `{stage:"parse"}`；`timeout` 为 `{stage:"worktree-list", timeoutMs:5000}`；
  `output-too-large` 为 `{stage:"worktree-list",origin,limitBytes}`（`origin`
  `git-probe`|`git-list` → 65536，`worker-result` → 7936，`workbench-result` → 32768）。
- `orchbee.launch-profile.list` → `{catalogRevision,profiles:[<entry>…]}`：十进制字符串
  的目录私有 revision 加目录内全部条目（至多 64 条），按目录次序——
  `{profileId,name,launch:{exec,args}}`，全部逐字。
- `orchbee.launch-profile.get` → `{catalogRevision,profile:<entry>}`：Server 解析出的
  单个 profile，形状与列表条目相同。
- `orchbee.cli.failure.launch-profile` → `{schema:"orchbee.cli.failure.launch-profile",
version:1,failure:{kind,details?},source?}`：`launch-profile` 查询越过 session 边界后
  一切失败、以及 `launch-profile delete`、`tab create --profile`、`panel split
--profile` 上 profile 解析失败的专用 envelope（边界前的语法与前导失败保持
  `orchbee.cli.failure`）。其 `details` 词汇封闭：`not-found` 为
  `{resource:"launch-profile",target}`；`ambiguous` 为
  `{resource:"launch-profile",target,candidates≤8,totalCount}`（候选 ID 遵循
  profile-id 语法）；`denied`/`transport-loss` 为
  `{stage:"launch-profile-catalog"}`；`service-unavailable` 同形加
  `reason:"service-unavailable"`；correlation `protocol-invalid` 同形加
  `reason:"correlation-mismatch"`；`timeout` 为
  `{stage:"launch-profile-catalog",timeoutMs:5000}`。`source` 在读取失败时是
  `workbench-snapshot` 源、在 mutation 侧失败时是 `workbench-command` 源。
- `orchbee.mutation.result` → `{action,outcome,revision?,acceptedRevision?,closeRequestId?,
attemptGeneration?,<resolved typed IDs>,direction?,label?,terminal?,catalogRevision?,
profileId?,name?}`，
  按上文 mutation 矩阵：`committed`/
  `unchanged` 带 `revision`，`accepted` 带 `acceptedRevision`+`closeRequestId`，
  `resumed` 带 `closeRequestId`+`attemptGeneration`，
  `tab create`/`panel split` 带 `terminal:{outcome,reason?,geometry?}`。launch-profile
  动作在两份收据上都追加 `catalogRevision` 与 `profileId`，`launch-profile.create`
  另带 `name`；envelope 为 `version:4`。
- `orchbee.cli.failure.close` → `{schema:"orchbee.cli.failure.close",version:1,
failure:{kind,details?},source?}`：`workspace close-status` 与 `workspace close-resume`
  越过 admitted session 后一切失败的专用 envelope（边界前的语法、前导失败与
  `workspace close` 命令自身的失败保持 `orchbee.cli.failure`）。其 `details` 词汇
  封闭：`denied`/`transport-loss` 与裸 `protocol-invalid` 为
  `{stage:"close-status"|"close-resume"}`；`service-unavailable` 同形加
  `reason:"server-busy"`；`timeout` 为 `{stage:"close-status",timeoutMs:5000}`；
  已分类的 `protocol-invalid` 为 `{stage,reason}`、`reason` 取
  `correlation-mismatch`|`scope-mismatch`；`rejected` 为 `{stage:"close-resume",reason}`、
  `reason` 为类型化 resume 判定（`stale-generation`|`operation-unavailable`|
  `operation-mismatch`|`request-identity-conflict`|`generation-mismatch`|
  `not-resumable`|`generation-exhausted`）；`outcome-unknown` 为
  `{stage:"close-resume",requestId,reason}`。`source` 在 close-status 失败上是
  `workbench-snapshot` 源、在 resume 失败上是 `workspace-close-resume` 源。
- `orchbee.project.rename` → `{action:"project.rename",outcome,revision,projectId,label}`，
  `outcome` 取值 `committed|unchanged`：`project rename` 收据的专用 v1 文档，
  共享 `orchbee.mutation.result` 的 action 枚举保持封闭、不改动。
- `orchbee.config.check` → `{ok,checks:[{name,status,details}]}`，固定次序
  `runtime-base`、`runtime-directory`、`cohort`、`locale`、`sockets`；五项永远齐全。存在失败
  项时仍产出该成功 schema（`ok:false`）并以 1 退出。
- `orchbee.agent.doctor` → `{complete,providers:[{id,executable,runtime:{node},hook}],
endpoint:{socket},environment:{observationSocketEnv},asset:{roots:[<root>…]},hints:
[{id,evidence,text}]}`。`providers` 按规范次序报告每个所选 provider；每个事实行是封闭
  state/reason 词汇上的 `{state[,reason][,detail]}`——`executable`/`node` 报告 PATH 扫描
  事实（钩子状态需要 provider 自身命令面的条目带 `manualCommand`），`hook` 报告托管钩子
  检查结果（`present`/`absent`/`disabled`/`conflict`/`unknown`/`update-required`/
  `unsupported`），`socket`/`observationSocketEnv` 报告观察端点事实，每个 root 携带
  `origin`（`supplied`/`codex`/`cline`/`opencode`/`pi`/`omp`/`gemini`/`qwen-code`）、
  `state` 与恰居其一的 `path`（UTF-8）或 `pathBytesHex`（非 UTF-8）。`hints[].text` 是规范
  `en-US` 观察句——机器文档保持 locale 中立，文本输出会按 `id` 与 `evidence` 重新本地化
  同一句子。`complete:false` 标记受预算限制的部分文档；命令仍以 0 退出。
- `orchbee.site.start` → `{outcome,scope,runtimeBase}`：`outcome` 取值 `started` 或
  `already-running`（本次调用实际走的分支，而非 Site 身份收据），`scope` 为
  `orchbee-native-<euid>`，`runtimeBase` 恰携带 `path`（UTF-8）或 `pathBytesHex`
  （非 UTF-8，绝非有损显示串）之一。
- `orchbee.site.diagnose` → `{complete,scope,runtimeBase,runtimeDirectory,marker,locks,
endpoints,hostScope,advisories}`：Site 诊断一节描述的有界观察文档——封闭行状态与
  reason、`locks:{activation,siteLifetime}`、
  `endpoints:{client,worker,clientPending,agentObservation,hostEndpoint}`、两个目录行的
  `path`/`pathBytesHex`，以及条件性 `advisories` token 清单。
- `orchbee.cli.failure` → `{schema:"orchbee.cli.failure",version:1,failure:{kind,details?},
source?}`；不存在英文 `message` 字段。`source` 只取自已可信来源（读取后失败用已验证快照；
  `api schema` not-found 用 `embedded`；mutation command 阶段失败一律用
  `workbench-command`；`site start` 的每个应答含其拒绝一律用 `local-runtime`；`terminal get`
  在可信 join 之后的输出准备失败用 `terminal-launch-read`）；读取之前
  的失败与语法失败没有 source。
- `orchbee.cli.failure.source` → `{schema:"orchbee.cli.failure.source",version:1,
failure:{kind,details}}`——send 源阶段专用 envelope，仅 `timeout` 与 `cancelled`。
  `details` 为 `{stage:"input",inputSource:"file"|"stdin"}`，超时另带 `timeoutMs`；
  绝不含权威 `source`、请求身份、路径或载荷字节，因为该失败可证明先于任何运行时
  副作用。
- `orchbee.cli.failure.agent` → `{schema:"orchbee.cli.failure.agent",version:1,failure:
{kind,details?},source?}`——准入后 agent 读取失败的文档：`kind` ∈
  `{cancelled,denied,internal-failure,not-found,protocol-invalid,rejected,
service-unavailable,timeout,transport-loss}`，`details` 为按 kind 封闭的词汇
  （`{stage:"agent-read",operation[,reason|sessionId|timeoutMs]}`，其中
  `operation` ∈ `{explain,get,list,wait}`；`wait` 在 `rejected` 上增加
  `revisionAheadDetails` `{sessionId,afterRevision,revision}`、在 `timeout`
  上增加 `waitTimeoutDetails` `{timeoutMs:null|1-3600000,elapsedMs,reason:wait-deadline|read-deadline,readBudgetMs?, completedObservations,lastStatusKind?,lastFreshness?,lastRevision?}`，而三个单次读取动词保持
  `timeoutMs:5000`），`source` 仅出现在可信语义应答上。agent 命令的准入前失败
  仍使用通用 `orchbee.cli.failure` 文档。

### Config 检查

| 检查项              | 规则                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime-base`      | 解析并观测 `ORCHBEE_RUNTIME_BASE`/默认值：存在且为目录则 pass；路径非法、缺失、不安全项或 I/O 错误则 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `runtime-directory` | 基座失败、或合法基座下 scope 目录不存在时为 `not-applicable`（Site 未运行不是配置错误）；既有目录通过校验为 pass；属主/模式/类型不安全或 I/O 错误为 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| `cohort`            | `NativeCohort::current` 在其 5 秒预算内验证通过则 pass（`verified:true`）；缺失、非法、散列不符（`invalid-cohort`）或超时（`deadline`）为 fail。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| `locale`            | 信息性 pass，携带 `locale`（`en-US`/`zh-CN`）与 `source`（`LC_ALL`/`LC_MESSAGES`/`LANG`/`default`）。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| `sockets`           | `client`/`worker` 在运行目录可安全检查后报告 `presence` 为 `present`/`absent`/`unchecked`，沿用既有 类型+属主+`0600`+no-follow 规则。`clientPending`/`host` 是仅派生路径预算条目：`presence` 恒为 `unchecked`，地址可容纳时为 `not-applicable`/`presence-not-inspected`，超预算时为 `fail`/`address-too-long`；两者始终携带 `path`/`pathBytesHex` 与 `addressBytes`/`addressCapacityBytes`。任一条目 `fail` 则整体 fail；否则至少一个合规存在的 `client`/`worker` 为 pass；其余为 `not-applicable`。存在绝不宣称服务或 Worker 活性，预算可容纳也绝不宣称端点存在——此处的 `not-applicable` 指刻意未检查 presence，而非预算未知。派生 socket 路径的字节长度超出本平台 `sockaddr_un` 容量时，无论 presence 如何都以 `address-too-long` fail——该地址永远无法 bind 或 connect。 |

details 一律类型化：`runtime-base`/`runtime-directory` 携带 `path`（UTF-8）或 `pathBytesHex`
（非 UTF-8，绝不用替换字符）；`exists` 仅在路径状态已确立时出现——已确立的缺失为 `false`、
观测到条目为 `true`，检查本身失败（权限、路径或 I/O 错误）时该字段整体省略——另可加 `reason`
∈ `{invalid-path,absent,unsafe,io-error}`；`sockets` 的 reasons 为 `{unsafe,io-error,
runtime-unavailable,address-too-long,presence-not-inspected}`。`clientPending`/`host` 预算条目与
每个 `address-too-long` 条目都携带 `path`/`pathBytesHex`
以及 `addressBytes`（派生路径的编码字节长度）和 `addressCapacityBytes`（本平台 socket 地址
容量）。不作跨文件原子健康快照声明。

## Mutations

`terminal send` 修改的是 Host 输入状态而非 Workbench 文档：它走上文描述的已准入 session
上的 OBCS `terminal.send` 交换，不触碰 `expectedRevision`、Workbench 命令通道，或 TUI 的
controller/viewer/resize 所有权。

十五条命令经已 admitted 的 OBWP `workbench.command` 通道修改 Workbench 状态：
`project create`/`rename`、`workspace create`/`rename`/`accept-directory`/`close`、
`tab create`/`rename`/`move`/`close`、`panel split`/`close`/`move` 与
`launch-profile create`/`delete`。每次调用原样执行查询状态机的第 1–5 阶段——一次 cohort 检查、
一次 discovery 与 admission、一次有界快照读取——然后在同一 5 秒 deadline 下再执行两个阶段：

6. **Resolve** —— 把已验证快照纯函数式地投影为 wire action：每个 target 以查询 resolver
   完全相同的语义解析（可能 `not-found`/`ambiguous`，此时提交零条命令），快照提供 `siteId`
   与 `expectedRevision`，并生成一个全新的 `WorkbenchRequestId`。
7. **Command** —— 在已准入 session 上恰好提交一次 `workbench.command`。随后在任何结果
   映射与输出准备之前完全释放 Client Session。

`expectedRevision` 的选择是协议的乐观并发契约：Server 将命令的 base revision 与当前
revision 比对，不符即拒绝且不应用任何效果，故"读取一次并按该 revision 提交"就是被 admitted
的交互序列——CLI 绝不为追赶变动的 base 而重复读取，也绝不在无 base 的情况下提交。

安全契约：

- **至多一次 effect。** 每次调用恰好提交一条命令；全部参数与 target 校验在命令构造之前
  完成，故解析缺陷或未解析 target 不提交任何命令。
- **全新身份。** 每次调用生成一个全新的全局唯一 `WorkbenchRequestId`；receipt 与
  `outcome-unknown` 失败都会报告它以便操作员对账。
- **Server 持有 revision。** `expectedRevision` 永远来自解析出全部 target 的同一份已验证
  快照——绝不猜测、绝不由客户端自增、绝不复用。
- **绝不自动重放。** 提交后结果不可判定的情形——写丢失、响应丢失/畸形/correlation 不符、
  或答案未通过校验——会 retire 该 session 并以 `outcome-unknown`（退出 8）呈现。命令可能
  已应用也可能未应用；绝不重发。本地化诊断会明确说明这一点，并指示操作员先检查
  Workbench 状态（`status`、`tab list`、`panel list` 等）再决定是否重复该命令。
- **关闭确认。** `tab close`、`panel close` 与 `workspace close` 是破坏性操作；解析器
  要求显式 `--confirm` flag，故非交互调用绝不会默认执行关闭。`workspace close-resume`
  同样要求该 flag：为保留的 close 开启新的尝试窗口本身也是 effect。

结果：`committed` 报告新 `revision`（base + 1）；`unchanged`——未造成变化的 rename 或
move——报告当前 revision；`accepted`——各条 close——报告 `acceptedRevision` 及 Server 为
异步关闭给出的 `closeRequestId`；`resumed`——`workspace close-resume`——报告
`closeRequestId` 与 Server 开启的新 `attemptGeneration`。`tab create` 与 `panel split`
另携带 `terminal`：Server
仍欠 spawn 时为 `pending`，否则为类型化 spawn 结果（`started` 带回显的 `geometry`，
`unavailable`/`failed` 带 reason，`unknown`）。

`project create` 是唯一无 target 操作数、无文件系统副作用的修改命令：动作只携带 label，
由 Server 分配新 `projectId` 并在 `committed` 收据中返回，新建 Project 无目录、无 Workspace。
label 允许重复——label 无法唯一标识新 Project；收据中的 `projectId` 可以。

`workspace accept-directory` 提交既有 `WorkspacePrimaryDirectoryAccept` 动作——与 TUI
目录向导所用操作相同——因此 `project create → workspace create → workspace
accept-directory → tab create` 整条序列无需 TUI。target 按普通 Workspace 规则解析（先
按 typed ID、再按精确 label；歧义 label 拒绝），真正的 `projectId` 取自与已解析
Workspace 相同的快照行。`--path` 在 CLI 边界解析为 `WorkbenchDirectoryPath`（字面绝对
路径，≤4,096 UTF-8 字节，不含 NUL；`~`/`$` 保持字面）——CLI 从不创建、修复、规范化
或探测它，故不存在或非目录路径仍正常提交，经 Server 的 `DirectoryUnavailable`
precommit 呈现为 `service-unavailable`（exit 4）。Server 对导航修改、目录关联、
worker 探测与 generation、乐观修订、资源上限保持权威：已持有目录的 Workspace——包括
相同路径——被拒绝（`workspace-directory-conflict`，exit 3），CLI 既不预检
`primary_directory_id` 也不推断幂等成功。`committed` 收据携带新 `revision`、Server
分配的 `directoryId` 与 `workerId`、已解析的 `projectId` 与 `workspaceId`，以及请求的
字面 `path`（永不为规范化形式）。

几何在请求中永远显式：非 TTY 调用无法继承终端尺寸，故 `tab create` 与 `panel split`
把四个 flag 作为全有或全无组接受，整组省略时请求默认的 80x24 网格与成对零（未知）像素
对——这是物化进 wire 几何的 CLI 请求策略，而非实测显示事实。`--cols`/`--rows` 为格数，
`--cell-width`/`--cell-height` 为每格像素尺寸；每个维度都是 `u16`，并在任何运行时访问
之前经协议 `WorkbenchTerminalGeometry` 校验。

`tab create` 与 `panel split` 的启动经三种选择之一：解析出的默认 shell、显式
`--exec`/`--arg` 对、或 `--profile <selector>`。selector 在命令中逐字透传——Server 在
命令时刻的目录锁下解析（先精确 ID，再按恰一匹配的名称），解析出的 retained 启动成为
终端的 provenance。未命中的 selector 是零变更的已提交 outcome，CLI 映射为 `not-found`；
多匹配的名称映射为 `ambiguous`——两者均走 launch-profile 失败 envelope，带该命令的
`workbench-command` 源。

`launch-profile create` 与 `launch-profile delete` 经同一"一条命令"envelope 修改目录
本身。`create` 携带 `--name` 与显式 flag 构建的同一份已校验 `LaunchSelection`，committed
收据报告 `revision`（Workbench revision）、`catalogRevision`（目录私有计数）、铸造的
`profileId` 与逐字的 `name`。`delete` 携带未解析的 selector——Server 在目录锁下应用
"先 ID 后名称基数"规则；committed 收据报告 `revision`、`catalogRevision` 与被删除的
`profileId`。已 settle 的 `ProfileUnavailable` outcome 映射为
`not-found{resource:"launch-profile"}`，`ProfileAmbiguous` 映射为携带受限候选 ID 的
`ambiguous`——均走
launch-profile 失败 envelope；目录容量拒绝走通用 envelope 下的普通 command 阶段拒绝。

Mutation 结果使用 `schema:"orchbee.mutation.result"`、`version:4`，
`source:{kind:"workbench-command",siteId,requestId,expectedRevision}`，`result` 携带 `action`
（点分命令名）、`outcome`、该结果的 revision 字段、已解析 typed ID，以及命令具备时的
`direction`/`label`/`terminal`/`catalogRevision`/`profileId`/`name`。`workspace
close-resume` 是唯一的来源例外：保留 lifecycle 的 resume 没有 `workbench.command`，故其
`resumed` 收据与提交后失败报告 `source.kind="workspace-close-resume"`——resume 请求自身的
回显，带 `siteId`、`requestId`、`closeRequestId` 与 `expectedAttemptGeneration`。
文本输出按与查询相同的转义规则逐行渲染同样字段。
`project rename` 是唯一例外：其收据使用专用的 `schema:"orchbee.project.rename"`、
`version:1` 文档——`source` 仍为同一 `workbench-command` 形态，`result` 封闭于
`action:"project.rename"`、`outcome`（`committed`/`unchanged`）、`revision`、
`projectId` 与 `label`。

`workspace close` 提交 `WorkspaceClose` 动作——由 Server 权威地关闭该 Workspace 的
Client 使用的全部终端，在整个 context scope 上复用与 `tab close`/`panel close` 相同的
terminal-tab-close lifecycle 机械。终端清理完成后，Workspace 及其 terminal context
从 active projection 移除；checkout、branch 与磁盘目录保留。`accepted` 收据携带 `acceptedRevision`、该
操作的 `closeRequestId` 与已解析的 `projectId`/`workspaceId`/`terminalContextId`；Site
异步完成关闭，保留的操作记录比 Workspace 的 active projection 活得更久。重复运行
`workspace close` 绝不是 resume——可恢复的保留操作只能经 `workspace close-resume` 继续。

`workspace close-status` 仅凭严格稳定 ID 读取该保留记录：它共享查询状态机的 session
准入但绝不在本地解析 Workspace membership——记录比 projection 行活得更久，故已完成的
close 仍以 `available` 连同其 `closed` tombstone `revision` 作答，而从未发生 close 的
scope 以 `not-closing` 作答。`workspace close-resume` 是保留 lifecycle 请求而非
`workbench.command`：它执行同样的准入纪律并恰好提交一次 resume 交换，携带该 close 操作的
`closeRequestId` 与调用方最后观察到的 `expectedAttemptGeneration` 栅栏。只有可恢复的
lifecycle 状态可以 resume——未知结果不可恢复——陈旧栅栏以类型化 `stale` 应答落定，在
close 失败 envelope 下报告为 `rejected`、`reason:"stale-generation"`，绝不产生第二次尝试。

明确不交付：`project close`、`panel resize`、终端 open 操作、Site restart、disable、recovery 或
修复命令（Windows 已支持 stop/status），以及一切远端操作。

## 失败矩阵

退出码：0 成功（含空表）；1 本地配置/输出/内部确定失败；2 usage/非法 target；3
not-found/ambiguous/denied/rejected；4 service-unavailable（含 `session-capacity`）；5
transport-loss/protocol-invalid；6 incompatible；7 `timeout`（`terminal wait-output`
期限耗尽未命中，或 send 源物化期间共享 deadline 到期）；8 outcome-unknown；130
`cancelled`（被处理的 SIGINT 类中断——`terminal wait-output` 期间收到
HUP/INT/QUIT/TERM，或 `agent doctor` 探测项之间捕获合格信号；更一般地，被处理的
`terminal send` 源阶段信号按惯例
`128 + signo` 退出：合格集对应 129/130/131/143）。`agent` 读取期间被捕获的
SIGINT 或 SIGTERM 结算为类型化 `cancelled` agent 失败、状态 130——按契约两个
成员相同——未被 watch 的 SIGHUP/SIGQUIT 保留其继承 disposition。源 guard 未捕获的合格信号按原
disposition 交付——OS 默认终止遵循 shell 的 `128 + signal` 约定，不是 CLI 合成的结果。

| 来源                                                                                                                                | `failure.kind`                                                             | 退出      | `details`                                                                                     |
| ----------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- | --------- | --------------------------------------------------------------------------------------------- |
| argv 封装/语法/format/target 形状错误                                                                                               | `invalid-usage`                                                            | 2         | 省略                                                                                          |
| 全零 ID/空/超长 label、非法 panel/terminal ID                                                                                       | `invalid-identifier`                                                       | 2         | `{resource,target}`                                                                           |
| 合法 target 或 schema ID 无匹配                                                                                                     | `not-found`                                                                | 3         | `{resource,target}`                                                                           |
| 多个精确 label 匹配                                                                                                                 | `ambiguous`                                                                | 3         | `{resource,target,candidates≤8,totalCount}`                                                   |
| `NativeCohort::current` 出错                                                                                                        | `check-failed`                                                             | 1         | `{stage:"cohort"}`                                                                            |
| 基座非法/缺失/非目录、不安全 runtime/socket、其他校验                                                                               | `check-failed`                                                             | 1         | `{stage:"runtime-base"                             \| "runtime-directory"   \| "socket"}`     |
| 合法基座下 scope 目录或 `client.sock` 不存在                                                                                        | `service-unavailable`                                                      | 4         | `{stage:"discovery"}`                                                                         |
| `ConnectClientError::ConnectFailed`                                                                                                 | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                         |
| `ConnectClientError::Io`                                                                                                            | `transport-loss`                                                           | 5         | `{stage:"admission"}`                                                                         |
| OBLC `Rejected(CapacityExceeded)`                                                                                                   | `session-capacity`                                                         | 4         | `{stage:"admission"}`                                                                         |
| OBLC `Rejected(HandshakeTimeout)`                                                                                                   | `service-unavailable`                                                      | 4         | `{stage:"admission"}`                                                                         |
| `Rejected(IncompatibleProtocol                                    \| IncompatibleCohort)`、`Protocol(IncompatibleVersion)`          | `incompatible`                                                             | 6         | `{stage:"admission"}`                                                                         |
| `Rejected(WrongMessage                                            \| MalformedFrame)`、`UnexpectedResponse`、其余 `Protocol`        | `protocol-invalid`                                                         | 5         | `{stage:"admission"}`                                                                         |
| `RequestIoConfiguration`、`SessionUnavailable`、`RequestEncoding`                                                                   | `internal-failure`                                                         | 1         | 省略                                                                                          |
| `SnapshotReadError::RequestNotSubmitted`（任何原因）                                                                                | `transport-loss`                                                           | 5         | `{stage:"snapshot"}`                                                                          |
| `Failed(Write                                                     \| ResponseDeadline)`、`Failed(Response(Io))`                     | `transport-loss`                                                           | 5         | `{stage:"snapshot"}`                                                                          |
| `Failed(Response(InvalidHeader                                    \| Contract))`、`UnexpectedResponse`                              | `protocol-invalid`                                                         | 5         | `{stage:"snapshot"}`                                                                          |
| `Failed(CorrelationMismatch)`                                                                                                       | `protocol-invalid`                                                         | 5         | `{stage:"snapshot",reason:"correlation-mismatch"}`                                            |
| outcome `Denied`                                                                                                                    | `denied`                                                                   | 3         | 省略                                                                                          |
| outcome `Unavailable`                                                                                                               | `service-unavailable`                                                      | 4         | `{stage:"snapshot"}`                                                                          |
| viewer `RequestNotSubmitted`、`OutcomeUnknown(Write\|Read)`                                                                         | `transport-loss`                                                           | 5         | `{stage:"viewer"}`                                                                            |
| viewer `OutcomeUnknown(InvalidResult\|UnexpectedResult)`、错误 outcome 类别                                                         | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                            |
| viewer `OutcomeUnknown(CorrelationMismatch)`                                                                                        | `protocol-invalid`                                                         | 5         | `{stage:"viewer",reason:"correlation-mismatch"}`                                              |
| viewer outcome `Rejected(AlreadyAttached\|TerminalUnavailable)` 或 `Unavailable`                                                    | `service-unavailable`                                                      | 4         | `{stage:"viewer"}`                                                                            |
| viewer outcome `Rejected(FeedbackMismatch)`、record 被投影拒绝                                                                      | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                            |
| `ReadRows` outcome `RowsRejected(LimitExceeded)`                                                                                    | `rejected`                                                                 | 3         | `{stage:"viewer",operation:"read-history",reason:"limit-exceeded"}`                           |
| `ReadRows` outcome `RowsRejected`（其他可产生 reason）                                                                              | `service-unavailable`                                                      | 4         | `{stage:"viewer",operation:"read-history",reason}`                                            |
| `ReadRows` outcome `RowsRejected(InvalidField\|InvalidAcknowledgement)`、行窗口 record 失配/畸形                                    | `protocol-invalid`                                                         | 5         | `{stage:"viewer"}`                                                                            |
| launch-read `RequestNotSubmitted`、`OutcomeUnknown(Write\|Read)`（含共享网络预算耗尽）                                              | `transport-loss`                                                           | 5         | `{stage:"launch-read"}`                                                                       |
| launch-read `OutcomeUnknown(InvalidResult\|UnexpectedResult)`、错误 outcome 类别                                                    | `protocol-invalid`                                                         | 5         | `{stage:"launch-read"}`                                                                       |
| launch-read `OutcomeUnknown(CorrelationMismatch)`                                                                                   | `protocol-invalid`                                                         | 5         | `{stage:"launch-read",reason:"correlation-mismatch"}`                                         |
| launch-read outcome `Unavailable(ServerBusy)`                                                                                       | `service-unavailable`                                                      | 4         | `{stage:"launch-read"}`                                                                       |
| `DefinitelyNotSubmitted::RequestNotSubmitted`（command）                                                                            | `transport-loss`                                                           | 5         | `{stage:"command"}`                                                                           |
| `DefinitelyNotSubmitted::SessionUnavailable`/`RequestEncoding`                                                                      | `internal-failure`                                                         | 1         | 省略                                                                                          |
| command `Rejected` precommit                                                                                                        | `rejected`                                                                 | 3         | `{stage:"command",reason,revision}`                                                           |
| command `Denied` precommit                                                                                                          | `denied`                                                                   | 3         | 省略                                                                                          |
| command `Unavailable` precommit                                                                                                     | `service-unavailable`                                                      | 4         | `{stage:"command",reason}`                                                                    |
| `OutcomeUnknown`（任意原因）                                                                                                        | `outcome-unknown`                                                          | 8         | `{stage:"command",requestId,reason}`                                                          |
| close-status `RequestNotSubmitted`、写/io 响应失败                                                                                  | `transport-loss`                                                           | 5         | `{stage:"close-status"}`，属 `orchbee.cli.failure.close`                                      |
| close-status 非法/意外响应或 scope 不符的应答                                                                                       | `protocol-invalid`                                                         | 5         | `{stage:"close-status"[,reason:"correlation-mismatch"\|"scope-mismatch"]}`，属 close envelope |
| close-status 响应 deadline 到期，或共享 5 秒命令 deadline 到期                                                                      | `timeout`                                                                  | 7         | `{stage:"close-status",timeoutMs:5000}`，属 close envelope                                    |
| close-status outcome `Denied`                                                                                                       | `denied`                                                                   | 3         | `{stage:"close-status"}`，属 close envelope                                                   |
| close-status outcome `Unavailable`                                                                                                  | `service-unavailable`                                                      | 4         | `{stage:"close-status",reason:"server-busy"}`，属 close envelope                              |
| close-resume `RequestNotSubmitted`、写/io 响应失败                                                                                  | `transport-loss`                                                           | 5         | `{stage:"close-resume"}`，属 close envelope                                                   |
| close-resume 非法/意外响应                                                                                                          | `protocol-invalid`                                                         | 5         | `{stage:"close-resume"}`，属 close envelope                                                   |
| close-resume outcome `Denied`                                                                                                       | `denied`                                                                   | 3         | `{stage:"close-resume"}`，属 close envelope                                                   |
| close-resume outcome `Unavailable`                                                                                                  | `service-unavailable`                                                      | 4         | `{stage:"close-resume",reason:"server-busy"}`，属 close envelope                              |
| close-resume outcome `Stale`                                                                                                        | `rejected`                                                                 | 3         | `{stage:"close-resume",reason:"stale-generation",attemptGeneration}`，属 close envelope       |
| close-resume outcome `Rejected`（类型化 reason）                                                                                    | `rejected`                                                                 | 3         | `{stage:"close-resume",reason}`，属 close envelope                                            |
| close-resume `OutcomeUnknown`（任意原因）                                                                                           | `outcome-unknown`                                                          | 8         | `{stage:"close-resume",requestId,reason}`，属 close envelope                                  |
| worktree-list `RequestNotSubmitted`、写/io 响应失败、或 `TransportLoss` outcome（投递后响应丢失）                                   | `transport-loss`                                                           | 5         | `{stage:"worktree-list"}`，属 `orchbee.cli.failure.worktree`                                  |
| worktree-list 非法/意外响应                                                                                                         | `protocol-invalid`                                                         | 5         | `{stage:"worktree-list"[,reason:"correlation-mismatch"]}`，属 worktree envelope               |
| worktree-list 响应 deadline 到期，或共享 5 秒命令 deadline 到期                                                                     | `timeout`                                                                  | 7         | `{stage:"worktree-list",timeoutMs:5000}`，属 worktree envelope                                |
| worktree-list outcome `NotFound`（Server 核验时目录不在 project 内）                                                                | `not-found`                                                                | 3         | `{resource:"directory",target}`，属 worktree envelope                                         |
| worktree-list outcome `Denied`                                                                                                      | `denied`                                                                   | 3         | `{stage:"worktree-list"}`，属 worktree envelope                                               |
| worktree-list `Unavailable`/`SpawnUnavailable`/`SpawnFailed`/`OwnerIo`                                                              | `service-unavailable`                                                      | 4         | `{stage:"worktree-list",reason:"server-busy"                                                  | "worker-unavailable"                                     | "job-dead" | "job-unknown"}` |
| worktree-list `GitFailed`（probe/list 带精确退出 status，被信号终止时为 `null`）或 `MalformedListing`                               | `check-failed`                                                             | 1         | `{stage:"probe"                                                                               | "list",status}`或`{stage:"parse"}`，属 worktree envelope |
| worktree-list `TooLarge`（共享 Git stdout、OBCN 结果或 Workbench 结果预算被越过）                                                   | `output-too-large`                                                         | 1         | `{stage:"worktree-list",origin,limitBytes}`，属 worktree envelope                             |
| launch-profile-catalog `RequestNotSubmitted`、写/io 响应失败                                                                        | `transport-loss`                                                           | 5         | `{stage:"launch-profile-catalog"}`，属 `orchbee.cli.failure.launch-profile`                   |
| launch-profile-catalog 非法/意外响应                                                                                                | `protocol-invalid`                                                         | 5         | `{stage:"launch-profile-catalog"[,reason:"correlation-mismatch"]}`，属 profile envelope       |
| launch-profile-catalog 响应 deadline 到期                                                                                           | `timeout`                                                                  | 7         | `{stage:"launch-profile-catalog",timeoutMs:5000}`，属 profile envelope                        |
| launch-profile-catalog outcome `NotFound`                                                                                           | `not-found`                                                                | 3         | `{resource:"launch-profile",target}`，属 profile envelope                                     |
| launch-profile-catalog outcome `Ambiguous`                                                                                          | `ambiguous`                                                                | 3         | `{resource:"launch-profile",target,candidates≤8,totalCount}`，属 profile envelope             |
| launch-profile-catalog outcome `Denied`                                                                                             | `denied`                                                                   | 3         | `{stage:"launch-profile-catalog"}`，属 profile envelope                                       |
| launch-profile-catalog outcome `Unavailable`                                                                                        | `service-unavailable`                                                      | 4         | `{stage:"launch-profile-catalog",reason:"service-unavailable"}`，属 profile envelope          |
| launch-profile mutation `ProfileUnavailable`（`delete`、`tab create --profile`、`panel split --profile` 上零变更已 settle outcome） | `not-found`                                                                | 3         | `{resource:"launch-profile",target}`，属 profile envelope                                     |
| launch-profile mutation `ProfileAmbiguous`（零变更已 settle outcome，同批命令）                                                     | `ambiguous`                                                                | 3         | `{resource:"launch-profile",target,candidates≤8,totalCount}`，属 profile envelope             |
| send `RequestNotSubmitted`（请求可证明从未到达线缆）                                                                                | `transport-loss`                                                           | 5         | `{stage:"send"}`                                                                              |
| send `SessionUnavailable`/`RequestEncoding`                                                                                         | `internal-failure`                                                         | 1         | 省略                                                                                          |
| send `OutcomeUnknown`（写/读/无效结果/correlation）                                                                                 | `outcome-unknown`                                                          | 8         | `{stage:"send",requestId,reason}`                                                             |
| `terminal.send.result` `Rejected`/`Unavailable`                                                                                     | 类型化应答，非失败 envelope                                                | 3/4       | send 结果 `reason`、`cleanupStatus`；`host-protocol-fault` 退出 5                             |
| `terminal.send.result` `Accepted` + `Unconfirmed` cleanup                                                                           | 类型化应答，非失败 envelope                                                | 5         | send 结果 `recordsAccepted`、`acceptedEncodedBytes`、`cleanupStatus`                          |
| agent `RequestNotSubmitted`/`Write`/`ResponseDeadline`/`Response(Io)`（deadline 未耗尽）                                            | `transport-loss`                                                           | 5         | `{stage:"agent-read",operation}`                                                              |
| agent 交换或准入后步骤内共享总 deadline 耗尽                                                                                        | `timeout`                                                                  | 7         | `{stage:"agent-read",operation,timeoutMs:5000}`                                               |
| agent armed span 捕获合格 SIGINT 或 SIGTERM（任意阶段；HUP/QUIT 保持原始 disposition）                                              | `cancelled`                                                                | 130       | `{stage:"agent-read",operation}`                                                              |
| agent `Response(wire fault)`/`UnexpectedResponse`/`CorrelationMismatch`、错误结果 kind                                              | `protocol-invalid`                                                         | 5         | `{stage:"agent-read",operation}`                                                              |
| agent outcome `Rejected(NotFound)`（get/explain）                                                                                   | `not-found`                                                                | 3         | `{stage:"agent-read",operation,sessionId}`                                                    |
| agent outcome `Rejected(ReadDenied)`                                                                                                | `denied`                                                                   | 3         | `{stage:"agent-read",operation,reason:"read-denied"}`                                         |
| agent list outcome `Rejected(CursorInvalid\|CursorStale)`                                                                           | `rejected`                                                                 | 3         | `{stage:"agent-read",operation:"list",reason:"cursor-invalid"\|"cursor-stale"}`               |
| agent outcome `Unavailable`                                                                                                         | `service-unavailable`                                                      | 4         | `{stage:"agent-read",operation,reason:"runtime-unavailable"}`                                 |
| agent `SessionUnavailable`/`RequestEncoding`                                                                                        | `internal-failure`                                                         | 1         | 省略                                                                                          |
| send 源校验：打不开/非 regular 路径、终端或不支持的 stdin、空、超限、非 UTF-8                                                       | `invalid-usage`                                                            | 2         | 省略（本地化 stderr 指明所声明选项与具体原因）                                                |
| send 源物化期间共享 deadline 到期                                                                                                   | `timeout`，属 `orchbee.cli.failure.source`                                 | 7         | `{stage:"input",inputSource,timeoutMs}`                                                       |
| send 源物化期间捕获合格信号                                                                                                         | `cancelled`，属 `orchbee.cli.failure.source`                               | 128+signo | `{stage:"input",inputSource}`                                                                 |
| send 源信号 setup/读取/恢复失败                                                                                                     | `internal-failure`                                                         | 1         | 省略                                                                                          |
| 预备输出超 8 MiB                                                                                                                    | `output-too-large`                                                         | 1         | `{limitBytes:8388608}`                                                                        |
| stdout 写/flush 失败                                                                                                                | `output-failed`                                                            | 1         | 不追加第二份 stdout 文档                                                                      |
| `config check` 完成但存在失败项                                                                                                     | 成功 schema `ok:false`                                                     | 1         | 五项全部保留                                                                                  |
| `site start` cohort `InvalidData`/`NotFound`/`PermissionDenied`/`TimedOut`/其他 io kind                                             | `incompatible`/`service-unavailable`/`denied`/`timeout`/`internal-failure` | 6/4/3/7/1 | `{stage:"cohort"}`                                                                            |
| `site start` socket 路径超 `sockaddr_un` 预算                                                                                       | `service-unavailable`                                                      | 4         | `{stage:"runtime-directory",reason:"address-too-long"}`                                       |
| `site start` connect/admission 失败                                                                                                 | 同上方 `ConnectClientError` 各行，逐字一致                                 | 4/5/6     | `{stage:"admission"}`                                                                         |
| `site start` 标记或 socket 已提交但无存活属主；Server spawn 或 `try_wait` io 失败                                                   | `rejected`                                                                 | 3         | `{stage:"activation",reason:"recovery-required"}`                                             |
| `site start` 激活 deadline 到期；取锁 `TimedOut`                                                                                    | `timeout`                                                                  | 7         | `{stage:"activation"}`                                                                        |
| `site start` 标记/探针/锁 io 失败（非超时）                                                                                         | `internal-failure`                                                         | 1         | `{stage:"activation"}`                                                                        |
| `site diagnose` 完整或部分事实文档——unsafe/uninspectable 行是事实而非失败                                                           | 类型化 `orchbee.site.diagnose` 文档                                        | 0         | 行 `state`/`reason` 字段                                                                      |
| `site diagnose` 在 Linux/Windows 以外；网络查询、mutation、`site start`、`config check` 或 `agent doctor` 在 Linux/Windows 以外     | `service-unavailable`/`check-failed`                                       | 4/1       | `{stage:"platform",reason:"unsupported-platform"[,command]}`                                  |
| `agent doctor` 在探测项之间捕获合格信号                                                                                             | `cancelled`                                                                | 130       | `{stage:"agent-doctor"}`——部分文档仅由预算耗尽产生                                            |
| `terminal wait-output` 期限耗尽未命中                                                                                               | `timeout`                                                                  | 7         | `{stage:"wait",timeoutMs,observed}`                                                           |
| `terminal wait-output` 期间收到 HUP/INT/QUIT/TERM                                                                                   | `cancelled`                                                                | 130       | `{stage:"wait",cleanup}`                                                                      |
| `terminal wait-output` 命中但 `Detach` 未确认                                                                                       | 类型化 viewer-stage 失败                                                   | *         | `{stage:"viewer",matchObserved:true,cleanup:"unconfirmed"}`                                   |

`resource` ∈ `project|workspace|tab|panel|terminal|schema`；`stage` ∈
`cohort|runtime-base|runtime-directory|socket|discovery|admission|activation|snapshot|viewer|command|send|agent-read|agent-doctor|platform|wait`。
专用 `orchbee.cli.failure.worktree` envelope 接纳 `resource:"directory"` 与上文随其
schema 记录的 `worktree-list|probe|list|parse` stage 词汇——这些值仅属该 envelope，
不进入通用 `orchbee.cli.failure` 词汇。
专用 `orchbee.cli.failure.close` envelope 接纳上文随其 schema 记录的
`close-status|close-resume` stage 词汇——这些值同样仅属该 envelope。
源阶段的 `input` stage 只存在于 `orchbee.cli.failure.source` envelope——不进入标准
`orchbee.cli.failure` 的 stage 词汇。
stage 与 reason 是有限 schema 值；任何异常文本都不进入机器字段。`details` 是按 kind 封闭的词汇——`{resource,target}`、
`{resource,target,candidates≤8,totalCount}`、`{stage[,reason]}`、
`{stage:"command",reason,revision}`、`{stage:"command",requestId,reason}`、
`{stage:"send",requestId,reason}`、
`{stage:"viewer",operation:"read-history"|"wait-output",reason}`、
`{stage:"wait",timeoutMs,observed}`、`{stage:"wait",cleanup}`、
`{stage:"viewer",matchObserved,cleanup}`、`{limitBytes}`、
`{stage:"platform",reason,command?}`（`command` 成员命名拒绝的检查命令，
`"agent doctor"`），或 agent-read 族——
`{stage:"agent-read",operation}`、`{stage:"agent-read",operation,sessionId}`、
`{stage:"agent-read",operation,reason}`、`{stage:"agent-read",operation,timeoutMs}`——
内嵌 schema 拒绝超出这些形状的文档。读路径错误永不归类为
outcome-unknown/failed-effect：只有 command 阶段可能产生不可判定效果；
`rejected`/`denied`/`unavailable` precommit 答案都是确定的——Server 未应用任何效果。
send 阶段对其单条输入记录遵循同一规则：`RequestNotSubmitted` 是确定的未提交，
`OutcomeUnknown` 表示记录可能已经落地，而类型化 `terminal.send.result` 应答是 Server
自己的判定，绝非客户端猜测。声明型 `--file`/`--stdin` 源更严格：其一切失败都发生在
任何运行时访问之前，故绝不可能产生 `outcome-unknown`；被捕获信号的取消或共享
deadline 到期在 wire 请求尚不存在时即退出。

## 输出契约与副作用

- stdout 只承载契约文档：`--format json` 下为一份带 2 空格缩进、以 `\n` 结尾的 JSON（`api`
  命令恒为 JSON），text/默认下为本地化人类文档。stderr 承载本地化人类诊断；查询路径不打印
  进度。
- `invalid-usage` 失败在 stderr 写一条本地化诊断，指明单一具体原因——未知命令或选项、
  该命令不接受的选项或参数、重复选项、冲突选项、缺少选项值、缺少必需选项或操作数、缺少
  命令、未知帮助路径、错位的 help/version 标志、非法选项值或几何参数组合、非 Unicode 或
  超限参数表——并以帮助提示结尾。提示对可识别的命令或族上下文给出 `orchbee <path>
--help`，否则回落到 `orchbee --help`；每条发出的提示都是无需 Site 即可退出 0 的真实
  路径。回显的不可信 token 复用下述带引号转义渲染，并先截到 256 个 Unicode 标量前缀、
  仍有剩余标量时显式追加省略号；非 Unicode 或超限参数表绝不回显。机器 envelope 不变——
  诊断只存在于 stderr。
- 文本把用户可控字符串渲染为带引号的转义值：反斜杠与引号用 `\\`/`\"`；C0/C1 控制字符（含
  ESC）、U+2028/U+2029 与双向格式控制符用无歧义 `\uXXXX` 转义；其余 Unicode 原样保留。列表
  表格只把 label 显示截到 80 显示列并加省略号，转义序列与收尾引号保持完整；ID 绝不截断；
  `get` 字段值完整呈现。JSON 用标准无损编码保留源数据——绝不用 TUI 的替换字符清洗器。
- 完整输出在首次 stdout 写入前于有界缓冲区内构造；预备超过 8 MiB 即 `output-too-large`，
  stdout 不被触碰。真实 OS 写失败可能留下部分字节，但绝不追加第二份文档；stderr 仍报告且
  退出码非零。
- 机器输出与 locale 无关且不含英文 `message`；stderr 诊断按 en-US/zh-CN 本地化。机器 `kind`、
  字段名与枚举值绝不本地化；用户 label/path 数据按数据原样保留。

## 不变量

- 语法非法的调用绝不触碰文件系统、cohort 或 socket。
- 离线命令（`help`、`--version`、`--skill`、`completion`、`api schema`）绝不执行平台、运行、cohort 或
  Site 发现。
- 网络查询不创建任何事物：无目录、锁、标记、终端、viewer、控制授权或第二个请求——唯一
  例外是 `terminal read`/`terminal wait-output`，其只读能力 viewer attachment 是唯一被
  认可的读路径副作用，最小化持有并在输出前释放。至多持有一个短暂 Client Session，且绝不
  带入输出阶段。
- Mutation 每次调用在全新 `WorkbenchRequestId` 下至多提交一次 `workbench.command`；绝不
  重试或重放，结果不可判定时报告 `outcome-unknown` 而非猜测。`terminal send` 对其唯一
  一次 OBCS `terminal.send` 交换遵循同一纪律：每次调用在全新请求身份下提交一组有界记录序列，
  绝不重试——不可判定的应答是 `outcome-unknown`，且本地化诊断会说明输入可能已经落地，
  由操作员据此决定是否重复。
- 声明型 `terminal send` 源在任何运行时访问之前被完整物化：一组有界输入序列（文本 4096 字节、
  不透明字节 1 MiB、粘贴 256 KiB）喂给恰好一条 `terminal.send` 请求——绝不切分、重放或
  部分提交。源超时、取消或校验失败产生零运行时副作用且无请求身份；物化之后普通 send
  生命周期权威接管，源阶段结果不能在语义上取消已被准入的 send。
- Linux 上的源信号 guard 绝不改动 disposition，并把继承的线程 mask 精确恢复——被捕获、继承
  阻塞与继承忽略的信号各自保持原语义；没有任何信号 owner 活过源阶段，也没有任何
  合格信号在输出最终化路径之后仍被阻塞。
- 每次调用至多一次快照读取、一次准入，且读取与命令共用一个网络 deadline；`terminal read`
  的 viewer 交换与 `terminal send` 的唯一 send 复用同一 session 与同一 deadline——绝无
  第二次准入或续期预算。Agent 读取的可选名称解析及读取交换遵循同一纪律：每次请求均用
  全新 `ObapRequestId`、一个相关联结果，任何不可判定结果即 retire session 且绝不
  重放——且完全不取 Workbench 快照。
- Agent 观察绝不虚构：结果是逐字的 wire outcome，`found` 的 `explanation` 是 Server
  的 canonical 11 字段解读，成功绝不从终端内容或截图推断。`agent-read` source 只在
  可信的相关联应答之后存在；transport、timeout 与 protocol-invalid 失败均不携带。
  `terminal wait-output` 保持一次准入，每次 I/O 同时受五秒上限及总 `--timeout`
  剩余预算约束。来源模式的 `--forever` 仅取消总时限；每附件 `requestSequence`
  在全部 `read-rows` 中从 1 严格递增。
- match 只证明该文本在本次观察的搜索窗口内被找到——绝不证明命令完成、新 turn 或没有
  其他输出；轮询间隙滚过 Host 保留区的输出对任何消费者都不可见。
- 没有可信来源的字段不存在；`null` 表示来源确定地表达"无"，`"unknown"` 表示来源无法回答——
  两者绝不混用。
- 失败 envelope 绝不虚构 `source`：siteId/revision 只来自已验证快照。
- Ambiguous 绝不猜测：报告有界候选与真实总数。
- `api schema`、help 与 completion 只枚举同一次构建中真实交付的命令。
- `site start` 是唯一的激活面，也是唯一会创建本地运行时状态的命令：它可能创建运行时目录、
  持有激活/生存期锁、提交标记并拉起 Server——只有语法失败保证零副作用。其 20 秒绝对
  deadline 绝不按阶段重置；`recovery-required` 拒绝绝不重试、修复或清理；超时绝不声称
  没有 Server 启动。`site diagnose` 是观察面，除其有界生存期锁探针外无任何效应：不
  connect、不创建、不取锁、不修复，也不作存活或重启声明。
- 各 CLI 自有 5 秒预算都是 checked deadline；其构造失败是 `internal-failure`，而 transport
  内部 `RequestNotSubmitted` 一律保持 `transport-loss`。

## 场景矩阵与证据

| 场景                                                                                                                                                                     | 预期                                                                             | 证据                                                              |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| 穷举语法：format、位置、`--`、help 形式、重复、非法                                                                                                                      | 精确 parse/失败类别                                                              | `src/invocation` 测试                                             |
| format envelope 次序：任何位置的缺陷即空 stdout；任何位置的合法 json 决定其后失败的 envelope                                                                             | envelope 优先于语法错误                                                          | `src/invocation` 测试 + `tests/cli_query.rs`                      |
| 过滤器范围：`workspace list` 只接受 `--project`；越界过滤器在任何运行时访问之前即 usage                                                                                  | 完整矩阵，过滤器绝不静默丢弃                                                     | `src/invocation` 测试 + `tests/cli_query.rs`                      |
| 目标规则：ID/label/全零/超长/歧义优先                                                                                                                                    | resolver 判定                                                                    | `src/cli` resolver 测试                                           |
| 已验证快照 → 各结果形状、空集合、null                                                                                                                                    | 精确 JSON/text fixture                                                           | `src/cli` 输出测试 + JSON Schema 校验（dev `jsonschema`）         |
| Schema 字典 ↔ 实际 envelope                                                                                                                                              | 每个成功/失败 fixture 通过校验；被拒变体不通过                                   | schema 对比测试                                                   |
| API 失败格式：text/默认失败保持 stdout 为空，`--format json` 产出 envelope                                                                                               | `api schema`/`api snapshot` 失败遵守调用方 format                                | `tests/cli_query.rs` + `src/cli` 测试                             |
| 无 Site 磁盘状态                                                                                                                                                         | `service-unavailable`，零 fs/进程/锁变更                                         | `tests/cli_query.rs`                                              |
| 真实 staged-cohort Server：status/list/get/api snapshot、8 个准入加第 9 个被拒加释放、活跃 TUI                                                                           | 成功/`session-capacity`/结构不变                                                 | `tests/local_startup_process`                                     |
| 脚本化 OBLC/OBWP peer：denied、unavailable、畸形、错误 correlation、超时、断连                                                                                           | 按矩阵 typed kind；绝不冒称真实 Server 证据                                      | `tests/cli_query.rs`                                              |
| 脚本化 OBCS viewer peer：attach-binding 字段、`Empty`→`record` 轮询、detach 释放、reject/unavailable/畸形/correlation/断连                                               | `terminal read` 成功 + typed viewer-stage 失败                                   | `tests/cli_query.rs`                                              |
| 脚本化 OBCS viewer peer + 手工 OBRW record：滚动定价、容量/滚动钳制、typed `rows-rejected`、窗口失配、超大 record、无重试                                                | `terminal read --history` 成功 + typed 失败                                      | `tests/cli_query.rs`                                              |
| 脚本化 OBCS send peer：UTF-8 文本与可选折叠回车、语义键均以一条请求到达，target/请求身份，accepted/rejected/unavailable/unknown 应答、停滞应答、无重试                   | `terminal send` 退出 0/3/4/5/8 且两轴结果字段齐全                                | `tests/cli_send.rs`                                               |
| send 源语法：四源互斥、固定次序冲突命名、`--enter` 适用范围、flag 范围                                                                                                   | 任何运行时访问前的精确 parse/`invalid-usage`                                     | `src/invocation` 测试                                             |
| 文件/stdin 物化：非阻塞 open + `fstat` 类型闸、regular 文件共享 offset、私有 FIFO 重开、cap+1、不透明字节、空/非 regular/不支持类型的拒绝                                | 有界记录或类型化源 `invalid-usage`；零运行时副作用                               | `src/cli/source` 单元测试 + `tests/cli_send.rs`                   |
| 源 deadline/取消：停滞管道在共享 deadline 退出 7、被捕获合格信号按 `128+signo` 携带 `orchbee.cli.failure.source`、继承 IGN/BLK 原样保留、恢复失败即 internal             | 源 envelope 事实、无 wire 请求、精确 mask 恢复                                   | `src/cli/source` 测试 + 标注的 OS/进程证据                        |
| 脚本化 OBCS send peer + 声明型源：物化后的文件/stdin 以一条 `terminal.send` 文本请求到达                                                                                 | 与 `--text` 完全相同的 envelope；恰好一条请求                                    | `tests/cli_send.rs`                                               |
| Windows 自有管道读取器：原始字节、EOF、空输入、字节/粘贴上限、期限、注入取消、子进程回收                                                                                 | 精确载荷或类型化拒绝；无残留管道读取器                                           | `src/cli/source_windows/pipe.rs` 测试                             |
| Windows 公开进程与 PowerShell 5.1/7 管道源校验                                                                                                                           | 源完成后抵达与文本对照相同的源后拒绝；非法源先失败                               | `tests/cli_windows_input.rs`                                      |
| PowerShell 5.1/7 补全与离线本地化 skill                                                                                                                                  | 限定候选、重复键、粘贴互斥、根独占 skill 且不访问运行时                          | `src/cli/completion/powershell/tests.rs` + `tests/process.rs`     |
| 脚本化 OBCS viewer peer + 手工 OBTF/OBRW record：递增 `requestSequence`、可见/历史边界窗口、每帧真实 `FrameApplied`、stale 重钉、行尾 excerpt 命中、含 preamble 的总期限 | `terminal wait-output` 成功 + typed 失败                                         | `tests/cli_query.rs`                                              |
| 脚本化 OBLC/OBAP peer：list/get/explain 逐字 outcome、选项→wire 回显、denied/not-found/cursor/unavailable 应答、畸形/错 kind/correlation 不符帧、无重试                  | agent 读取成功 + typed `agent-read` 失败                                         | `tests/cli_agent.rs`                                              |
| Agent envelope ↔ 内嵌 schema：发出的成功/失败文档通过校验；wire 不可能变体（计数 129、重复 capabilities/ceiling）被拒                                                    | wire 契约的 schema 重述                                                          | `tests/cli_agent.rs`                                              |
| 真实 staged-cohort Server + 活跃 TUI 终端：`wait-output` 命中已有/后到/滚出屏/UTF-8 输出，短期限退出 7，取消后槽位可复用                                                 | 公共等待观察与 TUI 自身 viewer 共存                                              | `tests/local_startup_process`                                     |
| 真实 staged-cohort Server + 活跃 TUI 终端：`terminal read` 返回其网格并通过 schema 校验                                                                                  | 真实只读观察与 TUI 自身 viewer 共存                                              | `tests/local_startup_process`                                     |
| Mutation 语法：全部命令、target 规则、`--confirm`、几何界限、flag 范围                                                                                                   | 运行时访问前的精确 parse/失败类别                                                | `src/invocation` 测试                                             |
| Mutation 脚本化 peer：全部十五条命令、precommit 判定、不可判定与未提交结果                                                                                               | 每次调用一条命令；`outcome-unknown` 绝不重放                                     | `tests/cli_mutation.rs`                                           |
| Workspace close 语法：`close` target+`--confirm`；`close-status`/`close-resume` 严格稳定 ID、`--request`/`--generation` 栅栏、陈旧但合法的 generation 可解析             | 在任何运行时访问之前给出精确 parse/`invalid-usage`                               | `src/invocation` 测试                                             |
| Workspace close envelope：close-status 逐字 outcome（含 tombstone）、resume 源回显、封闭的 close-envelope details                                                        | 每个 fixture 通过校验；被拒变体失败                                              | schema 对照测试                                                   |
| Worktree list 语法：`--project`/`--directory` 双必填、严格目录 ID 值规则、flag 作用域                                                                                    | 运行时访问前即精确 parse/`invalid-usage`                                         | `src/invocation` 测试                                             |
| Launch-profile 语法：四条命令路径、逐字 selector、`--name`/`--exec` 必填对、`--profile` 互斥与作用域                                                                     | 运行时访问前即精确 parse/`invalid-usage`                                         | `src/invocation` 测试                                             |
| Launch-profile 文档/失败 envelope：条目成员名、封闭 details 词汇、双 `source` 形态                                                                                       | 每个 fixture 校验通过；被拒变体失败                                              | schema 对照测试                                                   |
| Worktree list 文档/失败 envelope：条目 XOR 字节字段、bare/detached/HEAD 不变量、封闭 details 词汇                                                                        | 每个 fixture 校验通过；拒绝变体失败                                              | `tests/cli_query.rs` schema 对照测试                              |
| Worker 发现 job：真实一次性 Git 仓库（init + commit + detached 链接 worktree，仅逐命令 `-c` 身份）、共享 TTL/stdout 预算归因                                             | 逐字 `-z` 字节、真实 HEAD/ref、类型化 outcome                                    | `apps/orchbee-worker` `worktree_discovery_job` 测试               |
| Server worktree-list 处理器：`-z` 解析接受/拒绝矩阵、membership 权威、outcome→失败映射                                                                                   | 字节精确投影；畸形 listing 即 `check-failed`                                     | `apps/orchbee-server` `worktree_list` 测试                        |
| 非 TTY；stdout 阻塞时 session 已释放；各 format 预备 >8 MiB；真实写失败                                                                                                  | 文档化退出；阻塞前 transport 句柄已 drop                                         | `tests/local_startup_process` + emit 单元测试                     |
| `config check` 矩阵：每项 pass/fail/not-applicable、`exists` 语义                                                                                                        | 五项 typed 检查齐全                                                              | `tests/cli_query.rs` + `src/cli/config` 测试                      |
| `agent doctor` 语法：provider/asset-root 准入、去重、上限、帮助路径；合成 fixture → 全序判定（codex 阶梯、guarded/capsule/root/socket 探测）、封闭词汇                   | 确定性离线事实；产出文档通过 schema 校验；无 provider 执行、无 socket connect    | `src/invocation` 测试 + `src/agent_doctor` 测试                   |
| `site start` 真实 staged cohort：冷启动拉起+准入、第二次/并发启动、无属主标记拒止、持锁 deadline、cohort 故障、信号、非 UTF-8 base、zh-CN 文本                           | `started`/`already-running` 分支真实；被拒 wedge 后标记逐字节不变；deadline 有界 | `tests/local_startup_process`                                     |
| `site diagnose` 语法 + 观察矩阵：仅 format 选项、每行 state/reason、条件性通告、held/free/unsafe 生存期锁、未准入父级、耗尽预算、零创建                                  | 精确 parse；封闭事实行；无 connect/创建/修复；无存活声明                         | `src/invocation` 测试 + `src/cli/site_diagnose` + rendezvous 测试 |
| 补全脚本投影语法（bash/zsh 实执行，fish 结构对齐）                                                                                                                       | 正/负上下文；无运行时枚举                                                        | `tests/cli_query.rs`                                              |
| 双语文本/失败输出                                                                                                                                                        | 精确 en-US/zh-CN 字符串                                                          | `tests/process.rs` + `tests/cli_query.rs`                         |

模型义务：本切片不新增 Server 状态机、持久化或重试，复用已注册的 OBLC/OBWP/OBCS/Client
模型——`terminal read` 复用既有 `terminal.viewer` attach/poll/detach 生命周期，`--history`
在其中加一次有界 `read-rows`/`rows-rejected` 交换，`terminal wait-output` 则循环同一
poll 并对每个已应用帧加一次有界 `read-rows`——因此无需新 Server 侧协议模型。该命令自身的
结算门——逐步 flag 采样、cleanup 来源与无条件迟到采样——由
`formal/tla/NativeCliWaitSettlement` 单独建模；见 `native-cli-wait-model.md`。已注册模型中
触发器覆盖 `apps/orchbee` 源码、manifest、project scope、README 与测试路径者仍须实际运行；
清单输出不能替代执行。

## 单次启动 cwd 与环境

`tab create` 和 `panel split` 接受绝对 `--cwd` 及重复的字面量
`--env NAME=VALUE`，独立于默认 shell、`--exec` 或 `--profile` 选择。
显式 cwd 仅为该次启动覆盖已接纳的 Workspace 目录；二者都缺失时沿用 Worker
主目录默认值。Worker 放置、Workspace 成员关系及存储的 profile 不变。
Host 校验实际目录并报告既有启动失败结果，不创建目录，也不静默替换为回退路径。

环境覆盖继承/默认值：最多 32 组，名称和值合计最多 1024 个 UTF-8 字节；
名称是 1–148 字节 ASCII 标识符。禁止 NUL 和忽略大小写后重复的名称，允许
空值及值中的字面等号。`ORCHBEE_*`、`TERM`、`COLORTERM`、`TERM_PROGRAM`
按忽略 ASCII 大小写保留。不进行 shell 展开、profile 修改或全局环境修改。
用法诊断和 debug 投影不包含环境值。

选项参与请求/重放身份。待处理的 Tab 和分屏在提交前及完成时核对解析后的
cwd 和环境。空选项保留旧 JSON 和二进制编码。非空 OBCN 环境使用旧布局非法的
零可执行文件长度前缀，随后为扩展版本 1、可选显式程序及有界长度前缀键值组。
新解码器接受旧载荷；旧解码器拒绝扩展，不会丢弃。交付使用匹配 cohort 哈希。
最大 cwd、argv 和环境合计保持在现有 8192 字节帧上限内。

证据覆盖构造器/解码器拒绝、新旧编码往返、组合上限、CLI 作用域和脱敏、Server
组合及重放身份，以及公开 Tab 和分屏命令启动的 Windows 子进程观察真实 cwd/环境。
不新增 macOS/Linux 适配器或运行声明；TLA 执行继续延期。

原生启动命令使用独立的 48 KiB JSON 请求预算，使 cwd、argv、label 和环境的最大转义编码可同时容纳。OBWP command kind 2 在分配前按此上限检查；非启动 intent 仍为 16 KiB，结果沿用既有上限。OBCS v2 帧允许 48 KiB；OBCN 仍为 8 KiB。OBCS v2 schema/corpus 已覆盖这些启动值；共享 Workbench schema 和 Desktop binding 仍不构成对这些原生扩展的支持证据。

## 调用者 Panel 上下文

`panel current [--panel <panel-id-or-selector> | --current] [--client <id>]` 按
[当前 Panel 契约](native-current-panel.zh-CN.md)读取 Windows Panel，与
`panel process-info` 共用目标解析。显式 Panel 选择器忽略调用者环境，无需 Client。
显式 `--client` 忽略调用者环境，与显式 Panel 选择器互斥。否则仅当两个调用者变量
均缺失时允许回退至唯一 current Client；部分缺失、格式错误或过期的调用者信息
会失败。所选 Client 必须 Ready，且 Panel 仍属于其报告的 Context 和 Tab。
Client 选择歧义时必须使用 `--client`。

Windows Worker 从已接纳的启动授权写入子进程的 `ORCHBEE_TERMINAL_ID` 与
`ORCHBEE_WORKER_ID`，覆盖继承的冲突值。这些产品身份是查询提示而非凭证；两者
均须为非零、完整的 32 位小写十六进制 ID。格式错误、空值、非 Unicode 或部分
缺失返回 `invalid-usage`（退出码 2），提供本地化指引且不回显变量值。
显式 Panel 或 Client 模式绕过这些元数据。

一份已验证快照负责 Panel 成员关系。有效 Terminal/Worker 绑定解析其所在 Panel，
包括子 Presentation 和已移至另一 Tab 或 Workspace 的 Panel。绑定缺失或不匹配
返回 `not-found`（退出码 3）；身份重复返回 `protocol-invalid`（退出码 5）。
无效调用者绑定绝不回退。快照与可选 Client 观察并非原子操作，答案受观察时点限定。

当前 Panel 结果使用带快照来源的 `orchbee.panel.current` v3，与 Panel Get 投影
一致。五秒查询期限、取消、只读 Client 回退、进程查询的 Viewer 清理以及映射到
声明的 Windows 证据由所链接契约负责。目录 v50 包含 72 个 schema。不产生激活、
输入、控制权或焦点效果；Desktop 与其他平台运行时扩展仍排除。TLA 执行继续延后。

Windows Worker 身份在每次连接 Server 时由已有 OS 随机源重新生成；同一 Worker
的终端共享该 ID。不同 Site 可能复用 Terminal ID，因此精确匹配 Worker 可拒绝
来自上一轮或另一 Site 的调用者。不引入重连或接管。Tab close 接纳并非移除：
Server 发布移除前，查询仍反映成员状态；发布后才找不到旧绑定。

## 目录工作区创建

`workspace create --project <target> --label <label> --cwd <absolute-path>` 用一个原生 Server 请求（`workspace.launch`）创建可用的 Workspace。支持与 `tab create` 相同的 `--env`、`--exec`/`--arg`、`--profile` 和必须成组提供的四个尺寸选项。这些启动选项要求 `--cwd`；不带 cwd 时保留原有空 Workspace 操作。从一次快照解析现有 Project，不推断焦点或源 Workspace。

Worker 先探测请求目录，Server 再用一次修订原子登记 Workspace、目录、Terminal Context 及首个 Tab/Panel/Presentation。准入失败时不创建这些资源。提交成功返回现有 mutation v4 信封：action 为 `workspace.create`，包含 `projectId`、`workspaceId`、`directoryId`、`terminalContextId`、`tabId`、`panelId`、`presentationId`、字面 `path`、`label` 和 `terminal`。现有 schema 已允许这些字段，版本和目录不变。终端可能在提交后仍待定或启动失败：保留返回 ID，查询 Workspace，不再需要时显式关闭。响应未知时绝不自动重放或回滚。

未发布的原生线协议动作要求 `projectId`、`label`、`geometry` 和 `options.cwd`；可选的 `launch` 与 `profile` 仍互斥。复用现有 48 KiB 启动请求上限、精确请求账本和终端结算。`workspace-launch-pending` 与 `workspace-launched` 回显已提交修订和全部七个 ID；只有后者携带 `terminalOutcome`。Server 拥有六个新结构 ID 及终端绑定。共享 Workbench schema 和 Desktop binding 不构成支持此原生扩展的证据。必须整体启用及回退同一构建 cohort。Server 的[细化记录](../../orchbee-server/README.zh-CN.md#目录工作区启动)持有准入、提交和恢复语义。

## Agent Panel 目标模型

面向单个 Agent 的命令接受 `--panel <id|@panel:suffix>`，与位置目标、`--session`
和 `--name` 互斥。位置参数中的完整 Panel ID 优先按保留的精确 Session ID 或运行时
名称解析；只有明确未找到才回退到 Panel。显式 Session／名称不回退。带 `@panel:`
前缀的目标直接选择 Panel，不查询名称。Workbench 拥有 Panel 身份及其主 presentation；
Agent 状态拥有存活、可导航的 presentation 附件。隐藏附件仍可选。CLI 在持有的同一
Client Session 上读取一次 Workbench 快照，将主 presentation ID 交给现有 OBAP 名称族。
其 presentation 选择器在 Agent 状态折叠内完成解析、分配或释放：零个存活匹配为未找到，
一个为选中，多个会话为歧义。同一会话的重复附件只计一次。不分页、不取首个匹配，
不推断终端文本，也不维护客户端名称表。

工作流为解析 → 仅接入已有 Site → 可选的精确 ID／名称解析 → Panel 快照解析 →
OBAP presentation 解析／操作 → 固定 Session 的读取、等待或终端操作。全程共享调用者的
截止时间和取消唤醒；最终名称变更之前的失败属于读取失败，不是结果未知的变更。变更
回执丢失仍为 outcome-unknown，绝不重放。等待者不会跟随 Panel 后来的使用者。终端操作
保留选定的主 presentation，发送前拒绝缺失绑定或已改变的 Agent 附件；Server 仍拥有
终端授权。Workbench 和 Agent 读取是顺序采样，不承诺跨所有者原子事务。
Session／名称行为和已有回执来源不变。解析回执可报告 `resolvedBy: presentation`；
歧义是有界拒绝，不附候选列表。

原生语法与 Panel 投影测试覆盖选择器互斥及主／子 presentation 区分；Agent 折叠测试
覆盖零／一／多会话、已结束／不可导航／隐藏附件及重命名的原子拒绝。Windows 进程测试
覆盖共享接入、ID／名称优先级、固定目标等待、变更前读取失败和精确终端选择。
真实 Site／Worker／ConPTY 证据覆盖集成命令路径。这些检查不证明 provider 前台所有权
或就绪状态。TLA 执行继续推迟。

## Panel 进程信息

### Windows 解释与 Agent 输入预检

`src/cli/processes.rs` 负责对经过校验的 Host 样本作有界、无状态的解释；进程采集仍只归
Host 所有。每个进程增加可空 `argv` 和 `providerId`；`foreground` 返回 `kind`、可空
`pid` 与可空 `providerId`。进程结果为 version 2。通过现有安全 `winsafe` shell 绑定
调用 Windows `CommandLineToArgvW`，只解释非空、不含 NUL、至多 4096 个 UTF-16 单元
的命令行。空或不可用命令保持 null，不替换成 CLI 自身命令行，不做 Shell 展开、文件
查找或环境读取。这是 Windows 参数解析投影，不保证另一程序采用相同解析器。仅 Windows
的 feature 不增加 crate、版本、构建脚本或进程执行。

可执行映像必须直接标识 Agent，或者是已知运行时且其真实脚本入口标识 Agent。
eval／command 内容和任意后续参数都不能标识 Agent。已知包入口与可执行别名采用
显式列表。provider 匹配是诊断启发式，不是可执行文件认证；它不创建 canonical
Agent 状态，也不代表已就绪。

| 样本解释                        | 前台投影                            | 输入预检                        |
| ------------------------------- | ----------------------------------- | ------------------------------- |
| 一个 Agent 候选是所有候选的祖先 | `agent-chain`，选中 PID/provider    | 必须匹配预期 canonical provider |
| 多个无祖先包含关系的候选        | `ambiguous`，PID/provider 为 null   | 拒绝                            |
| 没有候选，保留的根进程仍存在    | `root`，根 PID，provider 为 null    | 拒绝                            |
| 根进程缺失且没有唯一候选        | `unavailable`，PID/provider 为 null | 拒绝                            |

父边只连接样本中创建时间不晚于子进程的父进程。遍历受样本大小限制并检测环；复用的
父 PID 不能伪造祖先关系。它采用 Herdr `e0507237` 的 Windows 祖先链选择意图，但范围
限制在 Orchbee 保留的 Job 内，不做全局扫描或 Git Bash 环境标记逃逸回退。歧义明确
返回，不伪装成可用 Shell；不声称存在 POSIX 前台进程组。

Windows 的 `agent send-keys` 和 `agent prompt` 先解析 canonical 会话及其精确终端，
在同一已接纳会话上查询该终端、核对预期 canonical provider ID（非可执行别名），然后在发送前重读固定 Agent。
第二次读取必须保持 provider、live 生命周期及选中的可导航 presentation 和 generation；prompt 也
使用最新观察检查 freshness 与 blocker。全部步骤共享原有五秒输入截止；每个 viewer
在下一步前尝试 detach，会话 drop 作为清理兜底。预检失败不发送输入。低层 `terminal send`
继续用于显式终端输入。检查和后续输入不具备原子性，进程可能在观察后退出；原有发送
回执与未知结果规则仍适用，不重试，也不声称持有进程租约。

运行时不匹配使用 `orchbee.cli.failure.agent-runtime` v1，携带相关进程 source 和有界
`stage/reason/sessionId/expectedProvider/detectedProvider` 详情。未知 provider、Agent
缺失、候选歧义、provider 不匹配与 canonical 绑定变化分别拒绝；采集和传输失败保留
原类型。验证覆盖 Windows Unicode／引号、空命令保护、运行时 eval 诱饵、祖先／环／
复用父 PID、封闭结果 schema、真实 CLI／命名管道预检顺序及零输入拒绝，以及真实隔离
Windows Job。上述检查不证明 provider 任务完成、身份认证或原子前台所有权。

`panel process-info [<panel-id>] [--panel <panel-id> | --current]` 读取 Panel 主 Terminal 的 Windows Job。位置参数与显式 Panel 选项互斥；完整 ID 和 `@panel:<suffix>` 使用既有 Panel 解析器。没有显式目标时，先在接入前验证 `ORCHBEE_TERMINAL_ID` 和 `ORCHBEE_WORKER_ID`，再从一次 Workbench 快照推导调用者所在 Panel。调用上下文缺失或过期时失败，不猜测有焦点的客户端。主 presentation 必须绑定终端，子终端不作回退候选。

一次仅接入既有 Site 的 Client Session 与一个绝对五秒预算覆盖快照、临时只读 viewer 接入、一次进程读取及尽力 detach。会话 drop 回收剩余 viewer。Server 再验证 Worker generation 与物理终端绑定；Worker 和 Host 保留进程事实授权边界。成功时发布确切的类型化 Windows Job 样本、Panel/Terminal 路由上下文和相关联的读取来源。顺序获取的快照与样本不宣称跨所有者原子观察。Host 失败保持明确，不转为空进程成功，也不重试。文本中的字符串经过转义，机器输出的创建时间保留十进制字符串。响应描述归属终端的进程，不代表 Windows 前台进程组或 Agent 的交互就绪状态。

JSON 封套为 `orchbee.panel.process-info` v2，schema 目录及双语帮助、补全与命令共同交付。证据覆盖隐式和显式目标、I/O 前的冲突拒绝、主与子终端选择、主绑定缺失、读取拒绝、响应身份错误、Host 错误保留、输出限额、viewer 回收，以及真实 Windows Site/Worker/Host 进程路由。共享类型化读取也是后续 Agent 启动和就绪工作的实际依赖，本身不关闭这些义务。

Host 的预期拒绝使用 `orchbee.cli.failure.process-info` v1，保留有界的错误代码；非法响应与传输失败使用通用 CLI 失败契约。成功的 `source` 同时记录快照 revision、OBCS request ID 与 Host correlation、epoch、attachment 和物理 Terminal ID；它们不是同一个身份。目录 version 39 包含 72 份 schema。

## Windows Agent 启动

[Windows Agent 启动](windows-agent-start.zh-CN.md)拥有 `agent start`、名称预留与就绪流程、Windows 引用、预算和恢复契约。`agent names` v2 独立显示启动预留；名称失败对 launch-pending 使用 v3。Schema 目录为 v50，含 72 个 schema。

## Windows Site 停止

`site stop --confirm [--request-id <32-hex>] [--retry] [--detach] [--timeout <ms>]` 停止现有托管 Windows Site，绝不激活新 Site。标识为非零小写 128 位十六进制值，省略时仅生成一次；`--retry` 必须提供标识。CLI 仅发送一次开始或重试请求，之后仅查询状态。`--detach` 立即返回已准入尝试；否则一个绝对截止时间（默认 65000 毫秒，范围 1–3600000 毫秒）覆盖运行时/cohort 准入、请求和轮询。取消或超时仅停止等待，不回滚已准入停止。每个运行时答案均保留请求标识，供 `site stop-status <id>` 查询。状态是事实文档，不是第二次变更。`orchbee.site.stop` v1 封套区分 stopped、pending、failed、busy、not-recorded、unmanaged 及 unavailable/unknown 传输结果；pending 可携带 detached、timed-out 或 cancelled 等待状态。已知 Server 失败保留尝试、阶段、原因及本次释放数量。只有准确匹配的严格已停止激活记录加上空闲的既有生命周期锁才能证明完成；端点消失、非法记录及不同标识均不能证明。后续启动消费该回执，因此未记录不能否定此前停止。Linux/macOS 返回 unsupported-platform，不产生运行时效果。

## Windows 保留 Worktree 操作

[Worktree 契约](windows-worktrees.zh-CN.md) 拥有 `worktree create/open/remove/status/resume`、来源／Project 推导、各命令 flag、操作身份、确认、等待、输出与恢复规则。它扩展本记录早先仅 list 的语法：Create/Open 接受可选标签和终端尺寸、源 Workspace 或 cwd，以及路径／分支选择器；Create 另接受 base。Remove 接受 force，要求 Workspace 及确认；Resume 要求操作 ID、generation 及确认。五条均接受 text/JSON；只有修改命令按该契约接受请求身份、信任与等待。`orchbee.worktree.operation` 封套列入目录 v50（72 个文档）。

Windows `worktree list` 额外接受来源 Workspace/cwd 与可选命令局部信任，默认采用调用者上下文／当前目录，无需 Project。本记录前面的已注册目录语法及封套仅适用于原 `--project/--directory` 形式。[来源发现契约](windows-worktrees.zh-CN.md#按来源发现-worktree) 拥有独立的 `orchbee.worktree.query` 结果（目录 v50/72）、互斥规则、总 deadline、失败退出码及已知 Workspace 关联。

## Workspace 组关闭投影

Windows `workspace close <target> --group --confirm` 使用独立的保留组合同；普通 Close 保持现有语法和文档。组 Close 接受 `--request-id`、`--detach` 和 `--timeout`（1–3600000 毫秒，默认 120000）。组状态使用 `workspace close-status --group --request <operation-id>`，纯读取预算五秒。组 Resume 使用 `workspace close-resume --group --request <operation-id> --generation <1..64> --confirm`，支持相同的 detach/timeout 选项。`--project` 和 `--workspace` 不能与组模式同用。这些是现有三个动词的选项；原生命令清单为 79 个。

`orchbee.workspace.group-close` v1 封套将保留的 Server 事实与可为 null 的调用者等待状态分开。source 为 `local-workspace-group`，Site ID 必须存在但可为 null。Start/Resume 只发送一次，后续请求均为读取，并固定到相同尝试和捕获成员身份。交付后丢失应答保留为未知，绝不自动重放。Pending/Closed 退出 0；拒绝或已知关闭失败退出 3；不可用/Worker 丢失/Site 停止退出 4；结果未知退出 8；内部错误或观测矛盾退出 1；等待连接中断/超时/取消分别退出 5/7/130，保留最后应答。[组归属与恢复](windows-worktrees.zh-CN.md#workspace-组关闭) 定义成员、上限和子操作观测。schema 均为内嵌；帮助、`--skill` 和四种补全投影均包含组形式，不探测 Site。

## Windows Client 导航

[Windows Client 视图控制](windows-client-control.zh-CN.md) 拥有 `client list/get/control-status`、`workspace focus`、`tab focus`、`panel focus` 和 `panel zoom`。导航通过 `--client` 或无歧义且已校验的调用者上下文定位一个 current TUI。只提交一次，并等待绘制确认；超时或 Ctrl+C 仅停止等待。单 Panel 缩放有明确无操作回执。目录 v50 包含 72 个 schema，新增均为 v1 的 `orchbee.client.view`、`orchbee.client.control` 和 `orchbee.cli.failure.client-control`。未增加操作系统窗口激活或 Mac/Linux 控制运行时。

## Windows 离线 schema 导出

`api schema [<schema-id>] --output <path>` 显式发布与 stdout 完全相同的完整 JSON 文档。`--format text|json` 控制失败呈现；成功时 stdout 与文件字节均为带末尾换行的相同 JSON。省略 `--output` 时保留现有只读行为。

解析器接受非空、不超过 4,096 UTF-8 字节的字面文件路径，在副作用前校验全部参数，不展开变量或前导波浪号。相对路径以调用进程目录为基准。拒绝 Windows 设备名、备用数据流和含糊的盘符相对路径。此命令不选择或启动 Site，不读取运行时配置，也不从网络获取 schema。本次不新增 Mac/Linux 文件导出运行时。

Schema 所有者先解析内嵌 ID，并在任何文件操作前准备完整的有界文档。未知 ID 保留现有 not-found 结果且不创建内容。Windows 导出器解析目标，拒绝已有的非普通文件或重解析目标，以独占方式创建随机同目录临时文件 `.orchbee-schema-<nonce>.tmp`，写入并同步完整文档，关闭句柄，再通过一次重命名发布。允许替换已有普通目标文件；不创建父目录，不回退为截断写入，不自动重试。

| 阶段                   | 结果与归属                                                          |
| ---------------------- | ------------------------------------------------------------------- |
| 语法或内嵌查询失败     | 无文件副作用；沿用现有类型化失败                                    |
| 目标或临时文件准备失败 | 目标不变；返回本地导出失败                                          |
| 写入、同步或发布失败   | 未成功发布；删除自有临时文件并报告具体失败阶段                      |
| 临时文件清理失败       | 保留并报告其路径，供调用者删除自有残留                              |
| 重命名成功             | 完整目标已发布；输出预先准备的 stdout，后续 stdout 失败不会回滚文件 |

专用 `orchbee.cli.failure.schema-export` 第 1 版文档包含 `write-failed`、封闭阶段值、字面目标以及可空的残留临时路径，退出码为 1。只有 `--format json` 会在 stdout 输出该文档，并始终在 stderr 给出本地化诊断。不改变 provider、终端、规范 Workbench 状态或运行时身份。进程崩溃可能遗留可识别、尚未发布的同目录临时文件；发布不声称父目录具有断电持久性。

维护中的 Windows 进程检查覆盖中文／空格和相对路径、完整及指定 schema、替换、语法／未知 ID 隔离、缺失父目录、目录目标、锁定目标时原字节保留、失败输出 schema 校验及正常清理无残留。选定文件导出能力以 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/cli/api.rs` 为参考；Orchbee 保留自身 schema 封套及完整文件发布约束。

## Windows Panel 布局查询

`panel layout`、`panel neighbor --direction left|right|up|down` 和 `panel edges` 检查精确的 Windows TUI Client。它们接受位置参数 Panel、`--panel` 或 `--current`，路由有歧义时使用 `--client`。读取保持焦点和终端尺寸，支持非活动或未访问目标。`orchbee.panel.layout` v1 保留单元格几何、可为 null 的隐藏矩形、未缩放邻居、物理边缘结果及操作回执。参见[几何契约](windows-client-layout.zh-CN.md)。目录 v50 包含 72 份文档；原生命令清单包含 85 条路径。

`panel focus [source] --direction left|right|up|down` 在指定 TUI 的未缩放单元格布局中查找一个邻居，保留缩放；无邻居返回 `direction-unchanged`，不切换到来源 Tab。找到邻居后固定目标，仅在绘制成功后返回 `direction-focused`。回执查询不重复移动。

`panel resize [panel] --direction left|right|up|down [--amount <number>]` 调整一个符合条件的分割边界，保留焦点和缩放。`layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` 设置精确二进制分割路径。两者都路由到一个 Windows TUI，并保留 `layout-adjusted` 回执。当前标签页的变化等待绘制，非活动标签页只保存、不导航。有限比例限制为 0.1–0.9；有限 resize 数量转为绝对值且最多 0.5，默认 0.05。比例归该 TUI 会话所有，重启后不恢复。参见[布局契约](windows-client-layout.zh-CN.md)。

## 共享窗格标题

`panel rename <panel-id> --label <text>` 设置共享主 Presentation 的自定义标题；`--clear`（或空白文本）恢复创建时的标签。接受精确 ID 或 `@panel:<suffix>`，不改变焦点或终端。[窗格标题契约](native-panel-labels.zh-CN.md) 负责规范化、修订与重放规则以及匹配 cohort 的兼容边界。`orchbee.panel.rename` v1 区分 `renamed` 与 `unchanged`，可空 `customLabel` 回显规范化后的请求元数据。`panel get` 与 `panel current` v3 提供不变的 `label`、可空 `customLabel` 及实际显示的 `displayLabel`。`api snapshot` v4 保留规范的可选自定义标题。目录 v50 包含 72 份 schema，原生命令清单包含 85 条路径。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。Windows CLI 通过原 Client 单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

Windows `tab create` 和 `panel split` 在创建前解析 `--cwd follow|home|current|workspace|<absolute-path>`；默认 follow 采样已解析来源的根进程目录，v2 回执记录实际目录来源／路径。它们接受可选目标及标签，默认不聚焦；`--focus` 请求创建后聚焦，分屏可带 Client 独立的 `--ratio`。显式目标无需 TUI 即可创建。`orchbee.creation.result` v2 分别报告已提交创建与后续比例／焦点结果；后续失败不能当作未创建而重试。参见[原生终端创建](native-terminal-creation.zh-CN.md)，其目标、总预算、并发保护和恢复规则为这些 Windows 命令的权威。

Windows `panel input (<panel>|--panel <panel>|--current) --right-click orchbee|panel` 设置共享右键策略，不移动焦点。`--current` 要求有效调用者元数据，显式目标绕过它。`panel split --right-click` 随创建提交新 Panel 策略。默认 `orchbee` 将右键保留给 Client；`panel` 允许无修饰键的右键在启用鼠标上报的终端内容内透传。已准入的手势在策略改变后仍保留匹配的松开事件。路由、排队输入复核及失败恢复见[Panel 输入契约](native-panel-input.zh-CN.md)。本功能不新增上下文菜单。`orchbee.panel.input` v1 返回 `configured` 或 `unchanged`；`panel list` v2 与 `panel get/current` v3 提供 `rightClick`，`tab get` v2 和 `api snapshot` v4 接受可选规范字段。目录 v50 含 72 个 schema，原生命令清单含 85 条路径。

## Tab 插入边界

Windows 上，`tab move <id|@tab:suffix|label> (--direction previous|next|first|last | --insert-index N)`
要求恰好一个位置选项。`N` 是原始上下文顺序中的零基边界，不是最终序号：0 表示首位，Tab 总数
表示末尾，源位置边界或紧接其后的边界保持顺序不变。解析接受 0..256 的整数；Server 拒绝超过
当前数量的值，不作截断。精确目标解析在既有总修改预算内使用一次快照、一个命令，不选择 Client
或隐式聚焦。

索引移动输出 `orchbee.tab.move` v1，包含 `action: tab.move`、请求的 `insertIndex`、精确上下文/Tab、
请求/Site/基础 revision，以及提交或未变化的 revision。方向移动保留 `orchbee.mutation.result` v4。
目录 v50 包含 72 个 schema 和 86 个命令路径。确定的越界拒绝保留 Server 原因。提交结果不明时
退役载体、报告 outcome unknown，绝不重放或补偿。原方向行为保留已有平台支持；此新增 CLI 形式
仅支持 Windows。不引入磁盘持久化或迁移。

索引命令运行时失败使用 `orchbee.cli.failure.tab-move` v1，包含封闭的
`tab-move-index-out-of-bounds` 原因。解析失败及原方向失败保留既有通用封装。索引载体将上下文
关闭的 `terminal-busy` 视为确定的提交前失败，不扩大方向载体契约。

PowerShell 补全将纯 ASCII 十进制整数 AST 常量按原始文本读取，因此 `--insert-index 2` 之后
仍可补全。此修复不求值或接纳变量、splat、表达式和脚本块。

## 按来源读取终端输出

Windows `terminal read` 和 `agent read` 接受 `--source visible|recent|recent-unwrapped|detection`。
recent 合并保留历史与实时屏幕；detection 读取最近的一个屏幕高度范围。
`--plain` 为管道直接导出文本，默认 recent，与 JSON 互斥。来源模式
`--lines` 允许零，钳制到 1000，recent 默认 80。它与既有 history/rows/unwrap
参数互斥，既有行为保持不变。[输出契约](native-terminal-output.zh-CN.md)
拥有选择、精确样本验证及 `orchbee.terminal.output` v1；目录 v50 共 72 个 schema。

Windows `terminal wait-output` 接受相同的 `--source` 和 `--lines` 选择，也可用 `--forever`
明确取消总等待时限。任一参数都会启用来源模式（默认 recent），与 `--rows` 互斥，
并输出 `orchbee.terminal.wait-output` v3。不带 `--forever` 时默认仍为 30000 ms；
`--forever` 与 `--timeout` 互斥，保留每次 I/O 的五秒上限并支持 Ctrl+C。
recent 等待合并软换行；visible/detection 按物理行匹配。既有调用保留 v1/v2 输出。
现有输出也能满足等待，匹配成功不代表命令执行完成。

Windows `terminal read` 与 `agent read --ansi` 导出带样式快照，默认读取 recent 输出。
`--format json` 使用 `orchbee.terminal.output` v2，并带 `result.encoding: "ansi"`；文本/plain
读取保持 v1。ANSI 与 `--plain` 及旧 history/rows/unwrap 参数互斥。它保留带样式空格和
Unicode，只生成 SGR；`recent-unwrapped` 拼接已验证的软换行。detection 仍是纯文本。
它不重现原始 PTY 字节、超链接或图形。渲染请求与预备输出各自保持既有 8 MiB 上限；
渲染资源超限直接失败，不回退、不重试。参见[输出契约](native-terminal-output.zh-CN.md)。

Windows `status server` 在 Client Session 准入前读取实际 Server 构建／协议／cohort 元数据；`status client` 离线检查当前程序。两者支持 `--format text|json`，不激活 Site。裸 `status` 仍查询 Workbench。[运行时状态契约](native-runtime-status.zh-CN.md) 拥有五秒时限、未知元数据、兼容性、schema 和失败语义。更新渠道管理仍未提供。

## Agent 导航

Windows `agent focus <target> [--client <id>]` 在一次客户端控制提交前，将名称、会话或窗格解析为唯一活动的主窗格。显式 `--session`、`--name` 和 `--panel` 使用现有解析器；`--request-id`、`--detach` 和默认 30000 ms 总期限沿用导航恢复契约。未就绪客户端在 Agent 查询前返回 `client-not-ready`。[Agent 导航契约](native-agent-navigation.zh-CN.md) 定义主呈现绑定、歧义、取消和保留操作查询。本命令不确认 seen/unseen 状态。

## Windows Agent 变更页

`agent changes [--cursor <c>] [--limit <n>] [--format text|json]` 在现有五秒 Agent 读取总期限内读取一页有界变更。省略 cursor 获得当前头部检查点；将返回的不透明 cursor 用于后续读取。Limit 为 1–100，默认 50。不筛选、不订阅、不长轮询，也不自动重放。命令准入一个 Client Session，在中断仲裁前关闭它，并输出一个结果。Ctrl-C 保持共享取消/收尾契约。

`orchbee.agent.changes` version-1 信封包含 canonical changes/current/reset-required 结果及关联 Agent source。Reset-required 明确表示连续性丢失，必须重建基线。无效/其他启动实例 cursor、禁止读取与运行时不可用使用现有 Agent 失败家族，operation 为 changes。返回页不得超过请求 limit。JSON 保留类型化事实与游标；本地化文本展示状态、revision 与续读信息。Server 拥有保留范围、认证及一致历史；CLI grammar/help/completion/schema/Skill 公开此边界。其他平台运行时暂不支持此新增命令。

Windows `agent get <target> --client <id>` 在 `orchbee.agent.get` v4 中附加所选 Client 投影；普通 Windows Get 为带运行时信息的 v3。[Client attention 契约](native-client-attention.md) 归属版本合并、聚焦及 idle/done 映射。读取共用五秒总预算，不确认已看见、不导航。缺失／过期观测的 seen 为空；规范状态保持独立。

Windows `agent prompt --wait` 在运行时预检后、单次提交输入前为保留的 Agent 变更日志建立检查点；检查点失败则不提交，其修订必须覆盖预检修订。每个完整验证的后续页保留短暂活动；fresh 且更新的 provider 版本必须显示 working/blocked 或 turn-finished 跃迁，才能打开活动门槛。只有 working/blocked 条件可匹配这个门槛事件；稳定状态条件必须匹配当前样本，因此短暂完成后又 working 时继续等待。单纯元数据或新鲜度变化不构成活动。保留五秒活动门槛、原始总／读取期限、固定身份和取消归属。当前规范样本须覆盖日志头。Windows v3 将可空 `matchedChange` 及其来源与当前观察分开；Linux 保持 v2。历史丢失或跨启动周期游标明确失败，并保留已接受的输入回执。这证明检查点后的已观察活动，不证明所提交回合的因果关系或独占归属。

Windows `agent prompt <target> <text> --wait --client <id>` 在发送前预检一个精确 Client，之后每次采样读取其 attention。规范活动门槛仍适用。Idle/done 只匹配其当前经核实的静止状态投影；缺失／过期版本继续等待。Client 丢失或读取失败时停止等待，保留已接受的输入回执。Client 读取组合最多两秒，并受既有五秒评估及原始总期限限制。读取不确认已看见、不聚焦、不重发。所选 Client 成功封套为 v4，要求 `client`；普通 Windows prompt 保持 v3，Linux 为 v2。`--client` 要求 `--wait`，不新增 Mac/Linux 实现。

Windows `agent get` v5 增加 `runtime`；`--client` 使用 v6 并保留既有 Client 投影。目标解析、规范 Get、唯一活动主 Terminal 的只读进程样本、第二次规范 Get 和可选 Client 证明共用五秒总预算。最终 Agent provider 和所选 Presentation generation 必须仍匹配；绑定变化时丢弃进程样本。已结束、未绑定或主绑定歧义时明确报告 runtime 不可用。已观察 runtime 保留实际进程查询来源及 Windows Job DTO（PID、创建时间、映像、命令行、argv 和可空 cwd）。`promptReady` 表示当前 fresh、已知且无阻塞的规范 Agent 与已识别的 provider 进程链一致。这是提交预检，不是交互 UI 就绪、操作系统前台或原子输入租约。进程错误保留既有封套，所有 Viewer attachment 均关闭。Linux get 保持 v1。内嵌 schema 保留旧版本，配套 Windows cohort 输出 v5/v6。

Windows `agent list --client <id>` 在 v2 列表封套中加入所选 TUI 的状态。它对有界页批量读取，重读原始游标并检查保留变更后才暴露已读／未读。空页仍验证精确 Client；无此参数的列表保持 v1。[Client 提示状态契约](native-client-attention.zh-CN.md) 拥有一致性与期限规则。

Windows Agent Get 现在输出 v5（使用 `--client` 时为 v6），增加独立的 `management` 观察，包含来源、修订及 Server 所有的启动状态。Active 记录只来自已完成的启动；手动命名会话仍为非托管。Pending 启动使 promptReady 为 false，`agent prompt` 在输入前拒绝。Status 有界读取共用命令原期限，失败可使用 Agent 读取失败 v3（`operation: management`）；它不重试、不完成启动。`agent start` v2 在现有成功完成后显式报告 interactiveReady。[启动契约](windows-agent-start.zh-CN.md) 拥有生命周期和限制。
