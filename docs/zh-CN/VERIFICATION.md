# 验证

## 合并门禁

`pnpm verify` 是唯一的本地和 CI 合并门禁。提交一个已完成的变更前，从仓库根目录运行它。它按顺序执行：

1. `pnpm format:check`
2. `pnpm test:tla:inventory`
3. `pnpm build`
4. `pnpm lint`
5. `pnpm typecheck`
6. `pnpm test`
7. `pnpm test:exports`

迭代期间可以使用聚焦命令，但完成工作必须通过根级门禁。

## 执行与证据

- 不要在同一工作树中把 `pnpm verify` 与另一次根门禁、`pnpm test` 或 build 重叠运行。
  独立调度的检查只有在可写产物和 fixture 隔离时才能并行。根级 Node workspace tests 完成后，
  才开始 Rust workspace tests：缓存分离不能保护 native 进程的启动截止时间免受高 CPU 负载 Node 测试的影响。
  这不禁止 workspace task 已拥有的并行执行。
- 在最终门禁前完成受治理编辑。如果相关输入在运行期间或之后改变，应识别失效的证据，
  并在宣布完成前针对最终输入重跑必需门禁。命令成功只证明它检查过的输入，不证明后续编辑。
- 在受版本控制源码之外保留首次失败的命令、源码状态、平台/工具链、可用的 seed 和完整日志。
  后续通过不能诊断首次失败，也不能证明已经修复；应把未解决失败与后续结果一起报告。
  诊断性重跑必须指明要回答的问题，并保留先前结果；不得重试到通过，也不得在缺乏证据时
  把失败归类为环境问题或测试不稳定。
- 区分失败、中断、跳过、未执行与通过。部分门禁通过不代表完成，聚合摘要也不是完整测试清单。

## 检查输出

新增或修改的面向人的验证输出必须定义有界摘要、可操作的失败诊断以及完整详情的访问方式。
兼容时复用现有输出层；保留机器可读输出契约和显式详细模式。减少输出绝不授权删减检查、
吞掉警告或用日志解析的成功替代进程失败。

根级 `test`、`verify` 命令、Node 测试包装入口和 Rust workspace runner 默认输出阶段摘要。
`scripts/run-checks.mjs` 拥有根命令执行序列；`scripts/check-output.mjs` 拥有共享的输出呈现。
阶段仍然顺序执行、失败即停；显式分组的独立检查并行运行，保持相同的 workspace 范围、
compiler flags 和手动测试排除项。组内所有成员排空后按声明顺序报告，并以声明顺序中首个
失败成员的结果停止。只有子进程结果决定成功与否，绝不使用日志解析结果判定成功。

- 每项检查报告开始、结果和从启动到自身进程及 stdio 关闭的耗时，不包含等待同组其他成员
  或格式化摘要的时间。已识别的测试计数以及有界的警告或跳过提示作为补充；
  它们仅供参考，不是完整测试清单。
- 完整的 stdout/stderr 合并后直接写入 OS 临时目录中的私有文件，执行前打印目录路径。
  日志位于工作树之外，在失败或中断后保留，直到用户或 OS 移除。不会自动上传；
  分享前应检查是否包含敏感诊断信息。
- 失败时报告日志路径、有界的已识别错误摘要和开头/末尾摘录，并明确标记省略内容。保留子进程非零退出码；
  子进程信号产生惯例对应的非零信号退出码。检查启动失败或日志保存/读取失败也会让命令失败。
- 嵌套包装入口继承外层捕获，不重复创建日志或摘要。
  `pnpm test --verbose`、`pnpm verify --verbose` 和
  `node scripts/run-rust-workspace.mjs test --verbose` 直接流式输出底层信息，不捕获日志。
  被包装的 Node 测试命令也接受相同的 `--verbose` 选项。
- 并行流式输出为每行添加检查名称前缀。超长行拆为有界、带标签的片段，不丢失文本或拆开
  Unicode 字符；末尾未终止的片段补上换行。每条流等待目标写入完成后才继续转发，
  因而慢速目标不会导致输出队列无限增长。
