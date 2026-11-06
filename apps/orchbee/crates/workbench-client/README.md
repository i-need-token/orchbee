# Orchbee Workbench Client

Private Workbench Client runtime for the [Orchbee native application](../../README.md). Only that
application consumes this crate.

## Boundary

The `orchbee` binary is the entry layer. It owns process entry, command-line grammar, CLI text and
machine output, Agent doctor diagnostics, startup and release installation, and the terminal shell
loop and backend that hold the host terminal. This crate is the Client runtime those entries
compose. It never depends on the binary, never parses invocations, and never writes command output.

Every implementation module is private. [`src/lib.rs`](src/lib.rs) re-exports, by explicit name,
only the items the application's production code uses, through the `application` facade. Each
facade submodule is the top-level implementation module that owns its items; a nested submodule
appears only where that owner's children export the same names, such as `Error` or `read`. The
facade never mirrors deeper implementation paths, re-exports a whole module, or uses a glob. Items
the application does not name keep crate or narrower visibility.

## Contents

- Workbench Client: verified Session activation, projected Workbench state, the mailbox and
  connection lanes, views, notices, input dispatch, Settings, and pane presentation.
- Terminal presentation: inherited terminal admission and mode lifecycle, projection, copy mode,
  links, graphics, themes, layout, pointer interaction, and rendering.
- Agent workflows: provider integration inspection and effects, runtime observation, and Agent
  start.
- Machines: saved-machine discovery, pickers, SSH carriers and tunnels, and observation.
- Client-local configuration, localization of runtime copy, notifications, onboarding, and
  termination signals.

## Dependency direction

The application depends on this crate; this crate never depends on the application target. It
depends on the sibling [local client](../local-client/README.md) for Client Connection transport
and on shared protocol and runtime crates under [`crates/`](../../../../crates). When runtime
behavior needs an entry-layer value, such as an exit mapping or a shell painter, the application
passes it in.

Modules inside the crate follow the [module layers](docs/module-layers.md): each module's layer,
the allowed reference direction, state ownership, and component placement.
The [TUI components](docs/tui-components.md) reference inventories the shared primitives and
recurring compositions.

## Background threads

Every Client background thread is owned through `background_thread::BackgroundThread`, under the
exit contract that the Worker's background jobs follow:

- `request_stop` asks an owner to stop and never waits.
- `stop_before(deadline)` asks the owner to stop and joins its thread only after the thread has
  ended. A thread still running at the deadline reports `TimedOut` and stays owned.
- Dropping an owner never blocks. It requests stop, joins a thread that has already ended, and
  detaches a running thread, which then ends on its own.

At process exit, the terminal shell asks every owner to stop before it waits for any of them: the
active and cached Clients, the machine connection attempt and observers, and the
system-notification owner. It then observes each owner against one deadline, `EXIT_BUDGET`
(10 seconds), which is the longest deadline that an owner can still be inside after it observes a
stop. Stop requests overlap, so a normal exit is not slower. A thread still running at the
deadline is detached, and the exit reports a cleanup failure instead of success.

Only process exit has this total bound. Reconnection and retirement still join the connection task
to recover its pending results, and an owner's explicit `close` keeps its bounded join.

A thread detached at the deadline can abandon this work when the process ends:

- The cancellation and reservation release of an in-flight Agent start. The reservation can stay
  pending until the Server expires it.
- The last position-cache write.
- A Windows clipboard delivery.
- The result of close-status recovery.

The saved-machine relay belongs to the retained-carrier guard of the sibling transport and stops
with that guard.

## Test support

Adjacent private unit tests own runtime evidence, and fixtures used only by them stay
`#[cfg(test)]`. The `test-support` feature exposes `test_support` modules for tests that the
application owns because they reach its shell loop, backend, or command layer:

- fake Server peers and native Workbench Client construction for shell-loop tests;
- terminal lifecycle fakes and ordered wire recorders for backend tests;
- Settings and pane graphics scenarios that take the application's real painter or backend while
  keeping their assertions on private runtime state;
- the frozen provider fixture packs, which take the application's integration exit mapping, with
  their fault-injection seams.

The root `test_support` facade exists only under `test` or `test-support` and re-exports, by
explicit name, what the application's tests use beyond the production facade. Items only tests use
carry the same gate. When a test needs a crate-internal member of a production type, the member
stays `pub(crate)` and a gated `<Type>TestAccess` trait beside the type delegates to it; tests
import the trait from the facade. Tests build values whose fields stay crate-internal from gated
`<Type>Fixture` structs through `From`.

