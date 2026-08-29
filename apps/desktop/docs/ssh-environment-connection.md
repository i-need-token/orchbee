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

## Runtime topology

```text
Desktop supervisor
  ├─ owner-only AskPass IPC → transient human interaction
  └─ one OpenSSH stdin/stdout child
       └─ fixed remote bootstrap / attach
            └─ owner-only Unix socket
                 └─ per-Environment Runtime daemon and epoch
                      ├─ bounded project-root-v1 inspection adapter
                      └─ one terminal-host-v1 opaque relay worker
                           └─ same-UID private endpoint
                                └─ retained Terminal Host daemon and Host epoch
```

The Desktop connection owner admits one explicit activation per durable Environment and owns one
supervisor for that active identity. The generation owns target resolution, release-catalog
admission, AskPass, OpenSSH, installed-cohort negotiation, handshake, and heartbeat. It does not own
either daemon lifetime. The Runtime advertises exactly `project-root-v1` then `terminal-host-v1`; it
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

Every protocol record has a 16-byte `OBSE` header, exact `0.2.0` version bytes, a kind, big-endian
payload length, and four reserved zero bytes. The complete record is capped at 1 MiB. Control
payloads are strict bounded UTF-8 JSON; channel data is binary; malformed, unknown, partial, or
oversized input fails closed.

After the fixed cohort commit, Desktop sends one Hello containing the expected Runtime instance
identity, random client nonce, frame bound, and initial credit. `connected` is impossible until
Hello-Accepted returns the exact instance, Runtime version and digest, release cohort digest,
Terminal Host version and digest, selected platform, exact ordered
`[project-root-v1, terminal-host-v1]` capabilities, and a valid Runtime epoch. Bootstrap status alone
never establishes readiness. The state observation retains only that epoch and secret-free
connection metadata.

Desktop immediately starts heartbeats and uses the Runtime-advertised 15-second interval and
three-miss limit. Runtime acknowledges the exact epoch and sequence. A mismatch is a protocol
failure; missed acknowledgements retire the transport and enter the existing bounded reconnect
policy. The Runtime also bounds incomplete records and an attachment that never completes Hello.

## Project-root inspection service

`DesktopApplication.inspectSshEnvironmentProjectRoot` delegates to the Environment-keyed connection
owner. It neither calls `connect` nor waits for retry: an inactive, reconnecting, stopped, or disposed
Environment returns `unavailable`. A valid call is pinned to the current Runtime epoch, allocates a
fresh channel identity, and follows `open → opened → request → result → close → closed`. Four calls
may be active per Environment; a fifth returns `busy`.

The protocol codec admits only `{"type":"inspect-project-root","path":"/..."}` with the D59 bounds.
Runtime work executes in a killable child of the immutable Runtime binary, with the path passed only
over stdin. The attachment loop polls worker output and heartbeat independently. The worker follows
symlinks, opens the canonical target as a directory, and returns the canonical UTF-8 path or one D64
failure reason. No shell or expansion is involved.

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
the exact `project-root-v1` inspection, and finally queues one Desktop Main-owned canonical Workbench
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

Eleven registered TLA+ models split responsibility:

- `SshEnvironmentConnection` checks generation exclusion, interaction, retry admission, blocking,
  stop fencing, and retry progress.
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

Executable evidence includes strict protocol and stream-decoder tests; canonical release-contract,
signed-manifest mutation, wrong-signer, exact-layout, symlink, and package-staging tests; shell syntax,
fixed-status decoding, installed/present/upload/mutation/concurrent-install local process checks, and
no-download assertions; OpenSSH security-floor arguments; owner-only AskPass IPC; Environment owner
serialization and shutdown; supervisor orchestration; all Desktop unit tests; native Runtime unit
and real-process attachment tests including canonical path and failure behavior; and cross-compilation
for both admitted remote targets.

Run the focused boundary from the repository root:

```sh
pnpm --filter @orchbee/ssh-environment-protocol test
pnpm --filter @orchbee/ssh-environment-runtime test
pnpm --filter @orchbee/ssh-environment-runtime test:cross
pnpm --filter @orchbee/desktop typecheck
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/desktop test:release-package
node scripts/tla/run-models.mjs --models desktop-ssh-environment-connection,desktop-ssh-environment-project-root-service,desktop-ssh-environment-runtime-attachment
```

## Current limits and next authority

Slices 2.3A, 2.3B, 2.3C, and 2.4 now authorize durable Environment identity, offline release
composition, explicit process-local activation, immutable Runtime installation or attachment, and
one read-only Project-root candidate observation. The only remote mutation is the exact versioned
Runtime upload requested by the same bootstrap child. There is still no renderer API or prompt UI,
automatic activation, Project/Workspace model, remote directory listing or browser, file bytes,
watch or mutation, terminal channel, Git operation, Agent execution, generic remote command, Runtime
removal, automatic old-version cleanup, or operating-system service installation. Windows activation
remains unsupported, and daemon survival across total logout or reboot is not claimed.

A directory-listing, file, or Terminal Host service still requires an explicit later authorization,
channel contract, reconnect inventory, and no-replay semantics. CLI/Skill-controlled
product orchestration remains a later semantic-command and Agent slice; SSH itself never becomes
that authority. Real-SSH interoperability, authentication UI, and network-failure evidence also
remain future bounded work rather than claims of this slice.
