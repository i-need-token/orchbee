# Local Site Runtime

供 Client、Server 和 Worker 共用的私有 Unix 原生运行时设施。本库负责相对目录描述符的运行时准入、
锁与标记原语，以及不可变开发 cohort 校验。应用负责进程启动、服务状态和终端授权。

## 本地启动模型

Linux 裸命令 `orchbee` 先准入继承的输入/输出 TTY 并校验几何尺寸，再创建运行时或启动进程。
默认使用 `/tmp` 下的用户作用域 `orchbee-native-<effective-uid>`；
`ORCHBEE_RUNTIME_BASE` 可指定另一个已存在的绝对基目录用于隔离开发。
运行时目录必须属于有效用户且权限为 0700；锁文件必须是权限 0600、单链接、属于该用户的普通文件。
拒绝已有的无效路径，不修复、不跟随符号链接、不删除也不替换。

托管 Linux 边界只准入 ext4 系列与 tmpfs 对象。每个打开的最终目录、cohort 产物、标记和锁
都必须没有 access 或 default POSIX ACL：按描述符检查文件系统与 ACL，只有 ENODATA 表示不存在。
不移除 ACL。Overlayfs、XFS、btrfs、DrvFS、网络文件系统和非 Linux 公共启动不在支持范围内。
首次并发创建要求 umask 保留所有者权限位（例如 0022 或 0077）；
限制所有者权限的 umask 可能暴露尚未完成初始化的目录，此时安全失败。

Client 从 TTY 准入到首个有效 Host 帧应用到投影，共用一个 20 秒绝对启动截止时间。
连接、OBLC、初始 OBCS 请求、就绪轮询和首帧等待均消耗同一预算。
超过 500 毫秒显示本地化进度提示；正常快速启动保持安静。
这是协作式截止时间，不涵盖内核/文件系统阻塞与进程暂停。超时只关闭 Client 连接。

| 状态 / 观察                                          | 动作和下一状态                                              |
| ---------------------------------------------------- | ----------------------------------------------------------- |
| 已准入的兼容 Client socket                           | 复用该连接；不启动进程                                      |
| 明确不存在/拒绝连接，激活锁被占用                    | 在原截止时间内等待并重试连接                                |
| 取得激活锁                                           | 启动前重新检查 socket 和生命周期                            |
| 无标记、无 socket 残留、生命周期锁空闲               | Client 独占创建 `activation.started`，随后只启动一次 Server |
| 标记存在，生命周期锁被持有                           | 等待兼容 Server + Worker 就绪                               |
| 标记存在、生命周期锁空闲；或无标记但生命周期锁被持有 | RecoveryRequired；不启动                                    |
| socket / cohort / 权限无效，不兼容或准入结果不明确   | 明确失败；无回退                                            |
| 创建标记后启动失败                                   | 保留标记；本次和后续尝试均需显式恢复                        |
| Server + Worker 就绪                                 | 释放激活锁，提交一次独立 Open，附加 viewer，决定 controller |
| 截止前应用首个有效帧                                 | 将同一继承 TTY 和已准备 Session 交给 TUI                    |
| Client 退出 / 到期                                   | 释放 Client 资源；保留 Server、Worker、Host 和终端          |
| Worker / Host 丢失                                   | 不可用或结果未知；不重启、不接管、不重放                    |

`activation.started` 是不可变的空普通文件，只有一个链接，属于有效用户，权限为 0600。
Client 是唯一写入者，以目录描述符相对的 O_EXCL、CLOEXEC 和 NOFOLLOW 创建。
它提交的是启动尝试，并非 PID 或成功报告。激活 guard 覆盖该提交、进程启动和 OBLC 就绪。
只有 Server 持有生命周期锁，绑定 Worker 监听器，启动恰好一个同 cohort Worker，
并在 Worker 注册后才发布 Client 监听器。Client 监听器先绑定私有临时名称、设置权限，
再以原子且不覆盖目标的 rename 发布；Client 不会观察到权限设置前的状态。
Worker socket 权限完成后才启动 Worker。Worker 等待自己新启动 Host 的 socket 达到精确 0600
后才进行唯一一次握手；已有端点仍立即失败。Server 使用 `setsid` 创建新会话；
Client 退出不会向子进程发信号或杀死它们。子进程所有者回收退出状态而不终止保留服务；
回收逻辑不授予重启权限。

Worker 注册时不启动 Host。授权内核先准入首个 Launch、准备启动事实，再在私有 `host`
目录中启动一个精确 cohort Host。无效授权返回已有类型化结果且不启动 Host。
已准入 dispatch 和内核跨 Host 启动保留；Host 交接时重新校验授权。
准备或启动失败在可交付时结算交接前失败并退出 Worker。任何已有 Host 端点均明确要求恢复。
不允许重新连接或接管 Host。

## 开发 cohort 契约

