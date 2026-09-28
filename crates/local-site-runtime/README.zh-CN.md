# Local Site Runtime

供 Client、Server 和 Worker 共用的私有原生运行时设施。本库负责相对目录描述符的运行时准入、
锁与标记原语，以及不可变开发 cohort 校验。应用负责进程启动、服务状态和终端授权。

## Windows 字面命令编码

纯 Windows 命令编码器由 CLI Agent 启动和 Server 重启共用。它生成有界 UTF-16LE Base64 脚本，参数仅以一个字面数组保存。原生程序、batch 和 PowerShell 脚本分派保留参数边界，包括空值、引号和百分号。它排除别名／函数、拒绝控制字符、将编码文本限制为 30,000 字符，并拒绝超过 8,000 字符的 batch 命令行。应用选择程序、shell、交互开关及更严格的传输上限；本库不启动进程。调用方保持 PowerShell 打开时，辅助函数及错误首选项仍局限在子作用域中。内嵌 PowerShell 资源与编码器及 CLI 的真实 Windows argv 测试共同维护。

## Windows 命名运行时身份

`WindowsSiteName` 验证区分大小写的精确名称：1–64 个 ASCII 字母、数字、点、下划线或连字符，
排除 `.` 和 `..`。`default` 保持现有按 SID 推导的 scope；其他名称由域分隔符、SID 和精确名称
推导为有界散列 scope。私有 `session-name.json` 保存严格的 version-1 名称记录，上限 256 字节。
只有持有匹配 activation lock 的注册操作可以创建它；现有字节只读取和验证，不覆盖、不修复。
首次写入中断产生的部分内容保持为可观察失败。只读 rendezvous 准入要求命名 scope 具有有效记录，
并验证其当前用户 scope 推导。运行时别名、未知字段、重复字段和不匹配名称均拒绝。
该身份不授予进程控制权，也不授权删除状态。

## Windows 持久会话清单

原生 Windows 命名及默认 Site 在 `LOCALAPPDATA/Orchbee/session-catalog/<runtime-base-digest>`
下保留有界私有生命周期记录。每条记录将精确名称、当前用户 scope 和规范运行时路径绑定到
Active、携带非零停止身份的 Stopped 或 Deleting。效果路径从已准入输入推导，不直接信任记录中的路径。
配置的运行时基目录仍须存在；删除已停止运行时的叶目录不会移除清单记录或检查点。读取合并运行时及
持久条目，不创建或迁移文件。损坏、身份不符、硬链接或重定向记录均明确失败，不进行修复。

固定基目录的命名空间门覆盖默认及命名会话激活。运行时缺失时，有效停止记录、空闲检查点锁与端点缺席
共同允许在原路由重建。Active、待删除状态或缺少停止证据的孤立检查点拒绝恢复。激活先原子发布 Active，
再提交启动标记或拉起 Server。每次激活带有新的非零 128 位身份；Server 在 Worker 效果前保留该精确代次，确认清理结束后发布 Stopped，随后
才发布运行时停止回执并释放生命周期。发布失败不能证明正常停止。

删除在临时意图或任何数据移除前发布持久 Deleting；先清理检查点/运行时及临时意图，最后移除持久记录。
中断的清理在临时元数据消失后仍可发现。发布使用有界私有暂存、刷新和原子替换。这是普通进程/失败恢复，
不保证掉电持久性，也不会自动处理崩溃后仍为 Active 的会话。

## Windows 命名会话生命周期

`WindowsNamedSiteCatalog` 用准入运行时基目录中的私有按用户 namespace lock 串行化默认/命名激活与命名删除。
激活从创建运行时之前持有它，直到就绪。只读枚举不创建任何内容，包含 default，按精确名称排序，
对不安全或损坏项明确失败，不隐藏它们。清单上限为 256 个会话。

清理所有者在文件系统阶段之间检查原期限和调用者的中断谓词。中断报告已发布的意图是否仍然保留；已完成移除不会回滚。

