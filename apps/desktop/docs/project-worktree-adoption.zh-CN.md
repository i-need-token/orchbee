# Project Worktree 接纳

## 权威与范围

本文档负责 Desktop 为当前 Workbench Project 发现并接纳已有 Git Worktree。Workbench 负责
Project、Directory、Workspace 和 terminal context 语义；Desktop Main 负责 Git 观察、时点校验和
canonical document 提交。Electron renderer 只接收有界的显示名称和不透明身份，绝不接收
repository 或 Worktree 路径。

发现和接纳不会执行 Git mutation。创建、删除、移动、prune、修复或 fetch Worktree，持续监听 Git
状态，以及移除已接纳的 Worktree Directory 均不在范围内。托管 Worktree 创建仍由
[`project-worktree-creation.md`](project-worktree-creation.zh-CN.md) 负责。

## 领域契约

| 字段 | 契约                                                                                                                            |
| ---- | ------------------------------------------------------------------------------------------------------------------------------- |
| 问题 | 用户打开新建 Workspace 时，可以看到 Project 所代表的每个 Git repository 中已有的 Worktree，并直接从其中一个创建 Workspace。     |
| 发现 | 一个当前 Project ID 和可选的首选 Directory ID，按稳定 Project 顺序生成 repository 分组与时点 Worktree 选项。                    |
| 分组 | 每个 repository 显示在首选排序后第一个代表它的 Project Directory 下；多个 repository 保持为独立分组。                           |
| 排除 | repository 当前的源 Worktree，以及已由任一 Project Directory 表示的所有路径，不出现在其发现选项中。                             |
| 接纳 | 一个 Project、源 Directory、repository 和不透明 Worktree 身份，在时点重新校验后唯一选中一个 Worktree。                          |
| 成功 | canonical Project 包含一个 `source: discovered` Worktree Directory 和一个引用它的活跃 Workspace，或者复用等价的当前 Directory。 |
| 失败 | 过期的 Project、源、repository、Worktree，不可用路径、无效 descriptor 或 document conflict 均不改变 Git 或 canonical document。 |
| 取消 | 关闭菜单不会调用接纳；renderer 退役会抑制响应投递，但不会取消 Main 已开始负责的 canonical commit。                              |

## 发现读取模型

`DesktopProjectWorktreeDiscovery.repositories` 保持为创建功能的 repository catalog，并为每个
repository 增加一个 Worktree inventory。可用 inventory 包含有序条目，且条目只有不透明稳定身份和
显示名称。不可用 inventory 会被显式表达，绝不会伪装成空列表成功。显示名称使用本地 branch 名；
Detached Worktree 使用路径最后一段。绝对路径、Git common directory 和原始命令输出仍由 Main 持有。

Main 只解析当前可用的 Project Directory。它观察 repository 身份，读取
`git worktree list --porcelain -z`，规范化存在的 Worktree 路径，拒绝 bare 和 prunable 记录，并按
Git common directory 的规范路径去重 repository。发现不会执行 fetch、prune 或文件系统 mutation。
每次重新打开 Project 菜单都会请求新的时点 inventory；产品不承诺持续同步。

## 接纳 transition 与工作流

Workbench 持有可序列化的 `project-worktree-adopted` transition。它要求一个可用的源 Directory、一个
有效可用且标记为 `source: discovered` 的 `worktree` Directory、repository 一致性，以及唯一的 Directory
身份与路径。它以原子方式追加 Directory 与一个 Workspace、展开 Project、激活 Workspace，并发出
一个以该 Worktree 为根目录的 terminal context 创建命令。

Desktop Main 按以下顺序执行接纳：

| 阶段   | 所属动作                                                                                                         | 失败结果                         |
| ------ | ---------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| 解析   | 重新读取 canonical Project、源 Directory 和 repository 身份                                                      | 无影响                           |
| 重校验 | 重新列出 Worktree，并用不透明身份匹配唯一一个可用规范路径                                                        | 无影响                           |
| 决策   | 复用当前等价 Directory，或构建一个已校验的 `source: discovered` descriptor                                       | 无影响                           |
| 提交   | 对等价 Directory 应用 `workspace-created`，或对新 Directory 通过 canonical owner 应用 `project-worktree-adopted` | 一次原子 canonical document 提交 |
| 发布   | 发布已提交 document，并返回 Directory 与 Workspace 身份                                                          | 已提交结果保持有效               |

canonical coordinator 串行化提交。接纳与托管创建共享 repository 串行化，使并发创建不会被误认成
所选 discovery 条目。由于接纳不改变外部 Git 或文件系统状态，因此无需 compensation ledger。

## 不变量与生命周期

- Renderer 接纳命令只携带 discovery 返回的身份；不能提交源路径或目标路径。
- 不透明 Worktree 身份由 repository 身份与规范 Worktree 路径派生，只要该注册关系稳定，身份就稳定。
- Project 绝不会获得重复的 Directory 身份或路径。如果该路径在 discovery 后已经被表示，接纳会基于
  等价的当前 Directory 创建 Workspace。
- Detached Worktree 的 `branch: null`；其精确 `HEAD` 保持为权威元数据。
- 一个 renderer 同时最多持有一个 Worktree discovery、creation 或 adoption 请求。
- 删除已接纳的 Workspace 会保留其 Worktree Directory，且绝不改变外部 Git Worktree，与普通
  Workspace 删除行为一致。
- 回滚此能力不需要 durable data migration：已接纳的 Directory 与 Workspace 记录已经满足 Workbench
  shared document schema，并会继续可用。

## 验证

Workbench 确定性测试覆盖原子接纳 transition、Detached metadata、repository 一致性、重复拒绝、
terminal context 物化和等价 Directory 复用。协议、客户端、broker、runtime 与真实临时 Git repository
测试覆盖不透明边界、多 repository 分组、porcelain 解析、当前路径与已表示路径排除、过期选择拒绝、
canonical commit 失败和 renderer 退役。组件与真实 Desktop 交互检查覆盖菜单刷新、分组、直接创建
Workspace 和子菜单右侧定位。证据不涵盖持续文件系统观察和外部 Git mutation。
