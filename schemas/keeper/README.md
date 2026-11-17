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
level. Each connection starts with one [`OBHS`](../connection/README.md#frame) exchange whose
`connection` is `keeper`. After the `answer`, both directions carry only `OBKP` frames until either
side closes. The frame has no version field: the answer's generation selects this contract.

- **Generations.** `keeper` is at generation 1 with no capability names. Like `worker-host`, it
  drops no generation within 1.x, because a Host released with generation 1 may hold a terminal for
  as long as its program runs.
- **Capabilities.** A request, member, event, or enumerated value added later comes under a
  capability name and is sent only when the answer lists that name.
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

| Value      | Rule                                                                                                                             |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------- |
| Name       | 1–64 bytes of `a`–`z`, `0`–`9`, and `-`, as in the connection contract; reasons, roles, and outcomes are names                   |
| `id`       | Integer from 1 to 4,294,967,295 that the connection has not used for an earlier request                                          |
| Process    | `{pid, created}`: `pid` an integer from 1 to 4,294,967,295, `created` its creation time                                          |
| Time       | 16 lowercase hexadecimal digits of a `FILETIME`, not all zero                                                                    |
| Handle     | 16 lowercase hexadecimal digits of a handle value in the process that holds it, nonzero and below 2^32, so never a pseudo handle |
| Holding id | 32 lowercase hexadecimal digits, not all zero, minted at random by a Host at a terminal's first `hold`                           |
| Mark id    | 32 lowercase hexadecimal digits, not all zero, minted at random by the owner for one quiesce                                     |
| Terminal   | `{creationId, hostEpoch, terminalId}`: a Terminal Host's terminal identity, each 32 lowercase hexadecimal digits                 |
| `handles`  | `{job, program, console, signal, reference, input, output}`, seven handles                                                       |
| `facts`    | `{terminal, program, console, consoleBuild, signalFormat, launch}`                                                               |

`handles` names, in the process that holds them, the terminal's Job, the program's process, the
OpenConsole process, the pseudoconsole's signal and reference handles, the input pipe's write end,
and the output pipe's read end. In `facts`, `terminal` is the owner's terminal identity, `program`
and `console` are the program's and OpenConsole's processes, `consoleBuild` is OpenConsole's SHA-256
as 64 lowercase hexadecimal digits, `signalFormat` is the console's signal format as an integer
from 1 to 65,535, and `launch` is the launch kind a Host reports in `ReadProcessInfo`.

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
| `mark`            | Owner       | `holding`, `for` (a process), `mark`     | None                                                                 |
| `resume`          | Owner       | `holding`, `mark`                        | `outcome`: `resumed` or `claimed`                                    |
| `inspect`         | Mark target | `holding`, `mark`                        | `handles` in the Keeper, `facts`                                     |
| `claim`           | Mark target | `holding`, `mark`, `program`, `terminal` | None                                                                 |
| `decline`         | Mark target | `holding`, `mark`                        | `outcome`: `declined` or `claimed`                                   |
| `release`         | Owner       | `holding`                                | None                                                                 |

| Reply         | Members besides `type`                              |
| ------------- | --------------------------------------------------- |
| `refused`     | `id` and `reason`                                   |
| `unsupported` | `id` and `request`, a name that is no known request |

- `status-result` lists up to 256 holdings in ascending holding id order, each after `after` when
  it is present. Each entry has `holding`, `owner` (a process), `state` (`held`, `quiesced`, or
  `orphaned`), `for` (the current mark's target, present exactly while a mark is current), and
  `facts`. The first page, requested without `after`, also lists every registered Host as
  `{process, ended}`, where `ended` is a boolean. `next` is the last listed holding id, present
  exactly when more holdings follow.
- A frame from the Keeper without `id` is an event. Generation 1 defines none; a receiver ignores an
  event it does not know.
- `reason`, `role`, `state`, and `outcome` grow: a receiver reads an unknown name as `unknown`, and
  a Host reads every `resume` answer but `resumed` and `claimed` as no answer. Reasons are
  `not-registered`, `wrong-role`, `not-owner`, `not-target`, `unknown-holding`, `holding-exists`,
  `marked`, `stale-mark`, `identity-mismatch`, `capacity`, `draining`, and `handle-failed`.

## Admission and roles

- The Keeper takes the peer's process id from the pipe before it decodes any frame.
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
| `hold` by registered Host H                                                             | Absent                                        | Held by H                                     |
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

- `hold` duplicates the seven handles from the requesting Host with `DUPLICATE_SAME_ACCESS` and no
  inheritance, then checks its own duplicates: each object's type, the program's and OpenConsole's
  identities against `facts`, and the program's membership in the Job. It records the Host as owner
  only when every step succeeds and otherwise closes what it made: a failed duplication gets
  `handle-failed` and a failed check `identity-mismatch`. A 1,025th holding gets `capacity`, and a
  draining Keeper `draining`. A holding id that exists gets `holding-exists`, except a repeat by the
  owner of a held holding, which gets `hold-result` and duplicates nothing.
- `mark` needs the owner (`not-owner`) and a `for` that names another registered Host that has not
  ended (`not-registered`). A holding already quiesced under another mark gets `marked`.
- `claim` needs `program` to equal the holding's program (`identity-mismatch`); `terminal` replaces
  the owner's terminal identity in `facts`. `inspect`, `claim`, and `decline` from a Host other
  than the current mark's target get `not-target`.
- `release` of a quiesced holding gets `marked`, and from another Host `not-owner`.
- A request naming a holding id that does not exist gets `unknown-holding`, except `release`.
- A repeated request whose effect already holds gets the same answer: a `hold` of the same holding
  by its owner, a `mark` of the same mark for the same target, a `resume` by the owner while no mark
  is current (`resumed`), a `claim` of the mark it consumed by the current owner, a `decline` of a
  mark that is neither current nor consumed (`declined`), and a `release` of an absent holding.
- A holding remembers the Host it was claimed from, and the mark that claim consumed, until that
  Host ends or is unregistered, or the holding is released.
- In generation 1 only a mark's target claims an orphaned holding; an unmarked orphan keeps its
  program running until `exit`.
- The Keeper closes a holding's duplicates only at `release` and when it ends, so a handle value
  that `inspect` answered stays valid while its mark is current.

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
  request sent on that Host's connection and its expected reply; `{ends}`, the named Host's end; or
  `{exit: true}`, an `OBHS` `exit` after which the Keeper has ended and no program has. `expect`
  gives the holdings after the last step as `status-result` entries without `facts`, naming the
  owner and the mark target by Host name.
- A runner replaces each `hold`'s `handles` and `facts` with a real terminal's, and the `program`
  and `terminal` of a `claim` with that terminal's program and the claimant's identity; it compares
  an `inspect-result` without its handle values.
- Every binding consumes every fixture. The Rust binding, which comes with the Worker in H2, also
  checks [`v1/keeper.schema.json`](v1/keeper.schema.json) against each case.
