# Orchbee Server, Connected Clients, and Workers

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Accepted direction; the Linux local native Tab development path is integrated but unpackaged: bare invocation, exact cohort, lazy startup, retained terminals, shared Tab creation, independent selection, and explicit refresh have implementation evidence. Current Browser/Desktop/TypeScript CLI remain shipped authorities; remote is deferred, with full Workbench, installation/update/recovery, cross-platform acceptance, and migration still pending.             |
| Owner              | Product architecture, Server coordination, Workbench, network ingress, Worker placement, remote-connection adapters, CLI, Agent, and future capability-node maintainers                                                                                                                                                                                                                                                                                                     |
| Current authority  | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) and the owning Desktop, Gateway, Web, and Terminal Host guides                                                                                                                                                                                                                                                                                                                                                         |
| Decision authority | The product owner accepted the uniform peer-Client model, separate public Rust `orchbee` and private Server/Worker/Terminal Host plus optional Rust Agent Bridge roles, Server-relayed first terminal path, bounded lazy activation, accepted local lifecycle and capability refinements in the native topology, and the target dispositions below; exact remote enrollment, production identity, TLS/Internet, multi-user, and mobile-background contracts remain reserved |
| Exit condition     | Accept and implement bounded owner-specific slices, move maintained facts into their owning guides and tests, then delete proposal                                                                                                                                                                                                                                                                                                                                          |

Simplified Chinese review translation:
[connected-client-capability-service.zh-CN.md](connected-client-capability-service.zh-CN.md).
English remains canonical.

## Product direction

An Orchbee Site is a product-state and coordination boundary with one active Server authority. It
may serve several machines and Client instances. A Client-only installation connects to a Site
without creating another Site or becoming a Worker. The product must eventually support:

- Desktop, Browser, TUI, mobile, and CLI clients that observe or control only their authorized product
  sessions;
- a comparatively weak work computer acting only as a product client for a remote site;
- another enrolled computer contributing typed compute or device capabilities; and
- the coordinating computer contributing local capabilities while also coordinating other nodes.

The externally reachable service is therefore an application backend, not only a static-file
server or reverse proxy. Static Web delivery, TLS termination, and request forwarding may be
deployment responsibilities, but they cannot own sessions, authorization, device enrollment,
capability leases, scheduling, or product state.

This direction does not turn the current local-development `apps/gateway` or the private
same-user Terminal Host endpoint into a network service. Those current boundaries remain in force
until separately accepted replacements or adapters exist. The target deletes `apps/gateway` and
`@orchbee/gateway-protocol` after Server assumes their supported Browser-session, terminal-relay,
and backpressure behavior; their framing and process split are not compatibility targets.

## Product outcomes and acceptance boundaries

The user should be able to leave work running on a capable machine and use an authorized Client
to return to that same work. Supporting additional execution machines is a separate outcome from
accessing one machine remotely. Neither outcome requires every Client form to ship together.

| User scenario                                  | Observable acceptance                                                                                                                                                                                             | Boundary                                                                                                             |
| ---------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| Use a weak computer to reach a capable machine | An authenticated Client opens the selected Site's existing Workbench and retained terminal, receives real output, and may request control. It requires no local Worker or source checkout.                        | Connected-Client outcome; production network identity and transport still need their own accepted slice.             |
| Open the same work in several Clients          | Clients observe the same authorized shared structure and terminal identity while retaining independent focus and viewport. Several viewers do not create another terminal or another input-and-resize controller. | Local-terminal evidence starts with peer TUI Sessions; Web/Desktop migration proves the shared cross-Client outcome. |
| Close a Client or lose its connection          | Retained execution continues when its Host remains healthy; a later authorized attachment returns to the same terminal. Unknown input is not replayed.                                                            | Client/transport continuity does not promise recovery after Host failure or OS restart.                              |
| Run without an Electron window                 | The local Site and retained terminal lifetime are independent of any Desktop or TUI Client.                                                                                                                       | Accepted local-native direction; successful process handshakes alone do not establish it.                            |
| Contribute another machine's capability        | A separately enrolled Worker executes one explicitly admitted kind of work, with observable admission, result, failure, and revocation behavior.                                                                  | A later distributed-capability vertical; enrollment alone does not establish useful scheduling.                      |

