# orchbee-agent-integration-writer

节点本地 Agent 集成写入方：provider 检查、启用与禁用计划，以及它们的文件系统与进程效果，使用经
cohort 接纳的观察适配器 capsule 与保留的变更结果。值与报告词汇来自
[Agent 集成契约](../agent-integration/README.zh-CN.md)；本 crate 从不定义它们。

## 归属

- 一次调用在一个 40 秒协作预算内运行。调用方传入 `TerminationSource`；写入方在有界步骤之间观察它，
  并以 cancelled 结算。
- 支持 Unix 与 Windows。Windows 配置位置辅助函数只在 Windows 上编译。
- 实际消费方是 `orchbee-workbench-client`，它服务 CLI 的 `agent integration` 动词以及其他检查
  provider 配置的 CLI 路径。

## 一致性 fixture

`tests/fixtures/` 保存本 crate harness 重放的两套语料。`test-support` feature 暴露这两个重放，
CLI 测试以 CLI 的 integration 退出码映射运行它们：

- `provider-conformance/` —— `payloads.json` 加 `cases/<provider>.cases.json`：
  按 provider 组织的 inspect、enable、disable 案例，钉定脚本化命令回复、
  期望 argv、文件系统效果以及渲染后的 CLI verdict。
- `provider-effects/` —— `cases.json`：文件系统效果机案例——胶囊发布、
  move-aside、备份摘要、remove 遍历、cohort 与版本门、预算与取消边界。

两套语料都由 harness 经 `CARGO_MANIFEST_DIR` 解析，任何检出均可直接重放，
不依赖环境覆盖或本机路径。案例修改即语义契约变更，须与被钉定的引擎行为
在同一受审变更中提交；新增 provider 在 `provider-conformance/cases/` 下
增加一个文件。

<!-- documentation-pairing: english-sha256=4cbd51c34dd2716765b376c009d0b0cf57b2eafcafe2b162caf7c4d38bfc724b zh-cn-sha256=34f7ef7d0c450feacc805c21c0f18408e4cf61fc7a9b521578187fb7f2c35f5f -->
