# Windows Worktree 操作

本记录拥有原生 CLI Worktree 创建、打开、移除与恢复契约。Git 副作用由 Windows Worker 拥有；Server 拥有保留操作与规范 Workspace 关联。CLI 等待观测，绝不自行运行 Git。`worktree list` 支持按来源发现，同时保留既有的已注册目录查询契约。

## 命令与默认值

Git checkout 创建抑制进度输出。有界 stderr 保留最后 2048 字节，避免大量诊断挤掉最终失败原因。部分失败的 Create 通过观察原路径恢复，绝不重放 `git worktree add`。若观察证明 checkout 已不存在，该操作变为不可恢复。请检查保留的分支与 Git 错误，解决原因后提交新的 Create；CLI 不会删除分支或静默重试影响。

```powershell
orchbee worktree create --project demo --cwd C:/code/demo --branch feature/example
orchbee worktree open --project demo --cwd C:/code/demo --branch feature/example
orchbee worktree remove --workspace example --confirm
orchbee worktree status <operation-id> --format json
orchbee worktree resume <operation-id> --generation <attempt> --confirm
```

Create 与 Open 接受 `--workspace <target>` 或 `--cwd <path>` 二选一。均省略时遵循下文 Client／调用者来源规则；两者均不存在才使用 CLI cwd。部分、过期或无效的调用者身份绝不回退到 cwd。只有 Directory 来源可用 `--project <target>` 指定已有项目；Workspace 来源与 Remove 始终提交必填可空的 `projectId: null`。Server 为未分组的 Workspace 来源及新同级 Workspace 原子创建 Project，或复用来源已有分组。关联检出的来源 Workspace 同样创建同级 Workspace。Directory 来源未显式指定 Project 时创建未分组 Workspace。CLI 不执行 Project 准备阶段。

Create 接受可选的 `--branch`、`--base`、`--path` 与 `--label`。默认分支为 `worktree/<full-operation-id>`，base 为 `HEAD`，路径为 `<main-repository-parent>/<main-repository-name>-worktrees/<branch-slug>`。路径名将 ASCII 字母、数字、`_`、`-` 以外的字符替换为 `-`；绝不覆盖已有路径。已有本地分支忽略 base。Open 必须提供 `--path` 或 `--branch` 之一；分支有歧义须使用路径。主检出与关联检出均可打开。选中 checkout 已全局关联唯一 Workspace 时复用，`--label` 也会重命名它。新关联创建 Workspace 并启动其首个默认 shell 终端。可选四项尺寸参数遵循 `tab create`；Workspace 已提交但终端启动失败或不确定时仍保留在结果中。

Remove 要求 `--workspace` 与 `--confirm`。关闭 Workspace 前检查登记关系、公共 Git 目录与脏状态。`--force` 明确允许 Git 丢弃检出目录中的改动。拒绝主检出、裸仓库及关联到其他已知 Workspace 的检出目录。移除成功保留分支，不使用递归文件系统删除兜底。

