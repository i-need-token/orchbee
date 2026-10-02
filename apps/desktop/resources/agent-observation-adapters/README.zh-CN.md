# Agent observation adapter

该打包资源包含十八个需要显式安装、与 provider 绑定的 observation adapter，分别用于 Claude Code、
Codex、Grok、OpenCode、Kilo、Cursor、Qoder CLI、Antigravity CLI、Mastracode、Droid、Kimi、Letta、Devin、Hermes、Pi、OMP、Gemini CLI、GitHub Copilot CLI、Cline 与 Qwen Code。每个 adapter
校验自己的 upstream event contract，再通过 Orchbee 共享的本地 socket／管道加 private APC correlation
transport 发出同一组不含内容的 normalized fact。Provider identity、accepted event 与 maximum
capability 由 Desktop 的 closed registry 授予，而不是由 adapter payload 声明。

## Windows Codex 传输

Windows 原生 cohort 复用 v3 rich envelope／私有 APC 关联。Capsule `0.4.1`
新增 Codex `commandWindows` hook，使用 provider 既有的 `cmd.exe /C` 执行契约
和 `PLUGIN_ROOT`。Windows x64 cohort 必须包含固定 ConPTY 运行时，因为系统 ConHost 可能吞掉私有 APC。
本适配器变更不提供 Windows Unix 远程发现或其他 provider 安装能力。
原生托管 enable/disable 和 start/prompt 仍是独立 CLI 责任。

| 字段           | 契约                                                                                                                                                                                                                                                                                           |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题与归属     | Orchbee ConPTY 中的 Codex 命令 hook 必须到达原生 Server 的既有 observation 入口。共享适配器拥有投递；Server／Host 保留 envelope 验证、generation 身份和精确关联。                                                                                                                              |
| 输入与参考     | 允许的规范 Codex 事件、继承的 `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` 和 provider 提供的插件根。Codex 0.155.1 的 Windows hook 契约接受 `commandWindows`，标准流重定向后仍保留父进程 Console。环境字符串不授予 Agent 或终端身份。                                                                 |
| 端点策略       | 只接纳精确本地管道拼写 `\\.\pipe\orchbee-<32-lowercase-hex>-observation`。缺失、畸形、远程 UNC 或非 observation 目标均不投递。Windows 不扫描管道、文件或 Unix SSH 转发。注入端点和原生 Server ACL 提供路由；适配器不声称检查了管道所有者。                                                     |
| 关联目标       | 只使用字面设备 `\\.\CONOUT$`，以读写方式打开，并在写入前验证为字符设备。不使用相对 `CONOUT$`、环境指定设备、stdout 或 stderr。直接可见性保持 unknown。缺少 Console 时不发 hint，不得回退到文件。                                                                                               |
| 流程与转换     | 验证 observation → 解析精确端点和固定 Console 目标 → 发送有界无内容 envelope → 等待精确有界 ACK → 写入匹配私有 APC → 关闭 Console handle。缺少端点、无效输入、超时、ACK 被拒和打开／写入失败终止相应投递。仅 rich half 的事件按既有入口规则过期。                                              |
| 不变量与副作用 | prompt／输出／路径内容不进入 envelope；接受前不写 APC；任何单半消息不能绑定 Agent；hook 不安装配置或启动 Site。Windows 不 unlink 管道。缺少 Node／runtime 或传输失败时 provider hook 成功退出。Socket 与 Console handle 在本地拥有，并在完成／失败后关闭。                                     |
| 发布与恢复     | 发布新的不可变 Capsule 版本，同步资源和消费者版本。Git 在 Windows 检出中保留受摘要校验载荷的 LF 字节。显式托管操作替换之前保留旧 Capsule 和 provider 声明。Unix command 字段及既有 Unix 传输行为继续保留。适配器不引入依赖或 wire 版本；Windows cohort 投递另外需要固定的 Host ConPTY 运行时。 |
| 证据与限制     | 确定性准入／ACK 顺序用例、真实 Windows 命名管道 ACK 用例、静默有界命令 hook 进程用例，以及隔离原生 Server／ConPTY 投递见证。传输证据不证明托管安装、真实 Agent 任务完成、物理 TUI 交互或所有 provider 的 Windows 支持。仅同作者质疑复查；按用户指令继续暂停 TLA 执行。                         |

## 支持的 contract

