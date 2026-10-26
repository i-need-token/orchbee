# Wait-output 结算契约

`src/cli/query.rs` 负责一次 `terminal wait-output` 调用的期限、取消和清理结算。这是 Client 所有的顺序工作流，其传输完成可控，证据来自实现测试。

## 结算义务

- 在 preamble、attach 及每次 poll、feedback、行读取交换前采样共享取消/期限门。
- 成功 attach 创建一个精确 viewer 的清理义务；失败 attach 不能虚构清理义务。
- 在单步期限检查前消耗新读取序号；过期响应使用更新序号，耗尽时不得回绕。
- 仅在结算门通过后匹配一致且当前的窗口。
- Detach 记录实际清理来源。迟到标志采样可以修订主结果，但不能改写已观察的清理事实。
- 最多输出一次最终答复；效果不明不能触发自动重放。

## 证据与边界

`src/cli/query/tests/wait.rs` 执行生产结算决策。`tests/cli_query.rs` 使用脚本化 OBCS viewer 和真实 frame/row 记录，覆盖请求序号递增、过期重新固定、期限与清理结果。`tests/local_startup_process` 提供真实进程组观察及取消/槽释放证据。

运行所属 Cargo 测试。证据覆盖选定完成顺序和实际协议边界，不声明穷举独立 Server/Worker 调度或实时保证。共享 viewer 所有权与 relay 隔离由独立注册的 Server 协议负责。

<!-- documentation-pairing: english-sha256=028cf6e0e7caa56b7f38a11ddbadf5a542f0e901613086c338dbc847824bdfa9 zh-cn-sha256=a6d51ebe16693feb2849f6a4450c1b218faf0150dee43ffeacf4c8dd150ddf15 -->
