# Windows CLI 配置

## 文件与校验

Windows CLI 创建操作读取 `APPDATA/Orchbee/config.toml`，或 `ORCHBEE_CONFIG_FILE` 指定的绝对文件路径。默认文件不存在时保持默认行为；显式文件不存在则报错。位置变量无效或为空、文件无法读取、目标是目录、文件超过 64 KiB、UTF-8/TOML 无效，均在创建前构成致命错误。成功解析的文档逐字段接纳：非法的受支持叶字段使用已有安全默认值，非法 profile 使用该 profile 的已有安全默认值，未知字段则忽略。读取器将每次回退作为警告报告，且不回显值或命令。读取和检查不会创建配置目录或文件。

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
channel = "stable"
manifest_check = true
```

只有本文记录的配置段和键会影响 Orchbee，包括下文的 `[keys]` 与 `[ui]` 配置族。已知仅属于 Herdr 的顶层配置族 `onboarding`、`server`、`advanced`、`experimental` 和 `remote` 会标记为不支持，而不是普通未知字段。已知不支持的嵌套字段包括 `terminal.kitty_graphics`、`update.version_check`、仅属于 Herdr 的呈现和剪贴板设置、自定义声音路径，以及不存在对应 Orchbee 动作的按键动作。识别这些字段绝不会启用插件、安装 Provider 或运行配置命令。Orchbee 不会自动读取 Herdr 配置文件。

`terminal.new_cwd` 接受 `follow`、`home`、`current`、`workspace` 或路径。空值表示 `follow`。路径使用 Windows CLI 路径适配器：相对路径基于配置文件父目录，波浪线路径使用 Windows 主目录，拒绝有歧义的驱动器相对路径、UNC 和设备路径。非法路径仅让该叶字段回退。缺少 `APPDATA` 时仍可使用默认值；显式值无效时不会静默回退。已记录的 `ORCHBEE_RESUME_AGENTS_ON_RESTORE` 环境覆盖仍采用严格策略：除精确的 `true` 或 `false` 外均为致命错误，不能被文件回退隐藏。

警告使用有界的规范原因语法和封闭的处置词汇，只携带有界的规范点分字段路径。警告按字段和原因去重、排序。一份结果最多携带 128 条警告及精确的省略数量，因此截断始终可见。未知路径段会转义并在 UTF-8 边界截短；不同原始字段在显示截短前去重。

### 默认配置输出

`orchbee --default-config` 是独占式离线调用。参数解析会在读取任何环境变量或发现运行时前选中它，且不会激活 Session。它向 stdout 写入确定性的 UTF-8 TOML，并以一个换行结尾。固定英文注释列出已接纳设置的默认值、选项和用途；使用英文可让此可复制配置产物在不同 CLI 语言环境下保持一致。已注册的 Client 动作会以注释按键数组列出，没有默认绑定的动作显示为 `[]`；只有取消注释的行才会成为覆盖，因此不改动生成文件会继续沿用 Client 持续演进的默认值。动作行由 Client 使用的同一默认绑定注册表生成。参考还说明命名主题和自定义命令规则。旧叶字段 `theme.name`、`ui.toast.agent_delivery` 和 `ui.sound.enabled` 保持注释；共享偏好记录不存在时，只有取消注释某一项才会将其显式迁移到共享用户偏好。当前共享偏好由 Settings 管理。`config check` 可无警告地接纳生成结果；Windows CLI 解析后的有效设置和按键绑定与配置文件缺失时相同。原生 `UserPreferencesStore` 读取生成文件时，共享偏好值也保持在文件缺失时的默认状态。

## 命名 chrome 主题

共享的[应用外观偏好](../../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)
支持浅色（`light`）、深色（`dark`）和系统（`system`）；系统是通用默认值。在原生 TUI 中，
系统模式会跟随宿主专用解析器检测到的外观，外观未知时回退到 Dark。Windows TOML `[theme] name` 是单独的旧命名主题
迁移输入，接受 `terminal-native`、`one-light` 或 `one-dark`；离线默认 TOML 将注册表默认值保留为
注释示例 `# name = "terminal-native"`，只有在共享偏好记录不存在且显式取消注释后才会迁移。无效名称或类型给出警告并让该字段回退；旧 TOML 设置仍不支持
`theme.custom`。[原生主题](native-themes.zh-CN.md)定义极性、颜色能力、所有 chrome 消费界面、
共享重载及回退。Herdr 导入保留现有转换排除项；在候选文件中显式选择主题。

`config check` v14 为已准入的 Windows user-config details 增加必需 `themeName`；
致命错误形状不变，1–13 版保持封闭。Session 配置 v9 为成功快照增加必需 `themeName`，
1–8 版保持封闭。目录 v142 发布这些扩展，不增加 schema 或命令路径。

## Herdr 配置导入

Windows `orchbee config import-herdr --input-file <source> --output-file <destination>
[--dry-run] [--format text|json]` 只把显式指定的 Herdr 文件转换为新的 Orchbee 候选配置。
它不发现任一应用的配置、不连接 Session、不执行命令、不重载运行中的 Client。拒绝
`--machine` 和 `--session`。两个文件参数均为必填、单次使用，绝不代表 stdin，拒绝 `-`。
相对参数使用一次捕获的调用者目录，波浪线使用现有 Windows 主目录适配器。解析后的普通
本地 Windows 路径拒绝 UNC/设备路径、备用数据流、设备名、点路径段、尾点/空格及空叶名。
源相对设置使用已解析源参数的父目录，与目标位置无关。

