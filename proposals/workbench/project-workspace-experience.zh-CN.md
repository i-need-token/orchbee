# Project 与 Workspace 平台 Effects

| 字段     | 内容                                                                                                                                       |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| 状态     | 当前模型、浏览器预览、原生现有 Folder 选择和本地 shared-document 持久化已经实现；文件系统 mutation、Git、启动和跨设备 effects 仍是未来工作 |
| Owner    | Workbench 产品维护者与平台应用 owner                                                                                                       |
| 当前权威 | [`packages/workbench/docs/project-workspace.md`](../../packages/workbench/docs/project-workspace.md)                                       |
| 退出条件 | 实现分别接受的平台切片，把维护事实迁入对应 application 和 package 指南，然后删除本提案                                                     |

英文 [project-workspace-experience.md](project-workspace-experience.md) 是 canonical 版本；本文用于简体中文评审。

## 当前基础

`@orchbee/workbench` 已经拥有内存中的 Project、Directory、Workspace、活动位置、Tab、Panel 和
terminal-presentation 组合。浏览器预览支持不产生文件系统或 Git effect 的合成 Folder path。这些已实现
事实由当前权威维护，不在这里重复。

平台应用只能替换 Folder acquisition、normalized Directory observation 等 capability boundary，
不能接管 Workbench 产品模型。

## 原生 Folder acquisition

Desktop 现在已经提供有类型的平台 capability，用于选择一个现有 Folder。Electron Main 拥有原生
picker 和时间点目录校验，再把 host-normalized path 交给浏览器预览所使用的同一个 Project-create
或 Directory-add intent。当前策略和证据维护在
[`apps/desktop/docs/project-folder-acquisition.md`](../../apps/desktop/docs/project-folder-acquisition.md)。

- 初次创建 Project 时只获取一个 Folder。
- 取消、权限拒绝、路径无效和选择失败都不改变 Workbench state。
- 时间点文件系统校验完成后，才能提交 authoritative Directory。
- Folder 名称是默认 Project 名称；用户可以独立修改产品 metadata。
- Project 管理界面复用同一个 Folder-acquisition capability 继续添加 Folder。
- Workbench 不获得通用文件系统 primitive。
- 一个 renderer 最多拥有一个请求，renderer retirement 会撤销晚到 completion。
- Add Folder 提交前重新检查当前 Project 和 exact duplicate path。

当前切片会规范化绝对路径，在保留所选符号链接拼写的同时跟随目标完成目录与 read/traverse 校验，且不
执行需要补偿的文件系统 mutation。New Folder 仍是未来工作，必须另行定义目标路径策略、竞态、回滚与
恢复。可移动位置的持续可用性也不属于本次时间点选择声明。

## Git Worktree effects

Git observation 和 mutation 属于平台或 Git capability owner。Workbench 可以消费语义 kind 为
`worktree` 的已验证 Directory descriptor。

已经接受的产品方向是：

- 创建 Worktree 是 Project 级操作，并可选择同时创建使用该 Directory 的 Workspace；
- “同时创建 Workspace”默认启用；
- 删除 Workspace 默认保留其 Worktree；
- 删除 Worktree 需要用户明确选择；其他 Workspace、terminal CWD 或 Project 最后一个 Directory
  仍依赖它时必须拒绝；
- Git 删除成功后才能从产品 state 删除 Workspace 和 Directory；失败时保留两者并显示 owner-owned
  diagnostic。

实现前，Git owner 必须定义多仓库 Project 的仓库选择、base ref 与分支规则、目标路径策略、已有
Worktree 的处理、取消、回滚，以及 Git 成功但产品状态提交失败时的恢复。

## Launch Configuration 与剩余持久能力

Workbench 当前只建模 launch-configuration identity 和选择，不定义可执行 payload。真实执行之前，
后续 owner 必须定义 command、arguments、environment、secret reference、平台适用性、验证、执行失败
和复制语义。

Desktop 已经持久化 canonical shared Workbench document 和 durable terminal intent。超出这一
本地权威的扩展必须继续遵守 shared document 和 client view 的 ownership split。Client View
恢复、Directory 缺失对账、外部文件系统变化、跨设备同步和冲突处理需要单独接受的行为与恢复证据。

## 排除项

除已接受的现有 Folder picker 及其时间点校验外，本提案不授权文件系统 mutation 或 Git 实现，不向
renderer 暴露通用原生 capability，不包含远程 Folder 连接、自动删除源码数据、Workbench 中的 PTY
ownership 或跨设备同步。每个新增的带 effect 平台切片仍需各自的有限授权和验证。
