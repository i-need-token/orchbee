# Windows Agent 集成

本文负责原生 `agent integration list`、`explain`、`enable`、`disable` 的 Windows
细化契约。CLI 负责本机接入，不启动 Site。共享的 provider 分类器与变更流程继续负责
归属、版本方向、计划、回执和残留物。

## 模型与边界

| 字段       | 契约                                                                                                                                                                                                                       |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 问题       | Windows 用户需要通过启动终端的同一个 CLI 安装、检查和移除观测集成。                                                                                                                                                        |
| 输入       | Provider ID、可选 capsule 来源、原生用户目录、PATH 和 Codex 配置根均是不可信路径。空 CODEX_HOME 使用用户目录默认值；非空无效覆盖值不得回退。                                                                               |
| 数据与归属 | Provider 拥有其配置。Orchbee 仅拥有带版本的 capsule 和明确标记的集成条目。外来或更新版本条目保持不变。                                                                                                                     |
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

Windows 托管 Claude Code、Codex、Grok 和 GitHub Copilot hooks 以及既有 OpenCode、Pi、OMP JavaScript 模块。其他 provider
在其 Windows hook 执行路径获得支持前保持 `manual-only`。检查与变更共享平台准入规则。

OpenCode 使用 ESM 入口；Pi/OMP 安装的 CommonJS 引导模块动态导入 capsule 的 **ESM**
入口，再调用具名注册函数。若导入其 CommonJS 包装器并查找同名导出，会静默跳过注册。
模块细化在载荷 0.4.4 中保持既有引导行为。确定性的引导
文件字节与 file URL 仍为归属证明：被修改、外来或指向其他版本的条目会被拒绝，不覆盖。
Windows doctor 从完整的 `bundle/<provider>/<entry>` 后缀推导 capsule 根目录，
健康的已安装模块因此对照其 capsule manifest 检查，不再误查 bundle 目录。
这是离线文件检查，不证明实时连接可用。

公开 CLI 验证在隔离的 Unicode 用户目录中安装、检查、移除三个模块。真实 Node 加载
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

Windows `agent integration list --outdated-only` 只执行一次原有有界本地检查，仅显示 update-required 条目，包括需要修复的自有安装。它不安装、不下载。`agent integration enable --provider <id>` 仍是显式且受保护的更新／修复操作；冲突及更新版本条目不会被替换。

筛选列表使用 v2，包含 outdatedOnly true、inspectedCount、表示检查未知的 unresolvedProviderIds，以及表示冲突／更新版本的 blockedProviderIds。Complete 在筛选前根据全部所选检查计算；空列表不能隐藏检查失败。文本包含这些计数和相同筛选条目。默认列表保持 v1 及既有行为。此参数不新增非 Windows 运行时；原有 40 秒期限、取消、用户目录隔离及仅本地归属保持不变。详细已安装版本报告及其余 provider 的 Windows hook 支持仍是独立义务。

## Claude exec hooks

Windows 也可使用 capsule 0.4.4 托管 Claude Code。Hook 声明使用 provider 的 `command` 加 `args` exec 形式：Node 将插件脚本路径作为原样参数接收，包括空格、单引号、Unicode、美元符和反引号。固定动态导入保护会忽略已删除的脚本；既有有界清洗器及 Windows 管道／控制台传输仍是唯一观察路径。不需要 Bash 或 PowerShell 启动器。Node 必须保留在 provider PATH 中；缺失时 Claude 报告其非阻塞启动诊断，不产生观察。这替代原先仅 Claude 无 Node 时仍静默成功的承诺。其他 provider 保留各自启动契约。[上游 exec 契约](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form) 定义参数替换。

启用、禁用和修复复用既有用户作用域插件命令及有界 Windows 进程所有者。旧版自有 capsule 需要显式修复；当前已启用条目为空操作，外部／更新版本条目仍拒绝。隔离 provider 命令及已发布 hook 见证覆盖配置和清洗后回传，不证明已认证 Claude 任务完成。共享可移植 hook 声明会变化，但本工作不增加 macOS/Linux 工作流或运行时验证。

## Copilot 命令兼容

Windows 对齐已用官方 Copilot CLI 1.0.88 验证的插件契约：`plugin list --json` 返回平面数组，插件命令不再接受已移除的 `--kind` 或 `--plugin` 参数。单一固定命令所有者为检查、计划和执行提供参数。检查要求准确的 marketplace、本地安装来源、live 来源及载荷版本；外部、更新版本和格式错误的观察都不能授权修复。Unix adapter 保留原命令和解析契约，不推测性重试变更。

Capsule 0.4.4 使用 Copilot 的 Node exec hook 形式。固定导入保护将 provider 的 COPILOT_PLUGIN_ROOT 环境变量作为路径读取，不进行 Shell 展开，并保留既有 PascalCase 事件输入契约。根目录或脚本缺失静默处理；Node 仍为前提。[插件参考](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference) 和 [hook 参考](https://docs.github.com/en/copilot/reference/hooks-reference) 拥有上游语义。既有本地变更期限、Job Object 回收、不可变发布、回执和无内容观察规则保持不变。受控安装和 hook 回传不证明已认证 Agent 任务完成。

即使 marketplace 根目录是旧版自有 capsule，Claude 或 Copilot 报告的更新插件版本仍会阻止 Windows 修复。版本数据缺失不能授权替换。

## Windows TOML 和 Grok 本地状态

Windows Codex 和 Grok 检查在既有 512 KiB 读取上限内解析完整 UTF-8 TOML 文档。语法错误、重复赋值、无效 UTF-8 和不可读数据返回 unknown；合法引号键／点分键、内联表和行尾注释保留原有含义。解析不写配置，也不回显其内容。仍要求插件布尔值类型正确及本地来源所有权精确匹配。仅 Windows 使用 toml 1.1.6，禁用默认特性，只启用 std、parse 和 serde；不启用序列化／display、unsafe 解析加速或 SIMD。新增依赖树为 serde_spanned 1.1.1、toml_datetime 1.1.1、toml_parser 1.1.3 和 winnow 1.0.4，复用已有 serde_core 1.0.229。锁文件也记录可选 toml_writer 1.1.2，但当前特性树不编译它。不新增构建脚本或过程宏；toml 系列许可为 MIT OR Apache-2.0，winnow 为 MIT。Winnow 内部包含 unsafe 切片操作；自有代码继续禁止 unsafe。既有 Unix 解析器不变。

Grok 的已安装 JSON 清单不报告启用状态。Windows 将精确归属的 bundle 来源及版本与用户配置关联：非空 GROK_HOME 优先，否则使用 USERPROFILE/.grok。无效覆盖不回退。显式 disabled 条目优先；未列出的用户插件处于禁用状态。这是本地接入配置状态：项目／托管策略及已经运行的会话仍由 provider 管理。启用使用既有 install/trust 或 enable 命令；禁用仅移除来源已证实的插件。即使来源是旧的自有 capsule，报告更新版本的插件仍阻止修复。

Capsule 0.4.4 将固定 Node 引导编码到 Grok 的纯命令 hook 声明中。用户路径和 hook 输入均不成为 Shell 源码。Node 读取 GROK_PLUGIN_ROOT，并通过 pathToFileURL 导入拼接脚本；根目录／脚本缺失静默处理，Node 则是前提。既有输入、事件、传输及 hook 时限保持不变。真实 PowerShell 5.1／7 用例在隔离配置中执行交付命令和重定向 JSON stdin；不代表已认证 Grok 任务或新增 macOS／Linux 流程。
