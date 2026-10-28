# Orchbee Workbench Client

[Orchbee 原生应用](../../README.zh-CN.md) 的私有 Workbench Client 运行时。仅该应用消费此
crate。

## 边界

`orchbee` 二进制是入口层。它拥有进程入口、命令行语法、CLI 文案与机器输出、Agent doctor 诊断、
启动与发布安装，以及持有宿主终端的终端 shell 循环和 backend。此 crate 是这些入口所组合的
Client 运行时。它从不依赖二进制，从不解析调用，也从不写入命令输出。

所有实现模块都是私有的。[`src/lib.rs`](src/lib.rs) 通过 `application` 门面按名称显式重导出
应用生产代码实际使用的条目，门面的子模块与实现模块路径一一对应。门面从不重导出整个模块，
也不使用 glob。应用未直接使用的条目都保持 crate 级或更窄的可见性。

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

## 维护

此 crate 继承仓库工具链、lint 和共享依赖版本。在仓库根目录遵循
[Rust 所有权与测试规则](../../../../docs/zh-CN/RUST.md) 和
[仓库验证门禁](../../../../docs/zh-CN/VERIFICATION.md)。
导出的模块和条目必须对应真实应用消费者，并保留私有测试的归属。

<!-- documentation-pairing: english-sha256=dd7a77c0e7e1d0f26f37585e02df320cec73511ef97874ea586cdfd249e36e2a zh-cn-sha256=7b16511019e8860cfd230c2c0fc2c46e3eac2ec1a1c6694fe9befd3df2851c02 -->
