# Workspace 终端目录

状态：本地协议及运行时切片已实现；原生 Client UX 与公开交付仍不在范围内。产品决策人在当前开发会话中接受了此切片。
本文拥有 Server 的 Directory 关联及 Workspace 终端创建行为；
[Workbench 权威](workbench-authority.zh-CN.md) 拥有账本及规范提交机制。

## 结果与范围

已准入本地 Client 将空且未配置 Workspace 关联到所选 Worker 上的目录。Worker 在 Server 接受前校验路径。之后 Tab／Panel 创建使用准确已接受路径及 Worker；缺少配置或 Worker 不可用不回退到 home。Workspace 可未分组，仍独立拥有 Context 及目录。

Navigation 拥有按身份排序的全局 `Directory { directoryId, workerId, path }` 记录池。
Workspace 必须包含可为 null 的 `primaryDirectoryId`，引用该全局池；另以必需且可为 null 的
`projectId` 表示可选分组。Project 仅含身份和标签，不拥有 Directory 或终端资源。不同 Project
或未分组的 Workspace 可引用同一 Directory。重新分组和解散 Project 保留 Directory 身份、
引用、Worker 放置及 cwd。未使用的 Directory 记录仍有效。

新建 Workspace 未配置且为 Empty。Directory 使用既有非零 128 位身份和 会话 分配游标。
全局池最多接纳 65,536 个 Directory，同时受 1 MiB snapshot 上限约束；历史上每个 Project
256 个 Directory 的上限不再限制当前准入。路径沿用已授权本地平台 cwd 契约：绝对 UTF-8、
1 至 4,096 字节、无 NUL，包括已准入的 Windows 驱动器绝对路径。
保留路径拼写，验证时跟随符号链接；不隐含 realpath 身份、枚举、Git
发现或目录创建。

显式 `workspace.primary-directory.accept` 在既有命令请求 ID、会话 和预期修订下携带 Workspace ID
和路径。Server 选择当前本地 Worker；Client 不提供 Worker generation、fence 或 lease。
首个操作原子创建一个全局 Directory 并将其选为 Workspace 主目录，包括未分组 Workspace，
仅允许设置一次且上下文必须为空。此命令不选择或去重既有 Directory 记录。重绑、删除和通用
Directory CRUD 不在此操作范围内。除导航权限外，
还需独立的目录关联授权；只读 Snapshot 不授予关联或启动权限。

## 准入与转移

| 阶段               | 输入与守卫                                                                             | 结果及所有者影响                                                                    |
| ------------------ | -------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| 准入               | 授权、可用权威、精确重放/冲突、账本容量、会话/修订、确切 Workspace、空上下文且无主目录 | 明确拒绝保留规范状态；精确重放直接返回保留结果，不重新验证                          |
| 准备验证           | 存在兼容的当前 Worker 及有界请求槽                                                     | 捕获确切 Worker 身份/generation 和路径；I/O 前释放 registry 和 会话 锁              |
| Worker 观察        | 精确请求/generation；在该 Worker 查目录元数据                                          | 返回关联的有效、无效目录、不可用或错误 Worker 结果；无 Host、shell 或规范变更       |
| 重新验证           | 载体截止前验证成功                                                                     | 按 registry 后 会话 顺序重新加锁；重复准入并确认捕获的 Worker generation 仍为当前值 |
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

创建成员通过已授权快照和 `attach-binding` 暴露。待完成／最终命令结果、不可变完成 token 和未知结果规则仍是权威。关联后路径被删除时报告真实启动失败并保留已提交成员，不随后在 home 再启动。关联只验证一个时间点，不保证未来可访问。期限不能使任意文件系统调用可中断，进程证据限于支持的宿主／文件系统 cohort。

## 正确性与证据

只有 Server 写入 Directory 身份、选择、修订和保留命令结果。Worker 拥有时间点路径校验及启动准入。测试须覆盖两个 Client、并发校验、修订变化、Worker 替换、结果丢失／重放、容量、准确 cwd／Worker、联合提交及最多一次执行。无关 Workspace 和终端必须不变。此流程没有单独保留 TLA 模型。

严格 schema 及独立 Rust/TypeScript binding 必须拒绝悬空 Directory 或 Project 引用、重复身份、
缺失必需可空字段、无效路径和越界。跨 Project 分组的 Directory 引用是有效的。保留检查点仅准入
当前规范化导航，历史的各 Project 嵌套形状不可读，见[检查点契约](workbench-checkpoint.zh-CN.md#当前规范化导航)。
确定性及状态化测试须将每个准入/重新验证分支映射到保留或提交状态的
结果。进程测试须覆盖真实目录、两个已授权观察者、Workspace Create 和 AttachBinding、独立槽不变、
缺失/被删除目录及 Worker 丢失且不从 home 启动。纯核心测试本身不证明文件系统或载体行为。

| 声明                                                | 保留证据                                                                                                                                            | 限制                                  |
| --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- |
| 准入、重新验证和重放保留准确 Directory 所有权       | [权威目录测试](../src/workbench_authority/tests/workspace_directory.rs)及[服务目录测试](../src/local_terminal_service/workspace_directory/tests.rs) | 确定性内存／传输夹具                  |
| Tab 创建使用 Workspace Worker／cwd 并保留无关成员   | [会话 创建测试](../src/site_coordinator/tests/tab_create.rs)                                                                                        | 准确准备与保留操作事实                |
| 两个 Client 观察同一真实 Workspace shell 及失败结果 | [Workspace 目录进程证据](../tests/client_process/workspace_directory.rs)                                                                            | 支持的 Linux cohort；其他平台独立验证 |

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

<!-- documentation-pairing: english-sha256=be40dc717ec5f712db96d851cf25d8cf8497a2db4f73f9b3bc91981c684ace8a zh-cn-sha256=a00cda72403cac5db3ffa36bda2d971023978c384192689ebfb14294005ebb2a -->
