# Terminal Launch Profiles

## Status and scope

Desktop Main owns one device-local catalog of local process launch profiles. The catalog controls
the executable and literal argument vector captured for a newly provisioned terminal. It does not
own terminal CWD selection, product-managed environment policy, terminal appearance, PTY state, or
the launch of an already durable binding. After profile resolution, Desktop applies the bounded
[automatic shell-integration refinement](kitty-automatic-shell-integration-alignment.md) before a
new binding captures the complete launch.

The local Desktop renderer may manage the catalog through the dedicated sandboxed capability port.
Gateway and ordinary Browser clients receive no catalog-management capability. The first delivery
uses the catalog default for ordinary terminal creation; selecting a non-default profile for one
creation remains a separately modeled Workbench command and recovery change.

## Problem

A local Desktop user needs to choose the system shell or one explicit executable and reuse that
choice for new terminals. The current Desktop runtime resolves one process-wide shell at startup
and provides no durable user control, named alternatives, login-shell option, or visible failure
recovery.

On macOS, a Desktop application launched outside a shell can inherit a minimal process environment.
The built-in system-shell profile therefore needs the platform's login-shell startup path so a new
terminal can receive the user's standard toolchain environment without Desktop synthesizing `PATH`.

## Inputs and validation

| Input                         | Validation and absence                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ |
| Persisted catalog             | UTF-8 JSON, bounded file size, strict version-1 schema; missing means the built-in catalog             |
| Profile ID                    | Stable bounded ASCII identity; missing IDs reject without state change                                 |
| Profile name                  | Trimmed, non-empty bounded user text; duplicate display names are allowed                              |
| System-shell command          | Uses the trusted startup shell; login mode is allowed only on POSIX                                    |
| Custom executable             | Non-empty bounded path without NUL; must be absolute for the Desktop platform                          |
| Custom arguments              | Ordered bounded literal strings without NUL; no quoting, interpolation, or shell parsing               |
| Renderer command              | Parsed by `@orchbee/desktop-client-protocol` before Main dispatch                                      |
| Base `TerminalLaunch`         | Trusted only after `TerminalLaunchSchema`; supplies every field not owned by this profile capability   |
| Resolved Host launch          | Must encode as one bounded `create-terminal` control request before the catalog can commit             |
| Terminal presentation CWD     | Existing Workbench-owned value; overrides only `currentDirectory` after the profile launch is resolved |
| Profile-store read or failure | Missing, invalid, read-failed, and write-failed are distinct expected outcomes                         |

Exact string, argument-count, profile-count, and file-size bounds are implementation constants no
larger than their Terminal Host protocol limits. They are changed with this contract and boundary
tests.

## Data

```text
TerminalLaunchProfileCatalog (version 1)
├── defaultProfileId
└── profiles[] (ordered, one or more)
    └── TerminalLaunchProfile
        ├── id
        ├── name (user text or the localized built-in label marker)
        └── command
            ├── system-shell(login: boolean)
            └── custom-executable(executable, arguments[])
```

The built-in catalog contains one stable `system-default` profile with the localized-label marker
and system-shell command. Its login mode is enabled on Darwin and disabled on every other platform.
This platform default applies only when the catalog is missing or invalid, or when the user commits
an explicit reset. A stored catalog remains authoritative and is never rewritten to follow a later
built-in default. A persisted profile name is user data and is never translated. Profile ID, not
display name or list position, owns identity.

`TerminalLaunchProfileSnapshot` contains the canonical catalog, its source (`built-in` or `stored`),
and an optional persistence issue. The snapshot is the only catalog projection published to the
renderer.

## Invariants

- The catalog contains between one and the configured maximum number of profiles.
- Profile IDs are unique and `defaultProfileId` names exactly one current profile.
- Profile order is stable; create inserts after its source, while edit and default selection do not
  reorder profiles.
- Exactly the stable built-in profile may use the localized-label marker. User-created profiles have
  explicit names.
- A system-shell profile has no custom executable or arguments. A custom-executable profile has no
  login-shell flag.
- The built-in System shell enables login mode on Darwin and disables it on Linux and Windows.
  Stored profiles keep their explicit login choice on every supported platform.
- A Windows catalog cannot contain an enabled login-shell command.
- Deleting the final profile or current default profile rejects. The user selects another default
  before deleting the previous one.
- Duplicate profile names never merge identities or change selection.
- A profile changes only `executable` and `arguments` in the trusted base `TerminalLaunch`.
- Automatic shell integration may refine that resolved launch only after the profile operation has
  completed; it never changes the selected executable or user-owned literal arguments.
- Every profile resolves against the trusted base and fits the Terminal Host control boundary before
  the complete catalog is admitted or committed.
- Product-managed environment entries, terminal profile, configuration, geometry, and base CWD
  survive profile resolution unchanged.
- A terminal binding captures one complete resolved launch before its first provision side effect.
  Later profile edits, retries, renderer reloads, and application restarts cannot change it.
- Restore uses the launch already stored with the binding and never resolves the current catalog.

## Commands and results

