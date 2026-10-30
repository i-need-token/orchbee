# orchbee-agent-integration

托管 Agent 集成身份，以及原生 Client、Server 中继与节点本地写入方之间交换的语言中立操作契约。本
crate 只拥有值与编解码器；它从不检查或修改 provider 配置。

## 身份

`IntegrationId` 命名 20 个托管集成。契约文档用 provider registry id（`provider()`，例如
`antigravity`）拼写集成，这是 wire 上唯一的 provider 词汇。`as_str()` 保留托管 payload 拼写（例如
`antigravity-cli`），它从不出现在契约文档中。

## 契约

- 文档是 UTF-8 JSON 对象：字段用 camelCase，`kind` 标签用 kebab-case，不允许未知字段。
- `IntegrationRequest` 为 `{operation, adapterSource?, budgetMilliseconds?}`。`operation.kind` 为
  带可选 `provider` 的 `list`，或带必填 `provider` 的 `explain`、`enable`、`disable`。出现的预算为
  1 到 40,000 毫秒。编码后的请求最多 8 KiB。
- `IntegrationReport.kind` 为 `list`（条目与 capsule 状态）、`explain`（条目、检查详情、capsule 状态
  与计划步骤）、`mutation`（一个变更报告）或 `home-unavailable`。编码后的报告最多 32 KiB。
- 封闭词汇（状态、问题、可用性、管理方式、来源、Marketplace 与目标状态、capsule 来源类型、文件系统
  步骤操作、裁决、拒绝与失败代码）只解码为已知成员。
- Capsule 身份是 `payloadSha256`。
- 节点路径无损：Unicode 路径用 `{"text": …}`，其他 Unix 字节用 `{"unixBytesHex": …}`，其他 Windows
  代码单元用 `{"windowsUtf16": […]}`。路径最多 4,096 个单元，从不包含 NUL，并且只在产生它的平台族上
  重建。

## 载体

[OBCN Agent 集成 job](../capability-node-protocol/README.zh-CN.md#agent-集成-job-消息)
在 Server 与 Worker 之间把编码后的文档作为不透明字节携带。
[Workbench Agent 集成方向](../workbench-protocol/README.zh-CN.md#agent-集成-job)
在 Client 与 Server 之间把它们作为 JSON 值嵌入。

## 验证

`src/contract/tests.rs` 覆盖 provider registry id、预算与未知字段拒绝、无损节点路径和封闭报告词汇。

<!-- documentation-pairing: english-sha256=9a7f1eb055b2fd9a4b607c2f4a394d3189c45383a09706c6fdb9df653fb25a18 zh-cn-sha256=5be5410e422368182f3bf02600f1095cf6d30fc0ac1ee7a786583fbb642206f7 -->
