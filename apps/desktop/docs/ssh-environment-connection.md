# SSH Environment Connection

## Scope and delivered boundary

Desktop Main owns the process-local Slice 2.2 connection foundation for one explicitly requested
SSH Environment: target resolution, one supervised OpenSSH child, transient AskPass, a verified
Runtime projection from the signed SSH Environment release, a no-download remote bootstrap, exact
Hello and heartbeat, generation-fenced retry, and attachment retirement. The native
`apps/ssh-environment-runtime` daemon and
`@orchbee/ssh-environment-protocol` wire contract are concrete consumers of this boundary.

Slice 2.3A separately composes the durable append-only Environment registry into
`DesktopApplication`. Slice 2.3B separately delivers an offline, release-only two-platform Runtime
bundle and Desktop trust composition. Slice 2.3C now composes an explicit process-local connection
owner keyed by durable Environment identity and adds same-child installed-artifact negotiation. There
is now one Slice 2.4 process-local API for inspecting an explicit remote Project-root candidate over
the already-connected epoch. Slice 2.5 composes that observation with the canonical Workbench writer
to admit one unavailable remote Project Directory through a Main-only process-local API. Slice 3.1B
atomically installs the signed Runtime and Terminal Host cohort, advertises one Terminal Host relay
on the existing attachment, and returns a negotiated process-local `HostConnection`. Slice 3.2A now
extends the same Main-only admission with exactly one inactive remote Workspace and empty Terminal
Context in the same canonical commit. Slice 3.2B adds one explicit Main-only operation that provisions
or reconciles the first terminal in that Workspace through an Environment-scoped Terminal runtime.
Slice 3.2C routes Electron and authenticated Gateway projection requests from canonical Presentation
containment to that exact prepared Environment runtime, reuses the existing interactive attachment
contract, and rechecks authority before publishing the lease. Slice 3.2D now observes metadata,
runtime failure, and terminal exit from the exact current Environment relay, durably journals exits
before listener return, and publishes only after Environment-scoped binding and canonical target
validation. Slice 3.2E adds one Electron-only product surface for creating or reconciling that first
singleton from an exact empty remote Workspace. It uses a dedicated renderer capability, derives
and revalidates all remote authority in Main, and waits for canonical client-session publication.
There is no Gateway or CLI creation surface, Runtime network feed, automatic SSH connection,
directory browser, file access, Git, or Agent authority. Tests use local temporary directories,
ephemeral signing keys, and controlled processes; no real SSH server is contacted.

## Accepted decisions D17-D26

| Decision | Accepted contract                                                                                                                                                                                                                                                                         |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D17      | The topology is immutable versioned installation → one detached Runtime daemon per Environment → future private service adapters. SSH is only a replaceable attachment.                                                                                                                   |
| D18      | One active OpenSSH child owns one Environment generation over stdin/stdout, without a PTY. OpenSSH connection sharing, agent/X11 forwarding, configured forwarding, and local commands are disabled.                                                                                      |
| D19      | The product protocol is carried directly over SSH stdio. There is no SOCKS or dynamic-forwarding layer.                                                                                                                                                                                   |
| D20      | `@orchbee/ssh-environment-protocol` initially owned exact version `0.0.1`, Hello, Runtime epoch, heartbeat, channel lifecycle, binary data, and byte-credit framing. D58 supersedes only that exact version and capability set; the protocol still has no generic execute-command record. |
| D21      | AskPass uses forced activation, owner-only local IPC, one random generation capability, transient untrusted prompts, and no credential state, log, telemetry, or disk record.                                                                                                             |
| D22      | Passwords are never cached. One active child naturally reuses its authenticated SSH connection; a replacement connection may prompt again. Key-, certificate-, hardware-key-, or agent-backed authentication is preferred.                                                                |
| D23      | Desktop streams the exact artifact; the remote host never downloads it. Admission requires a canonical Ed25519-signed manifest plus exact size and SHA-256. The signing private key belongs outside the repository and ordinary CI.                                                       |
| D24      | Installation is side-by-side at `~/.orchbee/ssh-runtime/versions/<version>/`. Execution uses that exact path; there is no mutable `current` link and no automatic deletion.                                                                                                               |
| D25      | The initial remote matrix is macOS arm64 and glibc Linux x86_64. Every other remote platform fails `compatibility-unsupported`.                                                                                                                                                           |
| D26      | Disconnect retires only the attachment. A surviving daemon keeps the same epoch and future inventory; a restarted daemon reports a new epoch. Unknown terminal or Agent effects are never replayed automatically.                                                                         |

## Durable Environment identity

Accepted decisions D27-D36 split durable identity, release composition, and installed-artifact
negotiation into independently reversible slices. Slice 2.3A delivers only the first: Desktop Main
is the colocated Hub owner of `environment-registry-v1.json`, with independent Environment and
Runtime identities, exact SSH target uniqueness, caller-preallocated idempotent create, strict
owner-only atomic persistence, and explicit reconciliation after an indeterminate commit.

Creating or loading a registry record starts no connection and performs no remote effect. The
record schema, transition table, privacy boundary, rollback behavior, TLA+ refinement, and remaining
authority live in [`environment-registry.md`](environment-registry.md). Slice 2.3B release
composition and Slice 2.3C fast-path/supervisor composition are now delivered independently.

## Offline release composition

Accepted decisions D37-D46 fix one canonical schema-version-1 release, exact macOS arm64 and glibc
Linux x86_64 stripped `ReleaseSafe` builds under Zig `0.16.0`, an external Ed25519 signature, an
independently supplied public key, and one exact manifest-pinned trust descriptor per Desktop
package. Repository code never accepts a production private key, and ordinary CI uses only
ephemeral keys.

The ordinary package command remains release-free. The release-only command verifies and buffers
the exact final bundle before Electron Packager, places its executable artifacts and manifest under
`Resources`, and keeps the trust descriptor inside `app.asar`. Fixed layouts, command ceremony,
trust assumptions, rollback, modeling record, verification mapping, and remaining release limits
live in [`ssh-runtime-release.md`](ssh-runtime-release.md).

Slice 3.1A supersedes only the current release packaging path with schema version 2: one independently
versioned Runtime and one independently versioned Terminal Host for each admitted platform, bound by
one signed canonical cohort manifest. The frozen v1 contract remains valid only for already delivered
Runtime behavior and cannot satisfy a future Terminal Host capability.

## Accepted decisions D47-D56

| Decision | Accepted contract                                                                                                                                                                                                                                                                                          |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D47      | Desktop Main owns a process-local connection owner keyed by `EnvironmentId`. The registry remains durable intent only: load and create never connect, and restart never auto-connects.                                                                                                                     |
| D48      | Before OpenSSH, Desktop loads and authenticates the packaged trust descriptor and manifest as one cached release catalog. It reads and verifies platform artifact bytes lazily only after the remote side requests upload.                                                                                 |
| D49      | Platform is neither persisted nor resolved by a second SSH probe. The same OpenSSH child that carries installation and Hello detects one admitted remote platform.                                                                                                                                         |
| D50      | Negotiation accepts only one newline-terminated, maximum-128-byte ASCII token: `OBSI/1 darwin-arm64 present`, `OBSI/1 darwin-arm64 upload`, `OBSI/1 linux-x64 present`, or `OBSI/1 linux-x64 upload`. Unknown, duplicate, extra, non-ASCII, or oversized bytes fail closed.                                |
| D51      | A present version must be a non-symlink regular file with the exact catalog digest. Absence requests upload. Installation uses a no-clobber hard link; concurrent identical installers converge, while any installed or winning mismatch blocks without replacement.                                       |
| D52      | The status token grants only flow-control authority. Exact Hello-Accepted remains the final trust decision for instance, version, digest, platform, protocol, and daemon epoch; an incompatible live daemon blocks rather than being replaced.                                                             |
| D53      | One supervisor exists per active Environment. Connect and disconnect are serialized per identity, repeated connect is idempotent, and the active supervisor pins a copied registry intent until explicit disconnect.                                                                                       |
| D54      | The existing owner-only AskPass bridge remains the credential boundary. Slice 2.3C adds no renderer prompt UI; an absent interaction handler cancels. Windows activation is unsupported, and release, compatibility, integrity, authentication, host-verification, and retryable failures remain distinct. |
| D55      | Activation is an explicit process-local call. Application shutdown disposes all active supervisors. There is no registry migration, no automatic activation, and immutable remote versions remain retained.                                                                                                |
| D56      | TLA+ and executable tests cover ordering, fast paths, mismatch/no-clobber behavior, owner serialization, idempotence, disposal, and failure mapping. Local controlled-process evidence is required; a real SSH server remains outside this slice's claim.                                                  |

## Accepted decisions D57-D66

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                            |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D57      | The first concrete Runtime service inspects one explicitly supplied remote Project root. The Runtime executes the filesystem observation, Desktop Main authorizes and coordinates it through the Environment connection owner, and the existing protocol package owns the wire schema. No new package or Workbench writer is created.                                        |
| D58      | The exact `OBSE` protocol becomes `0.1.0`. Never-advertised placeholder capabilities are removed before first use; the Runtime advertises exactly `project-root-v1`, Desktop requires that exact singleton, and neither side provides dual-stack negotiation or downgrade.                                                                                                   |
| D59      | One channel carries one strict JSON request no larger than 16 KiB. It contains one absolute POSIX UTF-8 path of at most 4,096 encoded bytes, with no NUL or control characters and no tilde, environment-variable, or shell expansion. The channel identity is the complete wire correlation identity.                                                                       |
| D60      | The Runtime follows symlinks through the remote operating system, requires the final target to be a directory accessible to the SSH account, and returns only its canonical absolute UTF-8 path. The result is a revocable observation, not a retained handle, Project identity, Checkout identity, Directory identity, or promise that later filesystem state is unchanged. |
| D61      | A Desktop-initiated service channel follows `open → opened → request → result → close → closed`. Each direction receives exactly 64 KiB of byte credit, one service message is bounded at 16 KiB, heartbeat remains independent, and an epoch, channel, framing, ordering, or credit violation retires the attachment.                                                       |
| D62      | One Environment admits at most four concurrent inspections. Each caller-visible inspection settles within 30 seconds; filesystem work is isolated from the attachment loop so it cannot block heartbeat. Capacity and deadline exhaustion return `busy` and `timed-out`.                                                                                                     |
| D63      | Inspection requires one already-connected exact Runtime epoch and never activates or waits for a connection. Cancellation or attachment loss terminates the invocation; reconnect never replays it. An explicit retry allocates a fresh channel, while late worker and old-epoch results are fenced from publication.                                                        |
| D64      | Remote outcomes are bounded to `invalid-path`, `not-found`, `not-directory`, `permission-denied`, `timed-out`, `busy`, or `unavailable`, without raw operating-system diagnostics. A valid channel containing an invalid service payload fails only that channel; transport, epoch, channel-table, ordering, or credit corruption retires the attachment.                    |
| D65      | Requested and canonical paths remain process-local and are absent from the Environment registry, logs, diagnostics, and telemetry. This slice adds no renderer or Browser API, persistence, directory listing, file bytes, watch, filesystem mutation, Git, shell, terminal, Agent, Project, Checkout, Workspace, or automatic connection.                                   |
| D66      | A registered TLA+ service model and executable evidence cover exact capability admission, channel order, credit, capacity, timeout, cancellation, attachment loss, no replay, stale-result fencing, strict codecs, real local Runtime behavior, and both native targets. Real SSH, authentication UI, and production signing remain outside the claim.                       |

