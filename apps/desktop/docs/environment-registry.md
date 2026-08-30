# Desktop Environment Registry

## Scope and delivered boundary

Slice 2.3A delivered the first durable Environment identity owner. The Hub owns
Environment-registry semantics; until a headless Hub exists, Electron Main is the colocated
implementation and sole writer. The current store writes SSH Environment intent to
`environment-registry-v2.json`, can read the legacy `environment-registry-v1.json` through the
bounded migration described below, loads before opening the local Terminal Host, and exposes only
process-local create, list, get, and explicit reload methods.

This slice has no renderer, Browser, CLI, Project, Workspace, Checkout, terminal, filesystem, Git,
Agent, or remote-service API. Creating a record does not resolve the SSH target, start OpenSSH,
install a Runtime, or mutate a remote machine. Tests use memory and local temporary directories;
no real SSH server is contacted.

## Accepted decisions D27-D36

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                       |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D27      | The Hub owns Environment-registry semantics. Desktop Main is the first colocated owner; Workbench and a speculative shared package do not own or duplicate it.                                                                                                                                                                                                                          |
| D28      | `EnvironmentId` and SSH `runtimeInstanceId` are independent immutable random 128-bit identities. Neither derives from an SSH address or from the other identity.                                                                                                                                                                                                                        |
| D29      | Version 1 admits only SSH records and persists a bounded display name, explicit SSH target intent, and Runtime identity. Secrets, host keys, resolved target observations, connection state, Runtime epoch, and diagnostics never enter the registry.                                                                                                                                   |
| D30      | Slice 2.3A provides create, list, and get only. Records retain insertion order; exact SSH targets are unique; duplicate display names are allowed. Caller-preallocated identical records replay idempotently, while identity or target reuse with different intent conflicts. Edit, repoint, delete, and remote cleanup remain unavailable.                                             |
| D31      | The bounded versioned file uses strict parsing, owner-only directories and files, serialized atomic replacement, file synchronization, and parent-directory synchronization. Missing means empty on load; invalid or unreadable data blocks; a post-rename durability failure is indeterminate and requires explicit reload before more operations. There is no prior-format migration. |
| D32      | Slice 2.3B composes one exact offline two-platform Runtime release bundle with Desktop. Remote bootstrap does not download from a network.                                                                                                                                                                                                                                              |
| D33      | Slice 2.3B binds an Ed25519 signing-key identity and exact manifest trust descriptor to release-only Desktop packaging. Production private keys stay outside the repository and ordinary CI.                                                                                                                                                                                            |
| D34      | A live remote Runtime with an incompatible version or digest will block; Desktop will not kill or silently upgrade it.                                                                                                                                                                                                                                                                  |
| D35      | Slice 2.3C will add one bounded same-child installed-artifact negotiation before Hello, without treating a remote status token as a trust decision.                                                                                                                                                                                                                                     |
| D36      | Slice 2.3A, 2.3B, and 2.3C remain independently authorized and reversible. This slice delivers only the durable registry and its evidence.                                                                                                                                                                                                                                              |

## Accepted extension D192

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D192     | Registry version 2 extends exact SSH target intent with a canonical bounded list of reviewed connection options. Desktop reads version 2 first. Only when the version-2 file is missing may it validate and migrate version 1 in memory; the first later commit writes version 2 while leaving version 1 byte-for-byte untouched. An invalid or unreadable version-2 file blocks instead of falling back. Older Desktop versions may therefore roll back to the untouched version-1 snapshot but cannot observe Environments first admitted by version 2. This supersedes D29 and D31 only for the current schema and its explicit migration. |

## Authority and identity model

`EnvironmentId` is a provider-neutral product identity with canonical form
`environment:<32 lowercase hex characters>`. `runtimeInstanceId` is the existing SSH Runtime
protocol identity with exactly 32 lowercase hexadecimal characters. A caller allocates both before
submitting create, retains them across an indeterminate result, and never recomputes them from a
destination, alias, configuration path, resolved hostname, user, or port.