共享 Windows 设置 owner 用一个普通、非 reparse、单硬链接的源文件句柄接纳快照，读取期间
不共享写入/删除。解码前限制为 64 KiB，再严格解析 UTF-8 TOML。源不可读、过大、非 UTF-8
或语法错误均致命。导入不受 `APPDATA`、`ORCHBEE_CONFIG_FILE` 或
`ORCHBEE_RESUME_AGENTS_ON_RESTORE` 的选择影响。只有明确记录的文件参数和波浪线展开
能够使用目录上下文或 Windows 主目录变量。

### 转换模型

主操作准备不可变的稀疏候选配置与有界字段报告。未指定的设置仍未指定，不冻结任一产品的
默认值。现有叶字段/profile 接纳规则决定有效值及安全默认。非法普通叶字段、prefix 和
profile 省略并报告；非法已知动作写为空绑定数组，仅禁用该动作。不支持的命令和值不会
进入候选配置。选定的兼容性参考是 Windows 上的 Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34`，具体为其配置模型、按键、侧栏和通知表示。
这是配置子集转换，不对 Herdr 完整运行时行为作出兼容声明。

| 显式源字段                                                  | 候选行为                                                                                                                         |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `terminal.default_shell`、`terminal.shell_mode`             | 使用现有 Windows 字面 Shell 接纳规则                                                                                             |
| `terminal.new_cwd`、`worktrees.directory`                   | 保留策略，将相对/波浪线路径按源上下文固定为绝对路径                                                                              |
| `session.resume_agents_on_restore`                          | 保留显式有效布尔值                                                                                                               |
| `ui.window_title`、`ui.sidebar.agents`、`ui.sidebar.spaces` | 保留兼容的显式标题/profile 字段，包括显式空 rows                                                                                 |
| `keys.prefix` 与当前内置动作注册表                          | 保留接纳的绑定；`zoom` 优先于 `fullscreen`；注册表顺序决定冲突并禁用落败动作                                                     |
| `ui.toast.delivery` 或旧 `ui.toast.enabled`                 | 显式 `delivery` 即使非法也优先；有效的 `herdr` 变为 `orchbee`，同时设置 `delivery` 和 `agent_delivery`；缺席时保留 Orchbee 默认  |
| `ui.toast.delay_seconds`、`ui.toast.herdr.position`         | 保留接纳的延迟和位置                                                                                                             |
| `ui.sound.enabled`、`ui.sound.agents`                       | 保留显式布尔值及 `default`/`on`/`off`；`open_code` 映射为 `opencode`，`github_copilot` 为 `copilot`，`agy` 仍为 `agy`            |
| 其他字段                                                    | 报告并省略，包括全部 Herdr 更新策略、onboarding/theme/server/advanced/experimental/remote、图形、自定义声音路径及自定义/插件命令 |

报告携带字段身份、目标字段、封闭处置及规范原因，不携带被拒绝的值或命令文本。先排序和
去重，再进行有界字段呈现，最多显示 128 条及精确省略计数。仅 dry-run JSON 携带
`candidateToml`，作为明确请求的预览数据；text/stderr 不回显配置值。普通成功回执只报告
候选字节大小和目标位置。在访问任何目标前，准备阶段输出以一个换行结尾的 UTF-8 TOML，
拒绝超过 64 KiB 的输出，并按目标上下文重新接纳候选，验证预期有效子集。兼容契约省略的
源值不能因为序列化而被启用。

### 文件工作流与恢复

| 阶段       | 提交和失败边界                                                                                   |
| ---------- | ------------------------------------------------------------------------------------------------ |
| 解析与获取 | 完整语法、路径接纳和一次源快照；失败不创建任何内容                                               |
| 准备       | 完成转换、报告、大小及往返校验；失败不创建任何内容                                               |
| 预览       | 返回准备好的候选/报告，不探测、锁定或创建目标                                                    |
| 创建       | 固定所有现有父目录祖先，防止路径重定向；末级父目录必须私有；独占新建普通单硬链接叶文件并保护权限 |
| 写入与同步 | 保留祖先与独占叶句柄直到关闭；只写一次并同步；只有完整同步成功才允许 `created` 成功              |
| 报告       | 返回一个准备好的回执；stdout 交付失败不撤销已知文件提交，也不授权重放                            |

目标父目录必须已存在；导入不创建目录、备份、临时文件或运行时状态。目标已存在或其他
创建者抢先创建时拒绝，绝不覆盖。先创建后写入是可恢复操作，不是原子文件系统事务：
创建之后的权限/写入/flush 失败报告确切新文件及 `recoveryRequired: true`，不删除可能
被其他 owner 替换的路径。创建前失败报告 `created: false`，无恢复残留。进程硬崩溃
可能留下空、部分或完整新文件且没有回执，应检查显式选择的目标。不编辑源或现有配置，
不自动重试。通过 `ORCHBEE_CONFIG_FILE` 试用候选，回到原配置选择即可回退。调用方拥有
交付的候选文件和明确报告的恢复残留。

内嵌 schema 目录 v148 包含此命令。
封闭的 `orchbee.config.import-herdr` v1 回执区分 `preview`、`created`、`failed`，携带
源/目标、字段报告、候选大小、创建/恢复事实及带类型的失败阶段/原因。退出 0 表示预览或
已同步创建，退出 1 表示已知本地失败，非法语法在获取前退出 2。中断进程沿用平台信号
退出行为，不输出伪造成功回执。

### 导入正确性

| 声明                                               | 可执行证据                                                 | 边界                                                                         |
| -------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------- |
| 稀疏转换保留受支持显式子集，仅禁用非法动作         | 源码导出的确定性向量、稳定生成分区和候选往返测试           | 固定配置子集和 64 KiB 边界；信任 TOML 解析器及现有叶字段接纳                 |
| 预览无目标或运行时副作用；非法准备绝不创建目标     | 拒绝能力的工作流测试及真实 Windows CLI 进程                | 无需物理终端或 Provider 交互                                                 |
| 并发导入不覆盖已有目标；只有完整同步输出才报告成功 | 写入故障切点和受控 Windows 双创建者轨迹                    | 两个调用与目标缺席/部分/完整抽象；Windows 独占创建/共享/ACL API 为适配器假设 |
| 公开语法、回执、本地化报告和发现入口一致           | 语法、封闭 schema、text/JSON、帮助、补全和离线 schema 测试 | 机器输出与 locale 无关；text 转义不可信身份                                  |

运行 `cargo test --locked -p orchbee -p orchbee-local-site-runtime --all-targets --all-features` 及适用根门禁。受控来源/预览/writer 测试检查保留、预览无效果、独占创建、创建前准备、既有目标不变及完整 flush 后才成功。它们不证明任意 Windows ACL 行为、断电持久性、回执交付或无界 Herdr 一致性。不另设导入 TLA 模型或全称 Lean 定理。

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

Windows `config check` 版本 13 为 `user-config` 检查增加已接纳的警告列表和省略数量。带警告的结果使用 `warning` 状态，整体文档仍成功并以 0 退出。致命文件或环境接纳错误使用 `fail` 状态并以 1 退出。版本 1 至 12 保持封闭。检查报告选定文件、存在状态和规范化有效配置；致命的创建配置继续使用独立的 `orchbee.cli.failure.configuration` 信封。来源发现失败会在首个 Project 创建前停止配置路径创建，不重试修改。

共享 Windows 设置读取器拥有有界文件检查、确定性的逐字段接纳和规范化；创建流程拥有优先级及已有副作用回执。读取没有写入器或可变缓存状态；下述显式恢复操作负责配置修复。核心证据覆盖格式错误和重复 TOML、字段局部回退、稳定且有界的警告、相对及波浪线路径、覆盖优先级和固定 Herdr 的目录名向量。Windows 公开 IPC 轨迹覆盖有效启动 cwd、配置检出意图、发现失败后无修改、显式路径不查询及无文件时行为不变。完整应用验收为独立关卡。参考行为：Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `TerminalConfig`、`WorktreesConfig` 和 `default_checkout_path`。

## 恢复策略

共享的 Windows 设置读取器位于 `orchbee-local-site-runtime`，负责有界文件检查、逐字段类型接纳和字面 Windows 路径规范化。CLI 将创建路径投影为 Workbench 类型；运行时库不新增 Workbench 依赖。现有固定版本的 `toml` 解析器作为 Windows 依赖，供实际 Client 和 Server 消费者使用。

`[session] resume_agents_on_restore` 是 TOML 布尔值，默认 `true`。`ORCHBEE_RESUME_AGENTS_ON_RESTORE` 用精确的 `true` 或 `false` 覆盖它；覆盖不能隐藏非法配置文件。此设置仅适用于启动新的 Session 生命周期。已有 Session 的准入不读取它，编辑它也不会改变正在运行的 Session。

Client 启动在持久 Active 发布或激活标记提交前读取并校验一次，将确定的恢复布尔值与冻结配置位置作为私有 Server 参数传递。Server 校验这些参数，恢复决定一经消费便不再重新推导。配置发布前，Server 在冻结位置按同一套继承的严格环境覆盖规则读取一个完整候选，并将其作为权威版本 0 基线。未携带私有参数的直接托管 Server 启动，通过自己的首次读取确定相同的启动决定和冻结位置。致命输入保留此前检查点和已停止状态，可修正后重试。私有参数需要匹配 cohort；回退使用此前完整 cohort。

Windows `config check` 版本 3 报告 `resumeAgentsOnRestore` 和 `resumeAgentsOnRestoreSource`（`default`、`file`、`environment`）。保留版本 1、2 的 schema。诊断只报告有界字段和原因。验证须覆盖默认值／文件／环境优先级、畸形及非 UTF-8 文件、字段级类型／键准入、读取边界、致命错误提交前拒绝及修正重试、已有 Session 准入不变和实际禁用／启用恢复。热重载、名称和托管状态恢复仍是独立义务。

## 规则更新策略

`[update] channel` 仅接受 `stable`（默认）或 `preview`，与 `manifest_check` 相互独立。`orchbee channel show [--format text|json]` 读取完整共享配置，不创建文件、不激活 Session、不访问网络。文本仅为频道标记；`orchbee.channel.show` v1 报告频道、default/file 来源、选定配置路径和文件存在状态。默认文件缺失时使用 stable；显式文件缺失或配置非法时失败。这是期望的发布频道偏好，不代表当前安装 cohort 的事实。Windows `config check` v9 和配置失败 v6 接受 `update.channel` 诊断；旧 schema 保持封闭。

`[update] manifest_check` 是 TOML 布尔值，默认 true，来源为 `default`／`file`。
Windows `config check` v4 增加 `manifestCheck` 和 `manifestCheckSource`，v1-v3 保持封闭。
Client 激活将已校验的检查值、来源、配置路径和显式文件标志与恢复策略一起传递。
托管 Server 将其接纳为启动策略快照，随后在冻结位置读取完整的权威版本 0 候选；已消费的恢复策略不会改变。
直接启动通过同一设置所有者建立这两种角色。cohort 必须匹配。
`read_windows_user_settings_at` 在冻结位置支持有界显式重载，保留默认文件缺失与显式文件缺失的
差别，不创建配置文件。

持久托管发布版 Session 在启动时及每三十分钟检查规则。调试／测试构建和独立端点关闭自动网络任务。
`server agent-manifests` 暴露实际策略及最近自动结果。`server reload-agent-manifests` 验证完整配置，
并将规则检查开关与检测器原子刷新。致命文件/TOML 错误保留两者；字段警告应用已接纳的回退设置。关闭检查停止调度；重新开启立即安排检查。
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
删除；清理失败则暴露准确路径。不自动准入 Session、重载、启动进程或改写无关设置。

纯变换、独占文件／备份检查、注入写入／恢复失败以及真实 Windows CLI 进程测试覆盖本契约。
完整快捷键、完整重载对齐和最终 Windows 替换验收仍是独立待办。

## Windows 配置 Shell

`terminal.default_shell` 是一个字面可执行文件，而非命令行：接受绝对本地 Windows 路径或裸文件名，最多 4,096 个 UTF-8 字节，不含控制字符。去掉首尾空白，空值沿用当前平台默认值。拒绝带分隔符的相对路径、驱动器相对路径、UNC 和设备路径。裸名称仅在 Worker 的绝对 PATH 目录中查找，先检查原名，没有扩展名时再检查 `.exe`；不回退到当前目录、PATHEXT 或脚本。配置的程序不存在时启动失败，不回退。空配置优先在绝对 PATH 目录中查找首个兼容的 `pwsh.exe`，然后使用绝对 `SystemRoot/System32/WindowsPowerShell/v1.0/powershell.exe`。COMSPEC/SHELL 和当前目录查找不覆盖该策略；选择时不执行探测程序。

`terminal.shell_mode` 仅接受 `auto`（默认）、`login`、`non_login`。Windows 上 auto/non_login 不增加参数；login 只给 sh、bash、zsh、ksh、dash、ash、mksh、fish、csh、tcsh 添加 `-l`，名称忽略大小写和可选 `.exe` 后缀。其他 Shell 不加登录参数。已有 PowerShell 提示符集成在选择 Shell 后执行。显式可执行文件和 Launch Profile 保留原参数，绕过默认 Shell 选择。

Client 激活在提交前连同已有配置冻结这两个值。匹配版本的 Server 把私有快照传给 Worker，不重读配置。Worker 保留快照作为延迟 Host 启动及后续准备的回退值。Server 授权的 CLI/TUI/默认 Shell 恢复准备捕获最新已发布默认值，详见下文。非托管 Worker 直接启动在接入前读取并校验一次配置。已准备操作及重放保留确定的启动事实。仅编辑文件不发布配置，也不改变运行中的 Shell；显式重载更新后续准备所用的默认值。Windows `config check` v5 报告 `defaultShell`、`shellMode`，旧版本保持封闭。Shell 专属创建错误使用 configuration failure v2，v1 保留原字段和原因集合。这些值代表配置，不保证程序实际能启动。

共享解析器、私有快照接入、Worker 启动解析、显式覆盖与运行循环测试，以及真实 Windows CLI 检查覆盖本契约。不修改公开线协议或持久化检查点；部署和回退均使用匹配的整套程序。

自动 PATH 候选必须通过有界 DOS/PE/COFF、可选头检查，非空节表必须处于文件边界内，具有可执行映像且非 DLL 标志，x86/x64/ARM64 机器类型兼容。损坏、缺失、不可读或不兼容的候选会被跳过。发现过程解析候选的规范路径，排除祖先目录中存在 `AppxManifest.xml` 文件的映像：Store/AppX 激活拒绝 Host 的原子 Job-list 启动。该选择发生在终端创建之前，继续搜索 PATH 或使用系统回退；不重试创建，也不放松 Job 进程归属约束。显式配置仍按字面选择。这些检查仅证明结构候选，不能保证依赖可加载。已有固定版本 `winsafe` kernel 安全包装获取系统架构，不添加自写 unsafe。[GetNativeSystemInfo](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getnativesysteminfo) 对 ARM64 上的 x86/x64 进程返回模拟架构；模拟 Worker 优先本机 ARM64 程序尚不声称与 Herdr 的 IsWow64Process2 策略完全对齐。显式配置仍可选择该程序。真实 x64 选择、合成头部／架构分区，以及直接／嵌套包标记分区覆盖本实现，不声称通过 ARM64 硬件验收。

## Windows Client 按键配置

有界共享设置读取器在 Client 启动前接纳 `[keys]`。每个 Client 在在线发布前持有不可变绑定快照；
编辑文件或重置在下一个 Client 或显式在线重载后生效。Client 将已实现的语义动作映射到默认注册表。
用户覆盖替换该动作的默认绑定，并取代冲突的默认按键。无效的显式动作仍保留为显式禁用覆盖，
不会静默恢复内置默认；其他动作及同一动作里的有效候选仍可使用。未知动作会警告并忽略，已知仅
属于 Herdr 的动作会报告为不支持。多个显式动作占用同一按键时，固定动作注册表顺序选择第一项并
报告后续冲突。诊断不保留原始按键或命令。

`prefix` 接受一个按键。动作值接受字符串或最多 16 个候选；空字符串或空数组保留为显式禁用覆盖，并发出有界警告。
`prefix+` 指定前缀层，其余必须是安全直达按键。`switch_tab` 和 `focus_agent` 支持带修饰键的 `1..9`。Agent 序号按过滤后的机器
Agent 列表计算，`focus_agent` 默认未绑定。
完整修饰键及大写字母在冲突检查前归一化。前缀本身在两层均保留。直达可打印字符、
裸编辑键及 Ctrl+C/D/Z 被拒绝。捕获与模式先于直达分发；粘贴及释放事件不触发绑定。
双前缀优先发送规范 C0，否则发送原始字节，不再硬编码回退为 Ctrl+B。

`copy_mode` 进入[键盘复制模式](native-keyboard-copy.zh-CN.md)，默认 `prefix+U`（大写 U）。它可以使用相同的安全直达组合键，也可以用空数组禁用入口。模式内移动键保持固定。Herdr 导入把明确提供的有效 `keys.copy_mode` 转换为该原生动作；旧 reader 将其报告为未支持。

`new_worktree` 与 `open_worktree` 进入 [Worktree 打开流程](native-worktrees.zh-CN.md)，默认
为 `prefix+W` 与 `prefix+O`。安全直达组合键、禁用和显式 Herdr 按键导入沿用相同注册表。
`remove_worktree` 仍未支持。

`menu` 打开[原生动作菜单](native-action-menus.zh-CN.md)，默认 `prefix+f2`。
同样支持安全直达组合键及显式禁用。Machines 保留 `prefix+M`。

`resize_mode` 进入[键盘调整模式](native-keyboard-resize.zh-CN.md)，默认 `prefix+P`。
`resize_pane_left`、`resize_pane_right`、`resize_pane_up` 和 `resize_pane_down` 默认
未绑定，接受相同的安全组合键。不进入模式，直接将最近可用分隔线移动五个百分点；
例如 `resize_pane_left = "ctrl+left"`。模式内方向键固定为五个百分点，Shift+方向键为一个，
Esc 退出。待定期间额外调整键被消费，不排队。使用这些新动作名的配置需要匹配当前 cohort；
使用旧 reader 前需删除这些键。其他现有配置保留默认行为。 直接调整绑定要求修饰键或功能键；裸编辑键仍属于 pane。

解析后的注册表同时提供分发、帮助行和主要实时提示。帮助会省略没有有效绑定的动作；需要会话的
动作即使当前未连接 Orchbee 会话，只要有有效绑定，帮助仍会保留该行并提示连接会话。现有原生
默认值仅为没有显式覆盖的动作保留；未支持的 Herdr 字段发出警告且不改变有效注册表。
自定义命令配置无效时禁用已配置命令，不执行部分或未经验证的命令。导航模式映射、按序
Agent/Workspace 按键及其他未支持动作仍未完成。定向证据覆盖边界、冲突与默认
优先级、输入归属、分发及发现；不宣称全量回归或视觉验收通过。

注册表的显示投影独立于接纳的配置拼写，对规范化绑定进行格式化。帮助行和实时提示中的
ASCII 字母只有在绑定包含 Shift 时才使用大写，其余保持小写。Unicode 字形模式使用方向键
字形，并在 Apple 宿主上使用 Apple 修饰键符号。简化字形模式使用 `Up`、`Down`、`Left`、
`Right` 以及 `Ctrl`、`Alt`、`Shift`、`Super`，Apple 宿主也一样。前缀与其成员之间使用一个
空格。这些显示规则不改变 TOML 拼写、规范化、匹配或分发。

支持的动作字段为 `help`、`menu`、`new_worktree`、`open_worktree`、`new_workspace`、`rename_workspace`、`workspace_picker`、
`detach`、`previous_workspace`、`next_workspace`、`previous_agent`、`next_agent`、`focus_agent`、`new_tab`、`rename_tab`、`previous_tab`、
`next_tab`、`move_tab_previous`、`move_tab_next`、`switch_tab`、`close_tab`、
`copy_mode`, `resize_mode`, `resize_pane_left/down/up/right`，以及 `focus_pane_left/down/up/right`（四个字段）、`split_vertical`、`split_horizontal`、
`close_pane`、`zoom`（别名 `fullscreen`）、`toggle_sidebar` 和 `reload_config`。它们调用现有原生动作；
Workspace 对应原生 Workbench 位置，竖向分屏向右、横向分屏向下。
Agent 上一个/下一个默认使用前缀 `{`/`}`，使用同一临时筛选/排序后的
[机器 Agent 列表](machines.zh-CN.md#agent-元数据视图控制与导航)，并重新验证精确物理目标。
这两个动作不增加按序 Agent 聚焦或自定义视图 JSON DSL。

```toml
[keys]
prefix = "ctrl+a"
help = ["prefix+?", "f1"]
new_tab = "prefix+n"
switch_tab = "alt+1..9"
focus_agent = "prefix+alt+1..9"
zoom = ""
```

Windows `config check` v13 保留 `keyPrefix`、`keyOverrides` 并增加有界警告记录；历史
按键配置 failure v3 对旧 cohort 保持封闭。机器字段 `keyPrefix` 保持稳定且不随平台变化
的报告拼写（本例为 `Ctrl+A`），TUI 快捷键发现则显示实际未按 Shift 的手势（`Ctrl+a`）。
示例让新建 Tab 取代默认的下一 Tab 快捷键；空 `zoom` 禁用放大快捷键。当前绑定只在主
TUI 使用。CLI 终端流 attach 保留独立的 Ctrl+B/q 转义；固定 Herdr `e0507237` 在
`src/client/attach.rs` 同样使用固定转义，
因此可配置 attach 不是对齐要求。全量键位导入仍是独立切片。

功能键使用 xterm 的实际 F1–F12 序列编号（跳过 16 和 22），带修饰符的 F1–F4
以及 kitty F1–F24。修饰符事件尾部保留按下、重复和释放；无效事件或无法表示的
Hyper/Meta 修饰位不会降级为普通快捷键。宿主是否实际提供这些按键仍取决于终端。

## 在线配置重载

侧栏 profile 使用同一接纳/发布转换：共享 Windows 配置所有者验证
`ui.sidebar.agents.rows`、`rows_by_agent`、`row_gap`，再发布有界规范 profile。
Client 在安装任何字段前同时验证键位、通知、两个侧栏及主题名称；畸形私有线协议 profile 保留全部原配置。
首个快照到达后，启动使用 Server 的完整已接纳基线；连接期间显示的本地投影只是临时值。
配置检查报告支持的布局及边界。token、Provider 选择、
规则、几何与证据见 [Agent 侧栏配置模型](machines.zh-CN.md#agent-侧栏配置模型)。

Windows `server reload-config` 请求运行中 Session 重载；`server config` 读取最后发布的版本。
两者均不激活 Session。Server 冻结启动配置位置，串行接纳重载，验证整个有界文件并准备
一个包含有效设置和警告的接纳候选后才提交。致命读取或解析失败会保留原版本、全部 profile、
诊断和在线 manifest-check 调度。带警告的候选会原子替换有效设置和诊断；Client 绝不会把一个
版本的设置与另一个版本的诊断拼接。版本 0 是权威启动基线。Client 会应用首个完整 Server
快照一次，包括版本 0；之后拒绝重复或更旧版本。首次成功重载发布版本 1，之后只要已接纳
设置或诊断任一变化，版本就递增。这也包括回退值与后来合法显式值
相同但警告已清除的情况。另一重载持有接纳权时读取可返回忙。提交后丢失响应属于结果未知；
Client 不自动重发变更，可通过读取当前状态恢复判断。

64 KiB 源文件限额和各 profile 的独立边界都是接纳输入；组合后的响应还必须满足不变的
256 KiB 配置响应上限。安装版本 0 或应用任何在线策略前，以及每次重载提交前，Server 使用同一个精确
serializer 预检，对包含固定宽度合法关联标识的完整 `ConfigurationResult` 执行序列化（启动时为
`Current`，重载时为 `Reloaded`）。已接纳源文件的组合转义 profile 和诊断如果超过该响应上限，则会被拒绝，
不截断其他合法设置。启动不安装配置状态或在线策略；重载保留完整旧快照和在线策略。

Server 提交已支持的在线 manifest-check 策略，并向连接中的主 TUI Client 发布键位、profile、主题名称
及有界警告记录。
发布不等于所有 Client 都已确认应用。每个 Client 拥有可取消的配置观察线程，仅在输入包
之间应用完整且已验证的新版本快照，并拒绝过时或重复版本；清除待处理前缀，保留编辑器／
确认归属和草稿。带警告的快照会应用有效设置，并显示本地化摘要，引导用户运行
`orchbee config check`；后续干净快照会清除此警告。配置健康状态与临时提示独立保留：检查点恢复提示
优先用于首次启动显示；该提示消除后，配置警告会重新显示，而且不会阻止 Agent 通知卡片。重载失败保留现有设置和警告记录，并显示
本地化失败提示。重载动作可通过 `reload_config` 配置。原生默认是前缀 F5；Herdr 使用前缀 Shift+R，与原生已有的 Project 重命名默认冲突。配置 `reload_config = "prefix+shift+r"` 会明确替换该冲突默认。

启动恢复策略相对 Session 启动时的变化通过 `restartRequired` 返回；已恢复会话仍由当前
生命周期拥有。默认 Shell 和模式无须重启，对后续准备生效。CLI 目录/worktree 默认值
在下一次调用读取。这是部分 Herdr 重载对齐：未实现的配置族和
Client 独立配置策略仍待补全。
在线键位配置上限为 64 KiB；每动作最多 144 个展开触发键，容纳用户文件最多 16 个数字范围候选。独立可取消连接每 500 毫秒读取一次，单次交换限时两秒。成功重载表示已发布，不表示每个 Client 都已应用。

不引入持久配置迁移或自动重试。确定性测试覆盖临时启动、首次完整应用、致命失败保留、原子 settings/diagnostic 代次、后续单调应用及警告发布/清除。获准 payload 包含 `themeName`；仅主题变化使用同一发布所有者，Client 在完整准入后解析缓存调色板。连接测试覆盖传播，不假设交付公平性。授权、有界序列化、无变化重载及退出仍是实现义务。设置提交/发布核心的范围更窄，只涉及 writer/CAS，不证明整个重载流程。

`server config` 与 `server reload-config` 版本 8 为每个成功快照增加必需的 `diagnostics` 对象。
文本输出包含可见数量、精确省略数量，以及每条有界的字段／原因／处置记录。版本 1 至 7
保持封闭，不接受该成员。

## 后续 Windows 终端的在线默认值

生产现在在每次新 Host 启动前，从一个确认快照读取
[共享设备启动配置](../../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)和滚动历史。
显式旧 TOML Shell 值只为未声明偏好提供初值；Desktop/TUI 持久化或声明后，
后续 TOML 重载不能覆盖该配置。下文配置回执描述已准入旧配置，不代表有效共享目录。
请求携带的覆盖路径保留给没有共享存储的兼容适配器。

重载把已验证的 `DefaultShellSelection` 提交到 Server 现有 TerminalCoordinator。新建接纳时将不可变值冻结到每个默认 `AuthorizedTerminalLaunch`；显式程序/Profile 请求不携带默认覆盖。Worker 在 Host 交接前校验 Windows Shell 语法，并使用自身的程序/PATH 和模式解析。没有覆盖时，非托管/旧私有请求保留 Worker 的启动回退。现有 PTY、已准备启动、精确重放和延迟 Start 不接收后续默认值。CLI/TUI/恢复新建共用同一 coordinator 接纳。

提交锁顺序为配置注册表、Session coordinator、manifest 调度。候选校验与线协议值构造在提交前完成；调度拒绝接纳时，启动策略和发布均不变。持锁期间没有 Worker IPC 或分布式确认。Shell/模式变化对后续新建生效，不再要求 Session 重启；启动恢复仍属于仅启动时生效的设置。重载成功证明配置接纳，不保证程序存在。

启动准备失败记录既有的确定交接前失败，保留 Worker/Server 连接，不创建 Host 终端。同一失败请求重放原失败；修正设置后必须使用新的创建标识。首次 Host 尚不存在时，延迟循环也保持可用。Host 启动/传输不确定性仍保留原退出与不重试策略。证据覆盖私有帧、默认/显式/缺省选择、跨重载准备、精确/冲突重放、失败保留和真实 Windows 管道交换；provider/多 Client/视觉全量验收仍延后。匹配的私有 cohort 整套部署或回退，不迁移持久化数据。

`server config` 和重载回执显示后续默认创建所用的已接纳 `defaultShell` 与 `shellMode`，包括重载响应丢失后的查询。空 Shell 表示平台默认；这些值不证明 Worker 已解析可执行文件或启动成功。CLI 回执 v2 必须包含两字段；历史 v1 schema 保持原有封闭字段集。匹配私有 cohort 的 Workbench 快照同样要求有界字面量（4096 UTF-8 字节、无控制字符）以及 `auto`/`login`/`non_login` 模式。人类可读输出转义字面量，并本地化空 Shell 标签。

## 外层窗口标题

`[ui] window_title` 默认为 `{hostname}: {workspace}`，可在线重载。[标题所有者](windows-terminal-title.zh-CN.md#配置模板) 定义变量、字面花括号、无效/空策略、有界渲染及 CLI 覆盖优先级。语法错误、类型错误或超过 4096 字节现在只让该叶字段回退，并出现在 `config check` v13 的警告记录中；未知 `[ui]` 键会忽略并单独警告。有效 `windowTitle` 始终可解析，因此 `windowTitleDiagnostic` 保持 null。`server config`/`server reload-config` v8 暴露已发布的有效模板及诊断，旧公开版本保持封闭。

## 自定义通知配置

`ui.toast.delivery` 接受 off（默认）、orchbee/herdr、terminal 或 system，用于手动 CLI 通知；仅在未指定 delivery 时，旧 `ui.toast.enabled` 提供关闭/卡片选择。`ui.toast.herdr.position` 选择四角默认位置。自动物理 Agent 通知使用独立的 `ui.toast.agent_delivery`（默认 orchbee）和 `ui.toast.delay_seconds`（默认一秒，范围 0–3600）。`[ui.sound] enabled` 默认 false，并与 `[ui.sound.agents]` 一起仅控制自动内置提示音，不改变显式 `notification show` 的声音；剪贴板策略仍然独立。启动配置属于 Client；显式 Session 重载与键位一起发布完整的有界通知 profile，Client 在同时校验两者之后应用。`server config`/重载 v7 携带 16 KiB `notificationProfile`，`config check` v12 暴露手动与自动策略，配置失败 v9 增加自动延迟／声音诊断。旧公开版本保持封闭。详见完整的[自动通知契约](windows-notifications.zh-CN.md#自动物理-agent-通知)。

通用 Agent 提醒模式与声音确认后覆盖旧自动通知字段；手动通知投递与逐 Agent 声音覆盖保持独立归属。

## Windows 版本安装

`orchbee update [--format text|json]` 安装配置的频道。`orchbee channel set <stable|preview> [--format text|json]` 先校验并原子保存频道，保留注释与其他 TOML 值，再运行同一更新器。该写入器可以创建显式指定但不存在的文件。更新失败保留已保存偏好。两个命令均无须 Session；不会启动、停止、替换或重新连接运行中的 Session。

将 `ORCHBEE_UPDATE_MANIFEST` 设置为显式本地发布清单绝对路径或 HTTPS URL。`{channel}` 替换为所选频道。没有隐式公共发布源。选定的清单是信任依据；逐文件 SHA-256 校验保证与清单一致，不等于独立发布者签名。`ORCHBEE_INSTALL_DIR` 默认为 `LOCALAPPDATA/Programs/Orchbee`。安装路径拒绝 PATH 分隔符、展开变量和控制字符。获取来源共用 600 秒时限并支持 Ctrl+C 取消。HTTPS 使用系统目录中的 curl 8.4 或更新版本，仅允许 HTTPS 有界重定向，不读取 curlrc 或继承代理/凭据环境，并限制下载大小。

发布清单为封闭 JSON，包含 `schemaVersion: 1`、`product: "orchbee"`、`platform: "windows-x86_64"`、`channel`、`version`、`buildId` 和 `files`。每文件恰含 `path`、`url`、小写 `sha256` 与字节数 `size`。文件 URL 为 HTTPS 或相对清单目录解析的可移植路径。清单最大 64 KiB，最多 4096 个文件，单文件最大 512 MiB，总量最大 2 GiB。可移植路径拒绝目录穿越、Windows 设备名、替代数据流、大小写别名及文件/目录冲突。必需内容包括四个程序、schema-2 `cohort.json`、适配器 capsule 及三个 ConPTY 分发文件。所有声明文件先复制到私有暂存区并计算哈希；通过 cohort 与 capsule 摘要接纳后，才执行新 Client 的精确版本启动检查。

安装所有者串行化写入器，并以清单 SHA-256 为名发布独立目录。现有版本经过校验后复用，不覆盖。用户 PATH 激活是后续副作用：失败或结果丢失均保留发布路径以供显式重试。现有进程继续使用原文件。旧版本属于安装状态，因可能仍在运行而保留；自动保留策略和在线交接是独立工作。未发布暂存区在子进程结束确认后清理；结束未确认或清理失败通过回执移交确切路径供恢复，不把旧版本当暂存目录删除。

`orchbee.update` v1 区分频道已保存、版本已发布、PATH 已激活、复用、中断、失败阶段/原因及待清理项。完成状态为 0，确定失败为 1，用法错误为 2，超时为 7，物理/激活结果未确认为 8，中断为 130。即使激活需重试，仍可直接使用已发布路径；重新登录可刷新继承的 PATH。完整分发发布、HTTPS 来源验收以及真实用户 PATH/整套 cohort 启动验收属于部署义务，本地暂存测试不代表这些义务已完成。

从已完成的 Windows cohort 准备清单而不执行发布：

```powershell
node scripts/package-native-windows.mjs C:/path/to/cohort stable 0.1.0 build-id
$env:ORCHBEE_UPDATE_MANIFEST = 'C:/path/to/cohort/release.json'
orchbee update --format json
```

打包工具校验现有 cohort、适配器 capsule 和固定 ConPTY 包，清点有界普通文件，再独占写入同目录的 `release.json`，拒绝已有清单。HTTPS 分发需将此目录发布到清单 URL；相对文件 URL 沿用目录布局。版本必须匹配所打包 Client 的 `--version`；buildId 区分同版本号的不同发布。

独立版本目录同时保存经过接纳的原始 `release.json`；该保留文件不属于文件清单条目，复用时核对其未改变。`status client` v3 将清单绑定到目录标识和已验证 cohort 后再投影实际安装来源，不以当前配置偏好替代。

## Windows Agent 侧栏布局

Windows 主 TUI 在启动及 `server reload-config` 后应用 `ui.sidebar.agents`。例如：

```toml
[ui.sidebar.agents]
rows = [["state_icon", "state_text"], ["machine", "workspace", "tab"], ["pane", "$summary"], ["terminal_title_stripped"]]
row_gap = 1

