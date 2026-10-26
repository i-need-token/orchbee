# Orchbee 本地客户端

[Orchbee 原生应用](../../README.zh-CN.md) 的私有 Client Connection 传输层。
仅该应用及其同级内部 crate 消费此 crate。

## 生产边界

[`src/lib.rs`](src/lib.rs) 列出导出边界。`LocalClient` 准入经过验证的对端，并使用调用者的
绝对截止时间和可选取消源提交类型化协议交换。Origin 保留连接身份和新鲜度，创建 lane，
并准入请求及订阅。客户端、origin、时钟、关闭句柄、订阅和保留的 carrier 将各自状态保持
私有。类型化回执与失败保留提交和结果的不确定性。

机器配置、SSH 进程创建、运行时发现、观测策略、CLI 输出和 Workbench 交互由应用负责。
此 crate 不选择功能，也不重放副作用。

## 生命周期与证据

传输层拥有取消、多路复用、连接健康、队列和 lane 关闭。
`RetainedCarrier` 接收已经启动的字节转发任务的停止标志和 join 句柄；最后一个 owner
停止并 join 该转发任务。连接关闭与逻辑 lane 关闭仍是独立操作。

相邻私有测试保留准入、分片、截止时间、类型化交换、取消、队列、关联、健康与清理证据。
其中的时钟、原始连接构造器、写前 gate 和 wake marker 工厂仅供所属测试模块使用。
应用行为测试必须消费生产边界。

## 维护

此 crate 继承仓库工具链、lint 和共享依赖版本。在仓库根目录遵循
[Rust 所有权与测试规则](../../../../docs/zh-CN/RUST.md) 和
[仓库验证门禁](../../../../docs/zh-CN/VERIFICATION.md)。
导出操作必须对应真实应用消费者，并保留私有测试的归属。

<!-- documentation-pairing: english-sha256=9ca3f764f9cfcccf99cbf7af2335d8987ae61e6a1961ef31eb472f05e3ab427a zh-cn-sha256=98f0da5df1070f5322da8c2f15da399c60211080f35795d348d9d8ba7b715c2d -->
