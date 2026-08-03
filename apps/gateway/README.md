# Local Development Browser Gateway

On-demand Electron Utility Process that connects same-machine Browser pages to Desktop Main's
transport-neutral Workbench client boundary.

## Runtime boundary

```text
Browser page
  ├─ one authenticated control WebSocket
  └─ one binary WebSocket per visible terminal attachment
          │
          ▼
apps/gateway (loopback listener and framing only)
          │ two page-scoped MessagePorts
          ▼
Desktop Main (Workbench, TerminalPresentation, and lifecycle authority)
          │
          ▼
apps/terminal-host (PTY, process, VT, history, and TerminalId authority)
```

The Utility Process never imports or connects to Terminal Host. It has no shared-document writer,
binding registry, application-lifetime port, filesystem capability, Client View storage, product
settings, or terminal count policy. Desktop starts it only when local Browser development access is
enabled and kills it when Desktop shuts down; process termination revokes the lifetime capability.

## Page and attachment lifecycle

| Transition                              | Effect                                                                                                                  |
| --------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Utility start                           | Bind an ephemeral `127.0.0.1` port and accept only Desktop's exact renderer Origin.                                     |
| Page authentication succeeds            | Request one fresh Desktop ClientSession and projection-control pair.                                                    |
| Page authentication fails               | Close the socket without allocating Desktop authority.                                                                  |
| Projection opens                        | Register one derived attachment capability and transfer only the matching attachment stream.                            |
| Attachment queue exceeds its byte bound | Retire the exact attachment and release its projection; the page opens a fresh projection and receives a full snapshot. |
| Page control closes                     | Release every page attachment and the page session; preserve shared Workbench state and Host terminals.                 |
| Utility stops                           | Close listeners, pages, attachments, and transferred ports before reporting stopped.                                    |

Control and attachment messages are parsed at every process or network boundary with
`@orchbee/gateway-protocol`. Terminal frame and row payloads stay binary; the Gateway performs no VT
parsing, row reconstruction, base64 conversion, or per-cell work. Transport byte limits protect one
connection and do not impose a product count ceiling.

## Capability delivery

Desktop publishes one `ORCHBEE_BROWSER_CLIENT_URL=<url>` bootstrap line in local development. The
high-entropy capability remains in the URL fragment, so it is not sent in the renderer HTTP request.
Treat the complete URL as a process-lifetime secret: do not place it in diagnostics, telemetry,
files, history, or remote logs. The Gateway itself emits no capability-bearing diagnostic.

## Correctness evidence

| Claim                                                | Evidence                                                                                                   |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| Authentication precedes Desktop page allocation      | Pure lifecycle transition tests and real loopback WebSocket tests.                                         |
| Wrong Origin or capability creates no page authority | Real loopback WebSocket rejection tests and the Desktop-plus-Browser system trace.                         |
| Disconnect affects only one page                     | Lifecycle tests, scoped-port tests, and multi-client system reconnection.                                  |
| Attachment failure cannot corrupt another attachment | Attachment identity validation, binary codec tests, bounded queue retirement, and projection reopen tests. |
| Gateway cannot bypass Main or Host ownership         | Workspace dependency boundaries, project contracts, and absence of a Host dependency.                      |

The lifecycle is a transport refinement of the existing Desktop multi-client model; it does not add
a new canonical Workbench or terminal-lifetime state machine.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/gateway typecheck
pnpm --filter @orchbee/gateway test
pnpm --filter @orchbee/gateway build
```
