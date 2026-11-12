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
| [`v1/fixtures/`](v1/fixtures/)                                                         | Valid, boundary, invalid, and admission fixtures                                             |
| [Versioning and Upgrade](../../proposals/platform/versioning-and-upgrade.md#handshake) | Accepted design and rationale until P0 to P2 ship                                            |

`crates/connection-handshake` is the Rust binding, and `apps/terminal-host/src/handshake` is the
Terminal Host's Zig binding. Both are independent implementations of this contract; nothing
generates source from it. From the first public release, `OBHS` and its messages never change
except by new members of `extensions`, and the fixtures of this directory freeze.

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

| `type`         | Sender    | Members besides `type` and optional `extensions`                                                            |
| -------------- | --------- | ----------------------------------------------------------------------------------------------------------- |
| `offer`        | Initiator | `connection` (name), `generations`, `capabilities`, `productVersion`, `build`, `environment`, `fingerprint` |
| `query`        | Initiator | `environment`                                                                                               |
| `drain`        | Initiator | `environment`                                                                                               |
| `exit`         | Initiator | `environment`                                                                                               |
| `answer`       | Acceptor  | `generation`, `capabilities`, `productVersion`, `build`                                                     |
| `rejection`    | Acceptor  | `reason` (name), and the acceptor's `generations`, `productVersion`, and `environment`                      |
| `query-result` | Acceptor  | `processes`, and `restartKeepsTerminals` (boolean) exactly when the first process's `role` is `server`      |
| `drain-result` | Acceptor  | `state`                                                                                                     |
| `exit-result`  | Acceptor  | None                                                                                                        |
| `unsupported`  | Acceptor  | `request` (name), the requested type                                                                        |

- An initiator frame whose `type` is a name that is no known message type is a control request of
  an unknown type. It must carry a valid `environment`; its other members are not read.
- A known message type in the wrong direction is malformed. An acceptor frame of an unknown type
  is malformed.
- `reason`, `role`, and `state` grow: a receiver reads an unknown name as `unknown`. Reasons are
  `malformed`, `connection-unsupported`, `environment-mismatch`, `initiator-too-old`,
  `initiator-too-new`, `fingerprint-mismatch`, `capacity`, `stopping`, and `timeout`. Roles are
  `server`, `worker`, `terminal-host`, `machine-hub`, and `machine-link`. States are `ready`,
  `draining`, and `stopping`.
- `processes` lists 1 to 258 entries: the answering process first, then for a Server its Worker
  and up to 256 connected Hosts. Each entry has exactly `role`, `productVersion`, `build`,
  `environment`, `pid` (integer from 1 to 4,294,967,295), `state`, `terminals` (integer from 0 to
  4,294,967,295), and `connections`.
- `connections` lists at most 64 entries in strictly ascending bytewise `kind` order. Each entry has
  `kind`, a connection or delivery name, and `generations`; entries of the first process also carry
  `capabilities`, and entries of later processes never do.

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

[`v1/connections.json`](v1/connections.json) holds three closed objects, each keyed by contract name.

| Member                  | Entries                                                       | Entry members                                          |
| ----------------------- | ------------------------------------------------------------- | ------------------------------------------------------ |
| `negotiatedConnections` | `client-server`, `machine-hub`, `machine-link`, `worker-host` | `generations`, `capabilities`, and `fingerprintInputs` |
| `frameFormats`          | `obkg`, `obpb`, `obrw`, `obtf`                                | `generations`                                          |
| `deliveries`            | `agent-hook`, `machine-route`                                 | `generations`                                          |

- `generations` is the supported range. Every contract starts at generation 1.
- `capabilities` lists `{name, addedIn}` entries in bytewise name order, where `addedIn` is the
  product version of the release that added the name. Only negotiated connections carry
  capability names; a frame format or delivery gains features through its connection or its
  `extensions`.
- `fingerprintInputs` lists repository-relative `/`-separated directories in strictly ascending
  bytewise order; none repeats or contains another, and each exists. The `worker-host` list
  includes the Terminal Host's `src/handshake` binding.
- Each binding mirrors these numbers in its own constants under a conformance check.
- What a connection carries after its `answer` belongs to that connection's own contract. The
  [machine connection contracts](../machine/README.md) state it for `machine-hub` and
  `machine-link`, whose `fingerprintInputs` both list `schemas/machine` and the source of their
  binding, `crates/machine-protocol`.

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

| File                                           | Claim                                                                                                                                                                                                                                                                                                         |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`valid.json`](v1/fixtures/valid.json)         | Each decodes in its direction, re-encodes to an equal message, and satisfies the schema                                                                                                                                                                                                                       |
| [`boundary.json`](v1/fixtures/boundary.json)   | Each value at its limit decodes and satisfies the schema                                                                                                                                                                                                                                                      |
| [`invalid.json`](v1/fixtures/invalid.json)     | Each fails to decode in its direction. `layer` names the first layer that refuses it: `frame`, `encoding`, `schema` (the schema also refuses it), or `decoder`. Frame cases add `reader`: `closed`, `foreign`, `malformed`, or `message` when a stream reader returns the frame and leaves the trailing bytes |
| [`admission.json`](v1/fixtures/admission.json) | `cases` give an acceptor, its load, a frame, and the decision; `replies` and `requestReplies` give the initiator's verdict on a reply                                                                                                                                                                         |

Collection bounds too large for a fixture file — 258 processes and a 1,048,576-byte payload — are
checked by the bindings' own tests. The Rust binding's
[`tests/`](../../crates/connection-handshake/tests/main.rs) consume every fixture, the schema, and
the registry.
