# Local Site Runtime

供 Client、Server 和 Worker 共用的私有原生运行时设施。本库负责相对目录描述符的运行时准入、
锁与标记原语，以及不可变开发 cohort 校验。应用负责进程启动、服务状态和终端授权。

## 共享用户偏好存储

`UserPreferencesStore` 负责[共享契约](../../schemas/workbench/README.zh-CN.md#共享用户偏好)
定义的有版本、有界设备偏好记录。作用域由精确配置路径冻结；配置文件名派生的私有目录让共用父目录的
配置仍然隔离。读取不创建文件；非法或不可读记录直接拒绝。更新取得操作系统持有的写锁，比较已观测
来源，只应用提供的叶字段，验证完整候选，再刷新并原子替换。重置与编辑一样声明对应键的归属。
Windows 复用固定祖先的私有缓存边界；Unix 使用基于目录描述符、不跟随链接的私有文件，并在替换后
同步所在目录。替换后的失败报告结果未知，绝不授权自动重放。

旧 Windows TOML 中显式配置的值为尚未声明的偏好提供初值，不修改旧文件；首次观测包含其来源指纹。
Desktop Main 与 renderer 分别持有一次性导入回执，按任意顺序只填充尚未声明的键；后到的导入不能
撤销编辑或重置。应用负责 UI 草稿、轮询、向运行中终端发布，以及进程启动。桥接程序和原生 Site
消费同一个存储所有者，而不是各建一份存储。

## 共享启动与通知策略

`default_interactive_launch` 负责纯平台 Shell 选择及原生可执行文件准入，
由 Desktop 匹配辅助程序和 Native Worker 共用，不启动进程。
偏好存储一起捕获滚动历史与选定启动配置；显式可执行文件绕过默认 Shell 解析，
仍采用共享滚动历史默认值。POSIX 登录模式必须显式选择，Windows 拒绝开启。
共享存储无效时，在 Host 交接之前让该操作失败。

`NativeNotificationSettings` 负责可移植的有界投递与声音策略。
Windows 配置解码为旧 profile 输入保留别名；TUI 观察与卡片调度消费可移植类型。
通用设置确认后覆盖旧投递与全局声音策略。
原三字段 TOML 设置写入器已退役；旧 TOML 保持只读迁移输入。

## Windows 私有帧目录

`WindowsPrivateFrameDirectory` 在已准入的私有运行时下拥有一个临时
`graphics-<32-hex-nonce>` 子目录。`reserve` 固定祖先目录但不创建该子目录；`create`
独占创建子目录并写入应用提供的有界归属记录。创建失败后，同一明确的所有者继续负责，
直到 `reap` 确认目录已不存在。既有子目录不被接管、修复或删除。应用负责生产者准入、
nonce 生成、保留、重试和关闭；本设施不启动进程或清理线程。

只有已准入的目录能读取帧。路径必须是该目录下的一个精确直接子文件，文件名符合 Windows
普通文件规则；拒绝遍历、备用数据流、保留名称、重解析点、硬链接、非普通文件和外来 ACL。
读取句柄不共享写入／删除权限，核对声明长度，最多复制 16 MiB，并确认 EOF。不可变副本
不再依赖文件。生产者提交前必须关闭写入句柄。
例如 `.frame.rgba` 这样以点开头的图像文件名是普通叶子名称；精确的归属标记仍为保留名称。

生命周期为 Reserved → Preparing → Ready → Releasing → Released。准备失败仍由同一
所有者转入 Releasing。回收关闭根目录固定句柄，核对保存的目录身份，并在祖先仍被固定时
仅删除由自身推导的已拥有子目录。忙碌或被重定向的目录继续保留责任供重试；对象析构不证明
已清理。应用租约保持 Ready，直到最后一个读取者结束。Windows 句柄轨迹覆盖有界读取、
私有准入、写入／重命名排斥、创建拒绝和删除失败重试。同一用户恶意修改 ACL、进程崩溃及
最终删除系统调用期间的替换不在本逐用户开发边界内；崩溃不证明运行时干净关闭。

## Windows 字面命令编码

纯 Windows 命令编码器由 CLI Agent 启动和 Server 重启共用。它生成有界 UTF-16LE Base64 脚本，参数仅以一个字面数组保存。原生程序、batch 和 PowerShell 脚本分派保留参数边界，包括空值、引号和百分号。它排除别名／函数、拒绝控制字符、将编码文本限制为 30,000 字符，并拒绝超过 8,000 字符的 batch 命令行。应用选择程序、shell、交互开关及更严格的传输上限；本库不启动进程。调用方保持 PowerShell 打开时，辅助函数及错误首选项仍局限在子作用域中。内嵌 PowerShell 资源与编码器及 CLI 的真实 Windows argv 测试共同维护。

## Windows 用户路径操作数

`WindowsUserPathOperand::parse` 准入有界的盘符绝对路径、主目录相对路径或当前目录相对路径语法，不读取进程上下文，也不探测文件系统。不透明操作数保留原始写法；`resolve` 仅使用最终执行方提供的环境变量或当前目录。绝对路径字面值不需要这两类能力。所需主目录值无效时，不回退到低优先级主目录来源。`normalize_windows_user_path` 仍是已拥有执行上下文的调用方可用的组合入口。应用负责完成语法校验、路由，以及决定何时允许展开路径。

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
Active、Crashed、携带非零停止身份的 Stopped 或 Deleting。清单 v2 引入 Server、Worker、Host
各自的 PID 和创建 FILETIME；v3 保留同一精确登记，并准入三槽位检查点布局及自有损坏原件。
清单 v4 增加共享比例，v5 增加可选 Project 组织及全局 Workspace 和 Directory 身份。
当前读取器接受 v1–v5，但 v1 Active 没有自动死亡证明。效果路径从已准入输入推导，
不直接信任记录中的路径。
配置的运行时基目录仍须存在；删除已停止运行时的叶目录不会移除清单记录或检查点。读取合并运行时及
持久条目，不创建或迁移文件。损坏、身份不符、硬链接或重定向记录均明确失败，不进行修复。

固定基目录的命名空间门覆盖默认及命名会话激活。运行时缺失时，有效停止记录、空闲检查点锁与端点缺席
共同允许在原路由重建。Stopped v1/v2 要求主检查点；Stopped v3–v5 可使用任一固定槽位或精确命名的
自有损坏原件。Active v2–v5 仅在确切登记身份全部消失、登记完整、所有门可独占且全部派生端点缺席
时允许恢复。恢复先发布旧代 Crashed，再协调其标记，保留检查点并开始新代。存活／未知身份、不完整登记、
旧版 Active、待删除状态和孤立存储均拒绝恢复。激活先原子发布 Active v5，再提交启动标记、
拉起 Server 或发布检查点存储。每次激活带有新的非零 128 位身份；Server 在 Worker 效果前保留该精确代次，确认清理结束后发布 Stopped，随后
才发布运行时停止回执并释放生命周期。发布失败不能证明正常停止。

删除在临时意图或任何数据移除前发布持久 Deleting；先清理检查点/运行时及临时意图，最后移除持久记录。
中断的清理在临时元数据消失后仍可发现。发布使用有界私有暂存、刷新和原子替换。这是普通进程/失败恢复，
不保证掉电持久性。每条记录的 writer lock 串行化直接子进程登记、恢复与正常停止发布。
`spawn_server` 要求持有清单及匹配的激活所有权；`spawn_windows_site_child` 要求 Worker／Host
的确切已登记直接父进程。子进程挂起创建，身份刷新成功后才允许运行。最多支持 v1–v4 的读取器拒绝 v5，
即使会话已正常停止也一样；不提供降级。活 Host 接管仍另行实现。

[Windows 崩溃恢复合同](docs/windows-crash-recovery.zh-CN.md) 规定门、发布中断、兼容和证据边界。
使用以下命令检查已登记的有界模型：

```sh
node scripts/tla/run-models.mjs --models windows-site-crash-recovery
```

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

Linux 与 macOS 裸命令 `orchbee` 先准入继承的输入/输出 TTY 并校验几何尺寸，再创建运行时或启动进程。
Linux 默认使用 `/tmp` 下的用户作用域 `orchbee-native-<effective-uid>`，macOS 使用 `/private/tmp`；
`ORCHBEE_RUNTIME_BASE` 可指定另一个已存在的绝对基目录用于隔离开发。
运行时目录必须属于有效用户且权限为 0700；锁文件必须是权限 0600、单链接、属于该用户的普通文件。
拒绝已有的无效路径，不修复、不跟随符号链接、不删除也不替换。

托管 Linux 边界只准入 ext4 系列与 tmpfs 对象。每个打开的最终目录、cohort 产物、标记和锁
都必须没有 access 或 default POSIX ACL：按描述符检查文件系统与 ACL，只有 ENODATA 表示不存在。
macOS 边界只准入 APFS，并通过锚定于已打开描述符的 ACL 检查拒绝所有扩展 ACL。两者都不移除 ACL。
Overlayfs、XFS、btrfs、DrvFS、网络文件系统和其他 Unix target 不在支持范围内。
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
组及其他用户不可写；可执行文件需要所有者执行位。Linux 要求目录和打开的文件没有 POSIX ACL；
macOS 还要求 APFS 且每个打开对象都没有扩展 ACL。清单和可执行文件均相对于已校验目录
以 NOFOLLOW 和 CLOEXEC 打开。每个托管 Rust 角色（Client、Server、Worker）都校验全部四个可执行文件，热启动也如此。
Host 消费已验证的相邻可执行文件，不实现清单读取。

根工作区的开发与测试 profile 以优化级别 3 编译现有 `sha2` 依赖，使较大的调试可执行文件能高效完成哈希。
应用代码保留正常的 debug profile。cohort 校验仍消耗调用者的原始截止期限。

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

以下 LS 声明描述 Linux 与 macOS 开发启动边界。受管 Windows 冷恢复由独立的
[代与进程登记契约](docs/windows-crash-recovery.zh-CN.md)负责。

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
Linux 与 macOS 测试覆盖真实内核锁、peer、哈希、PTY 和取消。
远程、Workbench 成员关系、守护进程重启、自动清理/恢复、Server 丢失后的持久化，
以及其他 Unix target 均不属于本契约。

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

Linux 会按准入的本机文件系统与 POSIX ACL 契约校验最终对象；macOS 会检查 APFS，并要求其
runtime base、scope、锁文件、标记、cohort 目录及已打开的 cohort 产物均没有扩展 ACL。

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
受限文件系统与 POSIX ACL 边界；macOS 对 runtime base、目录、锁、标记及打开的 cohort 产物执行 APFS
与无扩展 ACL 校验；托管消费者执行 socket 元数据及同用户 peer 校验。
Client、Server 和 Worker 共享本 crate，Server 拥有生命周期 guard。
Primitive 自身不选择应用启动策略、不重试、不移除陈旧端点，也不授予恢复权限。

### 建模与证据

| 字段   | Rendezvous primitive 契约                                                                                                                                                                                                              |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 提供共享路径、权限、锁和尝试标记准入；应用拥有启动、就绪和恢复策略。                                                                                                                                                                   |
| 输入   | 可信且已存在、精确规范化的绝对 base、规范 scope、有效 UID，以及可能无效的目录、锁、标记或 socket entry。                                                                                                                               |
| 数据   | 稳定路径、保留的 scope 描述符、拥有的 advisory-lock guard 和不可变空尝试标记。不推断 PID 或进程健康。                                                                                                                                  |
| 输出   | 已校验 rendezvous 或类型化失败；Acquired/Contended 锁结果；经过元数据校验的标记和 socket 存在性。路径 getter 不执行 I/O。                                                                                                              |
| 不变量 | 描述符相对的 no-follow 遍历；最终类型、所有者、mode 和普通文件链接数精确匹配；新目录身份与检查过的 inode 相同。Linux 与 macOS 最终对象分别通过受限文件系统及无 ACL 检查。已有无效 entry 不修复、不删除。Guard drop 只释放锁。          |
| 决策   | 规范路径与 scope 语法、新建或已有元数据、锁类型、一次非阻塞锁结果和独占标记创建决定结果。                                                                                                                                              |
| 模型   | 不包含应用重试或进程观察的原子 POSIX primitive。LocalSiteStartup 将其事实与 Client/Server/Worker 状态组合；Client 实现测试覆盖 connect-first 行为。                                                                                    |
| 转移   | 缺失 scope/lock 变为已校验的 durable inode。锁创建与锁获取可以由不同调用方获胜。Guard 获取/drop 只改变锁所有权。缺失标记只能提交一次，且永不清除。                                                                                     |
| 依赖   | 安全的 nix/rustix 文件系统封装和 File::try_lock；Linux 基于 fd 的文件系统/POSIX ACL 查询及 macOS APFS/扩展 ACL 检查。Rendezvous 不依赖子进程或服务。                                                                                   |
| 所有权 | 本 crate 拥有派生和准入。Client 选择开发 base/scope，拥有激活 guard 和标记提交；Server 拥有生命周期 guard。                                                                                                                            |
| 验证   | Scope/path 分区、符号链接/硬链接/FIFO 拒绝、错误所有者/mode、限制性 umask 初始化、保留所有者位的并发创建、陈旧 inode 保留、竞争/重新获取、标记排他性和 socket 冒充。Linux ACL 测试及 macOS 扩展 ACL 测试分别在 mode 位精确时验证拒绝。 |
| 限制   | 单个有效用户；错误所有者测试注入 expected UID。信任内核元数据与 advisory lock。并发创建要求保留所有者位的 umask。Durable 表示不 unlink/truncate，并非 fsync。协作式截止时间排除内核阻塞和进程暂停。模型不证明文件系统或内核语义。      |

## Windows 细化

`RetainedChild::terminate_before` 仅显式终止准确持有的进程，并在协作式期限内确认其退出。
它不终止后代进程。托管启动只在 Worker 尚不能创建 Host 时使用此操作；之后的清理由 Worker
关闭协议负责。丢弃 `RetainedChild` 仍只释放观察所有权，不终止任何进程。

`WindowsSessionStore` 在 `<LocalAppData>/Orchbee/native-sessions/<canonical-runtime-endpoint-digest>`
下保留有界检查点字节。调用者传入 LocalAppData 目录及已准入 Site，Server 拥有检查点校验和恢复。
不同运行基目录及命名作用域各自隔离存储。沿用私有祖先句柄、新目录的受保护 ACL、拒绝重解析点
及文件单链接检查。私有 `state.lock` 在存储所有者存活期间独占持有；竞争者收到 `WouldBlock`。

存储保留三个带版本及完整性校验的槽位，以私有 generation 排序。Server 提供完整文档验证；
选择最新有效 payload。无效副本保持原样，并产生明确的回退／空状态启动提醒。旧的普通
`checkpoint.json` 仍可读取。缺失数据与无效数据分别表达；文件系统／私有路径准入失败仍拒绝启动。
Server 的致命校验错误（包括历史迁移超过未改变的 Snapshot 字节上限）拒绝启动，不进行损坏回退或槽位写入。
checkpoint v6 可写入前先发布 catalog v5，因此最多支持 v1–v4 的读取器会在产生效果前拒绝升级后的组织语义。
准入保留升级后的字节，绝不降级 catalog。
详见 [Windows checkpoint 存储契约](docs/windows-checkpoint-storage.zh-CN.md)。

发布先写入并 flush 一个私有 `checkpoint.pending`，将被替换的无效原件保留到新的自有 raw 文件名，
再在同一目录中以 `MoveFileEx(REPLACE_EXISTING)` 原子替换槽位。pending 字节不能提供恢复权限；失败
保留此前有效槽位，并报告未确认的暂存清理。payload 未变化不消耗历史。raw 原件属于用户恢复数据，
直到显式删除 Site 或授权修复才移除。私有 ACL 排除其他用户，同用户写入者遵守独占锁。适配层不声称
目录项 write-through；已确认的 pending 文件 flush 及原子替换不证明断电或硬件故障时的持久性。

存储层不解释 JSON、不选择默认数据目录、不重新启动进程、不重放操作，也不授予 Site 重启权限。
Server 拥有此 primitive 的启动／停止用法；原生进程崩溃／重启及机器重启见证仍是独立证据义务。

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

共享纯函数 `windows_reported_directory` 解码并保留有效终端上报的驱动器／UNC 观察；调用方单独执行更窄的启动路径约束。`windows_powershell_initialization.ps1` 在原提示符之后同步 PowerShell 进程 cwd，保留状态且不改变非文件系统 provider。普通 Worker shell 与编码恢复 shell 使用同一钩子，以及子进程内的 UTF-8 原生输出解码与向外发送的管道编码。

`windows_interactive_powershell_initialization` 将该公共资产与 `windows_powershell_reader.ps1`
连接，后者包装既有交互 reader 并发送语义读行阶段标记。Worker 为已准入的普通 PowerShell argv
准备这一精确启动命令；Host 构建按同样顺序读取这两个资产，用于注入和精确识别。
编码启动／恢复帮助函数只使用公共初始化，不安装 reader 钩子。两个资产均不写 profile，也不改变
外部 PowerShell 进程。实际 5.1／7 管道、prompt 与 PTY 启动证据覆盖托管路径；任意启动脚本不在范围内。

reader 准入要求标准 PSReadLine 模块／导出身份和已识别的定义指纹；自定义或不受支持的定义保持
原样，状态为 unknown。重复或跳过 reader 安装不会中止外围初始化。现代适配器在输出标记前保留
上条命令状态，再将其与原模块上下文传给原生 ReadLine；旧 reader 保留原双参数行为。

`WINDOWS_DIRECTORY_PROMPT` 是冻结的旧 argv 指纹，绝不是当前启动源码。
`windows_legacy_powershell_initialization_matches` 精确识别该历史目录脚本与上一版 UTF-8／目录
脚本。CLI 保留 shell 准入将此兼容谓词与当前共享启动命令组合；任意后缀与额外 argv 仍被拒绝。
新初始化与编码帮助函数只消费所属资产。

## Windows 用户设置

同一读取器拥有 `WindowsUpdateChannel`（`stable`/`preview`，默认 stable）及 default/file 来源。文件值非法时仅让该叶字段回退并进入警告记录；已记录的环境边界仍为致命错误。原生 `channel show` 离线读取有效偏好，不推断已安装的发布身份，也不执行更新器。

共享有界设置读取器和纯 Windows 用户路径适配器服务于 CLI 配置、公共 Client 激活和直接托管 Server 启动，负责致命文件／路径／环境准入、逐字段回退到已有安全默认值、恢复策略解析及确定性警告记录；应用负责协议投影、激活提交和私有子进程参数。读取结果把有效设置与诊断保存在一起。诊断在显示截短前去重，按字段和原因排序，最多 128 条并携带精确省略数量，且绝不保留来源值或命令。参见[配置与优先级](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md)。仅 Windows 的固定版本 `toml` 依赖在此文件边界解析。读取器不依赖 Workbench。

纯函数 `windows_user_settings_path` 为伴随资源暴露相同配置文件位置，不打开文件或应用设置。它保留显式 `ORCHBEE_CONFIG_FILE` 优先于 `%APPDATA%/Orchbee/config.toml` 的规则；缺少根目录时仍返回缺失。Server 使用其父目录定位检测规则输入。

## Windows 私有更新缓存

`WindowsPrivateCache` 为 Server 识别缓存更新拥有固定的私有祖先目录、不跟随链接的单硬链接普通文件，
以及跨进程写入锁。应用仍拥有逻辑文件名、内容校验、版本和策略。仅在已准入私有父目录下创建缺失子目录；
不安全的已有数据直接拒绝，不修复。有界读取保留不存在的事实。替换先刷盘完整暂存文件，再原子替换准入叶文件。
保留名 `.orchbee-update.lock` 是持久缓存协调对象；`.orchbee-update.pending` 是唯一自有暂存叶，
在收尾后删除，或在下一次独占锁下恢复。清理失败向调用者返回，不转为发布成功。
Server 更新器是真实消费者；此适配器本身不代表已实现远程更新。

`read_existing` 固定并验证已有私有目录树，进行有界叶文件读取，不创建目录、锁文件或暂存文件。它服务于启动及重载时的状态检查；缺失缓存路径仍保持缺失。

Windows 设置也接纳 `[update] manifest_check`（默认 true），保留默认／文件来源及显式位置状态。非法的受支持叶字段独立回退；非法按键动作保留为显式禁用覆盖，同一动作里的合法 chord 仍会保留。`read_windows_user_settings_at` 在冻结路径读取完整候选，重新应用严格的恢复环境覆盖，保留默认缺失／显式缺失的差别且不创建文件。应用仍拥有设置／诊断原子发布、Server 权威启动基线及自动调度策略。

## Windows 快捷键配置修复

Windows 设置所有者提供有界离线快捷键恢复，保留配置语义，使用独占文件准入、已刷盘的私有备份和明确恢复结果。应用负责 CLI 回执与本地化；[配置契约](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md#恢复自定义快捷键) 负责策略、失败行为和备份生命周期。库不重载或启动进程。

同一键位归属模块为 Client 在线发布编码和校验规范 `[keys]` 配置，上限 64 KiB、每动作 144 个展开触发键。用户文件仍限制 16 个候选；Server 和 Client 负责发布与安装。

Windows 插件目录边界为原生插件管理提供有界清单解析、规范标识符、平台继承、源可用性及用户私有登记的原子更新。只读不创建存储；源不可用时保留明确标记的检查元数据。解除链接后配置/状态路径保持稳定。共享 Windows 命令所有者为 Worker 命令和离线安装提供有界输出与后代清理；应用拥有执行策略。托管安装通过应用提供的执行器管理串行的源码下载/构建/提交/清理，读取 v1 目录并保留 v2 来源/恢复状态；详见[原生插件契约](../../apps/orchbee/docs/windows-plugins.zh-CN.md)。

## Windows 独立版本存储

`WindowsReleaseStore` 获取已有私有缓存写锁并固定安装目录祖先。暂存区借用该锁，只创建经过验证的可移植路径与独占私有文件，再通过不替换目录重命名发布到小写 SHA-256 标识。应用拥有清单校验、下载、cohort 接纳、启动检查、用户 PATH 激活及保留版本。显式清理只删除自身未发布暂存区；子进程结束未确认时使用 `retain_for_recovery` 并报告保留路径。`open_windows_release_file` 拒绝不安全祖先、重解析点、外部 ACL 和硬链接。确定性 Windows 测试覆盖写入互斥、独立发布、别名拒绝和暂存清理。

`set_windows_update_channel` 是共享显式设置写入器：校验完整当前文件，通过已有固定版本 toml_edit 解析器保留 TOML 语法，串行化配合的写入器，复核外部修改并刷新原子私有替换。可以创建指定的不存在文件，不下载，也不会因调用方更新失败而回滚已保存偏好。

Windows 的 `NativeCohort::artifact_sha256` 保留成功接纳时各程序的摘要。安装来源将其声明摘要与这些观察比较，避免把两次读取间发生变化的 cohort 呈现为先前已接纳的版本。

## Windows 侧栏呈现配置

共享配置所有者分别接纳 `ui.sidebar.agents` 与 `ui.sidebar.spaces` 布局。
token 样式／原始值首条匹配规则共用接纳所有者；各配置族包装器拒绝不支持的 token 和非文字规则。
每个封闭规范 profile 独立限制 32 KiB。Workspace 布局使用 rows/row_gap；
Agent 布局保留 Provider 覆盖。启动、CLI 检查及 Site 重载共同消费该所有者。
参见[用户指南](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md#windows-workspace-侧栏布局)。

## Windows 交接管道身份

显式 Worker 交接按已有准入 runtime 的哈希和非零 128 位票据推导独立同用户命名管道。
`handoff_endpoint_path` 不创建文件系统叶子，也不授予任意路径授权。应用负责票据验证、目标
cohort 准入、备用租约及进程／Server 所有权；普通 client／worker／Host 端点沿用既有身份。
原生管道测试验证 scope／票据地址隔离，以及长 Unicode runtime 输入下仍有界的名称。
[Worker 契约](../../apps/orchbee-worker/docs/windows-server-handoff.zh-CN.md) 负责连接转移。

## Windows 显式配置导入

Windows 用户设置模块通过现有叶值／profile 准入准备稀疏 herdr 候选配置，发布前重新接纳整份序列化结果。`PreparedWindowsHerdrImport` 将 TOML 限制为 64 KiB，并持有不含值的 128 项报告及精确省略数。显式文件参数准入拒绝 Windows 数据流／设备别名。读取器取得一份有界、普通、非重解析且单链接的快照，不共享写入／删除权限。写入器固定现有祖先，要求末级父目录私有，并独占创建、保护、写入并同步一个新文件；创建后失败时保留该文件，由调用方负责恢复。不查找配置、不启动进程、不重载，也不覆盖、删除或重试。Orchbee CLI 负责语法、语言、回执、预览和文件选择指引；参见其[导入约定](../../apps/orchbee/docs/windows-cli-configuration.zh-CN.md#herdr-配置导入)。