`--trust-repository` 仅对此次 Git 命令授予信任，不修改全局或仓库配置。Windows Worktree 的 `--cwd` 及 create/open 的 `--path` 接受绝对驱动器路径、相对最终执行 CLI 当前目录的路径，以及开头的 `~`、`~/` 或 `~\`。绝对路径拼写保留。主目录按非空 `USERPROFILE`、`HOMEDRIVE` 加 `HOMEPATH`、`HOME` 的顺序选择；已选择但格式错误的主目录直接失败，不再回退。完整语法校验与机器路由先于上下文解析。发送端与中继保留原始操作数；只有最终执行的 CLI 为显式主目录操作数读取这些变量，Worker 不读取。解析失败先于请求标识生成、配置读取、信号所有权及运行时访问。输入及展开后的 UTF-8 路径均不超过 4096 字节。路径解析不打开或 canonicalize 文件，新检出目标无需已存在。不支持的 UNC／设备命名空间和驱动器相对路径会被拒绝。原始路径语法无效时，在读取主目录或当前目录上下文之前拒绝。不执行 shell、通配符、`$VAR` 或 `%VAR%` 替换，普通路径分量中的这些字符保持字面含义。其他命令沿用各自声明的路径策略。源路径可位于检出目录内部。引用参数非空、至多 1024 UTF-8 字节，不能以 `-` 开头或包含 ASCII 控制字符；分支语法由 Git 校验。Create 和 Open 可用 `--focus` 聚焦已有 Client，默认 `--no-focus`。Mac/Linux 变更执行与 Desktop 命令暴露不在此 Windows 实现范围内。

## 等待与机器输出

所有操作输出 `orchbee.worktree.operation` v5，原请求 Project 元数据可空，且没有 `setup` 阶段。`Available.projectId` 描述原请求，不代表 Workspace 的当前分组。后续失败保留已知/未知操作标识及部分 Git/终端事实。详见[Workspace 创建模型](windows-workspace-creation.zh-CN.md)。

Create、Open、Remove 接受非零 32 位小写十六进制 `--request-id`，也可在提交前生成操作标识。交换请求 ID 与操作 ID 分离。相同身份及意图观察保留操作，意图变化则冲突。`--detach` 返回首次接受观测。CLI 默认等待 120000 毫秒；`--timeout` 接受 1–3600000 毫秒。每次交换限两秒，轮询间隔 50 毫秒。超时、Ctrl+C 或断线停止等待；Server 继续执行已接受操作。CLI 只提交一次 Start 或 Resume，之后仅 Read；不自动重放不确定的提交。

`worktree status` 为总预算五秒的纯读取，不激活 Session 或启动维护线程。`worktree resume` 要求通过 `--generation` 提供可恢复失败状态的精确 `attempt`，并要求 `--confirm`，因为保留意图可能删除用户工作。尝试序号范围为 1–64。记录可跨 Client 断线保留，但不跨 Server 重启。

`orchbee.worktree.operation` v5 封套注册于 `api schema` 目录 v143。它包含操作 ID、已知时的本地 Session 身份、相关联的 Server 回答或明确的不可用／不确定观测，以及独立且可空的等待状态。Pending 与 detached 观测退出 0；终端启动失败保留自身非零退出码；Git 或关联失败退出 3，接纳不可用退出 4，观测断线退出 5，内部或不一致观测退出 1 且保留最近回答，超时退出 7，物理／提交不确定退出 8，取消退出 130。解析与提交前目标失败使用既有 CLI 失败封套。有界 Git 原始诊断在 JSON 中保留为字节数组，本地化文本中进行转义。

## 来源与打开后聚焦模型

来源形式的 List、Create、Open 接受精确 `--client`。显式 Workspace／cwd 优先且忽略无关调用者环境。否则显式 Client 优先于调用者；未指定 Client 时，依次使用经校验的调用者、唯一当前就绪 Client，只有调用者与 Client 均不存在才使用 CLI cwd。无效调用者以及过期、不可用或歧义 Client 均在 Git 前失败，不回退到 cwd。

CLI 准备来源；Server 仍是 Worktree/Workspace 的唯一写入者。Workspace 来源始终保留全局稳定的 Workspace ID，由 Server 验证 canonical 主 Directory。只有独立上下文来源才使用已证明 Panel 的主终端根进程 cwd。cwd 缺失、非法或不可用时在 Git 前失败。显式 Directory 来源可选择已有 Project，不根据唯一 Project 推导分组。Remove 保留显式 Workspace 契约。可配置检出路径由 [Windows CLI 配置](windows-cli-configuration.zh-CN.md)拥有。

Create 和 Open 接受互斥的 `--focus` 与 `--no-focus`（默认）。聚焦拒绝 `--detach`，要求已有就绪 Client，绝不启动 TUI。Remove、Status、Resume 拒绝 Client／聚焦参数。

| 转换                                        | 守卫与可见结果                                                                                       |
| ------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| 预检                                        | Git 前固定一个就绪 Client 及其语义选择                                                               |
| 已知 Opened 且终端 Started 或复用 Workspace | 读取同 Session 的新快照与精确 Client；要求语义选择未变且 全局 Workspace 归属有效，不依赖当前 Project |
| 提交聚焦                                    | 使用新视图版本与规范版本提交一次既有 Context 意图，等待已绘制／已见确认                              |
| 选择变化、断线、拒绝、超时或取消            | 停止聚焦，保留规范操作及 Workspace 身份，不回滚或重放 Git                                            |
| 聚焦交接不确定                              | 保留 Client 回执，绝不自动重提                                                                       |
| 未要求聚焦                                  | 保留 v5 输出与规范等待行为                                                                           |

语义选择指上下文、Tab、Panel 与缩放身份；派生面板数和普通版本刷新不表示导航变化。聚焦共用调用截止时间及有界交换。显式聚焦使用 `orchbee.worktree.operation` v5，附独立 `postOpen` 回执：Client ID、阶段、聚焦回执、失败和完成／未完成结果。聚焦失败保留规范成功并返回非零退出码。这不是 Git／界面原子事务。Server 工作可跨 CLI 断线继续；聚焦归本次调用所有，`worktree resume` 不恢复聚焦。

确定性来源／守卫用例、语法冲突、版本化 Schema 和 Windows 脚本 IPC 轨迹覆盖优先级、缺失 cwd、歧义／过期选择、视图变化、提交后不确定性及部分结果。可释放的真实 Windows Git 查询覆盖未变的 Worker 边界。完整双 TUI 验收独立进行；有界同作者检查不证明所有实时竞态。参考：Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` Worktree 来源／聚焦语义。

