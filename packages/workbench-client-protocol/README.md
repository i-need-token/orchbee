# @orchbee/workbench-client-protocol

Strict, runtime-neutral data contracts for one Orchbee Workbench page client.

## Scope

The package owns validated messages for a shared Workbench client session, terminal projection control, and the projected attachment stream opened for one visible terminal viewport. Electron, ordinary Browser pages, and future mobile clients may carry these messages through different transports without changing their Workbench-client meaning.

The projected attachment stream carries semantic bell events without choosing a sound, visual
flash, or notification policy. Each event retains the Host attachment identity and terminal-owned
sequence so the projected terminal can suppress stale or duplicate delivery.

The same stream carries a complete attachment-scoped resize-lease ownership change. A renderer
validates both attachment and terminal identities before dynamically enabling or disabling Host
resize requests; the immutable attachment bootstrap is not rewritten when ownership changes.

Feature failures retain their semantic operation and retryability across both Electron and Gateway
transports. A Host response additionally retains its exact correlation ID, while client-owned copy
and link effects retain their request sequence. These values identify consequences and diagnostics;
they never authorize automatic replay of input or another user effect.

Projection release remains correlated after the attachment stream closes. The client retains the
request identity until Main returns either `terminal-projection-released` or the exact typed cleanup
failure. The success acknowledgement transfers no port; a late cleanup result therefore settles only
its released request and cannot be mistaken for an unknown message that retires unrelated projections.

It contains no Electron, DOM, UI, broker, coordinator, transport lifecycle, network framing, Gateway, authentication, PTY, or VT implementation. Workbench and TerminalPresentation semantics remain with `@orchbee/workbench`; Terminal Host wire semantics remain with `@orchbee/terminal-protocol`.

Applications parse every untrusted IPC or network value with these schemas at the first trusted boundary. A transport owner remains responsible for framing, authentication, binary transfer, backpressure, and connection retirement.

Each client-session bootstrap carries `terminalHostName`, supplied by the application runtime that
owns the projected terminal machine. The shared Web client uses that trusted value to validate OSC
7 current-directory metadata; it never substitutes the Browser URL hostname, Gateway peer name, or
renderer input. Projected frames carry the complete nullable natural title and current-directory
observation. The protocol transports those values without selecting Tab-title precedence,
persistence, or URI trust policy; `@orchbee/workbench` owns those decisions.

Projected frame and row-window events accept either an exact `ArrayBuffer` or a bounded `Uint8Array` view so in-process adapters can preserve a validated payload view. Before structured-cloning a view into a less-trusted capability, the transport owner must copy its visible bytes into an exact-size `ArrayBuffer`; structured clone carries the complete backing buffer, including bytes outside the view. Receivers still honor the declared byte offset and length, while transport trust and terminal-protocol validation remain separate responsibilities.

Selection export remains attachment-scoped. A Desktop-effect attachment receives `selection-copied` after Main commits the clipboard effect; a client-effect attachment receives `selection-copy-content` with bounded UTF-8 bytes and commits those bytes through its own platform clipboard before reporting local completion. Neither event grants a generic clipboard capability.

Host-originated clipboard-write review is a separate, request-bound flow. Main forwards at most one pending bounded request per attachment, and the renderer can return only an exact `clipboard-write-decision` containing the original attachment, sequence, and text plus the user's boolean decision. Desktop Main rejects mismatched and duplicate decisions and performs the privileged write only for an approved Desktop-effect attachment. A client-effect attachment performs its local Browser write first and uses the same exact decision only to consume Main's pending request, so Main cannot duplicate the effect.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```
