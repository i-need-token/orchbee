# Retained operation status

## Command model

Async receipts are addressed by their original request identity within the selected Session.
Use a nonzero lowercase 32-digit hexadecimal `<request-id>`, not a resource label or selector; no
family word names the store that keeps it.

| Path                                                                                   | Effect                                      |
| -------------------------------------------------------------------------------------- | ------------------------------------------- |
| `operation status <request-id>`                                                        | Reads the retained operation; never resumes |
| `operation resume <request-id> --generation <n> --confirm [--detach] [--timeout <ms>]` | Submits the owner's resume once             |

Both take `--format text|json`.

The identity comes from the receipt: `closeRequestId` for a Workspace, Tab or Panel close,
`operationId` for a Project close-all, a Worktree operation, a Client control or an Agent detection
rule operation, and `requestId` for a Session stop. The command line sends every exchange about a
Project close-all, a Worktree operation, a Client control or an Agent detection rule operation
under its `operationId`, so each `requestId` such a receipt prints is that same identity. For
multi-stage receipts, use the identity of the particular stage. Status reads never resume, relaunch or acquire focus. `operation status` is
an observation command and, like `operation resume`, may be routed with `--machine`.

## Lookup

`operation status` asks the stores that keep operations in this fixed order and stops at the first
that does not answer a definite miss: the Session stop receipt, the close ledger (Workspace, Tab
and Panel closes), Project close-all, Worktree operations, Client controls, and Agent detection rule
operations. The store that keeps the identity answers with its own document and exit code:
`orchbee.server.stop`, `orchbee.operation.close`, `orchbee.project.close-all`,
`orchbee.worktree.operation`, `orchbee.client.control` or `orchbee.server.agent-manifests`. A
denied, unavailable, unknown, timed-out or interrupted answer, or a transport failure, ends the
lookup and is the result. With no Session the stop receipt is still read, so a completed stop
answers after shutdown; a miss there gives the usual no-Session failure (exit 4). An identity that
a caller reused across stores resolves to the first store in this order.

When every store misses, the result is `orchbee.cli.failure` with kind `not-found`, details
`{stage:"operation-status", reason:"not-recorded"}` and exit 3. Missing history does not prove that
an earlier effect did not happen.

## Close records

The close ledger is read by exact request ID, including tombstones after layout membership
disappears. The read has no persistent index or extra state and preserves Session grants.
`orchbee.operation.close` v1 contains the original identity, `family` (`workspace`, `tab` or
`panel` from the retained target, null when the answer has none), the correlated observation and a
nullable resume receipt. Missing, denied and unavailable observations stay distinct.

An unfinished close continues by itself on the Server. Each attempt has 30 seconds; when one ends
with the close still resumable, the Server starts the next, up to about three 30-second attempts in
all, and status keeps reporting `closing` meanwhile. Only after the last one does status report a
single failure, `cleanup-blocked`. `operation resume` is for a script that wants to continue a
close that finally stopped. The
[Server close contract](../../orchbee-server/docs/terminal-tab-close.md#automatic-renewal) owns
this allowance.

## Resume

`operation resume` finds the operation as `operation status` does, then submits the owner's resume
once. A close resume first observes the exact target, then submits with a fresh transport request
ID and the caller's generation fence; read and resume share one five-second budget and the command
returns once the resume is submitted. Returned, definitely not submitted and unknown submission are
distinct; unknown is exit 8 and must be inspected before another attempt. A Project or Worktree
resume keeps that owner's wait: `--detach` returns after admission and `--timeout` (1 to 3600000
ms, default 120000) bounds the wait; neither changes a close resume. Generation is a positive
decimal integer passed to the owner unchanged; a Project or Worktree request refuses an attempt
above 64 before anything is submitted.

A Session stop, a Client control or an Agent detection rule operation cannot be resumed: the
command refuses before any submission with kind `rejected`, details
`{stage:"operation-resume", reason:"resume-unsupported"}` and exit 3. For a stop the text names
`orchbee server stop --retry --request-id <id>`, the stop's own explicit retry. Cancellation
preserves obtained facts and stops local waiting. The CLI never retries on its own, resumes an
unsupported operation or replays one.

## Evidence

One-shot Client notices, native notices, settled setup results and the success notice of a hidden Opened Worktree result whose terminal entry finished expire after four seconds on the existing monotonic UI tick. Identical notices coalesce without extending their lifetime. Pending work, unknown outcomes, recovery-required results and persistent configuration warnings remain with their owning workflows; expiry never discards receipts or retries an operation.
