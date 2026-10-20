# Retained operation status

## Command model

Async receipts are addressed by their original request identity within the selected Session.
Use a nonzero lowercase 32-digit hexadecimal `<request-id>`, not a resource label or selector.
Each family uses `status <request-id>`; supported retries use
`resume <request-id> --generation <n> --confirm`.

| Family    | Status                          | Resume                                                     | Identity from receipt                             |
| --------- | ------------------------------- | ---------------------------------------------------------- | ------------------------------------------------- |
| Workspace | `workspace status <request-id>` | `workspace resume <request-id> --generation <n> --confirm` | `closeRequestId`                                  |
| Tab       | `tab status <request-id>`       | `tab resume <request-id> --generation <n> --confirm`       | `closeRequestId`                                  |
| Panel     | `panel status <request-id>`     | `panel resume <request-id> --generation <n> --confirm`     | `closeRequestId`                                  |
| Project   | `project status <request-id>`   | `project resume <request-id> --generation <n> --confirm`   | `operationId`                                     |
| Worktree  | `worktree status <request-id>`  | `worktree resume <request-id> --generation <n> --confirm`  | `operationId`                                     |
| Client    | `client status <request-id>`    | Unavailable                                                | `operationId`                                     |
| Server    | `server status <request-id>`    | Owner-specific explicit retry                              | `requestId` for stop; `operationId` for manifests |

Project and Worktree resume retain their optional timeout and detach controls. Generation is a
positive decimal integer; Project and Worktree attempts are bounded to 64. Close generations
retain the protocol's full-width bound. Status reads never resume, relaunch or acquire focus.
For multi-stage receipts, use the identity of the particular stage and its owning family.

## Close lookup and recovery

Close status reads the retained Server ledger by exact request ID, including tombstones after
layout membership disappears. A family mismatch refuses instead of selecting another resource.
The new matched-cohort read has no persistent index or extra state and preserves Site grants.
Missing, denied and unavailable observations remain distinct; missing history does not prove
that an earlier effect did not happen.

`orchbee.operation.close` v1 contains the original identity, family, correlated observation and
nullable resume receipt. Resume first observes the exact target, then submits once with a fresh
transport request ID and the caller's generation fence. Read and resume share one five-second
budget. Returned, definitely not submitted and unknown submission are distinct; unknown is exit
8 and must be inspected before another attempt. Cancellation preserves obtained facts and stops
local waiting. Generation checks remain owned by the existing close state machine.

Server status checks the durable stop receipt first so it works after shutdown. Only a definite
stop-record miss queries manifest operation status. Stop owns precedence if callers reused one
identity across both domains. An unavailable stop observation is preserved without guessing.
Client, Project, Worktree and Server retain their existing result schemas and failure semantics.

## Migration and evidence

The uniform paths are the preferred interface. Existing family-specific status and resume paths
remain declared compatibility entry points for saved scripts and historical recovery hints.
Their original parameter contracts remain unchanged; new examples use the uniform paths.
No automatic retry, generic resume of unsupported operations, or operation replay is introduced.

Parser cases cover exact IDs, missing operands, resource-selector rejection and confirmation
fences. Rust and TypeScript share the command conformance corpus with JSON Schema; it includes
request-only lookup, each target kind, denied/unavailable/missing results and closed tombstones.
Server authority cases cover retained identity and grant checks. Local verification follows the repository affected-host gate.

One-shot Client notices, native notices and settled setup results expire after four seconds on the existing monotonic UI tick. Identical notices coalesce without extending their lifetime. Pending work, unknown outcomes, recovery-required results and persistent configuration warnings remain with their owning workflows; expiry never discards receipts or retries an operation.
