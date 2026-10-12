# Workspace 终端目录

状态：本地协议及运行时切片已实现；原生 Client UX 与公开交付仍不在范围内。产品决策人在当前开发会话中接受了此切片。
本文拥有 Server 的 Directory 关联及 Workspace 终端创建行为；
[Workbench 权威](workbench-authority.zh-CN.md) 拥有账本及规范提交机制。

## 结果与范围

已准入的本地 Client 显式把空且未配置的 Workspace 关联到选定本地 Worker 上的目录。
Worker 验证目录后 Server 才接受关联。后续该 Workspace 的 `tab.create` 使用已接受的路径和
确切 Worker；缺少配置或 Worker 不可用时绝不回退 home。普通独立终端启动不变。首个消费者是
本地协议及进程测试，不是新增 TUI 或 Desktop 界面。

Navigation 拥有按身份排序的全局 `Directory { directoryId, workerId, path }` 记录池。
Workspace 必须包含可为 null 的 `primaryDirectoryId`，引用该全局池；另以必需且可为 null 的
`projectId` 表示可选分组。Project 仅含身份和标签，不拥有 Directory 或终端资源。不同 Project
或未分组的 Workspace 可引用同一 Directory。重新分组和解散 Project 保留 Directory 身份、
引用、Worker 放置及 cwd。未使用的 Directory 记录仍有效。

新建 Workspace 未配置且为 Empty。Directory 使用既有非零 128 位身份和 Site 分配游标。
全局池最多接纳 65,536 个 Directory，同时受 1 MiB snapshot 上限约束；历史上每个 Project
256 个 Directory 的上限不再限制当前准入。路径沿用已授权本地平台 cwd 契约：绝对 UTF-8、
1 至 4,096 字节、无 NUL，包括已准入的 Windows 驱动器绝对路径。
保留路径拼写，验证时跟随符号链接；不隐含 realpath 身份、枚举、Git
发现或目录创建。

显式 `workspace.primary-directory.accept` 在既有命令请求 ID、Site 和预期修订下携带 Workspace ID
和路径。Server 选择当前本地 Worker；Client 不提供 Worker generation、fence 或 lease。
首个操作原子创建一个全局 Directory 并将其选为 Workspace 主目录，包括未分组 Workspace，
仅允许设置一次且上下文必须为空。此命令不选择或去重既有 Directory 记录。重绑、删除和通用
Directory CRUD 不在此操作范围内。除导航权限外，
还需独立的目录关联授权；只读 Snapshot 不授予关联或启动权限。

## 准入与转移

| 阶段               | 输入与守卫                                                                             | 结果及所有者影响                                                                    |
| ------------------ | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| 准入               | 授权、可用权威、精确重放/冲突、账本容量、Site/修订、确切 Workspace、空上下文且无主目录 | 明确拒绝保留规范状态；精确重放直接返回保留结果，不重新验证                          |
| 准备验证           | 存在兼容的当前 Worker 及有界请求槽                                                     | 捕获确切 Worker 身份/generation 和路径；I/O 前释放 registry 和 Site 锁              |
| Worker 观察        | 精确请求/generation；在该 Worker 查目录元数据                                          | 返回关联的有效、无效目录、不可用或错误 Worker 结果；无 Host、shell 或规范变更       |
| 重新验证           | 载体截止前验证成功                                                                     | 按 registry 后 Site 顺序重新加锁；重复准入并确认捕获的 Worker generation 仍为当前值 |
| 准备提交           | 身份、Directory 数量、修订、Snapshot 编码和账本字节均可接纳                            | 所有规范变更前准备完整投影及固定结果                                                |
| 提交               | 所有准备检查成功                                                                       | 原子追加 Directory、设置主目录、推进一次修订并保留不可变接受结果                    |
| 结果丢失           | 提交可能已发生                                                                         | Client 不盲目重放效果；精确命令重放返回原保留接受结果                               |
| Workspace Tab 创建 | 确切上下文解析到所属 Workspace 和已配置的全局 Directory                                | 派生显式 cwd 和确切 Worker 约束；使用既有 Create 机制联合提交成员及终端意图         |

不同命令竞争配置同一 Workspace 时不能都提交。并发精确请求可在任一提交前重复执行只读验证，
但仅提交一次逻辑关联。负面/不可用验证及过期 Worker generation 不消耗规范身份、修订或保留账本
记录。验证后的修订冲突不静默重基。关联成功为 `workspace-primary-directory-accepted`，携带修订、
Directory ID 和 Worker ID。缺配置为 `workspace-directory-unavailable`；已配置或非空上下文为
`workspace-directory-conflict`；无效/不可用目录观察为 `directory-unavailable`。载体/Worker 不可用
继续使用既有类型化 unavailable 结果。

Workspace Create 不读、替换、完成或通知独立终端 first-Open 槽。成员 binding 通过既有已授权
Snapshot 和 OBCS AttachBinding 暴露。既有 pending/final Create 账本、不可变完成 token 和未知
结果规则继续有效。目录在关联后被移除会产生如实的启动失败，并保留已提交成员；绝不再尝试从
home 启动。关联只验证一个时间点，不保证将来可访问。Server 准入/结果截止时间不使任意文件系统
调用变为可中断；进程声明仍限于支持的本地 Linux 文件系统 cohort。不新增跨 generation 的重启/
接管保证。

