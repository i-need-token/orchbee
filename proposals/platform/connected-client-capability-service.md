# Orchbee Server, Connected Clients, and Capability Nodes

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Product direction, Client-Hub-Worker topology, dual native/SSH connection strategy, and continuity invariants accepted; initial ingress, bounded Personal-Access-Token-to-Cookie preview, Agent-ready local CLI, Slice 2.2 SSH Runtime spine, Slices 2.3A-C identity/release/activation composition, Slice 2.4 bounded Project-root inspection, Slice 2.5 remote-Project admission, Slices 3.1A-B signed Runtime/Terminal Host cohort plus bounded remote Host lifecycle and opaque relay, Slices 3.2A-E remote Workspace, first-terminal, projection, observation, and Electron creation surfaces, and Slice 3.3A POSIX Electron SSH Project enrollment delivered; broader lifecycle, non-Electron enrollment and creation, Agent orchestration, and remaining implementation contracts remain reserved |
| Owner             | Product architecture, Server coordination, Workbench, network ingress, remote-environment, CLI, Agent, and future capability-node maintainers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| Current authority | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) and the owning Desktop, Gateway, Web, and Terminal Host guides                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Exit condition    | Accept and implement bounded owner-specific slices, move maintained facts into their owning guides and tests, then delete proposal                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |

Simplified Chinese review translation:
[connected-client-capability-service.zh-CN.md](connected-client-capability-service.zh-CN.md).
English remains canonical.

## Product direction

One Orchbee installation is a site that may serve more than its local Electron window. The site
must eventually support:

- Desktop, Browser, mobile, and CLI clients that observe or control only their authorized product
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
until separately accepted replacements or adapters exist.

## Accepted distributed-workbench topology

The accepted logical topology has three independently authorized roles:

| Role   | Authority                                                                                                                                                     |
| ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Client | Renders one authorized product projection, submits semantic commands, and owns client-local view and platform effects; it owns no site-durable product state  |
| Hub    | Owns canonical Workbench state, authorized semantic-command commitment, Agent orchestration, environment registry, capability placement, and durable recovery |
| Worker | Owns its local Checkout, filesystem, Git, PTY, process, and bounded Agent-executor effects; it commits no canonical Workbench state directly                  |

One machine may colocate Client, Hub, and Worker roles, but colocation never merges their
identities, authority, state, or lifecycle. A weak work computer may run only a Client. A central
server may run the Hub and one Worker while coordinating additional Workers. **Terminal Host**
continues to name the node-local PTY service; it never names the central Hub.

A Project is a host-independent product goal. A Checkout is one concrete source tree in one
execution Environment. A Workspace selects one primary Checkout and therefore derives the default
placement for its terminals and executor Agents. The accepted relationship does not yet define a
durable schema, migration, cross-machine source-transfer policy, or whether `Directory` is renamed
or adapted internally; those decisions require a bounded Workbench proposal.

## Accepted semantic-control direction

Human controls, the Orchbee CLI, Skills, and approved automation must reach the same owner-defined
semantic operations instead of implementing parallel mutations. The CLI is a stable invocation
surface, not a product-state Store or an authority boundary. Skills compose structured CLI
operations and never parse human presentation text or simulate pointer, DOM, or keyboard input when
an eligible semantic operation exists.

Current in-process Action registrations and prepared invocations remain local runtime capabilities.
They are not serializable network requests and do not become remotely invocable through
enumeration. Each operation admitted to CLI or remote use requires an explicit versioned input and
safe output schema, caller authorization, noninteractive confirmation policy, compatibility
lifecycle, idempotency boundary, and owning domain handler. Client-local operations such as focus,
window geometry, native dialogs, and clipboard access require an exact authorized Client Session;
they never mutate every connected client's view implicitly.

The durable ownership split is:

| State or effect                                                                  | Owner                                   |
| -------------------------------------------------------------------------------- | --------------------------------------- |
| Project, Workspace, shared Tab/Panel structure, Agent task, and scheduling state | Hub product coordinator                 |
| Checkout, file, Git, PTY, process, and local Agent-executor state                | Exact Worker capability owner           |
| Active location, selection, focus, maximization, window geometry, and clipboard  | Exact Client Session or client platform |