The current record has this canonical shape:

```json
{
	"environmentId": "environment:11111111111111111111111111111111",
	"name": "Build machine",
	"provider": {
		"kind": "ssh",
		"runtimeInstanceId": "22222222222222222222222222222222",
		"target": {
			"configurationFilePath": null,
			"destination": "build-host",
			"options": [
				{ "enabled": true, "kind": "forward-agent" },
				{ "kind": "port", "value": 2222 }
			]
		}
	}
}
```

The SSH target key is the exact triple of nullable absolute configuration-file path, concrete
destination, and canonical structured options. Singleton options have one stable order regardless
of input order; repeated identity-file options retain their relative order. One target has one
Environment because multiple future Project paths belong beneath that Environment. Target
resolution remains a transient OpenSSH-owned observation and cannot silently change product
identity. A different alias, configuration path, or option value is different explicit intent;
discovering or merging physical-machine aliases remains reserved.

## Append-only lifecycle

The strict version-2 document stores `schemaVersion`, `revision`, and `environments`. Because this
owner has no update or deletion, `revision` equals the number of admitted records.

| Current condition                                    | Operation or result | Authoritative outcome                                                                                     |
| ---------------------------------------------------- | ------------------- | --------------------------------------------------------------------------------------------------------- |
| Both version 2 and version 1 missing                 | Load                | Ready with revision `0` and no records.                                                                   |
| Valid version-2 file                                 | Load                | Ready with the exact validated order and revision.                                                        |
| Version 2 missing and valid version 1 present        | Load                | Ready with the in-memory version-2 projection; neither file is written.                                   |
| Version 2 invalid, or fallback version 1 invalid     | Load                | Unavailable; no replacement file is created.                                                              |
| Unused identity and target                           | Create              | Append once and advance the revision after durable commit.                                                |
| Same identity and identical record                   | Create              | Return `existing`; do not write or advance revision.                                                      |
| Same identity with different intent                  | Create              | Return `environment-id` conflict; preserve state.                                                         |
| Same exact target under another identity             | Create              | Return `ssh-target` conflict; preserve state.                                                             |
| Capacity reached                                     | Create              | Reject without writing.                                                                                   |
| Store revision changed before commit                 | Create              | Refresh the trusted snapshot and return a conflict; do not silently reorder or retry the request.         |
| Failure before atomic rename                         | Create              | Return unavailable while preserving the readable prior snapshot and allowing caller-controlled retry.     |
| Failure confirming the parent directory after rename | Create              | Return indeterminate, discard the trusted in-memory snapshot, and block operations until explicit reload. |

The controller serializes every load and operation in arrival order. Returned records are detached
copies, so a caller cannot mutate the authoritative snapshot. No code path changes an existing
target, Runtime identity, name, or order, and no product path removes the registry file.

## Persistence and recovery

Desktop resolves the current file as `<userData>/environment-registry-v2.json`. The boundary is:

| Limit                       | Value                                                                    |
| --------------------------- | ------------------------------------------------------------------------ |
| Schema version              | Exactly `2`                                                              |
| Maximum encoded JSON bytes  | 1 MiB, plus one trailing newline on disk                                 |
| Maximum Environment records | 1,024                                                                    |
| Maximum display-name length | 256 UTF-16 code units, nonempty, trimmed, and without control characters |
| Maximum SSH target options  | 32 canonical reviewed options                                            |
| Environment identity        | `environment:` plus 32 lowercase hexadecimal characters                  |
| Runtime identity            | 32 lowercase hexadecimal characters                                      |

Creation ensures a `0700` parent directory and a fresh `0600` temporary file on POSIX. The adapter
writes and synchronizes the temporary file, atomically renames it over the registry, then
synchronizes the parent directory. A temporary-name collision or any other pre-rename error leaves
the prior file intact. If rename completed but directory synchronization failed, the adapter does
not claim success. Explicit reload observes whichever strict document the filesystem presents;
the caller can then replay the same preallocated record safely.

