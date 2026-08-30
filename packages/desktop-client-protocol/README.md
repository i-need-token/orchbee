# @orchbee/desktop-client-protocol

Strict, runtime-neutral data contracts for the Electron-specific capability boundary between Orchbee Desktop and the canonical `apps/web` renderer.

## Scope

The package owns one atomic Desktop renderer bootstrap, the fixed ordering and security attestation for its fourteen independent capability ports, one static SSH Project-enrollment availability boolean, the Agent-observation snapshot and semantic-event contract, the Agent-integration-management command contract, the device-local native-window layout and cross-window Tab-movement contract, the Browser-access and Personal Access Token management contract, the typed existing-Project-Folder acquisition and managed-Project-Worktree contracts, the device-local terminal-launch-profile and terminal-default-settings command contracts, the bounded plain-text clipboard-read contract, the exact first-remote-terminal creation contract, the purpose-specific SSH Project enrollment, existing-Project Folder attachment, reusable SSH Environment catalog, and explicit connection-control contract, the stable name of the context-isolated file-path lookup exposed by preload, and Electron application-lifetime control. The bootstrap embeds the transport-neutral Workbench client-session bootstrap owned by `@orchbee/workbench-client-protocol`. The availability boolean exposes no profile, path, signer, artifact, Environment identity, or diagnostic; Web uses it only to omit SSH surfaces when Main has no admitted release source.

Application lifetime distinguishes user quit, update restart, and development-only Terminal Host restart. User quit may cancel, preserve, or stop. Update restart automatically preserves. Development Host restart is an explicit non-interactive mode that durably stops current sessions and stages fresh provisions before Main reports readiness; it is not an updater alias. Commands carry only a request identity, mode, aggregate counts, the exact shared-document revision, progress, bounded typed failures, and the Workbench presentation identities needed to resume an in-progress development Host restart after renderer replacement. They never expose Host, binding, terminal, or filesystem authority.

Agent observation uses protocol version `2`: the renderer sends one correlated full-snapshot request, while Main sends correlated responses, unsolicited complete `AgentRuntimeSnapshot` publications, and strictly parsed canonical `AgentSemanticEvent` publications. Snapshot revisions remain the state authority; semantic publications are exact effect triggers delivered after their corresponding snapshot update and are not replayed as history. It carries no Agent hook payload, prompt, transcript, command, tool content, transport correlation capability, response channel, or orchestration authority; canonical Agent semantics remain with `@orchbee/agent`. Agent integration management carries one canonical ten-entry snapshot, an ordered bounded set of local direct-launch identities, unsolicited idempotent launch observations, and only correlated inventory, refresh, enable, disable, completed, and bounded failure messages. Launch observations contain no command text or arguments and grant no status or mutation authority. It exposes no executable path, packaged-resource path, raw Agent output, generic process/filesystem capability, or remote-host authority.

It contains no Electron imports, UI, broker, coordinator, transport lifecycle, network Gateway, authentication, terminal projection, PTY, or VT behavior. Window-layout snapshots carry only the current logical window, stable window identities and ordinals, and this renderer's Tab assignments. Move and drag commands carry bounded stable identities and screen points; they never carry shared document content, native handles, PTY identity, process state, arbitrary bounds, or filesystem authority. Browser-access snapshots carry only lifecycle state, sanitized Origins, and `configured`, `missing`, or `unavailable` Personal Access Token status. A correlated create/reset command may return one newly issued token exactly once; no message carries the persisted verifier, Browser-session signing key, Gateway port, child-process handle, or credential-bearing URL. Project Folder acquisition can request only one user-selected existing directory and returns a correlated selected, cancelled, or bounded failure result; it cannot accept an arbitrary renderer path, enumerate a directory, or mutate the filesystem. Project Worktree discovery carries one Project identity and an optional preferred Directory identity; it returns repository creation metadata plus bounded existing-Worktree inventories containing only labels and opaque identities. Creation carries only identities returned from canonical discovery, a base ref, a new branch, and the Workspace option. Adoption carries only the Project, source Directory, repository, and Worktree identities returned by discovery. No renderer command can submit a source or target path, invoke generic Git, or choose a location override. Discovery may project the fixed managed root for read-only product presentation. Terminal-launch-profile commands expose a bounded device-local catalog and correlated create, update, default, delete, and reset operations. A command can select the system shell with a POSIX login-shell decision or one absolute executable with literal ordered arguments; it cannot provide environment variables, secrets, a working directory, a project payload, a one-off launch selection, or generic filesystem access. Terminal-default-settings commands expose only a complete bounded `scrollbackBytes` configuration and a strict snapshot with persistence status; they cannot update a running terminal or carry Host or filesystem authority. Clipboard-read commands carry only a correlated request, then return at most one plain-text value within the terminal input byte limit or one typed failure; they expose no clipboard write, alternate format, image, history, or generic platform capability. Remote-terminal creation carries only correlated canonical Project and Workspace identities and can request only the default singleton in an empty remote context. SSH Project enrollment initially accepts only one bounded SSH address. Its correlated messages carry progress; exact prompt generation and handle; bounded prompt and transient response; exact directory-listing revision, canonical path, parent, bounded child-directory names, navigation or selection; connection status; cancellation; and typed settlement. They never carry an Environment or Runtime identity, resolved target, SSH argument vector, Host connection, release artifact, persisted credential, file bytes, file entries, filesystem mutation, or general Environment authority. The file-path bridge name likewise does not grant generic filesystem authority or define shell quoting; preload owns the Electron adapter and the Web application owns the consuming policy. Shared Workbench page-client contracts remain with `@orchbee/workbench-client-protocol`; Workbench semantics remain with `@orchbee/workbench`.

Folder attachment adds only the exact target Project identity to the one-address input and returns the
Main-allocated Directory identity with its canonical path. It reuses the same progress, prompt,
directory-selection, cancellation, and single-active-request records. Enrollment and Folder
attachment records carry no Environment identity, do not choose a Directory identity, and grant no
generic Project mutation authority.

SSH Environment management publishes bounded registry-backed rows and concrete `Host` suggestions.
Only those management records carry opaque registered Environment identities, and only for exact
catalog status, Connect, and Disconnect correlation. Discovery never carries configuration bodies or
paths, resolved SSH options, credentials, or filesystem authority. Refresh performs no activation;
Add resolves one destination and appends or reuses its registry intent without connecting, opening a
Project, choosing a directory, or creating a terminal.

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
