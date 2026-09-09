# 终端启动配置文件

## 现状和范围

Desktop Main 拥有本地进程启动配置文件的一个设备本地目录。该目录控制为新配置的终端捕获的可执行文件和文字参数向量。它不拥有终端 CWD 选择、产品管理的环境策略、终端外观、PTY 状态，或对已持久化 binding 的启动。profile 解析后，Desktop 会在新 binding 捕获完整 launch 前应用有界的[自动 Shell Integration 细化](kitty-automatic-shell-integration-alignment.zh-CN.md)。

本地桌面渲染器可以通过专用沙盒功能端口来管理目录。网关和普通浏览器客户端不具备目录管理功能。首次交付使用普通终端创建的目录默认；为一个创建选择非默认配置文件仍然是单独建模的工作台命令和恢复更改。

## 问题

本地桌面用户需要选择系统 shell 或一个显式可执行文件，并将该选择重新用于新终端。当前的桌面运行时在启动时解析一个进程范围的 shell，并且不提供持久的用户控制、命名替代方案、登录 shell 选项或可见的故障恢复。

在 macOS 上，不经 shell 启动的桌面应用可能只继承精简的进程环境。因此，内置系统 shell 配置文件需要采用平台的登录 shell 启动路径，让新终端获得用户的标准工具链环境，而不由 Desktop 合成 `PATH`。

## 输入和验证

| 输入                         | 验证与缺失（absence）                                                      |
| ---------------------------- | -------------------------------------------------------------------------- |
| 持久目录                     | UTF-8 JSON，有界文件大小，严格的 version-1 模式；缺少意味着内置目录        |
| 配置文件 ID                  | 稳定有界 ASCII 身份；缺少 IDs 拒绝而不改变状态                             |
| 配置文件名称                 | 修剪后的非空有界用户文本；允许重复的显示名称                               |
| 系统 shell 命令              | 使用受信任的启动 shell；仅在 POSIX 上允许登录模式                          |
| 自定义可执行文件             | 非空有界路径，无 NUL；对于桌面平台必须是绝对的                             |
| 自定义参数                   | 没有 NUL 的有序有界文字字符串；没有引用、插值或 shell 解析                 |
| 渲染器命令                   | 在主调度之前由 `@orchbee/desktop-client-protocol` 解析                     |
| 基础（Base）`TerminalLaunch` | 仅在 `TerminalLaunchSchema` 之后才可信；提供不属于此配置文件功能的每个字段 |
| 已解决主机启动               | 必须先编码为一个有界 `create-terminal` 控制请求，然后才能提交目录          |
| 终端呈现（presentation）CWD  | Workbench 持有的既有值；profile launch 解析后仅覆盖 `currentDirectory`     |
| 配置文件存储读取或失败       | 丢失、无效、读取失败和写入失败是不同的预期结果                             |

精确的字符串、参数计数、配置文件计数和文件大小界限是不大于其 Terminal Host 协议限制的实现常量。它们随着合同和边界测试而改变。

## 数据

```text
TerminalLaunchProfileCatalog (version 1)
├── defaultProfileId
└── profiles[] (ordered, one or more)
    └── TerminalLaunchProfile
        ├── id
        ├── name (user text or the localized built-in label marker)
        └── command
            ├── system-shell(login: boolean)
            └── custom-executable(executable, arguments[])
```

内置目录包含一个稳定的 `system-default` 配置文件，并使用本地化标签标记和系统 shell 命令。其登录模式在 Darwin 上启用，在其他所有平台上禁用。该平台默认值仅在目录缺失或无效时，或用户显式提交重置时生效。已存储目录始终具有权威性，绝不会为了跟随后续内置默认值而被重写。持久化的配置文件名称属于用户数据，绝不会被翻译。配置文件 ID（而非显示名称或列表位置）决定身份。

`TerminalLaunchProfileSnapshot` 包含规范目录、其来源（`built-in` 或 `stored`）以及可选的持久性问题。快照是发布到渲染器的唯一目录投影。

## 不变量

- 该目录包含 1 个和配置的最大数量之间的配置文件。
- 配置文件 IDs 是唯一的，并且 `defaultProfileId` 准确命名了一个当前配置文件。
- 配置文件顺序稳定；新建（create）在其源之后插入，编辑与默认选择不会重排配置文件。
- 只有稳定的内置配置文件才可使用本地化标签（localized-label）标记。用户创建的配置文件具有明确的名称。
- 系统 shell 配置文件没有自定义可执行文件或参数。自定义可执行配置文件没有登录 shell 标志。
- 内置系统 shell 在 Darwin 上启用登录模式，在 Linux 和 Windows 上禁用。已存储配置文件在每个受支持平台上都保留其显式登录选择。
- Windows 目录不能包含已启用的登录 shell 命令。
- 删除最终配置文件或当前默认配置文件会被拒绝。用户在删除前一个默认值之前选择另一个默认值。
- 重复的配置文件名称永远不会合并身份或更改选择。
- 配置文件仅更改受信任的基础 `TerminalLaunch` 中的 `executable` 和 `arguments`。
- 自动 Shell Integration 只能在 profile 操作完成后细化已解析 launch；它绝不改变选定 executable 或用户拥有的字面 arguments。
- 在完整目录被准入或提交之前，每个配置文件都会根据受信任的基础进行解析并符合 Terminal Host 控制边界。
- 产品管理的环境条目、终端配置文件、配置、geometry 与基础 CWD 在 profile 解析后保持不变。
- 终端绑定在其第一个供应副作用之前捕获一次完整解析的启动。稍后的配置文件编辑、重试、渲染器重新加载和应用程序重新启动都无法更改它。
- 恢复使用已与绑定一起存储的启动，并且永远不会解析当前目录。