## Accepted naming and package timing

The production backend executable is named **Orchbee Server**. Its runnable workspace is
`apps/server`, with package identity `@orchbee/server`; the accepted initial-ingress slice created
that workspace. `Server` names the complete product backend process; it does not collapse network
ingress, authentication, coordination, device identity, scheduling, and execution into one
undifferentiated module.

Two protocol boundaries have accepted names and are created only with concrete consumers:

| Package                             | Creation trigger and responsibility                                                                                                                                                                                                        |
| ----------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `packages/remote-client-protocol`   | Created by the accepted human Browser authentication slice. Currently owns Personal Access Token exchange and signed Cookie session-establishment contracts; broader production transport, negotiation, and authorization remain reserved. |
| `packages/capability-node-protocol` | Create with the first accepted concrete remote capability. Own node enrollment and identity exchange, capability/version advertisement, lease, heartbeat, progress, result, failure, cancellation, fencing, limits, and errors.            |

The two protocols remain separate because a human product client and a capability node have
different identities, credentials, authorities, revocation scopes, and compatibility lifecycles.
Neither package is a Server implementation, owns product state, or grants a generic remote shell.

The current `apps/gateway` and `@orchbee/gateway-protocol` remain explicitly preview-only Browser
transport until a production external-client slice migrates or retires them. They may bind loopback
or an explicitly selected trusted-LAN interface. `apps/server` exchanges the Personal Access Token
for a signed session Cookie, and Gateway admits the exact-Origin control connection with that same
session; no credential appears in the access URL. They must not be renamed to `server` or treated as
the production external-client contract. The production migration must avoid maintaining two
competing external-client network contracts indefinitely.

## Accepted connection strategy

Orchbee supports two complementary connection families:

| Connection family | Responsibility                                                                                                                                                                   |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Native site       | HTTPS establishes authenticated human or device sessions; WSS carries versioned control, event, progress, and bounded binary-projection messages through role-specific contracts |
| SSH environment   | Connects an unmanaged machine, validates its host identity, bootstraps or repairs an Orchbee runtime, and may tunnel native protocol traffic during an explicitly bounded phase  |

Native site connectivity is the long-term primary path for authenticated Clients and enrolled Workers. SSH
remains a first-use, compatibility, administration, and recovery path; it is not the Workbench,
Agent, scheduling, or state-synchronization protocol. After enrollment, a Worker should normally
establish its own outbound native connection to the Hub with a non-Cookie device credential rather
than requiring the Hub to maintain an inbound SSH login.

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
deployment decisions. They may not bypass Hub-issued authority or recovery fencing.

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

| Responsibility                          | Candidate owner                                                                                                     |
| --------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| HTTP/WebSocket ingress and Web UI       | A network ingress service; TLS, origin policy, routing fallback, rate limits, and static renderer delivery          |
| Human authentication                    | A site identity/session service; bootstrap token exchange, secure Cookie session, expiry, revocation, and audit     |
| Product coordination                    | The Hub application coordinator; authorized Workbench sessions, canonical state, semantic commitment, and recovery  |
| Environment and Checkout registry       | A product environment owner; stable placement identity, observed availability, and bounded provider adaptation      |
| Capability-node enrollment              | A device identity service; pairing, device credentials, attestation inputs, rotation, revocation, and presence      |
| Work scheduling                         | A typed capability scheduler; admission, placement, lease, cancellation, retry, quota, and result ownership         |
| Local terminal execution                | Terminal Host behind its existing trusted local broker boundary                                                     |
| Remote execution                        | A separately authenticated node runtime exposing only accepted versioned capability contracts                       |
| CLI and Skill invocation                | A narrow semantic-command client; structured invocation and observation without product-state ownership             |
| Desktop, Browser, and mobile projection | Product clients consuming authorized application APIs; client form factor grants no backend authority automatically |

