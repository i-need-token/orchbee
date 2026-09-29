# Windows CLI 配置

## 文件与校验

Windows CLI 创建操作读取 `APPDATA/Orchbee/config.toml`，或 `ORCHBEE_CONFIG_FILE` 指定的绝对文件路径。默认文件不存在时保持默认行为；显式文件不存在则报错。位置变量无效或为空、文件无法读取、目标是目录、文件超过 64 KiB、UTF-8/TOML 无效、未知键或类型错误，均在创建前失败。诊断标明字段和原因，不回显文件内容。读取和检查不会创建配置目录或文件。

```toml
[terminal]
new_cwd = "follow"
default_shell = "pwsh.exe"
shell_mode = "auto"

[worktrees]
directory = "~/Projects/worktrees"

[session]
resume_agents_on_restore = true

[update]
manifest_check = true
```

仅接受这四个配置段及对应键。`terminal.new_cwd` 接受 `follow`、`home`、`current`、`workspace` 或路径。空值表示 `follow`。路径使用 Windows CLI 路径适配器：相对路径基于配置文件父目录，波浪线路径使用 Windows 主目录，拒绝有歧义的驱动器相对路径、UNC 和设备路径。缺少 `APPDATA` 时仍可使用默认值；显式值无效时不会静默回退。

## 创建策略

| 输入                                 | 结果                                                                              |
| ------------------------------------ | --------------------------------------------------------------------------------- |
| 显式 CLI cwd 或 worktree 路径        | 覆盖对应的有效配置                                                                |
| Workspace、Tab 或 Panel 创建省略 cwd | 应用 `terminal.new_cwd`，随后执行已有的来源目录策略                               |
| 已配置根目录且省略 worktree 路径     | 在任何 Project 或 Git 修改前查询来源仓库，选择 `root/repository-name/branch-slug` |
| 未配置根目录                         | 保持已有的同级检出路径                                                            |
| Resume                               | 使用操作已保留的路径和分支，不重新加载配置                                        |

分支目录名将 ASCII 字母转为小写，将连续非字母数字字符压缩为一个连字符，去掉首尾连字符，空结果使用 `worktree`。自动分支仍为 `worktree/<operation-id>`，并固定在提交意图中。只读仓库查询与创建使用相同来源和信任策略。Server 仍复核来源并独占所有 Git 和规范 Workspace 副作用；查询不构成文件系统事务。显式路径省去额外查询。每次调用只读一次配置，并在应用显式覆盖前完成校验。

创建设置作用于 Windows CLI 创建。旧的显式空 Workspace 命令、TUI 直接创建、既有终端、Server 配置重载和键位仍由各自契约约束；下述 Shell 策略同时覆盖 TUI 创建。

## 检查与正确性

Windows `config check` 版本 2 在五项运行时检查后增加第六项 `user-config` 检查。版本 1 保留原有五项检查的 schema。新增检查报告选定文件、存在状态、规范化有效配置，或有界原因和字段。创建配置无效时使用独立的 `orchbee.cli.failure.configuration` 版本 1 信封并以 1 退出。来源发现失败会在首个 Project 创建前停止配置路径创建，不重试修改。

共享 Windows 设置读取器拥有有界文件检查和规范化；创建流程拥有优先级及已有副作用回执。读取没有写入器或可变缓存状态；下述显式恢复操作负责配置修复。核心证据覆盖格式错误和重复 TOML、未知键、大小边界、相对及波浪线路径、覆盖优先级和固定 Herdr 的目录名向量。Windows 公开 IPC 轨迹覆盖有效启动 cwd、配置检出意图、发现失败后无修改、显式路径不查询及无文件时行为不变。完整应用验收为独立关卡。参考行为：Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `TerminalConfig`、`WorktreesConfig` 和 `default_checkout_path`。

## 恢复策略

共享的 Windows 设置读取器位于 `orchbee-local-site-runtime`，负责有界文件检查、严格 TOML 类型和字面 Windows 路径规范化。CLI 将创建路径投影为 Workbench 类型；运行时库不新增 Workbench 依赖。现有固定版本的 `toml` 解析器作为 Windows 依赖，供实际 Client 和 Server 消费者使用。

