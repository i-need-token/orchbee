# Windows Worktree 操作

本记录拥有原生 CLI Worktree 创建、打开、移除与恢复契约。Git 副作用由 Windows Worker 拥有；Server 拥有保留操作与规范 Workspace 关联。CLI 等待观测，绝不自行运行 Git。`worktree list` 保持既有的已注册目录查询契约。

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

`orchbee.worktree.operation` 版本 1 封套注册于 `api schema` 版本 20（55 个文档）。它包含操作 ID、已知时的本地 Site 身份、相关联的 Server 回答或明确的不可用／不确定观测，以及独立且可空的等待状态。Pending 与 detached 观测退出 0；终端启动失败保留自身非零退出码；Git 或关联失败退出 3，接纳不可用退出 4，观测断线退出 5，内部或不一致观测退出 1 且保留最近回答，超时退出 7，物理／提交不确定退出 8，取消退出 130。解析与提交前目标失败使用既有 CLI 失败封套。有界 Git 原始诊断在 JSON 中保留为字节数组，本地化文本中进行转义。

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

## 验证

维护中的测试覆盖严格语法与确认、调用者身份与 cwd、部分结果及诊断转义、操作／交换关联、EOF 后不重放、授权及容量、脱离等待后的复用、Close 前拒绝脏目录、Close 后移除失败与显式恢复，以及 Site 关闭前取消。真实可丢弃的 Windows Git 夹具覆盖来源子目录、主检出打开、分支复用、强制移除、分支保留和成功 hook 改变所选分支。Rust 与 TypeScript 消费同一 Worktree 一致性语料；JSON Schema 覆盖结构校验，绑定实现字节限制与跨字段关系。这些是同作者测试和复查，不宣称独立审查或 TLA 执行。
