# 架构

## 依赖方向

```text
apps/*（JavaScript / TypeScript）-> packages/* -> 外部依赖
apps/*（Rust）                 -> crates/*   -> 外部依赖
```

- 应用是运行时入口，绝不能成为 package、crate 或其他应用的依赖。
- 包只能通过其他包声明的包导出来依赖它们。
- Rust crate 只能通过声明的 Cargo dependency 依赖其他 crate。
- 多个应用共享的代码只有具备稳定 semantic owner 和真实 consumer 后，才进入语言对应的 package
  或 crate。跨语言共享使用已接受的 serialized contract，不使用源码 import。

## 产品运行时模块

```mermaid
flowchart TB
 Native["apps/orchbee<br/>native CLI / TUI"] <-->|"local Client protocol"| Server["apps/orchbee-server<br/>local Site and Workbench authority"]
 Server <-->|"Worker protocol"| Worker["apps/orchbee-worker<br/>terminal execution coordination"]
 Worker <-->|"terminal protocol / private IPC"| Host["apps/terminal-host<br/>PTY / process / Ghostty VT / history authority"]
 Web["apps/web<br/>shared renderer and in-memory Preview"] -. "composes" .-> Workbench["@orchbee/workbench<br/>Project / Workspace / Tab / Panel"]
 Web -. "composes" .-> Terminal["@orchbee/terminal<br/>projected terminal client surface"]
 Web -->|"Electron renderer"| Desktop["apps/desktop<br/>local Main / preload and capability brokers"]
 Desktop <-->|"terminal protocol / private IPC"| Host
```

## 包边界

- 可运行的产品和开发工作区遵循 [`docs/APPLICATIONS.md`](APPLICATIONS.md)。
- 包的命名、清单、公共导出、构建、依赖和版本管理遵循 [`docs/PACKAGES.md`](PACKAGES.md)。
- Rust crate 的位置、清单、可见性、依赖和验证遵循 [`docs/RUST.md`](RUST.md)。
- 前端产品 UI 和平台适配器遵循 [`docs/frontend/package-boundaries.md`](frontend/package-boundaries.md)。
- `orchbee/no-cross-workspace-source-imports`：通过其他工作区包的公共包导入来使用它；绝不能跨入其他 `apps/*/src` 或 `packages/*/src` 源码树。
- 同一个应用、package 或 crate 内的相对 import 遵循 [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md)。
- 每个包通过 `package.json#exports` 定义公共 API；存在根导出时，由 `src/index.ts` 实现。

## 源模块边界

- 源码树使用分层可见性和显式 `index.ts` 入口点。
- [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md) 负责语义模块设计、可见性、重新导出、测试、组合和循环依赖规则。
- `orchbee/module-boundaries` 和 `import/no-cycle` 强制执行这些规则。

## 项目注册表

- 每个工作区都在基于 Schema 的 `project.json` 中声明其领域、职责、非目标、运行时和稳定性。
- [`docs/PROJECTS.md`](PROJECTS.md) 负责该契约；`pnpm lint:projects` 验证它。
- 不要在本概览中维护项目目录。

## 数据边界

在不可信的进程、网络、存储、IPC 和用户输入数据首次进入可信边界时解析它们；在内部传递经过验证的类型。

```ts
// ❌
const payload = rawPayload as RequestPayload;

// ✅
const payload = RequestPayloadSchema.parse(rawPayload);
```