| Command             | Success                                                               | Expected rejection or failure                                  |
| ------------------- | --------------------------------------------------------------------- | -------------------------------------------------------------- |
| Read snapshot       | Returns the current canonical snapshot                                | Control unavailable                                            |
| Create from profile | Main allocates one ID and inserts a command copy after the source     | Source missing, catalog full, invalid name, or write failure   |
| Update profile      | Replaces that profile's name and command without changing ID or order | Profile missing, invalid platform/path/value, or write failure |
| Select default      | Changes only `defaultProfileId`                                       | Profile missing or write failure                               |
| Delete profile      | Removes one non-default profile                                       | Missing, final, default, or write failure                      |
| Reset catalog       | Persists the platform's built-in catalog                              | Write failure                                                  |

Every renderer request has one correlated completion or expected failure. Malformed messages or
unexpected transferred ports retire that capability port. A port allows at most one request in
flight; a second distinct request receives `busy` and changes no state.

## State and transitions

```text
uninitialized
  └── load ──> ready(built-in, no issue)       when missing
           ├─> ready(stored, no issue)         when valid
           └─> ready(built-in, read issue)     when invalid or unreadable

ready(snapshot)
  └── valid mutation ──> writing(candidate)
       ├── atomic store commit ──> ready(stored candidate, no issue) + completed
       └── store failure ────────> ready(previous catalog, write-failed) + failed
```

Main serializes catalog mutations. The authoritative commit point is the successful atomic file
replacement. While a write is pending, terminal creation observes the previous committed catalog;
after commit it observes the new catalog. Port disposal does not cancel an already-started atomic
write, and Desktop shutdown waits for the mutation tail. Owner-only mode and temporary-file sync
complete before rename. A directory durability sync after rename is best-effort because its failure
cannot truthfully roll the already-visible catalog back to the prior snapshot.

Loading an invalid or unreadable file never rewrites it. A later explicit successful create,
update, default-selection, delete, or reset command may replace it. A custom executable that is
syntactically valid but unavailable at launch time fails through the existing terminal lifecycle;
Desktop never silently falls back to another profile.

## Ownership and dependencies

| Responsibility                                                         | Owner                              |
| ---------------------------------------------------------------------- | ---------------------------------- |
| Catalog state, transitions, persistence, and profile launch resolution | `apps/desktop`                     |
| Post-profile automatic shell-integration refinement                    | `apps/desktop`                     |
| Sandboxed wire schemas and fixed port role                             | `@orchbee/desktop-client-protocol` |
| Local settings presentation and request client                         | `apps/web`                         |
| Product messages and translations                                      | `@orchbee/i18n`                    |
| CWD and terminal-presentation intent                                   | `@orchbee/workbench`               |
| Concrete durable binding launch                                        | Desktop terminal binding store     |
| PTY creation and executable availability                               | Terminal Host                      |

The catalog store requires only load and atomic replace. The controller requires platform,
trusted base launch, stable ID allocation, and the store. Platform is an explicit input to both
built-in fallback construction and reset. The deterministic catalog transition and launch resolver
perform no filesystem, process, transport, localization, or React work.

## Verification and traceability

| Claim                                                                       | Executable evidence                                                               |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Darwin login default and non-Darwin defaults                                | Platform-matrix catalog and Main controller unit tests                            |
| Stored login choice remains authoritative                                   | Main controller load and resolution unit tests                                    |
| Catalog invariants and transition rejections                                | Deterministic catalog unit tests                                                  |
| Missing, invalid, bounded, atomic, and failed I/O                           | File-store unit tests with isolated temporary directories                         |
| Exact profile refinement, Host encoding, and platform login rules           | Resolver and Main controller unit tests                                           |
| Automatic shell detection, launch refinement, fallback, and loader behavior | Automatic shell-integration resolver, resource, and Desktop system tests          |
| Commit ordering and failure preservation                                    | Main controller unit tests                                                        |
| Wire validation, correlation, busy, and disposal                            | Protocol, Main broker, preload, and renderer-client unit tests                    |
| First provision captures; retry/restore retain                              | `DesktopTerminalLifecycleAdapter` unit tests                                      |
| Local-only capability allocation and cleanup                                | `DesktopApplication` and Desktop Web composition unit tests                       |
| Localized management, validation, and failure UI                            | Settings controller/component tests plus both-locale deterministic Story evidence |
| Real new-terminal launch                                                    | Desktop system test excluding the manual native-window test                       |

The tests control storage results, request order, IDs, platform, and process launch payloads. They
do not prove that an arbitrary user executable exists or behaves correctly. Native platform tests
support only the operating system on which they run; cross-platform schema and resolver tests do
not claim native process execution.

## Exclusions

Version 1 excludes environment overrides and secrets, synthesized or harvested `PATH`, macOS
`/usr/bin/login` wrapping, per-Project or per-Workspace launch configuration payloads, one-off
non-default profile selection, profile appearance, startup-directory policy, SSH or remote
connections, automatic switching, import/export, shell startup-file mutation, multi-window
behavior, and cross-device synchronization.
