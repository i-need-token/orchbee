# 应用模块分层

状态：`orchbee` 二进制 crate 已采纳的分层与横向依赖契约。当前代码已符合；
`pnpm lint:rust-modules` 会报告新出现的环和指向高层的引用，但不会因此失败。

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
| `agent_doctor`         | `workflow`     | 基于有界文件系统事实的离线 provider hook 诊断；`fs` 是它唯一的 I/O 边界。它返回带类型化提示的类型化报告；文档、提示文案和输出归 `cli`。                          |
| `agent_doctor::probes` | `values`       | Agent doctor 中纯分类的那一半；宿主路径风格由调用方传入。                                                                                                        |
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

## `cli` 内部的顺序

`cli` 的每个子模块都继承 `composition`，只能靠无环规则排序。以下不变量维持这一顺序：

- 结果模型（`results`、`output`、`handles` 和 `terminal_observation`）从不引用命令 owner。由
  owner 基于其他结果值组装、并自行渲染文本的查询文档，以 `results::OwnedDocument` 的形式进入
  `results::Projection`。
- `present` 只渲染结果值。需要命令 owner 类型的命令文案留在该 owner；例如 mutation 的 `Receipt`
  自行渲染。
- `query` 只做单步的读取、解析和交换，从不调用命令流程。把读取与 Client 注意力、输入提交或插件
  激活组合起来的流程放在各自的同级模块中，例如 `agent_wait`、`agent_list` 和 `terminal_run`。
  它们共用的辅助放在更低的同级模块中，例如 `link_activation`、`plugin_preparation`、
  `layout_topology` 和 `client_control::exchange`。
- 多个 owner 共同报告的失败 schema 标识位于 `invocation::grammar::schemas`。

## `invocation` 内部的顺序

`invocation` 的每个子模块都继承 `values`，只能靠无环规则排序。一次解析自上而下经过以下各组：

- `argv` 准入参数信封后调用 `machine_routing`，后者包装 `scan`；`scan` 执行词法扫描并调用
  `dispatch`；`dispatch` 把命令词映射到功能解析器。
- 功能解析器拥有各自命令的操作数，从不调用 `dispatch`、`scan`、`machine_routing` 或 `argv`。
- 共享的选项解码器（`option_scope`、`target_syntax`、`launch_options`、`workbench_options`、
  `wait_options`、`send_input`、`agent_options` 和 `server_stop`）从不调用命令的功能解析器。
- `help` 从 `grammar` 推导帮助路径、帮助请求和用法选项候选；`grammar` 从不引用 `help`。
- `failure` 持有失败词汇及其构造函数，不引用任何同级模块。根模块持有类型化请求词汇和
  `Options` 扫描状态。

## 维护

添加、删除、重命名或移动模块时，在同一变更中更新它的行。运行
`pnpm lint:rust-modules -- --details --member apps/orchbee` 可列出每个环和每个指向高层的引用及其
源码位置样本；[Lint](../../../docs/zh-CN/LINTING.md) 拥有该命令。

<!-- documentation-pairing: english-sha256=f43b3f1786cd5a96e3c29d5d94e18fc64fa739c3c39e3ee7ffb02ff15ef585b6 zh-cn-sha256=edab90657c78f8ef77039c53ee1ea6c023f4ed705863b5c97f1a743c664bd7c9 -->
