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

最多准入 24 个叶子和 16 层深度（根深度为零），输出不超过 1 MiB。所有分支身份、有序 slot 和
叶子绑定必须匹配捕获的规范树，不得默默丢弃 child Presentation。未跟踪启动事实、过期拓扑、
不可用观测和不支持的结构均失败，不输出部分成功文档。Source 记录 Site／修订和观测 Client／
视图修订；该视图修订是约束读取请求的预检观测值，并非之后发布的修订。终端 cwd 是顺序观测，并非同一个原子进程快照。最终规范修订检查拒绝交错的成员变化。
导出的 ID 不授权 apply 复用已有进程。

## 归属与证据

确定性投影负责拓扑边界、精确分支匹配和可移植叶子字段。Windows 适配器负责准入、Client 选择、
临时 Viewer 清理、期限、取消和最终修订检查。Server 继续独占成员／启动事实，Client 继续拥有
视图；不引入持久格式或新依赖。

确定性证据覆盖方向／顺序、非默认比例、标签、原样 argv、空环境、缺失／多余／重复事实、过期
分支 slot 和结构边界。Windows 验收覆盖实际 cwd 与冻结程序、活动／非活动及缩放 Tab、多 Client、
焦点／进程身份保持、取消与 schema 校验。这些检查不证明布局 apply、崩溃恢复、插件持久化或
通用事件投递已完成。