`[session] resume_agents_on_restore` 是 TOML 布尔值，默认 `true`。`ORCHBEE_RESUME_AGENTS_ON_RESTORE` 用精确的 `true` 或 `false` 覆盖它；覆盖不能隐藏非法配置文件。此设置仅适用于启动新的 Site 生命周期。已有 Site 的准入不读取它，编辑它也不会改变正在运行的 Site。

Client 启动在持久 Active 发布或激活标记提交前读取并校验一次，将确定的布尔值作为私有 Server 参数传递；Server 校验该参数，不重新读取可能变化的设置。未携带该参数的直接托管 Server 启动，在 Worker 启动前通过同一所有者读取文件和环境。非法输入保留此前检查点和已停止状态，可修正后重试。私有参数需要匹配 cohort；回退使用此前完整 cohort。

Windows `config check` 版本 3 报告 `resumeAgentsOnRestore` 和 `resumeAgentsOnRestoreSource`（`default`、`file`、`environment`）。保留版本 1、2 的 schema。诊断只报告有界字段和原因。验证须覆盖默认值／文件／环境优先级、非法及非 UTF-8 值、严格类型／键、读取边界、提交前拒绝及修正重试、已有 Site 准入不变和实际禁用／启用恢复。热重载、名称和托管状态恢复仍是独立义务。

## 规则更新策略

`[update] manifest_check` 是 TOML 布尔值，默认 true，来源为 `default`／`file`。
Windows `config check` v4 增加 `manifestCheck` 和 `manifestCheckSource`，v1-v3 保持封闭。
Client 激活将已校验的检查值、来源、配置路径和显式文件标志与恢复策略一起传递。
托管 Server 接纳完整私有快照，不重新读取文件；直接启动只调用一次共享设置读取器。cohort 必须匹配。
`read_windows_user_settings_at` 在冻结位置支持有界显式重载，保留默认文件缺失与显式文件缺失的
差别，不创建配置文件。

