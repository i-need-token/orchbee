# Native Runtime Topology and Responsibility Migration

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Accepted direction. Native Rust CLI/TUI and the local Server/Worker/Terminal Host chain are the current development authority. Desktop/Web are frozen while they migrate to Server Clients under the accepted [migration plan](#web-and-desktop-client-migration-plan). Remote enrollment, release packaging, installation/update/recovery, and cross-platform acceptance remain governed by their owning slices.                                                                                                                                                                                                                                                                                                                              |
| Owner              | Product architecture and the maintainers of Client, Server, Desktop, Worker, Terminal Host, and Agent integration boundaries                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Current authority  | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md), the owning application and package guides, and the accepted logical topology in [`connected-client-capability-service.md`](connected-client-capability-service.md)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Decision authority | The product owner has accepted the Client-Server-Worker topology; the public Rust Client and four private executable roles used by an Agent-capable installation; the delivered native foundations; code-agnostic launch and 1,024-binding Worker limit; uniform peer Clients and transient terminal attachments; the first relayed path and measured local-direct fallback; Agent observation, managed Worktree, Archive, Launch Profile, SSH-connection, service-lifecycle, cohort-update, and Server-recovery ownership below; and the native CLI, Workbench-package, and Desktop dispositions. Remote enrollment, production human/device identity, TLS/Internet ingress, multi-user tenancy, and mobile background policy remain reserved |
| Exit condition     | Accept bounded delivery slices, implement them with mapped evidence, move maintained facts into current authorities, resolve every workspace disposition, and delete this proposal after it no longer owns unresolved future-design evidence                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |

## Decision boundary

This proposal refines the parent proposal's physical process topology and explicitly narrows its
logical placement model for native terminal launch. The product runtime does not yet have this
target shape. Completed additive foundations plus the checked Client-side local Session activation
model, local Server-Worker, Client-Server, and Worker-Host connection spines, the integrated Server
OBCS terminal authorization, the terminal-control contracts, the integrated Worker authorization/default-launch/Host
execution loop, the application-private local Client transport and shared local-Session runtime, and the
checked terminal Client authority model are summarized with their current owners below. The local
vertical is integrated on Windows, Linux, and the unsigned Apple Silicon macOS development package,
with platform differences owned by the
[TUI platform boundary](../../apps/orchbee/docs/tui-platforms.md); every other runtime
implementation, migration, cutover, and deletion remains gated by the boundaries and reserved
decisions below.

The accepted direction is:

- one logical Client-Server-Worker topology for local and distributed operation;
- a mandatory native Rust `orchbee` entry containing both CLI and TUI surfaces;
- separate private Rust `orchbee-server`, Rust `orchbee-worker`, Zig
  `orchbee-terminal-host`, and optional Rust `orchbee-agent-bridge` executable roles; ordinary
  terminal creation does not require or start the Bridge;
- optional Web, Desktop, and mobile Client surfaces that consume the same Session authority;
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
delivered SSH Environment records and behavior remain current authority until the accepted
`SshTarget`/`SshConnection` migration removes that generic identity from the replacement path.

The product owner explicitly sunset the old POSIX SSH Environment routes and authorized deleting `apps/ssh-environment-runtime` and `@orchbee/ssh-environment-protocol` with their Desktop consumers. The native Worker and separately accepted future SSH design inherit no compatibility obligation to that implementation. Repository retirement does not uninstall or modify previously deployed user artifacts.

The product owner authorized retirement of the Desktop-bundled TypeScript `apps/cli`, local semantic endpoint, launcher, and injected variables. Native Rust CLI/TUI development remains authoritative under the [native application guide](../../apps/orchbee/README.md); future command design remains under [`native-command-surface.md`](../cli/native-command-surface.md). A newly packaged release is not a prerequisite for this explicit legacy sunset.

The product owner sunset the old Electron Browser-access chain and authorized the coupled deletion of `apps/server`, `apps/gateway`, and `@orchbee/gateway-protocol`. Browser authentication, relay, and backpressure remain future native Server capabilities; this retirement introduces no replacement Browser service.

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

The Desktop Main dispositions below are accepted target boundaries, including the now-explicit
Agent, managed Worktree, Terminal Archive, Launch Profile, and SSH connection ownership. Each is a
future migration contract, not evidence that the current Desktop implementation has already moved.

The accepted topology and Phase 1 repository foundation do not imply that the newly accepted
command, Agent, SSH, persistence, direct-data-path, or deletion targets are delivered. Each affected
current path remains authoritative until a bounded implementation or migration slice supplies its
model, evidence, cutover, and rollback boundary.

## Problem

The current feature-development path uses native `apps/orchbee`, `apps/orchbee-server`, `apps/orchbee-worker`, and Terminal Host. The retained local Electron composition still owns its own Workbench coordinator, local Host brokering, Agent observation, and platform effects; it is not a production Browser or POSIX SSH ingress. Native release packaging and future remote-client capabilities remain separate work.

This shape is insufficient for a product in which:

- CLI and TUI are mandatory on every full local installation;
- concurrent TUI, Web, Desktop, and mobile Client instances may observe the same Session;
- one Session may coordinate local and remote execution nodes;
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

One Session has exactly one active canonical Server authority, zero or more Workers, and zero or more
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
   `- optional Agent observation ingress <--- orchbee-agent-bridge <--- provider hook/plugin
```

Terminal Host is a specialized process inside the physical Worker-node boundary and does not create
an additional logical role. It belongs to the terminal baseline. Agent Bridge is an optional
one-shot ingress helper, not a long-running logical authority or a terminal dependency.

## Native executable set

| Executable              | Language | Visibility | Lifetime                        | Accepted responsibility                                                                                                      |
| ----------------------- | -------- | ---------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `orchbee`               | Rust     | Public     | One-shot CLI or interactive TUI | Human command entry, TUI Client, local Session discovery, and explicit lifecycle commands                                    |
| `orchbee-server`        | Rust     | Private    | Long-running Session service    | Server authority, Client sessions, canonical product state, placement, optional Agent orchestration, and remote Client relay |
| `orchbee-worker`        | Rust     | Private    | Long-running per execution node | Node identity, scoped code-agnostic launch, Host brokering, and optional Agent runtime effects                               |
| `orchbee-terminal-host` | Zig      | Private    | On-demand retained node service | PTY, process tree, canonical Ghostty VT, terminal history, input, and attachment authority                                   |
| `orchbee-agent-bridge`  | Rust     | Private    | One bounded provider callback   | Sanitize one provider observation, deliver it to the local Worker, and emit private correlation only after exact acceptance  |

The first four executable roles form the terminal-capable baseline. A full Agent-capable
distribution also recognizes `orchbee-agent-bridge`; it is installed as a versioned Capsule asset
and is not resident when unused.

The proposed installed layout keeps one user-facing name:

```text
bin/
`- orchbee

libexec/
|- orchbee-server
|- orchbee-worker
|- orchbee-terminal-host
`- orchbee-agent-bridge (Agent-capable cohort only)
```

Installation cohorts do not create protocol variants:

| Cohort                              | Required executables                                            |
| ----------------------------------- | --------------------------------------------------------------- |
| Native Client only                  | `orchbee`                                                       |
| Local terminal Session              | `orchbee`, Server, Worker, and Terminal Host                    |
| Terminal-only remote execution node | Worker and Terminal Host                                        |
| Agent-enabled execution node        | Worker, Terminal Host when terminals are used, and Agent Bridge |
| Browser or mobile Client            | No local native executable; it connects to an authorized Server |

Exact platform installation paths remain packaging details. Agent integration is never a
prerequisite for ordinary terminal creation.

## Role responsibilities

### Client

A Client renders one authorized projection and submits semantic intent. It owns active location,
focus, terminal viewport, search presentation, selection presentation, window geometry, and platform
clipboard effects for its exact Client Session. It owns no canonical Session state, Worker lease, PTY,
or terminal lifecycle.

Every active TUI invocation, browser tab, and independently active Desktop or mobile surface is one
Client Session. Renderer type grants no authority and creates no separate product model. Client
Sessions may select different active locations and view state while consuming the same authorized
shared Workspace, Tab, Panel, and Terminal facts.

The shared Workbench document owns Project, Workspace, terminal-context, Tab, Panel, and
Presentation membership and ordering. Each Client View owns navigation, active Tab, focus, layout
projection, viewport, scroll, search, and selection. Bare TUI opens its standalone terminal context
and first ordered Tab; an empty context may create its first terminal exactly once through one
Server-coordinated operation. Closing the final Tab leaves an explicit empty context. A Client
switches only its visible display attachment; the PTY continues independently.

`orchbee` contains both CLI and TUI Client surfaces. A CLI invocation is transient; a TUI owns a
longer Client Session. The CLI path must not initialize TUI rendering or load Server, Worker, or Host
engines. The single-entry interaction direction is accepted; the exact command grammar and
parameter contract are owned by `native-command-surface.md`. Browser,
Desktop, TUI, mobile, and CLI may consume the same versioned semantic projection snapshot, while
each interactive Client owns its own renderer and Client-local view state.

### Server

Server is the unique Session control plane. It owns:

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

Worker represents one execution node to one Session. It owns:

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

Terminal Host owns no Session, Client Session, Project, Workspace, Tab, Panel, Worker placement, Agent
semantics, remote authentication, network listener, or update policy. A Server or Client restart
does not own its lifetime. A Host or operating-system failure does not promise PTY recovery.

### Agent integration helper

`orchbee-agent-bridge` is a minimal synchronous Rust process invoked by an explicitly installed
provider hook, plugin, or adapter. The versioned Agent Capsule is its release source. Each invocation
accepts one byte-bounded input, allowlist-sanitizes the observation, resolves only the exact local
Worker endpoint and invocation context, attempts one delivery under a strict end-to-end deadline,
and retains no durable state.

The Bridge does not discover a remote Server, use the network, activate a service, read product
state, or expose a public CLI. It emits no stdout or stderr. Worker absence, timeout, malformed input,
or rejection exits zero and leaves the provider operation unchanged. Only an exact Worker ACK allows
the Bridge to emit the private APC correlation half to the resolved terminal. Worker performs the
bounded rich-event/correlation join with Terminal and Presentation generations, deduplication,
expiry, and fencing; Terminal Host only transports the private correlation and owns no Agent
semantics. The Agent proposal owns provider installation and the canonical observation model.

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
Session, Client Session, Workspace, Project, Checkout, Worktree, Environment, Worker-lease, or
placement semantics.

Without an explicit Launch Profile, the terminal baseline launches the Worker's accepted default
interactive shell: a valid absolute `$SHELL`, otherwise the operating-system account shell,
otherwise `/bin/sh`. It supplies an empty argument vector and does not force login mode. A named
Launch Profile is a separately authorized catalog selection described below; terminal-create
authority alone never grants raw shell text or an arbitrary argument vector.

The admitted Worker authorization kernel retains at most 1,024 terminal bindings per Worker
generation. A binding and its terminal result remain for that generation's lifetime so exact replay
and higher-fence behavior stay deterministic. Reaching capacity rejects only a previously unseen
binding identity; it never evicts, expires, or rewrites an existing result. An explicit retirement
contract is required before public production may depend on long-lived generation reuse.

### Starting-directory semantics

The starting `cwd` may be the user's home, a temporary directory, an ordinary non-Git directory, a
repository directory, or a generated Worktree. Product context resolves it before authorization:

| Creation context                  | Accepted starting directory                                                                                       |
| --------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Bare or standalone first terminal | The selected Worker's user home; the invoking shell's current directory is not inferred                           |
| Workspace terminal                | The Workspace's accepted Primary Directory on that exact Worker                                                   |
| New Tab in Current Directory      | The latest validated Host-observed `currentCwd` for the source terminal, pinned to the same Worker                |
| Explicit future CLI directory     | One explicitly named and authorized node-local path; this is not implied by running `orchbee` from that directory |

A generic create intent may carry the resulting explicit node-local path or omit it; omission asks
the selected Worker to resolve its user-home default. Worker validates the resolved launch facts and
Terminal Host starts the PTY. A shell may later change directories under normal operating-system
permissions without changing canonical product state.

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

Server owns managed-Worktree semantic intent and the canonical reference. An optional, explicitly
advertised `managed-worktree` capability on the exact Worker owns Git execution, serialization,
journaling, compensation, and recovery beneath a Worker-private managed root. Creation, discovery,
and adoption of an existing external Worktree form the first migration scope; fetch, move, prune,
and deletion remain later operations. Adoption never grants Orchbee ownership to delete the external
directory. Product-reference removal and `git worktree remove` remain distinct explicit actions.
There is no sixth executable, no inferred Worktree creation, and terminal creation remains fully
usable without this capability.

