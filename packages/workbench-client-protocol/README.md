# @orchbee/workbench-client-protocol

Strict, runtime-neutral data contracts for one Orchbee Workbench page client.

## Scope

The package owns validated messages for a shared Workbench client session, terminal projection control, and the projected attachment stream opened for one visible terminal viewport. Electron, ordinary Browser pages, and future mobile clients may carry these messages through different transports without changing their Workbench-client meaning.

Every `terminal-projection-open-requested` command carries one complete positive
`TerminalGeometry`. It is the caller's measured mounted geometry for a visual surface, not an
optional resize hint or fallback. Request identity therefore includes presentation identity and
geometry: an exact duplicate may replay, while reuse with different geometry is conflicting control
traffic. The transport contract supplies no timeout or 80x24 default.

The projected attachment stream carries semantic bell events without choosing a sound, visual
flash, or notification policy. Each event retains the Host attachment identity and terminal-owned
sequence so the projected terminal can suppress stale or duplicate delivery.

The same stream carries a complete attachment-scoped resize-lease ownership change. A renderer
validates both attachment and terminal identities before dynamically enabling or disabling Host
resize requests; the immutable attachment bootstrap is not rewritten when ownership changes.

Live input authority uses a separate `attachment-input-capability-changed` event containing the
complete Host-committed attachment capability state and positive revision. The transport cannot
request or synthesize this mutation. A client validates the attachment and terminal identities and
applies only an exact replay or exact-next changed input value; stale delivery is inert and a
conflict or revision gap fails closed. Downgrade makes PTY Actions unavailable synchronously while
history, search, selection, and copy remain local output interactions. Upgrade carries no queued
input and grants only future input stamped with the new revision.

Feature failures retain their semantic operation and retryability across both Electron and Gateway
transports. A Host response additionally retains its exact correlation ID, while client-owned copy
and link effects retain their request sequence. These values identify consequences and diagnostics;
they never authorize automatic replay of input or another user effect.

Projection release remains correlated after the attachment stream closes. The client retains the
request identity until Main returns either `terminal-projection-released` or the exact typed cleanup
failure. The success acknowledgement transfers no port; a late cleanup result therefore settles only
its released request and cannot be mistaken for an unknown message that retires unrelated projections.
Because command delivery and asynchronous open completion are independently scheduled, a correlated
`terminal-projection-opened` may reach the client after that client requested release. The client
closes exactly the transferred attachment port, keeps the released request identity until cleanup
settles, and leaves the shared control transport and unrelated requests available. A missing or extra
transferred port, conflicting identity, or duplicate open remains invalid control traffic and retires
the affected control transport.

It contains no Electron, DOM, UI, broker, coordinator, transport lifecycle, network framing, Gateway, authentication, PTY, or VT implementation. Workbench and TerminalPresentation semantics remain with `@orchbee/workbench`; Terminal Host wire semantics remain with `@orchbee/terminal-protocol`.

Applications parse every untrusted IPC or network value with these schemas at the first trusted boundary. A transport owner remains responsible for framing, authentication, binary transfer, backpressure, and connection retirement.

Each client-session bootstrap carries `terminalHostName`, supplied by the application runtime that
owns the projected terminal machine. The shared Web client uses that trusted value to validate OSC
7 current-directory metadata; it never substitutes the Browser URL hostname, Gateway peer name, or
renderer input. Projected frames carry the complete nullable natural title and current-directory
observation. The protocol transports those values without selecting Tab-title precedence,
persistence, or URI trust policy; `@orchbee/workbench` owns those decisions.

Projected frame and row-window events accept either an exact `ArrayBuffer` or a bounded `Uint8Array` view so in-process adapters can preserve a validated payload view. The visible payload is bounded by `maxFrameBytes` for frames and `maxRowWindowBytes` for row windows; a view's unrelated backing bytes do not count toward that limit. Before structured-cloning a view into a less-trusted capability, the transport owner must copy its visible bytes into an exact-size `ArrayBuffer`; structured clone carries the complete backing buffer, including bytes outside the view. Receivers still honor the declared byte offset and length, while transport trust and terminal-protocol validation remain separate responsibilities.

Selection export remains attachment-scoped and plain-text-only. The projected command has no format
selector. A Desktop-effect attachment receives `selection-copied` after Main commits the clipboard
effect; a client-effect attachment receives `selection-copy-content` with bounded UTF-8 bytes and
commits those bytes through its own platform clipboard before reporting local completion. Neither
event grants HTML, raw VT, or a generic clipboard capability.

Host-originated clipboard-write commit is a separate, request-bound flow. Main forwards at most one pending bounded request per attachment, and the renderer automatically returns only an exact `clipboard-write-decision` containing the original attachment, sequence, and text plus the commit result. Desktop Main rejects mismatched and duplicate decisions and performs the privileged write only for a positive Desktop-effect decision. A client-effect attachment performs its local Browser write first and uses the same exact decision only to consume Main's pending request, so Main cannot duplicate the effect.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```
