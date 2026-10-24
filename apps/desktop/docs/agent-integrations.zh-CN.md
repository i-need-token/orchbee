# Agent 集成

## 所有权与默认行为

Desktop Main 拥有本地 Agent 集成检查以及唯一允许的本地配置 effect。Renderer 只通过
`agent-integration-management` port 接收 provider-neutral inventory 与 correlated result；不会收到
executable path、resource path、command output、filesystem capability 或 generic process
capability。

启动时只构造未检查 snapshot，并且不执行任何 provider command。识别到一次 launch 后，可以只检查对应
provider；在设置中选择**刷新**会检查全部十个 provider。除非用户在设置中显式切换对应 Agent 的
switch，否则 Orchbee 永远不会安装、开启、关闭、更新、复制或修复集成。Launch reminder 只打开设置，
自身永远不执行 mutation。Browser Preview 不会获得 management control。

## 本地集成 registry

Registry 是封闭且有序的：Claude Code、Codex、Grok、OpenCode、Pi、OMP、Gemini CLI、GitHub
Copilot CLI、Cline、Qwen Code。每个 entry 只投影 availability、management mode、integration state
与 bounded issue。

| Agent              | 只读检查                                      | 显式开启/更新 effect                              | 显式关闭 effect                     |
| ------------------ | --------------------------------------------- | ------------------------------------------------- | ----------------------------------- |
| Claude Code        | 准确插件与本地 marketplace 内容身份           | 修复过期托管来源，再安装并启用当前 Capsule        | 关闭当前插件；移除过期托管入口      |
| Codex              | 有界读取 Codex 拥有的 marketplace/plugin 声明 | 重定向过期托管 marketplace，再重装准确插件        | 移除插件与过期托管 marketplace 声明 |
| Grok               | 准确插件来源身份与启用状态                    | 替换过期托管副本，或启用当前副本                  | 仅卸载来源已证实的插件              |
| OpenCode           | 准确的 provider-local bootstrap               | 原子发布受保护模块引导                            | 仅移除字节完全匹配的自有引导        |
| Pi                 | 准确的 provider-local bootstrap               | 原子发布受保护 CommonJS 引导                      | 仅移除字节完全匹配的自有引导        |
| OMP                | 准确的 provider-local bootstrap               | 原子发布受保护 CommonJS 引导                      | 仅移除字节完全匹配的自有引导        |
| Gemini CLI         | 有界自有标记与文件摘要                        | 原子复制自包含受保护扩展                          | 仅移除摘要校验后的自有目录          |
| GitHub Copilot CLI | 准确插件与本地 marketplace 来源身份           | 重定向过期托管 marketplace，再安装或启用插件      | 关闭当前插件；移除过期托管入口      |
| Cline              | 有界官方安装布局与 Orchbee 标记               | 安装当前 Capsule bundle，或显式替换带标记的过期包 | 仅卸载带 Orchbee 标记的包           |
| Qwen Code          | 有界自有标记与文件摘要                        | 原子复制自包含受保护扩展                          | 仅移除摘要校验后的自有目录          |

内容寻址的 Capsule 位于 `~/.orchbee/agent-observation-adapters/<payloadSha256>`，
不在应用 bundle 与提供方缓存中。仅在显式 enable/update 前生成，引用仍存在时保留，
包括移除应用之后。严格的契约 1 清单及实际载荷摘要必须与打包来源一致。
同一托管安装父目录下的其他身份报告 `update-required`；包 semver 不用于排序身份。
不再将应用 bundle 和检出资源根识别为托管安装。符号链接、外来或修改过的入口继续报告冲突。
不执行自动迁移或目录清理。

只有契约 1 是当前契约。Guarded 或 Cline 的其他正整数契约在 Orchbee 归属标记验证后
报告 `update-required`；受保护目录还必须逐一核对实际文件指纹。数字大小不赋予特殊状态。
重装或移除仍需显式操作；符号链接形式的 Cline 包或清单不能证明归属。

Provider-local 入口保持自包含，或在提供方代码报错前捕获 Capsule 缺失。
持久化 hook 保留运行时／脚本保护及有界输出策略。Cline 信任官方本地布局中唯一且
带准确 Orchbee 标记的安装。Desktop 不执行后台更新；其 Windows 注册表仍为 `manual-only`。

打包 adapter contract、compatibility floor、手动命令与 observation 隐私边界以
[adapter 指南](../resources/agent-observation-adapters/README.zh-CN.md)为准。

## 状态与 transaction

