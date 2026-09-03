# Orchbee Server, Connected Clients, and Workers

| Field              | Value                                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Client-Server-Worker product direction is accepted; existing Browser-ingress and Desktop SSH foundations remain delivered current authority, the first native local Server-Worker process spine is complete and unshipped, the remaining local-terminal vertical is in progress, and remote/distributed operation, consumer migration, cutover, and deletion remain bounded future work              |
| Owner              | Product architecture, Server coordination, Workbench, network ingress, Worker placement, remote-connection adapters, CLI, Agent, and future capability-node maintainers                                                                                                                                                                                                                              |
| Current authority  | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) and the owning Desktop, Gateway, Web, and Terminal Host guides                                                                                                                                                                                                                                                                                  |
| Decision authority | The product owner accepted the uniform peer-Client model, separate Rust `orchbee`/Server/Worker plus Zig Terminal Host processes, Server-relayed first local terminal path, bounded lazy local activation, single controller per terminal for the first milestone, and the target dispositions referenced below; exact remote, production-service, and optional-capability contracts remain reserved |
| Exit condition     | Accept and implement bounded owner-specific slices, move maintained facts into their owning guides and tests, then delete proposal                                                                                                                                                                                                                                                                   |

Simplified Chinese review translation:
[connected-client-capability-service.zh-CN.md](connected-client-capability-service.zh-CN.md).
English remains canonical.

## Product direction

One Orchbee installation is a site that may serve more than its local Electron window. The site
must eventually support:

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

| Package                           | Creation trigger and responsibility                                                                                                                                                                                                                                                           |
| --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `packages/remote-client-protocol` | Created by the accepted human Browser authentication slice. Currently owns Personal Access Token exchange and signed Cookie session-establishment contracts; broader production transport, negotiation, and authorization remain reserved.                                                    |
| Future Server-Worker contract     | Create a language-neutral authority and only the bindings required by the first accepted concrete remote capability. It owns node enrollment and identity exchange, capability/version advertisement, lease, heartbeat, progress, result, failure, cancellation, fencing, limits, and errors. |

The two contracts remain separate because a human product client and a Worker have
different identities, credentials, authorities, revocation scopes, and compatibility lifecycles.
Neither contract is a Server implementation, owns product state, or grants a generic remote shell.

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
Workers. If selected, SSH remains a first-use, compatibility, administration, and recovery path; it
is not the Workbench, Agent, scheduling, or state-synchronization protocol. After enrollment, a
Worker should normally establish its own outbound native connection to the Server with a non-Cookie
device credential rather than requiring the Server to maintain an inbound SSH login.

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
the explicitly warned trusted-LAN preview already delivered. Direct Client-to-Worker data paths,
QUIC or WebTransport adapters, relay service, and Internet ingress remain reserved optimizations or
deployment decisions. They may not bypass Server-issued authority or recovery fencing.

## Accepted initial ingress foundation

The first bounded Server slice creates `apps/server` as a Fastify-based Node application. One
HTTP/1.1 listener owns ordinary HTTP requests and WebSocket upgrades on the same origin and port.
The initial route namespaces are reserved as follows:

| Path                | Initial responsibility                                                                                                              |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------- |
| `/api/*`            | Application HTTP API namespace, initially versioned under `/api/v1`; an unimplemented endpoint returns JSON and never renderer HTML |
| `/ws/*`             | Application WebSocket namespace, initially versioned under `/ws/v1`; an ordinary HTTP request or unknown upgrade fails closed       |
| `/static/*`         | Immutable renderer assets; a missing asset returns `404` and never falls through to the Browser Router                              |
| Other product paths | Authenticated `GET` and `HEAD` HTML navigations receive the renderer entry; unauthenticated navigation redirects to `/login`        |

Fastify owns HTTP lifecycle, routing, and plugin composition. Its official WebSocket and static
plugins provide Upgrade handling and renderer delivery; application schemas remain Zod-owned at
their trust boundaries. The initial foundation defined no external API or WebSocket message
contract. The later bounded human Browser authentication slice created
`@orchbee/remote-client-protocol` without creating a general product API.

Loopback remains the standalone Server default. The accepted Desktop-owned trusted-LAN preview is
the only pre-production exception: an explicit UI action binds Server and Gateway to `0.0.0.0`,
publishes only enumerated RFC 1918 IPv4 Origins, requires a Personal Access Token exchange, admits
each exact-Origin Gateway control connection with the resulting signed Cookie, warns that HTTP can
expose credentials and terminal traffic, and revokes sessions on stop, mode change, token reset, or
process exit. This exception is not a production Internet deployment or multi-user account design.
Starting the Server entry by itself is not equivalent to enabling sharing.

## Human Web access direction

Network exposure remains off by default. Enabling sharing starts the accepted site ingress on an
explicit interface and port; the shareable entry is the short origin, such as
`https://192.0.2.1:8443/`, followed only by normal product routes. Credentials, bootstrap tokens,
and session capabilities never belong in the shared query string or fragment.

An unauthenticated Browser request retains its intended product path while entering a login flow.
The person enters a separately issued access token; a successful exchange creates a bounded,
revocable human session represented by an appropriately scoped Cookie, then resumes the intended
route. This human login flow never enrolls a capability node and never grants device credentials.