## Accepted decisions D67-D76

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                 |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D67      | Desktop Main owns one process-local remote-Project admission. The input is an `EnvironmentId` and requested path. Success creates exactly one Project with one remote Folder Directory, zero Workspaces, and zero terminal contexts.                                                                                                                                              |
| D68      | `WorkbenchDirectory` remains the Checkout-bearing entity and gains explicit placement: `local`, or `environment` with one opaque Environment reference. A Workspace later inherits placement through its primary Directory; no Checkout package or store is introduced.                                                                                                           |
| D69      | `WorkbenchSharedDocument` is exactly version 2 in `workbench-shared-document-v2.json`. If v2 is missing, one valid v1 file migrates every Directory to local placement without changing or deleting v1. Once present, v2 is the sole authority and no reverse migration exists.                                                                                                   |
| D70      | Admission resolves the registered Environment, requires it already connected, invokes exact `project-root-v1`, derives the name from the returned canonical path, and enters one Main-owned canonical mutation. It never auto-connects or waits for connectivity.                                                                                                                 |
| D71      | Workbench stores an opaque Environment reference; Desktop validates it against the registry. Client whole-document proposals may retain or delete an exact existing remote placement, but cannot add, change, forge, reparent, relabel as local, or mark it available. Only the Main workflow introduces remote placement.                                                        |
| D72      | `(EnvironmentId, canonicalPath)` is the durable admission idempotency key. Concurrent and retried equivalent admissions converge on one Project; distinct canonical roots commit in canonical queue order. A new remote Directory receives a Main-generated random 128-bit identity, while Project identity uses the existing Workbench sequence.                                 |
| D73      | Cancellation or attachment loss before inspection completion creates no Project. Once a canonical result enters the canonical mutation queue, caller cancellation no longer rolls it back and later disconnect does not invalidate the point-in-time result. An outcome that may have crossed rename is `indeterminate`; restart and retry reconcile through the idempotency key. |
| D74      | A committed Environment reference and canonical path persist in the shared document and project through authorized Electron, Browser, and semantic reads. Requested path, failures, and raw operating-system diagnostics are neither persisted nor logged nor emitted as telemetry. This supersedes D65 only for the committed canonical path.                                    |
| D75      | A remote Directory is forced to `available: false`; it cannot create a Workspace, resolve terminal CWD, or dispatch a Host effect. Project removal deletes only product metadata. This slice adds no remote delete, listing, file bytes, Git, shell, terminal, Agent, sync, renderer creation UI, or CLI operation.                                                               |
| D76      | Registered TLA+ admission and migration models, property evidence, Desktop file migration and authority tests, and full repository verification cover the slice. Real SSH and product UI remain excluded.                                                                                                                                                                         |

## Accepted decisions D77-D86

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D77      | Phase 3 is sliced. Slice 3.1A owns only remote-execution component construction, signing, verification, and Desktop packaging; it creates no remote installation, process, Workspace, or terminal effect.                                                                                                                                                                                                                                                  |
| D78      | Existing `orchbee-terminal-host` remains the sole PTY, process-tree, Ghostty-state, and terminal-recovery authority. SSH Runtime does not reimplement terminal behavior and gains no generic execution, shell, or network listener in this slice.                                                                                                                                                                                                          |
| D79      | `ssh-environment-release-v2` is independent from frozen v1 and contains exactly `ssh-environment-runtime` then `terminal-host`; each component contains exactly `darwin-arm64` then `linux-x64`, for four artifacts total.                                                                                                                                                                                                                                 |
| D80      | Runtime and Terminal Host retain distinct semantic component versions, sizes, and SHA-256 digests. The signed manifest digest identifies the compatibility cohort; it does not replace Runtime build identity or future Terminal Host epoch identity.                                                                                                                                                                                                      |
| D81      | Both components use Zig `0.16.0`, `ReleaseSafe`, stripping, and explicit versions. The exact targets are Apple Silicon macOS and x64 glibc Linux, with artifact names `orchbee-ssh-environment-runtime` and `orchbee-terminal-host`.                                                                                                                                                                                                                       |
| D82      | Each native application owns its build recipe. Repository-level release automation owns cross-application prepare/finalize and consumes installed build output; neither application imports the other's source tree.                                                                                                                                                                                                                                       |
| D83      | Prepare is offline and finalize accepts one external raw Ed25519 signature. One canonical signature covers every component role, version, platform, size, and digest. Layouts are exact, symlinks and extra entries fail closed, every artifact is bounded at 128 MiB, and the manifest is bounded at 64 KiB. Production private keys remain outside repository tooling.                                                                                   |
| D84      | Explicit Desktop release packaging independently verifies the signature, manifest, four artifacts, and public key before staging. It places the exact trust descriptor inside `app.asar` and buffered manifest/artifacts beneath `Resources`; ordinary development packaging remains release-free.                                                                                                                                                         |
| D85      | Frozen v1 artifacts remain valid only for already delivered Runtime and `project-root-v1` behavior. A future Terminal Host capability requires verified v2 and cannot use a v1 companion, remote `PATH`, remote build, network download, mixed cohort, or validation-failure downgrade.                                                                                                                                                                    |
| D86      | Evidence covers strict schemas and canonical bytes, missing/reordered/expanded/oversized/tampered inputs, wrong keys and signatures, symlinks, fixed build arguments, real target compilation, prepare/finalize/package round trips, documentation pairing, and the repository gate. Real SSH, remote upload, production signing, Workspace, terminal execution, and UI remain outside the claim; this bounded sequential workflow requires no TLA+ model. |

## Accepted decisions D87-D96

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                         |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D87      | Slice 3.1B owns immutable remote installation and reuse of the signed Runtime/Terminal Host cohort, one opaque Terminal Host relay, and a Desktop Main `HostConnection`. It creates no terminal, Workspace, renderer, CLI, Skill, or Agent effect.                                                                                                                        |
| D88      | One existing OpenSSH stdio attachment carries both `project-root-v1` and `terminal-host-v1`. The Runtime gains neither a second SSH connection, forwarding, network listener, generic execute, nor shell authority.                                                                                                                                                       |
| D89      | Bootstrap is exact `OBSI/2`: Runtime then Terminal Host status/upload, followed by one `OBSC/2` commit barrier. Runtime starts only after both exact artifacts are present; v1, downgrade, mixed cohorts, reordered components, and bytes after commit fail closed.                                                                                                       |
| D90      | Runtime remains under `~/.orchbee/ssh-runtime/versions/<version>` and Terminal Host installs under `~/.orchbee/terminal-host/versions/<version>`, each with a private `state` directory. Terminal Host endpoint identity is domain-separated from Runtime identity.                                                                                                       |
| D91      | `OBSE` is exactly `0.2.0`, capabilities are exactly `[project-root-v1, terminal-host-v1]`, and Hello-Accepted binds Runtime build, release cohort, Terminal Host version/build, platform, and epoch. There is no dual stack or downgrade.                                                                                                                                 |
| D92      | Main explicitly opens at most one relay. Runtime authenticates and connects to the same-UID Host endpoint or launches the exact signed Host path with fixed arguments. Channel identifiers are globally monotonic across services for one attachment.                                                                                                                     |
| D93      | The relay has 64 KiB credit and 64 KiB maximum chunks in each direction. Host connection and I/O run in an isolated worker so they cannot block heartbeat or the four project-root workers. `service-busy`, `service-unavailable`, and `compatibility-unsupported` are channel-local; outer corruption retires the attachment.                                            |
| D94      | Attachment loss closes only the relay broker and never replays an effect whose outcome is unknown. A surviving Host with retained terminals remains authoritative; reconnect creates a fresh relay and uses Host epoch/inventory reconciliation.                                                                                                                          |
| D95      | Only the exact compatible Host is reused. Runtime never kills, replaces, upgrades, downgrades, mixes, cleans up, or installs an operating-system service; there is no mutable `current` pointer or linger policy.                                                                                                                                                         |
| D96      | Registered TLA+ models plus adversarial codecs, deterministic process tests, and a real local Runtime-to-Host process trace cover cohort activation, relay exclusivity, credit, local-failure containment, attachment loss, and no replay. Real SSH, production signing, terminal creation, Workspace, UI, CLI, Skill, and Agent orchestration remain outside this slice. |

## Accepted decisions D97-D106

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                 |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D97      | Slice 3.2A extends the Main-owned remote-Project admission with exactly one durable Workspace placement. It creates no terminal presentation, Terminal Host effect, renderer-owned authority, UI, CLI, Skill, or Agent operation.                                                                                                                                 |
| D98      | `WorkbenchSharedDocument` remains exact version 2. A remote Workspace uses the existing Workspace fields, references one Environment-placed primary Directory, and owns one otherwise-valid empty Terminal Context. No file-name, schema-shape, or migration change is required.                                                                                  |
| D99      | Admission still requires one registered, connected SSH Environment and a successful `project-root-v1` canonical inspection before entering the Main mutation queue. The global idempotency key remains `(EnvironmentId, canonicalPath)`.                                                                                                                          |
| D100     | A missing remote Project and its Workspace commit atomically. Re-admitting a Slice 2.5 Project adds the missing Workspace atomically; re-admitting a Project that already owns that Workspace is unchanged. Sequence-owned Workspace and Terminal Context identities never reuse a deleted identity.                                                              |
| D101     | The Workspace name derives from the canonical path, references the admitted Directory, remains inactive, and does not expand its Project. Its Terminal Context has no Tabs, Panels, presentations, lifecycle intent, launch CWD, or pending removal.                                                                                                              |
| D102     | Environment Directories remain durably `available: false`; connection availability remains an observation outside the shared document. A remote Workspace can remain durable while disconnected, but terminal creation, restart, split, duplicate, primary edit, launch configuration, and local-CWD fallback remain rejected in this slice.                      |
| D103     | A normal Client proposal may retain or delete an exact canonical remote Workspace and its empty context. It cannot introduce, reparent, relabel, mutate, duplicate, convert, or add Terminal state to remote placement. Only the Main admission owner may introduce it.                                                                                           |
| D104     | Queue admission remains the cancellation commit point. Cancellation or attachment loss before it creates nothing; afterward it cannot roll back the queued mutation. Project, Workspace, Directory, and empty Terminal Context share one document commit, including pre-rename failure and post-rename indeterminate recovery.                                    |
| D105     | Project or Workspace deletion removes only product metadata and the empty context. It performs no remote filesystem deletion, Host close, terminal archive, Git, shell, source transfer, synchronization, or Environment lifecycle operation.                                                                                                                     |
| D106     | The expanded remote-admission TLA+ model, deterministic Workbench transition/document/runtime tests, Desktop authority and persistence tests, Client-forgery tests, documentation pairing, and the repository gate cover this slice. Real SSH, Terminal Host protocol effects, nonempty remote contexts, UI, CLI, Skill, and Agent orchestration remain excluded. |

