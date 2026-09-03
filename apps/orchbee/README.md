# Orchbee Native Entry

Experimental, unshipped Rust process entry, application-private local Client transport and
rendezvous primitive, checked Client-side local Site activation refinement, and staged uninvoked
TUI shell foundation for the accepted native-command and local-terminal milestone. The emitted
executable is named `orchbee`. It participates in the root Cargo verification gate but is not
consumed by Desktop, packaging, an installer, or a package manager.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Command surface

The exact development surface is:

```text
orchbee
orchbee --help
orchbee --version
```

Bare invocation selects the future TUI route and reports a localized development-unavailable
result. It does not inspect terminal devices, enter terminal mode, register signals, or render the
staged shell. `--help` and `--version` retain their exact output and likewise complete without TUI
or backend initialization. Every other, additional, or non-Unicode argument is a usage failure.
There are no short flags, aliases, abbreviations, business commands, target discovery,
current-directory inference, or compatibility behavior.

Help writes only stdout. The unavailable and usage results write only stderr. Version writes one
locale-neutral `orchbee <version>` line to stdout. Development exit statuses remain `0` for help
and version, `1` for the unavailable bare TUI route, and `2` for usage failure. They are not the
future stable public exit table.

## Private local Client transport

On Unix, the application-private `LocalClient` accepts an endpoint and executable cohort only from
an internal caller. It delegates admission to `orchbee-local-client-protocol`, which validates the
cohort before connecting, proves the Server has the same effective operating-system user, performs
the exact `OBLC` handshake with two-second read/write timeouts, and returns the accepted generation
plus the same unbuffered stream. Those existing handshake timeouts remain a separate OBLC boundary;
this slice does not add one total deadline around connect plus handshake. The application switches
the admitted stream to nonblocking I/O before the `OBCS` phase.

`submit_terminal_create` and `submit_terminal_start` each take ownership of one already validated
typed request and encode its complete OBCS frame before submission. Encoding failure is definite
and leaves the Session available. A successful encoding starts one two-second monotonic deadline
covering the complete frame write and the one Server-frame read; readiness waits recompute the
remaining budget, so partial progress cannot renew it. Failure before the first write syscall is
definite. Once that syscall begins, any write or read failure or deadline expiry, malformed or
incompatible result, wrong result kind, or wrong correlation makes the effect outcome typed
`OutcomeUnknown` and retires the connection. The transport never retries or returns the request for
replay. A correctly correlated Server `outcome-unknown` result remains a valid OBCS result rather
than a transport ambiguity.

No current invocation reaches this transport. Endpoint selection or discovery, activation,
reconnection, request-identity generation or reconciliation, public command grammar, TUI
composition, and terminal relay remain outside this slice. A later TUI adapter may receive an
already selected endpoint, retain `LocalClient` as its Client Session carrier, inspect its
generation, and submit typed Create or Start intent through these two methods.

### Submission correctness record

