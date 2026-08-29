# SSH Environment Connection

## Scope and delivered boundary

Desktop Main owns the process-local Slice 2.2 connection foundation for one explicitly requested
SSH Environment: target resolution, one supervised OpenSSH child, transient AskPass, signed Runtime
artifact verification, a no-download remote bootstrap, exact Hello and heartbeat, generation-fenced
retry, and attachment retirement. The native `apps/ssh-environment-runtime` daemon and
`@orchbee/ssh-environment-protocol` wire contract are concrete consumers of this boundary.

Slice 2.3A separately composes the durable append-only Environment registry into
`DesktopApplication`. Slice 2.3B separately delivers an offline, release-only two-platform Runtime
bundle and Desktop trust composition. Slice 2.3C now composes an explicit process-local connection
owner keyed by durable Environment identity and adds same-child installed-artifact negotiation. There
is now one Slice 2.4 process-local API for inspecting an explicit remote Project-root candidate over
the already-connected epoch. There is no renderer or product UI, Runtime network feed, automatic SSH
connection, Project or Workspace creation, directory browser, file access, terminal, Git, or Agent.
Tests use local temporary directories, ephemeral signing keys, and controlled processes; no real SSH
server is contacted.

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

## Offline Runtime release composition

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
                      └─ future explicitly admitted service adapters
```

The Desktop connection owner admits one explicit activation per durable Environment and owns one
supervisor for that active identity. The generation owns target resolution, release-catalog
admission, AskPass, OpenSSH, installed-artifact negotiation, handshake, and heartbeat. It does not
own the daemon lifetime. The Runtime advertises exactly `project-root-v1`; every other remote service
edge remains intentionally absent.

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

`SshRuntimeReleaseManifestSchema` requires the canonical two-platform matrix, one bounded semantic
version, exact artifact sizes and SHA-256 digests, and one canonical base64url Ed25519 signature.
Desktop loads the app-asar-bound trust descriptor and Resources-bound manifest, verifies the exact
manifest and signature, and checks both artifact file sizes before OpenSSH starts. It caches that
catalog result. Artifact contents are not read on the installed fast path; a requested upload lazily
reads and re-verifies the selected artifact bytes before writing them to SSH.

The same fixed POSIX bootstrap child detects macOS arm64 or glibc Linux x86_64 and checks the exact
version path. An exact non-symlink regular file emits `present`; absence emits `upload`; a mismatch,
symlink, non-regular file, unsupported platform, or missing hash facility blocks. The status decoder
accepts only the four fixed maximum-128-byte ASCII lines from D50, and those lines select flow rather
than establish trust. On `upload`, the bootstrap reads exactly the catalog byte count from SSH stdin,
verifies SHA-256 again on the remote machine, and installs the executable at:

```text
~/.orchbee/ssh-runtime/versions/<version>/orchbee-ssh-environment-runtime
```

It uses a private state directory at `~/.orchbee/ssh-runtime/state`, never invokes `curl` or `wget`,
and publishes a verified incoming file with a no-clobber hard link. Concurrent identical installers
converge by re-verifying the winner; a different winner blocks without overwrite. It starts `serve`
with inherited streams detached, waits for its private socket, and `exec`s `attach`. On `present`,
Hello immediately follows the status; on `upload`, bytes after the exact artifact become Hello. One
OpenSSH child therefore owns platform detection, optional installation, and active protocol.

The release-only Desktop package path now supplies the verified two-platform bundle and exact
manifest-pinned trust descriptor. Its private-key exclusion, external signing ceremony, canonical
layouts, public-key injection, rollback, and package placement are maintained in
[`ssh-runtime-release.md`](ssh-runtime-release.md). `DesktopApplication` now composes that source
with an Environment-keyed connection owner, while release-free development and ordinary packages
fail activation as `runtime-release-unavailable`. The bootstrap passes the verified artifact SHA-256
into `serve`; the daemon retains it for its epoch and returns it in Hello, avoiding a circular
self-digest build requirement.

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

Every protocol record has a 16-byte `OBSE` header, exact `0.1.0` version bytes, a kind, big-endian
payload length, and four reserved zero bytes. The complete record is capped at 1 MiB. Control
payloads are strict bounded UTF-8 JSON; channel data is binary; malformed, unknown, partial, or
oversized input fails closed.

After the fixed status, Desktop streams artifact bytes only for `upload`, then sends one Hello
containing the expected Runtime instance identity, random client nonce, frame bound, and initial
credit. `connected` is impossible until Hello-Accepted returns the exact instance, signed artifact
version and digest, status-selected platform, exact singleton `project-root-v1` capability, and a
valid daemon epoch. A status token alone never establishes readiness. The state observation retains
only that epoch and secret-free connection metadata.

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
future reconciliation fence. Project-root inspection is read-only and never replayed; the protocol
still contains no terminal, Git, command, or Agent effect.

`DesktopSshEnvironmentConnectionOwner` serializes connect and disconnect per `EnvironmentId`. The
first connect copies the registry target and Runtime instance identity into one supervisor; repeated
connect reuses it. Disconnect removes that supervisor only after retirement, and application
shutdown fences new activation before stopping every remaining supervisor. Registry load, create,
and application startup never invoke connect.

## Modeling and verification

Three registered TLA+ models split responsibility:

- `SshEnvironmentConnection` checks generation exclusion, interaction, retry admission, blocking,
  stop fencing, and retry progress.
- `SshEnvironmentRuntimeAttachment` checks trusted-catalog-before-transport ordering, present without
  artifact read, upload-only-on-request, exact installed digest, concurrent no-clobber convergence,
  mismatch rejection, one current attachment, Hello/epoch readiness, retained daemon identity,
  competing-attachment rejection, exact service authority, and no unknown-effect replay.
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
