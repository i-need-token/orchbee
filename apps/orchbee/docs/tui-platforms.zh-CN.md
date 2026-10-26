# TUI 平台边界

## 所有权与共享行为

原生 Client 在 Windows、Linux 和 macOS 使用同一套 TUI 实现。`workbench_client`
负责共享 Workbench 状态、命令准入和视图投影。输入分发、导航、操作菜单、设置草稿、
Agent 读取、保留终端内容、复制模式搜索、pane 布局与渲染均在所有支持的平台编译。
Server 仍逐项授权请求；编译可达不代表 Server 或 Host 实现了请求的副作用。

`tui_capabilities` 负责组合层使用的不可变宿主能力快照。快照显式传入 Client 视图与原生 UI。
纯状态和协议模块不含操作系统门控，也不探测环境、文件系统或进程。
旧 Unix 专用紧凑渲染器与 context 点击路径已移除；所有平台使用相同的完成帧命中几何、
释放所有权和共享交互规则。

`agent_runtime` 拥有共享 Agent 进程解释、观察事实、发送结果、请求身份和取消等待。
原生启动消费其类型化执行端口；CLI 适配器复用同一组事实。当前 Windows 执行适配器保留
已有的有界 query／send 桥，Unix 启动适配器则在连接、预留、viewer 或输入准入前拒绝。
纯启动工作流及其回执推理仍参与编译。

`local_client::cancellation` 私有地负责观察任务、事件观察、已保存机器观察及选择尝试的
单次取消。在线程启动前完成创建；创建失败走调用方既有的不可用结果。所有者先发布
`0 -> 1` 再通知，并存活到取消和 join 完成。线程完成仍通过 join／结果通道传递。
消费者借用 readiness 来源，不读取清空或重置它。Collector 先取消所有活跃及已退休任务，
再 join 任何任务。

Unix 适配器向真实非阻塞管道写入一个保持可读的字节，并保留原生 FD poll。
管道已满时保持就绪；被中断的写入只有有限重试预算。终止性写入失败或通知锁中毒时，
关闭唯一写端，使阻塞中的读取方收到唤醒故障，并在 join 前结算。Windows 适配器保留
精确的期望字节匹配；CLI 独立的 `0/1/2` 信号生命周期不变。借用的 CLI 信号描述符和
既有复制／链接 lane 的 wake 原样传递。readiness 检查采用不消费数据的零等待 poll：
可读代表取消，只有 HUP／错误／无效事件代表唤醒故障，没有就绪则仍在等待。
阻塞中的传输等待直接使用原生 wake，不以周期性的 Unix 取消轮询或新增传输线程替代。
既有观察冷却期与 SSH guard 退休调度保留原有节奏。

## 当前适配

| 职责                                                         | Windows                           | Linux 与 macOS               |
| ------------------------------------------------------------ | --------------------------------- | ---------------------------- |
| 继承终端准入、raw mode、输出、信号与指针捕获                 | 保留控制台适配                    | 既有 POSIX TTY 适配          |
| Workbench 协议、本地选择、pane 布局、菜单、帮助与 Agent 观察 | 共享 Client 实现                  | 共享 Client 实现             |
| 共享偏好草稿与已确认设置                                     | 共享偏好协议与存储                | 共享偏好协议与既有 Unix 存储 |
| 键盘复制模式导航、选择与搜索                                 | 共享 Client 实现                  | 共享 Client 实现             |
| 系统剪贴板写入                                               | 隐藏的 Win32 所有者               | 不支持                       |
| 浏览器链接激活与已保存机器 SSH 通道                          | Windows Shell 与自有 OpenSSH 进程 | 不支持                       |
| 已保存机器目录、记忆位置与首次引导关闭状态存储               | Windows 私有缓存                  | 不支持                       |
| 本地目录输入与 Agent 启动／提供方探测                        | 既有 Windows 适配                 | 不支持                       |
| 旧 TOML 配置与插件注册表持久化                               | 既有 Windows 适配                 | 不支持                       |
| 宿主窗口标题、终端／系统通知传输与终端图形                   | 既有 Windows 适配与宿主准入       | 不支持                       |
| 通知卡片与普通终端响铃                                       | 共享 TUI／已准入终端              | 共享 TUI／已准入终端         |
| 登录 Shell 偏好                                              | Windows Shell 适配不支持          | 既有 POSIX Shell 适配        |

