# Windows 原生插件

## 目录模型

原生插件目录属于用户共享配置，独立于 Session 生命周期。Windows 的 `plugin link PATH`、`unlink ID`、`enable ID`、`disable ID`、`list [ID]`、`config-dir ID` 和 `action list [ID]` 不启动 Session，直接使用此目录。链接仅登记清单，不执行 build 或 startup 命令。解除链接仅移除登记，保留源文件和用户配置/状态目录。

`plugin link` 在语法解析阶段校验路径写法；只有最终执行的 CLI 在完整语法和路由准入之后、访问插件目录之前，才展开用户主目录或当前目录路径。已保存机器的发送端和中继保留原始参数，不使用自身进程上下文展开路径。盘符绝对路径保持字面值。主目录写法（`~`、`~/`、`~\`）依次选择执行进程中首个可用来源：非空的 `USERPROFILE`，其次是 `HOMEDRIVE` 与 `HOMEPATH`，最后是 `HOME`。已选中的非空来源格式错误时不回退。其他相对路径使用该进程的当前目录。所需上下文无效时仍返回普通用法错误，不创建插件目录或运行时目录。

local-site-runtime 插件边界负责有界 TOML 校验和持久目录。目录参数优先解析 `orchbee-plugin.toml`，仅当其缺失时使用 `herdr-plugin.toml`；也可直接指定清单文件。导入 Herdr 元数据不代表兼容 Herdr 专用可执行文件或 API 调用。标识符去除首尾空白，区分大小写，最多 120 个 ASCII 字符；插件 ID 允许字母、数字、冒号、点、下划线和连字符，action ID 不允许点。参数保留为独立 argv 元素。未知字段、重复条目 ID、无效命令和无效平台名称均在登记前拒绝。

清单最多 128 KiB，目录最多 32 项且不超过 1 MiB。条目保留规范源路径、启用状态和最后一份有效清单。读取会刷新清单；源缺失或无效时保留检查元数据并明确警告，不能据此授权执行。条目平台列表优先于插件列表；均缺省时允许 Windows。其他平台标签仅为元数据。

## 持久化与转换

目录存储位于已准入的用户配置父目录下的 `plugins`；复用 Windows 私有缓存的祖先路径校验、独占写锁、落盘后原子替换和中断写入清理。只读操作不创建目录。修改在锁内重新读取，校验完整新目录，仅在实际变化时递增安全整数 revision。损坏、竞争和容量上限均返回错误，不当作空目录。解除不存在 ID 的链接不产生变化；启用不存在的 ID 失败。带前缀和哈希的 ID 路径组件使配置/状态目录稳定，并避开 Windows 保留名。解除链接后保留用户目录。

目录操作失败时保留原有状态码和类型化机器结果。text 模式的 stdout 为空；JSON 模式
输出一份与语言无关的文档。两种模式都向 stderr 写入相同的本地化诊断。
[真实 CLI 测试](../tests/cli_windows_plugins.rs) 检查输出流分离、两种已交付语言、
机器输出字节不变，以及无效清单的拒绝发生在配置或运行时创建之前。

## 运行时 action 与日志

`plugin action invoke ACTION [--plugin ID] [--client ID] [--context JSON]` 向已有 Session 提交普通 action ID 或 `plugin.action` 完整 ID。`plugin log list [ID] [--plugin ID] [--limit 1..200]` 查询最新保留日志，默认 50 条。list 和 action-list 也支持用 `--plugin` 替代位置过滤参数。均支持 `--format text|json`，运行时命令不启动 Session。上下文是封闭的 camelCase 对象，包含可选 workspace/tab/Pane 展示字段、worktree、选中文字、调用来源、关联 ID、点击 URL 和链接处理器 ID；编码总量最多 16 KiB，选中文字最多 8 KiB。上下文不携带授权。省略调用来源时使用 `cli`；关联 ID 默认取本次调用的操作 ID。

调用在同一个十秒期限内先推导上下文。显式 Client 优先于继承的调用者身份；否则从规范快照中的精确调用者绑定选择终端成员，或由唯一当前 Client 选择观察到的成员。多个当前 Client 需指定 `--client`；没有调用者且没有当前 Client 时按全局动作调用，不编造选区。Ready 和稳定 Busy 视图可读。补充信息期间规范成员、元数据、Client 选区及成员代际必须保持一致，否则不提交动作。运行时 cwd 与 Agent 观察属于该精确成员；该成员显式上报的 Agent 名称及非 unknown 状态优先，否则采用唯一观察到的 provider 及聚合状态；缺失的观察保持空值。工作区 cwd 优先采用观察到的 Pane cwd，再取工作区目录。Worktree 使用现有只读查询，不绕过信任，保留原生字段 `workerId`、`repositoryRoot`、`commonDirectory`、`repositoryName`、`checkoutPath`、`linked`；不可用时保持缺失。显式非 null 上下文字段覆盖推导的展示字段，但不选择或授权任何目标。API 调用不抓取 Client 选中文字，调用者显式提供 `selectedText`，与参考 API 一致。

Server 在解析调用前要求读取、终端创建和生命周期权限。确认 Session、action 唯一且源有效、已启用、支持 Windows 后，固定 argv、源 cwd 和环境。Worker 在插件根目录执行独立参数；显式 shell 命令保留自身 shell 语义。相对程序从插件根目录解析，再使用 Windows PATH/PATHEXT。原生上下文通过 `ORCHBEE_PLUGIN_CONTEXT_JSON` 及插件 ID/root/action/config/state 路径、`ORCHBEE_BIN_PATH`、可选 workspace/tab/Pane ID 和链接字段提供。受管 Worker 用自身 Session 路由覆盖 runtime base/scope。这些是 Orchbee 变量和 API 语义，Herdr 专属脚本可能需要调整。安装本次字段前会清除继承的插件/终端身份。

每个 Worker 生命周期最多拥有 32 个并发命令和 200 条日志。每个命令保留线程及 Windows Job，在 CLI 断开后继续运行。保留期内相同操作 ID 返回原回执，不同意图产生冲突；已淘汰日志不构成持久重放记录。CLI 在回复不确定时不重新提交修改。启动回执只确认受理，不确认执行成功。日志包含序号、起止时间、退出码、类型化失败、stdout/stderr 和显式截断标记。每路原始输出保留 64 KiB，超出后继续排空；无效 UTF-8 用替换字符解码。文本输出转义控制字符。优先淘汰最旧的已完成日志，运行中的命令保留归属。命令结束后提供输出。

运行时日志在同一个十秒 CLI 截止时间内按每页四条读取，并固定 Worker/generation 和递减序号游标。新增命令不会推移后续页面；Worker 变化会拒绝旧游标。调用使用 `orchbee.plugin.runtime` v1；历史使用 `orchbee.plugin.logs` v1，通过一个展平的 `logs` 数组按所选最近记录的时间正序返回，默认 50 条，与参照一致。内部游标分页仍为倒序。两者遵守常规 8 MiB 输出上限；旧 runtime schema 继续用于历史回执。日志查询将进程失败作为数据返回，不因此让每次历史查询失败。私有 get/cancel 请求指向准确 Worker 和 job ID。取消及 Session 停止会清理根进程和后代、排空输出并等待线程结束；Job 清理不确定时阻止新执行，重试不能将未知清理变成成功。没有隐藏辅助窗口、脱离归属的管道读取线程或任意命令时长限制。

## 托管源码安装与移除

`plugin install owner/repo[/subdir] --yes [--ref REF] [--timeout MS]` 不需要 Session，要求 Windows PATH 中有 Git。`--yes` 显式授权执行下载源码中的构建命令；原生 CLI 不提示确认或读取终端。执行期限默认 600000 毫秒，范围 1..3600000；中断或超时后，活动 Windows Job 另有最多五秒的清理预算。`plugin uninstall plugin-id|owner/repo[/subdir]` 移除登记及托管源码，保留用户配置/状态和本地链接源文件。两者均支持 text 或 JSON 输出。

共享私有目录在有限的 checkout/build/commit/cleanup 流程内持有写锁，读取仍可继续。新代目录先登记，再创建于 `plugins/managed/i-<operation-id>/checkout`。Git 初始化目录，获取一个分支/标签/提交（默认 HEAD），检出确切提交并记录哈希。生效的 Windows build argv 从 manifest 根目录执行。禁用 Git hooks 和符号链接检出；构建以当前用户运行，不宣称具备沙箱。构建子进程继承用户工具环境，移除 `ORCHBEE_*` 运行时上下文并禁止交互式 Git 凭据提示。同步 CLI 拥有每个 Job、有界输出读取和中断；契约不允许这些进程在所有者退出后继续存活。

构建后 manifest 必须保持不变。本地链接 ID 拒绝替换；托管替换保留启用状态。只有原子替换目录登记后新源才生效。构建失败保留原登记并移除暂存源。已提交的替换或卸载将旧代加入清理列表；清理失败不会撤销提交，也不能被当成提交前拒绝。清理根据生成的 ID 推导路径，固定私有祖先并拒绝重解析点、外部所有权或硬链接。下次显式 install/uninstall 恢复记录的清理，不重新执行之前的构建；进程创建前先持久化可能的物理所有权，仅在确认清理完毕后解除；进程运行时崩溃或物理所有权不确定时禁止自动清理。当前目录引用（包括显式链接进某一代的本地源）阻止删除该代。unlink 保留源文件，不安排清理。

存储仅准入当前版本 1，要求完整的托管来源和有界恢复字段。保留的 manifest 与来源必须包含所有序列化字段，并显式写出 null／空值；编写 TOML 时的缺省规则不适用于保留 JSON。不支持版本及不完整形状返回 InvalidRegistry，保留原字节并拒绝修改，不迁移。`orchbee.plugin.registry` v1 增加可空的托管来源，v1 schema 保持封闭。`orchbee.plugin.provision` v1 报告操作身份、阶段、已提交变更、失败命令/退出码/输出、截断及未清理路径。源码安装/更新提交与构建产生的外部副作用分开，删除 checkout 无法回滚外部副作用。退出码 7 表示执行超时，8 表示物理状态不确定，130 表示中断，其余安装/卸载失败使用 1。

## 原生 TUI 链接激活

Windows 原生 TUI 中的精确 Ctrl+主键点击，从最近一次完整绘制解析 cell，不改变焦点也不取得
终端控制权。已绘制来源携带精确 Pane／成员 binding 和终端内容围栏。私有只读 viewer 必须
重现捕获的 host epoch、generation、revision、几何和总行数，之后才可解析 OSC 8 元数据或
经过验证的软折行小写 HTTP(S) token。过期、重新绑定、格式错误、忙碌或不可用的观察都不
产生外部效果，也不重放。

TUI 与公开 `terminal link-activate` 共用一个应用私有的有序 handler 预检和结果边界。固定
持久注册目录中第一个清单 pattern 匹配的已启用 Windows handler，观察精确点击终端上下文，
并在单次插件提交前立即重读完整有序目录指纹、registry 条目、handler 和 action。受 guard
保护的 Server 路由在解析 action 前校验同一注册目录指纹。注册读取只访问有界私有目录文件：
链接准入只接受固定本地卷，固定私有缓存的非 reparse 祖先，并将文件限制为 1 MiB；它不刷新链接
清单。链接清单的编辑只有经过显式重新 link 或托管 update 发布后才可用于链接点击。registry
漂移会拒绝点击，不选择替代 handler。插件启动、失败或结果未知都是最终结果，绝不打开浏览器。
浏览器交接前会重读注册目录；只有仍能确定无 handler 时，才通过 Windows shell 打开一次小写
`http://` 或 `https://` URL。