| Field        | Contract                                                                                                                                                                                                                                                                                                                    |
| ------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | An internal Client with an already admitted same-user Session must submit one typed Create or Start request without duplicating an effect when transport completion is ambiguous. Endpoint acquisition, process activation, and presentation are excluded.                                                                  |
| Inputs       | Trusted typed `TerminalCreateRequest` or `TerminalStartRequest`, the admitted `ClientSessionConnection`, its accepted generation, a finite request budget, and monotonic time. Server bytes and readiness outcomes are untrusted. An absent connection means `SessionUnavailable`.                                          |
| Data         | One invocation owns one `requestId`, encoded frame, absolute deadline, connection state, possible-handoff boundary, expected result kind, and decoded result class. The `LocalClient` exclusively owns its Session.                                                                                                         |
| Outputs      | Exact correlated typed result; definite `RequestEncoding` or `RequestNotSubmitted`; typed `OutcomeUnknown` with the submitted identity; or `SessionUnavailable`. There is no cancellation, retry, replay return, or optimistic terminal mutation.                                                                           |
| Invariants   | Encoding precedes the deadline and transport; one invocation begins at most one frame handoff; only exact kind and identity complete; every ambiguity after possible handoff retires the Session; a retired Session sends no bytes.                                                                                         |
| Decisions    | Before versus after first possible handoff; write incomplete versus complete; result before versus at/after deadline; exact versus wrong kind or identity; valid versus malformed frame; known versus Server-reported unknown outcome. Deadline observation wins if acceptance has not completed.                           |
| Model        | `LocalClientSubmission` is the bounded one-invocation state refinement. `LocalClient::exchange` is the implementation workflow; its deadline-aware nonblocking stream wrapper is the time and I/O adapter.                                                                                                                  |
| Transitions  | The transition record below defines the serializable abstract phases and guards. Runtime phase is call-local; only the live-or-retired connection state persists between calls.                                                                                                                                             |
| Dependencies | One already admitted Unix stream, monotonic time, readiness polling, OBCS encoding/decoding, and same-stream OBLC admission. The OBCS workflow has no filesystem, process, endpoint-selection, randomness, localization, or UI capability.                                                                                  |
| Ownership    | `LocalClient` is the sole stream and retirement owner. OBCS owns frame and result semantics; OBLC owns admission, credentials, cohort, generation, and its existing handshake timeouts.                                                                                                                                     |
| Verification | Fake-Server tests cover exact Create/Start, correlation, protocol and EOF failures, silent timeout, deterministic partial read/write deadline exhaustion, pre-write expiry, retirement, and no duplicate send. TLC requires a witness for every registered semantic action.                                                 |
| Correctness  | Claims LC-01 through LC-05 below refine implementation branches and deterministic tests to the registered model. TLC exhausts 49 bounded states for one request and one Session; it does not prove OS scheduling, codec correctness, OBLC connect timing, activation, reconnection, reconciliation, or future TUI behavior. |

### Submission transition record

| Current phase                      | Event and guard                                                   | Next phase         | Result and connection effect                                           |
| ---------------------------------- | ----------------------------------------------------------------- | ------------------ | ---------------------------------------------------------------------- |
| `ready`                            | Submit while a Session exists                                     | `encoding`         | No transport effect                                                    |
| `encoding`                         | Frame encoding fails                                              | `definite-failure` | `RequestEncoding`; Session retained                                    |
| `encoding`                         | Frame encoding succeeds and creates one absolute deadline         | `write-ready`      | Encoded frame is private to this invocation                            |
| `write-ready`                      | Deadline or I/O setup fails before the first write syscall        | `definite-failure` | `RequestNotSubmitted`; Session retained                                |
| `write-ready`                      | First write syscall may hand bytes to the kernel                  | `write-begun`      | Handoff count becomes one                                              |
| `write-begun`                      | The one complete frame is written before expiry                   | `awaiting-result`  | Wait for exactly one Server frame under the same deadline              |
| `write-begun` or `awaiting-result` | Deadline, I/O, timeout, or EOF wins                               | `retired`          | `OutcomeUnknown`; connection closed                                    |
| `awaiting-result`                  | Decoded frame is malformed, wrong-kind, or wrong-ID before expiry | `retired`          | `OutcomeUnknown`; connection closed                                    |
| `awaiting-result`                  | Exact correlated result is decoded and deadline is still open     | `completed`        | Return typed known or Server-reported-unknown result; Session retained |
| `retired`                          | Any later submit call                                             | `retired`          | `SessionUnavailable`; no write and no retry                            |

### Correctness traceability