## 保留续执行

| 状态或事件   | 准入与下一步                                                                 | 保留事实                                         |
| ------------ | ---------------------------------------------------------------------------- | ------------------------------------------------ |
| Start        | 精确 Session 授权、可选 Project、来源与 Worker 实例；全局仅一个 Pending 操作 | Worker 交接前保留意图与操作 ID                   |
| Create       | 观察来源、通过 Git 创建、验证检出与分支、关联 Workspace                      | 验证失败时保留候选路径与可能的 Git 影响          |
| Open         | 观察选择的检出，复用或创建 Workspace                                         | 规范 Workspace 身份及终端启动结果                |
| Remove       | 先检查、保留 Workspace 发布、释放终端、通过 Git 移除、再发布 Close           | Git 失败保留规范布局；验证后恢复全新的默认 shell |
| Resume       | 相同意图及 Worker 实例、精确失败尝试、明确可恢复资格                         | 新的受限尝试；旧回调不能覆盖它                   |
| Read 或断线  | 只读保留状态；断线不取消续执行                                               | 不重复 Git 或 Workspace 创建                     |
| Session stop | Close 维护及 Worker 冻结前停止并 join 保留提交者                             | 取消不等于回滚；Worker 拥有物理收尾              |

Server 最多保留 64 个操作，每个操作至多 64 次尝试。Worker 作业另有独立的 64 次保留准入上限。续执行的协作式工作预算为 120 秒，每个 Git 步骤上限 40 秒；Worker 拥有自己的 30 秒执行及五秒物理清理预算。新准入满时仍可读取和重复观察。不确定的物理或传输收尾不允许恢复。可能已有 Git 副作用的 Create 通过观察保留路径及分支恢复，而不是再次 Git add。Remove 在已知 Git 成功前保留规范布局。终端释放并完成清理后，保留精确 Close 发布；删除结果未知时保持保留及已有结构栅栏。已知 Git 失败会启动下述 shell 恢复。Resume 从保留仓库根目录重新检查；复用仍保留的 Close，或在 shell 恢复后捕获新的 Close。Worktree 失败后，显式普通 Workspace Close 可不执行 Git 而释放保留；pending 时返回 busy。已知 Git 删除成功先保留再释放发布，因此最终 Close 的重试不会重复 Git。恢复保留 Workspace、Tab 和 Panel 身份，同时替换 Presentation 和物理终端身份。

## 移除失败后的 shell 恢复

收到 Git 移除失败后，只读检查必须证明原 checkout 路径、公共 Git 目录和仓库根仍一致。验证失败时保留已关闭的终端绑定和发布保留，不启动 shell。验证成功后，Server 为每个捕获的原生主 presentation 恢复新的默认 shell 绑定，使用 checkout cwd、空自定义环境和初始 80×24 尺寸。既有布局、标签和导航身份不变。旧 Agent 命令、自定义程序和启动配置绝不重跑。既有原生 Close 仍在效果前拒绝子 presentation 和分组／固定 Tab；恢复不会扩大该成员契约。

每个替换在 IPC 前同时预留规范投影和 TerminalCoordinator。串行启动收尾期间，旧 Close 继续限制结构。完成时记录已知成功、已知失败或不确定结果，不重复启动。随后旧 Close 以不可恢复的 `workspace-retained` 结束：旧绑定墓碑仍保留，当前结构恢复可用，新的 Close 捕获新绑定。部分恢复可能留下未尝试的已释放绑定或启动失败的新 shell。未知启动禁止 Remove Resume。其他符合资格的失败可显式重试删除，并先关闭已恢复的 shell。Session 停止保留部分事实，不会复活已释放的进程。

