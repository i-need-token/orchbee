# Windows 可复用布局

原生 Windows CLI 负责可移植布局导出，以及规范 Workbench 成员、Client 视图状态和只读终端
观测的组合。固定 Herdr 参考为 `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的
`src/app/api/layouts.rs`（R053/R054）。

## 导出

`layout export [--tab <target> | --panel <target> | --current] [--client <id>]
[--timeout <milliseconds>] [--format text|json]` 在两种格式下成功时均输出
`orchbee.layout.export` v1 JSON；失败遵循请求的文本／JSON 格式。总超时默认为 30,000 ms。
Ctrl+C 退出 130，并释放临时 Viewer 和已准入会话。

优先解析显式 Client，其次使用已验证调用者唯一的 current Client，最后使用唯一 current Client。
即使显式指定 Tab 也要求 Ready Client：比例、缩放和焦点归该 Client 所有。缺失或歧义观测直接失败，
不猜默认值。省略目标时使用已验证调用者 Panel 或所选 Client Panel。显式目标忽略调用者目标元数据。
Tab 接受精确 ID、后缀选择器或唯一标签；Panel 接受精确 ID 或后缀选择器。导出 Panel 实际导出
其所在 Tab。操作不激活 Tab、不聚焦 Panel、不调整 Host 尺寸、不获取终端控制权。

结果包含 `layout:{workspaceId,terminalContextId,tabId,zoomed,focusedPanelId,root}`；独立 Tab 的
`workspaceId` 为 null。分支含 `type:split`、`direction:right|down`、`ratio`、`first` 和 `second`。
叶子含 `type:panel`、描述性 `panelId`、可选手工 `label`、观测 `cwd`、包含可执行程序的可选
`command` argv，以及空 `env`。缺少 cwd 表示进程样本未提供目录。默认 shell 启动省略 command。
显式命令来自不可变的已准入启动选择，包含已删除的 profile；绝不从进程命令行文本重建。
不导出环境变量值。

最多准入 24 个叶子和 16 层（根层级为一），输出不超过 1 MiB。所有分支身份、有序 slot 和
叶子绑定必须匹配捕获的规范树，不得默默丢弃 child Presentation。未跟踪启动事实、过期拓扑、
不可用观测和不支持的结构均失败，不输出部分成功文档。Source 记录 Site／修订和观测 Client／
视图修订；该视图修订是约束读取请求的预检观测值，并非之后发布的修订。终端 cwd 是顺序观测，并非同一个原子进程快照。最终规范修订检查拒绝交错的成员变化。
导出的 ID 不授权 apply 复用已有进程。

## 应用

`layout apply (--file <path> | --stdin) [--tab <target> --confirm | --workspace <target> |
--standalone] [--label <label>] [--client <id>] [--focus | --no-focus]
[--timeout <milliseconds>] [--format text|json]` 新建 Tab 或替换指定 Tab。
省略目标时使用所选 Client 上下文。替换必须显式确认；导出标识不选择目标，也不复用终端。
比例及最终焦点要求 Ready Client。默认不聚焦；替换该 Client 的活动 Tab 时选择新 Tab。
显式标签覆盖被替换 Tab 的原手工名称；自动编号仍保持自动。

在运行时产生效果前，完整读取并校验文件或继承的非交互 stdin。限制为1 MiB，可带 UTF-8 BOM，
最多24个叶子、包含根在内16层。接受原生导出封装、包含 `root` 的布局描述或裸可移植节点。
Herdr 的 `type:pane` 和描述性 `pane_id` 是原生 `type:panel`／`panelId` 的可接受别名。
描述标识及导出的焦点／缩放仅作描述；模板创建全新、未缩放的终端。整棵树中的重复／未知操作字段、
错误嵌套值、无效目录、空命令、无效环境覆盖和越界均拒绝。分割比例必须有限，限制到0.1..0.9。
每个叶子可按已有启动契约指定手工标签、cwd、原样 command argv 和环境覆盖。产生效果前解析程序名，
不求值 shell 语法；回执不打印环境值。根 cwd 默认跟随被替换 Tab 记忆中的聚焦终端，否则使用
所选上下文的终端／默认目录。子叶省略 cwd 时跟随分割来源；显式 cwd 优先。

规范成员、启动结算和 Client 布局沿用既有归属。工作流在同一总期限内记录每个请求和已知效果。
先创建全部终端并等待 Started，设置标签和精确分支身份／比例，校验新树就绪后才关闭旧 Tab。
自有新树与原替换目标都受并发变更约束。先确认所需焦点，再提交破坏性 Close，避免根据自动导航
修复猜测焦点。Close 已接受不等于完成：观测到保留的 Closed 状态且 Client 发布达到该修订后
才报告替换成功。最终观测再次检查自有拓扑、模板比例与标签。后续失败
保留已完成阶段、请求标识、创建资源标识和恢复状态，不盲目重放变更。

新树就绪前，确定失败只能回收精确且未被改动的自有 Tab。并发加入／编辑或不确定结果保留资源，
返回明确的不完整回执。恢复使用原超时内预留的时间，不延长总期限。新树就绪后，焦点、替换关闭
或最终观测失败均保留可用新树。旧树在显式 Close 提交前保持可用；Close 已接受或结果不确定时
如实报告，不称为已回滚。Ctrl+C 完成已知、有界恢复后退出130，保留不确定效果。
通用生命周期事件继续由自动化能力负责。

结果为 `orchbee.layout.apply` v1，包含 completed/incomplete 结果、阶段、新旧 Tab 标识、
保留的规范与 Client 回执、恢复状态、最终布局及结构化失败，不包含环境变量值。
`api schema` 目录 v75 收录该结果及 `orchbee.cli.failure.layout-apply` v1。
文本失败保留本地化的输入来源诊断。

## 归属与证据

确定性投影负责拓扑边界、精确分支匹配和可移植叶子字段。Windows 适配器负责准入、Client 选择、
临时 Viewer 清理、期限、取消和最终修订检查。Server 继续独占成员／启动事实，Client 继续拥有
视图；不引入持久格式或新依赖。

确定性证据覆盖方向／顺序、非默认比例、标签、原样 argv、空环境、缺失／多余／重复事实、过期
分支 slot 和结构边界。Windows 验收覆盖实际 cwd 与冻结程序、活动／非活动及缩放 Tab、多 Client、
焦点／进程身份保持、取消与 schema 校验。这些检查不证明崩溃恢复、插件持久化或通用事件投递已完成。

Apply 证据覆盖完整输入校验、重复字段拒绝、阶段顺序、逐阶段失败边界、精确归属及最终布局检查。
打包后的 Windows 验收覆盖 BOM 文件与继承管道、嵌套及平衡布局、比例截断、原样／空／中文参数、
真实环境变量传递、非活动标签页记忆目录与子窗格跟随、两个独立 Client、活动／非活动替换、
自动标签、缩放重置、后续启动失败后原 Tab 保持、有界回收、并发编辑、阻塞输入、期限和控制台取消。
替换成功前必须同时观测 Closed 及 Client 发布。保留的不确定效果表示工作未完成，不能当作补偿成功。
