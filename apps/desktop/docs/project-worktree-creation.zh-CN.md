# 项目 Worktree 创建

## 权限和范围

Desktop 从已有 Workspace 创建一个托管 Git Worktree。Workbench 拥有 Workspace、可选 Project 成员关系、启动资源命名空间和终端结构；Main 拥有 Git、托管路径、规范提交和崩溃恢复。渲染器拥有本地化表单及类型化 capability，不拥有文件系统或进程权限。不包括 checkout 删除、移动、fetch 和用户选择存储位置。打开已有 Worktree 见 [project-worktree-adoption.md](project-worktree-adoption.zh-CN.md)。

## 领域契约

| 字段       | 契约                                                                                  |
| ---------- | ------------------------------------------------------------------------------------- |
| 输入       | 精确源 Workspace 和 Directory ID、已观察仓库 ID、base ref、新分支及 `createWorkspace` |
| 命名空间   | 源资源通过不可变 `resourceScopeId` 解析；GUI 文档原始 Directory ID 并非全局唯一       |
| 成功       | Directory、可选 Workspace、首 shell 和当前分组成员关系原子提交                        |
| 无 Project | 创建 Workspace 时新建一个包含来源和结果的组；仅创建 Directory 时不改变分组            |
| 重新分组   | 在规范提交时读取来源当前成员关系，不把旧分组当成执行权限                              |
| 来源关闭   | 保留已知 Git 结果；不复活来源或目标 Workspace                                         |
| 失败       | 类型化验证、Git、文档、保留结果或恢复失败；文案不含原始 stderr                        |
| 取消       | 提交前无效果；接纳后即使渲染器退役也由 Main 结算                                      |

## 身份与关系

一个 Project 可以含多个仓库，同一仓库可以跨 Project。Git 身份来自规范 common directory，与分组无关。Main 在 `<data-root>/worktrees/<worktree-id>` 分配不透明 Worktree ID。其 `source: created` Directory 含仓库、分支和 base 元数据。Workspace 身份与物理 checkout 身份不同。打开已关联 Worktree 会聚焦既有 Workspace，而不是重复分配。

## 发现和默认值

Discovery 读取源 Workspace 资源 scope 内可用本地 Directory，优先所选 Directory，再用 scope 回退项，按 Git 仓库去重并列出本地 ref，不 fetch。仅必要时渲染仓库选择器，base 默认为分支或 detached HEAD，建议可用的 `orchbee/workspace-N` 分支。新分支首先获焦，base 可编辑，托管位置只读。分组标签绝不限制 Git 身份。

## 创建事务

| 阶段 | 所有者操作                                                       | 失败结果                  |
| ---- | ---------------------------------------------------------------- | ------------------------- |
| 解析 | 捕获精确 Workspace、资源 scope、context、源 Directory 和路径     | 无效果                    |
| 验证 | 检查仓库、base、新分支、托管目标及恢复状态                       | 无效果                    |
| 准备 | 持久记录待处理操作身份和 Git 权限                                | 无 Git 或产品效果         |
| Git  | 使用参数向量执行 `git worktree add -b <branch> -- <path> <base>` | 先观察部分结果再决定恢复  |
| 产品 | 重新验证来源身份，在规范队列应用当前分组和首 shell               | 保留已知创建的 Worktree   |
| 发布 | 发布规范结构，既有协调器启动已提交终端意图                       | 产品和 Git 事实保持已提交 |
| 完成 | 清除待处理记录，返回精确 Directory/Workspace ID                  | 记录清理失败不反转成功    |

重新分组或解散来源组不会使源执行身份失效。改变 scope、context、Directory/路径或关闭来源才会。发布不会替其他客户端选择新 Workspace。发起客户端的最新激活意图等待精确结果及发布；较新导航会取消该意图。

## 补偿与恢复

严格版本 `1` 操作日志独立于分组记录 `sourceWorkspaceId`、`sourceResourceScopeId` 和 `sourceTerminalContextId`，字段必须显式存在。仅保存历史 Project ID 的形状被拒绝，不推导缺失身份。日志文件名保持稳定；无效或重复记录视为不可用，原始字节不变。

`git-created` 或 `retained` 结果绝不因规范提交失败、来源移动/关闭或渲染器退役而自动删除。Main 持久记录 `retained` 并返回 `worktree-retained`，用户之后可选择已有 Worktree。启动时跨分组清除已提交目标，保留已观察到的创建目标。保留记录不阻止其他 Worktree 操作。接纳在规范提交后清除对应保留回执。

仅无已观察 checkout 的未确认准备，才可通过适配器的精确所有权检查补偿自身未变残留。它检查分支/base、已跟踪、未跟踪和忽略文件，绝不使用递归删除路径作为回退。观察失败或归属不确定则保持 `recovery-required` 并阻止冲突操作。最后一个实际恢复消费者负责日志清理。

## 并发和生命周期

一个渲染器一次接纳一个 Worktree 请求。Main 按物理仓库身份串行；规范协调器把结构提交与其他提案串行。响应投递失败不会撤销 Git 或产品状态。异步准备中来源组变化使用当前成员关系，关闭来源返回保留的部分结果，绝不复活。

## 验证

Workbench 测试覆盖原子分组、scope 资源身份、首 shell 分配与稳定选择。Desktop 协议、broker、client、runtime 和日志测试覆盖类型化边界、当前组竞争、来源关闭、规范提交失败、保留的重启记录和跨组重新打开同一 Worktree。既有 Git 适配器测试使用临时仓库验证参数安全和精确残留归属。不宣称持续文件系统观察或原生窗口焦点行为。