新进度观测统一使用 `orchbee.worktree.operation` v5；schema 为旧消费者保留封闭的 v1–v4 文档。独立的 `progress.restoration` 回执包含 `total`、`attempted`、`started`、`failed`、`unknown`（各为 0–4096）和可空 `failure`。已完成计数之和不得超过 attempted，attempted 不得超过 total，串行启动至多一个未完成。最终回执不得存在未完成启动。原 Git 错误仍为操作失败原因；验证或准备失败留在恢复回执内。每个 presentation 的精确绑定及 Create 结果从规范查询和终端查询取得，不复制到这个有界 96 KiB 操作文档。中英文文本输出均展示全部计数和额外失败。恢复启动存在未知结果时退出码为 8；已知 Git 失败仍为 3。

registry 到 Session 的锁顺序保护接纳与规范事实检查；IPC 在两把锁之外执行。Remove Pending 期间，全局保留新的 Workspace 启动及主目录接纳。在关闭和删除前复查现有关联。这些检查不能排除任意外部 Git／文件系统写入者，也不构成文件系统事务。终态结果一经发布即不可变。维护所有者拥有线程，并在 Session 关闭时显式 join；Drop 仅请求取消。

## 按来源发现 Worktree

`worktree list --workspace <target>` 与 `worktree list --cwd <path>` 选择一个来源，可选 `--trust-repository`。均省略时遵循上文 Client／调用者规则，也接受精确 `--client`。无效调用者身份绝不回退。查询不要求 Project，不创建 Workspace、不改变焦点。注册目录形式现在只用 `--directory <id>`，输出不含 Project 身份的 `orchbee.worktree.list` v2；来源形式使用 `orchbee.worktree.query`。

来源查询采用五秒 CLI 总预算，不激活 Session，绝不重放交换。来源选择器与旧选择器组互斥。JSON 从目录 v143 输出 `{schema:"orchbee.worktree.query",version:2,result:<typed-reply>}`；Server 失败保留类型化回答。Found 退出 0，来源不存在／拒绝／变化或 Git 失败退出 3，不支持／不可用退出 4，传输／列表无效退出 5，超时退出 7，I/O／启动／输出上限失败退出 1。准入前／传输失败使用通用 CLI 失败 Schema；Ctrl+C 在释放信号所有者后退出 130。人类文本转义不可信路径／标签，并标示主／关联检出、分离／裸仓库、锁定／可清理状态及 Workspace 身份。

Server 解析来源及存活 Worker 代际，在 Git IPC 期间释放所有锁，发布前再核对两者。单个只读 Worker 任务观测 Git 公共目录、来源检出根与 porcelain 列表。命令内信任仅作用于所选规范仓库，不持久化配置。准入只检查驱动器路径语法。文件系统规范化与信任准备在所属任务内执行，并在创建子进程前复查取消／deadline；目录不存在返回类型化准备失败。OS 文件系统调用为协作式，不会被强制抢占。只读 owner 保持五秒预算、一个物理槽位、有界输出及六十秒回执；不消耗修改操作容量。Windows Git 盘符路径使用与其他 Worktree 条目相同的绝对路径校验。

结果报告主仓库根、common 目录、来源 checkout、Git 条目事实及已知规范 Workspace 关联。多个关联保持显式，不任意挑选首个 Workspace。每个关联的 `projectId` 必填可空，文本明确显示未分组状态。显式来源 Workspace 的归属由自身 Git 观测支持；其他关联要求已知匹配 checkout 路径。这是 Git 观测后进行规范成员校验，不是原子文件系统事务，也不证明未知路径别名。

| 转换               | 要求                            | 结果                           |
| ------------------ | ------------------------------- | ------------------------------ |
| 准入               | 有效读取权限、来源与存活 Worker | 捕获来源成员关系及 Worker 代际 |
| 观测               | 不持有 coordinator／registry 锁 | 单个有界只读 Git 任务          |
| 复核               | 来源成员关系与 Worker 代际不变  | 关联当前已知 Workspace 身份    |
| 来源或 Worker 变化 | 不复用旧观测                    | 明确失败                       |
| 超时、取消或断开   | Worker 继续拥有物理清理         | 不修改、不推断成功             |

严格请求／结果与二进制帧校验、Rust／TypeScript 共享样例、来源子目录／链接检出、拒绝信任、来源／Worker 变化及真实 Windows CLI 查询提供证据。旧查询保持独立覆盖。

## 验证

维护中的测试覆盖严格语法与确认、调用者身份与 cwd、部分结果及诊断转义、操作／交换关联、EOF 后不重放、授权及容量、脱离等待后的复用、Close 前拒绝脏目录、移除失败时保留布局、验证后重建默认 shell、已知／未知启动结果、显式重试与手动 Close，以及 Session 关闭前取消。真实可丢弃的 Windows Git 夹具覆盖来源子目录、主检出打开、分支复用、强制移除、分支保留和成功 hook 改变所选分支。Rust 与 TypeScript 消费同一 Worktree 一致性语料；JSON Schema 覆盖结构校验，绑定实现字节限制与跨字段关系。这些是同作者测试和复查，不宣称独立审查或 TLA 执行。

