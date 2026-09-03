# Native Runtime Topology and Responsibility Migration

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Accepted consequential target; the additive native foundations, local Server-Worker and Client Session process spines, deny-only Server OBCS phase, checked terminal Client authority model, Client-Server and Server-Worker terminal-control contracts, Worker-Host broker connection spine, and unintegrated Worker authorization kernel are complete and unshipped, the remaining local-terminal vertical implementation is in progress, and every later migration, cutover, and deletion remains bounded by this design                                                                                                                                                                                                         |
| Owner              | Product architecture and the maintainers of Client, Server, Desktop, Worker, Terminal Host, and Agent integration boundaries                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Current authority  | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md), the owning application and package guides, and the accepted logical topology in [`connected-client-capability-service.md`](connected-client-capability-service.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Decision authority | The product owner has accepted the Client-Server-Worker topology; separate Rust `orchbee`, Rust Server, Rust Worker, and Zig Terminal Host executables; the delivered native foundations summarized below; the code-agnostic launch boundary; one uniform multi-Client model; lazy local activation without Client lifetime ownership; the first Server-relayed local terminal path; one input-and-resize controller per terminal for this milestone; and the CLI, Gateway, SSH Runtime/protocol, Workbench-package, and non-SSH Desktop dispositions below. Agent, Worktree, Archive, Launch Profiles, SSH-specific Desktop behavior, direct Worker attachment, collaborative input, and later production concerns remain reserved |
| Exit condition     | Accept bounded delivery slices, implement them with mapped evidence, move maintained facts into current authorities, resolve every workspace disposition, and delete this proposal after it no longer owns unresolved future-design evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |

Simplified Chinese review translation:
[native-runtime-topology.zh-CN.md](native-runtime-topology.zh-CN.md).
English remains canonical.

## Decision boundary

This proposal refines the parent proposal's physical process topology and explicitly narrows its
logical placement model for native terminal launch. The product runtime does not yet have this
target shape. Completed additive foundations plus the local Server-Worker, Client Session, and
Worker-Host connection spines, the deny-only Server OBCS phase, the terminal-control contracts, the
unintegrated Worker authorization kernel, and the checked terminal Client authority model are
summarized with their current owners below. The remaining local-terminal vertical milestone is in
progress; every other runtime implementation, migration, cutover, and deletion remains gated by the
boundaries and reserved decisions below.

The accepted direction is:

- one logical Client-Server-Worker topology for local and distributed operation;
- a mandatory native Rust `orchbee` entry containing both CLI and TUI surfaces;
- separate private Rust `orchbee-server`, Rust `orchbee-worker`, and Zig
  `orchbee-terminal-host` executables; the optional Agent helper topology, name, and language remain
  reserved with Agent integration;
- optional Web, Desktop, and mobile Client surfaces that consume the same Site authority;
- Terminal Host as a separate node-local process and the sole PTY/VT authority; and
- a narrow Worker baseline for code-agnostic Terminal launch, with Agent runtime effects admitted
  only as an optional capability extension, not a generic Files, Git, remote shell, or
  arbitrary-command service.

Terminal creation does not depend on a Project, repository, Checkout, or Worktree. The baseline
launch contract selects one Worker and carries an optional node-local starting working directory;
omitting it uses the node's accepted default. A managed Worktree feature may produce such a
directory, but it remains an optional upstream capability and never becomes a Terminal dependency.
The target core therefore has no generic `Environment` registry or `EnvironmentId` resolution.
This decision supersedes the parent proposal's universal Environment-before-Worker invariant for
future native terminal launch: Server authorizes one exact Worker directly, and no Environment
identity reaches Terminal Host. The parent proposal is narrowed in the same change. Existing
delivered SSH Environment records and behavior remain current authority until a separately accepted
migration; an unmanaged-SSH adapter may retain `EnvironmentId` as its connection identity without
making it a core Terminal concept.

The current `apps/ssh-environment-runtime` and `@orchbee/ssh-environment-protocol` are not target
architecture assets. The product owner has accepted retiring them after a bounded slice either
migrates every supported use to native Workers or a new SSH design, or explicitly sunsets that use
and accounts for installed artifacts. A future unmanaged-SSH capability may be redesigned after
the old implementation is removed; it need not preserve the current executable, source, package
name, framing, services, release cohort, or Desktop-owned topology. This decision separates the SSH
product capability from its current implementation and does not itself authorize deleting code.

The product owner has also accepted replacing and then deleting the current TypeScript `apps/cli`
implementation. The Rust `orchbee` entry is a new Client implementation, not a source-compatible
port: its public commands, arguments, outputs, exit meanings, discovery, and activation behavior
will be designed under the owning
[`native-command-surface.md`](../cli/native-command-surface.md) proposal. Current CLI behavior
remains current authority until an accepted cutover; this decision alone deletes no code.

The product owner has accepted deleting `apps/gateway` and `@orchbee/gateway-protocol` after Server
owns the production Browser session, relay, and backpressure contract and every supported consumer
has migrated. Gateway is a current implementation boundary, not a target process or compatibility
contract.

The current `@orchbee/workbench` package also is not a target package. Its React shell, browser
interaction state, layout persistence, and presentation logic move into `apps/web`, which remains
the one renderer implementation consumed as a built artifact by Browser and Desktop. Shared
Workbench business vocabulary, semantic intent, snapshot, and delta contracts become
framework-neutral Server/Client contracts with a cross-language source of truth; they do not remain
canonical TypeScript reducers inside a browser package. Rust TUI and future mobile Clients implement
their own presentation over those contracts. CLI may request a bounded versioned Workbench
projection snapshot, but it never serializes a React tree, DOM state, pixels, or another Client's
local focus and window state. After contracts and Web behavior migrate, delete the mixed package
rather than repurposing its browser-only identity as the new cross-language authority.

The non-SSH Desktop Main dispositions below are accepted target boundaries. Agent-owned areas,
managed Worktree, Terminal Archive, Launch Profiles, and SSH-specific Desktop areas remain
deliberately reserved; this proposal does not resolve them by implication.

The accepted topology and Phase 1 repository foundation do not implicitly accept a future command
grammar, Agent design, future SSH design, persistence migration, direct Client-to-Worker data path,
or deletion before each affected current path has an accepted migration or sunset plus rollback
boundary.

## Problem

The current runtime composition concentrates canonical Workbench coordination, local launch and
SSH connection ownership, Terminal Host brokering, Agent observation, semantic-command serving,
Browser access, and Desktop platform effects in `apps/desktop`. The current `apps/server` is a
bounded Node Browser-ingress preview, `apps/cli` is a Node client launched through Electron's Node
mode, and Agent hooks use bundled scripts. The accepted Client-Server-Worker physical topology is
not yet the delivered runtime.

This shape is insufficient for a product in which:

- CLI and TUI are mandatory on every full local installation;
- concurrent TUI, Web, Desktop, and mobile Client instances may observe the same Site;
- one Site may coordinate local and remote execution nodes;
- closing or restarting a Client or Server must not implicitly terminate retained terminals;
- installations with Agent hooks need a small, fast, bounded native helper; and
- the public `orchbee` process must remain smaller and faster to start than the complete backend.

The architecture needs one explicit owner for product intent, node execution, terminal truth, and
client-local presentation, together with a migration path that identifies obsolete workspaces and
modules without deleting them before replacement behavior is accepted and verified.

## Scope

This proposal covers:

- logical roles, process boundaries, executable languages, visibility, and lifecycle;
- separation of terminal-only installation cohorts from optional Agent and managed-Worktree
  capabilities;
- all-local CLI/TUI, Web, and Desktop deployment shapes;
- distributed Worker nodes and remote or mobile Clients;
- control-plane and terminal-data-plane routing;
- state, identity, protocol, trust, continuity, and failure ownership;
- current-to-target responsibility migration at application and Desktop Main module granularity;
- package and workspace retention, reduction, replacement, or retirement candidates;
- delivery ordering, rollback boundaries, and correctness evidence; and
- questions that require product-owner acceptance before implementation.

## Non-goals

This proposal does not:

- claim that the target runtime, accepted local milestone, migration, or deletion is already
  delivered merely because its additive foundations exist;
- treat the current Terminal Host wire as the Client-Server or Server-Worker contract, or define a
  durable storage schema, complete public command grammar, or complete UI design;
- authorize a generic Files, Git, filesystem, remote-shell, or arbitrary-command Worker capability;
- require a Project, repository, Checkout, or Worktree before a terminal can be created;
- require Agent Bridge, Agent orchestration, or managed Worktree in a terminal-only cohort;
- select production TLS, Internet relay, NAT traversal, multi-user tenancy, high availability, or
  cloud deployment policy;
- promise PTY recovery after Terminal Host failure or operating-system restart;
- require a Desktop technology rewrite or make the current React renderer native;
- make Web or Desktop optional replacements for the mandatory CLI and TUI surfaces;
- choose the final takeover or collaborative-input policy beyond this milestone's single active
  controller; or
- treat a smaller public executable as a claim that total installation size must decrease.

## Logical topology

One Site has exactly one active canonical Server authority, zero or more Workers, and zero or more
Clients. A machine may colocate roles without merging their identities, state, or lifecycle.

```text
Client role
|- orchbee CLI
|- orchbee TUI
|- Desktop
|- Web
`- Mobile
        |
        | authentication, shared state, semantic commands
        v
Server role
`- orchbee-server
        |
        | placement, scoped lease, capability authorization
        v
Worker role x N
`- orchbee-worker
   |- Terminal launch validation
   |- Terminal Host broker --------> orchbee-terminal-host --> PTY / process tree / Ghostty VT
   `- optional Agent capability (process topology reserved)
```

Terminal Host is a specialized process inside the physical Worker-node boundary and does not create
an additional logical role. It belongs to the terminal baseline. Agent integration may add a
specialized helper later, but its process boundary is not accepted by this proposal.

## Native executable set

| Executable              | Language | Visibility | Lifetime                        | Accepted responsibility                                                                                                      |
| ----------------------- | -------- | ---------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `orchbee`               | Rust     | Public     | One-shot CLI or interactive TUI | Human command entry, TUI Client, local Site discovery, and explicit lifecycle commands                                       |
| `orchbee-server`        | Rust     | Private    | Long-running Site service       | Server authority, Client sessions, canonical product state, placement, optional Agent orchestration, and remote Client relay |
| `orchbee-worker`        | Rust     | Private    | Long-running per execution node | Node identity, scoped code-agnostic launch, Host brokering, and optional Agent runtime effects                               |
| `orchbee-terminal-host` | Zig      | Private    | On-demand retained node service | PTY, process tree, canonical Ghostty VT, terminal history, input, and attachment authority                                   |

The Agent hook helper's existence, executable name, language, lifetime, and Worker relationship
remain reserved with the Agent capability. It is not part of the accepted executable set.

The proposed installed layout keeps one user-facing name:

```text
bin/
`- orchbee

libexec/
|- orchbee-server
|- orchbee-worker
`- orchbee-terminal-host
```

Installation cohorts do not create protocol variants:

| Cohort                              | Required executables                                            |
| ----------------------------------- | --------------------------------------------------------------- |
| Native Client only                  | `orchbee`                                                       |
| Local terminal Site                 | `orchbee`, Server, Worker, and Terminal Host                    |
| Terminal-only remote execution node | Worker and Terminal Host                                        |
| Agent-enabled execution node        | Reserved; no helper executable set is accepted yet              |
| Browser or mobile Client            | No local native executable; it connects to an authorized Server |

Exact platform installation paths and whether full distributions bundle optional cohorts remain
reserved. Agent integration is never a prerequisite for ordinary terminal creation.

## Role responsibilities

### Client

A Client renders one authorized projection and submits semantic intent. It owns active location,
focus, terminal viewport, search presentation, selection presentation, window geometry, and platform
clipboard effects for its exact Client Session. It owns no canonical Site state, Worker lease, PTY,
or terminal lifecycle.

Every active TUI invocation, browser tab, and independently active Desktop or mobile surface is one
Client Session. Renderer type grants no authority and creates no separate product model. Client
Sessions may select different active locations and view state while consuming the same authorized
shared Workspace, Tab, Panel, and Terminal facts.

`orchbee` contains both CLI and TUI Client surfaces. A CLI invocation is transient; a TUI owns a
longer Client Session. The CLI path must not initialize TUI rendering or load Server, Worker, or Host
engines. The single-entry interaction direction is accepted; the exact command grammar and
parameter contract are owned by `native-command-surface.md`, not inherited from `apps/cli`. Browser,
Desktop, TUI, mobile, and CLI may consume the same versioned semantic projection snapshot, while
each interactive Client owns its own renderer and Client-local view state.

### Server

Server is the unique Site control plane. It owns:

- human authentication, authorization, Client Session lifecycle, and remote ingress;
- canonical Workspace, shared Tab/Panel, Terminal-intent, Agent-task, and optional Project or
  managed-Worktree product references, but no node-local Git effect or recovery journal;
- Worker registry, placement, scoped terminal-launch authorization, lease creation, and fencing
  decisions;
- semantic-command admission, canonical commit, publication, and durable recovery;
- versioned Workbench snapshots and deltas that expose authorized product projection without
  serializing a framework-specific UI tree or another Client's local view;
- product-level terminal creation, close, restart, retention, and outcome reconciliation; and
- a bounded terminal relay for Clients that cannot reach a local Worker attachment endpoint.

Server does not directly access a node's PTY, Terminal Host endpoint, Agent hook endpoint,
node-local files or directories, Git process, or unrestricted shell. Its terminal relay never
becomes a VT, history, or terminal-content authority.

### Worker

Worker represents one execution node to one Site. It owns:

- enrolled Node identity, device credential use, connection generation, capability advertisement,
  compatibility state, and outbound Server connection;
- validation and execution of exact Server-issued operations, operation identities, leases,
  cancellation, heartbeat, progress, results, and fencing;
- admission of one `AuthorizedTerminalLaunch` whose exact Worker, operation identity, lease,
  generation, and fence were bound by Server, never supplied as authoritative Client fields;
- resolution of omitted node-local defaults and delivery of the exact validated launch facts to
  Terminal Host;
- the sole broker relationship with the node-local Terminal Host;
- optional bounded Agent executor lifecycle and whichever local hook ingress is separately accepted
  when that capability is installed; and
- reconciliation of Host inventory and active execution observations after reconnect.

Accepting a starting `cwd` is not a filesystem API. Worker may validate that exact launch input but
does not enumerate directories, proxy ordinary file reads, or infer Git meaning from the path. A
shell or Agent process uses normal operating-system access after launch. Worker does not parse a
user's `git` command, mutate canonical Workbench state, authorize human Clients, schedule other
Workers, or offer a generic remote command escape hatch.

### Terminal Host

Terminal Host remains the node-local terminal authority. It owns retained Terminal identities,
PTYs, process trees, canonical Ghostty state, ordered input, geometry, revisions, generations,
history, selection, export, and attachment-scoped capabilities. It exposes only a private local
same-user endpoint and accepts Worker as its normal broker in the target topology.

Terminal Host owns no Site, Client Session, Project, Workspace, Tab, Panel, Worker placement, Agent
semantics, remote authentication, network listener, or update policy. A Server or Client restart
does not own its lifetime. A Host or operating-system failure does not promise PTY recovery.

### Agent integration helper (reserved)

If accepted later, an Agent hook helper should be a minimal, bounded synchronous native process. It
would accept one bounded invocation,
resolves only the local Worker endpoint and invocation-scoped context, attempts one delivery under a
strict end-to-end deadline, emits no product payload to stdout or stderr, exits zero without changing
the Agent's result when Worker is unavailable, and cannot activate a service. It starts neither
Server nor Worker and owns no durable state. The Agent proposal owns the exact deadline and provider
installation contract. None of this paragraph accepts the helper's existence, name, language, or
process topology.

Choosing Rust supersedes the existing Agent proposal's preference for a Zig bridge only after that
reserved language decision is explicitly accepted and the Agent proposal is updated in the same
implementation slice.

## Code-agnostic terminal-launch boundaries

Terminal creation uses distinct contracts at each trust and ownership boundary. The names below
describe semantic roles; their exact wire representation remains reserved.

| Contract                   | Boundary         | Owned content                                                                                                                                                   |
| -------------------------- | ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TerminalCreateIntent`     | Client to Server | Requested terminal semantics, an optional Worker constraint, cwd intent, and immediate or deferred start intent; never a lease, generation, or fence            |
| `AuthorizedTerminalLaunch` | Server to Worker | Exact Terminal and Worker placement, operation identity, lease, generation, fence, authorized cwd intent, and start mode                                        |
| `WorkerLaunchAdmission`    | Worker-local     | Validation result, accepted fence, execution phase, Host observation, retry status, and reconciliation outcome                                                  |
| `HostLaunch`               | Worker to Host   | Stable creation identity and fingerprint, resolved node-local cwd, exact default-interactive-shell launch facts, and either complete geometry or deferred start |

Server is the only writer of placement authorization, lease generation, and fence. Worker owns
only node-local admission and execution state for the issued authorization. Client fields cannot
be copied into an authorization envelope without Server validation, and Terminal Host receives no
Site, Client Session, Workspace, Project, Checkout, Worktree, Environment, Worker-lease, or
placement semantics.

The terminal baseline launches the Worker's accepted default interactive shell. It has no command
profile, raw shell text, arbitrary executable, or caller-supplied argument-vector variant. A later
profile or noninteractive-command capability requires its own authorization, catalog/default owner,
versioned representation, and negative proof that terminal-create authority alone cannot invoke it.

### Starting-directory semantics

The starting `cwd` may be the user's home, a temporary directory, an ordinary non-Git directory, a
repository directory, or a generated Worktree. A generic create intent may carry one explicit
node-local path or omit it; omission asks the selected Worker to resolve its accepted default.
Worker validates the resolved launch facts and Terminal Host starts the PTY. A shell may later
change directories under normal operating-system permissions without changing canonical product
state.

A Workspace or Panel may present the resulting terminal, and an optional feature may resolve a
starting directory before submitting intent, but association alone does not derive `cwd`. The
existing split and child-terminal semantic behavior is preserved: it references the exact source
Terminal, uses the latest accepted Host-reported `currentCwd` when available, and pins the new
terminal to the source Worker. A node-local path cannot be reused on another Worker. If no accepted
source observation exists, the owning feature applies its documented fallback before submission;
Server and Worker do not reconstruct Project or Workspace policy. Any future cross-Worker path
translation is a separate filesystem capability and is not inferred from string equality.

A managed Worktree flow is strictly upstream:

```text
optional managed Worktree operation --> node-local directory --> TerminalCreateIntent.cwd
```

Server may own canonical Worktree product references, but it does not own the node-local Git
effect, pending-operation journal, serialization, compensation, recovery, or cleanup. If managed
Worktree remains supported, one separately accepted node-local capability owner must own those
facts. Managed Worktree is absent from the baseline Worker capability, and terminal creation remains
fully usable without it.

### Start geometry

Start mode is one closed choice:

- immediate creation carries one complete geometry through the authorized operation and
  `HostLaunch`;
- deferred creation carries no partial or fallback geometry, leaves Host in `awaiting-start`, and
  requires one later Server-authorized `StartTerminal` operation through the exact Worker with one
  complete geometry.

Host retains its existing idempotent exact-replay and conflicting-geometry behavior. No layer may
invent a timeout or 80x24 fallback. Which Client operations choose immediate or deferred creation,
and which authorized Client supplies the first geometry, remain command- and attachment-policy
decisions.

### Environment-variable overlay

An environment-variable overlay is an invocation-scoped capability, not canonical Terminal intent
or durable recovery state. It is bounded and authorized independently, delivered only for the exact
Worker admission and Host creation generation, omitted from audit and diagnostics, and never replayed
after loss or expiry. Server may retain only non-secret outcome metadata; Host retains only the
target-normalized digest required by its current idempotency contract. A retry that still requires
overlay values obtains a fresh capability instead of reconstructing or persisting them.

## Why Server, Worker, and Terminal Host remain separate

- Server is location-independent Site authority, while Terminal Host must live beside the PTY.
  Merging Host into Server would either prevent remote execution nodes or reproduce a partial Server
  on every node.
- Worker is the node identity, capability, lease, and fencing boundary. Keeping it explicit makes a
  local node and a remote node follow the same product protocol instead of maintaining a hidden
  local-only execution path.
- Terminal Host has a different retention boundary. A Worker reconnect, upgrade, or crash can be
  recovered by inventory reconciliation without terminating the PTY and process tree that Host
  still owns.
- Zig keeps the Ghostty integration and terminal resource model in their native ownership domain.
  Rewriting Host in Rust would be justified only by measured maintenance, packaging, or runtime
  evidence, not by saving a process name.
- The separation adds one private node-local Worker-Host IPC boundary. Capability Node Protocol is
  not another process hop: it is the Server-Worker semantic contract carried by local IPC or a
  remote connection.
- Performance-sensitive frame routing can use bounded batching, shared buffers, or a later accepted
  direct attachment design without merging authorities. Process count is not the primary
  optimization target; startup work, copies, queue bounds, and failure coupling must be measured.

## State ownership

| State or effect                                                              | Sole authority                                  | Replicas or projections                                                            |
| ---------------------------------------------------------------------------- | ----------------------------------------------- | ---------------------------------------------------------------------------------- |
| Workspace, shared Tab/Panel, and optional Project/Worktree product reference | Server                                          | Authorized framework-neutral Client snapshots and deltas                           |
| Terminal product intent, placement, retention, and shared outcome            | Server                                          | Worker operation and Client presentation projections                               |
| Worker enrollment, placement authorization, lease generation, and fence      | Server                                          | Exact Worker receives one scoped operation                                         |
| Node credential use, connection generation, launch admission, and execution  | Exact Worker                                    | Server registry and reconciliation observations                                    |
| Node-local launch validation and default resolution                          | Exact Worker                                    | Server stores authorized intent and non-secret outcomes                            |
| PTY, process tree, VT, terminal history, input order, and geometry           | Exact Terminal Host                             | Attachment-scoped Client projections                                               |
| Invocation-scoped environment overlay                                        | Exact Worker launch admission                   | Host receives the values once and retains only its generation-scoped digest        |
| Optional managed-Worktree Git effect, journal, and recovery                  | Separately accepted node-local capability owner | Server may retain only canonical product intent, reference, and reconciled outcome |
| Optional Agent task, orchestration, and shared semantic observation          | Server                                          | Worker executor observation and Client presentation projections                    |
| Active Tab, focus, viewport, search, selection view, and window state        | Exact Client Session or client platform         | None unless an explicitly accepted client-local persistence feature owns it        |
| Platform clipboard and native window effects                                 | Exact client platform                           | No implicit cross-client replication                                               |
| Optional Agent hook delivery, if accepted                                    | Exact selected helper invocation                | Worker and Server may observe one accepted bounded event; helper retains none      |

No colocation path may introduce a second writer. In particular, Server records terminal intent and
outcome but never a second VT; Worker records Host identity and execution observations but never a
second terminal engine; a Client may stage a UI preview but never optimistically commit shared
Workbench state.

## Process lifecycle and supervision

| Process                 | Start policy                                                                  | Stop and restart consequence                                                                                      |
| ----------------------- | ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| `orchbee` CLI           | Per invocation                                                                | Ends only its request or observation                                                                              |
| `orchbee` TUI           | Explicit human launch; may initiate bounded lazy local-Site activation        | Releases its Client Session and attachments; it does not stop Site or terminals                                   |
| Server                  | Exact-cohort lazy local activation or an optional user-service policy         | Clients and Workers reconnect; retained node execution does not infer cancellation                                |
| Worker                  | Exact-cohort local activation or enrolled-node service policy                 | Host remains independently retained; replacement Worker reconciles Host inventory                                 |
| Terminal Host           | Worker starts or discovers the exact compatible version on first terminal use | May exit only under its accepted no-broker/no-retained-terminal policy; Host loss reports terminal loss           |
| Agent helper (reserved) | Provider hook invocation, only if separately accepted                         | Candidate: exits after one bounded attempt; failure is fail-open for the invoking Agent                           |
| Web or Desktop Client   | Explicit human launch                                                         | Releases only that Client Session unless the human separately requests a shared product or Site lifecycle command |

Bare `orchbee` may initiate Server and local Worker activation and wait for bounded readiness. Once
ready, each private process owns its own lifetime; the initiating Client is not its supervisor and
Client exit is not a stop signal. Operating-system user services remain an optional later delivery
mechanism. Exact launchd, systemd, Windows Service, login-time start, update, and uninstall policies
remain reserved.

## Deployment shapes

### Local terminal Site

```text
orchbee CLI -----.
                  +--> local Server --> local Worker --> Terminal Host
orchbee TUI -----'
```

This shape requires no Browser listener, Web renderer distribution, or Desktop shell. Server and
Worker use local IPC while preserving the same identities and protocol semantics used across nodes.

If Agent integration later accepts a helper, one candidate independent hook path is:

```text
Agent hook --> selected helper --> local Worker
```

Omitting the unaccepted helper changes no terminal-launch contract.

### Web plus CLI and TUI

```text
orchbee CLI/TUI -- native local IPC --.
                                        +--> Server --> Worker --> Terminal Host
Browser -- loopback HTTP/WebSocket -----'
```

The Web shape adds renderer assets, Browser authentication, and Browser ingress to Server. It does
not add another canonical coordinator. The current Gateway is a migration input, not a sixth
permanent target process.

### Desktop plus CLI and TUI

```text
orchbee CLI/TUI -- local IPC -----------.
                                        +--> same local Server --> Worker --> Host
Desktop shell / renderer -- local IPC --'
```

Desktop remains a Client shell and platform-effect adapter. It may reuse the Web renderer without
enabling Browser ingress. Closing the Desktop window does not mean stopping the Site or terminating
retained terminals.

### Distributed Site

```text
Machine A: Site
|- Server
|- optional local Worker
|  `- Terminal Host
`- optional local Clients

Machine B: execution node
|- Worker
|- Terminal Host
`- optional Agent helper (reserved)

Machine C: client only
`- Desktop / TUI / Web / Mobile
```

An unmanaged-SSH product capability may later be redesigned as a separate bootstrap, recovery, or
temporary remote path. The current SSH Environment Runtime and protocol are not members of the
target executable set, and a future adapter is not silently reclassified as an enrolled Worker or
a generic `Environment` domain.

## Control plane

Every shared product mutation reaches Server as an explicit versioned semantic command:

```text
Client
  |
  | semantic command
  v
Server
  |- authenticate and authorize
  |- validate identity and expected revision
  |- commit or reject canonical intent
  |- select Worker and issue AuthorizedTerminalLaunch
  |- issue exact operation and lease when a node effect is required
  `- publish the canonical result to authorized Clients
          |
          v
       Worker --> Terminal Host when required
```

Creating, renaming, moving, or closing a Workspace, Tab, or Panel always uses this path. Terminal
create, close, restart, retention, and Agent-task operations also use it. Server receives semantic
intent, not DOM events, pointer events, or human-formatted CLI output.

Client-local Tab activation, focus, window geometry, and presentation state do not become Server
mutations. Detaching a Client view is distinct from closing a shared Panel or terminating a shared
terminal.

## Terminal data plane

Server first authorizes every terminal attachment. The accepted first complete path is identical for
all Client renderers:

```text
Terminal Host <--> Worker <--> Server terminal relay <--> Client
```

Server and Worker inspect only bounded routing, identity, role, authorization, queue, and recovery
facts. They preserve terminal payload bytes and never emulate VT or become terminal-content
authorities. Terminal frames, input, history, selection, and export stay outside the canonical
Workbench commit queue.

A later measured optimization may let a colocated native Client use
`Terminal Host <--> Worker <--> Client`. That path would require an exact Server-issued attachment
capability bound to Client
Session, Terminal, Worker, connection/Host epoch, allowed operations, and expiry or revocation.
Direct Worker attachment is not part of the accepted local-terminal milestone.

Server terminal relay may authenticate, route, enforce bounds, isolate backpressure, and request a
fresh snapshot. It does not parse or emulate VT, own history, durably persist terminal content, or
automatically replay input with an unknown admission outcome.

## Primary workflows

### Local Site activation

1. A TUI, mutating CLI command, Web opener, or Desktop launch discovers the exact local Site.
2. Bare `orchbee`, and only another command whose contract explicitly permits it, may initiate
   bounded activation of the exact compatible private Server and local Worker cohort.
3. The launcher waits for readiness or a typed unavailable/incompatible result, then stops owning
   activation resources; it never remains the private processes' lifetime supervisor.
4. Local Server authenticates the Client through the same-user platform-local boundary, and local
   Worker authenticates independently as the execution node.
5. Help, version, observation-only, and hook paths do not implicitly activate unavailable services
   unless their exact accepted contract says otherwise.

### Shared structure mutation

1. Client submits one semantic Workspace, Tab, or Panel operation with request identity and expected
   revision.
2. Server authorizes and serializes the canonical transition.
3. Server publishes the committed revision to all authorized Clients.
4. If the transition requires terminal or Agent effects, Server reconciles them through exact Worker
   operations and publishes typed outcomes without claiming cross-process atomicity.

### Terminal creation

1. Client sends one `TerminalCreateIntent`; it may request a Worker constraint, cwd semantics, and
   start mode but carries no authoritative operation, lease, generation, or fence.
2. Server authenticates the Client, resolves exact Worker placement, authorizes the cwd and start
   semantics, and commits terminal product intent according to its modeled ordering. Generic
   Workspace or Panel association does not derive `cwd`.
3. For split or child-terminal creation, Server resolves the source Terminal's accepted
   `currentCwd`, pins the source Worker, or follows the owning feature's documented absence fallback;
   it never reuses a node-local path on a different Worker.
4. Server issues one idempotent, fenced `AuthorizedTerminalLaunch` to the exact Worker.
5. Worker validates the authorization and fence, resolves omitted node-local defaults, records one
   `WorkerLaunchAdmission`, and calls Terminal Host with one `HostLaunch`.
6. Host atomically prepares or creates the retained terminal under its existing geometry and
   idempotency contract and returns its typed result without receiving Project, repository,
   Checkout, Worktree, Environment, Site, or Worker-lease semantics.
7. Worker reports the exact observation; Server reconciles and publishes the product outcome.
8. Each Client opens a separate authorized attachment as needed. Deferred creation requires a later
   Server-authorized `StartTerminal` with complete geometry through the same Worker.

If an optional managed Worktree operation preceded terminal creation, its only input to this flow is
the resulting node-local directory used as intent `cwd`.

### Optional Agent observation

This candidate flow is not accepted until the Agent review selects its topology:

1. When the Agent capability is installed, an explicitly installed provider hook may invoke the
   selected helper with bounded event data and exact execution context.
2. The helper attempts one local Worker delivery and exits fail-open.
3. Worker validates and associates the event with an authorized local execution before forwarding a
   bounded observation.
4. Server owns Agent semantic reduction, durable orchestration state, and publication to Clients.
5. Terminal Host never derives Agent semantics from hook data.

## Multi-client terminal semantics

One retained terminal may have several attachment-scoped projections.

| Shared terminal fact                                      | Attachment- or Client-local fact                        |
| --------------------------------------------------------- | ------------------------------------------------------- |
| PTY, shell, current process, terminal output, and exit    | Active Tab and focus                                    |
| Canonical VT content, modes, title, and retained history  | Viewport and scroll position                            |
| Accepted ordered terminal input and its effects           | Search query, result cursor, and highlighting           |
| Canonical terminal geometry and terminal-generated resize | Selection presentation and platform copy effect         |
| Clear, reset, close, and process termination outcomes     | Projection demand, pause, recovery, and local animation |

Host attachment capabilities decide whether an attachment may read, input, or resize. Different
Client Sessions may control different terminals. Several attachments may read one terminal, but the
accepted local milestone grants input and the resize lease to at most one active controller for that
terminal. Other attachments remain non-controlling.

Explicit takeover, separate input and resize controllers, or opt-in collaborative input remains
reserved. No policy may present multiple input streams as independent shells. Selecting, searching,
copying, and scrolling history remain independent where they do not become application mouse or
other PTY input.

## Protocol and transport boundaries

| Boundary                          | Semantic purpose                                                                                                             | Local transport candidate        | Remote transport candidate                        |
| --------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------------------- |
| Client to Server                  | Human session, snapshots, deltas, semantic commands including `TerminalCreateIntent`, attachment bootstrap, and remote relay | Unix-domain socket or named pipe | HTTPS plus bounded binary WebSocket or equivalent |
| Server to Worker                  | Node enrollment, capability versions, `AuthorizedTerminalLaunch`, lease, heartbeat, fencing, progress, and result            | Unix-domain socket or named pipe | Authenticated encrypted outbound node connection  |
| Worker to Terminal Host           | `HostLaunch`, exact terminal lifecycle, input, projection, history, selection, and Host inventory                            | Existing private Host IPC        | None                                              |
| Agent helper to Worker (reserved) | One optional bounded local Agent hook observation, only if separately accepted                                               | Candidate private local IPC      | None                                              |
| Local native Client to Worker     | Server-authorized high-volume terminal attachment when accepted                                                              | Scoped private local IPC         | Reserved; not required for the initial topology   |

`Capability Node Protocol` names Server-Worker semantics, not an extra process or fixed transport.
It runs over local IPC when colocated and over a network connection for a remote Worker.

Current TypeScript protocol packages cannot silently become the Rust contract. The accepted
implementation must choose one canonical schema and behavior source, generate or independently
implement codecs with cross-language conformance evidence, and avoid two mutable protocol
authorities.

## Identity and fencing

At minimum, the target distinguishes:

- Site, human identity, and Client Session;
- stable Node identity, Node Session or connection generation, and device credential;
- optional Project or managed-Worktree product identity, Workspace presentation, exact Worker
  placement, and terminal-launch operation;
- Terminal, Host epoch, attachment, and capability revision;
- Agent task, Agent execution, and hook correlation;
- semantic request, operation, lease, and result identity; and
- transport connection and reconnect generation.

Transport identity never substitutes for product identity. Reconnect creates a fresh authenticated
transport and reconciles stable identities. Stale Server generations, Worker leases, Host epochs,
attachments, and capability revisions fail closed. Unknown terminal input is never replayed
automatically.

## Trust and security constraints

- Human Client credentials and Worker device credentials remain distinct and non-interchangeable.
- Worker normally initiates its Server connection; Terminal Host exposes no LAN or Internet
  listener, and any future Agent helper must preserve that constraint.
- Terminal Host retains its same-user local trust boundary and gains no Site authorization logic.
- A Server-authorized local data path uses a narrowly scoped attachment capability, not ambient
  same-user access as product authorization.
- Every untrusted message is bounded, parsed, authenticated, authorized, and identity-checked at its
  first trusted boundary.
- Worker accepts only explicitly versioned capabilities. A launch operation may carry one starting
  `cwd` and one separately authorized transient environment overlay, but Worker exposes no generic
  directory enumeration, file read/write, unrestricted environment-variable injection, command
  dispatch, raw shell text, arbitrary argument vector, or implicit Files/Git API.
- Terminal content, credentials, hook payloads, and secrets do not enter diagnostics or audit by
  default.
- Same-user process separation limits ownership and crash coupling but is not claimed as a hostile
  multi-tenant security sandbox.

## Continuity and failure model

| Event                                         | Required or candidate behavior                                                                                                        |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| CLI invocation exits                          | Ends only its request or observation                                                                                                  |
| TUI, Web, Desktop, or mobile Client exits     | Releases that Client Session and attachments; shared state and retained terminals remain                                              |
| Client transport loss                         | Marks the attachment disconnected; no deletion or terminal input replay                                                               |
| Server unavailable                            | Workers and Hosts preserve independently retained execution; exact Client and Worker degraded behavior remains reserved               |
| Server restart                                | Clients and Workers reconnect and reconcile Site revision, node generation, execution, Host inventory, and attachment state           |
| Server-Worker link loss                       | Worker accepts no invented authority; retained Host terminals continue; bounded observation retention or loss policy remains reserved |
| Worker restart                                | Host survives independently when possible; replacement Worker establishes a new generation and reconciles exact Host inventory        |
| Terminal Host crash                           | Worker reports terminal loss; no PTY recovery is claimed                                                                              |
| Node or operating-system restart              | Ordinary PTYs and processes may be lost; Site reports the distinct machine-restart outcome                                            |
| Selected Agent helper unavailable or rejected | If that capability is accepted, the hook exits fail-open and creates no fabricated observation                                        |
| Slow remote Client                            | Its bounded relay or attachment is paused or retired without blocking another Client or terminal                                      |
| Lost terminal-input acknowledgement           | Outcome is unknown and never automatically replayed                                                                                   |
| Version or protocol mismatch                  | Reject or quarantine the incompatible connection without replacing a Host that still owns retained terminals                          |

## Application responsibility migration

The additive native foundations and the local-terminal vertical milestone are authorized as stated
above. No row in this section alone authorizes a later consumer migration, public cutover, or
deletion.

| Current or candidate application | Review status and target disposition                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `apps/cli`                       | **Accepted:** build the Rust `orchbee` CLI/TUI as a new Client, migrate only explicitly selected semantic capabilities, switch the packaged public entry atomically, then delete the Node workspace and its launch artifacts; old command and parameter compatibility is not required                                                                                                                                                                                                                                                                                                                                          |
| `apps/orchbee-server`            | **Delivered additive boundary:** the Rust Server owns its in-memory kernel plus separate bounded same-user local Worker and Human Client listeners, admission registries, generations, and liveness attachments. After OBLC admission, Server consumes strict OBCS Create/Start requests sequentially on the same stream only to construct and attempt to write correlated `rejected/not-authorized` results without terminal or Worker effects. Next add authorized launch through a separately accepted slice, then later absorb production Browser ingress and remote terminal relay through separately accepted migrations |
| `apps/server`                    | **Accepted replacement:** keep the delivered Node Browser-ingress preview authoritative until its supported behavior migrates to `apps/orchbee-server`, then delete the Node application and obsolete package identity in the same audited cohort                                                                                                                                                                                                                                                                                                                                                                              |
| `apps/orchbee-worker`            | **Delivered additive boundary:** the separate Rust Worker owns bounded same-user Server registration, retained Worker generation, an authenticated exact-version Host broker connection with retained Host epoch and limits, coupled peer-loss cleanup, and an unintegrated deterministic OBCN authorization/replay/Host-dispatch kernel without terminal effects. Before a carrier invokes that kernel, define terminal retirement and binding capacity; then connect the exact OBCN carrier and Host command mapping. Remote enrollment remains later work                                                                   |
| `apps/terminal-host`             | **Delivered additive connection boundary:** retain the separate Zig application and Ghostty authority; the unshipped Worker now consumes its existing same-user endpoint and handshake as the normal broker connection without changing Zig, migrating Desktop/SSH adapters, sending a terminal command, or merging the process                                                                                                                                                                                                                                                                                                |
| `apps/agent-bridge`              | **Reserved with Agent:** create no target until Agent integration, absence/failure, language, and cohort decisions are accepted                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| `apps/desktop`                   | **Accepted outside reserved areas:** retain the Desktop Client shell, renderer bootstrap, packaging, platform effects, and application-local lifecycle; apply the exact non-SSH module disposition below and keep Agent, Worktree, Archive, Launch Profiles, and SSH-specific areas reserved                                                                                                                                                                                                                                                                                                                                   |
| `apps/web`                       | **Accepted:** retain the sole Browser/Desktop renderer application and absorb the React Workbench shell, browser interaction state, layout persistence, and presentation implementation from `@orchbee/workbench`                                                                                                                                                                                                                                                                                                                                                                                                              |
| `apps/gateway`                   | **Accepted retirement:** delete after Server owns the authenticated Browser session, terminal relay, and backpressure behavior and every supported Desktop/Web consumer migrates                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| `apps/ssh-environment-runtime`   | **Accepted retirement:** it is not a target Worker or compatibility layer. Keep it only as current authority until an accepted slice migrates or explicitly sunsets every supported route and accounts for installed artifacts, then delete it; any later unmanaged-SSH implementation is a new design with no executable-compatibility requirement                                                                                                                                                                                                                                                                            |
| `apps/storybook`                 | **Retain:** development-only visual evidence host; application-owned Web stories move with the React Workbench implementation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

## Desktop Main target disposition

The product owner delegated the non-SSH Desktop boundary and accepted this target disposition.
These are responsibility outcomes, not mechanical directory moves: an accepted row removes the
current Desktop owner only after the destination behavior and recovery evidence exist. Rows marked
reserved remain intentionally undecided.

| Current `apps/desktop/src/main` area                                                                                                      | Accepted or reserved target disposition                                                                                                                                                                                                                              |
| ----------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `client-session` and `workbench-document`                                                                                                 | **Move to Server, then remove from Desktop:** Server owns authenticated Client sessions, canonical Workbench state, commit, snapshot/delta publication, persistence, and reconciliation                                                                              |
| Shared-document portions of `terminal-document-control` and `remote-terminal-creation`                                                    | **Move to Server, then remove from Desktop:** Clients submit terminal intent; Server owns the canonical mutation and authorized Worker placement. Desktop retains no terminal-create authority                                                                       |
| `semantic-command` coordinator, execution ledger, publication, and native local endpoint                                                  | **Move to Server and Rust `orchbee`, then remove from Desktop:** Server owns admission/commit/observation; the public Client owns invocation. A surviving Desktop platform effect is exposed only through a narrow Desktop capability adapter                        |
| `host`, `terminal-binding-registry`, and Host-facing portions of `terminal-lifetime` and `terminal-output-read`                           | **Move to Worker, then remove from Desktop:** Worker owns Host discovery, brokering, bindings, lifecycle effect dispatch, output-read routing, and inventory reconciliation; Terminal Host remains the terminal authority                                            |
| `terminal-projection-control` and Host-facing `broker` behavior                                                                           | **Split, then reduce Desktop:** Server authorizes attachments and Worker routes Host data. Desktop may retain only a bounded Electron transport handoff needed by its sandboxed renderer; it owns no attachment policy, terminal state, or node routing              |
| `browser-access` service lifecycle, authentication material, and relay composition; all of `gateway`                                      | **Move ingress/session/relay ownership to Server and delete the old composition:** Desktop may retain explicit Client actions to request access, open an approved URL, or copy it, but it does not supervise Gateway, sign Browser sessions, or relay terminals      |
| Generic product-placement behavior in `environment`                                                                                       | **Delete from the target core:** native terminal placement resolves directly to Worker. Any registry code still required by the current SSH path remains only until the reserved SSH-specific disposition                                                            |
| `project-folder-acquisition`                                                                                                              | **Retain only the Desktop platform adapter:** it may show a native picker and return the user-selected local path to the authorized Client workflow; it does not create canonical Project state or claim that a client-only machine is the selected Worker           |
| `terminal-default-settings`                                                                                                               | **Split:** move node-local default interactive-shell resolution to Worker. Any product preference or named-profile behavior remains reserved with `terminal-launch-profiles`                                                                                         |
| `application-lifetime`                                                                                                                    | **Split and retain only Desktop lifetime:** window/app quit and renderer teardown remain in Desktop; Site shutdown, shared terminal intent, Worker drain, and Host retention belong to their owning Server, Worker, and Host contracts                               |
| `desktop-clipboard`, `window-actions`, `window-state`, `application-menu`, and `secure-keyboard-entry`                                    | **Retain in Desktop:** exact Client-local or operating-system effects                                                                                                                                                                                                |
| `desktop-renderer-protocol`, preload bootstrap, and remaining `broker` adapters                                                           | **Reduce and retain:** expose only the minimum sandboxed renderer bootstrap and exact Desktop platform capabilities; remove Server, Worker, terminal-authority, Gateway, Agent-owner, and legacy Environment messages when their owning slices migrate               |
| `composition` and `recovery`                                                                                                              | **Rewrite around retained Desktop responsibilities:** keep only Electron Client/platform composition and Client-local recovery; Server owns Site recovery, Worker owns node/Host reconciliation, and no replacement composition may recreate the removed authorities |
| Shared Agent state in `agent-observation` and `agent-integration-management`                                                              | **Reserved with `@orchbee/agent`:** do not decide package, Server, Worker, Bridge, or Desktop presentation ownership in this review                                                                                                                                  |
| `project-worktree`                                                                                                                        | **Reserved:** it remains optional and outside terminal baseline; this review does not decide retention, owner, Git authorization, journaling, cleanup, or deletion                                                                                                   |
| `terminal-archive`                                                                                                                        | **Reserved:** do not move or delete it merely because live-terminal routing moves                                                                                                                                                                                    |
| `terminal-launch-profiles` and non-baseline portions of `terminal-default-settings`                                                       | **Reserved:** do not decide named profiles, raw commands, argument vectors, preference scope, or compatibility here                                                                                                                                                  |
| `ssh-environment`, `ssh-project-enrollment`, `remote-project`, SSH portions of `remote-terminal`, and the remaining SSH registry behavior | **Reserved:** the current SSH Runtime/protocol retirement is accepted, but the exact Desktop user-experience migration or deletion list waits for its own review                                                                                                     |

The migration must update each source and destination `project.json` in the same accepted slice. A
module is deleted only after its state, behavior, recovery, tests, and documentation have one
verified surviving owner.

## Package and workspace reduction audit

No workspace is removed because its name appears in this table. Every removal requires a consumer
scan, public-export decision, migration/compatibility policy, replacement or explicit
product-sunset evidence, and deletion of stale documentation and tests in one accepted slice.

| Workspace or package                                                               | Candidate review outcome                                                                                                                                                                                                                                                                                                                       |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@orchbee/gateway-protocol` and `apps/gateway`                                     | **Accepted coupled retirement:** delete after Server provides the one production Browser/session/relay contract and every supported Desktop/Web consumer migrates; neither old framing nor process topology is a compatibility target                                                                                                          |
| `@orchbee/desktop-client-protocol`                                                 | **Accepted reduction:** retain only sandboxed Desktop bootstrap and exact platform-local capabilities; remove Server, Worker, terminal-authority, and Gateway messages after migration. Agent and SSH-specific messages follow their reserved reviews rather than gaining permanent ownership here                                             |
| `@orchbee/remote-client-protocol`                                                  | **Accepted retention:** keep the human Client boundary; expand only through accepted production session and relay slices, never with Worker device semantics                                                                                                                                                                                   |
| `@orchbee/semantic-command-protocol`                                               | **Accepted decomposition:** retain deliberately selected semantic operations only as framework-neutral contracts/TS bindings while consumed; replace or retire current CLI-specific HTTP/locator transport at cutover, with no syntax compatibility, and use the accepted cross-language schema authority for Rust                             |
| `@orchbee/workbench-client-protocol`                                               | **Accepted bounded retention:** keep only as the TypeScript binding for a transport-neutral Client session, semantic-intent, snapshot, and delta boundary while real TS consumers remain; the canonical cross-language schema is framework-neutral, and Rust/mobile bindings do not depend on this package                                     |
| `@orchbee/terminal-protocol`                                                       | **Accepted retention:** keep as the Host boundary and add Rust conformance without creating a second protocol authority                                                                                                                                                                                                                        |
| `@orchbee/ssh-environment-protocol` and `apps/ssh-environment-runtime`             | Accepted coupled retirement candidates; their source, wire, services, names, and release cohort are not target compatibility contracts. Delete them together only after current consumers, supported-route migration or sunset, installed-artifact handling, and rollback are accepted and verified                                            |
| `@orchbee/agent`                                                                   | **Reserved:** make no package or ownership disposition in this review                                                                                                                                                                                                                                                                          |
| `@orchbee/workbench`                                                               | **Accepted decomposition and deletion:** move its React shell, browser-local interaction state, layout persistence, styles, and stories into `apps/web`; extract shared vocabulary and snapshot/delta contracts to the framework-neutral cross-language authority; move canonical transitions to Server; then delete the mixed browser package |
| `@orchbee/actions`                                                                 | **Accepted retention:** keep framework-independent action discovery and Client-side execution lifecycle while the Web renderer consumes it; do not turn it into a second Server command authority                                                                                                                                              |
| `@orchbee/foundation`                                                              | **Accepted retention with pruning:** keep dependency-light domain-neutral primitives with surviving cross-workspace consumers; audit orphan exports after each owner migration rather than moving backend behavior into it                                                                                                                     |
| `@orchbee/keyboard-shortcuts`                                                      | **Accepted retention:** keep TypeScript Client shortcut normalization while Web or Desktop consumes it; Rust TUI owns its platform input adapter and may share identifiers only through an accepted contract                                                                                                                                   |
| `@orchbee/terminal`                                                                | **Accepted retention:** keep the Browser/Desktop terminal renderer; Rust TUI consumes Terminal Protocol through its own renderer and does not make this browser package obsolete                                                                                                                                                               |
| `@orchbee/ui`, `@orchbee/tab-strip`, `@orchbee/i18n`, and `@orchbee/design-tokens` | **Accepted retention:** keep while the Web/Desktop renderer consumes them; backend migration alone is not evidence for deletion                                                                                                                                                                                                                |
| Current Desktop Main modules                                                       | Apply the accepted non-SSH target-disposition table above; keep Agent, Worktree, Archive, Launch Profiles, and SSH-specific rows reserved; do not preserve compatibility wrappers without a released compatibility need                                                                                                                        |

The first reduction target is duplicated ownership, not package count. A small package with one
real owner and several consumers may remain; a large compatibility package with no surviving
semantic owner must not remain merely to avoid migration.

## Rust repository structure decision

Phase 1 resolves the repository gap before any Rust product runtime is created:

- the repository root owns one virtual Cargo workspace and one exact `rust-toolchain.toml`;
- the first real member creates the one tracked root `Cargo.lock`, and required Cargo work uses
  `--locked`;
- Rust executable crates live directly under `apps/*`; reusable Rust libraries with real consumers
  live directly under `crates/*`; `packages/*` remains the JavaScript and TypeScript library root;
- Cargo applications participate in the root gate directly and do not add `package.json`,
  placeholder TypeScript, or duplicated per-application Cargo orchestration;
- `project.json.runtime` remains `native` because it describes execution, while `Cargo.toml`
  authoritatively identifies Rust;
- [`docs/RUST.md`](../../docs/RUST.md) owns toolchain, dependency, module, safety, concurrency,
  testing, release-input, and cross-language conformance rules; and
- application crates may depend on declared reusable crates but never on another application's
  source or Cargo target.

Every serialized cross-language slice must still name its exact language-neutral schema or encoding
authority and canonical fixtures before implementation. Phase 1 established that rule without
selecting a wire schema. Delivered later slices now select the read-only Workbench version 1
projection, the OBCS version 1 Client-Server terminal-control schema and corpus, and the OBCN
version 1 Server-Worker terminal-authorization encoding. These additive contracts and bindings do
not migrate a current product consumer by themselves.

## Delivered native foundations

The following foundations are additive, unshipped, and have no production consumer unless the
owning guide says otherwise. Current TypeScript, Electron, Node, and Zig runtime behavior remains
authoritative until an accepted migration switches a complete consumer path.

| Foundation                      | Delivered boundary and current owner                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust repository contract        | The root Cargo workspace, pinned toolchain, Rust standards, project-contract support, and root verification are current repository infrastructure owned by [`docs/RUST.md`](../../docs/RUST.md) and the root build files.                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Workbench projection contract   | JSON Schema version 1 plus independent TypeScript and Rust bindings provide bounded read-only snapshots and atomic deltas. The schemas, package guide, crate contract, and conformance corpus own the maintained facts; no current writer or consumer has migrated.                                                                                                                                                                                                                                                                                                                                                                                                             |
| Client-Server terminal contract | [`schemas/client-server`](../../schemas/client-server/README.md) and [`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.md) own the strict bounded OBCS version 1 terminal Create/Start intent and result contract, including its cross-language corpus and at-most-once producer obligations. The private Server consumes it only for the deny-only Session phase; no Client runtime consumes it yet.                                                                                                                                                                                                                                               |
| Server-Worker launch contract   | [`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) owns the bounded OBCN version 1 authorization and result encoding, while Server's checked launch-authorization model and Worker's unintegrated private kernel own admission, fencing, exact replay, handoff, and result behavior under the documented Server-producer invariant. No carrier or Host command adapter consumes the kernel yet; terminal retirement and binding capacity must be accepted before that integration.                                                                                                                                                           |
| Native `orchbee` entry          | [`apps/orchbee`](../../apps/orchbee/README.md) owns the unshipped empty, `--help`, and `--version` foundation and its reference startup measurements. Bare invocation still reports that the TUI is unavailable.                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Rust Server kernel and service  | [`apps/orchbee-server`](../../apps/orchbee-server/README.md) owns bounded Workbench reads, failure-atomic in-memory projection publication selection, and separate unshipped private same-user local Worker and Human Client services. The Worker service admits one compatible Worker; the Client service admits up to eight independent compatible Client Sessions with bounded candidate handling. Both own nonzero generation-scoped liveness and cleanup. The Client service sequentially denies strict post-admission OBCS terminal intents without an effect; Server has no successful terminal processing, activation, persistence, packaging, or production lifecycle. |
| Terminal Client authority model | The Server-owned [`LocalTerminalClientAuthority`](../../apps/orchbee-server/formal/tla/LocalTerminalClientAuthority.tla) refinement model checks peer Client Sessions, multiple viewers, one combined input-and-resize controller per terminal, generation-scoped cleanup, and independently ordered Client, Server, Worker, Host, and terminal loss. It records the accepted target authority but does not add runtime messages or implement attach, control, takeover, or automatic controller promotion.                                                                                                                                                                     |
| Rust Worker process spine       | [`apps/orchbee-worker`](../../apps/orchbee-worker/README.md), [`orchbee-local-site-protocol`](../../crates/local-site-protocol/README.md), and [`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) own the unshipped Server registration plus normal Worker-to-Host broker connection, bounded private framing, POSIX peer proof on both links, exact Host handshake, retained admission identities and limits, coupled connection liveness, and the unintegrated authorization kernel described above. They perform no terminal effect and have no production consumer.                                                                          |
| Rust Terminal Host binding      | [`crates/terminal-protocol/project.json`](../../crates/terminal-protocol/project.json) and the package-owned [Terminal Protocol reference](../../packages/terminal-protocol/references/protocol.md) own the independent Rust envelope and payload bindings completed through terminal-configuration kinds 70 through 73. Worker consumes only the envelope and connection-control subset for its unshipped Host handshake; additional message codecs are admitted only by a concrete vertical consumer.                                                                                                                                                                         |

Historical slice-by-slice authorizations and command results are not maintained here. Current
behavior belongs to the linked owners and executable evidence; this proposal retains only
unresolved target design and the next accepted milestone.

## Accepted local-terminal vertical milestone

The product owner accepted this consequential milestone on 2026-09-02 and delegated its
dependency-ordered implementation slices after accepting the observable behavior, authority
boundaries, exclusions, evidence, and rollback below. A slice must return for review only when it
discovers a materially different product outcome, owner, public contract, risk, or exclusion.

| Field                   | Accepted decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem                 | The native entry, Server kernel, and Rust Host binding are individually verified but do not form a runnable product path. Continuing message-by-message parity would increase unused protocol surface while leaving the one-command experience, Worker boundary, process lifetime, real terminal rendering, and IPC performance untested.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Outcome                 | On one supported local machine, bare `orchbee` discovers or lazily activates the exact compatible private Server and local Worker cohort, enters a real TUI Client Session, and can create or attach one default-interactive-shell terminal whose actual output, input, and resize flow through the retained Zig Terminal Host. Closing one Client never implicitly terminates the Site, private services, or a retained terminal. A later invocation joins the same Site as another independent Client Session.                                                                                                                                                                                                                                                                                                                            |
| Scope                   | Deliver the private Rust Server service boundary, one private Rust local Worker, existing Zig Host discovery or activation, bounded same-user local IPC, minimal default-shell terminal intent with optional node-local `cwd`, Host launch mapping, create/attach/output/input/resize flow, a real single-terminal TUI surface, bounded relay queues, version-cohort rejection, and the process/fault/performance evidence required for that path.                                                                                                                                                                                                                                                                                                                                                                                          |
| Client model            | TUI, Web, Desktop, mobile, separate browser tabs, separate Desktop windows, and repeated `orchbee` invocations are peers under one Client model. Each active instance has its own Client Session and local focus, active location, viewport, scroll, and presentation state. Shared Workspace, Tab, Panel, and Terminal facts come from Server. Terminal Host sees typed attachments and capabilities, never renderer identity.                                                                                                                                                                                                                                                                                                                                                                                                             |
| Terminal control        | Different Clients may control different terminals. Several Client Sessions may view one terminal through separate attachments, but the milestone admits only one active input-and-resize controller for that terminal. Another attachment remains non-controlling; explicit takeover and collaborative input remain later decisions.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Activation and lifetime | Bare `orchbee` may initiate bounded lazy activation of the exact installed private cohort and wait for readiness, but it does not remain the Server or Worker's lifetime supervisor after readiness. `--help` and `--version` remain side-effect free. Client exit releases only that Client Session and its attachments. Exact daemonization or platform service mechanics are delegated implementation choices that must preserve singleton, version, cleanup, and failure claims.                                                                                                                                                                                                                                                                                                                                                        |
| Launch boundary         | Client intent never supplies authoritative Worker placement, lease, generation, fence, executable, arbitrary arguments, Environment, Project, Checkout, Worktree, or Agent identity. Server selects the one local Worker; Worker resolves the accepted default interactive shell and omitted local defaults; Host receives only its exact launch facts. The first milestone sends an empty transient environment overlay.                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Data path               | The first complete path is `Terminal Host <-> Worker <-> Server <-> Client`. Worker and Server validate routing, identity, role, size, queue, and authorization facts but do not emulate VT or reinterpret frame content. A direct local Client-to-Worker attachment is not part of this milestone and requires measured need plus a separately accepted scoped-capability design.                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Trust                   | The milestone exposes private same-operating-system-user local endpoints only. It adds no LAN or Internet listener, Cookie-to-device credential conversion, ambient direct Host access, or multi-user claim. Wrong peer role, stale identity, malformed data, and incompatible versions fail closed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Claims                  | One local Site has at most one active Server authority and one admitted local Worker identity. Help/version start no service. Mutating intent is submitted at most once and unknown terminal input is never replayed. A slow Client cannot block Worker progress or another Client. Client disconnect does not imply terminal termination. Server never becomes a VT/history authority, Worker never becomes a generic command/filesystem service, and the baseline terminal requires no Project, repository, Worktree, Environment, or Agent state.                                                                                                                                                                                                                                                                                        |
| Non-goals               | No complete Workbench UI, additional public CLI grammar, selection/export/history/theme/clear/reset feature parity, direct Worker data path, final collaborative-input or takeover UX, Agent helper, managed Worktree, Archive, Launch Profile, SSH redesign, remote Worker, Web/mobile migration, Gateway/SSH/Workbench/old-CLI deletion, production TLS, multi-user tenancy, durable Server recovery, operating-system-restart PTY recovery, login-time service installation, package-manager release, public cutover, or compatibility promise is admitted.                                                                                                                                                                                                                                                                              |
| Evidence                | Model singleton activation, independently ordered Client/Server/Worker/Host loss, controller ownership, bounded queues, and unknown input outcome where the existing topology model is insufficient. Add process tests for absent/present/incompatible cohorts, two simultaneous Client Sessions, Client exit with retained terminal, malformed peers, Host loss, and bounded shutdown. Add an end-to-end real PTY trace for create, attach, frame, input, resize, detach, reattach, and exit. Measure warm/cold activation, input latency, frame throughput, copies, queue memory, and slow-client isolation before proposing a direct path or numerical release budget. The applicable focused checks and root `pnpm verify` remain required; the manual focus-stealing native-window test remains excluded unless explicitly authorized. |
| Delivery and rollback   | First converge the three owning proposals. Then deliver a no-terminal-effect process spine, the minimal launch/Host broker, the relayed terminal stream, the TUI adapter, and finally loss/performance convergence. Each slice stays additive and unshipped, has no durable migration, and leaves current Desktop behavior authoritative. Rollback removes the new unused slice or returns to the last verified additive spine; it never deletes or switches an existing consumer.                                                                                                                                                                                                                                                                                                                                                          |

## Delivery and migration sequence

| Milestone                  | Independently reviewable outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Native foundations         | **Complete and unshipped:** repository/Cargo support, read-only Workbench projection contracts, the native entry foundation, the in-memory Server projection kernel, and the independent Rust Terminal Host binding. Their owning guides and executable tests maintain current facts.                                                                                                                                                                                                                                                                                                            |
| Local terminal vertical    | **In progress and unshipped:** the real local Server-Worker and Client Session spines, deny-only Server OBCS phase, checked terminal Client authority model, strict OBCS terminal-control contract, OBCN authorization contract, Worker authorization kernel, and Worker-Host broker connection, handshake, and coupled-loss boundary are complete. Next define Worker binding retirement/capacity, connect authorized Server processing to the OBCN carrier and Host command mapping, and then deliver the relayed terminal stream, real TUI adapter, and remaining fault/performance evidence. |
| Shared Web/Desktop Client  | After the local authority path exists, move React Workbench presentation into `apps/web`, migrate shared semantic transitions to Server, retain Desktop-only platform effects, and delete `@orchbee/workbench` only after its complete responsibility audit.                                                                                                                                                                                                                                                                                                                                     |
| Browser ingress retirement | Move supported Browser session, terminal relay, and backpressure behavior into Server, migrate Web/Desktop consumers, then delete `apps/gateway`, `@orchbee/gateway-protocol`, and superseded Desktop composition as one reversible cohort.                                                                                                                                                                                                                                                                                                                                                      |
| Optional capabilities      | Review Agent, managed Worktree, Archive, Launch Profiles, SSH-specific Desktop behavior, and any future unmanaged-SSH design under their own owners; none is required for the terminal baseline.                                                                                                                                                                                                                                                                                                                                                                                                 |
| Distributed operation      | Add enrolled remote Workers, mobile or remote Clients, production authentication, recovery, compatibility, and optimized direct attachments one accepted capability at a time.                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Legacy retirement          | Delete the current SSH Runtime/protocol, old CLI, Gateway, Workbench package, and reduced Desktop authorities only after each accepted replacement or sunset slice proves consumer, artifact, migration, and rollback coverage.                                                                                                                                                                                                                                                                                                                                                                  |

Rollback preserves one canonical writer. Every local-terminal slice is additive and unshipped,
so rollback removes only its unused artifacts or returns to the last verified process spine.
Later consumer or durable-schema migrations require their own cohort rollback and may never start
an old writer against unknown new state. Compatible retained Hosts are not replaced while they
own live terminals.

## Correctness claims and evidence plan

| Claim                                                                                                      | Required evidence before completion                                                                                                                              |
| ---------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| One mutable fact has one authority despite colocation and reconnect                                        | Ownership assertions, project contracts, deterministic transition tests, and a cross-process topology model                                                      |
| A shared semantic command commits at most once and is published from canonical state                       | Server state-machine tests, property tests, persistence fault injection, and modeled commit/publication ordering                                                 |
| Placement and fencing never route an operation or attachment to the wrong Worker or Host                   | Identity/epoch property tests, TLA+ lease and reconnect model, malformed-message tests, and multi-node deterministic traces                                      |
| Client, Server, or Worker disconnect does not imply terminal termination                                   | Host retention tests and system traces covering each independent process loss                                                                                    |
| Unknown input admission is never replayed                                                                  | Protocol tests, disconnect fault injection, and end-to-end input traces                                                                                          |
| When selected, a local data-path capability grants no more authority than Server authorized                | Capability decision tests, expiry/revocation races, peer-identity integration tests, and security review                                                         |
| A slow Client cannot block another Client, Worker heartbeat, or terminal progress                          | Bounded queue tests, adversarial backpressure traces, and measured relay limits                                                                                  |
| Rust, TypeScript, and Zig implementations agree on every shared wire contract                              | Canonical fixtures, generated-file stale checks where applicable, cross-language conformance, and differential tests                                             |
| Migration preserves accepted current behavior until an explicit replacement decision                       | Old/new differential scenarios, durable-data migration/rollback tests, and feature-specific acceptance evidence                                                  |
| Terminal creation requires no Project, repository, Checkout, Worktree, Environment, or Agent capability    | Terminal-only cohort tests for omitted `cwd`, an ordinary non-Git directory, a repository directory, and an optional upstream-generated path                     |
| Client cannot authoritatively supply Worker lease, generation, or fence                                    | Schema rejection, Server binding tests, stale-fence property tests, and reconnect traces                                                                         |
| Split and child terminals preserve accepted current CWD without crossing Workers                           | Same-Worker inheritance tests, absent-observation fallback tests, and two-Worker rejection or separately accepted translation traces                             |
| Transient environment overlays never become durable intent or replayable recovery data                     | Persistence redaction tests, Host digest replay tests, capability-expiry tests, and lost-delivery traces                                                         |
| Immediate and deferred creation preserve the Host geometry/start contract                                  | Complete-geometry, awaiting-start, exact-replay, conflicting-start, and no-fallback tests                                                                        |
| Worker exposes no implicit generic Files, Git, filesystem, profile, or remote-command authority            | Capability allowlist tests, project non-goals, protocol schema review, and negative integration tests                                                            |
| Retiring the current SSH Runtime and protocol neither strands a supported route nor trusts stale artifacts | Current-consumer and packaged-artifact inventory, migration or sunset evidence, uninstall/ignore policy, cutover tests, and rollback exercise                    |
| Every Client receives the same authorized Workbench semantics without sharing a renderer                   | Cross-language snapshot/delta fixtures, Server projection tests, Web/TUI/mobile conformance, CLI structured-output tests, and client-local-state exclusion tests |
| Removing Gateway preserves Browser admission, relay isolation, and backpressure                            | Old/new Browser session and terminal-relay differential tests, slow-client tests, consumer scan, packaged-process inventory, and rollback exercise               |
| Desktop retains no second Server, Worker, or terminal authority after migration                            | Module dependency scan, process-loss traces, authority-negative tests, project-contract updates, and removal of superseded composition roots                     |

The process/session/lease/reconnect model selects TLA+ because independently ordered Client,
Server, Worker, Host, transport, and persistence events can violate authority, continuity, or
at-most-once claims. Property tests cover identifiers, stale generations, topology cardinality,
and serializable transitions. Deterministic system tests cover at least one all-local Site and one
Site with two Workers and three concurrent Client forms. Performance evidence must measure input
latency, frame copies, relay memory, slow-client isolation, and startup cost; process-hop counts
alone are not performance evidence. Before the Rust CLI delivery slice is accepted, its evidence
plan must name the measured command and completion point, cold and warm conditions, artifact-size
scope, peak-memory metric, reference platforms, baselines, and acceptance-budget owner. Numerical
budgets may follow a bounded prototype, but the measurement contract may not be deferred until after
implementation.

## Accepted decisions from product review

1. Terminal launch is independent of Project, repository, Checkout, Worktree, and generic
   Environment identity. Future native placement resolves directly to one Server-authorized Worker;
   current SSH Environment behavior remains unchanged until an accepted migration.
2. One public Rust `orchbee` executable owns both CLI and TUI Client surfaces. The current
   TypeScript `apps/cli` is replaced and deleted after cutover; its source, commands, parameters,
   output, exit meanings, and Desktop-specific launcher contract are not compatibility targets.
3. Terminal-only cohorts do not require Agent Bridge, Agent orchestration, or managed Worktree.
4. The new launch boundary preserves current Host geometry/start rules, non-persistent overlay
   semantics, and same-Worker split/child `currentCwd` inheritance unless a later explicit product
   decision replaces an observable behavior.
5. The current `apps/ssh-environment-runtime` and `@orchbee/ssh-environment-protocol` are excluded
   from the target architecture and are deleted together after an accepted migration or product
   sunset. Any future unmanaged-SSH capability is a new design and has no source, executable,
   package-name, wire, service, release-cohort, or Desktop-composition compatibility obligation.
6. `apps/gateway` and `@orchbee/gateway-protocol` are deleted together after Server assumes the
   supported Browser-session, terminal-relay, and backpressure behavior and consumers migrate.
   Their process split and framing are not compatibility targets.
7. The current mixed `@orchbee/workbench` browser package is decomposed and deleted. React UI,
   browser-local interaction state, layout persistence, styles, and Stories move to `apps/web`;
   Server owns canonical Workbench transitions; framework-neutral semantic-intent, snapshot, and
   delta contracts use a cross-language authority. TUI and mobile own separate renderers, and CLI
   may return a bounded versioned semantic projection snapshot rather than Web UI state.
8. The non-SSH Desktop Main target-disposition table is accepted. Agent, managed Worktree, Terminal
   Archive, Launch Profiles, and SSH-specific Desktop areas remain reserved and are not decided by
   neighboring migrations.
9. The target baseline uses four separate executables: public Rust `orchbee`, private Rust
   `orchbee-server`, private Rust `orchbee-worker`, and private Zig `orchbee-terminal-host`.
   Colocation does not merge their roles or lifecycles. No Agent helper executable is accepted.
10. The Phase 1 Rust repository decision is accepted: one pinned root Cargo workspace, executable
    crates under `apps/*`, reusable Rust libraries under `crates/*`, direct root-gate integration,
    `native` as the runtime classification, and `Cargo.toml` as the implementation-language authority.
11. The first additive Phase 2 slice uses strict JSON Schema 2020-12 under
    `schemas/workbench/v1` as the language-neutral authority. It admits only a read-only snapshot
    intent, complete bounded projection snapshots, atomic revision-chained aggregate deltas, and
    independent TypeScript/Rust bindings with one canonical conformance corpus. It changes no
    current writer, consumer, transport, persistence, or runtime behavior.
12. The additive Phase 3A slice creates an unshipped Rust `apps/orchbee` application and admits
    only exact empty, `--help`, and `--version` dispatch. Empty dispatch reports the explicit
    development-unavailable TUI state; authored help and diagnostics ship in `en-US` and `zh-CN`;
    no Server, Worker, Host, TUI, business command, packaging, current CLI migration, or deletion is
    admitted. Its release-profile measurements are a reference baseline, not acceptance budgets.
13. TUI, Web, Desktop, mobile, browser tabs, Desktop windows, and repeated native invocations use
    one Client/Client Session model. Renderer form never changes authority; shared product state
    comes from Server and active location, focus, viewport, scroll, and presentation remain local to
    each Client Session.
14. The first local-terminal path relays through Server. Bare `orchbee` may lazily activate the
    exact private Server and local Worker cohort without supervising their lifetime, while help and
    version remain side-effect free. Client exit does not stop the Site or a retained terminal.
15. During the accepted local milestone, several attachments may view one terminal but at most one
    active controller holds input and resize authority. Direct Worker attachment, takeover, and
    collaborative input require later acceptance.
16. Rust Terminal Host protocol work is consumer-driven after the completed foundation. Message
    codecs that are not required by the next vertical runtime slice do not advance independently.
17. OBCS version 1 is the strict language-neutral Client-Server terminal Create/Start contract.
    Client request identities are producer-owned correlation values, mutating intent is submitted
    at most once, transport ambiguity is reconciled through a later read-only boundary, and Server
    maintains no protocol-required replay ledger. The Rust binding and bidirectional numeric corpus
    are delivered; the private Server consumes them only to construct and attempt to write
    sequential correlated `rejected/not-authorized` results without an effect, and no Client
    runtime consumes them yet.
18. OBCN version 1 and `TerminalLaunchAuthorization` own the Server-Worker authorization contract.
    Worker's private deterministic kernel implements its admission, fencing, exact cached replay,
    pre-effect handoff, and typed result behavior under the active Server authority's global-fresh
    `OperationId` producer invariant. It remains unintegrated; terminal retirement and binding
    capacity must be accepted before a carrier may invoke it.

## Reserved decisions for review

1. Decide whether Agent integration needs a hook helper and, if so, its executable, language,
   lifetime, installation cohort, failure contract, Worker relationship, and association model.
2. Define public commands beyond accepted bare TUI, `--help`, and `--version`: grammar, stable
   output and exit contracts, automation, completion, compatibility, and migration.
3. Decide ownership and behavior for managed Worktree, Archive, Launch Profiles, and every
   SSH-specific Desktop area. None is implied by the terminal baseline.
4. Admit a direct local Client-to-Worker data path only after relay measurements justify it and
   its scoped capability, expiry, revocation, reconnect, accounting, and fallback are accepted.
5. Define explicit controller takeover, separate input and resize ownership, or collaborative
   input beyond the accepted single-controller milestone policy.
6. Define the product scope, timing, transport, authentication, installation, and removal model
   of any future unmanaged-SSH capability independently from the retiring implementation.
7. Define production service installation, login-time start, update cohort, compatibility,
   durable Server recovery, backup, rollback, and uninstall beyond accepted lazy local
   activation and Client-independent lifetime.
8. Define later Workbench schemas, broader mutating intents, persistence and migration,
   authorization and redaction, and remaining projection domains beyond the local-terminal
   milestone.
9. Decide Desktop's long-term shell technology separately from its Client-only authority.
10. Define remote Worker enrollment, production human and device sessions, mobile backgrounding,
    TLS, discovery, Internet reachability, multi-user tenancy, and compatibility.
11. Set supported-platform startup, artifact-size, latency, throughput, queue-memory, and
    peak-memory budgets after the accepted milestone produces measurements; define package
    manager distribution, public cutover, and exact `apps/cli` deletion at release time.
12. Accept each Gateway, SSH Runtime/protocol, Workbench package, old CLI, and Desktop authority
    deletion only after its own consumer, installed-artifact, behavior, migration or sunset,
    documentation, evidence, and rollback audit.

## Review checklist

- Does every product, node, terminal, and Client-local fact have one convincing owner?
- Can one Server coordinate a local Worker and remote Workers without a special local business path?
- Can Server, Worker, Desktop, and TUI restart independently without fabricating terminal continuity?
- Is Worker still narrow enough to avoid becoming a second general backend?
- Do Client intent, Server authorization, Worker admission, and Host launch facts remain distinct?
- Can a terminal start with an omitted `cwd` or in an ordinary non-Git directory without Project,
  Checkout, Worktree, Environment, or Agent state?
- Do split and child terminals inherit an accepted `currentCwd` only on the source Worker?
- Are transient environment overlays absent from canonical intent, persistence, diagnostics, and
  replay?
- Does managed Worktree remain an optional path producer rather than a Terminal dependency?
- Does the first Server-relayed terminal path meet measured latency, throughput, queue-memory, and
  slow-client-isolation needs, and is any later direct path justified by evidence?
- Can mobile and remote Clients use Server relay without making Server a VT or history owner?
- Which current Desktop responsibilities remain genuinely platform-local?
- Does the accepted Desktop table leave any non-reserved Main module without exactly one target
  owner or deletion outcome?
- Which workspaces have a durable consumer after migration, and what exact evidence permits each
  proposed deletion?
- Can Rust become authoritative without maintaining competing TypeScript domain transitions?
- Can the new `orchbee` replace and delete `apps/cli` without accidentally preserving its old
  grammar or losing a deliberately selected semantic capability?
- Can the current SSH Runtime and protocol be removed as one complete legacy path without implying
  that either their implementation contracts or a generic Environment domain survive?
- Can Web, TUI, mobile, and CLI consume one semantic Workbench projection contract without sharing
  React, DOM, pixels, or Client-local focus state?
- Are migration, compatibility, rollback, and evidence obligations small enough to deliver in
  independently reviewable slices?
