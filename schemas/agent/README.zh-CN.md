# Agent 规范一致性语料库

## 状态

[`v1/agent-canonical-conformance-v1.json`](v1/agent-canonical-conformance-v1.json) 是一份
**已执行的等价性语料**，不是契约 schema，也不是公共入口权威。其中的预期由运行
`packages/agent` 在所记录基提交处的权威 TypeScript 实现产生；它存在的意义是让
`apps/orchbee-server` 中的阶段性 Rust 移植能够证明行为等价。该夹具格式为私有——
这里的任何内容都不构成 wire、CLI 或 provider 侧语义。

## 语料所钉住的内容

- 29 个向量覆盖全部九种语义状态、freshness 转换、attention 请求/解决与 turn 域自动
  清除、turn 结果、会话结束与启发式存在退场、stale/disconnected/结束后行为、sequence
  去重与 generation 栅栏、再观察幂等、展示附件的附加/摘除/可见性冲突、parentage、
  能力与来源门控、日志分页/保留/重置、UTF-16 规范排序、聚合与位置投影，以及运行时
  销毁。
- 每个向量记录其有序的 `events[]` 与逐事件预期转换（`kind`、拒绝时的 `reason`、
  转换后 `revision`、有序的 `semanticEvents`）、完整的终态会话表（含
  snapshot/summary/status/explanation/`matchedWaitConditions`）、`journalReads[]`，
  以及——当计算时——`aggregate` 与 `location` 投影结果及其**已记录输入**，使消费者
  可以精确重放。
- `events[].programGenerated` 标记 v24 保留程序的生成事件；输出的向量已内嵌全部
  520 个生成事件，消费者逐字重放文件，不需要再展开模板。
- `journalReads[].onEmptyRuntime` 为该读取选择一个独立的全新运行时；变更层没有
  `baseline` 类别（其上的操作层负责合成）。
- `aggregate` 或 `location` 为 `null` 表示"该向量未计算"——消费者跳过断言，而不是
  期待一个计算出的 null。

## 来源与再生成

夹具内的 `provenance` 记录基提交与 `sourceFingerprints`：生成语料时
`packages/agent/src` 每个文件的 SHA-256。再生成由 `packages/agent` 负责，并导入所属
包的构建产物——绝不 vendored TypeScript 源码拷贝：

```sh
pnpm -F @orchbee/agent build
pnpm -F @orchbee/agent conformance:generate   # 重写夹具
pnpm -F @orchbee/agent test:conformance       # 内存中再生成并逐字节比对（漂移检查）
```

若 TypeScript 权威或基提交变化，check 运行会报告漂移；此时应有意识地再生成并审查
夹具 diff。请勿手工编辑 JSON——预期必须始终是执行输出。

## 消费者

- `apps/orchbee-server` 的 `src/agent` 测试通过阶段性 Rust 移植重放语料
  （`include_str!`，使用现有 `serde_json` dev-dependency 解码）。
- `packages/agent` 拥有生成器，当 TypeScript 模型变化时可再运行作为证据。

英文原文为权威版本：[README.md](README.md)。
