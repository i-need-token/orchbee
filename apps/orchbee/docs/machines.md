# Remote machines

The native Client owns the catalog of saved remote machines. The `machine` commands manage
labels, connections, explicit remote Session names, and enabled state; the machines themselves
are reached only from Windows today. The person's own configuration for a connection kind owns
everything else: for SSH, OpenSSH owns credentials, keys, ports, host trust, and SSH configuration.

## Catalog and transitions

`internal/state/machines.json` in the data root is a private, atomic document; an explicit
`ORCHBEE_CONFIG_FILE` does not move it. A read keeps a record of an earlier version migrated in
memory; the first committed edit backs the original up under `internal/backups/` and saves the
migration before applying the edit. A record with a version this build cannot use, saved by a
newer version or not updated from an earlier one, is never rewritten: saved machines read as none,
edits last only until the process ends, and the process shows the record message once.

```json
{
	"version": 1,
	"revision": 7,
	"profiles": [
		{
			"id": "0f3c5a7e9b1d2f4a6c8e0b2d4f6a8c0e",
			"label": "devbox",
			"connection": { "kind": "ssh", "target": "luna@10.0.0.8" },
			"session": "default",
			"enabled": true,
			"remoteInstall": "manual",
			"extensions": {}
		}
	],
	"extensions": {}
}
```

| Member                     | Rule                                                                                                                                                                                                                                                                         |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `version`                  | Integer `1`                                                                                                                                                                                                                                                                  |
| `revision`                 | Nonnegative safe integer; advances once per actual change                                                                                                                                                                                                                    |
| `profiles`                 | At most 64 machines with distinct random 32-character lowercase hexadecimal IDs                                                                                                                                                                                              |
| `label`                    | Nonblank, control-free, at most 128 UTF-8 bytes; several machines may share one                                                                                                                                                                                              |
| `connection`               | An object with a lowercase `kind` of at most 64 letters, digits, or hyphens. `ssh` admits exactly `target`: at most 1024 ASCII bytes, an SSH host alias, `[user@]host`, or `ssh://[user@]host[:port]`, bracketed IPv6 supported, passwords and shell metacharacters rejected |
| `session`                  | The native Windows Session grammar, defaulting to `default`                                                                                                                                                                                                                  |
| `enabled`                  | Saved preference, never evidence of a live connection                                                                                                                                                                                                                        |
| `remoteInstall`            | `managed` when Orchbee installed the remote copy and keeps it in step with this computer; `manual` when the remote copy was installed some other way and is left alone                                                                                                       |
| `extensions` (both levels) | Objects a later release fills; this one keeps them unread, writes them back unchanged, and writes `{}` when they are absent                                                                                                                                                  |

A connection `kind` this build does not know is kept verbatim with every member it carries and
written back unchanged. That machine is listed as unsupported by this version, cannot be routed
to, chosen, or discovered, and leaves every other machine usable. The record never stores a
credential, password, port, key path, or the remote platform. Duplicate target/session pairs may
have distinct IDs. At most 128 KiB is admitted.

`machine add` saves `remoteInstall: "manual"`: this release only adds a machine whose Orchbee is
already installed. Without `--label`, the label is the target's host name, so
`luna@10.0.0.8` saves as `10.0.0.8` and `ssh://u@[2001:db8::1]:22` as `2001:db8::1`.

Every command that names a saved machine takes `<id|label>` and resolves it with the one rule
that `--machine` uses: an exact ID first, otherwise the only machine with that exact,
case-sensitive label. No match is `not-found`; several machines with that label are
`ambiguous-machine`, and only the ID selects one of them. Edits resolve the operand under the
writer lock, so the machine changed is the one the catalog names at commit time.

| Event          | Guard and transition                                                                                         | Effects                                       |
| -------------- | ------------------------------------------------------------------------------------------------------------ | --------------------------------------------- |
| List           | Missing catalog is empty; invalid data fails                                                                 | Read only; no directory creation              |
| Add            | Validate and check capacity; prepare remote; reload latest catalog under lock; append enabled `manual` entry | Remote Session may start before local commit  |
| Rename         | Operand resolves; valid label                                                                                | Replace label; same value is unchanged        |
| Enable/disable | Operand resolves                                                                                             | Set flag; same value is unchanged             |
| Remove         | Operand resolves                                                                                             | Remove machine; never stop its remote Session |

Writers use the existing Windows private-cache lock and atomic replacement. Reads reject unknown
members outside `extensions`, a malformed connection, an unknown `remoteInstall`, invalid
identities, duplicate IDs, overflow, oversized files, and unsafe filesystem objects; a record
without `connection` or `remoteInstall` is invalid, with no reader for an earlier shape.
Corruption never becomes an empty catalog and its bytes are preserved. A failed local commit does
not roll back a remotely started Session. Cancellation after commit preserves the committed
receipt.

## Commands not available yet

`machine connect <id|label> [--remember]`, `machine disconnect <id|label>`, `machine add
--no-install`, and `machine remove --stop-session` belong to later slices of remote-machine
support. They appear in help and completion, marked not available yet, and their whole grammar is
validated, so a malformed invocation is still a usage failure with exit 2. A well-formed one is
refused before it reads the catalog, contacts any machine, or changes anything: the
`orchbee.machine` receipt names the operation (`connect`, `disconnect`, `add`, or `remove`),
reports `error: "not-yet-available"`, `remote: "not-contacted"`, `profileId: null`, and the
command exits 4.

## Machine receipt

`orchbee.machine` version 1 reports each machine as
`{id, label, connection, session, enabled, remoteInstall}`. `connection` is
`{ "kind": "ssh", "target": … }`, or the shared `{ "kind": "unsupported" }` for a kind this build
does not know; the content of an unknown kind is never echoed. `error` is a closed list that grows
as remote-machine support lands; a reader treats a value it does not know as `unknown`.