## 正确性与证据

Server 是 Directory 身份、选择、修订和保留命令结果的唯一写入者。Worker 仅拥有 cwd 点时验证及
既有启动准入；Client 两者都不拥有。关联测试须覆盖两个 Client、并发验证、修订变化、Worker
替换、结果丢失/重放、资源耗尽及至多一次提交。Workspace Create 测试须覆盖确切规范 Worker/cwd、
联合提交、pending/final 完成和独立槽不变。既有独立终端行为及验证义务仍为必需。
这些流程不保留独立 TLA 模型或 cwd-validation 模型配置。

严格 schema 及独立 Rust/TypeScript binding 必须拒绝悬空 Directory 或 Project 引用、重复身份、
缺失必需可空字段、无效路径和越界。跨 Project 分组的 Directory 引用是有效的；历史检查点迁移
仍须在扁平化前校验旧的各 Project 所有权，见[检查点契约](workbench-checkpoint.zh-CN.md#规范化导航与历史迁移)。
确定性及状态化测试须将每个准入/重新验证分支映射到保留或提交状态的
结果。进程测试须覆盖真实目录、两个已授权观察者、Workspace Create 和 AttachBinding、独立槽不变、
缺失/被删除目录及 Worker 丢失且不从 home 启动。纯核心测试本身不证明文件系统或载体行为。

| 声明                                                          | 可执行证据                                                                                                                                                                                                                                                                | 限制                                                                                                                                                          |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 并发关联、精确重放及重新验证保留唯一规范接受结果              | [权威目录测试](../src/workbench_authority/tests/workspace_directory.rs)、[service 目录测试](../src/local_terminal_service/workspace_directory/tests.rs)                                                                                                                   | 有界请求 flight；仅内存 Site                                                                                                                                  |
| 全局 Directory 准入及编码投影、身份、修订和账本限制保持原子性 | `former_project_directory_limit_is_allowed_and_encoded_capacity_still_refuses_atomically`、`association_capacity_failures_preserve_canonical_state_and_ledger`、`exact_association_replay_survives_both_full_ledger_limits`                                               | 第 257 个 Directory 可接纳；传输上限可先于全局数量上限触发拒绝                                                                                                |
| Workspace Create 使用规范 placement 并保持独立终端所有权      | [Site Create 测试](../src/site_coordinator/tests/tab_create.rs)：`workspace_create_uses_canonical_worker_and_cwd_without_touching_standalone`、`workspace_completion_does_not_settle_a_pending_first_open`、`workspace_completion_preserves_a_settled_first_open`         | 独立终端 Empty、Creating 和 Settled 情形；不含迁移或恢复                                                                                                      |
| Worker 回执保持确切请求关联及载体停用                         | [严格 cwd 协议测试](../../../crates/capability-node-protocol/tests/cwd_validation.rs)及[carrier 测试](../src/worker_obcn_carrier/tests.rs)：`carries_an_exact_cwd_validation_on_the_existing_serialized_wire`、`cwd_validation_result_requires_the_complete_request_echo` | 既有队列及载体截止时间；不声明文件系统调用可中断                                                                                                              |
| Host 启动前可以重复验证                                       | [`validation_before_first_launch_uses_real_socket_and_never_creates_host_runtime`](../../orchbee-worker/src/terminal_operations/lazy/tests.rs)                                                                                                                            | 真实 lazy Worker 循环和 Unix socket，使用 fixture cohort；未启动 shell                                                                                        |
| 两个 Client 观察唯一关联并使用真实 Workspace shell            | [`real_server_worker_workspace_directory_association_and_tab_lifecycle`](../tests/client_process/workspace_directory.rs)                                                                                                                                                  | 真实本地 Server、Worker 及预启动 Host；缺路径重试、确切 cwd marker、AttachBinding/Detach、独立终端互不干扰、删除路径后失败且保留成员关系、Worker 丢失后不回退 |

Unix socket 与 Linux shell 见证需要对应原生主机及已构建的进程前置产物。
Windows 主机门禁或交叉目标编译不会执行这些见证。

lazy 循环回归区分 `server_transport::send` 的 Host 关闭标志与 Server 写入错误：尚无 Host 时，
成功的验证回执必须让 Worker 保持可用，以接收后续验证或 Launch。连续两次 probe 直接验证该边界。

## 交付与兼容性

协议、Server 和 Worker 作为一个未发布本地 cohort 一同启用。不导入已发布 Desktop 状态，也不切换
旧写入者。目录配置保留及版本 1–5 到规范化版本 6 的严格迁移遵循[检查点契约](workbench-checkpoint.zh-CN.md)。
仅关联成功不能证明重启或全新终端启动。远程 Worker、SSH、安装和公开发布仍为独立工作。
回退恢复先前未使用的 cohort，不能丢弃已接受执行效果或复活已终止进程。交付后本文继续保留于
Server，作为持续维护的行为和证据所有者。
