# 原生 Tab 分组与能力覆盖

Windows 原生 TUI 投影 canonical 分组身份、名称、颜色、成员及固定状态。紧凑标签栏绘制独立组标题、
展开标记和成员数量。Tab 菜单提供固定／取消固定、分组及既有同 context Tab 移动；分组菜单提供创建、
加入、移出、重命名、颜色、解散和相邻组移动。键盘菜单导航和已绘制指针目标调用相同 owner。
配置后台 Tab 时，保持当前 Tab、Panel、Presentation、终端绑定及 viewer 选择。

## 实际 Host 能力比较

此表比较 Workbench 组合真正接入的路径，不把可复用 Tab Strip 的全部导出能力视为产品能力。
GUI owner 为 `WorkbenchTabBar`、`useWorkbench`、`getWorkbenchTabCreationAvailability`、
`WorkbenchProjectWorkspaceRuntime` 和 `TabModel`。原生列指 Windows TUI。这是一部分能力覆盖，
不代表 GUI／TUI 已完全对等。

| 核心操作                         | GUI 实际接入路径                                                                                 | 原生覆盖及剩余缺口                                                                       |
| -------------------------------- | ------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- |
| 功能菜单                         | Tab、分组、Panel 及 Workbench Action 菜单                                                        | 既有对象／全局菜单；Tab 菜单新增移动、固定及分组。表现与锚定按 Host 区分。               |
| 创建、重命名、关闭单个 Tab       | 终端 owner 创建／关闭；标题 action 修改 canonical 名称                                           | 既有 canonical 原生操作；分组／固定元数据仍可完整表达。                                  |
| 固定／取消固定                   | 选中 Tab 集合经 `TabModel.setTabsPinned`                                                         | 单个指定 Tab；在固定前缀边界移动，固定时移除组归属。缺少多选。                           |
| 用已有 Tab 建组                  | 选中集合经 `createGroup`                                                                         | 单个指定 Tab、Server 分配组身份、名称编辑器及颜色菜单。缺少多选。                        |
| 加入／移出组                     | 选中集合经 `moveTabsToGroup`                                                                     | 单个指定 Tab；加入时取消固定，清理源空组，保持分组连续。                                 |
| 重命名／改色／解散组             | 分组菜单经 `updateGroup`／`ungroup`                                                              | 支持完整 canonical 组及全部九种颜色。解散保留终端。                                      |
| 移动组                           | `shiftGroupRelative`、已接受的拖动 arrangement                                                   | 跨过前／后相邻组或未分组 Tab；不越过固定边界或循环。缺少指针拖动 arrangement。           |
| 折叠／展开                       | Client view 的 `collapsedGroupIds` 投影入 `TabModel`                                             | 标题点击或菜单；按身份保存于 Client 本地。原生展开状态不持久化，一次只保留一个 context。 |
| 组内新建 Tab／新建带 Tab 的组    | 本地创建路由可用；远端拒绝组内定位，并禁用新建组                                                 | 缺少原生菜单入口及编排。GUI 新建组本身先添加 Tab，再以第二次提交加入分组。               |
| 复制                             | 本地创建路由可用；远端禁用                                                                       | 缺少原生复制命令。GUI 在 index + 1 处添加带源元数据的单个 Tab，不复制运行中的拆分树。    |
| 关闭组／选中集合／其他或后续 Tab | `onCloseTabs` 将精确 ID 集合交给 `requestActiveTerminalTabsClose`；终端 owner 提供运行中资源确认 | 缺少原生批量／分组 Close 及完整集合确认／恢复。逐个 Close 不能替代该能力。               |
| 单个 Tab 跨组边界移动            | GUI model 同时更新 arrangement 与分组                                                            | 既有原生方向／索引移动拒绝拆散组或破坏固定前缀。可显式改组再移动，但手势语义不同。       |
| 移到其他窗口                     | 库回调／runtime factory 可选；当前应用 Host 未安装新窗口回调                                     | 不将其算作当前 Host 已接入的对等能力。未新增原生窗口迁移操作。                           |

