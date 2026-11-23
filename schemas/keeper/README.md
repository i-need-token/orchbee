# Keeper connection contract

## Authority

This directory is the language-neutral authority for generation 1 of `keeper`, the negotiated
connection a Worker and each Terminal Host open to the Session's Keeper, and for the `OBKP` frame it
carries. The Keeper holds duplicates of the handles that keep each held terminal's program running
and arbitrates which Host may read it; it never reads or writes terminal data.

| File                                                                           | Content                                                                    |
| ------------------------------------------------------------------------------ | -------------------------------------------------------------------------- |
| This README                                                                    | Frame, admission, messages, holding states, bounds, freezing, and fixtures |
| [`v1/keeper.schema.json`](v1/keeper.schema.json)                               | JSON Schema of every message; decoder-only rules are listed under Payload  |
| [`v1/fixtures/`](v1/fixtures/)                                                 | Valid, boundary, invalid, and sequence fixtures, grouped by freeze point   |
| [Connection contract](../connection/README.md)                                 | `OBHS`, the generation registry, and the development fingerprint           |
| [Host handover](../../proposals/platform/host-handover.md#the-keeper-protocol) | The accepted design and its decision record                                |

`apps/terminal-host/src/keeper_protocol` is the Zig binding; it serves the Keeper and the Host. The
Rust binding comes with its first consumer, the Worker. Bindings are independent implementations of
this contract; nothing generates source from it. The registry's `keeper` fingerprint inputs are
this directory, the connection contract, both `OBHS` bindings, and each binding of this contract.

## Handshake and carrier

The Keeper runs as `orchbee-terminal-host keep` and serves its own runtime directory and endpoint
through the Terminal Host's Local IPC, with every protection a Host endpoint has: the same-user
check at the first byte, the pipe's DACL and integrity label, and no write from a lower integrity
level on Windows, and the private folder, the same-user check, and the lifetime lock of a Host's
Unix socket on macOS. Each connection starts with one [`OBHS`](../connection/README.md#frame) exchange whose
`connection` is `keeper`. After the `answer`, both directions carry only `OBKP` frames until either
side closes. The frame has no version field: the answer's generation selects this contract.

- **Generations.** `keeper` is at generation 1. Like `worker-host`, it drops no generation within
  1.x, because a Host released with generation 1 may hold a terminal for as long as its program
  runs.
- **Capabilities.** A request, member, event, or enumerated value added later comes under a
  capability name and is sent only when the answer lists that name. `recovery` adds the Worker's
  `assign` and `discard` and the reason `not-orphaned`; `programs` adds the Host's `hold-program`,
  the program holdings it makes, and their entries in a Worker's `status`. `pty` adds the Host's `hold-pty`, the PTY holdings it
  makes, and their entries in a Worker's `status`; `parent` adds the Host's `spawn` and `reap`, the
  event `exited`, and the reasons `spawn-failed` and `running` (see [PTY holdings](#pty-holdings)).
  The Keeper answers `assign`, `discard`, `hold-program`, `hold-pty`, `spawn`, and `reap` with
  `unsupported` on a connection whose answer did not list their capability, and lists program and
  PTY holdings only to a Worker whose answer listed `programs` or `pty`. A Windows Keeper serves
  `programs` and `recovery`; a macOS Keeper serves `pty`, `parent`, and `recovery`.
- **Control requests.** The Keeper answers `OBHS` control requests on a control slot. `query`
  reports one process entry of role `keeper` whose `terminals` counts its holdings and whose one
  connection entry is `keeper`. After `drain` it answers `register-host` and `hold` with `draining`
  and exits once it holds nothing. `exit` replies, closes every duplicate, and ends the Keeper; it
  terminates no Job and no process, so it ends a program only where the Keeper held the last handle.
- **Slots.** The Keeper serves at most 256 Host connections, 1 Worker connection, and 8 connections
  that are not yet admitted or carry a control request. A connection beyond these bounds closes at
  once, without a reply. A connection has 2 seconds from its accept for its `OBHS` frame and 2
  seconds after the `answer` for its `hello`; otherwise it closes.

## Frame

| Field          | Encoding                                    |
| -------------- | ------------------------------------------- |
| Magic          | The four ASCII bytes `OBKP`                 |
| Payload length | Big-endian `u32`, from 1 to 1,048,576       |
| Payload        | Exactly that many bytes holding one message |

- A reader rejects a byte that differs from `OBKP` as soon as it arrives, and an out-of-bounds length
  before it reads or allocates the payload. It never reads past the declared payload.
- Decoding one complete in-memory frame also rejects truncation and trailing bytes.

## Payload

The payload follows the connection contract's [payload rules](../connection/README.md#payload):
strict UTF-8 JSON holding one closed object with a `type` member, no repeated member at any depth,
at most 32 levels of nesting, integer members written as plain integer tokens, no `null` member,
optional members omitted when absent, and `extensions` as the only open slot, at the message's top
level.

The JSON Schema states shapes and value grammars. These rules are checked by decoders only:
duplicate members, nesting depth, integer spelling, an all-zero holding id, mark id, creation time,
or handle, the ascending holding order of a status page, a mark target absent from a `held` entry
and present in a `quiesced` one, the direction of a message, and the request-and-reply pairing. A
frame from the Keeper without `id` is an event and an initiator's frame without `id` is invalid,
whatever its `type`.

## Values

| Value             | Rule                                                                                                                             |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Name              | 1–64 bytes of `a`–`z`, `0`–`9`, and `-`, as in the connection contract; reasons, roles, and outcomes are names                   |
| `id`              | Integer from 1 to 4,294,967,295 that the connection has not used for an earlier request                                          |
| Process           | `{pid, created}`: `pid` an integer from 1 to 4,294,967,295, `created` its creation time                                          |
| Time              | 16 lowercase hexadecimal digits, not all zero: a `FILETIME` on Windows, the start time in microseconds since 1970 on macOS       |
| Handle            | 16 lowercase hexadecimal digits of a handle value in the process that holds it, nonzero and below 2^32, so never a pseudo handle |
| Holding id        | 32 lowercase hexadecimal digits, not all zero, minted at random by a Host at a terminal's first `hold`                           |
| Mark id           | 32 lowercase hexadecimal digits, not all zero, minted at random by the owner for one quiesce, or by the Worker for one `assign`  |
| Terminal          | `{creationId, hostEpoch, terminalId}`: a Terminal Host's terminal identity, each 32 lowercase hexadecimal digits                 |
| `handles`         | `{job, program, console, signal, reference, input, output}`, seven handles                                                       |
| `facts`           | `{terminal, program, console, consoleBuild, signalFormat, launch}`                                                               |
| Program `handles` | `{job, process, stdout, stderr}`: two handles, and the read end of each captured stream, both optional                           |
| Program `facts`   | `{process, programId, label}`                                                                                                    |

`handles` names, in the process that holds them, the terminal's Job, the program's process, the
OpenConsole process, the pseudoconsole's signal and reference handles, the input pipe's write end,
and the output pipe's read end. In `facts`, `terminal` is the owner's terminal identity, `program`
and `console` are the program's and OpenConsole's processes, `consoleBuild` is OpenConsole's SHA-256
as 64 lowercase hexadecimal digits, `signalFormat` is the console's signal format as an integer
from 1 to 65,535, and `launch` is the launch kind a Host reports in `ReadProcessInfo`.

A program holding, under `programs`, keeps a background program a Host runs without a terminal.
Its `handles` name the program's Job, its process, and the read end of each stream the Host
captures. In its `facts`, `process` is the program's process, `programId` the 32 lowercase
hexadecimal digits of the id its Worker gave it, and `label` the Worker's record of it: 1 to 2,048
characters of the base64 alphabet, which the Keeper returns unread.

## Messages

After the `answer`, the initiator sends requests and the Keeper sends replies. A request carries
`type` and `id`. Its reply carries the same `id` and is `<type>-result`, `refused` with `reason`,
or `unsupported` with `request`, the requested type. One request is outstanding per connection.

| Request           | Sender      | Members besides `type` and `id`          | Result members besides `type` and `id`                               |
| ----------------- | ----------- | ---------------------------------------- | -------------------------------------------------------------------- |
| `hello`           | Initiator   | `role`: `worker` or `host`               | None                                                                 |
| `register-host`   | Worker      | `process`                                | None                                                                 |
| `unregister-host` | Worker      | `process`                                | None                                                                 |
| `status`          | Worker      | `after`, an optional holding id          | `holdings`, `hosts` on the first page only, `next` while more remain |
| `hold`            | Host        | `holding`, `handles`, `facts`            | None                                                                 |
| `hold-program`    | Host        | `holding`, program `handles` and `facts` | None                                                                 |
| `mark`            | Owner       | `holding`, `for` (a process), `mark`     | None                                                                 |
| `resume`          | Owner       | `holding`, `mark`                        | `outcome`: `resumed` or `claimed`                                    |
| `inspect`         | Mark target | `holding`, `mark`                        | `handles` in the Keeper, `facts`, of the holding's kind              |
| `claim`           | Mark target | `holding`, `mark`, `program`, `terminal` | None                                                                 |
| `decline`         | Mark target | `holding`, `mark`                        | `outcome`: `declined` or `claimed`                                   |
| `release`         | Owner       | `holding`                                | None                                                                 |
| `assign`          | Worker      | `holding`, `for` (a process), `mark`     | None                                                                 |
| `discard`         | Worker      | `holding`                                | None                                                                 |
| `hold-pty`        | Host        | `holding`, PTY `facts`                   | None                                                                 |
| `spawn`           | Host        | `holding`, `input`, `launch`, `facts`    | `leader`, a process                                                  |
| `reap`            | Owner       | `holding`                                | None                                                                 |

| Reply         | Members besides `type`                              |
| ------------- | --------------------------------------------------- |
| `refused`     | `id` and `reason`                                   |
| `unsupported` | `id` and `request`, a name that is no known request |

- `status-result` lists up to 256 holdings in ascending holding id order, each after `after` when
  it is present. Each entry has `holding`, `owner` (a process), `state` (`held`, `quiesced`, or
  `orphaned`), `for` (the current mark's target, present exactly while a mark is current), and
  `facts`. A program holding's entry also has `kind`, which is `program`, and program `facts`; a
  receiver reads an entry of another `kind` as a holding it does not know. The first page,
  requested without `after`, also lists every registered Host as
  `{process, ended}`, where `ended` is a boolean. `next` is the last listed holding id, present
  exactly when more holdings follow.
- A frame from the Keeper without `id` is an event. `parent` defines `exited`, `{holding, status}`;
  a receiver ignores an event it does not know.
- `reason`, `role`, `state`, and `outcome` grow: a receiver reads an unknown name as `unknown`, and
  a Host reads every `resume` answer but `resumed` and `claimed` as no answer. Reasons are
  `not-registered`, `wrong-role`, `not-owner`, `not-target`, `unknown-holding`, `holding-exists`,
  `marked`, `stale-mark`, `identity-mismatch`, `capacity`, `draining`, `handle-failed`, under
  `recovery`, `not-orphaned`, and, under `parent`, `spawn-failed` and `running`.

## Admission and roles

- The Keeper takes the peer's process id from the pipe, or on macOS from the socket with
  `LOCAL_PEERPID`, before it decodes any frame.
- The initiator's first `OBKP` frame is `hello`. Any other first request, a frame that does not
  decode, an `id` the connection already used, a second request while one is outstanding, and a
  message in the wrong direction close the connection without a reply.
- `hello` with role `worker` is admitted from a process that is not a registered Host while no other
  Worker connection is open; otherwise it gets `refused` with `wrong-role` from a registered Host
  and `capacity` while another Worker connection is open. `hello` with role `host` is admitted only
  from a registered Host that has not ended; any other peer gets `refused` with `not-registered`.
  An unknown role gets `refused` with `wrong-role`. A refused `hello` closes the connection.
- After `hello`, a request that the connection's role does not send, a second `hello` included,
  gets `refused` with `wrong-role`, and an unknown request gets `unsupported`; the connection stays
  open.
- `register-host` opens the process with `SYNCHRONIZE`, `PROCESS_QUERY_LIMITED_INFORMATION`, and
  `PROCESS_DUP_HANDLE` and checks its creation time: a process that cannot be opened gets
  `handle-failed`, another creation time `identity-mismatch`, a 257th Host `capacity`, and a
  draining Keeper `draining`. The Keeper keeps the handle while the Host is registered.
- `unregister-host` of a Host that owns a holding or is the target of a current mark gets `refused`
  with `holding-exists`. Otherwise it forgets the Host and closes that Host's connections.
- A Host's end orphans the holdings it owns, clears what holdings remember of claims from it, and
  leaves it registered, as ended, until no holding names it as owner.
- A failed check, a refused request, and a connection that closes change nothing else: nothing a
  peer sends or does ends the Keeper or a holding.

## Holding states

The Keeper applies one request at a time across all its connections, under a lock held across no
I/O and no wait. Each connection reads and writes on its own and keeps at most one reply; a peer
that has not taken its reply within 2 seconds loses its connection.

| Event                                                                                   | From                                          | To                                            |
| --------------------------------------------------------------------------------------- | --------------------------------------------- | --------------------------------------------- |
| `hold` or `hold-program` by registered Host H                                           | Absent                                        | Held by H                                     |
| `mark` M by H for registered Host N                                                     | Held by H                                     | Quiesced, H for N under M                     |
| `resume` of M by H, or `decline` of M by N                                              | Quiesced, H for N under M                     | Held by H                                     |
| `inspect` of M by N                                                                     | Quiesced or orphaned, H for N under M         | Unchanged                                     |
| `claim` of M by N naming H's program                                                    | Quiesced or orphaned, H for N under M         | Held by N, claimed from H under M: the commit |
| `resume` of M by H                                                                      | Held by N or orphaned, claimed from H under M | Unchanged, answered `claimed`                 |
| `claim` or `decline` of M by N                                                          | Held by N, claimed from H under M             | Unchanged, answered `claimed`                 |
| `inspect` or `claim` of a mark that is not current, or `resume` of one while another is | Any                                           | Unchanged, refused `stale-mark`               |
| H ends                                                                                  | Held by H                                     | Orphaned                                      |
| H ends                                                                                  | Quiesced, H for N under M                     | Orphaned, H for N under M                     |
| `decline` of M by N                                                                     | Orphaned, H for N under M                     | Orphaned                                      |
| `release` by H                                                                          | Held by H                                     | Absent                                        |
| `exit`                                                                                  | Any                                           | The Keeper closes every duplicate and ends    |
| `assign` of M by the Worker for registered Host N                                       | Orphaned, unmarked or for an ended Host       | Orphaned, H for N under M                     |
| `discard` by the Worker                                                                 | Orphaned, unmarked or for an ended Host       | Absent; the Keeper closes its duplicates      |

- `hold` duplicates the seven handles from the requesting Host with `DUPLICATE_SAME_ACCESS` and no
  inheritance, then checks its own duplicates: each object's type, the program's and OpenConsole's
  identities against `facts`, and the program's membership in the Job. It records the Host as owner
  only when every step succeeds and otherwise closes what it made: a failed duplication gets
  `handle-failed` and a failed check `identity-mismatch`. A 1,025th holding gets `capacity`, and a
  draining Keeper `draining`. A holding id that exists gets `holding-exists`, except a repeat by the
  owner of a held holding, which gets `hold-result` and duplicates nothing.
- `hold-program` does the same with a program's handles: it checks that the Job is a Job, that the
  process is a process the Keeper may query and wait on whose identity matches `facts` and which
  runs in the Job, and that each stream is a file it may read. A program holding then moves through
  the same states as a terminal's, and a request that applies to a holding applies to it.
- `mark` needs the owner (`not-owner`) and a `for` that names another registered Host that has not
  ended (`not-registered`). A holding already quiesced under another mark gets `marked`.
- `claim` needs `program` to equal the holding's program, its process for a program holding
  (`identity-mismatch`). A terminal holding's claim names `terminal`, which replaces the owner's
  terminal identity in `facts`; a program holding's claim names none. A claim that does not match
  its holding's kind gets `identity-mismatch`. `inspect`, `claim`, and `decline` from a Host other
  than the current mark's target get `not-target`.
- `release` of a quiesced holding gets `marked`, and from another Host `not-owner`.
- Rows 14 and 15 come with `recovery`. `assign` and `discard` need an orphaned holding
  (`not-orphaned`) that no live Host may still claim: a current mark whose target has not ended gets
  `marked`. `assign` needs a `for` that names a registered Host that has not ended
  (`not-registered`). `discard` ends the program once no other process holds its Job, since its
  owner has ended.
- A request naming a holding id that does not exist gets `unknown-holding`, except `release` and
  `discard`.
- A repeated request whose effect already holds gets the same answer: a `hold` of the same holding
  by its owner, a `mark` of the same mark for the same target, a `resume` by the owner while no mark
  is current (`resumed`), a `claim` of the mark it consumed by the current owner, a `decline` of a
  mark that is neither current nor consumed (`declined`), a `release` or `discard` of an absent
  holding, and an `assign` of the current mark for the same target.
- A holding remembers the Host it was claimed from, and the mark that claim consumed, until that
  Host ends or is unregistered, or the holding is released.
- Only a mark's target claims an orphaned holding. Without `recovery` an unmarked orphan keeps its
  program running until `exit`; with it the Worker assigns the orphan to the Host that recovers it,
  or discards it.
- The Keeper closes a holding's duplicates only at `release`, at `discard`, and when it ends, so a
  handle value that `inspect` answered stays valid while its mark is current.

## PTY holdings

On macOS a holding keeps a terminal's PTY master. A descriptor never appears in a payload: it moves
by `SCM_RIGHTS` with the frame of the message that carries it, at most 64 per message. `hold-pty`
carries the master; `spawn` carries the master, the slave, and the standard input when `input` is
true, in that order; a PTY holding's `inspect-result` carries the master. A receiver counts the
descriptors that arrived against what the message carries: a short count, or control data the
kernel truncated, fails the request as `handle-failed` and closes what arrived.

| Value             | Rule                                                                                                                                                                                                                                                                                               |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PTY `facts`       | `{terminal, leader, slave, launch}`: the owner's terminal identity, the leader's process, the slave's name as `ptsname` reads it (`/dev/`, at most 128 bytes), and the launch kind                                                                                                                 |
| `spawn`'s `facts` | `{terminal, slave, launch}`, PTY `facts` without the leader                                                                                                                                                                                                                                        |
| `launch`          | `{executable, arguments, environment, candidates, directory}`: the executable as named, the arguments after the first, `name=value` entries, the paths to try in order when the executable names no folder, and the optional directory; no text has a NUL, and each list has at most 4,096 entries |
| Exit status       | `{code}`, from 0 to 255, or `{signal}`, from 1 to 255                                                                                                                                                                                                                                              |

- `register-host` on macOS checks the process's start time as the kernel's process record
  (`KERN_PROC_PID`) holds it and watches its end with a `kqueue` `EVFILT_PROC` filter; a process gone before the watch is armed gets
  `handle-failed`.
- `hold-pty` checks the master it received: a PTY master whose slave name equals `slave`, and a
  leader that runs with the recorded start time. A failed check gets `identity-mismatch` and closes
  the descriptor. The holding is not parented.
- `spawn` starts the leader as a Host does: in the directory, in a new session with the slave as
  its controlling terminal and standard streams, the standard input in place of the slave's when
  sent, default signal dispositions, and no other descriptor; it runs the executable, or each
  candidate in order, with `execve`. A child that cannot start gets `spawn-failed`. The Keeper
  closes the slave and the standard input, keeps the master, and answers the leader's process; the
  holding is owned by that Host and parented.
- The Keeper watches a parented leader from its fork with `EVFILT_PROC` and `NOTE_EXIT`, and polls
  it once a second with a `waitid` that never reaps. When the leader ends, the Keeper records the
  status that `waitid(P_PID, …, WEXITED | WNOWAIT)` reads and sends `exited` once for that end on a
  connection of its owner that negotiated `parent`; a claim makes it due again to the claimant. A PTY holding's status
  entry and `inspect-result` carry `parented` and, once a parented leader ended, `ended`.
- `reap` by the owner reaps an ended parented leader; a running one gets `running`. A repeat, and a
  `reap` of a holding the Keeper does not parent, gets `reap-result`. `release` reaps an ended
  leader, and the Keeper reaps a leader that ends after its release or `discard`; it reaps nothing
  else, so a leader's process id stays its session's until its owner contains it. A `release` or
  `discard` that would leave more running released leaders than twice the holding bound gets
  `capacity`.
- `claim` of a PTY holding names the leader as `program`.

## Bounds

- At most 1,024 holdings and 256 registered Hosts; a request beyond either gets `capacity`.
- A reply is due within 2 seconds of its request. A requester that has none by then closes the
  connection and counts the Keeper as unresponsive.

## Freezing

Every later Keeper and Worker must serve the Hosts of the first release whose Hosts speak this
contract, so its fixtures freeze in groups, each when a released process first depends on it. A
group's files join the connection registry's
[frozen fixtures](../connection/README.md#frozen-fixtures) with the change that makes a Host or a
Worker offer `keeper` and send that group, and the next release seals them; they never change
afterwards. A release that only carries this contract, while no Host or Worker offers `keeper`,
seals none of them, so no bytes are sealed before a real Keeper and Host exercise them.

| Group    | Messages                                                                                             | Joins the frozen fixtures with                     |
| -------- | ---------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Owner    | The frame, `hello`, `hold`, `mark`, `resume`, `release`, their results, `refused`, and `unsupported` | The change that makes a Host offer `keeper`        |
| Claimant | `inspect`, `claim`, `decline`, and their results                                                     | The H2 change that makes a release Worker offer it |
| Worker   | `register-host`, `unregister-host`, `status`, and their results                                      | The H2 change that makes a release Worker offer it |
| Recovery | `assign`, `discard`, their results, and `not-orphaned`                                               | The H2 change that makes a release Worker offer it |
| PTY      | `hold-pty`, `spawn`, `reap`, their results, `exited`, and PTY status entries and inspect results     | The change that makes a macOS Host offer `keeper`  |

A later Keeper answers an owner's `resume` of its current mark only with `resumed` or `claimed`.
No release Session moves a terminal into a Host of the first release, so the claimant's requests
first meet another version with H2's Keeper, and the Worker's requests with H2's Worker.

## Fixtures and conformance

| File                                                             | Content                                                  |
| ---------------------------------------------------------------- | -------------------------------------------------------- |
| [`owner.json`](v1/fixtures/owner.json)                           | Valid, boundary, and invalid cases of the owner group    |
| [`owner-sequences.json`](v1/fixtures/owner-sequences.json)       | The state-table rows that only owner requests drive      |
| [`claimant.json`](v1/fixtures/claimant.json)                     | Valid, boundary, and invalid cases of the claimant group |
| [`claimant-sequences.json`](v1/fixtures/claimant-sequences.json) | The state-table rows that a mark target's requests drive |
| [`worker.json`](v1/fixtures/worker.json)                         | Valid, boundary, and invalid cases of the worker group   |
| [`recovery.json`](v1/fixtures/recovery.json)                     | Valid, boundary, and invalid cases of the recovery group |
| [`recovery-sequences.json`](v1/fixtures/recovery-sequences.json) | The state-table rows that the Worker's requests drive    |
| [`pty.json`](v1/fixtures/pty.json)                               | Valid, boundary, and invalid cases of the PTY group      |

- A message file holds `valid`, `boundary`, and `invalid` arrays. Message cases hold `name`,
  `direction` (`initiator`, or `acceptor` for the Keeper), and `payload`, the exact JSON text. A
  valid case decodes in its direction, is refused from the other direction, re-encodes to an equal
  message, and satisfies the schema. A boundary case holds values at their limits. An invalid case
  fails to decode in its direction; `layer` names the first layer that refuses it: `frame`,
  `encoding`, `schema` (the schema refuses it too), or `decoder` (the schema accepts it). A frame
  case holds `frameHex` and `reader`, how a stream reader ends: `foreign`, `malformed`, `closed`,
  or `message` when it reads one frame and leaves the remaining bytes.
- A sequence case names the state-table `row` it covers, when it covers one, the `hosts` registered
  before its first step, as `{name, process}`, and its `steps`. The runner opens one connection per
  Host and sends `hello` with role `host` before the steps. A step is `{host, request, reply}`, a
  request sent on that Host's connection and its expected reply; `{worker: true, request, reply}`,
  one sent on the Worker's connection, whose `id` the runner replaces with an unused one in both;
  `{ends}`, the named Host's end; or `{exit: true}`, an `OBHS` `exit` after which the Keeper has
  ended and no program has. After a `discard-result` the discarded holding's program has ended.
  `expect` gives the holdings after the last step as `status-result` entries without `facts`,
  naming the owner and the mark target by Host name.
- A runner replaces each `hold`'s `handles` and `facts` with a real terminal's, and the `program`
  and `terminal` of a `claim` with that terminal's program and the claimant's identity; it compares
  an `inspect-result` without its handle values. The sequences hold no program, whose holding
  follows the same rows.
- Every binding consumes every fixture. The Rust binding, which comes with the Worker in H2, also
  checks [`v1/keeper.schema.json`](v1/keeper.schema.json) against each case.