### Start geometry

Start mode is one closed choice:

- immediate creation carries one complete geometry through the authorized operation and
  `HostLaunch`;
- deferred creation carries no partial or fallback geometry, leaves Host in `awaiting-start`, and
  requires one later Server-authorized `StartTerminal` operation through the exact Worker with one
  complete geometry.

Host retains its existing idempotent exact-replay and conflicting-geometry behavior. No layer may
invent a timeout or 80x24 fallback. A full-screen TUI obtains one complete actual cell and pixel
geometry and chooses immediate creation. Deferred creation remains for other Client flows that do
not yet possess a real first geometry and requires a later complete Start operation.

### Environment-variable overlay

An environment-variable overlay is an invocation-scoped capability, not canonical Terminal intent
or durable recovery state. It is bounded and authorized independently, delivered only for the exact
Worker admission and Host creation generation, omitted from audit and diagnostics, and never replayed
after loss or expiry. Server may retain only non-secret outcome metadata; Host retains only the
target-normalized digest required by its current idempotency contract. A retry that still requires
overlay values obtains a fresh capability instead of reconstructing or persisting them.

The default terminal environment declares `TERM=xterm-256color`, `COLORTERM=truecolor`, and the
current Ghostty compatibility identity `TERM_PROGRAM=ghostty`. It does not inherit old CLI, Agent,
Project, or caller-transient product variables. Any later profile environment or secret injection is
a separate capability; Launch Profiles themselves contain neither environment values nor secrets.

## Why Server, Worker, and Terminal Host remain separate

- Server is location-independent Session authority, while Terminal Host must live beside the PTY.
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
| Optional Agent hook delivery                                                 | Exact Agent Bridge invocation                   | Worker and Server may observe one accepted bounded event; Bridge retains none      |

No colocation path may introduce a second writer. In particular, Server records terminal intent and
outcome but never a second VT; Worker records Host identity and execution observations but never a
second terminal engine; a Client may stage a UI preview but never optimistically commit shared
Workbench state.

## Accepted optional-capability ownership

These capabilities are accepted target boundaries but remain optional and independently delivered.
None may broaden the terminal-only Worker or introduce another canonical Workbench writer.

| Capability        | Accepted target contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Agent observation | An explicitly enabled provider hook/plugin invokes private Rust `orchbee-agent-bridge`. Bridge sends one sanitized bounded rich observation to the local Worker and emits a private APC correlation only after exact ACK. Worker joins rich event and correlation against Terminal and Presentation generations with expiry, deduplication, and fencing. Server owns canonical Agent Session, Turn, Attention, Outcome, and shared projection; Clients render it. Observation ships before launch/orchestration. Content such as prompts, responses, tools, and transcripts is absent by default.                                                                                                                                                                                                                                                                                           |
| Managed Worktree  | Server owns semantic intent and canonical reference. The exact Worker may advertise one optional `managed-worktree` capability that owns Git effects, a Worker-private managed root, serialization, journal, compensation, and recovery. Creation, discovery, and adoption migrate first; adopting an external Worktree conveys no deletion ownership. Terminal receives only an ordinary resulting `cwd`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Terminal Archive  | Terminal Host owns live VT and scrollback. On explicit Stop, Worker requests one final read-only platform-neutral terminal snapshot before Host disposal and atomically stores its content; Server owns metadata, indexing, and authorization. Capture failure does not block Stop but records archive unavailable. Detach creates no archive. Stop creates one archive. Close deletes the Presentation and its archive. A later Start creates a fresh terminal rather than appending. Natural process exit does not automatically create a permanent archive. Initial bounds are 5,000 rows or 10 MiB per Presentation and 250 MiB per Worker.                                                                                                                                                                                                                                             |
| Launch Profiles   | Server owns the canonical product catalog; each profile is Worker/platform scoped and revisioned. A profile contains a name plus either system shell with an explicit login choice or one absolute executable with literal arguments. It contains no cwd, appearance, environment, or secrets. Worker validates and executes the exact resolved revision. Creation records that revision and resolved launch so later edits cannot change an existing terminal or replay. Missing, stale, unauthorized, or failed profiles report explicit failure and never fall back. Use and management permissions are distinct.                                                                                                                                                                                                                                                                        |
| Connect via SSH   | The enrolled-Worker path and an SSH-attached temporary Worker remain distinct. Product copy says “Connect via SSH”; internal contracts use `SshTarget` and `SshConnection`, not generic Environment. Server owns connection intent and state; one selected local Worker uses system OpenSSH, local config/agent/known-hosts, and ephemeral AskPass to start the same `orchbee-worker` in temporary connection mode on the remote account, followed by its Host. Private keys never reach Server. The identity lasts only for that connection and is never promoted to enrollment implicitly. Initial interactive creation is from local TUI or Desktop; Web/mobile may use an established connection. Disconnect is distinct from terminal exit and reconciles only exact retained identity/generation. Ordinary manual `ssh` remains supported but is not the product connection contract. |

The current mixed `@orchbee/agent` TypeScript package remains current authority during migration,
but it is not the target canonical runtime. Move its deterministic state machine to Rust Server,
publish one cross-language schema, move Web presentation into `apps/web`, and delete the mixed
package only after behavioral and consumer evidence proves the replacement. Worker, under Server
authorization, owns node-local Agent integration inspection and mutation. No separate Agent service,
Worktree service, Archive service, Profile service, or SSH executable is added.

## Accepted production lifecycle direction

| Concern                     | Accepted target contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| --------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Local direct terminal path  | The first and fallback path remains `Host <-> Worker <-> Server relay <-> Client`. Only measured relay insufficiency may enable `Worker <-> local native TUI/Desktop Client`; Browser and mobile continue through Server and no Client ever connects directly to Host. Server mints a short-lived capability bound to Client Session, Terminal, Worker, Host epoch, controller generation, and permissions. Control stays on Server. Server-Worker authority loss stops input/resize and permits at most one explicitly bounded read-only drain. A failed direct path may obtain a newly authorized relay path, but unknown input is never replayed. One multiplexed Client-Worker connection per Client Session and Worker is permitted.                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Controller transfer         | One combined input-and-resize controller remains. An unowned terminal can be acquired explicitly. An occupied terminal sends a request to the current active owner. Observed disconnect releases only after a liveness grace; no viewer is auto-promoted. After a modeled timeout, an authorized requester may perform a second-confirmation forced take. Server commits one atomic controller generation and fence, rejecting every late operation from the prior generation. Viewer scroll, search, and selection remain independent. Collaborative input and split input/resize ownership are excluded.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| User services               | Lazy start remains default. `orchbee session enable` installs per-user LaunchAgent or systemd-user units for an always-available Web/mobile/remote Session; no root or system daemon is used and no supervisor executable is added. Server and Worker are separate units from one compatible cohort with bounded restart backoff; Host stays lazy. `session restart` preserves live Host terminals. `server stop` stops admission plus Server/Worker and explicitly reports any retained Host terminals; terminating all terminals is a separate destructive operation. Disable, uninstall, and upgrade do not delete data or terminate terminals by default.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Update and compatibility    | One signed manifest records every component and asset version/digest. Versions install side by side and switch through one atomic active pointer; failure rolls back the entire cohort. Compatibility is an exact tested set, not an open range. A live Host is never replaced. Compatible Server/Worker may update around retained old Hosts; an incompatible Host update remains pending until its terminals stop, with no first-version parallel takeover. Referenced old Agent Capsules remain until their integration is explicitly updated. Database migration snapshots first and may reject unsafe downgrade. Package-manager installations update through their manager; automatic installation is off by default.                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Server persistence/recovery | Before public release, Server uses one transactional database, with SQLite in WAL mode as the default design. It persists Session/Workbench revision; Project, Workspace, Tab, and Panel state; terminal intent, placement, lifecycle, Worker identity/enrollment and operation ledger; Profiles; Worktree references; Archive metadata; SSH target/connection intent; and required operation identities. It does not persist Client View, attachment/controller leases, terminal frames, PTY/live scrollback, transient environment/secrets, raw hook payloads, or full Agent observation history by default. Intent commits before external effects. Restart reconciles exact Worker/Host inventory and never replays create or input. Exact binding, generation, epoch, and Terminal identity may recover; unknown Host terminals enter `RecoveryRequired` quarantine and are never adopted, deleted, or replaced automatically. Corrupt or incompatible storage opens read-only recovery rather than resetting. Credentials stay in the operating-system credential store; Worker Archive content and Worktree journals keep their node-local recovery owners. |

## Process lifecycle and supervision

| Process               | Start policy                                                                  | Stop and restart consequence                                                                                         |
| --------------------- | ----------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `orchbee` CLI         | Per invocation                                                                | Ends only its request or observation                                                                                 |
| `orchbee` TUI         | Explicit human launch; may initiate bounded lazy local-Session activation     | Releases its Client Session and attachments; it does not stop Session or terminals                                   |
| Server                | Exact-cohort lazy local activation or an optional user-service policy         | Clients and Workers reconnect; retained node execution does not infer cancellation                                   |
| Worker                | Exact-cohort local activation or enrolled-node service policy                 | Host remains independently retained; replacement Worker reconciles Host inventory                                    |
| Terminal Host         | Worker starts or discovers the exact compatible version on first terminal use | May exit only under its accepted no-broker/no-retained-terminal policy; Host loss reports terminal loss              |
| Agent Bridge          | One explicitly installed provider callback                                    | Exits after one bounded local-Worker attempt; failure is silent and fail-open for the invoking Agent                 |
| Web or Desktop Client | Explicit human launch                                                         | Releases only that Client Session unless the human separately requests a shared product or Session lifecycle command |

Bare `orchbee` may initiate Server and local Worker activation and wait for bounded readiness. Once
ready, each private process owns its own lifetime; the initiating Client is not its supervisor and
Client exit is not a stop signal. Lazy start is the default. `orchbee session enable` may install
per-user LaunchAgent or systemd-user units for always-on access; it never installs a root/system
daemon. Exact unit files and platform adapters are implementation details constrained by the
accepted user-service lifecycle above.

## Deployment shapes

### Local terminal Session

```text
orchbee CLI -----.
                  +--> local Server --> local Worker --> Terminal Host
orchbee TUI -----'
```

This shape requires no Browser listener, Web renderer distribution, or Desktop shell. Server and
Worker use local IPC while preserving the same identities and protocol semantics used across nodes.

When Agent observation is explicitly enabled, its independent hook path is:

```text
provider hook/plugin --> orchbee-agent-bridge --> local Worker
```

Omitting Agent Bridge changes no terminal-launch contract.

### Web plus CLI and TUI

```text
orchbee CLI/TUI -- native local IPC --.
                                        +--> Server --> Worker --> Terminal Host
Browser -- loopback HTTP/WebSocket -----'
```

The Web shape adds renderer assets, Browser authentication, and Browser ingress to Server. It does
not add another canonical coordinator. The target has no separate Browser-ingress process.

### Desktop plus CLI and TUI

```text
orchbee CLI/TUI -- local IPC -----------.
                                        +--> same local Server --> Worker --> Host
Desktop shell / renderer -- local IPC --'
```

Desktop remains a Client shell and platform-effect adapter. It may reuse the Web renderer without
enabling Browser ingress. Closing the Desktop window does not mean stopping the Session or terminating
retained terminals.

### Distributed Session

```text
Machine A: Session
|- Server
|- optional local Worker
|  `- Terminal Host
`- optional local Clients

Machine B: execution node
|- Worker
|- Terminal Host
`- optional orchbee-agent-bridge (one-shot, only when invoked)

Machine C: client only
`- Desktop / TUI / Web / Mobile
```

The accepted Connect-via-SSH capability is a separate bootstrap, recovery, or temporary remote path.
The retired SSH Environment Runtime and protocol are not members of the target executable set, and
the new temporary Worker connection is not silently reclassified as an enrolled Worker or a generic
`Environment` domain.

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
facts. They preserve bounded framed terminal projections without reinterpreting them and never
emulate VT or become terminal-content authorities. Terminal frames, input, history, selection, and
export stay outside the canonical Workbench commit queue.

A measured optimization may let a colocated native TUI or Desktop Client use
`Terminal Host <--> Worker <--> Client`. The accepted design requires an exact short-lived
Server-issued attachment capability bound to Client Session, Terminal, Worker, Host epoch,
controller generation, allowed operations, and expiry or revocation. It is implemented only after
relay measurements show need; Browser and mobile stay relayed, and no Client connects directly to
Host. Server-Worker authority loss stops input/resize and permits at most one explicitly bounded
read-only drain. Direct failure may request a fresh relay authorization, never replay unknown input.