## Accepted decisions D107-D116

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| -------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D107     | Slice 3.2B authorizes one explicit Desktop Main process-local operation to provision or reconcile the first terminal of an admitted SSH Environment Workspace. It adds no renderer creation API, semantic CLI operation, Skill, Agent authority, split, restart, close, or general remote command.                                                                                                                                                                                                    |
| D108     | The input is one `EnvironmentId`, Project identity, and Workspace identity. Admission requires the registered Environment, an already-connected observation, and a canonical Workspace whose primary Directory is placed on that exact Environment. The operation never starts SSH, waits for reconnect, accepts a path hint, or changes active/expanded Client View state.                                                                                                                           |
| D109     | The remote Workspace is the bounded idempotency key and owns at most one terminal in this slice. An empty canonical context may receive one Main-owned Tab, Panel, and Presentation; an exact retry returns and reconciles that same durable Presentation and operation identity. Any other pre-existing remote-context shape fails closed.                                                                                                                                                           |
| D110     | `WorkbenchSharedDocument` remains exact version 2. The new Presentation uses the existing provision intent and the canonical remote Directory path as `launchCwd`; the Environment Directory remains `available: false`, and no local, Project, system, or caller-supplied CWD fallback is permitted.                                                                                                                                                                                                 |
| D111     | Terminal effect routing derives only from durable Presentation containment through Terminal Context, Workspace, primary Directory, and Environment placement. Local placement can reach only the local Host; Environment placement can reach only that Environment's remote Host. Caller hints and matching path text grant no routing authority. A Client may retain or delete the exact canonical remote state but cannot introduce, move, mutate, duplicate, or retarget it.                       |
| D112     | Main prepares at most one cached process-local remote Terminal runtime and relay per Environment before entering the document mutation queue. Its binding ledger and archive namespace are deterministic and isolated per Environment. The admitted POSIX targets use a fixed `/bin/sh` launch with the canonical remote CWD and bounded terminal defaults; local executable paths, environment deltas, shell integration, CLI endpoints, profiles, and secrets are not copied to the remote process. |
| D113     | The canonical document commit precedes every Terminal Host create effect. Reconciliation may emit the Host command only after durability is confirmed. Cancellation before queue admission creates no document or terminal effect; relay preparation alone is not a terminal effect. Cancellation after the commit point cannot roll back the intent, binding, or remote outcome.                                                                                                                     |
| D114     | A disconnected or unprepared target defers the durable intent and never falls back to the local Host or publishes a synthetic terminal failure. An attachment loss or unknown Host response preserves the Environment-scoped binding and Creation identity. A later explicit repeat may open a fresh relay and reconcile Host epoch, inventory, and the same Creation identity; it never blindly creates a second terminal or automatically activates SSH.                                            |
| D115     | SSH disconnect and Desktop shutdown retire only the process-local relay/runtime attachment. They do not delete Project, Workspace, Terminal Context, binding identity, or retained remote terminal, and do not send a terminal stop. Remote projection, input, resize, metadata, exit publication, close, restart, split, archive capture, and application-stop draining remain reserved for later slices.                                                                                            |
| D116     | A dedicated TLA+ model checks target authority, singleton/idempotency, commit-before-effect, cancellation, deferred readiness, unknown-outcome reconciliation, and no local fallback or duplicate create. Deterministic Workbench, reconciler, runtime-owner, Desktop composition, persistence, anti-forgery, documentation-pairing, and repository-gate evidence refine it. Real SSH interoperability and every excluded interactive/lifecycle capability remain outside this slice's claim.         |

## Accepted decisions D117-D126

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D117     | Slice 3.2C authorizes live interactive projection of the existing singleton remote terminal for Electron and authenticated Gateway Browser clients. It adds no renderer terminal-creation API, semantic CLI operation, Skill, Agent authority, additional remote terminal, split, restart, close, or general remote command.                                                                                                                                                                                                                  |
| D118     | Main derives the projection target only from canonical Presentation containment through Terminal Context, Workspace, primary Directory, and placement. A projection request supplies only `TerminalPresentationId` and viewport geometry; caller hints, path text, Host identity, and Environment identity grant no routing authority. Local placement reaches only the local runtime, and Environment placement reaches only that exact Environment runtime.                                                                                 |
| D119     | A remote projection requires the exact Environment runtime to be already prepared and connected. Projection open never starts SSH, waits for reconnect, activates an Environment, prepares a relay, or falls back to the local Host. An unavailable target returns the existing retryable projection failure, while the bounded surface recovery policy may retry only the projection request.                                                                                                                                                |
| D120     | Exact target resolution immediately before runtime open is the projection admission point. If the retained terminal still requires start, that runtime may issue `StartTerminal` with the requested viewport geometry. Later deletion cannot roll back an already admitted Host start, but Main re-resolves canonical placement before publishing the lease; a missing or changed target disposes the attachment and publishes no projection.                                                                                                 |
| D121     | An admitted remote projection reuses the existing attachment contract without a remote variant: fresh `AttachmentId`, read/input/resize capability, input capability revision, bounded command admission, and serialized multi-client resize-lease ownership. The Renderer receives no HostConnection, Environment credential, binding ledger, SSH channel, or remote path authority.                                                                                                                                                         |
| D122     | Key, text, paste, mouse, focus, selection, search, and resize commands keep their existing bounded projection semantics. Input and another effect whose Host admission outcome becomes unknown are never replayed. Electron keeps Desktop-owned clipboard and allowlisted-link effects; a Gateway Browser keeps the same client-local effect target. Transport choice grants no additional side effect.                                                                                                                                       |
| D123     | Every Environment projection uses only its cached Environment-owned runtime, HostConnection, binding ledger, and recovery namespace. It cannot inspect or fall back to the local runtime, local binding, or local terminal archive. The local projection path and archive behavior remain unchanged.                                                                                                                                                                                                                                          |
| D124     | This slice admits attachment-scoped frames, bell, selection, search, clipboard requests, transient title/CWD/theme observations, and terminal-outcome delivery to the live surface. Durable remote metadata and exit publication, close, restart, split, archive capture, application-stop draining, and reconnect reconciliation remain reserved.                                                                                                                                                                                            |
| D125     | Client release detaches only that projection. SSH disconnect, Environment runtime retirement, renderer retirement, and Gateway session disposal revoke their owned projection leases without deleting Project, Workspace, Presentation, binding identity, or retained remote terminal and without sending terminal stop. Open/release operations remain serialized by the existing per-Presentation and per-Environment owners.                                                                                                               |
| D126     | A dedicated TLA+ model checks canonical target authority, prepared-runtime admission, start/open/delete races, stale-lease suppression, disconnect retirement, no SSH activation, and no local fallback. Existing projected-terminal reliability, broker-admission, attachment-capability, deterministic runtime-owner/router, Electron/Gateway composition, documentation-pairing, and repository-gate evidence complete the refinement. Real SSH interoperability and every reserved durable lifecycle capability remain outside the claim. |

## Accepted decisions D127-D136

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                            |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D127     | Slice 3.2D authorizes durable metadata, terminal-failure, and terminal-exit observation only for the existing singleton remote terminal. It adds no terminal creation, close, restart, split, archive, general command, CLI, Skill, or Agent authority.                                                                      |
| D128     | Main subscribes only to Terminal Host envelopes from the current connection record for the exact Environment. Protocol epoch validation and a current-record fence reject stale or foreign records before they can affect runtime or Client state.                                                                           |
| D129     | Every remote recovery, deduplication, binding, and publication identity is namespaced by `EnvironmentId` plus `TerminalIdentity`. It cannot collide with the local runtime or another Environment even when raw Host identities match.                                                                                       |
| D130     | Title and current-working-directory metadata are accepted monotonically by Host revision, resolved through the exact Environment runtime binding, and retained by `DesktopClientSessionCoordinator` for bootstrap without mutating the shared Workbench document.                                                            |
| D131     | `TerminalFailed` is published only after the exact remote binding resolves to its canonical Presentation. SSH disconnect, relay retirement, and unavailable runtime state never synthesize terminal failure.                                                                                                                 |
| D132     | A non-acknowledgement terminal-exit envelope enters the exact Environment's synchronous durable pending or rejection journal before the Host listener returns. Failure to preserve that outcome fails closed instead of acknowledging and losing it.                                                                         |
| D133     | Remote exit recovery orders effects as durable binding outcome, reconciler reservation, publication ledger commit, Client broadcast, then pending-journal compaction. Retry cannot publish before its durable predecessors.                                                                                                  |
| D134     | Canonical deletion or retargeting cannot redirect an admitted outcome. Missing, changed, or unresolved authority conservatively retains the Environment-scoped outcome for later reconciliation and never publishes it to another target.                                                                                    |
| D135     | An admitted outcome survives relay loss and Desktop restart and is reconsidered when that exact Environment runtime is next explicitly prepared. Recovery never activates or reconnects SSH by itself.                                                                                                                       |
| D136     | A dedicated TLA+ crash, retry, ordering, and Environment-isolation model plus deterministic owner, persistence, routing, coordinator, and composition tests refine this slice. Close, restart, split, archive capture, application-stop draining, UI creation, semantic CLI, Skill, and Agent orchestration remain reserved. |

## Accepted decisions D137-D146

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                           |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D137     | Slice 3.2E authorizes only the Electron renderer product Action that creates or reconciles the first singleton terminal of an existing remote Workspace. Environment enrollment, SSH connect/reconnect/AskPass, Project admission, additional terminals, groups, splits, restart, close, CLI, Skill, Agent, and Gateway creation remain excluded.                                           |
| D138     | The existing `workbench.terminal.create` Action, New Terminal controls, and shortcut are reused. Local behavior is unchanged. A remote Workspace admits only default placement while its exact Terminal Context is empty; after the singleton exists, every terminal-creation surface is unavailable.                                                                                       |
| D139     | Renderer input contains only `ProjectId` and `WorkspaceId`. Main derives `EnvironmentId`, canonical path, and Terminal Context from the current canonical document and revalidates them at commit. Missing, stale, mismatched, or local targets reject without local fallback.                                                                                                              |
| D140     | Electron bootstrap gains one purpose-specific `remote-terminal-creation` `MessagePort`. The preload bridge transfers it as the tenth exact bootstrap role. The capability is not exposed through a preload global, client document proposal, authenticated Gateway, or general command transport.                                                                                           |
| D141     | Main requires the derived Environment to be registered and currently connected. It may prepare or reuse that Environment's Terminal Host runtime, but this operation never starts, reconnects, waits for, or prompts for SSH.                                                                                                                                                               |
| D142     | Only default top-level placement is admitted. Concurrent and retried requests converge to the canonical created or existing singleton and cannot create a duplicate terminal, Tab, Group, Panel, or Presentation.                                                                                                                                                                           |
| D143     | Only a Main-confirmed pre-commit abort returns `cancelled`. Cancellation or unmount after commit does not roll back. Unconfirmed durability and control-port loss after dispatch return an outcome-unknown result rather than false cancellation.                                                                                                                                           |
| D144     | Renderer state never mutates optimistically. It waits for canonical client-session publication and assumes no ordering between the creation port and document publication. A late completion may update status but cannot navigate, select, or steal focus.                                                                                                                                 |
| D145     | The Action is enabled only for an exact empty remote context with the Electron capability. Missing capability, invalid context, unsupported placement, or an existing singleton disables Tab Strip controls, shortcuts, and Action execution consistently. Failures are bounded and localized and expose no credential, path, raw transport error, or log content.                          |
| D146     | This Consequential and Modeled slice updates the TLA+ abstraction before protocol, broker, client, Action, availability, and composition code. Deterministic tests cover cancellation, commit boundaries, concurrency, retry, port retirement, stale authority, no local fallback, and no automatic SSH activation. Full `pnpm verify` excludes real SSH and the manual native-window test. |

