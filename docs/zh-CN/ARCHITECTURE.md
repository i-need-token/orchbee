# 架构

## 依赖方向

```text
apps/* -> packages/* -> 外部依赖
```

- 应用是运行时入口，绝不能成为包或其他应用的依赖。
- 包只能通过其他包声明的包导出来依赖它们。
- 多个应用共享的代码应放入包中。

## 包边界

- 可运行的产品和开发工作区遵循 [`docs/APPLICATIONS.md`](APPLICATIONS.md)。
- 包的命名、清单、公共导出、构建、依赖和版本管理遵循 [`docs/PACKAGES.md`](PACKAGES.md)。
- 前端产品 UI、平台适配器和面向会话的库遵循 [`docs/frontend/package-boundaries.md`](frontend/package-boundaries.md)。
- `orchbee/no-cross-workspace-source-imports`：通过其他工作区包的公共包导入来使用它；绝不能跨入其他 `apps/*/src` 或 `packages/*/src` 源码树。
- 同一个应用或包内的相对导入遵循 [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md)。
- 每个包通过 `package.json#exports` 定义公共 API；存在根导出时，由 `src/index.ts` 实现。

## 源模块边界

源码树使用分层可见性和显式 `index.ts` 入口点。关于可见性、重新导出、测试、组合和循环依赖的权威规则位于 [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md)，并由 `orchbee/module-boundaries` 和 `import/no-cycle` 共同强制执行。

## 项目注册表

每个工作区都在基于 Schema 的 `project.json` 中声明其领域、职责、非目标、运行时和稳定性。[`docs/PROJECTS.md`](PROJECTS.md) 是权威契约；`pnpm lint:projects` 会验证注册表，避免把这个概览扩张成一份需要手工维护的项目目录。

## 数据边界

在不可信的进程、网络、存储、IPC 和用户输入数据首次进入可信边界时解析它们；在内部传递经过验证的类型。

```ts
// ❌
const message = rawMessage as AgentMessage;

// ✅
const message = AgentMessageSchema.parse(rawMessage);
```
