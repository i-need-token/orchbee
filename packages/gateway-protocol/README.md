# @orchbee/gateway-protocol

Strict, runtime-neutral contracts for Orchbee's local development Browser Gateway.

## Scope

The package owns the protocol version, loopback WebSocket paths, one-time attachment capability schemas, Desktop-to-Utility parent messages, page control framing, attachment binary framing, non-secret URL-query bootstrap, and transport safety limits. It contains no WebSocket, Electron, Browser, Desktop Main, Terminal Host, Workbench, UI, persistence, Cookie, or privileged-effect implementation. Human Browser authentication and signed session Cookie contracts belong to `@orchbee/remote-client-protocol`.

`@orchbee/workbench-client-protocol` remains authoritative for page-session, projection-control, and projected-terminal messages. This package only authenticates and frames those messages for the local Browser transport.

## Channels

| Boundary                        | Contract                                                                                                                           |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Desktop Main to Utility Process | One start/stop stream plus scoped page-open/page-close messages and exactly two transferred ports for each accepted page.          |
| Browser control WebSocket       | One authenticated connection per page carrying low-rate Workbench client-session and projection-control messages.                  |
| Browser attachment WebSocket    | One authenticated binary connection per visible projected-terminal attachment carrying only that attachment's commands and events. |
| Browser location                | Gateway port and protocol version encoded exactly once in the URL query; no credential is present.                                 |

The control endpoint is `/v1/control`; the attachment endpoint is `/v1/attachment`. Both require protocol `1.3`. The Gateway accepts only an exact renderer HTTP Origin supplied by Desktop Main. Its runtime adapter additionally requires the signed Browser session Cookie before accepting a control WebSocket.

## Model and invariants

| Concern        | Contract                                                                                                                                                                                                                                 |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | Give an ordinary Browser page the same client meaning as Electron without granting direct Host or application-lifetime authority.                                                                                                        |
| Inputs         | Untrusted query values, WebSocket messages, transferred-port messages, binary frames, socket lifecycle, and transport queue state.                                                                                                       |
| Owned data     | Fresh page connection identities, one-time attachment socket identities and capabilities, and framing metadata.                                                                                                                          |
| Outputs        | Validated generic Workbench-client messages, scoped Desktop parent requests, typed failures, and canonical binary frames.                                                                                                                |
| Authority      | Desktop Main owns canonical Workbench and TerminalPresentation state; Terminal Host owns PTY/VT state; the Browser owns Client View and Browser effects.                                                                                 |
| Authentication | Main shares one activation-scoped Browser session signing key with Server and Gateway. Cookie verification precedes control Upgrade; attachment capabilities are derived for one socket identity and never authorize another attachment. |
| Isolation      | A page receives only its client-session and projection ports. No Gateway message carries a Host connection, binding store, lifetime port, filesystem port, or generic Main capability.                                                   |
| Disconnect     | Closing a page control channel retires that page session and its attachments without changing shared topology or terminal lifetime.                                                                                                      |
| Backpressure   | A bounded attachment queue admits one maximum-size valid frame when empty and retires only the exact attachment on overflow. Reopening projection creates a fresh attachment and full snapshot.                                          |
| Cardinality    | No product count ceiling exists for pages, terminals, or attachments. Byte and buffered-transport limits reject the affected message or connection instead of evicting another client.                                                   |
| Liveness       | Every accepted socket has a 10-second authentication deadline. Authenticated sockets are pinged every 30 seconds and terminated after one unanswered interval.                                                                           |

Protocol `1.3` retains a projection request mapping after release until Desktop returns its correlated
release acknowledgement or cleanup failure. This lets the Gateway forward delayed cleanup results
without treating them as foreign control traffic or retiring another attachment.

The nested Workbench projection-open command carries the visual surface's complete measured
`TerminalGeometry`. A duplicate Gateway request ID is valid only when both its presentation identity
and geometry match the retained request; changed geometry is conflicting control traffic. Gateway
forwards the value unchanged and never supplies a startup default.

The page lifecycle is `awaiting-authentication -> opening-client -> active -> closed`; the retained internal phase name now represents the first protocol-version handshake after Cookie-authenticated Upgrade. `gateway-control-authenticate` carries only the protocol version and is the only first message that may request Desktop page ports. A late port bundle for a closed page is disposed. Attachment registration follows `registered -> authenticated -> retired`; it cannot be resumed or rebound. Control and attachment sockets that miss the handshake deadline are terminated without authority. Every active socket participates in the same ping/pong heartbeat, and one missed pong terminates only that socket. Gateway shutdown terminates sockets in every phase and clears all owned timers.

This protocol is a transport refinement of the existing Desktop multi-client session and demand attachment models. It adds no new product lifecycle state or canonical document transition. `apps/gateway/formal/tla/GatewayTransportLifecycle.tla` is authoritative only for the bounded listener-cancellation, authentication-deadline, and heartbeat refinement. Deterministic lifecycle traces, strict codec tests, and the real Electron-plus-two-Browser system trace provide the concrete transport evidence.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/gateway-protocol build
pnpm --filter @orchbee/gateway-protocol typecheck
pnpm --filter @orchbee/gateway-protocol test
pnpm --filter @orchbee/gateway-protocol test:exports
```
