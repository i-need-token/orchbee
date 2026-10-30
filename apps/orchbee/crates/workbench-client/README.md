# Orchbee Workbench Client

Private Workbench Client runtime for the [Orchbee native application](../../README.md). Only that
application consumes this crate.

## Boundary

The `orchbee` binary is the entry layer. It owns process entry, command-line grammar, CLI text and
machine output, Agent doctor diagnostics, startup and release installation, and the terminal shell
loop and backend that hold the host terminal. This crate is the Client runtime those entries
compose. It never depends on the binary, never parses invocations, and never writes command output.

Every implementation module is private. [`src/lib.rs`](src/lib.rs) re-exports, by explicit name,
only the items the application's production code uses, through the `application` facade whose
submodules mirror the implementation module paths. The facade never re-exports a whole module or
uses a glob. Items the application does not name keep crate or narrower visibility.

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

## Maintenance

The crate inherits the repository toolchain, lints, and shared dependency versions. Follow
[Rust ownership and testing rules](../../../../docs/RUST.md) and the
[repository verification gate](../../../../docs/VERIFICATION.md) from the repository root.
Keep exported modules and items tied to real application consumers and preserve private test
ownership.
