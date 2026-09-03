# Orchbee Native Entry

Experimental, unshipped Rust process entry for the accepted Phase 3A native-command foundation.
The emitted executable is named `orchbee`. It participates in the root Cargo verification gate but
is not consumed by Desktop, packaging, an installer, or a package manager.

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

Bare invocation selects the future TUI route and currently reports a localized
development-unavailable result. It does not enter terminal mode or render a placeholder interface.
`--help` and `--version` complete without TUI or backend initialization. Every other, additional,
or non-Unicode argument is a usage failure. There are no short flags, aliases, abbreviations,
business commands, target discovery, current-directory inference, or compatibility behavior.

Help writes only stdout. The unavailable and usage results write only stderr. Version writes one
locale-neutral `orchbee <version>` line to stdout. Development exit statuses are `0` for help and
version, `1` for the unavailable bare TUI route, and `2` for usage failure. They are not the future
stable public exit table.

## Localization and authority

Authored help and diagnostics ship in `en-US` and `zh-CN`. Locale selection checks nonempty
`LC_ALL`, `LC_MESSAGES`, then `LANG`; a `zh` language prefix selects `zh-CN`, and every other value
falls back to canonical `en-US`. Locale data affects presentation only and is never persisted or
used as authority.

The three delivered invocation routes use only Rust standard-library argument, environment,
stdout, and stderr capabilities. They call no rendezvous code, open no file or socket, start no
process or thread, initialize no terminal mode, and read no product state. The delivered TypeScript
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

| Field        | Current staged contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Give a later local Client activation workflow one path-component-safe, same-effective-UID, mode-checked rendezvous identity without prematurely implementing process activation, readiness, cleanup, or recovery.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Inputs       | A trusted existing exactly normalized absolute runtime base; an installation-owned scope string parsed at the 63-byte ASCII boundary; effective UID; and adversarial directory and lock entries. Relative and non-normal bases, missing scope, links, wrong types, owners, modes, stale files, and held locks are explicit cases.                                                                                                                                                                                                                                                                                                                                                                  |
| Data         | Canonical `SiteScope`; exact derived paths; one retained scope-directory descriptor; and one owned file descriptor per acquired advisory lock. No PID, readiness marker, process identity, durable record payload, or socket state exists.                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Outputs      | Preparation or validation returns the retained rendezvous or a typed failure. Each lock attempt returns `Acquired`, `Contended`, or a typed filesystem failure. Client and Worker socket methods return the exact stable paths and perform no I/O.                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Invariants   | Every runtime-base component and opened controlled directory or lock entry uses no-follow; a newly initialized scope's opened identity matches the entry inspected before chmod; accepted entries have exact type, effective-UID ownership, mode, and lock link count at admission; accepted activation and lifetime leaves cannot alias one inode at admission; a contended lock attempt never truncates, replaces, unlinks, changes the ownership of an existing inode, or repairs a pre-existing invalid entry, though it may create and mode-normalize a missing lock inode; dropping resources removes nothing; and scope identity is independent of executable cohort.                       |
| Decisions    | Exact normalized-base syntax, each descriptor-relative base-component result, scope syntax and length, create-versus-validate policy, descriptor metadata, lock kind, create-versus-existing lock file, and one kernel lock outcome completely determine the result.                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Model        | A pure bounded scope/path resolver feeds a POSIX adapter that walks from a trusted root descriptor without canonicalization, then retains the validated scope descriptor. The adapter has no retry loop, clock, process observer, or recovery branch.                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Transitions  | A missing scope may become one identity-matched validated directory even under a fully restrictive umask. A missing lock leaf may become one durable validated regular file, including when its creator ultimately returns `Contended`; the creation winner and advisory-lock winner need not be the same caller. An unlocked leaf may become guard-held and returns to unlocked when the guard drops. Existing invalid entries are unchanged.                                                                                                                                                                                                                                                     |
| Dependencies | Rust path/value operations, safe `nix` wrappers for effective UID and descriptor-relative POSIX calls, and `File::try_lock` for one advisory exclusive attempt. No network, socket, child-process, thread, time, randomness, persistence payload, or Server dependency is present.                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Ownership    | The typed scope comes from future installation composition; this application owns derivation, directory validation, lock-file validation, and guard lifetime. A future activation workflow will own deadlines and process sequencing; Server must separately gain an accepted lifetime-lock seam.                                                                                                                                                                                                                                                                                                                                                                                                  |
| Verification | Owner-local deterministic tests cover valid and invalid scope names, exact paths, relative and malformed lexical bases, trailing slash and dot/parent components, intermediate/final base links, scope links, restrictive-umask initialization, concurrent permissive-umask creation, non-directories, wrong owner/mode, lock symbolic/hard links, directories, FIFOs, wrong metadata, independent contention, stale inode/content preservation, reacquisition, and cleanup non-ownership.                                                                                                                                                                                                         |
| Limits       | The tests run under one effective user and trust kernel metadata and advisory-lock semantics. They inject a different expected UID rather than executing cross-user. Concurrent creation is exercised with a permissive umask; the primitive has no retry if a separate caller observes the short pre-chmod window created by another process with a user-bit-restrictive umask. Durable means that this owner never unlinks or truncates lock files, not that it fsyncs them; kernel filesystem calls have no wall-clock guarantee. No TLA+ model is selected for this atomic primitive; the accepted future activation workflow still owes its singleton/process-order model and total deadline. |

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

The Phase 3A reference run on 2026-09-02 used Apple M1 arm64, Darwin 23.6.0, Rust 1.97.0, the
default Cargo release profile, 5 warmups, and 50 measured invocations:

| Measurement          | Reference observation |
| -------------------- | --------------------- |
| Executable size      | 432,144 bytes         |
| Warm startup minimum | 1.179 ms              |
| Warm startup p50     | 1.466 ms              |
| Warm startup p95     | 2.921 ms              |
| Warm startup maximum | 4.046 ms              |
| Peak resident memory | 1,343,488 bytes       |

Rerun and replace this reference after a later accepted slice changes the executable's reachable
dependency graph or invoked startup path. This staged module is not reachable from an invocation route.
Supported-platform acceptance budgets remain reserved.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
pnpm verify
```
