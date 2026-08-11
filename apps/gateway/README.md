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

The Utility Process never imports or connects to Terminal Host. It has no shared-document writer, binding registry, application-lifetime port, filesystem capability, Client View storage, product settings, or terminal count policy. Desktop starts it only after an explicit Browser-access action and kills it when access stops, changes mode, or Desktop shuts down; each transition revokes the lifetime capability.

## Page and attachment lifecycle

| Transition                              | Effect                                                                                                                                          |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Utility start                           | Bind an ephemeral loopback port, or `0.0.0.0` only for explicit trusted-LAN preview, and accept only Desktop's exact renderer Origin allowlist. |
| Page authentication succeeds            | Request one fresh Desktop ClientSession and projection-control pair.                                                                            |
| Page authentication fails               | Close the socket without allocating Desktop authority.                                                                                          |
| Projection opens                        | Register one derived attachment capability and transfer only the matching attachment stream.                                                    |
| Attachment queue exceeds its byte bound | Retire the exact attachment and release its projection; the page opens a fresh projection and receives a full snapshot.                         |
| Page control closes                     | Release every page attachment and the page session; preserve shared Workbench state and Host terminals.                                         |
| Utility stops                           | Terminate sockets still awaiting authentication, then close listeners, pages, attachments, and transferred ports before reporting stopped.      |

Control and attachment messages are parsed at every process or network boundary with `@orchbee/gateway-protocol`. Terminal frame and row payloads stay binary; the Gateway performs no VT parsing, row reconstruction, base64 conversion, or per-cell work. Transport byte limits protect one connection and do not impose a product count ceiling.

## Capability delivery and trusted-LAN preview

Desktop Main retains every complete access URL and exposes only sanitized HTTP Origins to its sandboxed renderer. Copy Link and Open Browser are Main-owned effects confined to currently active exact Origins. The high-entropy capability remains in the URL fragment, so it is not sent in the renderer HTTP request. It is never placed in renderer state, diagnostics, telemetry, files, or readiness output; the Gateway itself emits no capability-bearing diagnostic.

Loopback is the default. Trusted-LAN preview binds both the renderer Server and Gateway to `0.0.0.0`, but publishes only enumerated RFC 1918 IPv4 Origins. It uses unencrypted HTTP, has no account login or TLS, and grants full terminal control to anyone holding a link until access is stopped or Desktop exits. The Desktop UI presents that warning before activation and provides immediate revocation.

## Correctness evidence

| Claim                                                 | Evidence                                                                                                              |
| ----------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| Authentication precedes Desktop page allocation       | Pure lifecycle transition tests and real loopback WebSocket tests.                                                    |
| Wrong Origin or capability creates no page authority  | Real loopback and trusted-LAN-style exact-Origin WebSocket rejection tests and the Desktop-plus-Browser system trace. |
| Disconnect affects only one page                      | Lifecycle tests, scoped-port tests, and multi-client system reconnection.                                             |
| Utility stop cannot wait on an unauthenticated socket | Real loopback attachment shutdown regression.                                                                         |
| Attachment failure cannot corrupt another attachment  | Attachment identity validation, binary codec tests, bounded queue retirement, and projection reopen tests.            |
| Gateway cannot bypass Main or Host ownership          | Workspace dependency boundaries, project contracts, and absence of a Host dependency.                                 |

The lifecycle is a transport refinement of the existing Desktop multi-client model; it does not add a new canonical Workbench or terminal-lifetime state machine.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/gateway typecheck
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/gateway build
```
