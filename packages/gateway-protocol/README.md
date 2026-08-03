# @orchbee/gateway-protocol

Strict, runtime-neutral contracts for Orchbee's local development Browser Gateway.

## Scope

The package owns the protocol version, loopback WebSocket paths, lifetime capability schemas,
Desktop-to-Utility parent messages, page control framing, attachment binary framing, URL-fragment
bootstrap, and transport safety limits. It contains no WebSocket, Electron, Browser, Desktop Main,
Terminal Host, Workbench, UI, persistence, or privileged-effect implementation.

`@orchbee/workbench-client-protocol` remains authoritative for page-session, projection-control,
and projected-terminal messages. This package only authenticates and frames those messages for the
local Browser transport.

## Channels

| Boundary                        | Contract                                                                                                                           |
| ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| Desktop Main to Utility Process | One start/stop stream plus scoped page-open/page-close messages and exactly two transferred ports for each accepted page.          |
| Browser control WebSocket       | One authenticated connection per page carrying low-rate Workbench client-session and projection-control messages.                  |
| Browser attachment WebSocket    | One authenticated binary connection per visible projected-terminal attachment carrying only that attachment's commands and events. |
| Browser location                | Gateway port, protocol version, and lifetime capability encoded only in the URL fragment.                                          |

The control endpoint is `/v1/control`; the attachment endpoint is `/v1/attachment`. Both require
protocol `1.0`. The Gateway accepts only the exact loopback HTTP Origin supplied by Desktop Main.

## Model and invariants

| Concern        | Contract                                                                                                                                                                               |
| -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | Give an ordinary Browser page the same client meaning as Electron without granting direct Host or application-lifetime authority.                                                      |
| Inputs         | Untrusted fragment values, WebSocket messages, transferred-port messages, binary frames, socket lifecycle, and transport queue state.                                                  |
| Owned data     | One process-lifetime capability, fresh page connection identities, one-time attachment socket identities, and framing metadata.                                                        |
| Outputs        | Validated generic Workbench-client messages, scoped Desktop parent requests, typed failures, and canonical binary frames.                                                              |
| Authority      | Desktop Main owns canonical Workbench and TerminalPresentation state; Terminal Host owns PTY/VT state; the Browser owns Client View and Browser effects.                               |
| Authentication | Main creates one 256-bit lifetime capability. Attachment capabilities are derived for one socket identity and never authorize another attachment.                                      |
| Isolation      | A page receives only its client-session and projection ports. No Gateway message carries a Host connection, binding store, lifetime port, filesystem port, or generic Main capability. |
| Disconnect     | Closing a page control channel retires that page session and its attachments without changing shared topology or terminal lifetime.                                                    |
| Backpressure   | A bounded attachment queue retires only the exact attachment. Reopening projection creates a fresh attachment and full snapshot.                                                       |
| Cardinality    | No product count ceiling exists for pages, terminals, or attachments. Byte and buffered-transport limits reject the affected message or connection instead of evicting another client. |

The page lifecycle is `awaiting-authentication -> opening-client -> active -> closed`. Authentication
is the only transition that may request Desktop page ports. A late port bundle for a closed page is
disposed. Attachment registration follows `registered -> authenticated -> retired`; it cannot be
resumed or rebound.

This protocol is a transport refinement of the existing Desktop multi-client session and demand
attachment models. It adds no new product lifecycle state or canonical document transition, so no
separate TLA+ model is authoritative for this package. Deterministic lifecycle traces, strict codec
tests, and the real Electron-plus-two-Browser system trace provide the transport evidence.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/gateway-protocol build
pnpm --filter @orchbee/gateway-protocol typecheck
pnpm --filter @orchbee/gateway-protocol test
pnpm --filter @orchbee/gateway-protocol test:exports
```