## Accepted decisions D147-D156

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D147     | Slice 3.3A authorizes the POSIX Electron Desktop `Connect via SSH…` enrollment and recovery surface. One explicit request establishes or reuses an SSH Environment, connects it, validates one remote Project root, and admits the canonical Project plus Workspace. macOS Desktop and Linux Desktop may target the already-admitted macOS arm64 and glibc Linux x86_64 remote platforms; Windows Desktop, Gateway, Browser Preview, CLI, Skill, and Agent enrollment remain excluded.                                                                                                                                                               |
| D148     | The existing Workbench Project-create and Project-navigation experiences gain the SSH entry, remote status, and Connect, Retry, and Disconnect controls. The multi-step credential and progress ceremony is a new Desktop-owned business dialog in `@orchbee/web`; it does not turn the shared resource form into a workflow or fork the Workbench Project model.                                                                                                                                                                                                                                                                                    |
| D149     | Renderer enrollment input contains a bounded Environment display name, an SSH destination or alias, an optional absolute SSH configuration path, and one absolute POSIX Project path. Main validates the boundary, runs the existing fixed `ssh -G` resolver, and allocates opaque identities. Renderer input never contains an Environment identity, Runtime identity, resolved hostname, SSH argument vector, shell fragment, credential, or host key. An exact existing target reuses its immutable identity and authoritative name.                                                                                                              |
| D150     | Electron bootstrap gains one purpose-specific `ssh-project-enrollment` `MessagePort` as its eleventh exact role. Bounded request, generation, prompt-handle, progress, status, cancellation, and result records cross that port. Main grants at most one interaction lease for an Environment; only its initiating request may answer the exact current prompt. Port loss revokes the lease, prompts are never reassigned, and submitted secrets remain transient AskPass responses absent from React persistence, diagnostics, telemetry, disk, and canonical documents.                                                                            |
| D151     | `environment-registry-v1.json` remains the unchanged append-only identity and exact-target owner. A canonical Workbench Environment placement is the only durable proof that a Project is enrolled. A registry-only record is a recoverable pending identity, including legacy records and records left by cancellation or failure. Slice 3.3A adds no second lifecycle journal, registry delete, rename, repoint, retirement, remote cleanup, or alias merging; an exact target always reuses its admitted identity.                                                                                                                                |
| D152     | Main orders enrollment as target resolution, durable Environment create or reuse, SSH/AskPass/Runtime readiness, canonical Project-root inspection, canonical Workbench commit, then result and client-session publication. Cancellation confirmed before Workbench commit stops the request while preserving any durable registry identity; once commit may have happened, cancellation, unmount, or port loss cannot roll it back. Registry and Workbench writes are not claimed atomic: indeterminate results fail closed and restart or explicit reload reconciles their independently durable facts.                                            |
| D153     | Desktop restart never activates SSH. Every enrolled remote Project derives its process-local status from the Environment connection observation and offers explicit Connect or Retry. A proven live-connection loss continues through the existing generation-fenced automatic retry; a replacement generation that requires interaction without a live renderer lease blocks recoverably until an explicit request supplies one. Disconnect retires only local attachment/runtime resources and preserves the Environment, Project, Workspace, and retained remote work.                                                                            |
| D154     | A successful new enrollment expands and activates the admitted Workspace only in the initiating Client View after canonical publication. Other clients receive the canonical Project without a focus or navigation effect. Enrollment creates no terminal; the existing Electron New Terminal operation remains the sole first-terminal surface.                                                                                                                                                                                                                                                                                                     |
| D155     | Ordinary development and normal Desktop packages remain release-free and fail closed with localized build-unavailable status. Real activation evidence uses only the existing explicit release package path with a finalized signed two-component cohort and independently supplied Ed25519 public key. Slice 3.3A adds no unsigned release, ambient artifact lookup, private signing key, download feed, or development trust bypass.                                                                                                                                                                                                               |
| D156     | This Consequential and Modeled slice persists and checks `SshEnvironmentProjectEnrollment` before dependent production behavior. Deterministic evidence covers parsing, identity reuse, owner serialization, prompt correlation and secrecy, cancellation and port retirement, status projection, reconnect, Workbench commit and publication ordering, localized UI, and restart reconciliation. Controlled loopback `sshd` evidence additionally covers host-key confirmation, encrypted-key AskPass, signed Runtime upload, Project admission, terminal I/O, and network interruption; full `pnpm verify` excludes the manual native-window test. |

## Accepted decisions D157-D167

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| D157     | The POSIX Electron SSH Project-enrollment ceremony is connect first and choose a directory second. Its initial surface asks for exactly one SSH address, connects through the existing bounded OpenSSH owner and exact AskPass interaction, and only after a connected Runtime exists presents a read-only remote-directory chooser. This extends D147-D156 without adding Windows Desktop, Gateway, Browser Preview, CLI, Skill, or Agent enrollment.                                                                                                                                                                                                                                                                                                                                                                         |
| D158     | Renderer enrollment input is reduced to one bounded SSH destination. It accepts the ordinary OpenSSH destination forms `alias` and `[user@]host`, plus an optional exact leading `ssh ` for paste compatibility; it does not accept options, shell syntax, or an argument vector. Main removes the optional prefix, validates the single destination, derives a bounded Environment display name, and retains native OpenSSH configuration discovery. When the Desktop application home differs from the OS-account home and contains `.ssh/config`, Main supplies that file as the per-user configuration so the application's filesystem context remains coherent. This supersedes D149 for new enrollment; previously stored exact targets, names, and optional configuration paths remain reconnectable without migration. |
| D159     | The exact `OBSE` protocol becomes `0.3.0`, the Runtime cohort becomes `0.2.0`, and capabilities become exactly `[project-root-v2, terminal-host-v1]`. `project-root-v2` retains canonical inspection and adds bounded directory listing; there is no dual stack, downgrade, mixed cohort, remote build, or ambient artifact selection. Older signed cohorts remain available to older Desktop builds. This supersedes D91 for the new cohort.                                                                                                                                                                                                                                                                                                                                                                                  |
| D160     | The first directory snapshot resolves the Runtime process's validated absolute `HOME`. Each listing returns a canonical absolute path, optional canonical parent, at most 256 deterministically sorted accessible UTF-8 directory names, and a `truncated` flag inside the bounded Project-root message and timeout budget. Files, file contents, invalid names, inaccessible entries, and mutation are excluded; directory symlinks may appear only when their targets can be opened as directories. An editable absolute-path navigation field remains the complete fallback for truncated or very large directories.                                                                                                                                                                                                        |
| D161     | Main owns exactly one transient directory-selection interaction for the initiating request. Every published snapshot carries the exact request identity and monotonically increasing revision; only an exact current response may navigate or select. Failed navigation keeps the last valid snapshot and returns a bounded localized failure. Selecting accepts only the current snapshot's canonical path, after which Main performs the existing independent canonical inspection before Workbench commit; a listing never becomes durable path authority.                                                                                                                                                                                                                                                                  |
| D162     | Enrollment is ordered as target resolution, durable Environment create or reuse, SSH/AskPass/Runtime readiness, initial home listing, zero or more directory navigations, exact selection, canonical Project-root inspection, and canonical Workbench commit. Initial-listing failure fails enrollment; cancellation or port loss during directory choice is pre-commit and retires only an operation-owned connection that is not referenced by an enrolled Project. Existing cancellation, indeterminate-write recovery, publication, activation, and no-terminal rules remain unchanged.                                                                                                                                                                                                                                    |
| D163     | Directory snapshots are transient initiating-renderer data and never enter the registry, Workbench document, diagnostics, telemetry, or logs. The new service cannot read file bytes, enumerate files, execute commands, open a terminal, mutate the remote host, or escape the connected Environment's existing same-UID authority. Credentials remain exact correlated AskPass responses and never enter controller state.                                                                                                                                                                                                                                                                                                                                                                                                   |
| D164     | Rollout and rollback are cohort based. A Desktop build that requires `project-root-v2` installs and admits only the exact signed `0.2.0` Runtime cohort while retained older cohorts continue serving compatible older builds. The change performs no Environment-registry or Workbench-document migration and does not rewrite existing SSH targets.                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| D165     | The existing canonical `desktop.ssh-project-enrollment-dialog` experience is extended rather than forked. The address and path navigation controls are native forms whose Enter key performs the primary step; both stages, progress, prompts, failures, empty/truncated listings, cancellation, and selection ship with equivalent `en-US` and `zh-CN` product copy and keyboard-accessible semantics.                                                                                                                                                                                                                                                                                                                                                                                                                        |
| D166     | This Consequential and Modeled extension updates and checks the enrollment and Project-root TLA+ abstractions before production behavior. Deterministic evidence covers input normalization, exact protocol codecs, listing bounds and failures, worker lifecycle, Main interaction correlation, stale revisions, cancellation, controller secrecy, Enter submission, localized rendering, and final re-inspection. Controlled loopback evidence covers authentication followed by home listing, navigation, selection, Project admission, and terminal I/O; full `pnpm verify` continues to exclude the manual native-window test.                                                                                                                                                                                            |
| D167     | The app-owned canonical `web.remote-directory-browser` experience is a controlled, source-neutral read-only view over bounded directory snapshots. The SSH enrollment dialog composes it through an SSH-owned adapter; a future Gateway or cross-device client may compose the same experience only after its owning protocol separately authorizes, correlates, and bounds directory listing. The component itself grants no transport or filesystem authority and contains no file listing, file bytes, mutation, generic command, or terminal behavior.                                                                                                                                                                                                                                                                     |

## Accepted decisions D168-D176

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D168     | Electron Project management extends the canonical `workbench.project-manager-dialog` instead of adding a parallel manager. When the SSH capability is present, its final Add Folder row opens an explicit choice between one local Folder and one SSH Folder. Local acquisition retains the native picker; SSH acquisition opens the existing one-address SSH dialog and composes the canonical remote-directory browser after authentication. Browser Preview and Gateway retain their current manual local-path adaptation and receive no SSH attachment authority.                                                                                                                                                                        |
| D169     | An SSH Folder-attachment request carries only the exact current Project identity and the bounded SSH destination. Main derives or reuses the Environment, connects through the existing AskPass lease, lists and selects a remote directory, independently re-inspects the selected canonical path, and revalidates the target Project immediately before the canonical Workbench mutation. The renderer never supplies Environment identity, Directory identity, resolved target facts, or canonical-path authority.                                                                                                                                                                                                                        |
| D170     | The attachment commit appends exactly one unavailable Environment-placed Folder to the target Project. It does not rename the Project, change its primary Directory, create or modify a Workspace, change active Client View location, create a terminal, or alter an existing terminal context. Project-name Save remains an independent action, while Folder attachment is visible only through ordinary canonical document publication.                                                                                                                                                                                                                                                                                                   |
| D171     | One Project may reference at most one SSH Environment identity in this slice, while it may contain any number of distinct canonical directories from that Environment together with local Directories. Reusing the same exact SSH target preserves the Environment identity. A different Environment for a Project that already has one fails before Workbench commit, preserving the existing Project-level Connect, Retry, and Disconnect semantics.                                                                                                                                                                                                                                                                                       |
| D172     | Exact `(EnvironmentId, canonical path)` equality inside the target Project is idempotent and settles as existing without a document write. The same remote location already owned by another Project and every identity or location collision fail closed. A removed target Project, a late duplicate, and an Environment conflict are rechecked inside the canonical mutation and cannot redirect the Folder to another Project.                                                                                                                                                                                                                                                                                                            |
| D173     | The existing dedicated SSH renderer port adds correlated Folder-attachment request, progress, directory-selection, prompt, cancellation, and bounded result records; it does not gain a generic filesystem or Workbench mutation RPC. Enrollment, attachment, connect, and disconnect share one active-request boundary. Port loss revokes transient prompt and directory-selection authority; a commit that may already have occurred is never rolled back.                                                                                                                                                                                                                                                                                 |
| D174     | The canonical Project manager renders an explicit localized SSH marker for every Environment-placed Folder, including its accessible identity, while local Folders remain unmarked. The complete canonical path remains available through the existing Folder tooltip. The marker derives only from canonical placement and does not expose an Environment identifier, resolved hostname, credential, or transport diagnostic.                                                                                                                                                                                                                                                                                                               |
| D175     | A successful attachment closes only the SSH overlay after the initiating renderer observes the exact Project, returned Directory identity, canonical path, and Environment placement in the published Workbench document; the underlying Project manager then reflects the new row. Cancellation or a known pre-commit failure returns without Folder mutation. Registry identity remains append-only, and an indeterminate registry or Workbench write follows the existing reconciliation and outcome-unknown policy.                                                                                                                                                                                                                      |
| D176     | This Consequential and Modeled extension persists and checks `SshEnvironmentProjectFolderAttachment` before dependent production behavior. Deterministic evidence covers attachment to an existing local Project, the pure remote-directory transition, protocol parsing, request serialization, target and Environment revalidation, duplicate and cross-Project rejection, prompt and directory correlation, cancellation, publication waiting, localized menu and marker rendering, and the unchanged Project fields in D170. Controlled loopback evidence extends the existing SSH enrollment system test with a second Folder attachment to an existing Project; full `pnpm verify` continues to exclude the manual native-window test. |

