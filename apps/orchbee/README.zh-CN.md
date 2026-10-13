# Orchbee 原生入口

Windows `events subscribe` 从已有 Site 输出有界 NDJSON 观察。`events wait` 在等待片段间保留
同一连接及过滤器，区分匹配、确定超时、历史丢失、拒绝与中断。使用 `--event tab.renamed`、
资源选择器或封闭 `--filter` JSON；`--after-cursor` 固定保留日志历史。输出／滚动探测保留真实
终端和 Client viewport 所有权。运行 `orchbee events --help` 查看上限与条件；
[事件契约](../orchbee-server/docs/windows-events.zh-CN.md) 管理行为与最终 Windows 证据。

`orchbee terminal run <terminal-id> "Write-Output ready" --format json` 在一个输入请求中粘贴命令，再发送 Enter。命令须为一个引号字符串；保持原始文本，目标只接受完整 ID 或 `@terminal:<suffix>`。回执确认输入提交，不保证 Shell 已就绪、命令完成或退出状态；部分或未知结果不得重放。 [命令提交契约](docs/native-terminal-run.zh-CN.md).

实验性的原生 Rust CLI 与 TUI Client。在 Windows、Linux 以及 Apple Silicon macOS 开发包中，
裸命令从已校验的可执行文件 cohort
连接或启动本地 Site，取得第一个已校验的终端 projection frame 后进入 TUI。原生 Client 操作共享
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
名称唯一，在会话结束或 Site 停止后消失。

`agent read <name>` 读取唯一绑定的终端；`agent send-keys <name> enter`
通过既有终端输入回执发送语义按键。两者也接受 `--session <id>` 和 `--name <alias>`，
绑定缺失或有歧义时拒绝操作。

`agent prompt <name> <text> --wait` 在新鲜且无阻塞的预检后提交字面 paste 加 Enter。
先观察活动再等待条件，后续超时或取消仍保留输入回执；不承诺轮次因果。

`agent start alpha --kind codex --panel <id>` 在空闲 Windows 终端中启动 Agent，
仅在进程和 canonical 就绪检查通过后绑定名称。配置、限制和恢复见
[Windows Agent 启动](docs/windows-agent-start.zh-CN.md)。

