# Windows CLI Workspace 创建

## 范围与兼容性

本记录拥有可选 Workspace 创建输入、首个 Project 设置及独立 Client 聚焦阶段。Server 拥有规范 Project／Workspace／Directory 身份和终端接纳；CLI 组合既有命令，不自行启动 shell。[原生设置交互](native-workspace-setup.zh-CN.md) 保留独立的 TUI 草稿和绘制规则。

既有有效的完整显式命令保留 mutation-v4 输出：`workspace create --project <target> --label <label>` 创建空 Workspace；加绝对 `--cwd` 则启动首个终端。没有新增选项时，这些旧形式行为不变。省略原有必填参数，或使用新增来源、Client、聚焦、超时、目录策略选项，会选择 Windows 增强启动流程。例如添加 `--no-focus` 可明确选择增强创建，同时不提交 Client 聚焦请求。Mac/Linux 运行时行为不变。

增强创建接受可选 `--project`、`--label`、来源 `--workspace`、精确 `--client`、互斥的 `--focus`／`--no-focus`（默认），以及 `--timeout`（1–3600000 毫秒，默认 30000）。既有 env、exec／args、profile 及终端尺寸四项全填的接纳规则继续生效。聚焦要求在任何变更前已有就绪 Client；绝不启动 TUI。

## 来源、目录与 Project 决策

显式路径、`home` 和 `current` 目录策略忽略无关的调用者／来源 Workspace 身份。路径使用既有 Windows 绝对、相对、波浪号归一化契约；解析器不做 shell 展开或文件系统探测。默认 `follow` 与显式 `workspace` 策略可使用来源 Workspace。显式 Client 优先于隐式调用者；否则经校验的调用者优先于唯一当前就绪 Client。无效／部分调用者身份、过期 Client 或歧义直接拒绝，不做猜测。无调用者和 Client 时允许无界面、不聚焦创建。

| 决策                                     | 结果                                              |
| ---------------------------------------- | ------------------------------------------------- |
| 显式 cwd 路径                            | 使用归一化字面路径                                |
| `home`／`current`                        | 使用已验证的 Windows 主目录／CLI cwd              |
| `workspace`                              | 要求来源 Workspace 的规范主目录                   |
| 默认／`follow` 且有已证明匹配 Panel      | 只采样其主终端根进程 cwd                          |
| 无匹配 Panel，或根进程 cwd 明确缺失      | 使用来源 Workspace 主目录，再回退 Windows 主目录  |
| 根进程缺失、cwd 错误、进程查询失败或中断 | 在创建 Project 前拒绝；不猜子进程或其他 Panel     |
| 显式目标 Project                         | 使用精确解析身份；来源和目标 Project 可以不同     |
| 无显式 Project，有已证明来源 Workspace   | 使用该 Workspace 的 Project                       |
| 无来源 Project，只有一个 Project         | 使用唯一 Project                                  |
| 无来源 Project，没有 Project             | 按捕获版本创建一个本地化的默认 Project            |
| 无来源 Project，多个 Project             | 拒绝歧义；不按标签选择，也不另建默认项目          |
| 未指定标签                               | 使用有界目录名；根目录使用本地化 Workspace 回退名 |

默认 Project 具有普通稳定 Project ID。标签不是角色、查找键或单例锁。并发创建改变版本，使过期尝试失败；CLI 不重试或悄悄采用另一 Project。持久目录策略、任意自定义 shell 的 cwd 跟踪及动态 Git Workspace 名称属于独立契约。

## 提交与聚焦模型

一次调用拥有准备好的目录及来源、可选选中 Client、可选 Project 回执、可选 Workspace 回执、可选聚焦回执、阶段和完整结构化失败。准备阶段在副作用前验证可获得的本地输入。元数据和控制交换限两秒；Workspace 启动使用剩余调用预算并接入既有信号唤醒。任何阶段遇到不确定性都不自动重提。