The logical Client-Hub-Worker topology is accepted; the final physical process topology remains
reserved. A first release may colocate ingress and coordination with Desktop, but colocating
processes never merges their trust, state, or lifecycle responsibilities. A later always-on
background service must preserve the same contracts without requiring another Workbench renderer.

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

| Shape                           | Required behavior                                                                                                        |
| ------------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Single computer                 | Client, Hub, local Worker capabilities, and optional Web ingress may be colocated                                        |
| Weak remote work computer       | A Desktop, Browser, mobile, or CLI Client renders and controls one authorized remote Hub without becoming a Worker       |
| SSH environment                 | The active product coordinator bootstraps and attaches one unmanaged remote Project environment without inventing a Site |
| Distributed site                | One Hub coordinates its local Worker and additional enrolled Workers through explicit identities and capabilities        |
| Coordinator temporarily offline | Clients and nodes fail closed or enter an explicitly modeled degraded mode; neither invents authority                    |

LAN discovery, Internet relay, NAT traversal, public DNS, and cloud rendezvous are transport and
deployment choices, not product authorization mechanisms.

## Invariants

- Network exposure is off by default until an authorized user enables and configures it.
- Every externally supplied message is parsed at its trust boundary and authorized after
  authentication; possession of an IP address and port grants nothing.
- Human sessions, device identity, capability leases, terminal attachments, and Workbench client
  sessions use distinct identities and revocation scopes.
- Hub, Worker, and Client Session state have one accepted authority each; transport adapters and
  colocation never create another writer.
- A Project is independent of machine identity; every concrete Checkout and execution effect
  resolves through one explicit Environment before reaching a Worker.
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
- Browser, Electron, and mobile clients observe the same authorized product model and route
  semantics; transport choice does not create another Workbench implementation.
- Audit records contain identities, decisions, and bounded metadata, never terminal content,
  credentials, capability tokens, or secret payloads by default.

## Reserved decisions

Implementation remains blocked on separate acceptance of:

- whether the site coordinator is an Electron-owned child, an independently supervised local
  service, or a deployable server;
- expansion beyond the delivered opaque Directory placement, Main-only remote-Project admission,
  empty remote Workspace, explicit first remote-terminal provision, exact live projection, and
  Environment-scoped durable metadata/failure/exit observation plus the Electron-only default first-
  remote-terminal creation surface: non-Electron creation surfaces, additional terminals, close,
  restart, split, archive, application-stop drain, automatic reconnect reconciliation, edit or
  removal of Environment references, provider discovery, source transfer, and cross-machine Git
  policy;
- expansion of the delivered Agent-ready CLI catalog, including each additional operation's
  structured contract, authorization, compatibility, confirmation, and client-view targeting
  policy;
