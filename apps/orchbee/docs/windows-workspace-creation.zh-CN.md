# Windows CLI 工作区创建

## 范围与所有权

`orchbee workspace create` 创建一个可用、未分组的 Workspace，并启动默认 shell 终端。无需先填 Project 或名称，也不会自动启动 AI 程序。Project 是可选的 Workspace 组织方式，可容纳来自多个仓库的 Workspace。普通 Workspace 与 worktree Workspace 使用同等的导航身份。

Server 拥有 canonical Workspace、Directory、context、Tab、Panel、presentation 标识及终端准入。CLI 准备输入后只提交一次带修订围栏的 `workspace.launch`，自身不启动 shell。所有 Windows 形式（包括显式 `--project`/`--label`）统一使用此流程。Mac/Linux 命令执行保留其单独声明的接口。

```powershell
orchbee workspace create
orchbee workspace create --cwd C:/code/demo --label main
orchbee workspace create --project demo --workspace source --cwd workspace --focus
```

可选输入包括 `--project`、`--label`、来源 `--workspace`、精确 `--client`、互斥的 `--focus`/`--no-focus`（默认）及 `--timeout`（1–3600000 毫秒，默认 30000）。既有环境变量、exec/args、launch-profile 和四项全填的终端尺寸选项保持可用。显式 exec/profile 选择指定程序。聚焦要求变更前已有就绪 Client，不启动 TUI。

## 来源与目标

来源选择与目标分组彼此独立。只有显式 `--project` 才选择已有 Project；省略时始终创建未分组 Workspace，不受来源分组或 Project 数量影响。不存在首次 Project 准备阶段。显式 Project 目标必须在启动前唯一解析。

| 目录策略                                 | 结果                                                 |
| ---------------------------------------- | ---------------------------------------------------- |
| 显式路径                                 | 使用解析后的 Windows 字面路径                        |
| `home` / `current`                       | 使用已验证的 Windows home / CLI cwd                  |
| `workspace`                              | 要求来源 Workspace 的全局主 Directory                |
| 默认 / `follow` 且存在已证明匹配的 Panel | 只采样其主终端根进程 cwd                             |
| 无匹配 Panel，或根 cwd 明确缺省          | 使用来源 Workspace 主 Directory，再回退 Windows home |
| 根进程缺失、cwd 非法、查询失败或中断     | 启动前拒绝；不猜测其他进程或 Panel                   |

显式 `--cwd` 路径接受带盘符的 Windows 绝对路径、相对于最终执行 CLI 当前目录的路径，以及主目录形式 `~`、`~/...` 或 `~\...`。语法准入不读取执行上下文，并在机器路由前完成；发送端和中继保留原始 argv。只有最终业务进程才解析显式路径，且早于配置加载和运行时访问。主目录依次选择第一个非空来源：`USERPROFILE`、`HOMEDRIVE` 加 `HOMEPATH`，最后是 `HOME`；选中值无效时直接拒绝，不继续回退。`~name`、`$HOME` 和 `%USERPROFILE%` 等其他文本保持字面含义。解析不探测或规范化文件系统、不展开 shell 变量，也不创建目录。

显式 Client 优先于隐式调用者；否则已验证调用者优先于唯一当前就绪 Client。调用者标识无效或不完整、Client 过期及歧义均拒绝。显式路径、`home`、`current` 策略忽略无关来源身份。无调用者或 Client 时允许无界面的 no-focus 创建。省略名称时使用目录 basename，根目录使用本地化 Workspace 后备名称。若规范 Workspace 已有完全相同的名称，则在全局导航中依次选择首个可用的 ` (2)`、` (3)` 等后缀。512 字符限制包含后缀，必要时只截断 basename。显式名称保持原样；并发变化仍由既有修订栅栏保护，不重试。[CLI 配置](windows-cli-configuration.zh-CN.md)拥有持久目录默认值。解析阶段不做 shell 展开、文件系统探测或目录创建。

## 提交、聚焦与恢复

一个绝对截止时间覆盖准备、启动与可选聚焦。元数据/控制交换限制为两秒；启动使用剩余预算并接入信号唤醒。所有准备先于变更完成。一次 `WorkspaceLaunch` 以 `Ungrouped` 或显式 `Project` placement 原子准入元数据、Directory 和第一个终端。没有独立 ProjectCreate 或刷新阶段。

后续工作前先保留已知 canonical 标识。终端 pending 或 started 时可继续可选聚焦。启动 failed、unavailable 或 unknown 时保留 Workspace 回执并停止聚焦。变更交接结果未知时保留原始请求标识，不重试、不回滚、不创建替代资源。

聚焦读取不早于提交修订的同 Session 新快照，全局解析返回的 Workspace，并核对精确 context/Tab/Panel 归属。Project 移动不使该作用域失效。Client 必须保持语义选择，或已通过首次 Workspace 协调选中精确的新 Workspace/Tab/Panel。context、Tab、Panel 或 zoom 改变时拒绝聚焦。一次 Context intent 使用新的 view revision 和既有 rendered/seen 确认。取消只停止等待，不取消 Server 已接受工作；后续失败保留先前回执。

## 机器输出与组织命令

所有 Windows 创建形式输出 `orchbee.workspace.creation` v1。`workspace.result` 包含 mutation v1 的资源标识及终端事实，`projectId` 必填但可为 null。外层记录目录来源、可选 Client/聚焦事实、阶段和结构化失败。已移除的外层 `project` 回执及 `project` 阶段会被拒绝。完成要求所有请求阶段成功；终端 pending 保持显式。解析/准备失败使用既有失败 envelope，包括 `workspace-creation` 阶段失败及未知结果原始请求标识。

已有 Workspace 可在不影响 PTY、cwd、Git 或文件系统的情况下组织：

```powershell
orchbee workspace set-project <workspace> --project <project>
orchbee workspace create-project <workspace> --label <label>
orchbee workspace ungroup <workspace>
orchbee project dissolve <project>
```

Create-project 原子创建 Project 并移入选中 Workspace。Dissolve 仅移除分组，保留全部 Workspace。标识、终端绑定、主 Directory 和全局顺序保持稳定。`workspace move` 与 `workspace move-block` 单独负责导航重排。

`workspace close <workspace> --confirm` 只关闭该 Workspace，保留 checkout 与分支。显式 `project close-all <project> --confirm` 关闭捕获的成员并保留 Project；参见 [Project 关闭](windows-worktrees.zh-CN.md#project-全部关闭)。

## 证据

语法与 schema 测试覆盖可选组织、精确全局目标、必填可空 Project 元数据、离线帮助/补全及先解析后副作用。Windows 进程脚本测试检查零/一/多个 Project 时都只启动一次、高级选项、来源优先级、精确副作用、未知身份不重放、终端失败与聚焦保护。原始路由和真实进程证据覆盖先语法后上下文、发送端/中继操作数不变、最终进程的 home/cwd 解析，以及早于配置或运行时副作用的上下文拒绝。fixture owner 释放临时 pipe、profile 和进程。真实终端验收属于匹配的原生 cohort 测试；这些局部测试不宣称覆盖全部并发竞态。

程序名使用最终 CLI 的 PATH，回执保留解析出的绝对路径；参见[可执行程序名](native-terminal-creation.zh-CN.md#可执行程序名)。