| Claim                                                                                                                              | Implementation and deterministic witness                                                                                                                                                                  | TLA+ evidence                                                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| LC-01: encoding and every failure before the first write are definite.                                                             | `exchange` encodes before starting `SubmissionDeadline`; `deadline_expiry_before_the_first_write_is_a_definite_failure` observes zero Server bytes and a retained generation.                             | `EncodingFails`, `PreWriteDeadline`, `PreWriteIoFails`; `DefiniteFailurePrecedesHandoff`.                      |
| LC-02: one invocation has at most one request-frame handoff and never retries it.                                                  | One encoded buffer is passed once to `write_all`; slow-write and silent-timeout tests reject a second request and subsequent calls return `SessionUnavailable`.                                           | `WriteBegins`; `AtMostOneHandoff`; `NoSendAfterRetirement`.                                                    |
| LC-03: every failure after possible handoff is unknown and retires the Session.                                                    | Write/read, EOF, incompatible frame, wrong kind, and wrong identity branches all call `retire_with_unknown`; focused tests assert the generation disappears.                                              | `WriteFails`, `WriteDeadlineExpires`, `ReadFails`, all retire actions; `UnknownRequiresHandoffAndRetirement`.  |
| LC-04: only exact kind and identity complete, while a correlated Server `outcome-unknown` is data rather than transport ambiguity. | Exact Create/Start, wrong-kind, wrong-ID, and correlated-unknown fake-Server tests exercise the result classifier.                                                                                        | Matching and wrong-result arrival actions; `AcceptedResultIsExactAndBeforeDeadline`.                           |
| LC-05: one monotonic deadline covers the full write and read and is not renewed by progress.                                       | `SubmissionDeadline` is created once; every readiness boundary recomputes its remainder. Stepping-clock tests force sub-deadline byte progress across both read and write until the total budget expires. | `WriteDeadlineExpires`, `ResultDeadlineExpires`, `RetireTimedOutResult`; `TimeoutCannotProduceAcceptedResult`. |

## Staged TUI shell foundation

The following application-private modules are compiled and tested as an integration seam, but no
current invocation enters them:

| Concern     | Current contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ----------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Admission   | The staged Unix adapter requires inherited stdin and stdout to be TTYs with the same terminal-device identity, duplicates only those admitted descriptors, and reads cell and pixel dimensions from the duplicated stdout descriptor. It has no `/dev/tty`, global Crossterm size, or process fallback.                                                                                                                                                                                                                                                                                                                                                                                                                    |
| Lifecycle   | One staged guard owns raw mode, the alternate screen, and cursor visibility. It marks cleanup responsibility before an operation may partially succeed, retains each flag until its inverse succeeds, and attempts each failed inverse at most twice before returning one aggregate error. A process-lifetime singleton panic hook dispatches only when the panicking thread is the active guard's owner, so another thread cannot clear a setup lease before its terminal state is saved; guard drop does not remove a later hook. Normal return, initialization/event/draw failure, owner-thread panic, and each of `SIGHUP`/`SIGINT`/`SIGQUIT`/`SIGTERM` have deterministic restoration or signal-observation evidence. |
| Client View | One invocation owns prefix mode, viewport class, sidebar projection, active overlay, and transient notice. It owns no shared Tab, Panel, Workspace, Site, or Terminal fact and persists nothing.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Layout      | At 96 columns or wider, a 30-column Orchbee business sidebar is docked beside the Workbench region. Narrower viewports keep the Workbench full width and expose the sidebar only as a left overlay. The Workbench projection contains a Tab strip and Panel/Terminal region but labels them as unavailable rather than fabricating shared data.                                                                                                                                                                                                                                                                                                                                                                            |
| Input       | The staged adapter polls and reads at most 4,096 raw bytes at a time from the admitted stdin descriptor. Every completed read is dispatched in order before any subsequent signal, viewport query, or resize decision can terminate that loop iteration. `Ctrl+B` enters visible prefix mode. `b`, `w`, `?`, `q`, and `d` drive only local shell state or exit. Typed Tab and Panel actions remain unavailable. Outside prefix mode, all bytes—including `Ctrl+C`, `Ctrl+D`, escape sequences, UTF-8, and input while an overlay is visible—reach an injectable terminal-input port in order; prefix then `Ctrl+B` forwards one literal byte.                                                                              |
| Outcome     | No current command reaches this shell. The seam does not claim a Server, Client Session, real terminal, detach, attachment, retained-terminal, activation, Workbench mutation, or Site-lifetime behavior.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

The deterministic Client View transition model is:

| State and event                                  | Next state, command, and visible result                                                                                                                                                                                                                                  |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Normal + `Ctrl+B`                                | Enter Prefix; emit no terminal input; render the prefix indicator.                                                                                                                                                                                                       |
| Normal + other bytes, with or without an overlay | Preserve local view state and offer the exact ordered bytes to the terminal-input port. The unavailable port rejects them without side effects and the staged renderer reports that input was not sent.                                                                  |
| Prefix + `b`                                     | Leave Prefix and toggle the docked wide sidebar or narrow sidebar overlay for this Client View.                                                                                                                                                                          |
| Prefix + `w` or `?`                              | Leave Prefix and open the Workbench navigator or shortcut-help overlay locally.                                                                                                                                                                                          |
| Prefix + `c`, `n`, `p`, `1` … `9`, or `h/j/k/l`  | Leave Prefix, preserve shared state, and surface the mapped backend-dependent action as unavailable; no command is submitted.                                                                                                                                            |
| Prefix + `Ctrl+B`                                | Leave Prefix and offer one literal `Ctrl+B` event to the terminal-input port.                                                                                                                                                                                            |
| Prefix + `q` or `d`                              | Leave Prefix and close this unavailable shell.                                                                                                                                                                                                                           |
| Prefix + `Esc`                                   | Cancel Prefix without changing an overlay. Outside Prefix, `Esc` and its following bytes are ordinary terminal input.                                                                                                                                                    |
| Resize                                           | Query the admitted stdout descriptor, update the fixed backend viewport, and recompute only the Client-local viewport class. Crossing into narrow mode hides the docked sidebar; returning wide closes a sidebar overlay and restores the prior wide-sidebar preference. |
| Event/draw error, panic, or termination signal   | Stop the loop and attempt cursor, alternate-screen, and raw-mode restoration without starting or stopping another process.                                                                                                                                               |

The invariant is that every prefix sequence produces at most one typed command, non-prefix bytes
remain ordered, shared Workbench and terminal state never changes in this application, and every
possibly acquired terminal mode retains one bounded restoration owner. Unit tests cover state,
input-port, fixed-backend, layout, renderer, real-PTY same-terminal admission, an actual panic hook,
all four signals, cross-thread setup interleaving, post-read viewport/resize failures, and lifecycle
fault injection. Native PTY process tests separately prove that the current bare/help/version paths
leave terminal modes untouched, that Linux PTY-master `EIO` after the final slave closes is a normal
hangup, and that the linked public binary contains no `/dev/tty` or `tput` fallback token. The staged
runtime and all of its third-party dependencies are Unix-only; the Windows target dependency tree
keeps the current one-shot CLI free of those dependencies, and no Windows TUI or signal-restoration
claim is made.

## Localization and authority

Authored help and diagnostics ship in `en-US` and `zh-CN`. Locale selection checks nonempty
`LC_ALL`, `LC_MESSAGES`, then `LANG`; a `zh` language prefix selects `zh-CN`, and every other value
falls back to canonical `en-US`. Locale data affects presentation only and is never persisted or
used as authority.

The three delivered invocation routes use only Rust standard-library argument, environment,
stdout, and stderr capabilities. They select neither private module, open no file or socket, start
no process or thread, initialize no terminal mode, and read no product state. The local Client
transport and rendezvous primitive are compiled on Unix but remain unreachable from public
dispatch. The delivered TypeScript
[`apps/cli`](../cli/README.md) remains the current CLI authority until a separately accepted
packaged cutover.

## POSIX local Site rendezvous foundation

The application-private `local_site_rendezvous` module stages a mode-checked macOS/Linux
rendezvous primitive for a later accepted activation workflow. Future installation composition
must supply a stable `SiteScope`; cohort version and digest do not belong in that scope, so an
update keeps one Site identity while release, local, and checkout-scoped development installations
can remain distinct. `SiteScope` accepts 1 through 63 ASCII bytes in the canonical form
`[a-z][a-z0-9]*(?:-[a-z0-9]+)*` and therefore cannot add a path separator, dot component, or
platform-dependent encoding.

For one trusted, existing, exactly normalized absolute per-user runtime base, the exact derived
paths are:

| Role               | Stable path                                      |
| ------------------ | ------------------------------------------------ |
| Scope directory    | `<runtime-base>/<site-scope>`                    |
| Activation lock    | `<runtime-base>/<site-scope>/activation.lock`    |
| Site lifetime lock | `<runtime-base>/<site-scope>/site-lifetime.lock` |
| Client socket      | `<runtime-base>/<site-scope>/client.sock`        |
| Worker socket      | `<runtime-base>/<site-scope>/worker.sock`        |

`ModeCheckedLocalSiteRendezvous` deliberately names the incomplete security boundary. Preparation
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

This foundation does not yet establish a production-private runtime boundary on filesystems where
an inherited or extended ACL can grant access beyond the mode bits: it neither inspects nor removes
ACLs. Public activation must remain unwired until an ACL-capable boundary closes that gap. It also
does not choose the runtime base or scope, validate Unix-socket path length or socket metadata,
bind, connect, authenticate peers, launch a process, wait for readiness, unlink an endpoint, or
claim stale-process recovery.

The primitive remains owned by `apps/orchbee` because only the public Client-side activation
boundary has an admitted need in this slice; there is no second source consumer.
`apps/orchbee-server` cannot import another application and does not yet acquire
`site-lifetime.lock`; a later Server lifecycle slice must admit a real shared owner or an explicit
composition seam before both applications can use the same contract.

### Modeling and evidence

| Field        | Current staged contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Give a later local Client activation workflow one path-component-safe, same-effective-UID, mode-checked rendezvous identity without prematurely implementing process activation, readiness, cleanup, or recovery.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Inputs       | A trusted existing exactly normalized absolute runtime base; an installation-owned scope string parsed at the 63-byte ASCII boundary; effective UID; and adversarial directory and lock entries. Relative and non-normal bases, missing scope, links, wrong types, owners, modes, stale files, and held locks are explicit cases.                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Data         | Canonical `SiteScope`; exact derived paths; one retained scope-directory descriptor; and one owned file descriptor per acquired advisory lock. No PID, readiness marker, process identity, durable record payload, or socket state exists.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Outputs      | Preparation or validation returns the retained rendezvous or a typed failure. Each lock attempt returns `Acquired`, `Contended`, or a typed filesystem failure. Client and Worker socket methods return the exact stable paths and perform no I/O.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Invariants   | Every runtime-base component and opened controlled directory or lock entry uses no-follow; a newly initialized scope's opened identity matches the entry inspected before chmod; accepted entries have exact type, effective-UID ownership, mode, and lock link count at admission; accepted activation and lifetime leaves cannot alias one inode at admission; a contended lock attempt never truncates, replaces, unlinks, changes the ownership of an existing inode, or repairs a pre-existing invalid entry, though it may create and mode-normalize a missing lock inode; dropping resources removes nothing; and scope identity is independent of executable cohort.                                                                                                    |
| Decisions    | Exact normalized-base syntax, each descriptor-relative base-component result, scope syntax and length, create-versus-validate policy, descriptor metadata, lock kind, create-versus-existing lock file, and one kernel lock outcome completely determine the result.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Model        | A pure bounded scope/path resolver feeds a POSIX adapter that walks from a trusted root descriptor without canonicalization, then retains the validated scope descriptor. The adapter has no retry loop, clock, process observer, or recovery branch.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Transitions  | A missing scope may become one identity-matched validated directory even under a fully restrictive umask. A missing lock leaf may become one durable validated regular file, including when its creator ultimately returns `Contended`; the creation winner and advisory-lock winner need not be the same caller. An unlocked leaf may become guard-held and returns to unlocked when the guard drops. Existing invalid entries are unchanged.                                                                                                                                                                                                                                                                                                                                  |
| Dependencies | Rust path/value operations, safe `nix` wrappers for effective UID and descriptor-relative POSIX calls, and `File::try_lock` for one advisory exclusive attempt. No network, socket, child-process, thread, time, randomness, persistence payload, or Server dependency is present.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Ownership    | The typed scope comes from future installation composition; this application owns derivation, directory validation, lock-file validation, and guard lifetime. A future activation workflow will own deadlines and process sequencing; Server must separately gain an accepted lifetime-lock seam.                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Verification | Owner-local deterministic tests cover valid and invalid scope names, exact paths, relative and malformed lexical bases, trailing slash and dot/parent components, intermediate/final base links, scope links, restrictive-umask initialization, concurrent permissive-umask creation, non-directories, wrong owner/mode, lock symbolic/hard links, directories, FIFOs, wrong metadata, independent contention, stale inode/content preservation, reacquisition, and cleanup non-ownership.                                                                                                                                                                                                                                                                                      |
| Limits       | The tests run under one effective user and trust kernel metadata and advisory-lock semantics. They inject a different expected UID rather than executing cross-user. Concurrent creation is exercised with a permissive umask; the primitive has no retry if a separate caller observes the short pre-chmod window created by another process with a user-bit-restrictive umask. Durable means that this owner never unlinks or truncates lock files, not that it fsyncs them; kernel filesystem calls have no wall-clock guarantee. No TLA+ model is selected for this atomic primitive; `LocalSiteActivation` owns the accepted workflow's bounded singleton/process-order and abstract total-deadline claims without refining this POSIX adapter or implementing activation. |

