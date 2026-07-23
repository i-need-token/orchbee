# 项目契约

## 目的

| 关注点   | 契约                                                                                                 |
| -------- | ---------------------------------------------------------------------------------------------------- |
| 位置     | `apps/*` 或 `packages/*` 下的每个直接工作区都在 `package.json` 旁保存 `project.json`。               |
| 用途     | 为 Agent 提供放置变更所需的局部上下文，无需加载整个仓库。                                            |
| 应用规则 | 放置和生命周期遵循 [`docs/APPLICATIONS.md`](APPLICATIONS.md)。                                       |
| 包规则   | 包契约遵循 [`docs/PACKAGES.md`](PACKAGES.md)。                                                       |
| 内容     | 只保留无法从路径或 `package.json` 可靠推导的事实；身份、脚本、依赖和导出仍保存在 `package.json` 中。 |
| Schema   | `schemas/project-contract.schema.json` 提供编辑器验证。                                              |
| 验证     | `pnpm lint:projects` 使用 Zod 解析契约，并拒绝缺失、过期或无效的元数据。                             |

## 必需字段

| 字段             | 契约                                                          |
| ---------------- | ------------------------------------------------------------- |
| `domain`         | 稳定的 kebab-case 产品或工程领域。                            |
| `runtime`        | `browser`、`electron`、`node` 或 `universal` 中的一项或多项。 |
| `stability`      | `experimental`、`stable` 或 `deprecated`。                    |
| `responsibility` | 一条正面陈述，说明这个工作区拥有的能力。                      |
| `nonGoals`       | 明确说明不属于这里的相邻职责。                                |

职责和非目标是架构放置规则。只在所有权确实发生转移时更新它们；不要仅仅为了证明某项实现合理就扩大它们。

## 变更规则

- 在创建工作区的同一个变更中添加 `project.json`。
- 当生成代码或公共类型获得新的平台要求时修改 `runtime`。
- 有意识地修改 `stability`；提升为 `stable` 要求公共 API 和行为契约已经过审查。
- 转移职责时同时更新来源和目标契约，使所有权保持明确。
- 在 `package.json#exports` 中维护公共入口点，在 `package.json#scripts` 中维护验证命令。
- 在没有测量证据证明指标能够以足够低的误报率预测缺陷时，不要把数值质量预算添加为合并阻塞项。

## 验证

`pnpm lint:projects` 验证每个直接工作区的契约，并让 JSON Schema 与其 Zod 源保持同步。对于 `packages/*`，它还会验证 `@orchbee/<directory>` 名称以及 [`docs/PACKAGES.md`](PACKAGES.md#必需的包结构) 所要求的标准库脚本。

验证器刻意不对文字打分、不统计源码行数、不限制依赖数量，也不复制导出映射。依赖方向和源码可见性仍由架构 lint 规则强制执行。
