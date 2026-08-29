# Agent 集成

## 所有权与默认行为

Desktop Main 拥有本地 Agent 集成检查以及唯一允许的本地配置 effect。Renderer 只通过
`agent-integration-management` port 接收 provider-neutral inventory 与 correlated result；不会收到
executable path、resource path、command output、filesystem capability 或 generic process
capability。

启动和刷新都是只读的。除非用户在设置中切换对应 Agent 的集成 switch，否则 Orchbee 永远不会安装、
开启、关闭、更新、复制或修复 Agent 集成。Browser 与 Gateway composition 不会获得 management
control。

## 本地集成 registry

Registry 是封闭且有序的：Claude Code、Codex、Grok、OpenCode、Pi、OMP、Gemini CLI、GitHub
Copilot CLI、Cline、Qwen Code。每个 entry 只投影 availability、management mode、integration state
与 bounded issue。

| Agent              | 只读检查                                                       | 显式开启 effect                                                                            | 显式关闭 effect                                         |
| ------------------ | -------------------------------------------------------------- | ------------------------------------------------------------------------------------------ | ------------------------------------------------------- |
| Claude Code        | `claude plugin list --json`                                    | 校验或添加准确的本地 `orchbee-local` marketplace，再安装并开启 `orchbee-agent-observation` | 关闭 `orchbee-agent-observation@orchbee-local`          |
| Codex              | `codex plugin list --json`                                     | 校验或添加准确的本地 `orchbee-local` marketplace，再添加 `orchbee-agent-observation`       | 移除 `orchbee-agent-observation@orchbee-local`          |
| Grok               | `grok plugin list --json`                                      | 仅在用户打开 switch 后以 trust 安装 Orchbee 打包 bundle，或开启其准确 plugin               | 关闭准确 plugin `orchbee-agent-observation`             |
| OpenCode           | 准确 target path 与 symlink 检查                               | 添加准确 symlink `~/.config/opencode/plugins/orchbee-agent-observation.mjs`                | 仅当 symlink 仍指向打包文件时移除它                     |
| Pi                 | 准确 target path 与 symlink 检查                               | 添加准确 symlink `~/.pi/agent/extensions/orchbee-agent-observation.cjs`                    | 仅当 symlink 仍指向打包文件时移除它                     |
| OMP                | 准确 target path 与 symlink 检查                               | 添加准确 symlink `~/.omp/agent/extensions/orchbee-agent-observation.cjs`                   | 仅当 symlink 仍指向打包文件时移除它                     |
| Gemini CLI         | 准确 target path 与 symlink 检查                               | 添加准确 directory symlink `~/.gemini/extensions/orchbee-agent-observation`                | 仅当 symlink 仍指向打包目录时移除它                     |
| GitHub Copilot CLI | `copilot plugins list --kind plugin --json`                    | 校验或添加准确的本地 `orchbee-local` marketplace，再安装或开启 `orchbee-agent-observation` | 关闭 `orchbee-agent-observation@orchbee-local`          |
| Cline              | 有界扫描 Cline 官方 local-install layout 与准确 Orchbee marker | 对打包 bundle 运行 `cline plugin install`                                                  | 运行 `cline plugin uninstall orchbee-agent-observation` |
| Qwen Code          | 准确 target path 与 symlink 检查                               | 添加准确 directory symlink `~/.qwen/extensions/orchbee-agent-observation`                  | 仅当 symlink 仍指向打包目录时移除它                     |

Claude Code、Codex 与 GitHub Copilot CLI 关闭后会保留准确的本地 marketplace，因为其他显式安装的
Orchbee plugin 可能还会使用它。Grok 关闭后保留已安装 plugin。Cline 检查不会执行 mutation，也不会
信任任意同名 entry：只有位于其官方 local-install layout 下、带 Orchbee 准确 contract marker 的唯一
package 才是 enabled。Orchbee 不执行后台更新。Windows 把所有 entry 投影为 `manual-only`；不会运行
受管本地 mutation。

