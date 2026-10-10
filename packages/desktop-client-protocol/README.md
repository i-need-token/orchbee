# @orchbee/desktop-client-protocol

Strict, runtime-neutral data contracts for the Electron-specific capability boundary between Orchbee Desktop and the canonical `apps/web` renderer.

## Scope

The package owns the atomic Desktop renderer bootstrap, its fixed ten-port ordering and security attestation, Agent observation and integration management, application lifetime, local Project Folder acquisition and managed Worktree creation, terminal launch profiles, terminal defaults, and bounded clipboard reads.

Application lifetime distinguishes user quit, update restart, and development-only Terminal Host restart. User quit may cancel, preserve, or stop. Update restart automatically preserves. Development Host restart is an explicit non-interactive mode that durably stops current sessions and stages fresh provisions before Main reports readiness; it is not an updater alias. Commands carry only a request identity, mode, aggregate counts, the exact shared-document revision, progress, bounded typed failures, and the Workbench presentation identities needed to resume an in-progress development Host restart after renderer replacement. They never expose Host, binding, terminal, or filesystem authority.

Agent observation uses protocol version `2`: the renderer sends one correlated full-snapshot request, while Main sends correlated responses, unsolicited complete `AgentRuntimeSnapshot` publications, and strictly parsed canonical `AgentSemanticEvent` publications. Snapshot revisions remain the state authority; semantic publications are exact effect triggers delivered after their corresponding snapshot update and are not replayed as history. It carries no Agent hook payload, prompt, transcript, command, tool content, transport correlation capability, response channel, or orchestration authority; canonical Agent semantics remain with `@orchbee/agent`. Agent integration management carries one canonical ten-entry snapshot, an ordered bounded set of local direct-launch identities, unsolicited idempotent launch observations, and only correlated inventory, refresh, enable, disable, completed, and bounded failure messages. Launch observations contain no command text or arguments and grant no status or mutation authority. It exposes no executable path, packaged-resource path, raw Agent output, generic process/filesystem capability, or remote-host authority.

It contains no Electron imports, UI, broker, coordinator, transport lifecycle, authentication, terminal projection, PTY, or VT behavior. The contract grants only the scoped capability represented by each port.

Folder acquisition returns a validated path without carrying group membership. Workbench resolves addition through the immutable resource namespace. Worktree discovery/create/adopt commands identify the source Workspace and resource Directory; Main revalidates source execution identity and uses current nullable Project membership at commit. Typed `worktree-retained` failures preserve known Git outcomes without replaying creation. No command grants generic Project, filesystem or process authority.

Local Project Folder acquisition distinguishes existing-Folder selection, native parent selection, and explicit child creation. Creation commands carry the parent selection identity, a monotonic attempt and one bounded name, never a renderer-selected path. Known no-effect failures permit explicit correction on that parent. Successful or uncertain creation consumes it and requires an exact creation-request acknowledgement before another operation; retirement reports retained receipts only during Main lifetime. See the [owning contract](../../apps/desktop/docs/project-folder-acquisition.md).

## Use

Applications import schemas and inferred types from the package root. Every process, IPC, Window, or MessagePort boundary parses untrusted values before passing them to an internal owner.

```ts
import { DesktopRendererBootstrapSchema } from '@orchbee/desktop-client-protocol';

const bootstrap = DesktopRendererBootstrapSchema.parse(untrustedBootstrap);
```

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/desktop-client-protocol build
pnpm --filter @orchbee/desktop-client-protocol typecheck
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop-client-protocol test:exports
```