## Accepted decisions D177-D184

| Decision | Accepted contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D177     | Electron Desktop Settings gains one dedicated **SSH Connections** category for reusable Environment-level connection intent. It extends the canonical Terminal Settings surface and does not create a second settings shell, Project manager, or Browser/Gateway SSH authority. Browser Preview and Gateway omit the category when the Desktop capability is absent.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| D178     | The page separates **Your connections** from **From your SSH config**. Managed rows come only from the append-only Environment registry and expose bounded display name, SSH destination, Project-reference count, and current process-local status. Suggested rows come only from automatic read-only discovery and are never represented as managed until an explicit Add action succeeds.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| D179     | Discovery starts from the Desktop application user's `~/.ssh/config`, follows bounded user-config `Include` expansion, and returns at most 1,024 concrete literal `Host` aliases. Wildcards, negations, option-like tokens, malformed values, duplicate aliases, configuration bodies, absolute configuration paths, resolved hostnames, users, ports, identity files, Proxy commands, and credentials never cross the renderer boundary. Missing configuration means an empty successful discovery; unreadable, invalid, or truncated discovery remains a bounded visible issue.                                                                                                                                                                                                                                                                                                                                                                                                        |
| D180     | Opening Settings or refreshing discovery may read only the bounded local configuration graph. It starts no OpenSSH child, performs no DNS or network access, prompts for nothing, writes no configuration, and changes no registry or Workbench state. Discovery is cached between explicit refreshes; registry and connection observations still publish current managed rows to every attached Desktop renderer.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| D181     | Manual Add and suggested-host Add both supply one D158 SSH destination. Main applies the existing fixed `ssh -G` security-floor resolver before the existing append-only create-or-reuse operation. A successful Add records reusable intent but does not connect, prompt, choose a directory, create a Project, attach a Folder, create a Workspace, or create a terminal. Exact managed targets disappear from suggestions; distinct exact targets are not heuristically merged.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| D182     | Connect, Retry, and Disconnect are explicit Environment-level actions. They reuse the existing connection owner, one-supervisor-per-Environment lifecycle, transient interaction lease, AskPass prompt UI, retry fencing, and disconnect retirement. Project-level and Settings-level actions observe the same connection; passive discovery and Desktop restart never activate it.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| D183     | The dedicated renderer contract carries only correlated requests, opaque Environment identities, bounded catalog rows, bounded discovery issues, connection state, Project-reference counts, prompt handles and text, and bounded settlements. Main remains the only reader of SSH configuration and writer of Environment intent. One renderer request is active at a time; stale responses, port loss, and competing prompt leases fail closed, while secrets remain absent from React snapshots, catalogs, storage, diagnostics, and canonical documents.                                                                                                                                                                                                                                                                                                                                                                                                                             |
| D184     | This Consequential and Modeled extension records the catalog and transition contract before production integration. The registered two-scenario `SshEnvironmentConnectionManagement` model checks stale-refresh fencing, append-only Add ordering, passive-operation non-activation, explicit connection ownership, exact prompt leases, port retirement, and the absence of Project, Directory, Workspace, and terminal effects. Deterministic evidence additionally covers bounded recursive discovery, concrete-host filtering, missing and partial configuration, exact suggestion suppression, resolver-before-record ordering, idempotent Add, shared status projection, explicit connect/disconnect, prompt correlation and secrecy, multi-renderer publication, localization, search, and capability absence. Controlled loopback evidence remains responsible for real OpenSSH/AskPass/runtime behavior; full `pnpm verify` continues to exclude the manual native-window test. |

### SSH connection management model

| Modeling field            | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem                   | A Desktop user cannot see reusable SSH Environments in one place, cannot tell whether a connection is active or referenced by Projects, and cannot adopt an existing OpenSSH Host alias without already knowing Orchbee's Project-enrollment flow.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Inputs                    | Untrusted inputs are one bounded renderer request identity, one D158 destination, one opaque registered Environment identity, transient correlated AskPass responses, and the bounded local user-configuration graph rooted at `~/.ssh/config`. Absence of the root configuration is an empty successful discovery; unavailable registry or configuration reads are explicit issues, never empty-authority fallbacks.                                                                                                                                                                                                                                                                                                                                                                                                 |
| Data and ownership        | The Environment registry owns immutable managed target intent. OpenSSH configuration remains external user-owned input and is never copied into Orchbee. The connection owner owns supervisors and observations. The canonical Workbench document owns Project references. Desktop Main derives the catalog; each renderer controller owns only a revocable projection and transient request state.                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Outputs                   | Catalog publication contains managed rows, concrete suggestions, and at most one bounded discovery issue. Add settles added, existing, known failure, cancellation, or durability-unknown. Connect and Disconnect settle connected, disconnected, known failure, or cancellation. Secrets and raw configuration never appear in any output.                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| States and transitions    | Discovery moves idle → reading → ready or issue without remote effect. Add moves validating → resolving → recording → added/existing or failure without activation. Connect moves claimed → optional prompt → resolving/connecting → connected, blocked, cancelled, or failed. Disconnect moves requested → retiring → disconnected or failed. Registry creation and connection observation independently trigger catalog publication.                                                                                                                                                                                                                                                                                                                                                                                |
| Invariants                | Every managed row corresponds to one current registry record; every suggestion is concrete, bounded, and absent by exact target from managed rows; discovery has no process, network, prompt, or write effect; Add resolves before durable create and never connects; at most one supervisor and one prompt lease exist per Environment; only explicit actions connect or disconnect; restart never auto-connects.                                                                                                                                                                                                                                                                                                                                                                                                    |
| Decisions                 | Suggestions preserve the literal Host alias as the reusable destination. Project-reference count is informative and never becomes lifecycle authority. The append-only registry retains D151: this slice adds no remove, rename, repoint, retirement, configuration editor, wildcard materialization, or semantic alias merging.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Dependencies              | A bounded injected OpenSSH-config discovery capability owns filesystem and glob access. The existing resolver owns `ssh -G`; the Environment registry owns create/list; the connection owner owns activation and observation; the Workbench document is read-only for reference counts; the Desktop renderer protocol owns validation; the canonical Settings and SSH prompt components own presentation.                                                                                                                                                                                                                                                                                                                                                                                                             |
| Cancellation and recovery | Refresh may be superseded by a newer refresh. Add cancellation before registry dispatch has no durable effect; an indeterminate registry commit retains the existing reconciliation requirement. Connect cancellation revokes only the initiating lease and follows existing supervisor retirement rules. Port loss cancels transient authority but never rewrites configuration, deletes an Environment, or rolls back a possibly durable create.                                                                                                                                                                                                                                                                                                                                                                    |
| Security/privacy          | Discovery reads only bounded configuration files required to find aliases and publishes no file content or path. Alias strings and prompt text are untrusted display data. Add cannot bypass the existing host-key security floor. Passwords and passphrases travel only in exact current AskPass responses and are never retained.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Verification              | `SshEnvironmentConnectionManagement` isolates catalog/Add ordering from explicit connection/prompt/port lifecycle while checking both with two symmetric renderers. It proves stale catalog completion cannot publish, discovery and Add cannot activate SSH or create Workbench resources, registry growth requires resolved Add, competing operations cannot steal ownership, and prompts require the exact live lease. Pure parser and catalog tests cover traversal bounds, cycles, Include order, malformed input, alias filtering, deduplication, issue mapping, exact target suppression, and reference counts. Runtime, broker, client, controller, component, localization, bootstrap, and composition tests trace every renderer action to its Main owner and prove capability absence has no UI or effect. |

### SSH Project enrollment model

| Modeling field     | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem            | A Desktop user can already exercise every low-level SSH and remote-Project operation only through process-local methods, but has no product path that owns identity establishment, authentication, progress, Project admission, failure recovery, or reconnect.                                                                                                                                                                                                                                                                                      |
| Inputs             | The renderer supplies one request identity and the single bounded SSH destination in D158. After Main derives and connects the exact target, renderer responses may only navigate to one bounded absolute path or select the exact current canonical directory snapshot. Main derives names, identities, resolved facts, canonical paths, and connection generations.                                                                                                                                                                                |
| Data and ownership | The Environment registry owns immutable identity and exact target intent. The canonical Workbench document owns enrolled Project and Workspace structure. The connection owner owns process-local status and one supervisor per Environment. The enrollment coordinator owns request serialization and transient AskPass and directory-selection leases. The initiating Client View alone owns transient snapshots and post-publication expansion and activation. No snapshot or lifecycle projection may become a second durable authority.         |
| States             | A request progresses through target resolution, registry create/reuse, pending identity, connection or reuse, optional exact AskPass interaction, initial home listing, zero or more successful or failed directory navigations, exact directory selection, connected Project-root inspection, Workbench commit, settlement, canonical publication, and initiating-view activation. Registry and Workbench commits each admit pre-write failure and post-rename uncertainty.                                                                         |
| Invariants         | Remote effects require a durable Environment; Workbench enrollment implies that Environment; registry-only pending identity is never published as an enrolled Project; one Environment has at most one active request and one exact interaction lease; stale or foreign prompt and directory responses cannot reach their owners; inspection and commit require an exact selected current snapshot; cancellation never rolls back a possibly committed Project; restart never auto-connects; enrollment never creates a terminal.                    |
| Commit/recovery    | The Environment commit precedes every remote effect and is retained on later failure. The Workbench canonical commit is the Project-enrollment commit point. A known pre-commit failure remains pending or absent according to the registry outcome. An indeterminate registry blocks further use until reload; an indeterminate Workbench write is reconciled from the store before another request claims created or absent. Startup reloads both owners and derives enrollment only from validated canonical placement.                           |
| Cancellation       | Target resolution and pre-commit workflow phases may stop with a confirmed cancellation. An operation-owned connection with no enrolled Project is retired; a pre-existing shared connection is preserved. During or after Workbench commit, cancellation and port loss remove only the request delivery/interaction lease, while Main completes or later reconciles the authoritative commit.                                                                                                                                                       |
| Security/privacy   | Renderer and Workbench never receive a Host connection, release bytes, Runtime identity, SSH arguments, resolved target, environment capability, raw diagnostic, or secret. Prompt text and directory metadata are bounded untrusted display data accepted or answered only through exact current correlation. Listing exposes directory names and canonical paths but no file bytes, file entries, mutation, command execution, or terminal authority.                                                                                              |
| Verification       | `SshEnvironmentProjectEnrollment` checks non-atomic registry/Workbench ordering, pending identity, connection and interaction-lease exclusion, stale prompt and directory rejection, initial listing failure, navigation failure with retained authority, exact selection before inspection, busy requests, cancellation and port retirement, reconciliation, reconnect/disconnect, restart, initiating-view activation, and no terminal effect. Existing connection, registry, admission, and first-terminal models remain independent refinements. |

