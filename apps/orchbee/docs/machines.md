# Remote machines

The native Client owns the catalog of saved remote machines. The `machine` commands manage
labels, connections, explicit remote Session names, and enabled state; the machines themselves
are reached from Windows and macOS today; Linux answers unsupported. From either, the far end must
run Windows x64 or ARM64: the [launch line](#adding-a-machine) is a PowerShell program, so a macOS
or Linux far end is not reached yet. The person's own configuration for a connection kind owns
everything else: for SSH, OpenSSH owns credentials, keys, ports, host trust, and SSH configuration.

## Catalog and transitions

`internal/state/machines.json` in the data root is a private, atomic document; an explicit
`ORCHBEE_CONFIG_FILE` does not move it. A read keeps a record of an earlier version migrated in
memory; the first committed edit backs the original up under `internal/backups/` and saves the
migration before applying the edit. A record with a version this build cannot use, saved by a
newer version or not updated from an earlier one, is never rewritten: saved machines read as none,
edits last only until the process ends, and the process shows the record message once.

Each machine has a color, `cyan`, `purple`, `pink` or `grey`, kept as `color` in the profile's
`extensions`, the record's forward slot, so an earlier release that rejects unknown members still
reads the catalog and writes the color back unchanged. A machine added without a color takes the
one the fewest saved machines use, the earliest in that order among equals; a record without a
color, or with another value, takes the same default at load, counting only the machines before it
in the catalog, and keeps it from the next save. Nothing recolors a saved machine by itself.
The color is text color only, on the machine's name: its `@name` on Workspace rows, the Change
machine list and the machine that list's entry ends with, Settings › Machines and a machine's page
title, and the color choices. Add Machine opens on the default and Settings › Machines › Color
changes it. At 16 colors and in `terminal-native` the TUI maps the color by name, cyan to Cyan,
purple to Magenta, pink to bright Magenta and grey to the default foreground dimmed, so a machine
never takes an Agent state's red, yellow or green; monochrome and `NO_COLOR` show the name alone.

A connection `kind` this build does not know is kept verbatim with every member it carries and
written back unchanged. That machine is listed as unsupported, the same word in every client,
cannot be routed to, chosen, or discovered, and leaves every other machine usable. The record never stores a
credential, password, port, key path, or the remote platform. Duplicate target/session pairs may
have distinct IDs. At most 128 KiB is admitted.

`internal/state/machine-spaces.json`, the last-seen list, keeps what each enabled saved machine
last answered of its Workspaces: its Session ID, Projects by ID and label, and Workspaces by ID,
terminal context, label and Project. The TUI reads it once when it starts, lists an entry for an
enabled machine that has not answered yet, writes the whole list two seconds after an answer
changes it, and drops the entries of machines no longer saved, so a machine that is not connected
still lists its Workspaces after a restart. The last writer wins; a change in the two seconds
before the TUI exits is not kept. A list this build cannot use reads as empty and is never
rewritten; at most 1 MiB is admitted.

Every command that names a saved machine takes `<id|label>` and resolves it with the one rule
that `--machine` uses: an exact ID first, otherwise the only machine with that exact,
case-sensitive label. No match is `not-found`; several machines with that label are
`ambiguous-machine`, and only the ID selects one of them. `local`, in any letter case, always
means this computer, so a person or an Agent can name every machine, this one included, the same
way: no saved machine is added or renamed to it, and naming it where a saved machine is expected
is `reserved-name`. A record an earlier version saved under that label still loads, and its ID
names it. Edits resolve the operand under the
writer lock, so the machine changed is the one the catalog names at commit time.

A command that commits an edit then asks a running [machine hub](#machine-hub) to `reload`,
through one attachment that holds and watches nothing, so the hub stops carrying a removed or
disabled record and carries a changed one as it now reads. The attachment opens
[`present`](#attach-or-start): it reaches only the hub serving the data root at that moment, and
when none does, or that hub is closing or goes before it answers, the command starts, replaces and
waits for no hub and does nothing more. A hub binds its endpoint before it reads the catalog, so a
hub that serves later reads the edit. The edit stands whatever the hub answers; only an interrupt
during that wait changes the receipt, to `cancelled` with `committed` true.

Corruption never becomes an empty catalog and its bytes are preserved. A failed local commit does not roll back a remotely started Session. Cancellation after commit preserves the committed receipt.

## SSH host suggestions

A form that adds a machine may offer the host aliases of the person's OpenSSH client configuration as address suggestions. They come from `.ssh/config` below the home folder (`USERPROFILE` on Windows, `HOME` elsewhere) and the files it includes, read as ssh_config(5) reads them: case-insensitive keywords, a keyword separated from its arguments by whitespace or one `=`, `#` comments, and quoted arguments. Every `Host` alias is kept except a pattern with `*` or `?`, a `!` negation, and anything the `ssh` target rule rejects. `Include` follows each argument wherever it appears, even inside a `Match` or `Host` block: `~/` starts at the home folder, a relative path at `.ssh/`, and `*` and `?` components expand to the matching files sorted by path. Nothing else is read and nothing but the aliases is kept. A configuration beyond the read's bounds, or a file that includes itself through its own chain, leaves no suggestions at all. The form opens at once and reads the suggestions on a thread of its own; a read that fails, or has not ended 5 seconds after the form opened, as on a path that does not answer, leaves the form without suggestions and says nothing. The suggestions never change how `ssh` connects.

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
- **Disconnect** starts no hub: it attaches [`present`](#attach-or-start), and with no hub
  running, or only a closing one, there is no connection, and it succeeds with
  `remote: "not-contacted"` once the operand resolves. Otherwise it asks the hub to `reload`
  before it resolves the operand, so a record that was removed or changed without telling the hub
  ends there too, then sends `disconnect`; a hub whose catalog does not list the machine carries
  nothing for it, and a closing hub, or one that went with no other in its place, ends every
  carrier, so all read as disconnected. The deadline is 15 seconds. The remote Session keeps
  running.
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

## Machine receipt

`orchbee.machine` reports a connection kind this build does not know as the shared `{ "kind": "unsupported" }`; the content of an unknown kind is never echoed. `error` is a closed list that grows as remote-machine support lands; a reader treats a value it does not know as `unknown`.

## Adding a machine

`machine add <ssh-target>` and the TUI's add form run one staged pipeline, `machine_add` in the
workbench Client crate, apart from any drawing, so each check passes or fails the same way in both
and another interface reuses it. The new machine gets its random ID first and is connected through
the [machine hub](#machine-hub) as a [draft](#draft-machines) under that ID: `connect` carries
`draft` with the target and the remote Session name, and the hub carries it as it carries a saved
Machine, so every check after the first runs on one connection with one login. The record is saved
only once the draft reads `online`, under the writer lock against the latest catalog; the hub that
then rereads the catalog finds the ID with the same connection and Session and keeps the draft's
link as the saved Machine's. A draft that never comes `online` is dropped with its link, its login
and any answer remembered for it, and nothing was saved.

- **The TUI.** The form sends `connect` with `origin` `user` through the TUI's
  [holder](#the-tui-as-a-hub-client), starting it, and a hub, when no saved Machine started one yet,
  so its login asks in the TUI's [login overlay](#login-questions) and the draft's events reach this
  TUI alone. Once every check passed it saves the record and rereads the catalog at once, so the new
  row is there and selected when the form closes. A failed check gives the add up at once, and so
  does Escape during the checks: the holder's `disconnect` drops the draft, and a retry takes a new
  ID. The holder also asks the hub to disconnect any draft it is told about that is neither the add
  in progress nor the add it just saved, and the add it saved when the hub's catalog at that save's
  revision or later does not name it, so a draft whose `connect` overtook its `disconnect`, or whose
  record went before the hub reread the catalog, does not stay connected.

`ssh` never changes the person's files by itself. The `ssh` command line Orchbee builds sets
`UpdateHostKeys=no`: since OpenSSH 8.5 the default otherwise lets `ssh` add and remove a host's keys
in `known_hosts` after a login, which a background hub would repeat at every connection. The Windows
remote command is an encoded PowerShell program with literal data.

The launch line resolves the remote program through the remote store's `active.json`, never
through `PATH`: in the store root (`ORCHBEE_INSTALL_DIR`, or a folder under
`%LOCALAPPDATA%\Programs`) it reads the active version and runs `versions\<version>\<name>.exe`, and
exits 125, read as `remote-not-installed`, when it resolves no program or cannot start it. Each
build looks only for its own environment's installation, so builds never meet on the remote either:
a release build reads the `Orchbee` store and looks for `orchbee.exe`, and a development or test
build reads only its own `<stem>` store and looks only for `<stem>.exe`. It never runs the release
installation or another build's, so nothing of theirs starts and nothing is written into their data
folder; a remote without its own installation reads as `remote-not-installed`, whose sentence in a
development or test build says that this environment's Orchbee is not there. See
[Windows release installation](windows-cli-configuration.md#windows-release-installation).
No protocol version, product version, or cohort is compared across machines beyond what the link's
offer and the remote Session's handshake admit.

An enabled profile is not evidence of a live connection. No remote Session is stopped by rename,
enable, disable, or a remove without `--stop-session`.

## Removing a machine

`machine remove <id|label>` removes the record at once and forgets its
[remembered answers](#remembered-answers). Nothing on the remote is deleted: its Orchbee, its files
and its Session stay, and the Session keeps running.

`machine remove <id|label> --stop-session` stops the remote Session first, ending every terminal in
it, and removes the record only once it stopped or was not running. It is destructive, so it takes
the confirmation a [remote Session stop](#remote-session-stop) takes: development and test builds
require `--confirm`, which is a usage failure without `--stop-session`, and a release build prompts
for the remote Session's name and refuses `--confirm`. The catalog is read and the selector resolved
once, before the confirmation, which names that Machine; a catalog that cannot be read or a
selector that names no Machine fails the command before anything is asked, and the stop and the
removal then act on the resolved ID alone. It then stops the Session exactly as the remote Session
stop does: the hub takes the catalog and is asked for `stop-session`, a Machine not
`online` is brought `online` under the `machine connect` rules and errors and asked once more, and
each answer may take 120 seconds.

An interrupt ends the command with `cancelled` and the record kept; once `stop-session` may have
been sent, the stop itself goes on. The TUI's "Remove and stop remote Session" asks its
own confirmation and is offered only while the Machine is `online`.

## Command routing contract

`orchbee --machine <id-or-exact-label> <command>` selects an enabled saved machine by the [`<id|label>` rule](#catalog-and-transitions) and never changes the catalog; `--machine local` runs the command on this computer as if the option were absent. Help is local. Local management, installation, TUI, and terminal interactive streams are refused before anything is read, as a usage failure that names the command and points at `orchbee machine --help`, since the option itself is valid. `server stop` is never routed as a command line: with the prefix it is the [remote Session stop](#remote-session-stop), and a far end of this build refuses an older near side's routed `server stop` as not submitted.

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
as every `connect` does. The `connect` brings the Machine's Session up with it: once the link
answers, the hub opens [`ensure-session`](#link-helper), which starts the remote Session as
`server start` would when it is not running. A read-only command therefore starts that Session
too: `orchbee --machine <machine> agent list` against a Machine whose Orchbee is not running
connects it, starts its Session, and then lists that Session's Agents. The routed command itself
never starts a Session, so a Session that stopped while its Machine stayed `online` is not started
again by a query, which answers as it would on that machine. On the route the command writes one
bounded request packet, sends `end`, and reads the reply packet until `close`; the hub
[relays](#channels) it to a link `route` channel, so three commands to one Machine share its one
carrier. The packet and its reply are closed objects whose `version` is their `machine-route`
generation in the [connection registry](../../../schemas/connection/v1/connections.json); each may
carry an `extensions` object whose member names follow the capability-name grammar and whose
content the receiver ignores. Neither carries a cohort. The remote [link helper](#link-helper) refuses a
generation it does not read, then revalidates the Session, argument grammar and command allowlist
under its own grammar before spawning its own executable with literal arguments in an owned
Windows Job.
The packet's `locale` names the sender's language; a value the receiver does not ship, including
one a later sender adds, falls back to `en_US.UTF-8` instead of refusing the command, and the
routed command runs under the resolved locale.
When the remote grammar cannot parse the routed arguments, the reply carries the remote's ordinary usage failure in the packet's locale and with the usage exit status, rendered by the link helper without spawning a command. A parsed request outside the routed scope, a packet that names a Session, and invalid input are refused as not submitted. The existing remote CLI remains the
business-operation owner. No local peer credential admission is bypassed.

Terminal file and stdin sources are acquired on the caller once and sent as raw
bytes; directories and executable paths are interpreted by the remote command: a home-relative
path operand reaches the far side as written and resolves against the far home.

Caller terminal/worker context is cleared; explicit targets or remote Client
selection determine the destination. The routed command runs with the private
`ORCHBEE_ROUTED_COMMAND=1`, so a creating command that would start its Session on first use does
not start one there. Output bytes, diagnostics and exit status
are returned in a bounded reply, never reconstructed from SSH diagnostics.

Local input has a 60-second bound, which a Windows stdin pipe reader's start moves later by its
own length, and reaching `online` 90 seconds: a hub start, then the hub's
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

A failure document `orchbee.cli.failure.machine-route` names its `reason`, the `remoteOutcome` (`not-submitted` or `unknown`) and a failure kind, and the exit code follows from the kind alone.

An unknown remote outcome keeps `cancelled` for an interrupt and `timeout` for a deadline and is
otherwise `outcome-unknown` with exit 8 whatever its reason, so no exit that states that nothing
ran ever follows a dispatched command.

## Remote Session stop

`orchbee --machine <id-or-exact-label> server stop` stops the selected Machine's remote Session and
every terminal in it through the hub's [`stop-session`](#stopping-a-session); nothing travels as a
routed command line. The grammar is the local `server stop`'s: development and test builds require
`--confirm`, and `--request-id`, `--retry`, `--detach` and `--timeout`, which have no counterpart in
`stop-session`, are usage failures (exit 2). The command then reads the catalog and selects the
Machine by the [routing rules](#command-routing-contract); a selection or catalog failure is a
not-submitted `orchbee.cli.failure.machine-route` document, and no hub is asked.

The near side next applies the [release confirmation](../README.md#release-destructive-confirmation)
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

The result document `orchbee.machine.server-stop` names the saved Machine as the [machine receipt](#machine-receipt) does and carries the hub's outcome word, and the exit code follows from the outcome alone. `failed` exits as a failure: the Session may have accepted the stop and ended part of it, so nothing is claimed. `not-running` reads as a local `server stop` that finds no Session.

`--format text` prints one line naming the Machine and the outcome, and says when an interrupt
ended the wait; raw `ssh` text never reaches it. The command never retries a sent `stop-session`.
An older build's far end is never sent anything by this command. An older near side's routed
`server stop` reaches a far end of this build as a routed command line, which it refuses.

## Changing a Workspace's machine

`orchbee [--machine <machine>] [--session <session>] workspace set-machine <workspace> --to <machine> [--confirm | --dry-run]` moves a Workspace by creating it on the machine `--to` names and then closing it where it ran. `--machine` names the machine that holds the Workspace and never routes this command, as with the [remote Session stop](#remote-session-stop): the command runs on this computer and asks each machine in turn with ordinary commands, a [routed command](#command-routing-contract) for a saved machine and a child of the same executable in the same Session for this computer. Since this computer's end runs here, `--session` still names its Session beside `--machine`, which it conflicts with for every routed command; a TUI on a named Session moves Workspaces from and to that Session. No new request crosses machines, so each machine answers under its own grammar and rules, and a far end of an older build answers with its own refusals.

Both machines are resolved by the [`<id|label>` rule](#catalog-and-transitions) before anything is asked, and the same machine twice is refused. A saved machine the hub reads as neither `online` nor `needs-login` is refused as not connected, naming its state, or `unknown` when no hub answers: the change never connects a machine, since connecting is the person's choice. A machine that needs a login asks as a routed command does, and the change goes on once someone logs in.

The machine that holds the Workspace answers `workspace get` and `api snapshot`. A Workspace in a linked worktree is refused, since the worktree's checkout stays with its repository there. A Project travels as its identity: the other machine runs `project create --id`, which changes nothing when its Session has that Project already, so Workspaces moved one by one land in one Project there. A Project whose identity its own Session could not make shareable is refused, and so is a machine whose CLI or Session does not take `--id`, before anything is created.

Orchbee moves no files. The new Workspace asks for the same folder relative to home, written `~/…`, which the other machine resolves against its own home; the home of the machine that held the Workspace is this computer's home, or the home a saved machine reports with its free disk. A folder outside that home, an unknown home, or a folder the other machine refuses before creating anything leaves the new Workspace in home, with one Tab and one shell. An automatic name is left to the other machine only in the same folder, where it follows that folder as before; in home the Workspace keeps its name.

The old Workspace closes, ending its terminals and what runs in them, only after the new one was created. A close that fails, or an interrupt after the creation, leaves both and names the new one, so a failure never leaves the person with neither. An interrupt before a step that changes something stops the change there. `--dry-run` changes nothing and lists the terminals the change would end; it wins over `--confirm`, as for `workspace close`.

What the change would stop comes from the machine that holds the Workspace: its `workspace close --dry-run` lists the terminals with each one's `activity`, read by the [Workspace idle check](../../orchbee-server/docs/terminal-tab-close.md#workspace-idle-check), the evidence a Workspace close that asks first reads. A terminal is `running` when its shell reported a running command or another program holds its console, `unknown` when that could not be read or did not tell, and `idle` otherwise. When that machine cannot read the evidence, because its Session lacks the `workspace-idle` capability, it does not answer in time, or its CLI is older and lists the terminals only, `evidence` is `unavailable` and every terminal is `unknown`, never idle. Without `--confirm` or `--dry-run` the change reads those facts and then refuses with `invalid-usage` and the reason `confirmation-required`, naming the terminals that are not idle, so a person or an Agent can decide; nothing is created or closed. Asking first is the Client's: a move that reads every terminal idle needs no question there, but the command always needs `--confirm`, as every command that ends terminals does.

The result document `orchbee.workspace.set-machine` names both machines, the folder relative to home and the new Workspace with its Session and first terminal context, so a Client can open it, and a dry run's `terminals`, `idle` and `evidence`. The failure document `orchbee.cli.failure.workspace-set-machine` names the `stage`, its `reason`, the machine concerned, the failed step's own failure kind and reason, a Workspace already created, and, for `confirmation-required`, the same `terminals`, `idle` and `evidence`. A step that failed passes its exit status on, so the exit code says what that step met; otherwise the exit code follows the kind.

### Change machine

The TUI's Change machine, on Windows and macOS where the TUI reaches saved machines, moves one Workspace by running the command above, so a person, a Client and an Agent get one result under one set of rules. A Workspace's menu offers it after Move to Project, and the global action menu offers it for the Workspace in use; the row ends with the machine the Workspace runs on, and it opens the list.

- **The list.** The heading `Runs on` with the column note `free CPU · memory · disk`, This computer, then every saved machine: usable ones with room first, then usable ones low on something, ones that need a login, and ones that cannot take a Workspace now (not connected, connecting, of another version, or stopping their terminals), each band by name as Settings orders it. The order is taken once, when the list opens, so a row never moves under the pointer; what the rows say keeps following the machines. `Manage machines…` ends the list and opens Settings on Machines. Past eight saved machines the list leads with a `Find a machine` query that matches a name or an address.
- **A row.** A machine that can take the Workspace ends with what it has free, `88% · 24 GB · 310 GB`, read as Settings reads it while the list is open; a number turns amber below 20% CPU, 4 GB of memory or 25 GB of disk, and a value its source cannot give reads "—". A machine that needs a login ends with that state word and can be chosen, since the move waits for the login where Orchbee asks for it; any other machine ends with its state word and cannot be chosen, since the command refuses it. The machine the Workspace runs on carries the check; choosing it changes nothing.
- **Worktrees.** A Workspace in a linked worktree cannot change machine, because its checkout stays with its repository: once Git has answered, every row says so. Before that the rows stay choosable and the command refuses.
- **Check, ask, then act.** Choosing a machine first runs the command's dry run, which creates and closes nothing. When every terminal is idle the move runs at once. Otherwise it asks, as the close-Workspace confirmation does: `Move Workspace?`, the Workspace, what would stop or that what runs could not be read, and `Files are not moved.`, with Move focused in the destructive tone and Esc and Cancel cancelling. A terminal is named by its Agent, else by a title that names a program Orchbee knows; the first three names are listed and the rest counted. The question appears on its own once the dry run answers, so, as with a command's login notice, Enter and Move act only after frames have shown it whole for a second in which no key or paste was dropped; a keystroke meant for the terminal never confirms it.
- **While it runs.** The status line says `Moving…` and the list offers no machine: one move runs at a time. The command runs as a child of the same executable on a background thread, so nothing it waits for, a login included, holds a frame or the local Session. Quitting ends a dry run at once and lets a move that started finish, so a Workspace is not left on both machines.
- **Landing.** A move that succeeded opens the new Workspace on its machine, as choosing its sidebar row does, and adds no notice: the new shell's prompt shows where it started. When the folder relative to home is not on that machine and the shell started in home, the status line names the folder that was not found, as that machine spells it.
- **Failures.** The status line says what a move that did not succeed left: the machine that is not connected, a worktree, a new Workspace beside an old one that could not close, nothing changed, or an outcome Orchbee could not tell.

## Link helper

`orchbee --machine-link-v1`, with no other argument, is the remote end of the
[`machine-link`](../../../schemas/machine/README.md) connection: the hub starts it over `ssh` and
speaks to it on its standard input and output. One pair of pipes carries several Sessions' byte
streams, routed commands, and Session starts at once. Windows and macOS serve it; elsewhere it
answers unsupported before reading a byte. It is the only remote entry Orchbee starts over `ssh`: the
earlier `--machine-tunnel-v1` and `--machine-relay-v1` helpers are gone, and a build that still
starts one finds an unknown argument. The contract owns every byte; this section owns how the
helper meets it.

Before it reads any Orchbee state, a Windows helper reads its own token; a macOS helper has none
to check. Win32-OpenSSH gives an
administrator's logon an elevated token, and a Session started with it refuses the Clients the
person starts at the desktop. A token above Medium integrity or with BUILTIN Administrators enabled
therefore never serves the link: the helper starts itself again with a
[standard token](../../../crates/local-runtime/README.md#windows-standard-rights) derived from its
own (administrator groups for deny only, the privileges a filtered token keeps, Medium integrity,
the user as owner, a default access list of the user and SYSTEM), hands it its standard input,
output and error, closes its own copies, and exits with the child's status. The child is the
helper, and the Session it starts runs with standard rights. When the derivation or the start
fails, or the derived token is still elevated, the helper exits 5 and writes `link-rights` to the
remote's [daily log](../../../crates/local-runtime/README.md#diagnostic-log) with
`outcome` `refused` and `reason` `derive`, `still-elevated` or `start`; it never starts an elevated
Session. A child status above 255 also ends with 5 and `link-rights` `outcome` `child-failed` with
that `status`. Every helper that passes the check writes `link-rights` with `level` `low` or
`medium`, `admin` `deny-only` or `absent`, and `elevated`, the token's `TokenIsElevated` as 0 or 1.
An administrator who wants an elevated remote terminal opens a plain `ssh` session instead.

`ensure-session` uses the same activation as `server start`; the Server it starts outlives the helper. A Session's endpoint is found as a local Client finds it and must belong to the same user.

On macOS the Server a helper starts leaves the helper's session and process group, as every local
start's Server does, so the end of the connection leaves it running; this is not yet proven over a
real `sshd`. Win32-OpenSSH runs each connection's processes in one Job that ends every member when the
connection closes and lets a member create a child outside it on request
(`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` and `JOB_OBJECT_LIMIT_BREAKAWAY_OK`, set in
`contrib/win32/win32compat/w32-doexec.c` from 7.9 on and in `session.c` in 7.7.2). The helper
therefore creates the Server with `CREATE_BREAKAWAY_FROM_JOB`: the Server, its Worker, Host and
terminals run outside that Job, so disconnecting, a killed `ssh` and a dropped connection leave
them running, and the next link's `ensure-session` joins the same Server. An elevated helper and
the standard helper it starts both stay in the Job and end with the connection. A Job that keeps its
processes refuses the request, and the Server then starts inside it, as before, and ends with the
connection. The start itself succeeds either way. Each start writes `session-start-source` and
`server-job` to the remote's
[daily log](../../../crates/local-runtime/README.md#diagnostic-log), the latter with
`outcome` `outside`, or `inside` with the refused request's error kind as `reason`.

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

Backpressure is per channel: a hub that reads slowly blocks the writer, so the stalled channel's workers stop committing and its pipe reader stops reading, and the Session sees its pipe fill while other channels keep their turns; a Session pipe that reads slowly returns no credit, so the hub stops sending on that channel only. At exit a Session start runs to its own deadline and is never abandoned halfway. Ending a channel never stops a Session, its Server, Worker, Host or terminals: the helper only closes its own pipe connection.

## Machine hub

`orchbee --machine-hub-v1`, with no other argument, is the hub: the one process per data root
through which interfaces reach saved machines over
[`machine-hub`](../../../schemas/machine/README.md#hub-records). It serves the catalog, its own
lifetime and each Machine's state, and [carries](#carrying-a-machine) each Machine it connects over
one child: `ssh` running the remote [link helper](#link-helper), and [relays](#channels) every
client's Session streams and routed commands over that child. Windows and macOS serve it;
elsewhere both hub entries answer unsupported before they touch the data root. A macOS hub leaves
the session and process group of the client that started it at once, so the client's terminal
signals it neither at an interrupt nor when it closes. The entry is an internal pairing
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

| Name                | Spelling                                                                                                                                                                                                                                                           |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Lock                | `<data root>/internal/machines/hub.lock`                                                                                                                                                                                                                           |
| Windows endpoint    | `\\.\pipe\orchbee-<id>-hub`: `<id>` is the first 32 lowercase hexadecimal digits of the SHA-256 of `orchbee-windows-machine-hub-v1`, one NUL byte, and the UTF-16LE code units of the data root's canonical `\\?\` path with ASCII letters lowercased              |
| macOS, Linux socket | `<socket folder>/<id>-hub.sock` in the environment's `/tmp/<stem>-<uid>` (`/private/tmp/` on macOS), `<id>` as on Windows over `orchbee-posix-machine-hub-v1`, NUL, and the canonical path's bytes, at most 90 bytes. A macOS hub binds it; Linux only computes it |

The lock and the Windows endpoint freeze at the first public release. After an update an older hub
meets a newer interface, or the reverse, so they must find each other by these spellings alone; a
later release changes the connection through the `OBHS` generation, never through the names. The
macOS spelling freezes with the first public release that serves saved machines there. A macOS hub
creates the socket folder through `prepare_socket_root` and holds it shared for as long as it
serves, as Session endpoints do, and binds under its lock, removing a socket file a killed hub left
behind; whether the canonical path stays stable on macOS is unverified.

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
  same way; on macOS each lock file is opened relative to its folder, never through a link, and
  checked as a Session start checks it. Neither lock file is replaced by rename: each is created once and removed only with its
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
  hub. The macOS monotonic clock stops while asleep, so a hub there closes that much later. In
  development and test builds `ORCHBEE_MACHINE_HUB_EXPIRY_MS`, from 1,000 to 600,000,
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

The hub owns one record per catalog Machine, and per [draft](#draft-machines): its reported state, its retry history, at most one
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
- **Start slots.** A `presence` attachment, and a record a reload adds or changes while one is
  attached, queue each idle Machine instead of starting it. The hub starts queued Machines while
  fewer than four attempts of any kind run: first those whose entry in
  `internal/state/machine-spaces.json`, read with the catalog, lists a Workspace, then catalog
  order. A `connect` request and a retry start at once. A queued Machine reads as an idle one does
  until its attempt starts.
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

#### The carrier child

| Rule           | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Program        | The system `ssh` (`<system folder>\OpenSSH\ssh.exe` on Windows, `/usr/bin/ssh` on macOS, never found through `PATH`) with the command line `machine_ssh::carrier_command` builds: `-T`, `StrictHostKeyChecking=ask`, `UpdateHostKeys=no`, no forwarding, no multiplexing, then `-- <target> <launch line>`. Batch mode is off                                                                                                                                                                                                                                                                                                                                           |
| Askpass        | Every carrier, the stand-in included, gets `SSH_ASKPASS` set to this program's own path, `SSH_ASKPASS_REQUIRE=force`, a placeholder `DISPLAY`, and `ORCHBEE_MACHINE_ASKPASS` with the hub's askpass endpoint and the carrier's own 16-byte ticket, so `ssh` never reads a console and an [askpass helper](#askpass-helper) is the only way a question reaches anyone                                                                                                                                                                                                                                                                                                    |
| Launch line    | The encoded PowerShell program that resolves the remote store's active version, as [adding a machine](#adding-a-machine) describes, and runs it with `--machine-link-v1`; it exits 125 when it finds none. It needs a Windows remote: a launch line for a macOS remote is not part of this build, so a macOS far end is reached only through a stand-in today                                                                                                                                                                                                                                                                                                           |
| Stand-in       | In a development or test build only, `ORCHBEE_MACHINE_HUB_SSH_STAND_IN` in the hub's view names an absolute program that runs with the one argument `--machine-link-v1` in place of `ssh` and the launch line, with everything else unchanged, and with `-V` alone in place of `ssh -V`; any other value fails every attempt with `ssh-unavailable` and never falls back to `ssh`. A release never reads it. On Windows a local far end runs inside the carrier's Job, which lets no process leave, so a Session it starts ends with the carrier, unlike one via `ssh`; on macOS that Session leaves the carrier's process group and outlives it, as one via `ssh` does |
| Process        | No console window (`CREATE_NO_WINDOW`) on Windows, the system folder as working directory (`/` on macOS), an empty signal mask, which on macOS the launch gives the child itself (the owner thread clears its own mask before every start), standard input and output the link, standard error a pipe the hub reads, the [child environment](#environment)                                                                                                                                                                                                                                                                                                              |
| Descriptors    | On macOS the child inherits no descriptor but its three standard streams, even while other hub threads create descriptors, because the child starts through the native launch. Linux creates close-on-exec descriptors atomically and keeps the standard library's spawn; Windows is unchanged                                                                                                                                                                                                                                                                                                                                                                          |
| Job or group   | On Windows each carrier starts suspended inside its own Job, which ends every process in it when its last handle closes. Only the hub holds the handle, so the hub's exit, however it happens, a kill or a panic included, ends every carrier and whatever it started. On macOS each carrier leads its own process group, which the hub ends with `SIGKILL`; when the hub is killed outright, `ssh` ends once its input closes, while an askpass helper waiting on a question keeps its 250-second bound                                                                                                                                                                |
| Standard error | The last 4 KiB are kept only to classify an end and are never copied into a record, an event, a log or product copy                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Reaping        | Each carrier has one owner thread, its only reaper: it starts the child, waits for the root process, then terminates whatever is left in the Job or group, takes the exit status, and reports the end once the carrier's reader, writer and standard-error threads have ended. Ending a carrier stops its writer, which closes the child's input, and terminates the root process; the owner does the rest                                                                                                                                                                                                                                                              |

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
| Before the answer, exit 127 with `powershell.exe` in standard error: a shell that has no PowerShell                                                         | `unsupported` cause `remote-platform`                                                    |
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
OpenSSH quotes it is not proven. The one exception is a development or test build started
with exactly `--machine-link-v1`, the far end a [stand-in](#the-carrier-child) starts, which runs as
the link helper; a release never makes it. Such a far end inherits the variable, which a far end
over `ssh` never has, so the routed commands it runs start without it. The variable is private: the hub sets it only in its
carrier children, as `<askpass endpoint> <ticket>`, with the ticket as 32 lowercase hexadecimal
digits.

1. It connects to the hub's askpass endpoint within 5 seconds, which admits only the hub's user.
2. It reads the prompt as text, replacing what is not Unicode and cutting at 4,096 bytes on a
   character boundary, and reads the [kind](#login-episode) from it.
3. It binds its own one-shot answer endpoint, `\\.\pipe\orchbee-answer-<pid>-<random>` on Windows
   and `answer-<pid>-<random>.sock` in the socket folder on macOS, and draws a 16-byte token.
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
failed password. The helper runs inside its carrier's Job or process group, so the carrier's end
ends it first:

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
  question's bound, to end the carrier; a Windows hub that exits ends the Job and every helper in
  it.
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
endpoint's server process against the id in its name, are not part of this build. On macOS both
endpoints are sockets in the socket folder of this user and environment, a folder only that user
can enter, which the hub holds while it serves; binding never replaces an existing file, and every
accept proves the peer's effective user before a byte is read.

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
  call names one item. A process makes one Credential Manager call at a time, a write's secret and
  label together: calls made at once, even on different items, were seen to bring back an item
  already deleted. macOS uses the login Keychain: a generic password whose service is the item's
  name and whose account is `orchbee`, written through `keyring-core` and
  `apple-native-keyring-store`, the same store family, for the same reason. That store writes no
  label, so Keychain Access names the item by its service. The Keychain's access list trusts the
  program that created an item by its code signature: an unsigned or ad-hoc-signed build gets a new
  signature with every update, so after an update macOS asks once more before that build reads an
  answer it remembered earlier. Linux answers "unavailable" in this build: a current limit, not the
  design, which gives it the Secret Service, otherwise the kernel's per-user keyring, as adapters of
  the same port that change no caller. A store that cannot be reached, such as Credential Manager
  in an SSH logon or a Keychain this session cannot reach, also answers unavailable: the TUI shows
  "Passwords can't be remembered in this kind of login." in place of the box. The TUI asks the
  store, and the `ssh` release below, on a thread of its own when the first password or passphrase
  question arrives or a remote row is first selected, and shows the box once they answered. A
  ticked answer the store does not take after a login that succeeded shows "Couldn't save the password." in
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
- **Forgetting.** "Forget password" in a remote row's [actions](#managing-machines-in-the-tui), or
  its key, offered while the store can be used, deletes both of its items without reading them; the
  key is taken only while the selected row can forget, so elsewhere it goes on to the picker's
  other bindings. Every forget tries both kinds, even when one fails, then reports. `machine remove`
  deletes them after the catalog commit; the removal succeeds either way, and standard error
  carries one line when they could not be deleted. The TUI's Remove deletes them on a thread of its
  own and reports nothing, since nothing reads the items of a removed Machine. `uninstall --delete-data` deletes the items of every Machine the
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

- **Truth about effects.** Before submission nothing reached the Session, so each failure reads as
  no effect. After it, only the Session's answer or its end decides the run, and anything else reads
  `unknown`. A far reset counts only before submission: the helper resets a `session` channel only
  before `accept`, and every answer follows `accept`.
- **The outcomes stay the contract's.** A `start` answered `busy` reads `failed`, although another
  stop may be ending the Session: the Session refused this one. So does a `start` answered
  `not-recorded`, which a Server restart's hold gives: nothing was recorded. The run cannot tell a
  Session that ended because of its stop from one that ended otherwise, so both read `stopped`.
- **`not-recorded` after `pending` means a later Server.** A Server keeps its stop entry until it
  ends, so a later `status` it does not know comes from another Server of the same Session, and the
  one asked has ended. Without the `pending` answer the `start` may never have reached the Server,
  or reached one that ended since; the run cannot tell which.
- **Joined requests.** A Machine has at most one run; a `stop-session` for a Machine with a run joins
  it and gets the same outcome. A client that leaves loses its replies, and its run goes on.

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

Names compare without letter case on Windows, as Windows does, and exactly on macOS. A variable whose name or value is not valid
Unicode is left out of the view.

A [carrier child](#the-carrier-child) receives the view without any `ORCHBEE_*` variable but the
askpass pairing `ORCHBEE_MACHINE_ASKPASS`, and without the terminal's own variables `TERM`, `COLORTERM`, `TERM_PROGRAM`, `TERM_PROGRAM_VERSION`,
`WT_SESSION` and `WT_PROFILE_ID`, since it runs in no terminal, plus only the variables its command
line sets and the [askpass settings](#the-carrier-child), which win over the view. One spawn path
clears the inherited environment first. The child has no console, piped standard streams and the
system folder as working directory, `/` on macOS. An askpass program inherits `ssh`'s whole environment, so
whatever the hub passes to `ssh` also reaches the askpass: that is the endpoint and the ticket,
never an answer. `PATH` stays as the view has it, an
[accepted limit](#accepted-limits).

### Admission and requests

The endpoint admits only the current user: its DACL grants access to the hub's own token user
alone, it refuses remote clients, and every accepted connection proves, by the token the client
opened the pipe with, that the client's token user equals the hub's before a byte is read, as
[`local-ipc`](../../../crates/local-ipc/README.md) does for machine and Session Client endpoints.
The `OBHS` frame must arrive within 2 seconds of the accept.

A disabled record reads `disabled`, a connection kind this build does not know `unsupported` with
cause `connection-kind`, and any other Machine starts `offline` without a cause or a next attempt.
A catalog that cannot be read when the hub starts serves as empty with revision 0 until a `reload`
succeeds.

### Draft Machines

A draft is a Machine a connection asks the hub to carry before the catalog holds it, so
[adding a machine](#adding-a-machine) checks it on the connection it keeps once saved. It is carried
as a catalog Machine is, by the same state machine and carrier, with these differences.

- **Admission.** `connect` with a `draft` whose ID the catalog does not hold makes, or reuses, this
  connection's draft of that target and Session. `invalid-draft` answers a target the record's rule
  rejects, an ID the catalog holds with another connection or Session, and another connection's
  draft. A draft the catalog already holds as it is goes on as that record. A connection keeps one
  draft: a `connect` that makes a new one first drops the one it already has.
- **The owner alone.** Only the owning connection sees the draft: its `machine` events carry
  `draft: true` and go to the owner alone, while it watches; `catalog` never lists it; another
  connection's `connect` and `disconnect` find no such Machine, and `stop-session` and a channel
  `open` find none for anyone until it is saved. Its login episode asks only the owner, another
  connection's `claim` or `dismiss` of it reads `ended`, and `presence` never starts an attempt for
  it.
- **Saved.** A reread catalog that holds the ID with the same connection and Session makes it an
  ordinary Machine that keeps its carrier, and every watcher gets its `machine` event.
- **Dropped.** The owner's `disconnect`, the owner's leaving, the owner's next draft, and a reread
  catalog that holds the ID otherwise drop it: its carrier ends, its waiters are replied `cancelled`, and the hub forgets it.
  An answer remembered for it is forgotten with it.

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

### Ends and races

- **A panic ends the hub.** No hub thread may unwind while the others serve: a listener that
  unwound would leave the lock held with no endpoint, so every attach would fail, and a reader that
  unwound would leave its `presence` counted, so the hub would never expire. The hub's panic hook
  writes a structured `panicked` diagnostic log record with `thread` (naming its role token such as
  `orchbee-hub-listener` or `orchbee-hub-peer-reader`), `file` stem and `line`, without any panic
  message, then reports and aborts the process, which the design recovers from as from a kill.
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
- **A connection Job that keeps its processes ends the Session with the connection.** A remote
  `sshd` whose Job refuses `CREATE_BREAKAWAY_FROM_JOB`, which none of the Win32-OpenSSH releases
  read, 7.7.2 to 10.4, does, gets the Server [inside it](#link-helper); the Machine still comes
  `online` and nothing on screen tells the two apart, only `server-job` `inside` in the remote's
  daily log.
- **The `ssh` phrases are one version's.** The classification reads OpenSSH 9.5's standard error
  wording and the prompts M0 recorded; another wording reads `ssh-failed`, and a prompt it does not
  recognize is asked as `other`.
- **Only a claimant is asked.** A silent attempt that meets a new host, or a password or key
  passphrase with nothing usable remembered, ends at that question, so an interface or a terminal
  must ask once per hub life of the link unless the person chose to
  [remember](#remembered-answers) the answer. No real `ssh` has run these questions yet: the first
  real run checks every kind against OpenSSH.
- **Two processes' store calls can meet.** Each process makes one Credential Manager call at a
  time, but the hub and a TUI, or two clients, calling at the same instant can still fail a write
  or leave a deleted answer behind, as two test processes did; nothing orders calls across
  processes. The macOS adapter does not serialize its calls within a process either.
- **An unsigned macOS build is asked about again after each update.** The Keychain trusts the
  program that saved an answer by its code signature, so until the macOS release is signed with a
  stable identity, every update brings one more access request before a remembered answer is used;
  a request declined, or made where no dialog can be shown, reads as no answer and `ssh` asks.
- **A killed hub sweeps orphaned draft answers on restart.** Before a draft can start a carrier
  that might save remembered answers, the hub persists the draft's ID in
  `internal/machines/drafts.json` off the hub's lock and keeps it there until promotion or deletion
  completes (on normal shutdown or bounded `exit`, failed or timed-out deletions stay on disk). A
  hub that is killed rather than closed leaves those draft IDs behind; once its listeners are
  running, the next hub for that data root sweeps every recorded ID absent from the saved catalog on
  a background thread at startup or on its first clean catalog reload if the catalog was unreadable at
  startup. If no hub starts again for that data root or the catalog stays unreadable, orphaned draft
  items stay in the store until one does.
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

### Client owner

One owner reaches the hub for every client: the diagnostic entries, `machine connect`, routed
commands, the remote Session stop, uninstall and the [TUI](#persistent-tui-connections) today,
the Desktop's Client in a later slice. It is the workbench Client crate's `machine_hub` module,
built on every platform and used where saved machines are served, Windows and macOS. Two rules
place it there. An internal crate never depends on the application
target ([Rust](../../../docs/RUST.md#placement-and-workspace-ownership)), so the TUI's crate cannot
reach code under the application's `cli`. Reusable code goes to its nearest semantic owner
([module boundaries](../../../docs/MODULE-BOUNDARIES.md#semantic-module-design)): the workbench
Client crate is the lowest crate that both the CLI and the TUI reach, and it already owns the
saved-machine catalog the hub serves. The hub process stays in the application, as a Server stays
apart from its clients. No second client loop exists: the state printer is a caller of the owner.

#### Attach or start

A call resolves the data root, creates `internal/machines/` privately unless it opens `present`,
names the endpoint, and within its deadline connects and offers `machine-hub`. Its opening decides
whether it may start a hub; the answers below then apply as far as the opening allows.

| Opening   | No endpoint                                                                               | A closing hub                                       | Another version's hub          |
| --------- | ----------------------------------------------------------------------------------------- | --------------------------------------------------- | ------------------------------ |
| `start`   | Starts one                                                                                | Waits for it to go, then starts one                 | Replaced when nothing holds it |
| `restart` | Starts one                                                                                | Fails `closing` at once                             | Replaced when nothing holds it |
| `join`    | Waits for one somebody else starts; fails `closing` 10 seconds after the endpoint is gone | Waits for it to go and for one somebody else starts | Reported, never replaced       |
| `present` | Fails `closing` at once and creates nothing                                               | Fails `closing` at once                             | Reported, never replaced       |

A hub the call started that exits 0 lost the lock to another hub: the call connects again and starts one more only after a growing pause. One that exits 1 ends the call at once, `unavailable`. `capacity` and `stopping` answers are retried within the deadline; a deadline that passes with the hub still closing fails `closing`, never `timeout`. A refusal no retry can cure fails at once: `elevated` for a standard process refused by the [High label](../../../crates/local-ipc/README.md#windows-transport-refinement) of a hub started as administrator, `denied` when the DACL refused this process or the server runs as another user. Another version of this environment's hub is [replaced](#incompatible-hubs) when nothing holds it, otherwise reported `incompatible` naming its version; a build of another environment that serves this data root refuses control requests too, so nothing here can end it.

The hub belongs to no Session's Server and outlives its starter.

**Closing within the caller's deadline.** A hub in `closing` normally exits within milliseconds. It
lasts up to the 60-second allowance only while a catalog read it had started or a `route` channel
is still running.
An attachment waits within its caller's deadline, 10 seconds for the diagnostic entries, and not
the whole allowance. The call attaches to the successor the moment the old hub is gone, so a longer
wait would only delay a failure. Only the caller knows how long a person or a task can wait. The
result names the real cause, `closing` rather than `timeout`, so the person knows that trying again
will work. The owner's re-attachment, which no person waits on, covers the whole allowance.

#### Administrator and standard processes

On Windows a data root keeps one hub, which runs at the integrity level of the process that
started it, standard or administrator. Every machine endpoint, the hub's, its askpass endpoint,
each helper's answer endpoint and each carrier's, proves a client by the token the client opened it
with rather than by its process
([local IPC](../../../crates/local-ipc/README.md#windows-transport-refinement)), so a hub a
standard process started admits the same user's processes started as administrator. A hub started
as administrator carries the High label, which refuses every standard process at the open: such a
client fails `elevated` at once, without retrying or starting a hub. The TUI's Machines rows read
`hub-unavailable` and the selected row's detail says that an Orchbee running as administrator holds
machine connections; `machine` commands, routed commands and the state printer report
`hub-unavailable`, exit 4, or the printer's exit 3, with a message that names the elevated hub.
Each tells the person to close the Orchbee running as administrator or to run this one as
administrator. The TUI then watches that hub and attaches once it goes, which, as for any hub,
happens only when nothing holds it. Another user's processes stay refused at every level.

#### Re-attachment

After `bye` `idle` or a lost connection, the owner re-attaches by the same rules, starting a hub
when none runs, and restores the caller's last `presence`, `asks` and `watch`.

- **An owner opened `present` ends with its hub.** Whether its hub stopped or was lost, it
  re-attaches only to a hub serving the data root at that moment and otherwise ends `closing` at
  once, so it never starts, replaces or waits for a hub.
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

1. `query` gives the process id; no endpoint means no hub runs. A hub started as administrator
   refuses a standard process's `query` at the open, which fails `elevated` at once.
2. The owner opens the process for waiting, then queries again: the same id proves that the handle
   is the hub's and not a reused id's. A process already gone is queried again; one this process
   may not wait for fails `denied` at once.
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

## Machines in Settings

Settings' Machines category, on Windows and macOS where the TUI reaches saved machines, is where a
person adds and looks after them; [native settings](native-settings.md#machines) owns how it is
drawn. It lists this computer and every saved machine, each with what it has free while it is
online and its state otherwise, and a machine's own page offers the one act its state calls for.
The page never reaches a machine or the machines store itself: it hands the TUI one request, which
the TUI carries out on its next tick through [discovery](#the-tui-as-a-hub-client) and the store,
and it learns the result from the facts the TUI gives it on every tick while Settings is open.
Nothing on that path waits for the hub, so a machine that does not answer never stalls the page or
the local Session.

- **Order.** Machines are listed by name, comparing runs of digits by their value and ignoring
  letter case, then by ID, so `devbox2` comes before `devbox10`. Once more than eight machines are
  saved, a filter narrows the list by name or address. Focus follows the machine, not the row,
  when the list re-sorts.
- **Status.** The act a machine's page offers follows its state: Connect while it is disconnected,
  Retry while it is offline, Retry now while it waits to reconnect, Log in while it needs a login,
  nothing while it connects or its terminals are stopping, and Disconnect otherwise. The sentence
  beside it is the state's [explanation](#states-and-details).
- **Disconnect and Connect.** Disconnect saves the machine disabled and then ends its link, so
  nothing, the hub's own retries included, connects it again until Connect saves it enabled; its
  Session and terminals keep running. The hub connects a machine saved enabled once it reloads the
  catalog, as it keeps every enabled machine while the TUI holds it. A save that fails leaves the
  machine as it was and says so.
- **Logins.** Connect, Retry, Retry now and Log in let that machine's login ask this TUI for 60
  seconds, and for as long as its question is open, since only a login the person waits for may
  [ask](#login-questions).
- **Stop its terminals.** Offered while the machine is online. The first Enter says what stops: the
  machine's terminals and the working Agents among them as it last reported them, or every terminal
  when it has not said, as Quit All counts this computer's. The second asks the hub's
  [Session stop](#stopping-a-session), and Esc asks nothing. A view showing that machine returns to
  this computer as the stop begins, since its terminals end; the record stays, and the result
  shows once the hub answers.
- **Remove machine.** It deletes the record at once, asks nothing and stops nothing: the machine's
  Session and terminals keep running, and adding the machine again finds them. Stop its terminals
  is the act that ends work, so removing cannot lose any. A view showing the machine returns to this
  computer.
- **Names.** A rename follows the [catalog rule](#catalog-and-transitions): at most 128 bytes, no
  control characters, and never `local` in any letter case. The name field takes no more than a
  name may hold.
- **What is free.** While the page shows, the TUI asks the hub what this computer and each online
  machine have free, at most once a second, with one request per machine at a time, each answer
  awaited for at most 4 seconds; the hub answers from a sample up to 3 seconds old. A machine shows
  what it has free only while it is online, and a value its source cannot give reads "—".
- **Add.** Add machine… opens the [add form](#adding-a-machine) over Settings; once the form saves
  the machine, it closes and the list focuses the new machine.
- **Outcomes.** How a request ended shows on the page's status line until the next input; one that
  ends while Settings is closed shows when it opens again.

## Persistent TUI connections

The Windows and macOS TUI discovers the saved catalog while open and lists every enabled
machine's Workspaces in the sidebar; `Ctrl+b Shift+M` and the action menu's `Machines…` open
Settings › Machines. There is no permanent footer indicator. Selection is by
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

| Rule            | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Presence        | The first catalog read that holds an enabled Machine of a known connection kind, or the first [add](#adding-a-machine), starts one background holder. It attaches with `presence` and `watch`, starting a hub when none runs, and keeps that attachment, across the owner's re-attachments, until the TUI exits or its hub is asked to stop. The hub's countdown begins only when its last holder leaves, so a TUI opened again within the expiry finds each Machine as the hub kept it: `online` at once, on the same carrier child. A TUI whose catalog never holds such a Machine starts no hub                                                                                                                                                                                                                                                                                                                                        |
| Questions       | The holder attaches with `asks` true and a selection sends `connect` with `origin` `user`, so the hub puts that login's questions to this TUI, as [login questions](#login-questions) shows them. The hub also offers this TUI every `command` episode, which it shows as a [notice](#login-questions) and claims when the person accepts it; the claim, a dismissal and a release ride on the holder's own requests, off the frame                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Off the frame   | Attaching, the hub's events, every request and every channel run on background threads that publish into one shared view and wake the UI loop. A frame reads that view and never waits for the hub                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Row words       | A row shows the hub's state of the Machine only while the holder's owner holds a live connection and the hub has not said it is closing. Otherwise it reads what the holder knows of the hub: Unknown while it attaches or re-attaches, or while the hub has not reported the Machine; Stopped; or Unavailable. No row reads Online without a live attachment, and the holder notices a lost connection within 100 ms                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Hub stopped     | A hub that says it is closing, or that an attachment finds closing, was asked to stop, by a person or by uninstall, or expired with nothing holding it. The holder starts no hub in its place: its rows read Stopped, the picker's status line says machine connections stopped and that choosing a Machine starts them, and Local remains available. The holder joins a hub somebody else starts, which it looks for once a second, and a selection, a person's act, starts one                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Hub crashed     | A connection lost without `closing` is the owner's to re-attach, starting a hub when none runs. When the owner ends, or no hub could start, the rows read Unavailable and the picker's status line says machine connections are unavailable; the holder starts a hub again under the [retry policy](#carrying-a-machine), but none in place of a hub that answers it is stopping. A selection skips the pause                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Lasting failure | A failure that trying again cannot cure is not retried on a timer: a hub of another version that something still holds, another environment's hub, another user's endpoint, a hub [started as administrator](#administrator-and-standard-processes) that refuses this standard process, repeated or handshake protocol violations, or a data root that cannot be used. The rows read Unavailable with the error tone, the picker's status line says machine connections cannot start, and the selected row's details say why and what to do, naming the held version. The holder waits on that hub's process, or, when it cannot open the process, looks at the endpoint once a second, reading a missing name or a hub that names another process to `query` as a later hub, and attaches again once the hub is gone; a selection tries once more at once. A data root that cannot be used names no hub, so only a selection tries again |
| Exit            | Quitting asks the holder to stop and does not wait for it, so the terminal is restored at once. An attachment in progress ends at once and starts no hub; the holder's thread writes nothing to the terminal, holds nothing that keeps the process alive, and ends with the process                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Catalog         | The TUI rereads `machines.json` every second. When the revision it read differs from the hub's last `catalog` revision, the holder asks the hub to `reload`, once per revision, so a Machine saved, changed or removed while the TUI runs reaches the hub                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Re-attachment   | After `reattached` the view forgets every Machine state until the new hub reports it again; the rows read Unknown meanwhile                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |

### Managing machines in the TUI

[Settings › Machines](#machines-in-settings) manages the saved machines; `Ctrl+b Shift+M` and the
action menu's `Machines…` open it. Its Add machine opens the [add form](#adding-a-machine) over the
page, which closes once the add is saved or given up and leaves the page on the machine it saved.
The page carries what the person asks to [discovery](#the-tui-as-a-hub-client) and the machines
store once per frame tick and never waits for the hub; a commit's catalog refreshes the list at once.
Removing or disconnecting the machine on screen returns the TUI to Local once the change is saved,
and stopping its terminals returns to Local as the stop begins, since its terminals end before its
record goes; the page shows the change. When another process removes the machine on screen, the TUI
returns to Local and the status line says the switch did not happen.

Disable saves the Machine disabled and then disconnects it, so nothing connects it again; a save that fails leaves it as it was. Disconnect ends the link without asking; the record and the remote Session stay. Remove and stop asks first and keeps the Machine in the catalog and list while the hub answers `stop-session`, removing the record only once the Session stopped (`stopped` or `not-running`); a stop that fails or finds the Machine gone keeps the record and says so. It is offered only while the Machine is `online`.

### Login questions

The TUI asks a person exactly what `machine connect` asks on a console, from the same wording and
`ssh`'s prompt shown beside it, escaped and bounded. The episode's client-side rules are one model
in the workbench Client crate, apart from the overlay that draws it, so another interface reuses
them; the holder brings the hub's events and carries out the model's deliveries and dismissals on
threads of their own. A machine being added is named by the name its form will save, never by its
draft's id, which the person never sees.

| Rule         | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Notice       | A `command` episode the hub offers shows first as a notice, never as the field, since the person may be typing in a Panel: the Machine and its target, how many commands wait to log in to it, that they fail without a login, and `Esc` Not now and `Enter` Log in. It is a capturing overlay, the one place where a background request takes the keyboard, so every other key and a paste are discarded rather than reaching a Panel or a field; `Enter`, `Esc` and its buttons act only once frames have shown the whole notice for one second in which no key or paste was discarded: every discarded key or paste, and a resize, starts that second again, so a person who is typing, for a Panel or for the question they expect, never accepts or declines it. `Enter` claims the login, and the login overlay opens with the question the hub then puts; `Esc` dismisses it, which fails every waiting command, closes the notice in every interface and mutes the Machine's command logins until the person selects it or every asking interface has closed. The count follows the hub's `waiters` |
| Kinds        | A password, a key passphrase and `other` ask for a secret. A new host shows its algorithm and fingerprint and asks for a typed `yes` before Trust and continue, and Orchbee then answers `yes` itself. The client reads the kind again against the Machine's target, so a question naming another host is `other` and a changed host key never offers trust                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Awaited only | Only the selection the person still waits on, or a `command` login the person accepted from its notice, may ask. A selection that ends any other way, such as by choosing another Machine or letting the picker go before the question came, abandons its login: the contract has no request that cancels a waiting `connect`, so its attempt runs on, and the login the hub claims for it at the first question is dismissed on arrival and never shown, as is a login of any Machine the person does not wait on; that Machine reads `needs-login`. A shown login is never replaced: while one overlay is open, another login is dismissed on arrival                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| On screen    | An answer can be given only once a frame has shown the whole question: Orchbee's asking sentence, `ssh`'s asking line for a question only `ssh` words, a new host's whole fingerprint, the field and the buttons. `ssh`'s lines are kept from the last one up, a `ssh: …` line marking the cut; a fingerprint wraps on lines of its own, never shortened, and widens the overlay where the terminal allows. A window that cannot hold all that shows the size notice instead: the overlay keeps every key, takes only Escape, and keeps the typed field, which shows again once the window grows. A resize counts the question unshown until the next frame shows it                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| The secret   | Typed and pasted characters go straight into the field's fixed buffer, which never grows and is shared, not copied, by every copy of the view. The overlay paints one mask glyph per character, except that a new host's field shows what was typed while it begins `yes`, ignoring case, and the field's debug form shows nothing. Enter moves the buffer to the delivery thread; Escape, the login's end, a dismissal and dropping it wipe it, and a resize or a lost focus keeps it. Enter on an empty password or passphrase sends nothing; another question may be answered empty. Keys that arrive after Enter do not join the answer. What else is wiped, and what is not, is in the [answer path](#answer-path)                                                                                                                                                                                                                                                                                                                                                                                     |

### Session carrier

A remote terminal, a Workspace switch to another machine and Agents mode run their ordinary Client
connection on a private, same-user local endpoint, a pipe on Windows and a socket on macOS, that the TUI creates for each observed
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
endpoint closed until the Machine is `online` again. A reconnect admitted then is a new Session
connection that holds none of the viewers the lost one attached, so the remote Session answers the
Client's next poll that the terminal is unavailable, and the Client retires.

An open remote surface whose link was lost without a Disconnect reattaches by itself to the
Workspace it showed once the hub reads its Machine `online` again; after a Disconnect it waits for
the Machine to be chosen again. The hub's state tells the two apart: a lost link reads
`reconnecting` until the hub restores it by itself, an owner that ends reads no state at all, and a
Disconnect reads `offline` without a cause, or `needs-login` with cause `disconnected`, until
something asks for the Machine. The reattachment is the connection the person's Enter would start,
targeted at that Workspace: it asks nothing, shows no selection running, starts once while the
Machine reads `online`, and replaces the surface only once nothing else holds the keyboard; a
selection the person makes ends it, and one that fails leaves the machine's card. A cached remote
surface whose Client has not noticed the loss comes back the same way once it is shown again; one
whose Client retired while hidden is replaced by the selection's own connection.

The local surface comes back by itself too once its Server connection ends. For up to 150 seconds
the TUI connects only, starting nothing, and opens the Workspace it showed once a Server serves the
Session again: the one a restart handed it to, or one someone started. The status line reads
Reconnecting… meanwhile. A Session the automatic update's idle move ended names the version it moved
to: a TUI whose update to that version is ready leaves for it as a click on Update does, and any
other starts the Session within 20 seconds. A person's stop, or a crash nobody follows with a start,
leaves the lost connection's status once the wait ends; a selection the person makes ends it.

While the machine on screen reads `reconnecting`, `offline`, `needs-login` or `login-failed`, a
card replaces its terminals in the terminal area: a title for the state (`Connection lost`,
`devbox is offline`, `devbox needs login`), what Settings › Machines says of it (a lost connection
says it is being made again and that its terminals keep running), the state's action (`Retry now`,
`Retry` or `Log in`, on Enter) and `Not now` (Esc), which keeps the terminals in view until the
state changes. The action chooses the machine again, so the hub tries at once and a login asks this
TUI. Choosing a Workspace or Agent of a machine that is not connected shows the same card for that
machine, `Connecting to devbox…` while the hub connects it, until the choice lands, another choice
is made, or `Not now` gives it up; a choice that fails keeps the card. While the card shows it
holds the keyboard and the pointer, and no key reaches the terminals beneath it.

Saved machines that need a login nobody is asked for, other than the one whose card says so, are
one notice on the status line that never takes the keyboard: `devbox needs login · Ctrl+b Shift+M
Machines`, or `3 machines need login` for several, naming the key bound to Settings › Machines.
Showing that page puts the notice away until another machine needs a login.

Agents on the connected machines other than the one on screen notify as local ones do, under the
same profile, delay and transitions, with the machine's name beside the Workspace as the sidebar
writes it (`Workspace api @devbox`). Every connected machine's Agents are read while notifications
or sound are on, whatever the sidebar shows. The Agent rows' seen state is this Client's: a
machine's first read, and the machine that comes into view, are a fresh baseline, so a switch
raises nothing; no frame acknowledges another machine's Agent, so its notification waits only for
the delay. Opening the card chooses that Agent as its sidebar row does, through the machine's card
while it connects; the machine on screen is the Session's to notify and open. `orchbee notification
show` in another connected machine's Session shows here as on a Client attached to it: each such
machine's connection listens to the Session its Agents were read from, an ordinary inbox that counts
nowhere else.

Local Panels use their own connection and are untouched. Each connection's threads, credit and pipe
are its own, and the UI loop never reads a channel, so one slow remote never stalls input or drawing
for local work or another Machine.

The Client connection actor owns replacement admission after EOF and never replays submitted
frames. Each Workbench Client surface holds one claim on its carrier, and only the last released
claim performs the shutdown. The observer promotes its initial admission before cloning any
retained or published origin; secondary query/control lanes are logical scopes on that carrier.
Buffers are bounded and backpressure pauses reads. No port forwarding, host-key acceptance,
remote installation or POSIX implementation is added.

A refusal that needs the user to change one side (`environment-mismatch`, `initiator-too-old`, `initiator-too-new` or a Session before the handshake, `fingerprint-mismatch`) is a typed `Refusal` that discovery keeps with the Machine, so selection names it instead of the generic failure, the row shows the side's state word, and automatic restarts never repeat it. Capacity, stopping, and timeout refusals stay transient: the observer starts again under the retry policy.

The picker owns keyboard and mouse input while open. Escape cancels; Enter and
click choose an exact painted identity. Resizing or catalog replacement invalidates
painted rows; pasted text and the remainder of a closing input packet cannot leak
into the underlying terminal. Each asynchronous attempt has an identity and a
cancellation owner; stale completion cannot switch the active machine.

## Automatic machine discovery

The running Windows or macOS TUI reconciles enabled profiles by ID, connection and Session; a connection
kind this build does not know is never observed, and the picker lists that machine as Unsupported
without offering it as a choice. Each endpoint generation owns at most one observer, cancellation
flag and latest-event slot. Disabled, removed and replaced entries cancel their observers, and
retired threads are joined before their slots are reused. Corrupt catalog input suspends all
discovery without changing the current terminal surface. Renaming preserves connection identity.
No observer creates a terminal, claims control, changes focus, starts a Session or installs
software.

### State words

A remote row shows one word, the hub's state of the Machine; every Orchbee interface uses the same
words, and the selected row adds its [details and action](#states-and-details).

`machine_status` in the workbench Client crate maps each hub state to its word and tone, and the crate's localization words them. When the hub's state of a Machine does not hold, as [row words](#the-tui-as-a-hub-client) set out, the row reads the holder's knowledge instead: Unknown, Stopped or Unavailable. A disabled record reads Disabled from the catalog, and a connection kind this build does not know reads Unsupported, without asking the hub; the CLI uses the same words.

### States and details

Beside its word, the selected row shows why the Machine is in its state and the one action that
helps; Enter or a click performs the action. `machine_status` in the workbench Client crate chooses
them from the hub's state and the facts its `machine` event carries, apart from drawing, and its
localization words them, so every interface says the same.

| State                                                     | Detail                                                                                                                                                                                                                                                                                     | Action                                                         |
| --------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------- |
| `online`                                                  | The remote's Orchbee version, what its Session holds, and up to three Project or Workspace names                                                                                                                                                                                           | Open                                                           |
| `connecting`                                              | None                                                                                                                                                                                                                                                                                       | None                                                           |
| `reconnecting`                                            | The cause, worded as for `offline`, or the lost link, and the time of the next attempt                                                                                                                                                                                                     | Retry now                                                      |
| `offline`                                                 | The cause: the host as the record names it could not be reached (timed out, refused, did not resolve, or no route), this computer has no OpenSSH and how to install it, `ssh` failed, the remote broke the protocol, or the remote Orchbee did not start; then when the last attempt ended | Retry, except without OpenSSH                                  |
| `unresponsive`                                            | The link is up but the remote does not answer                                                                                                                                                                                                                                              | Reconnect: end the link and connect                            |
| `disabled`                                                | None                                                                                                                                                                                                                                                                                       | Enable                                                         |
| `needs-login`, `login-failed`, `untrusted`, `key-changed` | As [login questions](#login-questions) explain them                                                                                                                                                                                                                                        | Log in, Log in again, Review and trust; none for a changed key |
| `not-installed`                                           | Orchbee is not on the remote                                                                                                                                                                                                                                                               | None until remote installation                                 |
| `unsupported`                                             | The remote's system and processor where known, and the systems Orchbee runs on; nothing for a connection kind                                                                                                                                                                              | None                                                           |
| `needs-update`                                            | The remote's Orchbee, with its version, or its running Session is too old, and how to update it there                                                                                                                                                                                      | None                                                           |
| `local-needs-update`                                      | This computer's Orchbee is too old for the remote, and to update it here                                                                                                                                                                                                                   | None                                                           |
| `incompatible`                                            | The remote is an Orchbee of another environment                                                                                                                                                                                                                                            | None                                                           |
| A holder status                                           | Why no state of the hub's holds now                                                                                                                                                                                                                                                        | None                                                           |

The host shows a port only when the target names one, since the `ssh` configuration may name
another. Times are local hours and minutes; a system without a local clock shows none. While Orchbee
is being copied, `connecting` and `reconnecting` say so without a byte count, which no `machine`
event carries.

### Observers

For each Machine the hub reads `online`, one observer connects through the [Session carrier](#session-carrier) and keeps one Workbench state subscription. It accepts the initial correlated snapshot and subsequent pushed values; it does not reread on a timer. A summary supplies the Machine's Workspaces only while the shared connection's application-progress deadline remains current. The [carrier contract](../../orchbee-server/README.md#multiplexed-client-delivery) owns the read probe and its resource and correlation bounds. Probe failure withdraws freshness without proving that the Server has died. Gap/Disconnected withdraw the summary and retain the same observer while the actor rebuilds its subscription; only a fresh snapshot restores it.

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

The centered machine picker is a capturing overlay. Remote labels are data and cannot supply terminal control bytes.

Every close, mode, Agent-control and row target is recorded only in the candidate
frame's central hit map. The target carries the picker episode, mode, effective
Agent evaluation basis and ordered stable selections; the list scroll target also
carries the exact painted window. Only a successful terminal draw publishes that
map. Release and each bounded wheel step revalidate the source against live state;
a changed episode, query, filter, order or catalog fails closed, while copy, status,
theme and pointer-only refreshes do not change semantic authority. List wheel moves
only the viewport and never changes the selection; the next key returns the viewport
to the selection, and `Enter` accepts only a selection inside the painted viewport,
so an `Enter` after scrolling it away only reveals it. A remote surface, the one on screen or a
cached one, is reused only while the hub, through a live attachment, reads its Machine `online`: a
Client learns of a lost link only when a request it sent runs out its deadline, seconds after a
Disconnect or a lost link, so its own status never decides reuse. Explicit
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
mutations.

## Cross-machine workspace navigation

The machine picker has Machines, Workspaces and Agents modes. Every build withholds the sidebar Search entry and its key until search is finished, while the Machines action opens Settings › Machines; see [withheld search](tui-shell-layout.md#withheld-search). Workspaces combines Local's last accepted canonical navigation with fresh enabled remote observations; rows carry machine identity, Session ID and a stable Workspace/context ID pair. Nullable Project membership is metadata, not part of selection identity, and labels and row positions never route a selection. Duplicate names are allowed, so same-named Workspaces show their names alone, as in the Sidebar, and the selected result's details name its Project or Ungrouped and the tail of its observed directory, which tells same-named results apart. Filtering never changes the main sidebar, and the query field's position does not move when the number of matches changes. Local retained navigation is a candidate list, not proof of remote liveness; offline or stale remote contexts are not selectable. Renaming, regrouping or reordering preserves selection identity; removal invalidates painted hits and cannot retarget a pending selection.

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

## Cross-machine Agent navigation

Agents mode combines on-demand physical Agent observations from Local and fresh,
enabled remote machines. Each complete read joins a native Workbench snapshot to
bounded physical state batches at one Workbench/report revision. Missing,
unavailable or stale observations are distinct from a successful empty list.
Only observations with a reported Agent label become rows; primary and child
Presentations retain their own identity. Labels are sanitized display data.
Rows carry machine, Session, context, Tab, Panel, Presentation and complete physical
binding; no name or row number grants navigation authority.

Each source registers one shared-carrier invalidation stream
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

The native canonical Agent list retains unattached canonical sessions; cross-machine aggregation of those sessions is a separate capability.

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

`Ctrl+b {` or `Ctrl+b }` requests previous/next Agent across machines; the configured
`previous_agent`/`next_agent` actions use the same projector over every Agent the active source
view admits, whatever status filter or query the picker shows. An eight-second intent
waits for bounded observations, wraps relative to the exact current member, chooses
last/first if it is absent, and excludes unavailable rows. Nothing shows while it waits, and keys
keep reaching the terminal; another choice cancels it. Empty lists, an intent that finds no Agent
within the eight seconds and a single already-focused target change nothing, as in the GUI. Either
way the Agent is chosen as its sidebar row is: on the current machine it is revalidated on a lane of
the current Session, then reached by that Session's own navigation, with no new connection; on
another machine it uses the separately admitted, revalidated, drawable candidate, behind the
machine's card while that machine is not connected. Both preserve the previous surface on
failure.

## Source-owned Agent views

Each Windows Session's Server owns one transient revisioned specification, which the
Workbench `agent-view` request reads, sets and clears; no shipped command sets it, and the
TUI projects it when present. The latest valid set replaces it; a clear
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

A row whose Agent is opening has status `opening`, which no filter value names; it ranks with working. Sort values compare without display sanitation; missing values sort last in either direction, and complete ties preserve canonical placement. Attention ranks Blocked, Error, Done, Working, Idle, Unknown. Sequence is an observation version, not a shared wall clock.

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

## Ordinal Agent focus

Windows `[keys] focus_agent` accepts configured `1..9` digits or ranges, with
prefix/direct modifiers and explicit disabling. It is unbound by default. A
matched binding digit selects the corresponding zero-based entry in the same
active-source-filtered, stable merged list used by previous/next actions; the picker's
status filter and query never narrow that list. The
first nine eligible Agent rows display those numbers; loading/unavailable/empty
placeholders never take an ordinal. The binding registry keeps the actual
configured triggers, including disabled bindings. Shifted digit symbols from
legacy terminals resolve the binding digit; an exact configured symbol binding
takes precedence over that compatibility path.

The existing eight-second intent waits for loading observations. Unknown active
sources remain explicit refusals; absent ordinals neither wrap nor clamp, and a
target already focused causes no attachment. Further input cancels the pending
intent. Once resolved, the selection freezes exact endpoint/member identity and
activates through the same current-Session or candidate path as cycle navigation.
No terminal acknowledgement follows from choosing or reading an ordinal.

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

A visible sidebar or open Workspace or Agents picker mode is a read consumer. Their shared collector
uses the existing endpoint, freshness, deadline, row/byte and cancellation bounds;
closing only the picker leaves a visible sidebar current, while hiding the last
consumer releases source rows/specifications and cancels reads. Sidebar rendering,
scrolling and opening the list do not acknowledge terminal versions. Geometric
sidebar overlays suppress terminal-draw acknowledgements even when short height
has undocked a Wide Client's sidebar.

## Agent sidebar row model

An Agent row is one fixed line beside the vertical guide and status glyph: what names the Agent,
then its provider in secondary text, or the provider alone, as
[Sidebar display identity](native-agent-navigation.md#sidebar-display-identity) defines. Within the
row's width the title gives way first, so the provider stays whole while the title keeps its first
few cells; narrower still, the title alone tells sibling Agents apart. Parent Machine/Workspace text
stays in the tree rather than each Agent. Status color stays on the glyph; the name uses the primary
text role. A pointer resting on the glyph names its state in a tooltip.

No setting changes an Agent row: the file has no Agent row keys, and a file that still sets
`appearance.sidebar.agents` receives an `unknown-key` diagnostic for the table, which is ignored.
Agent rows have no gap between them. The configuration snapshot still carries a fixed
`agentSidebarProfile` that the Client does not read. Paint geometry and input geometry share the
same resolved window; empty trailing space is a non-activating sidebar surface.

## Live terminal title observation model

An Agent row's identity title can come from the OSC title of the exact physical terminal.
The existing Host metadata event is the only title writer. The Worker on every platform
keeps a connection-owned, bounded projection keyed by the complete Host identity
and metadata revision, independent of Agent report sequence, view filters and
Client seen state. Closing the Host connection drops that projection; confirmed
physical disposal removes its entry. Missing observations remain unavailable,
while an observed null title means a confirmed clear. Neither case substitutes a
pane label or a custom `$terminal_title` token.

A read requests exact physical bindings on the existing owned observation lane. The Session checks membership and retained Worker, creation and Host generations before and after the Worker read, and the Worker reads without attaching a viewer, claiming input or resize, reading a terminal screen, starting a Host, or acknowledging a draw. Repeated exact physical identities across presentation aliases are read once and projected back to each independently fenced binding. Titles are sequential observations rather than an atomic snapshot of all Host terminals; the sidebar refreshes at the observation cadence and samples raw glyphs without promising every animation frame.

Newer metadata replaces the prior value; identical replay is a no-op; a decreasing revision, changed replay or reused terminal identity is invalid. The Client paints raw and stripped forms from that same observation. Stripping removes one Windows elevation prefix and at most one recognized leading activity glyph followed by whitespace, preserving Unicode text. Rules inspect the selected original form before display sanitization and grapheme truncation. Title-only changes may alter row height and cancel pending old scroll geometry, but do not change Agent attention or navigation identity.

No new public CLI command, persisted title registry, Host protocol, input authority, or independent terminal acknowledgement follows from these presentation tokens.

Agent rows retain a fixed leading state symbol independently of configured text tokens. `state_icon` remains accepted for configuration compatibility but cannot hide, duplicate, move or recolor that fixed status slot. A trailing blue dot independently reports any unread Agent version; the existing Client acknowledgement owner clears it without rewriting state.

## Workspace sidebar model

The Windows main sidebar resolves workspace rows from exact Session/context identities, not display labels or Agent source-view filtering. The settings owner admits `appearance.sidebar.workspaces` rows and `row_gap`; `$custom` values come from the Workspace report owner. Styles and first-match text rules inspect original values before terminal sanitization. Nonempty configured rows retain their slots when values are missing or hidden; an empty configuration retains one identity row for navigation.

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
`location` shows the observed branch unless it equals the Workspace name, otherwise the name of the
first terminal's recorded directory, else of the canonical Workspace folder; the Client's home folder itself reads `~`, and
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

Agent and workspace observations share the bounded job lifecycle with separate job pools, cancellation and joined disposal. Pure metadata follows commit/expiry invalidations and the carrier progress deadline; external Host/Git sampling retains its ten-second freshness. Rendering performs no I/O. Workspaces and their expanded Agent children share one Client scroll window. Activation rechecks the exact machine/Session/context; labels cannot redirect it. Text-token widths reserve fixed icon/Git-count widths; long branch/custom values shrink before readable ahead/behind counts.

The Session publishes keys, notifications and both bounded canonical sidebar profiles as one candidate. Fatal source failure or a malformed private publication retains every prior Client setting; an invalid configured sidebar uses its safe default and warning in the atomic candidate.