## Accepted local Site activation modeling record

[`LocalSiteActivation`](formal/tla/LocalSiteActivation.tla) is maintained design evidence for the
accepted connect-first and lazy-activation workflow. It changes no runtime behavior: bare
invocation still reaches the development-unavailable TUI result, and public activation remains
unwired.

| Field        | Accepted refinement target                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Two concurrent local `orchbee` TUI invocations must coordinate through one Site activation attempt without either becoming the Server or Worker's lifetime supervisor. A present compatible Site should require no launch; an absent Site should start at most one exact installed cohort; every non-success outcome must be explicit.                                                                                                                                                    |
| Inputs       | Two Client invocation slots; TUI, help, and version intent; an initial Site observation of absent, compatible-ready, incompatible, or partial; one activation-lock result; the exact installed Server-plus-Worker cohort capability; typed readiness, incompatibility, partial-cohort, and launcher-failure observations; and an abstract monotonic total-deadline expiry. External process outcomes are untrusted and may never arrive.                                                  |
| Data         | Per-Client phase, typed result, and connection path; one optional activation-lock owner; a zero-or-one launch count and exact-installed cohort tag; Site and typed-readiness state; and a Site-owned ready-service lifetime marker. No PID, path, runtime-base, scope, ACL, process tree, cleanup record, recovery identity, service-manager unit, or numeric time value exists.                                                                                                          |
| Outputs      | A compatible present Site connects directly. An absent Site lets one lock owner start the exact installed cohort while a contender waits for typed readiness. Success connects and releases the caller's activation resource. Incompatible Site, partial cohort, launcher failure, and deadline expiry are distinct failures. Help and version complete without discovery or activation. Client exit ends only that Client.                                                               |
| Invariants   | Every TUI path attempts connection before activation; the activation owner is singular; the launch count is at most one and every launch is the exact installed cohort under the lock; a contender becomes a readiness waiter; only a ready compatible Site can produce a connection; settled Clients own no activation resource; ready-service lifetime belongs to the Site; failures and Client exit never stop or rewrite Site state; and help/version preserve every activation fact. |
| Decisions    | Invocation kind determines whether Site work is permitted. Connection observation distinguishes direct success, absent activation, incompatible failure, and partial-cohort failure. After absence, lock ownership distinguishes launcher from waiter; a lock winner rechecks Site state before launching. Typed readiness, typed failure, or the Client-owned total deadline settles each waiter. Deadline observation may win until connection completes.                               |
| Model        | A bounded workflow over exactly two Client slots and four supported initial Site classes. One scalar `activationOwner` models `activation.lock`; `launchCount` and `launchedCohort` expose singleton exact-cohort launch; `serviceLifetimeOwner` distinguishes the ready Site from every Client. External launcher/readiness actions are nondeterministic and have no fairness assumption.                                                                                                |
| Transitions  | `idle -> connecting -> connected` for a present compatible Site; `connecting -> failed` for present incompatible or partial state; `connecting -> lock-ready -> lock-held -> waiting-readiness` for the launch winner; `lock-ready -> waiting-readiness` for contention; waiting or recheck states settle as connected or one typed failure; `connected -> exited` preserves Site state. Help/version go directly from `idle` to information completion.                                  |
| Dependencies | The model composes the accepted command contract with the current private `LocalClient` admission boundary and `ModeCheckedLocalSiteRendezvous` lock semantics only at their named abstractions. Future runtime refinement must supply installation-owned cohort selection, typed readiness, and one monotonic total deadline. This slice adds no process, socket, filesystem, localization, or public-command implementation.                                                            |
| Ownership    | `apps/orchbee` owns the future Client bootstrap workflow, deadline, activation guard, and typed Client result. Installation composition owns the exact cohort capability. Server and Worker own their readiness and service lifetimes after success. The model grants no Client authority to supervise, stop, replace, downgrade, clean up, or recover a Site.                                                                                                                            |
| Verification | The registered TLC configuration explores four initial Site classes and exactly two Client identities, reaches every maintained semantic action, and checks safety plus Client-local deadline progress. It checks 850 distinct states to depth 13. Symmetry reduction is intentionally disabled because TLC liveness checking with symmetry can miss violations. Runtime refinement evidence is N/A for this model-only slice and remains required before wiring.                         |
| Correctness  | LA-01 through LA-05 below map the accepted claims to actions, invariants, and progress properties. `WF_vars(DeadlineExpires(client))` is the only fairness clause: it represents the contract-guaranteed local total deadline. `CohortBecomesReady`, typed cohort failures, and `LauncherFails` are intentionally unfair, so a permanently silent external process still yields a bounded Client failure rather than an assumed response.                                                 |
| Limits       | The abstraction proves only the checked two-Client, one-Site state space. It does not prove runtime refinement, operating-system scheduling, process creation or detachment, daemonization, service-manager behavior, runtime base or scope, ACLs, socket metadata, cleanup, recovery, retry, update, a concrete deadline or progress threshold, localization, TUI behavior, packaging, public wiring, or external process progress.                                                      |

