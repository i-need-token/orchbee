# Architecture

## Dependency direction

```text
apps/* -> packages/* -> external dependencies
```

- Apps are runtime entry points and never dependencies of packages or other apps.
- Packages may depend on other packages only through their declared package exports.
- Code shared by multiple apps belongs in a package.

## Product runtime modules

```mermaid
flowchart TB
	Web["apps/web<br/>shared renderer and page-client runtime"]
	Workbench["@orchbee/workbench<br/>Project / Workspace / Tab / Panel"]
	Terminal["@orchbee/terminal<br/>projected terminal client surface"]

	Web -. "composes" .-> Workbench
	Web -. "composes" .-> Terminal
	Web -->|"runs as"| Electron["Electron renderer"]
	Web -->|"runs as"| Browser["Browser page"]

	Electron <-->|"@orchbee/desktop-client-protocol<br/>+ @orchbee/workbench-client-protocol<br/>through preload MessagePorts"| Desktop["apps/desktop<br/>Electron Main, session coordination,<br/>Environment registry, bindings,<br/>and application lifetime"]
	Browser <-->|"@orchbee/remote-client-protocol<br/>over HTTP + signed Cookie"| Server["apps/server<br/>renderer and Browser-session ingress"]
	Browser <-->|"@orchbee/gateway-protocol<br/>over Cookie-admitted WebSocket"| Gateway["apps/gateway<br/>local development browser bridge"]
	Desktop -->|"activation-scoped verifier<br/>and session-signing key"| Server
	Gateway <-->|"@orchbee/gateway-protocol start<br/>+ @orchbee/workbench-client-protocol<br/>over scoped MessagePorts"| Desktop
	Desktop <-->|"@orchbee/terminal-protocol<br/>over private local IPC"| Host["apps/terminal-host<br/>PTY / process / Ghostty VT / history authority"]
	Desktop <-->|"@orchbee/ssh-environment-protocol<br/>over one supervised OpenSSH stdio attachment"| SshRuntime["apps/ssh-environment-runtime<br/>per-Environment daemon / epoch<br/>bounded project-root-v1 observation"]
```

## Package boundaries

- Runnable product and development workspaces follow [`docs/APPLICATIONS.md`](APPLICATIONS.md).
- Package naming, manifests, public exports, builds, dependencies, and versioning follow [`docs/PACKAGES.md`](PACKAGES.md).
- Frontend product UI and platform adapters follow [`docs/frontend/package-boundaries.md`](frontend/package-boundaries.md).
- `orchbee/no-cross-workspace-source-imports`: Import another workspace package through its public package exports; never cross into another `apps/*/src` or `packages/*/src` tree.
- Relative imports inside one app or package follow [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md).
- Each package defines its public API through `package.json#exports`; `src/index.ts` implements the root export when present.

## Source module boundaries

- Source trees use hierarchical visibility and explicit `index.ts` entry points.
- [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md) owns semantic module design, visibility, re-export, test, composition, and cycle rules.
- `orchbee/module-boundaries` and `import/no-cycle` enforce those rules.

## Project registry

- Every workspace declares its domain, responsibility, non-goals, runtime, and stability in a schema-backed `project.json`.
- [`docs/PROJECTS.md`](PROJECTS.md) owns the contract; `pnpm lint:projects` verifies it.
- Do not maintain a project catalog in this overview.

## Data boundaries

Parse untrusted process, network, storage, IPC, and user-input data at its first trusted boundary; pass validated types internally.

```ts
// ❌
const payload = rawPayload as RequestPayload;

// ✅
const payload = RequestPayloadSchema.parse(rawPayload);
```