打包 adapter contract、compatibility floor、手动命令与 observation 隐私边界以
[adapter 指南](../resources/agent-observation-adapters/README.zh-CN.md)为准。

## 状态与 transaction

| 状态            | 含义                                                 |
| --------------- | ---------------------------------------------------- |
| `enabled`       | 准确的 Orchbee 集成已存在且开启                      |
| `disabled`      | 准确的 Orchbee 集成已存在但关闭                      |
| `not-installed` | 受管 entry 中没有准确的 Orchbee 集成                 |
| `conflict`      | 不同文件或 source 占用了 Orchbee 拥有的名称或 target |
| `unknown`       | 检查无法建立准确状态                                 |

一个 process-wide controller 串行执行 mutation。显式请求按以下顺序运行：

1. 初始化或复用最新的只读 inventory。
2. 在写入前拒绝不可用的开启请求、manual-only management、conflict 与并发操作。
3. 重新检查 provider-owned state，只执行 adapter 的固定 executable 与 argument vector，或准确
   symlink 操作。
4. 每次 effect 尝试后重新检查所选 adapter。
5. 发布观察到的状态。Command 成功但未观察到请求状态时返回 `verification-failed`；失败绝不会伪造
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

Symlink 开启只创建 parent directory 与准确的 file 或 directory link。Regular file、directory、损坏的
foreign link 或指向其他 source 的 link 都属于 conflict，并保持不变。关闭只会 unlink resolved source
与当前打包 adapter entry 完全一致的 link。

## 设置与提醒策略

设置使用由 snapshot 控制的 switch。一次 switch 变更就是用户的显式请求，但它的位置会保持最近一次
检查到的状态，直到 Main 发布 effect 后的重新检查结果；它绝不乐观跳动。Pending work 会标记受影响
row，并禁用所有竞争 switch。界面还会展示十个本地 entry、刷新、bounded failure、不会自动安装的说明
以及远端 manual-only 边界。用户请求关闭时，会先记录该 Agent 的 device-local reminder dismissal，再
尝试 effect。经过验证的成功开启会清除 dismissal，使之后发生的外部移除仍可再次被发现。

当 Agent 在本地可用，且其受管、无 conflict 的集成为 `not-installed` 或 `disabled` 时，renderer 每个
application session 最多在右下角显示一个紧凑且视觉低干扰的 reminder。Reminder 不执行 mutation，也不请求操作系统
通知权限。关闭 reminder 只在 `orchbee:web:agent-integration-reminders:v1` 中存储 dismissed integration
ID；**查看集成**会打开 Agent 集成设置 category。Storage failure 仍会在当前 session 保留内存中的
dismissal。

## 远端边界

Desktop 本地管理不会因为一个 SSH terminal 而获得权限。对于 Orchbee → SSH → remote tmux 或
Zellij → Agent，必须在 Agent process 实际运行的机器上手动 review 并开启 adapter。Orchbee 不会执行
远端 command、复制打包 resource、写远端文件、检查远端 home directory 或持久化远端 credential。

手动完成远端设置后，已有的 reverse Unix-socket 与 OSC 777 correlation transport 可以让 observation
穿过 SSH 与 nested mux。Transport 可达性不会授予 integration management 权限。

## 失败与恢复

`agent-unavailable`、`busy`、`conflict`、`control-unavailable`、`manual-action-required`、
`operation-failed` 与 `verification-failed` 是完整的 renderer-visible failure set。Effect 失败或结果不明
后会执行只读 reconciliation；设置展示该 observation，并要求用户再次显式操作。Conflict 需要手动
review，永远不会被覆盖。移除 Orchbee 准确的 managed entry 即为 rollback。

## 验证

从仓库根目录运行：

```sh
node scripts/tla/run-models.mjs --models desktop-agent-integration-management
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/web test
```