持久托管发布版 Site 在启动时及每三十分钟检查规则。调试／测试构建和独立端点关闭自动网络任务。
`site agent-manifests` 暴露实际策略及最近自动结果。`site reload-agent-manifests` 验证完整配置，
并将规则检查开关与检测器原子刷新；配置无效保留两者。关闭检查停止调度；重新开启立即安排检查。
活动更新期间重载返回忙碌。此命令不热重载其他运行中设置及键位。自动检查关闭时仍可显式更新。
参见[屏幕模型](../../orchbee-server/docs/agent-screen-model.zh-CN.md#自动检查与配置快照)。
发布版验收夹具若不以公开网络访问为测试目标，须显式关闭自动检查。回退到不认识此键的读取器时，
须先移除 update 配置节；用户覆盖和已提交规则缓存予以保留。

## 恢复自定义快捷键

Windows `config reset-keys [--format text|json]` 是离线配置修复操作，使用与 `config check`
相同的文件位置；文件不存在（包括显式路径）或没有顶层 `keys` 时成功返回，不创建或写入
任何内容。它移除 `[keys]` 及其嵌套表、命令数组，或可以安全删除的顶层赋值；其他设置、
被移除表之外的注释和行结束符逐字节保留。UTF-8／TOML 无效、文件超过 64 KiB，或编辑后解析结果不等于
原配置去掉 `keys` 时，在写入前失败。这可用于从当前设置读取器尚不支持的自定义键位配置
中恢复；重置操作不校验或修复其他设置，也不重载正在运行的 Client。

共享 Windows 设置修复所有者固定私有父目录树，拒绝重解析点、非普通文件或多硬链接文件，
独占重新打开文件后对比准备阶段的字节。输入变化或有并发文件持有者时拒绝编辑。相邻的
唯一 `.bak-keys-...` 备份以不覆盖方式创建，通过私有权限准入并刷盘后，才通过独占句柄
改写原文件，保留原文件身份和权限。写入／刷盘失败时，在释放句柄前恢复并刷盘原内容；
恢复失败则保留完整备份并报告 `recoveryRequired: true`。改写开始后的进程崩溃也保留此前
已刷盘的备份。这是可恢复的原地替换，不是跨断电的文件系统事务。

封闭的 `orchbee.config.reset-keys` v1 回执标明 `missing`、`no-keys`、`reset` 或 `failed`、
选定路径、可为 null 的备份路径、有界错误字段／原因及恢复要求。重置或无变更退出 0，
失败退出 1；用法错误在文件访问前退出 2。文本本地化并转义路径，不回显配置值。成功备份
属于用户恢复产物，通过回执明确交付给调用者负责恢复／删除。不再需要的失败尝试备份会
删除；清理失败则暴露准确路径。不自动准入 Site、重载、启动进程或改写无关设置。

纯变换、独占文件／备份检查、注入写入／恢复失败以及真实 Windows CLI 进程测试覆盖本契约。
完整快捷键、完整重载对齐和最终 Windows 替换验收仍是独立待办。

## Windows 配置 Shell

`terminal.default_shell` 是一个字面可执行文件，而非命令行：接受绝对本地 Windows 路径或裸文件名，最多 4,096 个 UTF-8 字节，不含控制字符。去掉首尾空白，空值沿用当前平台默认值。拒绝带分隔符的相对路径、驱动器相对路径、UNC 和设备路径。裸名称仅在 Worker 的绝对 PATH 目录中查找，先检查原名，没有扩展名时再检查 `.exe`；不回退到当前目录、PATHEXT 或脚本。配置的程序不存在时启动失败，不回退。空配置优先在绝对 PATH 目录中查找首个兼容的 `pwsh.exe`，然后使用绝对 `SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe`。COMSPEC/SHELL 和当前目录查找不覆盖该策略；选择时不执行探测程序。

`terminal.shell_mode` 仅接受 `auto`（默认）、`login`、`non_login`。Windows 上 auto/non_login 不增加参数；login 只给 sh、bash、zsh、ksh、dash、ash、mksh、fish、csh、tcsh 添加 `-l`，名称忽略大小写和可选 `.exe` 后缀。其他 Shell 不加登录参数。已有 PowerShell 提示符集成在选择 Shell 后执行。显式可执行文件和 Launch Profile 保留原参数，绕过默认 Shell 选择。

Client 激活在提交前连同已有配置冻结这两个值。匹配版本的 Server 把私有快照传给 Worker，不重读配置。Worker 持有快照，贯穿延迟 Host 启动、后续终端准备、CLI/TUI 新建和默认 Shell 恢复。非托管 Worker 直接启动在接入前读取并校验一次配置。已准备操作及重放保留确定的启动事实。编辑文件不改变运行中的 Shell 或 Worker 策略；Worker 的 Shell 默认值仍需新的 Site 生命周期生效，在线重载通过 `restartRequired` 明确返回这些变化。Windows `config check` v5 报告 `defaultShell`、`shellMode`，旧版本保持封闭。Shell 专属创建错误使用 configuration failure v2，v1 保留原字段和原因集合。这些值代表配置，不保证程序实际能启动。

共享解析器、私有快照接入、Worker 启动解析、显式覆盖与运行循环测试，以及真实 Windows CLI 检查覆盖本契约。不修改公开线协议或持久化检查点；部署和回退均使用匹配的整套程序。

自动 PATH 候选必须通过有界 DOS/PE/COFF、可选头检查，非空节表必须处于文件边界内，具有可执行映像且非 DLL 标志，x86/x64/ARM64 机器类型兼容。损坏、缺失、不可读或不兼容的候选会被跳过。这仅证明结构候选，不能保证依赖可加载。已有固定版本 `winsafe` kernel 安全包装获取系统架构，不添加自写 unsafe。[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo) 对 ARM64 上的 x86/x64 进程返回模拟架构；模拟 Worker 优先本机 ARM64 程序尚不声称与 Herdr 的 IsWow64Process2 策略完全对齐。显式配置仍可选择该程序。真实 x64 选择和合成头部/架构分区覆盖本实现，不声称通过 ARM64 硬件验收。

## Windows Client 按键配置

有界共享设置读取器在 Client 启动前接纳 `[keys]`。每个 Client 在在线发布前持有不可变绑定快照；
编辑文件或重置在下一个 Client 或显式在线重载后生效。Client 将已实现的语义动作映射到默认注册表。
用户覆盖替换该动作的默认绑定，并取代冲突的默认按键。用户绑定歧义、未支持动作或
语法错误会拒绝接纳。这种严格失败策略与 Herdr 的诊断并禁用策略有差异，仍明确列为缺口。

`prefix` 接受一个按键。动作值接受字符串或最多 16 个候选；空字符串或空数组禁用动作。
`prefix+` 指定前缀层，其余必须是安全直达按键。`switch_tab` 支持带修饰键的 `1..9`。
完整修饰键及大写字母在冲突检查前归一化。前缀本身在两层均保留。直达可打印字符、
裸编辑键及 Ctrl+C/D/Z 被拒绝。捕获与模式先于直达分发；粘贴及释放事件不触发绑定。
双前缀优先发送规范 C0，否则发送原始字节，不再硬编码回退为 Ctrl+B。

解析后的注册表同时提供分发、帮助行和主要实时提示。禁用动作保留在帮助中。现有原生
默认值保留；未支持的 Herdr 字段明确失败。完整重载对齐、自定义命令、导航模式映射、
Agent/按序 Workspace 按键及其他未支持动作仍未完成。定向证据覆盖边界、冲突与默认
优先级、输入归属、分发及发现；不宣称全量回归或视觉验收通过。


支持的动作字段为 `help`、`new_workspace`、`rename_workspace`、`workspace_picker`、
`detach`、`previous_workspace`、`next_workspace`、`new_tab`、`rename_tab`、`previous_tab`、
`next_tab`、`move_tab_previous`、`move_tab_next`、`switch_tab`、`close_tab`、
`focus_pane_left/down/up/right`（四个字段）、`split_vertical`、`split_horizontal`、
`close_pane`、`zoom`（别名 `fullscreen`）、`toggle_sidebar` 和 `reload_config`。它们调用现有原生动作；
Workspace 对应原生 Workbench 位置，竖向分屏向右、横向分屏向下。

```toml
[keys]
prefix = "ctrl+a"
help = ["prefix+?", "f1"]
new_tab = "prefix+n"
switch_tab = "alt+1..9"
zoom = ""
```

Windows `config check` v6 增加 `keyPrefix`、`keyOverrides`，按键配置错误使用
configuration failure v3；旧版本保留封闭字段集合。示例让新建 Tab 取代默认的下一 Tab
快捷键；空 `zoom` 禁用放大快捷键。当前绑定只在主 TUI 使用，CLI 终端流 attach 的
独立控制按键尚未迁移。全量键位导入和无害警告恢复仍需后续实现。

功能键使用 xterm 的实际 F1–F12 序列编号（跳过 16 和 22），带修饰符的 F1–F4
以及 kitty F1–F24。修饰符事件尾部保留按下、重复和释放；无效事件或无法表示的
Hyper/Meta 修饰位不会降级为普通快捷键。宿主是否实际提供这些按键仍取决于终端。

## 在线配置重载

Windows `site reload-config` 请求运行中 Site 重载；`site config` 读取最后发布的版本。
两者均不激活 Site。Server 冻结启动配置位置，串行接纳重载，验证整个有界文件并准备
规范键位配置后才提交。失败保留原版本、键位和在线 manifest-check 调度。版本 0 仅是启动基线，不是在线发布；首次成功重载发布版本 1，之后仅在已接纳设置变化时版本递增。另一重载持有接纳权时读取可返回忙。提交后丢失响应属于结果未知；
Client 不自动重发变更，可通过读取当前状态恢复判断。

Server 提交已支持的在线 manifest-check 策略，并向连接中的主 TUI Client 发布键位。
发布不等于所有 Client 都已确认应用。每个 Client 拥有可取消的配置观察线程，仅在输入包
之间应用已验证的新版本配置；清除待处理前缀，保留编辑器/确认归属和草稿。重载失败
保留现有键位并显示本地化提示。重载动作可通过 `reload_config` 配置。原生默认是前缀 F5；Herdr 使用前缀 Shift+R，与原生已有的 Project 重命名默认冲突。配置 `reload_config = "prefix+shift+r"` 会明确替换该冲突默认。

默认 Shell、Shell 模式和启动恢复策略相对 Site 启动时的变化通过 `restartRequired`
明确返回；现有 Worker 策略和已恢复会话仍由当前生命周期拥有。CLI 目录/worktree 默认值
在下一次调用读取。这是部分 Herdr 重载对齐：严格整文件拒绝、Worker 在线默认值、
未实现的配置族和 Client 独立配置策略仍待补全。
在线键位配置上限为 64 KiB；每动作最多 144 个展开触发键，容纳用户文件最多 16 个数字范围候选。独立可取消连接每 500 毫秒读取一次，单次交换限时两秒。成功重载表示已发布，不表示每个 Client 都已应用。

不新增持久化格式迁移或自动重试。
测试覆盖授权、有界序列化、失败保留、无变化重载、发布应用及关闭。