| `error`                                             | Exit | Produced by this release                                      |
| --------------------------------------------------- | ---- | ------------------------------------------------------------- |
| `configuration`, `invalid-catalog`, `storage`       | 1    | Catalog location, content or storage failure                  |
| `internal`                                          | 1    | Unexpected local failure                                      |
| `not-found`, `ambiguous-machine`                    | 3    | `<id\|label>` resolution                                      |
| `host-untrusted`, `host-key-changed`, `auth-failed` | 3    | Not yet; OpenSSH failures read as `ssh-failed`                |
| `busy`, `capacity`, `revision-exhausted`            | 4    | Another writer, 64 machines or 128 KiB, revision bound        |
| `not-yet-available`                                 | 4    | [Commands not available yet](#commands-not-available-yet)     |
| `ssh-unavailable`                                   | 4    | The system OpenSSH client is missing                          |
| `remote-not-installed`                              | 4    | The remote launch line finds no Orchbee, or no usable install |
| `remote-not-ready`                                  | 4    | The remote Session did not report ready                       |
| `host-unreachable`, `auth-required`                 | 4    | Not yet                                                       |
| `ssh-failed`, `remote-protocol`                     | 5    | Any other OpenSSH failure; a malformed remote document        |
| `remote-unsupported`, `remote-other-environment`    | 6    | Remote system or architecture; another environment kind       |
| `version-incompatible`                              | 6    | Not yet                                                       |
| `timeout`                                           | 7    | Deadline                                                      |
| `physical-unknown`                                  | 8    | SSH process cleanup unconfirmed                               |
| `remote-install-failed`                             | 9    | Not yet                                                       |
| `cancelled`                                         | 130  | Interrupt                                                     |

## Remote preparation

The app-private `machine_ssh` owner exposes executable discovery and bounded process execution
through the same typed ports for CLI preparation and TUI admission. Each workflow retains its
own readiness and submission facts. Windows ports keep the system OpenSSH and process owner;
Unix ports return `SavedMachines` unsupported before executable lookup or process creation.

Add uses the OS OpenSSH executable and a 60-second total deadline. Batch mode,
strict host-key checking, no forwarding, bounded output, and an owned process
Job apply. Password prompts and new host trust must first be handled through
OpenSSH. SSH diagnostics are not copied into machine JSON or product errors.
The Windows remote command is an encoded PowerShell program with literal data.

Preparation reads the remote installed Client's status document and applies the SSH admission
rule before starting the selected Session:

| Check            | Rule                                                                                                                           | Refusal                               |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------- |
| Platform         | `windows` on `x86_64` or `aarch64`                                                                                             | `remote-unsupported`, exit 6          |
| Executable       | An absolute drive path whose file name, ignoring ASCII case, is a valid environment stem plus `.exe`                           | `remote-protocol`, exit 5             |
| Environment kind | A release build admits only `orchbee.exe`; a development or test build admits any `ob-dev-<16 hex>` or `ob-test-<16 hex>` name | `remote-other-environment`, exit 6    |
| Product version  | Any version the status document grammar admits; it is reported as `remoteVersion` and never compared                           | `remote-protocol` only when malformed |
| Installation     | The cohort state is `available`                                                                                                | `remote-not-installed`, exit 4        |

The remote launch line exits 125 when it resolves no Orchbee program or cannot start it; that
status reads as `remote-not-installed` rather than an OpenSSH failure.

No protocol version, product version, or cohort is compared across machines: the remote Session
admits each later tunnel connection through the handshake, and the routed command runs the remote
Client under its own grammar. The probe resolves the remote program through the remote store's
`active.json`, never through `PATH`: in the store root (`ORCHBEE_INSTALL_DIR`, or a folder under
`%LOCALAPPDATA%\Programs`) it reads the active version and runs `versions\<version>\<name>.exe`. A
missing or unreadable pointer names no installation. A release build reads the `Orchbee` store
and looks for `orchbee.exe`; a development or test build reads its own `<stem>` store and then the
`Orchbee` store, looking for `<stem>.exe` and then `orchbee.exe` in each, so a remote with only
the release installation reads as another environment rather than a missing one. See
[Windows release installation](windows-cli-configuration.md#windows-release-installation).

A second SSH invocation, after the first has ended, requires the remote Client's
`orchbee.status.server` document to report the Session running with a ready Server process. Only
then can the profile be committed. The receipt distinguishes `not-contacted`, `unchanged`,
`possibly-started`, and `ready` remote state, independently of the local catalog commit, and an
admitted add reports the remote product version. Unknown process settlement is reported, never
retried as a successful add.

Missing, unsupported, or other-environment remote installations fail without saving. Automated
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
machine by the [`<id|label>` rule](#catalog-and-transitions). Selection refuses `not-found`,
`ambiguous-machine`, and `disabled-machine` with exit 3, and `unsupported-machine`, a connection
kind this build does not know, with exit 4. The selector
is a prefix, accepts `--machine=value`, cannot combine with `--session`, and never
changes the catalog. Help is local. Remote execution admits CLI commands
for workspace, tab, panel, Client queries/control, terminal queries/send, Agent operations, worktrees,
notifications, runtime configuration, manifests, and supported plugin operations.
Local management, installation, TUI, and terminal interactive streams are refused.

The owner is the native CLI relay, not the local IPC transport. It first probes
the remote Windows Client under the [SSH admission rule](#remote-preparation), then sends one
bounded request packet to that exact executable through `--machine-relay-v1`. The packet and its
reply are closed objects whose `version` is their `machine-route` generation in the
[connection registry](../../../schemas/connection/v1/connections.json); each may carry an
`extensions` object whose member names follow the capability-name grammar and whose content the
receiver ignores. Neither carries a cohort. A private relay entry point refuses a generation it
does not read, then revalidates the Session, argument grammar and command allowlist under its own
grammar before spawning its own executable with literal arguments in an owned Windows Job.
The packet's `locale` names the sender's language; a value the receiver does not ship, including
one a later sender adds, falls back to `en_US.UTF-8` instead of refusing the command, and the
routed command runs under the resolved locale.
When the remote grammar cannot parse the routed arguments, the reply carries the remote's ordinary usage failure in the packet's locale and with the usage exit status, rendered by the relay entry point without spawning a command. A parsed request outside the routed scope, a packet that names a Session, and invalid input are refused as not submitted. The existing remote CLI remains the
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

## Link helper

`orchbee --machine-link-v1`, with no other argument, is the remote end of the
[`machine-link`](../../../schemas/machine/README.md) connection: the hub starts it over `ssh` and
speaks to it on its standard input and output. One pair of pipes carries several Sessions' byte
streams, routed commands, and Session starts at once. Windows serves it; elsewhere it answers
unsupported before reading a byte, as `--machine-tunnel-v1` and `--machine-relay-v1` do. Those two
helpers stay until their last caller moves to the link. The contract owns every byte; this section
owns how the helper meets it.

A console on standard input or output refuses the start: the helper writes nothing and exits 5.
Otherwise it answers the `OBHS` frame within the 2-second candidate deadline counted from its
start. A `machine-link` offer gets the answer and then the `facts` event: `platform`
`windows`, the build's `architecture`, the running `executable`, and `storeRoot`, the release
store root `ORCHBEE_INSTALL_DIR` or the build's folder under `%LOCALAPPDATA%\Programs`, when that
resolves. The control request `query` gets one `machine-link` process entry with `terminals` 0;
`drain` gets state `stopping` and `exit` its reply, and both end the helper. Any other frame gets
the handshake's own answer: a rejection, or `unsupported` for an unknown request.

| Channel          | Accept                                | Responder data and result                                                                                                                                                             |
| ---------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `session`        | Once the named Session's pipe is open | Bytes from that pipe, then `close` `ended` when it closes or `failed` on a read error. A Session that is not running resets `session-not-running`; an unopenable pipe `unavailable`   |
| `route`          | At once                               | After the opener's `end`, the reply packet of the [routed command](#command-routing-contract), under the same admission and allowlist as `--machine-relay-v1`, then `replied`         |
| `ensure-session` | At once                               | `ready` with `started` once the Session admits a Client, as `server start` would; `unknown` when a start ran out of its 30 seconds or its Server may have started; `failed` otherwise |

A name the Windows Session grammar refuses resets `invalid-session` before `accept`. The pipe is
found as a local Client finds it, through the data root's Session catalog spelling, and must
belong to the same user. `ensure-session` uses the same activation as `server start`; the Server it
starts outlives the helper. `install` is answered as an unknown kind until its capability ships.

### Ownership and bounds

The helper is one reader of standard input, one writer of standard output, a supervisor, and
workers per channel. One lock guards the connection state machine of `crates/machine-protocol` and
the channel table; no thread holds it across I/O, and no thread is started per frame.

| Resource                        | Owner                                                     | Bound                                                                                            |
| ------------------------------- | --------------------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Standard input                  | Reader thread                                             | One 64 KiB read buffer and one partial frame of at most 65,552 bytes; an `OBHS` frame 1 MiB      |
| Queued frames other than `data` | Connection state machine                                  | The reader stops reading while 256 wait, so a peer that does not read cannot grow the queue      |
| Committed `data` of a channel   | Connection state machine, filled by the channel's workers | Workers commit only while less than 64 KiB of the channel is unwritten: at most 96 KiB           |
| Standard output                 | Writer thread                                             | One frame at a time, written in full before the next is taken                                    |
| `session` bytes for the pipe    | Channel table, drained by the channel's pipe writer       | 262,144 bytes: credit is granted again only for bytes written to the pipe                        |
| `session` bytes from the pipe   | The channel's pipe reader                                 | One 32 KiB buffer, read again only once all of it is committed                                   |
| `route` request packet          | Channel table until `end`, then the command worker        | 2,097,152 bytes; beyond it the state machine resets `too-large`                                  |
| `route` reply packet            | The command worker                                        | 25,165,824 bytes, built once from output capped at 8 MiB per stream                              |
| Channels                        | Connection state machine                                  | 256 open; the next `open` is reset `capacity`                                                    |
| Channel table entries           | Channel table                                             | 512, counting closed channels whose workers still settle; the next `open` is reset `unavailable` |
| Threads                         | Process                                                   | Three, plus two per `session` channel and one per started `route` or `ensure-session` channel    |

| Event                               | Effect                                                                                                                                                           |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Standard input ends or fails        | Every open channel is reset `stopping` and its work ends; best-effort `bye` `done`; exit 0                                                                       |
| The hub's `bye`                     | The same as the end of standard input                                                                                                                            |
| A frame breaks the contract         | The reader stops; only `bye` `violation` is written; every channel ends; exit 5                                                                                  |
| 90 seconds without a received frame | As at the end of standard input, with `bye` `idle`                                                                                                               |
| Standard output fails               | Every channel ends; exit 0                                                                                                                                       |
| The opener's `end` on `session`     | What the channel holds is written to the pipe, then the pipe is closed, which ends the channel `ended`                                                           |
| The opener's `reset`                | The pipe is closed, a running routed command is ended through its Job, an activation finishes unobserved                                                         |
| A Session pipe closes or fails      | What was read is committed, then `close` `ended` or `failed`                                                                                                     |
| The hub reads slowly                | The writer blocks; workers stop committing at the 64 KiB mark; the pipe reader stops reading, so the Session sees its pipe fill; other channels keep their turns |
| A Session pipe reads slowly         | The pipe writer blocks, no credit returns, and the hub stops sending on that channel only                                                                        |
| Exit                                | At most 2 seconds to write `bye` and 5 seconds for routed commands to end; a Session start runs to its own 30-second deadline, never abandoned halfway           |

Ending a channel never stops a Session, its Server, Worker, Host or terminals: the helper only
closes its own pipe connection. The exercise in `tests/cli_windows_machine_link.rs` starts the
helper with pipes against a real scratch Session, two Session streams, a routed `workspace list`
while another stream is stalled at its window, both `ensure-session` outcomes, `bye`, and pipe
closure, and the console refusal. It does not start `ssh`.

## Machine hub

`orchbee --machine-hub-v1`, with no other argument, is the hub: the one process per data root
through which interfaces reach saved machines over
[`machine-hub`](../../../schemas/machine/README.md#hub-records). It serves the catalog and its own
lifetime today; it starts no `ssh` and contacts no machine yet. Windows serves it; elsewhere both
hub entries answer unsupported before they touch the data root. The entry is an internal pairing
between this program and its own start path, not a command: nothing outside one installation runs
it, and a release may rename it. `orchbee --machine-hub-state-v1`, equally internal, attaches to
the hub, starting it when none runs, and prints the hub's state, its process id, and each
Machine's state; it holds nothing. Neither appears in help, completion, or the Agent guide.

### Names that freeze

| Name                | Spelling                                                                                                                                                                                                                                                                                         |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Lock                | `<data root>/internal/machines/hub.lock`                                                                                                                                                                                                                                                         |
| Windows endpoint    | `\\.\pipe\orchbee-<id>-hub`: `<id>` is the first 32 lowercase hexadecimal digits of the SHA-256 of `orchbee-windows-machine-hub-v1`, one NUL byte, and the UTF-16LE code units of the data root's canonical `\\?\` path with ASCII letters lowercased                                            |
| macOS, Linux socket | Not frozen until a hub serves it there; nothing binds it today. Computed as `<socket folder>/<id>-hub.sock` in the environment's `/tmp/<stem>-<uid>` (`/private/tmp/` on macOS), `<id>` as on Windows over `orchbee-posix-machine-hub-v1`, NUL, and the canonical path's bytes, at most 90 bytes |

The lock and the Windows endpoint freeze at the first public release. After an update an older hub
meets a newer interface, or the reverse, so they must find each other by these spellings alone; a
later release changes the connection through the `OBHS` generation, never through the names. The
macOS and Linux spelling freezes only when a hub first serves there: its binder must create and
hold the socket folder through `prepare_socket_root` and `hold_socket_root`, as Session endpoints
do, and whether the canonical path stays stable on macOS is unverified.

- **One data root, one hub.** The hub serves one data root's catalog and Sessions, so its lock
  lives in that root; a hub under another `ORCHBEE_HOME` never contends for it. `internal/` holds
  what people do not edit, and `internal/machines/` is the hub's own folder for what later slices
  add.
- **Named like a Session endpoint.** The `orchbee-` prefix in every environment, a hash of a
  canonical location so the length never depends on the home folder, a role suffix, and a domain
  tag in the hash so a hub id never equals a Session id. Environments have distinct default data
  roots and so distinct names, and the hub runs as the Client's `<stem>.exe`, so a development hub
  never carries a release name.
- **The data root alone names it.** The stem stays out of the Windows hash so that the endpoint and
  the lock always name the same hub. Two environments that share one data root through
  `ORCHBEE_HOME`, which the layout does not support, reach one hub, and its handshake refuses the
  other environment's clients with `environment-mismatch`; on macOS and Linux the second
  environment's hub cannot take the lock and its clients report the hub unavailable.
- **Holding is a handle, never a file.** The hub holds an operating-system lock on its open handle
  of `hub.lock`, which the system releases when the process ends however it ends. The file is never
  deleted, so a leftover file means nothing and every hub locks the same file.

### Lifetime

A holder is a connection attached with `presence` or a live local Session of the data root. The
hub's state follows the contract's [hub lifetime](../../../schemas/machine/README.md#hub-lifetime).

| State      | When                                                                                                           | Leaves to                                                                                                           |
| ---------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `held`     | A holder exists, or the last `presence` connection has left and the Session read it triggered has not finished | `expiring` when no `presence` connection is attached and a Session read finds no live Session; `closing` on `drain` |
| `expiring` | No holder; `expiresAt` is the moment the hub learned that, plus 600,000 ms                                     | `held` on a `presence` attachment or a read that finds a live Session; `closing` on `drain`                         |
| `expiring` | `expiresAt` is reached with no `presence` connection: the hub reads every Session once more                    | `held` when the read finds a live Session; `closing` when it finds none                                             |
| `closing`  | Entered once and never left                                                                                    | Exit                                                                                                                |

- **Start.** The hub takes the lock and binds its endpoint at once, then reads the catalog and
  every Session before it accepts a connection, so its first client sees `held` or `expiring`; a
  client that connects meanwhile waits in the pipe within its own deadline. With no live Session
  the expiry counts from that read.
- **Session reads.** A Session is live while its Server holds its lifetime lock. The hub reads every
  Session that `session list` shows in the environment the hub started in: once when the last
  `presence` connection closes or attaches again without `presence`, then every 30 seconds while
  none is attached, once more at `expiresAt` before the hub may close, and never while one is
  attached. One read covers every Session within 5 seconds; a read that fails or runs out of time
  finds no live Session. The read at `expiresAt` sees a Session started without an interface since
  the last periodic read, so the hub never closes under it.
- **A read never meets a Server.** The hub skips a Session the catalog marks `deleting`: its
  deleter holds both locks, so no Server lives there, and the hub's handles, opened without delete
  sharing, could fail the folder removal midway. For each other Session folder the hub opens its
  existing `activation.lock` and never creates it: no file means no Session; a lock someone holds
  means an activation, a stop, or a deletion is running, which counts as live; otherwise the hub
  holds the activation lock for the instant of `observe_session_lifetime_lock`. A Server takes its
  lifetime lock once, at start, while its activator holds the activation lock, and releases it
  during its stop under the same lock, so the read can never take a free lifetime lock at the
  moment a Server takes it. The bare `observe_session_lifetime_lock` that `session list` uses
  could.
- **Time.** The lifetime state machine takes the time as an argument; only the hub's supervisor
  reads the clocks, monotonic for deadlines and the wall clock for `expiresAt`. The Windows
  monotonic clock counts time asleep, so after a long sleep the deadline has passed at the first
  wake; the read at `expiresAt` then decides, so a Session that lives across the sleep holds the
  hub. In development and test builds `ORCHBEE_MACHINE_HUB_EXPIRY_MS`, from 1,000 to 600,000,
  replaces the 600,000 ms expiry; a release build never reads it. The hub reads it from the
  environment it inherits from the process that started it; nothing in Orchbee sets it.
- **Events.** Every change of state goes as `hub` to every connection attached with `watch`.

In `closing` the hub rejects offers with `stopping` and still answers control requests, `query`
with state `stopping`; it answers every request `stopping` and resets every `open` `stopping`. No
channel stays open in this slice, so the 60-second allowance waits only for catalog reads already
in flight, which then reply `stopping`. The hub then queues `bye` `stopping` on every connection,
gives its writers at most 2 seconds, closes the endpoint, releases the lock, and exits 0. `drain`
enters `closing`; `exit` replies and ends the process at once, without the allowance.

### Admission and requests

The endpoint admits only the current user: its DACL grants access to the hub's own token user
alone, it refuses remote clients, and every accepted connection proves that the client process's
token user equals the hub's before a byte is read, as [`local-ipc`](../../../crates/local-ipc/README.md)
does for Session endpoints. The `OBHS` frame must arrive within 2 seconds of the accept.

| Request                        | Answer while the hub cannot connect                                                                                                                              |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `attach`                       | `attached` with the state; turning `watch` on sends `catalog`, then one `machine` per id                                                                         |
| `reload`                       | Rereads `machines.json`: `reloaded`, or `invalid-catalog` or `storage` keeping the catalog; a change sends `catalog` and every changed `machine` to each watcher |
| `connect`                      | `not-found` for an id outside the catalog, `disabled-machine`, `unsupported-machine`; otherwise, and for a `draft`, `unsupported`                                |
| `disconnect`                   | `not-found` for an id outside the catalog, otherwise `unsupported`                                                                                               |
| `stop-session`                 | `not-found` for an id outside the catalog, otherwise `not-online`                                                                                                |
| `claim`, `dismiss`             | `ended`: the hub has no login episode                                                                                                                            |
| `open` of `session` or `route` | `reset` `not-found` for an id outside the catalog, otherwise `not-online`                                                                                        |

A disabled record reads `disabled`, a connection kind this build does not know `unsupported` with
cause `connection-kind`, and any other Machine `offline` without a cause or a next attempt. No
silent attempt runs until the hub can connect. A catalog that cannot be read when the hub starts
serves as empty with revision 0 until a `reload` succeeds.

### Ownership and bounds

| Resource                       | Owner                                                                  | Bound                                                                                      |
| ------------------------------ | ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `hub.lock`                     | The hub process, from before its endpoint exists until after it closes | One holder per data root                                                                   |
| Endpoint                       | The listener thread of the lock holder                                 | Bound only by the lock holder; a name it cannot take is retried for 2 seconds, then exit 1 |
| Candidate                      | Its admission thread                                                   | 2 seconds; 64 read at once, and the next gets `capacity` unread                            |
| Attached connection            | Its reader and writer threads; its protocol state is the hub state's   | 256 slots; the next offer gets `capacity`                                                  |
| Control request                | Its admission thread                                                   | 4 slots; the next gets `capacity`                                                          |
| Queued frames of a connection  | Its connection state machine                                           | Its reader stops reading while 256 wait; state events replace their own subject            |
| Catalog                        | The hub state                                                          | 64 Machines from a record of at most 128 KiB                                               |
| Expiry deadline, Session reads | The supervisor                                                         | One deadline; one read at a time, within 5 seconds                                         |
| Threads                        | The process                                                            | Listener and supervisor, one per candidate, two per attached connection                    |

### Ends and races

| Event                                         | Effect                                                                                                                                                                                                                                             |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The last `presence` connection leaves         | `held` while one Session read runs; `expiring` when it finds no live Session                                                                                                                                                                       |
| A Session ends                                | The next read, at most 30 seconds later, finds it; `expiring` when nothing else holds                                                                                                                                                              |
| The expiry fires while a request is in flight | Requests are answered under the hub state's lock, before or after the change; a `reload` reads its file outside the lock and, applied in `closing`, replies `stopping`, and the closing waits for it before `bye`                                  |
| A second hub starts                           | It cannot take the lock, writes and binds nothing, and exits 0; the client that started it attaches to the running hub                                                                                                                             |
| A client connects during `closing`            | Its offer is rejected `stopping`; attach-or-start waits and starts a new hub once the old one is gone                                                                                                                                              |
| The hub is killed                             | The system releases the lock and closes the pipe; the next attach finds no endpoint and starts a hub, which locks the same file                                                                                                                    |
| A hub thread panics                           | The hub reports the panic and aborts the whole process, which then ends as a killed hub does; no thread unwinds alone                                                                                                                              |
| The data root is unavailable                  | Attach-or-start fails before starting a hub, or the hub exits 1 when it cannot create `internal/machines/` or open the lock; a running hub keeps its catalog, answers `reload` `storage`, finds no Session, and expires unless `presence` holds it |

- **A panic ends the hub.** No hub thread may unwind while the others serve: a listener that
  unwound would leave the lock held with no endpoint, so every attach would fail, and a reader that
  unwound would leave its `presence` counted, so the hub would never expire. The hub's panic hook
  reports and aborts the process, which the design recovers from as from a kill.
- **Simultaneous starts.** Several clients that find no endpoint each start a hub; one takes the
  lock, the others exit before binding, and every client connects to the one that binds. The lock
  precedes the bind, and a bind never adopts an existing name.
- **Lock without endpoint.** A client that connects in the instant between another hub's lock and
  bind finds no endpoint and tries again; its own hub, if it started one, loses the lock and exits,
  and the client starts another only after a growing pause.
- **A pipe outliving its lock.** A killed hub's handles close in no fixed order, so its name can
  outlast its lock by a moment; the new holder retries its bind for 2 seconds. A clean exit closes
  the endpoint before it releases the lock.
- **Presence changing during a read.** A read's result applies only while no `presence` connection
  is attached and the hub is not closing; another leave during a read schedules the next read at
  once.
- **A read beside a Session start, stop, or deletion.** The activation lock excludes a Server's
  lifetime acquire and release. What remains is the hub's own instant on the activation lock: an
  activation or a stop that meets it retries 10 ms later, and a `session delete` that meets it
  before marking the Session `deleting` is refused `in-use` once. A read that lists the Session
  after the mark leaves its folder alone; only a read that listed it just before the mark can pin
  the folder for a moment, exactly as a concurrent `session list` can. A Server still starting
  when its activator gave up at its deadline takes its lifetime lock without the activation lock;
  a read at that instant is the one meeting left open.
- **Admission beside closing.** A candidate's slot and decision are taken together under the hub
  state's lock, so an accepted connection is registered before `closing` can begin and receives its
  `bye`, and an offer decided after it is rejected `stopping`.
- **Expiry beside attachment.** The read at `expiresAt` runs outside the hub state's lock and
  applies under it, as every read does: an attachment with `presence` before it applies returns the
  hub to `held` and the read changes nothing; one after `closing` is answered `stopping`.

### Accepted limits

- **A Session read that keeps failing finds no Session.** A data root with 256 or more Session
  names, or a runtime base the hub cannot read, fails every read, so the hub, and any Machine login
  it holds once it can connect, can expire under a live Session.
- **A Server that outlives its activator's deadline** takes its lifetime lock without the
  activation lock, so a read at that instant is the one meeting with a Server left open.

### Attach or start

A client attaches by one function: it resolves the data root, creates `internal/machines/`
privately, names the endpoint, and within 10 seconds connects and offers `machine-hub`. No
endpoint means no hub: it starts its own executable with `--machine-hub-v1` detached, as a Session
Server is started, with no inherited handle, the system folder as working directory, the inherited
environment, and a new process group. A hub it started that exits 0 lost the lock to another hub:
the call connects again and starts one more only after a pause of 200 ms that doubles with each
loss up to 1.6 seconds, at most eight times per call. `stopping` means a hub is closing: it tries
again 20 ms later. `capacity` is unavailable; `environment-mismatch` and any other rejection are
incompatible. A hub it started that exits 1 ends the call at once. The hub belongs to no Session's
Server and outlives its starter.

The exercise in `tests/cli_windows_machine_hub.rs` starts eight attachments at once on a fresh
data root, follows `held`, `expiring` and `closing` with a 20-second expiry, keeps a hub `held`
by a real scratch Session through its stop and a new start, recovers from a killed hub, reads a
catalog of two machines, and reloads it. It starts no `ssh`.

## Persistent TUI connections

The Windows TUI discovers the saved catalog while open and offers a machine
picker, opened with `Ctrl+b Shift+M`. There is no permanent footer indicator. Selection is by
profile identity, not row index or label. Local remains
available. A disabled, removed, ambiguous, or corrupt entry cannot silently
become another machine. Enabled profiles connect automatically for read-only discovery; only an explicit
selection opens a terminal surface. Saved enabled state is never online evidence.
The current surface stays usable until the candidate reaches a drawable state;
cancellation or failure keeps the current surface. Successful switching preserves
per-machine views and invalidates pointer/input state before accepting new input.
Local is retained; at most four inactive remote surfaces are cached. Eviction
releases the Client connection, leaving the remote Session running. A Client the picker
stops presenting — evicted, superseded or replaced by a fresh connection, removed with its
profile, or a cancelled or failed candidate — is asked to stop at once and stays owned until
its background threads end. Process exit observes those threads under the same deadline as
every other Client owner and reports any still running as unfinished cleanup. Cached inactive
surfaces withdraw their Client-control, notification and outer-title subscriptions;
returning reopens them and transfers the host focus observation. A hidden surface
must not advertise a Ready control destination or acknowledge visible drawing.

The shared connection workflow admits the saved-machine capability before inspecting the
profile or cancellation source. Unix refuses at that boundary without environment or process
access. Windows retains enabled-profile checking, one bounded Client probe under the
[SSH admission rule](#remote-preparation), and the cancellation check before passing the admitted
remote executable to the native carrier port for executable lookup and relay creation. The shared workflow immediately adopts the real stop flag and relay task into
the Client's retained carrier guard, then performs native admission, the final cancellation check
and guard attachment under the original deadline. It never repeats the probe; admission or
cancellation failure stops and joins the retained relay.

The persistent transport owner creates a private, same-user local pipe endpoint
and a bounded SSH byte relay for each physical native Client connection. Logical lanes
share that admitted carrier. SSH authenticates the remote account; the exact admitted remote
executable runs the private helper `--machine-tunnel-v1 <session>`, whose grammar never changes
(a change adds a new helper name), and opens only the selected existing Session's same-user pipe
in that installation's environment. The tunnel carries an ordinary Client connection: the local
Client sends this build's own `OBHS` offer, and the remote Session admits or refuses it exactly as
it would a local Client, so machines with different product versions connect whenever their
generations overlap. The helper judges no peer, and neither end fabricates an accepted generation.
Remote terminal input, layout, metadata and control keep their existing semantic owners.

A refusal that needs the user to change one side is a typed `Refusal` that discovery keeps with
the failed endpoint, so selection names it instead of the generic failure, and automatic retry
never repeats it:

| Cause                                                                        | Picker message                                                     | Sidebar notice                        |
| ---------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------- |
| Probe: remote of another environment kind; handshake: `environment-mismatch` | Another environment; use release on both or the same checkout path | Remote is another environment         |
| Probe: unsupported platform or unavailable installation                      | Needs Windows x64 or ARM64 and a usable Orchbee                    | Remote has no usable Orchbee          |
| Handshake: `initiator-too-old`                                               | This Orchbee is too old; update it on this computer                | Update Orchbee on this computer       |
| Handshake: `initiator-too-new`, or a Session before the handshake            | The remote Session is too old; update it and stop it               | Remote Session is too old             |
| Handshake: `fingerprint-mismatch`                                            | Another development build; stop the remote Session                 | Remote runs another development build |

Capacity, stopping, and timeout refusals stay transient and retry with backoff.

A concrete transport guard retained by Client origins owns the relay stop flag and join handle;
the Windows adapter creates the listener, relay task and SSH Jobs. Derived lanes and reconnects
retain the same guard without depending on saved-machine profiles. The actor's retained origin
does not hold the connection actor itself, avoiding a reference/join cycle. Closing a logical
lane releases its scope; connection shutdown signals actor stop and joins the task once.
Each Workbench Client surface holds one claim on its carrier, and only the last released claim
performs that shutdown.
The observer promotes its initial admission before cloning any retained or published origin;
secondary query/control lanes are logical scopes on that carrier. Buffers are bounded and
backpressure pauses reads. The connection actor owns replacement admission after EOF and never
replays submitted frames. Final retirement cancels and joins relay work and settles children.
The remote stdio adapter likewise owns and settles its binary reader/writer Jobs. Disconnecting
a Client does not stop the remote Session or terminal processes. No port forwarding, host-key
acceptance, credentials storage, remote installation or POSIX implementation is added.

The picker owns keyboard and mouse input while open. Escape cancels; Enter and
click choose an exact painted identity. Resizing or catalog replacement invalidates
painted rows; pasted text and the remainder of a closing input packet cannot leak
into the underlying terminal. Each asynchronous attempt has an identity and a
cancellation owner; stale completion cannot switch the active machine.

The picker header names its current Machines, Workspaces or Agents mode. While
open, it owns the floating status and displays that mode with direct navigation,
Enter, Tab and Escape hints, replacing pane-prefix hints. Agent search editing names
Search there and states that Escape finishes editing rather than closing the
picker. Workspace search instead keeps Enter Open and Escape Close. Short widths drop the full hint before the short exit hint, then retain
the Escape key; all choices use display width and keep each segment whole.
If the mode label and key cannot fit together, the key remains alone. Closing
restores the covered terminal cells. A closed picker paints no footer or pointer region.
At short heights that still admit a picker surface, optional mode controls and Agent filter summaries yield before
the current list row, or the existing search field while editing. The two footer
rows remain visible; hidden controls publish no pointer regions. Tab still
switches modes and search completion retains its existing input semantics.
TooSmall paints only the root size notice and picker status. It retains the picker model and
input capture, publishes no hidden pointer targets, and preserves the existing Escape route.
A restored presentable frame resumes the same picker state.

Evidence must include binary full-duplex/backpressure/EOF/cancellation, same-user
pipe admission through a real helper process, picker identity and stale-result
traces, and rendered keyboard/mouse paths. Local fixtures do not establish real
SSH network survival, remote installation, or complete two-machine acceptance.

## Automatic machine discovery

The running Windows TUI reconciles enabled profiles by ID, connection and Session; a connection
kind this build does not know is never observed, and the picker lists that machine as unsupported
by this version without offering it as a choice. Each
endpoint generation owns one observer, cancellation flag and latest-event slot.
New connections are limited to four at once; existing online observers do not
consume that startup budget. A rotating cursor gives later profiles a turn even when earlier endpoints keep failing. Disabled/removed/replaced entries cancel their old
observers, and retired threads are joined before their slots are reused. Corrupt
catalog input suspends all discovery without changing the current terminal surface.
Renaming preserves connection identity. No observer creates a terminal, claims
control, changes focus, starts a Session or installs software.

An observer uses one Workbench state subscription on the admitted persistent tunnel. It accepts
the initial correlated snapshot and subsequent pushed values; it does not reread every two seconds.
The bounded summary retains Session/revision, project/workspace/Tab counts, at most eight preview
labels of 80 scalars each, and the context identities and labels used by workspace navigation.
A successful snapshot means Online only while the shared connection's application-progress deadline
remains current. A saved profile, running SSH process or SSH keepalive alone is insufficient.
The [carrier contract](../../orchbee-server/README.md#multiplexed-client-delivery) owns the single
five-second-idle/two-second-total read probe and its resource and correlation bounds. Transport
privately generates a validated nonzero 32-digit lowercase hexadecimal TerminalId using fallible
OS entropy solely for this read-only TerminalLaunchRead; it allocates no machine profile or Session
identity. Probe failure withdraws freshness without proving that the Server has died. An expired
progress deadline is Stale and cannot supply a reusable connection. Gap/Disconnected withdraw the
summary and retain the same observer while the actor rebuilds its subscription; only a fresh
snapshot restores Online.

Workspace navigation and its count use canonical ordered Workspaces, including empty Workspaces.
Local sidebar checkpoints and remote summaries preserve those same Workspace identities.

Transitions are Waiting -> Connecting -> Online, or Connecting/Online ->
Reconnecting/Attention. Bootstrap transport failure retries after 500 milliseconds,
doubling up to 120 seconds; 60 seconds continuously online resets that history.
Authentication, host-trust, unsupported/malformed protocol, absent installation
denied snapshots and typed terminal carrier rejections enter Attention without automatic retries. Explicit selection
can request another attempt. Catalog generations fence all results, including a
removed/re-added ID; a one-slot mailbox coalesces observations without an unbounded
history. An admitted observer retains its transport through failure delivery; the UI
clears its cached origin and both collectors retire their cached sources/subscriptions before acknowledging retirement. The observer clears its
mailbox and releases its last transport guard on its own thread. Profile limits bound observers; shutdown cancels every worker before joins.

The centered machine picker is a capturing overlay. Its mode controls are
single-choice rows with a checked track; Agent commands use the shared action
grammar. List rows retain separate current, provisional-navigation, availability,
operation and localized-status tracks, so hover or press feedback does not replace
any persistent state. The footer formats its close, movement, acceptance and mode
keys from typed bindings under the active glyph policy. Remote labels are data and
cannot supply terminal control bytes.

Every close, mode, Agent-control and row target is recorded only in the candidate
frame's central hit map. The target carries the picker episode, mode, effective
Agent evaluation basis and ordered stable selections; the list scroll target also
carries the exact painted window. Only a successful terminal draw publishes that
map. Release and each bounded wheel step revalidate the source against live state;
a changed episode, query, filter, order or catalog fails closed, while copy, status,
theme and pointer-only refreshes do not change semantic authority. List wheel moves
only the viewport and never changes the selection; the next key returns the viewport
to the selection, and `Enter` accepts only a selection inside the painted viewport,
so an `Enter` after scrolling it away only reveals it. Explicit
selection waits at most 60 seconds for a fresh observer, reuses its admitted origin
on a logical lane of the same carrier, and commits only a drawable candidate. Selection
failure, expiry or Escape preserves the current surface. Escape cancels the
selection, while discovery of enabled profiles continues. Retry applies only to
observations, never submitted terminal input or Client mutations. The workspace
mode below provides combined navigation, and Agents mode adds physical Agent
navigation. Source-owned merged Agent views are described below. Remote
bootstrap/authentication and real-network longevity acceptance remain separate
obligations.

Evidence must exercise retry timing/stability, permanent refusals, concurrency,
profile replacement and corrupt catalogs, coalescing and cancellation, actual
read-only snapshot frames, stale-source refusal and rendered localized overview.
Local IPC fixtures do not establish remote SSH authentication or network recovery.

## Cross-machine workspace navigation

The machine picker has Machines, Workspaces and Agents modes, switched with Tab or their
painted mode controls. Every build withholds the sidebar Search entry and its key until search is
finished, while the Machines action keeps opening the whole picker; see
[withheld search](tui-shell-layout.md#withheld-search). When offered, the sidebar Search entry opens Workspaces with an empty, focused name query,
so existing Workspaces remain discoverable without any Agents. Typing filters Workspace names with
Unicode lowercase matching, including spaces and parentheses; printable `j`/`k` are text, while
Up/Down move the provisional result. Enter opens its exact target and Escape closes directly.
The shared line editor supports grapheme editing, cursor movement and atomic paste, with a visible
refusal beyond 128 scalars or for control-bearing text. Query changes preserve a matching selected
identity and invalidate old painted targets; filtering never changes the main sidebar. The query
field's position does not move when the number of matches changes. Workspaces combines Local's last accepted canonical
navigation with fresh enabled remote observations; rows carry machine identity,
Session ID and a stable Workspace/context ID pair.
Nullable Project membership is metadata, not part of selection identity. Labels
and row positions never route a selection. Duplicate names are allowed, so same-named Workspaces
show their names alone, as in the Sidebar, and a cut name ends in an ellipsis. Before it opens,
the selected result's details name its Project or Ungrouped and the tail of its observed
directory, which tells same-named results apart. Native protocol limits bound the list
to 1024 contexts per machine; project/workspace labels retain at most 80 safe
scalars each. Display labels use the shared terminal-chrome sanitizer to remove complete
escape sequences and forbidden control/format scalars, then stop at a whole grapheme within
that retention bound; painting still measures display-cell width. Shared immutable context
arrays avoid copying the list per UI tick.
Local retained navigation is a candidate list, not proof of remote liveness.
The overview's online counts describe remote observers. Offline/stale remote
contexts are not selectable. Renaming, regrouping or reordering preserves selection identity;
removal invalidates painted hits and cannot retarget a pending selection.

Context selection inside an explicitly opened picker uses a separately admitted native lane, including
when the destination machine is already active or cached. The candidate reads a
fresh snapshot and verifies the exact Session and context before any attachment.
This path never creates a terminal as a navigation side effect. A vanished/replaced target fails without selecting a fallback. An empty
context bootstraps Empty membership with no viewer or terminal; normal create
actions remain available after switching. The mailbox allocates a fresh generation
for empty and populated bootstrap alike. Startup reads canonical Workspace membership. Every machine candidate supplies an explicit
startup deadline; remote observation deadlines never become terminal startup
authority implicitly.

Workspace activation from the sidebar never opens a picker. A selection episode
has its own generation and origin; connection progress and cancellation do not
depend on modal visibility. Activating a context on the current machine uses its
existing Session's [context selection](native-context-selection.md), preserving
per-location Tab/Panel memory and retained execution. While that Session is busy,
one bounded slot retains only the latest exact target, including a return to the
still-displayed source. Once accepted work settles, that last intent is admitted;
an idle exact-current activation has no effect. The waiting slot expires after
60 seconds. Escape or explicitly opening a picker discards unadmitted sidebar
work; an already accepted Session transition retains its existing settlement
contract. A direct failure uses a localized status notice, never an implicit picker.

Cross-machine sidebar activation uses the candidate handoff below with the picker
closed. A newer selection or cancellation invalidates the old episode before any
late completion can install a surface. Explicit picker selection keeps its
existing input capture and Escape behavior.

Queued selection freezes endpoint and destination identity. A candidate replaces
the current surface only after the selected context is drawable; failure, timeout
and Escape keep the current surface, with no mutation replay. Local is retained
when switching away; replacing the same machine retires its previous surface
only after commit. A candidate derived from the current origin is a lane on the same
carrier with its own claim, so retiring the previous surface closes only that surface's
lanes and claim; the carrier and its Client identity continue under the installed surface.
Cached machine-only selection preserves its selected context, including empty workspaces
and unbound placeholders.

A targeted fresh lane for the same exact endpoint may inherit the bounded,
render-only text-image bank from that endpoint's active or cached surface before
the old surface is released. The candidate's fresh canonical Snapshot prunes the
bank by exact Session, context, Tab, Panel, Presentation and worker/terminal binding;
its own newer frame wins an identity collision. Geometry and any known HostEpoch
remain renderer gates. The handoff carries no attachment, selection, input,
controller, feedback or retained-context authority, and never crosses an endpoint
replacement.

A successful switch invalidates host input and painted regions through the
existing machine handoff. The Agent mode described below shares this handoff;
the main sidebar shares the Agent projection described below; Agent cycle
bindings are described below.

Evidence includes native snapshot-first exact bootstrap, absent/mismatched
Session/context refusal before effects, empty-context membership, actual machine
candidate startup and cancellation, duplicate labels and local/remote identity
separation, same-endpoint active/cached pixel handoff and replacement rejection,
a running source's shared carrier still serving the installed surface,
reordered/stale hits, direct closed-picker candidate admission, latest-intent and
cancel/deadline traces, same-Session round trips, mode keyboard/mouse and narrow rendering. Local
protocol peers do not establish real SSH or real-host visual acceptance.

## Cross-machine Agent navigation

Agents mode combines on-demand physical Agent observations from Local and fresh,
enabled remote machines. Each complete read joins a native Workbench snapshot to
bounded physical state batches at one Workbench/report revision. Missing,
unavailable or stale observations are distinct from a successful empty list.
Only observations with a reported Agent label become rows; primary and child
Presentations retain their own identity. Labels are sanitized display data.
Rows carry machine, Session, context, Tab, Panel, Presentation and complete physical
binding; no name or row number grants navigation authority.

At most four observation jobs run concurrently. Each job owns a logical lane, a five-second
budget and one result slot; reads admit at most 4096 bound Presentations and 8 MiB of serialized
physical, report and metadata rows. Each source registers one shared-carrier invalidation stream
before its first read and waits for an accepted initial snapshot. A commit during a read schedules
another complete read. Gap/Disconnected retire even a completed old job before consuming results
and wait for a fresh subscription snapshot; a canonical source withdraws its observation there, and
coalesced reset/snapshot delivery still preserves retirement. Source identity includes the carrier,
not just the saved choice. Registration or read failure retries only while failed, starting at 500
milliseconds and doubling to at most 30 seconds; successful canonical checkpoints have no periodic
fallback. Agent Host process titles and requested external Git state have no notification source,
so those observations retain a two-second sampling cooldown and ten-second age from job completion.
A read is an optimistic multi-request round, so a commit landing inside one fails it without saying
anything about the facts: a sampling source keeps presenting its last complete checkpoint through
failed reads, Gap/Disconnected and carrier loss until that age passes. Workspaces answer a commit
with a canonical read alone; their Git read runs only when its cooldown ends or after a fresh
subscription snapshot. One Client's requests run in order on the carrier its terminals share, so
that cooldown stretches to three times a slow sampled read, capped at four seconds so that the next
read completes within the age. Completed reads
signal the UI wakeup; invalidations are consumed in the existing UI tick with at most the existing
50-millisecond input wait, without an extra forwarding thread. Attached-machine hints also expedite
Agent attention and Panel report reads. Hiding the last consumer cancels jobs and subscriptions;
removed/disabled/replaced sources discard their results. Only explicit activation opens a terminal.
The candidate revalidates the selected snapshot and physical checkpoint, starts
at the exact member, and checks the returned Host identity before taking control.
No terminal creation or unrelated attachment precedes that selection. Old
surfaces survive failure, cancellation and target replacement; commit still
requires drawable content. There is no replay of terminal input or mutations.

Evidence must cover bounded complete reads, revision/membership/physical identity
mismatch, primary and child placement, stale job cancellation and fairness,
keyboard/mouse identity, exact bootstrap and Host mismatch before control.
Live terminal-title tokens and real SSH/OS visual acceptance remain
separate obligations. The native canonical Agent list and per-Session Agent board retain unattached canonical
sessions. Cross-machine aggregation of those sessions remains a separate capability.

## Agent metadata, view controls and navigation

The Agent list joins physical-state and Agent-report facts with independently
owned metadata only across overlapping stable reads: every batch has the same Workbench revision,
exact member/parent/terminal binding, and one revision for each read owner. Physical
and Agent-report reads share the report revision; metadata has its separate revision. After
all initial batches, metadata, report and physical fences confirm those versions.
Failure, missing members or a changed revision refuses the whole observation.
All reads share the existing deadline, row and serialized-state byte bounds.
Agent and authority facts guard effective metadata; display titles/providers/state
labels and tokens never replace the physical navigation target.

The Client owns ephemeral status/scope/order/search controls. `f` cycles status (including Done),
`c` cycles all/current machine/current context, `s` cycles placement/priority/provider/
title/state sequence, and slash edits a query of at most 128 safe scalars. Enter
or Escape first leaves query editing. The query takes the cursor keys and shared readline keys of
the [native line editor](native-line-editor.md), except `Ctrl+y`, which keeps copying the selected
row's ID. Painted controls accept
mouse activation. Filtering includes safe machine/location/title/provider/token
text. Stable ordering preserves canonical placement for ties; priority orders
Blocked, Error, Done, Working, Idle, Unknown. State sequence is a reported version, not a
cross-machine wall clock. Missing observations stay distinct from no matches.
Changing data, context or controls invalidates painted activation identities.

`Ctrl+b {` or `Ctrl+b }` requests previous/next Agent across machines; the configured
`previous_agent`/`next_agent` actions use the same projector. An eight-second intent
waits for bounded observations, wraps relative to the exact current member, chooses
last/first if it is absent, and excludes unavailable rows. Empty lists and a single
already-focused target cause no attachment. Further keys or pointer input cancel a
pending intent. An Agent on the current machine activates like a sidebar row: revalidated
on a lane of the current Session, then reached by that Session's own navigation, with no
new connection and no picker left on screen. An Agent on another machine uses the
separately admitted, revalidated, drawable candidate. Both preserve the previous surface
on failure.

Evidence covers guarded metadata, overlapping revision fences and binding faults,
stable ties and machine-qualified context/identity, query bounds and control
cancellation, painted mouse/keyboard/narrow/localized rendering and global action
input isolation. Live terminal-title tokens and real-network/OS visual acceptance remain separate obligations.

## Source-owned Agent views

`agent view set --spec <json>`, `agent view clear [--source <name>]` and
`agent view get` operate on the selected Windows Session. The Server owns one
transient revisioned specification: the latest valid set replaces it; a clear
without a source removes it, and a source-qualified clear removes only an exact
match. Restart removes the override. Reads require read permission; changes
require navigation management. Invalid, denied or unavailable operations preserve
the previous specification. No mutation is automatically retried after handoff;
lost mutation replies report an unknown outcome. Read or pre-submission exchange
failures have a distinct `exchange-failed` receipt and never claim a Server refusal
or uncertain mutation. Plugin sources use `plugin:<id>` and
require an installed enabled plugin; the next read clears a view whose
plugin has been disabled or removed. Other source names are attribution, not an
authentication credential.

The specification has `source`, optional `label`, optional `filter`, and optional
`sort`. Source is trimmed, 1–120 ASCII alphanumeric or `:._-` characters; labels
are trimmed and controls removed, then require 1–32 scalars. A 64 KiB encoded
specification limit also bounds string operands. Filters use `all`, `any`, `not`,
`eq`, `in`, or `exists` in the `op` field; nonempty groups, at most eight levels,
64 nodes and 32 `in` values are allowed. Fields are `status`, `workspace_id`,
`tab_id`, `panel_id`, `agent`, `seen`, `state_change_seq`, or `{ "token": "name" }`.
Values are strings, booleans for `seen`, full unsigned 64-bit integers for sequence,
or `{ "context": "current_workspace_id" }` / `current_tab_id` for matching ID
fields. Status admits idle, working, blocked, done, error and unknown. A row whose Agent is
opening has status `opening`, which no filter value names; it ranks with working.

Up to eight sort entries contain `field` and optional `order` (`asc` by default,
or `desc`). Fields are workspace_order, tab_order, panel_order, attention, status,
agent, seen, state_change_seq, or a token object. Token names are 1–32 ASCII
alphanumeric, underscore or hyphen characters. Values compare without display
sanitation; missing values sort last in either direction, and complete ties
preserve canonical placement. Attention ranks Blocked, Error, Done, Working, Idle,
Unknown. Sequence is an observation version, not a shared wall clock.

The active machine's stable source specification filters the merged list and
global Agent cycle; other machines' overrides do not stack. ID operands and
current-context values are qualified by that active machine. Workspace IDs are
native workspace identities (required for every terminal), Tab IDs are native
Tab identities, and pane IDs are exact Presentation identities, including child
Presentations. Placement order comes from the snapshot's ordered workspace, Tab
and pane placement. A missing context operand compares as missing, and two missing
operands compare as equal. UI presets/search further narrow the
source filter; explicit source sorting wins and the sort control explains its owner, otherwise the UI ordering applies.
The source specification is read before and fenced after physical checkpoints;
a changed or unavailable source never silently means an empty override.

Seen belongs to this Client. The first complete endpoint checkpoint is the seen
baseline; new/changed binding versions are unseen. Status always describes the
reported Agent fact, independently of seen. Existing native attention supplies exact binding/version acknowledgements
from visible terminal drawing or explicit focus. Listing, filtering and ordinary
read requests cannot acknowledge terminals. Endpoint replacement/restart and
physical rebinding cannot inherit another identity's acknowledgement.

Projection reuses unchanged source checkpoints and reconciles each attention
batch once. Private paint-version stamps are scoped to the current machine and
invalidated on Client handoff; a new Client cannot inherit an identical counter.
Hiding the last Agent read consumer releases cached rows/specifications; bounded binding-version
acknowledgements remain Client-owned, and removed/disabled sources are pruned.

Evidence covers specification bounds/type rejection, source-guarded replacement
and clear, plugin eligibility, revision and transport uncertainty, endpoint-scoped
context, missing/tied multikey sorting, complete view fences and exact attention
versions. Live terminal-title tokens and real SSH/OS
visual acceptance remain separate obligations.

## Ordinal Agent focus

Windows `[keys] focus_agent` accepts configured `1..9` digits or ranges, with
prefix/direct modifiers and explicit disabling. It is unbound by default. A
matched binding digit selects the corresponding zero-based entry in the same
active-source-filtered, stable merged list used by previous/next actions. The
first nine eligible Agent rows display those numbers; loading/unavailable/empty
placeholders never take an ordinal. Configured shortcut help lists the actual
triggers in both locales, including disabled bindings. Shifted digit symbols from
legacy terminals resolve the binding digit; an exact configured symbol binding
takes precedence over that compatibility path.

The existing eight-second intent waits for loading observations. Unknown active
sources remain explicit refusals; absent ordinals neither wrap nor clamp, and a
target already focused causes no attachment. Further input cancels the pending
intent. Once resolved, the selection freezes exact endpoint/member identity and
activates through the same current-Session or candidate path as cycle navigation.
No terminal acknowledgement follows from choosing or reading an ordinal.

Evidence covers key/profile/conflict admission, complete-modifier dispatch and
packet-tail capture, overlay priority, source-filtered numbering, loading,
absent/current targets, cancellation and exact cross-machine identity.

## Main Agent sidebar

On Windows, the main sidebar joins the Agents picker's eligible projection to
each exact Machine/Session/Context and renders Agents beneath their owning Workspace,
including the nonempty legacy Workspace. Labels never establish ownership. The active machine's source
view and Client-local status, scope, ordering and search still define eligibility.
The first nine eligible entries retain their global filtered-list ordinals for
ordinal focus even when other parents are collapsed; the sidebar does not paint them.
The focused physical member is marked.
[Sidebar navigation](tui-shell-layout.md#sidebar-navigation-model) owns disclosure
and the shared tree window. Search opens Workspace-name filtering; its Agents mode retains
keyboard navigation, filters, Agent search, source status and complete details. The configured
machine-picker binding followed by Tab twice also reaches Agents. Loading, unavailable, empty and no-match placeholders remain in the picker;
they never become nested Agent rows or navigation targets.
Sidebar rows are navigation entries: a left press
freezes their painted endpoint/member/physical identity, re-resolves eligibility,
and starts the existing revalidated drawable candidate. A current target is a
no-op. Changed, removed, disabled or unavailable identities never redirect to a
replacement row. Other buttons, modifiers, releases and drags cannot activate.

The sidebar uses the shell's bounded Full/Compact width and disappears at Minimal
or too-small geometry. Default Agent entries occupy one line; configured entries
use their resolved heights and gaps. Whole entries fit when possible; an oversized
entry is clipped to the actual body, with grapheme-safe horizontal truncation.
The shared footer reports the visible tree range and opens the Workspace picker.
The window follows keyboard focus or the exact visible current Agent, falling
back to its Context; explicit wheel scrolling holds the window until focus or
filters change. Wheel bursts queue three entries per notch,
coalesce at the input cadence, and apply at most one painted viewport per pass;
reversal cancels the old remainder. Overlay/modal ownership and released capture
continue to precede sidebar gestures. Successful drawing publishes the same
regions used to paint; resize invalidates them.

A visible sidebar or open Workspace or Agents picker mode is a read consumer. Their shared collector
uses the existing endpoint, freshness, deadline, row/byte and cancellation bounds;
closing only the picker leaves a visible sidebar current, while hiding the last
consumer releases source rows/specifications and cancels reads. Sidebar rendering,
scrolling and opening the list do not acknowledge terminal versions. Geometric
sidebar overlays suppress terminal-draw acknowledgements even when short height
has undocked a Wide Client's sidebar. Live terminal-title tokens and real
SSH/emulator visual acceptance remain separate obligations.

## Agent sidebar configuration model

Windows users can select sidebar content through `appearance.sidebar.agents.rows`,
`rows_by_agent` and `row_gap`. The shared Windows settings owner admits the
bounded whole file and a canonical, closed JSON profile of at most 32 KiB.
The existing Session configuration owner publishes that profile alongside keys and
notification policy; only the Client UI owner installs a newer, completely
validated publication. Fatal source failures retain all previously installed settings. An invalid
configured sidebar profile uses its safe default, emits a warning and remains part of the complete
atomic candidate; a malformed private wire profile is rejected before Client installation. No
renderer reads configuration files or starts background work.

Rows contain `state_icon`, `state_text`, `machine`, `workspace`, `tab`, `panel`,
`agent`, `terminal_title`, `terminal_title_stripped`, or `$name`. Missing values and tokens hidden by rules disappear; rows
with no remaining tokens disappear. An entry still retains its focus marker,
exact target and navigation eligibility. Custom values and custom state
labels are read from the already-fenced presentation metadata. Text rules match
the original value; only painting sanitizes controls and truncates graphemes.
`terminal_title` and `terminal_title_stripped` use the live physical title
observation below; pane title and `$terminal_title` never substitute for those
builtins.

Default rows show one line beside the vertical guide and status glyph: what names the Agent,
then its provider in secondary text, or the provider alone, as
[Sidebar display identity](native-agent-navigation.md#sidebar-display-identity) defines.
Parent Machine/Workspace text stays in the tree rather than each Agent. Status color stays on the
glyph; the name uses the primary text role. A pointer resting on the glyph names its state in a
tooltip; configured `state_text` and `agent`
tokens restore inline state and provider text.

| Input or event                                 | Decision and observable result                                                                                          |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| No rows setting or matching override           | Use the single-line title default                                                                                       |
| Provider override                              | Exact physical Agent label first, then its canonical report label; displayAgent cannot select an override               |
| Empty rows, missing custom value, or hide rule | Remove configured text, preserving at least the identity line and exact hit target                                      |
| Styled text token                              | Apply base foreground/bold/dim, then the first matching rule; `hide=true` removes that token                            |
| Text rule                                      | Exactly one of equals/contains/starts_with; optional ASCII case folding                                                 |
| Numeric rule                                   | Exactly one finite gt/lt threshold; strict finite number parsing without trimming or case folding                       |
| Layout or publication changes                  | Reset follow-focus window and pending scroll; do not alter filters, seen state or physical identity                     |
| Variable-height viewport                       | Fit whole entries and inter-entry gaps; an oversized single entry is clipped to the actual body                         |
| Scroll                                         | Move by entries, at most the painted entry count per cadence; clamp against the final page computed from actual heights |
| Invalid configured sidebar profile             | Use the safe sidebar default, emit a warning and publish it with the complete candidate                                 |
| Fatal file/TOML or malformed wire profile      | Reject before publication or UI installation; retain previous keys, notification policy and sidebar                     |

At most 16 rows, 16 tokens per row, 16 rules per token and 64 provider overrides
are admitted. Custom keys use the metadata ASCII grammar; provider keys are
bounded native IDs. Foreground accepts `#rgb` or `#rrggbb`. The theme owner maps
explicit user foregrounds and modifiers; default presentation remains semantic
theme roles. Nonnegative `row_gap` fits u16 and is applied only between entries.
Empty trailing space and gaps are non-activating sidebar surfaces. Paint geometry
and input geometry share the same resolved window, including clipped entries.

Core evidence must cover strict admission and canonical profile roundtrips,
first-match/hide/case/numeric rules, raw-value matching, provider identity,
missing/empty content, final-page and focus windows, bounded/reversed wheel,
and exact hit targets. Adapter evidence must cover whole-publication retention,
real configuration IPC, and both locales/themes at short and full heights.
No new Agent authority, terminal acknowledgement, persistence migration, network
acceptance or emulator visual proof follows from configurable painting.

## Live terminal title observation model

An Agent sidebar user may select `terminal_title` or
`terminal_title_stripped` to follow the OSC title of the exact physical terminal.
The existing Host metadata event is the only title writer. The Windows Worker
keeps a connection-owned, bounded projection keyed by the complete Host identity
and metadata revision, independent of Agent report sequence, view filters and
Client seen state. Closing the Host connection drops that projection; confirmed
physical disposal removes its entry. Missing observations remain unavailable,
while an observed null title means a confirmed clear. Neither case substitutes a
pane label or a custom `$terminal_title` token.

A read workflow requests at most four exact physical bindings per batch on the
existing owned observation lane. The Session checks membership and retained Worker,
creation and Host generations before and after the Worker read. The Worker drains
at most 32 queued Host events per read, yielding to a continuously active stream,
without attaching a viewer, claiming input or
resize, reading a terminal screen, starting a Host, or acknowledging a draw.
Repeated exact physical identities across presentation aliases are read once
and projected back to each independently fenced binding.
Every reply echoes the operation and physical identities; changed bindings,
invalid revisions, malformed frames and exceeded bounds reject the candidate.
Titles are sequential observations rather than an atomic snapshot of all Host
terminals. The sidebar refreshes through the existing approximately two-second
observation cadence; raw glyphs are sampled, without promising every animation
frame. The collector retains its existing total deadline, cancellation, row,
byte and final membership/report/source-view fences.

The projection admits at most 1024 physical titles and 64 MiB of title text per
Host connection; each title follows the Host's 64 KiB metadata bound. A separate
bounded Worker reply frame carries four complete values, avoiding truncation of
raw values. Newer metadata replaces the prior value; identical replay is a no-op;
a decreasing revision, changed replay or reused terminal identity is invalid.
The Client paints raw and stripped forms from that same observation. Stripping
removes one Windows elevation prefix and at most one recognized leading activity
glyph followed by whitespace, preserving Unicode text. Rules inspect the selected
original form before display sanitization and grapheme truncation. Title-only
changes may alter row height and cancel pending old scroll geometry, but do not
change Agent attention or navigation identity.

Core evidence covers absence versus clear, monotonic revisions and replay,
identity replacement, bounded framing and exact reply correlation, stripping,
raw-value rules and custom-token separation. Adapter evidence covers actual
Windows metadata events and pipes, Session before/after physical fences, collector
cancellation and rendering. No new public CLI command, persisted title registry,
Host protocol, input authority, or independent terminal acknowledgement follows
from these presentation tokens.

Agent rows retain a fixed leading state symbol independently of configured text tokens. `state_icon` remains accepted for configuration compatibility but cannot hide, duplicate, move or recolor that fixed status slot. A trailing blue dot independently reports any unread Agent version; the existing Client acknowledgement owner clears it without rewriting state.

## Workspace sidebar model

The Windows main sidebar resolves workspace rows from exact Session/context identities,
not display labels or Agent source-view filtering. The settings owner admits
`appearance.sidebar.workspaces` rows and `row_gap`, with at most 16 rows and 16 tokens per row.
Builtins are `state_icon`, `state_text`, `workspace`, `branch`, `location` and `git_status`;
`$custom` values come from the Workspace report owner. Styles and first-match
text rules inspect original values before terminal sanitization. Icon and Git
count tokens do not admit text rules. Nonempty configured rows retain their slots when values are missing or hidden;
an empty configuration retains one identity row for navigation.

Workspace state aggregates complete physical Agent facts independently of seen, using the established attention/error/outcome/working/idle/unknown ordering: blocked, error, done, working, idle, unknown. `state_text` shows the most urgent state, and a folded Workspace's status line lists every Agent's state in that order. Without an observed Agent both stay empty; neither manufactures idle. A known Agent with insufficient state evidence is unknown. Source filters and custom display labels do not change this authority.
Git observations follow the Workspace's first terminal, the leading Panel primary
of its first Tab, on the selected Worker. Its directory is the latest OSC 7/9;9
report, otherwise the root process cwd, from the Server's existing periodic process
sampler. Without a sample the canonical Workspace directory applies. A `cd` into or
out of a checkout therefore reaches the sidebar without changing the canonical
directory. Shells that change location without reporting it, such as
PowerShell without a prompt that emits OSC 7 or 9;9, keep their launch directory.
Each Workspace reads separately because equal canonical directories may diverge.
`git_status` is ahead/behind relative to the local upstream; neither a
dirty-file count nor a zero invented for an unavailable repository. Detached
and unborn branches, absent upstream, and nonrepositories remain explicit; a
nonrepository is an observation without branch facts, so earlier facts are not retained.
`location` shows the observed branch, otherwise the name of the first terminal's recorded
directory, else of the canonical Workspace folder; the Client's home folder itself reads `~`, and
the full directory is a pointer tooltip on that name.
Reads never fetch, change configuration, start a terminal or execute a plugin.

The Git read reuses the owned Windows discovery slot: checked start, poll,
cancel, five-second deadline, retained receipt and child/pipe settlement.
Server discovery exchanges use nonblocking queue admission and absolute-deadline
waits. Late checked discovery replies may be discarded without retiring a healthy
carrier; effectful operations retain required completion delivery. A timed-out
start is never resubmitted. Its known operation/job identity is cancelled once
through the same FIFO high lane without waiting; if admission or transport fails,
the Worker still owns the physical job and its five-second TTL. The Server does
not report that cleanup has been confirmed.
Literal Git arguments use command-local repository selection without trust
expansion. Session membership, directory and Worker generation are checked before
and after I/O. Branch text is control-free UTF-8 capped at 1024 bytes;
ahead/behind counts are uint32. Metadata reads retain their own revision and an empty final
fence; topology is rechecked before admitting a complete candidate.

Client presentation retains its last accepted Workspace checkpoint while a refresh is loading,
unavailable, expired or temporarily missing. Project disclosure, current identity and the manual
scroll window remain stable. A newer complete checkpoint replaces canonical membership; decreasing
Workbench or metadata revisions from the same Session cannot overwrite it. A partial Git failure
retains only the last result for the exact Workspace/context, directory ID, path, terminal directory
and Worker. An observed null branch confirms absence and clears the old branch; a changed directory
or Session never inherits it. A complete round that asked for Git and got none makes the retained
fact stale; a commit, a metadata-only read and a read that asked about other Workspaces say nothing
about Git and leave its freshness unchanged. The Workspace picker reports Stale; the sidebar paints
no marker for it.
A directory change moves the row once: its name and `location` line change together, to their final
value, and neither shows another directory's facts. The Client keeps the Session's last Git answer
for up to 64 directories, so a return to a known directory paints with the commit. For an unknown
directory the row keeps the name, location and counts it last settled on, an automatic name committed
with the directory included, and asks one read of that Workspace's Git ahead of the sampling cooldown,
once per Workspace and directory; the answer replaces all it kept. The wait lasts at most one second,
which covers a read of about 200 ms and one retry after a commit interrupted it, and a further
directory change does not restart it. At the bound, or after a complete round without an answer, a
withdrawn checkpoint or a rename, the row shows its real name and folder, and a late answer corrects
the line. A view that shows nothing from Git never waits. A kept answer is a presentation hint that
the next sampled round corrects; it grants nothing.
Endpoint replacement/removal invalidates its retained checkpoint;
hiding the last Workspace consumer releases it. No retained fact grants navigation or effect authority.
`crates/workbench-client/src/machine_picker/tests/spaces.rs` covers failed/missing and out-of-order rounds, stable grouping,
scroll/current identity, partial Git failure, confirmed absence and source replacement, and
`crates/workbench-client/src/tui_render/workspace_sidebar/tests/directory_change.rs` the painted
sequence of a directory change. These are
controlled implementation traces, not exhaustive TLC assurance or real multi-Client host evidence.

Agent and workspace observations share the bounded job lifecycle, with separate
four-job pools (at most eight combined), five-second operation budgets,
cancellation and joined disposal. Pure metadata follows commit/expiry invalidations and the
carrier progress deadline; external Host/Git sampling retains its ten-second freshness. Rendering performs no
I/O. Workspaces and their expanded Agent children share one Client scroll window. Resolved row
heights and gaps determine focus reveal, the final page, wheel flushing and
exact hit rectangles; one wheel flush advances at most a viewport. Changed
identity, data or settings cancel old pending geometry. Activation rechecks the
exact machine/Session/context; labels cannot redirect it. The workspace picker
provides an additional keyboard navigation path. Small viewports retain that path
while reducing the visible tree. Text-token widths reserve fixed icon/Git-count widths;
long branch/custom values shrink before readable ahead/behind counts.

The Session publishes keys, notifications and both bounded canonical sidebar
profiles as one candidate. Fatal source failure or a malformed private publication retains every
prior Client setting; an invalid configured sidebar uses its safe default and warning in the atomic
candidate. The two 32 KiB sidebar profiles use a 256 KiB configuration frame,
including JSON escaping; field budgets remain separate. Matched cohorts update
together; public CLI documents retain only their current version-1 contract. Core and adapter evidence cover strict admission,
revision/generation replacement, cancellation, real Windows Git and pipes,
null/unknown distinctions, style/Unicode rendering and navigation geometry.
Whole-cohort and real emulator/SSH acceptance remain separate evidence.

## Search presentation and explicit identifiers

With no configured remote profile, search omits remote totals and Local prefixes. The selected mode uses the shared selection surface plus bold and underline; it does not add an asterisk. Internal IDs do not appear in rows or details. The selected row exposes Copy ID (Ctrl+y); pointer activation retains its exact catalog/index proof, and keyboard copying uses the current selection. The validated identifier enters the existing bounded post-draw clipboard queue once, with no navigation or remote write. A connected native pane supplies the delivery lifecycle; without one, copying reports unavailable. Switching or retiring that pane cancels an undelivered request. Remote totals count remote observations only.

The Agent picker shows the assigned name and a distinct task title when present. Its details reserve separate rows for workspace, provider, title and custom tokens, plus machine context when remote profiles exist, while keeping Copy ID available. The mode bar uses the shared current-state track so every mode keeps a distinct painted label and hit region.

The Agent picker's query includes the assigned runtime name even when no other field contains
that alias. It uses the same sanitized, case-insensitive substring matching as its existing
machine, Workspace, Panel label, provider, title, member and token fields; sorting and entry
points remain unchanged.