- production SSH Environment signer operation and artifact publication or network feed; Windows AskPass;
  operating-system service or linger integration; version repair, removal, and old-version cleanup.
  Slice 2.2 already fixes target resolution, host-key admission, POSIX AskPass, the supervised
  OpenSSH stdio attachment, the signed no-download bootstrap, the two-platform matrix, immutable
  installation, exact Hello/epoch/heartbeat protocol, generation fencing, and reconnect policy.
  Slice 2.3A fixes independent Environment and Runtime identities, exact SSH target uniqueness,
  append-only idempotent create, strict local persistence, and indeterminate-commit reconciliation.
  Slice 2.3B fixes the canonical two-platform release, external Ed25519 signing boundary, exact
  manifest-pinned Desktop trust descriptor, release-only package command, and private staging.
  Slice 2.3C fixes explicit Environment-owned activation and same-child installed-artifact
  negotiation. Slice 2.4 fixes exact `project-root-v1` admission and bounded canonical-directory
  observation. Slice 2.5 fixes Workbench document version 2, one-way local-placement migration,
  Main-only idempotent remote-Project admission, and a forced-unavailable remote Directory without
  remote Workspace or general filesystem authority. Slice 3.1A fixes the signed schema-version-2
  Runtime and Terminal Host cohort, exact four-artifact Desktop verification, and private package
  placement. Slice 3.1B fixes strict `OBSI/2`/`OBSC/2` cohort activation, exact Runtime and Host
  identity in Hello, one `terminal-host-v1` opaque relay over the existing SSH attachment, same-user
  exact-Host reuse or launch, bounded byte credit and channel-local failures, and reconnect without
  replay. It grants no Workspace effect, terminal creation, general filesystem access, shell, or
  generic command authority. Slice 3.2A fixes exactly one inactive remote Workspace and empty
  Terminal Context in the same Main-owned canonical commit, legacy Project upgrade, exact Client
  retention/deletion, and rejection of Client forgery or terminal-state injection. It grants no
  terminal presentation or Host effect. Slice 3.2B fixes one Main-owned exact-empty-context
  first-terminal mutation, isolated per-Environment runtime resources, canonical target derivation,
  commit-before-effect ordering, pending retry without false outcome publication, exact singleton
  reconciliation, and the sole Client `provision`-to-`retain` convergence. It grants no terminal
  projection, input, resize, additional terminal creation, or broader lifecycle control. Slice 3.2C
  fixes canonical Presentation-to-runtime routing, already-prepared Environment admission, live
  interactive projection through the existing attachment protocol, local/archive no-fallback,
  post-open target revalidation, and Electron/authenticated-Gateway composition. It grants no
  creation surface, automatic SSH activation or reconnect, additional terminal, durable remote
  metadata/exit publication, close, restart, split, archive, application-stop drain, CLI, Skill, or
  Agent authority. Slice 3.2D fixes exact current-record and Host-epoch observation, per-Environment
  binding/recovery/publication namespaces, monotonic metadata, non-synthetic failure, synchronous
  durable exit admission or rejection, binding/reservation/ledger/broadcast/compaction ordering,
  retarget retention, deduplication, and restart recovery on the next explicit prepare without SSH
  activation. It grants no creation surface, automatic SSH reconnect, additional terminal, close,
  restart command, split, archive, application-stop drain, CLI, Skill, or Agent authority. Slice
  3.2E fixes the Electron-only `workbench.terminal.create` route for an exact empty remote Workspace,
  one dedicated bootstrap port, renderer input limited to Project and Workspace identities,
  Main-derived and revalidated Environment authority, connected-only admission, singleton
  convergence, canonical-publication waiting without Client View effects, and conservative
  post-dispatch cancellation and port-loss outcomes. It grants no Gateway or CLI creation,
  automatic SSH activation or prompt, alternate placement, additional terminal, broader lifecycle,
  Skill, or Agent authority. Slice 3.3A fixes the POSIX Electron `Connect via SSH…` enrollment and
  explicit reconnect surface, bounded renderer intent, an eleventh dedicated bootstrap port, exact
  request/generation/prompt interaction leases, append-only Environment identity reuse, registry-
  before-remote-effect and independently durable Workbench commit ordering, canonical Project and
  Workspace admission, initiating-view-only activation, no automatic startup connection, signed-
  release-only activation, and controlled real-SSH evidence. It grants no registry edit or removal,
  remote cleanup, non-Electron enrollment, additional terminal, broader terminal lifecycle, generic
  remote command, Skill, or Agent authority;
- enrollment and recovery ceremonies, device identity technology, credential storage, rotation,
  revocation, and administrator policy;
- production HTTPS/WSS endpoints, session-resumption credential, protocol negotiation, heartbeat,
  backoff, snapshot/delta, binary attachment, backpressure, and replay bounds;
- LAN-only, Internet-reachable, relay, and NAT-traversal deployment support;
- whether any authorized high-volume data path may connect a Client directly to a Worker, and the
  Hub-issued ticket, revocation, accounting, and fallback required if it does;
