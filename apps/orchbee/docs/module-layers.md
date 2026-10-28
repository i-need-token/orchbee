# Application Module Layers

Status: accepted layer and lateral-import contract for the `orchbee` binary crate. Current code does
not conform yet; `pnpm lint:rust-modules` reports the remaining cycles and upward references
without failing.

This document is the owner-local contract that
[module boundaries](../../../docs/MODULE-BOUNDARIES.md#native-source-trees) requires for the
binary's source tree under `src/`. The binary consumes the
[Workbench Client](../crates/workbench-client/README.md) only through its `application` facade.

## Layers

The binary uses the [Workbench Client layers](../crates/workbench-client/docs/module-layers.md#layers)
and their [dependency rules](../crates/workbench-client/docs/module-layers.md#dependency-rules). A
command flows upward through them: `values` declare the grammar, typed requests, and usage failures;
`workflow` modules perform command effects; `presentation` renders typed results; `composition`
dispatches requests and owns process exit, the TUI event loop, and startup wiring.

## Module assignment

Each row is the target layer. A note names the part that must move when a module mixes layers
today.

| Module                 | Layer          | Role and reason                                                                                                                                                                              |
| ---------------------- | -------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `agent_doctor`         | `workflow`     | Offline provider-hook diagnostics over bounded filesystem facts; `fs` is its only I/O boundary.                                                                                              |
| `agent_doctor::probes` | `values`       | The pure classification half of Agent doctor.                                                                                                                                                |
| `agent_projection`     | `values`       | Deterministic join of canonical sessions and physical Agent incarnations for Agent output.                                                                                                   |
| `cli`                  | `composition`  | Dispatches admitted requests to feature owners and assembles stdout, stderr, failures, and process outcomes. Per-command effects and text move out to `workflow` and `presentation` modules. |
| `cli_text`             | `presentation` | Human-readable help and result text over typed results.                                                                                                                                      |
| `help`                 | `presentation` | Help documents and their display-cell layout.                                                                                                                                                |
| `invocation`           | `values`       | Declared command grammar, typed requests, and structured usage failures; parsing is a deterministic function of the arguments.                                                               |
| `local_startup`        | `composition`  | Wires local activation, terminal admission, startup waiting, and the TUI shell.                                                                                                              |
| `presentation`         | `presentation` | Launch, version, and usage output projection.                                                                                                                                                |
| `release_store`        | `workflow`     | Release staging adapter over the filesystem.                                                                                                                                                 |
| `startup_wait`         | `workflow`     | Bounded wait for presentable terminal geometry with cancellation and mode restoration.                                                                                                       |
| `tui_backend`          | `workflow`     | Host-terminal frame writer adapter.                                                                                                                                                          |
| `tui_shell`            | `composition`  | TUI event loop, surface switching, input polling, and draw settlement.                                                                                                                       |

## Maintenance

Add, remove, rename, or move a module together with its row in the same change. Run
`pnpm lint:rust-modules -- --details --member apps/orchbee` for every cycle and every upward
reference with sample source locations; [Linting](../../../docs/LINTING.md) owns the command.