`cohort.json` 最大 16 KiB，采用严格 JSON，包含 `schemaVersion: 1` 和恰好四个
`artifacts` 字段，规范顺序为：`orchbee`、`orchbee-server`、
`orchbee-worker`、`orchbee-terminal-host`。值为完整可执行文件的 64 字符小写 SHA-256。
目录属于有效用户且权限为 0700。文件必须是单链接、属于用户的普通文件，
组及其他用户不可写；可执行文件需要所有者执行位。清单和可执行文件均相对于已校验目录
以 NOFOLLOW 和 CLOEXEC 打开。每个托管 Rust 角色（Client、Server、Worker）都校验全部四个产物，热启动也如此。
Host 消费已验证的相邻可执行文件，不实现清单读取。

精确 cohort 身份是以下 UTF-8 字节的 SHA-256 小写形式：
`orchbee-native-cohort-v1\n`，随后按上述固定顺序连接每个产物名、
一个 NUL 字节、其十六进制哈希和 LF。JSON 键顺序不影响身份。
相邻文件路径只来自这些固定名称。缺失、修改、混装、格式错误或不安全产物在激活前被拒绝。
显式端点测试模式保留原版本 cohort，不获得托管启动权限。

暂存脚本构建并复制四个可执行文件到新建私有目录，最后写入清单。
这属于未签名、单用户的开发暂存；发布签名、安装器、同用户恶意修改以及校验到执行之间的
竞态均不在范围内。清单确定一致的精确产物集合，不证明发布者身份。

## 正确性案例

| 声明                                            | 所有者 / 细化映射                                    | 证据                                          |
| ----------------------------------------------- | ---------------------------------------------------- | --------------------------------------------- |
| LS01 每个运行时生命周期至多一次启动尝试         | Client 在激活 guard 下独占创建标记                   | LocalSiteStartup 模型；竞争及失败标记进程轨迹 |
| LS02 无隐式恢复或 Client 所有的终止             | 持久标记、Server 生命周期 guard、Client 连接释放     | 模型丢失/超时；断连再打开真实进程轨迹         |
| LS03 授权 Launch 前没有 Host；保留同一 dispatch | Worker 授权内核和延迟启动适配器                      | 模型授权；Worker 无效/有效 Launch 进程测试    |
| LS04 副作用前角色一致且 rendezvous 私有         | NativeCohort 和 ModeCheckedLocalSiteRendezvous       | 清单损坏、权限、符号链接及已有锁测试          |
| LS05 一个预算内收到有效首帧才算启动就绪         | Client 截止时间、TerminalSession 投影、同一 TTY 交接 | 准入/提交截止测试及裸 TTY 进程测试            |

`formal/tla/LocalSiteStartup.tla` 的边界为两个 Client、一次 Server 启动尝试和一次 Host 尝试。
它将文件系统检查抽象为已准入事实，将截止时间观察抽象为到期事件，在无调度公平假设下检查安全性。
每个 Client 的超时公平性只支持有界尝试终止，不保证服务可用。
Linux 测试覆盖真实内核锁、peer、哈希、PTY 和取消。
远程、Workbench 成员关系、守护进程重启、自动清理/恢复、Server 丢失后的持久化，
以及非 Linux 公共启动均不属于本契约。

## POSIX 本机 Site rendezvous 底座

crate-private 的 `rendezvous` module 向 Client、Server 和 Worker 提供经过 mode 检查的
Unix rendezvous primitive。Installation composition 必须提供稳定的
`SiteScope`；cohort version 与 digest 不属于该 scope，因此 update 会保留同一个 Site identity，
release、local 与 checkout-scoped development installation 则可以彼此隔离。`SiteScope` 接受
`[a-z][a-z0-9]*(?:-[a-z0-9]+)*` canonical form 的 1 到 63 个 ASCII byte，因此不能加入 path
separator、dot component 或依赖平台的 encoding。

对于一个可信、已存在且为 exactly normalized absolute 的 per-user runtime base，确切 derived
path 是：

| 角色               | 稳定路径                                         |
| ------------------ | ------------------------------------------------ |
| Scope directory    | `<runtime-base>/<site-scope>`                    |
| Activation lock    | `<runtime-base>/<site-scope>/activation.lock`    |
| Site lifetime lock | `<runtime-base>/<site-scope>/site-lifetime.lock` |
| Client socket      | `<runtime-base>/<site-scope>/client.sock`        |
| Worker socket      | `<runtime-base>/<site-scope>/worker.sock`        |

`ModeCheckedLocalSiteRendezvous` 为其他 Unix target 保留 mode-checked 名称。Preparation 只会在
scope directory 不存在时创建它。Preparation 与 validation 把 `/` 作为 trusted root 打开，然后用
`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC` 逐个 descriptor-relative 地打开 runtime-base component 与
受控 scope。它们既不 canonicalize，也不让 operating system 对传入 base 执行 normalization。除 `/`
本身外，可接受 base 只含一个 leading slash、由单个 slash 分隔的一个或多个 nonempty normal
component，不含 trailing slash、`.` 或 `..` component，也不含 NUL byte。Malformed base 在 trusted
root 之外的 filesystem access 发生前被拒绝；symbolic-link 或 non-directory component 会在该
component 处被拒绝。保留的 scope 必须是 effective UID 拥有且 exact mode 为 `0700` 的 directory。
新 scope 会在不 follow leaf 的情况下接受检查，验证其 directory type 与 effective-UID ownership，在
依赖 permission 的 open 之前以 descriptor-relative 方式改为 `0700`，然后仅在打开 descriptor 的
device 与 inode 和 chmod 前检查的 entry 一致时才 admission。即使 process umask 移除了全部请求的
permission，这也能在不改变 process umask 的情况下达到 exact `0700`。已有的错误 mode 会被拒绝且
不会修复。