| Transition owner     | Event and guard                                                      | Result                                                                                                                          |
| -------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| Invocation           | Help or version                                                      | Information completion; Site, lock, launch, readiness, and service-lifetime facts are unchanged                                 |
| Client connection    | Compatible Site is already ready                                     | Direct Client connection; no activation resource or launch                                                                      |
| Client connection    | Site is incompatible or partial                                      | Corresponding typed failure; no activation or Site mutation                                                                     |
| Activation contender | Initial connection finds no ready Site and `activation.lock` is held | Wait for the same typed readiness result under the caller's total deadline                                                      |
| Activation winner    | Lock acquired, recheck still finds absence, no launch exists         | Start exactly one installed Server-plus-Worker cohort and wait while retaining the guard                                        |
| Activation winner    | Recheck finds ready, in-progress, incompatible, partial, or failed   | Connect, release and wait, or fail according to the exact observation; never start a second cohort                              |
| External cohort      | Launch is pending                                                    | May publish ready, incompatible, partial, launcher failure, or no result at all; no external completion is fair                 |
| Pending Client       | Typed result or total deadline is observed                           | Connect or return `incompatible`, `partial-cohort`, `launcher-failure`, or `timeout`; release any caller-owned activation guard |
| Connected Client     | Client exits                                                         | End only the Client Session; preserve Site, cohort, readiness, launch, and service-lifetime state                               |

Runtime refinement is deliberately N/A in this slice because no invocation reaches activation.
Future wiring must add deterministic process tests before claiming the model maps to implementation.