| Adapter            | Reference version                       | Integration surface     | Maximum status evidence                                                                                    |
| ------------------ | --------------------------------------- | ----------------------- | ---------------------------------------------------------------------------------------------------------- |
| Claude Code        | 当前文档化 hook/plugin contract         | Command-hook plugin     | Session、Turn、tool、question、approval、compaction、subagent、completion、failure                         |
| Codex              | `0.150.1`（Unix）、`0.155.1`（Windows） | Command-hook plugin     | Session、Turn、tool、question、approval、compaction、subagent、completion                                  |
| Grok               | `1.0.5`                                 | Command-hook plugin     | Session、Turn、tool、approval、compaction、subagent、idle、completion、failure、cancellation、interruption |
| OpenCode           | `1.1.25`, `1.18.30`                     | JavaScript plugin event | Session、busy/retry/idle、tool、question、approval、error                                                  |
| Kilo               | `cb67014f`                              | JavaScript 插件事件     | Session、busy/retry/idle/offline 问题、聊天、工具、问题、审批、错误                                        |
| Pi                 | `0.84.4`                                | Extension API           | Session、Agent start/settled、blocking UI prompt、tool、compaction                                         |
| OMP                | `18.0.10`                               | Extension API           | Session、Agent start/end、Ask tool、tool approval、tool、compaction、retry                                 |
| Gemini CLI         | `20f70757`                              | Command-hook extension  | Session、Agent start/end、tool、question、permission、compaction、completion                               |
| GitHub Copilot CLI | `1.0.81`                                | Command-hook plugin     | Session、Turn、实际 question/permission prompt、tool result、compaction、completion、不可恢复 failure      |
| Cline              | `3.0.60`                                | Plugin runtime API      | Run/Turn、tool、question、completion、failure、interruption                                                |
| Qwen Code          | `c3a4058a`                              | Command-hook extension  | Session、Turn、tool、question、permission、compaction、subagent、completion、failure                       |

这些是彼此独立的 compatibility contract。Pi 与 OMP 有意不共享 adapter；Gemini CLI 与 Qwen Code
同样保留独立 manifest 与 event interpretation。Matrix 以外的版本在 event fixture 与 capability grant
通过 review 前都不受支持。

## 显式设置

### Execution host

应在 Agent process 实际运行的每台机器上安装所选 adapter。对于 Orchbee → SSH → remote
tmux/Zellij → Agent，adapter 因此安装在 remote host；local shell integration 与 OpenSSH reverse
Unix-socket forward 只提供 transport。Orchbee 永远不会自动安装或更改 adapter。在本地 Desktop
device 上，用户可以通过 Agent 集成设置显式开启或关闭 Orchbee 的准确 entry。Orchbee 永远不会把
该资源复制到另一台 host、修改远端 Agent 配置或运行远端设置；远端安装始终保持手动。

下面用 `<adapter-root>` 表示该目录一份独立、耐久副本的 absolute path。不要把 application bundle
内部路径持久写进 Agent 配置：删除或替换 Orchbee application 不得破坏 Agent 启动。本地 Settings 会先
在用户 application-data directory 发布经过校验、带版本的 Capsule，再让 managed CLI integration
指向这个耐久副本。Provider-local module entry 会捕获 Capsule 缺失；Shell command hook 会先检查 runtime
是否仍存在；Gemini/Qwen entry 则是 self-contained copy。Uninstaller 必须保留仍被任意 provider
declaration 引用的 Capsule；对应 provider entry 只由显式 disable 移除。

本地 Claude 与 Copilot inspection 会同时验证准确 plugin 及其 marketplace source；Grok inspection
会验证 copied plugin 记录的 source 与 version。准确的旧 Orchbee application-bundle path 或 sibling
Capsule version 是 migration evidence，而不是已开启的 current adapter；只有显式 Settings action 才能
修改它。任何使用不同 source 的同名 entry 都保持为不受改动的 conflict。

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

接受安装前请 review plugin。Disable 或 uninstall `orchbee-agent-observation` 即可回滚。本地 Settings
关闭 switch 时，只会卸载 source-proven 的 Orchbee copy。

### OpenCode

