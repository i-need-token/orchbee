# Orchbee Workbench Client

[Orchbee 原生应用](../../README.zh-CN.md) 的私有 Workbench Client 运行时。仅该应用消费此
crate。

## 边界

`orchbee` 二进制是入口层。它拥有进程入口、命令行语法、CLI 文案与机器输出、Agent doctor 诊断、
启动与发布安装，以及持有宿主终端的终端 shell 循环和 backend。此 crate 是这些入口所组合的
Client 运行时。它从不依赖二进制，从不解析调用，也从不写入命令输出。

所有实现模块都是私有的。[`src/lib.rs`](src/lib.rs) 通过 `application` 门面按名称显式重导出
应用生产代码实际使用的条目。门面的每个子模块对应拥有这些条目的顶层实现模块；只有当该模块的
子模块导出同名条目（例如 `Error` 或 `read`）时，门面才保留一层嵌套子模块。门面从不映射更深的
实现路径，从不重导出整个模块，也不使用 glob。应用未直接使用的条目都保持 crate 级或更窄的
可见性。

## 内容

- Workbench Client：经过验证的 Session 激活、投影后的 Workbench 状态、mailbox 与连接 lane、
  视图、通知、输入分发、设置和窗格呈现。
- 终端呈现：继承终端准入与模式生命周期、投影、复制模式、链接、图形、主题、布局、指针交互和
  渲染。
- Agent 工作流：provider integration 检查与副作用、运行时观测，以及 Agent 启动。
- 机器：已保存机器的发现、选择器、SSH carrier 与隧道，以及观测。
- Client 本地配置、运行时文案本地化、通知、引导和终止信号。

## 依赖方向

应用依赖此 crate；此 crate 从不依赖应用 target。它依赖同级的[本地客户端](../local-client/README.zh-CN.md)
提供 Client Connection 传输，并依赖 [`crates/`](../../../../crates) 下的共享协议与运行时 crate。
当运行时行为需要入口层的值，例如退出码映射或 shell 绘制器时，由应用传入。

crate 内部模块遵循[模块分层](docs/module-layers.zh-CN.md)：每个模块所在的层、允许的引用方向、
状态归属和组件位置。
[TUI 组件](docs/tui-components.zh-CN.md)参考列出共享的基础控件和重复组合。

## 后台线程

Client 的每个后台线程都通过 `background_thread::BackgroundThread` 持有，遵循与 Worker 后台作业
相同的退出契约：

- `request_stop` 请求持有者停止，从不等待。
- `stop_before(deadline)` 请求持有者停止，只在线程已经结束后才 join。截止时间到达时仍在运行的
  线程报告 `TimedOut`，并继续归持有者所有。
- 丢弃持有者从不阻塞：它请求停止，join 已经结束的线程，并分离仍在运行的线程，由该线程自行结束。

进程退出时，终端 shell 先请求所有持有者停止，再等待其中任何一个：当前和缓存的 Client、机器连接
尝试与观察者，以及系统通知持有者。随后它以同一个截止时间 `EXIT_BUDGET`（10 秒）观察每个持有者；
这是持有者观察到停止请求后仍可能处于其中的最长截止时间。各停止请求同时进行，因此正常退出不会变慢。
截止时间到达时仍在运行的线程会被分离，退出报告清理失败，而不是成功。

只有进程退出设有这个总上限。重连和退役仍会 join 连接任务以取回待结算的结果，持有者显式的
`close` 保留其有界的 join。

在截止时间被分离的线程可能在进程结束时放弃以下工作：

- 进行中的 Agent 启动的取消和预留释放。预留可能保持待定，直到 Server 使其过期。
- 最后一次位置缓存写入。
- 一次 Windows 剪贴板投递。
- 关闭状态恢复的结果。

已保存机器的中继归相邻传输层的保留载体守卫所有，随该守卫停止。

## 测试支持

相邻的私有单元测试拥有运行时证据，仅被它们使用的 fixture 保持 `#[cfg(test)]`。
`test-support` feature 为应用拥有的测试导出 `test_support` 模块；这些测试归应用所有，是因为
它们会触达应用的 shell 循环、backend 或命令层：

