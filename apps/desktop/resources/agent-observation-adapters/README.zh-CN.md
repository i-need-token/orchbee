# Agent observation adapter

该打包资源包含六个需要显式安装、与 provider 绑定的 observation adapter，分别用于 Claude Code、
Codex、Grok、OpenCode、Pi 与 OMP。每个 adapter 校验自己的 upstream event contract，再通过 Orchbee
共享的 Unix socket 加 OSC 777 correlation transport 发出同一组不含内容的 normalized fact。Provider
identity、accepted event 与 maximum capability 由 Desktop 的 closed registry 授予，而不是由 adapter
payload 声明。

## 支持的 contract

| Adapter     | Reference version               | Integration surface     | Maximum status evidence                                                                                    |
| ----------- | ------------------------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------- |
| Claude Code | 当前文档化 hook/plugin contract | Command-hook plugin     | Session、Turn、tool、question、approval、compaction、subagent、completion、failure                         |
| Codex       | `0.150.1`                       | Command-hook plugin     | Session、Turn、tool、question、approval、compaction、subagent、completion                                  |
| Grok        | `1.0.5`                         | Command-hook plugin     | Session、Turn、tool、approval、compaction、subagent、idle、completion、failure、cancellation、interruption |
| OpenCode    | `1.18.25`                       | JavaScript plugin event | Session、busy/retry/idle、tool、question、approval、error                                                  |
| Pi          | `0.84.4`                        | Extension API           | Session、Agent start/settled、blocking UI prompt、tool、compaction                                         |
| OMP         | `18.0.10`                       | Extension API           | Session、Agent start/end、Ask tool、tool approval、tool、compaction、retry                                 |

这些是彼此独立的 compatibility contract。Pi 与 OMP 有意不共享 adapter。Matrix 以外的版本在 event
fixture 与 capability grant 通过 review 前都不受支持。

## 显式设置

### Execution host

应在 Agent process 实际运行的每台机器上安装所选 adapter。对于 Orchbee → SSH → remote
tmux/Zellij → Agent，adapter 因此安装在 remote host；local shell integration 与 OpenSSH reverse
Unix-socket forward 只提供 transport。Orchbee 永远不会自动安装或更改 adapter。在本地 Desktop
device 上，用户可以通过 Agent 集成设置显式开启或关闭 Orchbee 的准确 entry。Orchbee 永远不会把
该资源复制到另一台 host、修改远端 Agent 配置或运行远端设置；远端安装始终保持手动。

下面用 `<adapter-root>` 表示该目录的 absolute path。打包后的 Orchbee 应用会提供同等的
`agent-observation-adapters` resource directory。

本地 Settings transaction、准确 managed entry、conflict handling 与 rollback behavior 由
[Agent 集成指南](../../docs/agent-integrations.zh-CN.md)定义。下面的 command 仍是手动设置方式，并且在
remote host 上始终必须手动执行。

### Claude Code

```sh
claude plugin marketplace add <adapter-root>
claude plugin install orchbee-agent-observation@orchbee-local
claude plugin enable orchbee-agent-observation@orchbee-local
```

如需回滚，请 disable 或 uninstall 该 fully qualified plugin。仅添加 marketplace 不会启用
observation。

### Codex

```sh
codex plugin marketplace add <adapter-root>
codex plugin add orchbee-agent-observation@orchbee-local
```

Codex 提示时需要 review 并信任 bundled hook。使用
`codex plugin remove orchbee-agent-observation@orchbee-local` 回滚。

### Grok

```sh
grok plugin install <adapter-root>/bundle
```

接受安装前请 review plugin。Disable 或 uninstall `orchbee-agent-observation` 即可回滚。

### OpenCode

把 absolute entry path
`<adapter-root>/bundle/opencode/orchbee-agent-observation.mjs` 加为 local plugin，或在
`~/.config/opencode/plugins/` 中放置指向它的 symlink。移除 configured path 或 symlink 即可回滚。
Entry 会导入相邻的 shared transport，因此应让完整 bundle 保留在原始路径。

### Pi

```sh
pi install <adapter-root>/bundle/pi/orchbee-agent-observation.cjs
```

对同一个 local source 使用 `pi remove` 即可回滚。所需的 `agent_settled` 与 `ui_prompt_*` event
使 Pi `0.84.4` 成为 compatibility floor。

### OMP

对于单次显式运行：

```sh
omp --extension <adapter-root>/bundle/omp/orchbee-agent-observation.cjs
```

如需持久 opt-in，把该 absolute entry 加入 OMP configured extension。移除 configured path 即可回滚。

## 状态 fidelity

Adapter 只发出其 upstream surface 实际提供的 evidence。OpenCode、Pi 与 OMP 的 settlement 会成为
肯定的 `Ready`，但不会伪造 successful outcome，因此不会发出 success notification。有精确 error
时仍会保留 failure。Grok cancellation 用 `promptId` 限定 scope，避免延迟到达的 cancellation report
结束更新的 Turn。Event 缺失、hook 被禁用、版本不受支持、transport failure 与 SSH 完全断开都会
降级为 stale 或 unknown，而不是猜测 completion。

## 安全与失败边界

Desktop 只通过非持久化 terminal environment overlay 注入
`ORCHBEE_AGENT_OBSERVATION_SOCKET`。Transport 前，每个 entry 只选择有界的 event、session、Turn、
tool-operation 与 subagent identifier，并丢弃 prompt、question、tool input/output、transcript path、
working directory、error text、notification text 与未知字段。Rich half 发往同用户 private Unix-domain
socket；另一个不含内容的 OSC 777 correlation half 会直接写入每个解析出的 outer TTY。Desktop
要求对 live terminal generation 完成精确 event/correlation join，因此任一 half 都不能单独关联并
授权 rich state。

Shared resolver 支持 direct terminal、每个 attached tmux client、保守的 Zellij client visibility 与
nested mux environment。显式 source 的 Orchbee shell integration 可以增加随机 mode-`0600` remote
Unix socket，再由 OpenSSH reverse-forward 到 Desktop。这里没有 HTTP listener、TCP port、Bearer
token、继承的 terminal secret、自动 remote installation 或 durable remote cache。每个 adapter 都是
passive 的，不写 shell output，并会在配置缺失、input 无效、timeout、resolver failure 或 listener
failure 时 fail open。

## 验证

在仓库根目录运行：

```sh
claude plugin validate apps/desktop/resources/agent-observation-adapters --strict
grok plugin validate apps/desktop/resources/agent-observation-adapters/bundle
pnpm --filter @orchbee/desktop test:unit
```

所属 runtime 与 transport contract 记录在 [Desktop 指南](../../README.zh-CN.md)中。