每项 lock operation 都通过保留的 directory descriptor，以
`O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC` 只打开或创建固定 leaf。Leaf 必须是 effective UID 拥有且
exact mode 为 `0600` 且 hard link 数恰好为一的 regular file。已有且有效的 stale file 会被复用，
不会被 truncate 或 replace。`try_acquire_activation_lock` 与 `try_acquire_site_lifetime_lock` 各自只执行一次
nonblocking exclusive kernel lock attempt，并返回 owned guard 或 `Contended`；它们不会 wait、
retry 或推断 process health。File creation 与 advisory-lock acquisition 是分开的：返回 `Contended`
的 caller 可能先创建并规范化了此前缺失的 durable lock inode，之后另一个 descriptor 才赢得 lock。
Contended attempt 绝不会 truncate、replace、unlink 或改变 existing inode 的 ownership，也不会修复
pre-existing invalid entry。Drop guard 只释放其 advisory lock。该 module 绝不移除 runtime
directory、durable lock file、socket path 或这些 path 上的任何 entry。

该 primitive 在其他 Unix 系统上仍只检查 mode。Linux 在目录、锁、标记和 cohort 准入前加入上述
受限文件系统与无 ACL 边界；托管消费者执行 socket 元数据及同用户 peer 校验。
Client、Server 和 Worker 共享本 crate，Server 拥有生命周期 guard。
Primitive 自身不选择应用启动策略、不重试、不移除陈旧端点，也不授予恢复权限。

### 建模与证据

| 字段   | Rendezvous primitive 契约                                                                                                                                                                                                                           |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 提供共享路径、权限、锁和尝试标记准入；应用拥有启动、就绪和恢复策略。                                                                                                                                                                                |
| 输入   | 可信且已存在、精确规范化的绝对 base、规范 scope、有效 UID，以及可能无效的目录、锁、标记或 socket entry。                                                                                                                                            |
| 数据   | 稳定路径、保留的 scope 描述符、拥有的 advisory-lock guard 和不可变空尝试标记。不推断 PID 或进程健康。                                                                                                                                               |
| 输出   | 已校验 rendezvous 或类型化失败；Acquired/Contended 锁结果；经过元数据校验的标记和 socket 存在性。路径 getter 不执行 I/O。                                                                                                                           |
| 不变量 | 描述符相对的 no-follow 遍历；最终类型、所有者、mode 和普通文件链接数精确匹配；新目录身份与检查过的 inode 相同。Linux 最终对象通过受限文件系统及无 ACL 检查。已有无效 entry 不修复、不删除。Guard drop 只释放锁。                                    |
| 决策   | 规范路径与 scope 语法、新建或已有元数据、锁类型、一次非阻塞锁结果和独占标记创建决定结果。                                                                                                                                                           |
| 模型   | 不包含应用重试或进程观察的原子 POSIX primitive。LocalSiteStartup 将其事实与 Client/Server/Worker 状态组合；LocalSiteActivation 保留早期的抽象 connect-first 模型。                                                                                  |
| 转移   | 缺失 scope/lock 变为已校验的 durable inode。锁创建与锁获取可以由不同调用方获胜。Guard 获取/drop 只改变锁所有权。缺失标记只能提交一次，且永不清除。                                                                                                  |
| 依赖   | 安全的 nix/rustix 文件系统封装和 File::try_lock；Linux 基于 fd 的文件系统及 ACL 查询。Rendezvous 不依赖子进程或服务。                                                                                                                               |
| 所有权 | 本 crate 拥有派生和准入。Client 选择开发 base/scope，拥有激活 guard 和标记提交；Server 拥有生命周期 guard。                                                                                                                                         |
| 验证   | Scope/path 分区、符号链接/硬链接/FIFO 拒绝、错误所有者/mode、限制性 umask 初始化、保留所有者位的并发创建、陈旧 inode 保留、竞争/重新获取、标记排他性和 socket 冒充。Linux ACL 测试在保留精确 0700/0600 mode 的同时增加命名访问 ACL 或目录默认 ACL。 |
| 限制   | 单个有效用户；错误所有者测试注入 expected UID。信任内核元数据与 advisory lock。并发创建要求保留所有者位的 umask。Durable 表示不 unlink/truncate，并非 fsync。协作式截止时间排除内核阻塞和进程暂停。模型不证明文件系统或内核语义。                   |
