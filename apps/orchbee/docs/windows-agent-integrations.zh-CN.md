# Windows Agent 集成

本文负责原生 `agent integration list`、`explain`、`enable`、`disable` 的 Windows
细化契约。CLI 负责本机接入，不启动 Session。共享的 provider 分类器与变更流程继续负责
归属、载荷身份、计划、回执和残留物。

## 模型与边界

| 字段       | 契约                                                                                                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | Windows 用户需要通过启动终端的同一个 CLI 安装、检查和移除观测集成。                                                                                                                                                        |
| 输入       | Provider ID、可选 capsule 来源、原生用户目录、PATH 和 Codex 配置根均是不可信路径。空 CODEX_HOME 使用用户目录默认值；非空无效覆盖值不得回退。                                                                               |
| 数据与归属 | Provider 拥有其配置。Orchbee 仅拥有内容寻址的 capsule 和明确标记的集成条目。外来、被修改或无法检查的条目保持不变。                                                                                                         |
| 输出       | 现有集成快照、计划、变更回执、类型化拒绝和保留残留路径。不能仅根据 provider 退出就虚构检查状态。                                                                                                                           |
| 流程       | 校验平台与预算 → 为 enable 准备 capsule → 检查 provider 归属 → 执行 provider 操作序列 → 报告观测或不确定性。取消停止后续副作用并终止自有命令树。                                                                           |
| 发布       | 在目标旁暂存；仅复制普通条目；校验 capsule；使用原子的不可覆盖 MoveFile 发布。目标被占用即冲突。备份使用相同原语。                                                                                                         |
| 路径       | 托管副作用只接受本机绝对盘符路径。拒绝重解析点和歧义组件。每次文件系统操作期间持有禁止删除共享的祖先目录句柄。                                                                                                             |
| 进程       | 有界解析 PATH 中的绝对目录。接受原生 EXE 及 Windows CMD/BAT 包装脚本；不隐式执行 PowerShell 脚本。参数保持结构化并使用 Rust 的批处理命令引用规则。命令以挂起状态启动，加入自有 Job Object 后才恢复。                       |
| 边界       | 保持现有 40 秒调用和 3.6 秒命令槽。执行上限为 3 秒，stdout 上限为 256 KiB，回收使用轮询而非无界等待。                                                                                                                      |
| 转移       | 准入前取消或超时不产生副作用。发布或启动 provider 后的失败保留尝试步骤和不确定性。重试不得替换外来数据。                                                                                                                   |
| 依赖       | 仅 Windows 使用 process-wrap 10.0.1 补充 std 缺少的安全挂起式 Job Object 分配；只开启 std、job-object、creation-flags。匹配的 windows 0.62.2 绑定提供创建标志类型。现有 local-ipc 负责非阻塞子进程输出。不增加异步运行时。 |
| 回滚       | Disable 使用现有归属校验。失败或中断的发布报告保留的暂存和备份路径；CLI 不扫描清理无关文件。                                                                                                                               |
| 证据       | Windows 文件系统测试覆盖不可覆盖、Unicode 路径和重解析点拒绝；进程测试覆盖参数保真、输出上限、取消、超时和后代；隔离的公开 CLI 测试覆盖 Codex 接入与配置隔离。                                                             |
| 限制       | Provider 认证、远端安装、付费 Agent 任务和 TUI 交互不在接入证据范围内。按用户要求不执行 TLA。审查为作者自身的对抗性复查，不是独立证据。                                                                                    |

原始路径继续受 4,096 UTF-8 字节的准入上限约束。完成准入和祖先句柄固定后，MoveFile 和私有 ReplaceFile 接收经词法规范化为扩展形式的普通盘符路径；已有逐字盘符路径保持原样。回执和恢复记录中的逻辑路径保留原始拼写。长路径文件系统证据覆盖普通、混合分隔符和逐字路径拼写下的发布冲突、元数据流及备份恢复。

打开的后代条目可能使原子目录 MoveFile 在发布前暂时返回 ACCESS_DENIED。仅此目录失败允许进行 1、5、10 毫秒的三次准入等待，期间始终固定同一组祖先句柄。每次尝试仍使用不可覆盖原语；新到达的目标返回冲突，持续拒绝返回失败。文件移动及其他错误不等待。受控原生句柄测试覆盖句柄释放、目标抢占、持续拒绝及源字节保持不变；提供方工作流本身不会重放。

Windows 托管 Claude Code、Codex、Grok 和 GitHub Copilot hooks、OpenCode、Kilo、Pi、OMP JavaScript 模块、Gemini/Qwen 扩展以及 Cursor JSON 会话 Hook。其他 provider
在其 Windows hook 执行路径获得支持前保持 `manual-only`。检查与变更共享平台准入规则。

OpenCode 使用 ESM 入口；Pi/OMP 安装的 CommonJS 引导模块动态导入 capsule 的 **ESM**
入口，再调用具名注册函数。若导入其 CommonJS 包装器并查找同名导出，会静默跳过注册。
模块细化在当前载荷 中保持既有引导行为。确定性的引导
文件字节与 file URL 仍为归属证明：被修改或外来的条目会被拒绝；同一受管父目录下的其他身份需要显式修复。
Windows doctor 从完整的 `bundle/<provider>/<entry>` 后缀推导 capsule 根目录，
健康的已安装模块因此对照其 capsule manifest 检查，不再误查 bundle 目录。
这是离线文件检查，不证明实时连接可用。

