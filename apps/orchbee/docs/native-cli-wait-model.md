# Wait-output settlement contract

`src/cli/query.rs` owns one `terminal wait-output` invocation's deadline, cancellation and cleanup settlement. This is a client-owned sequential workflow with controlled transport completions; its evidence is implementation testing.

## Settlement obligations

- Sample the shared cancellation/deadline gate before preamble, attach and every poll, feedback or row-read exchange.
- A successful attach creates one exact viewer cleanup obligation. Failed attach cannot fabricate one.
- Consume a fresh read sequence before the step deadline check; stale replies use a newer sequence and exhaustion never wraps.
- Match only a consistent, current window after the settlement gate.
- Detach records actual cleanup provenance. A late flag sample may revise the primary outcome but never rewrite cleanup already observed.
- Emit at most one final answer; unknown effects do not cause automatic replay.

## Evidence and limits

`wait_tests` in `src/cli/query.rs` exercises the production settlement decisions. `tests/cli_query.rs` drives a scripted OBCS viewer with real frame/row records, increasing request sequences, stale re-pin, deadline and cleanup outcomes. `tests/local_startup_process` supplies real cohort observations and cancellation/slot-release evidence.

Run the owning Cargo tests. These witnesses cover selected completion orders and concrete protocol boundaries; they do not claim exhaustive independent Server/Worker schedules or real-time guarantees. Shared viewer ownership and relay isolation are separately registered Server protocols.