把 absolute entry path
`<adapter-root>/bundle/opencode/orchbee-agent-observation.mjs` 加为 local plugin。Entry 会导入相邻的
shared transport，因此应保留这份独立、耐久的完整 root；移除 configured path 即可回滚。本地 Settings
会改为写入 provider-local guarded bootstrap；以后即使 Capsule 不可用，该 bootstrap 也会捕获 import
failure，并且不返回 hook 或输出。

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

### Gemini CLI

手动设置时，确认 target 尚不存在，再复制一份 self-contained extension：

```sh
mkdir -p ~/.gemini/extensions/orchbee-agent-observation
cp -R <adapter-root>/bundle/gemini/. ~/.gemini/extensions/orchbee-agent-observation/
cp -R <adapter-root>/bundle/scripts ~/.gemini/extensions/orchbee-agent-observation/scripts
```

回滚时只移除这份准确 copied extension。本地 Settings 会原子发布并 fingerprint 同一套有界、
self-contained layout；modified、expanded 或 unmarked directory 会作为 conflict 保持不动。

### GitHub Copilot CLI

```sh
copilot plugins marketplace add <adapter-root>
copilot plugins install orchbee-agent-observation@orchbee-local
```

使用 `copilot plugins disable orchbee-agent-observation@orchbee-local --plugin` 关闭，并用对应的
`enable` command 恢复。Orchbee 不注册 `PreToolUse` 或 `PermissionRequest`：这些 hook 在 Copilot
permission policy 得出结果前运行，因而不能证明实际发生了面向用户的等待。

### Cline

```sh
cline plugin install <adapter-root>/bundle --json
```

使用 `cline plugin uninstall orchbee-agent-observation --json` 回滚。只读检查被限定在 Cline 官方
local-install layout，并要求 Orchbee 准确 package marker；没有 marker 的同名 entry 属于 conflict，
而不是 Orchbee 拥有的 installation。

### Qwen Code

手动设置时，确认 target 尚不存在，再复制一份 self-contained extension：

```sh
mkdir -p ~/.qwen/extensions/orchbee-agent-observation
cp -R <adapter-root>/bundle/qwen-code/. ~/.qwen/extensions/orchbee-agent-observation/
cp -R <adapter-root>/bundle/scripts ~/.qwen/extensions/orchbee-agent-observation/scripts
```

回滚时只移除这份准确 copied extension。本地 Settings 会原子发布并 fingerprint 同一套有界、
self-contained layout；modified、expanded 或 unmarked directory 会作为 conflict 保持不动。

## 状态 fidelity

Adapter 只发出其 upstream surface 实际提供的 evidence。OpenCode、Pi 与 OMP 的 settlement 会成为
肯定的 `Ready`，但不会伪造 successful outcome，因此不会发出 success notification。Copilot 只在
收到实际 `permission_prompt` 或 `elicitation_dialog` notification 时进入 attention。Qwen 只有在
`submitted_prompt` 证明 user provenance 时才启动 user Turn，其 continuation callback 会被忽略。Cline
plugin runtime 无法证明 session、approval 或 subagent fact，因此有意不声明这些 capability。有精确
error 时仍会保留 failure。Cline 会先记录 completion 再交付，并在对应 tombstone 仍被保留时，抑制该
准确 conversation 与 run 的后续 event。有界 cache 为最多 128 个 tracked conversation 各自保留最新
64 个 completed run。Run tombstone 按 completion 顺序进行 FIFO eviction；新的 completion 会先刷新其
conversation，然后淘汰最久没有 completion 的 conversation。两种 eviction 都允许较旧 identity 被再次
观察，而不会保留无界 plugin state。
`ClineObservationOrdering` 使用两个 conversation 与两个 run 的 retention bound 检查
completion-before-start、重复 completion、activity、session isolation 与有界 eviction。Grok
cancellation 用 `promptId` 限定 scope，避免延迟到达的 cancellation report 结束更新的 Turn。Event
缺失、hook 被禁用、版本不受支持、transport failure 与 SSH 完全断开都会降级为 stale 或 unknown，
而不是猜测 completion。

## 安全与失败边界

