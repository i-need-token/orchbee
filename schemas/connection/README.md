# Connection contract

## Authority

This directory is the language-neutral authority for `OBHS`, the permanent handshake frame that
opens every negotiated connection, and for the generation registry, the one declaration of
generation numbers for every connection, frame format, and unnegotiated delivery.

| File                                                                                   | Content                                                                                      |
| -------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| This README                                                                            | Frame, payload rules, messages, admission order, registry shape, and fingerprint             |
| [`v1/handshake.schema.json`](v1/handshake.schema.json)                                 | JSON Schema of every message; the rules it cannot state are listed under [Payload](#payload) |
| [`v1/connections.json`](v1/connections.json)                                           | The generation registry                                                                      |
| [`v1/fixtures/`](v1/fixtures/)                                                         | Valid, boundary, invalid, admission, and Host handover fixtures                              |
| [Versioning and Upgrade](../../proposals/platform/versioning-and-upgrade.md#handshake) | Accepted design and rationale until P0 to P2 ship                                            |

`crates/connection-handshake` is the Rust binding, and `apps/terminal-host/src/handshake` is the
Terminal Host's Zig binding. Both are independent implementations of this contract; nothing
generates source from it. From the first automatic-update release, `OBHS` and its messages never
change except by new members of `extensions`, and the fixtures of this directory
[freeze](#frozen-fixtures).

## Frame

| Field          | Encoding                                        |
| -------------- | ----------------------------------------------- |
| Magic          | The four ASCII bytes `OBHS`                     |
| Payload length | Big-endian `u32`, from 1 to 1,048,576           |
| Payload        | Exactly that many bytes holding one JSON object |

- A reader reports a foreign protocol as soon as a received byte differs from `OBHS`. From a reply,
  such a peer predates the handshake and is the older side, reported without a version.
- A reader rejects an out-of-bounds length before reading or allocating the payload and never
  reads past the declared payload. Decoding one complete in-memory frame also rejects truncation
  and trailing bytes.
- An acceptor waits at most 2 seconds for a candidate's complete frame, then sends a best-effort
  `timeout` rejection and closes.

## Payload

- The payload is UTF-8 JSON under RFC 8259 holding one object: surrounding whitespace is allowed;
  a byte order mark, a second value, or a non-object root is malformed.
- No object at any depth repeats a member name.
- Arrays and objects nest at most 32 deep, counting the message object as depth 1.
- A number is finite as an IEEE 754 binary64 value; a string escape never produces a lone
  surrogate.
- Every integer member is written as an integer token, without a fraction, exponent, or minus
  sign: `1.0`, `1e0`, and `-0` are malformed.
- Every message is a closed object with a `type` member. An unknown or missing member, `null` for
  any member, or a value out of bounds is malformed. Optional members are omitted when absent.
- `extensions` is the only open slot and appears only as a member of the message object. Its
  member names follow the name grammar, its values are any JSON, and a receiver ignores names it
  does not know. An empty object equals an absent one.
- Member order, whitespace, and equivalent string escapes carry no meaning.

The JSON Schema states shapes and value grammars. Duplicate members, nesting depth, integer
spelling, number range, bytewise order, `lowest` ≤ `highest`, and message direction are checked by
decoders only.

## Values

| Value            | Rule                                                                                                     |
| ---------------- | -------------------------------------------------------------------------------------------------------- |
| Name             | 1–64 bytes of `a`–`z`, `0`–`9`, and `-`, compared and ordered bytewise                                   |
| `generation`     | Integer from 1 to 65,535                                                                                 |
| `generations`    | `{lowest, highest}`, generations with `lowest` ≤ `highest`                                               |
| `capabilities`   | At most 256 names in strictly ascending bytewise order, so without repeats                               |
| `productVersion` | 1–64 bytes from `!` (0x21) to `~` (0x7E)                                                                 |
| `build`          | `{commit, cohort}`, both optional: 40 and 64 lowercase hexadecimal digits. Terminal Host sends no cohort |
| `environment`    | A name; `orchbee` is the release environment                                                             |
| `fingerprint`    | 64 lowercase hexadecimal digits, present exactly when `environment` is not `orchbee`                     |

## Messages

| `type`            | Sender    | Members besides `type` and optional `extensions`                                                            |
| ----------------- | --------- | ----------------------------------------------------------------------------------------------------------- |
| `offer`           | Initiator | `connection` (name), `generations`, `capabilities`, `productVersion`, `build`, `environment`, `fingerprint` |
| `query`           | Initiator | `environment`                                                                                               |
| `drain`           | Initiator | `environment`                                                                                               |
| `exit`            | Initiator | `environment`                                                                                               |
| `handover`        | Initiator | `environment`, `step` (name), and `ticket` (32 lowercase hexadecimal digits)                                |
| `answer`          | Acceptor  | `generation`, `capabilities`, `productVersion`, `build`                                                     |
| `rejection`       | Acceptor  | `reason` (name), and the acceptor's `generations`, `productVersion`, and `environment`                      |
| `query-result`    | Acceptor  | `processes`, and `restartKeepsTerminals` (boolean) exactly when the first process's `role` is `server`      |
| `drain-result`    | Acceptor  | `state`                                                                                                     |
| `exit-result`     | Acceptor  | None                                                                                                        |
| `handover-result` | Acceptor  | `state` (name), and `reason` (name) exactly when `state` is `refused`                                       |
| `unsupported`     | Acceptor  | `request` (name), the requested type                                                                        |

- An initiator frame whose `type` is a name that is no known message type is a control request of
  an unknown type. It must carry a valid `environment`; its other members are not read.
- A known message type in the wrong direction is malformed. An acceptor frame of an unknown type
  is malformed.
- `reason`, `role`, and `state` grow: a receiver reads an unknown name as `unknown`. Reasons are
  `malformed`, `connection-unsupported`, `environment-mismatch`, `initiator-too-old`,
  `initiator-too-new`, `fingerprint-mismatch`, `capacity`, `stopping`, and `timeout`. Roles are
  `server`, `worker`, `terminal-host`, `machine-hub`, `machine-link`, and `keeper`. States are
  `ready`, `draining`, and `stopping`.
- `handover` is the only control request with members besides `environment`. Only a Server may serve
  it, for a restart that keeps terminals; any other acceptor, and a Server that cannot hand its
  Session over, answers `unsupported`. `step` is `prepare`, `commit`, or `abort`. The result's
  `state` is `prepared`, `committed`, `aborted`, or `refused`, and its `reason` is `busy`,
  `background-programs`, `worker-unavailable`, `clients`, `not-recorded`, or `unknown-step`. All
  three grow like `reason`.
- `processes` lists 1 to 258 entries: the answering process first, then for a Server its Worker
  and up to 256 connected Hosts. Each entry has exactly `role`, `productVersion`, `build`,
  `environment`, `pid` (integer from 1 to 4,294,967,295), `state`, `terminals` (integer from 0 to
  4,294,967,295), and `connections`.
- `connections` lists at most 64 entries in strictly ascending bytewise `kind` order. Each entry has
  `kind`, a connection or delivery name, and `generations`; entries of the first process also carry
  `capabilities`, and entries of later processes never do.
- A Terminal Host adds `dispatch-progress` to its `query-result`'s `extensions`: an integer from 0
  to 9,007,199,254,740,991 counting the turns its dispatch loop has completed, which grows while the
  loop is not stopped, including while it waits on a Worker that is slow to send or take a record.
  A reader that does not know it ignores it.
- A Server adds `clients` to its `query-result`'s `extensions`: one object per admitted Client
  connection with exactly `productVersion`, `generations` (the `client-server` range that Client
  offered) and `capabilities` (every capability it offered, not only those both sides share). When
  its Worker answers in time it also adds `background-programs`, an integer from 0 to 510 counting
  the plugin jobs and configured commands that Worker runs itself; those its Terminal Host runs
  outlive a restart and are not counted. A reader that does not know them
  ignores them.

## Admission

The acceptor runs these checks in order and rejects at the first failure.

| Check                                                                       | Rejection                |
| --------------------------------------------------------------------------- | ------------------------ |
| The frame decodes as an offer or a control request                          | `malformed`              |
| The offer's `connection` is the endpoint's kind                             | `connection-unsupported` |
| `environment` equals the acceptor's, bytewise                               | `environment-mismatch`   |
| In `orchbee`, the initiator's `highest` is at least the acceptor's `lowest` | `initiator-too-old`      |
| In `orchbee`, the initiator's `lowest` is at most the acceptor's `highest`  | `initiator-too-new`      |
| Elsewhere, the fingerprints are equal and the ranges share a generation     | `fingerprint-mismatch`   |
| A connection slot is free                                                   | `capacity`               |
| The acceptor is neither draining nor stopping                               | `stopping`               |

- An admitted offer gets an `answer` whose `generation` is the highest in both ranges and whose
  `capabilities` are the names both sides listed.
- Equal fingerprints imply equal registries, so a development offer whose range shares no
  generation with the acceptor's is a fingerprint mismatch.
- A control request is checked only for environment and then for a free control slot; it skips the
  connection, generation, fingerprint, and lifecycle checks, so a draining or stopping process still
  answers it. A known request gets its one reply and an unknown type gets `unsupported`; the
  acceptor then closes.
- The initiator closes a connection whose answer selects a generation outside its range or lists a
  name it did not offer, and one whose reply pairs with neither its offer nor its request.

## Generation registry

[`v1/connections.json`](v1/connections.json) holds three closed objects, each keyed by contract
name, and the [frozen fixtures](#frozen-fixtures).

| Member                  | Entries                                                                 | Entry members                                                        |
| ----------------------- | ----------------------------------------------------------------------- | -------------------------------------------------------------------- |
| `negotiatedConnections` | `client-server`, `keeper`, `machine-hub`, `machine-link`, `worker-host` | `generations`, `superseded`, `capabilities`, and `fingerprintInputs` |
| `frameFormats`          | `obhr`, `obkg`, `obpb`, `obrw`, `obtf`                                  | `generations` and `superseded`                                       |
| `deliveries`            | `agent-hook`, `machine-route`                                           | `generations` and `superseded`                                       |
| `frozenFixtures`        | One per fixture file that freezes                                       | `path`, and `sha256` once a release seals it                         |

- `generations` is the supported range. Every contract starts at generation 1.
- `superseded` lists, in ascending order, each generation that a released later generation
  superseded, as `{generation, supersededIn, supersededOn}`: the product version of the release
  that first shipped the next generation, and that release's UTC date as `YYYY-MM-DD`, which starts
  the generation's [compatibility window](#compatibility-window). Each lies below `highest`. A
  superseded generation stays listed after it is dropped.
- `capabilities` lists `{name, addedIn}` entries in bytewise name order, where `addedIn` is the
  product version of the release that added the name. Only negotiated connections carry
  capability names; a frame format or delivery gains features through its connection or its
  `extensions`.
- `fingerprintInputs` lists repository-relative `/`-separated directories in strictly ascending
  bytewise order; none repeats or contains another, and each exists. The `worker-host` list
  includes the Terminal Host's `src/handshake` binding, and the `keeper` list also its
  `src/keeper_protocol` binding and `schemas/keeper`.
- Each binding mirrors these numbers in its own constants under a conformance check.
- What a connection carries after its `answer` belongs to that connection's own contract. The
  [machine connection contracts](../machine/README.md) state it for `machine-hub` and
  `machine-link`, whose `fingerprintInputs` both list `schemas/machine` and the source of their
  binding, `crates/machine-protocol`. The [Keeper contract](../keeper/README.md) states it for
  `keeper`.

### Compatibility window

Before the first automatic-update release, 0.1.0-beta.14, every contract stayed at generation 1
under [Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries), so every `superseded` list
starts empty. From that release a superseded generation stays supported for at least 90 days after its
`supersededOn`; a later release may then drop it by raising `lowest`. The Rust binding's registry
test, judged on the day it runs, refuses a registry that drops a generation without a `superseded`
entry or within those 90 days, and refuses any dropped `worker-host`, `keeper`, or `obhr`
generation, each of which stays supported in every 1.x release. The registry records no early end: a security fix that ends a
generation sooner changes this check together with its own decision.

### Frozen fixtures

`frozenFixtures` lists, in strictly ascending bytewise path order, the repository-relative files
that hold `OBHS` fixtures and the fixtures of each contract's generations. A release seals each
listed file it ships by recording `sha256`, the lowercase hexadecimal SHA-256 of the file's bytes,
and from then on the Rust binding's registry test fails when those bytes change: tests never change
a released generation's expected bytes to admit a new change. A new case goes into a new file, which
joins the list. A file leaves the list only with the generation it serves, after that generation is
dropped. The first automatic-update release, 0.1.0-beta.14, sealed the first list.

- The list holds the valid, boundary, invalid, admission, and sequence fixtures of this directory
  and of [`schemas/machine`](../machine/README.md#authority), the codec fixtures of
  [`schemas/terminal`](../terminal/README.md#fixtures-and-maintenance), and the wire conformance
  corpora of [`schemas/client-server`](../client-server/README.md) and the Workbench protocol
  under [`schemas/workbench`](../workbench/README.md).
- A fixture file that only a new capability or extension serves, such as `host-handover.json`,
  `dispatch-progress.json`, or a group of the [Keeper contract](../keeper/README.md#freezing),
  joins the list with the change that makes a Host offer or report it, and the next release seals
  it. Its names, such as a capability's `addedIn` or a connection kind's generation, are final
  from the change that registers them, so bytes that no released Host speaks are never sealed
  before a test exercises them.
- Generator inputs, the other-generation corpus, records that follow the pinned Ghostty's output,
  parity tables shared by the Clients, and generated tables are not frozen.

## Development fingerprint

A protocol fingerprint is the lowercase hexadecimal SHA-256 of UTF-8
`orchbee-protocol-fingerprint-v1` and LF, followed, for each input file in bytewise order of its
repository-relative `/`-separated path, by that path, one NUL byte, the file's lowercase
hexadecimal SHA-256, and LF.

- Input files are the regular files ending in `.rs`, `.zig`, or `.json` under a connection's
  `fingerprintInputs`, except paths with a `tests` or `__tests__` component and files named
  `tests.rs`.
- A symbolic link, a non-Unicode name, or another non-regular entry under an input directory fails
  the build instead of being skipped.
- Each build computes its fingerprints at build time, so a build describes its own source. A
  development or test build admits a peer only with an equal fingerprint; release builds never
  compare fingerprints.

## Fixtures and conformance

Payload fixtures hold `payload`, the exact JSON text; frame fixtures hold `frameHex`, the complete
bytes. `direction` names the sender, `initiator` or `acceptor`.

| File                                                                       | Claim                                                                                                                                                                                                                                                                                                         |
| -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`valid.json`](v1/fixtures/valid.json)                                     | Each decodes in its direction, re-encodes to an equal message, and satisfies the schema                                                                                                                                                                                                                       |
| [`boundary.json`](v1/fixtures/boundary.json)                               | Each value at its limit decodes and satisfies the schema                                                                                                                                                                                                                                                      |
| [`invalid.json`](v1/fixtures/invalid.json)                                 | Each fails to decode in its direction. `layer` names the first layer that refuses it: `frame`, `encoding`, `schema` (the schema also refuses it), or `decoder`. Frame cases add `reader`: `closed`, `foreign`, `malformed`, or `message` when a stream reader returns the frame and leaves the trailing bytes |
| [`host-handover.json`](v1/fixtures/host-handover.json)                     | Valid cases of the `keeper` connection and the `keeper` role, held to the claim of `valid.json`                                                                                                                                                                                                               |
| [`dispatch-progress.json`](v1/fixtures/dispatch-progress.json)             | Valid cases of a Host's query result with `dispatch-progress`, held to the claim of `valid.json`                                                                                                                                                                                                              |
| [`server-handover.json`](v1/fixtures/server-handover.json)                 | Valid cases of `handover` and `handover-result`, held to the claim of `valid.json`                                                                                                                                                                                                                            |
| [`server-handover-invalid.json`](v1/fixtures/server-handover-invalid.json) | Refused cases of `handover` and `handover-result`, held to the claim of `invalid.json`                                                                                                                                                                                                                        |
| [`admission.json`](v1/fixtures/admission.json)                             | `cases` give an acceptor, its load, a frame, and the decision; `replies` and `requestReplies` give the initiator's verdict on a reply                                                                                                                                                                         |

Collection bounds too large for a fixture file — 258 processes and a 1,048,576-byte payload — are
checked by the bindings' own tests. The Rust binding's
[`tests/`](../../crates/connection-handshake/tests/main.rs) consume every fixture, the schema, and
the registry, including its compatibility window and frozen fixtures.