- 供 shell 循环测试使用的假 Server 对端与原生 Workbench Client 构造；
- 供 backend 测试使用的终端生命周期 fake 与有序 wire 记录器；
- 接收应用真实绘制器或 backend、同时把断言保留在私有运行时状态上的设置与窗格图形场景；
- 冻结的 provider fixture 包及其故障注入接缝，它们接收应用的 integration 退出码映射。

根部的 `test_support` 门面只在 `test` 或 `test-support` 下存在，按名称显式重导出应用测试在生产
门面之外用到的条目。只给测试用的条目带有同样的门控。测试需要某个生产类型的 crate 内部成员时，
该成员保持 `pub(crate)`，由类型旁边带门控的 `<Type>TestAccess` trait 委托调用；测试从门面导入
该 trait。字段保持 crate 内部的值，由测试通过带门控的 `<Type>Fixture` 结构体经 `From` 构造。

只有应用的开发依赖会开启此 feature，并同时开启 `orchbee-local-client/test-support`。发布构建
和生产边界都不包含它，应用代码只消费生产边界。

## 基准

[`benches/frame_pipeline`](benches/frame_pipeline/main.rs) 通过 `application` 门面和公开的
协议 crate 测量 TUI 帧管线各环节的耗时，遵循[基准规则](../../../../docs/zh-CN/TESTING.md#基准)。
在仓库根目录运行：

```sh
cargo bench -p orchbee-workbench-client --bench frame_pipeline --locked
cargo bench -p orchbee-workbench-client --bench frame_pipeline --locked -- apply/delta --samples 100
```

| 用例            | 阶段   | 被测操作                                                                                           |
| --------------- | ------ | -------------------------------------------------------------------------------------------------- |
| `decode/*`      | 两者   | 对一个 OBTF payload 调用 `decode_terminal_text_frame`；`snapshot` 属于初始化，`delta-*` 属于稳态。 |
| `apply/init`    | 初始化 | 新建的 `TerminalProjection` 接收第一份完整快照记录。                                               |
| `apply/full`    | 稳态   | 活动投影接收下一份完整快照记录，与重新同步后的情形相同。                                           |
| `apply/delta-*` | 稳态   | 活动投影接收下一份增量记录：信封与路由检查、解码、行合并、行校验和 `FrameApplied` 反馈。           |

记录是确定性合成的 OBTF 帧，尺寸为 80×24、120×32 和 240×64 个单元格，内容包含 ASCII、双宽
CJK 字符和带样式的片段。增量替换一行（`delta-1`）、四分之一的行（`delta-quarter`）或全部行
（`delta-all`）。基准不包含传输读取、布局、绘制、缓冲区比较、后端编码和控制台输出；实际循环
由 [TUI 计时观测](../../docs/native-terminal-environment.zh-CN.md#tui-计时观测)覆盖。

每次调用单独计时，用例返回的状态在计时停止后释放；表头报告读取时钟本身的开销。每行按单次
调用报告：一次冷启动的首次调用，以及各样本的最小值、中位数、第 90 百分位数和中位数绝对偏差，
另附输入字节数和中位吞吐量。`--list` 列出用例；`--samples <n>`、`--sample-ms <n>` 和
`--warmup-ms <n>` 的默认值分别为 50、2 和 300；其他参数按子串选择用例。

## 维护

此 crate 继承仓库工具链、lint 和共享依赖版本。在仓库根目录遵循
[Rust 所有权与测试规则](../../../../docs/zh-CN/RUST.md) 和
[仓库验证门禁](../../../../docs/zh-CN/VERIFICATION.md)。
导出的模块和条目必须对应真实应用消费者，并保留私有测试的归属。

<!-- documentation-pairing: english-sha256=6c5bfaaffcc9d1cb6cf66c67f8b8066871df5818edd95fc572302936d7b581a0 zh-cn-sha256=2cbbf6164c5a637f75c10cee5b69eb1c14a6b055be3d9ac83ad4049bc99311ed -->