Server terminal relay may authenticate, route, enforce bounds, isolate backpressure, and request a
fresh snapshot. It does not parse or emulate VT, own history, durably persist terminal content, or
automatically replay input with an unknown admission outcome.

## Primary workflows

### Local Session activation

1. A TUI, mutating CLI command, Web opener, or Desktop launch discovers the exact local Session.
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
4. If the transition includes terminal creation, one Server-owned state machine coordinates the
   Workbench mutation, durable terminal intent, exact Worker effect, and typed outcome. The Client
   never performs two independently visible phases. Cross-process effects are reconciled explicitly
   rather than falsely claimed as a database-atomic Host launch.

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
   Checkout, Worktree, Environment, Session, or Worker-lease semantics.
7. Worker reports the exact observation; Server reconciles and publishes the product outcome.
8. Each Client opens a separate authorized attachment as needed. Deferred creation requires a later
   Server-authorized `StartTerminal` with complete geometry through the same Worker.

If an optional managed Worktree operation preceded terminal creation, its only input to this flow is
the resulting node-local directory used as intent `cwd`.

### Optional Agent observation

1. When the Agent capability is explicitly enabled, an installed provider hook/plugin invokes
   `orchbee-agent-bridge` with bounded event data and exact execution context.
2. Bridge sanitizes the event, attempts one local Worker delivery, waits for one exact ACK, emits the
   private correlation half only after acceptance, and exits fail-open.
3. Worker validates and joins the rich event and correlation against authorized Terminal and
   Presentation generations before forwarding one bounded observation.
4. Server owns Agent semantic reduction, canonical shared state, and publication to Clients.
5. Terminal Host transports only the private bounded correlation and never derives Agent semantics.

## Multi-client terminal semantics

One retained terminal may have several attachment-scoped projections.

| Shared terminal fact                                      | Attachment- or Client-local fact                        |
| --------------------------------------------------------- | ------------------------------------------------------- |
| PTY, shell, current process, terminal output, and exit    | Active Tab and focus                                    |
| Canonical VT content, modes, title, and retained history  | Viewport and scroll position                            |
| Accepted ordered terminal input and its effects           | Search query, result cursor, and highlighting           |
| Canonical terminal geometry and terminal-generated resize | Selection presentation and platform copy effect         |
| Clear, reset, close, and process termination outcomes     | Projection demand, pause, recovery, and local animation |

An attachment is a transient display/control relationship with a retained terminal; it does not
create, clone, restart, or own that terminal. Host attachment capabilities decide whether an
attachment may read, input, or resize. Different Client Sessions may control different terminals.
Several attachments may read one terminal, but one combined input-and-resize controller exists at a
time. Attach acquires it only when unowned; otherwise the Client is explicitly view-only. Release
does not auto-promote another viewer. Normal transfer asks the current active owner; after observed
disconnect plus grace and a modeled timeout, an authorized second-confirmation forced take commits a
new Server controller generation and fence. Late prior-generation input or resize is rejected.
Separate controllers and collaborative input remain excluded. Selecting, searching, copying, and
scrolling history stay independent where they do not become application mouse or other PTY input.

## Protocol and transport boundaries

| Boundary                      | Semantic purpose                                                                                                               | Local transport candidate        | Remote transport candidate                        |
| ----------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------- | ------------------------------------------------- |
| Client to Server              | Human session, snapshots, deltas, semantic commands including narrow OBCS Create/Start, attachment bootstrap, and remote relay | Unix-domain socket or named pipe | HTTPS plus bounded binary WebSocket or equivalent |
| Server to Worker              | Node enrollment, capability versions, `AuthorizedTerminalLaunch`, lease, heartbeat, fencing, progress, and result              | Unix-domain socket or named pipe | Authenticated encrypted outbound node connection  |
| Worker to Terminal Host       | `HostLaunch`, exact terminal lifecycle, input, projection, history, selection, and Host inventory                              | Existing private Host IPC        | None                                              |
| Agent Bridge to Worker        | One optional bounded local Agent observation plus exact ACK                                                                    | Private local IPC                | None                                              |
| Local native Client to Worker | Measured optimization carrying one Server-authorized live-terminal attachment                                                  | Scoped private local IPC         | None; Browser and mobile remain Server-relayed    |

`Capability Node Protocol` names Server-Worker semantics, not an extra process or fixed transport.
It runs over local IPC when colocated and over a network connection for a remote Worker.

Current TypeScript protocol packages cannot silently become the Rust contract. The accepted
implementation must choose one canonical schema and behavior source, generate or independently
implement codecs with cross-language conformance evidence, and avoid two mutable protocol
authorities.

OBCS version 1 owns Client-Server terminal Create and Start and the live viewer and control
operations: display attachment, controller acquisition and transfer, OBTF frames and
acknowledgements, input, and resize. No separate live-terminal protocol exists. TUI submits one
Workbench business intent, and Server coordinates the shared document mutation with terminal
intent/effect reconciliation. OBCS never exposes Host binding identities, Host capabilities, or raw
PTY byte streams to a Client.

## Identity and fencing

At minimum, the target distinguishes:

- Session, human identity, and Client Session;
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

- A local Human Client running as the same effective operating-system user may read its authorized
  Workbench projection and submit ordinary terminal operations under Server policy. Terminal
  ownership belongs to Session/Workbench state, not to the Client Session that created it; request
  identities are correlation values, not capabilities. Remote authentication remains separate.
- Human Client credentials and Worker device credentials remain distinct and non-interchangeable.
- Worker normally initiates its Server connection; Terminal Host exposes no LAN or Internet
  listener, and Agent Bridge preserves that constraint.
- Terminal Host retains its same-user local trust boundary and gains no Session authorization logic.
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

| Event                                     | Required or candidate behavior                                                                                                                                                                               |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| CLI invocation exits                      | Ends only its request or observation                                                                                                                                                                         |
| TUI, Web, Desktop, or mobile Client exits | Releases that Client Session and attachments; shared state and retained terminals remain                                                                                                                     |
| Client transport loss                     | Marks the attachment disconnected; no deletion or terminal input replay                                                                                                                                      |
| Server unavailable                        | Host preserves retained execution; Worker accepts no input/resize without current Server authority and exits in the first milestone                                                                          |
| Server restart                            | First unshipped milestone has no durable adoption: a successor accepts only exact known identity and otherwise reports `RecoveryRequired`; production later reconciles from the accepted transactional store |
| Server-Worker link loss                   | Worker accepts no invented authority; retained Host terminals continue; bounded observation retention or loss policy remains reserved                                                                        |
| Worker restart                            | Host survives independently when possible; replacement Worker establishes a new generation and reconciles exact Host inventory                                                                               |
| Terminal Host crash                       | Worker reports terminal loss; no PTY recovery is claimed                                                                                                                                                     |
| Node or operating-system restart          | Ordinary PTYs and processes may be lost; Session reports the distinct machine-restart outcome                                                                                                                |
| Agent Bridge unavailable or rejected      | The hook exits zero, stays silent, and creates no fabricated observation                                                                                                                                     |
| Slow remote Client                        | Its bounded relay or attachment is paused or retired without blocking another Client or terminal                                                                                                             |
| Lost terminal-input acknowledgement       | Outcome is unknown and never automatically replayed                                                                                                                                                          |
| Version or protocol mismatch              | Reject or quarantine the incompatible connection without replacing a Host that still owns retained terminals                                                                                                 |

For the first unshipped local milestone, Server loss closes the Worker control connection and the
Worker exits; Host may retain its terminals. There is deliberately no durable Server adoption in
that milestone. A successor that observes any nonempty Host inventory it cannot match exactly to
its own known bindings fails closed with `RecoveryRequired`; it does not adopt, delete, replace, or
send input. The production persistence contract above must be implemented and verified before this
failure mode can be promoted to public release.

## Application responsibility migration

The additive native foundations and the local-terminal vertical milestone are authorized as stated
above. No row in this section alone authorizes a later consumer migration, public cutover, or
deletion.

| Current or candidate application | Review status and target disposition                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| -------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `apps/orchbee-server`            | **Delivered additive boundary:** the unshipped Rust Server owns projection mechanics, bounded same-user Worker admission and Client Connections, terminal Create/Start authorization, generation-pinned read-only viewer routing, exact projection feedback and disconnect cleanup. Client output does not retain the Worker operation slot. Explicit exclusive controller grants now carry input and resize to Host, with generation-pinned release cleanup. The native Client activates it on Windows, Linux, and the macOS development package; production Browser ingress and remote relay remain later integrations or migrations.        |
| `apps/server`                    | **Retired:** explicitly sunset with its consumers; no replacement package is required for source deletion.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `apps/orchbee-worker`            | **Delivered additive boundary:** the separate Rust Worker owns bounded same-user Server registration, retained Worker generation, an authenticated exact-version Host broker connection with retained Host epoch and limits, coupled peer-loss cleanup, and an integrated deterministic OBCN authorization/replay kernel and bounded Host Create/Start execution loop. The generation limit is accepted at 1,024 retained bindings with no implicit eviction; the private carrier now consumes exact authorization and returns correlated results. Explicit retirement is required before production, and remote enrollment remains later work |
| `apps/terminal-host`             | **Delivered additive connection boundary:** retain the separate Zig application and Ghostty authority; the unshipped Worker now consumes its existing same-user endpoint and handshake as the normal broker connection and existing Create/Start contracts without changing Zig, migrating Desktop/SSH adapters, or merging the process                                                                                                                                                                                                                                                                                                        |
| `apps/agent-bridge`              | **Accepted target:** add a private one-shot Rust `orchbee-agent-bridge` only with its first migrated provider integration. It is a versioned Capsule/libexec asset, talks only to local Worker, stays silent and fail-open, starts no service, and owns no durable state                                                                                                                                                                                                                                                                                                                                                                       |
| `apps/desktop`                   | **Accepted:** retain the Desktop Client shell, renderer bootstrap, packaging, platform effects, and application-local lifecycle; apply the complete target disposition below, including the separately accepted Agent, Worktree, Archive, Launch Profile, and SSH connection boundaries                                                                                                                                                                                                                                                                                                                                                        |
| `apps/web`                       | **Accepted:** retain the sole Browser/Desktop renderer application and absorb the React Workbench shell, browser interaction state, layout persistence, and presentation implementation from `@orchbee/workbench`                                                                                                                                                                                                                                                                                                                                                                                                                              |
| `apps/ssh-environment-runtime`   | **Retired:** explicitly sunset with its consumers; no replacement package is required for source deletion.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| `apps/storybook`                 | **Retain:** development-only visual evidence host; application-owned Web stories move with the React Workbench implementation                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

## Desktop Main target disposition