- 包装层异步执行子进程并继承现有进程组。未捕获的单项检查继承 stdin；捕获输出或并行管道
  检查忽略 stdin。不引入重试、超时、后台执行或取消监督器。
  中断仍由现有终端/job 取消机制负责；只打印开始、没有结果不代表通过。

执行模型是固定有序列表：启动一个阶段的成员，保留输出，分别记录各成员关闭时间，排空整组，
然后在全部零退出时前进，或在失败时停止。摘要和详细模式执行相同列表。`scripts/check-output.test.mjs` 通过成功、失败、
信号终止、可执行文件缺失、嵌套和大输出子进程，以及无损日志和有界诊断，验证这个公开操作。
它不证明任意后代进程的取消，也不证明未实际运行这些用例的平台行为。独立 TLC 输出及其
机器可读结果保持不变。

## 必需任务

| 任务                 | 契约                                                                                                                                     |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `format:check`       | Oxfmt 管理的文件与 `.oxfmtrc.json` 一致，并且 rustfmt 与每个应用自有原生 formatter 都接受其所属文件。                                    |
| `lint`               | 代码、Rust workspace 与 Clippy、文档配对、样式、存储、架构、依赖、项目契约、体验目录、导出和清单规则全部通过。                           |
| `test:tla:inventory` | TLA+ 执行清单、产物登记、owner 命令、触发映射以及 runner 负向控制彼此一致。                                                              |
| `build`              | 每个可构建 pnpm 与 Cargo workspace 都使用其固定工具链和依赖生成所支持的产物。                                                            |
| `typecheck`          | 每个具有 TypeScript 契约的工作区都满足该契约。                                                                                           |
| `test`               | 契约检查器测试、文档配对负向测试、自定义 lint 规则测试、pnpm 与 Cargo 测试套件、必需的受控本地系统见证和已声明原生交叉编译检查全部通过。 |
| `test:exports`       | 每个库的打包导出、运行时格式和声明均可被使用。                                                                                           |

- `build` 先于 `lint` 和 `typecheck`，因为依赖分析和生产使用方解析库产物，不解析另一个包的源码。全新检出必须先生成这些导出，再让 Knip 判断依赖使用情况。
- 根构建先完成全部共享包，再启动应用构建；两个阶段各自保持拓扑顺序。Story 发现可能使用其所属方的包依赖，而宿主并不直接在运行时依赖这些包，因此应用不能与这些包的构建争抢执行顺序。
- 应用模块开发不消费库的 `dist`；Rsbuild 把已声明源码入口解析到开发图中。通过 Node 执行、且消费库 runtime export 的开发准备流程，必须在进程启动前显式构建该依赖。
- `pnpm verify` 可以重建包输出而不中断正在运行的应用服务器；验证和生产继续使用按拓扑顺序构建的产物。
- Rust workspace 工作使用精确的根工具链与已提交 Cargo lockfile；Zig 源码依赖使用不可变 URL 与
  content hash。必需 build 必须在生成 artifact 前拒绝不同 compiler 或 dependency resolution。
- `scripts/run-rust-workspace.mjs` 把根 pnpm 门禁连接到 Cargo。第一个已接受 Rust member 存在前，
  它验证固定工具链和 empty workspace metadata；之后同一批 task 使用 `--locked` 对每个 member
  运行 rustfmt、Clippy、build 与 test。
- 原生交叉编译检查只证明源码和依赖可为已声明目标编译；平台行为仍然需要原生 runner。
- 必需的 Desktop SSH Project-enrollment 见证会打包真实 Dev profile，并在 macOS arm64 或 glibc
  Linux x64 上针对临时 loopback `sshd` 运行。不支持的本地平台会明确报告跳过；必需的 Linux CI
  job 会配置 `sshd`、Xvfb 与 X authentication 并执行该见证。见证会移除自己的临时 home、key、
  已安装 runtime 与 application data，而且绝不会调用 manual native-window test。

## 变更触发的 TLA+ 验证

`pnpm test:tla` 会通过共享的有界 runner 运行所有已登记模型。`scripts/tla/models.json` 中的
执行清单为每项检查登记模块/配置、语义 owner、claim 权威、变更路径触发器、超时、配置变体和
必需 action coverage。触发器遵循抽象所对应的实现 owner 及其依赖；无关样式不触发文档或
终端生命周期模型。收窄触发器时，在排除用例旁保留 owner 路径的正向回归用例。
它是执行索引，而不是第二份行为规范；package 自有契约和 TLA+ 模块
仍然是模型语义的权威。

