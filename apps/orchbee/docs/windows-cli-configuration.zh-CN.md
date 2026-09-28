# Windows CLI 配置

## 文件与校验

Windows CLI 创建操作读取 `APPDATA/Orchbee/config.toml`，或 `ORCHBEE_CONFIG_FILE` 指定的绝对文件路径。默认文件不存在时保持默认行为；显式文件不存在则报错。位置变量无效或为空、文件无法读取、目标是目录、文件超过 64 KiB、UTF-8/TOML 无效、未知键或类型错误，均在创建前失败。诊断标明字段和原因，不回显文件内容。读取和检查不会创建配置目录或文件。

```toml
[terminal]
new_cwd = "follow"

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

创建设置作用于 Windows CLI 创建。旧的显式空 Workspace 命令、TUI 直接创建、既有终端、Server 配置重载、自定义 Shell 和键位仍由各自契约约束。

## 检查与正确性

Windows `config check` 版本 2 在五项运行时检查后增加第六项 `user-config` 检查。版本 1 保留原有五项检查的 schema。新增检查报告选定文件、存在状态、规范化有效配置，或有界原因和字段。创建配置无效时使用独立的 `orchbee.cli.failure.configuration` 版本 1 信封并以 1 退出。来源发现失败会在首个 Project 创建前停止配置路径创建，不重试修改。

共享 Windows 设置读取器拥有有界文件检查和规范化；创建流程拥有优先级及已有副作用回执。没有配置写入器或缓存可变状态。核心证据覆盖格式错误和重复 TOML、未知键、大小边界、相对及波浪线路径、覆盖优先级和固定 Herdr 的目录名向量。Windows 公开 IPC 轨迹覆盖有效启动 cwd、配置检出意图、发现失败后无修改、显式路径不查询及无文件时行为不变。完整应用验收为独立关卡。参考行为：Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `TerminalConfig`、`WorktreesConfig` 和 `default_checkout_path`。

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