可选的受 guard 保护调用字段是匹配 cohort 的协议扩展；Rust、TypeScript、JSON、Client 和
Server peer 必须一起部署。普通的不带 guard 插件调用保留现有的实时清单刷新行为。

上下文观察始终绑定到被点击的 presentation。它在对精确终端目录、presentation Agent 报告、
匹配的存活 Agent session 和 Workspace worktree 事实进行有界读取的前后，重新核对规范成员关系。
Pane 目录覆盖 Workspace 主目录，作为 `workspaceCwd`；未知的 Agent、状态、目录或 worktree 事实
会被省略，不会推断。来源、报告或目录漂移会在效果交接前拒绝点击。

其他 scheme 没有 fallback。TUI 只保留一个有界作业，忙碌点击不排队也不重试；关闭时 join
自有通道，但不宣称拥有已经
受理的插件 action 或 shell 启动的浏览器进程。Ctrl 悬停装饰属于后续表面，不启用
any-motion capture。

## 交付与证据

原生已接通目录、action 调用、保留日志及托管源码安装/卸载。插件 Pane 生命周期如下。公开 `terminal link-activate` 和原生 TUI 点击路径通过共享边界，按顺序调用已启用的 Windows 链接处理器并带上精确点击终端上下文；CLI 的读取围栏、无匹配／拒绝／未知回执和限制属于 [原生 CLI 查询](native-cli-query.zh-CN.md#激活终端链接)，指针／来源归属属于[原生鼠标输入](native-mouse-input.zh-CN.md#链接激活)。目录接受不代表命令已执行。参照固定 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`，主要为 `cli/plugin.rs`、`app/api/plugins/manifest.rs` 和 `persist/plugin_registry.rs`。

定向证据覆盖解析上限、标识符/路径碰撞、源不可用时保留条目、缺失目录只读、目录原子转换及真实 Windows CLI 回执。测试夹具拥有并回收临时路径。完整 Windows 视觉/provider/多 Client 与全量回归验收另行执行。

## 覆盖层生命周期模型

Windows overlay placement 对所选实时 Client 的活动 Panel 创建规范水平 split，强制聚焦和 zoom，并保留该 Client 先前的焦点与 zoom。它不激活操作系统窗口。Popup 几何属于另一项能力。调用方环境不能将覆盖层定向到其他 Panel；显式 Workspace、target 和 direction 覆盖在创建前拒绝。创建与 Client 交接共用一个截止时间；交接中断或结果不明时保留已知创建，不再次启动。创建 v1 明确记录 overlay 请求；v1 不接受该字段。

Client 最多拥有 32 个临时覆盖层记录，以精确 context、Tab、Panel、Presentation 和 binding 为键。准入固定新建成员和返回成员，并在改变焦点前检查原始选择。记录在第一个焦点／zoom 效果前安装；之后中断仍保留记录。规范成员存活时记录跨本地导航保留，但不持久化，也不在 Client 重启后重建。嵌套覆盖层从最新移除的成员开始回退；返回成员被移除时折叠返回链，并保留最外层先前 zoom。不可用或重新绑定的返回成员绝不被选中。

| 转换         | 守卫与结果                                                                                        |
| ------------ | ------------------------------------------------------------------------------------------------- |
| 打开         | 精确当前 Client、源选择与规范成员；在焦点／zoom 前预留有界记录                                    |
| 观察         | 针对完整物理身份读取 Host inventory，限制页数和截止时间；不改变生命周期，也不占用独占关闭所有权   |
| 自动关闭     | 已知终端 exited／failed 且插件成员未变；提交一次带规范 revision 守卫的 Panel Close，保留操作身份  |
| 关闭回执丢失 | 保留不确定性并停止自动提交关闭；读取或断连不能证明清理完成                                        |
| 规范移除     | 仅覆盖层仍拥有该 Tab 焦点时恢复先前焦点／zoom；保留当前其他 context／Tab，以及手动焦点／zoom 选择 |
| 源／成员丢失 | 丢弃无效视图记录，沿用确定性选择修复，绝不选择复用身份                                            |
| Client 关闭  | 中断并 join 独立生命周期通道；Server 继续拥有已接受 Close 和规范终端                              |

Server 串行拥有临时观察 viewer，以及结果不明的清理，直到精确 detach／disposal 或 Session 物理关闭；清理未解决时拒绝另一次观察。生命周期通道一次拥有一个有限读取／关闭工作，不阻塞渲染，不重试结果不明的变更。测试必须区分自然终止、显式关闭、读取不可用、嵌套移除、导航和重新绑定，并覆盖实际渲染与 Windows CLI 传输。同作者定向证据不代表操作系统终端模拟器或已部署 Host 验收。

## 终端 Pane 创建

`plugin pane open --plugin ID --entrypoint ID [--placement tab|split|zoomed|overlay]` 通过规范创建流程生成真实终端 Tab 或分屏。清单提供默认布局和标题；`--target-pane` 选择分屏目标，`--direction right|down` 控制方向，`--workspace` 选择 Tab 的目标 Workspace。默认聚焦；`--no-focus`、`--client`、`--cwd`、重复 `--env`、几何尺寸与超时沿用现有创建控制。默认目录为 Server 解析的插件根目录，不受普通 shell 的 cwd 配置影响；即使覆盖 cwd，清单程序仍相对插件根目录解析。原生回执使用 `orchbee.creation.result` v1，区分规范创建与 Client 聚焦结果。 zoomed 布局通过一次受视图版本约束的 Client Zoom 请求强制聚焦（即使指定 `--no-focus`）；其 v1 回执增加 `postCreate.requested.zoom=true`，聚焦及放大的合并结果保留在 `postCreate.focus`。选区变化或 Client 回执丢失仍保留创建回执，不会重新创建终端。与 Herdr 一致，split/zoomed 忽略 `--workspace`，tab 忽略 `--target-pane`。

CLI 提交目录标识，不提交程序执行权限。Server 受理时重新检查已启用、源有效且支持 Windows 的入口，在终端预留前冻结 argv、cwd 与受保护的插件环境。重放使用保留的规范请求。显式程序／profile 与插件 Pane 选择互斥；Workspace 启动和子终端创建拒绝此选择器。CLI 和 Server 保留程序／argv 标识；允许绝对 `.cmd`/`.bat` 入口，由 Host 使用系统命令处理器执行，禁用 AutoRun 与延迟展开。batch 参数编码保护元字符及百分号字面值，脚本自己负责其后续参数展开。含 NUL/CR/LF 的参数、不支持的 verbatim 路径写法或超过 8191 字节的编码命令行，在 Host 创建 PTY／进程前失败；规范创建回执仍保留已受理的启动。独立 popup 布局使用下文单独的 popup 生命周期。`plugin pane focus <pane-id> [--client ID] [--timeout MS]` 和 `plugin pane close <pane-id> [--timeout MS]` 先读取 Server 所有的成员记录。精确 Presentation／绑定及规范修订号必须与 CLI 快照一致；普通、重新绑定或过期目标在产生效果前拒绝。close 提交一次修订号围栏下的原生 Panel 关闭并返回其保留生命周期回执；focus 沿用 Client 选择与视图围栏。投递结果未知时不重试；稍后发生的取消保留已取得的回执并返回非零退出码。

成员记录为与终端预留同时提交的临时 Server 状态，以精确 Presentation／绑定为键，上限 4096 条。记录跟随移动，成员消失或重新绑定时清除。处于 pending、失败或物理结果未知状态的启动仍关联规范 Pane，交由正常生命周期清理。离线目录 unlink 不妨碍关闭已经创建的插件 Pane；重启／检查点恢复不会凭空制造插件成员记录。

Overlay 在创建前通过被动读取固定源 Client 实际选中的主或子 Presentation。其 v1 回执在强制焦点／zoom 请求之外增加 `postCreate.requested.overlay=true`；已知 Overlay 答复使用 Client control v1。Server 在返回终止事实前 detach 临时 viewer；detach 结果不明时保留一个清理所有者并阻止替换。Host inventory 遍历共用两秒预算，最多八页、每页 128 个条目，核对稳定 generation／cursor、重复身份及完整身份。Client 通道使用十秒可取消作业并公平调度记录；现有 Server attach／detach 上限独立保留，因此这不是十秒内完成物理服务关闭的保证。手动关闭插件 Pane 也走相同的规范移除／恢复路径。

## Pane 启动来源上下文

插件 Pane 创建可携带 `LaunchOptions.pluginContext`，复用已有插件调用上下文契约的有界显示观察。仅在 Tab 创建或 Panel 分屏同时提供插件 Pane 选择器时接纳。CLI 在创建前通过规范成员以及被动 Client／调用者、主／子成员、物理 Agent／cwd 和 Workspace worktree 读取观察原始来源。同一个创建 deadline 覆盖这些读取与后续创建／交接；来源或 Client 观察过时则在提交前拒绝。空、headless、未绑定或不可用的观察不伪造聚焦终端或选中文本。

Server 验证来源成员及规范标签，将 `invocationSource` 固定为 `plugin-pane`、correlation 固定为实际创建请求，并在 reservation 前冻结到已有受保护启动环境。上下文描述 placement 在创建前选定的来源；不提供可执行文件、目录策略、Worker 或终端权限。未携带该字段的已有请求保持原有接纳。不会新增上下文存储或 replay 查询；冻结启动 intent 拥有上下文生命周期。Rust、TypeScript 与 JSON 对端需成套部署。独立 popup 的几何、reservation、exposure、输入和关闭使用下文的生命周期。

冻结于 `e0507237` 的 Herdr 基线在 split／zoomed 上下文中选择显式目标 Pane，在 Tab 上下文中选择目的 Workspace 记住的焦点。CLI 从已解析的规范创建命令推导来源，不使用显示标签或待执行导航 intent。Overlay 保留原始选中的主／子成员。已有私有上下文使用原生调用来源／关联约定。两边基线的 Pane 启动都不会自动复制选中文本。

| 创建来源             | 观察与缺省策略                                                                                                  |
| -------------------- | --------------------------------------------------------------------------------------------------------------- |
| Split 或 zoomed      | 精确目标 Panel 及其规范 Tab／Workspace；仅当实际调用者／Client 主或子成员属于该目标时使用它，否则观察规范主成员 |
| 当前上下文中的 Tab   | 调用者或所选 Client 的实际焦点及主／子成员                                                                      |
| 非活动上下文中的 Tab | 精确观察 Client 以只读方式记住的活动 Tab 和布局焦点；核对规范父级成员后观察该 Panel 的主成员                    |
| 空或 headless 目的地 | 可用时提供规范 Workspace 标签、cwd 和 worktree；省略未知 Tab、焦点和物理事实，不猜测树中的第一个成员            |

Client 拥有记住的 Tab／焦点；Session 拥有规范成员和标签；已有物理收集器拥有经核实的 cwd／Agent 观察及临时 viewer 清理。后台布局探测仅将规范 Panel 用作查询锚点，不将它当作焦点 fallback。非活动上下文中的原生子终端选择未由这些只读记忆契约暴露，因此后台来源为规范主成员，对齐 Herdr 每个 Pane 一个终端的模型。

来源流程依次解析规范目标、观察精确记忆／成员、收集物理事实、重新读取后台 Tab／焦点及原始 Client，最后提交已有的单次创建请求。所有阶段共用创建 deadline 和取消所有者。父级错误、revision 过时、记住的 Tab／焦点或原始 Client 变化均在创建前拒绝；读取不会导航、聚焦、zoom、启动或重试 mutation。来源上下文仍为显示数据，不能改变 plugin-root cwd、program／argv 或 Worker 权限。聚焦证据必须覆盖不同显式目标、记住的 Tab 和 Pane 都不是首项的非活动目的地、当前／调用者子成员保留、空／headless 缺省，以及变化或无效记忆时拒绝。这些检查证明原生 Windows CLI 传输和有主观察投影，不证明已部署 Host／模拟器验收。

## 独立终端 popup

Windows 支持独立插件 popup 终端及原生关闭入口。冻结的 Herdr 基线在当前 Pane 区域上方创建一个独立终端，不改变规范 Tab／分屏成员；外框宽高默认各占可用区域的一半，支持单元格数和 1–100 百分比，并在终端结束后关闭。popup 的按键和粘贴先于普通绑定处理：Escape 和配置的前缀键属于其中的程序。区域外鼠标事件不能激活底下布局。原生标题栏关闭按钮是额外的明确关闭入口。它使用共享的三单元格 `[x]` 操作，只有普通主键按下与匹配释放都解析到同一个已绘制 popup 请求时才会激活；过期或已替换目标不会产生效果，关闭开始后该操作会禁用。在周围空白背景上普通左键按下并匹配释放，会调用精确 popup 的同一个已绘制关闭目标。带修饰键的点击和滚轮不会关闭；移入 popup、失焦或几何变化会取消手势。

既有终端协调器负责身份分配和独立 Terminal Create；popup 不预留隐藏 Tab、Panel 或 Presentation。一个独立且已认证的 Client 连接最多持有一个 popup；Session 最多保留 32 个 popup 所有者，含清理不确定项。Server 在预留之前解析已启用的 Windows 目录入口，冻结 argv、插件根 cwd、受保护环境及已验证的来源展示上下文。调用输入只有选择器、有界上下文、可选 cwd 和普通启动环境，不能提供受保护环境或物理身份。UI 另按精确 Client／视图版本和来源成员准入一个 popup。来源身份复用完整 Client Presentation selection：已绑定 attachment generation／incarnation，或未绑定 selection generation；帧和滚动更新不改变成员身份。尺寸是有类型的单元格数或百分比；省略时先采用清单值，再采用半区域默认值。外框居中并限制在至少 6×4 单元格，原生边框内容宽高各减两个单元格。视口过小时暂停 popup 输入和尺寸调整，不把终端缩成零。

| 状态／转换         | 归属与守卫                                                                                              |
| ------------------ | ------------------------------------------------------------------------------------------------------- |
| 无到打开中         | 一次 UI 预留；精确来源／视图围栏、足够区域、无冲突对话框；Server 连接在一次启动派发前预留所有者         |
| 打开中到活动       | 关联且已知的立即启动，以精确创建引用附加，获取控制权，接纳可绘制的有界投影                              |
| 活动输入／调整尺寸 | 独立串行通道，精确 attachment／controller／frame 围栏；按键和粘贴只到 popup，鼠标使用最近完整绘制的矩形 |
| 终端结束           | 只读观察精确物理生命周期；安排一次关闭，不把缺帧或断连当作进程结束                                      |
| 显式关闭／断连     | 先撤销输入；Server 将关闭责任交给既有 Session 维护通道                                                  |
| 已知释放           | 完整 Worker 代际、binding fence 和 Host 身份；已知物理释放后才删除 popup binding 和所有者               |
| 启动或关闭不确定   | 保留有界 Server 所有者并停止重放变更；不宣称清理成功，也不在同一连接启动替代 popup                      |
| Client 退出        | 取消并回收独立通道；连接析构把已接纳的终端交给 Session 清理                                             |
| Session 退出       | 既有物理 Session／Worker 关闭收尾保留的不确定所有者；状态不持久化，也不伪造恢复                         |

`plugin pane open --placement popup [--width CELLS|PERCENT%] [--height CELLS|PERCENT%]` 使用清单尺寸默认值，可由参数覆盖。popup 布局在产生效果前拒绝规范几何、target、Workspace 和 direction 覆盖。`popup close [--client ID] [--timeout MS]` 即使从无规范 Pane 的终端调用，也按 Client 定位。两者使用 `orchbee.popup.result` v1；通用 Client control 使用 v1。打开完成要求已接纳帧及完成绘制；关闭完成等待已知物理释放。没有 popup 时关闭返回 `hadPopup=false`，保留其他对话框。

私有 Workbench 插件协议提供连接归属的 popup 打开、生命周期读取和关闭动作及封闭的有类型结果。Open 需要 read、create 和 lifecycle 权限；Read 需要 read；Close 需要 read 和 lifecycle 权限。自然结束观察按精确独立创建引用附着，不占用独占生命周期关闭所有者。显式关闭先撤销输入并围栏 binding，Session 维护再提交 Force，通过有界 inventory 观察精确目标 exited／failed，最后提交 Dispose。Inventory 观察采用协作式五秒窗口，每次遍历最多八页、最多 32 个生命周期步骤；进行中的 Worker 交换仍有独立传输界限。只有已知 Dispose 才释放保留的 binding。启动或关闭回执丢失时保留不确定状态，不重放变更；UI 显示重启 Session 提示并阻止替代 popup。启动前已知拒绝允许重试。Rust／TypeScript 对端需一起部署；不宣称 Herdr RPC 名称或 Desktop 功能相同。不引入新 Host 能力、依赖、进程框架或持久化存储。

定向验收覆盖重复准入、来源／目录拒绝时不启动、规范布局不变、丢失启动／关闭答复、完整身份释放、连接退休、尺寸、已解码按键／粘贴／鼠标归属、实际 Ratatui 组合绘制和原生 Windows CLI 传输。测试拥有其根目录、管道、线程和 Job，直到最后消费者结束。定向 fixture 证据不代表已部署 Host／终端模拟器验收；最终集成需通过更广的 Windows 验收流程。
