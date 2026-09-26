# Agent 集成

## 所有权与默认行为

Desktop Main 拥有本地 Agent 集成检查以及唯一允许的本地配置 effect。Renderer 只通过
`agent-integration-management` port 接收 provider-neutral inventory 与 correlated result；不会收到
executable path、resource path、command output、filesystem capability 或 generic process
capability。

启动时只构造未检查 snapshot，并且不执行任何 provider command。识别到一次 launch 后，可以只检查对应
provider；在设置中选择**刷新**会检查全部十个 provider。除非用户在设置中显式切换对应 Agent 的
switch，否则 Orchbee 永远不会安装、开启、关闭、更新、复制或修复集成。Launch reminder 只打开设置，
自身永远不执行 mutation。Browser 与 Gateway composition 不会获得 management control。

## 本地集成 registry

Registry 是封闭且有序的：Claude Code、Codex、Grok、OpenCode、Pi、OMP、Gemini CLI、GitHub
Copilot CLI、Cline、Qwen Code。每个 entry 只投影 availability、management mode、integration state
与 bounded issue。

| Agent              | 只读检查                                                            | 显式开启/更新 effect                                                    | 显式关闭 effect                                   |
| ------------------ | ------------------------------------------------------------------- | ----------------------------------------------------------------------- | ------------------------------------------------- |
| Claude Code        | 准确 plugin 加 `plugin marketplace list --json` source/version 检查 | 迁移准确 legacy source，再从 stable Capsule 安装并开启                  | 关闭 current plugin；移除准确 legacy entry        |
| Codex              | 有界、准确读取 Codex-owned marketplace/plugin declaration           | 修复准确 legacy marketplace、重定向到 stable Capsule，再重装准确 plugin | 移除准确 plugin；同时移除准确 legacy source       |
| Grok               | 准确的 `plugin list --json` source/version 检查                     | 替换准确 legacy copy，或安装/开启 stable Capsule copy                   | 仅卸载 source-proven 的准确 plugin copy           |
| OpenCode           | 准确 provider-local bootstrap 与 legacy-symlink 检查                | 原子发布 guarded local module bootstrap                                 | 仅移除 byte-exact bootstrap 或准确 legacy symlink |
| Pi                 | 准确 provider-local bootstrap 与 legacy-symlink 检查                | 原子发布 guarded local CommonJS bootstrap                               | 仅移除 byte-exact bootstrap 或准确 legacy symlink |
| OMP                | 准确 provider-local bootstrap 与 legacy-symlink 检查                | 原子发布 guarded local CommonJS bootstrap                               | 仅移除 byte-exact bootstrap 或准确 legacy symlink |
| Gemini CLI         | 对 provider-local extension 执行有界 manifest 与 digest 检查        | 原子复制 self-contained guarded extension                               | 仅移除 digest-verified owned directory            |
| GitHub Copilot CLI | 准确 plugin 加 `plugins marketplace list --json` source 检查        | 重定向准确 legacy marketplace，再安装或开启准确 plugin                  | 关闭 current plugin；移除准确 legacy entry        |
| Cline              | 有界扫描 Cline 官方 local-install layout 与准确 Orchbee marker      | 安装 stable Capsule bundle；仅在用户请求后替换准确旧 contract           | 仅卸载准确的 current 或 older Orchbee package     |
| Qwen Code          | 对 provider-local extension 执行有界 manifest 与 digest 检查        | 原子复制 self-contained guarded extension                               | 仅移除 digest-verified owned directory            |

Versioned Capsule 位于 Desktop user data 下，不在 application bundle 与 provider cache 内。它只会在
显式 local enable/update 前立即 materialize；只要仍有任一 provider declaration 引用它，即使 application
已移除也必须保留。Provider-local entry 要么保持 self-contained，要么会在
provider code 报错前捕获 Capsule 缺失。每个持久化 command hook 都检查 runtime 与 script、重定向
stdout/stderr，并在 observation 不可用时以零退出码结束。Cline inspection 只信任其官方 local-install
layout 中带 Orchbee 准确 contract marker 的唯一 package。Orchbee 不执行后台更新。Windows 把所有
entry 投影为 `manual-only`；不会运行受管本地 mutation。

Claude 与 GitHub Copilot inspection 会把准确 plugin inventory 和准确 local marketplace declaration
结合起来。Grok 的 provider-owned installed copy 是 self-contained，但 inspection 仍会校验其记录的
source 与 payload version。准确的 Orchbee application-bundle source 或较旧 sibling Capsule 会成为
`update-required`；只有显式 Enable/Update 才能重定向或替换。任何使用其他 source 的同名 marketplace、
plugin 或 copy 都是 conflict，永远不会被修改。Grok disable 会移除 source-proven copy，避免其当前 CLI
inventory 通过 provider-specific enablement representation 掩盖仍已安装的 hook。

打包 adapter contract、compatibility floor、手动命令与 observation 隐私边界以
[adapter 指南](../resources/agent-observation-adapters/README.zh-CN.md)为准。

## 状态与 transaction