## 命令和结果

| 命令           | 成功                                           | 预期的拒绝或失败                         |
| -------------- | ---------------------------------------------- | ---------------------------------------- |
| 读取快照       | 返回当前的规范快照                             | 控制不可用                               |
| 从配置文件创建 | Main分配一个ID，并在源后面插入命令副本         | 源丢失、目录已满、名称无效或写入失败     |
| 更新配置文件   | 替换该配置文件的名称和命令，而不更改 ID 或顺序 | 配置文件丢失、平台/路径/值无效或写入失败 |
| 选择默认值     | 仅更改 `defaultProfileId`                      | 配置文件丢失或写入失败                   |
| 删除配置文件   | 删除一个非默认配置文件                         | 缺失、最终、默认或写入失败               |
| 重置目录       | 持久化当前平台的内置目录                       | 写入失败                                 |

每个渲染器请求都有一个相关的完成或预期失败。格式错误的消息或意外传输的端口会停用该功能端口。一个端口最多允许一个飞行中的请求；第二个不同的请求接收 `busy` 且不更改任何状态。

## 状态和转换

目录读取只接纳普通文件。POSIX 使用非阻塞打开，使没有写入端的 FIFO 也能进入同一句柄的文件类型检查，并返回 `invalid-record`，而不是等待另一端。此约束不为任意文件系统 I/O 设定截止时间，也不改变符号链接处理方式。

```text
uninitialized
  └── load ──> ready(built-in, no issue)       when missing
           ├─> ready(stored, no issue)         when valid
           └─> ready(built-in, read issue)     when invalid or unreadable

ready(snapshot)
  └── valid mutation ──> writing(candidate)
       ├── atomic store commit ──> ready(stored candidate, no issue) + completed
       └── store failure ────────> ready(previous catalog, write-failed) + failed
```

Main 串行化目录变更（mutations）。权威提交点是成功的原子文件替换。当写入挂起时，终端创建会观察先前提交的目录；提交后，它会观察新目录。端口处置不会取消已开始的原子写入，并且桌面关闭会等待变更尾部。仅所有者模式和临时文件同步在重命名之前完成。重命名后的目录持久性同步是尽力而为的，因为它的失败无法将已经可见的目录真实地回滚到先前的快照。

加载无效或不可读的文件永远不会重写它。稍后显式成功的创建、更新、默认选择、删除或重置命令可能会取代它。语法有效但在启动时不可用的自定义可执行文件在现有终端生命周期中失败；桌面永远不会默默地退回到另一个配置文件。

## 所有权和依赖性

| 责任（Responsibility）                       | Owner                              |
| -------------------------------------------- | ---------------------------------- |
| 目录状态、转换、持久性与 profile launch 解析 | `apps/desktop`                     |
| profile 之后的自动 Shell Integration 细化    | `apps/desktop`                     |
| 沙箱化 wire schema 与固定端口角色            | `@orchbee/desktop-client-protocol` |
| 本地设置呈现和请求客户端                     | `apps/web`                         |
| 产品消息（messages）与翻译                   | `@orchbee/i18n`                    |
| CWD 和终端呈现意图                           | `@orchbee/workbench`               |
| 具体（concrete）持久绑定（binding）的启动    | Desktop terminal binding store     |
| PTY 创建和可执行文件可用性                   | Terminal Host                      |

目录存储仅需要加载和原子替换。控制器需要平台、可信基础启动、稳定的 ID 分配和存储。平台是构造内置回退和执行重置时的显式输入。确定性目录转换和启动解析器不执行文件系统、进程、传输、本地化或 React 工作。

## 验证和可追溯性

| 声明                                             | 可执行证据                                               |
| ------------------------------------------------ | -------------------------------------------------------- |
| Darwin 登录默认值和非 Darwin 默认值              | 平台矩阵目录和 Main 控制器单元测试                       |
| 已存储登录选择保持权威性                         | Main 控制器加载和解析单元测试                            |
| 目录不变量和转换拒绝                             | 确定性目录单元测试                                       |
| 丢失、无效、有界、原子和失败的 I/O               | 使用隔离的临时目录进行文件存储单元测试                   |
| 精确的 profile 细化、Host 编码和平台 login 规则  | resolver 与 Main controller 单元测试                     |
| 自动 Shell 检测、launch 细化、回退与 loader 行为 | 自动 Shell Integration resolver、资源与 Desktop 系统测试 |
| 提交排序和失败保存                               | 主控制器单元测试                                         |
| 线路验证、关联、繁忙和处置                       | 协议、主代理、预加载和渲染器客户端单元测试               |
| 首次供应时捕获；重试/恢复时保留                  | `DesktopTerminalLifecycleAdapter` 单元测试               |
| 仅限本地的能力分配和清理                         | `DesktopApplication` 和桌面 Web 组合单元测试             |
| 本地化管理、验证和故障 UI                        | 设置控制器/组件测试以及双区域确定性故事证据              |
| 真正的新终端上线                                 | 桌面系统测试，不包括手动本机窗口测试                     |

这些测试控制存储结果、请求顺序、IDs、平台和进程启动有效负载。它们不能证明任意用户可执行文件存在或行为正确。本机平台测试仅支持它们运行的​​操作系统；跨平台模式和解析器测试不声明本机进程执行。

## 排除情况

版本 1 不包括环境覆盖和机密、合成或采集 `PATH`、macOS `/usr/bin/login` 包装、per-Project 或 per-Workspace 启动配置有效负载、一次性非默认配置文件选择、配置文件外观、启动目录策略、SSH 或远程连接、自动切换、导入/导出、shell 启动文件变更、多窗口行为和跨设备同步。
