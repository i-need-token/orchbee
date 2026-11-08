# Versioning and Upgrade

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Accepted by the product owner on 2026-11-07 in ORC-384; ORC-393 settled the P0 details on 2026-11-08. Windows first; macOS and Linux adopt the same rules with ORC-328.                                                                                                                                                                                                                                                                                                                                            |
| Owner             | Maintainers of `crates/local-runtime`, `apps/orchbee`, `apps/orchbee-server`, `apps/orchbee-worker`, `apps/terminal-host`, and `apps/desktop`, with the owner of each contract in [Connections](#connections)                                                                                                                                                                                                                                                                                                      |
| Current authority | [Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries), the [connection contract](../../schemas/connection/README.md), the [terminal version policy](../../schemas/terminal/docs/version-history.md), the [development cohort contract](../../crates/local-runtime/README.md#development-cohort-contract), [Windows release installation](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation), and [native runtime status](../../apps/orchbee/docs/native-runtime-status.md) |
| Exit condition    | Delete this proposal once P0, P1, and P2 ship and their rules live in the owning standards and package references. A P3 or P4 still open then moves to its own proposal.                                                                                                                                                                                                                                                                                                                                           |

## Outcomes

- An upgrade never interrupts running terminals or Agents.
- An update replaces only the components it changed: a Client-only change needs only a Client
  reopen.
- Versions coexist: a CLI and Desktop of different versions, or a remote machine that was not
  upgraded, share one Session.
- One explicit rule decides whether a restart is needed, including after changes that no code
  anticipated.

Everything a person can lose — shells, Agents, builds, screens, and history — lives in the Terminal
Host. The Host therefore stays, and every other process may be replaced at any time.

## Version kinds

| Kind                                     | Example                                        | Used by                                   | Changes when                                                                  |
| ---------------------------------------- | ---------------------------------------------- | ----------------------------------------- | ----------------------------------------------------------------------------- |
| Product version                          | `1.4.2`                                        | People, the release feed, and bug reports | Every release                                                                 |
| Protocol generation and capability names | Terminal generation 1 with `clear` and `reset` | Each connection's [handshake](#handshake) | The generation only on an incompatible change; a name with each added feature |
| Handoff format version                   | Host handoff format `3`                        | Live handoff (P4)                         | The handoff content format changes                                            |
| Data format version                      | Session checkpoint format `2`                  | Reading and writing persisted data        | A persisted format changes (ORCL-29)                                          |

Only the product version is shown to people. Build identity — the source commit and the
installation's cohort digest — serves diagnostics and integrity; only
[exact pairs](#what-stays-exact) compare it.

## Connections

| Connection                                       | Contracts                                                              | Today                                                   | Target                                                      |
| ------------------------------------------------ | ---------------------------------------------------------------------- | ------------------------------------------------------- | ----------------------------------------------------------- |
| Client ↔ Server                                  | OBHS, then OBLC, OBWP, OBCS, OBAP                                      | OBLC `ClientHello` admits only an equal cohort identity | Negotiated in the [handshake](#handshake)                   |
| Terminal frames, Host → Worker → Server → Client | OBTF, OBPB, OBRW, OBKG                                                 | Exact `0.1.0`                                           | A generation per format, negotiated per attachment          |
| Worker ↔ Terminal Host                           | OBHS, then OBTP                                                        | OBTP `Hello` with exact `0.1.0`                         | Negotiated; the first released generation stays through 1.x |
| Agent hook → Server                              | Rich observation envelope, correlation hint, and private APC           | Closed `version: 1`; older Capsules stay installed      | [Agent hook deliveries](#agent-hook-deliveries)             |
| Server ↔ local Worker                            | OBLW, OBCN                                                             | Exact cohort identity                                   | [Exact](#what-stays-exact)                                  |
| Desktop Main ↔ bundled Client                    | OBWP relayed over stdio                                                | One package                                             | [Exact](#what-stays-exact)                                  |
| Saved machine, local Client → remote Orchbee     | SSH probe, `--machine-tunnel-v1` helper, and routed command packet     | Equal product version, protocols, and cohort            | [Saved machines](#saved-machines)                           |
| Server ↔ remote Worker                           | OBCN                                                                   | Not implemented                                         | Negotiated, when ORC-297 and ORC-338 build it               |
| Release feed → `orchbee update`                  | Release manifest                                                       | Closed, unsigned `schemaVersion: 1`; never published    | [Release manifest](#release-manifest)                       |
| Client → runtime store                           | Store layout, active version, and Server start                         | Each Client starts its own installation's Server        | [Shared runtime store](#shared-runtime-store)               |
| Persisted data                                   | Session catalogs, checkpoints, configuration, and plugin registrations | No migration before the first public release            | [Persisted data](#persisted-data)                           |

## Handshake

Every negotiated connection starts with one offer from the initiator and one answer or rejection
from the acceptor, before any other record, and keeps the result for its lifetime. The generation
and capability names cover every record family the connection carries.

| Connection kind | Initiator → endpoint              | After the answer                                                                                                                                                      |
| --------------- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `client-server` | Client → Server's Client endpoint | OBLC drops its cohort fields: kind 1 enters the Client phase and kind 2 accepts it, or kinds 4/5 run a Session stop. [`query`](#control-requests) replaces kinds 6/7. |
| `worker-host`   | Worker → Host's broker endpoint   | OBTP `Hello` keeps its session limits; its protocol field carries the generation, and its feature list stays empty.                                                   |

The permanent handshake frame `OBHS` carries the offer, answer, rejection, and
[control requests](#control-requests): four ASCII bytes `OBHS`, a big-endian `u32` payload length
from 1 to 1,048,576, and that many bytes holding one UTF-8 JSON object.

- Every message is a closed object with a `type` member. An unknown or duplicate member, or a value
  out of bounds, is malformed. The optional `extensions` object is the only open slot: its member
  names follow the capability-name grammar, and a receiver ignores names it does not know.
- `generations` is `{lowest, highest}`, with 1 ≤ `lowest` ≤ `highest` ≤ 65,535.
- `capabilities` lists at most 256 distinct names in bytewise order, each 1–64 bytes of `a`–`z`,
  `0`–`9`, and `-`.
- `productVersion` is 1–64 printable ASCII bytes, for status and the
  [restart decision](#restart-decision).
- `build` is `{commit, cohort}`, both optional: the 40-hex source commit that release packaging
  passes as `ORCHBEE_SOURCE_COMMIT`, and the 64-hex cohort identity. Host sends no cohort.
- `environment` is the compiled identity from [What stays exact](#what-stays-exact): 1–64 bytes of
  `a`–`z`, `0`–`9`, and `-`, compared bytewise.
- `fingerprint` is 64 lowercase hex digits, present exactly when `environment` is not `orchbee`.

ORC-393 phase A resolved the byte-level details this section leaves open by taking the smallest,
strictest reading; the [connection contract](../../schemas/connection/README.md) states them
normatively:

- The payload is one RFC 8259 object with optional surrounding whitespace; a byte order mark or a
  second value is malformed. Duplicate members are malformed at every depth, including inside
  `extensions` values, and arrays and objects nest at most 32 deep.
- Integer members use plain integer tokens: `1.0`, `1e0`, and `-0` are malformed. Numbers must be
  finite binary64 values, and a lone surrogate escape is malformed.
- `null` is malformed for every member, and absent optional members are omitted. `extensions`
  appears only on the message object, and an empty `extensions` equals an absent one.
- `capabilities` must arrive in strictly ascending bytewise order; a receiver never sorts.
  "Printable ASCII" in `productVersion` excludes the space (0x21–0x7E).
- An initiator frame whose `type` is a name but no known message type is a request of an unknown
  type: it needs a valid `environment`, its other members are unread, and it passes the request
  checks before getting `unsupported`. A known type sent in the wrong direction is malformed.
- Outside `orchbee`, equal fingerprints whose ranges share no generation are a
  `fingerprint-mismatch`; the answer selects the highest common generation in every environment.
- A request is checked only for environment and then for a free control slot, rejected with
  `capacity`; draining and stopping processes still answer it.
- `query-result` lists 1 to 258 processes. `pid` is 1 to 4,294,967,295 and `terminals` 0 to
  4,294,967,295. Each process lists at most 64 connections in strictly ascending `kind` order,
  `capabilities` appears exactly in the answering process's entries, and `restartKeepsTerminals`
  appears exactly when the first process is a Server. `drain-result` uses the query's state names.
- A reader reports a foreign protocol at the first byte that differs from `OBHS`, and refuses an
  out-of-bounds length before reading the payload. An initiator also closes on a reply that pairs
  with neither its offer nor its request.

| Message     | Sent by   | Members besides `type` and `extensions`                                                                              |
| ----------- | --------- | -------------------------------------------------------------------------------------------------------------------- |
| `offer`     | Initiator | `connection`, `generations`, `capabilities`, `productVersion`, `build`, `environment`, and `fingerprint`             |
| `answer`    | Acceptor  | `generation`, the highest in both ranges; `capabilities`, the names both sides listed; `productVersion`; and `build` |
| `rejection` | Acceptor  | `reason`, plus the acceptor's `generations`, `productVersion`, and `environment`                                     |

The acceptor runs these checks in order and rejects at the first failure:

| Check                                                                              | Rejection                |
| ---------------------------------------------------------------------------------- | ------------------------ |
| The frame decodes as an offer or a control request                                 | `malformed`              |
| `connection` is the endpoint's kind                                                | `connection-unsupported` |
| `environment` equals the acceptor's                                                | `environment-mismatch`   |
| In `orchbee`, the initiator's highest generation is at least the acceptor's lowest | `initiator-too-old`      |
| In `orchbee`, the initiator's lowest generation is at most the acceptor's highest  | `initiator-too-new`      |
| In any other environment, the [fingerprints](#development-builds) are equal        | `fingerprint-mismatch`   |
| A connection slot is free                                                          | `capacity`               |
| The acceptor is neither draining nor stopping                                      | `stopping`               |

- A candidate that sends no complete frame within 2 seconds is closed after a best-effort `timeout`
  rejection. A receiver reads an unknown `reason` as `unknown`.
- The initiator closes a connection whose answer selects a generation outside its range or lists a
  name it did not offer. A reply that does not start with `OBHS` comes from a peer that predates the
  handshake; the initiator reports that peer as the older side, without a version.
- The rejection's product version and range suffice for a message such as "Desktop 1.2 is too old
  for the running Session 1.6; update Desktop"; [Status and diagnostics](#status-and-diagnostics)
  lists the messages.
- The environment check is the connection-level check that
  [environment isolation](environment-isolation.md#out-of-scope) assigns here; it backs up the
  separate names each environment already uses.
- Records after the answer carry the negotiated generation in their existing version field: the
  OBLC, OBWP, OBCS, and OBAP frame versions, and the OBTP envelope major with minor and patch zero.
  Generation 1 replaces OBTP `0.1.0`. A record with any other version is malformed.
- Each frame format carries its own generation in its major byte, with minor zero; generation 1
  replaces `0.1.0`. A generation 1 attachment carries no format list, which means generation 1 of
  every format. The release that first adds a format generation adds the capability
  `frame-generations` to both negotiated connections: the Client then lists the range it decodes
  for each format, Server forwards the list unchanged, and Worker has the Host produce the highest
  common generation or converts one the Host produces; otherwise the attachment is rejected with
  `format-unsupported`, naming the format. Inside the [window](#compatibility-window) every
  Client–Host pair attaches: a release that changes a format keeps the older generation in the Host,
  in the Client, or as a Worker conversion.
- From the first public release, `OBHS` and its messages never change except by new members of
  `extensions`.

## Generation registry

`schemas/connection` becomes the language-neutral authority for `OBHS` and its valid, boundary, and
invalid fixtures, and holds the one declaration of generation numbers. Its `v1/connections.json`
lists, for each negotiated connection (`client-server`, `worker-host`), frame format (`obtf`,
`obpb`, `obrw`, `obkg`), and unnegotiated delivery (`agent-hook`, `machine-route`), the supported
generation range and the capability names with the release that added each, plus each negotiated
connection's [fingerprint inputs](#development-builds).

- Every contract starts at generation 1 with no capability names. Only negotiated connections
  carry capability names, as `{name, addedIn}` entries; a frame format or a delivery gains features
  through its connection's names or its `extensions`.
- `crates/connection-handshake` is the Rust binding and the Host's `src/handshake` the Zig binding.
  Their constants, and those of the TypeScript frame decoders, mirror the registry under a
  conformance check, as `schemas/terminal/v1/constants.json` works today.
- Each contract's version history records what a generation changed and links to the registry
  instead of declaring numbers.

## What stays exact

- Server ↔ local Worker keeps OBLW admission on the cohort identity, which serves as its build
  identity; its `IncompatibleCohort` rejection becomes `BuildMismatch`. Each pair comes from one
  installation and starts and restarts together, so it keeps no generation range, and OBLW and OBCN
  never freeze. A [Server handoff](../../apps/orchbee-worker/docs/windows-server-handoff.md) that
  keeps its Worker is therefore valid only between identical builds.
- Desktop Main ↔ bundled Client is exact by construction: Main starts only the `orchbee` executable
  bundled or staged with it, which verifies its own `cohort.json`. No check is added on stdio, and
  the relay may change freely. Desktop reaches the Server through that Client's `server connect`,
  which runs the Client ↔ Server handshake.
- Every connection requires one environment. Negotiated connections check it in the handshake, each
  exact pair shares one build and so one environment, and an Agent hook reaches only the endpoint
  its terminal's Session exported.
- The environment is the build's compiled identity. `crates/local-runtime` derives it on every
  platform the way Windows does today: `ORCHBEE_ARTIFACT_STEM`, otherwise
  `ob-test-<checkout digest>`. Host receives it as the `environment` build option from staging,
  packaging, and the test gate, and a Host built without it matches no peer. On macOS and Linux,
  development staging passes its `ob-dev-` identity while file, socket, and directory names stay
  `orchbee` until ORC-328, so there the check alone keeps a development Client out of a release
  Session.
- Known fields stay strictly parsed; out-of-bound or malformed values are rejected. Unknown content
  is accepted only in declared extension slots.
- `cohort.json` keeps its pre-start digest check as the installation's integrity manifest and plays
  no part in connection admission.

## Control requests

From the first public release, every version sends and answers three requests with unchanged
meaning. They belong to the permanent handshake layer and need no common generation, so a newer
side can always inspect and stop an older one.

- A request is the only `OBHS` frame its connection sends, in place of an offer, and carries the
  requester's `environment`. The acceptor checks user and environment as for an offer, replies once,
  and closes. An unknown request type gets `unsupported`, carrying the requested type.
- Server answers on its Client endpoint, on every platform, for itself, its Worker, and its Hosts.
  Each Host answers on its broker endpoint beside its single Worker session, with at most four
  control connections at a time; an offer while a Worker session is active is rejected with
  `capacity`.
- `query` replaces the OBLC metadata request (P0 delivered it as the
  [process query](../../crates/local-client-protocol/README.md#process-query)).
  Session stop stays an OBLC request after the handshake; without a common generation, a newer side
  ends an older Session with `exit` to its Server and then to each Host (P1).
- Request types, reply members, and their meanings freeze with `OBHS`; later facts go into
  `extensions`.

| Request | Reply                       | Effect                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------- | --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `query` | `query-result`              | Changes nothing. `processes` lists the answering process first, then for a Server its Worker and up to 256 connected Hosts. Each entry has `role` (`server`, `worker`, or `terminal-host`, read as `unknown` otherwise), `productVersion`, `build`, `environment`, `pid`, `state` (`ready`, `draining`, or `stopping`, read as `unknown` otherwise), `terminals`, and `connections`: each connection or delivery kind the process takes part in, with its `generations`, plus `capabilities` in the answering process's entry. A Server adds `restartKeepsTerminals`, false until P1. |
| `drain` | `drain-result` with `state` | Idempotent; stops accepting new work while existing work continues. A drained Server rejects offers with `stopping`, accepts no new intents, finishes in-flight operations, and drains its Worker. A drained Host accepts no new terminals and exits after its last terminal closes.                                                                                                                                                                                                                                                                                                  |
| `exit`  | `exit-result`               | Settles in-flight work, replies, and ends the process. A Server ends itself and its Worker and never asks a Host to end or a terminal to be disposed; until P1 records the Session for this case, the next activation treats it as an unclean end. A Host ends its terminals, so it receives `exit` only after the person confirms ending them.                                                                                                                                                                                                                                       |

## Message evolution

P0 adopts these rules in every negotiated contract; they bind from the first public release.

1. A new feature adds a capability name. A peer without it loses only that feature, never the
   connection.
2. A released message never changes a field's meaning or encoding and never loses a field; a change
   adds a message or a capability. New fields go only into declared extension slots. If an older
   peer that ignores a field could report false success, use a new message or capability instead.
3. An enum that may grow declares its fallback: an older peer maps an unknown value to unknown or
   refuses only that request.
4. An unknown request gets an `unsupported` reply and an unknown event is ignored; the connection
   stays.
5. Once a generation appears in a release, its fixtures freeze. Tests never change an older
   generation's expected bytes to admit a new change.

These mechanisms enforce the rules:

- Unnegotiated messages — `OBHS`, [Agent hook deliveries](#agent-hook-deliveries), the routed
  [machine packet](#saved-machines), and the [release manifest](#release-manifest) — are closed
  objects whose only open slot is `extensions`, because their receiver cannot first announce what
  it understands.
- Negotiated record families keep closed decoders and gain a field, message, or enum value only
  under a capability name, which a sender uses only when the answer lists it. For them the
  capability is the extension slot of rule 2.
- A growable enum names its fallback value in its schema, and decoders map unknown values to it.
- Each negotiated record family defines one `unsupported` reply to an unknown request kind and
  ignores unknown event kinds; OBTP keeps its optional flag for this.
- The first public release records the SHA-256 of each released generation's fixtures in the
  [registry](#generation-registry), and a gate check added with P2 fails when one changes.

## Compatibility window

Before the first public release no older generation is kept: an incompatible change advances the
generation and drops the previous one, so negotiation rejects a stale pre-release peer once cohort
admission is gone. The first public release freezes each current generation.

From then on, a release R that introduces generation N+1 of a connection also supports N.
Generation N stays supported in R, in at least the next two stable releases, and for at least 90
days after R ships; a later release may drop it. The Worker ↔ Host generation current at the first
public release stays supported in every 1.x release unless a security fix requires its removal.
A security fix may end any generation before its window closes; a process left without a common
generation then reaches the `required` [tier](#restart-decision).

The [generation registry](#generation-registry) lists each contract's supported generations, and
every combination inside the window has [cross-version tests](#cross-version-tests).

## Agent hook deliveries

- The rich envelope's `version`, its observation's `version`, and the correlation hint's `version`
  carry one generation, declared as `agent-hook` in the registry.
- Server accepts any generation inside its window and answers with that generation's ACK, whose
  bytes never change: generation 1 answers `{"accepted":true,"version":1}` and LF. For any other
  generation it closes without an ACK, and the hook exits silently and successfully.
- The envelope and the hint gain an optional `extensions` object; their other members stay closed.
- The private APC prefix `orchbee.agent.v1;` is a permanent channel name, not a generation. Host
  forwards the payload unread, so only Server judges a hook's generation.
- Before any delivery the hook compares its Capsule's owner record with the inherited
  `ORCHBEE_ENVIRONMENT` and stays silent for another environment, as
  [environment isolation](environment-isolation.md#agent-integrations) describes; the check changes
  no generation.

## Saved machines

- The machine tunnel carries an ordinary Client ↔ Server connection, so the remote Session admits
  the local Client through the handshake.
- The SSH probe requires only a supported remote platform and architecture with an available
  Orchbee installation, and shows the remote product version; it compares no product version,
  protocol version, or cohort across machines.
- The remote helper becomes `--machine-tunnel-v1 <session>` before the first public release,
  dropping its cohort argument. From then on that grammar never changes; a change adds a new helper
  name.
- The routed command packet and its reply drop `cohort`; their `version` is their generation,
  declared as `machine-route`, and each gains an optional `extensions` object. The remote CLI parses
  the routed arguments under its own grammar and reports usage errors as usual.

## Release manifest

The current
[release manifest](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation)
is closed, unsigned JSON with `schemaVersion: 1`. No updater that reads it was ever released: the
repository has no release tag, GitHub Release, or public feed. P0 therefore replaces it with version
2 outright and deletes the version 1 reader, without a bridge release; every released
`orchbee update` reads version 2 and its later additions.

- Each channel and platform has one manifest, `release.json`, with a detached signature document,
  `release.json.sig`, beside it. The default feed is the latest manifest under orchbee.com, served
  from Cloudflare R2; GitHub Releases keeps every release. `ORCHBEE_UPDATE_MANIFEST` still
  overrides the location, and the signature is required either way.
- The updater reads at most 1 MiB of manifest and 4 KiB of signature document, verifies the
  signature over the exact bytes before parsing, and checks product, platform, and channel. It then
  downloads every file and verifies its size and SHA-256 before it publishes the version directory.
  These bounds never shrink.
- Installation requires `cohort.json` and every file it names, and installs every listed file; the
  fixed file list of version 1 goes away.
- The reader and the verification compile on every platform. Install effects stay Windows-only
  until ORC-328 and ORC-381 add macOS and Linux; ORC-397 decides where release data lives.

| Member          | Content                                                                                                                                                                                                                                                                                                                                 |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `schemaVersion` | `2`                                                                                                                                                                                                                                                                                                                                     |
| `product`       | `orchbee`                                                                                                                                                                                                                                                                                                                               |
| `channel`       | 1–32 bytes of `a`–`z`, `0`–`9`, and `-`                                                                                                                                                                                                                                                                                                 |
| `platform`      | `windows-x86_64`, `macos-aarch64`, `macos-x86_64`, `linux-x86_64`, or another 1–32 bytes of `a`–`z`, `0`–`9`, `_`, and `-`; an updater accepts only its own                                                                                                                                                                             |
| `version`       | The product version, SemVer 2.0.0 without build metadata                                                                                                                                                                                                                                                                                |
| `buildId`       | As in version 1                                                                                                                                                                                                                                                                                                                         |
| `files`         | Up to 4,096 `{path, url, sha256, size}` entries within the version 1 bounds                                                                                                                                                                                                                                                             |
| `components`    | Entries for `client`, `server`, `worker`, `terminal-host`, and `agent-observation-capsule`, each with `id`, `version`, `files` (paths listed in `files`), `connections` (each kind with its `generations`), and optional `securityFloor` and `recommendedFloor`. An updater skips an unknown `id`, and a missing entry imposes nothing. |
| `extensions`    | The only open slot, in the manifest and in each component                                                                                                                                                                                                                                                                               |

| Signature element  | Decision                                                                                                                                                                                                                                                                                                                                                                      |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Algorithm          | Ed25519 as defined in RFC 8032                                                                                                                                                                                                                                                                                                                                                |
| Signed bytes       | UTF-8 `orchbee-release-manifest-v2`, LF, then the exact manifest bytes                                                                                                                                                                                                                                                                                                        |
| Signature document | Closed JSON with 1–4 `signatures`, each `{keyId, algorithm: "ed25519", signature}`, and optional `extensions`. `signature` is base64 of 64 bytes; `keyId` is the first 16 hex digits of SHA-256 over the 32-byte public key. One entry from a compiled key that verifies suffices; other entries are skipped.                                                                 |
| Public keys        | A constant table of `keyId` and public key compiled into `orchbee update`; tests use a separate test key and never the release table                                                                                                                                                                                                                                          |
| Private key        | Generated and held by the product owner outside the repository. The release workflow reads it from the GitHub Actions secret `ORCHBEE_RELEASE_SIGNING_KEY`, PKCS #8 PEM, in a protected environment, and signs with Node's built-in `crypto.sign`.                                                                                                                            |
| Rotation           | A release adds the next public key to the table; later manifests carry signatures from both keys until the old key retires, and then a release removes the old public key. An updater that trusts only a retired key needs a manual reinstall, so a key retires no sooner than a year after its successor ships unless it is compromised, which stops its signatures at once. |
| Verifier           | `ed25519-dalek` with default features off, subject to dependency admission                                                                                                                                                                                                                                                                                                    |

`securityFloor` is the lowest version of the component that may keep running. `recommendedFloor` is
the lowest version that gains nothing from a restart; release tooling proposes the release in which
the component last changed. A person sets both when publishing, so the decision never depends on
code predicting future changes. An absent floor imposes nothing.

## Restart decision

The newer side decides; an older process only answers `query` and obeys `drain` and `exit`.
`orchbee update` queries every running Session of its environment, and a newly opened TUI or
Desktop queries its Session. Each gives every Server, Worker, and Host older than the
[active version](#shared-runtime-store) one tier, from the generation ranges and floors in the
active version's manifest, so every evaluator reaches the same tier:

| Tier          | Condition, first match                                                                                                        | Shown as                                                                         |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `required`    | No common generation with the active version's Client (for Server) or Worker (for a Host), or a version below `securityFloor` | A confirmation that states the effect                                            |
| `recommended` | A version below `recommendedFloor`                                                                                            | One status word, such as "Update available", whose details offer Restart Session |
| `none`        | Otherwise                                                                                                                     | Nothing                                                                          |

- Server and Worker share one tier, the higher of their two. A Host below its `recommendedFloor`
  raises that shared tier to at least `recommended`, because only Restart Session moves new
  terminals to a newer Host; the Host itself is never restarted for `recommended`, since under the
  new Worker it drains.
- Restart Session in TUI and Desktop, and the planned `session restart` command, replace only
  Server and Worker through [Server and Worker restart](#server-and-worker-restart); terminals and
  Agents continue.
- A `required` confirmation names the effect, such as "Restart the Session; terminals continue" or
  "These 3 terminals restart; supported Agents resume". Declining keeps the older processes running
  and the tier visible. A Server and Worker restart that would leave a Host without a common
  generation lists that Host's terminals in its confirmation.
- A running TUI older than the active version applies the active manifest's Client floors to itself
  and shows the resulting tier. It never restarts itself.
- `update` text and `orchbee.update` JSON, under [CLI output](../../docs/cli/output.md), list each
  older running process with its tier. `update` itself never restarts or stops a process.

## Component upgrade

| Component                     | After a newer version installs                                                         | The person sees                                     |
| ----------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------- |
| CLI command                   | The next invocation runs it                                                            | Nothing                                             |
| TUI                           | Keeps running and shows its tier; reopening runs the newer version on the same Session | One reopen; terminals continue                      |
| Desktop                       | Updates through its own updater and restarts; the Session is untouched                 | An application restart; terminals continue          |
| Server and Worker             | Keep running while compatible; Restart Session replaces them                           | A few seconds of "reconnecting"; terminals continue |
| Terminal Host                 | Never replaced; see [Host lifetime](#host-lifetime)                                    | Nothing                                             |
| Host that must change, rarely | See [Required Host replacement](#required-host-replacement)                            | One confirmation                                    |

## Server and Worker restart

The Client that starts a restart runs these steps. Terminal processes keep running, and Hosts keep
their output and screens.

1. Send `drain` to the Server. It admits no new Client Connections or intents, finishes in-flight
   operations, and drains its Worker.
2. The Server records Workbench state and terminal bindings durably, in the transactional store the
   [topology proposal](native-runtime-topology.md#accepted-production-lifecycle-direction) accepted
   or a reviewed equivalent, not only in the Workbench checkpoint.
3. Send `exit`. Server and Worker end; Hosts keep running.
4. Activate the active version. Its Worker finds running Hosts through their registrations in the
   Session runtime directory, verifies user, environment, and Host epoch, negotiates, and reconciles
   each inventory with the recorded bindings. A terminal it cannot match enters `RecoveryRequired`
   quarantine and is never guessed at, deleted, or sent input.
5. Clients reconnect and renegotiate without user action.

- Input during the gap is neither queued nor replayed; Clients show that input is unavailable.
- If the new version fails to start or to adopt Hosts, the active pointer returns to the version
  that was running, which starts again from its retained files. Server and Worker never run
  different versions.
- Any later activation completes an interrupted restart, because a new Worker always adopts running
  Hosts.

## Host lifetime

- No update, restart, or tier replaces a Host or ends its terminals without the person's
  confirmation.
- Each Host has its own endpoint, Host epoch, inventory, and registration in the Session runtime
  directory. One Worker brokers Hosts of several versions at once.
- A Worker starts Hosts only from its own installation, and every new terminal goes to that Host.
  It sends `drain` to each older Host it adopts, which then accepts no new terminals and exits after
  its last terminal closes.
- A Host outlives Server and Worker exit, and the next Worker adopts it as in
  [Server and Worker restart](#server-and-worker-restart).
- A machine restart still loses Hosts; the next activation restores the layout and resumes supported
  Agents (ORC-363).

## Required Host replacement

A Host must change only when it is below the Host `securityFloor` or shares no generation with the
active version's Worker. Inside 1.x, only a security fix can cause either.

- The first public release has no live handoff. The confirmation lists the affected terminals; once
  the person confirms, they restart on the current Host, supported Agents resume (ORC-363), and the
  old Host exits. Declining keeps the old Host running. A CLI path that ends terminals follows the
  [release destructive-command rule](environment-isolation.md#release-destructive-commands).
- P3 builds a Windows prototype and a feasibility report. It answers whether terminal processes
  survive the old Host's exit after its ConPTY pipes and handles are duplicated into a new Host;
  whether the screen and history can move in the existing row-window format; and which terminal
  state cannot move, such as some modes or graphics, given that Ghostty VT offers no state
  serialization. It also evaluates splitting the Host into a minimal process that holds PTYs and a
  restartable process that parses terminal output.
- P4, if accepted, adds live handoff, off by default and used only for a required replacement. It
  commits in steps — validate, restore in the new Host, ready, commit, take over — and any failure
  before commit leaves the old Host serving. The handoff content carries its handoff format version;
  different versions fall back to the confirmation. POSIX passes PTY descriptors with `SCM_RIGHTS`,
  scheduled with ORC-328.

## Shared runtime store

- Release Desktop and CLI share one per-user store with one immutable directory per installed
  version, extending the existing
  [Windows release store](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation).
  Desktop copies its bundled runtime into the store when it is installed or updated.
- One atomic pointer names the store's active version, and activation starts that version.
  Installing a version moves the pointer to it, except that only the person's explicit choice, such
  as switching channel, moves it to a lower version. A Client activates a Session only from a
  verified store directory, and every process starts its children from its own directory.
- Installing only adds directories; running files are never overwritten.
- The store layout, the choice of active version, and how a Client starts the active Server never
  change incompatibly after the first public release, because every released Client activates
  Sessions through them.
- A version directory is deleted only when no process runs from it, it is not the rollback target,
  and no Session, Host, or Capsule references it. The previous version stays as the rollback target.
- A Capsule stays while an Agent integration references it; `update` lists those integrations, and
  the person updates them.
- Development and test builds keep starting their staged cohort; see
  [Development builds](#development-builds).
- ORC-381 still selects the Desktop installer; that choice may change how Desktop copies its
  runtime, not these rules.

## Status and diagnostics

- `orchbee --version` prints the product version.
- Session status, in text and JSON
  ([`status server`](../../apps/orchbee/docs/native-runtime-status.md) today), lists Server, Worker,
  and each Host with its product version, terminal count, and tier.
- TUI and Desktop show one short status word only at `recommended` or `required`; the explanation
  lives in details the person opens or in the confirmation.
- A rejection names the side that is too old and the action to take, without internal terms such
  as generation, capability, or cohort. P0 ships the messages below; a missing version is left out
  of the text, and P1 replaces "stop it" with Restart Session.
- Every message ships in `en-US` and `zh-CN` through the localization system.

| Rejection              | TUI `StartupFailure` and CLI `failure.details.reason` | Desktop `session.connection.unavailable.*` | `en-US` text                                                                                                                                       |
| ---------------------- | ----------------------------------------------------- | ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `initiator-too-old`    | `ClientTooOld`, `client-too-old`                      | `desktopTooOld`                            | "Orchbee {client} is too old for the running Session ({session}). Update Orchbee, then try again." Desktop names itself and says "Update Desktop." |
| `initiator-too-new`    | `SessionTooOld`, `session-too-old`                    | `sessionTooOld`                            | "The running Session ({session}) is too old for Orchbee {client}. Run `orchbee server stop`, then try again."                                      |
| `environment-mismatch` | `OtherEnvironment`, `environment-mismatch`            | `otherEnvironment`                         | "This copy of Orchbee belongs to another environment and cannot join the running Session."                                                         |
| `fingerprint-mismatch` | `DevelopmentBuildMismatch`, `fingerprint-mismatch`    | `developmentBuildMismatch`                 | "This development build cannot join the running Session. Stop it with `orchbee server stop`, then try again."                                      |

- The CLI failure keeps kind `incompatible` and adds `clientVersion` and `sessionVersion` details
  when known; Desktop reads them from `server start --format json`. These messages replace
  `StartupFailure::Incompatible`, and every other message that says "cohort" says "installation".

## Integrity and trust

- Windows binaries are signed through SignPath. macOS builds are ad-hoc signed and not notarized at
  first, and Linux binaries carry no code signature (ORC-381). On every platform `orchbee update`
  verifies the [manifest](#release-manifest) signature and every file digest before installing.
- Dropping cohort comparison does not widen the local boundary: it stays the same operating-system
  user, through pipe permissions and peer token checks, plus the environment check.
- Cohort narrows to one installation's file set and its integrity manifest and no longer means
  connection compatibility. Product copy says "installation" instead; code names such as `Cohort`
  stay. P0 rewrites the [Glossary](../../docs/GLOSSARY.md#runtime-roles) entry to "One
  installation's matched set of executables and the Agent observation Capsule, verified against its
  integrity manifest `cohort.json` before any of them starts; it decides no connection admission",
  and adds Protocol generation: "The number naming one incompatible revision of a connection's
  records; peers use the highest generation both support. Never shown to people."
- [Environment isolation](environment-isolation.md) still decides file, pipe, and data names;
  compatibility is judged only inside one environment.

## Development builds

- A protocol fingerprint is the lowercase hexadecimal SHA-256 of UTF-8
  `orchbee-protocol-fingerprint-v1` and LF, followed, for each input file in bytewise order of its
  repository-relative `/`-separated path, by that path, one NUL byte, the file's lowercase
  hexadecimal SHA-256, and LF.
- Inputs are the `.rs`, `.zig`, and `.json` files under the directories the
  [registry](#generation-registry) lists for the connection, except paths with a `tests` or
  `__tests__` component and files named `tests.rs`. `client-server` lists `schemas/connection`,
  `schemas/client-server`, `schemas/workbench`, `schemas/agent-observation`, `schemas/terminal/v1`,
  and the `src` directories of `crates/connection-handshake`, `crates/local-client-protocol`,
  `crates/client-server-protocol`, `crates/workbench-protocol`, `crates/agent-observation-protocol`,
  and `crates/terminal-protocol`. `worker-host` lists `schemas/connection`, `schemas/terminal/v1`,
  `crates/connection-handshake/src`, `crates/terminal-protocol/src`, and the Host's `src/handshake`,
  `src/protocol`, and `src/control`. The Host's `src/handshake` joins the list in the change that
  creates it, because every listed directory must exist. The list is strictly ascending and never
  nests one directory in another.
- Input files must be regular files: a symbolic link, a non-Unicode name, or another entry type
  under an input directory fails the build rather than being skipped.
- `crates/connection-handshake/build.rs` and the Host's `build.zig` compute fingerprints at build
  time from the registry and those files, so each build describes its own source. Frame formats need
  no fingerprint of their own, because both lists include the terminal schema and binding.
- Development and test builds admit a peer only with an equal fingerprint, in place of a generation
  range. A rebuilt Client with unchanged fingerprints joins the running development Session; a
  changed fingerprint is rejected with `fingerprint-mismatch`.
- `pnpm desktop` keeps the running development Session when the newly staged Server, Worker, and
  Terminal Host digests in `cohort.json` equal those of the cohort that started it, and otherwise
  stops it as today; cohort cleanup keeps the cohort that started a running Session. Elsewhere until
  P1, a Server, Worker, or Host change takes effect only after a Session stop.
- After P1, a rebuilt Server or Worker restarts only those two and keeps every Host whose Worker ↔
  Host fingerprint still matches; any other Host follows
  [Required Host replacement](#required-host-replacement).
- Release builds use maintained generations and capability names, backed by cross-version tests;
  fingerprints play no part in release decisions.

## Persisted data

Before the first public release, persisted formats follow
[Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries) without migration. Afterwards, a newer
version backs up data before migrating it, and an older version refuses to open data in a newer
format and never rewrites it, so rollback stays possible. The
[data root layout](data-root-layout.md#persisted-data-versions-and-migration) gives every persisted
file its version, migration owner, backups, and failure behavior (ORCL-29).

## Cross-version tests

- Before the first public release, tests cover the handshake and negotiation rules inside one
  build.
- From P2, every change runs the current build against the previous release in both directions for
  every connection in the window, and the current Worker against the oldest Host generation in the
  window. Earlier release binaries come from the release archive.
- A scripted Windows upgrade installs the previous release, runs a long process in a terminal,
  upgrades to the current build, and checks that the process survives, its output stays
  continuous, Clients reconnect, and no input is replayed.
- Tests assert these rules, not one implementation, under [Testing](../../docs/TESTING.md).

## Out of scope

- Operating-system process migration such as CRIU.
- System services or daemons that need administrator rights.
- Compatibility outside the window.
- Browser and mobile Client version policy: they follow the Client ↔ Server rules, refined when
  their work is scheduled.
- Automatic Server and Worker restart at `recommended`, until P1 is stable.

## Delivery phases

| Phase                                       | Content                                                                                                                                                                                                                                                                                                                                                                                                                    | Depends on                                                     | Gate                            |
| ------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ------------------------------- |
| P0. Handshake                               | `OBHS`, the generation registry, control requests, and evolution rules on every negotiated connection; exact pairs on build identity; frame-format and Agent hook generations; saved machines on the handshake; signed release manifest version 2 with floors; the environment check on every platform; development fingerprints; the pre-release generation rule in Data boundaries; Glossary and product copy for cohort | ORC-382 slice 1, shipped                                       | Before the first public release |
| P1. Upgrade without terminal loss (ORC-296) | Restart decision and display; Server and Worker restart; Host adoption, lifetime, and required replacement; the shared runtime store; status output                                                                                                                                                                                                                                                                        | P0, the Windows Host check below, and a durable binding record | Before the first public release |
| P2. Cross-version tests                     | [Cross-version tests](#cross-version-tests) in the gate                                                                                                                                                                                                                                                                                                                                                                    | P1 and the first public release                                | From the second release         |
| P3. Live handoff prototype                  | Windows prototype, feasibility report, and Host split evaluation                                                                                                                                                                                                                                                                                                                                                           | P1                                                             | Optional; no date               |
| P4. Live handoff                            | Opt-in handoff for a required replacement; POSIX with ORC-328                                                                                                                                                                                                                                                                                                                                                              | P3, accepted by the product owner                              | Optional                        |

P1 starts only after a Windows check shows that a Host and its running terminal processes survive
Server and Worker exit, in Sessions started by the CLI and by Desktop. The check ends Server and
Worker by means other than explicit Session stop, which disposes every terminal today. P1 also
closes two current gaps: a Worker cannot adopt a running Host
([Worker](../../apps/orchbee-worker/README.md#managed-linux-lazy-host-startup)), and Server has no
transactional binding record.

## Acceptance

- **P0:** a Client rebuilt after a Client-only change joins the running development Session
  without a restart; an unsupported generation produces a structured rejection and a localized
  message naming the older side; a Client of another environment that reaches a Session through
  explicit runtime variables is refused; `query` reports every process's version; `orchbee update`
  installs only a manifest whose signature verifies with a compiled key.
- **P1**, on real Windows: with Claude Code or a long-running script in a terminal, upgrading
  Client, Server, and Worker keeps the process running and its output continuous, TUI and Desktop
  reconnect by themselves, and no input is replayed. Desktop and CLI of different versions share
  one Session. With different Host versions, new terminals start on the new Host, old terminals
  continue, and the old Host exits when empty. ORC-296's acceptance scenarios pass.
- **P2:** the current build and the previous release interoperate in both directions, and the
  oldest Host generation in the window works with the current Worker.
- **P3:** the report answers every prototype question in
  [Required Host replacement](#required-host-replacement), and the product owner decides on P4.
- **P4:** with handoff enabled, a required replacement keeps terminal processes running, and a
  failure before commit leaves the old Host serving.