runner 使用 Node 内置模块和独立固定的 Java jar。因此根 `package.json` 与 `pnpm-lock.yaml`
变更通过必需的清单/命令检查验证，不会选中全部 TLC 模型。三个 TLA runner 测试入口也由必需的
inventory gate 验证。仅清单变化时，对比指定 Git 基线与工作区清单：检查新增或修改的模型条目，
并用新旧 owner 触发路径共同匹配实现变更。纯格式变化不选择模型。删除条目、修改清单顶层字段、
基线不可读或无效、修改 TLA 运行时代码（包括新增 helper）、CI 工具链工作流，以及验证或建模规范
均选择全量。协议包或原生进程本身构成抽象边界时，保留较宽的 owner 触发范围；缩小选择不能漏掉证据。

TLA+ 模型检查属于变更触发的证据：`pnpm test` 和必需的 CI `verify` Job 不会隐式运行 TLC。
`pnpm verify` 只运行低成本的 `test:tla:inventory` 一致性与负向控制检查。独立的 CI `tla`
Job 会把 Git diff 映射到已登记模型，并只运行这些检查。diff base 缺失或无效时会安全地
退化为选择所有模型。

当变更修改了所属工作区的 TLA+ 模块、配置、owner 契约、转换、不变量或精化映射时，运行
该工作区的 `test:tla` 命令。当多个模型 owner 或共享模型基础设施发生变化时，运行根命令。
`pnpm test:tla:changed -- <ref>` 使用与 CI 相同的清单映射。包自有的 `verify:correctness`
命令可以在该命令属于包的完整正确性工作流时包含 TLA+ 检查。

每项已登记检查都使用固定 SHA-256 的 TLC、已声明的 worker 数（默认为一，可通过模型的
`workers` 字段设为二）、固定指纹多项式、已声明的有限边界、
逐模型超时和 coverage 输出。runner 会在必需语义 action 缺失或没有可达 witness 时失败，
并输出机器可读的状态数、耗时、worker 数以及模型/配置 SHA-256 报告。配置边界和 TLC 的有限状态假设
仍是证据限制；action coverage 不是实现精化证据。

runner 使用 `-lncheck final`，在完整可达图生成后执行一次活性检查，保留公平性和所有配置
性质，同时避免反复检查部分图。安全性检查仍在探索过程中执行。在最终活性检查前超时
属于失败，不能作为进度证据。保留 coverage 采集以验证必需动作见证，并等待 Java 的 stdio
关闭后再解析最终报告。

独立模型/配置共享按资源预算调度的队列。默认槽位受可用处理器数和机器内存的 60% 限制
（遵守已报告的进程内存限制），每槽按 1 GiB 堆与 512 MiB 非堆预留计算。可传正整数 `--jobs N`
显式选择并发。每个 JVM 显式设置 `-Xmx`：总预算均分给实际槽位、扣除预留，每个堆最多 2 GiB；
低于每 JVM 256 MiB 的分配会被拒绝。这是容量预算，不是操作系统内存限制。每个模型的 TLC worker
数量保持不变。变体共享槽位，但只有同一模型全部变体成功后，才合并该模型内的必需覆盖证据。
每个配置独占状态目录；Java 日志持久保存在启动时打印的操作系统临时日志目录。状态清理后仍可
访问日志，运行期间也可读取；日志遵循根检查相同的本地保留策略。Java 关闭后输出完整日志，避免
并发反例交错。已完成的 `TLA_RESULT` 包含堆大小和日志路径。超时或子进程失败输出独立的
`TLA_FAILURE`，包含真实进程结果、耗时、可解析时的部分状态数与日志路径；部分状态数不代表通过。
失败后停止派发新任务，为尚未启动的配置输出 `TLA_SKIPPED`，等待活跃检查在各自超时内结束，
并令命令失败。同机顺序耗时比较使用 `--jobs 1`，保持已声明的 worker 数不变。

