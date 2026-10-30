# 应用模块分层

状态：`orchbee` 二进制 crate 已采纳的分层与横向依赖契约。当前代码尚未符合；
`pnpm lint:rust-modules` 会报告剩余的环和指向高层的引用，但不会因此失败。

本文是[模块边界](../../../docs/zh-CN/MODULE-BOUNDARIES.md#原生源码树)要求二进制 `src/` 源码树
具备的 owner 本地契约。二进制只通过 `application` 门面消费
[Workbench Client](../crates/workbench-client/README.zh-CN.md)。

## 分层

二进制使用 [Workbench Client 分层](../crates/workbench-client/docs/module-layers.zh-CN.md#分层)
及其[依赖规则](../crates/workbench-client/docs/module-layers.zh-CN.md#依赖规则)。命令自下而上
流经这些层：`values` 声明语法、类型化请求和用法错误；`workflow` 模块执行命令副作用；
`presentation` 渲染类型化结果；`composition` 分发请求，并拥有进程退出、TUI 事件循环和启动接线。

## 模块归属

每行给出目标层。模块目前混合多个层时，说明会指出需要移走的部分。

| 模块                   | 层             | 职责与理由                                                                                                                                                       |
| ---------------------- | -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `agent_doctor`         | `workflow`     | 基于有界文件系统事实的离线 provider hook 诊断；`fs` 是它唯一的 I/O 边界。                                                                                        |
| `agent_doctor::probes` | `values`       | Agent doctor 中纯分类的那一半。                                                                                                                                  |
| `agent_integrations`   | `values`       | CLI 与 Session Worker 交换并渲染的 Agent 集成契约值。                                                                                                            |
| `agent_projection`     | `values`       | 为 Agent 输出确定性地连接规范 Session 与物理 Agent 实例。                                                                                                        |
| `cli`                  | `composition`  | 把已准入的请求分发给功能 owner，并组装 stdout、stderr、失败和进程结果。逐命令的副作用和文案移出到 `workflow` 与 `presentation` 模块。                            |
| `cli_outcome`          | `values`       | 类型化的 CLI 结果：失败模型及其稳定的种类标记、退出码和 JSON 信封，以及 `cli_text` 渲染的类型化结果值。结果在 owner 之间移动时，其标记、退出码和文档形状都不变。 |
| `cli_text`             | `presentation` | 基于 `cli_outcome`、`invocation` 和 Workbench Client 门面中的类型化值，生成人类可读的帮助、结果文案、文本转义和用法建议；从不反向引用 `cli`。                    |
| `help`                 | `presentation` | 帮助文档及其显示单元格布局。                                                                                                                                     |
| `invocation`           | `values`       | 声明的命令语法、类型化请求和结构化用法错误；解析是参数的确定性函数。                                                                                             |
| `local_startup`        | `composition`  | 接线本地激活、终端准入、启动等待和 TUI shell。                                                                                                                   |
| `native_path`          | `workflow`     | 平台路径拼写：文件系统路径的转义显示文本和精确的原生机器字段。                                                                                                   |
| `presentation`         | `presentation` | 启动、版本和用法输出投影。                                                                                                                                       |
| `release_store`        | `workflow`     | 基于文件系统的发布暂存适配器。                                                                                                                                   |
| `startup_wait`         | `workflow`     | 有界等待可呈现的终端几何，支持取消并恢复模式。                                                                                                                   |
| `tui_backend`          | `workflow`     | 宿主终端帧写入适配器。                                                                                                                                           |
| `tui_shell`            | `composition`  | TUI 事件循环、表面切换、输入轮询和绘制结算。                                                                                                                     |

## 维护

添加、删除、重命名或移动模块时，在同一变更中更新它的行。运行
`pnpm lint:rust-modules -- --details --member apps/orchbee` 可列出每个环和每个指向高层的引用及其
源码位置样本；[Lint](../../../docs/zh-CN/LINTING.md) 拥有该命令。

<!-- documentation-pairing: english-sha256=ba355233b0208ea71f47a9102c3ae720499c133d2ad4b303cd335f721ad9c2fd zh-cn-sha256=1f96142b069038dcdf31d36e5329330963d7cc35fa68e7e0135cc7617425b5a8 -->
