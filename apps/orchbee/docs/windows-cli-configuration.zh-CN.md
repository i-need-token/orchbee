# Windows CLI 配置

## 文件与校验

Windows CLI 创建操作读取 `APPDATA/Orchbee/config.toml`，或 `ORCHBEE_CONFIG_FILE` 指定的绝对文件路径。默认文件不存在时保持默认行为；显式文件不存在则报错。位置变量无效或为空、文件无法读取、目标是目录、文件超过 64 KiB、UTF-8/TOML 无效、未知键或类型错误，均在创建前失败。诊断标明字段和原因，不回显文件内容。读取和检查不会创建配置目录或文件。

```toml
[terminal]
new_cwd = "follow"

[worktrees]
directory = "~/Projects/worktrees"
```

仅接受这两个配置段及对应键。`terminal.new_cwd` 接受 `follow`、`home`、`current`、`workspace` 或路径。空值表示 `follow`。路径使用 Windows CLI 路径适配器：相对路径基于配置文件父目录，波浪线路径使用 Windows 主目录，拒绝有歧义的驱动器相对路径、UNC 和设备路径。缺少 `APPDATA` 时仍可使用默认值；显式值无效时不会静默回退。

## 创建策略

| 输入                                      | 结果                                                                              |
| ----------------------------------------- | --------------------------------------------------------------------------------- |
| 显式 CLI cwd 或 worktree 路径             | 覆盖对应的有效配置                                                                |
| 增强 Workspace、Tab 或 Panel 创建省略 cwd | 应用 `terminal.new_cwd`，随后执行已有的来源目录策略                               |
| 已配置根目录且省略 worktree 路径          | 在任何 Project 或 Git 修改前查询来源仓库，选择 `root/repository-name/branch-slug` |
| 未配置根目录                              | 保持已有的同级检出路径                                                            |
| Resume                                    | 使用操作已保留的路径和分支，不重新加载配置                                        |

分支目录名将 ASCII 字母转为小写，将连续非字母数字字符压缩为一个连字符，去掉首尾连字符，空结果使用 `worktree`。自动分支仍为 `worktree/<operation-id>`，并固定在提交意图中。只读仓库查询与创建使用相同来源和信任策略。Server 仍复核来源并独占所有 Git 和规范 Workspace 副作用；查询不构成文件系统事务。显式路径省去额外查询。每次调用只读一次配置，并在应用显式覆盖前完成校验。

这些设置作用于 Windows CLI 创建。旧的显式空 Workspace 命令、TUI 直接创建、既有终端、Server 配置重载、自定义 Shell 和键位仍由各自契约约束。

## 检查与正确性

Windows `config check` 版本 2 在五项运行时检查后增加第六项 `user-config` 检查。版本 1 保留原有五项检查的 schema。新增检查报告选定文件、存在状态、规范化有效配置，或有界原因和字段。创建配置无效时使用独立的 `orchbee.cli.failure.configuration` 版本 1 信封并以 1 退出。来源发现失败会在首个 Project 创建前停止配置路径创建，不重试修改。

CLI 配置读取器拥有有界文件检查和规范化；创建流程拥有优先级及已有副作用回执。没有配置写入器或缓存可变状态。核心证据覆盖格式错误和重复 TOML、未知键、大小边界、相对及波浪线路径、覆盖优先级和固定 Herdr 的目录名向量。Windows 公开 IPC 轨迹覆盖有效启动 cwd、配置检出意图、发现失败后无修改、显式路径不查询及无文件时行为不变。完整应用验收为独立关卡。参考行为：Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `TerminalConfig`、`WorktreesConfig` 和 `default_checkout_path`。