A missing current file is empty only when no valid legacy version-1 file exists. When version 2 is
missing, Desktop may read `<userData>/environment-registry-v1.json`, strictly validate it, and
project the same revision and records into the version-2 shape without options. The migration is
read-only until a later create commits the complete version-2 document. Once version 2 exists it is
the sole authority; invalid or unreadable version 2 never falls back. If a store expected an
admitted nonzero revision but both files disappeared, that commit blocks instead of recreating an
empty registry. Version 1 is never rewritten or removed, so an older Desktop can roll back to that
snapshot without corrupting it; it intentionally cannot see version-2-only records. A further
schema change must again own an explicit migration and rollback contract.

## Privacy and trust boundary

The file contains a user-selected label, an SSH destination or alias, an optional absolute SSH
configuration path, canonical reviewed options, and opaque identities. Option values may include a
user name, port, jump destination, or identity-file path. That inventory can itself be sensitive,
so it remains inside the owner-only Desktop data root and is not emitted as a diagnostic, trace,
telemetry field, renderer bootstrap, or Browser payload. The explicit SSH-management UI may receive
only a bounded canonical command projection for selection; it never receives the registry document.

Passwords, passphrases, one-time codes, private-key contents, agent sockets, `known_hosts` data,
resolved target observations, host-key lookup name, Runtime artifact, Runtime epoch, availability,
retry state, and prompt text are structurally absent. An explicit identity-file path is metadata,
never key material. OpenSSH remains the authority for configuration, credentials, and host
verification. Persisting a record grants no connection or remote effect.

## Modeling and verification

`EnvironmentRegistryCommit` is the bounded TLA+ model for independently ordered creates, atomic
rename, pre-commit failure, post-rename uncertainty, process restart, reload, and caller replay. It
checks canonical identity and target uniqueness, stable append order, revision/count agreement,
capacity, retained successful claims, and progress out of an indeterminate phase under reload
fairness.

| Claim                                                        | Deterministic evidence                                              | Bounds and exclusions                                                               |
| ------------------------------------------------------------ | ------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| Identity and exact target remain unique                      | Example tests plus fixed-seed stateful property traces              | Eight generated identities and targets; physical SSH-host aliasing is not inferred. |
| Create is idempotent and insertion order is stable           | Pure transition tests and controller tests                          | Create/list/get only; no edit or delete.                                            |
| Concurrent callers cannot reuse one stale in-memory revision | Controller serialization and real file-store competing-commit tests | One Desktop process owns the Electron single-instance lock.                         |
| Invalid or oversized data never becomes an empty replacement | Strict-schema and real-file tests                                   | Operating-system failure codes are reduced to bounded reasons.                      |
| Pre-rename failure preserves the prior file                  | Temporary-file collision fault injection                            | Filesystem or hardware behavior after a successful sync remains platform authority. |
| Post-rename sync failure never reports a durable commit      | Directory-sync fault injection and `EnvironmentRegistryCommit`      | Explicit reload is required; no UI currently initiates it.                          |

Run the focused boundary from the repository root:

```sh
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop exec vitest run src/main/environment/__tests__
node scripts/tla/run-models.mjs --models desktop-environment-registry-commit
```

## Current limits and next authority

Slice 2.3A does not expose Environment management to a renderer or remote Client, automatically
create a record, resolve or connect its SSH target, compose `SshEnvironmentSupervisor`, negotiate an
installed artifact, add a Runtime service, or map a Project, Checkout, Directory, Workspace,
terminal, Git operation, or Agent onto an Environment. Slice 2.3B now packages a signed Runtime
release independently and grants none of those registry or connection effects.

Slice 2.3C separately composed explicit process-local activation and the installed-artifact fast
path without making registry load/create connect automatically. D192 later introduced the bounded
version-1-to-version-2 migration above without changing that non-activation rule. Environment
rename, target edit, removal, remote cleanup, provider expansion, physical-host alias reconciliation,
any further migration, and Project placement require later accepted decisions; they must not be
inferred from this append-only file.