### Existing-Project SSH Folder attachment model

| Modeling field     | Contract                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem            | A person managing an existing Project cannot discover and attach a remote Folder through SSH, and a late or ambiguous result must not mutate a removed Project, another Project, or a Project already bound to a different Environment.                                                                                                                                                                                 |
| Inputs             | The renderer supplies one exact Project identity, one bounded SSH destination, and correlated prompt and directory-selection responses. Main derives the Environment identity and canonical path and reads the current canonical Project at commit time.                                                                                                                                                                |
| Data and ownership | The Environment registry owns immutable SSH target identity. The canonical Workbench document owns Project and Directory membership. Desktop Main owns connection, inspection, request serialization, canonical mutation, and publication observation. Workbench owns the provider-neutral Environment placement and renders the SSH-specific action only through the injected Desktop capability.                      |
| States             | A request may resolve and record an Environment, connect or reuse a connection, prompt, browse, select, inspect, revalidate the target, reject a missing target, duplicate, cross-Project location, or Environment conflict, commit or reconcile one Folder, publish it, cancel, fail, or lose its renderer port.                                                                                                       |
| Invariants         | A committed remote Folder belongs only to the exact requested live Project; its Environment is durable and is the Project's sole Environment identity; duplicate completion does not add a second Folder; no attachment changes Project name, primary Directory, Workspace structure, active location, or terminal count; stale or foreign interactions cannot commit; publication never precedes canonical commit.     |
| Commit/recovery    | The append-only Environment write precedes remote effects. The canonical Workbench mutation is the Folder-attachment commit point and rechecks every Project and location guard atomically. A known pre-commit failure leaves Folder membership unchanged. A possibly committed write is reconciled from the document store and may be retried idempotently.                                                            |
| Cancellation       | Cancellation or port loss before commit revokes the request and its transient interactions; an operation-owned unreferenced connection may be retired. Cancellation during or after a possible commit cannot roll back the Folder. The Project manager observes only canonical publication.                                                                                                                             |
| Security/privacy   | The renderer receives only bounded prompt text and directory metadata. Credentials remain transient AskPass responses. No Environment identifier, SSH argument vector, resolved target, file entry, file bytes, mutation, generic command, or terminal authority crosses the presentation boundary.                                                                                                                     |
| Verification       | `SshEnvironmentProjectFolderAttachment` checks exact-target commit, one-Environment-per-Project, idempotent duplicate settlement, cross-Project and stale-target rejection, serialization, interaction ownership, registry-before-effect ordering, cancellation and port retirement around commit, reconciliation, publication ordering, and zero unrelated Project or terminal mutation within declared finite bounds. |

### Renderer remote-terminal creation surface model

| Modeling field     | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem            | An Electron user in an admitted remote Workspace needs the ordinary New Terminal interaction to materialize the already-authorized first remote terminal without gaining Environment, SSH, Project, arbitrary placement, or general remote-command authority.                                                                                                                                                                                    |
| Inputs             | The renderer supplies one request identity plus exact `ProjectId` and `WorkspaceId`. Main reads the current canonical document, Environment registry, and current connection observation. Placement, Environment identity, canonical remote path, and context shape are never renderer inputs.                                                                                                                                                   |
| Data and ownership | Workbench owns creation availability and the structural singleton intent; the canonical shared document owns Project, Workspace, and Terminal Context authority; Desktop Main owns target derivation, connection admission, commit/reconciliation, and the dedicated request lifecycle; Client View owns navigation and focus and is not a creation-workflow writer.                                                                             |
| Outputs            | Main settles one bounded `created`, `existing`, `cancelled`, known `failed`, or outcome-unknown result. Created and existing results identify the canonical singleton. Renderer-visible terminal state arrives only through the ordinary client-session canonical publication.                                                                                                                                                                   |
| States             | A request progresses through client availability, dispatch, single Main admission, canonical target resolution, connected-runtime preparation, commit-or-existing reconciliation, settlement, and independent canonical publication. Port retirement may occur before admission, before commit, during an uncertain commit, or after commit.                                                                                                     |
| Invariants         | Main derives the Environment from the current canonical Workspace; remote creation never reaches the local runtime; no operation creates more than one canonical terminal per Workspace; only explicit SSH activation grows connectivity; renderer visibility implies canonical publication; creation completion never changes active location; only confirmed pre-commit abortion is reported as cancellation.                                  |
| Decisions          | Default placement means no requested Directory, Group, split, or insertion index. Tab Strip New Tab, New Group with Tab, and Duplicate Tab projections share the same creation-availability policy so a remote context has no alternate Tab-creation path. Existing structural lifecycle proposals remain protected by Main's remote-placement proposal fence.                                                                                   |
| Commit/publication | Canonical mutation remains the authoritative commit. A successful Host reconciliation cannot make renderer state authoritative. Commit and client-session publication are independently ordered; a late result is status-only, while canonical reconciliation chooses the required first-Tab validity fallback without a workflow-directed focus change.                                                                                         |
| Cancellation       | An abort observed and confirmed before canonical commit may settle `cancelled`. Once commit might have happened, abort, unmount, broker disposal, or port loss cannot roll back and settles or is conservatively observed as outcome unknown. A fresh request may reconcile the same singleton after a replacement port.                                                                                                                         |
| Dependencies       | `@orchbee/desktop-client-protocol` owns the bounded Electron wire contract; Desktop Main owns the broker and canonical resolver; `@orchbee/workbench` owns the host capability and Action route; `@orchbee/tab-strip` owns consistent creation-control availability; existing SSH and Terminal Host owners remain unchanged.                                                                                                                     |
| Verification       | `SshEnvironmentRemoteTerminalCreationSurface` checks bounded cancellation, commit, retry, concurrency, stale-target, port-retirement, publication, focus, no-local-fallback, and no-auto-connect traces. Deterministic protocol, broker, client, resolver, Workbench runtime, Action, Tab Strip, preload, and composition tests form the executable refinement chain. Real SSH, network quality, and the manual native-window test are excluded. |

## Runtime topology

```text
Desktop supervisor
  ├─ owner-only AskPass IPC → transient human interaction
  └─ one OpenSSH stdin/stdout child
       └─ fixed remote bootstrap / attach
            └─ owner-only Unix socket
                 └─ per-Environment Runtime daemon and epoch
                      ├─ bounded project-root-v2 inspection/listing adapter
                      └─ one terminal-host-v1 opaque relay worker
                           └─ same-UID private endpoint
                                └─ retained Terminal Host daemon and Host epoch
```

The Desktop connection owner admits one explicit activation per durable Environment and owns one
supervisor for that active identity. The generation owns target resolution, release-catalog
admission, AskPass, OpenSSH, installed-cohort negotiation, handshake, and heartbeat. It does not own
either daemon lifetime. The Runtime advertises exactly `project-root-v2` then `terminal-host-v1`; it
interprets the former and relays the latter without parsing Terminal Host records.

Projection requests carry only `TerminalPresentationId` and viewport geometry. Desktop Main resolves
Presentation containment through Terminal Context, Workspace, primary Directory, and placement.
Local placement reaches only the local runtime; Environment placement reaches only the already
prepared runtime cached for that exact Environment. Main repeats the resolution after runtime open
and disposes the provisional lease when the target is missing or changed. Projection never prepares
or reconnects SSH, and the Environment path disables local binding and terminal-archive fallback.

## Target resolution and OpenSSH transport

`SshEnvironmentTargetReference` contains one concrete destination or alias and at most one
explicitly selected absolute configuration path. Resolution invokes, without a shell:

```text
/usr/bin/ssh -G [-F <absolute-config-file>] -- <destination>
```

The bounded result admits one effective hostname, user, port, host-key lookup name, and
`StrictHostKeyChecking` value. `yes`, `ask`, and `accept-new` are accepted; disabled checking fails
closed. OpenSSH remains the only SSH configuration and `known_hosts` authority.

The transport adapter then spawns the same absolute executable with an argument array. It pins the
resolved host identity and requires `-T`, `RequestTTY=no`, `ClearAllForwardings=yes`,
`ForwardAgent=no`, `ForwardX11=no`, `ForkAfterAuthentication=no`, `StdinNull=no`, `Tunnel=no`,
`GSSAPIDelegateCredentials=no`, `PermitLocalCommand=no`, `ControlMaster=no`, `ControlPath=none`,
`ControlPersist=no`, 15-second server-alive probes, and three missed probes. It does not pass `-D`.
The selected SSH configuration may still own identity files, agents, ProxyJump/ProxyCommand
connectivity, authentication methods, and `known_hosts` interaction.

Stderr is retained only as a bounded in-memory diagnostic tail. It is never used as a protocol,
returned as a product payload, or persisted by this subsystem. Before Hello, fixed bootstrap exit
codes classify compatibility and integrity failures; transient AskPass prompt kinds and bounded
constant matching distinguish authentication or host-verification failures from retryable network
loss without exposing the diagnostic text.

## Verified bootstrap and installation

The schema-version-2 release catalog contains an ordered Runtime and Terminal Host for both admitted
platforms. Desktop authenticates the exact canonical manifest, signer, cohort digest, sizes, and
digests before OpenSSH. Installed fast paths read no artifact bytes; an `upload` status lazily loads
and re-verifies only that selected component.

The same POSIX bootstrap detects macOS arm64 or glibc Linux x86_64, then emits exact maximum-128-byte
ASCII `OBSI/2 <platform> ssh-environment-runtime <present|upload>` followed by
`OBSI/2 <platform> terminal-host <present|upload>`. Desktop accepts only that role order and platform
agreement. After both exact artifacts exist, bootstrap emits `OBSC/2 <platform> ready`; only then may
Desktop send Hello. v1, a missing or duplicate role, reordering, mixed platform, or bytes after the
commit barrier fail closed.

Each `present` file must be a non-symlink regular file with the exact digest. Each `upload` reads the
exact signed byte count, verifies SHA-256 remotely, and publishes with a no-clobber hard link at:

```text
~/.orchbee/ssh-runtime/versions/<version>/orchbee-ssh-environment-runtime
~/.orchbee/terminal-host/versions/<version>/orchbee-terminal-host
```

Runtime and Host state live at `~/.orchbee/ssh-runtime/state` and
`~/.orchbee/terminal-host/state`. Bootstrap never invokes `curl` or `wget`, overwrites an installed
artifact, selects a remote `PATH` binary, or creates a mutable `current` link. Concurrent identical
installers converge by re-verifying the winner; any mismatch blocks the cohort before Runtime start.
Bootstrap then launches the exact Runtime with the cohort and Host metadata, waits for its private
socket, and `exec`s `attach` on the same OpenSSH stdio stream.

The Host endpoint identifier is a domain-separated digest of Runtime instance and release cohort,
not a credential or Runtime identity. Runtime connects only to its same-UID private endpoint. If the
endpoint is absent, an isolated relay worker launches the exact signed Host path with only its fixed
`serve --runtime-directory ... --endpoint-id ...` ABI; it never kills or replaces an existing Host.