Windows `channel show [--format text|json]` 从共享用户配置读取严格的 stable/preview 偏好，不激活 Site。参见[配置及频道来源](docs/windows-cli-configuration.zh-CN.md#规则更新策略)。

Windows `machine list|add|rename|remove|enable|disable` 管理已保存的 SSH 配置，并在添加前检查远端 Windows 就绪状态。持久化、部分结果、命令路由和 TUI 实时切换见[已保存的 Windows 机器](docs/machines.zh-CN.md)。

Windows `update` 与 `channel set` 共用完整 cohort 安装、显式发布源、独立版本发布和单独的用户 PATH 激活。详见 [Windows 版本安装](docs/windows-cli-configuration.zh-CN.md#windows-版本安装)。

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
orchbee status                   # 运行中 Site 的 Workbench 快照
orchbee <family> <command>       # 查询与 Workbench 修改命令
orchbee api snapshot|schema      # 机器快照与内嵌 schema
orchbee site start               # 确保本地 Site 正在运行（无需 TTY）
orchbee site diagnose            # 只读的本地 Site 运行时事实
orchbee site stop --confirm      # 停止托管 Windows Site 及其终端
orchbee site stop-status <id>    # 查询停止尝试或完成回执
orchbee config check             # 本地运行时检查
orchbee help [<family>|<command>]
orchbee completion bash|zsh|fish|powershell|elvish
orchbee --help | --version
```

查询 family 为 `client`、`project`、`workspace`、`tab`、`panel`、`terminal`、`worktree`、
`launch-profile`、`agent`（`list`/`get`，另有
`terminal read`/`wait-output`；Windows `worktree list` 接受 `--workspace <target>` 或
`--cwd <path>`，支持可选的命令局部信任；同时保留严格的 `--directory <directory-id>` 形式；
`launch-profile` 提供 `list` 与 `get <selector>`——对运行中 Site 启动配置目录的一次类型化读取，
由 Server 端解析、CLI 不做本地预读；
`agent` 提供 `list`（支持 `--include-ended`/`--limit`/`--cursor` 分页）、`get <session-id>` 与
`explain <session-id>`——经 OBAP 读取 Server 权威 Agent session 的只读类型化视图，绝不从终端
内容推断——`wait <session-id> [--until <condition>]... [--timeout <ms>]`，在持有的
Client Session 上等待状态或新鲜度；Windows 跟随保留变更，默认无总超时，单次读取仍有界——以及
`doctor [--provider <id>]... [--asset-root <path>]...`，一个离线有界文件系统
诊断，仅凭本地事实解释 Agent 观察为何可能无法到达（不执行 provider 命令、不 connect socket、
不做任何写入）。
Windows `agent explain` 也会将名称和 Panel 解析为精确的物理终端绑定。
规范会话保留 `orchbee.agent.explain` v1；物理终端使用 v2，分别返回有效上报视图与一次新鲜的
屏幕规则评估。结果包含清单来源/版本、同优先级首条胜出规则、所有规则匹配及有界区域证据。
生效的完整生命周期权威直接返回明确的屏幕跳过结果，不读取 Host。一次准入与 5 秒 CLI 总预算
覆盖解析和读取；Server 队列/读取预算为 4 秒。绑定或上报上下文变化时拒绝，不重试。
不发送输入、申请控制器或改变焦点。Windows 启动依次选择本地覆盖、符合条件的远程缓存及内置规则。物理 v3 增加实际路径、缓存版本、覆盖状态及类型化警告；v2 对旧版、无清单或跳过结果保持封闭。Windows `site agent-manifests`、`site reload-agent-manifests` 和 `site agent-manifests-status` 暴露已安装快照及保留的热重载回执。显式 `site update-agent-manifests` 下载并启用经过校验的缓存规则，分别保留逐 provider 缓存结果和启用回执。启用 `update.manifest_check` 时，托管发布版 Site 在启动时及每三十分钟自动检查；调试／测试及独立端点不调度。
参见[屏幕模型](../orchbee-server/docs/agent-screen-model.zh-CN.md)。

Windows `panel create-terminal` 启动未绑定主项或精确成员，或追加子项，不分割 Panel。
它保留字面启动选项、返回保留的等待／最终回执，不改变 Client 焦点。
详见 [Panel 终端创建](docs/windows-panel-terminals.zh-CN.md)。

作用于运行中 Site 的修改命令包括 `project create`/`rename`/`dissolve`/`close-all`、
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
`orchbee.project.rename` 文档下报告 `committed`/`unchanged`。`site start` 是无头
激活路径：它执行与 TUI 启动相同的 connect-or-activate 序列——验证 cohort、准备
运行时目录、取锁、提交标记、在没有运行中 Site 应答时拉起 Server——全程共享一个
20 秒绝对 deadline，然后仅当本次调用实际拉起了子进程时报告 `started`，当既有
Site 准入了其探针时报告 `already-running`。该命令即使在拒绝时也可能产生效果：只有
语法失败保证零效果；被中断或超时的启动可能遗留已提交的激活状态，后续调用会按该
持久状态继续处理。Windows 下一次调用只有在全部 guard 下证明 v2–v5 完整登记的旧代确切进程
全部退出后才自动恢复；
活 owner、不完整登记和旧版 Active 记录仍为 `rejected`/`recovery-required`。恢复保留 checkpoint，
不终止旧进程。新激活会在提交标记、拉起 Server 或发布检查点存储前先发布 catalog v5；只支持
v1–v4 的读取器会拒绝 v5，且不改变其字节。这些边界由
[Windows 恢复契约](../../crates/local-site-runtime/docs/windows-crash-recovery.zh-CN.md)维护。
`site diagnose` 是只读对偶面：它报告运行时目录、标记、锁与有限端点路径的有界事实——
每行是各自观察瞬间的封闭状态行——共享一个协作式 5 秒预算；它绝不 connect、创建、
取锁、修复，也不声称任何进程存活或消失。
`agent integration`（`list`、`explain`、`enable`、`disable`）是本地 provider
接入命令族：仅在本机对调用用户的 provider 配置做有界读取与受管修改——
不建 socket、不取锁、不接触 Site；只有经核实归 Orchbee 所有的条目才会被
安装或移除。完整语法、
target 与 filter 规则、修改结果及含退出分类的 failure matrix 由
[native CLI query](docs/native-cli-query.zh-CN.md) 持有；`orchbee help <family>` 与
`orchbee help <family> <command>` 直接从二进制输出同一套语法。端到端的无头
序列——启动 Site、创建 Project/Workspace/目录、拉起并驱动终端、查看
worktree、观察 Agent 并干净地关闭——见
[原生 CLI 工作流](docs/cli-workflows.zh-CN.md)。

按一次会话通常需要的顺序列出常用命令模板——先按链接的工作流完成
Workspace 与 terminal 准备，再把每个 `<…>` 占位符替换为实际 ID 或
provider：

```sh
orchbee site start                        # 激活一次本地 Site（无需 TTY）
orchbee status                            # Workbench 是否可达？
orchbee project create --label demo       # 随后 workspace create / accept-directory
orchbee tab create --workspace <ws> --label shell \
  --cols 80 --rows 24 --cell-width 8 --cell-height 16
orchbee terminal send <terminal-id> --text 'make test' --enter
orchbee terminal wait-output <terminal-id> --match 'PASS'
orchbee agent integration enable --provider <id>   # 一次性本地接入
orchbee agent list                        # 运行中 Site 观察到的会话
orchbee agent wait <session-id> --until ended
```

Apple Silicon macOS 开发 cohort 支持查询 Site 的 `agent list` 及其有界分页选项。按 Client
投影的状态（`--client`）仍仅限 Windows；其他 Agent 观察命令继续遵循各自文档中的平台范围。

`orchbee help <family>` 直接从二进制输出同一套语法，
`orchbee completion <shell>` 打印对应的按键发现脚本，由你的 shell
自行 source。

Help 只写 stdout，失败只写 stderr；Version 写入一行与 locale 无关的 `orchbee <version>`。
正常关闭 TUI、help、version 以及查询、修改、检查成功的开发退出状态为 `0`；usage failure 为 `2`；
其余失败分类以持有记录的 failure matrix 为准。这些不是未来稳定公开退出状态表。

## Windows 原生快速开始

`workspace create --cwd C:\existing\repo` 用一个请求创建未分组 Workspace 及首个终端。显式 `--project <target>` 添加可选分组；名称和目录均有默认值。可附加 `--env NAME=VALUE`、显式 `--exec`/`--arg` 或 `--profile`。回执返回资源 ID 与终端结果；启动失败时保留 Workspace，供查询和显式关闭。PowerShell 示例见 [Windows 工作流](docs/cli-workflows.zh-CN.md#windows-目录工作区)。

使用仓库固定版本的 Rust、Zig 工具链及 Node.js。在仓库根目录的 PowerShell 中运行；输出目录必须
尚不存在，且父目录已经存在：

```powershell
node scripts/stage-native-site.mjs --output "$env:LOCALAPPDATA\OrchbeeNative"
& "$env:LOCALAPPDATA\OrchbeeNative\orchbee.exe"
```

目录包含四个精确的同级 `.exe`、摘要清单和观察适配器 capsule，必须保存在一起。这是使用 Windows
命名管道和 ConPTY 的原生开发 cohort；交互宿主为 Windows Terminal，无需 WSL。新终端捕获共享启动配置与滚动历史默认值。
内置系统 Shell 选择绝对 PATH 中首个兼容 PowerShell 7 或系统 Windows PowerShell；显式旧
`terminal.default_shell` 只为未声明共享配置提供初值。
Shell 模式及启动生命周期见 [Windows 配置](docs/windows-cli-configuration.zh-CN.md#windows-配置-shell)。
`site start --format json` 也可在非交互控制台中运行。

Windows Site 完全为空且没有保存位置时，`cd <project>; orchbee` 创建以当前目录命名的
未分组 Workspace，并在该目录启动首个终端。之后启动恢复最后显示的 Workspace、Tab
和 Panel；从其他目录启动不会增建 Workspace。详见[目录启动](docs/native-directory-startup.zh-CN.md)
及[启动位置](docs/native-context-selection.zh-CN.md#windows-启动偏好)。

前缀键为 `Ctrl+B`，随后按 `|` 左右分屏、`-` 上下分屏、`h/j/k/l` 切换窗格，或 `d` 脱离。
分屏保留当前焦点。再次运行可执行文件即可重新连接保留的 Site。脱离后 Shell 继续运行；退出的 Shell
仍可观察，直至显式关闭其 Tab/Panel。全部已登记的 Site 进程退出后，再次运行可执行文件会自动开始
新代，并恢复已有 checkpoint。旧 owner 仍活着或无法证明退出时拒绝启动；当前恢复不接管活 Host。
`session list` v2 用 `recoverable` 报告受 guard 保护的冷恢复资格。持续 Workbench checkpoint
会保留布局变更、采样所得的终端目录及 provider 恢复引用，但不保存屏幕内容；详见
[Windows Workbench checkpoint 契约](../orchbee-server/docs/workbench-checkpoint.zh-CN.md)。
`config check --format json` 检查本地 cohort/runtime 输入，不启动 Site。

Windows 已验证原生启动、Unicode 渲染、连续及逐字输入、CLI 终端发送/读取、分屏/焦点/调整大小、
脱离后保留精确终端身份并重新连接，以及 Git worktree 发现。Console 事件进入既有按键及 SGR 鼠标解码器；
退出 TUI 时由模式守卫恢复继承的输入/输出模式。大字节记录采用有界分段写入；取消 exchange 会关闭共享
管道，不留下分离的 flush 线程。

Windows `agent doctor --provider codex --format json` 现在可以检查本地配置、PATH、Node
和胶囊完整性，不启动 Site，也不改变 provider 接入配置。版本 3 文档报告 Windows 端点投递
尚未检查；已安装配置不证明 Agent 已接通。

Windows `agent integration list/explain/enable/disable --provider <id>` 可管理 Claude Code、Codex、Grok、GitHub Copilot、OpenCode、Kilo、Pi、OMP、Gemini 和 Qwen Code。详见 [Windows 集成契约](docs/windows-agent-integrations.zh-CN.md)。Windows `site diagnose` 使用 `orchbee.site.diagnose.windows` v2 报告只读运行时、锁和管道名称事实。Capsule 0.4.16 提供 Claude Code、Codex、Grok 和 GitHub Copilot hook 与 JavaScript 模块，
通过原生 observation 管道与 Console 关联投递。Windows x64 cohort 包含保留关联信息所需的
固定 ConPTY 运行时。其他 provider 保留既有平台限制。
`terminal send --stdin` 接受重定向普通文件和 Windows 管道；所属隐藏 Windows PowerShell
读取进程使管道输入可取消。它从操作系统系统目录解析，绝不通过 PATH 查找，失败时终止并回收。
Shell 补全提供 bash/zsh/fish/powershell/elvish。在 PowerShell 中显式运行
`Invoke-Expression (orchbee completion powershell | Out-String)` 加载，不会修改 profile。
在交互式 Elvish 0.21 中执行 `eval (orchbee completion elvish | slurp)`；静态回调注册
`orchbee` 与 `orchbee.exe`，不查询 Site 或修改 profile。
在 Windows PowerShell 5.1 中使用 Unicode 管道时，在调用脚本内设置 `$OutputEncoding = [Text.UTF8Encoding]::new($false)`；传给原生命令的字节由 PowerShell 编码。Orchbee 原样保留字节，不修复 shell 已经造成的编码损失。
`orchbee --skill` 离线输出内嵌 [Agent 指引](docs/cli-agent-skill.zh-CN.md)。操作系统服务安装、发布签名、
应用自动更新和活 Host 接管仍不在这些命令的范围内。

## macOS 与 Linux 本地启动

从仓库根目录构建一个新的开发 cohort：

```sh
node scripts/stage-native-site.mjs --output /tmp/orchbee-native-dev
# Windows 主机使用 WSL Linux：
node scripts/stage-native-site.mjs --wsl Ubuntu --output /tmp/orchbee-native-dev
# Apple Silicon macOS：
node scripts/stage-native-site.mjs --output /private/tmp/orchbee-native-dev
```

随后在交互式终端执行暂存的 `orchbee`（Linux 使用 `/tmp/orchbee-native-dev/orchbee`，
macOS 使用 `/private/tmp/orchbee-native-dev/orchbee`）。
脚本从锁定输入重新构建三个 Rust release 二进制和 Zig ReleaseSafe Host，
将规范的观察适配器胶囊复制为 cohort 的惰性校验 sidecar，并在写清单前校验其声明的负载摘要；
已有输出目录会被拒绝。可用 `--cargo-target-dir <absolute-directory>` 复用 Cargo 构建缓存。
这会暂存一个开发 cohort，不安装系统服务。

Linux 默认 Site 位于 `/tmp/orchbee-native-<effective-uid>`，macOS 默认位于
`/private/tmp/orchbee-native-<effective-uid>`。开发和测试可将 `ORCHBEE_RUNTIME_BASE`
指向准入本机文件系统上的另一个已有绝对目录来隔离 Site。
[共享运行时契约](../../crates/local-site-runtime/README.zh-CN.md) 定义 0700 目录、
0600 锁/socket/不可变启动标记、声明适配器 sidecar 的四可执行文件精确 cohort、
20 秒共用截止时间和 500 毫秒延迟提示。
Server + Worker 就绪与终端首帧就绪是不同阶段。初始 Open、snapshot read、attach、acquire、
获权后的 resize 以及首帧 Poll 都使用原启动截止时间。原生 Ready 要求完整帧已绘制、其精确反馈已获接受，
并满足当前控制模式尺寸。Worker 消费 Ready reply 后才恢复常规请求预算。
未绘制的 FrameApplied 不会提前发送；NeedSnapshot 恢复反馈可以继续推进首帧获取。

Client 不重启或接管异常 Site。失败/残留运行时明确要求恢复，不自动删除。
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

| 按键                   | 已连接原生行为                                                                                                                      |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `c`                    | 请求一次 Server 拥有的 Tab 创建，再选择已验证的提交成员。                                                                           |
| `n` / `p`              | 在当前 Client snapshot 中选择下一个 / 上一个 Tab。                                                                                  |
| `1` … `9`              | 该位置存在时选择它。                                                                                                                |
| `[` / `]`              | 按 Server 导航顺序选择上一个 / 下一个独立上下文或 Workspace。                                                                       |
| `Shift+N`              | Windows 上使用默认名称和目录创建未分组 Workspace 及首个 shell，然后进入它。                                                         |
| `a`                    | 使用同一一步创建 Workspace 流程。                                                                                                   |
| `e`                    | 配置当前尚未配置且为空的 Workspace 的首个目录。                                                                                     |
| `r` / `R`              | 重命名当前 Workspace / 其所属 Project；standalone 没有这两种目标。                                                                  |
| `<` / `>`              | 将活动 Tab 移到相邻项之前 / 之后，在两端循环。                                                                                      |
| `,`                    | 编辑所选 Tab 的共享 label；首次完整绘制后 Enter 提交，Escape 取消。                                                                 |
| `x`                    | 确认关闭所选 Tab 对所有已连接 Client 的执行，或取消。                                                                               |
| `w`                    | 打开 navigator 并刷新共享 Tab，保留活动终端身份。Windows 中用方向键／Tab 选择，Enter 打开，Esc 返回终端。                           |
| `h` / `j` / `k` / `l`  | 当前 Tab 有分屏树时，将本地焦点移到相邻 Panel。                                                                                     |
| `-` / `_` / `\` / `\|` | 将焦点 Panel 向下 / 向上 / 向左 / 向右分屏出第二个叶子（临时键位）。                                                                |
| `X`                    | 为焦点 Panel 叶子请求关闭确认——确认完整绘制后 `y` 确认关闭，`Esc` 取消；末叶会指名整个 Tab 对所有已挂接 Client 的影响（临时键位）。 |
| `H` / `J` / `K` / `L`  | 将焦点 Panel 叶子移向其方向邻居（临时键位）。                                                                                       |
| `P`                    | Windows 上进入[键盘调整模式](docs/native-keyboard-resize.zh-CN.md)：方向键移动分隔线 5%，Shift+方向键 1%，Esc 退出。                |
| `q` / `d`              | 关闭此 Client，保留的执行继续运行。                                                                                                 |

[上下文选择](docs/native-context-selection.zh-CN.md) 在切换位置时保留各终端的执行。
Workspace 身份独立于可选的 Project。侧边栏将 Agent 直接列在各 Workspace 下，支持同时
展开多个 Workspace 的 Agent 列表。[Workspace 设置](docs/native-workspace-setup.zh-CN.md)
使用默认名称、解析出的目录和首个 shell 直接创建未分组 Workspace；从 Project 菜单创建则
显式加入该 Project。之后可重命名、整理分组；目录接受绝不启动终端。已知部分结果可通过
`orchbee workspace list` 检查；未知结果绝不重放，延迟完成不能覆盖更新的选择。
`Ctrl+B W` 在发现来源后，用生成的分支名和默认起点创建 Git 检出目录；`Ctrl+B O` 按精确
路径选择已有检出目录。[Worktree 打开流程](docs/native-worktrees.zh-CN.md) 添加同级 Workspace，
沿用来源的当前 Project，或将未分组来源和新 Workspace 原子地加入新 Project。其他 Client
保持原选择；提交后 Escape 隐藏观察界面并保留操作 ID。

Windows `Ctrl+B F2` 打开[动作菜单](docs/native-action-menus.zh-CN.md)。
右键点击 pane、tab、Workspace 或 Project 行即可发现已有动作。打开和取消保持焦点；
成员动作使用准确目标导航，关闭动作保留原有确认。Project 重命名为准确的目标打开编辑器，
包括空和非当前 Project，不进入子 Workspace；重命名或取消期间仍选中当前终端。
鼠标打开的菜单靠近点击单元格，并限制在状态行上方。悬停只选择可见行，不执行；点击或 Enter
执行，Esc 或外部点击取消。
Workspace 移动提供已有 Project、不属于 Project、以及原子的新建 Project 并移入操作。
移动非当前 Workspace 保留当前终端。解散 Project 将所有 Workspace 保留为未分组状态；
关闭全部 Workspace 是需要明确确认的独立操作。

`Ctrl+B Shift+D` 打开 Windows [工作区关闭确认](docs/native-workspace-close.zh-CN.md)。
完整绘制后，小写 `y` 只结束该 Workspace 的终端，影响所有 Client；Esc 取消。
Git 目录保留，规范移除后此 Client 回到 Standalone。
[位置标签](docs/native-location-labels.zh-CN.md) 使用预填编辑器与共享 Server 命令；
重命名保留运行中终端，peer 通过 publication 探测或显式刷新看到变化。

Windows 上，用无修饰左键拖动可见 pane 分隔线来预览新位置，松手后应用一次共享并保存的比例。
Esc、导航或 viewport 变化会取消预览，Shift 仍交给外层终端。提交中再次拖动不会排队。
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
[原生 Session 契约](docs/native-tab-session.zh-CN.md) 定义准入、尺寸、反馈、失败、恢复与证据边界。
Close 要求独立生命周期授权，只读 Client 也适用。关闭最后一个 Tab 后留下 Empty；显式 `c`
创建新终端，普通 Client 退出则保留该终端供后续裸 Open 使用。终端被关闭的 peer 通过显式 `w`
恢复当前成员关系。Linux 进程证据覆盖此流程。规范分屏投影、本地面板焦点及
`panel.split` / `panel.close` / `panel.move` 提交保留 Tab 的规范分组和固定元数据；生产重启恢复仍未完成。
面板操作复用同一结构变更准入：split 提交实际控制 cell/pixel 几何并沿用叶子 label，close 以
叶子为作用域并经普通 publication 路径提交，move 将叶子重定位到其方向邻居旁。关闭 Tab 的末叶
会移除整个 Tab——其终端执行被处置，Tab 对每个已挂接 Client 结束，与显式 `x` Tab close 警告的
后果相同——而临时 `X` 键位只能经由已绘制的 [panel 关闭确认](docs/native-panel-close-confirmation.zh-CN.md)
到达该 close：确认指名被冻结的叶及其影响后，才由精确的 `y` 准入。Panel 命令是在 Server `WorkbenchGrant` 能力下准入的 Workbench 变更——split 需要
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
Session。Encoding 成功后启动一个五十秒 monotonic deadline，覆盖完整 frame write 与一次 Server-frame
read；每次 readiness wait 都重新计算剩余 budget，因此 partial progress 不会续期。第一次 write
syscall 之前的 failure 是 definite failure。该 syscall 一旦开始，任何 write 或 read failure、
deadline expiry、malformed 或 incompatible result、错误 result kind 或错误 correlation，都会使
effect outcome 成为 typed `OutcomeUnknown` 并让 connection 退役。transport 绝不 retry，也不返回
request 供 replay。Server 返回的正确 correlated `outcome-unknown` 是有效 OBCS result，而不是
transport ambiguity。

Linux 与 macOS 裸入口通过该 transport 提交独立 Open、查看和控制意图。仅允许关闭的 `LocalClientShutdown`
handle 停止阻塞 exchange，不增加 stream reader 或 writer。重连和结果协调仍未提供。

五十秒提交上限容纳 Server 一个排队及一个在途 Worker exchange（各自最多两秒 write 加二十秒
read）和 Client 结果交付。Handshake 限制保持独立。查看提交验证 action/outcome 兼容性与精确
correlation，并保留 opaque OBSR byte 供已连接的 projection consumer 使用；它不授予 controller 权限，
也不执行终端呈现解码。控制提交针对其 action 验证精确 grant 获取、input acknowledgement 和
完整 resize geometry。Server 报告的 unknown control outcome 原样返回且不重试；不匹配的 outcome
让 transport 退役。Caller 拥有新的 request identity、绑定 grant 的递增 sequence 和后续显式获取。
Session 丢失后的 cleanup 可超出单次提交 deadline。

所有操作既有 viewer 的 action 和所有 controller action 都携带 Server 签发的精确 `attachmentId`。
原生 executor 在 Attach 成功后立即保留该身份，早于任何可能失败或被取代的 setup。
Poll、控制、projection feedback 和 cleanup 都使用这个已捕获的身份；有界 busy 重试保留它，
只更新契约允许刷新的 request identity 和 control sequence。提交前验证 projection feedback
属于其 attachment，绝不重新指定目标。`LocalClient` 不保存隐式的当前 viewer。
已连接的原生 Session 中，活动 Tab 规范 Panel 树的每个可见叶子都拥有自己的 viewer 附着
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
[呈现调度与保留画面](docs/native-tab-session.zh-CN.md#呈现调度与保留画面)。

### 提交正确性记录

| 字段   | 契约                                                                                                                                                                                                                                                                                                      |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | 持有 already admitted same-user Session 的 internal Client 必须提交一条 typed Create、Start、查看或控制 request，并在 transport completion 含糊时避免重复 effect。Endpoint acquisition、process activation 与 presentation 不在范围内。                                                                   |
| 输入   | Trusted typed `TerminalCreateRequest`、`TerminalStartRequest`、`TerminalViewerRequest` 或 `TerminalControlRequest`、admitted `ClientSessionConnection`、accepted generation、finite request budget 与 monotonic time。Server byte 与 readiness outcome 不可信。connection 缺失表示 `SessionUnavailable`。 |
| 数据   | 一次 invocation 拥有一个 `requestId`、encoded frame、absolute deadline、connection state、possible-handoff boundary、expected result kind 与 decoded result class。`LocalClient` 独占其 Session。                                                                                                         |
| 输出   | Exact correlated typed result；definite `RequestEncoding` 或 `RequestNotSubmitted`；带 submitted identity 的 typed `OutcomeUnknown`；或 `SessionUnavailable`。不存在 cancellation、retry、replay return 或 optimistic terminal mutation。                                                                 |
| 不变量 | Encoding 先于 deadline 与 transport；一次 invocation 至多开始一次 frame handoff；只有 exact kind 与 identity 才完成；possible handoff 后的每种 ambiguity 都让 Session 退役；retired Session 不发送 byte。                                                                                                 |
| 决策   | 第一次 possible handoff 前或后；write 未完成或完成；result 在 deadline 前或等于/晚于 deadline；exact 或错误 kind/identity；有效或 malformed frame；known 或 Server-reported unknown outcome。若 acceptance 尚未完成，deadline observation 胜出。                                                          |
| 模型   | 单次调用在一个总截止时间下串行提交请求，每项至多一次。`LocalClient::exchange` 是 implementation workflow；其 deadline-aware nonblocking stream wrapper 是 time 与 I/O adapter。                                                                                                                           |
| 转换   | 下方 transition record 定义 serializable abstract phase 与 guard。Runtime phase 是 call-local；只有 live-or-retired connection state 跨 call 保留。                                                                                                                                                       |
| 依赖   | 一条 already admitted Unix stream、monotonic time、readiness polling、OBCS encoding/decoding 与 same-stream OBLC admission。OBCS workflow 没有 filesystem、process、endpoint-selection、randomness、localization 或 UI capability。                                                                       |
| 所有权 | `LocalClient` 是唯一 stream 与 retirement owner。OBCS 拥有 frame 与 result semantics；OBLC 拥有 admission、credential、cohort、generation 与既有 handshake timeout。                                                                                                                                      |
| 验证   | 假 Server 测试覆盖准确 Create/Start、大 OBSR 记录、不兼容 viewer/control 结果、resize 几何、关联、协议/EOF 失败、静默超时、部分 I/O 期限耗尽、写入前过期、退役及无重复发送。                                                                                                                              |
| 正确性 | LC-01 至 LC-05 映射到下方实现分支与确定性测试。不保留独立提交 TLA 模型。这些案例不证明 OS 调度、任意编解码输入、激活、重连或协调。                                                                                                                                                                        |

### 提交转换记录

| 当前 phase                         | Event 与 guard                                                 | 下一 phase         | Result 与 connection effect                                          |
| ---------------------------------- | -------------------------------------------------------------- | ------------------ | -------------------------------------------------------------------- |
| `ready`                            | Session 存在时提交                                             | `encoding`         | 无 transport effect                                                  |
| `encoding`                         | Frame encoding 失败                                            | `definite-failure` | `RequestEncoding`；Session retained                                  |
| `encoding`                         | Frame encoding 成功并创建一个 absolute deadline                | `write-ready`      | Encoded frame 仅属于本次 invocation                                  |
| `write-ready`                      | 第一次 write syscall 前 deadline 或 I/O setup 失败             | `definite-failure` | `RequestNotSubmitted`；Session retained                              |
| `write-ready`                      | 第一次 write syscall 可能向 kernel 移交 byte                   | `write-begun`      | Handoff count 变为一                                                 |
| `write-begun`                      | 唯一完整 frame 在 expiry 前写完                                | `awaiting-result`  | 在同一 deadline 下等待恰好一条 Server frame                          |
| `write-begun` 或 `awaiting-result` | Deadline、I/O、timeout 或 EOF 胜出                             | `retired`          | `OutcomeUnknown`；connection closed                                  |
| `awaiting-result`                  | Decoded frame 在 expiry 前为 malformed、wrong-kind 或 wrong-ID | `retired`          | `OutcomeUnknown`；connection closed                                  |
| `awaiting-result`                  | Exact correlated result 已 decode 且 deadline 仍 open          | `completed`        | 返回 typed known 或 Server-reported-unknown result；Session retained |
| `retired`                          | 后续任何 submit call                                           | `retired`          | `SessionUnavailable`；不 write 且不 retry                            |

### 正确性可追溯性

| Claim                                                                                                           | Implementation 与 deterministic witness                                                                                                                                   |
| --------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LC-01：encoding 与第一次 write 前的每种 failure 都是 definite。                                                 | `exchange` 在启动 `SubmissionDeadline` 前 encode；`deadline_expiry_before_the_first_write_is_a_definite_failure` 观察到 Server 收到零 byte 且 generation 保留。           |
| LC-02：一次 invocation 至多有一次 request-frame handoff，且绝不 retry。                                         | 一个 encoded buffer 只传给 `write_all` 一次；slow-write 与 silent-timeout test 拒绝第二条 request，后续 call 返回 `SessionUnavailable`。                                  |
| LC-03：possible handoff 后的每种 failure 都是 unknown，并让 Session 退役。                                      | Write/read、EOF、incompatible frame、wrong kind 与 wrong identity branch 都调用 `retire_with_unknown`；focused test 断言 generation 消失。                                |
| LC-04：只有 exact kind 与 identity 才完成；correlated Server `outcome-unknown` 是数据而非 transport ambiguity。 | Exact Create/Start、wrong-kind、wrong-ID 与 correlated-unknown fake-Server test 覆盖 result classifier。                                                                  |
| LC-05：一个 monotonic deadline 覆盖完整 write 与 read，progress 不会续期。                                      | `SubmissionDeadline` 只创建一次；每个 readiness boundary 重新计算余量。Stepping-clock test 在 read 与 write 两侧制造 sub-deadline byte progress，直到 total budget 用尽。 |

## TUI shell 底座

Application-private shell adapter 拥有以下终端与呈现边界。裸 Linux 与 macOS 启动使用准备好的 Session；
未连接的 `run` 与显式 `run_connected` 仍是测试与组合接缝。

下面的输入与转换表描述未连接时的 fallback。已连接的上下文和 Tab 操作由上文的原生 Session 处理。

| 关注点      | 当前契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 准入        | 已暂存的 Unix adapter 要求继承的 stdin 与 stdout 都是 TTY，且具有相同 terminal-device identity；它只复制这些已准入 descriptor，并从复制后的 stdout descriptor 读取 cell 与 pixel dimension。它没有 `/dev/tty`、Crossterm global size 或 process fallback。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 生命周期    | 一个已暂存 guard 拥有 raw mode、alternate screen 与 cursor visibility。它会在 operation 可能部分成功前标记 cleanup responsibility，在 inverse 成功前保留每个 flag，并为每个失败的 inverse 最多尝试两次，随后返回一个 aggregate error。Process-lifetime singleton panic hook 只在发生 panic 的 thread 是 active guard owner 时派发，因此其他 thread 无法在保存 terminal state 前清除 setup lease；guard drop 不会移除后来安装的 hook。正常返回、initialization/event/draw failure、owner-thread panic，以及 `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` 中的每一种，都有确定性 restoration 或 signal-observation 证据。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Client View | 每个 invocation 拥有 prefix mode、viewport class、sidebar projection、active overlay 与 transient notice。它不拥有任何共享 Tab、Panel、Workspace、Site 或 Terminal fact，也不持久化任何内容。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| 布局        | [Shell 布局](docs/tui-shell-layout.zh-CN.md) 拥有视口级别、有界侧栏宽度与终端几何。Windows 侧栏将 Agent 嵌在精确归属的 Workspace 或 Standalone 下面，共用一棵可滚动树；展开操作保留终端选择与键盘归属。Tab 顶栏和侧栏的 `+` 控件复用已有创建动作及菜单。规范 Panel 身份与终端归属保持不变。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 输入        | 已暂存 adapter 每次最多从已准入 stdin descriptor poll 并读取 4,096 个 raw byte。一个有界 legacy decoder 把字节流转成 typed item——一个 UTF-8 标量、一个 CSI/SS3 或 kitty-`u` 键、一段原子 bracketed-paste、或一个 non-decision 完整序列。每个正在收集的 frame（ESC 等待、CSI/SS3、UTF-8 标量、paste 段）在有界注入等待内跨 read 持有自己的字节；在被捕获 packet 中打开的 frame 即使跨 read 完成也保留其 owned 归属，越界或超时 frame 会 drain 到可靠边界而不是让尾巴漏进 binding，paste 段只按 opener 记法配对的终结符结束。legacy decoder 对无效 UTF-8、越界或超时的粘贴只产生一次拒绝，不保留 raw 内容，然后静默消费到对应终止符；Pane 不会收到被拒绝的片段，已启用的 prefix 仍保持等待。attach 和文本编辑器保留已有的拒绝反馈，普通 Pane 与 prefix 分派不增加全局提示。每次完整 read 都会先依序 dispatch，之后才允许后续 signal、viewport query 或 resize decision 结束本轮 loop。`Ctrl+B` 进入可见 prefix mode。`b`、`w`、`?`、`q` 与 `d` 驱动本地 shell state 或退出；已连接的 `c`、Tab 选择和 `h`/`j`/`k`/`l` action 由 Native Session 处理。在 prefix、调整模式和独占捕获外，普通 byte——包括 `Ctrl+C`、`Ctrl+D`、escape sequence、UTF-8，以及非模态 shortcut Help 可见时的输入——都会逐字节按原序到达可注入 terminal-input port；单独的 `Esc` 在有界等待后才 forward，prefix 后的 `Ctrl+B` 会 forwarding 一个 literal byte。 |
| 结果        | Shell 消费准备好的 Session。Bootstrap 拥有激活，Server 拥有终端身份，Host 拥有终端生命周期。Shell 不改变 Workbench 状态。                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |

确定性 Client View transition model 如下：

| 状态与事件                                     | 下一状态、命令与可见结果                                                                                                                                                                                                                                                                                                                                              |
| ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Normal + `Ctrl+B`                              | 进入 Prefix；不发出 terminal input；渲染 prefix indicator。                                                                                                                                                                                                                                                                                                           |
| Normal + 其他 byte，在独占捕获外               | 保留本地 view state，并将确切、有序的 byte 提供给 terminal-input port。Unavailable port 会在无副作用的情况下拒绝，已暂存 renderer 会报告 input 未发送。                                                                                                                                                                                                               |
| Prefix + `b`                                   | 离开 Prefix，并为此 Client View 切换停靠的宽屏 sidebar 或窄屏 sidebar overlay。                                                                                                                                                                                                                                                                                       |
| Prefix + `w` 或 `?`                            | 离开 Prefix，并打开 Workbench navigator 或 shortcut Help。Windows 导航器捕获键盘与粘贴：方向键和 Tab 浏览，Enter 打开已绘制目标，Esc 或配置后的切换键返回；普通 Help 保持非模态，再按其绑定即可关闭。                                                                                                                                                                 |
| Prefix + `PageUp` 或 `PageDown`                | 离开 Prefix，并且仅当 shortcut-help overlay 打开时翻页；页码属于 Client-local state、以 registry 行数为界、帮助关闭再打开时归零，并由 UI loop 按 renderer 每次绘制回执的有效页码结算（例如 resize 后）。帮助关闭时，Windows Prefix + PageUp 进入保留历史，PageDown 仍未绑定；其他平台保持未绑定行为。在独占捕获与保留历史外，无 Prefix 的按键原样到达 terminal port。 |
| Windows 上 Prefix + `P`                        | 进入 Client 本地调整模式。方向键及 Shift+方向键调整共享比例；未绑定键和粘贴被消费，`?` 显示模式帮助，Esc 将输入交回 pane。                                                                                                                                                                                                                                            |
| Prefix + 其他完整键或完整序列                  | 离开 Prefix 并报告 unknown-prefix notice；一个完整的未知序列（CSI 尾巴、UTF-8 标量、带修饰键）恰为一个未绑定事件，绝不会把字节留在 pane 中。                                                                                                                                                                                                                          |
| Prefix + `c`、`n`、`p`、`1` … `9` 或 `h/j/k/l` | 离开 Prefix；已连接 Session 执行对应的 Tab 或本地 Panel 焦点操作，未连接 fallback 保持共享状态不变并显示不可用；不由 shell fallback 提交 command。                                                                                                                                                                                                                    |
| Prefix + `m`                                   | 离开 Prefix，并通过租约模式对释放或恢复宿主指针捕获；released 期间状态行显示持久指示，宿主原生选择与滚动不受影响。鼠标捕获、命中测试、pane 透传与滚轮回退由[原生鼠标输入](docs/native-mouse-input.zh-CN.md)治理。                                                                                                                                                     |
| Prefix + `Ctrl+B`                              | 离开 Prefix，并向 terminal-input port 提供一个 literal `Ctrl+B` event。                                                                                                                                                                                                                                                                                               |
| Prefix + `q` 或 `d`                            | 离开 Prefix，并关闭此 Client shell。                                                                                                                                                                                                                                                                                                                                  |
| Prefix + `Esc`                                 | 取消 Prefix，且不改变 overlay。在 Prefix、调整模式及 Windows 导航器捕获外，`Esc` 及其后续 byte 都是普通 terminal input。                                                                                                                                                                                                                                              |
| Resize                                         | 查询已准入 stdout descriptor，更新 fixed backend viewport，并且只重新计算 Client-local viewport class。进入 narrow mode 会隐藏停靠 sidebar；回到 wide 会关闭 sidebar overlay，并恢复此前的 wide-sidebar preference。                                                                                                                                                  |
| Event/draw error、panic 或 termination signal  | 停止 loop，并尝试恢复 cursor、alternate-screen 与 raw-mode，不启动或停止其他 process。                                                                                                                                                                                                                                                                                |

不变量是：每个 prefix sequence 至多产生一个 typed command；非 prefix byte 保持顺序且逐字节一致；一个完整的未知序列是单个 non-decision 事件而不是尾巴泄漏；未连接 adapter 绝不改变 shared Workbench 与 terminal state；每个可能取得的 terminal mode 都会
保留一个有界 restoration owner。Unit test 覆盖 state、input-port、fixed-backend、layout、renderer、
真实 PTY 的 same-terminal admission、实际 panic hook、全部四种 signal、cross-thread setup
interleaving、post-read viewport/resize failure 与 lifecycle fault injection。Native PTY process test
则分别证明缺失 cohort 的 bare 与 help/version path 不改变 terminal mode、Linux PTY master 在最后一个 slave
关闭后的 `EIO` 是正常 hangup，并且 linked public binary 不包含 `/dev/tty` 或 `tput` fallback token。
这些进程见证描述 Unix 适配器。Windows 适配器及其独立的 Console、ConPTY 证据见上文。

## 已连接单终端 TUI 细化

已接受的本地终端里程碑让私有 TUI 成为 `LocalClient` 的实际消费者：`run_connected` 接收已准入
Client 和显式创建或附着意图。它不选择 endpoint、激活服务、改变公开语法或发布共享 Workbench
事实。真实进程／PTY 见证已经经此 adapter 呈现 Server/Worker/Host 终端；裸 Linux 与 macOS 启动在完成有界 Site 激活后，进入同一个已准备好的 shell。

以下记录保留仅供测试使用的私有 Create/Attach 适配器。其实现位于
[`terminal_session/tests/legacy.rs`](src/terminal_session/tests/legacy.rs)，服务于现有的
[Session 回归测试](src/terminal_session/tests.rs) 和
[Unix 进程／PTY 见证](src/tui_shell/tests/connected.rs)。Standalone 启动使用 native
Tab Session；对应的[传输回归测试](src/terminal_session/connection/native/tests.rs) 验证生产后端。

| 字段   | 契约                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题   | Shell 已有输入和生命周期端口，但不能显示或控制保留终端。接入一个 Session，同时保留及时退出、有界队列、精确反馈和终端生命周期。                                                                                                                                                                                                                                                                                                                                                                            |
| 输入   | 已准入 LocalClient、可选默认 Shell cwd 或已接受 create 引用、继承终端的 geometry 和原始输入、不可信 OBCS/OBSR 响应，以及终止信号。                                                                                                                                                                                                                                                                                                                                                                        |
| 数据   | 一个拥有连接的 task、八条排队 command（每条 input 最多 4,096 byte）、一个 response slot、一个待发 frame feedback、一个有界文本投影、新 request ID 和一个 controller sequence。只有 Host 拥有 VT 和 history。获取已有终端也提交初始 panel geometry；input 与后续 resize 共用同一递增 sequence。                                                                                                                                                                                                            |
| 输出   | Create/attach 和显式 acquire、按序准入的原始输入和 geometry、文本／style／cursor 呈现、精确 applied feedback 或 NeedSnapshot、本地化连接中／只读／blocked／失败状态，以及仅针对 Session 的清理。                                                                                                                                                                                                                                                                                                          |
| 不变量 | UI 线程不执行 Server I/O。Task 串行交换且不重试副作用。队列已满的输入被明确判定为未提交。Unknown outcome 关闭 task 并丢弃未发送 command。没有 task 超过其 TUI owner 的生命周期。                                                                                                                                                                                                                                                                                                                          |
| 模型   | Opening -> 按需执行 standalone snapshot selection -> 已附着 controller 或只读 viewer -> polling -> 收到 record -> 原子文本投影提交 -> draw -> feedback -> polling。控制意图与单一 projection flight 分开排队。关闭会 shutdown 已准入 socket 并 join task；Server 拥有按代次划分的 detach cleanup。                                                                                                                                                                                                        |
| 决策   | Snapshot 替换全部行且不回退已提交 revision（包括 projection reset 后）；delta 要求精确 attachment、epoch、sequence、generation、base revision、screen 和 geometry。无效内容请求新 snapshot 且不应用任何行。错误 envelope 或 scope 让 Session 退役。ProjectionBlocked 清除呈现，直到更高 revision 的 snapshot。没有自动提升 controller、reconnect 或 unknown input 重放。                                                                                                                                  |
| 依赖   | 既有 OBCS 和 Rust Terminal Protocol binding、有界 channel、POSIX socket shutdown、已准入 descriptor、现有 Ratatui renderer 和本地化文案。不增加三方依赖、直连 Host 路径或 VT 模拟器。                                                                                                                                                                                                                                                                                                                     |
| 所有权 | `terminal_session` 拥有连接 task 和输入准入；`terminal_projection` 拥有 Client 文本状态和反馈；Terminal Protocol binding 拥有有界 OBTF 文本解码；shell 拥有 mode 与 draw 顺序。                                                                                                                                                                                                                                                                                                                           |
| 验证   | Canonical OBTF fixture、畸形／边界分区、确定性的 snapshot/delta 和队列／退役轨迹、render buffer 断言，以及真实进程／PTY 见证。[Session 生命周期测试](src/terminal_session/tests.rs) 和[保留适配器的背压测试](src/terminal_session/tests/legacy/connection/backpressure/tests.rs) 映射 transport 退役及 projection/control 顺序；其有限边界不是性能保证。                                                                                                                                                  |
| 限制   | 投影文本 cell 和基本 style/cursor；排除 graphics、hyperlink 激活、history、selection、terminal interaction-mode negotiation、cursor shape 对齐和完整 Workbench 行为。不支持的 cell width 使用替代字形。缺失或无效的继承 cell/pixel geometry 会在 Create 或初始 attach 前拒绝，或经恢复边界结束活动 TUI；绝不编造 pixel size。Optional extension 内容保持 opaque，文本 decoder 不保留它们。投影按共享文本分配计数，在提交前拒绝超过 36 MiB 的保留文本。显示 cell 中的控制字符绝不成为终端 escape command。 |
| 审查   | 检查覆盖内存放大、至多一次副作用、draw 失败反馈和 join 取消。原 authority-model 超时问题已解决；启动链路另有独立审查与 Linux 进程证据。                                                                                                                                                                                                                                                                                                                                                                   |

`terminal_text` 拥有禁止显示的 Unicode 标量规则：Unicode 控制字符及 U+2028/U+2029。
终端单元格渲染将该规则与既有宽度检查一起使用，后端最终的 `Print` 还会独立地把禁止标量替换为
U+FFFD。安全文本通过借用使用，不产生分配；规范字符串保持不变。捕获输出测试验证单元格载荷
不能注入终端命令，同时保留后端自身的光标与样式命令。

## Client 已知未改变结果的背压处理

Carrier 保留一个排队请求和一个 wire owner；即使 wire 空闲，队列也可能短暂已满。
Terminal Session 为精确的 viewer/control ServerBusy 结果提供有界处理：
每次都是已完成、已关联且没有派发 Host action 的请求。等待 10 毫秒后，以新 ID 发起新请求。
Input 和 Resize 还使用新的递增 controller sequence，因为 Server 在队列准入前已消耗前一个序号。
Carrier Backpressure 在 OBCN dispatch 前拒绝；Attach 的 Worker 容量拒绝可能已完成 OBCN exchange，
但在创建 Host attachment 前拒绝。逻辑操作只保留一个绝对截止时间；首帧前受原启动截止时间约束，之后使用常规 50 秒预算。
每次底层 LocalClient 提交仍只写入一次。

Create、Open 和 Start 绝不重新提交。普通拒绝、controller 竞争、运行时不可用、错误响应、
transport 失败和未知结果均为最终结果。Feedback 在接受前保持同一个精确的待确认内容；
此策略不能确认未绘制帧。Client 取消关闭 stream、打断 exchange，在正常调度下至多等待一次
10 毫秒退避后 join。到期或身份耗尽会结束操作。

| 声明                                 | 细化映射和证据                                                                                                                                                                                                                                           |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 只有已知未改变的容量结果允许再次尝试 | Server viewer/control 的 ServerBusy 分支证明 Host effect 未发生；Client 仅匹配这个精确结果。Fake Server 分区检查 busy、拒绝、不可用和未知结果。                                                                                                          |
| 不重放任何可能已发生的副作用         | [LocalClient 测试](src/local_client/tests.rs) 检查单次 exchange 后的超时与 EOF 不会触发重提交；[native command 轨迹](src/terminal_session/connection/native/tests/commands.rs) 检查 busy input/resize/feedback，以及输入结果不明后的退役。               |
| 截止时间与身份不重置                 | 同一操作截止时间覆盖全部尝试；请求 ID 与 controller sequence 递增；有界 fake Server 轨迹覆盖到期和 busy Resize。                                                                                                                                         |
| 保留反馈与取消仍有界                 | [Mailbox 测试](src/terminal_session/mailbox/tests.rs) 检查八条记录的容量；[native 传输测试](src/terminal_session/connection/native/tests.rs) 检查精确的已绘制帧反馈；[Session 生命周期测试](src/terminal_session/tests.rs) 检查保留适配器释放时的 join。 |

## 本地化与权威

Authored help 与 diagnostic 同时提供 `en-US` 和 `zh-CN`。Locale selection 依次检查非空
`LC_ALL`、`LC_MESSAGES` 与 `LANG`；以 `zh` 开头的 language prefix 选择 `zh-CN`，其他 value
均 fallback 到 canonical `en-US`。Locale data 只影响 presentation，绝不会持久化或用作 authority。

只有 help/version/usage 一次性路径仅依赖标准库参数、环境和流。Windows 与 Linux 的 bare invocation 组合共享 runtime、私有 Client transport 与 TUI；platform adapter 负责 console 与 transport effect。原生应用拥有当前开发中的 CLI/TUI 界面。

## POSIX 本机 Site rendezvous 底座

共享所有权与完整原语建模记录现由
[local-site-runtime](../../crates/local-site-runtime/README.zh-CN.md) 维护；应用不再保留重复实现。

## 已接受的本机 Site activation 建模记录

| 字段   | 已接受的 refinement target                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 问题   | 两个并发的本机 `orchbee` TUI invocation 必须通过同一次 Site activation attempt 协调，且二者都不能成为 Server 或 Worker 的 lifetime supervisor。已存在的 compatible Site 不应产生 launch；Site 不存在时至多启动一个 exact installed cohort；每个非成功 outcome 都必须显式。                                                                                                                                                           |
| 输入   | 两个 Client invocation slot；TUI、help 与 version intent；absent、compatible-ready、incompatible 或 partial 的 initial Site observation；一次 activation-lock result；exact installed Server-plus-Worker cohort capability；typed readiness、incompatibility、partial-cohort 与 launcher-failure observation；以及 abstract monotonic total-deadline expiry。External process outcome 不受信，且可以永远不抵达。                     |
| 数据   | 每个 Client 的 phase、typed result 与 connection path；一个可选 activation-lock owner；zero-or-one launch count 与 exact-installed cohort tag；Site 与 typed-readiness state；以及 Site-owned ready-service lifetime marker。不包含 PID、path、runtime-base、scope、ACL、process tree、cleanup record、recovery identity、service-manager unit 或 numeric time value。                                                               |
| 输出   | Compatible present Site 直接连接。Site 不存在时，一个 lock owner 启动 exact installed cohort，contender 等待 typed readiness。成功时连接并释放 caller 的 activation resource。Incompatible Site、partial cohort、launcher failure 与 deadline expiry 是不同的 failure。Help 与 version 完成时不 discovery 或 activate。Client exit 只结束该 Client。                                                                                 |
| 不变量 | 每条 TUI path 都先尝试 connection 再 activation；activation owner 唯一；launch count 至多为一，且每次 launch 都是在 lock 下启动 exact installed cohort；contender 成为 readiness waiter；只有 ready compatible Site 能产生 connection；settled Client 不拥有 activation resource；ready-service lifetime 属于 Site；failure 与 Client exit 都不停止或改写 Site state；help/version 保留全部 activation fact。                        |
| 决策   | Invocation kind 决定是否允许执行 Site work。Connection observation 区分 direct success、absent activation、incompatible failure 与 partial-cohort failure。发现 absent 后，lock ownership 区分 launcher 与 waiter；lock winner 在 launch 前重新检查 Site state。Typed readiness、typed failure 或 Client-owned total deadline 结算每个 waiter。在 connection 完成之前，deadline observation 可以胜出。                               |
| 模型   | 对恰好两个 Client slot 与四种受支持 initial Site class 建模的 bounded workflow。一个 scalar `activationOwner` 对 `activation.lock` 建模；`launchCount` 与 `launchedCohort` 暴露 singleton exact-cohort launch；`serviceLifetimeOwner` 把 ready Site 与每个 Client 区分。External launcher/readiness action 是 nondeterministic，且没有 fairness assumption。                                                                         |
| 转换   | Present compatible Site 下为 `idle -> connecting -> connected`；present incompatible 或 partial state 下为 `connecting -> failed`；launch winner 为 `connecting -> lock-ready -> lock-held -> waiting-readiness`；contention 为 `lock-ready -> waiting-readiness`；waiting 或 recheck state 会结算为 connected 或一种 typed failure；`connected -> exited` 保留 Site state。Help/version 从 `idle` 直接进入 information completion。 |
| 依赖   | Model 仅在具名 abstraction 上组合已接受 command contract、当前 private `LocalClient` admission boundary 与 `ModeCheckedLocalSiteRendezvous` lock semantics。未来 runtime refinement 必须提供 installation-owned cohort selection、typed readiness 与一个 monotonic total deadline。本 slice 不增加 process、socket、filesystem、localization 或 public-command implementation。                                                      |
| 所有权 | `apps/orchbee` 拥有 Client bootstrap workflow、deadline、activation guard 与 typed Client result。Installation composition 拥有 exact cohort capability。Server 与 Worker 在成功后拥有各自 readiness 与 service lifetime。Model 不向 Client 授予 supervise、stop、replace、downgrade、clean up 或 recover Site 的 authority。                                                                                                        |
| 验证   | 启动状态、期限与真实进程测试覆盖受支持初始 Site 类、竞争 Client、准确 cohort 启动、既有服务保留及失败结算。共享 LocalSiteStartup 核心仅提供其单独声明的有界协议证据。                                                                                                                                                                                                                                                                |
| 正确性 | LA-01 至 LA-05 仍是启动与准入行为要求。受控期限及进程轨迹必须展示无需假设外部进程响应的结算。已移除激活流程不提供当前 TLC 证明。                                                                                                                                                                                                                                                                                                     |
| 限制   | 该 abstraction 只证明 checked two-Client、one-Site state space。它不证明 runtime refinement、operating-system scheduling、process creation 或 detachment、daemonization、service-manager behavior、runtime base 或 scope、ACL、socket metadata、cleanup、recovery、retry、update、concrete deadline 或 progress threshold、localization、TUI behavior、packaging、public wiring 或 external process progress。                       |

| Transition owner     | Event 与 guard                                                   | Result                                                                                                            |
| -------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Invocation           | Help 或 version                                                  | Information completion；Site、lock、launch、readiness 与 service-lifetime fact 不变                               |
| Client connection    | Compatible Site 已 ready                                         | Direct Client connection；无 activation resource 或 launch                                                        |
| Client connection    | Site incompatible 或 partial                                     | 对应 typed failure；无 activation 或 Site mutation                                                                |
| Activation contender | Initial connection 找不到 ready Site 且 `activation.lock` 已持有 | 在 caller 的 total deadline 下等待同一个 typed readiness result                                                   |
| Activation winner    | Lock 已获得、recheck 仍发现 absent 且不存在 launch               | 启动恰好一个 installed Server-plus-Worker cohort，并在持有 guard 时等待                                           |
| Activation winner    | Recheck 发现 ready、in-progress、incompatible、partial 或 failed | 根据 exact observation 连接、释放并等待或失败；绝不启动第二个 cohort                                              |
| External cohort      | Launch pending                                                   | 可以发布 ready、incompatible、partial、launcher failure，也可以完全不返回；external completion 没有 fairness      |
| Pending Client       | 观察到 typed result 或 total deadline                            | 连接或返回 `incompatible`、`partial-cohort`、`launcher-failure` 或 `timeout`；释放 caller 拥有的 activation guard |
| Connected Client     | Client exits                                                     | 只结束 Client Session；保留 Site、cohort、readiness、launch 与 service-lifetime state                             |

该模型抽象了运行时细化；`LocalSiteStartup` 提供具体的启动安全论证。在声称该模型映射到实现
之前，未来接线必须补充确定性进程测试。

| Claim                                                                                         | Bound                                                                                                                           |
| --------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| LA-01：每个 activating TUI 都先连接；present compatible、help 与 version path 从不 launch。   | 对 command intent 与 abstract observation 建模，不覆盖 parser、transport 或 process refinement。                                |
| LA-02：并发 absent 在一个 lock 下至多产生一次 exact installed-cohort launch。                 | 恰好两个 Client slot 与一个 activation round；不声称 runtime-base、lock-file 或 process implementation。                        |
| LA-03：incompatibility、partial cohort、launcher failure 与 timeout 均显式，且不会停止 Site。 | Failure presentation、cleanup、recovery、retry 与 stale-artifact policy 仍不在范围内。                                          |
| LA-04：connection 释放 Client activation ownership；Client exit 不 supervise 或 stop Site。   | 只对 ready-service ownership 建模；不选择 process 如何 detach 或 service lifetime 如何实现。                                    |
| LA-05：每个 started TUI 都会结算，且不假设 external process 响应。                            | 假设未来 adapter 确实保证观察到一个 finite monotonic total deadline；不选择 numeric value、OS scheduling 或 external liveness。 |

## 测量

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
960×640 pixel，测量 5 个全新 Site 和每个 Site 3 次重连。隔离调用环境仅包含账号的
HOME/USER/LOGNAME、Cargo 和标准可执行文件路径、仓库根目录 PWD、
`LANG=LC_ALL=en_US.UTF-8`、`SHELL=/usr/bin/zsh`、`TERM=xterm-256color` 和唯一的
`ORCHBEE_RUNTIME_BASE`。文件系统缓存已预热；新 Site 不代表冷 OS 缓存。

记录的界面计时从 Client spawn 到一次完整绘制，其中包含 `[*1 1]` 和当时底行的
`Tab 1 · Ready · controlling` footer。该节点位于完整帧绘制、精确 FrameApplied /
FeedbackAccepted exchange 及最新尺寸的 UI Ready 发布之后；不证明 Worker 已消费 Ready reply
或清除启动上限。Client VmHWM 在该节点采样：它衡量截至 Ready 的驻留内存峰值，
不代表整个 TUI 生命周期的峰值。
当前观察器在采样前显式打开并关闭前缀状态浮层；新计时包含这一输入往返，
不能与这些参考数字直接比较。每次关闭均验证 terminal mode 精确恢复并退出
alternate screen。每次热重连之前，在计时区间之外按条件等待，确认先前 Server Session task
和 socket 已退出。排队中的 viewer Detach 没有公开完成见证；残余 detach 工作可能计入下次
attachment 的测量延迟。此前单终端 reference 使用了不同的就绪见证，因此这些观察不构成严格的前后对照。

| 测量                                                | 参考观察                         |
| --------------------------------------------------- | -------------------------------- |
| Client 可执行文件大小                               | 2,989,768 bytes                  |
| Version 启动 min / p50 / p95 / max                  | 0.791 / 0.834 / 1.176 / 1.406 ms |
| Version 峰值驻留内存                                | 2,818,048 bytes                  |
| 新 Site 完整 Ready 帧 p50 / p95（5 次）             | 485.526 / 501.903 ms             |
| 热 Site 完整 Ready 帧 p50 / p95（15 次）            | 439.502 / 486.007 ms             |
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
node scripts/tla/run-models.mjs --models local-site-startup
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
真实进程验收。Fixture 先校验该 cohort，再复制到各自隔离目录；测试不改变源产物或默认 Site。

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

## 独立终端意图

私有的已连接 TUI 接受 `TerminalIntent::Standalone`。它提交一次
`submit_terminal_open` 并要求 Opened，然后在同一条已准入连接上读取相关联的 OBWP snapshot。
它使用 snapshot 中确切的 Site/Worker/Server-terminal binding 提交 OBCS `attach-binding`；
Open create reference 仍是显式旧路径的值，不用于选择此 viewer。Linux 与 macOS 裸入口通过已校验的
本地 Site 激活与 Worker 按需启动 Host 消费此路径。显式 Create 与 Attach intent
保留现有 Create-reference attachment 行为。

Windows 裸启动先读取 canonical 成员关系，再解析保存位置或符合条件的空 Site
[目录创建](docs/native-directory-startup.zh-CN.md)。已有 Site 没有保存位置时，执行 standalone
Open 后重新读取 snapshot。这些路径共享本 bootstrap/attachment owner 和原截止。

纯投影器按稳定身份解析恰好一个规范 context，允许零到 256 个有序 Tab。它保留标签、固定状态、
分组元数据、成员身份、split/slot 拓扑、presentation 与 terminal binding。固定 Tab 构成前缀，
组成员保持连续；不受支持的成员关系会使整个投影被拒绝。Canonical OBWP validation 拥有全局唯一性
和引用完整性。刷新与已创建成员验证器校验修订、成员不可变事实及关联创建事实。
[原生 Tab 分组契约](docs/native-tab-groups.zh-CN.md)管理组标题、本地展开状态、配置菜单及实际 GUI/TUI 能力覆盖。

终端投影适配器还为原生 Session 提供控制模式下的完整帧尺寸门。传入期望尺寸时，四个字段必须
全部匹配；不匹配会清除可见帧并请求快照，但不会重置序号、修订号或阻塞修订号栅栏。原生控制模式
建立流程使用该尺寸门；保留的显式 Create/Attach adapter 不传入期望尺寸。针对性测试覆盖每个字段，
以及通过新的匹配完整帧恢复。

Open、snapshot read、attach、controller 获取、首次获权 Resize 和第一个已校验的终端
projection frame 共用现有 startup ceiling。Observer 不调整大小。Snapshot exchange
保持单一 carrier 及现有非阻塞 deadline I/O owner；Workbench request ID 与 OBCS correlation
及逻辑终端 ID 保持独立。

| 条件                                                                                 | Carrier 状态及 standalone 行为                                                         |
| ------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------- |
| 本地编码或确定的首次写入前失败                                                       | 保留 carrier，返回明确的 not-submitted 失败；workflow 以 unavailable 退出，不重试。    |
| 可能 handoff 后发生 I/O 错误；畸形、超限、截断、错误 kind 或错误关联的 snapshot 响应 | 退役混合 carrier，报告 snapshot-read 失败；已经成功的 Open 不变成未知 launch。         |
| 有效的 denied 或 unavailable snapshot 结果                                           | 返回有类型的结果，不执行协议 retirement；workflow 以 unavailable 退出并丢弃 Session。  |
| 有效 snapshot 的 standalone 结构不受支持                                             | 以 unavailable 退出，不再次选择，也不退回 Open reference。                             |
| 已知未改变状态的 viewer server-busy 结果                                             | 现有 retry 保留确切 binding 三元组，使用新 OBCS request ID，并保留同一个逻辑截止时间。 |
| Unknown Open/submission 或不明确的 attachment 结果                                   | 停止，不重放 Open、不改为 Create，也不退回 Create-reference attachment。               |

Snapshot read 不重试。Attachment 后仍须完成现有 controller 与 projection 准入，
持有 binding 不授予 controller 权限。独立 Site/Worker/Terminal 身份域中的相同编码字节是
有效的；每个字段独立拥有类型并接受校验。

[Server 所有的记录](../orchbee-server/docs/workbench-authority.zh-CN.md#从-workbench-绑定选择终端)
定义确切 membership lookup、失败优先级、保留 Worker generation 与 attachment 线性化。
此 Client 不拥有独立终端，也不写入 Workbench 成员关系。Session trace 覆盖混合请求顺序、
确切选择三元组、结构拒绝、有界 I/O 和不存在 fallback；新构建 release 的 PTY 测试覆盖
首次打开及返回保留 shell。Carrier 与 Client 期限测试负责进展及结算。远程访问、重启后接管、
持久化及完整 Workbench Client 不属于此切片。

已接受的[原生 Tab 会话契约](docs/native-tab-session.zh-CN.md)负责定义多 Tab 创建、本地选择、
显式刷新及其共享载体上的操作屏障，并规定相应的实现与进程验证要求。纯投影与分类 TabCreate
适配器已经实现。适配器区分明确提交前、已提交及未知结果；调用方截止时间方法从同一个注入的
共享时钟计算普通、调用方与启动上限，采用其中最早者。确定性的会话状态与有界邮箱实现原子的
选择准入、Create/刷新屏障、精确事件结算、尺寸门及载体退役。退役保留首个原因，并区分明确的
未知结果与确定失败。这些核心实现已经与原生连接执行器及 UI 消费方集成。屏障生成测试、
脚本化载体轨迹及 Linux 上真实的双 Client Tab/刷新进程见证提供实现证据；契约的
[声明与证据矩阵](docs/native-tab-session.zh-CN.md#声明证据矩阵)记录已检查的声明及其边界。
这些进程证据不证明其他操作系统、安装、重启恢复或远程访问。

`tab create` 和 `panel split` 还接受单次启动的 `--cwd <absolute-path>` 和可重复的
`--env NAME=VALUE`，可与 `--profile` 或默认 shell 配合。cwd 覆盖 Workspace 目录，
Windows 省略时先尝试跟随已解析来源的根进程目录；`--cwd workspace` 保留 Workspace／Worker 主目录默认值。环境值逐字传递且有界；参见命令帮助与
[原生 CLI 合约](docs/native-cli-query.zh-CN.md)。

面向单个 Agent 的命令还支持 `--panel <id|@panel:suffix>` 或位置参数 Panel ID。位置参数优先按 Session ID 和名称解析，再回退到 Panel。Panel 选择主终端且要求唯一存活 Agent；等待固定到选定的 Session。

## Windows 显式 Site 停止

`site stop --confirm` 结束现有托管 Windows Site 及其终端进程。使用 `--detach` 仅等待准入，保留返回的标识供 `site stop-status <id>` 查询。显式重试必须使用 `--retry --request-id <id> --confirm`。取消或超时不撤销已接受的停止。准确匹配的已停止记录与空闲生命周期锁证明完成；下次 `site start` 消费记录并创建新 Site。详见 [CLI 停止契约](docs/native-cli-query.zh-CN.md#windows-site-停止)。尚未实现 Linux/macOS 停止运行时。

## Windows Worktree 操作

`worktree create/open/remove/status/resume` 使用 Server 保留操作与 Windows Worker Git 执行。[Worktree 契约](docs/windows-worktrees.zh-CN.md) 拥有来源选择、默认值、确认、部分结果与恢复规则。这些命令不激活 Site。Create／Open 接受可选 `--focus` 与精确 `--client`；默认 `--no-focus` 保持视图。省略来源时 List／Create／Open 跟随经校验的调用者或所选 TUI 上下文。

## Windows Workspace 组关闭

`project close-all <target> --confirm` 关闭该 Project 中捕获的 Workspace，保留 Project 记录和 Git checkout。普通 `workspace close` 始终只关闭指定 Workspace。成员消失后使用 `project close-status --request <operation-id>`；只有明确可恢复失败才使用 `project close-resume --request <operation-id> --generation <attempt> --confirm`。Project 关闭支持分离/有界等待，报告每个子 Close 的身份及已观测状态。参见[Project 全部关闭契约](docs/windows-worktrees.zh-CN.md#project-全部关闭)。

Windows CLI 的 Panel／Tab／Workspace Close 会捕获并结束全部已绑定的主、子 Presentation，也支持分组／固定 Tab。只有全部目标的处置与清理完成，并重新验证完整成员关系后，才移除布局；未知效果继续保留原回执与恢复约束。未绑定 Presentation、子终端导航和其他结构操作仍按各自契约处理，见 [Close 模型](../orchbee-server/docs/terminal-tab-close.zh-CN.md#完整-presentation-成员关系)。

## Windows Client 导航

[Windows Client 视图控制](docs/windows-client-control.zh-CN.md) 拥有 `client list/get/control-status`、`workspace focus`、`tab focus`、`panel focus` 和 `panel zoom`。导航通过 `--client` 或无歧义且已校验的调用者上下文定位一个 current TUI。只提交一次，并等待绘制确认；超时或 Ctrl+C 仅停止等待。单 Panel 缩放有明确无操作回执。目录 v68 包含 76 个 schema，新增均为 v1 的 `orchbee.client.view`、`orchbee.client.control` 和 `orchbee.cli.failure.client-control`。未增加操作系统窗口激活或 Mac/Linux 控制运行时。

Windows 进程测试在 Git for Windows Bash 中执行输出的补全，通过 `git --exec-path` 定位；该见证要求已安装此 Bash。检查命令动词前后的互斥选项，不连接 Site。PowerShell 另有真实 Shell 语法见证。

## Windows schema 导出

`api schema [<schema-id>] --output <path>` 离线导出内嵌 JSON 文档，同时将其输出到 stdout。目标父目录必须存在；完整的同目录文件替换已有普通目标文件。[导出契约](docs/native-cli-query.zh-CN.md#windows-离线-schema-导出) 拥有失败阶段、临时文件清理以及发布后的 stdout 失败。目录 v68 包含 76 份文档，包括 `orchbee.cli.failure.schema-export` v1。

## Windows Panel 布局查询

`panel layout`、`panel neighbor --direction left|right|up|down` 和 `panel edges` 检查精确的 Windows TUI Client。它们接受位置参数 Panel、`--panel` 或 `--current`，路由有歧义时使用 `--client`。读取保持焦点和终端尺寸，支持非活动或未访问目标。`orchbee.panel.layout` v1 保留单元格几何、可为 null 的隐藏矩形、未缩放邻居、物理边缘结果及操作回执。参见[几何契约](docs/windows-client-layout.zh-CN.md)。目录 v68 包含 76 份文档；原生命令清单包含 88 条路径。

`panel focus [source] --direction left|right|up|down` 在指定 TUI 的未缩放单元格布局中查找一个邻居，保留缩放；无邻居返回 `direction-unchanged`，不切换到来源 Tab。找到邻居后固定目标，仅在绘制成功后返回 `direction-focused`。回执查询不重复移动。

`panel resize [panel] --direction left|right|up|down [--amount <number>]` 调整一个符合条件的分割边界，保留焦点和缩放。`layout set-split-ratio [--tab <tab> | --panel <panel> | --current] --path root|0.1 --ratio <number>` 设置精确二进制分割路径。两者都路由到一个 Windows TUI，并保留 `layout-adjusted` 回执。当前标签页的变化等待绘制，非活动标签页只保存、不导航。有限比例限制为 0.1–0.9；有限 resize 数量转为绝对值且最多 0.5，默认 0.05。Server 将比例提交到共享 Workbench 状态及 checkpoint v6；所有 Client 按各自视口尺寸投影。Client 焦点、zoom 与控制权仍独立。参见[布局契约](docs/windows-client-layout.zh-CN.md)。

## 共享窗格标题

`panel rename <panel-id> --label <text>` 设置共享主 Presentation 的自定义标题；`--clear`（或空白文本）恢复创建时的标签。接受精确 ID 或 `@panel:<suffix>`，不改变焦点或终端。[窗格标题契约](docs/native-panel-labels.zh-CN.md) 负责规范化、修订与重放规则以及匹配 cohort 的兼容边界。`orchbee.panel.rename` v1 区分 `renamed` 与 `unchanged`，可空 `customLabel` 回显规范化后的请求元数据。`panel get` 与 `panel current` v3 提供不变的 `label`、可空 `customLabel` 及实际显示的 `displayLabel`。`api snapshot` v4 保留规范的可选自定义标题。目录 v68 包含 76 份 schema，原生命令清单包含 88 条路径。

Panel swap 使用共享结构 placement 权威，在同一 Tab 中原子交换完整叶子，保留 Split／slot 及终端身份。Windows CLI 通过原 Client 单独确认源聚焦；后续失败或聚焦未知不抹去已知 canonical 提交。[原生 Panel 交换](docs/native-panel-swap.zh-CN.md) 拥有工作流程、回执及证据。

Windows `tab create` 和 `panel split` 在创建前解析 `--cwd follow|home|current|workspace|<absolute-path>`；默认 follow 采样已解析来源的根进程目录，v3 回执记录实际目录来源／路径。它们接受可选目标及标签，默认不聚焦；`--focus` 请求创建后聚焦，分屏可带共享 `--ratio`。显式目标无需 TUI 即可创建。`orchbee.creation.result` v3 分别报告已提交创建与后续比例／焦点结果；后续失败不能当作未创建而重试。参见[原生终端创建](docs/native-terminal-creation.zh-CN.md)，其目标、总预算、并发保护和恢复规则为这些 Windows 命令的权威。

Windows `panel input (<panel>|--panel <panel>|--current) --right-click orchbee|panel` 设置共享右键策略，不移动焦点。`--current` 要求有效调用者元数据，显式目标绕过它。`panel split --right-click` 随创建提交新 Panel 策略。默认 `orchbee` 将右键保留给 Client；`panel` 允许无修饰键的右键在启用鼠标上报的终端内容内透传。已准入的手势在策略改变后仍保留匹配的松开事件。路由、排队输入复核及失败恢复见[Panel 输入契约](docs/native-panel-input.zh-CN.md)。Client 的[动作菜单](docs/native-action-menus.zh-CN.md)在面板内容区沿用此策略。`orchbee.panel.input` v1 返回 `configured` 或 `unchanged`；`panel list` v2 与 `panel get/current` v3 提供 `rightClick`，`tab get` v2 和 `api snapshot` v4 接受可选规范字段。目录 v68 含 76 个 schema，原生命令清单含 88 条路径。

Windows `tab move <target> --insert-index N` 按原顺序的零基插入边界移动完整 Tab；0 为首位，
Tab 总数为末尾，源边界及其后一边界无变化，越界拒绝。它与 `--direction` 互斥，不改变焦点或终端。
成功使用 `orchbee.tab.move` v1，运行时失败使用 `orchbee.cli.failure.tab-move` v1。目录 v68/76，
原方向封装不变。

## 自动 Tab 命名

[Tab 标签契约](docs/native-tab-labels.zh-CN.md) 拥有可选 `automaticLabel` 元数据。省略标签的新建 Tab 按当前上下文位置编号；显式数字名称及旧 Tab 保留固定名称。移动或关闭后 CLI 查询、标签选择器和 TUI 显示保持一致，重命名切换为固定名称。原生 `api snapshot` v4 暴露规范元数据；目录 v68 包含 76 份 schema。共享协议需要匹配 cohort。

## 按来源读取终端输出

Windows `terminal read` 和 `agent read` 接受 `--source visible|recent|recent-unwrapped|detection`。
recent 合并保留历史与实时屏幕；detection 读取最近的一个屏幕高度范围。
`--plain` 为管道直接导出文本，默认 recent，与 JSON 互斥。来源模式
`--lines` 允许零，钳制到 1000，recent 默认 80。它与既有 history/rows/unwrap
参数互斥，既有行为保持不变。[输出契约](docs/native-terminal-output.zh-CN.md)
拥有选择、精确样本验证及 `orchbee.terminal.output` v1；目录 v68 共 76 个 schema。

Windows `terminal wait-output` 接受相同的 `--source` 和 `--lines` 选择，也可用 `--forever`
明确取消总等待时限。任一参数都会启用来源模式（默认 recent），与 `--rows` 互斥，
并输出 `orchbee.terminal.wait-output` v3。不带 `--forever` 时默认仍为 30000 ms；
`--forever` 与 `--timeout` 互斥，保留每次 I/O 的五秒上限并支持 Ctrl+C。
recent 等待合并软换行；visible/detection 按物理行匹配。既有调用保留 v1/v2 输出。
现有输出也能满足等待，匹配成功不代表命令执行完成。

Windows `terminal read` 与 `agent read --ansi` 导出带样式快照，默认读取 recent 输出。
`--format json` 使用 `orchbee.terminal.output` v2，并带 `result.encoding: "ansi"`；文本/plain
读取保持 v1。ANSI 与 `--plain` 及旧 history/rows/unwrap 参数互斥。它保留带样式空格和
Unicode，只生成 SGR；`recent-unwrapped` 拼接已验证的软换行。detection 仍是纯文本。
它不重现原始 PTY 字节、超链接或图形。渲染请求与预备输出各自保持既有 8 MiB 上限；
渲染资源超限直接失败，不回退、不重试。参见[输出契约](docs/native-terminal-output.zh-CN.md)。

Windows `status server` 在 Client Session 准入前读取实际 Server 构建／协议／cohort 元数据；`status client` 离线检查当前程序。两者支持 `--format text|json`，不激活 Site。裸 `status` 仍查询 Workbench。[运行时状态契约](docs/native-runtime-status.zh-CN.md) 拥有五秒时限、未知元数据、兼容性、schema 和失败语义。发布偏好与安装由显式 Windows 更新命令拥有。

## Agent 导航

Windows `agent focus <target> [--client <id>]` 在一次客户端控制提交前，将名称、会话或窗格解析为唯一活动的主窗格。显式 `--session`、`--name` 和 `--panel` 使用现有解析器；`--request-id`、`--detach` 和默认 30000 ms 总期限沿用导航恢复契约。Starting 或 Unavailable 客户端在 Agent 查询前返回 `client-not-ready`；Busy 允许 Client 执行可取消模式的前置检查。[Agent 导航契约](docs/native-agent-navigation.zh-CN.md) 定义主呈现绑定、歧义、取消和保留操作查询。显式聚焦成功后确认所选 Tab 中当前已知的 Agent 版本，即使外层终端失焦。普通绘制仍要求宿主聚焦。`agent attention <target> [--client <id>]` 独立读取该 Client，不标记已读。

Windows 宿主焦点上报由[原生 Client 提示状态](docs/native-client-attention.zh-CN.md)管理；焦点报告属于 Client 元数据，不是终端文本。提示状态与 CLI 确认的集成仍在开发。

Windows `agent changes [--cursor <c>] [--limit <n>]` 返回有界 canonical 变更页或当前头部检查点。默认 limit 为 50（1–100），后续复用返回的 cursor。`reset-required` 与 `cursor-invalid` 要求显式重建基线。参见[查询契约](docs/native-cli-query.zh-CN.md#windows-agent-变更页)。Catalog v68 提供 `orchbee.agent.changes` v1，共 76 个 schema、88 条命令路径。

Windows 支持通过 `agent integration enable --provider cursor`、`agent integration enable --provider qodercli` 或 `agent integration enable --provider antigravity-cli` 管理 Cursor、Qoder 和 Antigravity 会话身份 Hook，保留既有 Hook 和其他配置值。

Windows `agent integration enable --provider mastracode` 安装会话、运行、工具、权限及子 Agent 观察，可修复不完整的自有 Hook 集合。

Windows `agent integration enable --provider droid` 管理生效用户配置中的会话身份 Hook，不遮蔽已有回退 Hook。

Windows `agent integration enable --provider kimi` 管理当前 Kimi 的 TOML 会话 hook；工作中／阻塞／空闲对齐仍未完成。

Windows `agent integration enable --provider letta` 安装静默 SessionStart hook，并按 Agent 区分默认对话。

Windows `agent integration enable --provider devin` 管理 APPDATA/devin/config.json 中的六组会话身份 hook。

Windows `agent integration enable --provider hermes` 发布两个自有 Python 插件入口，并通过提供方 CLI 启用；停用保留插件及其缓存。

Windows `workspace create` 可省略 Project、名称和目录；按来源终端／Workspace／主目录解析默认值，默认创建未分组 Workspace，可用 `--focus` 聚焦现有 TUI。所有 Windows 形式均启动首个终端；显式参数和不聚焦使用相同流程与回执。Workspace 与聚焦保留各自结果；可选 Project 与 Workspace 创建原子提交，不再存在单独的 Project 准备步骤。详见[Workspace 创建契约](docs/windows-workspace-creation.zh-CN.md)；当前命令与 schema 目录由 `orchbee api schema` 提供。

## Windows CLI 配置

共享应用与终端主题通过和 Desktop 相同的偏好权威应用于原生 TUI。旧 Windows 主题值作为迁移输入。参见[原生主题](docs/native-themes.zh-CN.md)。

`config check` 还校验 `APPDATA/Orchbee/config.toml`，可由绝对路径 `ORCHBEE_CONFIG_FILE` 覆盖。`[terminal] new_cwd` 设置创建 cwd 默认值；`[worktrees] directory` 选择检出根目录。`[session] resume_agents_on_restore` 控制下次 Site 启动时恢复 Agent，默认 true，精确 true/false 环境覆盖值优先。`config check` JSON v3 报告有效值和来源。显式命令参数覆盖有效配置。参见[配置与优先级](docs/windows-cli-configuration.zh-CN.md)。Windows CLI 每次调用读取；公共 TUI/CLI 激活在提交前读取启动策略。TUI 直接创建和 Server 热重载仍是独立能力。

## Windows 终端观察流

`terminal session observe <terminal> [--cols <n>] [--rows <n>] [--timeout <ms>]` 从一个已有绑定输出 NDJSON，不获取控制权，也不改变 Host 尺寸。[流契约](docs/windows-terminal-streams.zh-CN.md) 拥有完整安全 ANSI 帧、重置、先刷新后确认、期限和清理规则。默认视口为 120×40，默认不限时。两种输出格式均产生 `orchbee.terminal.stream` v2 记录。目录 v68 含 76 个 schema、90 条原生命令路径。机器控制现通过 `terminal session control` 接收 NDJSON 输入；交互式 `terminal attach <terminal>` 或 `agent attach --name <name>` 要求控制台 stdin/stdout，并采用实际控制台尺寸。二者均支持 `--takeover`、`--mouse on|off` 和可选总时限 `--timeout`。`Ctrl+B` 后按 `q` 脱离；连续两次前缀发送一个字面前缀。粘贴不会触发脱离。任一全局格式下均显示交互画面。不启动 Site、不改变 Workbench 焦点、不重连或重放输入。

## Workspace 元数据集成

[Workspace 元数据契约](docs/windows-workspace-metadata.zh-CN.md) 定义上报规范化、来源序号栅栏、TTL 及一致读取组合。其 Server 和共享通信所有者独立于权威拓扑及终端 OSC 元数据。`workspace report-metadata` 和 Workspace list/get 通过有界关联读取及未知结果回执消费该所有者；通用事件与可配置 TUI token 模板需要各自集成。

Windows `panel report-metadata` 支持来源拥有的展示字段、守卫、token 补丁、序号和 TTL，提供绑定校验及单次提交回执。见 [Panel 元数据契约](docs/windows-panel-metadata.zh-CN.md)。

Windows `panel report-agent`、`report-agent-session`、`clear-agent-authority` 和 `release-agent` 提交显式来源上报并返回相关处理结果。参见 [Agent 上报契约](docs/windows-panel-agent-reports.zh-CN.md)。

## Windows CLI 通知

`orchbee notification show "Build complete" --body "All checks passed" --position bottom-right --sound done` 向运行中 Site 上可接收的主 TUI Client 排入自定义通知。配置 `[ui.toast] delivery="orchbee"` 显示卡片，`terminal` 或 `system` 使用外部投递；默认 `off`。使用 `--format json` 获取封闭投递回执。详见[投递、边界与生命周期](docs/windows-notifications.zh-CN.md)；接收只表示入队，不确认显示或播放声音。

## Windows 外层终端标题

`orchbee terminal title set "Build workspace"` 向前台主 TUI 发布覆盖标题；`orchbee terminal title clear` 恢复在线 `ui.window_title` 模板（回退为 `Orchbee`）。两者接受 `--format json`，没有 Client 连接时仍保留发布状态。模板规则及实际宿主限制见[标题归属与恢复](docs/windows-terminal-title.zh-CN.md)。

Windows 的 `plugin link/unlink/enable/disable/list/config-dir` 和 `plugin action list` 离线管理用户共享目录。`plugin action invoke` 向已有 Site 提交具有归属的命令，`plugin log list` 查询保留结果。上下文、进程归属、有界输出、托管安装/卸载及剩余 hook/Pane 集成见 [Windows 插件](docs/windows-plugins.zh-CN.md)。启动回执确认受理，执行结果由日志提供。

## Windows Panel 图形命令

`panel graphics set|clear|info|stream` 支持封闭图片输入、文件/管道读取与有界原始帧流，通过现有 Workbench 归属提交一次。参见 [Windows Panel 图形 CLI](docs/windows-panel-graphics.zh-CN.md) 了解参数、未知回执与流清理。[原生 TUI 渲染器](docs/native-panel-graphics.zh-CN.md) 暂存完整图像并发布成功绘制事实；仍需最后的 Windows 组合验收。

Windows `config import-herdr --input-file SOURCE --output-file DEST [--dry-run] [--format text|json]` 将 herdr 支持的显式设置转换为新文件。保留已有文件；预览不访问目标，且只有 JSON 预览包含候选 TOML。用 `ORCHBEE_CONFIG_FILE` 为新 Client 选择候选文件，选择旧文件即可回退。参见[转换与恢复约定](docs/windows-cli-configuration.zh-CN.md#herdr-配置导入)。

## 原生设置

在 Windows 或 Linux TUI 中按 Ctrl+B F4 打开通用与启动配置；Windows 还通过导航提供设置入口。
所有通用偏好与 Desktop 使用同一设备记录，包括语言、应用与终端主题、选择、Agent 尾部展示、
通知、声音、滚动历史与启动配置。编辑在保存前保持草稿，重置需要确认，
来源过期或结果不明会明确显示且不重放。
参见[原生设置契约](docs/native-settings.zh-CN.md)与[共享清单](../../crates/workbench-protocol/docs/user-preferences.zh-CN.md)。