[ui.sidebar.agents.rows_by_agent]
claude = [["state_icon", "agent"], [{ token = "$model", fg = "#8bc", rules = [{ contains = "fast", ignore_case = true, bold = true }] }]]
```

省略布局保持已有双行默认，显式空布局保留身份及导航行；缺失或隐藏的值不占行。
[侧栏模型](machines.zh-CN.md#agent-侧栏配置模型)定义 Provider 别名、规则、按实际高度
计算的窗口及不变的物理权威。`terminal_title`/`terminal_title_stripped` 现在读取精确物理终端的 OSC 标题。
原始形式保留活动符号；去动画形式移除一个提权前缀和一个已知前导活动符号。
缺失或清除的标题消失；`$terminal_title` 仍是独立自定义值。
`ui.sidebar.spaces` 使用独立 Workspace 布局与 profile，见下文。

`config check` v11 和 Session 配置回执 v6 必需携带 `agentSidebarProfile` 与 `spacesSidebarProfile`（最多 32 KiB
的封闭规范 JSON）。配置失败 v7 增加 `ui.sidebar.agents` 及布局/颜色/token/规则/Provider
诊断。旧公开版本保持原有封闭字段。目录 v123 仍有 105 个 schema、141 条语法路径。
匹配的私有 cohort 一起携带新必需 profile，整套部署/回滚，无持久迁移。

## Windows Workspace 侧栏布局

主侧栏将 Workspace 放在所属 Project 下，可用 Agent 放在精确归属的 Workspace 或
Standalone 行下，共用一个滚动窗口。[侧栏导航模型](tui-shell-layout.zh-CN.md#侧栏导航模型)
拥有展开状态和键盘导航。Workspace 选择器仍可通过打开 Machines 后按 Tab，或点击
侧栏标题／范围页脚进入。即使重名，点击行仍保留完整机器／Session/context 身份。
`ui.sidebar.spaces` 控制 Workspace 行，`ui.sidebar.agents` 控制其 Agent 子行。
Agent 的 `row_gap` 用于 Agent 行之前及相邻 Agent 行之间；Workspace 的 `row_gap`
用于下一条 Project 或 Context 行之前。

```toml
[ui.sidebar.spaces]
rows = [["state_icon", "workspace"], [{ token = "branch", fg = "#8bc", rules = [{ starts_with = "worktree/", dim = true }] }, "git_status"], ["$issue"]]
row_gap = 1
```

默认值为 `[["state_icon", "workspace"], ["branch", "git_status"]]`，间隔 0。
显式 `rows = []` 保留身份／导航行。另一个内置值是 `state_text`；
`$name` 读取 Workspace 报告，不替代内置值。样式与首条匹配规则共用 Agent 接纳机制，
但图标／Git 计数拒绝文字规则，Workspace 布局没有 `rows_by_agent`。
最多 16 行、每行 16 个 token、每个文字 token 16 条规则，`row_gap` 范围 0–65535。
缺失／隐藏值折叠；图标／Git 计数预留宽度后，文字共享剩余单元格。

状态使用 Client 已读投影后的完整物理 Agent 快照，不受列表／来源过滤影响。
Git 在所属 Worker 上读取已提交的 Workspace 目录，显示相对本地 upstream 的非零 ahead／behind；
缺少 upstream 时无计数；非仓库无 Git 观测。游离时无分支名，未出生分支保留名字。
不 fetch、不改配置、不执行 hook/fsmonitor、不启动终端，绘制时无 I/O。
自定义 `$branch`／`$git_status` 保持独立。
[Workspace 侧栏模型](machines.zh-CN.md#工作区侧栏模型)定义保护、聚合、预算、精确激活及验收限制。

配置失败 v8 接纳 `ui.sidebar.spaces`。Session／Client 重载先验证键位、通知及两个侧栏 profile，
再统一发布。每个 profile 最多 32 KiB；匹配 cohort 的配置帧包括转义最多 256 KiB。
旧公开 CLI 版本保持原有封闭字段。
