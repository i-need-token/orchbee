# Saved Windows machines

The native Windows Client owns saved SSH machine profiles. The six `machine`
commands manage labels, SSH targets, explicit remote Site names, and enabled
state. OpenSSH owns credentials, keys, host trust, and SSH configuration.
Reference behavior is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`.

## Catalog and transitions

`machines/catalog.json` beside the selected Windows configuration file is a
private, atomic, schema-version-1 document. Its revision is a nonnegative safe
integer; profiles have distinct random 32-character lowercase hexadecimal IDs.
At most 64 profiles and 128 KiB are admitted. Labels are nonblank, control-free,
and at most 128 UTF-8 bytes. Targets are at most 1024 ASCII bytes: an SSH host
alias, `[user@]host`, or `ssh://[user@]host[:port]`; bracketed IPv6 is supported.
Passwords and shell metacharacters are rejected. Session names use the native
Windows Site grammar, defaulting to `default`. Duplicate target/session pairs
may have distinct IDs, as in the reference.

| Event          | Guard and transition                                                                                  | Effects                                    |
| -------------- | ----------------------------------------------------------------------------------------------------- | ------------------------------------------ |
| List           | Missing catalog is empty; invalid data fails                                                          | Read only; no directory creation           |
| Add            | Validate and check capacity; prepare remote; reload latest catalog under lock; append enabled profile | Remote Site may start before local commit  |
| Rename         | Exact ID exists; valid label                                                                          | Replace label; same value is unchanged     |
| Enable/disable | Exact ID exists                                                                                       | Set flag; same value is unchanged          |
| Remove         | Exact ID exists                                                                                       | Remove profile; never stop its remote Site |

Writers use the existing Windows private-cache lock and atomic replacement.
Revision advances once per actual change. Reads reject unknown fields, invalid
identities, duplicate IDs, overflow, oversized files, and unsafe filesystem
objects. Corruption never becomes an empty catalog. A failed local commit does
not roll back a remotely started Site. Cancellation after commit preserves the
committed receipt.

## Remote preparation

Add uses the OS OpenSSH executable and a 60-second total deadline. Batch mode,
strict host-key checking, no forwarding, bounded output, and an owned process
Job apply. Password prompts and new host trust must first be handled through
OpenSSH. SSH diagnostics are not copied into machine JSON or product errors.
The Windows remote command is an encoded PowerShell program with literal data.

Preparation checks the installed `orchbee.exe` Client version, platform,
protocols, and admitted cohort before starting the selected Site. A second SSH
invocation checks that the same cohort is still running and compatible after
the first invocation has ended. Only then can the profile be committed. The
receipt distinguishes `not-contacted`, `unchanged`, `possibly-started`, and
`ready` remote state, independently of the local catalog commit. Unknown
process settlement is reported, never retried as a successful add.

Missing or incompatible remote installations fail without saving. Automated
remote installation and interactive authentication remain outside the implemented
boundary; an enabled profile is not evidence of a live connection. No remote
session is stopped by rename, enable, disable, or remove.

## Evidence and lifecycle

Deterministic validation and mutation traces cover invalid inputs, repeated
operations, capacity, and revision bounds. Private-store tests exercise the
real Windows lock and atomic writer. Remote-program tests run real PowerShell
against owned fixtures, including failed preparation and malformed replies.
These tests do not establish a real two-machine SSH/TUI witness. Fixtures and
their process Jobs are owned by the test and released after the last consumer.

## Command routing contract

`orchbee --machine <id-or-exact-label> <command>` selects an enabled saved
profile. ID takes precedence; duplicate labels are ambiguous. The selector
is a prefix, accepts `--machine=value`, cannot combine with `--site`, and never
changes the catalog. Help is local. Remote execution admits CLI commands
for workspace, tab, panel, Client queries/control, terminal queries/send, Agent operations, worktrees,
notifications, runtime configuration, manifests, and supported plugin operations.
Local management, installation, TUI, and terminal interactive streams are refused.