The delivered preview uses one installation-scoped 256-bit Personal Access Token, persists only its
verifier, reveals plaintext only after create or reset, and exchanges it under an in-memory
per-source failure limit. Its session Cookie contains a signed random identity, is `HttpOnly`,
`SameSite=Strict`, `Path=/`, and lasts for the Browser session. Each Browser-access activation uses
a fresh signing key; stop, mode switch, reset, and exit invalidate earlier sessions. Exact Origin is
required for token exchange and Gateway control admission. TLS requirements, production session
duration and revocation storage, multi-user authorization, and account recovery remain reserved.

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

## Candidate responsibility split

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
`orchbee`, private Rust Server, private Rust Worker, and separate Zig Terminal Host. Their additive
foundations and first local Server-Worker process spine are delivered but unshipped, and the
remaining local-terminal vertical is in progress; the current Desktop-colocated runtime stays
authoritative until bounded migration slices replace it. The first complete local terminal path
relays through Server, while bare `orchbee` may lazily activate the exact local cohort without
supervising its lifetime. Colocation never merges trust, state, or lifecycle responsibilities.
Production service installation, remote enrollment, and any measured direct Worker attachment
remain reserved.

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
Every accepted capability contract must define:

- versioned input, progress, result, failure, cancellation, and compatibility schemas;
- resource requirements and scheduler-visible capacity without exposing unrelated device data;
- lease identity, expiry, heartbeat, duplicate-delivery behavior, and idempotency boundary;
- artifact, source-data, secret, log, and diagnostic transfer policy;
- disconnect, reassignment, partial-result, and late-result behavior; and
- the product owner that authorizes work and commits its durable outcome.

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

1. Agent integration, including orchestration semantics, an optional hook helper, Worker-local
   execution, association, installation cohort, and failure policy.
2. Managed Worktree, Archive, Launch Profiles, SSH-specific Desktop behavior, and broader terminal
   lifecycle such as split, restart, close, export, and long-term history.
3. Migration or sunset of the current SSH path, including every consumer and installed artifact;
   independently, the product scope, timing, transport, authentication, and lifecycle of any future
   unmanaged-SSH adapter.
4. Production human and device sessions, Worker enrollment, remote capability contracts,
   scheduling, leases, fencing, cancellation, quota, and data-locality policy.
5. Production HTTPS/WSS, TLS and Internet ingress, relay or NAT traversal, mobile backgrounding,
   multi-user tenancy, authorization hierarchy, audit, secrets, and artifact transfer.
6. A direct Client-to-Worker terminal path. It is admitted only if Server-relay measurements justify
   it and its scoped capability, expiry, revocation, accounting, reconnect, and fallback are
   separately accepted.
7. Production service installation, login-time start, update cohorts, compatibility, durable Server
   recovery, backup, rollback, and uninstall beyond the accepted bounded lazy local activation.
8. Public command grammar beyond bare TUI, `--help`, and `--version`; stable automation output and
   exit contracts; Skills and completion; package-manager distribution; public cutover; and
   compatibility policy.
9. Each deletion of the Node Server preview, Gateway/protocol, SSH Runtime/protocol, mixed Workbench
   package, old CLI, or migrated Desktop authority. Deletion requires its own consumer,
   installed-artifact, behavior, migration or sunset, documentation, evidence, and rollback audit.

Do not create an empty remote Worker protocol workspace or broaden a current preview protocol
without a concrete accepted vertical consumer. The current Browser-ingress and SSH paths remain
authoritative only for their delivered behavior until a complete replacement or explicit sunset is
accepted and verified.

## Delivery sequence

Current delivered behavior belongs to its owning application guides and executable tests rather
than a phase ledger in this proposal.

| Milestone                             | Independently reviewable outcome                                                                                                                                                                                                                                                                      |
| ------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Existing current paths                | Keep the Node Browser-ingress preview, current TypeScript CLI, Desktop-colocated coordinator, and delivered SSH path authoritative for their supported behavior while additive native work remains unshipped.                                                                                         |
| Native foundations                    | **Complete and unshipped:** root Rust support, framework-neutral Workbench projection contracts, native `orchbee` entry foundation, Rust Server kernel, and Rust Terminal Host binding. [`native-runtime-topology.md`](native-runtime-topology.md) links their current owners.                        |
| Local terminal vertical               | **In progress and unshipped:** the real local Server/Worker process spine is complete; next deliver the minimal Host broker, Server-relayed terminal stream, and real TUI Client, followed by fault and performance convergence. This path changes no current consumer and creates no public cutover. |
| Shared Client and Server migration    | Move canonical Workbench coordination and supported Browser session/relay behavior into Rust Server, move React presentation into `apps/web`, reduce Desktop to a Client/platform adapter, and migrate only explicitly admitted CLI capabilities.                                                     |
| Optional and distributed capabilities | Review Agent, Worktree, Archive, Launch Profiles, future SSH, enrolled remote Workers, mobile/remote Clients, production authentication, and scheduling through separate vertical capabilities.                                                                                                       |
| Legacy retirement                     | Delete the Node Server preview, Gateway/protocol, SSH Runtime/protocol, mixed Workbench package, old CLI, and removed Desktop authorities only after each replacement or sunset cohort proves compatibility, installed-artifact handling, and rollback.                                               |

Every implementation slice must preserve the accepted role boundaries and prove its own security,
privacy, compatibility, observability, rollback, and recovery claims. Historical delivery details
remain in the owning current guides; this proposal records unresolved target decisions, not task
chronology.
