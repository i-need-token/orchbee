# Browser Gateway

On-demand Electron Utility Process that connects loopback or explicitly authorized trusted-LAN Browser pages to Desktop Main's transport-neutral Workbench client boundary.

## Runtime boundary

```text
Browser page
  ├─ one authenticated control WebSocket
  └─ one binary WebSocket per visible terminal attachment
          │
          ▼
apps/gateway (authenticated listener and framing only)
          │ two page-scoped MessagePorts
          ▼
Desktop Main (Workbench, TerminalPresentation, and lifecycle authority)
          │
          ▼
apps/terminal-host (PTY, process, VT, history, and TerminalId authority)
```

The Utility Process never imports or connects to Terminal Host. It has no shared-document writer, binding registry, application-lifetime port, filesystem capability, Client View storage, product settings, or terminal count policy. Desktop starts it only after an explicit Browser-access action and kills it when access stops, changes mode, or Desktop shuts down; each transition discards the active Browser-session signing key and revokes every admitted page.

## Page and attachment lifecycle

| Transition                              | Effect                                                                                                                                             |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Utility start                           | Bind an ephemeral loopback port, or `0.0.0.0` only for explicit trusted-LAN preview, and accept only Desktop's exact renderer Origin allowlist.    |
| Control Upgrade arrives                 | Require an exact allowed Origin and a valid signed Browser session Cookie before accepting the WebSocket; otherwise allocate no Desktop authority. |
| Page protocol handshake arrives         | Require the protocol-version handshake within 10 seconds, then request one fresh Desktop ClientSession and projection-control pair.                |
| Attachment handshake arrives            | Require one page-derived, one-time attachment capability within 10 seconds and transfer only the matching attachment stream.                       |
| Authenticated socket remains connected  | Ping every 30 seconds; terminate the exact socket when the preceding ping received no pong.                                                        |
| Projection opens                        | Register one derived attachment capability and transfer only the matching attachment stream.                                                       |
| Attachment queue exceeds its byte bound | Retire the exact attachment and release its projection; retain correlation until Main acknowledges cleanup, then reopen from a full snapshot.      |
| Page control closes                     | Release every page attachment and the page session; preserve shared Workbench state and Host terminals.                                            |
| Utility stops                           | Terminate sockets still awaiting a handshake, then close listeners, pages, attachments, and transferred ports before reporting stopped.            |

Control and attachment messages are parsed at every process or network boundary with `@orchbee/gateway-protocol`. Terminal frame and row payloads stay binary; the Gateway performs no VT parsing, row reconstruction, base64 conversion, or per-cell work. Transport byte limits protect one connection and do not impose a product count ceiling.

## Session admission and trusted-LAN preview

Browser access URLs contain only the renderer route, Gateway port, and protocol version. Desktop Main confines Copy Link and Open Browser effects to currently active exact Origins; no reusable credential appears in the URL, renderer state, diagnostics, telemetry, or readiness output. The Server exchanges the installation-scoped Personal Access Token for an `HttpOnly` signed session Cookie. Gateway validates that Cookie with the same activation-scoped signing key before accepting a control WebSocket. Desktop persists only the token verifier, and a reset stops Browser access before replacing the verifier, so the old signing key and every active Browser session are retired.

Loopback is the default. Trusted-LAN preview binds both the renderer Server and Gateway to `0.0.0.0`, but publishes only enumerated RFC 1918 IPv4 Origins. It uses unencrypted HTTP and has no TLS, Internet-ingress, or multi-user account claim. A network observer may read the token, Cookie, and terminal traffic. The Desktop UI presents that warning, requires an explicit Personal Access Token, and provides immediate revocation by stopping access or resetting the token.

## Correctness evidence

| Claim                                                 | Evidence                                                                                                              |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Cookie admission precedes Desktop page allocation     | Pure lifecycle transition tests and real loopback WebSocket tests.                                                    |
| Wrong Origin or Cookie creates no page authority      | Real loopback and trusted-LAN-style exact-Origin WebSocket rejection tests and the Desktop-plus-Browser system trace. |
| Disconnect affects only one page                      | Lifecycle tests, scoped-port tests, and multi-client system reconnection.                                             |
| Utility stop cannot wait on an unauthenticated socket | Real loopback attachment shutdown regression.                                                                         |
| Listener cancellation always settles                  | Deterministic listen/dispose ordering tests plus the bounded Gateway transport model.                                 |
| Authentication and heartbeat timers cannot leak       | Fake-timer WebSocket tests plus `GatewayTransportLifecycle.tla`.                                                      |
| Attachment failure cannot corrupt another attachment  | Attachment identity validation, binary codec tests, bounded queue retirement, and projection reopen tests.            |
| Gateway cannot bypass Main or Host ownership          | Workspace dependency boundaries, project contracts, and absence of a Host dependency.                                 |

The lifecycle is a transport refinement of the existing Desktop multi-client model; it does not add a new canonical Workbench or terminal-lifetime state machine. `formal/tla/GatewayTransportLifecycle.tla` owns the bounded listener-cancellation, authentication-deadline, and heartbeat refinement.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/gateway typecheck
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/gateway test:tla
pnpm --filter @orchbee/gateway build
```