The owner is the native CLI relay, not the local IPC transport. It first probes
the remote Windows Client, then sends one bounded versioned request to that
exact executable. A private relay entry point revalidates the cohort, Site,
argument grammar and command allowlist before spawning its own executable with
literal arguments in an owned Windows Job. The existing remote CLI remains the
business-operation owner. No local peer credential admission is bypassed.
Terminal file and stdin sources are acquired on the caller once and sent as raw
bytes; directories and executable paths are interpreted by the remote command.
Caller terminal/worker context is cleared; explicit targets or remote Client
selection determine the destination. Output bytes, diagnostics and exit status
are returned in a bounded reply, never reconstructed from SSH diagnostics.

Preflight and local input each have a 60-second bound. Execution honors the
command's own deadline, including explicit forever waits; cancellation settles
the local SSH Job. Losing transport after dispatch cannot prove remote process
settlement or business rollback: the failure reports an unknown remote outcome
and never retries. Before dispatch it reports not-submitted. The remote helper
owns and settles its command Job; a disconnected peer can still leave an already
accepted business action, and no distributed cancellation guarantee is claimed.
Wire request/reply limits and output truncation fail closed. No real SSH or
cross-machine acceptance is inferred from fixture-only relay tests.

Private requests are at most 2 MiB and replies at most 24 MiB; raw stdout and
stderr are each at most 8 MiB. Arguments are bounded to 64 KiB and 1024 entries.
PowerShell completion supports the selector prefix and routed command scope.

## Persistent TUI connections

The Windows TUI discovers the saved catalog while open and offers a machine
picker, opened with Ctrl+B then Shift+M or the footer indicator. Selection is by
profile identity, not row index or label. Local remains
available. A disabled, removed, ambiguous, or corrupt entry cannot silently
become another machine. Catalog discovery does not claim online status or seize
remote terminal control: a selected enabled profile starts a connection attempt.
The current surface stays usable until the candidate reaches a drawable state;
cancellation or failure keeps the current surface. Successful switching preserves
per-machine views and invalidates pointer/input state before accepting new input.
Local is retained; at most four inactive remote surfaces are cached. Eviction
releases the Client connection, leaving the remote Site running. Cached inactive
surfaces withdraw their Client-control, notification and outer-title subscriptions;
returning reopens them and transfers the host focus observation. A hidden surface
must not advertise a Ready control destination or acknowledge visible drawing.

The persistent transport owner creates a private, same-user local pipe endpoint
and a bounded SSH byte relay for each required native Client lane. SSH authenticates
the remote account; the exact admitted remote executable verifies its installed
cohort and opens only the selected existing Site's same-user pipe. Native Client
admission and application frames pass unchanged through that stream. Neither end
fabricates an accepted generation or bypasses Server cohort admission. Remote
terminal input, layout, metadata and control keep their existing semantic owners.

A transport guard retained by Client origins owns its listener, relay task and
SSH Jobs, including secondary query/control lanes. Each lane has bounded buffers;
backpressure pauses reads instead of dropping bytes. EOF retires that lane, and
no submitted frame is replayed. Dropping the last guard cancels and joins the
relay and settles children. The remote stdio adapter likewise owns and settles
its binary reader/writer Jobs. Disconnecting a Client does not stop the remote
Site or terminal processes. No port forwarding, host-key acceptance, credentials
storage, remote installation or POSIX implementation is added.

The picker owns keyboard and mouse input while open. Escape cancels; Enter and
click choose an exact painted identity. Resizing or catalog replacement invalidates
painted rows; pasted text and the remainder of a closing input packet cannot leak
into the underlying terminal. Each asynchronous attempt has an identity and a
cancellation owner; stale completion cannot switch the active machine.

Evidence must include binary full-duplex/backpressure/EOF/cancellation, same-user
pipe admission through a real helper process, picker identity and stale-result
traces, and rendered keyboard/mouse paths. Local fixtures do not establish real
SSH network survival, remote installation, or complete two-machine acceptance.
