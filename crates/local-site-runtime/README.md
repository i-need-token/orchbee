# Local Site Runtime

Private native runtime facilities shared by the Client, Server, and Worker. This library
owns descriptor-relative runtime admission, lock/marker primitives, and immutable development
cohort verification. Applications own process launch, service state, and terminal authorization.

## Shared user preference storage

`UserPreferencesStore` owns the versioned, bounded device preference record described by the
[shared contract](../../schemas/workbench/README.md#shared-user-preferences). Its scope is frozen
from the exact configuration path. A filename-derived private directory separates configurations
even when their parent is shared. Reads create nothing; invalid or unreadable records are refused.
Updates acquire an OS-held writer lock, compare the observed source, apply only supplied leaves,
validate the complete candidate and publish a flushed atomic replacement. A reset claims its keys
just like an edit. Windows uses the pinned private-cache boundary; Unix uses descriptor-relative,
no-follow private files and synchronizes the containing directory after replacement. A failure after
replacement reports an unknown outcome and never authorizes automatic replay.

Reads preserve validated current values, claims and source without reinterpreting an unclaimed old default or rewriting bytes. Desktop import fields are outside the current strict record.

An absent preference record returns canonical defaults, no claimed keys and the `missing` source token. Only the preference record participates in source observation. Applications own UI drafts, polling, publication into running terminals, and process launch. The helper and native Site consume this same storage owner, not separate stores.

## Shared launch and notification policy

`default_interactive_launch` owns pure platform shell selection and native executable admission,
shared by Desktop's matched helper and Native Worker. It starts no process. The preference store
captures scrollback and the selected launch profile together; an explicit executable bypasses
default shell resolution while retaining the shared scrollback default. POSIX login is explicit;
Windows login is refused. Invalid shared storage fails the operation before Host handoff.

`UserTerminalColors` resolves terminal default colors for both native presentation and new Worker
profiles. Host follows explicit application Light/Dark; System leaves Host defaults with their
runtime owner, which builds them with `UserTerminalColors::from_rgb`. Explicit terminal
Light/Dark/Custom take precedence; Custom and `from_rgb` derive the dark or light scheme from
background luminance. The launch snapshot freezes
this result with scrollback and launch selection; the resolver performs no terminal mutation.
Retained query state and reversible Client appearance follow the [native theme boundary](../../apps/orchbee/docs/native-themes.md).

`NativeNotificationSettings` owns portable bounded delivery/sound policy. Windows configuration
decoding accepts the declared current keys; TUI observation and card scheduling consume the
portable type. Its complete private notification profile carries the current effective delivery
and global sound policy, including values governed by shared preferences.

## Windows host appearance fallback

`read_windows_apps_use_light_theme` performs a read-only current-user lookup of
`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
It returns `Some(true)` or `Some(false)` only for the documented `REG_DWORD` values `1` and `0`;
missing, unreadable, and other values return `None`. The Client invokes this once only when its
native terminal OSC `10`/`11` probe has no valid OSC `11` background by the deadline. The Worker
invokes it for a Host-default launch before any Client has reported outer-terminal colors. Each
caller owns its precedence; this runtime helper changes no Windows setting and reads no other theme
state.

## Windows private frame directories

`WindowsPrivateFrameDirectory` owns one ephemeral `graphics-<32-hex-nonce>` leaf beneath an
already admitted private runtime. `reserve` pins its ancestors without creating the leaf;
`create` exclusively creates it and writes the application's bounded owner record. Failed
creation keeps the same explicit owner until `reap` confirms absence. An existing leaf is never
adopted, repaired or deleted. Applications own producer admission, nonce generation, retention,
retry and shutdown; this facility starts no process or cleanup thread.

Only an admitted directory reads frames. The supplied path must be one exact direct child,
with a normal Windows filename; traversal, alternate streams, reserved names, reparse points,
hard links, nonregular files and foreign ACLs are refused. A read opens without write/delete
sharing, checks the exact declared length, copies at most 16 MiB and confirms EOF. The immutable
copy has no remaining file dependency. Producers must close their writer before submitting it.
Leading-dot image names such as `.frame.rgba` are ordinary leaf names; the exact owner marker
remains reserved.

The lifecycle is Reserved → Preparing → Ready → Releasing → Released. Preparation failure moves
to Releasing under the same owner. Releasing closes the root pin, checks the saved directory
identity and removes only its derived owned leaf while retaining pinned ancestors. A busy or
redirected root remains owned for retry; dropping the object is not cleanup evidence. Application
leases keep Ready alive until the last reader finishes. Windows handle traces cover bounded reads,
private admission, writer/rename exclusion, creation refusal and failed-removal retry. Same-user
malicious ACL changes, process crashes and replacement during the final delete syscall are outside
this per-user development boundary; a crash does not prove clean runtime shutdown.

## Windows literal command encoding

The pure Windows command encoder is shared by CLI Agent start and Server restart. It emits a bounded UTF-16LE Base64 script with one literal argument array. Native, batch and PowerShell script dispatch preserve argument boundaries, including empty values, quotes and percent signs. It excludes aliases/functions, rejects control characters, caps encoded text at 30,000 characters and refuses batch command lines over 8,000 characters. Applications select programs, shells, interactive flags and stricter carrier limits; this library starts no process. Helper functions and error preferences remain in a child scope when the caller keeps PowerShell open. The embedded PowerShell asset is maintained with this encoder and the CLI's real Windows argv test.

## Windows user path operands

`WindowsUserPathOperand::parse` admits bounded drive-absolute, home-relative or current-directory-relative syntax without reading process context or probing the filesystem. The opaque operand preserves its spelling; `resolve` uses only the final execution owner's supplied environment or current directory. Absolute literals need neither capability. Invalid required home values never fall through to a lower-priority home source. `normalize_windows_user_path` remains the combined entry point for callers that already own execution context. Applications own grammar completion, routing and the point at which resolution is permitted.

## Windows named runtime identity

`WindowsSiteName` validates exact case-sensitive names of 1–64 ASCII letters, digits,
dot, underscore or hyphen, excluding `.` and `..`. `default` retains the existing SID-derived
scope. Other names derive a bounded hashed scope from a domain separator, SID and exact name.
The private `session-name.json` contains a strict version-1 name record, capped at 256 bytes.
Only registration under the matching activation lock may create it; existing bytes are read
and verified, never overwritten or repaired. A partial first write remains a visible failure.
Read-only rendezvous admission requires a valid record for named scopes and verifies its
current-user scope derivation. Runtime aliases, unknown fields, duplicate fields and mismatched
names are rejected. This identity does not grant process control or authorize state deletion.

## Windows durable session catalog

Native Windows named and default Sites retain bounded private lifecycle records under
`LOCALAPPDATA/Orchbee/session-catalog/<runtime-base-digest>`. Each record binds the exact name,
current-user scope and canonical runtime path to Active, Crashed, Stopped with a nonzero stop identity,
or Deleting. Only current numeric version 1 with all fields, including the Server/Worker/Host
PID and creation-FILETIME registry, is admitted. Other versions or shapes return InvalidData;
reads never migrate records. Effect paths derive from admitted inputs, never serialized paths.

The configured runtime base must still exist; removing a stopped runtime leaf does not remove its
catalog entry or checkpoint. Reads merge runtime and durable entries without creating or migrating
files. Unsupported or malformed records with an admitted name are listed as unreadable without repair.
Mismatched identities, linked or redirected files still fail admission.

The pinned-base namespace gate covers default and named activation. A missing runtime with a
validated stopped record, free checkpoint lock and absent endpoints can be recreated at the same
route. Stopped records may use any fixed slot or exactly named owned corrupt original. Active
recovery requires all exact registered identities gone, complete registration, free guards and absent
endpoints. Recovery publishes Crashed before reconciling its marker and beginning a fresh generation.
Live/unknown identity, incomplete registration, pending deletion and orphan storage refuse recovery.
Activation atomically publishes the current Active record before marker, Server or storage effects.

Each activation carries a fresh nonzero 128-bit identity. The Server retains that exact generation before Worker effects and publishes Stopped after confirmed shutdown,
before the runtime stopped receipt and lifetime release. Failed publication never proves a clean stop.

Deletion publishes durable Deleting before the ephemeral intent or any data removal. It removes
checkpoint/runtime data and the ephemeral intent before removing the durable record last; interrupted
cleanup remains discoverable even if temporary runtime metadata disappears. Publication uses bounded
private staging, flush and atomic replacement. This is ordinary process/failure recovery, not a
power-loss guarantee. A per-record writer lock serializes direct-child registration, recovery and
clean-stop publication. `spawn_server` requires the catalog and matching activation owner;
`spawn_windows_site_child` requires the exact registered direct parent for Worker or Host. Each
child is created suspended and released only after its identity is flushed. Explicit confirmed
named-session deletion can replace an unreadable record with its deletion fence only after the
existing owner, endpoint and storage guards pass; ordinary publication cannot overwrite it.
Live Host adoption remains separate.

The [Windows crash-recovery contract](docs/windows-crash-recovery.md) owns guards, interrupted
publication, compatibility and evidence limits. Check the registered bounded model with:

```sh
node scripts/tla/run-models.mjs --models windows-site-crash-recovery
```

## Windows named session lifecycle

`WindowsNamedSiteCatalog` serializes default/named activation and named deletion with one private per-user
namespace lock in the admitted runtime base. Activation holds it from before runtime creation
through readiness. Read-only enumeration creates nothing, includes default, sorts exact names
and rejects unsafe or corrupt entries instead of hiding them. Inventory is limited to 256 sessions.

The cleanup owner checks the original deadline and its caller's interruption predicate between filesystem phases. Interruption reports whether a published intent remains; completed removals are not rolled back.

Deletion refuses default, live or uncertain processes, unsafe files and unknown directory content.
An admitted stopped receipt (or never-activated runtime), free lifetime and absent endpoints are
required before publishing a bounded, flushed sibling deletion intent. The intent fences normal
rendezvous admission and survives partial cleanup. The owner removes only known checkpoint and
runtime files, releases their exact handles before removal, retains ancestor pins and removes the
ephemeral intent before the durable record last. Retrying resumes that deletion; absent state succeeds. It never recursively deletes
arbitrary trees or stops a process. The shared namespace lock intentionally outlives individual
sessions. This covers cooperating current-user invocations and ordinary process interruption;
power-loss durability and inventory recovery after external removal of the runtime base are unclaimed.

## Local startup model

Bare Linux and macOS `orchbee` admit their inherited input/output TTY and validate geometry before
runtime creation or process launch. The default per-user scope `orchbee-native-<effective-uid>` is
under `/tmp` on Linux and `/private/tmp` on macOS; `ORCHBEE_RUNTIME_BASE` can select another
existing absolute base for isolated development. Runtime directories must be owned by the effective user with mode 0700; lock files
are regular, single-link, owned files with mode 0600. Existing invalid paths are rejected, never
repaired, followed through symlinks, unlinked, or replaced.

The managed Linux boundary admits only ext4-family and tmpfs objects. Every opened final directory,
cohort artifact, marker, and lock must have no access or default POSIX ACL: fd-based filesystem
inspection and ACL queries reject every result except ENODATA absence. The macOS boundary admits
only APFS objects and rejects every extended ACL through descriptor-anchored ACL inspection. Neither
boundary strips ACLs. Overlayfs, XFS, btrfs, DrvFS, network filesystems, and other Unix targets are
unclaimed.
First-creation concurrency assumes an umask that preserves owner bits (for example 0022 or 0077);
a user-bit-restrictive umask may expose an incomplete directory and fails closed.

The Client owns one absolute 20-second startup deadline, from TTY admission through the first
validated Host frame applied to its projection. Connect, OBLC, initial OBCS requests, readiness
polling, and first-frame waiting consume this same budget. A delayed localized progress notice
appears after 500 ms; normal fast startup is silent. This is a cooperative deadline, excluding
kernel/filesystem stalls and process suspension. Timeout closes only the Client connection.

| State / observation                                                        | Action and next state                                                                        |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Admitted compatible Client socket                                          | Reuse that exact connection; no process launch                                               |
| Definite absent/refused socket, activation lock contended                  | Wait and retry connection until the original deadline                                        |
| Activation lock acquired                                                   | Recheck socket and lifetime before any launch                                                |
| No marker, no socket residue, lifetime free                                | Client exclusively creates `activation.started`, then spawns Server once                     |
| Marker present, lifetime held                                              | Wait for compatible Server + Worker readiness                                                |
| Marker present, lifetime free; or no marker with lifetime held             | RecoveryRequired; never launch                                                               |
| Invalid socket / cohort / permissions, incompatible or ambiguous admission | Explicit failure; no fallback                                                                |
| Launch failure after marker creation                                       | Retain marker; this and later attempts require explicit recovery                             |
| Server + Worker ready                                                      | Release activation lock, read Workspace membership, attach an existing binding when selected |
| First validated frame applied before deadline                              | Enter TUI with the same inherited TTY and prepared Session                                   |
| Client exit / deadline                                                     | Release Client resources; retain Server, Worker, Host, and terminal                          |
| Worker / Host loss                                                         | Unavailable or unknown outcome; never respawn, adopt, or replay                              |

`activation.started` is an immutable, empty, regular, single-link, effective-user-owned 0600
file. The Client is its sole writer, using descriptor-relative O_EXCL, CLOEXEC, and NOFOLLOW.
It is a launch-attempt commit, not a PID or success report. The activation guard spans that commit,
spawn, and OBLC readiness. The Server alone holds the lifetime lock, binds the Worker listener,
starts exactly one cohort Worker, and publishes the Client listener only after Worker registration.
The Client listener is bound under a private pending name, chmodded, then published with an atomic
no-replace rename; clients never observe its pre-permission state. Worker spawning follows final
Worker socket permissions. The Worker waits for its newly spawned Host socket to reach exact 0600
before the sole handshake attempt; a pre-existing endpoint still fails immediately.
The Server starts a new session with `setsid`; Client exit does not signal or kill its children.
Child owners reap exits without killing retained services; no reaper grants restart authority.

The Worker registers without starting a Host. Its authorization kernel admits the first Launch
and prepares launch facts before spawning one exact cohort Host in a private `host` directory.
Invalid authorization produces the existing typed result without a Host. The admitted dispatch
and kernel survive Host startup; the Host handoff revalidates authorization. The absolute Host
endpoint address is checked against the platform socket-address ABI before the `host` directory
or the Host process exists; an address that cannot fit settles the same pre-handoff failure
without either side effect. Preparation or startup failure settles a pre-handoff failure when
deliverable and retires the Worker. Any existing Host endpoint is an explicit recovery failure.
No Host reconnect/adoption is allowed.

## Development cohort contract

`cohort.json` is bounded to 16 KiB, strict JSON with `schemaVersion: 1` and exactly five
`artifacts` fields, in this canonical order: `orchbee`, `orchbee-server`,
`orchbee-worker`, `orchbee-terminal-host`, `agent-observation-adapters`. The first four values are
the lowercase 64-character SHA-256 of each complete executable. The fifth declares the lowercase
SHA-256 payload digest of the observation-adapters capsule staged beside the executables — the same
value its `capsule.json` carries as `payloadSha256`. A manifest carrying any other `schemaVersion`,
is incompatible; a `schemaVersion: 1` manifest missing a required member, carrying
duplicated or unknown members, or declaring a malformed digest is an invalid manifest. Both fail
closed through the existing typed rejection. The directory is effective-user-owned 0700. Files are
regular, single-link, owned, and not writable by group/other; executables have an owner execute bit.
Linux requires no POSIX ACL on the directory or opened files; macOS additionally requires APFS and
no extended ACL on each opened object. Manifest and executables are opened relative to the verified
directory with NOFOLLOW and CLOEXEC. Each managed Rust role (Client, Server, Worker) validates all
four executables, including on warm startup.
Host consumes the verified sibling executable; it does not implement the manifest reader.

The root development and test profiles compile the existing `sha2` dependency at optimization
level 3 so large debug executables can be hashed efficiently. Application code retains its normal
debug profile. Cohort validation still consumes the caller's original deadline.

The `agent-observation-adapters` sibling directory is a lazily admitted sidecar. `NativeCohort`
never opens, traverses, or validates the payload during startup; it exposes only the fixed sidecar
directory and the manifest-declared digest through `observation_adapters_sidecar()`. The first
integration consumer validates that directory privately and answers a typed unavailable or invalid
result, so an absent or damaged sidecar never blocks Site startup or Agent reads.

The exact cohort identity is lowercase SHA-256 of UTF-8 `orchbee-native-cohort-v1\n` followed,
in the above fixed order, by each executable name, one NUL byte, its hexadecimal hash, and LF —
the four executables only; the adapters declaration is deliberately excluded so a payload bump does
not churn admission identity.
JSON key order is immaterial. Sibling paths come only from these fixed names. Missing, changed,
mixed, malformed, or unsafe artifacts fail admission before activation. Explicit endpoint test
modes retain their existing version cohort and do not acquire managed startup authority.

The staging script builds and copies all four executables into a new private directory, copies the
canonical `apps/desktop/resources/agent-observation-adapters` capsule verbatim to the sidecar
directory, verifies its strict `capsule.json` members (`contractVersion` 3, a declared
`payloadSha256`, the required payload entries, and a recomputed canonical digest equal to the
declared one), then writes the manifest last. A capsule that fails that verification stops staging
rather than emitting a bad package. This is unsigned, per-user development staging: release signing,
installers,
same-user malicious mutation, and verification-to-exec races are excluded. The manifest establishes
a coherent exact artifact set, not publisher authenticity.

## Correctness case

The LS claims below describe the Linux and macOS development startup boundary. Managed Windows cold recovery
has the separate [generation and process registration contract](docs/windows-crash-recovery.md).

| Claim                                                                 | Owner / refinement                                                  | Evidence                                                            |
| --------------------------------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------------------- |
| LS01 at most one launch attempt per runtime lifetime                  | Exclusive activation marker; Client under activation guard          | LocalSiteStartup model; contention and failed-marker process traces |
| LS02 no implicit recovery or Client-owned termination                 | Persistent marker, Server lifetime guard, dropped Client connection | Model losses/timeouts; disconnect/reopen real process trace         |
| LS03 no Host before authorized Launch; one retained dispatch          | Worker authorization kernel and lazy startup adapter                | Model authorization; Worker invalid/valid Launch process tests      |
| LS04 coherent roles and private rendezvous before effects             | NativeCohort and ModeCheckedLocalSiteRendezvous                     | Manifest corruption, mode, symlink, and existing lock tests         |
| LS05 startup readiness includes a valid first frame within one budget | Client deadline, TerminalSession projection, same TTY handoff       | Admission/submission deadline tests and bare TTY process tests      |

`formal/tla/LocalSiteStartup.tla` bounds two Clients, one Server launch attempt, and one Host
attempt. It abstracts filesystem checks as admitted facts, abstracts deadline observation as an expiry event, and
checks safety without scheduler fairness. Per-Client timeout fairness supports bounded-attempt
termination only, not service availability. Real kernel locks, peers, hashing, PTYs, and
cancellation are exercised by Linux and macOS tests. Remote, Workbench membership, daemon restart,
automatic cleanup/recovery, persistence across Server loss, and other Unix targets are
outside this contract.

## POSIX local Site rendezvous foundation

The crate-private `rendezvous` module exposes mode-checked Unix rendezvous primitives to the
Client, Server, and Worker. Installation composition
must supply a stable `SiteScope`; cohort version and digest do not belong in that scope, so an
update keeps one Site identity while release, local, and checkout-scoped development installations
can remain distinct. `SiteScope` accepts 1 through 63 ASCII bytes in the canonical form
`[a-z][a-z0-9]*(?:-[a-z0-9]+)*` and therefore cannot add a path separator, dot component, or
platform-dependent encoding.

For one trusted, existing, exactly normalized absolute per-user runtime base, the exact derived
paths are:

| Role                | Stable path                                                           |
| ------------------- | --------------------------------------------------------------------- |
| Scope directory     | `<runtime-base>/<site-scope>`                                         |
| Activation lock     | `<runtime-base>/<site-scope>/activation.lock`                         |
| Site lifetime lock  | `<runtime-base>/<site-scope>/site-lifetime.lock`                      |
| Client socket       | `<runtime-base>/<site-scope>/client.sock`                             |
| Client pending name | `<runtime-base>/<site-scope>/client-pending.sock`                     |
| Worker socket       | `<runtime-base>/<site-scope>/worker.sock`                             |
| Host scope          | `<runtime-base>/<site-scope>/host`                                    |
| Host endpoint       | `<runtime-base>/<site-scope>/host/00000000000000000000000000000001.s` |

The pending name is the transient pre-publish bind the Server renames to `client.sock`; the Host
endpoint is the absolute address the Worker dials while the Host binds it relatively inside the
`host` scope. The pending leaf, `host` scope, and endpoint filename are shared constants, and the
derived paths are pure joins that answer even when the directories cannot be inspected — so every
derived socket address can be budget-checked against the platform `sockaddr_un` capacity before any
directory, marker, or process side effect.

`ModeCheckedLocalSiteRendezvous` retains its mode-checked name for other Unix targets. Preparation
creates only the scope directory when absent. Preparation and validation open `/` as the trusted
root, then open every runtime-base component and the controlled scope descriptor-relatively with
`O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`. They neither canonicalize nor ask the operating system to
normalize the supplied base. Apart from `/` itself, an accepted base has exactly one leading slash,
one or more nonempty normal components separated by single slashes, no trailing slash, no `.` or
`..` component, and no NUL byte. A malformed base is rejected before filesystem access beyond the
trusted root; a symbolic-link or non-directory component is rejected at that component. The
retained scope must be a directory owned by the effective UID with exact mode `0700`. A new scope
is inspected without following its leaf, checked for directory type and effective-UID ownership,
changed descriptor-relatively to `0700` before a permissions-dependent open, and then admitted only
when the opened descriptor has the same device and inode. This reaches exact `0700` even when the
process umask removes every requested permission, without changing the process umask. An existing
wrong mode is rejected without repair.

Linux validates final objects against the admitted local-filesystem and POSIX ACL contract. macOS
validates APFS and the absence of extended ACLs on its runtime base, scope, lock files, marker,
cohort directory, and opened cohort artifacts.

Each lock operation opens or creates only its fixed leaf through the retained directory descriptor
with `O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`. The leaf must be a regular file owned by the effective
UID with exact mode `0600` and exactly one hard link. An existing valid stale file is reused without
truncation or replacement. `try_acquire_activation_lock` and `try_acquire_site_lifetime_lock` each perform one
nonblocking exclusive kernel lock attempt and return either an owned guard or `Contended`; they do
not wait, retry, or infer process health. File creation and advisory-lock acquisition are separate:
a caller returning `Contended` may have created and normalized a previously missing durable lock
inode before another descriptor won the lock. A contended attempt never truncates, replaces,
unlinks, or changes the ownership of an existing inode, and never repairs a pre-existing invalid
entry. Dropping a guard releases only its advisory lock. The runtime directory, durable lock files,
socket paths, and any entries at those paths are never removed by this module.

`observe_*` entry probes expose the same finite names to read-only diagnostics: a
descriptor-relative `fstatat` with `AT_SYMLINK_NOFOLLOW` answers each entry's raw kind,
ownership, mode, size, and link count — the caller owns every verdict. `observe_host_endpoint`
traverses into `host/` only after the parent itself validates as an owned directory and refuses
rather than follows anything else. `observe_site_lifetime_lock` never creates the leaf: only an
entry already stat-checked as an owned regular `0600` single-link file is opened, probed with one
`try_lock`, and released on the same descriptor — `Held` is a lock fact at that instant, never a
process claim. No observation connects, creates, writes, unlinks, repairs, or infers process
health.

The primitive remains mode-checked on other Unix systems. Linux adds the restricted-filesystem
and ACL-absence boundary above before directory, lock, marker, or cohort admission; socket metadata
and same-user peer proof are enforced by its managed consumers. Client, Server, and Worker now
share this crate, and Server owns the lifetime guard. The primitive does not itself select
application startup policy, retry, remove stale endpoints, or grant recovery authority.

### Modeling and evidence

| Field        | Rendezvous primitive contract                                                                                                                                                                                                                                                                                                                         |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Provide shared path, permission, lock and attempt-marker admission; applications own launch, readiness and recovery policy.                                                                                                                                                                                                                           |
| Inputs       | A trusted existing exactly normalized absolute base, canonical scope, effective UID, and potentially invalid directory, lock, marker or socket entries.                                                                                                                                                                                               |
| Data         | Stable paths, retained scope descriptor, owned advisory-lock guards, and an immutable empty attempt marker. No PID or process-health inference.                                                                                                                                                                                                       |
| Outputs      | Validated rendezvous or typed failure; Acquired/Contended lock outcome; marker and socket existence with metadata validation. Path getters perform no I/O.                                                                                                                                                                                            |
| Invariants   | Descriptor-relative no-follow traversal; exact final type, owner, mode and regular-file link count; new directory identity matches its inspected inode. Linux and macOS final objects pass their respective restricted-filesystem and ACL-absence checks. Existing invalid entries are never repaired or removed. Guard drop only releases its lock.  |
| Decisions    | Normalized path and scope syntax, create-versus-existing metadata, lock kind, one nonblocking lock outcome, and exclusive marker creation determine results.                                                                                                                                                                                          |
| Model        | Atomic POSIX primitives without application retry or process observation. LocalSiteStartup composes these facts with Client/Server/Worker state; Client implementation tests exercise the connect-first behavior.                                                                                                                                     |
| Transitions  | Missing scope/lock becomes a validated durable inode. Lock creation and lock acquisition may have different winners. Guard acquisition/drop changes only lock ownership. An absent marker can be committed once and is never cleared.                                                                                                                 |
| Dependencies | Safe nix/rustix filesystem wrappers and File::try_lock; Linux fd-based filesystem/POSIX ACL queries and macOS APFS/extended ACL inspection. No child-process or service dependency in rendezvous.                                                                                                                                                     |
| Ownership    | This crate owns derivation and admission. Client chooses the development base/scope and owns the activation guard and marker commit; Server owns the lifetime guard.                                                                                                                                                                                  |
| Verification | Scope/path partitions, symlink/hardlink/FIFO rejection, wrong owner/mode, restrictive-umask initialization, concurrent owner-preserving creation, stale inode preservation, contention/reacquisition, marker exclusivity and socket impostors. Linux ACL tests retain exact 0700/0600 modes while adding named access ACLs or directory default ACLs. |
| Limits       | One effective user; wrong-owner tests inject the expected UID. Kernel metadata and advisory locks are trusted. Concurrent creation requires owner-preserving umask. Durable means no unlink/truncate, not fsync. Cooperative deadlines exclude kernel stalls and process suspension. The model does not prove filesystem or kernel semantics.         |

## Windows refinement

`RetainedChild::terminate_before` explicitly terminates only the exact held process and confirms
its exit before a cooperative deadline. It does not terminate descendants. Managed startup uses
it only before the Worker can create a Host; later cleanup belongs to the Worker shutdown protocol.
Dropping `RetainedChild` continues to release observation ownership without terminating anything.

`WindowsSessionStore` retains bounded checkpoint bytes under
`<LocalAppData>/Orchbee/native-sessions/<canonical-runtime-endpoint-digest>`. Its caller supplies
the LocalAppData directory and admitted Site; the Server owns checkpoint validation and recovery.
Different runtime bases and named scopes have separate stores. Private ancestor handles, protected
new-directory ACLs, no-reparse admission and single-link file checks apply. A private `state.lock`
is held exclusively for the store lifetime; a competing owner receives `WouldBlock`.

The store retains three versioned, integrity-checked slots, ordered by their private generation.
The Server supplies whole-document validation; the newest valid payload wins. Invalid copies remain
intact and produce an explicit recovered/empty startup notice. Unframed files and unsupported
payloads use this same recovery policy. Missing data and invalid data remain distinct;
filesystem/private-path admission errors still refuse startup. Current durable and checkpoint
formats each use version 1 and reject preceding numeric versions without migration. See the
[Windows checkpoint storage contract](docs/windows-checkpoint-storage.md).

Publication writes and flushes one private `checkpoint.pending`, preserves any displaced invalid
original under a fresh owned raw filename, then atomically replaces a slot in the same directory
with `MoveFileEx(REPLACE_EXISTING)`. Pending bytes never supply recovery authority; failure keeps
prior valid slots and reports any unconfirmed staging cleanup. Unchanged payloads do not consume
history. Raw originals are user recovery data retained until explicit Site deletion or authorized
repair. Other users are excluded by the private ACL; same-user writers respect the lock. The
adapter does not claim directory-entry write-through; the acknowledged pending-file flush and
atomic replacement do not prove power-loss or hardware durability.

The store does not interpret JSON, select a default data directory, relaunch a process, replay an
operation or grant Site restart authority. The Server owns startup/stop use of this primitive;
native process-crash/restart and machine-restart witnesses remain separate evidence obligations.

The native Windows adapter retains activation/lifetime-lock ownership; the explicit
stop record below refines the original immutable attempt marker. It admits an absolute local path only after
rejecting reparse points and retaining directory handles without delete sharing.
The runtime parent and leaf must be owned by the token user with a DACL granting
access only to that user, SYSTEM, or Administrators. Newly created leaves receive
a protected inheritable DACL; existing unsafe entries are rejected, never repaired.
Named-pipe addresses are derived from the canonical runtime path; the default scope is keyed by the token SID.
Host identities use the same path digest so different Sites cannot collide. Connection
admission still proves the peer's token user. Pipe existence is advisory and never
proves readiness or process health. Windows executables use exact `.exe` siblings
and the existing cohort manifest digest rules. Child processes use detached,
native creation flags with handle inheritance disabled and remain owned by the Site after Client detach.

The safe `windows-permissions` dependency owns descriptor/ACL queries that `std`
cannot express. `winapi-util` supplies handle-based hard-link counts; `winsafe` supplies
non-inheriting process creation and exact wait/exit observation. Its WinAPI FFI and legacy bitflags dependency remain outside the
authored unsafe-code prohibition; no async runtime or shell-based security probe
is introduced. Native tests cover ACL rejection, hard-link and junction refusal, marker
exclusivity, lock contention, distinct endpoints, and cohort tampering. This is a
self-reviewed refinement, not an independent review: the challenge cases include
existing broad ACLs, path replacement, duplicate starts, and pipe impersonation.

Windows read-only diagnostics reuse admission without creating a scope or file.
The marker must be regular, single-link, privately owned and contain a valid activation record. The
activation lock is inspected without locking. The lifetime observation opens only
an existing admitted file, tries one nonblocking exclusive lock, and immediately
releases it: absent/free/held/unsafe are observations, not service health. A probe
can briefly contend with startup; failure never authorizes repair or deletion.
Invalid parents prevent descendant inspection. Tests cover zero creation,
free/held transitions, malformed markers and hard-link rejection.

### Windows explicit-stop activation record

The Windows stop refinement replaces the immutable-attempt-only interpretation
with a bounded record in the same admitted `activation.started` file. Empty bytes
mean Active; the exact ASCII prefix `orchbee-stopped-v1:` followed by a nonzero
32-character lowercase hexadecimal stop request ID means Stopped. Absence remains
distinct. Every other size, prefix or encoding is invalid and grants no restart.
Linux retains its immutable empty-marker contract.

The managed Server retains an existing Active file handle without delete sharing.
Only its lifecycle owner may complete that record, after exact terminal containment,
Host exit and Worker exit have been confirmed. Completion seeks, writes the bounded
record, fixes its length and flushes data. Partial I/O is not success. The retained
owner can retry only the same attempted ID; a different ID is refused. An exact
already-completed ID is idempotent. Filesystem stalls are outside cooperative time
bounds; crash-torn or malformed records fail closed. No pathname is deleted or
replaced and no receipt proves that an unrelated process exited.

Under the existing activation/lifetime guards, startup may consume only a valid
Stopped record by truncating that same admitted file to Active before the single
Server spawn. Failure after this boundary still requires recovery. A subsequent
start consumes the previous stop receipt, so its absence never disproves an older
stop. Older cohorts reject Stopped bytes and must be upgraded before restart.

The decision partitions are absent/Active/Stopped/invalid, same/different stop ID,
first completion/retry, and start before/after completion. Runtime tests establish
strict parsing, zero versus malformed data, completion identity and restart cycles;
managed lifecycle and process witnesses must establish the exit precondition before
the public stop command can claim completion. This record primitive itself does not
stop processes, acquire lifecycle authority, or perform automatic recovery.

## Windows directory primitives

The shared pure `windows_reported_directory` decoder preserves valid terminal-reported drive/UNC observations; callers separately enforce narrower launch path bounds. `windows_powershell_initialization.ps1` synchronizes PowerShell process cwd after the original prompt, preserving status and leaving non-filesystem providers unchanged. Both ordinary Worker shells and encoded resume shells use this single hook and child-local UTF-8 native-output decoding and outgoing pipeline encoding.

`windows_interactive_powershell_initialization` concatenates that common asset with
`windows_powershell_reader.ps1`, which wraps the existing interactive reader with semantic
read-phase markers. The Worker prepares this exact startup for admitted plain PowerShell argv;
the Host build reads the same two assets, in the same order, for injection and exact recognition.
Encoded launch/resume helpers use only the common initialization and install no reader hook.
Neither asset writes a profile or changes an external PowerShell process. Actual 5.1/7 pipeline,
prompt and PTY startup witnesses cover the managed path; arbitrary startup scripts remain outside it.

Reader admission requires standard PSReadLine module/export identity and a recognized definition
fingerprint; custom and unsupported definitions stay untouched and unknown. Repeated/skipped reader
installation cannot abort the enclosing initializer. The modern adapter preserves the preceding
command's status before marker output, and passes it to native ReadLine with the original module
context; the legacy reader retains its original two-argument behavior.

Retained-shell admission recognizes only the exact current shared PowerShell startup.
Historical directory-only/UTF-8 argv fingerprints, arbitrary suffixes and extra argv are rejected.
Initialization and encoded helpers consume only the owned assets.

## Windows user settings

The same reader owns `WindowsUpdateChannel` (`stable`/`preview`, default stable) and default/file provenance. An invalid file value defaults that leaf and enters the warning record; the documented environment boundary remains fatal. Native `channel show` consumes the effective preference offline; it does not infer installed release identity or run an updater.

The shared bounded settings reader and pure Windows user-path adapter, which normalizes resolved relative and tilde operands lexically while keeping absolute literals verbatim, serve CLI configuration, common Client activation and direct managed Server startup. They own fatal file/path/environment admission, field-local fallback to existing safe defaults, resolved restore policy and a deterministic warning record; applications own wire projection, activation commit and private child arguments. The load result keeps effective settings and diagnostics together. Diagnostics are deduplicated before display truncation, sorted by field and reason, capped at 128 entries with an exact omitted count, and never retain source values or commands. See [configuration and precedence](../../apps/orchbee/docs/windows-cli-configuration.md). The Windows-only pinned `toml` dependency parses at this file boundary. The reader has no Workbench dependency.

The pure `windows_user_settings_path` resolver exposes the same configuration-file location for companion assets without opening the file or applying settings. It preserves explicit `ORCHBEE_CONFIG_FILE` precedence over `%APPDATA%/Orchbee/config.toml`; absent roots remain absent. The Server uses its parent for detection-rule inputs.

## Windows private update cache

`WindowsPrivateCache` owns pinned private ancestors, no-follow regular single-link files and a
cross-process writer lock for Server-owned detection cache updates. Applications retain logical
filenames, content validation, versions and policy. Missing descendants are created only below an
admitted private parent; unsafe existing data is refused without repair. Bounded reads preserve
absence. Replacement flushes a complete staging file, then atomically replaces the admitted leaf.
The reserved `.orchbee-update.lock` is a persistent cache coordination object;
`.orchbee-update.pending` is the one owned staging leaf, removed after settlement or recovered under
the next exclusive lock. Cleanup failures are returned, never converted into successful publication.
The Server updater is the real consumer; this adapter alone does not implement remote updates.

`read_existing` pins and validates an existing private directory tree and performs a bounded leaf read without creating directories, lock files or staging files. It serves startup/reload status inspection; missing cache paths remain absent.

Windows settings also admit `[update] manifest_check` (default true) and retain default/file provenance plus explicit-location status. Invalid supported leaves default independently; invalid key actions remain explicit disabled overrides, while valid chords from the same action survive. `read_windows_user_settings_at` reads a complete candidate at a frozen path, reapplies the strict resume environment override, preserves missing-default/missing-explicit behavior and creates no files. Applications retain atomic settings/diagnostics publication, the authoritative Server startup baseline and automatic scheduling policy.

## Windows keybinding configuration repair

The Windows settings owner exposes bounded offline keybinding reset with semantic preservation, exclusive-file admission, a flushed private backup and explicit recovery results. The application owns CLI receipts and localization; [the configuration contract](../../apps/orchbee/docs/windows-cli-configuration.md#reset-custom-keybindings) owns policy, failure behavior and backup lifecycle. The library does not reload or launch a process.

The same keys owner encodes and validates canonical `[keys]` profiles for live Client publication, capped at 64 KiB and 144 expanded triggers per action. User files keep the 16-alternative bound; Server and Client own publication and installation.

The Windows plugin catalog boundary supplies bounded manifest parsing, normalized identifiers, platform inheritance, source availability and atomic private user registration for native plugin management. Reads do not create storage; unavailable sources retain explicitly marked inspection metadata. Configuration/state paths remain stable after unlink. The shared Windows command owner provides bounded output capture and descendant settlement for Worker commands and offline provisioning; applications own execution policy. Managed provisioning owns serialized source download/build/commit/cleanup through an application-supplied executor, with one strict version 1 catalog including required provenance/recovery fields; see the [native plugin contract](../../apps/orchbee/docs/windows-plugins.md).

## Windows immutable release storage

`WindowsReleaseStore` acquires the existing private cache writer lock and pins installation ancestors. A stage borrows that lock, creates only validated portable paths and exclusive private files, and publishes by a no-replace directory rename to a lowercase SHA-256 identity. The application owns manifest validation, downloads, cohort admission, smoke checks, user PATH activation and retained versions. Explicit cleanup removes only its unpublished stage; unconfirmed child settlement uses `retain_for_recovery` and reports the retained path. `open_windows_release_file` rejects unsafe ancestors, reparse points, foreign ACLs and hard links. Deterministic Windows tests exercise writer exclusion, immutable publication, rejected aliases and staging cleanup.

`set_windows_update_channel` is the shared explicit settings writer: it validates the entire current file, preserves TOML syntax with the already pinned toml_edit parser, serializes cooperating writers, rechecks for edits, and flushes an atomic private replacement. It may create the selected absent file. It never downloads or rolls back a saved preference after a caller's update failure.

On Windows, `NativeCohort::artifact_sha256` retains each executable digest from successful admission. Installation provenance compares its declared digests against these observations, so a cohort change between reads cannot be presented as the previously admitted release.

## Windows sidebar presentation settings

The shared settings owner admits separate `ui.sidebar.agents` and `ui.sidebar.spaces`
layouts. Token styling/raw-value first-match rules have one common admission owner;
family-specific wrappers reject unsupported tokens and non-text rules. Each closed
canonical profile caps at 32 KiB independently. Workspace layouts use rows/row_gap;
Agent layouts retain provider overrides. Startup, CLI checks and Site reload all
consume this owner. See the [user guide](../../apps/orchbee/docs/windows-cli-configuration.md#windows-workspace-sidebar-layouts).

## Windows handoff pipe identity

An explicit Worker handoff derives a separate same-user named pipe from the already admitted
runtime hash and its nonzero 128-bit ticket. `handoff_endpoint_path` never creates a filesystem
leaf or grants authority to an arbitrary path. Applications own ticket validation, target cohort
admission, the standby lease and process/Server ownership; ordinary client/worker/Host endpoints
keep their existing identity. Native pipe tests verify separate scope/ticket addresses and bounded
names even for long Unicode runtime input. [The Worker contract](../../apps/orchbee-worker/docs/windows-server-handoff.md)
owns the connection transition.