Desktop 只通过非持久化 terminal environment overlay 注入
`ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`。Transport 前，每个 entry 只选择有界的 event、session、Turn、
tool-operation 与 subagent identifier，并丢弃 prompt、question、tool input/output、transcript path、
working directory、error text、notification text 与未知字段。Rich half 发往同用户 private Unix-domain
socket 或 Windows 命名管道。Local process 只使用 environment 中的准确 endpoint；只有没有该 overlay 的 remote process
才发现 same-user forwarded candidate，并且 discovery 要求继承的 `SSH_CONNECTION`、`SSH_CLIENT` 或
`SSH_TTY` marker 至少一个非空。没有准确 overlay 的普通 local terminal 不执行 socket scan。只有
endpoint 返回准确、有界的 v3 acceptance ACK 后，adapter
才会把一份不含内容的 private APC correlation half 直接写入每个解析出的 outer TTY。没有准确 ACK 时，
adapter 不写 terminal control sequence。Desktop 要求对 live terminal generation 完成精确
event/correlation join，因此任一 half 都不能单独关联并授权 rich state；发送到另一个 live instance 的
unmatched envelope 会到期，不产生 canonical effect。
每个 Unix TTY candidate 都必须是 `/dev` 下经过词法规范化的路径；writer 以 nonblocking、禁止跟随
symlink 的方式打开它，验证它是 character device，并且除 kernel-owned `/dev/tty` alias 外要求
same-user ownership。Resolver 无法把 hint 重定向到 regular file 或其他用户的 terminal。

Unix resolver 支持 direct terminal、每个 attached tmux client、保守的 Zellij client visibility 与
nested mux environment。只有 attached mux client 正在显示另一个 pane 时，它才断言 `hidden`；direct
route 与 active-pane route 保持 `unknown`，因为 hook 无法证明 application-window visibility。
Workbench 会把这份 mux evidence 与自己当前的 Tab/Panel view 合并。显式 source 的 Orchbee shell
integration 可以增加随机 mode-`0600` remote Unix socket，再由 OpenSSH reverse-forward 到 Desktop。
这里没有 HTTP listener、TCP port、Bearer
token、继承的 terminal secret、自动 remote installation 或 durable remote cache。每个 adapter 都是
passive 的，不写 shell output，并会在配置缺失、input 无效、timeout、resolver failure 或 listener
failure 时 fail open。持久化 command hook 会先检查 Node 与可读 runtime；其中任一个消失时仍始终
成功退出。Orchbee 从不把 OSC 9、OSC 777、OSC 99、OSC 1337、BEL 或 provider notification hook
作为 correlation channel，因此 Warp 等无关 terminal 不会把 observation 变成用户可见的 notification
popup。

## 验证

在仓库根目录运行：

```sh
claude plugin validate apps/desktop/resources/agent-observation-adapters --strict
grok plugin validate apps/desktop/resources/agent-observation-adapters/bundle
pnpm --filter @orchbee/desktop test:unit
```

所属 runtime 与 transport contract 记录在 [Desktop 指南](../../README.zh-CN.md)中。

## Claude 直接执行

Capsule 0.4.2 将 Claude 的 `/bin/sh` hook 启动器替换为 `node` 加显式参数向量，遵循[上游 exec 契约](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form)。固定模块表达式通过 pathToFileURL 导入原样插件脚本路径，并捕获导入失败。脚本删除及输入格式错误仍静默处理；Node 本身必须存在于 provider PATH 中。Node 无法启动时，Claude 可能显示其非阻塞诊断。集成从不产生工具权限决定。

此共享声明让原生 Windows 配置不依赖 Shell。既有事件、hook 期限、输入界限、清洗及传输归属不变。[原生 Windows 契约](../../../orchbee/docs/windows-agent-integrations.zh-CN.md) 拥有安装及受控 Windows 证据；此变化不证明已认证 Agent 任务完成，也不增加 macOS/Linux 工作流支持。旧 capsule 保持不可变，显式修复安装 0.4.2。

## Copilot 直接执行

