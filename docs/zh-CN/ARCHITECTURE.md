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
	Web["apps/web<br/>共享 renderer 与页面客户端 runtime"]
	Workbench["@orchbee/workbench<br/>Project / Workspace / Tab / Panel"]
	Terminal["@orchbee/terminal<br/>Projected Terminal 客户端界面"]

	Web -. "组合" .-> Workbench
	Web -. "组合" .-> Terminal
	Web -->|"运行形态"| Electron["Electron renderer"]
	Web -->|"运行形态"| Browser["Browser 页面"]

	Electron <-->|"@orchbee/desktop-client-protocol<br/>+ @orchbee/workbench-client-protocol<br/>通过 preload MessagePorts"| Desktop["apps/desktop<br/>Electron Main、Session 协调、<br/>Environment registry、Binding<br/>与应用生命周期"]
	Browser <-->|"@orchbee/remote-client-protocol<br/>通过 HTTP + 签名 Cookie"| Server["apps/server<br/>renderer 与 Browser-session 入口"]
	Browser <-->|"@orchbee/gateway-protocol<br/>通过 Cookie 准入的 WebSocket"| Gateway["apps/gateway<br/>本地开发 Browser bridge"]
	Desktop -->|"仅当前 activation 有效的 verifier<br/>与 session 签名密钥"| Server
	Gateway <-->|"@orchbee/gateway-protocol start<br/>+ @orchbee/workbench-client-protocol<br/>通过 scoped MessagePorts"| Desktop
	Desktop <-->|"@orchbee/terminal-protocol<br/>通过私有本地 IPC"| Host["apps/terminal-host<br/>PTY / process / Ghostty VT / history 权威"]
	Desktop <-->|"@orchbee/ssh-environment-protocol<br/>通过一个受监督的 OpenSSH stdio attachment"| SshRuntime["apps/ssh-environment-runtime<br/>每个 Environment 一个 daemon / epoch<br/>有界 project-root-v1 observation"]
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