删除拒绝 default、存活或不确定的进程、不安全文件和未知目录内容。只有已准入的停止回执
（或从未激活的运行时）、空闲 lifetime 和缺席端点共同成立后，才发布有界且已刷新的同级删除意图。
该意图阻止普通 rendezvous 准入，部分清理后继续保留。删除者只移除已知检查点与运行时文件，
在移除前释放其准确句柄，保留祖先句柄，先移除临时意图，最后移除持久记录。重试继续该删除，状态缺席视为成功。
不递归删除任意目录树，也不停止进程。共享 namespace lock 有意比单个会话存活更久。
范围限于协作的当前用户调用与普通进程中断；不声称断电耐久性或运行时基目录被外部移除后的清单恢复。

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
Host 端点的绝对地址在 `host` 目录或 Host 进程出现之前先按平台 socket 地址 ABI 检查；
无法容纳的地址以同一交接前失败结算，且不产生这两种副作用。
准备或启动失败在可交付时结算交接前失败并退出 Worker。任何已有 Host 端点均明确要求恢复。
不允许重新连接或接管 Host。

## 开发 cohort 契约

`cohort.json` 最大 16 KiB，采用严格 JSON，包含 `schemaVersion: 2` 和恰好五个
`artifacts` 字段，规范顺序为：`orchbee`、`orchbee-server`、
`orchbee-worker`、`orchbee-terminal-host`、`agent-observation-adapters`。前四个值是各自完整可执行文件的
64 字符小写 SHA-256；第五个声明与可执行文件同目录暂存的观察适配器胶囊的 SHA-256 负载摘要——
即其 `capsule.json` 中 `payloadSha256` 所载的同一值。携带任何其他 `schemaVersion`（包括 `1`）的清单不兼容；
`schemaVersion: 2` 清单缺失必填成员、携带重复或未知成员、或声明格式错误的摘要则为无效清单。
两者都经现有类型化拒绝闭合失败。目录属于有效用户且权限为 0700。文件必须是单链接、属于用户的普通文件，
组及其他用户不可写；可执行文件需要所有者执行位。清单和可执行文件均相对于已校验目录
以 NOFOLLOW 和 CLOEXEC 打开。每个托管 Rust 角色（Client、Server、Worker）都校验全部四个可执行文件，热启动也如此。
Host 消费已验证的相邻可执行文件，不实现清单读取。

`agent-observation-adapters` 相邻目录是惰性准入的 sidecar。`NativeCohort` 在启动期间不打开、不遍历、
不校验负载；仅通过 `observation_adapters_sidecar()` 暴露固定的 sidecar 目录与清单声明的摘要。
首个集成消费者在首次使用时私有校验该目录，并返回类型化的 unavailable 或 invalid 结果；
缺失或损坏的 sidecar 不阻断 Site 启动或 Agent 读取。

精确 cohort 身份是以下 UTF-8 字节的 SHA-256 小写形式：
`orchbee-native-cohort-v1\n`，随后按上述固定顺序连接每个可执行文件名、
一个 NUL 字节、其十六进制哈希和 LF——仅四个可执行文件；适配器声明被有意排除，
使负载更新不搅动准入身份。JSON 键顺序不影响身份。
相邻文件路径只来自这些固定名称。缺失、修改、混装、格式错误或不安全产物在激活前被拒绝。
显式端点测试模式保留原版本 cohort，不获得托管启动权限。

暂存脚本构建并复制四个可执行文件到新建私有目录，将规范的
`apps/desktop/resources/agent-observation-adapters` 胶囊原样复制到 sidecar 目录，校验其严格
`capsule.json` 成员（`contractVersion` 为 3、声明的 `payloadSha256`、必备负载条目，且重算的规范摘要
与声明值一致），最后写入清单。未通过该校验的胶囊会中止暂存，而不是产出坏包。
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

| 角色                | 稳定路径                                                              |
| ------------------- | --------------------------------------------------------------------- |
| Scope directory     | `<runtime-base>/<site-scope>`                                         |
| Activation lock     | `<runtime-base>/<site-scope>/activation.lock`                         |
| Site lifetime lock  | `<runtime-base>/<site-scope>/site-lifetime.lock`                      |
| Client socket       | `<runtime-base>/<site-scope>/client.sock`                             |
| Client pending name | `<runtime-base>/<site-scope>/client-pending.sock`                     |
| Worker socket       | `<runtime-base>/<site-scope>/worker.sock`                             |
| Host scope          | `<runtime-base>/<site-scope>/host`                                    |
| Host endpoint       | `<runtime-base>/<site-scope>/host/00000000000000000000000000000001.s` |

pending name 是 Server 在原子 rename 发布为 `client.sock` 之前使用的瞬态绑定名；Host endpoint
是 Worker 以绝对地址拨号的端点，Host 在 `host` scope 内以相对地址绑定它。pending 叶名、
`host` scope 与 endpoint 文件名是共享常量，派生路径是纯拼接——即使目录无法检查也能回答——
因此每个派生 socket 地址都能在任何目录、marker 或进程副作用之前按平台 `sockaddr_un` 容量
做预算检查。

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