可选能力列表不控制整个 TUI。尤其是缺少已保存机器适配时，本地 Workspace／Agent 侧栏
仍可使用，其观察结果不会被反复退休。Unix 本地 Client 不读取 Windows 目录，也不伪造
空的持久化目录。共享偏好保留既有平台支持，与旧 Windows TOML 相互独立。
本地激活在所有平台保留已准入的运行时目录。记忆位置存储只在 Windows 适配器内推导
缓存路径；Unix 适配器返回类型化 `PositionStorage` 拒绝，不读取 Windows 环境变量，也不伪造路径。

激活恢复反馈在所有平台都是同一个可选的类型化事实，由同一套 TUI 提示和 CLI 警告投影消费。
Windows 读取器保留激活代次检查与 I/O 失败。Unix 读取器不探测存储，直接报告能力不支持；
激活继续执行且没有恢复提示。能力缺失不会掩盖受支持读取器的 I/O 失败，也不会伪造已恢复或空检查点。

## 准入与不支持结果

不支持的可选操作不出现在菜单与快捷键发现中。无目录输入能力时，侧栏省略新建工作区。
缺少启动适配器时，空 Agent 看板提供刷新和本地化的不支持说明，不呈现启动表单。无剪贴板时，复制模式导航与搜索仍可使用；
复制按钮和复制提示不显示。UI 在调用时重新校验能力，因此陈旧或显式输入的操作会通过
既有非捕获通知所有者显示 `en-US` 或 `zh-CN` 平台不支持提示。
设置把登录 Shell 能力作为草稿输入，呈现和编辑使用同一能力。

适配层在打开文件、启动进程或写入剪贴板／窗口状态前独立拒绝不支持的副作用。
`io::ErrorKind::Unsupported`、类型化浏览器错误、剪贴板 `Outcome::Unsupported`
和注册表 `PluginRegistryError::Unsupported` 保留确定的无副作用结果。
缺失支持不会变成成功或提交结果未知，不要求重放，也不会把复制选区当作已成功投递而清除。
既有已接受插件操作的歧义结果仍不得继续启动浏览器。

共享运行时在所有平台暴露设置／manifest 类型、命令结果类型、Windows 路径语法和字面量
命令编码。Windows 路径按 Windows 线协议语法解析，不使用宿主 `std::path` 语法；
执行所有者提供冻结的环境和当前目录输入。这不增加 Unix 下的 Windows 路径文件系统访问、
命名会话注册、插件安装、SSH、剪贴板或浏览器实现。

## 验证所有权

纯状态、渲染、输入、能力拒绝和回执结算轨迹在所有支持的平台运行。真实 Win32、进程和
私有缓存证据留在 Windows 适配层。Client crate 不再有覆盖整个 macOS 的 `dead_code` 豁免。
删除不可达实现及其伴随内容，不把退休渲染器保留为只用于测试的副本。
Linux 启动与 peer-publication 进程验证共用一个有界 Workbench peer 辅助模块；
fixture 准备不导入另一组场景的测试模块。
取消证据保留既有在途读取／不重试及退休 join 验证，并补充 readiness 缺口：独立借用者
持续可读、借用 wake 故障、通知失败关闭以及 Windows 精确信号字节匹配。
Unix FD 验证仍需要在原生 Unix 上运行。

在每个原生平台运行 `cargo check -p orchbee --all-targets --all-features --locked` 和
`cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings`，
设置 `CARGO_BUILD_JOBS=6`。仓库 gate 仍由[验证标准](../../../docs/zh-CN/VERIFICATION.md)
管理：Windows 使用 `pnpm verify:rust-native`；Linux 与 macOS 使用适用的宿主 gate。
外来目标编译只提供编译证据。

无法使用原生 runner 时，保留精确 revision、未执行命令和所需人工交接。原生宿主证据记录
OS、模拟器／版本、几何、locale、主题和能力，覆盖共享导航、菜单、调整大小和复制模式搜索，
以及缺失适配的本地化拒绝。远程连接失败或 Windows 检查都不能证明 macOS 或 Linux 通过。
[TUI 验证](../../../docs/zh-CN/tui/verification.md)管理证据类别。

<!-- documentation-pairing: english-sha256=6343ee586580484997a1e02e2006b26ac77e3ab6529d6e180e2baa82863dc437 zh-cn-sha256=013b2a6aeafd8ffe04c098cfd8ebdab59e00b039d159cfa47e3fba5d4ff5129d -->