The release-only Desktop package supplies the verified cohort and exact manifest-pinned trust
descriptor. Its private-key exclusion, external signing ceremony, canonical layouts, public-key
injection, rollback, and package placement are maintained in
[`ssh-runtime-release.md`](ssh-runtime-release.md). `DesktopApplication` now composes that source
with an Environment-keyed connection owner, while release-free development and ordinary packages
fail activation as `runtime-release-unavailable`.

## AskPass security boundary

For each generation, `SshAskPassBridge` creates a fresh `0700` temporary directory, a `0600` Unix
socket, a `0700` fixed launcher, and a random 256-bit capability. It sets
`SSH_ASKPASS_REQUIRE=force`; the helper sends one bounded JSON request and receives one bounded
answer over the private socket. At most one prompt is active and at most eight prompts are admitted
per generation.

Prompt text is untrusted OpenSSH input. Only an opaque handle, prompt kind, and transient prompt are
given to the injected Main interaction owner. The response is written directly to the waiting
AskPass process and then released. Password, passphrase, OTP, prompt, and answer never enter
`SshEnvironmentConnectionState`, diagnostics, telemetry, or durable storage. Cancellation retires
the generation. The delivered secure bridge is POSIX-only; a Windows ACL-backed implementation
remains required before enabling this path on Windows Desktop. Until a Main interaction handler is
composed, every prompt is cancelled without exposing it to the renderer.

## Protocol, identity, and heartbeat

Every protocol record has a 16-byte `OBSE` header, exact `0.3.0` version bytes, a kind, big-endian
payload length, and four reserved zero bytes. The complete record is capped at 1 MiB. Control
payloads are strict bounded UTF-8 JSON; channel data is binary; malformed, unknown, partial, or
oversized input fails closed.

After the fixed cohort commit, Desktop sends one Hello containing the expected Runtime instance
identity, random client nonce, frame bound, and initial credit. `connected` is impossible until
Hello-Accepted returns the exact instance, Runtime version and digest, release cohort digest,
Terminal Host version and digest, selected platform, exact ordered
`[project-root-v2, terminal-host-v1]` capabilities, and a valid Runtime epoch. Bootstrap status alone
never establishes readiness. The state observation retains only that epoch and secret-free
connection metadata.

Desktop immediately starts heartbeats and uses the Runtime-advertised 15-second interval and
three-miss limit. Runtime acknowledges the exact epoch and sequence. A mismatch is a protocol
failure; missed acknowledgements retire the transport and enter the existing bounded reconnect
policy. The Runtime also bounds incomplete records and an attachment that never completes Hello.

## Project-root inspection and directory-listing service

`DesktopApplication.inspectSshEnvironmentProjectRoot` delegates to the Environment-keyed connection
owner. It neither calls `connect` nor waits for retry: an inactive, reconnecting, stopped, or disposed
Environment returns `unavailable`. A valid call is pinned to the current Runtime epoch, allocates a
fresh channel identity, and follows `open → opened → request → result → close → closed`. Four calls
may be active per Environment; a fifth returns `busy`.

The protocol codec admits only exact inspection or directory-listing requests within the D160
bounds. A null listing path resolves the Runtime process's validated absolute `HOME`; every other
request carries one absolute POSIX path. Runtime work executes in a killable child of the immutable
Runtime binary, with the path passed only over stdin. The attachment loop polls worker output and
heartbeat independently. The worker follows symlinks, opens the canonical target as a directory,
and returns either its canonical UTF-8 path or at most 256 sorted accessible UTF-8 child-directory
names, an optional canonical parent, and truncation metadata. No file entry, file byte, shell,
expansion, retained handle, or mutation is involved.

An `AbortSignal` settles locally as `cancelled` and sends a channel cancellation. The caller deadline
settles as `timed-out` within 30 seconds even while close acknowledgement is pending. Attachment loss
settles outstanding calls as `unavailable`; the supervisor may reconnect, but it never replays those
channels. A malformed service result closes only that valid channel with `protocol-failure`, while an
epoch, unknown-channel, order, framing, or credit violation retires the attachment. Paths are never
copied into connection observations, registry records, diagnostics, telemetry, or logs.

## Terminal Host relay service

`DesktopApplication.openSshEnvironmentTerminalHost` requires one already-connected Runtime epoch and
never activates SSH. A fresh globally monotonic channel opens `terminal-host-v1`; at most one relay
may exist beside four project-root channels. Runtime delays `channel-opened` until its isolated worker
has authenticated and connected the same-UID Host endpoint or launched and connected the exact
signed Host executable.

After opening, Runtime treats every channel-data byte as opaque Terminal Host protocol. Each
direction has exactly 64 KiB credit and every chunk is nonempty and at most 64 KiB. Bytes accepted by
the worker replenish Desktop input credit; bytes consumed by the Desktop `HostConnection` replenish
Host output credit. Blocking endpoint discovery, launch, and Host I/O occur outside the attachment
loop, so heartbeat and project-root progress remain independent.

Main constructs `HostConnection` over the relay and requires its negotiated Host version to equal the
signed cohort. Busy, unavailable, and incompatible Host outcomes close only that channel. Invalid
outer framing, epoch, ordering, identity, chunk, or credit retires the attachment. Attachment loss
destroys the broker stream and makes pending Host effects outcome-unknown; nothing is replayed. A new
attachment allocates a new channel and reconciles retained terminals through the Host epoch and
inventory owned by Terminal Host.

## Remote Project admission

`DesktopApplication.admitSshEnvironmentRemoteProject` accepts one `EnvironmentId` and requested path. It
first resolves the durable registry record, then requires an already-connected observation, invokes
the exact `project-root-v2` inspection, and finally queues one Desktop Main-owned canonical Workbench
mutation. It never starts or waits for an SSH connection. Before the inspection result is queued,
cancellation or attachment loss creates no product state.

The global admission key is the returned `(EnvironmentId, canonicalPath)`. A missing key allocates a
random 128-bit Directory identity and the next Workbench Project identity; the Project name is the
canonical path's final segment. Equivalent concurrent or retried admissions return the same Project.
The committed version-2 document contains one unavailable remote Folder Directory, one inactive
Workspace that selects it, and one empty Terminal Context. The requested spelling and failed
observation never enter durable state, diagnostics, telemetry, or logs.

Once the canonical result enters the mutation queue, caller cancellation does not roll it back and a
later disconnect does not invalidate the point-in-time observation. A pre-rename write failure is a
safe negative result. A failure that may follow rename is `indeterminate`: the coordinator freezes
further mutation, and restart plus retry reconciles through the durable admission key.

Only Desktop Main may introduce remote placement. A client proposal may preserve or delete the exact
remote Directory, Workspace, and context, but cannot introduce, alter, reparent, relabel, duplicate,
or mark them available. Desktop validates every retained opaque Environment reference against the
registry. Project removal deletes product metadata only; it performs no remote delete, listing,
file, Git, shell, terminal stop, Agent, synchronization, renderer creation, or CLI effect.

## Remote terminal provision

`DesktopApplication.createSshEnvironmentRemoteTerminal` accepts one `EnvironmentId`, Project ID, and
Workspace ID. It requires a registered Environment, an already-connected observation, and a
Workspace whose primary Directory is placed on that exact Environment. It never calls `connect`,
waits for reconnect, accepts a path hint, or changes active or expanded Client View state.

Before entering the canonical mutation queue, Main prepares at most one cached Terminal runtime and
one relay per Environment. Its binding ledger and archive directory use a deterministic
Environment-isolated namespace. The remote launch is fixed to `/bin/sh`, uses only bounded terminal
defaults plus `COLORTERM`, `TERM`, and `TERM_PROGRAM`, and receives the canonical remote Directory
path as `launchCwd`; no local executable, CWD fallback, environment delta, shell integration, CLI
endpoint, profile, or secret is transferred.

An empty remote context receives exactly one Main-owned Tab, Panel, and provision Presentation. The
Workspace is the idempotency key: an exact retry reuses that Presentation and operation identity;
any other pre-existing shape fails closed. The canonical version-2 commit precedes effect
reconciliation. Target selection is derived only from Presentation containment through Terminal
Context, Workspace, primary Directory, and placement, so local placement reaches only the local Host
and Environment placement reaches only that Environment's Host.

An unavailable runtime or retryable/unknown Host outcome leaves the durable intent pending without
publishing a synthetic failure or caching a settlement. A later explicit repeat opens or reuses a
relay and reconciles the same Environment-scoped binding and Creation identity; it never falls back
locally or blindly creates a second terminal. Client proposals may apply only the exact
`provision → retain` convergence or delete the protected remote state. Disconnect and shutdown
dispose the process-local runtime and relay without deleting product state or sending terminal stop.
Close, restart, split, archive capture, and application-stop draining remain outside this slice.

## Electron remote terminal creation surface

The ordinary `workbench.terminal.create` Action, shortcut, and New Terminal controls route through
one Workbench-owned availability policy. Local placement keeps the existing local command. An
Electron renderer in an exact empty remote Workspace may request only the default top-level first
terminal; New Group with Tab, Duplicate Tab, explicit Group or insertion placement, and every
creation entry point become unavailable once the singleton exists or the capability is absent.

The renderer sends only a correlated request identity, `ProjectId`, and `WorkspaceId` over the
tenth purpose-specific `remote-terminal-creation` bootstrap port. Desktop Main derives the
Environment, canonical Directory path, and Terminal Context from the current shared document,
requires a registered and currently connected Environment, and revalidates the target around the
canonical commit. The broker may prepare or reuse that Environment's Terminal Host runtime, but it
never starts, reconnects, waits for, or prompts for SSH. The capability is not a preload global and
is not offered to Gateway, Browser, semantic CLI, Skill, or Agent callers.

The renderer performs no optimistic document or Client View mutation. A `created` or `existing`
settlement completes only after the independently ordered client-session stream publishes the exact
canonical Project, Workspace, Context, Tab, Panel, and Presentation identity. Completion never
navigates, selects, or steals focus. Only Main-confirmed abort before commit is `cancelled`; abort,
unmount, broker retirement, or control-port loss after dispatch or possible commit is a bounded
failed/outcome-unknown result. A fresh request on a replacement port reconciles the singleton rather
than creating a duplicate or falling back to the local runtime.

## Remote terminal projection and durable observation

Electron and authenticated Gateway clients project an existing remote Presentation through the same
attachment protocol as a local terminal. Main derives the target only from canonical containment and
requires that Environment's runtime to be already prepared. Projection does not activate SSH,
prepare a relay, wait for reconnect, or use local binding/archive fallback. Main rechecks the target
after open and disposes a provisional lease when the Presentation was deleted or retargeted.

Before opening the remote `HostConnection`, the Environment runtime owner loads that Environment's
`terminal-exits-v1.json` and `terminal-exit-publications-v1.json`, then installs a current-record and
Host-epoch-fenced envelope listener before runtime reconciliation. The binding ledger, pending and
rejection journals, publication ledger, retry owner, and identity association all live under the
same deterministic Environment directory. Identical raw `TerminalIdentity` values in local or other
Environment runtimes therefore remain separate.

Metadata accepts only increasing Host revisions, resolves through the Environment runtime binding,
and enters `DesktopClientSessionCoordinator`'s retained bootstrap state without changing the shared
document. A `TerminalFailed` event is recorded through that binding and published only while its
Presentation still resolves to that exact Environment. Relay loss and runtime unavailability retire
the observation owner; they never synthesize a terminal failure.

