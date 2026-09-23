# Agent session target resolution

| Field             | Value                                                                                                                                                                                                  |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Direct named targets in get/explain/wait remain unimplemented; the runtime alias table, OBAP names family, and names/resolve/rename commands are implemented                                           |
| Decision owner    | The active requester directing complete Windows CLI alignment with herdr                                                                                                                               |
| Semantic owners   | Server AgentState and AgentService; native CLI target and wait execution                                                                                                                               |
| Current authority | [Canonical Agent model](../../apps/orchbee-server/docs/agent-canonical-model.md), [native CLI](../../apps/orchbee/docs/native-cli-query.md), [OBAP](../../crates/agent-observation-protocol/README.md) |
| Exit condition    | Direct target reads and waits use the existing names authority with the evidence below; owning docs converge and this proposal pair is removed                                                         |

Simplified Chinese translation: [session-names.zh-CN.md](session-names.zh-CN.md).

## Remaining behavior

Extend get/explain/wait to accept one positional auto selector or an explicit
`--session`/`--name` selector. Resolve on the held admitted Client Session under the
command's existing absolute deadline and signal ownership. Exact retained session ID
existence wins over an alias; explicit name selection escapes collisions. No second
alias authority, persistence, or terminal-text inference is introduced.

Pin the resolved full session ID once before reading or polling. Renaming, releasing,
or reusing that name must never retarget a held waiter. Disposal, poison, missing
names, denied resolution, malformed replies and lost transport remain typed failures.
Resolve replies may provide only their own source identity; never invent a get reply
or a source for an operation that was not executed.

## Evidence and boundaries

Add real transport/process witnesses for resolution followed by get/explain on one
session, one total deadline and cancellation owner, and name reassignment during a
wait. Keep explicit session addressing compatible and verify ID/name collisions.
Existing names service, schema and codec witnesses remain the alias authority evidence.
Agent start reservation and direct Agent input commands are separate report gaps;
this proposal does not claim they are implemented.
