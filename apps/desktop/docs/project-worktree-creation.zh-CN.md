# Project Worktree 创建

## 权威与范围

本参考拥有 Desktop 为现有 Workbench Project 创建一个托管 Git Worktree 的行为。Workbench
拥有 Project、Directory、Workspace 和 terminal context 语义；Desktop Main 拥有 Git、托管路径、
规范文档提交、补偿和崩溃恢复。Electron renderer 拥有本地化表单并调用类型化能力，但不会获得
文件系统或进程权限。

删除、移动、导入外部 Worktree、检出现有分支、自动 fetch、重写 base、用户可选存储位置和跨设备
路径同步均不在范围内。

## 领域契约

| 字段         | 契约                                                                                                                          |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------- |
| 问题         | 用户从 Project 创建隔离 Git Worktree，并通常获得使用该 Worktree 的 Workspace，同时不会把文件放在源仓库旁边。                  |
| Project 输入 | 一个当前 Project ID，以及从其可用 Directory 推导出的一个当前 repository 选择。                                                |
| Git 输入     | 一个现有本地 base ref 和一个新的有效分支名。                                                                                  |
| 产品选项     | `createWorkspace`，默认开启。                                                                                                 |
| 成功         | Git 包含新分支与 Worktree；规范 Project 包含一个 `worktree` Directory；可选 Workspace 引用该 Directory。                      |
| 取消         | 提交前关闭既不修改 Git，也不修改规范文档。提交后由 Main 持有操作直到 commit 或 recovery。                                     |
| 失败         | 类型化 discovery、校验、Git、文档、补偿和恢复失败显示本地化诊断，但不暴露原始 stderr。                                        |
| 空操作       | 过期 Project、过期 repository 选择、重复分支、不可用 base、重复请求或已退役 renderer 绝不转而操作其他 Project 或 repository。 |

## 身份与关系

- 一个 Project 可以包含来自零个、一个或多个 Git repository 的 Directory。
- Repository 身份由规范 Git common directory 推导，与 Project 身份无关。
- Main 分配一个不透明的托管 Worktree 身份。其路径是
  `<data-root>/worktrees/<worktree-id>`。
- 提交后的 Worktree Directory 具有 `source: created`、必需的 Git metadata，以及从托管
  Worktree 身份派生的稳定 Directory 身份。
- 一个可选的新 Workspace 引用该 Directory。已有及后续 Workspace 也可以引用它；Worktree 与
  Workspace 绝不是同一个领域对象。

## Discovery 与默认值

Main 只解析规范 Project 中已经存在的路径。它通过 Git 观察每个可用 Directory，按 repository ID
去重 repository，并返回磁盘上已经存在的本地 ref。Discovery 不执行 fetch，也不产生变更。

Renderer 优先选择属于目标 Project 的当前活动 Workspace 主 Directory，其次选择 Project 主
Directory，最后选择第一个发现的 repository。Base 默认是该 Directory 当前附着的分支；detached
状态使用精确 `HEAD`。新分支默认是第一个未占用的 `orchbee/workspace-N`，并作为首个获得焦点的
字段。只有 discovery 发现多个 repository 时才显示 repository 选择器，其浮层由产品自行渲染。
Base 组合框可筛选已发现的本地 ref，同时会原样接受用户输入的任意文本，包括 commit ID。Base 与
branch 可以编辑；优先级较低的 base 和只读托管位置排在新分支之后。预填表单有效时按 Enter 即可
提交。本期不提供应用级或 Project 级位置设置。

## 创建事务

| 阶段           | 所有者动作                                                                            | 失败结果                                               |
| -------------- | ------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| Resolve        | 重新读取规范 Project 和所选 repository                                                | 无副作用                                               |
| Validate       | 验证 base commit、新分支不存在、目标不存在且恢复状态就绪                              | 无副作用                                               |
| Prepare        | 在 Git 变更前持久化一条 pending operation 记录                                        | 无 Git 或产品副作用                                    |
| Git commit     | 以参数向量运行 `git worktree add -b <branch> -- <path> <base>`                        | 补偿本操作拥有的所有部分结果                           |
| Product commit | 通过 Workbench 拥有的 transition 构建下一文档，并经 Desktop 规范 coordinator 串行提交 | 删除已创建的 Worktree 与分支                           |
| Publish        | 发布已提交的规范文档并协调 terminal intent                                            | Git 与产品状态保持已提交                               |
| Finish         | 删除 pending operation 记录并返回成功                                                 | 保留的 marker 可恢复，且绝不把已提交工作重新标记为失败 |

Workbench transition 原子添加 Directory 与可选 Workspace。Project 缺失、Directory 路径或身份
重复、Worktree descriptor 无效或源不可用时拒绝。`createWorkspace` 为 true 时，它还会分配一个
terminal context 和以新 Worktree 路径为根的 provision intent。

## 补偿与恢复

Git 变更前，Main 持久记录 operation ID、repository、分支、base、目标和 phase。Git 成功但规范
产品提交失败时，Main 只删除精确注册、分支仍指向已验证 base，且 tracked、untracked 与 ignored
文件均未改变的 Worktree；随后只删除本操作拥有且未变化的分支。Main 不执行递归文件系统 fallback。
已变化或所有权不明确的路径或分支会保留以供恢复，绝不冒险删除用户数据。

启动时在加载规范文档后检查保留记录。目标已经被规范 Project 引用的记录视为完成，可以清除；
否则 Main 重试相同的有界补偿。补偿失败时保留记录、阻止冲突的新操作，并显示一条本地化的
recovery-required 诊断；原始路径与 Git 输出不会进入产品文案。

## 并发与生命周期

- 一个 renderer 同时至多拥有一个 discovery 或创建请求。
- Main 按 repository 身份串行创建；不同 repository 可以独立进行。
- 规范文档 coordinator 把产品变更与 renderer proposal 串行化。
- Renderer 退役会抑制延迟 response 投递。Main 一旦接受创建命令，就会持有操作直到产品提交或补偿，
  即使请求它的 renderer 已经退役。
- 规范提交后的响应投递失败绝不回滚 Git 或产品状态。
- 原始命令输出和绝对源路径仅保留在所有者诊断中，绝不进入产品文案或 telemetry。

## 验证

确定性 Workbench 测试覆盖仅 Directory、Directory 加 Workspace 的原子 transition、repository
一致性、terminal context 物化和拒绝不变量。协议、client、broker、Git adapter、coordinator 和恢复
ledger 测试覆盖边界校验、参数安全、可编辑默认值、含糊的分支所有权、已变化与 ignored 文件、规范
提交失败、补偿失败、renderer 退役后的 response 抑制，以及重启恢复。临时真实 Git repository 在
无网络条件下验证命令细化。

已注册 TLA+ 模型把状态空间限制为两个 repository，且每个 repository 只有一个 operation state。
它检查产品必须依赖 Git、成功时两侧 owner 一致、部分 Git 结果具有持久恢复记录、response 只出现在
commit 后，以及 Main 自有效果公平时最终进入终态或保留恢复状态。模型抽象了 Git 命令、文件系统
原子性与表单行为；确定性 adapter 和 UI 证据覆盖这些细化。