| 转换                              | 守卫、结果与恢复                                                                  |
| --------------------------------- | --------------------------------------------------------------------------------- |
| 准备                              | 解析来源、路径、标签、Project 选择；副作用前固定所需就绪 Client                   |
| 创建默认 Project                  | 只有无 Project 快照才接纳一次版本围栏 ProjectCreate；下一次读取前记录已提交身份   |
| Project 创建后刷新                | 要求同 Site、足够版本和精确新 Project；刷新失败仍保留回执                         |
| 启动 Workspace                    | 一次既有 WorkspaceLaunch 拥有目录探测及规范 Workspace／Directory／Tab／Panel 提交 |
| Project 或 Workspace 提交结果未知 | 保留完整请求／来源失败信息及先前回执；停止，不回滚或重放                          |
| Workspace 终端 pending 或 started | 保留规范 ID；可选聚焦通过既有绘制／已见所有者选中待启动占位界面                   |
| Workspace 终端失败、不可用或未知  | 保留规范 ID、停止聚焦并返回对应非零退出码                                         |
| 聚焦准备                          | 同 Site 新快照、精确 Project／Workspace 归属、足够规范版本和精确当前就绪 Client   |
| 用户选择守卫                      | 要求语义选择未变，或首次 Workspace 调整已选中精确新 Workspace／Tab／Panel         |
| 提交聚焦                          | 使用新视图版本提交一次 Context 意图，并等待既有已绘制／已见确认                   |
| 后续失败、超时或取消              | 保留先前提交及身份；取消停止等待，不取消 Server 已接受工作                        |

允许已选中新建目标的规则仅适用于 Workspace 创建。Worktree 聚焦仍采用更严格的选择未变守卫。派生面板数和普通规范刷新版本不是导航身份。观测到其他上下文／Tab／Panel 或缩放状态会停止聚焦。这些检查均不证明 Git／进程／界面的原子事务。

增强输出使用 `orchbee.workspace.creation` v1，分别记录 Project 设置、Workspace 变更、目录来源和 Client 副作用。已知失败保留原始结构化失败，包括不确定提交的请求身份。仅请求阶段全部成功才为完成；待启动终端仍明确为 pending。解析／准备失败使用既有失败封套。旧变更输出保持不变。

## Worktree 首个 Project 复用

相同的无 Project 准备用于 [Worktree Create／Open](windows-worktrees.zh-CN.md)。来源与聚焦接纳在 Project 创建前完成。既有 Project 推导及歧义规则不变。默认 Project 尝试使用 `orchbee.worktree.operation` v3，附独立设置回执／阶段／完整失败；无需设置的普通调用保留 v1／v2。已知 Project 创建保留于后续 Start 前失败、Git 失败或提交不确定之后。保留 Git 意图及 Server 续执行不变；可配置检出路径与删除失败恢复另有所有者。

## 证据与边界

确定性证据覆盖旧／增强语法、路径脱敏、零／单／多个 Project 选择、来源优先级、目录缺失与无效观测、标签及聚焦守卫。Windows CLI 脚本轨迹覆盖并发版本、Project 提交后失败、不确定交接且不重放、终端 pending／失败、首次 Workspace 调整、选择变化及超时。版本化 Schema 检查保留各阶段部分回执。夹具拥有并释放自己的配置目录、管道与进程。

参考语义为 Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34` 的 WorkspaceCreate 及焦点面板／初始目录／主目录创建策略。Orchbee 的显式 Project 歧义、精确 Client 路由和部分回执保留仍是可见适配。这些是有界同作者检查；完整双 TUI 验收独立进行，TLA 暂缓。匹配版本组整体发布；回退不会撤销已提交的用户 Project 或 Workspace。

组合回执的失败观测完整保留 `kind`、可空 `details`、可空 `source`、原始 `schema` 和 `version`。该嵌套记录由组合 Schema 拥有；无详情的本地守卫错误不伪装成要求详情的独立失败封套。API 目录 v64 含 73 个 schema，命令路径仍为 86 条。

没有更具体详情的创建预检错误使用通用 `orchbee.cli.failure` v2，仅接受 `workspace-creation` 阶段及缺失／不可用／协议／传输／超时／取消类别。原有 v1 失败矩阵保持不变。嵌套失败观测按各生产者的精确详情／来源约束校验；本地无详情守卫必须同时没有来源。
