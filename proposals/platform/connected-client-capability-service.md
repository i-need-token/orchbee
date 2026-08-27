# Orchbee Server, Connected Clients, and Capability Nodes

| Field             | Value                                                                                                                                                                                 |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Product direction, initial ingress, and bounded Personal-Access-Token-to-Cookie preview accepted and delivered; production trust model and remote execution contracts remain reserved |
| Owner             | Product architecture, Desktop service, network ingress, and future capability-node maintainers                                                                                        |
| Current authority | [`docs/ARCHITECTURE.md`](../../docs/ARCHITECTURE.md) and the owning Desktop, Gateway, Web, and Terminal Host guides                                                                   |
| Exit condition    | Accept and implement bounded owner-specific slices, move maintained facts into their owning guides and tests, then delete proposal                                                    |

Simplified Chinese review translation:
[connected-client-capability-service.zh-CN.md](connected-client-capability-service.zh-CN.md).
English remains canonical.

## Product direction

One Orchbee installation is a site that may serve more than its local Electron window. The site
must eventually support:

- Browser and mobile clients that observe and control only their authorized product sessions;
- another enrolled computer contributing typed compute or device capabilities; and
- the primary computer contributing its local capabilities while coordinating the site.

The externally reachable service is therefore an application backend, not only a static-file
server or reverse proxy. Static Web delivery, TLS termination, and request forwarding may be
deployment responsibilities, but they cannot own sessions, authorization, device enrollment,
capability leases, scheduling, or product state.

This direction does not turn the current local-development `apps/gateway` or the private
same-user Terminal Host endpoint into a network service. Those current boundaries remain in force
until separately accepted replacements or adapters exist.

## Accepted naming and package timing

The production backend executable is named **Orchbee Server**. Its runnable workspace will be
`apps/server`, with package identity `@orchbee/server`, when the first accepted production slice
requires that process. `Server` names the complete product backend process; it does not collapse
network ingress, authentication, coordination, device identity, scheduling, and execution into one
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

## Candidate responsibility split

| Responsibility                    | Candidate owner                                                                                                     |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| HTTP/WebSocket ingress and Web UI | A network ingress service; TLS, origin policy, routing fallback, rate limits, and static renderer delivery          |
| Human authentication              | A site identity/session service; bootstrap token exchange, secure Cookie session, expiry, revocation, and audit     |
| Product coordination              | A site application coordinator; authorized Workbench sessions, shared-document authority, and capability registry   |
| Capability-node enrollment        | A device identity service; pairing, device credentials, attestation inputs, rotation, revocation, and presence      |
| Work scheduling                   | A typed capability scheduler; admission, placement, lease, cancellation, retry, quota, and result ownership         |
| Local terminal execution          | Terminal Host behind its existing trusted local broker boundary                                                     |
| Remote execution                  | A separately authenticated node runtime exposing only accepted versioned capability contracts                       |
| Browser and mobile presentation   | Product clients consuming authorized application APIs; client form factor grants no backend authority automatically |

The final process topology is reserved. A first release may colocate ingress and coordination with
Desktop, but colocating processes never merges their trust, state, or lifecycle responsibilities.
A later always-on background service must be able to preserve the same contracts without requiring
another Workbench renderer.

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
remain local semantic contracts unless a separately reviewed API explicitly exposes one operation.

## Deployment shapes

| Shape                           | Required behavior                                                                                     |
| ------------------------------- | ----------------------------------------------------------------------------------------------------- |
| Single computer                 | Electron, site coordination, local capabilities, and optional Web ingress may be colocated            |
| Shared Browser or mobile        | The site serves the renderer and authenticates a human session before exposing product APIs           |
| Additional compute computer     | An enrolled node advertises typed capacity and receives only explicitly scheduled leases              |
| Coordinator temporarily offline | Clients and nodes fail closed or enter an explicitly modeled degraded mode; neither invents authority |

LAN discovery, Internet relay, NAT traversal, public DNS, and cloud rendezvous are transport and
deployment choices, not product authorization mechanisms.

## Invariants

- Network exposure is off by default until an authorized user enables and configures it.
- Every externally supplied message is parsed at its trust boundary and authorized after
  authentication; possession of an IP address and port grants nothing.
- Human sessions, device identity, capability leases, terminal attachments, and Workbench client
  sessions use distinct identities and revocation scopes.
- Terminal Host retains its private local transport and never accepts an Internet or LAN peer
  directly under the current contract.
- Remote execution never receives a general Desktop, filesystem, shell, secret, or Action
  capability merely because one accepted capability needs a narrower effect.
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
- enrollment and recovery ceremonies, device identity technology, credential storage, rotation,
  revocation, and administrator policy;
- LAN-only, Internet-reachable, relay, and NAT-traversal deployment support;
- the first concrete remote capability and whether its data can leave the primary computer;
- scheduling policy, quotas, fairness, priority, preemption, energy awareness, and user consent;
- multi-user tenancy, authorization hierarchy, sharing scope, and audit retention;
- protocol version negotiation, rolling update, compatibility, and node quarantine;
- artifact and secret transfer, encryption at rest, deletion, recovery, and failure diagnostics;
- offline behavior for clients, coordinator, and nodes; and
- service installation, automatic start, update continuity, backup, and uninstall behavior.

Do not create an empty `capability-node-protocol` workspace, broaden
`@orchbee/remote-client-protocol` beyond its accepted human Browser authentication contracts, or
extend `@orchbee/gateway-protocol` into the production contract before another bounded slice has a
concrete consumer and accepted contract. The delivered Server and Browser authentication slices do
not authorize capability-node or production remote-execution protocols.

## Delivery order

1. Delivered: establish the default-loopback Orchbee Server ingress foundation and the separately
   explicit trusted-LAN preview described above.
2. Delivered: implement opt-in Web ingress and human Personal-Access-Token-to-Cookie authentication
   as one bounded slice.
3. Implement device enrollment and a read-only typed capability inventory without remote work.
4. Select one concrete compute capability and model its lease, cancellation, failure, data, and
   fencing semantics.
5. Add scheduling and remote execution for that capability with deterministic, fault-injected, and
   security evidence before admitting another capability.