The product owner delegated the Desktop boundary and accepted this target disposition.
These are responsibility outcomes, not mechanical directory moves: an accepted row removes the
current Desktop owner only after the destination behavior and recovery evidence exist. The table
covers every current Main area; the `user-preferences` row is pending review. The areas of the
retired TypeScript CLI, generic Environment, and SSH Runtime no longer exist;
[decisions 14 and 20](#accepted-decisions-from-product-review) govern their outcomes.

| Current `apps/desktop/src/main` area                                                                   | Accepted target disposition                                                                                                                                                                                                                                                                                                                         |
| ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `client-session` and `workbench-document`                                                              | **Move to Server, then remove from Desktop:** Server owns authenticated Client sessions, canonical Workbench state, commit, snapshot/delta publication, persistence, and reconciliation                                                                                                                                                             |
| Shared-document portions of `terminal-document-control`                                                | **Move to Server, then remove from Desktop:** Clients submit terminal intent; Server owns the canonical mutation and authorized Worker placement. Desktop retains no terminal-create authority                                                                                                                                                      |
| `host`, `terminal-binding-registry`, and Host-facing portions of `terminal-lifetime`                   | **Move to Worker, then remove from Desktop:** Worker owns Host discovery, brokering, bindings, lifecycle effect dispatch, output-read routing, and inventory reconciliation; Terminal Host remains the terminal authority                                                                                                                           |
| `terminal-projection-control` and Host-facing `broker` behavior                                        | **Split, then reduce Desktop:** Server authorizes attachments and Worker routes Host data. Desktop may retain only a bounded Electron transport handoff needed by its sandboxed renderer; it owns no attachment policy, terminal state, or node routing                                                                                             |
| `project-folder-acquisition`                                                                           | **Retain only the Desktop platform adapter:** it may show a native picker and return the user-selected local path to the authorized Client workflow; it does not create canonical Project state or claim that a client-only machine is the selected Worker                                                                                          |
| `terminal-default-settings`                                                                            | **Split:** move node-local default interactive-shell resolution to Worker; move the canonical revisioned named-profile catalog to Server; retain only Client presentation and preference editing in Desktop                                                                                                                                         |
| `application-lifetime`                                                                                 | **Split and retain only Desktop lifetime:** window/app quit and renderer teardown remain in Desktop; Session shutdown, shared terminal intent, Worker drain, and Host retention belong to their owning Server, Worker, and Host contracts                                                                                                           |
| `desktop-clipboard`, `window-actions`, `window-state`, `application-menu`, and `secure-keyboard-entry` | **Retain in Desktop:** exact Client-local or operating-system effects                                                                                                                                                                                                                                                                               |
| `desktop-renderer-protocol`, preload bootstrap, and remaining `broker` adapters                        | **Reduce and retain:** expose only the minimum sandboxed renderer bootstrap and exact Desktop platform capabilities; remove Server, Worker, terminal-authority, Agent-owner, and legacy Environment messages when their owning slices migrate                                                                                                       |
| `composition` and `recovery`                                                                           | **Rewrite around retained Desktop responsibilities:** keep only Electron Client/platform composition and Client-local recovery; Server owns Session recovery, Worker owns node/Host reconciliation, and no replacement composition may recreate the removed authorities                                                                             |
| Shared Agent state in `agent-observation` and `agent-integration-management`                           | **Move canonical state to Server and node integration effects to Worker:** Desktop retains only its Client presentation and explicit platform interaction; provider callbacks migrate to private Rust Agent Bridge                                                                                                                                  |
| `project-worktree`                                                                                     | **Split:** Server owns semantic intent/reference, Worker owns the optional managed-root Git workflow and recovery journal, and Desktop retains only Client UI; first migration covers create/discover/adopt and Client-created removal                                                                                                              |
| `terminal-archive`                                                                                     | **Split:** Worker captures/stores the final platform-neutral snapshot, Server owns metadata/index/authorization, and Desktop retains only Client presentation; live VT/history remains in Host                                                                                                                                                      |
| `terminal-launch-profiles` and non-baseline portions of `terminal-default-settings`                    | **Split:** Server owns canonical catalog, scope, revision, permissions, and exact selected revision; Worker validates and executes; Desktop retains only Client presentation and editing                                                                                                                                                            |
| `user-preferences`                                                                                     | **Pending review — move to the Server settings contract, then remove from Desktop:** the native shared preference store and schema stay authoritative; Desktop Settings reads and commits through the same Server settings requests as the TUI and retains only presentation and editing; `apps/orchbee-settings` is removed with its last consumer |
| SSH connection workflow, which has no current Main area                                                | **Rewrite around `SshTarget`/`SshConnection`:** Server owns intent/state, selected local Worker runs system OpenSSH and temporary remote Worker/Host, and Desktop retains the first local interactive credential/host-verification UX; remove generic Environment                                                                                   |

The migration must update each source and destination `project.json` in the same accepted slice. A
module is deleted only after its state, behavior, recovery, tests, and documentation have one
verified surviving owner.

## Package and workspace reduction audit

No workspace is removed because its name appears in this table. Every removal requires a consumer
scan, public-export decision, migration/compatibility policy, replacement or explicit
product-sunset evidence, and deletion of stale documentation and tests in one accepted slice.

| Workspace or package                                                               | Candidate review outcome                                                                                                                                                                                                                                                                                                                       |
| ---------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `@orchbee/desktop-client-protocol`                                                 | **Accepted reduction:** retain only sandboxed Desktop bootstrap and exact platform-local capabilities; remove Server, Worker, terminal-authority, canonical Agent, managed-Worktree, Archive, Profile, and SSH execution messages after their accepted migrations                                                                              |
| `@orchbee/remote-client-protocol`                                                  | **Retired:** no runtime consumers. Browser authentication belongs to the native Server; future Web consumes its admitted contracts without a separate TS authentication package.                                                                                                                                                               |
| `@orchbee/workbench-client-protocol`                                               | **Accepted bounded retention:** keep only as the TypeScript binding for a transport-neutral Client session, semantic-intent, snapshot, and delta boundary while real TS consumers remain; the canonical cross-language schema is framework-neutral, and Rust/mobile bindings do not depend on this package                                     |
| `@orchbee/terminal-protocol`                                                       | **Accepted retention:** keep the TypeScript binding; `schemas/terminal` is the common Host wire authority for Rust, Zig and TypeScript conformance                                                                                                                                                                                             |
| `@orchbee/ssh-environment-protocol` and `apps/ssh-environment-runtime`             | **Retired:** explicitly sunset with its consumers; no replacement package is required for source deletion.                                                                                                                                                                                                                                     |
| `@orchbee/agent`                                                                   | **Accepted decomposition and deletion:** move its deterministic canonical state machine to Rust Server, define one cross-language schema, move node-local integration effects to Worker and provider ingress to Agent Bridge, move Web presentation to `apps/web`, then delete the mixed TypeScript package after migration evidence           |
| `@orchbee/workbench`                                                               | **Accepted decomposition and deletion:** move its React shell, browser-local interaction state, layout persistence, styles, and stories into `apps/web`; extract shared vocabulary and snapshot/delta contracts to the framework-neutral cross-language authority; move canonical transitions to Server; then delete the mixed browser package |
| `@orchbee/actions`                                                                 | **Accepted retention:** keep framework-independent action discovery and Client-side execution lifecycle while the Web renderer consumes it; do not turn it into a second Server command authority                                                                                                                                              |
| `@orchbee/foundation`                                                              | **Accepted retention with pruning:** keep dependency-light domain-neutral primitives with surviving cross-workspace consumers; audit orphan exports after each owner migration rather than moving backend behavior into it                                                                                                                     |
| `@orchbee/keyboard-shortcuts`                                                      | **Accepted retention:** keep TypeScript Client shortcut normalization while Web or Desktop consumes it; Rust TUI owns its platform input adapter and may share identifiers only through an accepted contract                                                                                                                                   |
| `@orchbee/terminal`                                                                | **Accepted retention:** keep the Browser/Desktop terminal renderer; Rust TUI consumes Terminal Protocol through its own renderer and does not make this browser package obsolete                                                                                                                                                               |
| `@orchbee/ui`, `@orchbee/tab-strip`, `@orchbee/i18n`, and `@orchbee/design-tokens` | **Accepted retention:** keep while the Web/Desktop renderer consumes them; backend migration alone is not evidence for deletion                                                                                                                                                                                                                |
| Current Desktop Main modules                                                       | Apply the complete accepted target-disposition table above; do not preserve compatibility wrappers without a released compatibility need                                                                                                                                                                                                       |

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

| Foundation                                 | Delivered boundary and current owner                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rust repository contract                   | The root Cargo workspace, pinned toolchain, Rust standards, project-contract support, and root verification are current repository infrastructure owned by [`docs/RUST.md`](../../docs/RUST.md) and the root build files.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Workbench projection contract              | JSON Schema version 1 plus independent TypeScript and Rust bindings provide bounded read-only snapshots and atomic deltas. The schemas, package guide, crate contract, and conformance corpus own the maintained facts; no current writer or consumer has migrated.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Client-Server terminal contract            | [`schemas/client-server`](../../schemas/client-server/README.md) and [`orchbee-client-server-protocol`](../../crates/client-server-protocol/README.md) own the strict bounded OBCS version 1 terminal Create/Start, viewer, and controller intent and result contract, including its cross-language corpus and at-most-once producer obligations. The private Server maps it through its local terminal coordinator to the OBCN carrier. The private Rust `orchbee` Client can submit typed requests serially, each at most once under one monotonic total deadline, but no public invocation reaches that transport.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Server-Worker launch contract              | [`orchbee-capability-node-protocol`](../../crates/capability-node-protocol/README.md) owns OBCN frame version 1 with terminal-launch capability version 1. Server now owns a generation-scoped serialized OBCN carrier; its launch-authorization implementation and Worker's private kernel own admission, fencing, 1,024 retained bindings, exact replay, capacity rejection, handoff, and result behavior under the documented Server-producer invariant. The private Worker now consumes authorized requests through that kernel and its bounded Host execution loop; explicit binding retirement remains a production prerequisite.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Native `orchbee` entry                     | [`apps/orchbee`](../../apps/orchbee/README.md) owns the experimental bare TUI and CLI entry on Windows, Linux, and the unsigned Apple Silicon macOS development package, with effect-free help/version and the private local transport. Through the shared runtime it validates the exact cohort and rendezvous, connects to or activates the Session, and enters the TUI after a valid first frame or a stable navigation view. Windows adds named Sessions and complete-cohort release installation and updates; Linux and macOS run staged development cohorts. The [TUI platform boundary](../../apps/orchbee/docs/tui-platforms.md) owns the remaining platform differences.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Local Session activation model             | The retained LocalSessionStartup protocol and Client implementation tests cover connect-first activation, singleton exact-cohort launch, typed readiness/failure, deadlines, resource release and Client/Session lifetime separation. The older standalone activation model is retired; current claims and bounds belong to the Client and local-runtime references.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| Rust Server kernel and service             | [`apps/orchbee-server`](../../apps/orchbee-server/README.md) owns bounded Workbench reads, failure-atomic in-memory projection publication selection, and separate unshipped private same-user local Worker and Human Client services. The Worker service admits one compatible Worker and retains its post-OBLW stream behind one serialized OBCN owner with one submitted slot; the Client service admits a bounded number of compatible Client Connections. The OBCS coordinator authorizes Create/Start through that carrier, commits exact terminal bindings across Client Connections, and preserves unknown outcomes without replay. Viewer records/feedback and single-controller input/resize now relay through the same carrier with exact Connection/grant cleanup. Managed activation from an exact cohort is delivered on Windows, Linux, and the macOS development package. Only Windows adds restart checkpoints, durable Session lifecycle records, and complete-cohort release installation; Linux and macOS startup implements no recovery or restart. The accepted transactional persistence, user services, and remaining production lifecycle are undelivered. |
| Terminal Client authority and relay models | The Server retains compact ConnectionViewerOwnership and RelayDeliveryIsolation protocol cores for exact viewer ownership, stale cleanup, ordered feedback and slow-viewer isolation. Native control takeover has its own bounded protocol. Concrete carrier, encoding, queue-capacity and cross-process failure combinations use implementation tests. The Server reference owns exact bounds and exclusions; this proposal does not claim the former combined authority/relay state space is still checked.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Rust Worker process spine                  | [`apps/orchbee-worker`](../../apps/orchbee-worker/README.md), [`orchbee-local-worker-protocol`](../../crates/local-worker-protocol/README.md), and [`orchbee-local-peer-credentials`](../../crates/local-peer-credentials/README.md) own the unshipped Server registration plus normal Worker-to-Host broker connection, bounded private framing, POSIX peer proof on both links, exact Host handshake, retained admission identities and limits, coupled connection liveness, the integrated authorization kernel, default interactive-shell resolver, Host command mapper, and bounded OBCN request/result loop described above. The private Worker can create/start retained Host terminals, relay viewer records and feedback, and apply authorized input/resize; it has no production consumer.                                                                                                                                                                                                                                                                                                                                                                                |
| Rust Terminal Host binding                 | [`crates/terminal-protocol/project.json`](../../crates/terminal-protocol/project.json) and the package-owned [Terminal Protocol reference](../../schemas/terminal/references/protocol.md) own the independent Rust envelope and payload bindings completed through terminal-configuration kinds 70 through 73. Worker consumes the launch, viewer, feedback, and controller bindings for its unshipped terminal path. The private TUI consumes bounded OBTF 0.1.0 text-table decoding; optional extension contents stay opaque. Additional message codecs require a concrete vertical consumer.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |

Historical slice-by-slice authorizations and command results are not maintained here. Current
behavior belongs to the linked owners and executable evidence; this proposal retains only
unresolved target design and the next accepted milestone.

## Accepted local-terminal vertical milestone

The product owner accepted this consequential milestone on 2026-09-02 and delegated its
dependency-ordered implementation slices after accepting the observable behavior, authority
boundaries, exclusions, evidence, and rollback below. A slice must return for review only when it
discovers a materially different product outcome, owner, public contract, risk, or exclusion.

| Field                   | Accepted decision                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem                 | The native entry, Server kernel, and Rust Host binding are individually verified but do not form a runnable product path. Continuing message-by-message parity would increase unused protocol surface while leaving the one-command experience, Worker boundary, process lifetime, real terminal rendering, and IPC performance untested.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Outcome                 | On one supported local machine, bare `orchbee` discovers or lazily activates the exact compatible private Server and local Worker cohort, enters a real TUI Client Session, and can create or attach one default-interactive-shell terminal whose actual output, input, and resize flow through the retained Zig Terminal Host. Closing one Client never implicitly terminates the Session, private services, or a retained terminal. A later invocation joins the same Session as another independent Client Session.                                                                                                                                                                                                                                                                                                                                 |
| Scope                   | Deliver the private Rust Server service boundary, one private Rust local Worker, existing Zig Host discovery or activation, bounded same-user local IPC, minimal default-shell terminal intent with optional node-local `cwd`, Host launch mapping, create/attach/output/input/resize flow, a real single-terminal TUI surface, bounded relay queues, version-cohort rejection, and the process/fault/performance evidence required for that path.                                                                                                                                                                                                                                                                                                                                                                                                     |
| Client model            | TUI, Web, Desktop, mobile, separate browser tabs, separate Desktop windows, and repeated `orchbee` invocations are peers under one Client model. Each active instance has its own Client Session and local focus, active location, viewport, scroll, and presentation state. Shared Workspace, Tab, Panel, and Terminal facts come from Server. Terminal Host sees typed attachments and capabilities, never renderer identity.                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Terminal control        | Different Clients may control different terminals. Several Client Sessions may view one terminal through separate attachments, but the milestone admits only one active input-and-resize controller for that terminal. Another attachment remains non-controlling. The accepted transfer/forced-take contract above is a later implementation slice; collaborative input remains excluded.                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Activation and lifetime | Bare `orchbee` may initiate bounded lazy activation of the exact installed private cohort and wait for readiness, but it does not remain the Server or Worker's lifetime supervisor after readiness. `--help` and `--version` remain side-effect free. Client exit releases only that Client Session and its attachments. Exact daemonization or platform service mechanics are delegated implementation choices that must preserve singleton, version, cleanup, and failure claims.                                                                                                                                                                                                                                                                                                                                                                   |
| Launch boundary         | Client intent never supplies authoritative Worker placement, lease, generation, fence, executable, arbitrary arguments, Environment, Project, Checkout, Worktree, or Agent identity. Server selects the one local Worker; Worker resolves the accepted default interactive shell and omitted local defaults; Host receives only its exact launch facts. The first milestone sends an empty transient environment overlay.                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Data path               | The first complete path is `Terminal Host <-> Worker <-> Server <-> Client`. Worker and Server validate routing, identity, role, size, queue, and authorization facts but do not emulate VT or reinterpret frame content. A direct local Client-to-Worker attachment is not part of this milestone; measurements gate the already accepted scoped-capability optimization above.                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Trust                   | The milestone exposes private same-operating-system-user local endpoints only. It adds no LAN or Internet listener, Cookie-to-device credential conversion, ambient direct Host access, or multi-user claim. Wrong peer role, stale identity, malformed data, and incompatible versions fail closed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Claims                  | One local Session has at most one active Server authority and one admitted local Worker identity. Help/version start no service. Mutating intent is submitted at most once and unknown terminal input is never replayed. A slow Client cannot block Worker progress or another Client. Client disconnect does not imply terminal termination. Server never becomes a VT/history authority, Worker never becomes a generic command/filesystem service, and the baseline terminal requires no Project, repository, Worktree, Environment, or Agent state.                                                                                                                                                                                                                                                                                                |
| Non-goals               | No complete Workbench UI, additional public CLI grammar, selection/export/history/theme/clear/reset feature parity, direct Worker data path, final collaborative-input or takeover UX, Agent helper, managed Worktree, Archive, Launch Profile, SSH redesign, remote Worker, Web/mobile migration, production TLS, multi-user tenancy, durable Server recovery, operating-system-restart PTY recovery, login-time service installation, package-manager release, public cutover, or compatibility promise is admitted.                                                                                                                                                                                                                                                                                                                                 |
| Evidence                | Model singleton activation, independently ordered Client/Server/Worker/Host loss, controller ownership, bounded queues, and unknown input outcome where the existing topology model is insufficient. Add process tests for absent/present/incompatible cohorts, two simultaneous Client Sessions, Client exit with retained terminal, malformed peers, Host loss, and bounded shutdown. Add an end-to-end real PTY trace for create, attach, frame, input, resize, detach, reattach, and exit. Measure warm/cold activation, input latency, frame throughput, copies, queue memory, and slow-client isolation before enabling the direct path or setting a numerical release budget. The applicable focused checks and root `pnpm verify` remain required; the manual focus-stealing native-window test remains excluded unless explicitly authorized. |
| Delivery and rollback   | First converge the three owning proposals. Then deliver a no-terminal-effect process spine, the minimal launch/Host broker, the relayed terminal stream, the TUI adapter, and finally loss/performance convergence. Each slice stays additive and unshipped, has no durable migration, and leaves current Desktop behavior authoritative. Rollback removes the new unused slice or returns to the last verified additive spine; it never deletes or switches an existing consumer.                                                                                                                                                                                                                                                                                                                                                                     |

## Delivery and migration sequence

| Milestone                            | Independently reviewable outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native foundations                   | **Complete and unshipped:** repository/Cargo support, read-only Workbench projection contracts, the native entry and staged TUI shell foundations, the in-memory Server projection kernel, and the independent Rust Terminal Host binding. Their owning guides and executable tests maintain current facts.                                                                                                                                                                                                                                                                                                             |
| Local terminal vertical              | **Integrated and experimental:** on Windows, Linux, and the unsigned Apple Silicon macOS development package, bare invocation connects to or activates a local Session from a verified cohort and enters the TUI after a validated first frame or a stable navigation view. Windows additionally has named Sessions, restart checkpoints, and complete-cohort installation and updates. Other platform differences belong to [TUI platform boundaries](../../apps/orchbee/docs/tui-platforms.md); current contracts: [Client](../../apps/orchbee/README.md) and [shared runtime](../../crates/local-runtime/README.md). |
| Shared Web/Desktop Client            | **In progress:** move React Workbench presentation into `apps/web`, migrate shared semantic transitions to Server, retain Desktop-only platform effects, and delete `@orchbee/workbench` only after its complete responsibility audit. Order follows the accepted [migration plan](#web-and-desktop-client-migration-plan).                                                                                                                                                                                                                                                                                             |
| Optional capabilities                | Implement the accepted Agent observation, managed Worktree, Archive, Launch Profile, and Connect-via-SSH contracts as independent owner-specific migrations; none is required for the terminal baseline.                                                                                                                                                                                                                                                                                                                                                                                                                |
| Production and distributed operation | Implement accepted local persistence/recovery, user services, cohort updates, controller transfer, and measured direct attachment independently. Remote enrollment, production human/device identity, TLS/Internet ingress, multi-user tenancy, and mobile backgrounding still require later decisions.                                                                                                                                                                                                                                                                                                                 |
| Remaining ownership migrations       | Require consumer evidence, installed-artifact handling, and rollback for the remaining Workbench, Agent, and Desktop ownership changes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |

Rollback preserves one canonical writer. Every local-terminal slice is additive and unshipped,
so rollback removes only its unused artifacts or returns to the last verified process spine.
Later consumer or durable-schema migrations require their own cohort rollback and may never start
an old writer against unknown new state. Compatible retained Hosts are not replaced while they
own live terminals.

## Web and Desktop Client migration plan

**Accepted and in progress since 2026-11-01.** The product owner opened the Web/Desktop phase and
accepted the decisions below, which replace the slice-by-slice plan recorded on 2026-10-28. Facts
were re-checked at `66189cf9e`. Delivery is tracked by ORC-201.

### Decisions

1. **No parallel legacy path.** Web and Desktop need not stay usable during the migration. The
   legacy GUI is frozen and receives no fixes. When the Server path runs, the replaced owner, its
   tests, and its documentation are deleted without a compatibility wrapper; slices do not switch
   one at a time behind a development composition, and one build never has two writers for one
   state. Desktop-era Workbench documents, binding stores, and archives are not migrated.
2. **Workflows move to Server where they can.** A multi-step workflow that the Rust Client drives
   today moves to Server as one intent when both Clients would otherwise implement it: creating a
   Workspace sends one request and Server chooses the default directory. Presentation state stays
   in each Client. A second TypeScript implementation of a workflow is the last resort, and shared
   Rust compiled to WebAssembly is not part of this plan.
3. **Windows leads.** Each step is proven on Windows first. POSIX Server and Worker capabilities
   proceed in parallel from step 3 and do not gate Windows steps.
4. **Desktop bundles its complete native cohort.** Desktop ships and activates its own exact Server,
   Worker, and Terminal Host cohort. A CLI of another cohort on the same machine cannot join
   Desktop's Session and reports the required upgrade. Long-running task continuity across cohorts
   belongs to ORC-296, which covers Desktop as well.
5. **Desktop first.** The first release has no Browser connection to real Sessions. Browser ingress
   and its authentication are designed together with remote networking (ORC-297); a Browser cannot
   present a peer credential, so loopback admission does not carry over.
6. **GUI-only capabilities wait.** Terminal Archive, broadcast input, image preview, file drop,
   Ghostty configuration import, and terminals retained after Stop are absent from the first
   migrated release and are decided one by one afterward. Terminal Archive leaves this migration.

These defaults also apply until the product owner overrules them:

- Quitting Desktop detaches, as the TUI does, and terminals keep running. Stopping the Session is an
  explicit action.
- The first Managed Worktree migration includes removal of Worktrees the Client created.
- macOS evidence stays a recorded manual run; no macOS Rust CI job is added.
- The language-neutral palette waits for the GUI design follow-up and does not block the migration.
- A small Rust process owns Server activation, peer verification, and OBLC admission, then relays
  the admitted byte stream to Desktop Main over stdio. TypeScript starts at OBWP. Step 2 proved this
  on Windows; POSIX runs it at step 7.

### Re-checked starting point

| Area                       | Current fact                                                                                                                                                                                                                                                                                                                      | Consequence                                                                                           |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Native Workbench           | The TUI uses the Server Workbench authority, OBWP, OBCS, and the relay on Windows, Linux, and the macOS development package. Restart checkpoints, stop, handoff, and eleven request classes are withheld off Windows.                                                                                                             | Desktop's Workbench switch cannot reach macOS or Linux before those POSIX capabilities exist.         |
| TypeScript Client bindings | Workbench bindings exist only as development conformance code. No TypeScript binding exists for OBWP multiplexing or OBCS. `@orchbee/terminal-protocol` binds only the Host wire.                                                                                                                                                 | Step 2 adds the first production bindings.                                                            |
| Terminal relay             | OBCS viewers already move complete Host records, so OBTF frames and row windows reach a Client without decoding. Control carries raw input, resize, host colors, clear, and paste. The GUI renderer sends semantic key, text, mouse, and focus input and complete theme and reset operations that Host encodes per terminal mode. | The relay gains these control operations before the renderer can use it.                              |
| Client workflows           | Workspace setup, Tab close, Worktree launch, and other multi-step workflows run inside the Rust Client `workbench_state`.                                                                                                                                                                                                         | Decision 2 moves them before Desktop needs them.                                                      |
| Agent integration          | Worker is the native writer. Desktop Main is a second, frozen writer. The Capsule source still lives in `apps/desktop/resources` and is read by native code and packaging.                                                                                                                                                        | The Capsule moves with Desktop's writer in step 4, so its identity changes once.                      |
| Agent semantics            | Windows manages its providers without an Agent Bridge: Server hook ingress receives the hooks. The canonical corpus is still generated from `@orchbee/agent`.                                                                                                                                                                     | Step 5 no longer assumes Agent Bridge, and contract-authored corpora precede the TypeScript deletion. |
| Parity sources             | Action identities and Tab close targets are language-neutral in `schemas/workbench`. Native theme tests still read `tokens.css`.                                                                                                                                                                                                  | Only the palette remains, under its default above.                                                    |
| Shared preferences         | Launch Profiles are part of the shared preference store that Server settings requests already serve. Desktop reaches that store through `apps/orchbee-settings` and still composes separate launch-profile and terminal-default file stores.                                                                                      | No new contract is needed; step 4 removes the Desktop owners.                                         |

### Order

| Step                    | Outcome                                                                                                                                                                                                                                                                            | Removed at this step                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 Plan                  | This section and the ORC-201 issue tree agree.                                                                                                                                                                                                                                     | Nothing                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 2 Narrowest path        | On Windows, a development page connects to a real Server and opens one terminal: it types, shows output, and resizes while a TUI stays connected to the same Session. Measures full-screen throughput and input latency, and answers whether the polling viewer model must change. | Nothing                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 3 Workbench rewire      | Project, Workspace, Tab, and Panel reads and writes go through Server, and multi-step workflows move under decision 2.                                                                                                                                                             | Nothing yet                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| 4 Switch and delete     | The Server path is the only path. Settings use Server settings requests, Agent integration management invokes Worker through Server, and Desktop keeps only platform effects. Quitting detaches. Recovery covers Desktop restart; Server, Worker, or Host loss; and Session stop.  | `client-session`, `workbench-document`, `host`, `terminal-binding-registry`, `terminal-archive`, `user-preferences`, `terminal-default-settings`, `terminal-launch-profiles`, `agent-integration-management`, `apps/orchbee-settings`; the shared-document parts of `terminal-document-control`; Host-facing `terminal-lifetime`, `terminal-projection-control`, and `broker`; terminal recovery in `recovery`; replaced composition; Desktop-era contracts. The POSIX Host endpoint addon goes when the POSIX Rust connection process replaces it. |
| 5 Capability completion | Agent observation, Managed Worktree, the GUI design follow-up, and TUI capabilities the GUI lacks: Agent Board, Recents grouping, controller takeover, configured commands, window titles, Workspace Git facts, Worktree removal, and Machines.                                    | `agent-observation`, then `@orchbee/agent` after its audit; `project-worktree`. The Project Folder picker stays.                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 6 Package ownership     | `apps/web` owns the React Workbench after the rewire is complete. The fate of the in-memory preview is decided here.                                                                                                                                                               | `@orchbee/workbench` after its responsibility audit                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| 7 POSIX                 | Server persistence and restart, the withheld request classes, and Worker Worktree mutation on macOS and Linux. Runs in parallel from step 3.                                                                                                                                       | Nothing                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| 8 Browser and remote    | Designed with ORC-297 after Desktop ships.                                                                                                                                                                                                                                         | Nothing                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |

### Step 2 result

Measured on 2026-11-01 on Windows with a Release cohort, Electron 42.8.0, and the real projected
renderer in a real window. **Poll stays the only way to take a record.** While this step ran, the
Server gained `ViewerRecordReady`, a hint that says when to Poll and carries no frame. The GUI
follows it the way the TUI does, and that answers the polling question: no frame push is needed.

| Question                  | Measurement                                                                                                                                                                                                                                                                                                                     |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Full-screen throughput    | 7.19 MB through `cmd /c type`: 3,641–4,006 ms with no viewer, 4,356–4,465 ms with the renderer, which receives about 60 frames per second. The Host spends most of the difference, 3.6–4.2 CPU seconds instead of 2.1–2.5.                                                                                                      |
| Typing latency            | PowerShell, keydown to frame bytes p50 3.8, p95 5.4, max 17.6 ms; keydown to the next paint p50 5.2, p95 15.5, max 23.2 ms. At the connection, from sending a key to its text on screen: PowerShell p50 3.4, p95 5.8 ms; `cmd.exe` p50 2.2, p95 4.2 ms; with another terminal on the connection flooding, p50 2.1, p95 3.5 ms.  |
| Relay and connection cost | At typing pace, with each request paired to its answer by stream: a key input through Main and the relay p50 0.8, p95 2.1, max 19.4 ms; a Poll returning a record p50 0.4, p95 0.7, max 1.0 ms. An idle viewer polls about four times per second, and the relay process uses at most 0.1% of one core with one viewer or eight. |
| TUI on the same Session   | The TUI and the window show one terminal live. Explicit takeover works in both directions, and the former controller becomes read-only without a failure. This row comes from an earlier cohort of this step, before hints.                                                                                                     |

The measured cohort also proved Vim editing with application cursor keys and a mouse click, a click
in a TUI hosted inside the window, and takeover between two windows. Pointer hover in that TUI and
focus-claim repair on pause and detach were proved on the earlier cohort. Limits carried forward:

- A connection serializes its requests, which costs nothing at typing pace: at 14 keys per second,
  344 of 346 commands left before the typing ended. A burst behaves differently. At about 850 keys
  per second, a reply takes up to 134 ms to reach the page, so only 25–29 commands reach the
  connection while keys arrive; the backlog then drains at about 1,500 commands per second. 670 keys
  arrived intact. 970 keys overflowed the renderer queue's bound of 1,024 commands 30 times through
  `renderer-backpressure`, and those inputs never reached the connection.
- The temporary page had no recovery surface, so input that followed an overflow could land behind
  the hole. Step 3 proved that the shared surface retires the attachment on that failure, as
  `apps/web/README.md` requires.
- Paste in a real window waits for step 3, which supplies the clipboard platform effects; only the
  connection layer has proved it.
- An absolute row read for the viewport, which revealing a find match in history sends, could not
  be expressed over OBCS. Step 3 added it as an absolute `read-viewport-rows` origin; a scrollbar
  drag reads by distance from the bottom and never needed it.
- After Worker loss, `server stop` fails in its background-jobs phase and the CLI cannot stop the
  Session. After Server loss with a live Host, `server start` refuses fresh activation because
  nothing adopts the Host. Step 4 owns both.

### Step 3 progress

Reads, writes of Project, Workspace, Tab, and Panel structure, and every close go through Server.
These choices stand unless review changes them:

- **The write port is the Workbench's existing event vocabulary.** `WorkbenchAuthority` takes those
  events and the preview keeps its reducer, so product surfaces stay unchanged. Step 6 decides what
  remains of the shared-document shape along with the preview.
- **A write waits for its publication.** Commands run one at a time and each is decided from the
  snapshot its revision names. A result carrying a revision settles when a publication reaches it,
  with no deadline of its own. A stale revision is shown and never replayed, and an unknown outcome
  is never repeated.
- **The read model comes only from publications.** Terminal lifecycle comes from Presentation
  bindings: a known launch failure and a missing binding are both failed states that nothing
  retries, because no Client launches implicitly and an unbound member stays unbound until someone
  starts it. Selection, expansion, focus, and maximize stay local, and Split ratios are shared.
- **Creation follows the TUI.** Automatic Workspace names avoid the snapshot's labels at submit, so
  they never jump. A later press revokes landing and the keyboard handover while creation completes
  in the background, and the first Workspace still replaces the empty location.
- **No speculative order or pending row.** Measured frame by frame, a released row otherwise flashed
  back for about 20 ms. It now holds where it was dropped until its result, which keeps the benefit
  without a second order to reconcile.
- **The Server says where New Workspace starts.** Both Clients ask `workbench.current-directory`
  for the focused Workspace. The Server samples its first terminal on its process observer and
  otherwise answers the Workspace's directory. The sample takes a directory the shell reported
  before the root process's, where the TUI used to read only the root process. The GUI waits at
  most five seconds, then uses the last directory the Server followed.
- **Decision 2 has moved one workflow so far.** Of the Rust Client's multi-step workflows, only the
  New Workspace directory moved to the Server. A Tab set close waits until ORC-301 settles the TUI's
  own sequence, and Duplicate waits for step 5's decision on progress and the full comparison. The
  TUI's Worktree launch and Worktree sidebar keep sampling the source terminal themselves until step 5. The rest stays in each Client, either as presentation, such as selection, navigation memory,
  drafts, and the close and take-control prompts, or because it belongs to the Client's own
  connection: Viewer membership and switching, settling a commit against the publications that
  Client sees, recovery fences, and removing a closed context.
- **A close follows the TUI's states without its probes.** A Workspace asks the Server's conditional
  close first and a Project always asks. A single Tab or Panel asks from the close activity its
  surface last observed, not from the TUI's shell report and process sample, and a set of Tabs always
  asks. Step 5 decides whether the difference still matters. A set compares the facts of its own
  Tabs, not the whole snapshot.
- **A close is followed to an end and never replayed.** Stale Workspace closes alone are sent again,
  within the TUI's limit and only on unchanged facts. An accepted close reads its operation for at
  most 50 seconds. Resume and a status surface belong to step 5; until then an unknown outcome names
  the status command.
- **Child terminals have no close.** The Server has no command that closes one child Presentation,
  so that entry stays disabled.
- **A Tab strip says what was asked for.** With an authority, its Actions report a change and the next
  publication replaces the model. Selection and collapsing stay local, and a move announces itself
  only once it completed.
- **A drag is one command, from its release.** The command comes from the arrangements before and
  after, and its boundary belongs to the order the drag began from, so a drag whose order changed
  before its turn is not sent. A publication under a held pointer ends the drag without a write, as a
  changed source does in the TUI. A whole group keeps its identity, including a one-member group.
- **A name is one change.** A group name commits when typing ends. Only a name someone typed fixes a
  Tab's, and a blank one asks for nothing, because Server has no way back to the automatic name.
- **Duplicate is narrower than the TUI's.** It makes one creation per source and reports only when it
  stops. It compares location, active Tab, and selection rather than the whole snapshot, and checks
  its 120-second bound only between copies. Step 5 decides whether progress and the full comparison
  are needed.
- **A divider is one command, from its release.** Held moves stay local and the released geometry
  holds until its publication. Following the TUI's divider rule, a stale revision goes again for at
  most 5 seconds while the split still holds the ratio it moved from, which makes this the second
  exception to never replaying. A ratio is rounded to five decimal places. Three differences from the
  TUI stand until step 5 decides them: a drag during a pending commit is not queued, another Client's
  change to that split under a held divider does not cancel the draft, and Esc does not cancel a
  divider drag.
- **Evening a Tab is several commands.** The Server changes one split per command, so a failure
  leaves the earlier ones committed. Evening has no TUI counterpart.
- **A dropped Panel holds its previewed place.** The preview already has the Server's geometry, so
  the new layer takes over with no visible change. Any other result returns it at once. A publication
  that changes the Tab's structure ends a held drag without a write, and one that changes only ratios
  or names lets it continue.
- **A new Panel takes a directory only if its source reported one.** Otherwise the Server's default
  applies, as for the TUI's split. The runtime's PowerShell reports none, so after `cd` a split opens
  in the Workspace's directory, unlike the CLI's `follow`. Decision 2 leaves this as it is, because
  the TUI's split does not sample either; the CLI keeps its own sample.
- **A surface that only views a terminal never waits to fit.** It cannot change the terminal's size,
  and waiting left the whole Tab inert for as long as the other Client held control. It draws the
  other Client's grid from its top-left corner, so a larger grid is cut off at the right and bottom
  and can hide the prompt. The TUI follows the cursor in a smaller viewer and says the view is
  cropped; the GUI does neither yet.
- **Take control is the way back, as in the TUI.** No notice arrives when another Client takes
  control, so a surface learns that it only views when its next input or resize is refused, and the
  keys of that attempt go nowhere. It then shows View only with a Take control button at the bottom
  right, away from the alerts at the top, and the terminal context menu leads with Take control;
  neither exists while the surface controls the terminal. A grant resizes the terminal to the pane
  and returns the keyboard unless the person has moved it. Nothing repeats a request: a busy Server
  read nothing, and an unknown outcome is reported for the person to repeat. Unlike the TUI, input
  resumes at the grant rather than after a fresh frame, and Take control has no default shortcut.
- **Typing carries across a layer swap.** While both layers are inert, keys that reach nothing are
  held, and once the new terminal has focus they reach it in order, with the text a real key would
  insert. They go nowhere if focus ended up elsewhere or more than a second passed, since the person
  may have typed them again. A surface being replaced still sends what it had accepted before its
  lane closes, for at most half a second, and the next surface waits for it. Before this, a split
  lost three to five keys. Keys typed through a swap with an input method are untested.
- **A terminal that cannot be shown says so.** As in the TUI, a failed launch says that creation
  failed and names the program when the launch named one, and a missing binding says only that no
  terminal is available. Both offer New Tab and the Panel's Close, and nothing retries. The page
  takes the keyboard where the terminal would have, so a key typed as it appears runs nothing.
  Without a launch profile client, it names no profile and offers no Launch profiles link or
  diagnostics copy. Only the CLI's `panel create-terminal` starts an unbound member, as with the
  TUI, and Close does nothing on a failed child terminal while child terminals have no close.
- **An empty Workspace says where a new Tab starts, as in the TUI.** Once its last Tab closes, the
  content names the directory at the top left, where a terminal's first row would be, followed by a
  hint pairing the New tab shortcut with its name. The hint is not a button, because the strip's
  New tab control is the pointer's way, so the content still holds no centered creation control.
  The keyboard is left on the page, where the shortcut works. A Workspace without a directory says
  so without the hint, because no Tab can start there and the GUI cannot set a directory, unlike the
  TUI. Windows Clients always create a Workspace with a directory, so no window has shown this yet.
  Without a Workspace, the content stays blank.
- **Tabs and terminals are named as in the TUI.** A console's default title, the path of its
  launched program, names that program instead, such as `PowerShell` or `Command Prompt`, and keeps
  a command that cmd runs. Other titles show as written once one elevation prefix and one activity
  mark are dropped. A terminal that shows no title takes the program the Server launched for it,
  else its Tab's place, such as `Tab 2`. The strip, the close question, and each terminal's own name
  agree, and a language change renames them at once by projecting the snapshot again. Unlike the
  TUI, a terminal that sets no title but reports its directory is named by that directory, a step
  the shared Workbench keeps for the frozen path.
- **Terminal colors follow the TUI.** The Server has no Host theme transaction, so a window paints
  every terminal's default-coded cells in the selected theme's colors itself, and a theme change
  recolors open terminals at once. Under the Host theme with the System application theme it also
  reports the light or dark defaults for the system's appearance as `host-colors`, so a terminal
  that follows its controller answers color queries to match. Programs in other terminals keep
  querying their launch colors, as in the TUI. Selection and search keep the dark overlay colors on
  a light terminal; step 5 decides them with the rest of the GUI's colors.
- **`panel.relocate` and `panel.rename` stay unconnected.** The GUI has no entry for either.

Measured on 2026-11-01 and 2026-11-02 on Windows with a Release cohort, in a real window beside a
real TUI or CLI on the same Session:

| Question                      | Measurement                                                                                                                                                                                                                                          |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Same state on both Clients    | A Workspace made in the window was on the TUI's screen at its first read, 78 ms later. One made with the TUI's `Ctrl+B a` reached the window 78 ms after the key and left the window's selection alone. A rename reached the TUI in 63 ms.           |
| Command to visible result     | New Workspace row 28–62 ms, and the location moved 6–14 ms later. Rename 19–22 ms, move to a Project 17–28 ms, reorder 12–17 ms.                                                                                                                     |
| Keyboard after creation       | New workspace, a row menu, the folder dialog, and the empty state each left typing in the new terminal without a click, and the Workspace count did not change.                                                                                      |
| Later press during a creation | With the Server's answer held back, a click on another row or a key press kept the person where they were, and the Workspace still appeared when the answer arrived.                                                                                 |
| Reorder held back             | A dropped row stayed at its drop position for the 600 ms the answer was held. A real `revision-mismatch` then restored the order and showed the stale notice, 3 ms after the answer arrived. Six plain drags showed only two frame states each.      |
| Refused input                 | A relative path was refused locally and a missing path by the Server's `directory-unavailable`. Both showed the same notice without moving focus, past the transient lifetime, until the next press.                                                 |
| Close with nothing running    | A Workspace or a shown Tab closed with no question, about 270 ms after the request, nearly all of it in the Server's `waiting-grace` phase. The keyboard stayed in or returned to the terminal.                                                      |
| Close with a program running  | `Start-Sleep` made `workspace.close-if-idle` answer `confirmation-required`, and the question opened about 20 ms after the click. Cancel changed nothing. Close sent a new request, landed on the first Workspace, and typing reached its terminal.  |
| Question over changed facts   | A CLI rename withdrew the Workspace question within 64 ms, a new member withdrew the Project's, and a rename withdrew the Tab's. None sent a command.                                                                                                |
| Stale close                   | An unrelated Workspace moved the revision under a held answer. A new request followed the real `revision-mismatch` 5 ms later and was accepted.                                                                                                      |
| Project close all             | The start and its reads, each under a new request ID, removed both members in 573 ms. The Project stayed, the window landed on a survivor, and the keyboard reached its terminal.                                                                    |
| Lost answer                   | With the command dropped and the Server stopped, the notice named the status command and nothing was sent again.                                                                                                                                     |
| Close activity                | At PowerShell's prompt, a finished line returned to idle 107–342 ms after Enter. A line left unfinished stayed unknown, and a resize read unknown for about 500 ms.                                                                                  |
| New Tab                       | The answer arrived 15–36 ms after the command, the Tab was selected, and typing reached its terminal without a click. A press during a held answer left the person where they were, and the Tab appeared behind them.                                |
| Tab drag                      | One `move-tabs-by-drag`, answered in 3–14 ms. By frame, only the dragged Tab moved after release, landing in about 190 ms with no reversal, and the current Tab stayed. A release where the drag began sent nothing.                                 |
| Tab drag against a change     | A rename from the CLI ended a held drag and its release sent nothing. A real `revision-mismatch` returned the Tabs and showed the stale notice. With the answer lost, the Tabs slid back in about 200 ms and the notice said the outcome is unknown. |
| Duplicate                     | Two selected Tabs sent two requests in order, each copy following its source, and selection did not move. Changing the selection under a held answer stopped at 1 of 2 with a notice.                                                                |
| Tabs on both Clients          | A Tab made with the TUI's `Ctrl+B c` reached the window 81–84 ms after the key, and a Tab made or dragged in the window reached the TUI's screen within a 600 ms sample. Neither Client's selection moved.                                           |
| Split and child terminal      | One command, answered in 17–41 ms. The new layer settled about 125 ms after the key, and typing reached the new terminal without a click. A press during a held answer left the keyboard where it was.                                               |
| Panel drop                    | One `panel.move`, answered in 7–9 ms. By frame, the dropped place held through the answer at the same geometry and the new layer took over about 90 ms after release. A release where it began sent nothing.                                         |
| Panel drop against a change   | A real `revision-mismatch` returned the Panel in one frame with a notice. Another Client's split ended a held drag and its release sent nothing, while another Client's ratio let the drag finish.                                                   |
| Divider                       | Nothing while moving and one command on release, with one picture from the press until 700 ms after release. It stopped at 0.1 and 0.9, and a release where it began sent nothing.                                                                   |
| Divider against a change      | A rename under a held answer led to a new request with the same ratio at the new revision and no notice. Another Client's ratio on that split led to no second request, a notice, and that Client's ratio shown.                                     |
| Resize dialog                 | Each key sent one command, in order, with no trailing digits. At the limit the keyboard stayed in the dialog and the opposite key still worked. Reset and Evening sent one command per split that differed, with one visible change.                 |
| Directory of a new Panel      | With `file://host/C:/Windows` and `file:///C:/Users` reported, new terminals opened in `C:\Windows` and `C:\Users`. With nothing reported, the command asked for no directory.                                                                       |
| Directory of New Workspace    | After `cd` into a Chinese-named folder with a space, the Server answered 14 ms after the window asked, the row appeared 56 ms after the click, and the shell started there. The TUI's `Ctrl+B a` took the shell's folder, not the Workspace's own.   |
| Panels on both Clients        | A split with the TUI's `Ctrl+B \|` reached the window in 156 ms from the key, and the window's focus stayed. A split and a divider from the window were on the TUI's screen at an 800 ms sample. The TUI's terminal opened read-only here in 307 ms. |
| Typing through a swap         | Keys 8 ms apart through five splits from another Client, two of its own and a Tab switch reached the shell whole and in order. A held Enter ran once, a held Tab completed, and keys held past a focus move or 1.3 s went nowhere.                   |
| Terminal that cannot start    | A Tab and a split given a missing executable by the CLI showed the page with its path, in English and Chinese. Three ordinary creations never showed it. Keys typed on the page after Ctrl+PageDown sent nothing.                                    |
| Leaving a failed terminal     | New Tab sent one `tab.create`, answered in 20 ms, and typing reached the new terminal. Close sent one `tab.close` for a lone Panel and one `panel.close` for a split's failed half, where typing in the other half stayed intact.                    |
| Burst past the command queue  | 2,000 keys at about 610 per second overflowed it. The terminal had the first 1,418 in order and nothing typed after the hole; the surface said it was restoring, took control on a new attachment 523 ms later, and typing worked.                   |
| View only                     | After the CLI took control at 60×12, the window drew that grid from its top-left and still acted as the controller until it typed. Those keys were refused and went nowhere, and View only appeared 2 ms later. At 200×50 the prompt was cut off.    |
| Take control                  | From the button, View only left 11–18 ms after the click, and the pane's size and the keyboard were back 14–19 ms later. From the menu, all three took up to 125 ms, the keyboard returning as the menu closed. Typing then reached the shell.       |
| The former controller         | The CLI received nothing until its next input, which was refused with `not-controller` and closed its stream.                                                                                                                                        |
| Empty Workspace               | After its last Tab closed, the content named the directory at the top left with a Ctrl+Shift+T New tab hint, 262–276 ms after Close, in English and Chinese. A 120-character path wrapped in a 405-pixel pane.                                       |
| New Tab from an empty one     | The keyboard was on the page, not in a control. Ctrl+Shift+T replaced the hint with a Tab in 29–32 ms, whose terminal had the keyboard 53–54 ms after the key, opened in the named directory, and took typing.                                       |
| Close of an untouched shell   | A shell that had taken no input still asked Close Tab? 1.5 s after it started, in English and Chinese.                                                                                                                                               |
| PowerShell Tab                | From the first strip sample, 9 ms after the Workspace opened, the Tab read `PowerShell`, with no path shown first, and Close Tab? said `PowerShell. Ends 1 terminal.`                                                                                |
| Command Prompt Tab            | Opened by the CLI, the Tab read `Command Prompt` within 2 ms while inactive. Running `ping -n 6 127.0.0.1` showed `Command Prompt - ping -n 6 127.0.0.1` 14 ms after Enter, and `Command Prompt` again once it ended.                                |
| Language change               | Choosing Chinese in Settings renamed the Command Prompt Tab within 60 ms, and English renamed it back within 25 ms. Typing after the change reached cmd, and Close Tab? named the Tab in Chinese.                                                    |
| Bell and OSC 52               | A bell's cue showed in the millisecond its record reached the page. An OSC 52 request reached the page's clipboard writer, stubbed for the run, 1 ms after its record. A second bell and a Chinese write followed, and no failure showed.            |
| Terminal link                 | Ctrl+click on a printed address reached Main's browser call, stubbed for the run, 5 ms after the click. No window opened, nothing reached the projection, and no failure showed. A plain click opened nothing.                                       |
| Reset                         | From the action search, the page sent `reset` 25 ms after the click and had `reset-accepted` 13 ms later. The screen emptied with no failure. Typing reached PowerShell, at the old prompt's place but without it, as reset is not PTY input.        |
| Window colors                 | Opening a terminal sent the light defaults 1 ms after control was granted, answered in 9 ms. A page-local switch of the system appearance to dark sent the dark ones 12 ms later, answered in 4 ms, and the terminal recolored with the window.      |
| Terminal display theme        | Choosing Dark in Settings repainted the open terminal and the space around it 27 ms after the change and sent the terminal nothing. Typing reached PowerShell. Host brought the light colors back 29 ms after the change, again sending nothing.     |
| Find in history               | Finding line 37 of 400 lines of output read the history and then the window at that absolute row, answered in 3 and 6 ms. The match showed centered 71 ms after the last key with no failure, and typing brought the bottom back in 32 ms.           |

### Evidence

- Each step records the owning Web and Desktop builds and a Server-backed run of the path it
  changed. While macOS Rust CI is excluded, macOS evidence is a recorded manual run; the manual
  native-window test stays excluded unless explicitly authorized.
- Step 4 removes the Desktop `test:native` step from the macOS and Windows native job once the addon
  is gone; Terminal Host tests stay. Linux `verify` keeps Xvfb while Desktop Electron tests run
  there.
- Steps that delete Desktop or Web inputs update `scripts/verification-scope.mjs` and the affected
  model triggers in `scripts/tla/models.json`, such as the projected-terminal and settings-save
  models.
- Rollback restores the previous complete Desktop build and native cohort.

### Single Agent integration writer

- Worker is the only node-local writer of provider configuration and Capsule installation. Server
  authorizes each operation; CLI, TUI, and Desktop Settings are Clients of the same operation.
- The Agent identity provider registry is the one provider vocabulary. Worker owns the integration
  specification, and the operation contract gets a language-neutral schema in place of the
  TypeScript Desktop contract.
- The Capsule source moves from `apps/desktop` to the owner of the native writer; Desktop packaging
  and native staging consume the same staged artifact, and `payloadSha256` remains its identity.
  The Capsule README is part of that payload, and its repository links and maintainer commands name
  the Desktop app, so the move rewrites it and changes the identity once. The move therefore lands
  with the Desktop writer's removal in step 4, whose README changes it would otherwise repeat.
- Until step 4, the Desktop TypeScript writer is frozen: no new providers, targets, or Capsule
  semantics. A shared hook-transport change updates the Capsule and both envelope parsers together,
  and users repair through `orchbee agent integration enable`.

### Cross-language parity sources

| Current source                                                                             | Target authority                                                              | Consumers                                              |
| ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- | ------------------------------------------------------ |
| `tokens.css` in `@orchbee/design-tokens`                                                   | One language-neutral palette, at the GUI design follow-up                     | CSS registry and native theme                          |
| Agent canonical corpus generated from `@orchbee/agent`, and the join corpus citing Desktop | Contract-authored corpora in [`schemas/agent`](../../schemas/agent/README.md) | Server, Worker, and TypeScript bindings until deletion |
| Integration vocabulary declared canonical in `@orchbee/desktop-client-protocol`            | A language-neutral integration operation schema                               | Worker, Server, native Client, and Desktop Settings    |

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
| Desktop retains no second Server, Worker, or terminal authority after migration                            | Module dependency scan, process-loss traces, authority-negative tests, project-contract updates, and removal of superseded composition roots                     |

The process/session/lease/reconnect model selects TLA+ because independently ordered Client,
Server, Worker, Host, transport, and persistence events can violate authority, continuity, or
at-most-once claims. Property tests cover identifiers, stale generations, topology cardinality,
and serializable transitions. Deterministic system tests cover at least one all-local Session and one
Session with two Workers and three concurrent Client forms. Performance evidence must measure input
latency, frame copies, relay memory, slow-client isolation, and startup cost; process-hop counts
alone are not performance evidence. Before the Rust CLI delivery slice is accepted, its evidence
plan must name the measured command and completion point, cold and warm conditions, artifact-size
scope, peak-memory metric, reference platforms, baselines, and acceptance-budget owner. Numerical
budgets may follow a bounded prototype, but the measurement contract may not be deferred until after
implementation.

## Accepted decisions from product review

1. One Worker generation admits at most 1,024 retained terminal bindings. Existing results remain
   available for exact replay and fencing; capacity rejects only new identities. There is no TTL,
   LRU, or implicit eviction. Explicit retirement is required before production.
2. A full-screen TUI submits immediate first creation only after obtaining complete real cell and
   pixel geometry. It never invents 80x24. Deferred Create plus complete Start remains valid for
   Clients that do not yet know real geometry.
3. TUI is a peer Workbench Client like Web, Desktop, and mobile. Shared document membership and
   ordering belong to Server; every Client owns an independent Client View. Bare TUI opens the
   standalone terminal context, creates its first terminal exactly once when empty, and uses normal
   Tab navigation rather than a startup picker. Closing the last Tab leaves an empty context.
4. Attachment means a transient display/control relationship to an existing retained terminal. It
   never creates, clones, restarts, or owns the terminal. Several viewers are allowed; one combined
   input-and-resize controller is acquired only when unowned, and release never auto-promotes or
   steals. Product copy says “open” or “connect”.
5. TUI exit detaches only, and no last-Client rule stops the Session. The first unshipped milestone has
   no durable Server adoption: Server loss closes Worker; Host may retain terminals; unmatched
   nonempty Host inventory yields `RecoveryRequired` with no automatic adoption, deletion, or
   replacement. Production recovery is a pre-release requirement.
6. Same-effective-user local Human Clients may read the authorized Workbench and submit ordinary
   terminal operations under Server policy. Terminals belong to Session/Workbench state, not their
   creator Client Session; a request identity is not a capability. Remote auth remains separate.
7. OBCS v1 is the Client-Server terminal contract. One TUI business intent lets Server coordinate
   Workbench mutation and terminal creation; Clients do not run two phases. **Superseded in part:**
   OBCS itself now owns attachment, controller, OBTF frames/ACKs, input, and resize, without
   exposing Host identities, capabilities, or raw PTY authority; no separate live-terminal protocol
   was added.
8. Bare/standalone launch starts in Worker user home; Workspace launch uses its Primary Directory;
   Current Directory uses validated same-Worker `currentCwd`. Bare invocation never infers caller
   cwd. Default shell resolution is valid absolute `$SHELL`, OS account shell, then `/bin/sh`, with
   no forced login arguments. Default environment is `TERM=xterm-256color`, `COLORTERM=truecolor`,
   and `TERM_PROGRAM=ghostty`, without old product or caller-transient variables.
9. TUI uses a full-screen alternate screen, Orchbee business sidebar, Workbench tabs/panels, and a
   real terminal, collapsing the sidebar on narrow layouts. Ratatui/Crossterm and the documented
   `Ctrl+B` prefix map are the initial implementation contract.
10. Bare `orchbee` is TTY-only, connects first, and uses one per-user activation lock when the Session
    is absent. Concurrent invocations converge on one Server/Worker cohort and each creates a TUI;
    Host stays lazy. Success is silent when fast, delayed startup has localized progress, one total
    monotonic deadline fails explicitly, and the Client never kills or downgrades an existing Session.
11. One Rust `orchbee` exposes bare/explicit TUI and the accepted resource-first command families in
    the CLI proposal. There is no public `orchbee server`, fuzzy abbreviation, old raw-session mode,
    or restored `--skill`; Session lifecycle is under `orchbee session`, and update behavior respects the
    installation manager. **Superseded in part:** the public `server` command family now manages the
    selected Session's background service, as listed in the
    [Client command surface](../../apps/orchbee/README.md#command-surface), and the
    [Glossary](../../docs/GLOSSARY.md#runtime-roles) keeps that family name instead of `session`. No
    later numbered decision replaces this item; its remaining clauses stand.
12. Human-readable localized output is default. Explicit machine output is stable versioned JSON;
    streams use NDJSON; stdout contains data and stderr contains diagnostics. Stable semantic exit
    categories are accepted while exact numeric values and schemas remain slice decisions.
13. CLI inputs are explicitly selected and bounded; secrets never use argv. Mutations carry one
    globally unique request identity, submit at most once, never retry unknown results, and require
    explicit noninteractive confirmation when destructive. Detached execution is only for durable,
    observable operations; completion is generated from the accepted grammar.
14. The old TypeScript CLI and its consumers have retired under the separately accepted sunset;
    source deletion does not wait for replacement packaging. Its grammar and output are not native
    compatibility targets. Future Skills are owner-versioned assets, and future packaged public
    cutover requires one authoritative entry and its own release evidence.
15. Standard packaging exposes `bin/orchbee` and private versioned libexec artifacts. Shell,
    Homebrew, and npm distribute one signed/digest-verified native cohort; npm platform packages do
    not require Node at runtime. macOS/Linux come first. Atomic side-by-side install and rollback,
    migration evidence, and measured budgets precede public cutover. **No longer applicable to the
    platform order:** Windows came first, with
    [complete-cohort release installation and updates](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation),
    while macOS and Linux have only
    [staged development cohorts](../../apps/orchbee/README.md#macos-and-linux-local-startup). No later
    numbered decision records that order, and this note changes no other clause.
16. Agent observation uses private one-shot Rust `orchbee-agent-bridge` from a versioned Capsule.
    Bridge is local-only, bounded, silent, fail-open, nonactivating, and stateless. Worker joins the
    acknowledged rich event and private correlation; Server owns canonical Agent semantics; Clients
    render. Observation precedes launch/orchestration, and content capture is off by default. The
    mixed TypeScript Agent package is decomposed and deleted only after migration.
17. Managed Worktree is an optional upstream capability. Server owns semantic intent/reference; an
    exact Worker capability owns Git, a private managed root, journal, compensation, and recovery.
    First migration covers create/discover/adopt and removal of Worktrees the Client created
    (**superseded in part** by the [migration plan](#web-and-desktop-client-migration-plan)), never
    infers creation, and gives adopted external paths no delete ownership. Terminal receives only an
    ordinary cwd.
18. Explicit Stop asks Worker to capture and atomically store one final platform-neutral terminal
    snapshot before Host disposal; Server owns archive metadata/index/auth. Detach does not archive,
    Close deletes Presentation plus archive, Start is fresh, and natural exit is not permanent
    archive. Initial bounds are 5,000 rows or 10 MiB per Presentation and 250 MiB per Worker.
19. Server owns the revisioned Worker/platform-scoped Launch Profile catalog; Worker validates and
    executes. A profile contains a name and either system-shell/login choice or an absolute
    executable with literal args—never cwd, appearance, environment, or secrets. Creation freezes
    the resolved revision; missing/stale/unauthorized/failure never falls back, and use/manage
    permissions differ.
20. “Connect via SSH” remains a product capability without generic Environment. Server owns
    `SshTarget`/`SshConnection` intent; a selected local Worker uses system OpenSSH and local
    config/agent/known-hosts plus ephemeral AskPass to start temporary remote Worker/Host. It is
    connection-scoped, never implicit enrollment, and initially created from local TUI/Desktop.
    Disconnect differs from terminal exit. The old SSH Runtime/protocol retire after migration.
21. Relay remains the first and fallback terminal path. Only measurements may enable a local native
    TUI/Desktop Client-to-Worker path with a short Server capability bound to all terminal, epoch,
    controller, and permission identities. Browser/mobile remain relayed; control stays on Server;
    authority loss stops input/resize and permits at most one bounded read-only drain; no Client
    reaches Host; fallback never replays unknown input.
22. Controller transfer keeps one input-and-resize owner. Acquire is explicit when unowned; an active
    owner receives normal requests; disconnect releases after observed liveness grace. No viewer is
    auto-promoted. A permitted second-confirmation forced take after timeout atomically advances the
    Server generation/fence and rejects old input. Collaborative and split control stay excluded.
23. Lazy service start is default. `orchbee session enable` installs per-user LaunchAgent/systemd-user
    Server and Worker units from one cohort with bounded backoff; no root daemon or supervisor is
    added and Host stays lazy. Restart preserves retained terminals; stop reports retained Host
    terminals; terminate-all is separate and destructive. Disable/uninstall/upgrade preserve data
    and terminals by default.
24. A signed manifest and side-by-side version directories define one atomic cohort. Compatibility
    is an exact tested set. Live Hosts are never replaced; incompatible Host updates wait for their
    terminals; referenced Capsules remain. Failed cohort or database migration rolls back safely,
    package-manager installs update through their manager, and auto-install is off by default.
25. Production Server state uses a transactional SQLite/WAL design and persists canonical product
    intent, revisions, topology, ledgers, profiles, references, and metadata—not Client View, live
    attachments, frames, PTY/scrollback, transient secrets/env, raw hooks, or default full Agent
    history. Intent precedes effect; restart reconciles exact identities without replay; unknown Host
    inventory quarantines; corrupt/incompatible storage opens read-only recovery; credentials stay
    in the OS store and Worker-owned archives/journals remain node-local. **Superseded in part:**
    Server currently persists through its
    [Workbench checkpoint](../../apps/orchbee-server/docs/workbench-checkpoint.md); SQLite has not
    been adopted.

## Reserved decisions for review

1. Define remote Worker enrollment, production human/device identity, remote Session authentication,
   TLS and Internet ingress, discovery/relay/NAT traversal, multi-user tenancy, authorization
   hierarchy, and mobile background/push behavior.
2. Select exact JSON/NDJSON schemas and numeric CLI exit codes, remote CLI targeting, first-release
   command subset, detailed TUI visual/accessibility/mouse behavior, installed Skill catalog, and
   configurable shortcut policy within the accepted command/TUI boundaries.
3. Select exact controller liveness/grace durations, activation deadlines, service unit mechanics,
   relay/direct-path thresholds, queue and performance budgets, archive retention values beyond the
   accepted initial bounds, and platform-specific resource limits from measurements.
4. Define Agent launch/orchestration, provider-native question or approval response, durable Agent
   history/content, remote automatic integration installation, and mobile notification delivery;
   none follows from accepted observation.
5. Define destructive managed-Worktree operations, archive export/synchronization, broader Launch
   Profile capabilities, and remote SSH credential UX beyond the accepted first migrations.
6. Define later Workbench schemas, broader mutating intents, fine-grained authorization/redaction,
   and projection domains beyond the selected local-terminal and CLI slices.
7. Decide Desktop's long-term shell technology separately from its accepted Client-only authority,
   and decide Windows service, packaging, terminal, and credential adapters before Windows support.
   **No longer applicable to Windows:** Windows support has been delivered; the
   [Windows quick start](../../apps/orchbee/README.md#native-windows-quick-start) and the
   [TUI platform boundary](../../apps/orchbee/docs/tui-platforms.md) own its current adapters and
   limits. No later numbered decision records these Windows adapter choices; the Desktop shell
   decision remains reserved.
8. Authorize each remaining Workbench-package, Agent-package, and Desktop-authority deletion
   only after its consumer, installed-artifact, behavior, migration or
   sunset, documentation, evidence, and rollback audit.
9. Authorize public cutover only after the accepted persistence, recovery, update, migration,
   packaging, security, and measured-performance requirements are implemented and verified.

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
- Does the native command surface preserve only deliberately selected semantic capabilities?
- Can the current SSH Runtime and protocol be removed as one complete legacy path without implying
  that either their implementation contracts or a generic Environment domain survive?
- Can Web, TUI, mobile, and CLI consume one semantic Workbench projection contract without sharing
  React, DOM, pixels, or Client-local focus state?
- Are migration, compatibility, rollback, and evidence obligations small enough to deliver in
  independently reviewable slices?
