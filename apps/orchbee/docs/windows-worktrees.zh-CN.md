# Windows Worktree 操作

本记录拥有原生 CLI Worktree 创建、打开、移除与恢复契约。Git 副作用由 Windows Worker 拥有；Server 拥有保留操作与规范 Workspace 关联。CLI 等待观测，绝不自行运行 Git。`worktree list` 支持按来源发现，同时保留既有的已注册目录查询契约。

## 命令与默认值

```powershell
orchbee worktree create --project demo --cwd C:/code/demo --branch feature/example
orchbee worktree open --project demo --cwd C:/code/demo --branch feature/example
orchbee worktree remove --workspace example --confirm
orchbee worktree status <operation-id> --format json
orchbee worktree resume <operation-id> --generation <attempt> --confirm
```

Create 与 Open 接受 `--workspace <target>` 或 `--cwd <absolute-path>` 二选一。均省略时，经校验的调用者终端选择其规范 Workspace；Orchbee 外使用当前目录。部分、过期或无效的调用者身份绝不回退到 cwd。可从 Workspace 推导项目，或只有一个 Project 时，可省略 `--project`。项目有歧义或不存在均在提交前失败；此命令不创建默认 Project。

Create 接受可选的 `--branch`、`--base`、`--path` 与 `--label`。默认分支为 `worktree/<full-operation-id>`，base 为 `HEAD`，路径为 `<main-repository-parent>/<main-repository-name>-worktrees/<branch-slug>`。路径名将 ASCII 字母、数字、`_`、`-` 以外的字符替换为 `-`；绝不覆盖已有路径。已有本地分支忽略 base。Open 必须提供 `--path` 或 `--branch` 之一；分支有歧义须使用路径。主检出与关联检出均可打开。同项目中已有唯一 Workspace 时复用，`--label` 也会重命名它。新关联创建 Workspace 并启动其首个默认 shell 终端。可选四项尺寸参数遵循 `tab create`；Workspace 已提交但终端启动失败或不确定时仍保留在结果中。

Remove 要求 `--workspace` 与 `--confirm`。关闭 Workspace 前检查登记关系、公共 Git 目录与脏状态。`--force` 明确允许 Git 丢弃检出目录中的改动。拒绝主检出、裸仓库及关联到其他已知 Workspace 的检出目录。移除成功保留分支，不使用递归文件系统删除兜底。

`--trust-repository` 仅对此次 Git 命令授予信任，不修改全局或仓库配置。Windows 路径为字面绝对盘符路径；不展开 `~` 或环境变量。源路径可位于检出目录内部。引用参数非空、至多 1024 UTF-8 字节，不能以 `-` 开头或包含 ASCII 控制字符；分支语法由 Git 校验。所有命令均不改变 Client 焦点。Mac/Linux 变更执行与 Desktop 命令暴露不在此 Windows 实现范围内。

## 等待与机器输出

Create、Open、Remove 接受非零 32 位小写十六进制 `--request-id`，也可在提交前生成操作标识。交换请求 ID 与操作 ID 分离。相同身份及意图观察保留操作，意图变化则冲突。`--detach` 返回首次接受观测。CLI 默认等待 120000 毫秒；`--timeout` 接受 1–3600000 毫秒。每次交换限两秒，轮询间隔 50 毫秒。超时、Ctrl+C 或断线停止等待；Server 继续执行已接受操作。CLI 只提交一次 Start 或 Resume，之后仅 Read；不自动重放不确定的提交。

`worktree status` 为总预算五秒的纯读取，不激活 Site 或启动维护线程。`worktree resume` 要求通过 `--generation` 提供可恢复失败状态的精确 `attempt`，并要求 `--confirm`，因为保留意图可能删除用户工作。尝试序号范围为 1–64。记录可跨 Client 断线保留，但不跨 Server 重启。

`orchbee.worktree.operation` 版本 1 封套注册于 `api schema` 版本 22（57 个文档）。它包含操作 ID、已知时的本地 Site 身份、相关联的 Server 回答或明确的不可用／不确定观测，以及独立且可空的等待状态。Pending 与 detached 观测退出 0；终端启动失败保留自身非零退出码；Git 或关联失败退出 3，接纳不可用退出 4，观测断线退出 5，内部或不一致观测退出 1 且保留最近回答，超时退出 7，物理／提交不确定退出 8，取消退出 130。解析与提交前目标失败使用既有 CLI 失败封套。有界 Git 原始诊断在 JSON 中保留为字节数组，本地化文本中进行转义。

## 保留续执行