Windows 路径适配器是对显式环境与当前目录输入的有界纯转换。解析后的相对或波浪号操作数按 Windows 的方式做词法规范化：去掉 `.`，`..` 移除前一段但绝不越过盘符根，且不保留尾部分隔符；绝对字面量保持原样。文本输出以原生分隔符显示 Git 报告的路径，探测阶段的 Git 失败会说明该目录不是 Git 仓库或 Git 不信任其所有者。固定 Herdr e0507237 的常见路径向量覆盖相对路径拼接、波浪号分隔符及主目录优先级；严格拒绝无效上下文是有意的差异。测试覆盖未使用环境／cwd 隔离、UTF-8 和 4096 字节上限、歧义根路径、不探测文件系统，以及访问配置或运行时前隐去环境值的解析错误。纯语法解析保留四处显式路径操作数；注入能力验证目标路径先于来源解析，以及选择、标识、几何和等待策略不变。`cli_windows_machine_routing` 检查路由中原始操作数、语法先于上下文、机器选择及中继准入。`cli_windows_worktree_context` 通过独立提供最终进程的主目录与 cwd，检查完整的 Create/Open 类型化 intent 和 List 来源。实际 Windows Git 查询／创建／打开覆盖相对及波浪号操作数。这是有界对齐证据，不代表验证了所有 Windows 文件系统别名。

## Project 全部关闭

普通 `workspace close <target> --confirm` 只关闭选中的 Workspace，包括仍有同仓库关联同级 Workspace 的主检出 Workspace，不删除 checkout 或分支。`project close-all <target> --confirm` 显式关闭一个组织性 Project 的当前成员，即使成员来自不同仓库。关闭后保留 Project；`project dissolve <target>` 是独立元数据操作，只取消分组并保留全部 Workspace。

准入按全局导航顺序冻结至多 256 个精确 Workspace ID，允许空 Project。不静默加入新成员。每个捕获成员交给普通 Workspace Close，分组和 checkout 祖先关系不扩大作用域。成员已改变或不可用时保留部分事实，不复活已关闭终端。

原生 `workbench.workspace-group-close` 操作现在表示单个 Project。Start 携带 `operationId` 与 `projectId`；结果携带 `projectId`、attempt、status 和 members，每个成员仅有 `workspaceId`、label 及必填可空的子 Close。没有来源 Workspace 或成员 Project ID。等待前保留子请求标识；`close` 为 null 表示未提交，子 `observation` 为 null 表示已接受但未观测。未知或未观测副作用不能授权重放。结果上限 1 MiB、成员上限 256、尝试上限 64。相同身份/作用域读取保留事实，变化则冲突。

```powershell
orchbee project close-all <project> --confirm [--request-id <id>] [--detach] [--timeout <ms>]
orchbee project close-status --request <operation-id>
orchbee project close-resume --request <operation-id> --generation <1..64> --confirm [--detach] [--timeout <ms>]
```

Start/Resume 使用同一个总预算（默认 120000 毫秒，允许 1–3600000）；status 是五秒读取。CLI 只提交一次变更，后续仅读取并检查相同 attempt 与捕获子身份。Detach、超时和 Ctrl+C 只停止等待；Server continuation 继续拥有已接受工作。Resume 必须显式请求，且仅适用于已知合格失败。记录跨 CLI 断线保留，不跨 Server 重启。

`orchbee.project.close-all` v1 将 Server 保留事实与可空等待状态分开。Pending/Closed 退出 0，已知拒绝/失败 3，不可用/Worker 丢失/Session 停止 4，未知 8，不一致/内部观测 1，等待连接中断/超时/取消 5/7/130，保留最后回执。它与普通 `orchbee.workspace.close-status` v2 独立；后者只需稳定 Workspace 选择器，不再需要 Project 身份。离线帮助、补全与 `--skill` 声明两类操作；旧 `workspace close --group` 被拒绝。

语法、精确副作用进程测试、空成员和部分/未知 schema 测试覆盖 CLI 契约。Server 与真实终端验收拥有捕获成员执行、物理终端清理及 Git 路径/分支保留验证。这些有界测试不宣称 Git/UI 原子事务或穷尽竞态。