`observe_*` entry 探针把同一组固定名称开放给只读诊断：描述符相对的 `fstatat` 配合
`AT_SYMLINK_NOFOLLOW` 回答每个 entry 的原始 kind、ownership、mode、size 和 link count——
每个判定均由 caller 负责。`observe_host_endpoint` 仅在 `host/` 父级自身校验为拥有的目录后
才向其内部遍历，对其他任何类型一律拒绝而非跟随。`observe_site_lifetime_lock` 绝不创建
leaf：只有先经 stat 检查为拥有的 regular `0600` 单链接文件的 entry 才会被打开，接受一次
`try_lock` 探测，并在同一描述符上释放——`Held` 仅是那一瞬的锁事实，绝非进程断言。所有
观察均不 connect、不创建、不写、不 unlink、不修复，也不推断进程健康。

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

## Windows 细化

`RetainedChild::terminate_before` 仅显式终止准确持有的进程，并在协作式期限内确认其退出。
它不终止后代进程。托管启动只在 Worker 尚不能创建 Host 时使用此操作；之后的清理由 Worker
关闭协议负责。丢弃 `RetainedChild` 仍只释放观察所有权，不终止任何进程。

`WindowsSessionStore` 在 `<LocalAppData>/Orchbee/native-sessions/<canonical-runtime-endpoint-digest>`
下保留有界检查点字节。调用者传入 LocalAppData 目录及已准入 Site，Server 拥有检查点校验和恢复。
不同运行基目录及命名作用域各自隔离存储。沿用私有祖先句柄、新目录的受保护 ACL、拒绝重解析点
及文件单链接检查。私有 `state.lock` 在存储所有者存活期间独占持有；竞争者收到 `WouldBlock`。

`checkpoint.json` 缺失与空文件、超限或不可读文件分别表达，读取受调用者已准入字节限制约束。
发布先检查大小及已有目标，创建 `checkpoint.pending`，写入并刷新完整字节，关闭暂存句柄，
再使用同目录 `MoveFileEx` 和 `REPLACE_EXISTING` 替换。发布前失败保留已提交
文件，清理只移除已准入的自有暂存文件。清理失败明确返回，由下一位独占所有者重试回收。
崩溃遗留的 pending 文件未提交，不能替代已提交检查点。私有 ACL 排除其他用户，同用户写入者
必须遵守锁。文件刷新和替换成功不保证目录项在断电或存储设备故障后仍持久有效；固定版本的
安全 API 不提供可组合的写穿透替换标志。

存储层不解释 JSON、不选择默认数据目录、不重新启动进程、不重放操作，也不授予 Site 重启权限。
Server 启动／停止集成及实际重启证据齐备前，不声称已支持会话持久化。

Windows 原生适配保留激活锁和生命周期锁归属；下文的显式停止记录细化原有不可变尝试标记。
绝对本地路径必须先拒绝重解析点，并保留不允许共享删除的目录句柄。
运行时父目录和末级目录必须归令牌用户所有，DACL 只能向该用户、SYSTEM
或 Administrators 授权。新建目录设置受保护且可继承的 DACL；已有不安全
对象直接拒绝，绝不修复。命名管道地址由规范运行时路径派生，默认 scope 以令牌 SID 为键；
Host 标识使用相同路径摘要，避免不同 Site 冲突。连接准入仍证明对端令牌用户。
管道存在只是参考事实，不证明就绪或进程健康。Windows 可执行文件使用精确
的 `.exe` 同级路径，并遵循现有 cohort 清单摘要规则。子进程使用原生分离创建标志并禁止句柄继承，在 Client 断开后继续由 Site 持有。

安全的 `windows-permissions` 依赖负责 `std` 无法表达的描述符和 ACL 查询。
`winapi-util` 提供基于句柄的硬链接数，`winsafe` 提供禁止继承的进程创建及精确等待/退出观察。
其 WinAPI FFI 和旧版 bitflags 依赖仍位于自有 unsafe 代码禁令之外；不引入
异步运行时或通过 Shell 探测安全属性。原生测试覆盖 ACL 拒绝、硬链接及联接点
拒绝、标记独占、锁争用、端点隔离和 cohort 篡改。这是自行审查的细化，并非
独立审查；挑战场景包括已有宽松 ACL、路径替换、重复启动和管道冒充。

