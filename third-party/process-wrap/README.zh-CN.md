# process-wrap 依赖修正

本目录保留 MIT／Apache-2.0 许可的 `process-wrap` 10.0.1 registry 源码
（上游提交 `ca45003a831ac125e6673b8759430e5ef33cc1db`），原许可证与版权声明同目录保留。
它属于第三方依赖，从项目自写 Cargo 工作区排除，Orchbee 不发布此包。

原生 Client、Server 和 `orchbee-local-site-runtime` 使用已有的 `std`、`job-object`、
`creation-flags` 特性。Windows Worker 命令消费该共享运行时。
根 patch 固定此修正版源码，不增加第二个 Windows 绑定版本。manifest 保留上游
依赖和特性图，仅增加 `publish = false`；不复制上游测试夹具和开发产物。unsafe FFI
仍是上游实现；Orchbee 修正没有新增 unsafe 块，也不改变项目自写代码的 unsafe 策略。

## 完成契约

上游 Windows 完成端口读取器会把任意 Job 通知当作完成。修正版等待私有 Job 端口的
`JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO`（代码 4），传播 API 错误，并在无关通知之间
保持原有超时预算。标准子进程只有收到该事件才缓存退出状态；非阻塞轮询在根进程和
整个 Job 都退出前返回 `None`，随后读取保留结果，不再消费完成通知。

修正前上游 `src/windows.rs` 的 SHA-256 为
`76214e10c89af6589af8fc9c2ae1d4c319596a07b2a2945afbc4a23b1d52a2b9`。
运行时只修改此文件及 `src/std/job_object.rs`。其他源码原样保留，不代表接纳新的运行时特性。

## 维护与证据

共享运行时在
[`windows_command_child/tests.rs`](../../crates/local-site-runtime/src/windows_command_child/tests.rs)
中拥有原生完成证据：保留存活后代的句柄，放行根进程或取消 Job，并要求返回时后代句柄
已发出退出信号。其失败夹具还证明，失败的 Job 等待不能在重试时变成缓存的根进程退出成功。
Worker 发现测试覆盖输出边界、取消和失败清理；原生 Client 集成测试
覆盖现有 provider 进程适配器。修改此补丁前运行受影响测试及[验证](../../docs/VERIFICATION.md)
中适用的根主机门禁。

更新时明确审核上游版本，对照完整源码差异、保留许可证，并重跑原生句柄判据。
当上游版本提供等价完成语义并通过这些测试时，删除本目录和根 patch。根进程退出、
成功发起终止或成员列表为空，都不能替代完成事件。