公开 CLI 验证在隔离的 Unicode 用户目录中安装、检查、移除四个模块。真实 Node 加载
每个已安装的引导模块，经私有 Windows 管道按顺序上报提供方事件；独立原生 ConPTY
验证检查控制台关联能否进入 canonical 会话。这些受控提供方 API 验证集成边界，
不证明提供方认证或任意版本的行为。

## 依赖审查

[process-wrap](https://docs.rs/process-wrap/10.0.1/process_wrap/) 使用 MIT OR Apache-2.0
许可证，无构建脚本，增加 indexmap 2.14.2 并复用已解析的 hashbrown 0.17.1。
Windows 绑定增加 windows-core、collections、future、numerics、result、strings、
threading 以及 implement/interface 过程宏。过程宏复用现有 syn/quote/proc-macro2 版本。
未增加重复包版本或异步运行时。Windows unsafe 调用留在依赖内部；自有 Rust 保持 `unsafe_code = "forbid"`。
适配器不调用其阻塞 wait 方法。每条退出路径都显式终止 Job，包括主进程正常完成；
仅丢弃 std 包装器不提供 kill-on-drop。操作系统以不可捕获方式强制终止 Orchbee
不在此协作式回收承诺内。

## 维护筛选

Windows `agent integration list --outdated-only` 只执行一次原有有界本地检查，仅显示 update-required 条目，包括需要修复的自有安装。它不安装、不下载。`agent integration enable --provider <id>` 仍是显式且受保护的更新／修复操作；冲突及无法安全检查的条目不会被替换。

筛选列表使用 v1，包含 outdatedOnly true、inspectedCount、表示检查未知的 unresolvedProviderIds，以及表示冲突条目的 blockedProviderIds。Complete 在筛选前根据全部所选检查计算；空列表不能隐藏检查失败。文本包含这些计数和相同筛选条目。默认列表使用 v1。当前 schema 接纳全部 20 个规范提供方。此参数不新增非 Windows 运行时；原有 40 秒期限、取消、用户目录隔离及仅本地归属保持不变。详细已安装版本报告及其余 provider 的 Windows hook 支持仍是独立义务。

## Claude exec hooks

Windows 也可使用 当前 capsule 托管 Claude Code。Hook 声明使用 provider 的 `command` 加 `args` exec 形式：Node 将插件脚本路径作为原样参数接收，包括空格、单引号、Unicode、美元符和反引号。固定动态导入保护会忽略已删除的脚本；既有有界清洗器及 Windows 管道／控制台传输仍是唯一观察路径。不需要 Bash 或 PowerShell 启动器。Node 必须保留在 provider PATH 中；缺失时 Claude 报告其非阻塞启动诊断，不产生观察。这替代原先仅 Claude 无 Node 时仍静默成功的承诺。其他 provider 保留各自启动契约。[上游 exec 契约](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form) 定义参数替换。

启用、禁用和修复复用既有用户作用域插件命令及有界 Windows 进程所有者。旧版自有 capsule 需要显式修复；当前已启用条目为空操作，外部或被修改的条目仍拒绝。隔离 provider 命令及已发布 hook 见证覆盖配置和清洗后回传，不证明已认证 Claude 任务完成。共享可移植 hook 声明会变化，但本工作不增加 macOS/Linux 工作流或运行时验证。

## Copilot 命令兼容

Windows 对齐已用官方 Copilot CLI 1.0.88 验证的插件契约：`plugin list --json` 返回平面数组，插件命令不再接受已移除的 `--kind` 或 `--plugin` 参数。单一固定命令所有者为检查、计划和执行提供参数。检查要求准确的 marketplace、本地安装来源、live 来源及提供方声明元数据；外部和格式错误的观察都不能授权修复。Unix adapter 保留原命令和解析契约，不推测性重试变更。

当前 Capsule 使用 Copilot 的 Node exec hook 形式。固定导入保护将 provider 的 COPILOT_PLUGIN_ROOT 环境变量作为路径读取，不进行 Shell 展开，并保留既有 PascalCase 事件输入契约。根目录或脚本缺失静默处理；Node 仍为前提。[插件参考](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference) 和 [hook 参考](https://docs.github.com/en/copilot/reference/hooks-reference) 拥有上游语义。既有本地变更期限、Job Object 回收、不可变发布、回执和无内容观察规则保持不变。受控安装和 hook 回传不证明已认证 Agent 任务完成。

提供方包版本仍是必需的清单元数据，不用于排序 capsule 身份。元数据缺失为无法检查；受管来源摘要不同需要修复。

## Windows TOML 和 Grok 本地状态

Windows Codex 的检查与修改共用一个经过校验的配置根：非空 `CODEX_HOME`，或在其缺失／为空时
使用 `USERPROFILE/.codex`。每条自有 Codex 命令都通过仅属于子进程的 `CODEX_HOME` 接收该根，
提供方的已知文件夹发现不能选择另一份用户配置。无效显式根拒绝且不回退。缺失的默认目录
在提供方命令前通过受保护祖先创建，记录为 `prepare-codex-home`，并在计划中作为 guarded write
呈现。现有根在整个命令序列中保持固定；显式非空根必须已存在。提供方命令成功后，
会对同一配置进行新的有界检查；未观察到安装／移除时返回 `verification-failed`，不报告完成。
隔离 CLI 测试覆盖默认与显式根、空覆盖值，以及退出成功但未产生效果的情况。

Windows Codex 和 Grok 检查在既有 512 KiB 读取上限内解析完整 UTF-8 TOML 文档。语法错误、重复赋值、无效 UTF-8 和不可读数据返回 unknown；合法引号键／点分键、内联表和行尾注释保留原有含义。解析不写配置，也不回显其内容。仍要求插件布尔值类型正确及本地来源所有权精确匹配。仅 Windows 使用 toml 1.1.6，禁用默认特性，只启用 std、parse 和 serde；不启用序列化／display、unsafe 解析加速或 SIMD。新增依赖树为 serde_spanned 1.1.1、toml_datetime 1.1.1、toml_parser 1.1.3 和 winnow 1.0.4，复用已有 serde_core 1.0.229。只读 toml 依赖不需要 display；下述 Kimi 编辑额外编译已锁定的 toml_writer 1.1.2。不新增构建脚本或过程宏；toml 系列许可为 MIT OR Apache-2.0，winnow 为 MIT。Winnow 内部包含 unsafe 切片操作；自有代码继续禁止 unsafe。既有 Unix 解析器不变。

Grok 的已安装 JSON 清单不报告启用状态。Windows 将精确归属的 bundle 来源身份与用户配置关联：非空 GROK_HOME 优先，否则使用 USERPROFILE/.grok。无效覆盖不回退。显式 disabled 条目优先；未列出的用户插件处于禁用状态。这是本地接入配置状态：项目／托管策略及已经运行的会话仍由 provider 管理。启用使用既有 install/trust 或 enable 命令；禁用仅移除来源已证实的插件。提供方包版本元数据不覆盖来源身份。

当前 Capsule 将固定 Node 引导编码到 Grok 的纯命令 hook 声明中。用户路径和 hook 输入均不成为 Shell 源码。Node 读取 GROK_PLUGIN_ROOT，并通过 pathToFileURL 导入拼接脚本；根目录／脚本缺失静默处理，Node 则是前提。既有输入、事件、传输及 hook 时限保持不变。真实 PowerShell 5.1／7 用例在隔离配置中执行交付命令和重定向 JSON stdin；不代表已认证 Grok 任务或新增 macOS／Linux 流程。

## OpenCode 自动发现

Windows 将受保护的 OpenCode 引导安装为 `plugins/orchbee-agent-observation.js`。Provider 的目录加载器扫描 .js／.ts，因此此前通过 Node 直接加载 .mjs 目标并不证明自动发现。Capsule 源码继续使用 .mjs。集成与 doctor 共用一个 Windows 路径所有者：非空且绝对的 XDG_CONFIG_HOME/opencode 优先，否则使用 USERPROFILE/.config/opencode。无效显式根目录不回退。

检查合并当前 .js 和废弃 .mjs 目标。外来、较新或不可读条目阻止变更；自有废弃条目要求修复。启用先发布并验证当前入口，再重新检查并移除废弃的自有入口。失败或取消保留已提交效果并报告回执，不以补偿为由删除可用的新入口。禁用对两个目标使用相同保护。Explain 从既有检查中报告受保护写入／移除，不再次读取。Doctor 检查可被发现的 .js 入口；仅有旧安装时在此报告缺失，集成查询则要求修复。既有上限及 no-replace 原语不变。隔离的官方 OpenCode 1.18.30 debug-config 用例确认自动发现；此证据不包括认证及付费 Agent 任务。

Windows 普通模块检查按解码后的绝对 file URL 验证完整固定引导，再分类精确的 capsule 根目录及载荷相对后缀。旧的自有模块可以修复；归属不明的入口拒绝替换；修改过的代码及外来根目录仍为冲突。16 KiB 读取器不导入引用模块。此规则适用于既有 OpenCode、Kilo、Pi 和 OMP 模块所有者；Unix 检查不变。

## Kilo Windows 集成与共享事件模型

Kilo 是 Windows 原生托管集成，沿用 OpenCode 的受保护模块归属、载荷身份、截止时间、取消和回执规则。可发现入口是非空绝对 KILO_CONFIG_DIR 下的 plugin/orchbee-agent-observation.js，否则使用 XDG_CONFIG_HOME/kilo，再否则使用 USERPROFILE/.config/kilo。显式根目录无效时拒绝且不回退；doctor 共用此解析。启用发布不可变 capsule 和精确标记的引导文件；禁用仅删除归属已证实的入口。不改变提供方命令、项目配置或认证。Kilo 在其他平台仍为手动管理。本地安装不会覆盖 KILO_PURE，也不代表已完成真实认证任务。

共享 OpenCode 家族规范化器接收不可信的事件/回调对象，仅返回规范观察身份和枚举。创建/删除事件接受 sessionID 或旧版 info.id；提供的身份必须合法且一致。其他事件必须提供 sessionID。当前状态对象映射 busy/retry/idle；Kilo 还接受旧版状态字符串，将 offline 请求映射为问题。聊天和工具回调从不检查或修改输出/参数。未知结构静默忽略。内容、路径、提示词、输出或权限决策均不跨过此边界。

每个插件实例拥有最多 64 个待完成规范观察的 FIFO。发送串行、尽力而为，受现有传输边界约束。溢出后停止接收直到重新加载，并丢弃排队后缀；正在发送的一项可以完成。dispose 使用相同停止规则。不在丢失未知生命周期证据后恢复发送，也不伪造完成。传输失败静默处理，不阻断后续已接收发送。确定性测试覆盖规范化、身份歧义、顺序、溢出、失败和释放；真实 Node 引导文件/Windows 私有管道见证覆盖安装到发送。这些受控见证不证明已完成认证提供方任务。

参考：[Kilo 插件 API](https://github.com/Kilo-Org/kilocode/blob/cb67014ffdb39ffe890e3a47a0a9c35c0e63538b/packages/plugin/src/index.ts)、[Kilo 全局路径](https://github.com/Kilo-Org/kilocode/blob/cb67014ffdb39ffe890e3a47a0a9c35c0e63538b/packages/core/src/global.ts)、[OpenCode 旧版生命周期](https://github.com/anomalyco/opencode/blob/v1.1.25/packages/opencode/src/session/index.ts)和 [OpenCode 当前生命周期](https://github.com/anomalyco/opencode/blob/v1.18.30/packages/schema/src/v1/session.ts)。当前 Capsule 携带共享模块；规范观察注册表新增 Kilo，Desktop 引导安装保留自身提供方边界。

## Gemini 与 Qwen Windows 扩展

Windows 使用现有受保护发布和删除流程管理自包含 Gemini、Qwen 扩展目录。Gemini 将非空绝对 GEMINI_CLI_HOME 解析为主目录父级，安装到其下 .gemini/extensions/orchbee-agent-observation；Qwen 将 QWEN_HOME 解析为配置根目录，安装到其下 extensions/orchbee-agent-observation。未设置覆盖值时分别使用 USERPROFILE/.gemini 和 USERPROFILE/.qwen。显式根目录无效时拒绝且不回退。Doctor 共用这些路径。全局/项目激活策略和已运行会话仍由提供方拥有；本地文件状态不证明实际送达。

证明目录标记和完整文件指纹后，Windows 还要求准确扩展名称和提供方声明的版本元数据。正整数的非当前自有契约需要重装；元数据缺失为 unknown，修改过或外来内容为 conflict。现有取消、备份恢复、已提交前缀回执和残余归属不变。启用将源码和共享脚本复制为一个自包含目录；禁用仅删除重新验证过归属的目录。

当前 Capsule 使用固定 Node 命令，读取提供方替换后的 ORCHBEE_AGENT_EXTENSION_ROOT 环境值。用户路径不进入 Shell 源码。Gemini 递归替换 hooks/hooks.json；Qwen 递归替换 qwen-extension.json 中的内联 hooks，因此 Qwen 不保留重复的外部 hooks 文件。保护入口导入拼接的 forward-hook.mjs，保留现有输入边界和事件语义。Node 是前提；根目录/脚本缺失或非法输入静默处理。源码契约见 [Gemini 扩展加载](https://github.com/google-gemini/gemini-cli/blob/20f707577424c0c60cde3470b1495f228773e307/packages/cli/src/config/extension-manager.ts) 与 [Qwen 扩展加载](https://github.com/QwenLM/qwen-code/blob/c3a4058a0c7207de7b421e9da0622a9ec890dbad/packages/core/src/extension/extensionManager.ts)。受控 PowerShell 5.1/7 和 cmd 见证使用原始 JSON stdin 和 Windows 私有观察管道执行交付命令；不代表已完成认证提供方任务。

Windows 暂存 Qwen 清单在写入指纹前为每个自有命令 Hook 明确选择 `shell: powershell`。上游默认 cmd 启动可能将完整 Node 表达式引用为不执行的字符串；提供方退出码 0 不能单独作为执行证据。Gemini 在 Windows 已选择 PowerShell。共享 capsule 保持可移植，仅原生 Windows Qwen 安装应用此投影。额外的直接 cmd 见证按原样传入命令源码，与上游默认 Shell 分派分别记录。

## Qoder Windows JSON Hook

Qoder 使用相同的 Windows 发布流程，但 JSON 形状不同。provider `qodercli` 解析可执行文件
`qoder` 和非空绝对路径 `QODER_CONFIG_DIR`，否则使用 `USERPROFILE/.qoder/settings.json`。
显式根目录无效时不回退。`SessionStart` 条目是含一个命令 Hook 的组；只拥有精确生成的组。
修改匹配条件、在组内添加 Hook、修改命令、重复或错放自有组均判为冲突。其他组、事件和原始设置
保留，包括不相关的 `version` 字段。Cursor 保持扁平的版本 1 Hook 格式。

固定编码启动器选择 `shell: powershell`，使用十秒的 provider 超时，仅把
`hook_event_name: SessionStart` 中有效的 `session_id` 转发为 `session-identified`，并返回
`{}`。路径始终作为编码数据。恢复或压缩可能重复上报身份，但不改变回合状态。Node 仍是前提。
本地产物归属不证明项目或组织策略实际生效、进程存活或已完成认证任务。测试覆盖公开安装、覆盖路径
解析、修复与拒绝、外来内容保留、doctor 一致性及安装命令在 PowerShell 5.1/7 和 cmd 中执行。
来源契约：[Qoder Hook](https://docs.qoder.com/cli/hooks)、
[配置路径](https://docs.qoder.com/cli/settings-reference)和
[原生 Windows 安装](https://docs.qoder.com/cli/installation)。

## Antigravity Windows 命名 Hook

集成 `antigravity-cli` 使用可执行文件 `agy` 和规范 Agent 身份 `antigravity`。
它编辑文档规定的 `USERPROFILE/.gemini/config/hooks.json`；这是与其他 Antigravity 产品形态共享的
全局定制位置。其他设置和插件位置仍由 provider 管理。

此集成仅拥有 `orchbee-agent-observation` 命名块，且必须通过精确固定启动器和唯一的
`PreInvocation` 处理器证明归属。其他命名块保留为不透明的原始 JSON；复制到其他名称下的命令不归
此集成拥有。可选布尔 `enabled` 默认为 true。显式 false 报告禁用；格式错误的标志或修改过的自有
内容拒绝变更。启用写入当前已启用块；禁用移除活动的自有块。已禁用块保持既有无操作行为。
来源归属保护仍阻止替换外来或被修改的条目。Doctor 报告禁用事实，并单独给出检查后的 capsule 根目录。

编码 PowerShell 命令返回 `{}`，只将有效的 `conversationId` 观察为 `session-identified`；
提供事件名称时必须为 `PreInvocation`。不转发转录路径、内容或权限，也不从重复模型调用推断空闲或
就绪。沿用有界 JSON 发布和元数据保留。核心用例与安装命令的 Windows 管道测试覆盖归属、禁用状态、
拒绝和身份投递；认证后的 Agent 执行仍单独验证。
来源：[Antigravity Hook](https://antigravity.google/docs/hooks?app=antigravity)和
[原生安装](https://antigravity.google/docs/cli/install/)。

## Cursor Windows JSON Hook

Windows JSON Hook 接入只管理与生成结果完全一致的命令条目，保留其他原始 JSON 值，拒绝被编辑对象中的重复成员，以及格式错误、被修改或外来的条目。Cursor 在非空绝对路径 CURSOR_CONFIG_DIR 下使用版本 1 的 `hooks.json`，否则使用 USERPROFILE/.cursor；其适配器仅上报会话身份。固定 PowerShell EncodedCommand 启动器将 capsule 路径作为数据解码，调用有界 Node 观测运行时并返回空 JSON 对象，不批准或修改 Agent 工作。

配置替换在移开原文件前后比较原始字节，以不覆盖方式发布，验证结果后仅删除未改变的备份。此阶段的读取句柄拒绝原地写入。取消、超时或冲突会在变更回执中保留未恢复备份和状态不确定的暂存目录。这是乐观更新流程，不是全局文件系统 CAS 或崩溃原子替换：目标可能短暂不存在，进程突然退出可能留下同级恢复文件；不防御同一用户的恶意重命名或删除行为。用户配置策略仍由 provider 管理。

Windows CLI 复用仅启用 alloc 的 base64 0.22.1 实现规范 PowerShell 编码；该库没有运行时依赖、构建脚本或过程宏，且禁止 unsafe。既有 serde_core 1.0.229 仅启用 std，提供拒绝重复键的对象 visitor；serde_json 的 raw_value 特性保留其他数字和子树，避免数值转换。这些均复用锁定版本，不增加传递包。测试覆盖快照变化、目标被占用、取消、精确归属和原始值保留；受控 Hook 执行不代表已认证 Cursor 会话。

发布前，Windows ReplaceFile 将经过验证的私有原文件的 DACL、属性和命名流合并到暂存结果中，不忽略合并错误。原文件保存在第二个私有路径，精确备份保留到验证完成。实际目标仍只通过不覆盖方式发布。元数据合并部分失败时重新检查两个原文件位置并保留无法证实的路径；自定义 DACL 和备用数据流测试覆盖成功路径。

Cursor 支持原生 Windows 的 `agent` 可执行文件。[Hook 参考](https://cursor.com/docs/hooks)定义扁平事件数组和会话身份字段；[CLI 安装说明](https://cursor.com/docs/cli/installation)定义原生 Windows 可用性。编码 PowerShell 启动器将固定 JavaScript 的单引号放在双引号参数中，使 Windows PowerShell 5.1 保留它们。公开 CLI 的受控见证覆盖安装后的命令在 PowerShell 5.1、PowerShell 7、cmd 中执行，以及非法输入、运行时缺失、内容身份、自定义 DACL 和备用数据流。

## Mastracode Windows 生命周期 Hook

原生管理器编辑 `USERPROFILE/.mastracode/hooks.json`，根对象将事件映射到扁平命令数组。保留外来条目与不透明值。每个选定事件只拥有一个生成命令，`timeout: 10000` 的单位为毫秒。全部自有命令必须指向同一 capsule 根；重复、修改或错放的命令均冲突。其余归属精确但缺少条目的集合需要修复。准确的根目录归属决定修复与拒绝。禁用仅移除已证明自有的条目。共享有界发布保留 Windows 元数据。Doctor 记录根目录，检查根后将不完整集合报告为 update-required。项目 Hook 仍由 provider 管理。

SessionStart 仅识别会话，不推断空闲；SessionEnd 结束会话。AgentStart 以 `run_id` 标识运行。AgentEnd 是唯一终止信号：complete 表示成功，aborted 表示取消，error 表示失败；suspended 不发事件，保留待处理提示。工具事件提供活动，权限请求／结果事件建立和解除提示，不作权限决定。子 Agent 事件以 `tool_call_id` 标识子会话。拒绝无效标识与未知原因；内容、工具参数、路径及结果不离开清洗器。

不安装 UserPromptSubmit，因为它没有运行标识，会先于带标识的 AgentStart。Stop 和 Interrupt 会重复终止，异步 Notification 不具备相同操作顺序；这些事件不作规范化。受控安装命令的 Windows 管道和规范入口轨迹验证所选生命周期，不证明认证后的 provider 执行、策略生效或交互就绪。固定编码 PowerShell 启动器经 provider 的原样 `cmd /c` 分派返回 `{}`；仍需 Node。

上游[事件类型](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/types.ts)、[配置加载器](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/config.ts)与[执行器](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/executor.ts)定义 provider 边界。

## Droid Windows 配置来源

Droid 使用 USERPROFILE 下首个存在的受支持用户 Hook 来源：`.factory/hooks.json`、旧版 `.factory/hooks/hooks.json`，最后是 `.factory/settings.json` 的 `hooks` 字段。更高优先级路径不可读或不安全时不回退。全部缺失时创建首选 hooks.json。原地编辑所选来源，不复制、归档或迁移其他来源，因此启用不会新建文件遮蔽已有回退 Hook。外部修改来源优先级会改变生效配置；未选文件仍由 provider 管理。

共享路径权威返回带类型的候选项，包含独立事件映射或嵌套设置的文档形状。接入与 doctor 各自通过有界文件系统适配器读取候选项。变更重新解析来源并执行已有受保护发布，随后重新检查活动配置；这不提供多文件原子性，也不重新加载已运行的 Droid 会话。

仅拥有一个精确生成的 SessionStart matcher 组，命令超时为十秒。保留外来组与原始设置值。适配器要求精确 SessionStart 与有效 session_id，随后只发出 session-identified。启动、恢复和压缩不推断空闲或就绪。编码 Windows 启动器返回 `{}`，不提供上下文或权限决定。不改变仅托管 Hook 或禁用 Hook 的策略；安装配置本身不能证明有效策略或投递。

来源：[Factory Hook](https://docs.factory.ai/harness/hooks)与 [Windows 设置](https://docs.factory.ai/droid-cli/settings)。原生来源优先级、保留与安装命令管道见证验证这些本地契约；认证后的 Agent 执行仍单独验证。

## Kimi Windows TOML hook

Kimi 使用当前原生 Windows `kimi` 命令，配置路径为非空绝对路径
`KIMI_CODE_HOME/config.toml`，否则为 `USERPROFILE/.kimi-code/config.toml`。
无效的显式根目录拒绝且不回退。旧 `.kimi` 数据保持原样；
安装不会执行迁移或修改凭据、模型、权限。

配置所有者仅管理两条 `[[hooks]]` 规则：`SessionStart` 和
`SessionEnd`，各自只有 event、固定编码命令和 10 秒 timeout。
也支持内联 hook 数组。完整命令往返、精确字段以及同一胶囊根证明所有权。
缺少部分自有规则需要修复；重复、修改、错误事件、混合根或外来条目拒绝。
语法保真编辑器保留其他规则及设置的 TOML 值、注释和格式。
换行符写法可能规范化。JSON 与 TOML 共用既有有界 Windows 发布、
元数据保留、恢复和残留规则；路径选择与离线 doctor 共享。

纯 JSON、Kimi 和事件集扫描器与 doctor 共用原有六种所有权状态。
它们向 Windows 变更保留原始解析文档，并显式使用 Windows 根准入和相等策略。
候选定位、有界读取、资源根观察与发布仍归 Windows 所有；共同解析不会在不支持的宿主上启用配置观察。

只转发精确 `SessionStart`/`SessionEnd` 输入中的有效 `session_id`，
分别作为仅身份识别或会话结束。不会发送提示词、路径或响应。
仍需 Node；固定 PowerShell 命令返回 `{}`，通过提供方 Windows cmd shell 验证。
本切片建立会话跟踪，并未完成 Kimi 工作中／阻塞／空闲对齐：当前 `Stop`
先于可能继续轮次的阻塞 hook 执行，异步生命周期事件也没有提供统一关联的有序流。
这些状态缺口仍未关闭。本地配置也不证明有效策略或认证任务完成。

公共 `toml_edit` 0.25.15+spec-1.1.0 依赖关闭默认特性，仅启用 parse/display。
它没有 unsafe 源码、构建脚本或过程宏，许可证为 MIT OR Apache-2.0，
复用现有 indexmap、toml_datetime、toml_parser 和 winnow 版本。
之前已锁定的 toml_writer 1.1.2 现用于格式输出。Winnow 启用标准
ASCII/binary 解析特性，SIMD 仍关闭；锁文件出现其可选 memchr 边不代表启用该特性。
不需要重复版本或新运行时。
既有传递依赖的 unsafe 仍归依赖所有。共享 Kimi 检查、既有配置见证与 Windows 编辑使用同一依赖。
当最后一个检查、编辑及夹具消费者消失时移除此依赖。

证据包括保留外来规则的夹具、两种 TOML 表示、部分修复与拒绝、隔离的公开生命周期／
doctor／根目录覆盖检查、已安装命令的 PowerShell 5.1/7 与 cmd 投递，以及共享 JSON 回归。
无需付费提供方会话。当前胶囊 与匹配接收端一同发布；旧接收端拒绝新适配器。
禁用只移除已证明自有的规则。

来源：[Kimi 配置](https://moonshotai.github.io/kimi-code/en/configuration/config-files)、
[hook](https://moonshotai.github.io/kimi-code/en/customization/hooks) 和
[固定版本提供方实现](https://github.com/MoonshotAI/kimi-code/tree/be7d5f5fea7800778e4660cd5f36780ba783bddd/packages/agent-core-v1/src/features/externalHooks)。

## Letta Windows 会话 hook

Letta 使用非空绝对路径 `HOME`，否则使用原生用户目录，
配置为其中的 `.letta/settings.json`。无效显式根拒绝且不回退；
不将 `LETTA_HOME` 当作上游配置覆盖。Windows 管理一个精确的
`hooks.SessionStart` 命令组，timeout 为 10000 毫秒且 `quiet: true`。
保留外来组、设置及原始未来字段，包括 `hooks.disabled`。
沿用既有所有权、内容身份、元数据、备份／恢复和取消规则。
这是本地配置产物检查；项目／全局 hook 策略不变，有效策略诊断仍待完成。

固定编码启动器的 stdout 与 stderr 均保持静默：Letta 将 hook stdout
作为提示词上下文，即使 `{}` 也会改变用户对话。其他提供方保留所需响应。
仍需 Node。适配器要求精确 `event_type: SessionStart`、布尔 `is_new_session`
以及有效 `conversation_id`。`default` 对话必须提供有效 `agent_id`，使用
`default:<agent_id>` 且满足相同身份长度上限，防止不同 Agent 被合并。
仅发出 `session-identified`；不推断提示词、空闲、就绪或结果。
缺失／无效载荷保持静默。恢复会话保留轮次状态。

纯所有权和身份测试，以及隔离的公开安装／修复／拒绝／禁用、HOME 覆盖、
doctor 和已安装 PowerShell 5.1/7/cmd 管道见证覆盖此契约。
这些证据不证明认证后的 Letta 工作。当前胶囊 与接收端组成同一 cohort；
回滚选择此前 cohort，禁用只移除已证明自有的声明。
不增加新依赖或其他平台流程。
来源：[Letta 0.33.2 hook 实现](https://github.com/letta-ai/letta-code/tree/46e1f82ba44145e8b8d0ac283c2470b59822715e/src/hooks)。

## Devin Windows 会话 hook

Devin 使用文档规定的 Windows 用户配置，即非空绝对路径
`APPDATA/devin/config.json`。缺失或无效 APPDATA 拒绝；不推测用户目录回退，
也不采用未记载的 Windows XDG 覆盖。项目 hook、配置导入、权限及 MCP
仍归提供方所有且保持原样。有界严格 JSON 读取器拒绝无法解析的输入，
不会重写；本实现尚未证明提供方对 JSON 注释的支持。

Windows 管理六个精确命令组：SessionStart、UserPromptSubmit、
PreToolUse、PostToolUse、PermissionRequest 和 Stop。各自超时 10 秒，
固定编码启动器返回 `{}`，不提供决策或上下文。完整集合必须引用同一个
自有胶囊根。缺少条目需要修复；重复、修改、错误位置、混合根或外来条目拒绝。其他设置／组保留原始值。既有多事件所有者现在服务 Mastracode
扁平映射和 Devin 嵌套组；发布、元数据保留、取消和恢复规则继续共享。

仅精确事件名与有效 session_id 发出 session-identified。重复身份保留轮次
状态。不推断 prompt_id、内容、按 cwd 查找的会话、空闲、权限决定或完成。
本地配置产物检查不证明有效策略或正在执行的 Agent 任务。

纯集合所有权测试、隔离的公开安装／修复／拒绝／移除和 doctor 检查，以及
六条已安装命令经 PowerShell 5.1/7、cmd／私有管道验证受控边界。
这些证据不独立证明真实 Devin hook 调度器。探测只使用已安装 CLI 的
帮助／版本；不运行 Devin Agent 任务或编排。当前胶囊 与匹配接收端一同发布；回滚
选择此前 cohort。夹具同时隔离 APPDATA 和原生用户目录。
不新增依赖或其他平台流程。

来源：[Devin hook](https://docs.devin.ai/cli/extensibility/hooks/overview)、
[生命周期](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks) 和
[Windows 配置](https://docs.devin.ai/cli/reference/configuration/config-file)。

## Hermes Windows 会话插件

Hermes 持有配置及插件激活。Windows 优先解析去除首尾空白后的非空绝对字面
路径 HERMES_HOME，否则使用 LOCALAPPDATA/hermes；LOCALAPPDATA 为空时回退到
原生用户目录的 AppData/Local/hermes。安全的单路径组件 HERMES_DATA_DIR_SUFFIX
仅作用于默认目录。相对或尚未展开的显式根拒绝。提供方命令收到已解析的
HERMES_HOME，确保检查和激活选择同一配置档。

Orchbee 仅持有 plugins/orchbee-agent-observation/**init**.py 和 plugin.yaml。
清单使用 YAML 子集 JSON；每个完整生成文件证明其胶囊根。外来或被修改的成员拒绝。单个成员缺失，或分别证明为过期／当前身份的成员需要修复，包括
中断的双文件更新。启用通过共享有界 Windows 文件发布原语先发布入口，再发布
清单，保留元数据和可恢复原像；之后运行 Hermes 自身的 plugins enable，并通过
plugins list --json 验证。停用运行 plugins disable，保留两个文件。Python 缓存、
其他插件文件、YAML 配置及能力授权仍归提供方持有。不删除目录。已停用的部分／旧版本入口仍报告 update-required，保留修复需要。
提供方准入失败
或超时保留已观察副作用与恢复路径；激活需要后续提供方会话生效。

Python 插件注册 on_session_start、on_session_reset 和 pre_llm_call。
仅准入 cli/tui/desktop/acp 平台，pre_llm_call 仅支持 CLI。严格会话身份经有界、
隐藏、无 Shell 的 Node 子进程进入既有观察传输。回调不返回上下文或决定；仅发出
session-identified。不推断内容、工作／空闲或完成。离线 doctor 报告
provider-command-inspection-required，不将存在插件冒称为已启用。

纯文件／列表判定、隔离的公开激活／修复／拒绝／停用，以及 Python 导入和真实
Windows 管道证据覆盖这些边界，不证明经过认证的 Hermes 工作或活动会话重载。
当前胶囊 与匹配接收端一同发布；回滚选择此前 cohort。
不新增依赖或其他平台实现。
来源：[固定 Hermes 插件实现](https://github.com/NousResearch/hermes-agent/tree/2a05208012cf50bbe8ff7b7b54a3825fd1e4692a/hermes_cli)
和 [原生 Windows](https://hermes-agent.nousresearch.com/docs/user-guide/windows-native)。

## 运行令牌转发

受管 PowerShell 只把 `ORCHBEE_RUN_TOKEN` 导出给 Agent 启动命令，Agent 的 hook 继承它。共享传输仅在它是 32 位小写十六进制且非零时，才把它作为可选的 `runToken` 加入 envelope；其他值一律省略，观测仍会送达。Server 和 Desktop 的 envelope 解析器接受带或不带该字段的 envelope，拒绝已出现但格式错误的令牌。Server 把携带令牌的会话精确绑定到其运行，不带令牌的会话沿用最新运行回退，见 [canonical 模型](../../orchbee-server/docs/agent-canonical-model.zh-CN.md#托管启动预留)。

在该字段之前安装的集成继续发送不带令牌的 envelope，并沿用回退规则。该字段改变了 Capsule 负载，因此 doctor 会把它们报告为 `update-required`；`orchbee agent integration enable --provider codex`（选择相应 provider）会安装转发令牌的传输。Envelope 版本不变，也不需要协商。

Gemini 和 Qwen 扩展把脚本复制到自己的目录中，其是否为最新取决于条目契约版本，而不是 Capsule 摘要。因此已安装的扩展仍显示 `enabled`，继续使用不带令牌的传输并沿用回退规则；先执行 `agent integration disable` 再执行 `enable`，即可重新安装带转发的版本。

## 当前 Capsule 身份

Capsule 契约为 1，以 `payloadSha256` 为身份；准确清单及规范摘要规则见
[适配器指南](../../desktop/resources/agent-observation-adapters/README.zh-CN.md#打包载荷身份)。
CLI Capsule 对象暴露该摘要，目录为 `~/.orchbee/agent-observation-adapters/<payloadSha256>`。
不保留 semver 上限、特定旧根目录或自动迁移。最终载荷任何字节变化（包括文档和格式）均产生新身份。

Doctor 核对安装字节与自身清单，并与当前 sidecar 身份比较。自身一致但身份过期的载荷报告
`update-required`。非当前 capsule 元数据只有在实际摘要、必需运行时文件及带准确名称的 Orchbee 包标记均验证后才提示重装。损坏、不安全或不可读输入保留受保护的结果；这一只读诊断不将非当前来源准入为就绪。显式修复使用
`orchbee agent integration enable --provider codex`（选择实际提供方）。冲突或无法检查的入口
仍需人工复核。已有目录保持不变；disable 沿用既有提供方入口删除规则。回滚选择完整且保留的 cohort。

Windows 公开 doctor 用例将可执行文件、当前 sidecar 和安装树复制到隔离用户目录，修改运行时及文档字节，
通过 Desktop 的摘要实现更新来源清单，核对更新状态、本地化重装命令及安装树保持不变。
Unicode 摘要向量区分 UTF-16 路径顺序与 UTF-8 字节顺序。

<!-- documentation-pairing: english-sha256=9d7bdc3f0c1f1065aef8dd65f8f507f289ee07c580aaf8b3c5e3c25383a890c6 zh-cn-sha256=c650a765982b78da703d00652861c4c02f2a603b2068a16b10e9f8e0b10e8a37 -->