Every `TerminalExited` envelope is synchronously persisted to the Environment pending journal—or to
its durable rejection journal when pending admission is unavailable—before the Host listener
returns. Reconciliation then persists the binding outcome, reserves the Presentation outcome,
commits runtime and Environment publication evidence, commits the reconciler event, broadcasts it,
and only then compacts the pending journal. Missing association, canonical deletion, or retargeting
parks the original Environment-scoped event; it cannot publish to another target. Transient local
persistence/publication failures may retry, but recovery never connects SSH. Relay loss or Desktop
restart reloads the durable journals only when a later explicit operation prepares that exact
Environment runtime.

## Connection lifecycle and recovery

| Current phase          | Event                              | Next phase and effect                                                                |
| ---------------------- | ---------------------------------- | ------------------------------------------------------------------------------------ |
| `stopped` or `blocked` | explicit start                     | Fresh generation enters `resolving`.                                                 |
| `resolving`            | exact target resolved              | `connecting`; the supervisor verifies the release catalog, then opens one transport. |
| `connecting`           | AskPass request                    | `interaction-required`; only the matching opaque handle may resume it.               |
| `connecting`           | exact Hello accepted               | `connected`; the Runtime epoch becomes observable.                                   |
| `connecting`           | initial retryable failure          | `blocked`; unattended retry is not started before one proven connection.             |
| `connected`            | transport loss or missed heartbeat | Old generation is retired; a fresh generation enters `retry-wait`.                   |
| `retry-wait`           | bounded jitter elapsed             | The target is resolved again and a replacement child is opened.                      |
| any active phase       | stop                               | Local generation resources are retired and fenced; the remote daemon is not stopped. |
| any phase              | stale completion                   | No state change and no authority.                                                    |

Retry uses equal jitter: 500-1,000 ms first, doubles to a 30-second cap, and has one timer. A
replacement attachment to a surviving daemon observes the same epoch. If the daemon died, the
bootstrap starts the exact installed version and Hello reports a new epoch. This distinction is the
future reconciliation fence. Project-root inspection is read-only and never replayed. A lost
Terminal Host relay is also never replayed; a later explicit open negotiates a new broker stream
against retained Host state. The protocol still contains no Git, generic command, or Agent effect.

`DesktopSshEnvironmentConnectionOwner` serializes connect and disconnect per `EnvironmentId`. The
first connect copies the registry target and Runtime instance identity into one supervisor; repeated
connect reuses it. Disconnect removes that supervisor only after retirement, and application
shutdown fences new activation before stopping every remaining supervisor. Registry load, create,
and application startup never invoke connect.

## Modeling and verification

Twelve registered TLA+ models split responsibility:

- `SshEnvironmentConnection` checks generation exclusion, interaction, retry admission, blocking,
  stop fencing, and retry progress.
- `SshEnvironmentConnectionManagement` checks stale catalog fencing, append-only Add ordering,
  passive-operation non-activation, explicit connection ownership, exact prompt leases, renderer
  retirement and replacement, and the absence of Project, Directory, Workspace, and terminal
  effects.
- `SshEnvironmentRuntimeAttachment` checks trusted-catalog-before-transport ordering, present without
  artifact read, ordered dual-component upload, atomic cohort commit, exact installed digests,
  concurrent no-clobber convergence, mismatch rejection, one current attachment, full cohort
  Hello/epoch readiness, retained daemon identity, competing-attachment rejection, exact service
  authority, and no unknown-effect replay.
- `SshEnvironmentTerminalHostRelay` checks one relay, global channel identity, bounded bidirectional
  credit, isolated Host setup/I/O, channel-local failures, attachment loss, retained Host epoch,
  reconnect reconciliation, and no replay.
- `SshEnvironmentProjectRootService` checks exact capability admission, channel order and credit,
  bounded capacity, timeout, cancellation, attachment loss, no replay, stale-result fencing, and
  worker progress.
- `SshEnvironmentRemoteProjectAdmission` checks registry/connectivity/inspection/mutation order,
  cancellation boundaries, global canonical-key idempotency, commit order, and indeterminate-write
  reconciliation.
- `SshEnvironmentProjectEnrollment` checks registry-before-effect and independently durable
  Workbench ordering, pending identity, per-Environment request and prompt-lease exclusion, stale
  interaction rejection, cancellation and port retirement around the commit point, explicit
  reconnect/disconnect, automatic retry without a prompt lease, restart, initiating-view activation,
  and no terminal effect.
- `SshEnvironmentRemoteTerminalProvision` checks target authority, singleton Workspace idempotency,
  commit-before-effect, cancellation, deferred readiness, unknown-outcome reconciliation, and no
  local fallback or duplicate create.
- `SshEnvironmentRemoteTerminalProjection` checks canonical Presentation target authority, prepared
  runtime admission, local-versus-Environment isolation, start/open/delete races, stale-lease
  suppression, disconnect retirement, no SSH activation, and no local fallback.
- `SshEnvironmentRemoteTerminalOutcome` checks current-record subscription, per-Environment identity
  isolation, monotonic metadata, non-synthetic failure, synchronous exit admission or rejection,
  binding/reservation/ledger/publication/compaction order, retargeting, retry, owner restart, and
  recovery without SSH activation.
- `SshEnvironmentRemoteTerminalCreationSurface` checks renderer admission, Main-derived target
  authority, singleton convergence, cancellation boundaries, port retirement, independently ordered
  canonical publication, focus preservation, no local fallback, and no automatic SSH activation.
- `WorkbenchDocumentPlacementMigration` checks exact v1-to-v2 local placement migration, v1 source
  preservation, v2-present authority, and absence of reverse migration.

Executable evidence includes strict protocol and stream-decoder tests; canonical release-contract,
signed-manifest mutation, wrong-signer, exact-layout, symlink, and package-staging tests; shell syntax,
fixed-status decoding, ordered dual-component installed/present/upload/commit/mutation/concurrent-
install local process checks, and no-download assertions; OpenSSH security-floor arguments;
owner-only AskPass IPC; Environment owner serialization and shutdown; supervisor orchestration; all
Desktop unit tests; native Runtime unit and real-process attachment tests including canonical path,
failure behavior, actual Terminal Host launch, opaque Host Hello relay, and credit return; and
cross-compilation for both admitted remote targets. Workbench fixed-seed properties cover repeated
remote registration, while Desktop tests cover file migration, v2 authority, registry
cross-reference validation, proposal anti-forgery, concurrent admission, cancellation,
indeterminate restart/retry, isolated remote runtime reuse, commit-before-effect, pending-intent
retry, exact provision-to-retain convergence, canonical target resolution, local/Environment routing,
stale-target lease disposal, relay-close fencing, archive-fallback suppression, and Electron plus
authenticated Gateway projection composition. Environment observation tests additionally cover
listener-before-runtime initialization, Host-epoch and current-record fencing, synchronous durable
exit admission, exact publication order, retry and restart, same-identity cross-Environment
isolation, monotonic metadata, non-synthetic failure, retarget retention, and exact remote outcome
composition without local-runtime fallback. Electron creation-surface tests additionally cover the
dedicated protocol and bootstrap role, broker concurrency and retirement, stale Main authority,
connected-only admission, no automatic SSH activation, renderer request correlation and
outcome-unknown handling, canonical-publication waiting without optimistic mutation, Action
cancellation boundaries, and one availability policy across shortcuts and Tab Strip creation
controls. Enrollment-surface evidence additionally covers the eleventh bootstrap role, strict
request/progress/prompt/status/result parsing, exact interaction-lease ownership, registry and
Workbench commit boundaries, cancellation and port retirement, status projection, Connect, Retry,
Disconnect, initiating-client-only navigation, localized dialog behavior, and publication-timeout
recovery without identity replacement. The explicit signed loopback system witness exercises
host-key confirmation, encrypted-key AskPass, signed Runtime and Terminal Host installation,
canonical Project admission, no automatic terminal creation, first-terminal I/O, connection loss,
recovery, and durable placement retention against a controlled `sshd`.

Run the focused boundary from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/desktop test:release-package
pnpm --filter @orchbee/desktop test:system:ssh-project-enrollment
node scripts/tla/run-models.mjs --models desktop-ssh-environment-connection,desktop-ssh-environment-connection-management,desktop-ssh-environment-project-root-service,desktop-ssh-environment-remote-project-admission,desktop-ssh-environment-remote-project-enrollment,desktop-ssh-environment-remote-terminal-creation-surface,desktop-ssh-environment-remote-terminal-outcome,desktop-ssh-environment-remote-terminal-projection,desktop-ssh-environment-remote-terminal-provision,desktop-ssh-environment-runtime-attachment,desktop-ssh-environment-terminal-host-relay,desktop-workbench-document-placement-migration
```

## Current limits and next authority

Slices 2.3A through 2.5 authorize durable Environment identity, explicit process-local activation,
immutable Runtime attachment, one read-only Project-root observation, and Main-only remote Project
admission. Slices 3.1A and 3.1B add the signed v2 cohort, atomic immutable remote installation, exact
cohort negotiation, and one process-local Terminal Host `HostConnection` over the existing SSH
attachment. Slice 3.2A adds one durable inactive remote Workspace and empty Terminal Context through
the same atomic admission. Slice 3.2B adds an explicit Main-only first-terminal provision whose
durable intent commits before the Environment-routed Host effect and whose retry reconciles unknown
outcomes without local fallback. Runtime and Host uploads plus the explicitly provisioned terminal
are the only new durable remote mutations. Slice 3.2C adds only live interactive projection of that
existing terminal for Electron and authenticated Gateway clients. Its fresh attachment, input,
selection, search, clipboard, link, and resize behavior remains attachment-scoped; disconnect or page
release removes the projection without deleting the retained terminal or its Workbench structure.
Slice 3.2D adds exact Environment-scoped metadata and failure publication plus crash-safe exit
admission, ordered publication evidence, restart recovery, and target-aware deduplication. It does
not create, stop, restart, split, archive, or automatically reconnect a terminal. Slice 3.2E adds
only the Electron Action and product controls for the exact default first singleton in an empty
remote Workspace. Main derives and revalidates its target, requires an existing connection, and the
renderer waits for canonical publication without changing navigation or focus.

Slice 3.3A plus D157-D167 delivers the POSIX Electron enrollment and recovery surface through the
purpose-specific eleventh bootstrap role. One explicit request creates or reuses the durable
Environment identity, supplies an exact transient interaction lease, reaches SSH readiness, inspects
the remote home, lets the initiating client navigate bounded directory snapshots, independently
inspects the exact selected canonical root, commits one canonical Project and Workspace, waits for publication, and then
activates only the initiating Client View. Enrolled Projects expose process-local status plus
explicit Connect, Retry, and Disconnect controls. Restart and projection still never activate SSH,
and enrollment still creates no terminal.

There is still no creation of an additional remote terminal, remote terminal close, restart, split,
archive capture, application-stop draining, general or retained remote filesystem browser, file
listing or bytes, watch or mutation, Git operation, Gateway or CLI enrollment, Agent execution, generic remote command,
Environment rename/repoint/delete, component removal, automatic old-version cleanup, or
operating-system service/linger installation. CLI/Skill-controlled product orchestration remains a
later semantic-command and Agent slice; SSH itself never becomes that authority. The controlled
loopback witness establishes the admitted host-key, encrypted-key, upload, Project, terminal-I/O,
loss, and recovery path; heterogeneous production SSH servers, other authentication mechanisms,
network-quality coverage, and daemon survival across total logout or reboot remain future bounded
work rather than claims of this slice.
