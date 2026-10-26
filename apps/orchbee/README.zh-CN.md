# Orchbee 原生入口

## 会话

一个会话包含你的 Workspace、终端和 Agent；退出界面后，后台服务使其中的程序继续运行。
首次使用默认会话，用 Workspace 组织日常工作。Windows 支持通过 `--session <name>` 和
`session attach <name>` 使用独立的命名会话；`server` 命令管理所选会话的后台服务。
Agent 标识使用 `--agent-session <id>`。[会话契约](docs/native-cli-query.zh-CN.md#会话与后台服务)
规定术语、支持边界以及保持原样的存储／协议身份。

Windows 在所选会话内的终端再次启动 TUI 时，会在打开另一个 Client 前拒绝进入，并显示本地化
说明后返回当前 Shell；CLI 子命令及显式进入其他会话仍可使用。失败和已结束终端保留导航、
新建和关闭入口；[终端结束状态](docs/native-panel-lifecycle.zh-CN.md#终端结束状态)保留最终输出与已知退出码。

Windows `events subscribe` 从已有 Session 输出有界 NDJSON 观察。`events wait` 在等待片段间保留
同一连接及过滤器，区分匹配、确定超时、历史丢失、拒绝与中断。使用 `--event tab.renamed`、
资源选择器或封闭 `--filter` JSON；`--after-cursor` 固定保留日志历史。输出／滚动探测保留真实
终端和 Client viewport 所有权。运行 `orchbee events --help` 查看上限与条件；
[事件契约](../orchbee-server/docs/windows-events.zh-CN.md) 管理行为与最终 Windows 证据。

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` 在一个输入请求中粘贴命令，再发送 Enter。命令须为一个引号字符串；保持原始文本，目标只接受完整 ID 或 `@terminal:<suffix>`。回执确认输入提交，不保证 Shell 已就绪、命令完成或退出状态；部分或未知结果不得重放。 [命令提交契约](docs/native-terminal-run.zh-CN.md).

实验性的原生 Rust CLI 与 TUI Client。在 Windows、Linux 以及 Apple Silicon macOS 开发包中，
裸命令从已校验的可执行文件 cohort
连接或启动本地 Session，取得第一个已校验的终端 projection frame，或不可用终端的稳定导航视图后进入 TUI。原生 Client 操作共享
Workbench 状态，同时保持独立选择与保留的终端进程。Windows 还支持已保存机器的 SSH 命令路由和
TUI 实时切换、用户配置，以及完整 cohort 的版本安装与更新，具体契约见下方链接。
产物名为 `orchbee`，参与根 Cargo 验证。macOS 包是未签名、按用户隔离的 Apple Silicon 开发 cohort；
不包含发布签名或安装流程。

英文规范原文：
[README.md](README.md)。
如有冲突，以英文为准。

用 `agent rename <session-id> <name>` 设置运行时名称，`agent names` 列出名称，
`agent resolve <name>` 获取完整会话 ID。`agent rename <target> --clear` 释放名称。
`agent get/explain/wait <name>` 可直接解析名称；等待固定原会话，不因名称复用而改变目标。
名称唯一，在会话结束或 Session 停止后消失。

`agent read <name>` 读取唯一绑定的终端；`agent send-keys <name> enter`
通过既有终端输入回执发送语义按键。两者也接受 `--agent-session <id>` 和 `--name <alias>`，
绑定缺失或有歧义时拒绝操作。

`agent prompt <name> <text> --wait` 在新鲜且无阻塞的预检后提交字面 paste 加 Enter。
先观察活动再等待条件，后续超时或取消仍保留输入回执；不承诺轮次因果。

`agent start alpha --kind codex --panel <id>` 在空闲 Windows 终端中启动 Agent，
仅在进程和 canonical 就绪检查通过后绑定名称。配置、限制和恢复见
[Windows Agent 启动](docs/windows-agent-start.zh-CN.md)。

Windows `channel show [--format text|json]` 从共享用户配置读取严格的 stable/preview 偏好，不激活 Session。参见[配置及频道来源](docs/windows-cli-configuration.zh-CN.md#规则更新策略)。

Windows `machine list|add|rename|remove|enable|disable` 管理已保存的 SSH 配置，并在添加前检查远端 Windows 就绪状态。持久化、部分结果、命令路由和 TUI 实时切换见[已保存的 Windows 机器](docs/machines.zh-CN.md)。

Windows `update` 与 `channel set` 共用完整 cohort 安装、显式发布源、独立版本发布和单独的用户 PATH 激活。详见 [Windows 版本安装](docs/windows-cli-configuration.zh-CN.md#windows-版本安装)。

[TUI 平台边界](docs/tui-platforms.zh-CN.md)定义共享 Client 实现、可选 OS 适配、
本地化不支持结果与原生平台证据。状态、输入、菜单和渲染在 Windows、Linux、macOS
一起编译；可选 Windows 宿主副作用在相应平台适配实现前保持明确不可用。

## 命令面

裸调用和显式 `tui` 调用先准入继承的交互终端和有效 cell geometry，再在 Windows、Linux 与 Apple Silicon
macOS 上运行下面的本地启动流程。两者使用同一 Client 路径；`tui` 不选择另一种模式。
其他平台仍返回本地化不可用结果。
`--help` 与 `--version` 不初始化 TUI 或服务。命令使用下方的精确语法，不提供缩写；未知命令、未声明
选项、多余操作数和非 Unicode 参数都是 usage failure。Windows 远程选择遵循
[已保存机器的命令路由契约](docs/machines.zh-CN.md#命令路由契约)。

```text
orchbee                          # 本地终端（TUI）
orchbee tui                      # 显式打开同一终端
orchbee status                   # 运行中 Session 的 Workbench 快照
orchbee <family> <command>       # 查询与 Workbench 修改命令
orchbee api snapshot|schema      # 机器快照与内嵌 schema
orchbee server start               # 确保本地 Session 正在运行（无需 TTY）
orchbee server diagnose            # 只读的本地 Session 运行时事实
orchbee server stop --confirm      # 停止托管 Windows Session 及其终端
orchbee server stop-status <id>    # 查询停止尝试或完成回执
orchbee config check             # 本地运行时检查
orchbee help [<family>|<command>]
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --help | --version
```

查询 family 为 `client`、`project`、`workspace`、`tab`、`panel`、`terminal`、`worktree`、
`launch-profile`、`agent`（`list`/`get`，另有
`terminal read`/`wait-output`；Windows `worktree list` 接受 `--workspace <target>` 或
`--cwd <path>`，支持可选的命令局部信任；同时保留严格的 `--directory <directory-id>` 形式；
`launch-profile` 提供 `list` 与 `get <selector>`——对运行中 Session 启动配置目录的一次类型化读取，
由 Server 端解析、CLI 不做本地预读；
`agent` 提供 `list`（支持 `--include-ended`/`--limit`/`--cursor` 分页）、`get <session-id>` 与
`explain <session-id>`——经 OBAP 读取 Server 权威 Agent session 的只读类型化视图，绝不从终端
内容推断——`wait <session-id> [--until <condition>]... [--timeout <ms>]`，在持有的
Client Connection 上等待状态或新鲜度；Windows 跟随保留变更，默认无总超时，单次读取仍有界——以及
`doctor [--provider <id>]... [--asset-root <path>]...`，一个离线有界文件系统
诊断，仅凭本地事实解释 Agent 观察为何可能无法到达（不执行 provider 命令、不 connect socket、
不做任何写入）。
Windows `agent explain` 也会将名称和 Panel 解析为精确的物理终端绑定。
规范会话保留 `orchbee.agent.explain` v1；物理终端使用 v2，分别返回有效上报视图与一次新鲜的
屏幕规则评估。结果包含清单来源/版本、同优先级首条胜出规则、所有规则匹配及有界区域证据。
一次准入与 5 秒 CLI 总预算
覆盖解析和读取；Server 队列/读取预算为 4 秒。绑定或上报上下文变化时拒绝，不重试。
不发送输入、申请控制器或改变焦点。Windows 启动依次选择本地覆盖、符合条件的远程缓存及内置规则。物理诊断包含实际路径、缓存版本、覆盖状态及类型化警告。Windows `server agent-manifests`、`server reload-agent-manifests` 和 `server agent-manifests-status` 暴露已安装快照及保留的热重载回执。显式 `server update-agent-manifests` 下载并启用经过校验的缓存规则，分别保留逐 provider 缓存结果和启用回执。启用 `update.manifest_check` 时，托管发布版 Session 在启动时及每三十分钟自动检查；调试／测试及独立端点不调度。
参见[屏幕模型](../orchbee-server/docs/agent-screen-model.zh-CN.md)。

Windows `panel create-terminal` 启动未绑定主项或精确成员，或追加子项，不分割 Panel。
它保留字面启动选项、返回保留的等待／最终回执，不改变 Client 焦点。
详见 [Panel 终端创建](docs/windows-panel-terminals.zh-CN.md)。

作用于运行中 Session 的修改命令包括 `project create`/`rename`/`dissolve`/`close-all`、
`workspace create`/`rename`/`set-project`/`create-project`/`ungroup`/`accept-directory`/`close`、`tab create`/`rename`/`move`/`close`、
`panel split`/`move`/`close` 与 `launch-profile create`/`delete`——
`workspace close <target> --confirm` 在同一 tab-close lifecycle 下关闭 Workspace context 内
的全部终端，随后移除该 Workspace，保留其 checkout、branch 与目录；其异步操作可经
`workspace close-status --workspace <id>`（严格稳定 ID，含 tombstone）查询，
并可经 `workspace close-resume --workspace <id> --request <close-id>
--generation <n> --confirm` 恢复。
`launch-profile create --name <name> --exec <path> [--arg <value>]...` 在目录中存储一个命名
launch selection，`launch-profile delete <selector>` 将其移除。其中 `tab create` 与
`panel split` 还接受临时的
`--exec <path> [--arg <value>]...` launch selection 或 `--profile <selector>` 目录启动——二者互斥——
仅为该次请求替换解析出的默认 shell。`terminal send <id> (--text <utf8> | --file <path> | --stdin) [--enter] | --key <name>`
经同一已准入 session 向已绑定终端投递恰好一条输入记录——字面文本、常规文件内容或管道
stdin，均可加可选折叠回车，或十五个语义键之一——不接管 TUI
controller/viewer 所有权；其两轴收据分别报告 Server 的输入判定与 cleanup 状态。`project create`
提交既有 `ProjectCreate` 动作——一条导航记录，无目录或文件系统副作用——并返回
Server 分配的 project id，供 `workspace create`/`tab create` 链使用；
`workspace accept-directory` 提交既有 `WorkspacePrimaryDirectoryAccept` 动作——由
Server 权威探测该字面绝对路径——使 CLI 创建的 Workspace 无需 TUI 目录向导即可接纳 Tab。
`project rename <target> --label <label>` 提交既有 `ProjectRename` 动作——同一
"一次快照/一条命令" 信封先按精确 ID、显式 `@<kind>:<suffix>` selector 或精确 label
解析 target——并在专用
`orchbee.project.rename` 文档下报告 `committed`/`unchanged`。`server start` 是无头
激活路径：它执行与 TUI 启动相同的 connect-or-activate 序列——验证 cohort、准备
运行时目录、取锁、提交标记、在没有运行中 Session 应答时拉起 Server——全程共享一个
20 秒绝对 deadline，然后仅当本次调用实际拉起了子进程时报告 `started`，当既有
Session 准入了其探针时报告 `already-running`。该命令即使在拒绝时也可能产生效果：只有
语法失败保证零效果；被中断或超时的启动可能遗留已提交的激活状态，后续调用会按该
持久状态继续处理。Windows 下一次调用只有在全部 guard 下证明 v2–v5 完整登记的旧代确切进程
全部退出后才自动恢复；
活 owner、不完整登记和旧版 Active 记录仍为 `rejected`/`recovery-required`。恢复保留 checkpoint，
不终止旧进程。新激活会在提交标记、拉起 Server 或发布检查点存储前先发布 catalog v5；只支持
v1–v4 的读取器会拒绝 v5，且不改变其字节。这些边界由
[Windows 恢复契约](../../crates/local-runtime/docs/windows-crash-recovery.zh-CN.md)维护。
`server diagnose` 是只读对偶面：它报告运行时目录、标记、锁与有限端点路径的有界事实——
每行是各自观察瞬间的封闭状态行——共享一个协作式 5 秒预算；它绝不 connect、创建、
取锁、修复，也不声称任何进程存活或消失。
`agent integration`（`list`、`explain`、`enable`、`disable`）是本地 provider
接入命令族：仅在本机对调用用户的 provider 配置做有界读取与受管修改——
不建 socket、不取锁、不接触 Session；只有经核实归 Orchbee 所有的条目才会被
安装或移除。完整语法、
target 与 filter 规则、修改结果及含退出分类的 failure matrix 由
[native CLI query](docs/native-cli-query.zh-CN.md) 持有；`orchbee help <family>` 与
`orchbee help <family> <command>` 直接从二进制输出同一套语法。端到端的无头
序列——启动 Session、创建 Project/Workspace/目录、拉起并驱动终端、查看
worktree、观察 Agent 并干净地关闭——见
[原生 CLI 工作流](docs/cli-workflows.zh-CN.md)。

按一次会话通常需要的顺序列出常用命令模板——先按链接的工作流完成
Workspace 与 terminal 准备，再把每个 `<…>` 占位符替换为实际 ID 或
provider：

```sh
orchbee server start                        # 激活一次本地 Session（无需 TTY）
orchbee status                            # Workbench 是否可达？
orchbee project create --label demo       # 随后 workspace create / accept-directory
orchbee tab create --workspace <ws> --label shell \
  --cols 80 --rows 24 --cell-width 8 --cell-height 16
orchbee terminal send <terminal-id> --text 'make test' --enter
orchbee terminal wait-output <terminal-id> --match 'PASS'
orchbee agent integration enable --provider <id>   # 一次性本地接入
orchbee agent list                        # 运行中 Session 观察到的会话
orchbee agent wait <session-id> --until ended
```

Apple Silicon macOS 开发 cohort 支持查询 Session 的 `agent list` 及其有界分页选项。按 Client
投影的状态（`--client`）仍仅限 Windows；其他 Agent 观察命令继续遵循各自文档中的平台范围。

`orchbee help <family>` 直接从二进制输出同一套语法，
`orchbee completion <shell>` 打印对应的按键发现脚本，由你的 shell
自行 source。

帮助在每个层级使用共用的章节标题与顺序。根帮助提供快速开始、命令分组和全局选项；
命令族帮助列出直接子项，并为每个子项给出一行语法概要；命令帮助描述参数、选项、示例和操作约束。会话概念及新手引导
保留在本指南中，不另设帮助章节。

Help 只写 stdout，失败只写 stderr；Version 写入一行与 locale 无关的 `orchbee <version>`。
正常关闭 TUI、help、version 以及查询、修改、检查成功的开发退出状态为 `0`；usage failure 为 `2`；
其余失败分类以持有记录的 failure matrix 为准。这些不是未来稳定公开退出状态表。

## Windows 原生快速开始

`workspace create --cwd C:\existing\repo` 用一个请求创建未分组 Workspace 及首个终端。显式 `--project <target>` 添加可选分组；名称和目录均有默认值。可附加 `--env NAME=VALUE`、显式 `--exec`/`--arg` 或 `--profile`。回执返回资源 ID 与终端结果；启动失败时保留 Workspace，供查询和显式关闭。PowerShell 示例见 [Windows 工作流](docs/cli-workflows.zh-CN.md#windows-目录工作区)。

使用仓库固定版本的 Rust、Zig 工具链及 Node.js。在仓库根目录的 PowerShell 中运行；输出目录必须
尚不存在，且父目录已经存在：

```powershell
node scripts/stage-local-runtime.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
& "$env:LOCALAPPDATA\OrchbeeNative\orchbee.exe"
```

目录包含四个精确的同级 `.exe`、摘要清单和观察适配器 capsule，必须保存在一起。这是使用 Windows
命名管道和 ConPTY 的原生开发 cohort；交互宿主为 Windows Terminal，无需 WSL。新终端捕获共享启动配置与滚动历史默认值。
内置系统 Shell 选择绝对 PATH 中首个兼容 PowerShell 7 或系统 Windows PowerShell；显式旧
`terminal.default_shell` 只为未声明共享配置提供初值。
Shell 模式及启动生命周期见 [Windows 配置](docs/windows-cli-configuration.zh-CN.md#windows-配置-shell)。
`server start --format json` 也可在非交互控制台中运行。

Windows Session 中没有 Workspace 或终端内容时，`cd <project>; orchbee` 创建以当前目录命名的
未分组 Workspace，并在该目录启动首个终端。之后启动恢复最后显示的 Workspace、Tab
和 Panel；从其他目录启动不会增建 Workspace。详见[目录启动](docs/native-directory-startup.zh-CN.md)
及[启动位置](docs/native-context-selection.zh-CN.md#windows-启动偏好)。

前缀键为 `Ctrl+B`，随后按 `|` 左右分屏、`-` 上下分屏、`h/j/k/l` 切换窗格，或 `d` 脱离。
分屏默认聚焦新窗格；设置 `terminal.focus_new_panel = false` 并重启 Client 后可保留当前焦点。再次运行可执行文件即可重新连接保留的 Session。脱离后 Shell 继续运行；退出的 Shell
仍可观察，直至显式关闭其 Tab/Panel。全部已登记的 Session 进程退出后，再次运行可执行文件会自动开始
新代，并恢复已有 checkpoint。旧 owner 仍活着或无法证明退出时拒绝启动；当前恢复不接管活 Host。
`session list` v2 用 `recoverable` 报告受 guard 保护的冷恢复资格。持续 Workbench checkpoint
会保留布局变更及采样所得的终端目录，但不保存屏幕内容；详见
[Windows Workbench checkpoint 契约](../orchbee-server/docs/workbench-checkpoint.zh-CN.md)。
`config check --format json` 检查本地 cohort/runtime 输入，不启动 Session。

连接后，TooSmall 恢复界面会暂停上报 pane 几何，不将其误报为全部 pane 已离开。
宽度低于 20 或高度低于 5 时，终端 Viewer、controller grant、当前选择与最后有效的
子终端几何保持不变。恢复可绘制 viewport 后，无需分离即可继续正常观察与导航；
20×5 Minimal 边界仍上报真实 pane 几何。分屏和存在通知卡片时也遵循此规则。

Windows 已验证原生启动、Unicode 渲染、连续及逐字输入、CLI 终端发送/读取、分屏/焦点/调整大小、
脱离后保留精确终端身份并重新连接，以及 Git worktree 发现。Console 事件进入既有按键及 SGR 鼠标解码器；
退出 TUI 时由模式守卫恢复继承的输入/输出模式。大字节记录采用有界分段写入；取消 exchange 会关闭共享
管道，不留下分离的 flush 线程。

Windows `agent doctor --provider codex --format json` 现在可以检查本地配置、PATH、Node
和胶囊完整性，不启动 Session，也不改变 provider 接入配置。版本 3 文档报告 Windows 端点投递
尚未检查；已安装配置不证明 Agent 已接通。

Windows `agent integration list/explain/enable/disable --provider <id>` 可管理 Claude Code、Codex、Grok、GitHub Copilot、OpenCode、Kilo、Pi、OMP、Gemini 和 Qwen Code。详见 [Windows 集成契约](docs/windows-agent-integrations.zh-CN.md)。Windows `server diagnose` 使用 `orchbee.server.diagnose.windows` v2 报告只读运行时、锁和管道名称事实。Capsule 0.4.16 提供 Claude Code、Codex、Grok 和 GitHub Copilot hook 与 JavaScript 模块，
通过原生 observation 管道与 Console 关联投递。Windows x64 cohort 包含保留关联信息所需的
固定 ConPTY 运行时。其他 provider 保留既有平台限制。
`terminal send --stdin` 接受重定向普通文件和 Windows 管道；所属隐藏 Windows PowerShell
读取进程使管道输入可取消。它从操作系统系统目录解析，绝不通过 PATH 查找，失败时终止并回收。
Shell 补全提供 bash/zsh/fish/powershell/elvish。在 PowerShell 中显式运行
`Invoke-Expression (orchbee completion powershell | Out-String)` 加载，不会修改 profile。
在交互式 Elvish 0.21 中执行 `eval (orchbee completion elvish | slurp)`；静态回调注册
`orchbee` 与 `orchbee.exe`，不查询 Session 或修改 profile。
在 Windows PowerShell 5.1 中使用 Unicode 管道时，在调用脚本内设置 `$OutputEncoding = [Text.UTF8Encoding]::new($false)`；传给原生命令的字节由 PowerShell 编码。Orchbee 原样保留字节，不修复 shell 已经造成的编码损失。
`orchbee --skill` 离线输出内嵌 [Agent 指引](docs/cli-agent-skill.zh-CN.md)。操作系统服务安装、发布签名、
应用自动更新和活 Host 接管仍不在这些命令的范围内。

## macOS 与 Linux 本地启动

从仓库根目录构建一个新的开发 cohort：

```sh
node scripts/stage-local-runtime.mjs --output /tmp/orchbee-native-dev
# Windows 主机使用 WSL Linux：
node scripts/stage-local-runtime.mjs --wsl Ubuntu --output /tmp/orchbee-native-dev
# Apple Silicon macOS：
node scripts/stage-local-runtime.mjs --output /private/tmp/orchbee-native-dev
```

随后在交互式终端执行暂存的 `orchbee`（Linux 使用 `/tmp/orchbee-native-dev/orchbee`，
macOS 使用 `/private/tmp/orchbee-native-dev/orchbee`）。
脚本从锁定输入重新构建三个 Rust release 二进制和 Zig ReleaseSafe Host，
将规范的观察适配器胶囊复制为 cohort 的惰性校验 sidecar，并在写清单前校验其声明的负载摘要；
已有输出目录会被拒绝。可用 `--cargo-target-dir <absolute-directory>` 复用 Cargo 构建缓存。
这会暂存一个开发 cohort，不安装系统服务。

Linux 默认 Session 位于 `/tmp/orchbee-native-<effective-uid>`，macOS 默认位于
`/private/tmp/orchbee-native-<effective-uid>`。开发和测试可将 `ORCHBEE_RUNTIME_BASE`
指向准入本机文件系统上的另一个已有绝对目录来隔离 Session。
[共享运行时契约](../../crates/local-runtime/README.zh-CN.md) 定义 0700 目录、
0600 锁/socket/不可变启动标记、声明适配器 sidecar 的四可执行文件精确 cohort、
20 秒共用截止时间和 500 毫秒延迟提示。
Server + Worker 就绪与终端首帧就绪是不同阶段。初始 Open、snapshot read、attach、acquire、
获权后的 resize 以及首帧 Poll 都使用原启动截止时间。原生 Ready 要求完整帧已绘制、其精确反馈已获接受，
并满足当前控制模式尺寸。Worker 消费 Ready reply 后才恢复常规请求预算。
未绘制的 FrameApplied 不会提前发送；NeedSnapshot 恢复反馈可以继续推进首帧获取。

Client 不重启或接管异常 Session。失败/残留运行时明确要求恢复，不自动删除。
`Ctrl+B q` 或 `Ctrl+B d` 只关闭 Client；再次打开连接同一个保留 shell。
同时打开的其他 Client 只读，除非它取得独立 controller grant。
Windows 上用 `Ctrl+B t` 显式接管当前 pane，并提供本窗口的 pane 尺寸；普通打字和点击
从不接管。原 Client 继续查看，在下次输入或 resize 交互时确认只读；
见[原生控制权接管](docs/native-control-takeover.zh-CN.md)。
TTY 只准入一次，所有复制描述符均带 CLOEXEC，准备好的 Session 和同一 TTY 交给界面。
当前要求终端报告真实且有效的 cell/pixel 尺寸；成对 `(0,0)` 像素上报作为诚实的未知像素声明被准入，绝不虚构缺失的像素值。
当上报的几何不可绘制时（包括临时零单元格上报），Client 会在原始截止时间之内等待真实
的可绘制几何，而不是准入失败或虚构几何；[原生启动等待](docs/native-startup-wait.md)
拥有该有界等待模型、其取消/超时/故障结果以及不执行任何服务工作的保证。
Linux 启动已有公共进程证据；macOS Apple Silicon 暂存和 TUI 启动按 APFS/ACL 准入完成原生验证；
Windows 原生启动与 ConPTY 流程证据见上文。尚不承诺发布性能预算。

## 原生 Tab 交互

原生 Session 呈现当前上下文的有序 Tab 和 canonical 标签。每个 Client 独立拥有活动选择、
navigator 与 viewport；另一 Client 已提交的新 Tab 经自动 publication 探测可见，显式刷新仍可用。以下操作沿用 `Ctrl+B` 前缀：

| 按键                   | 已连接原生行为                                                                                                                          |
| ---------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| `c`                    | 请求一次 Server 拥有的 Tab 创建，再选择已验证的提交成员。Windows 当前未打开工作区时改用显式工作区创建。                                 |
| `n` / `p`              | 在当前 Client snapshot 中选择下一个 / 上一个 Tab。                                                                                      |
| `1` … `9`              | 该位置存在时选择它。                                                                                                                    |
| `[` / `]`              | 选择上一个 / 下一个可见工作区（含非空历史内容），跳过空兼容槽。                                                                         |
| `Shift+N`              | Windows 上使用默认名称和目录创建未分组 Workspace 及首个 shell，然后进入它。                                                             |
| `a`                    | 使用同一一步创建 Workspace 流程。                                                                                                       |
| `e`                    | 配置当前尚未配置且为空的 Workspace 的首个目录。                                                                                         |
| `r` / `R`              | 重命名当前规范 Workspace / 所属 Project；历史内容保留终端操作，并以本地化说明限制规范操作。                                             |
| `<` / `>`              | 将活动 Tab 移到相邻项之前 / 之后，在两端循环。                                                                                          |
| `,`                    | 编辑所选 Tab 的共享 label；首次完整绘制后 Enter 提交，Escape 取消。                                                                     |
| `x`                    | 确认关闭所选 Tab 对所有已连接 Client 的执行，或取消。                                                                                   |
| `w`                    | 打开 navigator 并刷新共享 Tab，保留活动终端身份。Windows 中用方向键／Tab 选择，Enter 打开，Esc 返回终端。                               |
| `h` / `j` / `k` / `l`  | 当前 Tab 有分屏树时，将本地焦点移到相邻 Panel。                                                                                         |
| `-` / `_` / `\` / `\|` | 将焦点 Panel 向下 / 向上 / 向左 / 向右分屏出第二个叶子（临时键位）。                                                                    |
| `X`                    | 为焦点 Panel 叶子请求关闭确认——确认完整绘制后 `Enter` 确认关闭，`Esc` 取消；末叶会指名整个 Tab 对所有已挂接 Client 的影响（临时键位）。 |
| `H` / `J` / `K` / `L`  | 将焦点 Panel 叶子移向其方向邻居（临时键位）。                                                                                           |
| `P`                    | Windows 上进入[键盘调整模式](docs/native-keyboard-resize.zh-CN.md)：方向键移动分隔线 5%，Shift+方向键 1%，Esc 退出。                    |
| `q` / `d`              | 关闭此 Client，保留的执行继续运行。                                                                                                     |

[上下文选择](docs/native-context-selection.zh-CN.md) 在切换位置时保留各终端的执行。
Workspace 身份独立于可选的 Project。侧边栏将 Agent 直接列在各 Workspace 下，支持同时
展开多个 Workspace 的 Agent 列表。[Workspace 设置](docs/native-workspace-setup.zh-CN.md)
在当前聚焦 Workspace 的目录（没有时为启动目录）立即创建未分组 Workspace 与首个 shell；
“在指定目录新建工作区…”才打开可编辑的选择器；从 Project 菜单创建则
显式加入该 Project。之后可重命名、整理分组；目录接受绝不启动终端。已知部分结果可通过
`orchbee workspace list` 检查；未知结果绝不重放，延迟完成不能覆盖更新的选择。
Windows TUI 保持侧栏，提交后按已选名称显示位置与行高稳定的待创建项，终端启动继续异步进行。
`Ctrl+B W` 在发现来源后，用生成的分支名和默认起点创建 Git 检出目录；`Ctrl+B O` 按精确
路径选择已有检出目录。[Worktree 打开流程](docs/native-worktrees.zh-CN.md) 添加同级 Workspace，
沿用来源的当前 Project，或将未分组来源和新 Workspace 原子地加入新 Project。其他 Client
保持原选择；提交后 Escape 隐藏观察界面并保留操作 ID。

Windows `Ctrl+B F2` 打开[动作菜单](docs/native-action-menus.zh-CN.md)。
右键点击 pane、tab、Workspace 或 Project 行即可发现已有动作。打开和取消保持焦点；
已寻址的组织与删除动作也保留当前终端。Project 重命名为准确的目标打开编辑器，
包括空和非当前 Project，不进入子 Workspace；重命名或取消期间仍选中当前终端。
上下文菜单仅使用矩形边框、动作行及可选的禁用原因，不显示标题、X、页脚提示或选择计数。
子菜单继承根菜单的构件策略；全局动作菜单保留按键发现控件。
鼠标打开的菜单靠近点击单元格，并限制在状态行上方。悬停只选择可见行，不执行；点击或 Enter
执行，Esc 或外部点击取消。
Workspace 的 Project 子菜单提供新建 Project 或离开 Project，之后列出已有 Project。
Worktree 子菜单使用首个终端的根进程 cwd；Project 从规范顺序的首个成员推导该来源，
并不拥有目录。来源未解析或不是 Git 时省略该子菜单。红色删除动作使用独立的破坏性强调色：
删除 Workspace 关闭该 Workspace；删除 Project 只解散组织，保留所有 Workspace 与程序。
只有权威的从未创建／空运行时证明才跳过确认。已有运行时的作用域需要审阅；待处理或不可用的
权威保留准确拒绝。关闭全部 Workspace 仍是独立的兼容动作。

`Ctrl+B Shift+D` 打开 Windows [工作区关闭确认](docs/native-workspace-close.zh-CN.md)。
完整绘制后，无修饰的 `Enter` 只结束该 Workspace 的终端，影响所有 Client；Esc 取消。
Git 目录保留，规范移除后选择存活工作区，或显示显式的无工作区新建入口。
[位置标签](docs/native-location-labels.zh-CN.md) 使用预填编辑器与共享 Server 命令；
重命名保留运行中终端，peer 通过 publication 探测或显式刷新看到变化。

Windows 上，用无修饰左键拖动可见 pane 分隔线，pane 及受控终端视口随鼠标移动实时调整，
松手后应用一次共享并保存的比例。侧栏边框也在按住期间改变实际布局。Esc、导航或外部
viewport 变化取消局部草稿，Shift 仍交给外层终端。提交中再次拖动不会排队。
[分隔线调整](docs/native-divider-resize.zh-CN.md) 共用键盘调整的确认流程，不改变终端控制权。

Windows 上，无程序鼠标跟踪的 pane 中，无修饰拖动选择实时或保留历史文本；双击选词，松开后
将 Host 的完整导出复制到系统剪贴板。`Shift` 拖动仍交给外层终端。复制失败使用本地化提示，
绝不重放。[选择与效果](docs/native-terminal-selection-effects.zh-CN.md) 拥有剪贴板依赖和生命周期；
pane 程序写入、OSC 52 模式和剪贴板配置仍需接入。

[Windows 自定义命令](docs/windows-custom-commands.zh-CN.md) 让 `command list`、`command invoke`、
配置快捷键和快捷键发现共用一个已安装目录。Shell 作业、Panel 创建、独立弹窗和插件动作
保留各自既有的执行所有者与回执。

创建会排空此前已接受的工作，并在截止时间或 Client 退役时保留已知提交事实。
切换先释放旧 viewer，再附着所选 binding；不会停止或重建任一终端。
移动改变共享顺序，同时保留活动终端及其 viewer/controller。如果 Move 已提交且快照返回明确的 denied/unavailable 结果，
Client 保留终端交互，并通过 `w` 显式恢复快照后才允许下一次结构操作。

Rename 精确保留最多 512 个 Unicode 标量。Bracketed paste 作为一次原子编辑落地——payload 内的
prefix、方括号、换行字节绝不会升级为命令。Backspace 删除一个标量，Ctrl+U 清空草稿；无修饰的
Left/Right/Home/End/Delete 以及 Ctrl+A/Ctrl+E、Ctrl+W 作为语义编辑到达草稿，而带修饰或未绑定的
序列被整帧吞掉、不取消草稿。已知 Rename 提交但投影不可用时，同样通过 `w` 显式恢复，绝不重发命令。
[原生 Session 契约](docs/workbench-client-runtime.zh-CN.md) 定义准入、尺寸、反馈、失败、恢复与证据边界。
Close 要求独立生命周期授权，只读 Client 也适用。关闭最后一个 Tab 后留下 Empty；显式 `c`
创建新终端，普通 Client 退出则保留该终端供后续绑定附着使用。终端被关闭的 peer 通过显式 `w`
恢复当前成员关系。Linux 进程证据覆盖此流程。规范分屏投影、本地面板焦点及
`panel.split` / `panel.close` / `panel.move` 提交保留 Tab 的规范分组和固定元数据；生产重启恢复仍未完成。
面板操作复用同一结构变更准入：split 提交实际控制 cell/pixel 几何并沿用叶子 label，close 以
叶子为作用域并经普通 publication 路径提交，move 将叶子重定位到其方向邻居旁。关闭 Tab 的末叶
会移除整个 Tab——其终端执行被处置，Tab 对每个已挂接 Client 结束，与显式 `x` Tab close 警告的
后果相同——而临时 `X` 键位只能经由已绘制的 [panel 关闭确认](docs/native-panel-close-confirmation.zh-CN.md)
到达该 close：确认指名被冻结的叶及其影响后，才由无修饰的 `Enter` 准入。Panel 命令是在 Server `WorkbenchGrant` 能力下准入的 Workbench 变更——split 需要
`mutate_navigation` 与 `create_terminal`，move 需要 `mutate_navigation`，close 需要
`close_terminal`（[Workbench authority](../orchbee-server/docs/workbench-authority.zh-CN.md)）——
该层独立于终端侧控制：viewer 附件既不授予 input 也不授予 resize 权威——其
`read`/`input`/`resize` capability 标志只作用于该附件本身——input 与 resize 只经终端唯一的
合并 input-and-resize controller 授权流动，该授权绑定于一个 Session、viewer、Worker 代次与
Host 终端
（[`schemas/client-server/README.md`](../../schemas/client-server/README.zh-CN.md)）。终端的
input/resize 控制绝不授权 Workbench 变更，只读附件无论持有何种 Workbench 授权都不能 input 或
resize。与聚合 Close
重叠产生的 terminal-busy 拒绝属于确定的未应用结果；焦点叶子被提交移除时，Session 将成员
关系恢复到同一 Tab 的存活叶子上。`-`、`_`、`\`、`|`、`X` 与 `H`/`J`/`K`/`L` 键位为临时
绑定，待输入设计裁决后可能重绑；split 与 close 键形遵循 input-vocabulary 草案表，而
`H`/`J`/`K`/`L` 是草案 `PANEL` 模式 `h`/`j`/`k`/`l` move 的过渡性顶层替身。

## 私有本机 Client transport

在 Unix 上，application-private `LocalClient` 只从 internal caller 接收 endpoint 与 executable
cohort。它把 admission 委托给 `orchbee-local-client-protocol`：该 crate 在连接前验证 cohort，证明
Server 与当前进程具有相同 effective operating-system user，以两秒 read/write timeout 执行确切
`OBLC` handshake，并返回 accepted generation 与同一条无 buffer stream。这些既有 handshake
timeout 仍是独立的 OBLC boundary。在 Linux 与 macOS 上，私有 `LocalClient::connect_before` 将调用方的
绝对单调时钟截止时间贯穿连接、peer proof、hello、响应与最终准入。
[所属 adapter 与正确性记录](../../crates/local-client-protocol/README.zh-CN.md#posix-有截止时间的连接准入)
定义失败清理与截止时间不续期保证。两条路径都在 `OBCS` 前将 admitted stream 切换为
nonblocking I/O。常规请求有独立预算；本地初始请求受原启动截止时间约束。

`submit_terminal_open`、`submit_terminal_create`、`submit_terminal_start`、`submit_terminal_viewer` 与 `submit_terminal_control` 各自取得一条已经验证的 typed request 的
ownership，并在提交前 encode 完整 OBCS frame。Encoding failure 是 definite failure，并保留
Client Connection。Encoding 成功后启动一个五十秒 monotonic deadline，覆盖完整 frame write 与一次 Server-frame
read；每次 readiness wait 都重新计算剩余 budget，因此 partial progress 不会续期。第一次 write
syscall 之前的 failure 是 definite failure。该 syscall 一旦开始，任何 write 或 read failure、
deadline expiry、malformed 或 incompatible result、错误 result kind 或错误 correlation，都会使
effect outcome 成为 typed `OutcomeUnknown` 并让 connection 退役。transport 绝不 retry，也不返回
request 供 replay。Server 返回的正确 correlated `outcome-unknown` 是有效 OBCS result，而不是
transport ambiguity。

Linux 与 macOS 裸入口通过该 transport 读取规范 Workspace 选择并提交查看和控制意图。仅允许关闭的 `LocalClientShutdown`
handle 停止阻塞 exchange，不增加 stream reader 或 writer。重连和结果协调仍未提供。

五十秒提交上限容纳 Server 一个排队及一个在途 Worker exchange（各自最多两秒 write 加二十秒
read）和 Client 结果交付。Handshake 限制保持独立。查看提交验证 action/outcome 兼容性与精确
correlation，并保留 opaque OBSR byte 供已连接的 projection consumer 使用；它不授予 controller 权限，
也不执行终端呈现解码。控制提交针对其 action 验证精确 grant 获取、input acknowledgement 和
完整 resize geometry。Server 报告的 unknown control outcome 原样返回且不重试；不匹配的 outcome
让 transport 退役。Caller 拥有新的 request identity、绑定 grant 的递增 sequence 和后续显式获取。
Client Connection 丢失后的 cleanup 可超出单次提交 deadline。

所有操作既有 viewer 的 action 和所有 controller action 都携带 Server 签发的精确 `attachmentId`。
原生 executor 在 Attach 成功后立即保留该身份，早于任何可能失败或被取代的 setup。
Poll、控制、projection feedback 和 cleanup 都使用这个已捕获的身份；有界 busy 重试保留它，
只更新契约允许刷新的 request identity 和 control sequence。提交前验证 projection feedback
属于其 attachment，绝不重新指定目标。`LocalClient` 不保存隐式的当前 viewer。
已连接的原生 Client Connection 中，活动 Tab 规范 Panel 树的每个可见叶子都拥有自己的 viewer 附着
——逻辑 `PaneKey`（{选中 anchor、`WorkbenchPanelId`}）保持稳定，其物理附着携带按记录铸造的
`PaneRef` incarnation。Input 与 resize 仅放行焦点 pane 的精确 incarnation；poll 对每个已
放行 pane incarnation 持续进行，后台叶子持续渲染自己的帧。同 Tab 的 `h`/`j`/`k`/`l` 焦点
移动是本地选中变更——绝不 detach、重新附着或重编已被接受的工作；跨 Tab 选中仍走既有的
switch 清理/附着链。已 detach 并重新绑定的 pane 会栅栏所有携带其已退 incarnation 的迟到
input、帧、feedback 与结算。viewer 容量仍受共享 HostViewers 上限（8）约束。

Windows worker 完成现在会唤醒活动 shell 的输入等待；普通空轮询按 8 ms 间隔放行。再次访问
Tab 时，可从最多 8 张画面／计费 16 MiB 的缓存立即显示该 Tab 自己在相同单元格尺寸下保留
的实时文本，而输入和反馈仍由新附着的 readiness 放行。首次访问及缓存／尺寸未命中时保留正常
首帧路径。身份、生命周期、缓存计费及受控时序证据见
[呈现调度与保留画面](docs/workbench-client-runtime.zh-CN.md#呈现调度与保留画面)。

### 提交正确性记录

| 字段   | 契约                                                                                                                                                                                                                                                                                                  |
| ------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 持有 already admitted same-user Client Connection 的 internal Client 必须提交一条 typed Create、Start、查看或控制 request，并在 transport completion 含糊时避免重复 effect。Endpoint acquisition、process activation 与 presentation 不在范围内。                                                     |
| 输入   | Trusted typed `TerminalCreateRequest`、`TerminalStartRequest`、`TerminalViewerRequest` 或 `TerminalControlRequest`、admitted `ClientConnection`、accepted generation、finite request budget 与 monotonic time。Server byte 与 readiness outcome 不可信。connection 缺失表示 `ConnectionUnavailable`。 |
| 数据   | 一次 invocation 拥有一个 `requestId`、encoded frame、absolute deadline、connection state、possible-handoff boundary、expected result kind 与 decoded result class。`LocalClient` 独占其 Client Connection。                                                                                           |
| 输出   | Exact correlated typed result；definite `RequestEncoding` 或 `RequestNotSubmitted`；带 submitted identity 的 typed `OutcomeUnknown`；或 `ConnectionUnavailable`。不存在 cancellation、retry、replay return 或 optimistic terminal mutation。                                                          |
| 不变量 | Encoding 先于 deadline 与 transport；一次 invocation 至多开始一次 frame handoff；只有 exact kind 与 identity 才完成；possible handoff 后的每种 ambiguity 都让 Client Connection 退役；retired Client Connection 不发送 byte。                                                                         |
| 决策   | 第一次 possible handoff 前或后；write 未完成或完成；result 在 deadline 前或等于/晚于 deadline；exact 或错误 kind/identity；有效或 malformed frame；known 或 Server-reported unknown outcome。若 acceptance 尚未完成，deadline observation 胜出。                                                      |
| 模型   | 单次调用在一个总截止时间下串行提交请求，每项至多一次。`LocalClient::exchange` 是 implementation workflow；其 deadline-aware nonblocking stream wrapper 是 time 与 I/O adapter。                                                                                                                       |
| 转换   | 下方 transition record 定义 serializable abstract phase 与 guard。Runtime phase 是 call-local；只有 live-or-retired connection state 跨 call 保留。                                                                                                                                                   |
| 依赖   | 一条 already admitted Unix stream、monotonic time、readiness polling、OBCS encoding/decoding 与 same-stream OBLC admission。OBCS workflow 没有 filesystem、process、endpoint-selection、randomness、localization 或 UI capability。                                                                   |
| 所有权 | `LocalClient` 是唯一 stream 与 retirement owner。OBCS 拥有 frame 与 result semantics；OBLC 拥有 admission、credential、cohort、generation 与既有 handshake timeout。                                                                                                                                  |
| 验证   | 假 Server 测试覆盖准确 Create/Start、大 OBSR 记录、不兼容 viewer/control 结果、resize 几何、关联、协议/EOF 失败、静默超时、部分 I/O 期限耗尽、写入前过期、退役及无重复发送。                                                                                                                          |
| 正确性 | LC-01 至 LC-05 映射到下方实现分支与确定性测试。不保留独立提交 TLA 模型。这些案例不证明 OS 调度、任意编解码输入、激活、重连或协调。                                                                                                                                                                    |

### 提交转换记录

| 当前 phase                         | Event 与 guard                                                 | 下一 phase         | Result 与 connection effect                                                    |
| ---------------------------------- | -------------------------------------------------------------- | ------------------ | ------------------------------------------------------------------------------ |
| `ready`                            | Client Connection 存在时提交                                   | `encoding`         | 无 transport effect                                                            |
| `encoding`                         | Frame encoding 失败                                            | `definite-failure` | `RequestEncoding`；Client Connection retained                                  |
| `encoding`                         | Frame encoding 成功并创建一个 absolute deadline                | `write-ready`      | Encoded frame 仅属于本次 invocation                                            |
| `write-ready`                      | 第一次 write syscall 前 deadline 或 I/O setup 失败             | `definite-failure` | `RequestNotSubmitted`；Client Connection retained                              |
| `write-ready`                      | 第一次 write syscall 可能向 kernel 移交 byte                   | `write-begun`      | Handoff count 变为一                                                           |
| `write-begun`                      | 唯一完整 frame 在 expiry 前写完                                | `awaiting-result`  | 在同一 deadline 下等待恰好一条 Server frame                                    |
| `write-begun` 或 `awaiting-result` | Deadline、I/O、timeout 或 EOF 胜出                             | `retired`          | `OutcomeUnknown`；connection closed                                            |
| `awaiting-result`                  | Decoded frame 在 expiry 前为 malformed、wrong-kind 或 wrong-ID | `retired`          | `OutcomeUnknown`；connection closed                                            |
| `awaiting-result`                  | Exact correlated result 已 decode 且 deadline 仍 open          | `completed`        | 返回 typed known 或 Server-reported-unknown result；Client Connection retained |
| `retired`                          | 后续任何 submit call                                           | `retired`          | `ConnectionUnavailable`；不 write 且不 retry                                   |

### 正确性可追溯性

| Claim                                                                                                           | Implementation 与 deterministic witness                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LC-01：encoding 与第一次 write 前的每种 failure 都是 definite。                                                 | `exchange` 在启动 `SubmissionDeadline` 前 encode；`deadline_expiry_before_the_first_write_is_a_definite_failure` 观察到 Server 收到零 byte 且 generation 保留。           |
| LC-02：一次 invocation 至多有一次 request-frame handoff，且绝不 retry。                                         | 一个 encoded buffer 只传给 `write_all` 一次；slow-write 与 silent-timeout test 拒绝第二条 request，后续 call 返回 `ConnectionUnavailable`。                               |
| LC-03：possible handoff 后的每种 failure 都是 unknown，并让 Client Connection 退役。                            | Write/read、EOF、incompatible frame、wrong kind 与 wrong identity branch 都调用 `retire_with_unknown`；focused test 断言 generation 消失。                                |
| LC-04：只有 exact kind 与 identity 才完成；correlated Server `outcome-unknown` 是数据而非 transport ambiguity。 | Exact Create/Start、wrong-kind、wrong-ID 与 correlated-unknown fake-Server test 覆盖 result classifier。                                                                  |
| LC-05：一个 monotonic deadline 覆盖完整 write 与 read，progress 不会续期。                                      | `SubmissionDeadline` 只创建一次；每个 readiness boundary 重新计算余量。Stepping-clock test 在 read 与 write 两侧制造 sub-deadline byte progress，直到 total budget 用尽。 |

## TUI shell 底座

Application-private shell adapter 拥有以下终端与呈现边界。裸 Linux 与 macOS 启动使用准备好的 Workbench Client；
未连接的 `run` 与显式 `run_connected` 仍是测试与组合接缝。

下面的输入与转换表描述未连接时的 fallback。已连接的上下文和 Tab 操作由上文的原生 Workbench Client 处理。

| 关注点      | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 准入        | 已暂存的 Unix adapter 要求继承的 stdin 与 stdout 都是 TTY，且具有相同 terminal-device identity；它只复制这些已准入 descriptor，并从复制后的 stdout descriptor 读取 cell 与 pixel dimension。它没有 `/dev/tty`、Crossterm global size 或 process fallback。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 生命周期    | 一个已暂存 guard 拥有 raw mode、alternate screen 与 cursor visibility。它会在 operation 可能部分成功前标记 cleanup responsibility，在 inverse 成功前保留每个 flag，并为每个失败的 inverse 最多尝试两次，随后返回一个 aggregate error。Process-lifetime singleton panic hook 只在发生 panic 的 thread 是 active guard owner 时派发，因此其他 thread 无法在保存 terminal state 前清除 setup lease；guard drop 不会移除后来安装的 hook。正常返回、initialization/event/draw failure、owner-thread panic，以及 `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` 中的每一种，都有确定性 restoration 或 signal-observation 证据。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Client View | 每个 invocation 拥有 prefix mode、viewport class、sidebar projection、active overlay 与 transient notice。它不拥有任何共享 Tab、Panel、Workspace、Workbench Client 或 Terminal fact，也不持久化任何内容。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| 布局        | [Shell 布局](docs/tui-shell-layout.zh-CN.md) 拥有视口级别、有界侧栏宽度与终端几何。Windows 侧栏将 Agent 嵌在精确归属的 Workspace 下面（包括非空历史内容），共用一棵可滚动树；展开操作保留终端选择与键盘归属。Tab 顶栏和侧栏的 `+` 控件复用已有创建动作及菜单。规范 Panel 身份与终端归属保持不变。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| 输入        | 已暂存 adapter 每次最多从已准入 stdin descriptor poll 并读取 4,096 个 raw byte。一个有界 legacy decoder 把字节流转成 typed item——一个 UTF-8 标量、一个 CSI/SS3 或 kitty-`u` 键、一段原子 bracketed-paste、或一个 non-decision 完整序列。每个正在收集的 frame（ESC 等待、CSI/SS3、UTF-8 标量、paste 段）在有界注入等待内跨 read 持有自己的字节；在被捕获 packet 中打开的 frame 即使跨 read 完成也保留其 owned 归属，越界或超时 frame 会 drain 到可靠边界而不是让尾巴漏进 binding，paste 段只按 opener 记法配对的终结符结束。legacy decoder 对无效 UTF-8、越界或超时的粘贴只产生一次拒绝，不保留 raw 内容，然后静默消费到对应终止符；Pane 不会收到被拒绝的片段，已启用的 prefix 仍保持等待。attach 和文本编辑器保留已有的拒绝反馈，普通 Pane 与 prefix 分派不增加全局提示。每次完整 read 都会先依序 dispatch，之后才允许后续 signal、viewport query 或 resize decision 结束本轮 loop。`Ctrl+B` 进入可见 prefix mode。`b`、`w`、`?`、`q` 与 `d` 驱动本地 shell state 或退出；已连接的 `c`、Tab 选择和 `h`/`j`/`k`/`l` action 由 Workbench Client 处理。没有命名所有者或独占捕获时，普通 byte——包括 `Ctrl+C`、`Ctrl+D`、escape sequence 与 UTF-8——都会逐字节按原序到达可注入 terminal-input port；单独的 `Esc` 在有界等待后才 forward，prefix 后的 `Ctrl+B` 会在当前所有者允许时转发一个 literal byte。快捷键帮助打开时捕获普通输入，`Esc` 会关闭它且不会转发；模态任务和确认优先于捕获覆盖层准入的关闭或切换前缀绑定。 |
| 结果        | Shell 消费准备好的 Workbench Client。Bootstrap 拥有激活，Server 拥有终端身份，Host 拥有终端生命周期。Shell 不改变 Workbench 状态。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

确定性 Client View transition model 如下：

| 状态与事件                                     | 下一状态、命令与可见结果                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Normal + `Ctrl+B`                              | 进入 Prefix；不发出 terminal input；渲染 prefix indicator。                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Normal + 其他 byte，无输入所有者               | 保留本地 view state，并将确切、有序的 byte 提供给 terminal-input port。Unavailable port 会在无副作用的情况下拒绝，已暂存 renderer 会报告 input 未发送。                                                                                                                                                                                                                                                                                                                                                 |
| Prefix + `b`                                   | 离开 Prefix，并为此 Client View 切换停靠的宽屏 sidebar 或窄屏 sidebar overlay；重复绑定会关闭它。                                                                                                                                                                                                                                                                                                                                                                                                       |
| Prefix + `w` 或 `?`                            | 离开 Prefix，并打开 Workbench navigator 或 shortcut Help。Windows 导航器捕获键盘与粘贴：方向键和 Tab 浏览，Enter 打开已绘制目标，Esc 或其切换绑定会关闭；全局前缀动作由绑定所有者切换表面。Help 捕获普通输入；Esc 关闭它，重复 `?` 也会关闭，其他全局前缀动作经同一动作所有者切换表面。                                                                                                                                                                                                                 |
| Prefix + `PageUp` 或 `PageDown`                | 离开 Prefix；帮助 overlay 打开时滚动 8 行，Up/Down 滚动 1 行。帮助按本地化动作领域分组展示有效全局绑定，并在复制、调整大小或侧栏导航模式仍活动时同时展示该模式的本地绑定；帮助捕获可打印输入，以模糊筛选动作名称和按键标签；Backspace 编辑，Ctrl+U 清空，Esc 关闭且不会转发到终端。滚动位置属于 Client-local state，并按精确绘制视口结算。帮助关闭时，Windows Prefix + PageUp 进入保留历史，PageDown 仍未绑定；其他平台保持未绑定行为。在独占捕获与保留历史外，无 Prefix 的按键原样到达 terminal port。 |
| Prefix + `g`                                   | 切换 Agent 看板；重复绑定会关闭它。其他配置的前缀动作经常规 Client 动作所有者切换；模态任务与确认优先捕获输入。                                                                                                                                                                                                                                                                                                                                                                                         |
| Windows 上 Prefix + `P`                        | 进入 Client 本地调整模式。方向键及 Shift+方向键调整共享比例；未绑定键和粘贴被消费，`?` 显示模式帮助；若 Help 已打开，Esc 关闭 Help，否则 Esc 将输入交回 pane。                                                                                                                                                                                                                                                                                                                                          |
| Prefix + 其他完整键或完整序列                  | 离开 Prefix 并报告 unknown-prefix notice；一个完整的未知序列（CSI 尾巴、UTF-8 标量、带修饰键）恰为一个未绑定事件，绝不会把字节留在 pane 中。                                                                                                                                                                                                                                                                                                                                                            |
| Prefix + `c`、`n`、`p`、`1` … `9` 或 `h/j/k/l` | 离开 Prefix；已连接 Workbench Client 执行对应的 Tab 或本地 Panel 焦点操作，未连接 fallback 保持共享状态不变并显示不可用；不由 shell fallback 提交 command。                                                                                                                                                                                                                                                                                                                                             |
| Prefix + `m`                                   | 离开 Prefix，并通过租约模式对释放或恢复宿主指针捕获；released 期间状态行显示持久指示，宿主原生选择与滚动不受影响。鼠标捕获、命中测试、pane 透传与滚轮回退由[原生鼠标输入](docs/native-mouse-input.zh-CN.md)治理。                                                                                                                                                                                                                                                                                       |
| Prefix + `Ctrl+B`                              | 离开 Prefix，并向 terminal-input port 提供一个 literal `Ctrl+B` event。                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Prefix + `q` 或 `d`                            | 离开 Prefix，并关闭此 Client shell。                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Prefix + `Esc`                                 | 取消 Prefix，且不改变 overlay。Help 打开时 `Esc` 关闭 Help；原生捕获使用各自的本地关闭路径。只有 pane 拥有输入时，`Esc` 才作为普通 terminal input 到达终端。                                                                                                                                                                                                                                                                                                                                            |
| Resize                                         | 查询已准入 stdout descriptor，更新 fixed backend viewport，并且只重新计算 Client-local viewport class。进入 narrow mode 会隐藏停靠 sidebar；回到 wide 会关闭 sidebar overlay，并恢复此前的 wide-sidebar preference。                                                                                                                                                                                                                                                                                    |
| Event/draw error、panic 或 termination signal  | 停止 loop，并尝试恢复 cursor、alternate-screen 与 raw-mode，不启动或停止其他 process。                                                                                                                                                                                                                                                                                                                                                                                                                  |

不变量是：每个 prefix sequence 至多产生一个 typed command；非 prefix byte 保持顺序且逐字节一致；一个完整的未知序列是单个 non-decision 事件而不是尾巴泄漏；未连接 adapter 绝不改变 shared Workbench 与 terminal state；每个可能取得的 terminal mode 都会
保留一个有界 restoration owner。Unit test 覆盖 state、input-port、fixed-backend、layout、renderer、
真实 PTY 的 same-terminal admission、实际 panic hook、全部四种 signal、cross-thread setup
interleaving、post-read viewport/resize failure 与 lifecycle fault injection。Native PTY process test
则分别证明缺失 cohort 的 bare 与 help/version path 不改变 terminal mode、Linux PTY master 在最后一个 slave
关闭后的 `EIO` 是正常 hangup，并且 linked public binary 不包含 `/dev/tty` 或 `tput` fallback token。
这些进程见证描述 Unix 适配器。Windows 适配器及其独立的 Console、ConPTY 证据见上文。

## TUI 共享连接

`WorkbenchClient::start` 将引导 OBLC 准入升级为一条共享 OBWP v1 carrier。所有功能 origin
引用同一个连接 actor。创建逻辑 lane 只分配资源作用域和取消 handle，不会新建 Server socket
或 OBLC 身份。actor 拥有重连并恢复订阅；一次性 mutation 在可能写入后绝不重放。
本地唤醒 stream 让命令／取消准入与传入 frame 中断同一次 readiness 等待，不通过轮询 channel
的 timer 交付。[Server 交付契约](../orchbee-server/README.zh-CN.md#client-多路复用交付)
负责 frame、上限、gap 恢复和有限模型的准确声明与排除范围。

已保存机器观察器在公开 origin 前升级 bootstrap 连接。发现保留一条状态订阅；Agent／Workspace
collector 在首次读取前注册失效提示，并在 reset 时退休旧 job。Metadata 读取跟随提交与 owner
到期，Host 进程标题和按需外部 Git 状态保留采样。每条 carrier 共用一个应用进展探测限制空闲
新鲜度；SSH keepalive 不足以证明进展。注册或读取失败时采用有界失败退避，不轮询成功状态。
失败远端观察器保留后台守卫，直到两个 collector 都撤掉缓存来源和订阅。

| 消费者                                                                             | 共享 carrier 行为                                                                                                                                   |
| ---------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| 窗口标题、配置、Settings                                                           | UI 拥有状态订阅；配置 reload 和偏好写入是异步一次性请求，没有转发线程。                                                                             |
| 通知                                                                               | UI 拥有有界事件订阅；取消只结束该 listener。                                                                                                        |
| Client control                                                                     | 稳定提交失效提示加一次性 Synchronize／completion、配置命令和布局请求。domain worker 协调快照／控制顺序，不拥有 socket、准入重试或 heartbeat timer。 |
| Workbench 成员、Worktree 操作、Agent registry／保留的物理 report、graphics catalog | 提交失效提示调度既有有界读取。空／无 attachment 视图不再周期读取成员。                                                                              |
| Worktree 发现、popup、Plugin、copy、link 和 Agent read job                         | 既有有界 domain job 使用同一 carrier 的逻辑作用域。取消只释放其自身终端资源。                                                                       |

Host 终端输出、标题／进程诊断、Plugin／Popup 进程退出、close disposal 和外部 Git／文件系统
状态没有事件源时仍需既有有界采样，原因在采样位置说明。启动偏好锁竞争和拥有明确生命周期的
OS 可执行文件查找是有限本地 job，不是 Server 订阅轮询。UI 渲染频率和语义恢复期限不代表
额外网络读取。

正确性证据包括取消与迟到回复、事件溢出与 resnapshot、待决 mutation 与另一条 stream，以及
不重放的配置恢复等真实 carrier 轨迹。性能必须在同一 Windows 主机、私有开发 runtime 上另测：
一个空闲 Workspace／PowerShell，预热十秒，再取三段各三十秒的 Client／Server／Worker CPU
时间、线程数、Server 连接处理线程数及线程上下文切换。全程按 PID 和进程创建时间配对每个
必需角色，以实际单调时钟采样间隔归一化 CPU，并保留无法配对的线程生命周期数量。采样前清除
继承的 Session 选择并核实私有 runtime。具名 Server 处理线程只是空闲准入的代理指标，不是
OBLC 握手计数。冻结可编译基线、候选、工作负载、工具链和采样脚本；测量窗口排除构建和其他
测量。上下文切换只是调度代理指标，不是硬件唤醒或 ReadyThread 事件。Windows 直接就绪测量
需要获授权的 ETW 采集，将 [ReadyThread payload 的目标线程身份](https://learn.microsoft.com/en-us/windows/win32/etw/readythread)
与进程／线程生命周期对应，并保留 trace 丢失计数。若无采集权限，记录具体限制和原生人工交接，
不能声称已采集该指标。这些命令和源码改动本身不能证明 CPU 改善或 macOS／Linux 结果。

## 原生终端投影

`run_connected` 接收已准入的 `LocalClient` 并启动 Native Workbench 客户端。shell 拥有终端模式与绘制；
客户端拥有 mailbox、连接 task 与已准入输入。`terminal_projection` 校验终端 frame 并产生精确反馈。
Terminal Host 保持 VT 与 history 权威。

[Workbench Client 生命周期回归](src/workbench_client/tests.rs) 覆盖 Drop 中断与 join、未知输入结果退役及只读输入拒绝。
[Native 传输回归](src/workbench_client/connection/native/tests.rs) 与
[mailbox 轨迹](src/workbench_client/mailbox/tests.rs) 覆盖有界准入、projection/feedback 顺序及退役。
[Linux 公开启动／PTY 见证](tests/local_startup_process.rs) 与
[原生指针见证](tests/local_startup_process/native_pointer.rs) 覆盖保留 shell、resize、指针路由及终端模式恢复。
其有限 deadline 是测试边界，不是性能保证。

`terminal_text` 拥有禁止显示的 Unicode 标量规则：Unicode 控制字符及 U+2028/U+2029。
终端单元格渲染将该规则与既有宽度检查一起使用，后端最终的 `Print` 还会独立地把禁止标量替换为
U+FFFD。安全文本通过借用使用，不产生分配；规范字符串保持不变。捕获输出测试验证单元格载荷
不能注入终端命令，同时保留后端自身的光标与样式命令。

## Client 已知未改变结果的背压处理

Carrier 保留一个排队请求和一个 wire owner；即使 wire 空闲，队列也可能短暂已满。
Workbench Client 为精确的 viewer/control ServerBusy 结果提供有界处理：
每次都是已完成、已关联且没有派发 Host action 的请求。等待 10 毫秒后，以新 ID 发起新请求。
Input 和 Resize 还使用新的递增 controller sequence，因为 Server 在队列准入前已消耗前一个序号。
Carrier Backpressure 在 OBCN dispatch 前拒绝；Attach 的 Worker 容量拒绝可能已完成 OBCN exchange，
但在创建 Host attachment 前拒绝。逻辑操作只保留一个绝对截止时间；首帧前受原启动截止时间约束，之后使用常规 50 秒预算。
每次底层 LocalClient 提交仍只写入一次。

Create、Open 和 Start 绝不重新提交。普通拒绝、controller 竞争、运行时不可用、错误响应、
transport 失败和未知结果均为最终结果。Feedback 在接受前保持同一个精确的待确认内容；
此策略不能确认未绘制帧。Client 取消关闭 stream、打断 exchange，在正常调度下至多等待一次
10 毫秒退避后 join。到期或身份耗尽会结束操作。

| 声明                                 | 细化映射和证据                                                                                                                                                                                                                                                    |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 只有已知未改变的容量结果允许再次尝试 | Server viewer/control 的 ServerBusy 分支证明 Host effect 未发生；Client 仅匹配这个精确结果。Fake Server 分区检查 busy、拒绝、不可用和未知结果。                                                                                                                   |
| 不重放任何可能已发生的副作用         | [LocalClient 测试](src/local_client/tests.rs) 检查单次 exchange 后的超时与 EOF 不会触发重提交；[native command 轨迹](src/workbench_client/connection/native/tests/commands.rs) 检查 busy input/resize/feedback，以及输入结果不明后的退役。                        |
| 截止时间与身份不重置                 | 同一操作截止时间覆盖全部尝试；请求 ID 与 controller sequence 递增；有界 fake Server 轨迹覆盖到期和 busy Resize。                                                                                                                                                  |
| 保留反馈与取消仍有界                 | [Mailbox 测试](src/workbench_client/mailbox/tests.rs) 检查八条记录的容量；[native 传输测试](src/workbench_client/connection/native/tests.rs) 检查精确的已绘制帧反馈；[Workbench Client 生命周期测试](src/workbench_client/tests.rs) 检查保留适配器释放时的 join。 |

## 本地化与权威

Authored help 与 diagnostic 同时提供 `en-US` 和 `zh-CN`。Locale selection 依次检查非空
`LC_ALL`、`LC_MESSAGES` 与 `LANG`；以 `zh` 开头的 language prefix 选择 `zh-CN`，其他 value
均 fallback 到 canonical `en-US`。Locale data 只影响 presentation，绝不会持久化或用作 authority。

`src/localization.rs` 负责语言选择和显式本地化边界；功能文案位于
`src/localization/<feature>.rs`。CLI 文案位于 `src/localization/cli/<feature>.rs`，
`src/localization/cli.rs` 只声明模块和共享类型。新增文案应归入本地化目录树内对应的功能模块；
领域模块继续返回类型化结果。

只有 help/version/usage 一次性路径仅依赖标准库参数、环境和流。Windows、Linux 与 macOS 的 bare invocation 组合共享 runtime、私有 Client transport 与 TUI；platform adapter 负责 console 与 transport effect。原生应用拥有当前开发中的 CLI/TUI 界面。

## POSIX 本机 Session rendezvous 底座

共享所有权与完整原语建模记录现由
[local-runtime](../../crates/local-runtime/README.zh-CN.md) 维护；应用不再保留重复实现。

## 已接受的本机 Session activation 建模记录

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                                 |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题   | 两个并发的本机 `orchbee` TUI invocation 必须通过同一次 Session activation attempt 协调，且二者都不能成为 Server 或 Worker 的 lifetime supervisor。已存在的 compatible Session 不应产生 launch；Session 不存在时至多启动一个 exact installed cohort；每个非成功 outcome 都必须显式。                                                                                                                                                        |
| 输入   | 两个 Client invocation slot；TUI、help 与 version intent；absent、compatible-ready、incompatible 或 partial 的 initial Session observation；一次 activation-lock result；exact installed Server-plus-Worker cohort capability；typed readiness、incompatibility、partial-cohort 与 launcher-failure observation；以及 abstract monotonic total-deadline expiry。External process outcome 不受信，且可以永远不抵达。                        |
| 数据   | 每个 Client 的 phase、typed result 与 connection path；一个可选 activation-lock owner；zero-or-one launch count 与 exact-installed cohort tag；Session 与 typed-readiness state；以及 Session-owned ready-service lifetime marker。不包含 PID、path、runtime-base、scope、ACL、process tree、cleanup record、recovery identity、service-manager unit 或 numeric time value。                                                               |
| 输出   | Compatible present Session 直接连接。Session 不存在时，一个 lock owner 启动 exact installed cohort，contender 等待 typed readiness。成功时连接并释放 caller 的 activation resource。Incompatible Session、partial cohort、launcher failure 与 deadline expiry 是不同的 failure。Help 与 version 完成时不 discovery 或 activate。Client exit 只结束该 Client。                                                                              |
| 不变量 | 每条 TUI path 都先尝试 connection 再 activation；activation owner 唯一；launch count 至多为一，且每次 launch 都是在 lock 下启动 exact installed cohort；contender 成为 readiness waiter；只有 ready compatible Session 能产生 connection；settled Client 不拥有 activation resource；ready-service lifetime 属于 Session；failure 与 Client exit 都不停止或改写 Session state；help/version 保留全部 activation fact。                     |
| 决策   | Invocation kind 决定是否允许执行 Session work。Connection observation 区分 direct success、absent activation、incompatible failure 与 partial-cohort failure。发现 absent 后，lock ownership 区分 launcher 与 waiter；lock winner 在 launch 前重新检查 Session state。Typed readiness、typed failure 或 Client-owned total deadline 结算每个 waiter。在 connection 完成之前，deadline observation 可以胜出。                               |
| 模型   | 对恰好两个 Client slot 与四种受支持 initial Session class 建模的 bounded workflow。一个 scalar `activationOwner` 对 `activation.lock` 建模；`launchCount` 与 `launchedCohort` 暴露 singleton exact-cohort launch；`serviceLifetimeOwner` 把 ready Session 与每个 Client 区分。External launcher/readiness action 是 nondeterministic，且没有 fairness assumption。                                                                         |
| 转换   | Present compatible Session 下为 `idle -> connecting -> connected`；present incompatible 或 partial state 下为 `connecting -> failed`；launch winner 为 `connecting -> lock-ready -> lock-held -> waiting-readiness`；contention 为 `lock-ready -> waiting-readiness`；waiting 或 recheck state 会结算为 connected 或一种 typed failure；`connected -> exited` 保留 Session state。Help/version 从 `idle` 直接进入 information completion。 |
| 依赖   | Model 仅在具名 abstraction 上组合已接受 command contract、当前 private `LocalClient` admission boundary 与 `ModeCheckedLocalSessionRendezvous` lock semantics。未来 runtime refinement 必须提供 installation-owned cohort selection、typed readiness 与一个 monotonic total deadline。本 slice 不增加 process、socket、filesystem、localization 或 public-command implementation。                                                         |
| 所有权 | `apps/orchbee` 拥有 Client bootstrap workflow、deadline、activation guard 与 typed Client result。Installation composition 拥有 exact cohort capability。Server 与 Worker 在成功后拥有各自 readiness 与 service lifetime。Model 不向 Client 授予 supervise、stop、replace、downgrade、clean up 或 recover Session 的 authority。                                                                                                           |
| 验证   | 启动状态、期限与真实进程测试覆盖受支持初始 Session 类、竞争 Client、准确 cohort 启动、既有服务保留及失败结算。共享 LocalSessionStartup 核心仅提供其单独声明的有界协议证据。                                                                                                                                                                                                                                                                |
| 正确性 | LA-01 至 LA-05 仍是启动与准入行为要求。受控期限及进程轨迹必须展示无需假设外部进程响应的结算。已移除激活流程不提供当前 TLC 证明。                                                                                                                                                                                                                                                                                                           |
| 限制   | 该 abstraction 只证明 checked two-Client、one-Session state space。它不证明 runtime refinement、operating-system scheduling、process creation 或 detachment、daemonization、service-manager behavior、runtime base 或 scope、ACL、socket metadata、cleanup、recovery、retry、update、concrete deadline 或 progress threshold、localization、TUI behavior、packaging、public wiring 或 external process progress。                          |

| Transition owner     | Event 与 guard                                                      | Result                                                                                                            |
| -------------------- | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Invocation           | Help 或 version                                                     | Information completion；Session、lock、launch、readiness 与 service-lifetime fact 不变                            |
| Client connection    | Compatible Session 已 ready                                         | Direct Client connection；无 activation resource 或 launch                                                        |
| Client connection    | Session incompatible 或 partial                                     | 对应 typed failure；无 activation 或 Session mutation                                                             |
| Activation contender | Initial connection 找不到 ready Session 且 `activation.lock` 已持有 | 在 caller 的 total deadline 下等待同一个 typed readiness result                                                   |
| Activation winner    | Lock 已获得、recheck 仍发现 absent 且不存在 launch                  | 启动恰好一个 installed Server-plus-Worker cohort，并在持有 guard 时等待                                           |
| Activation winner    | Recheck 发现 ready、in-progress、incompatible、partial 或 failed    | 根据 exact observation 连接、释放并等待或失败；绝不启动第二个 cohort                                              |
| External cohort      | Launch pending                                                      | 可以发布 ready、incompatible、partial、launcher failure，也可以完全不返回；external completion 没有 fairness      |
| Pending Client       | 观察到 typed result 或 total deadline                               | 连接或返回 `incompatible`、`partial-cohort`、`launcher-failure` 或 `timeout`；释放 caller 拥有的 activation guard |
| Connected Client     | Client exits                                                        | 只结束 Client Connection；保留 Session、cohort、readiness、launch 与 service-lifetime state                       |

该模型抽象了运行时细化；`LocalSessionStartup` 提供具体的启动安全论证。在声称该模型映射到实现
之前，未来接线必须补充确定性进程测试。

| Claim                                                                                            | Bound                                                                                                                           |
| ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------- |
| LA-01：每个 activating TUI 都先连接；present compatible、help 与 version path 从不 launch。      | 对 command intent 与 abstract observation 建模，不覆盖 parser、transport 或 process refinement。                                |
| LA-02：并发 absent 在一个 lock 下至多产生一次 exact installed-cohort launch。                    | 恰好两个 Client slot 与一个 activation round；不声称 runtime-base、lock-file 或 process implementation。                        |
| LA-03：incompatibility、partial cohort、launcher failure 与 timeout 均显式，且不会停止 Session。 | Failure presentation、cleanup、recovery、retry 与 stale-artifact policy 仍不在范围内。                                          |
| LA-04：connection 释放 Client activation ownership；Client exit 不 supervise 或 stop Session。   | 只对 ready-service ownership 建模；不选择 process 如何 detach 或 service lifetime 如何实现。                                    |
| LA-05：每个 started TUI 都会结算，且不假设 external process 响应。                               | 假设未来 adapter 确实保证观察到一个 finite monotonic total deadline；不选择 numeric value、OS scheduling 或 external liveness。 |

## 测量

实际执行 TUI 时，`ORCHBEE_TUI_MEASUREMENTS` 可选择现有自有目录中的一个新的绝对诊断文件路径。
帮助、版本、查询及其他非 TUI 命令既不读取该变量，也不创建诊断文件。显式启用的采集器只在内存中
记录有界计时和计数样本，终端恢复后才写出；不记录输入字节或 pane 文本。
[计时契约](docs/native-terminal-environment.zh-CN.md#tui-计时观测)定义 schema、失败与截断规则、
数据包关联及内部完成时间的证据边界。把延迟归因于被测运行时前，先比较启用与禁用采集的运行。

从仓库根目录收集 reference release baseline：

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

该脚本构建确切 locked release package，warm 后重复调用 `orchbee --version`，报告 artifact byte
与 warm latency percentile，并在 `/usr/bin/time` 可以提供时报告 peak resident memory。结果只描述
输出中注明的 host、toolchain、profile 与 invocation。它不是 acceptance budget、cold-cache
guarantee，也不是未来 TUI performance 的证据。

2026-09-07 的当前 Linux reference 使用源码提交 `ac620e35`、x86_64 WSL2、
Linux 6.18.33.2、Rust 1.97.0、默认 Cargo release profile 和同 cohort Zig ReleaseSafe
Host。Python 3.14.4 驱动测量：version 先预热 5 次再测量 50 次；PTY 为 120×32 cell、
960×640 pixel，测量 5 个全新 Session 和每个 Session 3 次重连。隔离调用环境仅包含账号的
HOME/USER/LOGNAME、Cargo 和标准可执行文件路径、仓库根目录 PWD、
`LANG=LC_ALL=en_US.UTF-8`、`SHELL=/usr/bin/zsh`、`TERM=xterm-256color` 和唯一的
`ORCHBEE_RUNTIME_BASE`。文件系统缓存已预热；新 Session 不代表冷 OS 缓存。

记录的界面计时从 Client spawn 到一次完整绘制，其中包含 `[*1 1]` 和当时底行的
`Tab 1 · Ready · controlling` footer。该节点位于完整帧绘制、精确 FrameApplied /
FeedbackAccepted exchange 及最新尺寸的 UI Ready 发布之后；不证明 Worker 已消费 Ready reply
或清除启动上限。Client VmHWM 在该节点采样：它衡量截至 Ready 的驻留内存峰值，
不代表整个 TUI 生命周期的峰值。
当前观察器在采样前显式打开并关闭前缀状态浮层；新计时包含这一输入往返，
不能与这些参考数字直接比较。每次关闭均验证 terminal mode 精确恢复并退出
alternate screen。每次热重连之前，在计时区间之外按条件等待，确认先前 Server connection handler
和 socket 已退出。排队中的 viewer Detach 没有公开完成见证；残余 detach 工作可能计入下次
attachment 的测量延迟。此前单终端 reference 使用了不同的就绪见证，因此这些观察不构成严格的前后对照。

| 测量                                                | 参考观察                         |
| --------------------------------------------------- | -------------------------------- |
| Client 可执行文件大小                               | 2,989,768 bytes                  |
| Version 启动 min / p50 / p95 / max                  | 0.791 / 0.834 / 1.176 / 1.406 ms |
| Version 峰值驻留内存                                | 2,818,048 bytes                  |
| 新 Session 完整 Ready 帧 p50 / p95（5 次）          | 485.526 / 501.903 ms             |
| 热 Session 完整 Ready 帧 p50 / p95（15 次）         | 439.502 / 486.007 ms             |
| Client 截至 Ready 的峰值驻留内存 p50 / max（20 次） | 4,792,320 / 4,894,720 bytes      |

这些是该主机的实际观察，不是性能承诺，也不适合直接和其他 OS、driver 或旧 one-shot 产物比较。
后续改变可达依赖或启动路径时重新测量并替换 reference；支持平台的发布验收预算仍待确定。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/terminal-host build
cargo build -p orchbee-server -p orchbee-worker --locked
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
node scripts/tla/check-inventory.mjs
node scripts/tla/run-models.mjs --workspace apps/orchbee
node scripts/tla/run-models.mjs --models local-session-startup
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```

Windows CLI peer fixture 复用只读 cohort 准备结果与已编译的 schema validator。
每个 fixture 独立拥有运行时端点和配置；修改 cohort 文件的测试使用
`Fixture::private_cohort()`。共享产物只在测试线程或 fixture 使用期间保留，最后一个所有者
释放时删除临时目录。可执行文件仍经过正常的单硬链接 cohort 准入。Schema 成员与语法组合
由核心测试覆盖；进程测试保留离线发布、真实 IPC、失败、期限和清理验证。

每条被捕获的命令都持有一个 Windows Job，直到根进程及其后代完成回收。默认交换在进程创建后
设置 45 秒兜底期限，stdout/stderr 合计上限为 8 MiB，并使用非阻塞输入输出；stdin 提前关闭仍
保留子进程的真实退出状态与输出。只有确认 Job 已回收后，才报告原始期限、取消、捕获或输出
超限失败及有界的部分输出。无法确认回收时改为报告 `PhysicalUnknown`，不能宣称清理成功。诊断最多保留每个输出流
开头的 8 KiB；清理失败会报告所属路径，并避免在栈展开期间再次 panic。

Peer guard 借用 fixture，并负责取消及 join，包括父测试断言失败的路径。它的准入期限仅在进程
成功创建后开始；已接受连接的读写上限为 10 秒，peer 脚本自身的等待也必须有界。Guard 析构时先
等待当前有界 peer 操作结束，再释放 fixture/cohort。失败的 peer 会取消命令；正常完成的脚本
允许 CLI 继续发布结果。[Fixture 生命周期验证](tests/cli_windows_fixture_lifecycle.rs) 覆盖
全双工管道、上限、stdin 提前关闭、准入取消、peer join 和受管后代退出。

使用 `ORCHBEE_TEST_NATIVE_COHORT` 指向已暂存的绝对路径，可对实际 Release 产物运行同一套
真实进程验收。Fixture 先校验该 cohort，再复制到各自隔离目录；测试不改变源产物或默认 Session。

```sh
ORCHBEE_TEST_NATIVE_COHORT=/tmp/orchbee-native-dev cargo test -p orchbee --test local_startup_process --locked
```

### Agent integration 一致性 fixture

`tests/fixtures/agent-integrations/` 保存 `agent_integrations` 单元 harness 重放的两套语料：

- `provider-conformance/` —— `payloads.json` 加 `cases/<provider>.cases.json`：
  按 provider 组织的 inspect、enable、disable 案例，钉定脚本化命令回复、
  期望 argv、文件系统效果以及渲染后的 CLI verdict。
- `provider-effects/` —— `cases.json`：文件系统效果机案例——胶囊发布、
  move-aside、备份摘要、remove 遍历、cohort 与版本门、预算与取消边界。

两套语料都由 harness 经 `CARGO_MANIFEST_DIR` 解析，任何检出均可直接重放，
不依赖环境覆盖或本机路径。案例修改即语义契约变更，须与被钉定的引擎行为
在同一受审变更中提交；新增 provider 在 `provider-conformance/cases/` 下
增加一个文件。

## Workspace 终端启动

已连接 TUI 在选择位置前读取关联的 OBWP 快照。选择使用准确 Workspace／Context 对、可用的记忆位置，或显示顺序中的首个规范 Workspace，包括空 Workspace。Windows 在目录启动条件满足时可显式启动首个 Workspace；其他空 Workbench 保持为空。不存在 `TerminalIntent::Standalone`、`terminal.open` 请求或无 Workspace ID 的选择。

无身份的 `NoWorkspace` 仅是 Client 的空落点：不拥有上下文、不创建终端、不出现导航行，也不保存为 Workspace 位置。Workspace 的 Project 成员关系仍可为 null。空 Workspace 保留身份并支持显式配置／创建。

Client 只通过 `attach-binding` 附着快照中的准确 会话／Worker／Terminal 绑定。已知待完成、失败、不可用或结果未知的创建来源不能被替换为附着许可。只读、控制者、Resize 与首帧就绪沿用既有授权和期限。Client 退出保留共享成员与运行终端；关闭最后一个 Tab 留下空 Workspace，之后启动不会重新创建它。[上下文选择](docs/native-context-selection.zh-CN.md)及[终端创建](docs/native-terminal-creation.zh-CN.md)拥有详细规则。

## Windows 显式 Session 停止

`server stop --confirm` 结束现有托管 Windows Session 及其终端进程。使用 `--detach` 仅等待准入，保留返回的标识供 `server stop-status <id>` 查询。显式重试必须使用 `--retry --request-id <id> --confirm`。取消或超时不撤销已接受的停止。准确匹配的已停止记录与空闲生命周期锁证明完成；下次 `server start` 消费记录并创建新 Session。详见 [CLI 停止契约](docs/native-cli-query.zh-CN.md#windows-session-停止)。尚未实现 Linux/macOS 停止运行时。

## Windows Worktree 操作

`worktree create/open/remove/status/resume` 使用 Server 保留操作与 Windows Worker Git 执行。[Worktree 契约](docs/windows-worktrees.zh-CN.md) 拥有来源选择、默认值、确认、部分结果与恢复规则。这些命令不激活 Session。Create／Open 接受可选 `--focus` 与精确 `--client`；默认 `--no-focus` 保持视图。省略来源时 List／Create／Open 跟随经校验的调用者或所选 TUI 上下文。

## Windows Workspace 组关闭

`project close-all <target> --confirm` 关闭该 Project 中捕获的 Workspace，保留 Project 记录和 Git checkout。普通 `workspace close` 始终只关闭指定 Workspace。成员消失后使用 `project close-status --request <operation-id>`；只有明确可恢复失败才使用 `project close-resume --request <operation-id> --generation <attempt> --confirm`。Project 关闭支持分离/有界等待，报告每个子 Close 的身份及已观测状态。参见[Project 全部关闭契约](docs/windows-worktrees.zh-CN.md#project-全部关闭)。

Windows CLI 的 Panel／Tab／Workspace Close 会捕获并结束全部已绑定的主、子 Presentation，也支持分组／固定 Tab。只有全部目标的处置与清理完成，并重新验证完整成员关系后，才移除布局；未知效果继续保留原回执与恢复约束。未绑定 Presentation、子终端导航和其他结构操作仍按各自契约处理，见 [Close 模型](../orchbee-server/docs/terminal-tab-close.zh-CN.md#完整-presentation-成员关系)。

## Windows Client 导航

[Windows Client 视图控制](docs/windows-client-control.zh-CN.md) 拥有 `client list/get/control-status`、`workspace focus`、`tab focus`、`panel focus` 和 `panel zoom`。导航通过 `--client` 或无歧义且已校验的调用者上下文定位一个 current TUI。只提交一次，并等待绘制确认；超时或 Ctrl+C 仅停止等待。单 Panel 缩放有明确无操作回执。目录 v68 包含 76 个 schema，新增均为 v1 的 `orchbee.client.view`、`orchbee.client.control` 和 `orchbee.cli.failure.client-control`。未增加操作系统窗口激活或 Mac/Linux 控制运行时。

Windows 进程测试在 Git for Windows Bash 中执行输出的补全，通过 `git --exec-path` 定位；该见证要求已安装此 Bash。检查命令动词前后的互斥选项，不连接 Session。PowerShell 另有真实 Shell 语法见证。

## Windows schema 导出

`api schema [<schema-id>] --output <path>` 离线导出内嵌 JSON 文档，同时将其输出到 stdout。目标父目录必须存在；完整的同目录文件替换已有普通目标文件。[导出契约](docs/native-cli-query.zh-CN.md#windows-离线-schema-导出) 拥有失败阶段、临时文件清理以及发布后的 stdout 失败。目录 v68 包含 76 份文档，包括 `orchbee.cli.failure.schema-export` v1。

## Windows Panel 布局查询

`panel layout <panel>` 未带 `--client` 时无需 TUI 即可读取规范结构；带 `--client` 时读取该 Client 的单元格几何。`panel neighbor --direction left|right|up|down` 和 `panel edges` 也依赖 Client 几何。Panel 命令接受位置参数 Panel 或 `--current`，拒绝旧 `--panel` 别名。读取保持焦点和终端尺寸，支持非活动或未访问目标。结构与渲染回执见[几何契约](docs/windows-client-layout.zh-CN.md)。

`panel focus [source] --direction left|right|up|down` 在指定 TUI 的未缩放单元格布局中查找一个邻居，保留缩放；无邻居返回 `direction-unchanged`，不切换到来源 Tab。找到邻居后固定目标，仅在绘制成功后返回 `direction-focused`。回执查询不重复移动。

`panel resize [panel] --direction left|right|up|down [--amount <number>]` 通过 Windows TUI 调整一个符合条件的分割边界，保留焦点和缩放。`layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` 设置精确二进制分割路径。显式目标未带 `--client` 时直接提交共享比例；经 Client 路由的调整保留 `layout-adjusted` 回执，目标被选中时等待绘制。有限比例限制为 0.1–0.9；有限 resize 数量转为绝对值且最多 0.5，默认 0.05。Server 拥有共享比例，Client 按各自视口尺寸投影并保留本地焦点、缩放与控制权。参见[布局契约](docs/windows-client-layout.zh-CN.md)。

## 共享窗格标题

`panel rename <panel-id> --label <text>` 设置共享主 Presentation 的自定义标题；`--clear`（或空白文本）恢复创建时的标签。接受精确 ID 或 `@panel:<suffix>`，不改变焦点或终端。[窗格标题契约](docs/native-panel-labels.zh-CN.md) 负责规范化、修订与重放规则以及匹配 cohort 的兼容边界。`orchbee.panel.rename` v1 区分 `renamed` 与 `unchanged`，可空 `customLabel` 回显规范化后的请求元数据。`panel get` 与 `panel current` v3 提供不变的 `label`、可空 `customLabel` 及实际显示的 `displayLabel`。`api snapshot` v4 保留规范的可选自定义标题。目录 v68 包含 76 份 schema，原生命令清单包含 88 条路径。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。显式 Panel 未带 `--client` 时走规范结构流程，不改变焦点。经 Client 路由的交换单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](docs/native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

Windows `tab create` 和 `panel split` 在创建前解析 `--cwd follow|home|current|workspace|<absolute-path>`；默认 follow 采样已解析来源的根进程目录，回执记录实际目录来源／路径。它们接受可选目标及标签，默认不聚焦；`--focus` 请求创建后聚焦，分屏可带共享 `--ratio`。显式目标无需 TUI 即可创建。`orchbee.creation.result` 分别报告已提交创建与后续比例／焦点结果；后续失败不能当作未创建而重试。参见[原生终端创建](docs/native-terminal-creation.zh-CN.md)，其目标、总预算、并发保护和恢复规则为这些 Windows 命令的权威。

Windows `panel input (<panel>|--current) --right-click orchbee|panel` 设置共享右键策略，不移动焦点。`--current` 要求有效调用者元数据，显式目标绕过它。`panel split --right-click` 随创建提交新 Panel 策略。默认 `orchbee` 将右键保留给 Client；`panel` 允许无修饰键的右键在启用鼠标上报的终端内容内透传。已准入的手势在策略改变后仍保留匹配的松开事件。路由、排队输入复核及失败恢复见[Panel 输入契约](docs/native-panel-input.zh-CN.md)。Client 的[动作菜单](docs/native-action-menus.zh-CN.md)在面板内容区沿用此策略。`orchbee.panel.input` v1 返回 `configured` 或 `unchanged`；`panel list` v2 与 `panel get/current` v3 提供 `rightClick`，`tab get` v2 和 `api snapshot` v4 接受可选规范字段。目录 v68 含 76 个 schema，原生命令清单含 88 条路径。

Windows `tab move <target> --insert-index N` 按原顺序的零基插入边界移动完整 Tab；0 为首位，
Tab 总数为末尾，源边界及其后一边界无变化，越界拒绝。它与 `--order` 互斥，不改变焦点或终端。
成功使用 `orchbee.tab.move` v1，运行时失败使用 `orchbee.cli.failure.tab-move` v1。

## 自动 Tab 命名

[Tab 标签契约](docs/native-tab-labels.zh-CN.md) 拥有可选 `automaticLabel` 元数据。省略标签的新建 Tab 按当前上下文位置编号；显式数字名称及旧 Tab 保留固定名称。移动或关闭后 CLI 查询、标签选择器和 TUI 显示保持一致，重命名切换为固定名称。原生 `api snapshot` v4 暴露规范元数据；目录 v68 包含 76 份 schema。共享协议需要匹配 cohort。

## 按来源读取终端输出

Linux 与 Windows 的 `terminal read` 和 `agent read` 接受
`--source visible|recent|recent-unwrapped|detection`，默认读取 recent 输出。
recent 合并保留历史与实时屏幕；detection 读取最近的一个屏幕高度范围。
`--lines` 允许零，钳制到 1000，recent 默认 80。拒绝已移除的
`--history`、`--rows` 和 `--unwrap` 参数。`--plain` 直接导出 UTF-8，与 JSON 互斥。
[输出契约](docs/native-terminal-output.zh-CN.md) 拥有选择、精确样本验证及
`orchbee.terminal.output` v3，包括可为 null 的输出游标。

`terminal wait-output` 使用相同的来源与行数限制，默认总超时为 30000 ms。
`--forever` 显式取消总时限，与 `--timeout` 互斥，保留每次 I/O 的五秒上限及 Ctrl+C 取消。
recent 等待合并软换行；visible/detection 按物理行匹配。普通等待输出 v3，可匹配已有输出。
`--after <cursor|now>` 输出 v4，仅匹配已验证边界之后的行。匹配成功不证明命令完成；
受管理 Shell 的命令完成由 `terminal run --wait` 观察。

`terminal read` 与 `agent read --ansi` 导出带样式快照。JSON 使用
`orchbee.terminal.output` v4，并带 `result.encoding: "ansi"`；纯文本读取使用 v3。
ANSI 与 `--plain` 互斥，保留带样式空格与 Unicode，只生成 SGR，
并在 `recent-unwrapped` 下拼接已验证的软换行。detection 仍为纯文本。
不重现原始 PTY 字节、超链接或图形。渲染请求与预备输出各自保留 8 MiB 上限。
参见[输出契约](docs/native-terminal-output.zh-CN.md)。

Windows `status server` 在 Client Connection 准入前读取实际 Server 构建／协议／cohort 元数据；`status client` 离线检查当前程序。两者支持 `--format text|json`，不激活 Session。裸 `status` 仍查询 Workbench。[运行时状态契约](docs/native-runtime-status.zh-CN.md) 拥有五秒时限、未知元数据、兼容性、schema 和失败语义。发布偏好与安装由显式 Windows 更新命令拥有。

## Agent 导航

Windows `agent focus <target> [--client <id>]` 在一次客户端控制提交前，将名称、会话或窗格解析为唯一活动的主窗格。显式 `--agent-session`、`--name` 和 `--panel` 使用现有解析器；`--request-id`、`--detach` 和默认 30000 ms 总期限沿用导航恢复契约。Starting 或 Unavailable 客户端在 Agent 查询前返回 `client-not-ready`；Busy 允许 Client 执行可取消模式的前置检查。[Agent 导航契约](docs/native-agent-navigation.zh-CN.md) 定义主呈现绑定、歧义、取消和保留操作查询。显式聚焦成功后确认所选 Tab 中当前已知的 Agent 版本，即使外层终端失焦。普通绘制仍要求宿主聚焦。`agent attention <target> [--client <id>]` 独立读取该 Client，不标记已读。

Windows 宿主焦点上报由[原生 Client 提示状态](docs/native-client-attention.zh-CN.md)管理；焦点报告属于 Client 元数据，不是终端文本。提示状态与 CLI 确认的集成仍在开发。

Windows `agent changes [--cursor <c>] [--limit <n>]` 返回有界 canonical 变更页或当前头部检查点。默认 limit 为 50（1–100），后续复用返回的 cursor。`reset-required` 与 `cursor-invalid` 要求显式重建基线。参见[查询契约](docs/native-cli-query.zh-CN.md#windows-agent-变更页)。Catalog v68 提供 `orchbee.agent.changes` v1，共 76 个 schema、88 条命令路径。

Windows 支持通过 `agent integration enable --provider cursor`、`agent integration enable --provider qodercli` 或 `agent integration enable --provider antigravity` 管理 Cursor、Qoder 和 Antigravity 会话身份 Hook，保留既有 Hook 和其他配置值。

Windows `agent integration enable --provider mastracode` 安装会话、运行、工具、权限及子 Agent 观察，可修复不完整的自有 Hook 集合。

Windows `agent integration enable --provider droid` 管理生效用户配置中的会话身份 Hook，不遮蔽已有回退 Hook。

Windows `agent integration enable --provider kimi` 管理当前 Kimi 的 TOML 会话 hook；工作中／阻塞／空闲对齐仍未完成。

Windows `agent integration enable --provider letta` 安装静默 SessionStart hook，并按 Agent 区分默认对话。

Windows `agent integration enable --provider devin` 管理 APPDATA/devin/config.json 中的六组会话身份 hook。

Windows `agent integration enable --provider hermes` 发布两个自有 Python 插件入口，并通过提供方 CLI 启用；停用保留插件及其缓存。

Windows `workspace create` 可省略 Project、名称和目录；按来源终端／Workspace／主目录解析默认值，默认创建未分组 Workspace，可用 `--focus` 聚焦现有 TUI。所有 Windows 形式均启动首个终端；显式参数和不聚焦使用相同流程与回执。Workspace 与聚焦保留各自结果；可选 Project 与 Workspace 创建原子提交，不再存在单独的 Project 准备步骤。详见[Workspace 创建契约](docs/windows-workspace-creation.zh-CN.md)；当前命令与 schema 目录由 `orchbee api schema` 提供。

## Windows CLI 配置

共享应用与终端主题通过和 Desktop 相同的偏好权威应用于原生 TUI。参见[原生主题](docs/native-themes.zh-CN.md)。

`config check` 还校验 `APPDATA/Orchbee/config.toml`，可由绝对路径 `ORCHBEE_CONFIG_FILE` 覆盖。`[terminal] new_cwd` 设置创建 cwd 默认值；`[worktrees] directory` 选择检出根目录。显式命令参数覆盖有效配置。参见[配置与优先级](docs/windows-cli-configuration.zh-CN.md)。Windows CLI 每次调用读取；公共 TUI/CLI 激活在提交前读取启动策略。TUI 直接创建和 Server 热重载仍是独立能力。

## Windows 终端观察流

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` 从一个已有绑定输出 NDJSON，不获取控制权，也不改变 Host 尺寸。[流契约](docs/windows-terminal-streams.zh-CN.md) 拥有完整安全 ANSI 帧、重置、先刷新后确认、期限和清理规则。默认视口为 120×40，默认不限时。两种输出格式均产生 `orchbee.terminal.stream` v2 记录。目录 v68 含 76 个 schema、90 条原生命令路径。机器控制现通过 `terminal session control` 接收 NDJSON 输入；交互式 `terminal attach <terminal>` 或 `agent attach --name <name>` 要求控制台 stdin/stdout，并采用实际控制台尺寸。二者均支持 `--takeover`、`--mouse on|off` 和可选总时限 `--timeout`。`Ctrl+B` 后按 `q` 脱离；连续两次前缀发送一个字面前缀。粘贴不会触发脱离。任一全局格式下均显示交互画面。不启动 Session、不改变 Workbench 焦点、不重连或重放输入。

## Workspace 元数据集成

[Workspace 元数据契约](docs/windows-workspace-metadata.zh-CN.md) 定义上报规范化、来源序号栅栏、TTL 及一致读取组合。其 Server 和共享通信所有者独立于权威拓扑及终端 OSC 元数据。`workspace report-metadata` 和 Workspace list/get 通过有界关联读取及未知结果回执消费该所有者；通用事件与可配置 TUI token 模板需要各自集成。

Windows `panel report-metadata` 支持来源拥有的展示字段、守卫、token 补丁、序号和 TTL，提供绑定校验及单次提交回执。见 [Panel 元数据契约](docs/windows-panel-metadata.zh-CN.md)。

Windows `panel report-agent`、`clear-agent-authority` 和 `release-agent` 提交显式来源上报并返回相关处理结果。参见 [Agent 上报契约](docs/windows-panel-agent-reports.zh-CN.md)。

## Windows CLI 通知

`orchbee notification show "Build complete" --body "All checks passed" --position bottom-right --sound done` 向运行中 Session 上可接收的主 TUI Client 排入自定义通知。配置 `[ui.toast] delivery="orchbee"` 显示卡片，`terminal` 或 `system` 使用外部投递；默认 `off`。使用 `--format json` 获取封闭投递回执。详见[投递、边界与生命周期](docs/windows-notifications.zh-CN.md)；接收只表示入队，不确认显示或播放声音。

## Windows 外层终端标题

`orchbee terminal title set "Build workspace"` 向前台主 TUI 发布覆盖标题；`orchbee terminal title clear` 恢复在线 `ui.window_title` 模板（回退为 `Orchbee`）。两者接受 `--format json`，没有 Client 连接时仍保留发布状态。模板规则及实际宿主限制见[标题归属与恢复](docs/windows-terminal-title.zh-CN.md)。

Windows 的 `plugin link/unlink/enable/disable/list/config-dir` 和 `plugin action list` 离线管理用户共享目录。`plugin action invoke` 向已有 Session 提交具有归属的命令，`plugin log list` 查询保留结果。上下文、进程归属、有界输出、托管安装/卸载及剩余 hook/Pane 集成见 [Windows 插件](docs/windows-plugins.zh-CN.md)。启动回执确认受理，执行结果由日志提供。

## Windows Panel 图形命令

`panel graphics set|clear|info|stream` 支持封闭图片输入、文件/管道读取与有界原始帧流，通过现有 Workbench 归属提交一次。参见 [Windows Panel 图形 CLI](docs/windows-panel-graphics.zh-CN.md) 了解参数、未知回执与流清理。[原生 TUI 渲染器](docs/native-panel-graphics.zh-CN.md) 暂存完整图像并发布成功绘制事实；仍需最后的 Windows 组合验收。

## 原生设置

在 Windows 或 Linux TUI 中按 Ctrl+B F4 打开通用与启动配置；Windows 还通过导航提供设置入口。
所有通用偏好与 Desktop 使用同一设备记录，包括语言、应用与终端主题、选择、Agent 尾部展示、
通知、声音、滚动历史与启动配置。编辑在保存前保持草稿，重置需要确认，
来源过期或结果不明会明确显示且不重放。
参见[原生设置契约](docs/native-settings.zh-CN.md)与[共享清单](../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)。

Windows 首次运行引导在终端可继续输入时提供 Workspace 创建、Agent 选择、操作与帮助入口，关闭状态按原生配置档案保存。参见[原生首次运行引导](docs/native-onboarding.zh-CN.md)。

<!-- documentation-pairing: english-sha256=2b9c285af9b0c9ac9488cfb0b4e01f9e04facc5cf44946b8535b5ba108db7a80 zh-cn-sha256=956c30e1c86eeb4aff00bd2719a123106646b7c7ccec044cf790551ae195def5 -->
