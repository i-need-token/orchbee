# Windows 可复用布局

原生 Windows CLI 负责可移植布局导出，以及规范 Workbench 成员关系、Client 视图状态和只读终端事实的组合。
固定 Herdr 参考为 `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 `src/app/api/layouts.rs`（R053／R054）。

## 导出

`layout export [--tab <target> | --panel <target> | --current] [--client <id>]
[--timeout <milliseconds>] [--format text|json]` 成功时两种格式均输出 `orchbee.layout.export` v2 JSON；
失败遵循请求的格式。默认总超时为 30,000 ms。Ctrl+C 以 130 退出，释放自有临时 viewer 和已接纳会话。

优先选择显式 Client，否则选经过验证的调用方唯一当前 Client，否则选唯一当前 Client。
要求 Ready Client，因为比例、缩放和焦点来自该 Client 的观察。观察缺失或有歧义时失败。
省略目标时使用经过验证的调用方 Panel 或所选 Client Panel；显式目标忽略调用方元数据。
Tab 接受身份、后缀或唯一标签；Panel 接受身份或后缀。导出 Panel 会导出其所在 Tab。
导出不激活 Tab、不改变焦点、不调整 Host，也不获取终端控制权。

结果包含 `layout:{workspaceId,terminalContextId,tabId,zoomed,focusedPanelId,root}`。
独立上下文的 workspaceId 为 null。分割包含 `type:split`、`direction:right|down`、`ratio`、`first`、`second`。
Panel 包含 `type:panel`、描述性 `panelId`、`rightClick`、`primary` 和有序 `children`。
成员包含描述性 `presentationId`、回退 `name`、可选手工 `label`、`start` 及空 `env`。
已绑定成员还包含可获得的 cwd 观察和不可变显式命令 argv；默认 shell 启动省略 command。
未绑定成员为 `start:false`，既不读取终端，也不会在应用时启动默认 shell。不导出任何环境值。

最多允许 24 个 Panel、含根节点的 16 层、每个 Panel 的 16 个子成员及 1 MiB 文档。
Split 身份、有序槽位及所有已绑定主／子成员必须匹配捕获的树。运行时事实按精确 Presentation 身份及绑定关联。
启动事实未知或运行时观察不可用时失败，不输出部分成功。来源记录 Site／修订及预检使用的 Client／视图修订，
不是后续发布修订。顺序读取的 cwd 不是原子进程快照；最终规范修订检查拒绝期间发生的成员修改。

## 应用

`layout apply (--file <path> | --stdin) [--tab <target> --confirm | --workspace <target> |
--standalone] [--label <label>] [--client <id>] [--focus | --no-focus]
[--timeout <milliseconds>] [--format text|json]` 创建新 Tab 或替换所选 Tab。
省略目的地时使用所选 Client 上下文。替换需要确认；导出身份不用于选择目的地或复用进程。
要求 Ready Client 提供比例和最终焦点。默认不聚焦；替换该 Client 的活动 Tab 时选择新 Tab。
显式标签优先，否则保留旧自定义 Tab 标签或保持自动编号。

产生副作用前完整读取文件或继承的非交互 stdin，接受 UTF-8 BOM，遵守导出限额。
接受 v1／v2 原生导出信封、含 root 的描述或裸节点。旧平铺叶节点及 Herdr `type:pane`／`pane_id`
规范化为一个需要启动、使用数字回退名称的主成员。结构化成员保留自己的回退名称、标签及启动意图。
描述中的身份和焦点／缩放仅用于描述，新布局初始不缩放。混用平铺启动字段和结构化成员、重复／未知字段或
畸形嵌套子成员均无效。不启动的成员不能指定 cwd、command 或非空 env。比例须有限并限制到 0.1..0.9。
创建前验证全部 cwd、字面 argv 和环境覆盖，并解析所有拟启动程序，不执行 shell 语法。环境值不进入回执。

首先通过一次 `tab.create-layout` 事务创建完整未绑定树，立即具备标签、有序成员及右键策略。
随后通过精确 `panel.create-terminal` 命令只绑定需要启动的成员，先主成员再子成员。
缺省根 cwd 跟随被替换 Tab 记忆焦点所在的主成员，否则跟随所选上下文主成员／默认目录。
分屏主成员跟随来源主成员 cwd 或继承目录；子成员跟随自己的主成员或同一继承目录。
未绑定主成员不借用子成员运行时。显式 cwd 优先。全部拟启动成员必须达到 Started，才能就绪并关闭旧 Tab。

整个流程在一个总期限内保留每个请求和已知副作用。绑定回执只允许所拥有树中精确成员从未绑定变为已绑定，
不吸收并发的拓扑或标签修改，同时为原替换目标设围栏。设置精确 Split 身份及比例，再验证就绪。
关闭旧 Tab 前须确认所需焦点。Close 接纳不等于完成：报告成功前须观察保留的 Closed 状态及 Client 发布。
最终观察再次检查拓扑、标签、成员顺序／启动意图、输入策略及比例。

就绪前，确定失败只可关闭完全未变的自有 Tab。并发修改或未知结果保留资源和明确的不完整回执。
恢复使用原总超时中的预留部分。就绪后焦点／Close／最终观察失败保留新布局。
旧树在提交 Close 前仍可使用；已接纳或未知 Close 不得称为已回滚。Ctrl+C 完成有界已知恢复并保留不确定副作用。

结果为 `orchbee.layout.apply` v2：完成／不完整、阶段、新旧 Tab 身份、最多 448 条有界副作用回执、恢复、
结构化最终布局及失败，遵守原生 8 MiB 输出上限。最终导出仍受自身 1 MiB 限额约束；超限产生就绪后的
不完整回执，保留新 Tab 及副作用记录。布局副作用只记录动作、Site、请求、基准修订及已验证结果，不记录启动参数或环境。
目录 v98 描述两种 v2 输出和保持不变的 layout-apply 失败 v1。文本诊断继续本地化。
分组／固定往返和完整已保存焦点恢复仍是独立工作。

## 所有权与证据

纯模板／投影负责形状、限额、成员意图及无身份重建。Server 负责规范成员和启动；Client 负责视图观察。
Windows 适配器负责精确绑定关联、单次提交、所有权围栏、取消和清理。不引入持久迁移或依赖。
私有契约要求使用匹配 cohort，不能混用发布版本。

定向测试覆盖新旧输入、子项／未绑定往返、字面启动数据、就绪前各失败边界、精确绑定变化、陈旧元数据及响应关联。
受控 Windows CLI 对端测试覆盖真实进程分派和回执，不代表真实 provider 或交互 Client 验收。
完整 Windows 打包／TUI、provider、多 Client 验收及全量回归仍属于最终集成义务；旧 v1 验收不能证明改动后的 v2 流程。