- the first concrete remote capability and whether its data can leave the primary computer;
- scheduling policy, quotas, fairness, priority, preemption, energy awareness, and user consent;
- multi-user tenancy, authorization hierarchy, sharing scope, and audit retention;
- protocol version negotiation, rolling update, compatibility, and node quarantine;
- artifact and secret transfer, encryption at rest, deletion, recovery, and failure diagnostics;
- exact degraded behavior and retention duration while Clients, Hub, Workers, or transports are
  offline; and
- service installation, automatic start, update continuity, backup, and uninstall behavior.

Do not create an empty `capability-node-protocol` workspace, broaden
`@orchbee/remote-client-protocol` beyond its accepted human Browser authentication contracts, or
extend `@orchbee/gateway-protocol` into the production contract before another bounded slice has a
concrete consumer and accepted contract. The delivered Server and Browser authentication slices do
not authorize capability-node or production remote-execution protocols.

## Delivery order

The initial default-loopback Server ingress, explicit trusted-LAN preview, and bounded human
Personal-Access-Token-to-Cookie authentication were delivered before the complete roadmap. They are
reused foundations, not evidence that a later phase is complete.

| Phase | Independently deliverable outcome                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 0     | Accepted here: logical Client-Hub-Worker roles, authority split, dual native/SSH strategy, connection-continuity invariants, ordered phases, and explicit reserved decisions                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| 1     | Delivered through the current [Agent-ready local CLI](../../apps/cli/README.md) for an accepted catalog of owner-defined semantic operations, with structured input/output, execution observation, cancellation, authorization, and noninteractive policy; it adds no remote execution                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                    |
| 2     | Delivered foundation: the [Desktop SSH connection foundation](../../apps/desktop/docs/ssh-environment-connection.md) provides target resolution, host-key admission, transient POSIX AskPass, one supervised OpenSSH stdio transport, signed no-download Runtime bootstrap, exact Hello/epoch/heartbeat, connection reuse, retained-daemon attachment recovery, installed-artifact negotiation, bounded Project-root observation, and Main-only remote-Project admission. The [durable Environment registry](../../apps/desktop/docs/environment-registry.md) provides stable provider-neutral identity and strict append-only persistence                                                                                                                                                                                                                                                                                                                                                                                                                                |
| 3     | In progress: [offline SSH Environment release composition](../../apps/desktop/docs/ssh-runtime-release.md) binds exact Runtime and Terminal Host artifacts for both admitted platforms into one signed v2 cohort and Desktop package; the [Desktop SSH connection foundation](../../apps/desktop/docs/ssh-environment-connection.md) installs that cohort atomically, exposes one bounded opaque Terminal Host relay over its existing SSH attachment, admits one durable inactive remote Workspace, provisions its first remote terminal with commit-before-effect ordering, projects that existing terminal interactively to Electron and authenticated Gateway clients through canonical target routing, publishes Environment-scoped metadata, failure, and crash-safe ordered exit observation, and exposes the ordinary Electron New Terminal interaction for only that exact default singleton. Later accepted slices must authorize non-Electron creation and broader durable lifecycle; begin manually and add no automatic cross-machine source synchronization |
| 4     | Establish a headless Hub authority and production remote Client session so a weak Desktop or Browser client can reconnect to the same Workbench, terminals, and durable Agent work; migrate trusted whole-document proposals toward authorized semantic commands before admitting untrusted writers                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| 5     | Add durable Hub Agent orchestration, Worker-local executor Agents, and Skills that compose the authorized CLI surface; one goal may coordinate explicitly selected Workers without automatic resource scheduling                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| 6     | Add node enrollment and read-only capability inventory, then one modeled remote capability, then its lease-based scheduler with heartbeat, fencing, cancellation, retry, data-locality, fault-injection, and security evidence before admitting another capability                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| 7     | Harden production deployment with accepted TLS and ingress policy, multi-user authorization if required, relay or NAT traversal if required, secret and artifact policy, quotas, audit, rolling compatibility, backup, service supervision, and Hub recovery                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |

Every phase requires its own accepted bounded or consequential authorization before governed
implementation. Security, privacy, compatibility, observability, rollback, and recovery are
cumulative requirements of each phase rather than work deferred entirely to Phase 7.