Windows 只读诊断复用准入，不创建 scope 或文件。标记仍须为普通、单链接、私有归属且包含合法激活记录。激活锁只检查、不加锁。生命周期观察仅打开已有且通过准入的文件，
尝试一次非阻塞独占锁后立即释放；absent/free/held/unsafe 是观察结果，不是服务健康。
探测可能短暂与启动争用；失败不授权修复或删除。父目录无效时不检查后代。
测试覆盖零创建、free/held 转换、非法标记及硬链接拒绝。

### Windows 显式停止激活记录

Windows 停止细化将仅不可变启动尝试的解释改为同一个已准入 `activation.started`
文件中的有界记录。空字节表示 Active；精确 ASCII 前缀 `orchbee-stopped-v1:` 后接
非零、32 字符的小写十六进制停止请求 ID 表示 Stopped。缺失独立表达。其他长度、
前缀或编码均无效，不授予重启权限。Linux 保留不可变空标记契约。

托管 Server 保留已有 Active 文件句柄并禁止删除共享。只有其生命周期所有者，
在确认精确终端收敛、Host 退出和 Worker 退出后，才能完成该记录。完成操作定位、
写入有界记录、设置长度并刷新数据；部分 I/O 不算成功。保留的所有者只能重试同一个
已尝试 ID，拒绝其他 ID；已完成的相同 ID 幂等。文件系统停顿不在协作式时间保证内，
崩溃撕裂或非法记录失败关闭。不删除或替换路径，也不以回执证明无关进程已退出。

启动在现有激活／生命周期守卫下，只能消费合法 Stopped 记录，将同一个已准入文件
截为 Active 后执行唯一一次 Server 启动。该边界后失败仍要求恢复。后续启动消费旧
停止回执，因此没有回执不能否定更早的停止。旧 cohort 拒绝 Stopped 字节，重启前须升级。

决策分区为缺失／Active／Stopped／非法、相同／不同停止 ID、首次完成／重试，以及
完成前／后启动。runtime 测试证明严格解析、空与非法数据、完成身份和重启循环；
托管生命周期与进程证据须先证明退出前置条件，公共停止命令才能宣称完成。此记录原语
本身不停止进程、不获取生命周期权限，也不自动恢复。

## Windows 目录原语

共享纯函数 `windows_reported_directory` 解码并保留有效终端上报的驱动器／UNC 观察；调用方单独执行更窄的启动路径约束。`WINDOWS_DIRECTORY_PROMPT` 在原提示符之后同步 PowerShell 进程 cwd，保留状态且不改变非文件系统 provider。普通 Worker shell 与编码恢复 shell 使用同一钩子。

## Windows 用户设置

共享有界设置读取器和纯 Windows 用户路径适配器服务于 CLI 配置、公共 Client 激活和直接托管 Server 启动，负责严格文件／类型／路径准入和恢复策略解析；应用负责协议投影、激活提交和私有子进程参数。参见[配置与优先级](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md)。仅 Windows 的固定版本 `toml` 依赖在此文件边界解析。不引入可变缓存、写入器或 Workbench 依赖。

纯函数 `windows_user_settings_path` 为伴随资源暴露相同配置文件位置，不打开文件或应用设置。它保留显式 `ORCHBEE_CONFIG_FILE` 优先于 `%APPDATA%/Orchbee/config.toml` 的规则；缺少根目录时仍返回缺失。Server 使用其父目录定位检测规则输入。

## Windows 私有更新缓存

`WindowsPrivateCache` 为 Server 识别缓存更新拥有固定的私有祖先目录、不跟随链接的单硬链接普通文件，
以及跨进程写入锁。应用仍拥有逻辑文件名、内容校验、版本和策略。仅在已准入私有父目录下创建缺失子目录；
不安全的已有数据直接拒绝，不修复。有界读取保留不存在的事实。替换先刷盘完整暂存文件，再原子替换准入叶文件。
保留名 `.orchbee-update.lock` 是持久缓存协调对象；`.orchbee-update.pending` 是唯一自有暂存叶，
在收尾后删除，或在下一次独占锁下恢复。清理失败向调用者返回，不转为发布成功。
Server 更新器是真实消费者；此适配器本身不代表已实现远程更新。

`read_existing` 固定并验证已有私有目录树，进行有界叶文件读取，不创建目录、锁文件或暂存文件。它服务于启动及重载时的状态检查；缺失缓存路径仍保持缺失。
