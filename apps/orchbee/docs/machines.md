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
written back unchanged. That machine is listed as unsupported, the same word in every client,
cannot be routed to, chosen, or discovered, and leaves every other machine usable. The record never stores a
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

A command that commits an edit then asks a running [machine hub](#machine-hub) to `reload`,
through one attachment that holds and watches nothing, so the hub stops carrying a removed or
disabled record and carries a changed one as it now reads. It starts no hub and does nothing more
when none runs, and the edit stands whatever the hub answers; only an interrupt during that wait
changes the receipt, to `cancelled` with `committed` true.

| Event          | Guard and transition                                                                                         | Effects                                                                                                |
| -------------- | ------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ |
| List           | Missing catalog is empty; invalid data fails                                                                 | Read only; no directory creation                                                                       |
| Add            | Validate and check capacity; prepare remote; reload latest catalog under lock; append enabled `manual` entry | Remote Session may start before local commit                                                           |
| Rename         | Operand resolves; valid label                                                                                | Replace label; same value is unchanged                                                                 |
| Enable/disable | Operand resolves                                                                                             | Set flag; same value is unchanged                                                                      |
| Remove         | Operand resolves                                                                                             | Remove machine and forget its [remembered answers](#remembered-answers); never stop its remote Session |

Writers use the existing Windows private-cache lock and atomic replacement. Reads reject unknown
members outside `extensions`, a malformed connection, an unknown `remoteInstall`, invalid
identities, duplicate IDs, overflow, oversized files, and unsafe filesystem objects; a record
without `connection` or `remoteInstall` is invalid, with no reader for an earlier shape.
Corruption never becomes an empty catalog and its bytes are preserved. A failed local commit does
not roll back a remotely started Session. Cancellation after commit preserves the committed
receipt.

## Commands not available yet

`machine add --no-install` and `machine remove --stop-session` belong to later slices of
remote-machine support. Help and completion leave them out while they do nothing, but their
whole grammar is validated, so a malformed invocation is still a usage failure with exit 2. A
well-formed one is refused before it reads the catalog, contacts any machine, or changes anything:
the `orchbee.machine` receipt names the operation (`add` or `remove`), reports
`error: "not-yet-available"`, `remote: "not-contacted"`, `profileId: null`, and the command exits 4.

## Connect and disconnect

`machine connect <id|label>` and `machine disconnect <id|label>` resolve the operand in the
catalog, then ask the [machine hub](#machine-hub) through one attachment that holds nothing and
watches nothing. Neither writes the catalog, so the receipt reports `changed` and `committed`
false, with the resolved `profileId` and the catalog it was read from.

- **Connect** starts a hub when none runs, asks it to `reload` so a machine saved after the hub
  started is known to it, then sends `connect` with `origin` `command`. It returns when the reply
  arrives: once the Machine is `online`, with `remote: "ready"` and the remote product version, or
  with the reason it is not. The deadline is 90 seconds: a hub start, then the hub's own
  [60-second attempt](#carrying-a-machine). The connection stays after the command ends, carried
  by the hub until it is disconnected or the hub expires.
- **Asking on a terminal.** When standard input and standard error are both consoles, the command
  sends `answers` true and becomes the claimant of the [login episode](#login-episode), so the hub
  puts each question `ssh` asks to this terminal; otherwise it sends `answers` false and a sign-in
  that needs a person ends `auth-required` at once, as before. With a client that `asks` attached,
  such as a running TUI, and the Machine not [muted](#login-episode), the interface asks instead,
  as the design's commands that wait for a login do, and the command, with or without `answers`,
  writes `Waiting for login to <label>…` on standard error, in the person's locale, and waits for
  the hub's reply: the person logs in there, declines, or the hub's bound on the question passes,
  which ends it `auth-required`. The 90 seconds stand still for as long as that wait lasts, and a
  request sent again through a hub that replaced a closing one writes no second line. The line
  does not say where the person answers: another terminal's `machine connect` may hold the login
  as well as an interface, and the reply does not tell them apart. Each question goes on standard error in the person's locale: first
  `ssh`'s own prompt, every line marked `ssh:` and escaped, so control characters, escape
  sequences and bidirectional controls show as `\u{…}`, its last 16 lines, each its last 200
  characters, with `…` standing for what was cut, so the sentence that asks always shows, then one
  line naming the Machine's `ssh` target and what is asked, with the host key's algorithm
  and fingerprint for a new host and a note when `ssh` refused the previous answer of that kind.
  The client reads each prompt as the hub does ([kinds](#login-episode)), so only `ssh`'s own
  first-time host sentence for the Machine's host is read with echo and trusted by typing `yes`;
  every other answer is read without echo. Every answer goes straight to the question's
  [answer endpoint](#answer-path), never to the hub. One the endpoint does not take is reported at
  once: an answer holding a character `ssh` cannot take, such as NUL, reached nothing, so the same
  question is asked again; an endpoint that is gone or refuses it dismisses the episode, and the
  command ends with the hub's reply, `auth-required` when the dismissal ended the attempt. An
  empty line, or `yes` missing for a new host, sends `dismiss`, which ends the attempt
  `auth-required`; Ctrl+C sends `dismiss`, closes the attachment and ends `cancelled` with exit 130. Either way the console's echo is restored. Time spent at a question does not count against
  the 90 seconds; each question has the hub's own bound.
- **`--remember`.** A password or key passphrase typed at the console is kept, in a buffer wiped
  when dropped, until the login ends; a login that ends `succeeded`, or a reply `online`, writes it
  to the [credential store](#remembered-answers), and any other end wipes it. `ssh` asking the
  same kind again refused the kept answer, which goes. Where no store can be used, or the local
  `ssh` is older than OpenSSH 8.5 or names no release, the command says why once, at the first
  password or passphrase question: that passwords cannot be remembered in this kind of login, or
  that they can be with OpenSSH 8.5 or later; it then asks as without the option. Without a
  console nothing is typed, so nothing is remembered. Whatever the option, a question marked `remembered: rejected` is preceded
  by a line saying that the saved password no longer works.
- **Agents in a Panel.** An Orchbee Panel gives a command a console on both streams, so an Agent
  that runs `machine connect` there is asked and waits for a person. An Agent that cannot answer
  redirects standard input away from the console, for example from the null device; the command
  then sends `answers` false and a sign-in that needs an answer ends `auth-required` at once.
- **Disconnect** starts no hub: with none running there is no connection, and it succeeds with
  `remote: "not-contacted"` once the operand resolves. Otherwise it asks the hub to `reload`
  before it resolves the operand, so a record that was removed or changed without telling the hub
  ends there too, then sends `disconnect`; a hub whose catalog does not list the machine carries
  nothing for it, and a closing hub ends every carrier, so both read as disconnected. The deadline
  is 15 seconds. The remote Session keeps running.
- A `reload` answered `invalid-catalog` or `storage` leaves the hub on the catalog it had, which
  may name a record as it was, so the command ends with that code and asks for nothing more.
- A reply `stopping`, or a connection lost with the request in flight, is asked once more through
  the hub that takes the closing one's place, from its `reload`; a connect whose second ask meets
  `stopping` again reads `hub-unavailable`.
- An interrupt ends the wait with `cancelled` and exit 130 and leaves the hub's attempt running,
  unless a question is open on this terminal, which it dismisses as above. A
  connect that a `disconnect` from elsewhere ended, or a `reload` that changed the record's target
  or Session, reads `cancelled` with exit 7, the accepted operation's cancelled result; the reply
  does not tell the two apart, so the message names both.
- `remote` reads `possibly-started` from the moment `connect` is sent, so a deadline or interrupt
  reports that the remote Session may have started, but an interrupt at an open question, or while
  the command waits for a person elsewhere, reads `unchanged`, since `ssh` asks before the far
  side runs anything; a reply settles it: `ready` once
  online, `possibly-started` after `remote-not-ready`, `timeout`, or `cancelled`, `not-contacted` when
  the hub never tried the machine (`not-found`, `disabled-machine`, `unsupported-machine`,
  `ssh-unavailable`, or a failure of the hub itself), and `unchanged` otherwise.

| Hub reply or failure                                           | `error`                |
| -------------------------------------------------------------- | ---------------------- |
| `connect-result` failure that names a receipt code             | The same code          |
| `connect-result` `cancelled`                                   | `cancelled`, exit 7    |
| `connect-result` `timeout`, never given for a login            | `timeout`              |
| `reload-result` `invalid-catalog` or `storage`                 | The same code          |
| Connect: `stopping` after the second ask                       | `hub-unavailable`      |
| `connect-result` `invalid-draft`, `internal`, or `unknown`     | `internal`             |
| The data root cannot be resolved                               | `configuration`        |
| No hub can be reached or started, it refuses, closes, or drops | `hub-unavailable`      |
| Another version or environment's hub serves the data root      | `version-incompatible` |
| The deadline passes                                            | `timeout`              |

## Machine receipt

`orchbee.machine` version 1 reports each machine as
`{id, label, connection, session, enabled, remoteInstall}`. `connection` is
`{ "kind": "ssh", "target": … }`, or the shared `{ "kind": "unsupported" }` for a kind this build
does not know; the content of an unknown kind is never echoed. `error` is a closed list that grows
as remote-machine support lands; a reader treats a value it does not know as `unknown`.

| `error`                                          | Exit | Produced by this release                                                        |
| ------------------------------------------------ | ---- | ------------------------------------------------------------------------------- |
| `configuration`, `invalid-catalog`, `storage`    | 1    | Catalog location, content or storage failure                                    |
| `internal`                                       | 1    | Unexpected local failure                                                        |
| `not-found`, `ambiguous-machine`                 | 3    | `<id\|label>` resolution                                                        |
| `disabled-machine`                               | 3    | Connect to a disabled machine                                                   |
| `auth-failed`, `host-key-changed`                | 3    | Connect: OpenSSH refused the answers or the key; the host key changed           |
| `host-untrusted`                                 | 3    | Connect without a terminal to ask: the host key is not trusted yet              |
| `busy`, `capacity`, `revision-exhausted`         | 4    | Another writer, 64 machines or 128 KiB, revision bound                          |
| `not-yet-available`                              | 4    | [Commands not available yet](#commands-not-available-yet)                       |
| `unsupported-machine`                            | 4    | Connect to a machine whose connection kind this build does not know             |
| `hub-unavailable`                                | 4    | [Connect and disconnect](#connect-and-disconnect) could not use the machine hub |
| `ssh-unavailable`                                | 4    | The system OpenSSH client is missing                                            |
| `remote-not-installed`                           | 4    | The remote launch line finds no Orchbee, or no usable install                   |
| `remote-not-ready`                               | 4    | The remote Session did not report ready                                         |
| `host-unreachable`                               | 4    | Connect: the host did not resolve, refused, timed out, or had no route          |
| `auth-required`                                  | 4    | Connect: signing in needs a person and nobody answered here                     |
| `ssh-failed`, `remote-protocol`                  | 5    | Any other OpenSSH failure; a malformed remote document or link                  |
| `remote-unsupported`, `remote-other-environment` | 6    | Remote system or architecture; another environment kind                         |
| `version-incompatible`                           | 6    | Connect: either side too old for the other, or another build's hub runs         |
| `timeout`                                        | 7    | Deadline                                                                        |
| `cancelled`                                      | 7    | Connect: a `disconnect` or a changed record ended the attempt                   |
| `physical-unknown`                               | 8    | SSH process cleanup unconfirmed                                                 |
| `remote-install-failed`                          | 9    | Not yet                                                                         |
| `cancelled`                                      | 130  | Interrupt                                                                       |

## Remote preparation

The app-private `machine_ssh` owner exposes executable discovery, the `ssh` command line and
bounded process execution to the two places that start `ssh`: the hub's
[carrier child](#the-carrier-child) and `machine add`'s preparation below, the one start outside
the hub until adding moves onto it. Each retains its own readiness and submission facts. Windows
ports keep the system OpenSSH and process owner; Unix ports return `SavedMachines` unsupported
before executable lookup or process creation.

Add uses the OS OpenSSH executable and a 60-second total deadline. Batch mode,
strict host-key checking, no forwarding, bounded output, and an owned process
Job apply. Password prompts and new host trust must first be handled through
OpenSSH. SSH diagnostics are not copied into machine JSON or product errors.

`ssh` never changes the person's files by itself. `machine_ssh::command`, which builds every `ssh`
Orchbee starts, sets `UpdateHostKeys=no`: since OpenSSH 8.5 the default otherwise lets `ssh` add
and remove a host's keys in `known_hosts` after a login, which a background hub would repeat at
every connection.
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
admits each later Client connection, carried by a hub Session stream, through the handshake, and
the routed command runs the remote Client under its own grammar. The probe resolves the remote program through the remote store's
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
real Windows lock and atomic writer. These tests do not establish a real
two-machine SSH/TUI witness. Fixtures and their process Jobs are owned by the
test and released after the last consumer.

## Command routing contract

`orchbee --machine <id-or-exact-label> <command>` selects an enabled saved
machine by the [`<id|label>` rule](#catalog-and-transitions). Selection refuses `not-found`,
`ambiguous-machine`, and `disabled-machine` with exit 3, and `unsupported-machine`, a connection
kind this build does not know, with exit 4. Like `--session`, the
option may stand anywhere before `--`, accepts `--machine=value`, may appear once, cannot combine
with `--session`, and never changes the catalog. An option's value is never read as it. Help is local. Remote execution admits CLI commands
for workspace, tab, panel, Client queries/control, terminal queries/send, Agent operations, worktrees,
notifications, runtime configuration, manifests, `server status <id>`, and supported plugin
operations. Local management, installation, TUI, and terminal interactive streams are refused.
`server stop` is never routed as a command line: with the prefix it is the
[remote Session stop](#remote-session-stop), and a far end of this build refuses an older near
side's routed `server stop` as not submitted (`remote-rejected-or-interrupted`, exit 5).

The owner is the [machine hub](#machine-hub), not the local IPC transport. The command reaches it
through the [client owner](#client-owner), attached with neither `presence`, `asks` nor `watch`,
and starts a hub when none runs. It asks the hub to `reload`, a hub closing under the request once
more through the hub that replaces it, and then opens one hub `route` channel at once: a Machine
already `online` serves the command without a `connect`, so the hub's [countdown](#lifetime) stays
where it was. Only when the hub resets the route `not-online`, or `stopping` because it closed
meanwhile, does the command ask it to `connect` the Machine with `origin` `command` and no
`answers`, under the rules and errors of `machine connect`: a login that needs a person waits for a
person at an interface that `asks`, within the hub's bound on the question, with the waiting line
on standard error, and ends `auth-required` at once when none `asks` or the Machine is muted, nothing
submitted; then it opens the route once more; that `connect` restarts the countdown
as every `connect` does. On the route the command writes one bounded request packet, sends `end`,
and reads the reply packet until `close`; the hub [relays](#channels) it to a link `route` channel,
so three commands to one Machine share its one carrier. The packet and its
reply are closed objects whose `version` is their `machine-route` generation in the
[connection registry](../../../schemas/connection/v1/connections.json); each may carry an
`extensions` object whose member names follow the capability-name grammar and whose content the
receiver ignores. Neither carries a cohort. The remote [link helper](#link-helper) refuses a
generation it does not read, then revalidates the Session, argument grammar and command allowlist
under its own grammar before spawning its own executable with literal arguments in an owned
Windows Job.
The packet's `locale` names the sender's language; a value the receiver does not ship, including
one a later sender adds, falls back to `en_US.UTF-8` instead of refusing the command, and the
routed command runs under the resolved locale.
When the remote grammar cannot parse the routed arguments, the reply carries the remote's ordinary usage failure in the packet's locale and with the usage exit status, rendered by the link helper without spawning a command. A parsed request outside the routed scope, a packet that names a Session, and invalid input are refused as not submitted. The existing remote CLI remains the
business-operation owner. No local peer credential admission is bypassed.
Terminal file and stdin sources are acquired on the caller once and sent as raw
bytes; directories and executable paths are interpreted by the remote command. A process case
proves it through the hub's route and the stand-in: a home-relative path operand reaches the far
side as written, the far business process resolves it against the far home, and a far home that
cannot be used fails as that command's usage failure, naming the variable and never its value,
and writes nothing to the far profile.
Caller terminal/worker context is cleared; explicit targets or remote Client
selection determine the destination. Output bytes, diagnostics and exit status
are returned in a bounded reply, never reconstructed from SSH diagnostics.

Local input has a 60-second bound, and reaching `online` 90 seconds: a hub start, then the hub's
own 60-second attempt. Execution honors the command's own deadline, including explicit forever
waits: the route has no deadline of its own, and the hub's expiry never ends a route in flight
([lifetime](#lifetime)). Only an interrupt, a lost link, or a hub stopped or replaced on purpose,
which gives open routes 60 seconds, ends it first. An interrupt resets the `route` channel
`cancelled`, which ends the remote command and the channel and leaves the connection and the
Machine as they were; the command exits 130. Dispatch is the hub's `end` on the channel. Losing the
link, the hub, or the attachment after it, or a far end that resets the route after it, cannot
prove remote process settlement or business rollback: the failure reports an unknown remote
outcome and never retries. Before dispatch it reports not-submitted. The remote helper owns and
settles its command Job; a disconnected peer can still leave an already accepted business action,
and no distributed cancellation guarantee is claimed. Wire request/reply limits and output
truncation fail closed. No real SSH or cross-machine acceptance is inferred from tests through the
stand-in.

A failure document `orchbee.cli.failure.machine-route` names its `reason`, the `remoteOutcome`
(`not-submitted` or `unknown`) and a failure kind, and the exit code follows from the kind alone.
A not-submitted failure takes the kind of its reason:

| Kind                  | Exit | Reasons                                                                                                                                                                                                                                                                                           |
| --------------------- | ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `check-failed`        | 1    | `configuration`, `invalid-catalog`, `storage`                                                                                                                                                                                                                                                     |
| `internal-failure`    | 1    | `internal`                                                                                                                                                                                                                                                                                        |
| `not-found`           | 3    | `not-found`                                                                                                                                                                                                                                                                                       |
| `ambiguous`           | 3    | `ambiguous-machine`                                                                                                                                                                                                                                                                               |
| `rejected`            | 3    | `disabled-machine`, `input-source`                                                                                                                                                                                                                                                                |
| `denied`              | 3    | `auth-failed`, `host-key-changed`, `host-untrusted`                                                                                                                                                                                                                                               |
| `service-unavailable` | 4    | `busy`, `capacity`, `revision-exhausted`, `ssh-unavailable`, `remote-not-installed`, `remote-not-ready`, `unsupported-machine`, `hub-unavailable`, `host-unreachable`, `auth-required`, `remote-install-failed`, and `cancelled` when the hub ended the connection attempt the command waited for |
| `transport-loss`      | 5    | `ssh-failed`                                                                                                                                                                                                                                                                                      |
| `protocol-invalid`    | 5    | `remote-protocol`, `remote-rejected-or-interrupted`                                                                                                                                                                                                                                               |
| `incompatible`        | 6    | `remote-unsupported`, `remote-other-environment`, `version-incompatible`                                                                                                                                                                                                                          |
| `timeout`             | 7    | `timeout`: a local deadline elapsed                                                                                                                                                                                                                                                               |
| `outcome-unknown`     | 8    | `physical-unknown`                                                                                                                                                                                                                                                                                |
| `cancelled`           | 130  | `cancelled` from an interrupt                                                                                                                                                                                                                                                                     |

An unknown remote outcome keeps `cancelled` for an interrupt and `timeout` for a deadline and is
otherwise `outcome-unknown` with exit 8 whatever its reason, so no exit that states that nothing
ran ever follows a dispatched command.

Private requests are at most 2 MiB and replies at most 24 MiB; raw stdout and
stderr are each at most 8 MiB. Arguments are bounded to 64 KiB and 1024 entries.
PowerShell completion supports the selector prefix and routed command scope.

## Remote Session stop

`orchbee --machine <id-or-exact-label> server stop` stops the selected Machine's remote Session and
every terminal in it through the hub's [`stop-session`](#stopping-a-session); nothing travels as a
routed command line. The grammar is the local `server stop`'s: development and test builds require
`--confirm`, and `--request-id`, `--retry`, `--detach` and `--timeout`, which have no counterpart in
`stop-session`, are usage failures (exit 2). The command then reads the catalog and selects the
Machine by the [routing rules](#command-routing-contract); a selection or catalog failure is a
not-submitted `orchbee.cli.failure.machine-route` document, and no hub is asked.

The near side next applies the [release confirmation](../README.md#windows-release-destructive-confirmation)
exactly as a local `server stop` does, before the hub is asked anything: the release build refuses
`--confirm` and every invocation without two consoles, and otherwise prompts on standard error with
the Machine's label, the remote Session's name and that every terminal in it stops, and runs only
when the person types that Session's name. A refusal is the local one, naming
`orchbee --machine <selector> server stop` to run by hand, or the Machine's ID when the selector is
not a plain word. The request log records it as `machine server stop` with the remote Session.

The command then reaches the hub through the [client owner](#client-owner), starting one when none
runs, asks it to `reload`, and sends `stop-session`. When the hub answers `not-online` or `stopping`,
nothing was asked of the Session, so the command has the Machine brought `online` under the rules
and errors of a routed command, whose failures are that command's not-submitted documents, and sends
`stop-session` once more. Starting the hub and reaching `online` are each bounded by 90 seconds, and
each `stop-session` answer by 120 seconds, past the run's 65-second watch and its last 30-second
exchange. An interrupt before `stop-session` is sent ends the command with the not-submitted
`cancelled` failure (exit 130). Once it may have been sent, an interrupt ends the wait and not the
stop, an unanswered or lost request reads the same, and the outcome is `unknown`.

The result is `orchbee.machine.server-stop` version 1:
`{schema, version, result: {machine: {id, label, session}, outcome}}`, where `machine` names the
saved Machine as the [machine receipt](#machine-receipt) does and `outcome` is the hub's word. The
exit code follows from the outcome alone:

| Outcome       | Exit | Class                                                                                      |
| ------------- | ---- | ------------------------------------------------------------------------------------------ |
| `stopped`     | 0    | Success                                                                                    |
| `not-running` | 4    | Unavailable, as a local `server stop` that finds no Session                                |
| `not-online`  | 4    | `service-unavailable`                                                                      |
| `stopping`    | 4    | `service-unavailable`                                                                      |
| `not-found`   | 3    | `not-found`                                                                                |
| `failed`      | 9    | Failed: the Session may have accepted the stop and ended part of it, so nothing is claimed |
| `unknown`     | 8    | `outcome-unknown`                                                                          |

`--format text` prints one line naming the Machine and the outcome, and says when an interrupt
ended the wait; raw `ssh` text never reaches it. The command never retries a sent `stop-session`.
An older build's far end is never sent anything by this command. An older near side's routed
`server stop` reaches a far end of this build as a routed command line, which it refuses.

## Link helper

`orchbee --machine-link-v1`, with no other argument, is the remote end of the
[`machine-link`](../../../schemas/machine/README.md) connection: the hub starts it over `ssh` and
speaks to it on its standard input and output. One pair of pipes carries several Sessions' byte
streams, routed commands, and Session starts at once. Windows serves it; elsewhere it answers
unsupported before reading a byte. It is the only remote entry Orchbee starts over `ssh`: the
earlier `--machine-tunnel-v1` and `--machine-relay-v1` helpers are gone, and a build that still
starts one finds an unknown argument. The contract owns every byte; this section owns how the
helper meets it.

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
| `route`          | At once                               | After the opener's `end`, the reply packet of the [routed command](#command-routing-contract), under its admission and allowlist, then `replied`                                      |
| `ensure-session` | At once                               | `ready` with `started` once the Session admits a Client, as `server start` would; `unknown` when a start ran out of its 30 seconds or its Server may have started; `failed` otherwise |

A name the Windows Session grammar refuses resets `invalid-session` before `accept`. The pipe is
found as a local Client finds it, through the data root's Session catalog spelling, and must
belong to the same user. `ensure-session` uses the same activation as `server start`; the Server it
starts outlives the helper. `install` is answered as an unknown kind until its capability ships.

A channel is begun once an effect may have started on it: a `session` from its `accept`, once its
pipe writer runs; a `route` from the opener's `end`, once its command worker runs; an
`ensure-session` from its `accept`, since its start runs at once. The contract lets a responder's
`reset` say only that nothing happened, so the helper never resets a begun channel: it ends with
`close` and its outcome, or, when the link ends first, without a result as the carrier closes, which
the hub reads as an unknown effect. A channel on which nothing began may still be reset. The link
ends when its standard input ends or fails, on the hub's `bye`, and after 90 seconds without a
received frame; each of the three ends every channel alike:

| Channel          | Nothing began    | Begun                                                                                |
| ---------------- | ---------------- | ------------------------------------------------------------------------------------ |
| `session`        | Reset `stopping` | The pipe connection closes; no further frame                                         |
| `route`          | Reset `stopping` | The command ends through its Job, as the contract requires; no reply, no other frame |
| `ensure-session` | Never the case   | The start runs to its own 30-second deadline unobserved; no further frame            |

Frames committed before the end, a begun channel's data and a `close` included, are written ahead
of `bye` within the writer's 2 seconds.

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
| Standard input ends or fails        | Every open channel ends as the [link's end](#link-helper) states; best-effort `bye` `done`; exit 0                                                               |
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
[`machine-hub`](../../../schemas/machine/README.md#hub-records). It serves the catalog, its own
lifetime and each Machine's state, and [carries](#carrying-a-machine) each Machine it connects over
one child: `ssh` running the remote [link helper](#link-helper), and [relays](#channels) every
client's Session streams and routed commands over that child. Windows serves it; elsewhere both
hub entries answer unsupported before they touch the data root. The entry is an internal pairing
between this program and its own start path, not a command: nothing outside one installation runs
it, and a release may rename it. Every client reaches the hub through one
[client owner](#client-owner). Two diagnostic entries, equally internal, use it:
`orchbee --machine-hub-state-v1` attaches with `watch`, starting the hub when none runs, prints the
hub's state, its process id, and each Machine's state, with the host key's algorithm and
fingerprint for `untrusted` and `key-changed`, and holds nothing;
`orchbee --machine-hub-hold-v1` attaches with `presence` and `watch` and prints one line per
attachment and event until its standard input ends. None appears in help, completion, or the Agent
guide.

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

| State      | When                                                                                                                                                             | Leaves to                                                                                                                        |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| `held`     | A holder exists, or the last `presence` connection has left and the Session read it triggered has not finished                                                   | `expiring` when no `presence` connection is attached and a Session read finds no live Session; `closing` on `drain`              |
| `expiring` | No holder; `expiresAt` is 600,000 ms after the later of the moment the hub learned that and the last `connect` that ended with its Machine `online`              | `held` on a `presence` attachment or a read that finds a live Session; `closing` on `drain`                                      |
| `expiring` | `expiresAt` is reached, or has passed and the last open route just ended, with no `presence` connection and no route open: the hub reads every Session once more | `held` when the read finds a live Session; `closing` when it finds none and still no route is open                               |
| `expiring` | `expiresAt` has passed with a route open: the state keeps the past `expiresAt` and the periodic reads go on                                                      | `held` on a `presence` attachment or a read that finds a live Session; the row above once no route is open; `closing` on `drain` |
| `closing`  | Entered once and never left                                                                                                                                      | Exit                                                                                                                             |

- **Start.** The hub takes the lock and binds its endpoint at once, then reads the catalog and
  every Session before it accepts a connection, so its first client sees `held` or `expiring`; a
  client that connects meanwhile waits in the pipe within its own deadline. With no live Session
  the expiry counts from that read.
- **Session reads.** A Session is live while its Server holds its lifetime lock. The hub reads every
  Session folder in the runtime base of the environment it started in, where every Session started
  in that environment keeps its runtime. It reads once when the last `presence` connection closes
  or attaches again without `presence`, then every 30 seconds while none is attached, once more at
  `expiresAt`, or when the last route ends after it, before the hub may close, and never while one
  is attached. One read covers every Session within 5 seconds; a read that fails or runs out of time
  finds no live Session. The read at `expiresAt` sees a Session started without an interface since
  the last periodic read, so the hub never closes under it.
- **What a read opens.** A read opens no Session record and nothing in the data root's
  `sessions/`. First it lists the runtime base, holding the listing open only while it reads it,
  and keeps the folders whose names are Session names; a base with more than 256 of them is not
  probed, and the read finds no live Session. Then it probes one folder at a time: it pins the
  base's ancestors, the base and the folder, shared for reading and writing but not for deletion,
  opens the folder's existing `activation.lock` and never creates it, and closes all of them before
  the next folder. No file means no Session; a lock someone holds means an activation, a stop, or a
  deletion is running, which counts as live; otherwise the hub holds the activation lock for the
  instant of `observe_session_lifetime_lock`, which opens the existing `session-lifetime.lock` the
  same way. Neither lock file is replaced by rename: each is created once and removed only with its
  folder. A probe takes a few system calls and never waits; a Session operation that meets it tries
  again, as the next race below describes.
- **A read never meets a Server.** A Server takes its lifetime lock once, at start, while its
  activator holds the activation lock, and releases it during its stop under the same lock, so the
  read can never take a free lifetime lock at the moment a Server takes it. The bare
  `observe_session_lifetime_lock` that `session list` uses could.
- **Time.** The lifetime state machine takes the time as an argument; only the hub's supervisor
  reads the clocks, monotonic for deadlines and the wall clock for `expiresAt`. The Windows
  monotonic clock counts time asleep, so after a long sleep the deadline has passed at the first
  wake; the read at `expiresAt` then decides, so a Session that lives across the sleep holds the
  hub. In development and test builds `ORCHBEE_MACHINE_HUB_EXPIRY_MS`, from 1,000 to 600,000,
  replaces the 600,000 ms expiry; a release build never reads it. The hub reads it from its
  view of the environment ([environment](#environment)); nothing in Orchbee sets it. It
  is listed with every other variable in [environment variables](environment-variables.md).
- **A connect restarts the countdown.** Every `connect` reply `online`, joined or answered at once
  for a Machine already `online`, moves `expiresAt` to 600,000 ms after that reply while the hub is
  `expiring`, and the new state goes to every watcher; in `held` and `closing` it changes nothing.
  A channel, opening or ending, and any other request restart nothing. A routed command opens its
  route first and sends `connect` only for a Machine that is not `online`, so commands to a Machine
  already `online` never extend its login.
- **Expiry never ends work in flight.** A route is a hub `route` channel or a `stop-session`
  [run](#stopping-a-session) until its outcome. While one is open the expiry does not begin closing:
  past `expiresAt` the hub stays `expiring`, the state and every watcher keep the past `expiresAt`,
  and no event is sent for the overdue interval; the state printer reads such a hub as expired and
  closing once no remote command runs. The last route ending wakes the supervisor, whose
  read is then due at once; it closes the hub when it finds no live Session and still no route
  open. Closing for any other reason, `drain`, replacement or uninstall, still gives open routes
  the 60-second allowance below.
- **Events.** Every change of state goes as `hub` to every connection attached with `watch`.

In `closing` the hub rejects offers with `stopping` and still answers control requests, `query`
with state `stopping`; it answers every request `stopping`, resets every `open` `stopping`, starts
no attempt, and answers every waiting `connect` `stopping`. Every open `session` channel closes
`lost` toward its client at once and its link channel is reset `stopping`. Open `route` channels
and `stop-session` runs may finish until 60 seconds after `closing` began; catalog reads already in
flight finish within the same allowance and reply `stopping`. At the allowance a route still open
ends as a lost link would end it ([channels](#channels)), and a run as
[stopping a Session](#stopping-a-session) states. The hub then queues `bye`
`stopping` on every connection, gives its writers at most 2 seconds, ends every carrier child,
closes the endpoint, releases the lock, and exits 0. Which channel ends when is a rule over the
time `closing` began and the time now, which the supervisor passes in. `drain` enters `closing`;
`exit` replies and ends the process at once, without the allowance; its children end with it
through their Jobs.

### Carrying a Machine

The hub owns one record per catalog Machine: its reported state, its retry history, at most one
carrier, and the `connect` requests waiting on it. A carrier is one child process whose standard
input and output are the Machine's [`machine-link`](../../../schemas/machine/README.md#link-records)
connection, with the hub as initiator. The record is a state machine that takes the time and each
carrier event as arguments and returns its effects (start a carrier, end one, open
`ensure-session`, reply to waiters) for the hub to apply; it reads no clock and starts nothing
itself.

| Phase        | Carrier | Reports                                                                                                                                                                                                                        |
| ------------ | ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `idle`       | None    | Its last result: `offline` (with or without a cause), a state that waits for a person or a change, or `disabled` and `unsupported` from the catalog                                                                            |
| `waiting`    | None    | `reconnecting` with the cause and `nextAttempt`                                                                                                                                                                                |
| `attempting` | One     | `connecting` for a first attempt, `reconnecting` with the last cause for an attempt after a failure or a lost link; `stage` `authenticating` while a [login episode](#login-episode) runs, `starting` once the helper answered |
| `online`     | One     | `online`, or `unresponsive` while a `ping` has gone unanswered for 15 seconds                                                                                                                                                  |

| Event                                            | In                           | Next and effects                                                                                                                                                                                                                             |
| ------------------------------------------------ | ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `connect`                                        | `idle`, `waiting`            | `attempting` now, the request waiting                                                                                                                                                                                                        |
| `connect`                                        | `attempting`, `unresponsive` | The request joins and waits                                                                                                                                                                                                                  |
| `connect`                                        | `online`                     | Replied `online` at once                                                                                                                                                                                                                     |
| A `presence` connection attaches                 | `idle`                       | `attempting`, silently: no request waits                                                                                                                                                                                                     |
| A reload adds the record                         | —                            | `idle`; while a `presence` connection is attached, `attempting` silently, as if it had just attached                                                                                                                                         |
| The retry time comes                             | `waiting`                    | `attempting`                                                                                                                                                                                                                                 |
| The helper answers the offer                     | `attempting`                 | Remote version known; `stage` `starting`                                                                                                                                                                                                     |
| `facts` arrives                                  | `attempting`                 | Platform `windows` on `x86_64` or `aarch64`: open `ensure-session` for the Machine's Session. Any other: `unsupported` with `remote-platform`                                                                                                |
| `ensure-session` closes `ready`                  | `attempting`                 | `online`; every waiter replied `online` with the remote version                                                                                                                                                                              |
| `ensure-session` closes otherwise, or is reset   | `attempting`                 | A failure with `remote-not-ready`                                                                                                                                                                                                            |
| The carrier ends, or the offer is refused        | `attempting`                 | A failure with the [classified](#how-a-carrier-end-reads) cause                                                                                                                                                                              |
| 60 seconds pass since the carrier started        | `attempting`                 | The carrier is ended: a failure with `host-timeout` before the helper answered, `remote-not-ready` after. Time while a question is open does not count                                                                                       |
| `ssh` asks a question                            | `attempting`                 | The [login episode](#login-episode) decides: the question goes to the claimant, or the attempt ends at once in the state the question names                                                                                                  |
| The carrier ends                                 | `online`                     | `waiting` with `link-lost`; with a request waiting, `attempting` at once instead                                                                                                                                                             |
| A `ping` stays unanswered 15 seconds; its `pong` | `online`                     | `unresponsive`; back to `online`                                                                                                                                                                                                             |
| `disconnect`                                     | Any                          | The carrier is ended, waiters replied `cancelled`, the history forgotten; `idle` reporting `offline` without a cause, or `needs-login` with cause `disconnected` when it ended a login episode                                               |
| A reload changes the record                      | Any                          | As `disconnect`, waiters replied `cancelled`; a record now disabled or of an unknown kind reports `disabled` or `unsupported` and replies `disabled-machine` or `unsupported-machine`; a removed record replies `not-found` and is forgotten |
| A reload changes or enables an SSH record        | Any                          | After the row above, while a `presence` connection is attached: `attempting` silently, as if it had just attached                                                                                                                            |
| The hub enters `closing`                         | Any                          | No timer runs and no attempt starts; waiters replied `stopping`; a carrier in a login episode ends at once, the others at the end of closing                                                                                                 |

- **Failure.** An attempt that fails records `lastAttempt` and replies to every waiter with the
  `connect` outcome the [state table](../../../schemas/machine/README.md#machine-state) gives the
  result, and `state`. An `offline` cause is transient. A transient failure of a Machine the hub
  keeps goes to `waiting`; of any other Machine to `idle` reporting `offline` with the cause and
  no next attempt. Every other result (`needs-login`, `login-failed`, `key-changed`,
  `not-installed`, `unsupported`, `needs-update`, `local-needs-update`, `incompatible`) is
  permanent: `idle` until a `connect`, a `presence` attachment or a reload asks again.
- **Kept.** The hub keeps a Machine, retrying it by itself, while a `presence` connection is
  attached or once the Machine has been `online` since its last `disconnect`, permanent failure,
  or change. A command-line `connect` that fails therefore leaves no retry running behind it.
- **Retry policy.** The workbench Client crate's `machine_retry` owns it; the TUI uses the same
  schedule for its own attachment and its Session observers. After the n-th consecutive failure,
  from 0, the next attempt waits 500 ms × 2ⁿ, at most 120 seconds. A failure after 60 seconds or more `online`
  starts the count again. Permanent failures schedule nothing.
- **Questions.** Every attempt runs `ssh` with this program's [askpass mode](#askpass-helper) as its
  only way to ask, so no attempt can wait on a console and none is silent by `ssh`'s batch mode:
  the hub's [login episode](#login-episode) decides each question. A key or an agent login connects
  without one.
- **One carrier.** A Machine has at most one carrier and at most one attempt; every `connect` for
  it joins that attempt. Each carrier has a number unique in the hub's life, and an event of a
  carrier that is no longer the Machine's is ignored, so a carrier that ends late never moves a
  Machine that has moved on.
- **What reaches clients.** Every change of a Machine's reported state, `lastAttempt` or `remote`
  goes as one `machine` event to every watcher. `remote` carries the helper's product version from
  its answer and its platform and architecture from `facts`, and stays as last known after the
  link ends.

#### The carrier child

| Rule           | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Program        | The system `ssh` (`<system folder>\OpenSSH\ssh.exe`, never found through `PATH`) with the command line `machine_ssh::carrier_command` builds: `-T`, `StrictHostKeyChecking=ask`, `UpdateHostKeys=no`, no forwarding, no multiplexing, then `-- <target> <launch line>`. Batch mode is off; `machine add`'s preparation keeps `machine_ssh::command`, with batch mode and strict host keys                                                                                                                                                                   |
| Askpass        | Every carrier, the stand-in included, gets `SSH_ASKPASS` set to this program's own path, `SSH_ASKPASS_REQUIRE=force`, a placeholder `DISPLAY`, and `ORCHBEE_MACHINE_ASKPASS` with the hub's askpass endpoint and the carrier's own 16-byte ticket, so `ssh` never reads a console and an [askpass helper](#askpass-helper) is the only way a question reaches anyone                                                                                                                                                                                        |
| Launch line    | The encoded PowerShell program that resolves the remote store's active version, as the probe does, and runs it with `--machine-link-v1`; it exits 125 when it finds none                                                                                                                                                                                                                                                                                                                                                                                    |
| Stand-in       | In a development or test build only, `ORCHBEE_MACHINE_HUB_SSH_STAND_IN` in the hub's view names an absolute program that runs with the one argument `--machine-link-v1` in place of `ssh` and the launch line, with everything else unchanged, and with `-V` alone in place of `ssh -V`; any other value fails every attempt with `ssh-unavailable` and never falls back to `ssh`. A release never reads it. A local far end runs inside the carrier's Job, so a Session it starts ends with the carrier, where a remote Session over `ssh` runs outside it |
| Process        | No console window (`CREATE_NO_WINDOW`), the system folder as working directory, standard input and output the link, standard error a pipe the hub reads, the [child environment](#environment)                                                                                                                                                                                                                                                                                                                                                              |
| Job            | Each carrier starts suspended inside its own Job, which ends every process in it when its last handle closes. Only the hub holds the handle, so the hub's exit, however it happens, a kill or a panic included, ends every carrier and whatever it started                                                                                                                                                                                                                                                                                                  |
| Standard error | The last 4 KiB are kept only to classify an end and are never copied into a record, an event, a log or product copy                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Reaping        | Each carrier has one owner thread, its only reaper: it starts the child, waits for the root process, then terminates whatever is left in the Job, takes the exit status, and reports the end once the carrier's reader, writer and standard-error threads have ended. Ending a carrier stops its writer, which closes the child's input, and terminates the root process; the owner does the rest                                                                                                                                                           |

The link client writes its `OBHS` offer the moment the child starts, since the helper counts its
2-second candidate deadline from its own start, then reads the answer and the frames that follow.
It never opens a channel before the Machine's `facts` is accepted. Once `online` it sends `ping`
every 30 seconds, well inside the helper's 90-second idle limit; an attempt's 60-second deadline
comes before that limit.

#### How a carrier end reads

| Seen                                                                                                                                                        | Result                                                                                   |
| ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| The program cannot be started because it does not exist                                                                                                     | `offline` cause `ssh-unavailable`                                                        |
| The program cannot be started otherwise                                                                                                                     | `ssh-failed`                                                                             |
| Before the answer, exit 125                                                                                                                                 | `not-installed`                                                                          |
| Before the answer, exit 255 with `Permission denied` after the claimant delivered a password, passphrase or other answer in this attempt                    | `login-failed` cause `password-rejected`                                                 |
| Before the answer, exit 255 with `Permission denied` and a password or keyboard-interactive method, or `Host key verification failed` without a changed key | `needs-login` cause `auth-required`                                                      |
| Before the answer, exit 255 with `Permission denied` and only `publickey`                                                                                   | `login-failed` cause `key-rejected`                                                      |
| Before the answer, exit 255 with `REMOTE HOST IDENTIFICATION HAS CHANGED`                                                                                   | `key-changed`, with `hostKey` when `ssh` printed the offered key                         |
| A question nobody can answer in the [login episode](#login-episode)                                                                                         | `untrusted` with `hostKey` for a new host, otherwise `needs-login` cause `auth-required` |
| An [askpass helper](#askpass-helper) the hub cannot ask while attempting                                                                                    | `needs-login` cause `auth-required`                                                      |
| Before the answer, exit 255 with a resolution, refusal, timeout or route failure                                                                            | `host-unresolved`, `host-refused`, `host-timeout` or `host-unreachable`                  |
| Before the answer, exit 255 otherwise                                                                                                                       | `ssh-failed`                                                                             |
| Before the answer, any other end, or a frame that is not an `OBHS` answer                                                                                   | `remote-protocol`                                                                        |
| `initiator-too-old` / `initiator-too-new`                                                                                                                   | `local-needs-update` / `needs-update` cause `remote-too-old`                             |
| `environment-mismatch`, `fingerprint-mismatch`                                                                                                              | `incompatible`                                                                           |
| Any other refusal                                                                                                                                           | `remote-protocol`                                                                        |
| After the answer, a violation by either end, `bye`, or the end of the output                                                                                | `remote-protocol` while attempting; `link-lost` once `online`                            |

OpenSSH exits 255 for every failure and only its standard error tells them apart; the phrases
above are the OpenSSH 9.5 wording [M0](#accepted-limits) recorded. A changed host key is never a
question: `StrictHostKeyChecking=ask` makes `ssh` refuse it before it asks anything. No end is
ever a hub failure: a far end that speaks another version, sends a wrong frame, or closes becomes
the Machine's state.

#### Login episode

An episode is one occasion on which a person is asked to log in to one Machine, as the contract's
[login episode](../../../schemas/machine/README.md#login-episode) defines it. It lives inside one
attempt, at most one per Machine, and the Machine's state machine owns it: questions, answers taken,
the clients' claims, dismissals and leavings, the requests waiting and the clock are its inputs, and
its outputs are effects the hub applies: events to the clients that show it, closing the open
question's helper, ending the carrier, and replies.

An episode holds its `trigger`; its claimant, none while it is `offered`; its audience, the clients
its `login` events go to: the claimant alone for a `user` episode, and every client that `asks`, the
claimant included, for a `command` one; and at most one open question. A question is put to the
claimant only. While the episode is `offered` the open question is held, unshown, for at most 120
seconds from when `ssh` asked it; when a client claims it the held question goes to that client,
which has the whole 120-second question bound from then. A waiter is a `command` request, with or without `answers`, that waits on the
attempt; each starts its own 120-second bound when the hub sends it `connect-waiting`, either when
the episode begins or when it joins a running one, and the bound lifts when the episode ends, so a
login that succeeded waits for `online` under the attempt clock alone. `waiters` in every `login`
event counts the `command` requests waiting, the claimant's own excluded. The Machine also holds
`muted`.

| From                 | Event                                                                                                                                                    | To and effects                                                                                                                                                                                                                                                                                                         |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| None                 | A question, and a `user` request waits; the first in arrival order                                                                                       | `claimed` by it, trigger `user`: `connect-waiting` for its request and for each waiter, `login` `claimed` `you`, then `question` 1; `stage` `authenticating`; the 60-second attempt clock stops while a question is open                                                                                               |
| None                 | A question, a waiter, a client that `asks` attached, and the Machine not muted                                                                           | `offered`, trigger `command`: `connect-waiting` for each waiter, `login` `offered` to every client that `asks`; the question is held; `stage` `authenticating`; the attempt clock stops while a question is open                                                                                                       |
| None                 | A question, a `command` with `answers` waits, and no client `asks` or the Machine is muted; the first in arrival order                                   | `claimed` by it, trigger `user`, as for a `user` request                                                                                                                                                                                                                                                               |
| None                 | A question and nobody to ask: a silent attempt, a reconnect, or only `command` requests without `answers` while no client `asks` or the Machine is muted | No episode and no event: the carrier ends at once, then the helper is closed; `untrusted` with `hostKey` for a new host, otherwise `needs-login`, with cause `login-dismissed` while muted, `remembered-rejected` when `ssh` refused a remembered answer, and `auth-required` otherwise; waiters replied by that state |
| `offered`            | `claim` from a client that `asks`                                                                                                                        | `claimed` by it: `login` `claimed` `you` to it and `claimed` `other` to the rest of the audience, then the held question to it, 120 seconds from now, but never past 240 seconds after `ssh` asked it                                                                                                                  |
| `offered`            | A `user` `connect` for the Machine                                                                                                                       | `claimed` by its requester as by a `claim`, after `connect-waiting` for its request; the mute, if any, lifts                                                                                                                                                                                                           |
| `offered`            | `dismiss` from a client that `asks`                                                                                                                      | `login` `ended` `dismissed`; the carrier is ended; waiters replied `auth-required`; the Machine is muted                                                                                                                                                                                                               |
| `offered`            | The last waiter left: its connection closed or its 120 seconds passed; or the last client that `asks` closed or stopped asking                           | `login` `ended` `abandoned`; the carrier is ended; `needs-login` `auth-required`; waiters replied `auth-required`                                                                                                                                                                                                      |
| `offered`, open      | Nobody claims the open question within its bound: 120 seconds after `ssh` asked it, or the bound a claimant that left still had                          | `login` `ended` `abandoned`; the question closes; the carrier is ended; `needs-login` `auth-required`; every waiter replied `auth-required`, whenever it joined                                                                                                                                                        |
| `claimed`, `command` | The claimant's connection closes or it stops asking, a waiter remains, and another client `asks`                                                         | `offered` again to every client that `asks`; an open question is held again, its bound still running                                                                                                                                                                                                                   |
| `claimed`            | The claimant's connection closes, or a `command` episode's claimant stops asking, and nobody else can be asked                                           | `login` `ended` `abandoned`; the open question closes; the carrier is ended; `needs-login` `auth-required`; other waiters replied `auth-required`                                                                                                                                                                      |
| `claimed`            | The claimant's `dismiss`                                                                                                                                 | `login` `ended` `dismissed`; the open question closes; the carrier is ended; `needs-login` `auth-required`; waiters replied `auth-required`; a `command` episode mutes the Machine                                                                                                                                     |
| Either               | A waiter's 120 seconds pass                                                                                                                              | It is replied `auth-required` and leaves; a claimed episode stays claimed and its audience reads the new count                                                                                                                                                                                                         |
| Either               | A request joins, or a waiter's connection closes                                                                                                         | The audience reads the new count; a joining waiter receives `connect-waiting` and starts its bound                                                                                                                                                                                                                     |
| Either               | A client starts asking                                                                                                                                   | A `command` episode adds it to its audience and sends it the current phase: `offered`, or `claimed` `other`                                                                                                                                                                                                            |
| `claimed`, open      | The helper reports the answer taken                                                                                                                      | The question closes as answered; the attempt clock runs again                                                                                                                                                                                                                                                          |
| Either, open         | The helper leaves without an answer, or a second question arrives while one is open                                                                      | `login` `ended` `unavailable`; the carrier is ended before either helper is closed, so `ssh` never sends an empty answer; `needs-login` `auth-required`; waiters replied `auth-required`, except a person's own request `timeout` when the helper left                                                                 |
| Either, none open    | A question                                                                                                                                               | Next id; `repeated` when an answer of the same kind was taken earlier in the episode; put to the claimant, or held while `offered`                                                                                                                                                                                     |
| Either               | The helper answers the carrier's offer: `ssh` authenticated                                                                                              | `login` `ended` `succeeded`; waiters then wait for `online`                                                                                                                                                                                                                                                            |
| `claimed`            | The carrier ends with `Permission denied` after a password, passphrase or other answer                                                                   | `login` `ended` `failed`; `login-failed` `password-rejected`; waiters replied `auth-failed`                                                                                                                                                                                                                            |
| Either               | The carrier ends otherwise, or a reload, `disconnect` or `closing` ends the attempt                                                                      | `login` `ended` `unavailable`; the Machine's state names the reason; waiters replied as that end says                                                                                                                                                                                                                  |
| `claimed`, open      | The question stays open to its bound: 120 seconds after the claim put it, never past 240 seconds after `ssh` asked it                                    | `login` `ended` `unavailable`; the question closes; the carrier is ended; `needs-login` `auth-required`; `command` waiters replied `auth-required`, as nothing was submitted, and a person's own request `timeout`                                                                                                     |

- **Invariants.** A `command` episode's audience is exactly the clients that `asks`, plus a claimant
  that came by a `user` `connect`; an `offered` episode has a waiter and a client that `asks`; every
  open question has a bound, offered or claimed; a waiter has a bound only while an episode runs;
  a `command` request is never replied `timeout` for a login that did not complete.
- **Mute.** A `command` episode that ends `dismissed` mutes the Machine: every waiter has already
  failed, the hub offers it no `command` episode, and its `needs-login` reads cause
  `login-dismissed`. A `user` `connect` lifts the mute, and so does the last client that `asks`
  closing or no longer asking, which turns that cause back into `auth-required`. A reload that
  changes the record starts the Machine anew, unmuted.

- **Remembered first.** Before any of the rows above, the first password and the first key
  passphrase question of each attempt is held while its helper reads the
  [remembered answer](#remembered-answers), when the local `ssh` is OpenSSH 8.5 or later and no
  hold keeps that kind back; only when none is usable does the question go on as the table says.
  So a `command` attempt tries a usable remembered answer before any client is offered the login:
  its waiters receive no `connect-waiting` and no interface shows anything. A remembered answer
  `ssh` refuses goes on as the table says, the question it refused marked `rememberedRejected`
  when it is put to a claimant, which for a `command` episode is when a client claims it.
- **No retry of the hub's own.** Wrong answers lead wherever `ssh` goes: its next question, whose
  `repeated` says the last one was refused, or its end, which the table above reads. The hub never
  asks again by itself and never starts another attempt for a person.
- **One question at a time, in order.** `ssh` asks one question at a time and waits for its
  answer, and the episode admits one open question; ids ascend within the episode, and login ids
  ascend in the hub's process life.
- **Kinds.** The [askpass helper](#askpass-helper) reads the kind from `ssh`'s prompt alone, since
  `ssh` passes none, and only `ssh`'s own local sentences, whole and from their first word, read
  as a kind: the one line `Enter passphrase for key '…':` is a key passphrase, the one line
  `user@host's password:` a password, and `The authenticity of host '…' can't be established`
  with a later line exactly `<algorithm> key fingerprint is <fingerprint>.` and a last line asking
  to continue connecting is a host-key confirmation with that key. Anything else is `other`: a
  keyboard-interactive prompt, which OpenSSH 8.5 and later prefix with `(user@host) `, a server's
  text, a confirmation whose key it cannot read. The hub reads the prompt again against the
  Machine's `ssh` target, and so does the asking client: a password or a new host that names
  another host than the target's, such as a jump host's, is `other`, so it is never shown as the
  Machine's own, never read with echo, and never marks the Machine `untrusted`. The host compared
  is the target's host part, without its user and port; an alias whose `HostName` differs reads
  `other` too.
- **`claim` and `dismiss`.** `claim` of an `offered` episode by a client that `asks` makes it the
  claimant and answers `claimed`, after the events it causes; it answers `claimed` to the claimant,
  `taken` to another client or to one that does not ask, and `ended` for an episode that is over or
  unknown. `dismiss` answers `dismissed` to the claimant, or to any client that `asks` while the
  episode is `offered`, `not-claimant` to another client and `ended` likewise; both answer
  `stopping` while closing.
- **The question bound.** A question nobody claims ends 120 seconds after `ssh` asked it, the bound
  the contract gives a command's wait for a login. A claim gives the person the whole 120 seconds
  from then, and a claim after a claimant left gives 120 seconds again, but a question never stays
  open past 240 seconds after `ssh` asked it. The hub ends the carrier, and with it the helper,
  before it closes the question; the helper's own 250-second bound, past those 240 seconds, comes
  only for a hub that stopped working. A waiter's 120 seconds are the same figure. In development
  and test builds `ORCHBEE_MACHINE_HUB_QUESTION_MS`, from 1,000 to 120,000, shortens all of them
  but the helper's for tests; a release never reads it.
- **A server that stopped waiting.** OpenSSH's default `LoginGraceTime` on the server is also 120
  seconds, so a late claim can meet a connection the server already dropped. `ssh` then ends on
  its own words for a closed connection, which read `ssh-failed`: the episode ends `unavailable`,
  the interface says the connection failed and to try again, and nothing reads as a refused
  answer, so no remembered answer is deleted and no answer typed is saved. An `ssh` that ends
  while the question is still open reads as its helper leaving, `auth-required`, or as its own
  end when the hub sees that first; the episode ends `unavailable` either way.

#### Askpass helper

`ssh` runs this program as its askpass with the question's prompt as its argument. Whenever
`ORCHBEE_MACHINE_ASKPASS` is set, the program is the helper and nothing else, before it parses any
command line and whatever its arguments are, so text from the far side never selects a local
command; the prompt is every argument joined by single spaces, since `ssh` passes one and how
Windows OpenSSH quotes it is not proven. The one exception is a development or test build started
with exactly `--machine-link-v1`, the far end a [stand-in](#the-carrier-child) starts, which runs as
the link helper; a release never makes it. Such a far end inherits the variable, which a far end
over `ssh` never has, so the routed commands it runs start without it. The variable is private: the hub sets it only in its
carrier children, as `<askpass endpoint> <ticket>`, with the ticket as 32 lowercase hexadecimal
digits.

1. It connects to the hub's askpass endpoint within 5 seconds, which admits only the hub's user.
2. It reads the prompt as text, replacing what is not Unicode and cutting at 4,096 bytes on a
   character boundary, and reads the [kind](#login-episode) from it.
3. It binds its own one-shot answer endpoint, `\\.\pipe\orchbee-answer-<pid>-<random>`, and draws
   a 16-byte token.
4. It writes one record: `OBAQ`, a big-endian `u32` length, and a JSON object with the ticket and
   the question: the kind, the prompt, the host key when there is one, the answer endpoint and the
   token. A helper that cannot ask, for an empty prompt or when it cannot draw random bytes or bind
   its endpoint, writes the ticket alone. A record holds at most 32 KiB, which any 4,096-byte
   prompt fits. The hub reads it within 5 seconds and matches the ticket to a carrier it is
   running.
5. For the first password or key passphrase question of an attempt that the
   [remembered-answer rules](#remembered-answers) let it recall, the hub writes a recall record: `R`, the kind byte, `p` or `k`, and the Machine's 32-digit id. The helper reads that
   Machine's item of that kind from the [credential store](#remembered-answers) of this build's
   environment. A usable answer, nonempty, at most 1,024 bytes and holding nothing `ssh` cannot
   take, goes as step 7 says after the helper wrote `M`; otherwise the helper writes `N` and the
   question stands. The hub never sends a recall for any other question.
6. It waits for an answer on its endpoint, for the hub to close its connection, or for 250 seconds,
   whichever comes first. A connection whose record is malformed or carries another token is closed
   without a byte and the endpoint keeps waiting, for at most eight such connections.
7. With an answer it writes the byte `T` to the hub, so the hub knows the question was answered,
   replies 0x06, drops the endpoint, writes the answer and one line feed to its standard output for
   `ssh`, wipes its buffer and exits 0.

A helper without an answer writes nothing and does not exit before its 250-second bound, because
`ssh` reads a helper that exits as an empty answer and may send it, which a server counts as a
failed password. The helper runs inside its carrier's Job, so the carrier's end ends it first:

- Every helper the hub cannot ask whose ticket names a running carrier ends that carrier before
  its connection is closed: a ticket alone, a record the hub does not admit, a second helper while
  one is open, a question once the attempt cannot ask, which ends the attempt `auth-required` or,
  once `online`, loses the link, and a question nobody can answer. Dismissal, the question bound,
  the claimant leaving and `closing` likewise end the carrier before they close the helper.
- A helper that gives up after it handed over its question, at the eighth stranger, drops its
  connection, which the hub reads as an unanswered question.
- A record whose ticket names no running carrier is closed; such a carrier is already ending.
- A helper that cannot reach the hub, cannot read its pairing, or meets a full askpass endpoint,
  where the 17th connection is closed unread, waits for the attempt's 60-second clock, or an open
  question's bound, to end the carrier; a hub that exits ends the Job and every helper in it.
- Only when nothing ends the carrier within 250 seconds, a hub that stops working without exiting
  or a helper that cannot reach the hub after its Machine is `online`, where no attempt clock runs,
  does the helper exit 1; `ssh` may then send one empty answer.

The helper never reads its standard input, which `ssh` shares with the link, and never writes its
standard error. Its pairing with the hub is internal to this program and may change in any
release; only the `OBMA` record of the answer endpoint is frozen. A carrier whose ticket could not
be drawn does not start and reads `ssh-failed`.

Both endpoints are bound as the first instance of their name, so an existing pipe of that name is
never adopted, but the library adds a fresh instance after each accept, so the name alone is not
exclusive. What protects them: a security descriptor that grants only the current user, so only a
process of that user can add an instance or connect; no remote clients; a check, before any byte
is read, that the peer runs as the same user; for the answer endpoint, a name with the helper's
process id and 8 random bytes, the question's 16-byte token, and one record taken once. A process
of the same user is not a boundary: it could read the console or the helper's memory. A hub that
admits a report only from a process inside that carrier's Job, and a client that checks the answer
endpoint's server process against the id in its name, are not part of this build.

#### Answer path

An answer travels from the client that asked straight to the helper's one-shot endpoint in the
contract's [`OBMA` record](../../../schemas/machine/README.md#answer-delivery), and from the helper
to `ssh` over the standard output pipe `ssh` gave it. The hub learns only that a question was
answered.

| Process                                      | Holds the answer                                                                                                                 | For how long                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The console host of the terminal             | The keys typed into its line-input buffer; not echoed                                                                            | Until `ReadConsoleW` returns them; outside Orchbee's control                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `orchbee machine connect`, the asking client | The UTF-16 read buffer, the UTF-8 answer and the `OBMA` record, each in a buffer sized up front so nothing is copied as it grows | From Enter until the helper's 0x06 or its 5-second bound; each buffer is wiped then, before the next step                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| The TUI, the asking client                   | Each input read and the key or paste decoded from it; the login field's fixed buffer; the `OBMA` record                          | Wiped: the Windows reader's batch of console records once the next batch is read, each key's encoded bytes once collected, a collected burst, which never outgrows its first allocation, and a held run left over by a burst; the Unix read buffer once copied; the decoder's partial character and paste run once their item leaves, and its old allocation when one grows; a read and its decoded item of a shown login once the overlay took them; the field at Enter, when it moves unchanged to the delivery thread, which wipes it after the helper's 0x06 or its 5-second bound, and at Escape, the login's end, a dismissal or its drop. Kept through a resize or a lost focus: the field. Not wiped: the console's own input queue and the console host crate's native record array, the copies of one key on the stack and in the reader's surrogate state, CSI and SS3 bytes, which carry no typed character since Orchbee asks no terminal for keys in CSI u form, and what the terminal and the operating system hold |
| The operating system's credential store      | A remembered answer, written by the client that took it                                                                          | Until a refusal, Forget password, the Machine's removal or uninstall with `--delete-data` deletes it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| The TUI, remembering                         | A copy of a ticked answer in a second fixed buffer; the writing thread's copy                                                    | From Enter until the login ends: wiped at any end but `succeeded`, at a repeated question of its kind, and at the episode's drop; after `succeeded` it moves to a writing thread, which wipes it once the store took it. The store adapter's own copy is wiped by the crate                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| `machine connect --remember`                 | A copy of a typed password or passphrase                                                                                         | Until the login ends; written after `succeeded`, wiped either way                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| The askpass helper                           | The record's answer bytes, or a remembered answer it read, and the line it writes                                                | From reading the record until its standard output takes the line, milliseconds; wiped, and the process exits                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| `ssh`                                        | The line it reads from the helper                                                                                                | As OpenSSH handles a passphrase or password                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

The answer never passes the hub, a Session's Server or Worker, the Terminal Host or any remote
Orchbee, and is never an argument, an environment variable, a log line, a diagnostic, a persisted
record, a panic message or a debug form: the buffers that hold it have no debug form and are wiped
when dropped. The contract's token reaches the claimant in the `question` event and is never
logged; its debug form shows no digit. The asking client connects only to an endpoint whose peer
runs as its own user and waits at most 5 seconds for 0x06. The helper takes one record, for its own
token only, then drops the endpoint, so a second answer, an answer for another question, and an
answer after the question closed all meet a closed or missing endpoint.

#### Remembered answers

The client that took an answer remembers it in the operating system's credential store, after the
login ended `succeeded` and only when the person asked: the TUI's "Remember password" box, unticked
by default, or `machine connect --remember`. The hub never holds an answer and learns only, from
the helper, that a remembered one went to `ssh`; nothing falls back to a file, and the Desktop
application calls no store.

- **Items.** One item per Machine and kind, named `Orchbee/<environment>/machine/<machine id>/<kind>`:
  the environment is `orchbee` for a release and the build's `ob-dev-*` or `ob-test-*` stem
  otherwise, so builds never share items; the kind is `password` or `key-passphrase`. The item's
  readable label, `Orchbee: <machine label>` without control characters and at most 120 of its
  characters, is written once, when the answer is saved. The answer is its UTF-8 bytes.
- **Stores.** One port in the workbench Client crate selects this platform's store; every caller
  goes through it. Windows uses Credential Manager: a generic credential with local-computer
  persistence, written through the `keyring-core` and `windows-native-keyring-store` crates, since
  the workspace admits no authored `unsafe` for the Win32 calls. The store is never listed; each
  call names one item. macOS and Linux answer "unavailable" in this build: a current limit, not
  the design, which gives them the login Keychain and the Secret Service, otherwise the kernel's
  per-user keyring, as adapters of the same port that change no caller. A store that cannot be
  reached, such as Credential Manager in an SSH logon, also answers unavailable: the TUI shows
  "Passwords can't be remembered in this kind of login." in place of the box. The TUI asks the
  store, and the `ssh` release below, on a thread of its own when the first password or passphrase
  question arrives or a remote row is first selected, and shows the box once they answered. A
  ticked answer the store does not take after a login that succeeded shows "Password not saved" in
  the status line.
- **The `ssh` release.** Only an `ssh` of OpenSSH 8.5 or later is given a remembered answer: an
  older one may pass a server's keyboard-interactive prompt without the `(user@host) ` prefix, so
  a server or a jump host could ask in `ssh`'s own words for the Machine's password or passphrase.
  The hub runs `ssh -V`, or the stand-in with `-V`, once in its life for the program it carries
  with, which contacts nothing, and reads the first line of its standard error, or of its standard
  output when that is empty: `OpenSSH_<major>.<minor>`, with `for_Windows_` after the underscore in
  Microsoft's builds. A release below 8.5, or a line that names none, sends no recall. The TUI and
  `machine connect --remember` read the same line for the program the hub would run and treat the
  store as unable to remember: no box, and in its place, as for `--remember`, "Passwords can be
  remembered with OpenSSH 8.5 or later."; "Forget password" is still offered. Typed answers work as
  they do without remembering.
- **Reading.** The helper reads the item for the first password and the first key passphrase
  question of every attempt, the silent ones too: one a kept Machine starts when an interface
  attaches, and one after a lost link. It is tried once per kind per attempt.
- **Held back.** An attempt that handed a kind's remembered answer to `ssh`, or let the helper read
  it, and then ended before the link handshake, whatever ended it, holds that kind back: a server
  that closes the connection after too many tries is no refusal, and the hub's own retries would
  otherwise send the same answer again. No later attempt recalls a held kind until the Machine has
  been `online`, a person asks for the connection with origin `user`, or a person's answer is taken.
  A refused answer whose deletion fails, or whose deletion thread cannot start, is held back until
  a person logs in with answers of their own. The hub keeps only these marks, never an answer.
- **A refusal.** `ssh` asking for a password again after the remembered one, or ending with
  `Permission denied` after it, refused it. A remembered key passphrase is refused only by the next
  passphrase question in exactly the words it was given for; another key's question is asked as any
  question and deletes nothing, and an end with `Permission denied` after it deletes nothing either,
  since the server may have refused the key rather than the passphrase. The hub deletes a refused
  item on a thread of its own and never tries it again in that attempt. With a claimant the next
  question carries `remembered: rejected`, which the TUI and the console show as "The saved
  password no longer works."; without one the attempt ends `needs-login` `remembered-rejected`.
- **Forgetting.** The picker's "Forget password" on a selected remote Machine, offered while the
  store can be used, deletes both of its items without reading them; the key is taken only while
  the selected row can forget, so elsewhere it goes on to the picker's other bindings. Every
  forget tries both kinds, even when one fails, then reports. `machine remove` deletes them after
  the catalog commit; the removal succeeds either way, and standard error carries one line when
  they could not be deleted. `uninstall --delete-data` deletes the items of every Machine the
  catalog still names before it removes the data; the default uninstall keeps them, as it keeps the
  catalog. As a current limit it cannot delete an item whose Machine the catalog no longer names,
  or every item when the catalog cannot be read: such an item, left by a removal while the store
  could not be reached, stays, since the store is never listed and never searched by prefix.

### Channels

The hub relays the contract's [hub channel kinds](../../../schemas/machine/README.md#hub-channel-kinds).
Each channel a client opens is one pair: the client's channel on its connection and one link
channel on the Machine's current carrier, a link `session` for the record's Session or a link
`route`. The relay table in the hub state holds each pair: its two ends, its kind, whether the link
accepted, whether the hub has queued the opener's `end` toward the link, and the bytes each
direction holds. The current channel of a [`stop-session` run](#stopping-a-session) is a pair whose
client end is the run instead of a client's channel.

| Step                 | The hub                                                                                                                                                                                                                                                                                                                        |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `open`               | Resets `stopping` in `closing`, `not-found` for an id outside the catalog, `not-online` for a Machine that is not `online` (`unresponsive` included), and `unavailable` when the link already holds 256 channels the hub opened. Otherwise it opens the link channel at once, so the opener's early data flows before `accept` |
| Link `accept`        | Sends `accept`                                                                                                                                                                                                                                                                                                                 |
| `data`, either way   | Commits the bytes to the other side's state machine as far as that side's credit allows, takes from the sending side exactly what was committed, and keeps the rest until the other side's credit returns                                                                                                                      |
| `end`, either way    | Relayed once the bytes ahead of it are committed; the opener's `end` toward a `route` link is the dispatch                                                                                                                                                                                                                     |
| Link `close`         | Relayed with its outcome once the bytes ahead of it are committed; an outcome this build does not know relays as `lost`                                                                                                                                                                                                        |
| Link `reset`         | Passed on with its reason at once while no effect can have started: before `accept` for a `session`, before the opener's `end` was queued for a `route`; a reason the kind does not list, or one this build does not know, passes on as `unavailable`. Past that point the pair ends as at the carrier's end, `close` `lost`   |
| The client's `reset` | Resets the link channel `cancelled`, so the helper ends a routed command it runs; a `too-large` the hub's state machine sent does the same                                                                                                                                                                                     |
| The client leaves    | Resets each of its link channels `cancelled`; the Machine, its carrier and every other client's channels go on                                                                                                                                                                                                                 |
| The carrier ends     | Every pair on it ends: a `session` that was accepted closes `lost` after the bytes ahead of it, one that was not resets `link-lost`; a `route` whose opener's `end` the hub had not queued resets `link-lost`, any other closes `lost`                                                                                         |
| `closing`            | As [Lifetime](#lifetime) states; at the allowance an open route ends as at the carrier's end                                                                                                                                                                                                                                   |

- **Bounds.** A relay grants credit on one side only as the other side commits the bytes, so a pair
  holds at most one window, 262,144 bytes, per direction in the table, and the receiving side's
  state machine holds at most what that side's credit admits. A client that stops reading, or a far
  end that stops reading, holds back its own pair only: the other pairs of that client, that link
  and every other Machine keep their turns, and no queue grows with the stall.
- **A queued `end` counts as dispatched.** The hub cannot tell whether a queued `end` reached the
  helper before the link ended, so a `route` whose `end` was queued ends `lost`, which its opener
  reads as unknown. Claiming no effect then could be false; claiming an unknown one never is.
- **A far reset after dispatch is no statement of no effect.** The [link helper](#link-helper)
  never resets a begun channel, but earlier builds of it reset every open channel `stopping` when
  their link ended, a route whose command had begun included, and the hub cannot check a far end's
  no-effect claim. The hub is the one place that knows whether the opener's `end` was queued, so it
  reads a reset after that point as it reads a lost link, and its client never retries a command
  that may have run.
- **Carrier ends reach every pair.** A lost link, `disconnect`, a reload that changes or removes the
  record, and a failed attempt all end the carrier, and each ends its pairs the same way.

### Stopping a Session

`stop-session` stops the Machine's remote Session the way an admitted Client of that Session does:
with the Session's own [stop request](../../../crates/local-client-protocol/README.md#managed-session-stop-control),
the one `server stop` sends after its destructive admission and the Desktop sends after its dialog.
Each exchange is a link `session` channel for the record's Session, which the
[link helper](#link-helper) joins to the Session's Client endpoint and the Session admits by its own
same-user rule, as for any client's `session` channel. The far end gains no entry, option or
variable, runs no command, and reads no confirmation. Confirmation belongs to the asker: whoever
asks the hub on a person's behalf first applies the
[destructive-command rule](../../../docs/cli/command-surface.md) of its own build, as a local
`server stop` does and the [remote Session stop](#remote-session-stop) does; the hub carries out a
request already confirmed.

A run is the work of one `stop-session` for one Machine, and its thread performs it:

1. **Start.** The run picks a fresh stop identity, opens a link `session` channel, sends this
   build's `ClientServer` offer, and, once the Session answers, sends `start` and reads its result.
   The request is _submitted_ from the moment its first byte is committed to the link.
2. **Watch.** While the stop is pending, or a submitted `start` went unanswered, the run asks the
   stop's `status` over a new link `session` channel every 250 ms, until an answer or the end of the
   Session decides it, or until 65 seconds after the start, the default wait of `server stop`.
3. **One channel at a time.** Each exchange is bounded by 30 seconds, past the helper's 20-second
   connect window. Once it has its result, or its channel ended, the run resets the channel
   `cancelled`, which the helper reads as the client leaving.

| What the run sees                                                                                                                                                                          | Outcome                                       |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------- |
| The Machine not `online` at the request                                                                                                                                                    | `not-online`                                  |
| `start`: the channel reset `session-not-running`                                                                                                                                           | `not-running`                                 |
| `start`: answered `pending`, or submitted and then no answer                                                                                                                               | Watch                                         |
| `start`: answered `busy`, `failed`, `unmanaged` or `not-recorded`; the offer refused or the request rejected; the channel reset otherwise, or ended before submission; a link with no room | `failed`                                      |
| Watch: the channel reset `session-not-running`, or `not-recorded` after a `pending` answer                                                                                                 | `stopped`                                     |
| Watch: `not-recorded` while the `start` was never answered                                                                                                                                 | `unknown`                                     |
| Watch: answered `failed`, `busy` or `unmanaged`                                                                                                                                            | `failed`                                      |
| Watch: answered `pending`; the offer refused; the channel reset otherwise or ended without an answer; a link with no room                                                                  | Watch again                                   |
| Watch: 65 seconds passed                                                                                                                                                                   | `unknown`                                     |
| The carrier ends during the run                                                                                                                                                            | `unknown` once submitted, `not-online` before |
| `closing` reaches its allowance during the run                                                                                                                                             | `unknown` once submitted, `stopping` before   |

- **Truth about effects.** Before submission nothing reached the Session, so each failure reads as
  no effect. After it, only the Session's answer or its end decides the run, and anything else reads
  `unknown`. A far reset counts only before submission: the helper resets a `session` channel only
  before `accept`, and every answer follows `accept`.
- **The outcomes stay the contract's.** A `start` answered `busy` reads `failed`, although another
  stop may be ending the Session: the Session refused this one. The run cannot tell a Session that
  ended because of its stop from one that ended otherwise, so both read `stopped`.
- **`not-recorded` after `pending` means a later Server.** A Server keeps its stop entry until it
  ends, so a later `status` it does not know comes from another Server of the same Session, and the
  one asked has ended. Without the `pending` answer the `start` may never have reached the Server,
  or reached one that ended since; the run cannot tell which.
- **Joined requests.** A Machine has at most one run; a `stop-session` for a Machine with a run joins
  it and gets the same outcome. A client that leaves loses its replies, and its run goes on.
- **Ownership.** The hub state owns each run: its Machine and carrier, its waiting requests, and
  whether its request was submitted. The first end that decides a run answers every waiting request
  and removes it: the thread's reading, the carrier's end, or the allowance. A thread that finds its
  run gone stops. Each exchange's channel is a relay pair whose client end is the run; the thread
  commits to it and reads from it under the hub state's lock, returns credit as it reads, and waits
  outside every lock on the hub state's signal, which the carrier's reader fires after each frame.

### Environment

The hub outlives the client that started it and serves every client of its data root, so it
carries no client's route or context. A client starts the hub with its own environment, as it
starts any detached helper; at start the hub takes one view of that environment, reads every
variable from the view and nothing from the process environment afterwards:

| Variable                                                     | In the hub's view                                                                                                                                                          |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ORCHBEE_HOME`, `ORCHBEE_INSTALL_DIR`, `ORCHBEE_CONFIG_FILE` | Unchanged: they select the data root, the store, and the settings file, which every client shares                                                                          |
| `ORCHBEE_RUNTIME_BASE`                                       | Only when it applies to this build, as a Session route does; the hub reads its Sessions there                                                                              |
| `ORCHBEE_MACHINE_HUB_EXPIRY_MS`                              | Unchanged in a development or test build; never in a release                                                                                                               |
| `ORCHBEE_MACHINE_HUB_QUESTION_MS`                            | Unchanged in a development or test build; never in a release                                                                                                               |
| Every other `ORCHBEE_*`                                      | Never: `ORCHBEE_RUNTIME_SCOPE`, `ORCHBEE_ENVIRONMENT`, `ORCHBEE_SESSION`, terminal, Worker, Workspace, Tab, Panel, command, Agent, plugin, Desktop, TUI and test variables |
| Every other variable                                         | Unchanged                                                                                                                                                                  |

Names compare without letter case, as Windows does. A variable whose name or value is not valid
Unicode is left out of the view.

A [carrier child](#the-carrier-child) receives the view without any `ORCHBEE_*` variable but the
askpass pairing `ORCHBEE_MACHINE_ASKPASS`, and without the terminal's own variables `TERM`, `COLORTERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`,
`WT_SESSION` and `WT_PROFILE_ID`, since it runs in no terminal, plus only the variables its command
line sets and the [askpass settings](#the-carrier-child), which win over the view. One spawn path
clears the inherited environment first. The child has no console, piped standard streams and the
system folder as working directory. An askpass program inherits `ssh`'s whole environment, so
whatever the hub passes to `ssh` also reaches the askpass: that is the endpoint and the ticket,
never an answer. `PATH` stays as the view has it, an
[accepted limit](#accepted-limits). A process test reads what a child receives.

### Admission and requests

The endpoint admits only the current user: its DACL grants access to the hub's own token user
alone, it refuses remote clients, and every accepted connection proves that the client process's
token user equals the hub's before a byte is read, as [`local-ipc`](../../../crates/local-ipc/README.md)
does for Session endpoints. The `OBHS` frame must arrive within 2 seconds of the accept.

| Request                        | Answer                                                                                                                                                                                                                                                                                             |
| ------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `attach`                       | `attached` with the state; turning `watch` on sends `catalog`, then one `machine` per id; gaining `presence` starts a silent attempt for every `idle` Machine                                                                                                                                      |
| `reload`                       | Rereads `machines.json`: `reloaded`, or `invalid-catalog` or `storage` keeping the catalog; a change sends `catalog` and every changed `machine` to each watcher, and a changed record is reconciled as above; while `presence` holds, a new, changed or enabled SSH Machine is attempted silently |
| `connect`                      | `not-found` for an id outside the catalog; otherwise [carried](#carrying-a-machine), which answers `disabled-machine` and `unsupported-machine` at once; a `draft` is `unsupported`                                                                                                                |
| `disconnect`                   | `not-found` for an id outside the catalog, otherwise the Machine is disconnected and the answer is `disconnected`                                                                                                                                                                                  |
| `stop-session`                 | `not-found` for an id outside the catalog, `not-online` for a Machine that is not `online`, otherwise a run that [stops the Session](#stopping-a-session), or joins the Machine's run                                                                                                              |
| `claim`, `dismiss`             | As the [login episode](#login-episode) states                                                                                                                                                                                                                                                      |
| `open` of `session` or `route` | [Relayed](#channels) for an `online` Machine; otherwise `reset` `not-found` for an id outside the catalog or `not-online`                                                                                                                                                                          |

A disabled record reads `disabled`, a connection kind this build does not know `unsupported` with
cause `connection-kind`, and any other Machine starts `offline` without a cause or a next attempt.
A catalog that cannot be read when the hub starts serves as empty with revision 0 until a `reload`
succeeds.

### Ownership and bounds

| Resource                       | Owner                                                                   | Bound                                                                                                                                                                                                   |
| ------------------------------ | ----------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `hub.lock`                     | The hub process, from before its endpoint exists until after it closes  | One holder per data root                                                                                                                                                                                |
| Endpoint                       | The listener thread of the lock holder                                  | Bound only by the lock holder; a name it cannot take is retried for 2 seconds, then exit 1                                                                                                              |
| Candidate                      | Its admission thread                                                    | 2 seconds; 64 read at once, and the next gets `capacity` unread                                                                                                                                         |
| Attached connection            | Its reader and writer threads; its protocol state is under its own lock | 256 slots; the next offer gets `capacity`                                                                                                                                                               |
| Control request                | Its admission thread                                                    | 4 slots; the next gets `capacity`                                                                                                                                                                       |
| Queued frames of a connection  | Its connection state machine                                            | Its reader stops reading while 256 wait; state events replace their own subject; a writer writes at most 64 KiB of whole frames at once                                                                 |
| Catalog                        | The hub state                                                           | 64 Machines from a record of at most 128 KiB                                                                                                                                                            |
| Machine records                | The hub state; each is a state machine without a clock of its own       | One per catalog Machine; the waiting `connect` requests of a connection leave when it closes                                                                                                            |
| Carrier                        | Its owner thread, from the start of its child to the report of its end  | One per Machine; 4 KiB of standard error; the link's own window and queue bounds; its link reader stops reading while 256 frames other than `data` wait to be written                                   |
| Relay pairs                    | The relay table in the hub state, from `open` to the end of both sides  | 256 per connection and 256 per link, as each side's own channel bound; one window per direction each                                                                                                    |
| `stop-session` runs            | The hub state; each run's exchanges, its thread                         | One per Machine; one link channel at a time; the waiting requests of a connection leave when it closes                                                                                                  |
| Expiry deadline, Session reads | The supervisor                                                          | One deadline; one read at a time, of at most 256 folders, within 5 seconds                                                                                                                              |
| Machine timers                 | The supervisor, which wakes at the earliest Machine deadline            | No thread per timer                                                                                                                                                                                     |
| Askpass endpoint               | Its listener thread, from the hub's start to its close                  | `\\.\pipe\orchbee-askpass-<pid>-<random>`, bound as the first instance, the hub's user only; 16 helpers at once, each record read within 5 seconds and at most 32 KiB                                   |
| Helper connection              | Its thread, until the helper leaves or the hub closes the question      | One per carrier; a second while one is open is refused                                                                                                                                                  |
| Threads                        | The process                                                             | Listener and supervisor, the askpass listener, one per candidate, two per attached connection, four per carrier: owner, link reader, link writer, standard-error reader; one per helper and one per run |

### Locks

A terminal stream carries many frames a second on one connection while others idle, so a frame
wakes only the threads of the two sides it touches.

| Lock             | Guards                                                                                                                                                                                                                                                                                                | Its signal wakes                                               |
| ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Hub state        | The lifetime, the catalog with every Machine record, the carriers, the registry of connections with each one's `presence`, `asks`, `watch` and thread count, the open helper connections, the relay table, the `stop-session` runs, the candidate and control counts, and the catalog reads in flight | The supervisor, the closing, and each run's thread             |
| Connection state | One connection's protocol state machine and whether it is still open at the hub                                                                                                                                                                                                                       | That connection's writer, and its reader paused at the backlog |
| Link state       | One carrier's `machine-link` state machine, whether it is still open, and its `ensure-session` channel                                                                                                                                                                                                | That carrier's writer, and its reader paused at the backlog    |

- **Order.** A thread that holds the hub state's lock and a connection's or a link's lock took the
  hub state's first. A thread holding a connection's or a link's lock takes no other lock, and no
  thread holds any of them while it reads or writes a pipe, reads the catalog, reads the Sessions,
  or starts a process.
- **Links.** A carrier's reader hands a frame to its link under the link's lock alone, releases
  it, and only then gives the Machine what the frame means under the hub state's lock. The hub
  applies a Machine's effects under the hub state's lock, taking a link's lock for a channel or a
  `ping`; it starts a carrier's process outside every lock.
- **Frames.** The reader takes only its connection's lock to hand a frame to the state machine.
  A request, `bye`, or `open`, whose answer depends on the hub state, is then answered under the hub
  state's lock and that connection's lock, taken in order. An event for every watcher is queued
  under the hub state's lock, taking each watcher's lock in turn and waking only that writer.
- **Relayed frames.** A channel frame is handed to its own side's state machine first, under that
  side's lock alone; the relay then finds its pair under the hub state's lock and takes the other
  side's lock to commit, releases it, and takes the first side's lock again to take the committed
  bytes. Each step holds the hub state's lock only for a copy of at most one frame's bytes into a
  state machine, and wakes only the two writers involved.
- **Wake-ups.** A frame wakes its own writer, never every thread of the hub. The hub state's signal
  fires when the lifetime, the catalog reads in flight, the registry, or a run change, and after
  each link frame, which a run's thread may wait for.

The Windows link helper shares the carrier pieces with the hub: the lock-and-signal pair, the frame
splitting, the handshake answer and the `query` result. They are unchanged. The hub now keeps one
such pair for its state and one per connection, while the link keeps its one pair for its one
connection, so nothing changed for the link.

### Ends and races

| Event                                         | Effect                                                                                                                                                                                                                                             |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| The last `presence` connection leaves         | `held` while one Session read runs; `expiring` when it finds no live Session                                                                                                                                                                       |
| A Session ends                                | The next read, at most 30 seconds later, finds it; `expiring` when nothing else holds                                                                                                                                                              |
| The expiry fires while a request is in flight | Requests are answered under the hub state's lock, before or after the change; a `reload` reads its file outside the lock and, applied in `closing`, replies `stopping`, and the closing waits for it before `bye`                                  |
| The expiry passes with a route open           | The hub stays `expiring` with the past `expiresAt`; a route opened while the read at `expiresAt` runs keeps it so; the read due when the last route ends closes it unless it finds a live Session                                                  |
| A second hub starts                           | It cannot take the lock, writes and binds nothing, and exits 0; the client that started it attaches to the running hub                                                                                                                             |
| A client connects during `closing`            | Its offer is rejected `stopping`; attach-or-start waits and starts a new hub once the old one is gone, or fails `closing` at its deadline; an owner [re-attaching](#re-attachment) after its hub was asked to stop starts none                     |
| The hub is killed                             | The system releases the lock and closes the pipe; the next attach finds no endpoint and starts a hub, which locks the same file                                                                                                                    |
| A hub thread panics                           | The hub reports the panic and aborts the whole process, which then ends as a killed hub does; no thread unwinds alone                                                                                                                              |
| The data root is unavailable                  | Attach-or-start fails before starting a hub, or the hub exits 1 when it cannot create `internal/machines/` or open the lock; a running hub keeps its catalog, answers `reload` `storage`, finds no Session, and expires unless `presence` holds it |
| A carrier ends while the hub ends it          | The owner reports one end; the Machine has moved on to another carrier or to `idle`, so the report changes nothing                                                                                                                                 |
| A carrier is ended before its child exists    | Its handle is marked ending; the owner terminates the child the moment it registers it, or a start still queued is dropped                                                                                                                         |
| The hub is killed with carriers running       | The system closes the hub's Job handles and every Job ends its processes, so no carrier outlives the hub                                                                                                                                           |
| The hub closes with carriers running          | After the writers' 2 seconds each carrier gets `bye` `stopping` and is ended; the hub waits up to 10 seconds for their owners, and its exit ends any Job still open                                                                                |

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
  lifetime acquire and release, and the read holds no file a Server writes or replaces. What
  remains is the probe's own instant, which every Session operation outlasts: an activation or a
  stop that meets the activation lock retries 10 ms later, and deletion's activation and lifetime
  takes and its removals try again for up to half a second, the
  [momentary hold](../../../crates/local-runtime/README.md#durable-session-catalog) every Session
  writer allows a reader. Only a machine that stalls a probe past that bound refuses a deletion, an
  accepted limit. A Server still starting when its activator gave up at its deadline takes its
  lifetime lock without the activation lock; a read at that instant is the one meeting left open.
- **Admission beside closing.** A candidate's slot and decision are taken together under the hub
  state's lock, so an accepted connection is registered before `closing` can begin and receives its
  `bye`, and an offer decided after it is rejected `stopping`.
- **Expiry beside attachment.** The read at `expiresAt` runs outside the hub state's lock and
  applies under it, as every read does: an attachment with `presence` before it applies returns the
  hub to `held` and the read changes nothing; one after `closing` is answered `stopping`.

### Accepted limits

- **A Session read that keeps failing finds no Session.** A runtime base with more than 256 Session
  folders, or one the hub cannot list, fails every read, so the hub, and any Machine login it holds
  once it can connect, can expire under a live Session.
- **A Session outside the runtime base is not read.** A Session whose record names a runtime folder
  elsewhere, started under another `ORCHBEE_RUNTIME_BASE`, never holds this hub.
- **A deletion that meets a stalled probe stops.** When a probe holds a Session's activation lock,
  runtime folder or one of its lock files for longer than deletion's half-second retry, as only a
  stalled machine would, `session delete` is refused `in-use` before it marks the Session
  `deleting`, or its removal fails after the mark and the Session stays marked `deleting`; running
  `session delete` again completes it.
- **A Server that outlives its activator's deadline** takes its lifetime lock without the
  activation lock, so a read at that instant is the one meeting with a Server left open.
- **The incompatible-hub check errs toward holding.** No record shows an incompatible client a
  hub's `presence` attachments, so every process that could be a client of that version counts as
  a holder, as do a process whose program file cannot be read and a Session read that cannot
  finish, and the hub stays; the person closes that program, or uninstall ends the hub.
- **A carrier child keeps the hub's `PATH`.** `ssh` reads its own configuration, and an askpass,
  `ProxyCommand` or `Match exec` program it starts finds programs through `PATH`, so the child
  keeps the `PATH` of the client that started the hub, which may be a terminal's or an Agent's.
  Replacing it would break those configurations; a `PATH` that changes after the hub started does
  not reach later carriers.
- **An unresponsive link stays until something ends it.** A Machine that stops answering `ping`
  reads `unresponsive` and keeps its carrier; the helper's 90-second idle limit or the end of the
  `ssh` process ends the link, which then reconnects.
- **A remote stop reads the Session's end, not its receipt.** The hub cannot read the far
  activation record that `server stop` reads for its receipt, so a Session that ends after the
  request was submitted reads `stopped` whatever ended it, and a stop still pending after 65
  seconds reads `unknown`, although it may still finish.
- **A reconnect can start the Session again.** A carrier that ends and reconnects during or after
  a stop sends `ensure-session`, which starts the record's Session again.
- **The `ssh` phrases are one version's.** The classification reads OpenSSH 9.5's standard error
  wording and the prompts M0 recorded; another wording reads `ssh-failed`, and a prompt it does not
  recognize is asked as `other`.
- **Only a claimant is asked.** A silent attempt that meets a new host, or a password or key
  passphrase with nothing usable remembered, ends at that question, so an interface or a terminal
  must ask once per hub life of the link unless the person chose to
  [remember](#remembered-answers) the answer. No real `ssh` has run these questions yet: the first
  real run checks every kind against OpenSSH.
- **A remembered answer's deletion runs on.** The hub deletes a refused remembered answer on a
  thread of its own; an attempt the person starts at once may still read it, and is refused once
  more.
- **Holds last one hub life.** The marks that [hold back](#remembered-answers) a remembered answer
  live in the hub's memory, since keeping them would need a new record; a hub that starts again
  may recall a held answer once more in its first attempt.
- **The same key through a jump host.** With `ProxyJump` each hop's `ssh` asks for the key's
  passphrase in the same words, so the second hop's question reads as a refusal of the remembered
  passphrase: it is deleted and the person is told the saved password no longer works.
- **A question's prompt is `ssh`'s.** A one-time code's prompt comes from the server and is shown
  as `ssh` passed it, its control characters stripped; it may name an account or a host.
- **A remote surface stays disconnected after the TUI's owner ended.** A Session carrier serves the
  owner it was created with. When the TUI holder's owner ends and the holder opens a new one, an
  open remote surface keeps reading disconnected until the Machine is selected again. A loss the
  owner re-attaches heals by itself, since the carrier keeps its owner.

### Client owner

One owner reaches the hub for every client: the diagnostic entries, `machine connect`, routed
commands, the remote Session stop, uninstall and the [TUI](#persistent-tui-connections) today,
the Desktop's Client in a later slice. It is the workbench Client crate's `machine_hub` module,
on Windows only. Two rules place it there. An internal crate never depends on the application
target ([Rust](../../../docs/RUST.md#placement-and-workspace-ownership)), so the TUI's crate cannot
reach code under the application's `cli`. Reusable code goes to its nearest semantic owner
([module boundaries](../../../docs/MODULE-BOUNDARIES.md#semantic-module-design)): the workbench
Client crate is the lowest crate that both the CLI and the TUI reach, and it already owns the
saved-machine catalog the hub serves. The hub process stays in the application, as a Server stays
apart from its clients. No second client loop exists: the state printer is a caller of the owner.

| Operation                         | Effect                                                                                                                                                                                 |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `open(attachment, deadline)`      | Attaches or starts with the caller's `presence`, `asks` and `watch`; returns the owner, the hub's state, and the hubs it started                                                       |
| `request(request, deadline)`      | Sends one request and waits for its reply; one that cannot be sent, such as one too large for a frame, fails at once; an `attach` also becomes what a re-attachment restores           |
| `next(deadline)`                  | The next notice: a hub event, a re-attachment, or the owner's end                                                                                                                      |
| `open_channel(channel, deadline)` | Opens a hub channel on the attached connection, waiting for one while re-attaching; returns its handle                                                                                 |
| `write(handle, bytes, deadline)`  | Commits as many leading bytes as the channel's credit admits and returns their count; waits for credit until the deadline when none is left                                            |
| `end(handle)`, `cancel(handle)`   | Sends `end`, or `reset` `cancelled` and forgets the channel                                                                                                                            |
| `receive(handle, deadline)`       | The channel's next delivery, in order: `accepted`, data, `ended`, `closed` with its outcome, `reset` with its reason, or `lost`; credit returns only as the caller takes data          |
| `close`                           | Sends `bye` `done`, cancels a re-attachment in progress, which then starts no hub, gives `bye` and the threads at most 2 seconds, and ends the owner; dropping the owner does this too |
| `running(root, deadline)`         | The running hub's process id through `query`; starts nothing                                                                                                                           |
| `end(root, deadline)`             | [Ends](#ending-a-hub) the running hub through `drain` and `exit`                                                                                                                       |

#### Attach or start

A call resolves the data root, creates `internal/machines/` privately, names the endpoint, and
within its deadline connects and offers `machine-hub`.

| Answer                                                           | The call                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| ---------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No endpoint                                                      | No hub runs: it starts its own executable with `--machine-hub-v1` detached, as a Session Server is started, with no inherited handle, the system folder as working directory, its own environment, which the hub narrows to [its view](#environment), and a new process group. A hub it started that exits 0 lost the lock to another hub: the call connects again and starts one more only after a pause of 200 ms that doubles with each loss up to 1.6 seconds, at most eight starts per call. A hub it started that exits 1 ends the call at once, `unavailable` |
| `capacity`                                                       | Every slot of its kind is taken: it tries again every 20 ms for at most 1 second from its first `capacity` answer, then fails `unavailable`. Every client retries `capacity` this way, for an offer and a control request alike                                                                                                                                                                                                                                                                                                                                      |
| `stopping`, or an attachment answered `stopping`                 | A hub is closing: it tries again after a pause of 20 ms that doubles up to 250 ms, and starts a new hub once the endpoint is gone. When its deadline passes with the hub still closing, it fails `closing`, never `timeout`                                                                                                                                                                                                                                                                                                                                          |
| Endpoint refused, or another user's server                       | `denied` at once: the DACL or an elevated creator's integrity level refused this process, or the server runs as another user, so no retry can succeed                                                                                                                                                                                                                                                                                                                                                                                                                |
| `initiator-too-old`, `initiator-too-new`, `fingerprint-mismatch` | Another version of this environment's hub: [replaced](#incompatible-hubs) when nothing holds it, otherwise `incompatible` naming its version                                                                                                                                                                                                                                                                                                                                                                                                                         |
| `environment-mismatch`                                           | `incompatible`: a build of another environment serves this data root, and it refuses control requests too, so nothing here can end it                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Any other rejection or answer                                    | `incompatible`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| The deadline passes otherwise                                    | `timeout`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

The hub belongs to no Session's Server and outlives its starter.

**Closing within the caller's deadline.** A hub in `closing` normally exits within milliseconds. It
lasts up to the 60-second allowance only while a catalog read it had started or a `route` channel
is still running.
An attachment waits within its caller's deadline, 10 seconds for the diagnostic entries, and not
the whole allowance. The call attaches to the successor the moment the old hub is gone, so a longer
wait would only delay a failure. Only the caller knows how long a person or a task can wait. The
result names the real cause, `closing` rather than `timeout`, so the person knows that trying again
will work. The owner's re-attachment, which no person waits on, covers the whole allowance.

#### Re-attachment

After `bye` `idle` or a lost connection, the owner re-attaches by the same rules, starting a hub
when none runs, and restores the caller's last `presence`, `asks` and `watch`.

- **A stopped hub stays stopped.** A hub that sends `bye` `stopping` or a `closing` state event,
  or answers an attachment or the re-attachment `stopping`, was asked to stop or expired with
  nothing holding it. The owner never starts a hub in reaction: it waits for that hub to go and
  attaches to one somebody else starts. Ten seconds after the endpoint is gone it ends
  `closing`; a hub of another version that appears is reported `incompatible` and left running.
  A client that opens a new owner afterwards keeps the rule: the [TUI](#the-tui-as-a-hub-client)
  starts the next hub only on a person's act, and uninstall relies on that.
- **Bound.** One attempt has 10 seconds. A hub that is stopping is waited for until 75 seconds
  after the loss: the allowance, the writers' bound, and a margin. A fifth loss within 60 seconds
  ends the owner `unavailable` instead, so a hub that drops every connection cannot keep it
  looping.
- **Telling the caller.** Once attached again, the next notice is `reattached` with the new hub's
  state. Everything the caller read before must be read again: the hub state, the catalog and each
  Machine. A watcher receives `catalog` and every `machine` again from the new hub. A request in
  flight at the loss fails `lost`, since the old hub may or may not have acted on it. A request made
  while re-attaching waits for the new connection within its own deadline. Every channel of the
  lost connection delivers `lost` as its last delivery, which its opener reads as an unknown effect
  unless its kind says otherwise; a new channel waits for the new connection.
- **Ending.** When re-attachment fails, the owner's last notice is `ended` with the failure, and
  every later request fails with it. A protocol violation by either side, or `bye` `violation`,
  ends the owner at once as `lost` without re-attaching: the next connection would fail the same
  way. `close` ends a re-attachment in progress: its connection attempt ends at once, and it starts
  no hub.
- **Pace.** Events wait in the owner for `next`. While 256 wait, the owner stops reading, so a
  watcher that never calls `next` also stops its requests' replies; the hub's own queue holds the
  rest under its bound.

#### Incompatible hubs

A hub of another version of this environment refuses the offer but answers control requests. The
owner replaces it only when nothing holds it:

1. `query` proves the endpoint is a `machine-hub` of this environment and gives its process id.
2. The holder check reads every Session as the hub does, in the same runtime base; a read that
   cannot finish counts as a holder. It also scans the processes whose program file name equals
   the hub's: each may be a client of that version attached with `presence`, so it counts as a
   holder, except the hub, the caller and the caller's ancestors, a launcher in a store's `bin\`
   folder, which only waits for the client it started, and any process running the caller's own
   program file. A process whose program file cannot be read, or a scan that fails, counts as a
   holder.
3. With no holder, the owner sends `drain` once and continues attach or start: `stopping`, then no
   endpoint, then a start of its own version's hub. With a holder, it fails `incompatible` with the
   running version and leaves the hub alone.

#### Ending a hub

`end` ends a hub on purpose and waits for its process; uninstall uses it. The replacement above
sends one `drain` and lets attach or start wait out the closing. Nothing ends a hub with holders
unless the person asked for that.

1. `query` gives the process id; no endpoint means no hub runs.
2. The owner opens the process for waiting, then queries again: the same id proves that the handle
   is the hub's and not a reused id's. A process already gone is queried again; one this process
   may not wait for, such as an elevated hub, fails `denied` at once.
3. `drain` puts the hub in `closing`. The owner waits up to 10 seconds for the process to exit,
   then sends `exit` and waits up to 5 seconds more.
4. It queries once more: a hub that a client started meanwhile is ended the same way, within the
   caller's deadline.

When `end` returns success, the hub's program file is free. An owner whose hub `end` drains starts
none in reaction, so a hub found afterwards was started by a client that was starting anyway. A
hub of another environment refuses `drain` and `exit`; `end` reports it and leaves it.

`orchbee uninstall` treats the hub of its data root as a background process kind, ended after the
Sessions; an interface still running starts no hub in reaction to that ending. Its listing names
the hub's process id and says that every machine login the hub holds ends with it; the run ends it
with `end`, and the store wait then covers its program file and any hub a client started anyway.
The uninstall document is unchanged: the hub has no member of its own, and a hub that will not end
stops the run at the store wait, which names it.

The exercise in `tests/cli_windows_machine_hub.rs` starts eight attachments at once on a fresh
data root, follows `held`, `expiring` and `closing` with a 20-second expiry, keeps a hub `held`
by a real scratch Session through its stop and a new start, reads a catalog of two machines, and
reloads it. It also kills the hub under a holder and finds the holder's `presence` restored on a
new hub, drains the hub under a holder and finds that the holder starts none, retries a full hub
until a slot frees, reports `closing` for a hub that stays closing, and has uninstall's background
step end a hub a Session holds. Through the stand-in, which runs the Client under test as the far
end against a real scratch Session, it connects a Machine with `machine connect`, kills the one
carrier child and finds the Machine reconnecting and then online again with one child, removes a
connected Machine with `machine remove` and finds its child gone and the hub listing no Machine,
ends the hub and finds the child gone, reads the variables a child receives, and finds a far end
that closes at once reported as the Machine's state. Through the same stand-in it runs routed
commands with `--machine` and compares each answer with the same command on the far side, finds
three routed commands on one child, opens a Session stream through the hub and reads the far
Session's handshake answer, ends the child under a routed command in flight and finds a definite
unknown-outcome failure within a bound, leaves a client mid-stream and finds the hub and the
Machine still serving, and stops the far Session with `stop-session` as a Client of that Session,
which a second request then reads as `not-running`. It starts no `ssh`. The time rules of closing
with channels open, of the countdown a `connect` restarts and no route moves, and of an expiry held
off by open routes are unit tests over the injected clock, as are the reading of a far reset by
whether an effect may have started, the reading of each end of a run's exchanges as its outcome,
and the kind and exit code of every routed failure.

## Persistent TUI connections

The Windows TUI discovers the saved catalog while open and offers a machine
picker, opened with `Ctrl+b Shift+M`. There is no permanent footer indicator. Selection is by
profile identity, not row index or label. Local remains
available. A disabled, removed, ambiguous, or corrupt entry cannot silently
become another machine. Enabled machines the hub reads `online` are observed for read-only
discovery; only an explicit selection opens a terminal surface. Saved enabled state is never online
evidence.
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

### The TUI as a hub client

The TUI reaches saved machines only through the [machine hub](#machine-hub), as one more client of
the [client owner](#client-owner). It starts no `ssh`, runs no remote program and probes nothing:
the hub's carrier is the one connection to each Machine, which the picker, discovery, remote
terminals, Workspace switches, Agents mode and every `orchbee --machine` command share.

| Rule            | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Presence        | The first catalog read that holds an enabled Machine of a known connection kind starts one background holder. It attaches with `presence` and `watch`, starting a hub when none runs, and keeps that attachment, across the owner's re-attachments, until the TUI exits or its hub is asked to stop. The hub's countdown begins only when its last holder leaves, so a TUI opened again within the expiry finds each Machine as the hub kept it: `online` at once, on the same carrier child. A TUI whose catalog never holds such a Machine starts no hub                                                                                                                                                                                                                                               |
| Questions       | The holder attaches with `asks` true and a selection sends `connect` with `origin` `user`, so the hub puts that login's questions to this TUI, as [login questions](#login-questions) shows them. The hub also offers this TUI every `command` episode, which it shows as a [notice](#login-questions) and claims when the person accepts it; the claim, a dismissal and a release ride on the holder's own requests, off the frame                                                                                                                                                                                                                                                                                                                                                                      |
| Off the frame   | Attaching, the hub's events, every request and every channel run on background threads that publish into one shared view and wake the UI loop. A frame reads that view and never waits for the hub                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Row words       | A row shows the hub's state of the Machine only while the holder's owner holds a live connection and the hub has not said it is closing. Otherwise it reads what the holder knows of the hub: Unknown while it attaches or re-attaches, or while the hub has not reported the Machine; Stopped; or Unavailable. No row reads Online without a live attachment, and the holder notices a lost connection within 100 ms                                                                                                                                                                                                                                                                                                                                                                                    |
| Hub stopped     | A hub that says it is closing, or that an attachment finds closing, was asked to stop, by a person or by uninstall, or expired with nothing holding it. The holder starts no hub in its place: its rows read Stopped, the picker's status line says machine connections stopped and that choosing a Machine starts them, and Local remains available. The holder joins a hub somebody else starts, which it looks for once a second, and a selection, a person's act, starts one                                                                                                                                                                                                                                                                                                                         |
| Hub crashed     | A connection lost without `closing` is the owner's to re-attach, starting a hub when none runs. When the owner ends, or no hub could start, the rows read Unavailable and the picker's status line says machine connections are unavailable; the holder starts a hub again under the [retry policy](#carrying-a-machine), but none in place of a hub that answers it is stopping. A selection skips the pause                                                                                                                                                                                                                                                                                                                                                                                            |
| Lasting failure | A failure that trying again cannot cure is not retried on a timer: a hub of another version that something still holds, another environment's hub, another user's endpoint, a protocol violation, or a data root that cannot be used. The rows read Unavailable with the error tone, the picker's status line says machine connections cannot start, and the selected row's details say why and what to do, naming the held version. The holder waits on that hub's process, or, when it cannot open the process, looks at the endpoint once a second, reading a missing name or a hub that names another process to `query` as a later hub, and attaches again once the hub is gone; a selection tries once more at once. A data root that cannot be used names no hub, so only a selection tries again |
| Exit            | Quitting asks the holder to stop and does not wait for it, so the terminal is restored at once. An attachment in progress ends at once and starts no hub; the holder's thread writes nothing to the terminal, holds nothing that keeps the process alive, and ends with the process                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Catalog         | The TUI rereads `machines.json` every second. When the revision it read differs from the hub's last `catalog` revision, the holder asks the hub to `reload`, once per revision, so a Machine saved, changed or removed while the TUI runs reaches the hub                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Re-attachment   | After `reattached` the view forgets every Machine state until the new hub reports it again; the rows read Unknown meanwhile                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

### Login questions

The TUI asks a person exactly what `machine connect` asks on a console, from the same wording and
`ssh`'s prompt shown beside it, escaped and bounded. The episode's client-side rules are one model
in the workbench Client crate, apart from the overlay that draws it, so another interface reuses
them; the holder brings the hub's events and carries out the model's deliveries and dismissals on
threads of their own.

| Rule           | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| One overlay    | A question of the episode this TUI claimed opens the login overlay over everything, the picker included: the Machine and its target, `ssh`'s prompt, Orchbee's line for the kind, one field, Cancel and the primary action. The rest of the interface keeps running under it                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Notice         | A `command` episode the hub offers shows first as a notice, never as the field, since the person may be typing in a Panel: the Machine and its target, how many commands wait to log in to it, that they fail without a login, and `Esc` Not now and `Enter` Log in. It is a capturing overlay, the one place where a background request takes the keyboard, so every other key and a paste are discarded rather than reaching a Panel or a field; `Enter`, `Esc` and its buttons act only once frames have shown the whole notice for one second in which no key or paste was discarded: every discarded key or paste, and a resize, starts that second again, so a person who is typing, for a Panel or for the question they expect, never accepts or declines it. `Enter` claims the login, and the login overlay opens with the question the hub then puts; `Esc` dismisses it, which fails every waiting command, closes the notice in every interface and mutes the Machine's command logins until the person selects it or every asking interface has closed. The count follows the hub's `waiters` |
| Queue          | One notice or login overlay at a time: the lowest login first, and a notice waits while a login overlay is open. A notice nobody here touched closes by itself when another interface claims or dismisses its login or the login ends, its last command having left; one this TUI claimed stays and completes even when every command left. Once the person pressed `Enter` on the notice a frame showed, only the person closes it, even when the hub's news of another claim arrived meanwhile: a login another interface claimed first, that the hub ended, or whose link was lost before its question opened turns the notice into an ended one that says why in a few words, still discards every key and paste, and closes on `Enter` or `Esc` under the same one-second rule. An offer for a Machine this TUI has not read yet is kept unseen and looked up again on every tick: it shows as soon as the catalog names the Machine, and goes only when another interface claims or dismisses the login or it ends                                                                                    |
| Deferred       | While another overlay, the picker, a menu, a mode, copy mode, a native capture or the armed prefix holds the keyboard, the notice is not shown and takes no key: the status line, the picker's own bottom row while it is open, or the Settings page's status row while no Settings notice leads, says "devbox needs login" instead, and the notice opens once the keyboard is free. An open action menu, and the indicator of resize mode, navigation or the prefix, still take that row without the sign                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Kinds          | A password, a key passphrase and `other` ask for a secret. A new host shows its algorithm and fingerprint and asks for a typed `yes` before Trust and continue, and Orchbee then answers `yes` itself. The client reads the kind again against the Machine's target, so a question naming another host is `other` and a changed host key never offers trust                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Again and next | A refused answer comes back as the next question with `repeated`: the overlay says so and asks again with an empty field. Several questions of one login follow each other in the same overlay. An answer `ssh` could not take is asked again at once; one that reached no endpoint dismisses the login, and its overlay and the selection say so                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Remember       | A password or key passphrase question shows a "Remember password" box under the field, unticked, which Tab reaches and Shift+Tab leaves, Space or a click toggles; while it has the keyboard, typed characters do not reach the field. A ticked box keeps a copy of the answer until the login ends and saves it after `succeeded`, as [remembered answers](#remembered-answers) say; the privacy line then says where the answer goes. Where no store can be used one secondary line says so in place of the box. A question marked `remembered: rejected` says "The saved password no longer works."                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Cancel         | Escape, Cancel or the close control dismisses the login, cancels its selection and closes the picker under it, leaving the person where they were; nothing asks again by itself                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Awaited only   | Only the selection the person still waits on, or a `command` login the person accepted from its notice, may ask. A selection that ends any other way, such as by choosing another Machine or letting the picker go before the question came, abandons its login: the contract has no request that cancels a waiting `connect`, so its attempt runs on, and the login the hub claims for it at the first question is dismissed on arrival and never shown, as is a login of any Machine the person does not wait on; that Machine reads `needs-login`. A shown login is never replaced: while one overlay is open, another login is dismissed on arrival                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Ending alone   | A login that ends without the person keeps its overlay: the hub ended it, the link was lost, the answer reached no endpoint, or its selection ended. The field is wiped and gone, the overlay says how the login ended in the words the picker uses for that end, and it keeps every key and press, discarding all but Escape and Enter, which close it onto the state that end left. Only a login that comes online after the person's own answer closes at once: keys read after that Enter and before the close reach nothing                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| On screen      | An answer can be given only once a frame has shown the whole question: Orchbee's asking sentence, `ssh`'s asking line for a question only `ssh` words, a new host's whole fingerprint, the field and the buttons. `ssh`'s lines are kept from the last one up, a `ssh: …` line marking the cut; a fingerprint wraps on lines of its own, never shortened, and widens the overlay where the terminal allows. A window that cannot hold all that shows the size notice instead: the overlay keeps every key, takes only Escape, and keeps the typed field, which shows again once the window grows. A resize counts the question unshown until the next frame shows it                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Clock          | While a login is claimed the selection's 60 seconds stop, as the hub's attempt clock does. A selection that ends without `online` because of its login says how: login failed, the host key changed, the login was not completed, or the answer did not reach `ssh`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Picker states  | `needs-login`, `login-failed`, `untrusted` and `key-changed` rows explain themselves in the selected row's details: why, the offered key where there is one, and what to do; a changed key names `ssh-keygen -R` with the host, as `"[host]:port"` for a port other than 22                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Lost link      | When the current remote Machine's hub state needs a login again, its pane carries "connection lost · Enter to log in again", and Enter starts the same selection, which connects again even where the surface would otherwise be reused                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| The secret     | Typed and pasted characters go straight into the field's fixed buffer, which never grows and is shared, not copied, by every copy of the view. The overlay paints one mask glyph per character, except that a new host's field shows what was typed while it begins `yes`, ignoring case, and the field's debug form shows nothing. Enter moves the buffer to the delivery thread; Escape, the login's end, a dismissal and dropping it wipe it, and a resize or a lost focus keeps it. Enter on an empty password or passphrase sends nothing; another question may be answered empty. Keys that arrive after Enter do not join the answer. What else is wiped, and what is not, is in the [answer path](#answer-path)                                                                                                                                                                                                                                                                                                                                                                                     |

### Session carrier

A remote terminal, a Workspace switch to another machine and Agents mode run their ordinary Client
connection on a private, same-user local pipe endpoint that the TUI creates for each observed
Machine. Each physical connection the endpoint accepts opens one hub `session` channel to the
Machine and copies bytes both ways on two threads of its own:

- **Up.** The pipe's bytes are committed to the channel as its credit admits; the pipe's end sends
  `end`.
- **Down.** The channel's data is written to the pipe as it is taken, which returns its credit, so
  a Client that stops reading pauses only its own channel. The channel's `close`, any `reset`, or
  `lost` closes the pipe.
- **Admission.** The Client sends this build's own `OBHS` offer through the stream, and the remote
  Session admits or refuses it exactly as it would a local Client, so machines with different
  product versions connect whenever their generations overlap. The carrier judges no byte, and
  neither end fabricates an accepted generation.
- **Bounds.** Logical lanes share one admitted connection; the Client actor's reconnects after a
  loss open new channels. An endpoint carries at most 16 connections and closes a 17th at once.
  While the hub does not read the Machine `online`, the endpoint closes each new connection without
  opening a channel, so a Client that keeps reconnecting to a lost Machine costs no hub request.
- **Ending.** A guard retained by every Client origin derived from the endpoint owns its stop flag
  and accept thread; the last release stops them. The accept thread ends within 10 ms, and each
  connection's threads see the flag within 100 ms, reset their channel `cancelled`, and close their
  pipe. Ending a Client connection never stops the remote Session or its terminal processes.

Losing the link or the hub closes every channel, which closes every carried pipe: the remote surface
ends as a lost Session connection ends, with a definite state, and the Client's reconnects find the
endpoint closed until the Machine is `online` again. Local Panels use their own connection and are
untouched. Each connection's threads, credit and pipe are its own, and the UI loop never reads a
channel, so one slow remote never stalls input or drawing for local work or another Machine.

The Client connection actor owns replacement admission after EOF and never replays submitted
frames. Each Workbench Client surface holds one claim on its carrier, and only the last released
claim performs the shutdown. The observer promotes its initial admission before cloning any
retained or published origin; secondary query/control lanes are logical scopes on that carrier.
Buffers are bounded and backpressure pauses reads. No port forwarding, host-key acceptance,
remote installation or POSIX implementation is added.

A refusal that needs the user to change one side is a typed `Refusal` that discovery keeps with the
Machine, so selection names it instead of the generic failure, the row shows the side's state word,
and automatic restarts never repeat it:

| Handshake answer                                       | Row            | Picker message                                                     | Sidebar notice                        |
| ------------------------------------------------------ | -------------- | ------------------------------------------------------------------ | ------------------------------------- |
| `environment-mismatch`                                 | Incompatible   | Another environment; use release on both or the same checkout path | Remote is another environment         |
| `initiator-too-old`                                    | Local outdated | This Orchbee is too old; update it on this computer                | Update Orchbee on this computer       |
| `initiator-too-new`, or a Session before the handshake | Needs update   | The remote Session is too old; update it and stop it               | Remote Session is too old             |
| `fingerprint-mismatch`                                 | Needs update   | Another development build; stop the remote Session                 | Remote runs another development build |

Capacity, stopping, and timeout refusals stay transient: the observer starts again under the retry
policy.

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

The hub's process target opens a Session stream through the hub and reads the far Session's
handshake answer. Picker identity, stale-result traces and rendered keyboard/mouse paths are unit
traces. Local fixtures, the stand-in far end among them, do not establish real SSH network
survival, remote installation, or complete two-machine acceptance.

## Automatic machine discovery

The running Windows TUI reconciles enabled profiles by ID, connection and Session; a connection
kind this build does not know is never observed, and the picker lists that machine as Unsupported
without offering it as a choice. Each endpoint generation owns at most one observer, cancellation
flag and latest-event slot. Disabled, removed and replaced entries cancel their observers, and
retired threads are joined before their slots are reused. Corrupt catalog input suspends all
discovery without changing the current terminal surface. Renaming preserves connection identity.
No observer creates a terminal, claims control, changes focus, starts a Session or installs
software.

### State words

A remote row shows one word, the hub's state of the Machine; every Orchbee interface uses the same
words. Details and actions per state come with the machine status screens.

| Hub state                              | Word (en-US / zh-CN)        | Tone        |
| -------------------------------------- | --------------------------- | ----------- |
| `online`                               | Online / 在线               | Success     |
| `connecting`                           | Connecting / 连接中         | In progress |
| `reconnecting`                         | Reconnecting / 重连中       | In progress |
| `offline`                              | Offline / 离线              | Neutral     |
| `needs-login`                          | Needs login / 需要登录      | Neutral     |
| `unresponsive`                         | Unresponsive / 无响应       | Warning     |
| `untrusted`                            | Untrusted / 未信任          | Warning     |
| `not-installed`                        | Not installed / 未安装      | Warning     |
| `needs-update`                         | Needs update / 需更新       | Warning     |
| `local-needs-update`                   | Local outdated / 本机需更新 | Warning     |
| `login-failed`                         | Login failed / 登录失败     | Error       |
| `key-changed`                          | Key changed / 密钥已变      | Error       |
| `unsupported`                          | Unsupported / 不支持        | Error       |
| `incompatible`                         | Incompatible / 不兼容       | Error       |
| `unknown`, or a state this build lacks | Unknown / 未知              | Neutral     |

When the hub's state of a Machine does not hold, as [row words](#the-tui-as-a-hub-client) set out,
the row reads the holder's knowledge instead:

| The holder                                              | Word (en-US / zh-CN) | Tone    |
| ------------------------------------------------------- | -------------------- | ------- |
| Attaches or re-attaches, or the hub has not reported it | Unknown / 未知       | Neutral |
| Found the hub asked to stop, or closing                 | Stopped / 已停止     | Neutral |
| Reaches no hub for now and tries again                  | Unavailable / 不可用 | Warning |
| Cannot use a hub until a cause outside it goes          | Unavailable / 不可用 | Error   |

A disabled record reads Disabled / 已停用 from the catalog, and a connection kind this build does
not know reads Unsupported / 不支持, without asking the hub; the CLI uses the same words. Installing
arrives with remote installation. The former Waiting, Needs attention and Stale words are retired.

### Observers

For each Machine the hub reads `online`, one observer connects through the
[Session carrier](#session-carrier) and keeps one Workbench state subscription. It accepts the
initial correlated snapshot and subsequent pushed values; it does not reread every two seconds.
The bounded summary retains Session/revision, project/workspace/Tab counts, at most eight preview
labels of 80 scalars each, and the context identities and labels used by workspace navigation.
A summary supplies the Machine's Workspaces only while the shared connection's application-progress
deadline remains current.
The [carrier contract](../../orchbee-server/README.md#multiplexed-client-delivery) owns the single
five-second-idle/two-second-total read probe and its resource and correlation bounds. Transport
privately generates a validated nonzero 32-digit lowercase hexadecimal TerminalId using fallible
OS entropy solely for this read-only TerminalLaunchRead; it allocates no machine profile or Session
identity. Probe failure withdraws freshness without proving that the Server has died. An expired
progress deadline cannot supply a reusable connection. Gap/Disconnected withdraw the
summary and retain the same observer while the actor rebuilds its subscription; only a fresh
snapshot restores it.

Workspace navigation and its count use canonical ordered Workspaces, including empty Workspaces.
Local sidebar checkpoints and remote summaries preserve those same Workspace identities.

- **Start and end.** An observer starts when the hub reads the Machine `online` and ends when the
  hub reads anything else, or when the record is disabled, removed or replaced. The hub owns every
  connection attempt and its retries; an observer never brings a Machine `online`.
- **Failure.** An observer that fails while the hub still reads `online` starts again under the
  [retry policy](#carrying-a-machine). A typed refusal, a denied snapshot, a malformed answer and a
  terminal carrier rejection hold the Machine without automatic restarts until a selection asks
  again or the hub reads it `online` anew.
- **Fences.** Catalog generations fence all results, including a removed/re-added ID; a one-slot
  mailbox coalesces observations without an unbounded history. An admitted observer retains its
  carrier through failure delivery; the UI clears its cached origin and both collectors retire
  their cached sources/subscriptions before acknowledging retirement. The observer clears its
  mailbox and releases its last carrier guard on its own thread. The catalog's 64 records bound the
  observers; shutdown cancels and joins every observer, and asks the holder to stop without
  joining it.

### Selection

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
selection of a Machine the hub does not read `online` asks the hub to `connect` it, with origin
`user`, so the login's questions come to this TUI; the request joins the hub's attempt or starts
one at once. While the holder reaches no hub, the selection asks it for one, a
person's act that starts a hub even after a stop, and waits for it within the same bound.
Selection waits at most 60 seconds for the Machine `online` and a
fresh observer, reuses the observer's admitted origin on a logical lane of the same carrier, and
commits only a drawable candidate. Time a login question waits for the person does not count
against those 60 seconds. A `connect` that ends without `online` fails the selection at once with
the generic failure, or with how the login ended when the login decided it; an attachment the selection asked for that fails, or a lasting
failure, fails it with the hub message. The row keeps its word, and a picker still open stays on
screen in a new episode with the reason.
Selection
failure, expiry or Escape preserves the current surface. Escape cancels the selection, while
discovery continues. Retry applies only to observations, never submitted terminal input or Client
mutations. The workspace mode below provides combined navigation, and Agents mode adds physical
Agent navigation. Source-owned merged Agent views are described below. Real-network authentication
and longevity acceptance remain separate obligations.

Evidence covers generation fences, refusal holds, coalescing and cancellation, actual read-only
snapshot frames, stale-source refusal, the mapping from hub state to row word and tone, and the
rendered localized overview. Local IPC fixtures do not establish remote SSH authentication or
network recovery.

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

The Client owns ephemeral status/search controls. `f` cycles status (including Done),
and slash edits a query of at most 128 safe scalars. Enter
or Escape first leaves query editing. The query takes the cursor keys and shared readline keys of
the [native line editor](native-line-editor.md). Painted controls accept
mouse activation. Filtering includes safe machine/location/title/provider/token
text. Rows keep canonical placement order. Missing observations stay distinct from no matches.
Changing data, context or controls invalidates painted activation identities.

`Ctrl+b {` or `Ctrl+b }` requests previous/next Agent across machines; the configured
`previous_agent`/`next_agent` actions use the same projector over every Agent the active source
view admits, whatever status filter or query the picker shows. An eight-second intent
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
source filter; explicit source sorting applies, otherwise canonical placement order applies.
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
active-source-filtered, stable merged list used by previous/next actions; the picker's
status filter and query never narrow that list. The
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
view and Client-local status and search still define eligibility.
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

With no configured remote profile, search omits remote totals and Local prefixes. The selected mode uses the shared selection surface plus bold and underline; it does not add an asterisk. Internal IDs do not appear in rows or details. Remote totals count remote observations only.

The Agent picker shows the assigned name and a distinct task title when present. Its details reserve separate rows for workspace, provider, title and custom tokens, plus machine context when remote profiles exist. The mode bar uses the shared current-state track so every mode keeps a distinct painted label and hit region.

The Agent picker's query includes the assigned runtime name even when no other field contains
that alias. It uses the same sanitized, case-insensitive substring matching as its existing
machine, Workspace, Panel label, provider, title, member and token fields; sorting and entry
points remain unchanged.