| 状态              | 含义                                                      |
| ----------------- | --------------------------------------------------------- |
| `enabled`         | 准确的 Orchbee 集成已存在且开启                           |
| `disabled`        | 准确的 Orchbee 集成已存在但关闭                           |
| `not-installed`   | 受管 entry 中没有准确的 Orchbee 集成                      |
| `update-required` | 准确旧版或 dependency 缺失的 Orchbee entry 需要经同意迁移 |
| `conflict`        | 不同文件或 source 占用了 Orchbee 拥有的名称或 target      |
| `unknown`         | 检查无法建立准确状态                                      |

一个 process-wide controller 串行执行 mutation。显式请求按以下顺序运行：

1. 初始化或复用最新的只读 inventory。
2. 在写入前拒绝不可用的开启请求、manual-only management、conflict 与并发操作。
3. Preflight 并原子发布 Capsule `0.4.15`，不修改 provider configuration。
4. 重新检查 provider-owned state，只执行固定 command vector 或准确 guarded-entry operation；只有
   准确 legacy entry 可以迁移。
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

Provider-local bootstrap installation 会先 stage 再发布。Module bootstrap 会捕获缺失或无效的 Capsule
import，并且没有任何输出；extension directory 包含 guarded runtime 与记录每个 file digest 的 owned
manifest。Regular file、directory、foreign link、被修改的 bootstrap、额外文件或 digest mismatch 都是
conflict，并保持不变。准确的旧 application-bundle 或 stable-Capsule symlink 是 `update-required`；只有
显式 switch action 才会迁移。关闭只移除 byte-exact bootstrap、digest-verified directory 或已识别的
准确 legacy symlink。

任何 managed provider entry 都不会持久指向 application-bundle path。已识别的准确 legacy bundle path
绝不会被静默跟随或修复：inspection 会报告 `update-required`，只有 Settings 中的显式操作才能迁移。
因此，移除 application 但保留已启用 integration 时，留下的会是 self-contained provider entry、静默的
guarded bootstrap，或被有意保留的 Capsule。Uninstall tooling 不得删除仍被引用的 Capsule；完整清理
必须先 disable integration。

## 设置与提醒策略

设置使用由 snapshot 控制的 switch。一次 switch 变更就是用户的显式请求，但它的位置会保持最近一次
检查到的状态，直到 Main 发布 effect 后的重新检查结果；它绝不乐观跳动。Pending work 会标记受影响
row，并禁用所有竞争 switch。界面还会展示十个本地 entry、刷新、bounded failure、不会自动安装的说明
以及远端 manual-only 边界。用户请求关闭时，会先记录该 Agent 的 device-local reminder dismissal，再
尝试 effect。经过验证的成功开启会清除 dismissal，使之后发生的外部移除仍可再次被发现。

本地 Zsh、Bash 与 Fish resource 只会在第一个 command word 表示受支持 Agent 且解析为外部 executable
时发出 version `3` launch hint。Hint 只含封闭的 integration ID 与 protocol version；command text 与
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

手动完成远端设置后，v3 reverse Unix socket 与 private APC correlation transport 可以让 observation
穿过 SSH 与 nested tmux/Zellij layer。Adapter 会解析 outer client TTY 并直接写入 private frame，因此
不依赖 mux passthrough。Transport 可达性不会授予 integration management 权限。

## 失败与恢复

`agent-unavailable`、`busy`、`conflict`、`control-unavailable`、`manual-action-required`、
`operation-failed` 与 `verification-failed` 是完整的 renderer-visible failure set。Effect 失败或结果不明
后会执行只读 reconciliation；设置展示该 observation，并要求用户再次显式操作。Conflict 需要手动
review，永远不会被覆盖。移除 Orchbee 准确的 managed entry 即为 rollback。

## 验证

[`AgentIntegrationLaunchReminder`](../formal/tla/AgentIntegrationLaunchReminder.tla) 使用两个有代表性的
integration ID，并把 launch publication 携带的已检查 eligibility 作为稳定的有界输入。模型中的首次观察
顺序是 renderer delivery 顺序：provider-scoped inspection 的完成顺序可能不同于 Main 的原始 launch
观察顺序，而 management response 可能合并 Main 记录的顺序。模型检查去重、delivery merge、包含
`update-required` 的 eligibility、每个 application session 最多一次 offer、dismissal，以及携带已 offer
integration ID 的只读 Settings callback。并发 launch publication 与 management response 之间的
item/revision reconciliation 不在此有界模型内。

[`DesktopAgentIntegrationManagement`](../formal/tla/DesktopAgentIntegrationManagement.tla) 使用两个有代表性的
provider，并从生产中的全 `unknown`、未检查 snapshot 开始。模型检查显式 Refresh、每个 provider 独立的
inspection failure projection、一个已准入 operation、与该准入绑定的 authorization、接受 `disabled` 或
`not-installed` 的 disable verification，以及每次 effect 尝试后的 reconciliation，包括失败和结果不明的
outcome。`actual` 变量表示 adapter preflight/effect 时稳定的 provider state；snapshot staleness、并发
third-party edit、same-state no-op response、static availability 与 `manual-only` 准入细节是显式排除项。
Launch-scoped inspection 与 Settings work 的调度竞争是显式的跨模型排除项。

从仓库根目录运行：

```sh
node scripts/tla/run-models.mjs --models desktop-agent-integration-management,agent-integration-launch-reminder
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