Remote viewing and control need a Server and an execution Worker at the selected Site; they do not
require remote Worker enrollment, automatic placement, source synchronization, or a general Agent
scheduler. A shared Project identity also does not make files available on another Worker or make a
live terminal movable between machines.

## Milestone and release boundaries

The initial accepted engineering milestone is the
[local terminal vertical](native-runtime-topology.md#accepted-local-terminal-vertical-milestone):
bare `orchbee` reaches a real retained shell through Client, Server, Worker, and Host; a second
invocation joins the same Site; output, input, resize, detach, and reattach work through that path.
Its process, fault, and performance evidence must exercise the composed runtime. Models, codecs,
handshakes, and staged UI modules are supporting evidence, not completion of this user outcome.

The integrated local development path also supports shared Tab creation, canonical Move, independent
selection, and explicit refresh across native Clients. Current interaction and process-evidence
boundaries belong to the [native Tab guide](../../apps/orchbee/README.md#native-tab-interactions).
Native Close-to-Empty, manual Create and retained-shell reentry also have bounded Linux process
evidence in that guide. The remaining Workbench operations still require their own delivery slices.

The [Workspace directory slice](../../apps/orchbee-server/docs/workspace-terminal-directory.md)
adds Project-owned Directories, an explicit first association after Worker validation, and Workspace
Tab creation using the accepted directory and exact Worker.
[Native context selection](../../apps/orchbee/docs/native-context-selection.md) connects the Linux
TUI to existing Workspaces and their Tab operations.
[Native Workspace setup](../../apps/orchbee/docs/native-workspace-setup.md) adds staged Project and
Workspace creation and explicit first-directory configuration; terminal launch remains a separate
explicit action. Desktop migration remains separate work.

This milestone is additive and unshipped. It does not prove production restart recovery, remote
access, or migration of the existing Desktop. In particular, its accepted `RecoveryRequired`
behavior for unmatched retained Host inventory is a development boundary, not the production
continuity experience. Public cutover still requires the recovery, migration, packaging, and
performance obligations in the native topology.

The recommended subsequent product validation is one person using a remote Web or Desktop Client
against one Site and its local Worker. This would test the connected-workbench value before adding
enrolled remote Workers or scheduling. The choice of first remote Client, supported network and
identity contract, and its order relative to other migrations remains a product decision; this
recommendation does not accept those contracts or expand the current local milestone.

## Accepted distributed-workbench topology

The accepted logical topology has three independently authorized roles:

| Role   | Authority                                                                                                                                                                                               |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Client | Renders one authorized product projection, submits semantic commands, and owns client-local view and platform effects; it owns no site-durable product state                                            |
| Server | Owns canonical Workbench state, authorized semantic-command commitment, Agent orchestration, Worker registry and capability placement, and durable recovery                                             |
| Worker | Owns only its installed node-local capabilities and effects; the terminal baseline owns launch admission and Host brokering, while Files, Checkout, Git, and Agent effects remain optional capabilities |

One machine may colocate Client, Server, and Worker roles, but colocation never merges their
identities, authority, state, or lifecycle. A weak work computer may run only a Client. A central
machine may run Server and one Worker while coordinating additional Workers. **Terminal Host**
continues to name the node-local PTY service; it never names the central Server.

TUI, Web, Desktop, mobile, separate Browser tabs, separate Desktop windows, and repeated native
invocations are peer Client implementations. Each active instance owns one Client Session and its
local focus, active location, viewport, scroll, and presentation state. Shared Workspace, Tab,
Panel, and Terminal facts come from Server. Renderer form never grants additional authority.

A Project is a host-independent product goal. A Checkout is an optional concrete source tree on one
exact Worker or, while the compatibility path remains, behind one SSH Environment adapter. A
Workspace may select a primary Checkout or Directory as a feature-owned starting-directory
suggestion, but neither Project nor Workspace identity is a prerequisite for terminal creation or
an independent source of Worker placement. The accepted relationship does not yet define a durable
schema, migration, cross-machine source-transfer policy, or whether `Directory` is renamed or
adapted internally; those decisions require a bounded Workbench proposal.

The accepted local [directory contract](../../apps/orchbee-server/docs/workspace-terminal-directory.md)
now chooses a Project-owned Directory and nullable Workspace primary reference for the unpublished
in-memory native cohort. This closes that local representation and first-association decision;
durable schema migration, general Directory CRUD and cross-machine transfer remain outside it.

The accepted native refinement in
[`native-runtime-topology.md`](native-runtime-topology.md) supersedes the former rule that every
execution effect must resolve through a generic Environment. Future native terminal creation
resolves directly through one Server-authorized Worker; Project, Checkout, Worktree, and Environment
identity stay outside the Terminal Host boundary. Delivered SSH Environment records and remote
Workspace behavior remain current authority until a separately accepted migration or product
sunset. The current `apps/ssh-environment-runtime` and `@orchbee/ssh-environment-protocol` are
accepted retirement candidates rather than target architecture assets. Any future SSH adapter is a
new Server/Worker-aligned design and may arrive after the old path is deliberately removed; it need
not preserve the current executable, package, wire, services, release cohort, or Desktop-owned
composition. Until cutover, the current unmanaged-SSH path may continue resolving its exact legacy
Environment adapter before reaching its Runtime without restoring Environment as a universal
product domain.

The Workbench product domain is shared; its renderer is not. Server owns canonical Workbench
transitions and publishes framework-neutral, versioned semantic snapshots and deltas. Browser and
Desktop use the one React implementation owned by `apps/web`; Rust TUI and future mobile Clients
own separate presentation implementations. CLI may return a bounded structured semantic projection
snapshot, never HTML, a React tree, pixels, or another Client's local view. The current mixed
`@orchbee/workbench` package is decomposed along that boundary and deleted after migration.

## Accepted semantic-control direction

Human controls, an Orchbee CLI, Skills, and approved automation must reach the same owner-defined
semantic operations instead of implementing parallel mutations. A CLI is a versioned invocation
surface, not a product-state Store or an authority boundary; replacing an experimental Client may
redesign its grammar without changing the semantic owner's contract. Skills compose structured CLI
operations and never parse human presentation text or simulate pointer, DOM, or keyboard input when
an eligible semantic operation exists.

A read-only CLI operation may expose an authorized, versioned Workbench semantic projection
snapshot for automation. That snapshot is a Server product projection, not a serialization of any
Web, TUI, mobile, or Desktop renderer.

The accepted Rust replacement direction and its deliberately redesigned public grammar are owned by
[`native-command-surface.md`](../cli/native-command-surface.md).

Current in-process Action registrations and prepared invocations remain local runtime capabilities.
They are not serializable network requests and do not become remotely invocable through
enumeration. Each operation admitted to CLI or remote use requires an explicit versioned input and
safe output schema, caller authorization, noninteractive confirmation policy, compatibility
lifecycle, idempotency boundary, and owning domain handler. Client-local operations such as focus,
window geometry, native dialogs, and clipboard access require an exact authorized Client Session;
they never mutate every connected client's view implicitly.

The durable ownership split is:

| State or effect                                                                  | Owner                                      |
| -------------------------------------------------------------------------------- | ------------------------------------------ |
| Project, Workspace, shared Tab/Panel structure, Agent task, and scheduling state | Server product coordinator                 |
| Worker registration, capability placement, operation authorization, and fencing  | Server product coordinator                 |
| Terminal launch admission                                                        | Exact Worker                               |
| PTY, process tree, and terminal state                                            | Exact Terminal Host                        |
| Optional Checkout, file, Git, and Agent-executor effects                         | Exact separately admitted capability owner |
| Active location, selection, focus, maximization, window geometry, and clipboard  | Exact Client Session or client platform    |

## Accepted names and contract timing

The target backend role is **Server** and its private native executable is `orchbee-server`, owned
by `apps/orchbee-server`. The existing Node `apps/server` / `@orchbee/server` workspace is a
delivered Browser-ingress preview, not the target native process; it remains current authority only
until supported behavior migrates. `Server` names the complete product backend role without
collapsing ingress, authentication, coordination, device identity, scheduling, and execution into
one undifferentiated module.

The human-Client and Worker communication boundaries stay distinct and are materialized only with
concrete consumers:

| Package                                                     | Creation trigger and responsibility                                                                                                                                                                                                                                                                         |
| ----------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `packages/remote-client-protocol`                           | Created by the accepted human Browser authentication slice. Currently owns Personal Access Token exchange and signed Cookie session-establishment contracts; broader production transport, negotiation, and authorization remain reserved.                                                                  |
| `schemas/client-server` and `crates/client-server-protocol` | Existing experimental OBCS Create/Start intent and result contract for the local terminal vertical. Local admission is separate; this is not a complete live-terminal or production remote-Client protocol.                                                                                                 |
| `crates/capability-node-protocol`                           | Existing experimental OBCN authorized terminal Launch/Start values and codec for Server-Worker integration. It does not provide enrollment or a general scheduler; its exact current authority is its owning guide.                                                                                         |
| Future remote extensions                                    | Add only contracts required by an accepted remote Client or concrete remote Worker capability. Production identity, enrollment, capability advertisement, progress, cancellation, data transfer, and compatibility must be specified for that consumer; none follows merely from the existing local codecs. |

The Client and Worker boundaries remain separate because a human product client and a Worker have
different identities, credentials, authorities, revocation scopes, and compatibility lifecycles.
Neither boundary is a Server implementation, owns product state, or grants a generic remote shell.
Current local contracts are maintained by the
[Client-Server schema](../../schemas/client-server/README.md),
[Client-Server binding](../../crates/client-server-protocol/README.md), and
[Capability Node guide](../../crates/capability-node-protocol/README.md).

The current `apps/gateway` and `@orchbee/gateway-protocol` remain explicitly preview-only Browser
transport until a production external-client slice migrates and deletes them. They may bind loopback
or an explicitly selected trusted-LAN interface. `apps/server` exchanges the Personal Access Token
for a signed session Cookie, and Gateway admits the exact-Origin control connection with that same
session; no credential appears in the access URL. They must not be renamed to `server` or treated as
the production external-client contract. The production migration must avoid maintaining two
competing external-client network contracts indefinitely.

## Accepted connection strategy

Orchbee supports two complementary connection families:

| Connection family     | Responsibility                                                                                                                                                                   |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native site           | HTTPS establishes authenticated human or device sessions; WSS carries versioned control, event, progress, and bounded binary-projection messages through role-specific contracts |
| SSH bootstrap/adapter | Connects an unmanaged machine, validates its host identity, bootstraps or repairs an Orchbee runtime, and may tunnel native protocol traffic during an explicitly bounded phase  |

This connection family names a product capability, not a commitment to retain the current SSH
Environment Runtime or protocol. Their accepted retirement may deliberately precede a replacement
adapter when the existing product path is explicitly sunset.

Native site connectivity is the long-term primary path for authenticated Clients and enrolled
Workers. The accepted Connect-via-SSH path remains a first-use, compatibility, administration, and
temporary-connection path; it is not the Workbench, Agent, scheduling, or state-synchronization
protocol. An enrolled Worker normally establishes its own outbound native connection to Server with
a non-Cookie device credential; the SSH-attached temporary Worker is connection-scoped and is never
promoted to enrollment implicitly.

An explicitly user-authorized SSH Project may provide terminal and filesystem effects within that
SSH account. This does not enroll the machine automatically, turn the capability-node contract into
a generic remote shell, or grant scheduled work any authority beyond its accepted typed capability.

Desktop and Browser clients consume the same authorized product model through the remote-client
boundary. Capability nodes use their separate node boundary even when both connection roles use the same
TLS listener and WebSocket implementation. HTTP and WebSocket framing do not define product
identity, authority, retry, or compatibility semantics. The application contracts remain
transport-neutral so an accepted SSH tunnel, relay, or later transport adapter cannot create a
second product model.

Production network traffic uses authenticated encryption supplied by established TLS and SSH
implementations; Orchbee does not invent transport cryptography. Plain HTTP remains restricted to
the explicitly warned trusted-LAN preview already delivered. The measured local-native
Client-to-Worker path is accepted only under the scoped Server-capability contract below; remote
direct paths, QUIC or WebTransport adapters, relay service, and Internet ingress remain reserved
optimizations or deployment decisions. None may bypass Server-issued authority or recovery fencing.

## Accepted initial ingress foundation

The delivered Node preview's HTTP routes, Fastify composition, session exchange, listener defaults,
and static-resource behavior belong to the [Server guide](../../apps/server/README.md).
Its Desktop-owned activation, exact-Origin Browser admission, terminal relay, and backpressure
belong to the [Gateway guide](../../apps/gateway/README.md). These are migration inputs, not a
requirement to recreate a second Node foundation beside the native Server.

The trusted-LAN HTTP preview remains an explicitly enabled and warned exception to the production
encrypted-transport direction. It establishes neither production Internet deployment nor
Desktop-independent Site lifetime. Starting the standalone Server entry does not enable sharing.

## Human Web access direction

Network exposure remains off by default. Enabling sharing starts the accepted site ingress on an
explicit interface and port; the shareable entry is the short origin, such as
`https://192.0.2.1:8443/`, followed only by normal product routes. Credentials, bootstrap tokens,
and session capabilities never belong in the shared query string or fragment.

An unauthenticated Browser request retains its intended product path while entering a login flow.
The person enters a separately issued access token; a successful exchange creates a bounded,
revocable human session represented by an appropriately scoped Cookie, then resumes the intended
route. This human login flow never enrolls a capability node and never grants device credentials.

The delivered preview's token and Cookie implementation remains owned by the Server and Gateway
guides linked above. Production session duration and revocation storage, TLS deployment, multi-user
authorization, and account recovery remain reserved; preview authentication must not be presented
as acceptance of those production contracts.

## Accepted connection-continuity model

A network connection is a replaceable transport attachment, not the lifetime owner of a Project,
Workspace, Checkout, retained terminal, Agent task, or durable product session. An SSH, WebSocket,
or underlying network failure moves the affected attachment through an explicit disconnected,
reconnecting, and recovering lifecycle. It does not request deletion, cancellation, or process
termination.

Reconnect establishes a fresh authenticated transport and reconciles stable site, Client Session,
Node Session, Workbench revision, terminal inventory, Agent execution, and event-sequence identities.
The authority returns a bounded missing-event range or a fresh snapshot when incremental recovery is
unsafe or unavailable. Safe semantic commands use owner-defined operation identities and
idempotency rules. Terminal input or another effect with an unknown admission outcome is never
replayed automatically.

Clients and nodes use bounded randomized retry with increasing backoff, expose offline and recovery
state, and stop retrying when identity, authorization, host verification, compatibility, or policy
requires user or administrator action. Authentication convenience never weakens host-key or TLS
identity validation. Key- or agent-backed SSH and enrolled device credentials are the preferred
unattended recovery mechanisms; password-only SSH may require renewed human authentication after a
transport loss.

Transport loss, remote runtime failure, and remote-machine restart remain distinct failure classes.
The first preserves remote execution, the second reconciles against independently retained owners
where available, and the third may require explicit task restart because an operating-system reboot
cannot preserve ordinary PTYs or processes.

These are target continuity requirements. The first local-native milestone has no durable Server
adoption, and the current preview has its own narrower lifecycle. Retained execution, restored
observation, and restored control must each be demonstrated at the boundary being delivered;
preserving a Host process alone does not prove that a successor Server can safely resume control.

## Accepted native-runtime refinements

[`native-runtime-topology.md`](native-runtime-topology.md) is the canonical physical-process and
local-lifecycle refinement of this logical design. Product review has additionally accepted:

| Area                     | Accepted direction                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Client and terminal use  | Every TUI invocation is an independent peer Client View over one shared Workbench document. Display attachment is transient and several Clients may view one retained terminal. One combined input-and-resize controller remains Server-fenced; explicit request, disconnect grace, and second-confirmation forced take define transfer, with no automatic promotion or collaborative input.                                          |
| Optional capabilities    | Private Rust Agent Bridge feeds bounded local observation through Worker to Server-owned Agent semantics; managed Worktree Git effects and Archive content remain Worker-local; Server owns their canonical references/metadata plus the revisioned Launch Profile catalog; and each Client renders its own projection. None is required for a terminal-only Site.                                                                    |
| Connect via SSH          | Generic Environment is removed from the target. Server owns `SshTarget`/`SshConnection` intent, while a selected local Worker uses system OpenSSH, local host verification and credentials, and ephemeral AskPass to start a connection-scoped temporary remote Worker/Host. It is distinct from enrolled Workers, initially created from local TUI/Desktop, and replaces the old SSH Runtime/protocol only after migration evidence. |
| Terminal path            | Server relay is the first and fallback route. Only measured need may enable a local native TUI/Desktop Client-to-Worker route under a short Server-minted, identity- and generation-bound capability. Browser/mobile stay relayed, no Client reaches Host directly, Server keeps control authority, and fallback never replays unknown input.                                                                                         |
| Local Site lifecycle     | Lazy activation is default. An explicit `orchbee site enable` may install same-cohort per-user LaunchAgent/systemd-user Server and Worker units; Host remains lazy and no root daemon or extra supervisor exists. Restart preserves retained Host terminals; stop reports them and does not imply terminate-all. Signed side-by-side cohorts update atomically without replacing a live Host.                                         |
| Persistence and recovery | Production Server uses a transactional SQLite/WAL design for canonical intent, topology, ledgers, profiles, and references—not Client View, live attachments, terminal frames, PTY/scrollback, transient secrets, or raw hooks. Intent precedes effects; restart reconciles exact identities without replay. Unknown Host inventory is quarantined, and corrupt/incompatible storage opens read-only recovery rather than resetting.  |

These are accepted future boundaries, not claims about the delivered Node Browser preview or
Desktop-co-located runtime. Exact remote Worker enrollment, production human/device identity,
TLS/Internet ingress, multi-user tenancy, and mobile background behavior remain undecided.

## Candidate responsibility split

These are semantic responsibilities within the accepted roles. A row does not require an additional
process, workspace, framework, or independently deployed service. The native topology owns the
executable split; a concrete consumer and its behavior determine any further module boundary.

| Responsibility                                       | Candidate owner                                                                                                                                                 |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| HTTP/WebSocket ingress and Web UI                    | A network ingress service; TLS, origin policy, routing fallback, rate limits, and static renderer delivery                                                      |
| Human authentication                                 | A site identity/session service; bootstrap token exchange, secure Cookie session, expiry, revocation, and audit                                                 |
| Product coordination                                 | The Server application coordinator; authorized Workbench sessions, canonical state, semantic commitment, and recovery                                           |
| Worker placement and optional source-tree references | Server product coordinator; exact Worker identity is native placement, while legacy SSH Environment identity exists only until current-path migration or sunset |
| Capability-node enrollment                           | A device identity service; pairing, device credentials, attestation inputs, rotation, revocation, and presence                                                  |
| Work scheduling                                      | A typed capability scheduler; admission, placement, lease, cancellation, retry, quota, and result ownership                                                     |
| Local terminal execution                             | Terminal Host behind its existing trusted local broker boundary                                                                                                 |
| Remote execution                                     | A separately authenticated node runtime exposing only accepted versioned capability contracts                                                                   |
| CLI and Skill invocation                             | A narrow semantic-command client; structured invocation and observation without product-state ownership                                                         |
| Desktop, Browser, and mobile projection              | Product clients consuming authorized application APIs; client form factor grants no backend authority automatically                                             |

The logical Client-Server-Worker topology and native physical baseline are accepted: public Rust
`orchbee`, private Rust Server, private Rust Worker, separate Zig Terminal Host, and optional private
Rust Agent Bridge. Their additive foundations and first local Server-Worker process spine are
delivered but unshipped, and the Linux local development vertical is integrated; the current
Desktop-colocated runtime stays authoritative until bounded migration slices replace it. The first
complete local terminal path relays through Server, while bare `orchbee` may lazily activate the
exact local cohort without supervising its lifetime. Colocation never merges trust, state, or
lifecycle responsibilities. User-service installation and the direct-path contract are accepted
above but remain gated implementation work; remote enrollment remains reserved.

## Connection roles

A connection has one explicit authenticated role. Device type, user agent, IP address, and local
network presence never grant a role.

| Role                 | Authority                                                                                                  |
| -------------------- | ---------------------------------------------------------------------------------------------------------- |
| Human product client | Authorized read and semantic command access for selected product resources                                 |
| Capability node      | Advertise accepted capability versions, accept scoped leases, report progress, and publish bounded results |
| Site administrator   | Enroll and revoke devices, approve capabilities, set policy, and inspect security-relevant audit records   |

A device may hold more than one role through separate credentials and scoped sessions. Browser
Cookies authenticate human sessions; capability nodes require non-Cookie device credentials with
rotation and revocation. A browser token must never become a node credential.

## Capability execution model

Remote capacity is exposed as typed capabilities rather than generic remote command execution.
Every accepted remote capability contract must define:

- versioned input, progress, result, failure, cancellation, and compatibility schemas;
- resource requirements and scheduler-visible capacity without exposing unrelated device data;
- lease identity, expiry, heartbeat, duplicate-delivery behavior, and idempotency boundary;
- artifact, source-data, secret, log, and diagnostic transfer policy;
- disconnect, reassignment, partial-result, and late-result behavior; and
- the product owner that authorizes work and commits its durable outcome.

The local terminal baseline needs its concrete launch and live-terminal contracts. This list does
not require adding a general task queue, scheduler, artifact service, or remote enrollment before
that baseline or before a human Client can use a Site remotely. A new capability must name actual
work, its required node-local data and credentials, and its completion evidence before its generic
infrastructure is admitted. Existing terminal retention never implies live migration or automatic
reassignment of terminal input to another Worker.

The site coordinator owns placement policy; a capability node owns execution of an accepted lease.
Neither a node nor a network adapter becomes a writer of Workbench shared state. Product Actions
remain owner-internal semantic contracts. A separately reviewed CLI or remote API may expose only
an explicitly automation-admitted operation through a serializable boundary; it never serializes a
prepared invocation or grants registry-wide authority.

## Deployment shapes

| Shape                           | Required behavior                                                                                                                                   |
| ------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| Single computer                 | Client, Server, local Worker capabilities, and optional Web ingress may be colocated                                                                |
| Weak remote work computer       | A Desktop, Browser, mobile, or CLI Client renders and controls one authorized remote Server without becoming a Worker                               |
| Current SSH Environment         | Until migration or sunset, the active product coordinator bootstraps and attaches one unmanaged remote Project environment without inventing a Site |
| Distributed site                | One Server coordinates its local Worker and additional enrolled Workers through explicit identities and capabilities                                |
| Coordinator temporarily offline | Clients and nodes fail closed or enter an explicitly modeled degraded mode; neither invents authority                                               |

LAN discovery, Internet relay, NAT traversal, public DNS, and cloud rendezvous are transport and
deployment choices, not product authorization mechanisms.

## Invariants

- Network exposure is off by default until an authorized user enables and configures it.
- Every externally supplied message is parsed at its trust boundary and authorized after
  authentication; possession of an IP address and port grants nothing.
- Human sessions, device identity, capability leases, terminal attachments, and Workbench client
  sessions use distinct identities and revocation scopes.
- Server, Worker, and Client Session state have one accepted authority each; transport adapters and
  colocation never create another writer.
- A Project is independent of machine identity. Native terminal effects resolve through one exact
  Server-authorized Worker without requiring Project, Checkout, Worktree, or Environment identity.
  A concrete Checkout resolves to its exact Worker, while a retained unmanaged-SSH path may first
  resolve its explicit Environment adapter.
- Terminal Host retains its private local transport and never accepts an Internet or LAN peer
  directly under the current contract.
- SSH and native site connections are replaceable transport attachments. Their loss never implies
  deletion of a Project or Workspace, termination of retained remote work, or cancellation of an
  Agent task.
- Recovery reconciles stable identities and revisions. It never replays terminal input or another
  effect whose admission outcome is unknown.
- Remote execution never receives a general Desktop, filesystem, shell, secret, or Action
  capability merely because one accepted capability needs a narrower effect.
- CLI, Skills, and remote clients invoke only explicitly admitted semantic operations and cannot
  bypass feature-owned validation, authorization, confirmation, commit, or concurrency policy.
- Disconnect and retry cannot commit one logical result twice or transfer a lease to two active
  owners without an explicitly modeled fencing mechanism.
- Browser and Electron consume the `apps/web` React renderer, while TUI and mobile implement their
  own presentations over the same authorized product semantics; transport choice never creates a
  second canonical Workbench authority.
- Audit records contain identities, decisions, and bounded metadata, never terminal content,
  credentials, capability tokens, or secret payloads by default.

## Reserved decisions

The accepted local-terminal milestone does not settle these later choices:

1. Agent launching/orchestration, provider-native responses, durable content/history, automatic
   remote integration installation, and mobile notification delivery beyond the accepted
   observation topology.
2. Destructive managed-Worktree operations, Archive export/synchronization and longer-term
   retention, broader Launch Profile capabilities, and remote SSH credential UX beyond the accepted
   first migrations.
3. Production human and device sessions, Worker enrollment, remote capability contracts,
   scheduling, leases, fencing, cancellation, quota, and data-locality policy.
4. Production HTTPS/WSS, TLS and Internet ingress, relay or NAT traversal, mobile backgrounding,
   multi-user tenancy, authorization hierarchy, audit, secrets, and artifact transfer.
5. Exact service units, activation/grace deadlines, performance thresholds, numeric limits, database
   schema/migrations, backup UI, and package-manager adapter mechanics inside the accepted local
   lifecycle contracts.
6. Remote CLI targeting, exact machine schemas and numeric exits, detailed TUI presentation,
   installed Skill catalog, and the exact first public command cohort inside the accepted command
   architecture.
7. Each deletion of the Node Server preview, Gateway/protocol, SSH Runtime/protocol, mixed Workbench
   package, old CLI, or migrated Desktop authority. Deletion requires its own consumer,
   installed-artifact, behavior, migration or sunset, documentation, evidence, and rollback audit.

Do not create an empty remote Worker protocol workspace or broaden a current preview protocol
without a concrete accepted vertical consumer. The current Browser-ingress and SSH paths remain
authoritative only for their delivered behavior until a complete replacement or explicit sunset is
accepted and verified.

## Delivery sequence

Current delivered behavior belongs to its owning application guides and executable tests rather
than a phase ledger in this proposal.

| Milestone                          | Independently reviewable outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Existing current paths             | Keep the Node Browser-ingress preview, current TypeScript CLI, Desktop-colocated coordinator, and delivered SSH path authoritative for their supported behavior while additive native work remains unshipped.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Native foundations                 | **Complete and unshipped:** root Rust support, framework-neutral Workbench projection contracts, native `orchbee` entry foundation, Rust Server kernel, and Rust Terminal Host binding. [`native-runtime-topology.md`](native-runtime-topology.md) links their current owners.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Local terminal vertical            | **Linux development path integrated, unpackaged:** bare invocation validates inherited TTY, the exact four-artifact cohort, and private runtime, connects first, and exclusively commits one Server attempt when needed. Server holds the lifetime guard and starts one Worker; Worker starts Host only after the first authorized Launch. Open, viewer, controller, and first frame share the Client startup deadline. Real PTY process tests cover two cold Clients, controlling/read-only roles, retained shell PID and variables after close/reopen, invalid cohort, failed marker, lock-contention deadline, and no replacement after Server/Worker/Host loss. The supported development boundary is verified Linux ext4/tmpfs with real cell/pixel geometry; macOS/Windows public startup, full Workbench, terminal interaction-mode parity, automatic recovery, installation/update, and release performance budgets remain undelivered. Handshake readiness is not first-frame readiness; this does not replace shipped entry points. Current contracts: [Client](../../apps/orchbee/README.md) and [shared runtime](../../crates/local-site-runtime/README.md). |
| Shared Client and Server migration | Move canonical Workbench coordination and supported Browser session/relay behavior into Rust Server, move React presentation into `apps/web`, reduce Desktop to a Client/platform adapter, and migrate only explicitly admitted CLI capabilities.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Accepted optional capabilities     | Implement Agent observation, managed Worktree, Archive, Launch Profiles, and Connect via SSH through their accepted owner-specific vertical migrations.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Connected remote Client            | Validate a human Client on another machine against the same Site with its existing execution Worker. Select identity, network exposure, Client scope, and reconnect evidence in its own slice; remote Worker enrollment and scheduling are not prerequisites. Its priority follows the product decision described above.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Distributed Worker capabilities    | Review enrollment and one concrete kind of remote work, including device identity, data/secret availability, placement, cancellation, and failure behavior. Add mobile background behavior, multi-user policy, and broader scheduling only through their own accepted scopes.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Legacy retirement                  | Delete the Node Server preview, Gateway/protocol, SSH Runtime/protocol, mixed Workbench package, old CLI, and removed Desktop authorities only after each replacement or sunset cohort proves compatibility, installed-artifact handling, and rollback.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |

Every implementation slice must preserve the accepted role boundaries and prove its own security,
privacy, compatibility, observability, rollback, and recovery claims. Historical delivery details
remain in the owning current guides; this proposal records unresolved target decisions, not task
chronology.
