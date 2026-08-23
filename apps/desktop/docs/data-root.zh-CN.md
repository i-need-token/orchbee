# Desktop 数据根目录

## 权威与范围

本参考拥有 Desktop 产品数据根目录与开发实例隔离。数据根目录统一限定 Electron
`userData`、单实例锁、Terminal Host 身份、恢复状态、本地偏好设置和托管 Git Worktree 的默认位置。
它不定义同步产品状态、用户可选存储位置、从早期 Electron 管理位置迁移数据或自动删除。

## 解析

| 环境     | 数据根目录                           |
| -------- | ------------------------------------ |
| 打包环境 | `<home>/.orchbee`                    |
| 开发环境 | `<home>/.orchbee-dev-<scope-digest>` |

开发环境在哈希前解析规范的源码 checkout 根目录。Scope digest 是对包含平台规范化真实路径的带版本
identity 执行 SHA-256 后所得小写十六进制结果的前 16 个字符。版本标记可以显式管理未来的 identity
迁移，但进程 CWD 和嵌套 package 路径不会进入 identity。开发启动器显式提供 checkout 根目录。

每个开发数据根目录都包含一个有界的 scope marker，其中保存完整的规范 checkout 路径和 digest
版本。启动时只有 marker 一致才接受已有目录；不一致时关闭失败，绝不打开其他 checkout 的状态。
checkout 移动后会获得新的数据根目录，旧目录保持不动。

内置托管 Worktree 根目录是 `<data-root>/worktrees`。每个新 Worktree 使用 Main 分配的不透明身份
作为直接子目录名。Project ID、repository ID、base ref、分支名和展示名绝不决定物理路径。

## 启动顺序与所有权

Desktop Main 在请求 Electron 单实例锁之前解析并创建数据根目录。它先把 Electron `userData`
设置为该目录，使磁盘状态与锁身份由同一个 scope 控制。因此同一个 checkout 只允许一个 Desktop
实例，而打包版 Desktop 与不同开发 checkout 可以并行运行。

在 POSIX 上，Main 以仅所有者可访问的权限创建产品拥有的数据目录。Electron renderer 可以收到固定的
托管 Worktree 根目录作为表单中的只读文案，但不会收到源 repository 路径、数据根目录枚举、位置覆盖
或通用文件系统能力。Gateway 客户端不获得托管 Worktree 能力。测试使用显式注入的临时 home 与源码
根目录。

## 不变量

- 打包环境与开发环境绝不解析为同一路径。
- 相同的规范开发 checkout 路径解析为相同的数据根目录。
- 被接受的开发根目录 marker 精确记录用于计算 digest 的路径。
- 不同路径不能共享一个被接受的根目录，包括截断 digest 发生碰撞时。
- checkout 变更或移动绝不删除或迁移旧根目录。
- Worktree 是持久用户代码而非缓存；任何重置或清理操作都不得隐式删除它。
- 本切片只有固定 Worktree 根目录规则。应用级与 Project 级覆盖保持不存在。

## 验证

确定性 resolver 测试覆盖打包与开发路径、规范路径稳定性、平台规范化、marker 创建与不匹配，以及
不同 checkout 隔离。Desktop 配置测试验证选定的 `userData` 根目录进入托管 Worktree 与恢复路径；
Main 入口会在请求单实例锁之前选择 `userData`。
