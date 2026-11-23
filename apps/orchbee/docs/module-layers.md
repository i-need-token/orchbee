# Application Module Layers

Status: accepted layer and lateral-import contract for the `orchbee` binary crate. Current code
does not conform yet; `pnpm lint:rust-modules` passes the cycles and upward references that the
[Rust module record](../../../docs/LINTING.md#rust-module-record) lists and fails any new one.

This document is the owner-local contract that
[module boundaries](../../../docs/MODULE-BOUNDARIES.md#native-source-trees) requires for the
binary's source tree under `src/`. The binary consumes the
[Workbench Client](../crates/workbench-client/README.md) only through its `application` facade.

## Layers

The binary uses the [Workbench Client layers](../crates/workbench-client/docs/module-layers.md#layers)
and their [dependency rules](../crates/workbench-client/docs/module-layers.md#dependency-rules). A
command flows upward through them: `values` declare the grammar, typed requests, and usage failures;
`workflow` modules perform command effects; `presentation` renders typed results; `composition`
dispatches requests and owns process exit, the TUI event loop, and startup wiring. Each module
names its layer in its own file, as those rules describe.

## Order inside `cli`

Every `cli` child inherits `composition`, so only the no-cycle rule orders them. These invariants
keep that order:

- The result model, `results`, `output`, and `terminal_observation`, never names a
  command owner. A query document whose owner composes other result values and renders its own text
  enters `results::Projection` as a `results::OwnedDocument`.
- `present` renders result values only. Command text that needs a command owner's types stays with
  that owner; a mutation `Receipt`, for example, renders itself.
- `query` reads, resolves, and exchanges single steps and never calls a command workflow. Workflows
  that combine a read with Client attention, input submission, or plugin activation live in their
  own siblings, such as `agent_wait`, `agent_list`, and `terminal_run`. Helpers they share live in
  lower siblings, such as `link_activation`, `plugin_preparation`, `layout_topology`, and
  `client_control::exchange`.
- Failure schema identifiers that several owners report live in `invocation::grammar::schemas`.

## Order inside `invocation`

Every `invocation` child inherits `values`, so only the no-cycle rule orders them. One parse runs
down through these groups:

- `argv` admits the argument envelope and calls `machine_routing`, which wraps `scan`; `scan` runs
  the lexical pass and calls `dispatch`; `dispatch` maps command words to feature parsers.
- Feature parsers own their commands' operands and never call `dispatch`, `scan`,
  `machine_routing`, or `argv`.
- The shared option decoders, `option_scope`, `target_syntax`, `launch_options`,
  `workbench_options`, `wait_options`, `send_input`, `agent_options`, and `server_stop`, never
  call a command's feature parser.
- `help` derives help paths, help requests, and usage option candidates from `grammar`; `grammar`
  never reaches `help`.
- `failure` holds the failure vocabulary and its constructors and names no sibling. `handles`
  prints the compact selectors that `target_syntax` accepts and names no sibling either. The root holds
  the typed request vocabulary and the `Options` scan state.

## Maintenance

A new top-level module names its layer in the change that adds it. Run
`pnpm lint:rust-modules -- --details --member apps/orchbee` for every cycle and every upward
reference with sample source locations; [Linting](../../../docs/LINTING.md) owns the command.
