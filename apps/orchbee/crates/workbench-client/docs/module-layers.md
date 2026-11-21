# Workbench Client Module Layers

Status: accepted layer and lateral-import contract for this crate. Current code does not conform
yet; `pnpm lint:rust-modules` passes the cycles and upward references that the
[Rust module record](../../../../../docs/LINTING.md#rust-module-record) lists and fails any new one.

This document is the owner-local contract that
[module boundaries](../../../../../docs/MODULE-BOUNDARIES.md#native-source-trees) requires for this
crate. The [application binary](../../../docs/module-layers.md) applies the same layers to its own
modules. The [sibling transport](../../local-client/README.md#module-direction) keeps a separate
lateral list.

## Layers

Layers are listed from lowest to highest.

| Layer          | Owns                                                                                                                                                                                                                                                                                      | Never                                                                                                      |
| -------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `values`       | Identities, tokens, typed facts, geometry, component control-state types, action definitions and their discovery metadata, localized message catalogs, theme roles, and deterministic functions over them, such as event decoding and text projection                                     | Owns state that outlives a call, performs I/O, or reads clocks or the environment                          |
| `state`        | The single writer of one piece of Client state and its synchronous transitions, classified under [State owners](#state-owners)                                                                                                                                                            | Performs transport, terminal, or process I/O, paints, starts workflows, or mints canonical Workbench facts |
| `workflow`     | Multi-step and effectful operations: the connection task and lanes, observation and polling jobs, activation, Agent start, terminal-mode and exit lifecycles, and the platform-named adapters through which they reach transport, host terminal, signals, clipboard, files, and processes | Paints, routes input, or writes another owner's state except by settling its own outcome into that owner   |
| `presentation` | Read-only view models derived from state and workflow status, layout geometry, render, the frame hit map and interaction projection that render records, and the shared TUI components; layout never references render                                                                    | Mutates feature state, starts effects, or resolves input                                                   |
| `input`        | Delivery of decoded host input to the current capture owner, pointer gestures resolved against the last published frame, binding resolution, and conversion to feature intents or actions                                                                                                 | Owns feature state or performs effects other than through the intents and actions it emits                 |
| `composition`  | Lifetimes and wiring: the Workbench Client connection-task and UI-loop composition, execution of feature actions, port implementations, and the crate's export facade                                                                                                                     | Holds a business rule that a lower layer can own                                                           |

## Dependency rules

- A module may reference its own layer and lower layers. A lower layer never references a higher
  one, whether by path, `use`, re-export, or type alias.
- A module names its layer in a `// module-layer: <id>` line in the comment block that opens its
  file, or that opens its body when it is inline. A module without the line inherits its nearest
  ancestor's layer. The line is the target layer; a note under it names the part that must move
  when the module mixes layers today. A parent never re-exports or uses a descendant assigned to a
  higher layer.
- References within one layer stay acyclic. When two modules need each other, move the shared part
  into the lower of the two or into a lower layer.
- A value that a higher module produces and a lower module holds is defined below both: in a
  `values` module, or in a child of the producer assigned to a lower layer, which the producer may
  re-export for its own callers. Lower modules name such an item by its defining module, never
  through a higher module's re-export or the `workbench_client` facade.
- External protocol, runtime, and rendering crates sit below `values`. The sibling Client Connection
  transport's handles are reachable only through `local_client`, a `workflow` module; state owners
  settle its exchange outcome values without holding a handle, and sample time through their own
  clock port, which `composition` implements for the connection's operation clock.
- Operating-system `cfg` selection belongs in `workflow` adapters whose path names the platform or
  capability, and in `composition`; the `host_capabilities` admission table is the one `values`
  exception. Other modules stay platform-neutral.
- The application consumes this crate only through `application`; nothing inside the crate
  references it. Code under `cfg(test)` or the `test-support` feature stays outside the layer graph
  and never widens production visibility.

## State owners

Every piece of Client state has exactly one writing module. Other modules read it through shared
borrows or receive derived values; render reads only the derived view model.

| Source               | Writer contract                                                                                                                                                         | Owners                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Server projection    | Applies only validated Server or Worker results and events; never mints canonical membership or infers physical cleanup from a missing projection                       | `workbench_projection`, `workbench_state`, `agent_board`, `terminal_projection`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| Client-local view    | Owns chrome, selection, focus, drafts, menus, notices, copy mode, preferences, and positions that exist only in this Client; persists only through its own single store | `client_view`, `client_view::notifications`, `action_menu`, `machine_agents::attention`, `machine_agents::view`, `machine_picker`, `machines`, `native_settings`, `startup_directory`, `startup_preference`, `terminal_copy_mode`, `terminal_find`, `terminal_viewport`, `transient_notice`, `workbench_client::command_notice`, `workbench_client::completion_reminders`, `workbench_client::copy_episode`, `workbench_client::normal_exit_hold`, `workbench_client::settings_notice`, `workbench_client::terminal_selection`, `workbench_client::ui::action_menus::episode`, `workbench_client::ui::history_positions`, `workbench_client::ui::retained_frames` |
| Connection lifecycle | Owns the queues, correlation, generations, retained evidence, and wakeups of one Client Connection; the `workflow` connection task drives it                            | `workbench_client::mailbox`, `ui_wakeup`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                          |

`ClientView` and `WorkbenchClientUi` share five values. Each has one writer, and the other holder
receives the owner's value through composition:

- Host capabilities are a compile-time admission. Each holder receives them once at construction,
  and nothing writes them afterwards.
- `ClientView::apply_user_preferences` writes the confirmed common setting values. `WorkbenchClientUi`
  receives the owner's values in the same settlement.
- The configuration watcher writes the notification policy into the `ClientView` notifications.
  `WorkbenchClientUi` receives that policy with the revision that admitted it.
- `workbench_client::copy_episode` owns copy-mode activity. `ClientView` keeps a mirror that only
  `settle_copy_mode` writes from the owner's current answer.
- `client_view::notifications` owns the current Agent notification card and its target.
  `WorkbenchClientUi` keeps a mirror of whether one is current, which only the automatic
  notification tick writes, so the action menu offers Go to the Agent that notified only then.

Client-local notices in `ClientView` and Workbench command notices in
`workbench_client::command_notice` are separate facts with separate writers.

## Components

The levels from [TUI components](../../../../../docs/tui/components.md#ownership-and-admission)
map to layers as follows.

| Level                 | Placement                                                                                                                                                                                                                                                                                                                                                                                            |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Primitive             | Shared component modules in `presentation` under `tui_render::controls`, placed by the [TUI components reference](tui-components.md); their control-state types belong in `values`, and the regions they paint are recorded into the frame hit map. Features never fork them.                                                                                                                        |
| Recurring composition | Shared `presentation` modules that arrange primitives, such as lists, pickers, modal windows, and shortcut bars; the overlay frame and modal compositions live under `tui_render::modal`. They own no feature effects.                                                                                                                                                                               |
| Business surface      | The owning feature module. Each child keeps one layer: state in `state`, view model and painter in `presentation`, gesture-to-intent mapping in `input`, and effects in `workflow`. A painter is either a feature child assigned `presentation` or a `tui_render` module that reads the feature only through its view model or accessors. A child whose layer differs from its parent names its own. |

## Input port

`TerminalInputPort` in `workbench_client::input` is the only way `input` modules reach
composition. `WorkbenchClient` implements it, and `workbench_client::ui` routes each entry to the
feature composition that owns its target. The port contract defines the input and action
vocabulary, grouped by the feature that names each action, because the witnesses it carries come
from `state`, `presentation`, and `input` modules. The port has one entry per responsibility, never
one per gesture:

| Group             | Entries                                                                                                                                                   | Carries                                                                                                                                                     |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Facts             | `native_capture_claims`, `native_capture_context`, `copy_mode_active`, `pointer_probe`, drag sources, divider and resize projections, `navigator_catalog` | What dispatch and gesture resolution read                                                                                                                   |
| Terminal          | `forward`, `observe_terminal`                                                                                                                             | Terminal forwarding and host or input-stream observations                                                                                                   |
| Local input       | `pane_input`, `surface_input`, `scroll_list`                                                                                                              | Pane steps, input for the surface that holds input, and painted-list scrolling; the result stays with the owner                                             |
| Committed results | `surface_action`, `apply_client_action`, `tab_create_action`, action-menu entries, `activate_navigator`, `open_notification_target`                       | A feature-defined `SurfaceAction`, or a Client action resolved from bindings, menus, the navigator, or a picker, or the Agent target of a notification card |

Every implementation keeps these invariants:

- Local input never commits a product result by itself; a captured key may still lead its owner
  to a transition that the owner's keymap defines.
- A value aimed at a painted or captured incarnation carries that incarnation's witness. The owner
  revalidates it and refuses a stale witness without changing state.
- `WorkbenchClient` settles native retirement after exactly these entries: geometry observations;
  history-key, history-wheel, and scrollbar pane steps; close-confirmation keys; close decisions;
  rename and setup submissions; setup choices; and presentation activation.
- Resize-control and divider actions run through the `workbench_client::client_control` lane.
  Every other action runs through the feature composition that owns its target.
- An action whose owner reports only acceptance answers `KeepNavigator` when accepted and nothing
  when refused.
- Help, the navigator, and the machine picker are view-owned lists; `input` scrolls them without
  the port.
- `composition` builds the witnesses that the Facts entries carry, such as the navigator catalog and
  the action-menu and divider sources, from read accessors of the state view; `state` never names a
  `presentation` or `input` witness type. Drag-source witnesses are port vocabulary in
  `workbench_client::input::drag_sources`.

## Pane arrangement port

`tui_layout` projects pane geometry from the same shell layout the renderer paints. A shell without
a Workbench Client, including startup, uses the shell projection `terminal_geometry` and
`active_pane_area`. A Workbench Client's panes reach layout only through `PaneArrangement` and
`TabPanes`, which `workbench_client::ui` implements for the Client view, so layout never
references the UI composition. `WorkbenchClientView::pane_geometries` is the only production
entry into the arranged projection; the arranged active-pane area and geometry exist only for
tests.

## Maintenance

A new top-level module names its layer in the change that adds it. The check reports top-level
modules without a layer and fails on an unknown layer or a misplaced layer line. Run
`pnpm lint:rust-modules -- --details --member apps/orchbee/crates/workbench-client` for every cycle
and every upward reference with sample source locations; [Linting](../../../../../docs/LINTING.md)
owns the command.