方向、悬停卡和垂直栏偏好属于 Host 表现选择。以上批量选择、终端创建／复制和组关闭生命周期
仍是核心缺口；此实现不完成整个能力对等 issue。

## 共享权威与回执

`tabs.configure` 对已有 `terminalContextId` 执行一种封闭操作：`create-group`、`set-group`、
`update-group`、`ungroup`、`set-pinned` 或 `move-group`。既有认证 command／revision CAS 及
请求账本负责接入、重放及候选的原子发布。Server 仅在创建时分配组身份，并在 `tabs-configured`
中返回；其他成功变更的必填 `groupId` 为 null。无变化返回原版本的 `unchanged`。过期版本、
不存在目标、数量上界及 context 内任何未完成 Close 均在写入前拒绝。该操作没有终端或 Worker 效果。

原生适配器复用 setup 事务的队列、截止时间、已知／未知结果处理和快照恢复。它用 `Arc` 保留有界
canonical 输入快照，而非从渲染后的 Tab 重建可写文档。精确提交版本的回执必须等于预期完整快照，
包括其他 context、导航、树、名称及绑定。新组 ID 必须不存在于基线的任何 context。后续版本遵循
既有 canonical 刷新规则，因为 peer 可能已提交另一合法修改。投递结果未知会退役连接，绝不自动重试变更。

`configure_tabs` 是 authority 规划及回执检查共用的 Rust 纯转换。它保留每个已有
Tab／Panel／Split／Slot／Presentation 身份和终端绑定。组清理遵循 GUI `removeEmptyGroups`；
从仍存活的源组中间成员建新组时，将新组插在源组块之后，避免拆散源组。
`TabModel.moveTabsToGroup` 执行相同边界规则。这是 canonical 元数据，不是 TUI 独有的第二套分组模型或持久化路径。

折叠状态仍归 Client 本地，遵循 `packages/workbench/src/workbench/document/workbench-client-view.ts`
的权威边界。组标题展开操作不选中成员、不提交 command、不推进共享版本，也不从导航器隐藏成员。
组颜色使用用户选择的成对前景／背景色块；单色模式保留名称、数量、展开及当前标记。名称经过既有
终端文字清理及单元宽度裁剪。指针区域与标题、成员 token 从同一次绘制产生。

## 兼容性与验证

Rust、TypeScript 和 JSON command schema 共用新增 wire 用例。这是本地未发布 OBWP v1 的
匹配 cohort 扩展：Client／Server 一起启用或回滚；旧版封闭解码器会拒绝。它不新增 Desktop
Workbench command 或公开 CLI 语法。GUI 修改仍经既有 canonical 共享文档 proposal owner，
而非此原生适配器。

`schemas/workbench/v1/tab-configuration-conformance-v1.json` 同时运行于 GUI `TabModel` 和
Rust `configure_tabs`，覆盖固定／取消固定、固定边界、组加入／移出／创建、源组清理、元数据更新及
整组移动，包括从 `[a:G,b:G,c:G]` 的 `b` 建新组。独立 authority 测试覆盖原子
prepare／drop／commit、精确重放、过期版本及 Close 围栏。原生 wire、敌意回执和 UI 测试覆盖关联、
新 ID、无关变动、选择、完整绘制门、键盘交接及指针展开／菜单。

完整 TestBackend 矩阵覆盖两种语言、120×40、80×24、80×20、40×10、20×5，以及
native／浅色／深色／降色／单色主题、展开／折叠分组及固定 Tab。临时完整屏幕证据可通过将
`ORCHBEE_TUI_CAPTURE_DIR` 指向已有任务临时目录，再运行
`cargo test -p orchbee tab_groups_composed_screen` 取得。它输出标准与紧凑尺寸默认主题的实际
buffer 单元，包括分组菜单。调用任务负责这些捕获，在最后一个审阅者使用完后删除。
Buffer 证据不能证明物理字体／调色板表现或实际 Desktop／原生跨 runtime 集成。
