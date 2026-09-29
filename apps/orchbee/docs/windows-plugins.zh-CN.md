# Windows 原生插件

## 目录模型

原生插件目录属于用户共享配置，独立于 Site 生命周期。Windows 的 `plugin link PATH`、`unlink ID`、`enable ID`、`disable ID`、`list [ID]`、`config-dir ID` 和 `action list [ID]` 不启动 Site，直接使用此目录。链接仅登记清单，不执行 build 或 startup 命令。解除链接仅移除登记，保留源文件和用户配置/状态目录。

local-site-runtime 插件边界负责有界 TOML 校验和持久目录。目录参数优先解析 `orchbee-plugin.toml`，仅当其缺失时使用 `herdr-plugin.toml`；也可直接指定清单文件。导入 Herdr 元数据不代表兼容 Herdr 专用可执行文件或 API 调用。标识符去除首尾空白，区分大小写，最多 120 个 ASCII 字符；插件 ID 允许字母、数字、冒号、点、下划线和连字符，action ID 不允许点。参数保留为独立 argv 元素。未知字段、重复条目 ID、无效命令和无效平台名称均在登记前拒绝。

清单最多 128 KiB，目录最多 32 项且不超过 1 MiB。条目保留规范源路径、启用状态和最后一份有效清单。读取会刷新清单；源缺失或无效时保留检查元数据并明确警告，不能据此授权执行。条目平台列表优先于插件列表；均缺省时允许 Windows。其他平台标签仅为元数据。

## 持久化与转换

目录存储位于已准入的用户配置父目录下的 `plugins`；复用 Windows 私有缓存的祖先路径校验、独占写锁、落盘后原子替换和中断写入清理。只读操作不创建目录。修改在锁内重新读取，校验完整新目录，仅在实际变化时递增安全整数 revision。损坏、竞争和容量上限均返回错误，不当作空目录。解除不存在 ID 的链接不产生变化；启用不存在的 ID 失败。带前缀和哈希的 ID 路径组件使配置/状态目录稳定，并避开 Windows 保留名。解除链接后保留用户目录。

## 运行时 action 与日志

`plugin action invoke ACTION [--plugin ID] [--client ID] [--context JSON]` 向已有 Site 提交普通 action ID 或 `plugin.action` 完整 ID。`plugin log list [ID] [--plugin ID] [--limit 1..200]` 查询最新保留日志，默认 50 条。list 和 action-list 也支持用 `--plugin` 替代位置过滤参数。均支持 `--format text|json`，运行时命令不启动 Site。上下文是封闭的 camelCase 对象，包含可选 workspace/tab/Pane 展示字段、worktree、选中文字、调用来源、关联 ID、点击 URL 和链接处理器 ID；编码总量最多 16 KiB，选中文字最多 8 KiB。上下文不携带授权。省略调用来源时使用 `cli`；关联 ID 默认取本次调用的操作 ID。

调用在同一个十秒期限内先推导上下文。显式 Client 优先于继承的调用者身份；否则从规范快照中的精确调用者绑定选择终端成员，或由唯一当前 Client 选择观察到的成员。多个当前 Client 需指定 `--client`；没有调用者且没有当前 Client 时按全局动作调用，不编造选区。Ready 和稳定 Busy 视图可读。补充信息期间规范成员、元数据、Client 选区及成员代际必须保持一致，否则不提交动作。运行时 cwd 与 Agent 观察属于该精确成员；该成员显式上报的 Agent 名称及非 unknown 状态优先，否则采用唯一观察到的 provider 及聚合状态；缺失的观察保持空值。工作区 cwd 优先采用观察到的 Pane cwd，再取工作区目录。Worktree 使用现有只读查询，不绕过信任，保留原生字段 `workerId`、`repositoryRoot`、`commonDirectory`、`repositoryName`、`checkoutPath`、`linked`；不可用时保持缺失。显式非 null 上下文字段覆盖推导的展示字段，但不选择或授权任何目标。API 调用不抓取 Client 选中文字，调用者显式提供 `selectedText`，与参考 API 一致。

Server 在解析调用前要求读取、终端创建和生命周期权限。确认 Site、action 唯一且源有效、已启用、支持 Windows 后，固定 argv、源 cwd 和环境。Worker 在插件根目录执行独立参数；显式 shell 命令保留自身 shell 语义。相对程序从插件根目录解析，再使用 Windows PATH/PATHEXT。原生上下文通过 `ORCHBEE_PLUGIN_CONTEXT_JSON` 及插件 ID/root/action/config/state 路径、`ORCHBEE_BIN_PATH`、可选 workspace/tab/Pane ID 和链接字段提供。受管 Worker 用自身 Site 路由覆盖 runtime base/scope。这些是 Orchbee 变量和 API 语义，Herdr 专属脚本可能需要调整。安装本次字段前会清除继承的插件/终端身份。

