# 项目 Worktree 接纳

## 权限和范围

Desktop 从当前 Workspace 发现并打开已有 Git Worktree。Workbench 拥有组织关系和终端结构，Main 拥有 Git 观察、路径验证和规范提交。命令携带源 Workspace、源 Directory、仓库及不透明 Worktree 身份，绝不接受渲染器选择的路径。Discovery 和接纳不修改 Git。创建见 [project-worktree-creation.md](project-worktree-creation.zh-CN.md)。

## 发现读取模型

Discovery 读取源 Workspace 不可变资源命名空间内可用本地 Directory。选项按物理 Git 仓库分组，与 Project 成员关系独立。一个项目可代表多个仓库，另一个项目也可以代表同一仓库。优先 Directory 顺序只影响标签。

Main 读取 `git worktree list --porcelain -z`，规范化已有路径，排除 bare/prunable 记录和当前来源，并按物理仓库身份去重。已表示本地 Worktree 从当前资源渲染；外部选项携带不透明稳定身份，以及分支或 detached 路径名称。不可用 inventory 明确呈现，不是空成功。重新打开 Workspace 菜单刷新时点清单，不 fetch、prune 或修改文件系统。

## 接纳转变和工作流

| 阶段     | 所有者操作                                              | 结果                                                  |
| -------- | ------------------------------------------------------- | ----------------------------------------------------- |
| 解析     | 捕获精确源 Workspace、scope、context 和 Directory/路径  | 无效果                                                |
| 重新验证 | 重新列出并匹配不透明 Worktree 身份及仓库                | 陈旧或不可用选择无效果                                |
| 决定     | 跨全部组与资源命名空间查找已关联物理 Worktree           | 已存在则返回既有 Workspace                            |
| 提交     | 在规范队列重新验证来源，应用 `project-worktree-adopted` | Directory、Workspace、首 shell 和当前成员关系一起提交 |
| 发布     | 发布并返回精确身份，清理匹配的保留创建回执              | 不强制其他客户端选择                                  |

已移动来源使用当前 Project。未分组来源与结果原子获得一个组；已关闭来源绝不重建。如果仅有等价 Directory，复用其身份，必要时升级文件夹描述。已关联 Worktree 跨组返回既有 Workspace，不分配第二个终端上下文。仓库身份不依赖组名或成员关系。

## 不变量和生命周期

接纳与创建共享物理仓库串行，与所有提案共享规范提交顺序。Detached Worktree 保留 `branch: null` 与精确 `HEAD`。规范提交失败不触碰外部 Git。关闭已接纳 Workspace 保留资源和 checkout。渲染器退役抑制迟到投递；发起客户端激活意图等待发布，较新导航会退役该意图。接纳效果本身无需补偿日志。

## 验证

纯 Workbench 与 Desktop runtime 测试覆盖 scope 身份、多仓库、当前组变化、等价 Directory 复用、陈旧来源拒绝和已移动 Worktree 重新打开。协议/client/broker 与 Git 适配器测试覆盖不透明边界、解析、规范提交失败和退役。Hook 与组件测试覆盖 discovery 菜单及结果/发布顺序，不允许迟到导航抢占。持续外部 Git 观察和原生窗口焦点不在这些证据内。

<!-- documentation-pairing: english-sha256=06652ae2edf314ec6b5d071402daa88bf1b3f27a263718ba2f177d6e7efc43f7 zh-cn-sha256=22aba2d2d63ae04daf5270187cceb0dd46313a03ae6769da38574d3cabd606a3 -->
