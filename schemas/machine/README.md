# Machine connection contracts

## Authority

This directory is the language-neutral authority for generation 1 of the two negotiated connections
of remote machines, for the `OBMX` frame both share, and for the answer delivery record.

| Connection     | Initiator                                         | Acceptor                                  | Carrier                                               |
| -------------- | ------------------------------------------------- | ----------------------------------------- | ----------------------------------------------------- |
| `machine-link` | The hub                                           | `orchbee --machine-link-v1` on the remote | The helper's standard input and output, through `ssh` |
| `machine-hub`  | A TUI, a CLI command, or Desktop's bundled Client | The hub, one per user and environment     | The hub's same-user local endpoint                    |

| File                                           | Content                                                             |
| ---------------------------------------------- | ------------------------------------------------------------------- |
| This README                                    | Frame, channels, flow control, every record, bounds, and violations |
| [`v1/fixtures/`](v1/fixtures/)                 | Valid, boundary, invalid, and sequence fixtures                     |
| [Connection contract](../connection/README.md) | `OBHS`, the generation registry, and the development fingerprint    |

[`crates/machine-protocol`](../../crates/machine-protocol/README.md) is the Rust binding: a codec
and a channel state machine without I/O.
Product behavior behind these records — retry policy, login policy, copy, the launch line that
starts the helper, the hub's endpoint name and lock — belongs to
[Remote machines](../../apps/orchbee/docs/machines.md), not to this contract.

## Handshake and carrier