每个 Worker 生命周期最多拥有 32 个并发命令和 200 条日志。每个命令保留线程及 Windows Job，在 CLI 断开后继续运行。保留期内相同操作 ID 返回原回执，不同意图产生冲突；已淘汰日志不构成持久重放记录。CLI 在回复不确定时不重新提交修改。启动回执只确认受理，不确认执行成功。日志包含序号、起止时间、退出码、类型化失败、stdout/stderr 和显式截断标记。每路原始输出保留 64 KiB，超出后继续排空；无效 UTF-8 用替换字符解码。文本输出转义控制字符。优先淘汰最旧的已完成日志，运行中的命令保留归属。命令结束后提供输出。

运行时日志在同一个十秒 CLI 截止时间内按每页四条读取，并固定 Worker/generation 和递减序号游标。新增命令不会推移后续页面；Worker 变化会拒绝旧游标。调用使用 `orchbee.plugin.runtime` v1；历史使用 `orchbee.plugin.logs` v1，通过一个展平的 `logs` 数组按所选最近记录的时间正序返回，默认 50 条，与参照一致。内部游标分页仍为倒序。两者遵守常规 8 MiB 输出上限；旧 runtime schema 继续用于历史回执。日志查询将进程失败作为数据返回，不因此让每次历史查询失败。私有 get/cancel 请求指向准确 Worker 和 job ID。取消及 Site 停止会清理根进程和后代、排空输出并等待线程结束；Job 清理不确定时阻止新执行，重试不能将未知清理变成成功。没有隐藏辅助窗口、脱离归属的管道读取线程或任意命令时长限制。

## 托管源码安装与移除

`plugin install owner/repo[/subdir] --yes [--ref REF] [--timeout MS]` 不需要 Site，要求 Windows PATH 中有 Git。`--yes` 显式授权执行下载源码中的构建命令；原生 CLI 不提示确认或读取终端。执行期限默认 600000 毫秒，范围 1..3600000；中断或超时后，活动 Windows Job 另有最多五秒的清理预算。`plugin uninstall plugin-id|owner/repo[/subdir]` 移除登记及托管源码，保留用户配置/状态和本地链接源文件。两者均支持 text 或 JSON 输出。

共享私有目录在有限的 checkout/build/commit/cleanup 流程内持有写锁，读取仍可继续。新代目录先登记，再创建于 `plugins/managed/i-<operation-id>/checkout`。Git 初始化目录，获取一个分支/标签/提交（默认 HEAD），检出确切提交并记录哈希。生效的 Windows build argv 从 manifest 根目录执行。禁用 Git hooks 和符号链接检出；构建以当前用户运行，不宣称具备沙箱。构建子进程继承用户工具环境，移除 `ORCHBEE_*` 运行时上下文并禁止交互式 Git 凭据提示。同步 CLI 拥有每个 Job、有界输出读取和中断；契约不允许这些进程在所有者退出后继续存活。

构建后 manifest 必须保持不变。本地链接 ID 拒绝替换；托管替换保留启用状态。只有原子替换目录登记后新源才生效。构建失败保留原登记并移除暂存源。已提交的替换或卸载将旧代加入清理列表；清理失败不会撤销提交，也不能被当成提交前拒绝。清理根据生成的 ID 推导路径，固定私有祖先并拒绝重解析点、外部所有权或硬链接。下次显式 install/uninstall 恢复记录的清理，不重新执行之前的构建；进程创建前先持久化可能的物理所有权，仅在确认清理完毕后解除；进程运行时崩溃或物理所有权不确定时禁止自动清理。当前目录引用（包括显式链接进某一代的本地源）阻止删除该代。unlink 保留源文件，不安排清理。

存储 v2 保留托管 GitHub 来源和有界恢复列表。v1 目录仍可读取，并在修改时迁移；旧二进制拒绝 v2，不丢弃其状态。`orchbee.plugin.registry` v2 增加可空的托管来源，v1 schema 保持封闭。`orchbee.plugin.provision` v1 报告操作身份、阶段、已提交变更、失败命令/退出码/输出、截断及未清理路径。源码安装/更新提交与构建产生的外部副作用分开，删除 checkout 无法回滚外部副作用。退出码 7 表示执行超时，8 表示物理状态不确定，130 表示中断，其余安装/卸载失败使用 1。

## 交付与证据

原生已接通目录、action 调用、保留日志及托管源码安装/卸载。startup/event hook、链接处理器及插件 Pane 生命周期仍需集成，之后才能认定插件完整对齐。目录接受不代表命令已执行。参照固定 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`，主要为 `cli/plugin.rs`、`app/api/plugins/manifest.rs` 和 `persist/plugin_registry.rs`。

定向证据覆盖解析上限、标识符/路径碰撞、源不可用时保留条目、缺失目录只读、目录原子转换及真实 Windows CLI 回执。测试夹具拥有并回收临时路径。完整 Windows 视觉/provider/多 Client 与全量回归验收另行执行。
