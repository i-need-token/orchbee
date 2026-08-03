# @orchbee/workbench-client-protocol

Strict, runtime-neutral data contracts for one Orchbee Workbench page client.

## Scope

The package owns validated messages for a shared Workbench client session, terminal projection
control, and the projected attachment stream opened for one visible terminal viewport. Electron,
ordinary Browser pages, and future mobile clients may carry these messages through different
transports without changing their Workbench-client meaning.

It contains no Electron, DOM, UI, broker, coordinator, transport lifecycle, network framing,
Gateway, authentication, PTY, or VT implementation. Workbench and TerminalPresentation semantics
remain with `@orchbee/workbench`; Terminal Host wire semantics remain with
`@orchbee/terminal-protocol`.

Applications parse every untrusted IPC or network value with these schemas at the first trusted
boundary. A transport owner remains responsible for framing, authentication, binary transfer,
backpressure, and connection retirement.

## Verification

Run from the repository root:

```sh
pnpm --filter @orchbee/workbench-client-protocol build
pnpm --filter @orchbee/workbench-client-protocol typecheck
pnpm --filter @orchbee/workbench-client-protocol test
pnpm --filter @orchbee/workbench-client-protocol test:exports
```