Each connection starts with one [`OBHS`](../connection/README.md#frame) exchange whose `connection`
is `machine-link` or `machine-hub`. After the `answer`, both directions carry only `OBMX` frames
until the carrier closes. The frame has no version field: the answer's generation selects this
contract for the connection's lifetime, and a later generation of either connection states its own
frames.

- **Capabilities.** A member, request, event, channel kind, or enumerated value marked with a
  capability name under [Baseline and capabilities](#baseline-and-capabilities) is sent
  only when the answer lists that name. A peer without it loses that feature and nothing else.
- **Hub admission.** The hub checks that the connecting peer runs as its own user before it reads a
  byte. It has 256 connection slots and 4 control slots, and rejects offers with `stopping` once
  its lifetime state is `closing`.
- **Hub control requests.** The hub answers `query` with one process entry of role `machine-hub`,
  `terminals` 0, and `state` `ready`, or `stopping` once `closing`. `drain` moves it to `closing`.
  `exit` replies, ends every `ssh` child, and ends the process without the closing allowance.
- **Link start.** The link's first byte in each direction is the first byte of an `OBHS` frame.
  Everything the remote shell or the launch line writes before the helper starts belongs to the
  launch line, which consumes it. The helper refuses to start when its standard input or output is
  a terminal, and writes nothing but `OBHS` and `OBMX` bytes to standard output.
- **Link candidate deadline.** The helper applies the 2-second `OBHS` candidate deadline from its
  start. The hub therefore writes its offer together with the last input the launch line reads,
  without waiting for a line from the remote first.
- **Link control requests.** The helper answers `query` with one process entry of role
  `machine-link` and `terminals` 0, and answers `drain` and `exit` by replying and exiting.
- **Standard error.** The helper's and `ssh`'s standard error is not part of the link. The hub
  reads it only to classify a failed start, keeps a bounded tail for diagnostics, and never copies
  it into a record of either connection or into product copy. It may hold unrelated text, such as
  PowerShell CLIXML progress records.
- **`ssh` exit status.** No record carries an `ssh` exit status: OpenSSH exits 255 for every
  connection failure, so the hub derives a [cause](#machine-state) from diagnostics and reports
  `ssh-failed` when none matches.

## Frame

| Offset | Bytes  | Field    | Encoding                                 |
| ------ | ------ | -------- | ---------------------------------------- |
| 0      | 4      | Magic    | The four ASCII bytes `OBMX`              |
| 4      | 1      | Kind     | A frame kind from the table below        |
| 5      | 3      | Reserved | Zero                                     |
| 8      | 4      | Channel  | Big-endian `u32`                         |
| 12     | 4      | Length   | Big-endian `u32`, the payload byte count |
| 16     | Length | Payload  | As the kind states                       |

| Kind | Name      | Channel | Sender             | Payload                                  | Length      |
| ---- | --------- | ------- | ------------------ | ---------------------------------------- | ----------- |
| 1    | `message` | 0       | Either end         | One [control message](#control-messages) | 2 to 65,536 |
| 2    | `open`    | New id  | The channel opener | JSON object with `kind`                  | 2 to 65,536 |
| 3    | `accept`  | Open id | The responder      | JSON object                              | 2 to 65,536 |
| 4    | `data`    | Open id | Either side        | Opaque bytes                             | 1 to 32,768 |
| 5    | `credit`  | Open id | Either side        | Big-endian `u32` from 1 to 4,194,304     | 4           |
| 6    | `end`     | Open id | Either side        | None                                     | 0           |
| 7    | `close`   | Open id | The responder      | JSON object with `outcome`               | 2 to 65,536 |
| 8    | `reset`   | Open id | Either side        | JSON object with `reason`                | 2 to 65,536 |
| 9    | `ping`    | 0       | Either end         | 8 opaque bytes                           | 8           |
| 10   | `pong`    | 0       | Either end         | The 8 bytes of the `ping` it answers     | 8           |

- A reader checks the whole 16-byte header — magic, a known kind, zero reserved bytes, a channel
  the kind admits, and a length inside the kind's bounds — before it reads or allocates the
  payload, and never reads past the declared payload. Decoding one complete in-memory frame also
  rejects truncation and trailing bytes.
- Readers and writers support partial I/O. A writer never interleaves the bytes of two frames.

## JSON payloads

The payload of a `message`, `open`, `accept`, `close`, or `reset` frame is one JSON object under
every rule of the [`OBHS` payload](../connection/README.md#payload): strict UTF-8 RFC 8259, no
repeated member name at any depth, nesting at most 32 deep, plain integer tokens, closed objects,
no `null`, optional members omitted when absent, and `extensions` as the only open slot, on the
top-level object only.

- Every payload object may carry `extensions` under the `OBHS` rule; no nested object does, and no
  record of this contract puts a meaning there in generation 1.
- In every record, `machine` is a machine id, `session` a Session name, `login` and `question` are
  ids, and `outcome`, `reason`, `state`, and `cause` are names.
- A member whose absence at an older peer could turn a failure into a reported success is never an
  extension; it takes a capability name.
- An enumerated value marked _grows_ has a fallback: a receiver reads a name it does not know as
  that fallback. Every other enumeration is closed.

| Value           | Rule                                                                                       |
| --------------- | ------------------------------------------------------------------------------------------ |
| Name            | The `OBHS` name: 1–64 bytes of `a`–`z`, `0`–`9`, and `-`                                   |
| Token           | 1–64 bytes of `a`–`z`, `0`–`9`, `-`, and `_`                                               |
| Machine id      | 32 lowercase hexadecimal digits, a profile `id` of `machines.json`                         |
| Session name    | 1–64 bytes of ASCII letters, digits, `.`, `_`, and `-`, neither starting nor ending in `.` |
| Product version | The `OBHS` `productVersion`: 1–64 bytes from `!` to `~`                                    |
| Path            | 1–4,096 bytes of UTF-8 without U+0000–U+001F or U+007F                                     |
| Id              | Integer from 1 to 4,294,967,295                                                            |
| Count           | Integer from 0 to 9,007,199,254,740,991                                                    |
| Time            | A count of milliseconds since the Unix epoch, on the hub's clock                           |

## Channels

A channel is one bidirectional byte stream with an opening record, an optional accepting record,
and a result. The side that sends `open` is the opener; the other side is the responder.

- **Ids.** Channel 0 carries `message`, `ping`, and `pong` only. The `OBHS` initiator opens odd
  ids and the acceptor even ids. Each `open` uses an id greater than every id that side used
  before; an id is never reused. A side that has used its last id opens no further channel on that
  connection.
- **Kinds.** `open` names a channel `kind` and that kind's members. Generation 1 defines kinds
  opened by the initiator only. A responder answers a kind it does not know with `reset` and
  reason `unsupported`, reading no other member, and the connection continues.
- **Capacity.** Each side keeps at most 256 channels it opened open at once. A responder answers
  an `open` beyond that with `reset` and reason `capacity`.

| State       | Entered by          | The opener may send              | The responder may send                    |
| ----------- | ------------------- | -------------------------------- | ----------------------------------------- |
| Opening     | `open`              | `data`, `credit`, `end`, `reset` | `accept` or `reset`, as its first frame   |
| Open        | `accept`            | `data`, `credit`, `end`, `reset` | `data`, `credit`, `end`, `close`, `reset` |
| Half-closed | `end` from one side | That side: no more `data`        | That side: no more `data`                 |
| Closed      | `close` or `reset`  | Nothing                          | Nothing                                   |

- **Early data.** The opener may send `data` and `end` before `accept` arrives, inside the initial
  credit. A responder that resets the channel discards them.
- **Half-close.** `end` says its sender will send no more `data` on the channel; the other
  direction continues. Each side sends `end` at most once.
- **Result.** `close` is the responder's last frame on the channel: it implies `end`, carries the
  channel's `outcome` and result members, and closes the channel for both sides. The opener
  receives every earlier `data` byte before it. An opener that wants only to finish sends `end`
  and waits for `close`.
- **Cancel.** `reset` closes the channel at once for both sides and gets no reply. The opener
  cancels with reason `cancelled`. Bytes not yet delivered may be dropped.
- **No effect.** A responder's `reset` asserts that the channel caused no effect. Once an effect
  may have started, the responder ends with `close` and an outcome, `unknown` included.
- **Late frames.** A frame other than `open` for a closed id is discarded without effect, because
  the peer may have sent it before it learned of the close. An id is closed when it is not open
  and is not greater than the highest id its opener has used; a greater id was never opened.
- **Connection loss.** When the carrier closes, every open channel ends without a result. The
  opener reads that as an unknown effect, unless the kind states that no effect can have started.

`reset` carries `reason`, a name that _grows_ with fallback `unknown`. Either connection uses
`unsupported`, `capacity`, `cancelled`, `stopping`, and `unavailable`; each kind adds its own.

## Flow control

Credit bounds what a receiver must buffer and is counted per channel and direction, in `data`
payload bytes. No other frame consumes credit.

| Number                                    | Value     |
| ----------------------------------------- | --------- |
| Initial credit, each direction, at `open` | 262,144   |
| Largest increment of one `credit` frame   | 4,194,304 |
| Largest outstanding credit of a direction | 4,194,304 |
| Regrant threshold                         | 131,072   |

- A sender never sends `data` beyond its credit; it waits. Outstanding credit is what was granted
  minus what was received.
- A receiver grants credit only for bytes its consumer has taken, except to raise its window above
  the initial credit, and sends a `credit` frame no later than when the taken, not yet regranted
  bytes reach the regrant threshold. Backpressure of the consumer thereby reaches the sender.
- A process that relays one channel into another — the hub between a client and a link, the helper
  between a link and a Session endpoint — grants credit on one side only as the other side takes
  the bytes, so backpressure composes end to end and the relay buffers at most one window per
  direction.
- A sender with several channels ready sends at most one `data` frame of a channel before giving
  every other ready channel a turn, and sends every pending frame of another kind first.
- `message` frames are not flow-controlled. A sender of state events keeps only the newest
  undelivered event per subject, so a slow reader costs bounded memory and sees the latest state,
  not every transition.

## Control messages

A `message` payload is a closed object with a `type` name.

| Class   | Shape                                                                                                | Unknown `type`                                             |
| ------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Request | Has `id`, an id greater than every earlier request id of that sender; `type` never ends in `-result` | Answered with `unsupported`; only `type` and `id` are read |
| Reply   | Has the request's `id`; `type` is the request's type followed by `-result`, or `unsupported`         | Violation                                                  |
| Event   | Has no `id`                                                                                          | Ignored; only `type` is read                               |

- Every request gets exactly one reply. A reply's `outcome` is a name that _grows_ with fallback
  `unknown`: the receiver then assumes neither that the request took effect nor that it did not.
- `unsupported` is the reply `{type: "unsupported", id, request}`, where `request` is the unknown
  type. The connection continues.
- A sender keeps at most 64 requests unanswered.
- `bye` is the event `{type: "bye", reason}` an end sends, best effort, as its last frame before
  it closes the carrier. `reason` _grows_ with fallback `unknown`: `done`, `stopping`,
  `violation`, and `idle`.
- A `ping` is answered with a `pong` of the same bytes ahead of any `data` frame not yet being
  written, whatever the channels are doing. A sender has at most one `ping` unanswered.
  `machine-hub` sets no liveness rule, since a local endpoint reports a lost peer by closing;
  [Link liveness](#link-liveness) sets the link's.

## Violations

A violation ends the connection, never one channel. The end that detects it stops reading, sends
`bye` with reason `violation` when it still can, and closes the carrier: the hub ends that
Machine's `ssh` child or closes that one client's connection, the helper exits with a failure
status, and a client closes its connection. No frame is replayed on a later connection.

- A header that fails a [frame](#frame) check, or a payload that breaks the
  [JSON rules](#json-payloads).
- A known open kind, request, reply, or event that is not exactly its closed object, or that
  travels in a direction this contract does not give it.
- `open` with id 0, with an id of the peer's parity, or with an id not greater than the sender's
  earlier ids.
- A frame other than `open` for an id never opened.
- `accept` or `close` from the opener, a second `accept`, or a responder frame before `accept`
  other than `reset`.
- `data` after that side's `end`, a second `end`, or `data` in a direction the kind gives none.
- `data` beyond the credit granted, or a `credit` frame that raises outstanding credit above
  4,194,304.
- Responder `data` beyond its kind's bound.
- A request id not greater than the sender's earlier ones, a 65th unanswered request, or a second
  unanswered `ping`.
- A reply or `pong` that answers nothing outstanding.
- On `machine-link`, a first acceptor frame that is not `facts`, or a second `facts`.

The three unknown cases are never violations: an unknown channel kind is reset as `unsupported`,
an unknown request type is answered `unsupported`, and an unknown event type is ignored.

## Ordering

- A connection delivers frames in the order sent, and a channel delivers bytes in the order sent.
- Channels are independent: no order holds between frames of two channels beyond the order in
  which their sender wrote them.
- An end sends the events that report a request's effects before that request's reply, and the
  `machine` event for a state before any `accept`, `close`, or `reset` that depends on it.

## Link records

Generation 1 of `machine-link` defines one event, `facts`, from the acceptor; `bye` from either
end; no request; and four channel kinds, opened by the initiator.

### Facts

The helper's first frame after the answer is the event `facts`, sent once. The hub may open
channels without waiting for it.

| Member         | Value                                                             | Presence                  |
| -------------- | ----------------------------------------------------------------- | ------------------------- |
| `platform`     | Token: `windows`, `macos`, or `linux`; _grows_                    | Always                    |
| `architecture` | Token: `x86_64` or `aarch64`; _grows_                             | Always                    |
| `executable`   | Path of the running helper                                        | Always                    |
| `storeRoot`    | Path of the store root this helper resolves                       | When it resolves one      |
| `origin`       | `store`, `bootstrap`, or `external`; _grows_, fallback `external` | With capability `install` |
| `install`      | `{available}` and, exactly when `available` is false, `reason`    | With capability `install` |

- `platform` and `architecture` describe the helper's own build; the release platform name is
  `<platform>-<architecture>`. A value the hub does not know makes the Machine `unsupported`.
  They are the only authoritative statement of the remote system: an SSH server banner is a
  guess.
- `origin` is `store` for a version directory of `storeRoot`, `bootstrap` for a file the launch
  line received in this `ssh` session, and `external` otherwise.
- `install.reason` _grows_ with fallback `unknown`: `development-build`, `foreign-architecture`
  (the store holds a release for another architecture), and `store-unavailable`.
- The remote product version and build are the answer's `productVersion` and `build`.

### Link channel kinds

| Kind             | `open` members                          | Opener data                                     | Responder data           | `accept` members |
| ---------------- | --------------------------------------- | ----------------------------------------------- | ------------------------ | ---------------- |
| `session`        | `session`                               | Bytes for the Session's Client endpoint         | Bytes from that endpoint | None             |
| `route`          | None                                    | One request packet, then `end`                  | One reply packet         | None             |
| `ensure-session` | `session`                               | None                                            | None                     | None             |
| `install`        | `version`, `platform`, `files`, `bytes` | [Release records](#install-channel), then `end` | None                     | None             |

| Kind             | `close` outcomes (_grow_, fallback `unknown`)                                                                                                                 | `reset` reasons of its own                              |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------- |
| `session`        | `ended`, `failed`                                                                                                                                             | `session-not-running`, `invalid-session`, `unavailable` |
| `route`          | `replied`                                                                                                                                                     | `too-large`                                             |
| `ensure-session` | `ready` with boolean `started`, `failed`, `unknown`                                                                                                           | `invalid-session`                                       |
| `install`        | `installed`, `present`, `verification-failed`, `platform-mismatch`, `foreign-architecture`, `downgrade`, `store-unwritable`, `disk-full`, `failed`, `unknown` | `too-large`                                             |

- **`session`.** The helper connects to the Client endpoint of that Session of its own user and
  environment and sends `accept` once connected. It relays bytes unread in both directions; the
  far opener, a client through the hub or the hub itself, runs its own `OBHS` exchange with the
  Session through the channel, an offer or a control request, so the Session admits or refuses it
  exactly as it would a local Client. The far opener sends that `OBHS` frame as early data, since
  the Session's 2-second candidate deadline starts when the helper connects. On the
  opener's `end` the helper delivers what it holds and closes the endpoint connection; when the
  endpoint closes, the helper sends what it read and then `close` with `ended`. `failed` reports an
  endpoint I/O error. The Session keeps running whatever happens to the channel or the link.
- **`route`.** The data is the `machine-route` request packet and reply packet of
  [Command routing](../../apps/orchbee/docs/machines.md#command-routing-contract), each as its
  JSON text and nothing else: at most 2,097,152 and 25,165,824 bytes. The helper admits and runs
  the request only after the opener's `end`, so a channel that ends before the hub wrote `end`
  caused no effect. After `end`, only a complete reply packet tells the outcome; `close` with
  `replied` follows it. A cancel or a lost link after `end` is an unknown outcome and is never
  retried. The helper ends a command whose channel is cancelled or whose link is lost.
- **`ensure-session`.** The helper starts that Session's Server when it is not running and waits
  at most 30 seconds for it to report ready. `ready` carries `started`, true when this channel
  started it. `failed` means it did not start; `unknown` means a start was attempted and readiness
  was not confirmed. The request is idempotent.
- A Session name the helper's platform refuses gets `reset` with `invalid-session`.

### Install channel

Requires capability `install`. `version` is a product version, `platform` a token of at most 32
bytes naming the release platform, `files` a count from 1 to 4,096, and `bytes` the count of all
content bytes, at most 2,147,483,648.

The opener's data is a sequence of records, without padding:

| Field       | Encoding                                                                 |
| ----------- | ------------------------------------------------------------------------ |
| Path length | Big-endian `u16` from 1 to 1,024                                         |
| Path        | That many bytes: a `/`-separated relative path under the release's rules |
| Size        | Big-endian `u64`, at most 536,870,912                                    |
| Content     | Exactly that many bytes                                                  |

- The first record is `release.json`, the second `release.json.sig`, and the rest are every file
  the manifest lists, once each, in manifest order. `files` counts the listed files only.
- The helper verifies the signature with its compiled release keys before it accepts the third
  record, verifies each file's size and digest as it completes, and publishes and activates the
  version only after the opener's `end`. Nothing it received is usable before that.
- An `open` beyond a bound gets `reset` with `too-large`, and one the helper cannot serve
  `unavailable`. Records that are malformed, exceed what `open` declared, or disagree with the
  manifest end the channel with `verification-failed`.
- `present` means that version is already installed and was left untouched. `downgrade` means the
  store's active version is higher. A reset, a failed outcome, or a lost link leaves no usable
  partial version.
- The helper raises its window on this channel to 4,194,304 bytes.

### Link liveness

- The hub lets no 30 seconds pass without sending a frame, sending `ping` when it has no other.
- The helper treats 90 seconds without a received frame as a lost link: it sends `bye` with reason
  `idle`, ends the commands its channels started, and exits. Sessions are not affected.
- The helper also exits when its standard input ends.

## Hub records

On `machine-hub` every request travels from the initiator to the acceptor, every event but `bye`
from the acceptor to the initiator, and only the initiator opens channels.

### Attachment

The request `attach` states what a client is to the hub with three booleans, all required. A
client may send it again; each one replaces the last. A client that never sends it has all three
false.

| Member     | Meaning when true                                                    |
| ---------- | -------------------------------------------------------------------- |
| `presence` | This connection is a holder of the hub's lifetime until it closes    |
| `asks`     | This client shows command-triggered login notices and may claim them |
| `watch`    | This client receives `hub`, `catalog`, and `machine` events          |

- A TUI sets `presence` and `asks`. Desktop's bundled Client sets `presence` for the Desktop's
  whole process life and `asks` only while a window exists. A command sets neither.
- `attach-result` has outcome `attached` with `hub`, an object of the members of the
  [`hub` event](#hub-lifetime), or `stopping` without it.
- After an `attach` that turns `watch` on, the hub sends `catalog`, then one `machine` event for
  each id in it, in that order, and then changes as they happen.

### Hub lifetime

The event `hub` carries `state` and, exactly in `expiring`, `expiresAt`, a time.

| State      | Meaning                                                                                                                                                                                                                                      | Leaves to                               |
| ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------- |
| `held`     | A holder exists: a connection with `presence`, or a live local Session of the hub's data root                                                                                                                                                | `expiring`, `closing`                   |
| `expiring` | No holder exists; every login ends at `expiresAt`, 600,000 ms after the latest of: the last holder leaving, the hub's start, and the last `connect` that ended with its Machine `online`, or once no work is in flight when that comes later | `held` when a holder appears, `closing` |
| `closing`  | The expiry was reached with no work in flight, or a control request asked for it                                                                                                                                                             | Process exit                            |

- Holders of the second kind are observed by the hub, not announced through this protocol.
- In `closing` the hub rejects offers with `stopping`, answers every request with outcome
  `stopping`, resets every new `open` with reason `stopping`, closes open `session` channels with
  `lost`, lets open `route` channels finish for at most 60 seconds, then sends `bye` with reason
  `stopping`, ends every `ssh` child, and exits. A client that needs a hub then starts a new one.
- `closing` never returns to another state.
- A `connect` reply `online` restarts the countdown while `expiring` and the hub sends the new
  `expiresAt`. No channel and no other request restarts it.
- Expiry never ends work in flight. Work in flight is an open `route` channel or a `stop-session`
  request waiting for its outcome. While any is in flight, reaching `expiresAt` does not begin
  `closing`: the hub stays `expiring` with the past `expiresAt`, which no channel opening or ending
  moves, and sends no event for it. Once `expiresAt` has passed, the hub begins `closing` as soon as
  no work is in flight and no holder has appeared. `closing` for any other reason still lets open
  `route` channels finish for at most 60 seconds.

### Machine state

The event `catalog` carries `revision`, the count `revision` of `machines.json`, and `machines`,
at most 64 machine ids in catalog order. A Machine absent from the newest `catalog` is gone. The
request `reload` asks the hub to reread the catalog now; a client sends it after it commits an
edit. `reload-result` has outcome `reloaded` with `revision`, or `invalid-catalog`, `storage`, or
`stopping` without it; after a failed reload the hub keeps the catalog it had.

The event `machine` replaces everything known about one Machine.

| Member        | Value                                                                                                          | Presence                                               |
| ------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| `machine`     | Machine id                                                                                                     | Always                                                 |
| `state`       | A state name; _grows_, fallback `unknown`                                                                      | Always                                                 |
| `cause`       | A cause name; _grows_, an unknown one read as absent                                                           | Where the state table lists one                        |
| `stage`       | `probing`, `authenticating`, `starting`, `copying`; _grows_, read as absent                                    | Optional in `connecting`, `reconnecting`, `installing` |
| `lastAttempt` | Time the last connection attempt ended                                                                         | Optional                                               |
| `nextAttempt` | Time of the next automatic attempt                                                                             | Optional in `reconnecting` and `offline`               |
| `remote`      | Object with at least one of `productVersion`, a product version, and the tokens `platform` and `architecture`  | When known                                             |
| `hostKey`     | `{algorithm, fingerprint}`: a token, and 1–128 bytes from `!` to `~`                                           | In `untrusted`; optional in `key-changed`              |
| `draft`       | `true`                                                                                                         | Exactly for a draft Machine, sent to its owner only    |
| `offer`       | `{version, bytes, storeRoot}`, a product version, a count, and a path: what an `install` request would install | With capability `install`, while consent is awaited    |
| `transfer`    | `{doneBytes, totalBytes}`, counts                                                                              | With capability `install`, in `installing`             |

| `state`              | Meaning                                                               | `cause`                                                                                                                                       | `connect` outcome                                                   |
| -------------------- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------- |
| `disabled`           | The record is disabled                                                | —                                                                                                                                             | `disabled-machine`                                                  |
| `connecting`         | A first attempt is running                                            | —                                                                                                                                             | Pending                                                             |
| `online`             | The link is up and the Machine's Session answered ready               | —                                                                                                                                             | `online`                                                            |
| `reconnecting`       | The link was lost or an attempt failed, and the hub retries by itself | As `offline`, or `link-lost`                                                                                                                  | Pending                                                             |
| `offline`            | No link and no attempt in progress                                    | `host-unreachable`, `host-timeout`, `host-refused`, `host-unresolved`, `ssh-unavailable`, `ssh-failed`, `remote-protocol`, `remote-not-ready` | `host-unreachable` for the four `host-` causes, otherwise the cause |
| `unresponsive`       | The link exists and a `ping` went unanswered for 15 seconds           | —                                                                                                                                             | Pending                                                             |
| `needs-login`        | A person must answer a question; not an error                         | `auth-required`, `remembered-rejected`, `login-dismissed`, `disconnected`                                                                     | `auth-required`                                                     |
| `login-failed`       | `ssh` gave up after refused answers or a refused key                  | `password-rejected`, `key-rejected`                                                                                                           | `auth-failed`                                                       |
| `untrusted`          | The host key is not in `known_hosts`                                  | —                                                                                                                                             | `host-untrusted`                                                    |
| `key-changed`        | The host key differs from `known_hosts`; no question is offered       | —                                                                                                                                             | `host-key-changed`                                                  |
| `not-installed`      | The remote has no Orchbee of this environment                         | —                                                                                                                                             | `remote-not-installed`                                              |
| `installing`         | A release is being copied or started; capability `install`            | —                                                                                                                                             | Pending                                                             |
| `unsupported`        | The remote system, or the record's connection kind, is not supported  | `remote-platform`, `connection-kind`                                                                                                          | `remote-unsupported`, `unsupported-machine`                         |
| `needs-update`       | The remote Orchbee or its Session is too old to work with this one    | `remote-too-old`, `session-too-old`                                                                                                           | `version-incompatible`                                              |
| `local-needs-update` | This computer's Orchbee is too old for the remote                     | —                                                                                                                                             | `version-incompatible`                                              |
| `incompatible`       | The remote Orchbee belongs to another environment                     | —                                                                                                                                             | `remote-other-environment`                                          |

- State and outcome names equal the `error` names of the
  [machine receipt](../../apps/orchbee/docs/machines.md#machine-receipt) wherever one exists.
- To a client without capability `install` the hub reports `connecting` in place of `installing`
  and omits `offer` and `transfer`.
- The hub attempts every enabled Machine silently when a `presence` connection attaches and after
  a lost link. A silent attempt never produces a `login` or `question` event: a question ends it
  in `needs-login`, `untrusted`, or `login-failed`.

### Connect and disconnect

| Request      | Members                                                     | Outcomes                                                                                                                                                |
| ------------ | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `connect`    | `machine`, `origin`, optional `answers`, `draft`, `install` | `online`, a `connect` outcome of the state table, `not-found`, `invalid-draft`, `cancelled`, `timeout`, `remote-install-failed`, `internal`, `stopping` |
| `disconnect` | `machine`                                                   | `disconnected`, `not-found`, `stopping`                                                                                                                 |

- **`connect`** asks for the Machine to be `online` and replies when it is or when the attempt
  ends. Several `connect` requests for one Machine join one attempt. `connect-result` adds `state`,
  the Machine's state name at the reply, whenever the hub knows the Machine, and `remoteVersion`,
  a product version, exactly when `online`.
- **`origin`** is `user` or `command`, closed. `answers`, a boolean read as false when absent,
  says the requester itself can show questions and deliver answers. With `origin` `user`,
  `answers` is required and true.

| `origin`  | A question arises                                                                                                                                                                                                             |
| --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `user`    | The requester becomes the claimant of a [login episode](#login-episode) only it sees; no deadline                                                                                                                             |
| `command` | With a client that `asks` attached and the Machine not muted: an episode is offered to every such client and the requester waits at most 120,000 ms. Otherwise, with `answers`: as `user`. Otherwise the request ends at once |

- A `command` request that cannot be asked, is dismissed, or outwaits its 120,000 ms ends with
  `auth-required`; nothing was submitted to the remote.
- When a `connect` starts waiting for a person the hub sends its requester the event
  `connect-waiting` with `request`, that request's id, and `reason`, a name that _grows_ and is
  read as `login` when unknown: `login`, or `install-consent`.
- **`draft`** is `{connection: {kind: "ssh", target}, session}` for a Machine not yet in the
  catalog, connected under the id it will be saved with; `target` is 1–1,024 ASCII bytes under the
  record's rules and `session` a Session name. A draft is visible to its owner's connection only.
  It becomes an ordinary Machine, keeping its link, when a reread catalog holds that id with the
  same connection and Session; it is dropped, and its link closed, when its owner's connection
  closes, on `disconnect`, or when the catalog holds that id with other content.
- **`install`**, a boolean with capability `install`, consents in advance to installing Orchbee
  on the remote when it has none.
- **`disconnect`** ends the Machine's login and link, ends a running attempt so that pending
  `connect` requests reply `cancelled`, and leaves the remote Session running. The Machine reads
  `needs-login` with cause `disconnected`, or returns to silent attempts when it needs no person.

### Login episode

An episode is one occasion on which a person is asked to log in to one Machine. A Machine has at
most one. The event `login` replaces everything known about an episode.

| Member    | Value                                                                                       | Presence             |
| --------- | ------------------------------------------------------------------------------------------- | -------------------- |
| `login`   | Id, unique in the hub's process life and ascending in creation order                        | Always               |
| `machine` | Machine id                                                                                  | Always               |
| `trigger` | `user` or `command`; _grows_, fallback `command`                                            | Always               |
| `phase`   | `offered`, `claimed`, or `ended`; closed                                                    | Always               |
| `waiters` | Count of `command` requests waiting on the episode                                          | Always               |
| `claim`   | `you` or `other`; closed                                                                    | Exactly in `claimed` |
| `end`     | `succeeded`, `failed`, `dismissed`, `abandoned`, `unavailable`; _grows_, fallback `unknown` | Exactly in `ended`   |

| From                 | Event                                                                                   | To                         |
| -------------------- | --------------------------------------------------------------------------------------- | -------------------------- |
| None                 | A `command` attempt needs a person, a client that `asks` is attached, Machine not muted | `offered`                  |
| None                 | A `user` attempt needs a person                                                         | `claimed` by the requester |
| `offered`            | The first `claim`, or a `user` `connect` with `answers` for that Machine                | `claimed`                  |
| `offered`            | `dismiss` from any client that `asks`                                                   | `ended`, `dismissed`       |
| `offered`            | The last waiter left, or the last client that `asks` left                               | `ended`, `abandoned`       |
| `claimed`            | The claimant's `dismiss`                                                                | `ended`, `dismissed`       |
| `claimed`, `command` | The claimant's connection closed or its attachment stopped asking, and another asks     | `offered`                  |
| `claimed`            | The claimant left and nobody else can be asked                                          | `ended`, `abandoned`       |
| `offered`, `claimed` | The link authenticated                                                                  | `ended`, `succeeded`       |
| `claimed`            | `ssh` gave up on the answers                                                            | `ended`, `failed`          |
| `offered`, `claimed` | The attempt ended for another reason, which the Machine's state names                   | `ended`, `unavailable`     |

- A `command` episode is sent to every client that `asks`; a `user` episode to its requester only.
  A claimed episode stays claimed when its last waiter leaves.
- A client shows one episode at a time, lowest `login` first.
- `claim` and `dismiss` carry `login`. `claim-result` is `claimed`, `taken` when another client
  claimed first, `ended`, or `stopping`. `dismiss-result` is `dismissed`, `not-claimant` when the
  episode is claimed by another client, `ended`, or `stopping`. `ended` also answers a `login` the
  hub does not know.
- **Mute.** After a `command` episode ends `dismissed`, every waiter fails with `auth-required`
  and the hub offers no `command` episode for that Machine, which reads `needs-login` with cause
  `login-dismissed`, until a `user` `connect` for it arrives or no client that `asks` remains.

The event `question` goes to the claimant only.

| Member       | Value                                                                           | Presence              |
| ------------ | ------------------------------------------------------------------------------- | --------------------- |
| `login`      | The episode                                                                     | Always                |
| `question`   | Id, ascending within the episode                                                | Always                |
| `kind`       | `password`, `key-passphrase`, `host-key`, or `other`; _grows_, fallback `other` | Always                |
| `prompt`     | The question as `ssh` worded it: 1–4,096 bytes of UTF-8                         | Always                |
| `hostKey`    | `{algorithm, fingerprint}` as in the `machine` event                            | Exactly in `host-key` |
| `repeated`   | `true`: `ssh` refused the previous answer of this kind in this episode          | Optional              |
| `remembered` | `rejected`: a remembered answer was tried for this question and refused; closed | Optional              |
| `answer`     | `{endpoint, token}`: a path, and 32 lowercase hexadecimal digits                | Always                |

- An episode has at most one open question. A later `question`, or `login` in `ended`, closes it;
  its endpoint then accepts nothing.
- `kind` is the hub's reading of `prompt`, since `ssh` passes no kind. `other` covers one-time
  codes and anything unrecognized: it is shown verbatim, entered masked, and never remembered.
  Only `password` and `key-passphrase` answers may be remembered.
- A `host-key` question is answered with the three bytes `yes`, or dismissed. `prompt` may span
  lines; a client strips control characters before it paints it. The hub replaces bytes that are
  not UTF-8 with U+FFFD and cuts the text at 4,096 bytes on a character boundary.
- `claimed` to `succeeded` is the claimant's signal that the answers it delivered were accepted.

### Hub channel kinds

| Kind      | `open` members | Relayed to                                         | `close` outcomes          | `reset` reasons of its own                                                       |
| --------- | -------------- | -------------------------------------------------- | ------------------------- | -------------------------------------------------------------------------------- |
| `session` | `machine`      | A link `session` channel for the Machine's Session | `ended`, `failed`, `lost` | `not-found`, `not-online`, `link-lost`, `session-not-running`, `invalid-session` |
| `route`   | `machine`      | A link `route` channel                             | `replied`, `lost`         | `not-found`, `not-online`, `link-lost`, `too-large`                              |

- The hub opens a channel only for a Machine that is `online`; it never starts a login for an
  `open`. A client that may need one sends `connect` first.
- The hub sends `accept` after the link channel was accepted, relays data, `end`, and the link's
  `close` unchanged, and passes on a link `reset` with its reason. `session-not-running` and
  `invalid-session` are the link `session` channel's own reasons, passed on as they came.
- `link-lost` as a `reset` reason keeps the no-effect rule: the hub sends it for a `session`
  channel only before `accept`, and for a `route` channel only when it had not yet written the
  opener's `end` to the link. When the link is lost later, the hub sends `close` with `lost`,
  which a `route` opener reads as an unknown outcome.

### Remote Session and install requests

| Request        | Members   | Outcomes                                                                             | Effect                                                          |
| -------------- | --------- | ------------------------------------------------------------------------------------ | --------------------------------------------------------------- |
| `stop-session` | `machine` | `stopped`, `not-running`, `not-online`, `not-found`, `failed`, `unknown`, `stopping` | Stops the Machine's remote Session and every terminal in it     |
| `install`      | `machine` | `started`, `not-offered`, `not-found`, `stopping`                                    | Consents to the Machine's current `offer`; capability `install` |

- The hub stops a Session with a link `route` channel that carries that Session's `server stop`
  command; the link has no member for it. `unknown` means the link was lost after submission.
- After `install` replies `started`, the Machine's state reports progress and the result.

## Answer delivery

An answer travels from the client that asked to the question's one-shot endpoint, and from there
to `ssh`. It is no frame of either connection, and the hub is not on its path.

| Field      | Encoding                                                             |
| ---------- | -------------------------------------------------------------------- |
| Magic      | The four ASCII bytes `OBMA`                                          |
| Generation | Big-endian `u16`, the `machine-hub` generation the client negotiated |
| Token      | The 16 bytes the question's `answer.token` spells                    |
| Length     | Big-endian `u16` from 0 to 1,024                                     |
| Answer     | That many bytes without NUL, LF, or CR                               |

- The client connects to `answer.endpoint`, checks that the peer runs as its own user, writes one
  record, and waits at most 5 seconds for the single byte 0x06, which says the record was taken.
- The endpoint takes one record once. A wrong token, a second connection, or a closed question
  gets no byte, only a close.
- The record's layout belongs to `machine-hub` generation 1 and freezes with it.

## Secrets

No frame, member, or event of `machine-link` or `machine-hub` carries a password, a passphrase, a
one-time code, or any other answer to a login question.

- The complete set of login members is `login`, `question`, `kind`, `prompt`, `hostKey`,
  `repeated`, `remembered`, and `answer` with `endpoint` and `token`; the requests `claim` and
  `dismiss`; and the episode's `end`. None holds what a person typed.
- `prompt` is what `ssh` asked. It may name an account, a host, or a key file, never an answer.
- `token` admits one delivery to one endpoint and reveals nothing. It is still never logged.
- No member says whether to remember an answer. The client that took the answer writes the
  operating system's credential store itself after `succeeded`; the hub learns only that a
  remembered answer was refused.
- `extensions` never carries an answer. A decoder cannot check that; review of each sender does.
- Channel `data` and route packets are the person's own content. They are never logged, but they
  are outside this rule: it governs login answers.

## Bounds

| Bound                                           | Value                                   | Beyond it                                                 |
| ----------------------------------------------- | --------------------------------------- | --------------------------------------------------------- |
| JSON payload of one frame                       | 65,536 bytes                            | Violation                                                 |
| `data` payload of one frame                     | 32,768 bytes                            | Violation                                                 |
| Open channels per opener and connection         | 256                                     | `reset` with `capacity`                                   |
| Outstanding credit per channel and direction    | 4,194,304 bytes                         | Violation                                                 |
| Unanswered requests per sender                  | 64                                      | Violation                                                 |
| Unanswered `ping` per sender                    | 1                                       | Violation                                                 |
| Route request packet; reply packet              | 2,097,152; 25,165,824 bytes             | `reset` with `too-large`; violation                       |
| Path of one install record                      | 1,024 bytes                             | `close` with `verification-failed`                        |
| Install: files, one file, all content           | 4,096; 536,870,912; 2,147,483,648 bytes | `reset` with `too-large` at `open`                        |
| Machines in `catalog`                           | 64                                      | Violation                                                 |
| `prompt`                                        | 4,096 bytes                             | The hub truncates before sending                          |
| Hub connection slots; control slots             | 256; 4                                  | `OBHS` rejection `capacity`                               |
| Hub: wait of a `command` request for a login    | 120,000 ms                              | `auth-required`                                           |
| Hub: expiry; closing allowance                  | 600,000 ms; 60,000 ms                   | Logins end once no work is in flight; channels end `lost` |
| Link: hub send interval; helper receive silence | 30 s; 90 s                              | The helper exits                                          |
| Helper: wait for a Session to report ready      | 30 s                                    | `close` with `unknown`                                    |

## Baseline and capabilities

| Part                                                                                                                                                         | Baseline | Capability                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------ | -------- | --------------------------- |
| Frame, channels, flow control, `message` classes, `unsupported`, `bye`, `ping`, `pong`                                                                       | Yes      | —                           |
| Link: `facts` with `platform`, `architecture`, `executable`, `storeRoot`                                                                                     | Yes      | —                           |
| Link: `session`, `route`, `ensure-session`                                                                                                                   | Yes      | —                           |
| Link: `install` channel; `facts` members `origin` and `install`                                                                                              | No       | `install` on `machine-link` |
| Hub: `attach`, `hub`, `catalog`, `machine`, `reload`                                                                                                         | Yes      | —                           |
| Hub: `connect` with `origin`, `answers`, `draft`; `connect-waiting`; `disconnect`                                                                            | Yes      | —                           |
| Hub: `login`, `question`, `claim`, `dismiss`; the answer delivery record                                                                                     | Yes      | —                           |
| Hub: `session`, `route`; `stop-session`                                                                                                                      | Yes      | —                           |
| Hub: `install` request; `connect` member `install`; `machine` members `offer` and `transfer`; state `installing`; `connect-waiting` reason `install-consent` | No       | `install` on `machine-hub`  |

A baseline part is in every generation 1 peer and needs no capability name. Generation 1 lists no
capability name until `install` ships. A later channel kind, request, event, or member arrives
under its own capability name, and a peer without it answers `unsupported` or ignores the event.

## Fixtures and conformance

Fixtures live in [`v1/fixtures/`](v1/fixtures/). Frame fixtures hold `frameHex`, the complete
bytes; payload fixtures hold `kind`, the frame kind name, and `payload`, the exact JSON text, framed
on channel 0 for `message` and on channel 1 for every other kind. Each names its `connection` and
its `direction`, `initiator` or `acceptor`.

| File                                           | Claim                                                                                                                                                                          |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| [`valid.json`](v1/fixtures/valid.json)         | Each decodes in its connection and direction and re-encodes to an equal value                                                                                                  |
| [`boundary.json`](v1/fixtures/boundary.json)   | Each value at its limit decodes                                                                                                                                                |
| [`invalid.json`](v1/fixtures/invalid.json)     | Each fails to decode. `layer` names the first layer that refuses it: `frame`, `encoding`, or `record`                                                                          |
| [`sequences.json`](v1/fixtures/sequences.json) | Each is an ordered list of frames with their senders and the verdict of the receiving state machine: accepted, discarded, a reply it must send, or a violation at a named step |

A sequence case names its `connection`, the `machine` under test, `initiator` or `acceptor`, and
its `steps` in order:

| Step member                        | Content                                                                                                                                                        |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `sender`                           | `initiator` or `acceptor`. The machine sends its own steps through its interface and receives the peer's                                                       |
| `kind`, `channel`                  | The frame kind name and the channel, 0 when absent                                                                                                             |
| `payload`, `payloadHex`, or `fill` | At most one: the exact JSON text, the bytes in lowercase hexadecimal, or `{byte, length}` for `length` copies of `byte`; an empty payload when none is present |
| `repeat`                           | How many times in a row the step happens, 1 when absent                                                                                                        |
| `verdict`, `violation`             | Received steps only: `accepted`, `discarded`, `reply`, or `violation`, and with `violation` the name of the broken rule                                        |
| `replies`                          | The frames the machine writes next, in order, each with `kind`, `channel`, a payload member, and `repeat`                                                      |

- After its own step the machine writes exactly the step's frames and its `replies`. After a
  received step with `replies` it writes exactly those; without `replies`, what it queued stays
  queued for a later step. After the last step nothing is left to write.
- A written JSON frame matches when its kind, channel, and parsed JSON value are equal; member
  order and spacing are free. Every other frame matches byte for byte.
- The consumer takes every delivered `data` byte at once, so `credit` follows the regrant
  threshold.
- A `machine-link` acceptor's first step is its `facts`, which it sends on its own at start.
- A violation is the last step of its case; the machine then writes only `bye` with reason
  `violation`. Violation names: `frame`, `encoding`, and `record` for the three layers; `parity`
  and `open-id-not-increasing` for `open` ids; `never-opened`; `opener-accept-or-close`,
  `second-accept`, and `before-accept`; `data-after-end`, `second-end`, and `data-direction`;
  `credit-exceeded` and `credit-overflow`; `data-bound`; `request-id-not-increasing`,
  `too-many-requests`, and `second-ping`; `unmatched-reply` and `unmatched-pong`; and
  `facts-missing` and `facts-repeated`.

Bounds too large for a fixture file — a 25,165,824-byte reply, 257 channels — are checked by the
binding's own tests, as is the two-ended exercise of open, flood, half-close, and cancel.
