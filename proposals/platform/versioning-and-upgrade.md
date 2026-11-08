# Versioning and Upgrade

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Accepted by the product owner on 2026-11-07 in ORC-384. Windows first; macOS and Linux adopt the same rules with ORC-328.                                                                                                                                                                                                                                                                                                                           |
| Owner             | Maintainers of `crates/local-runtime`, `apps/orchbee`, `apps/orchbee-server`, `apps/orchbee-worker`, `apps/terminal-host`, and `apps/desktop`, with the owner of each contract in [Connections](#connections)                                                                                                                                                                                                                                       |
| Current authority | [Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries), the [terminal version policy](../../schemas/terminal/docs/version-history.md), the [development cohort contract](../../crates/local-runtime/README.md#development-cohort-contract), [Windows release installation](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation), and [native runtime status](../../apps/orchbee/docs/native-runtime-status.md) |
| Exit condition    | Delete this proposal once P0, P1, and P2 ship and their rules live in the owning standards and package references. A P3 or P4 still open then moves to its own proposal.                                                                                                                                                                                                                                                                            |

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

| Connection                                       | Contracts                                                              | Today                                                           | Target                                                                                                                                                 |
| ------------------------------------------------ | ---------------------------------------------------------------------- | --------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Client ↔ Server                                  | OBLC, OBWP, OBCS, OBAP                                                 | Exact cohort identity                                           | Negotiated in the [handshake](#handshake)                                                                                                              |
| Terminal frames, Host → Worker → Server → Client | OBTF, OBPB, OBRW, OBKG                                                 | Exact `0.1.0`                                                   | Negotiated per attachment                                                                                                                              |
| Worker ↔ Terminal Host                           | OBTP                                                                   | Exact `0.1.0`                                                   | Negotiated; the first released generation stays through 1.x                                                                                            |
| Agent hook → Server and Host                     | v1 rich observation envelope and private APC                           | Older Capsules stay installed until replaced; no version policy | Each delivery declares its protocol generation; Server and Host accept any inside their window, and otherwise the hook exits silently and successfully |
| Server ↔ local Worker                            | OBLW, OBCN                                                             | Exact cohort identity                                           | [Exact](#what-stays-exact)                                                                                                                             |
| Desktop Main ↔ bundled Client                    | OBWP relayed over stdio                                                | One package                                                     | [Exact](#what-stays-exact)                                                                                                                             |
| Server ↔ remote Worker                           | OBCN                                                                   | Not implemented                                                 | Negotiated, when ORC-297 and ORC-338 build it                                                                                                          |
| Release feed → `orchbee update`                  | Release manifest                                                       | Closed `schemaVersion: 1`                                       | [Release manifest](#release-manifest)                                                                                                                  |
| Client → runtime store                           | Store layout, active version, and Server start                         | Each Client starts its own installation's Server                | [Shared runtime store](#shared-runtime-store)                                                                                                          |
| Persisted data                                   | Session catalogs, checkpoints, configuration, and plugin registrations | No migration before the first public release                    | [Persisted data](#persisted-data)                                                                                                                      |

## Handshake

Every negotiated connection starts with one offer from the initiator and one answer from the
acceptor, before any other message, and keeps the result for its lifetime. The generation and
capability names cover every record family the connection carries. On Client ↔ Server the
handshake replaces the cohort comparison of OBLC `ClientHello`; on Worker ↔ Host it replaces the
exact version of OBTP `Hello`, whose feature list becomes the capability names.

| Offer field          | Content                                                                                        |
| -------------------- | ---------------------------------------------------------------------------------------------- |
| Connection kind      | One value per negotiated connection                                                            |
| Generation range     | Lowest and highest supported generation                                                        |
| Capability names     | At most 256 names, each 1–64 bytes of `a`–`z`, `0`–`9`, and `-`                                |
| Product version      | For status and the [restart decision](#restart-decision)                                       |
| Build identity       | For diagnostics and [exact pairs](#what-stays-exact)                                           |
| Environment          | The compiled artifact stem from [environment isolation](environment-isolation.md#environments) |
| Protocol fingerprint | Set only by [development and test builds](#development-builds)                                 |
| Extension slot       | Named optional fields; a receiver ignores names it does not know                               |

The acceptor selects the highest generation in both ranges and answers with it, the capability
names both sides listed, and its own product version, build identity, and extension slot. Otherwise
it rejects the connection, or for frame formats the attachment, with one reason:

| Rejection              | Condition                                                                                          |
| ---------------------- | -------------------------------------------------------------------------------------------------- |
| `initiator-too-old`    | The initiator's highest generation is below the acceptor's lowest                                  |
| `initiator-too-new`    | The initiator's lowest generation is above the acceptor's highest                                  |
| `environment-mismatch` | The environments differ                                                                            |
| `build-mismatch`       | An exact pair whose build identities differ                                                        |
| `fingerprint-mismatch` | A [development or test build](#development-builds) whose fingerprint differs                       |
| `format-unsupported`   | An attachment with no common frame-format generation and no Worker conversion; it names the format |

A version rejection also carries the acceptor's generation range and product version, enough for a
message such as "Desktop 1.2 is too old for the running Session 1.6; update Desktop". Existing
rejections for capacity, malformed input, timeout, and shutdown keep their meanings.

- The environment check is the connection-level check that
  [environment isolation](environment-isolation.md#out-of-scope) assigns here; it backs up the
  separate names each environment already uses.
- From the first public release, the offer, answer, and rejection never change except by new
  optional fields in the extension slot.
- Terminal frames negotiate when a Client attaches. The Client lists the generations it decodes for
  each format, Server forwards the list unchanged, and Worker has the Host produce the highest
  common generation or converts one the Host produces. Inside the
  [window](#compatibility-window) every Client–Host pair attaches: a release that changes a format
  keeps the older generation in the Host, in the Client, or as a Worker conversion.

## What stays exact

- Server ↔ local Worker and Desktop Main ↔ bundled Client admit only an identical build identity
  and reject any other with `build-mismatch`. Each pair comes from one installation and starts and
  restarts together, so it keeps no generation range. A
  [Server handoff](../../apps/orchbee-worker/docs/windows-server-handoff.md) that keeps its Worker
  is therefore valid only between identical builds.
- Every connection requires one environment.
- Known fields stay strictly parsed; out-of-bound or malformed values are rejected. Unknown content
  is accepted only in declared extension slots.
- `cohort.json` keeps its pre-start digest check as the installation's integrity manifest and plays
  no part in connection admission.

## Control requests

From the first public release, every version sends and answers three requests with unchanged
meaning. They belong to the permanent handshake layer and need no common generation, so a newer
side can always inspect and stop an older one; user and environment checks still apply. Server
answers them on its Client endpoint for itself and its Worker; each Host answers them on its broker
endpoint, where a Worker connects. The
[OBLC metadata request](../../crates/local-client-protocol/README.md#read-only-windows-server-metadata)
is the starting point.

| Request | Effect                                                                                                                                                                                                                                |
| ------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `query` | Reports product version, build identity, environment, process ID, generation ranges, and terminal count. A Server also reports its Worker, every Host, and whether its Session can restart without ending terminals. Changes nothing. |
| `drain` | Stops accepting new work while existing work continues. A drained Host accepts no new terminals and exits after its last terminal closes.                                                                                             |
| `exit`  | Settles in-flight work and ends the process. A Host ends its terminals, so it receives `exit` only after the person confirms ending them.                                                                                             |

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

Each negotiated contract's version policy lists its supported generations: the terminal policy in
[version history](../../schemas/terminal/docs/version-history.md), and an equivalent section with
every other contract owner. Every combination inside the window has
[cross-version tests](#cross-version-tests).

## Release manifest

The current
[release manifest](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation)
is closed JSON with `schemaVersion: 1`, so a released updater would reject any later field. P0
replaces it before the first public release, because that release's updater must read every later
manifest:

- The manifest is signed. Installation verifies the signature and every file digest before it
  publishes the version directory.
- Each component — Client, Server, Worker, Terminal Host, and the Agent observation Capsule — lists
  its product version, file digests, the generation range of each connection it takes part in,
  `securityFloor`, and `recommendedFloor`.
- A declared extension slot carries every later field as optional and ignorable. The feed format
  gains only such additions, and every released `orchbee update` can finish an upgrade, at least to
  a bridge release that reads the newer format.

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
  as generation, capability, or cohort.
- Every message ships in `en-US` and `zh-CN` through the localization system.

## Integrity and trust

- Release builds carry code signatures — Windows Authenticode, macOS signing and notarization —
  under ORC-381.
- Dropping cohort comparison does not widen the local boundary: it stays the same operating-system
  user, through pipe permissions and peer token checks, plus the environment check.
- Cohort narrows to one installation's file set and its integrity manifest and no longer means
  connection compatibility. Product copy stops using the word; code names such as `Cohort` stay.
  P0 updates the [Glossary](../../docs/GLOSSARY.md#runtime-roles) entry.
- [Environment isolation](environment-isolation.md) still decides file, pipe, and data names;
  compatibility is judged only inside one environment.

## Development builds

- At build time, each connection gets a protocol fingerprint: a digest of the files that define its
  messages, which P0 lists per connection.
- Development and test builds admit a peer only with an equal fingerprint, in place of a generation
  range. A rebuilt Client with unchanged fingerprints joins the running development Session; a
  changed fingerprint requires a Session restart. After P1, a rebuilt Server or Worker restarts
  only those two and keeps every Host whose Worker ↔ Host fingerprint still matches; any other Host
  follows [Required Host replacement](#required-host-replacement).
- Release builds use maintained generations and capability names, backed by cross-version tests;
  fingerprints play no part in release decisions.

## Persisted data

Before the first public release, persisted formats follow
[Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries) without migration. Afterwards, a newer
version backs up data before migrating it, and an older version refuses to open data in a newer
format and never rewrites it, so rollback stays possible. ORCL-29 decides which formats freeze and
owns their data format versions.

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

| Phase                                       | Content                                                                                                                                                                                                                                                                                                                                     | Depends on                                                     | Gate                            |
| ------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------- | ------------------------------- |
| P0. Handshake                               | Handshake, control requests, and evolution rules on every negotiated connection; exact pairs on build identity; frame-format and Agent hook generations; the signed release manifest with floors; the environment check; development fingerprints; the pre-release generation rule in Data boundaries; Glossary and product copy for cohort | ORC-382 slice 1, shipped                                       | Before the first public release |
| P1. Upgrade without terminal loss (ORC-296) | Restart decision and display; Server and Worker restart; Host adoption, lifetime, and required replacement; the shared runtime store; status output                                                                                                                                                                                         | P0, the Windows Host check below, and a durable binding record | Before the first public release |
| P2. Cross-version tests                     | [Cross-version tests](#cross-version-tests) in the gate                                                                                                                                                                                                                                                                                     | P1 and the first public release                                | From the second release         |
| P3. Live handoff prototype                  | Windows prototype, feasibility report, and Host split evaluation                                                                                                                                                                                                                                                                            | P1                                                             | Optional; no date               |
| P4. Live handoff                            | Opt-in handoff for a required replacement; POSIX with ORC-328                                                                                                                                                                                                                                                                               | P3, accepted by the product owner                              | Optional                        |

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
  explicit runtime variables is refused; `query` reports every process's version.
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