| 状态              | 含义                                                     |
| ----------------- | -------------------------------------------------------- |
| `enabled`         | 准确的 Orchbee 集成已存在且开启                          |
| `disabled`        | 准确的 Orchbee 集成已存在但关闭                          |
| `not-installed`   | 受管 entry 中没有准确的 Orchbee 集成                     |
| `update-required` | 内容身份过期、非当前自有契约或依赖缺失的入口需要显式修复 |
| `conflict`        | 不同文件或 source 占用了 Orchbee 拥有的名称或 target     |
| `unknown`         | 检查无法建立准确状态                                     |

一个 process-wide controller 串行执行 mutation。显式请求按以下顺序运行：

1. 初始化或复用最新的只读 inventory。
2. 在写入前拒绝不可用的开启请求、manual-only management、conflict 与并发操作。
3. Preflight 并原子发布当前内容寻址的 Capsule，不修改 provider configuration。
4. 重新检查 provider-owned state，只执行固定 command vector 或准确 guarded-entry operation；只有
   已证实的托管入口可以修复。
5. 每次 effect 尝试后重新检查所选 adapter。
6. 发布观察到的状态。Command 成功但未观察到请求状态时返回 `verification-failed`；失败绝不会伪造
   成功状态。

### Executable discovery

通过 Finder 启动的 macOS 应用以及类似的 POSIX 桌面启动可能只继承精简 `PATH`，与用户的
interactive terminal 不同。Desktop Main 拥有 process-local `AgentExecutableSearchPath`：继承的
`PATH` 是第一来源，一次成功或失败的 interactive-login-shell probe 是可选第二来源。Probe
是惰性的，由并发 adapter inspection 共享，并在 Desktop process 生命期内缓存。用户修改 shell
startup configuration 后，重启 Desktop 是恢复路径。

Probe 以 interactive login mode 运行配置的 absolute user shell，忽略 stdin、丢弃 stderr、把
stdout 限制在 256 KiB，并在三秒后超时。一个 marker-bounded helper 只输出最终的
`PATH`；Desktop 不导入其他 shell environment variable。因此，继承检测未命中时 shell startup file
可能运行一次，但 Agent inspection 或 mutation 都不使用 shell command。失败、格式错误、过大或超时的
probe 不提供 search path，并保持只读的 `not-found` 结果。

`resolveAgentExecutableCandidates` 是确定性 resolver。它只接受 registry 中有界的小写 command
name，保留 source 和 directory 顺序、移除重复项、拒绝空白、relative、含 control character 或过长的
entry，并把每个 source 限制在 256 个 directory。Platform adapter 接受第一个跟随后目标为
regular file 且可执行的 candidate。它保留选中的 symlink path，使 version-manager shim 继续保持调用
identity。Alias、function、current-directory lookup、递归扫描 home directory，以及固定的 package-manager
installation-directory inventory 都在范围外。

通过 shell source 发现的 command 只会把该已解析 `PATH` 叠加到继承的 command environment，从而保留
script interpreter lookup，但不采集 shell secret。实际 Agent command 使用选中的 absolute path 直接运行，
忽略 stdin、丢弃 stderr、把 stdout 限制在 256 KiB，并在三秒后超时。可能表示 interactive trust
或 review 步骤的超时会成为 `manual-action-required`。Path、environment value 或 command output 都不会
跨过 renderer boundary。

