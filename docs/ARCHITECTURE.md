# Architecture

## Dependency direction

```text
apps/* (JavaScript / TypeScript) -> packages/* -> external dependencies
apps/* (Rust)                  -> crates/*   -> external dependencies
```

- Apps are runtime entry points and never dependencies of packages, crates, or other apps.
- Packages may depend on other packages only through their declared package exports.
- Rust crates may depend on other crates only through declared Cargo dependencies.
- Code shared by multiple apps belongs in the language-appropriate package or crate after it has a
  stable semantic owner and real consumers. Cross-language sharing uses an accepted serialized
  contract, not source imports.

## Product runtime modules

Product roles and identity boundaries use the [Glossary](GLOSSARY.md). A TUI Workbench Client owns
its local interaction runtime; each accepted Client–Server transport is one Client Connection.
Neither lifecycle is a Session identity.
A TUI uses one admitted OBLC physical connection for its OBWP v1 request/subscription streams; logical lanes share it without acquiring additional Client identities. Server commit owners publish invalidations to the connection's standard-library reader/writer pair.

```mermaid
flowchart TB
 Native["apps/orchbee<br/>native CLI / TUI"] <-->|"local Client protocol"| Server["apps/orchbee-server<br/>local Session and Workbench authority"]
 Server <-->|"Worker protocol"| Worker["apps/orchbee-worker<br/>terminal execution coordination"]
 Worker <-->|"terminal protocol / private IPC"| Host["apps/terminal-host<br/>PTY / process / Ghostty VT / history authority"]
 Web["apps/web<br/>shared renderer and in-memory Preview"] -. "composes" .-> Workbench["@orchbee/workbench<br/>Project / Workspace / Tab / Panel"]
 Web -. "composes" .-> Terminal["@orchbee/terminal<br/>projected terminal client surface"]
 Web -->|"Electron renderer"| Desktop["apps/desktop<br/>local Main / preload and capability brokers"]
 Desktop <-->|"terminal protocol / private IPC"| Host
```

## Package boundaries

- Runnable product and development workspaces follow [`docs/APPLICATIONS.md`](APPLICATIONS.md).
- Package naming, manifests, public exports, builds, dependencies, and versioning follow [`docs/PACKAGES.md`](PACKAGES.md).
- Rust crate placement, manifests, visibility, dependencies, and verification follow [`docs/RUST.md`](RUST.md).
- Frontend product UI and platform adapters follow [`docs/frontend/package-boundaries.md`](frontend/package-boundaries.md).
- `orchbee/no-cross-workspace-source-imports`: Import another workspace package through its public package exports; never cross into another `apps/*/src` or `packages/*/src` tree.
- Relative imports inside one app, package, or crate follow [`docs/MODULE-BOUNDARIES.md`](MODULE-BOUNDARIES.md).
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

Before the first public release, each wire protocol, persisted format, and machine-readable
document retains only its current contract. Integer versions stay at `1`; the five OBTP version
domains stay at `0.1.0`. Shape changes update that current contract directly, without historical
versions, compatibility readers, or migrations. Cohort admission or strict parsing rejects peers
and files with other shapes; where neither protects version reuse, replace the magic or schema
identifier instead of retaining an older contract.

The language-neutral Terminal authority is [`schemas/terminal`](../schemas/terminal/README.md).
Its reference, version policy, constant registry and canonical fixtures govern the independent
Rust, Zig and TypeScript bindings. Language packages own implementation and fixture production,
not competing wire contracts.

Shared domain state and Client-local presentation follow the responsibility boundaries in
[`INTERFACE-DESIGN.md`](INTERFACE-DESIGN.md). GUI and TUI project the owning contracts; their
different renderers do not establish separate authority over the same product meaning.

Parse untrusted process, network, storage, IPC, and user-input data at its first trusted boundary; pass validated types internally.

```ts
// ❌
const payload = rawPayload as RequestPayload;

// ✅
const payload = RequestPayloadSchema.parse(rawPayload);
```