Capsule 0.4.16 使用基于 Node 的 [Copilot exec hook 契约](https://docs.github.com/en/copilot/reference/hooks-reference)。固定导入保护直接从 provider 子进程环境读取 COPILOT_PLUGIN_ROOT，检查其为绝对路径，并将拼接后的脚本路径传给 pathToFileURL。根目录或脚本缺失时不输出内容。Node 为前提；可执行文件不可用属于 provider 启动错误，不是已送达观察。PascalCase 事件名称保留文档所述的 VS Code 兼容输入字段，供既有清洗器使用。没有 Shell 解释路径。Windows 配置使用单独验证的当前插件命令／JSON 契约；共享载荷兼容不提升 Desktop CLI 对齐判定。

### Grok 命令引导

Capsule 0.4.16 保留 Grok 的纯命令 hook 契约，使用固定的 base64 编码 Node 引导。解码后的代码将 GROK_PLUGIN_ROOT 作为数据读取，设置原样脚本／provider 参数，并执行受保护的动态导入；不把用户路径插入 Shell 语法。PowerShell 5.1／7 在此命令中保留重定向的 JSON stdin。Node 是前提；Node 缺失是 provider 命令错误，根目录／脚本缺失则继续静默处理。Grok 既有 camelCase 事件输入、清洗器、时限和观察传输不变。编码用于保护 Shell 引号，不用于隐藏秘密。Windows 本地安装状态另从 provider 管理的 TOML 配置读取；不证明项目有效策略或活动会话已经重新加载。

## OpenCode 家族 Windows 引导安装

Capsule 0.4.16 新增 Kilo，并与 OpenCode 共用事件规范化器。共享工厂接受 sessionID 或旧版 info.id 生命周期身份，不一致时拒绝。回调参数与输出保持私有。发送按顺序保留最多 64 个待完成清洗事件；溢出/dispose 后停止接收并丢弃排队后缀，直到重新加载；正在发送的一项可以完成。传输失败静默处理。Windows 原生引导安装可发现的 .js 入口，保留外来/更新版本入口。路径、转移与证据见[归属 Windows 模型](../../../orchbee/docs/windows-agent-integrations.zh-CN.md#kilo-windows-集成与共享事件模型)。Desktop 引导安装保留独立提供方注册表。

## Gemini 与 Qwen 命令根目录

Capsule 0.4.16 使用提供方各 Hook 环境中的 ORCHBEE_AGENT_EXTENSION_ROOT 执行固定 Node 保护入口。Gemini 通过递归替换外部 Hook 文件提供该值；Qwen 通过递归替换 qwen-extension.json 中的内联 hooks 提供该值。替换作用于环境值，从不作用于命令源码。两个入口导入其自包含脚本，保留现有不携带内容的事件/传输边界。Windows 原生引导安装与 doctor 共用提供方特定目录覆盖规则，拒绝无效显式根目录。Windows 目录归属检查包含载荷版本方向，旧入口需要修复，新入口拒绝降级。参见 [Windows 扩展模型](../../../orchbee/docs/windows-agent-integrations.zh-CN.md#gemini-与-qwen-windows-扩展)。

Cursor 适配器仅从一致的 session/conversation 身份别名发出 session-identified，不推导工作、空闲状态或权限决定。原生 Windows JSON 接入安装固定编码启动器，不代表已执行认证后的 provider 任务。

Qoder CLI 对精确的 SessionStart/session_id 输入使用 qodercli 的仅识别身份观察。Windows 原生管理器安装编码命令组；需要 Node，Hook 输出保持空 JSON 对象。

Antigravity CLI 仅把 PreInvocation 中的 conversationId 准入为 session-identified；适配器 ID 为 antigravity-cli，规范 provider 为 antigravity。Windows 命名 Hook 返回空 JSON 对象，不声明生命周期或权限。

Mastracode 从十种选定 Hook 准入不含内容的运行、工具、批准及子 Agent 事实。只有 AgentEnd 决定终止结果，suspended 不表示完成。参见 [Windows 契约](../../../orchbee/docs/windows-agent-integrations.zh-CN.md#mastracode-windows-生命周期-hook)。

Droid 仅将 SessionStart/session_id 准入为 session-identified，包括恢复与压缩。Windows 管理器编辑活动配置来源；适配器不推断空闲或就绪。

Kimi 将精确 SessionStart/SessionEnd 中的有效 session_id 作为 session-identified/session-ended 准入。不会从可阻塞的 Stop hook 推断轮次完成。原生 Windows 负责两条 TOML 规则的安装。

Letta 接受精确 SessionStart event_type 和 conversation_id。默认对话要求 agent_id，身份为 default:<agent_id>；只观察会话身份。Windows 启动器不会输出提示词反馈。

Devin 接收六个精确 hook_event_name 值与有效 session_id，仅发出 session-identified。丢弃提示词、工具和权限内容；Stop 不证明完成。

Hermes 提供仅依赖标准库的 Python 插件，注册三个身份回调，并无 Shell 调用共享 Node 传输。排除非交互平台及回调内容，不返回上下文或权限决定。