Only the application's development dependency enables the feature, together with
`orchbee-local-client/test-support`. Release builds and the production boundary never include it,
and application code consumes only the production boundary.

## Benchmarks

[`benches/frame_pipeline`](benches/frame_pipeline/main.rs) times TUI frame-pipeline stages through
the `application` facade and the public protocol crates, following the
[benchmark rules](../../../../docs/TESTING.md#benchmarks). Run it from the repository root:

```sh
cargo bench -p orchbee-workbench-client --bench frame_pipeline --locked
cargo bench -p orchbee-workbench-client --bench frame_pipeline --locked -- apply/delta --samples 100
```

| Case                | Phase  | Measured operation                                                                                                                              |
| ------------------- | ------ | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `decode/*`          | both   | `decode_terminal_text_frame` on one OBTF payload; `snapshot` is initialization, `delta-*` steady state.                                         |
| `apply/init`        | init   | A new `TerminalProjection` receiving its first complete snapshot record.                                                                        |
| `apply/full`        | steady | A live projection receiving the next complete snapshot record, as after a resynchronization.                                                    |
| `apply/delta-*`     | steady | A live projection receiving the next delta record: envelope and route checks, decoding, row merge, row validation, and `FrameApplied` feedback. |
| `layout/init`       | init   | A new `ClientView` for the window and `terminal_geometry` for its first pane, as at startup.                                                    |
| `layout/frame`      | steady | The shell projection a drawn frame recomputes: `project_shell_areas`, `active_pane_area`, and `native_terminal_area` for a split Tab.           |
| `paint/init`        | init   | A new `TestBackend` terminal's first `Terminal::draw` of the shell; its diff writes every cell.                                                 |
| `paint/frame`       | steady | The next `Terminal::draw` of an unchanged shell: paint, diff, backend cell writes, and buffer swap.                                             |
| `paint/buffer`      | steady | The paint closure alone: `render_terminal_shell_with_resize` into a reset buffer.                                                               |
| `multi/<n>/init`    | init   | `n` new projections each receiving a first snapshot at its pane size, then a new terminal's first draw.                                         |
| `multi/<n>/delta-1` | steady | `n` live projections each receiving a one-row delta in the same turn, then the next draw.                                                       |

Records are deterministic synthetic OBTF frames with ASCII, double-width CJK, and styled runs. A
delta replaces one row (`delta-1`), a quarter of the rows (`delta-quarter`), or every row
(`delta-all`). `decode` and `apply` frames fill 80×24, 120×32, and 240×64 cells; the other cases use
those sizes as the host window. `multi` divides the window's split-tree root area into an even grid
of 1, 2, 4, or 8 panes (1×1, 2×1, 2×2, or 4×2) with one divider cell between neighbors, an
approximation of the panel tree.

Painting renders the Client shell through `render_terminal_shell_with_resize` into ratatui's
`TestBackend`. A Workbench Client view has no stable public constructor, so the shell paints its
unavailable-Workbench content: Workbench Tabs, pane frames and dividers, terminal cells, and
Workbench overlays are not painted, and the panel-tree layout that places split panes is not
measured. No case includes transport reads, backend escape encoding, or console output; the
[TUI timing observations](../../docs/native-terminal-environment.md#tui-timing-observations) cover
the actual loop.

Every call is timed separately, and state that a case returns is released after its clock stops;
the header reports the clock-read cost. Each row reports, per call, a single cold first call and the
minimum, median, 90th percentile, and median absolute deviation over samples, plus input bytes and
median throughput. `--list` prints the cases; `--samples <n>`, `--sample-ms <n>`, and
`--warmup-ms <n>` default to 50, 2, and 300; other arguments select cases by substring.

## Maintenance

The crate inherits the repository toolchain, lints, and shared dependency versions. Follow
[Rust ownership and testing rules](../../../../docs/RUST.md) and the
[repository verification gate](../../../../docs/VERIFICATION.md) from the repository root.
Keep exported modules and items tied to real application consumers and preserve private test
ownership.

## Refused shared preference recovery

Opening Settings after an `invalid-source` refusal keeps a populated defaults view and shows the
dedicated unreadable-file notice in both locales. These are recovery defaults, not confirmed
storage. Ordinary editing stays disabled; Reload and confirmed Reset all remain available. Reset
all stages a full-null reset, saved through the ordinary explicit Save boundary with the virtual
`unreadable` basis. Storage rechecks the current refused record and preserves a concurrent repair.
The [shared preference contract](../../../../crates/workbench-protocol/docs/user-preferences.md#explicit-recovery-from-a-refused-record)
owns token and storage rules. No read silently repairs or migrates a record.