对于固定 TLC 的 action-header 格式，coverage 要求 `distinct:generated` 中的 generated-state
count 为正数；单计数 header 则使用唯一的 count。转移到已经发现的 state 仍然是可达 witness；
expression evaluation cost 不构成 action evidence。此解释遵循固定版本的
[TLC action reporter](https://github.com/tlaplus/tlaplus/blob/v1.7.4/tlatools/org.lamport.tlatools/src/tlc2/tool/coverage/ActionWrapper.java)。
`scripts/tla/model-inventory.test.mjs` 固定了返回已有 state 的回归案例，以及 missing、
zero-generated、expression-only negative control，不改变任何 model 或 required action。

本地 TLC 执行需要 Java 17 或更高版本，并使用 `PATH` 中的 `java`。设置
`ORCHBEE_TLA_JAVA` 可指定可执行文件，设置 `ORCHBEE_TLA2TOOLS_JAR` 可使用已配置且具有固定
SHA-256 的副本，设置 `ORCHBEE_TLA_TIMEOUT_MILLISECONDS` 可对所有选中检查应用正整数超时覆盖。

每次 TLC 调用（包括同一模型的并发变体）都为模型检查状态和 `java.io.tmpdir` 独占一个唯一临时目录。
固定版本的解析器以固定文件名提取标准模块，并在其 JVM 退出时删除；共享 JVM 临时目录可能截断或
删除另一次调用的模块。目录保持至子进程关闭，再由既有运行器生命周期清理。此隔离不改变模型并发数、
worker 数量、截止时间、配置或覆盖要求。

无关实现变更不需要重放未修改的模型。未修改模型通过也不能证明其抽象仍与实现对应；
确定性测试和可追踪性证据负责这种对应关系。影响模型的变更只有在选定的 TLA+ 命令通过，
且完成报告指出已检查模型和边界后才算完成。

## CI 契约

- `.github/workflows/verify.yml` 安装冻结的 lockfile，并在 Pull Request、向 `main` 推送和手动运行时执行 `pnpm verify`。
- CI 传入 `--verbose`，让完整诊断在临时 runner 被回收后仍保留于现有 job 日志中；
  只有本地临时日志路径不能构成持久的 CI 证据。
- 必需的 `verify` Job 不运行 TLC。必需的 `tla` Job 从已跟踪清单中选择并运行变更触发检查；
  无关变更会在不配置 Java 或 TLC 的情况下完成该 Job。
- 工作流通过固定的官方工具链机制安装每个已跟踪原生 workspace 要求的精确 compiler version。
  直接下载的 archive 在加入 `PATH` 前必须校验 digest。
- 在仓库 ruleset 中要求工作流的 `verify` 和 `tla` Job。
- CI 在验证后运行 `git diff --exit-code`；已跟踪的生成变更在提交前会导致失败。
- 工作流使用只读仓库权限。
- 凭据、写权限、发布操作、外部检出和其他副作用需要单独人工审查，不得隐式进入必需 Job。

## 外部来源审计

- 生产构建使用的内容寻址原生源码依赖是已声明构建输入，不是外部来源审计。其不可变 URL 和摘要属于所属原生清单。
- 需要外部检出、凭据、服务、设备或未声明环境输入的包专属审计，继续作为所属包记录的显式维护命令，不属于 `pnpm verify`。
- 只有当独立 CI Job 固定并配置每个外部输入、拥有稳定所有者且不为必需合并门禁添加副作用时，才提升这类审计。

## 扩展阈值

分别测量逐阶段耗时和输出字节数、行数。比较时保持命令范围、工具链及冷缓存或热缓存条件等价；
输出更安静本身不代表运行更快。下面的阈值只允许启动设计审查，不授权跳过必需检查。

| 决策           | 契约                                                             |
| -------------- | ---------------------------------------------------------------- |
| 默认           | 必需门禁保持快速时，运行完整工作区任务。                         |
| 受影响任务阈值 | 至少三个可独立变更的工作区，或冷启动 CI 关键路径持续超过 30 秒。 |
| 变更前         | 先测量瓶颈。                                                     |
| 全局策略检查   | 即使包任务改为仅运行受影响项，也保留显式输入。                   |