| 状态或事件  | 准入与下一步                                                         | 保留事实                                      |
| ----------- | -------------------------------------------------------------------- | --------------------------------------------- |
| Start       | 精确 Site 授权、Project、来源与 Worker 实例；全局仅一个 Pending 操作 | Worker 交接前保留意图与操作 ID                |
| Create      | 观察来源、通过 Git 创建、验证检出与分支、关联 Workspace              | 验证失败时保留候选路径与可能的 Git 影响       |
| Open        | 观察选择的检出，复用或创建 Workspace                                 | 规范 Workspace 身份及终端启动结果             |
| Remove      | 先检查、提交 Workspace Close、等待已知 Closed、通过 Git 移除         | 后续 Git 失败仍保留关闭请求 ID 与 Closed 事实 |
| Resume      | 相同意图及 Worker 实例、精确失败尝试、明确可恢复资格                 | 新的受限尝试；旧回调不能覆盖它                |
| Read 或断线 | 只读保留状态；断线不取消续执行                                       | 不重复 Git 或 Workspace 创建                  |
| Site stop   | Close 维护及 Worker 冻结前停止并 join 保留提交者                     | 取消不等于回滚；Worker 拥有物理收尾           |

Server 最多保留 64 个操作，每个操作至多 64 次尝试。Worker 作业另有独立的 64 次保留准入上限。续执行的协作式工作预算为 120 秒，每个 Git 步骤上限 40 秒；Worker 拥有自己的 30 秒执行及五秒物理清理预算。新准入满时仍可读取和重复观察。不确定的物理或传输收尾不允许恢复。可能已有 Git 副作用的 Create 通过观察保留路径及分支恢复，而不是再次 Git add。Remove 恢复保持已关闭 Workspace 为关闭，并在源检出已消失后使用保留的仓库根。

registry 到 Site 的锁顺序保护接纳与规范事实检查；IPC 在两把锁之外执行。Remove Pending 期间，全局保留新的 Workspace 启动及主目录接纳。在关闭和删除前复查现有关联。这些检查不能排除任意外部 Git／文件系统写入者，也不构成文件系统事务。终态结果一经发布即不可变。维护所有者拥有线程，并在 Site 关闭时显式 join；Drop 仅请求取消。

## 按来源发现 Worktree

`worktree list --workspace <target>` 与 `worktree list --cwd <absolute-path>` 选择一个来源，可附加 `--trust-repository`。两者均省略时，已验证的调用者终端选择其 Workspace；Orchbee 外使用当前目录。无效调用者身份绝不回退。查询无需默认 Project，不创建 Workspace，也不改变焦点。原有 `--project <target> --directory <id>` 形式保持原请求及结果契约；来源形式使用 `orchbee.worktree.query`。

来源查询采用五秒 CLI 总预算，不激活 Site，绝不重放交换。来源选择器与旧选择器组互斥。JSON 从目录 v22/57 输出 `{schema:"orchbee.worktree.query",version:1,result:<typed-reply>}`；Server 失败保留类型化回答。Found 退出 0，来源不存在／拒绝／变化或 Git 失败退出 3，不支持／不可用退出 4，传输／列表无效退出 5，超时退出 7，I/O／启动／输出上限失败退出 1。准入前／传输失败使用通用 CLI 失败 Schema；Ctrl+C 在释放信号所有者后退出 130。人类文本转义不可信路径／标签，并标示主／关联检出、分离／裸仓库、锁定／可清理状态及 Workspace 身份。

Server 解析来源及存活 Worker 代际，在 Git IPC 期间释放所有锁，发布前再核对两者。单个只读 Worker 任务观测 Git 公共目录、来源检出根与 porcelain 列表。命令内信任仅作用于所选规范仓库，不持久化配置。准入只检查驱动器路径语法。文件系统规范化与信任准备在所属任务内执行，并在创建子进程前复查取消／deadline；目录不存在返回类型化准备失败。OS 文件系统调用为协作式，不会被强制抢占。只读 owner 保持五秒预算、一个物理槽位、有界输出及六十秒回执；不消耗修改操作容量。Windows Git 盘符路径使用与其他 Worktree 条目相同的绝对路径校验。

结果报告主仓库根、公共目录、来源检出、Git 条目事实及已知规范 Workspace 关联。多个关联明确保留，不任意选择第一个 Workspace。显式来源 Workspace 的成员关系由其自身 Git 观测支持；其他关联要求已知匹配的检出路径。这是 Git 观测之后的规范成员核对，不是原子文件系统事务，也不证明未知路径别名。

| 转换               | 要求                            | 结果                           |
| ------------------ | ------------------------------- | ------------------------------ |
| 准入               | 有效读取权限、来源与存活 Worker | 捕获来源成员关系及 Worker 代际 |
| 观测               | 不持有 coordinator／registry 锁 | 单个有界只读 Git 任务          |
| 复核               | 来源成员关系与 Worker 代际不变  | 关联当前已知 Workspace 身份    |
| 来源或 Worker 变化 | 不复用旧观测                    | 明确失败                       |
| 超时、取消或断开   | Worker 继续拥有物理清理         | 不修改、不推断成功             |

严格请求／结果与二进制帧校验、Rust／TypeScript 共享样例、来源子目录／链接检出、拒绝信任、来源／Worker 变化及真实 Windows CLI 查询提供证据。旧查询保持独立覆盖。

## 验证