| Claim                                                                                                    | TLA+ evidence                                                                                                                                                                                                                               | Bound                                                                                                                                                       |
| -------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| LA-01: every activating TUI connects first; present compatible, help, and version paths never launch.    | `ConnectFindsCompatible`, all connection-observation actions, `CompleteHelp`, `CompleteVersion`; `ConnectionProbeNeverActivates`, `InformationInvocationHasNoActivationEffect`.                                                             | Models command intent and abstract observations, not parser, transport, or process refinement.                                                              |
| LA-02: concurrent absence produces at most one exact installed-cohort launch under one lock.             | `AcquireActivationLock`, `ActivationLockContended`, `LaunchExactInstalledCohort`; `ActivationOwnerIsUniqueAndPending`, `LaunchIsSingletonAndExact`, `LaunchStartsOnlyUnderTheActivationLock`, `ContenderWaitsForTypedReadiness`.            | Exactly two Client slots and one activation round; no runtime-base, lock-file, or process implementation claim.                                             |
| LA-03: incompatibility, partial cohort, launcher failure, and timeout are explicit and do not stop Site. | Direct, recheck, and readiness failure actions plus `DeadlineExpires`; `ClientShapeIsExact`, `ExplicitFailureHasNoServiceStopEffect`.                                                                                                       | Failure presentation, cleanup, recovery, retry, and stale-artifact policy remain excluded.                                                                  |
| LA-04: connection releases Client activation ownership; Client exit does not supervise or stop Site.     | `RecheckCompatible`, `ObserveReady`, `ExitClient`; `SettledClientReleasesItsActivationResource`, `ReadySiteOwnsItsServiceLifetime`, `NoClientSupervisesServiceLifetime`, `ClientExitPreservesSite`, `ActivationResourceEventuallyReleases`. | Models ready-service ownership only; it does not select how processes detach or how service lifetime is implemented.                                        |
| LA-05: every started TUI settles without assuming that an external process responds.                     | `EveryPendingTuiEventuallySettles` under only per-Client weak fairness for `DeadlineExpires`; external cohort and launcher completions have no fairness clause.                                                                             | Assumes the future adapter actually guarantees observation of one finite monotonic total deadline; no numerical value, OS scheduling, or external liveness. |

The current one-shot process path uses only Rust standard-library argument, environment, stdout,
and stderr capabilities. It opens no file or socket, starts no process or thread, reads no product
state, and never reaches the staged shell. The staged modules use Ratatui for stateful rendering,
Crossterm only for queued ANSI output, Nix/Rustix only against inherited descriptors, and
signal-hook for atomic flags. They have no Server, Worker, or Terminal Host dependency, and all of
these third-party dependencies are declared only for Unix targets. The delivered TypeScript
[`apps/cli`](../cli/README.md) remains the current CLI authority until a separately accepted
packaged cutover.

## Measurement

Collect the reference release baseline from the repository root:

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

The script builds the exact locked release package, warms and repeatedly invokes
`orchbee --version`, reports artifact bytes and warm latency percentiles, and reports peak resident
memory where `/usr/bin/time` exposes it. Results describe only the printed host, toolchain, profile,
and invocation. They are not acceptance budgets, cold-cache guarantees, or evidence for future TUI
performance.

The staged-shell, current-public-path reference run on 2026-09-03 used Apple M1 arm64, Darwin
23.6.0, Rust 1.97.0, the default Cargo release profile, 5 warmups, and 50 measured invocations:

| Measurement          | Reference observation |
| -------------------- | --------------------- |
| Executable size      | 432,144 bytes         |
| Warm startup minimum | 1.216 ms              |
| Warm startup p50     | 1.564 ms              |
| Warm startup p95     | 1.936 ms              |
| Warm startup maximum | 2.151 ms              |
| Peak resident memory | 1,343,488 bytes       |

Rerun and replace this reference after a later accepted slice changes the executable's reachable
dependency graph or invoked startup path. This staged module is not reachable from an invocation route.
Supported-platform acceptance budgets remain reserved.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
node scripts/tla/check-inventory.mjs
node scripts/tla/run-models.mjs --models local-client-submission,local-site-activation
cargo tree -p orchbee --target x86_64-pc-windows-msvc --locked
pnpm verify
```