这里只采用 VS Code [`cd429513` 版本的 `shellEnv.ts`](https://github.com/microsoft/vscode/blob/cd429513258458bcbe37b17fe714874197fe2adf/src/vs/platform/shell/node/shellEnv.ts)
中“只探测一次并缓存 interactive-login-shell environment”的模式，参考日期为 2026-08-29。Orchbee 不声明
source compatibility，也不采用 VS Code 的完整 environment merge、shell catalog、configuration surface、
timeout、diagnostic 或 CLI-launch policy。确定性 candidate test 覆盖顺序与拒绝边界；command-runner test
覆盖精简继承环境、含噪声的并发 shell probe、缓存的失败或成功、executable validation，以及通过选中
shell `PATH` 完成的 interpreter lookup。Native macOS verification 覆盖 packaged-launch compatibility claim；
其他 shell family 仍以各自的 maintained platform test 为边界。

Provider-local bootstrap 安装先暂存再发布。模块引导静默捕获缺失或无效的 Capsule 导入。
扩展目录包含运行时，以及记录各文件摘要的自有清单。符号链接、外来根目录、修改过的引导、
额外文件和摘要失配均为冲突，保持不变。Disable 仅移除字节完全匹配的自有引导或已校验摘要的目录。
托管入口不会持久引用应用 bundle 路径。移除应用后保留被引用的 Capsule 或自包含提供方入口；
完整清理前先关闭集成。

## 设置与提醒策略

设置使用由 snapshot 控制的 switch。一次 switch 变更就是用户的显式请求，但它的位置会保持最近一次
检查到的状态，直到 Main 发布 effect 后的重新检查结果；它绝不乐观跳动。Pending work 会标记受影响
row，并禁用所有竞争 switch。界面还会展示十个本地 entry、刷新、bounded failure、不会自动安装的说明
以及远端 manual-only 边界。用户请求关闭时，会先记录该 Agent 的 device-local reminder dismissal，再
尝试 effect。经过验证的成功开启会清除 dismissal，使之后发生的外部移除仍可再次被发现。

本地 Zsh、Bash 与 Fish resource 只会在第一个 command word 表示受支持 Agent 且解析为外部 executable
时发出 version `1` launch hint。Hint 只含封闭的 integration ID 与 protocol version；command text 与
argument 永远不会进入 signal。Main 只把它当作不可信的 reminder gating evidence，而不是 Agent status
或 configuration authority，并按首次观察顺序对每个 ID 记录一次。Alias、function、wrapper、remote
shell，以及没有 Orchbee shell integration 的 terminal 都保持静默。

只有在观察到对应本地 launch，且 provider-scoped inspection 证明 Agent 在本地可用、其受管且无
conflict 的 integration 处于 `not-installed`、`disabled` 或 `update-required` state 时，renderer 才可在
每个 application session 的右下角显示一个紧凑且视觉低干扰的 reminder。候选按首次观察到的 eligible
launch 顺序选择。渲染不执行 mutation，也不请求操作系统通知权限。**打开集成设置**会隐藏当前 reminder
并直接导航到 Agent 集成；只有其中的 switch 才能修改 configuration。关闭 reminder 只在
`orchbee:web:agent-integration-reminders:v1` 中存储 dismissed integration ID；storage failure 仍会在
当前 session 保留内存中的 dismissal。

## Passive presence 边界

Launch hint 只能创建 presentation-scoped heuristic presence，不含 capability，Turn 为 unknown。它不能
断言 thinking、tool use、idle、attention、completion、success、failure 或 notification eligibility。
Evidence 携带 curated signature ID、source、generation、freshness 与 presentation identity；diagnostic
不暴露 command 或 screen content。Fresh evidence 在 30 秒后 stale，若未刷新则在 120 秒后 detach。
Retirement 只 detach presentation，永远不会伪造 semantic session end。Rich adapter 仍是 semantic
status 与准确 hidden-mux state 的唯一权威。

## 远端边界

Desktop 本地管理不会因为一个 SSH terminal 而获得权限。对于 Orchbee → SSH → remote tmux 或
Zellij → Agent，必须在 Agent process 实际运行的机器上手动 review 并开启 adapter。Orchbee 不会执行
远端 command、复制打包 resource、写远端文件、检查远端 home directory 或持久化远端 credential。

手动完成远端设置后，v1 reverse Unix socket 与 private APC correlation transport 可以让 observation
穿过 SSH 与 nested tmux/Zellij layer。Adapter 会解析 outer client TTY 并直接写入 private frame，因此
不依赖 mux passthrough。Transport 可达性不会授予 integration management 权限。

## 失败与恢复

`agent-unavailable`、`busy`、`conflict`、`control-unavailable`、`manual-action-required`、
`operation-failed` 与 `verification-failed` 是完整的 renderer-visible failure set。Effect 失败或结果不明
后会执行只读 reconciliation；设置展示该 observation，并要求用户再次显式操作。Conflict 需要手动
review，永远不会被覆盖。移除 Orchbee 准确的 managed entry 即为 rollback。

## 验证

从仓库根目录运行：

```sh
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/web test
```

共享观测边界还接受 Cursor、Qoder 和 Antigravity 的 session-identified 身份事件，以及旧版 Cursor 的 session-started。JSON 接入由原生 Windows 负责，Desktop 管理注册表保持独立。

观察注册表还准入 Mastracode 生命周期事实。Windows 原生管理负责其 Hook 配置；Desktop 管理保持独立。

共享观察边界准入 Droid 会话身份。配置管理由原生 Windows 负责；Desktop 管理保持独立。

共享观察边界接受 Kimi 会话身份和结束事件。原生 Windows 负责 TOML 接入；Desktop 管理保持独立。

共享观察边界接受 Letta 会话身份；原生 Windows 负责接入，Desktop 管理保持独立。

共享观察边界接收 Devin 会话身份；其配置管理由 Windows 原生 CLI 持有。

共享接收器接收 Hermes 会话身份；Windows 原生接入与 Desktop 管理仍相互独立。