维护中的测试覆盖严格语法与确认、调用者身份与 cwd、部分结果及诊断转义、操作／交换关联、EOF 后不重放、授权及容量、脱离等待后的复用、Close 前拒绝脏目录、Close 后移除失败与显式恢复，以及 Site 关闭前取消。真实可丢弃的 Windows Git 夹具覆盖来源子目录、主检出打开、分支复用、强制移除、分支保留和成功 hook 改变所选分支。Rust 与 TypeScript 消费同一 Worktree 一致性语料；JSON Schema 覆盖结构校验，绑定实现字节限制与跨字段关系。这些是同作者测试和复查，不宣称独立审查或 TLA 执行。

## Workspace 组关闭

`workspace close <target> --group --confirm` 关闭已捕获的 worktree 组，不删除 Git checkout 或分支。已知主 checkout 的 Workspace 若有关联成员，普通 Close 会返回 `workspace-group-close-required`；关联 checkout 的 Workspace 即使带 `--group` 也只关闭自身。成员关系来自已观测的 checkout/common-directory 事实、精确的规范 Directory 和 Worker 实例，不能依赖标签或任意路径前缀。Create 和 Open 同时保留源 Workspace 的已观测 checkout，支持位于 checkout 内部的主 Directory。Directory 改变后该观测失效。精确匹配主根目录的 Directory 可加入已知仓库组；仓库身份歧义必须拒绝。

每个子操作提交前，组还要求 Close 维护 owner 可用；缺失时返回可恢复的接纳失败，即使空 Workspace 也不会被接纳。

Server 在接纳时冻结最多 128 个成员，最后关闭选定的源 Workspace；每个尚未提交的成员交给现有 Workspace Close owner 前，都重查其 Directory 和 Worker。之后创建的 Workspace 不会悄悄加入。每个子 Close 保留自己的身份、代次和生命周期事实。只有已保留组成员的令牌才能绕过普通主工作区保护；线上协议没有绕过标志。失败保留所有已关闭、处理中和尚未提交的成员。Git 与规范关闭不构成事务，已关闭终端不会复活。

`workbench.workspace-group-close` 请求包含 Start、Read 和代次围栏保护的 Resume。结果使用独立于交换身份的操作身份，报告捕获的 Project/Workspace 标签以及必须存在、可为 null 的精简子 Close 记录。记录始终保留已接纳的请求 ID；必须存在、可为 null 的 `observation` 在观测后包含尝试代次和状态。已接纳但未观测的子操作不能授权重放或 Resume。分配前请求上限为 16 KiB，结果上限为 512 KiB。最多保留 64 个操作，全局只允许一个 Pending，每个操作最多 64 次尝试。同一 Start 身份及作用域返回保留观测，改变作用域则冲突。Read 不要求 Workspace 仍然存在。记录跨 Client 断连保留，但不跨 Server 重启。

| 状态或事件             | 守卫和转换                                    | 可观测事实                                |
| ---------------------- | --------------------------------------------- | ----------------------------------------- |
| Start                  | 生命周期授权、可用 owner、当前源/Worker、容量 | 子操作提交前冻结精确的有序成员            |
| 子操作接纳             | 精确的保留成员及当前规范源                    | 等待前保存一个子 Close ID                 |
| 子操作 Closed          | 已知 Close 墓碑                               | 推进下一成员，源最后关闭                  |
| 子操作受阻或接纳拒绝   | 停止序列                                      | Failed 保留部分成员；已知安全失败可恢复   |
| 结果未知或源改变       | 不再提交另一 Close                            | 不可恢复失败，保留原子操作事实            |
| Resume                 | 精确失败代次且明确允许恢复                    | 新尝试沿用原成员及子操作身份              |
| Read、超时或调用者退出 | 读取不推进状态；调用者只结束等待              | 保留 owner 独立继续                       |
| Site stop              | Close 维护冻结前停止并加入组提交者            | 不再接纳子操作，已有 Close owner 负责结算 |

`workspace close-status --group --request <operation-id>` 在任意成员消失后仍可读取保留组。`workspace close-resume --group --request <operation-id> --generation <attempt> --confirm` 恢复已知可恢复失败。Close 和 Resume 支持分离观测及有界等待；取消等待不取消已接纳的关闭。独立的 `orchbee.workspace.group-close` version-1 文档投影这些事实，不改变普通 Workspace Close 输出。原生执行仅限 Windows；Desktop 命令暴露单独跟踪。

正确性证据将成员选择映射到确定性的主/关联/重复/源改变用例，将有序续跑映射到部分失败/重放/代次轨迹，将传输映射到 Rust/TypeScript/schema 共享样例及有界帧测试。真实 Windows 轨迹必须证明终端关闭而 Git 路径仍在，以及显式 Site-stop 加入。这些是有界的同作者证据，不暗示文件系统原子性、独立审查或 TLA 执行。参考语义为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `workspace_close_indices` 和 `handle_workspace_close`；焦点/事件投影仍由独立 Client 合同管理。
