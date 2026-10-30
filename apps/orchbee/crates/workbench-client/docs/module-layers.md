# Workbench Client Module Layers

Status: accepted layer and lateral-import contract for this crate. Current code does not conform
yet; `pnpm lint:rust-modules` reports the remaining cycles and upward references without failing.

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
- A descendant inherits its parent's layer unless [Module assignment](#module-assignment) names it.
  A parent never re-exports or uses a descendant assigned to a higher layer.
- References within one layer stay acyclic. When two modules need each other, move the shared part
  into the lower of the two or into a lower layer.
- External protocol, runtime, and rendering crates sit below `values`. The sibling Client Connection
  transport's handles are reachable only through `local_client`, a `workflow` module; state owners
  settle its exchange outcome values without holding a handle.
- Operating-system `cfg` selection belongs in `workflow` adapters whose path names the platform or
  capability, and in `composition`; the `host_capabilities` admission table is the one `values`
  exception. Other modules stay platform-neutral.
- The application consumes this crate only through `application`; nothing inside the crate
  references it. Code under `cfg(test)` or the `test-support` feature stays outside the layer graph
  and never widens production visibility.

## State owners

Every piece of Client state has exactly one writing module. Other modules read it through shared
borrows or receive derived values; render reads only the derived view model.

| Source               | Writer contract                                                                                                                                                         | Owners                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Server projection    | Applies only validated Server or Worker results and events; never mints canonical membership or infers physical cleanup from a missing projection                       | `workbench_client::workbench_projection`, `workbench_client::state`, `workbench_client::agent_board`, `terminal_projection`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| Client-local view    | Owns chrome, selection, focus, drafts, menus, notices, copy mode, preferences, and positions that exist only in this Client; persists only through its own single store | `client_view`, `action_menu`, `machine_picker`, `machines`, `native_settings`, `notifications`, `onboarding`, `terminal_copy_mode`, `transient_notice`, `workbench_client::command_notice`, `workbench_client::completion_reminders`, `workbench_client::copy_episode`, `workbench_client::normal_exit_hold`, `workbench_client::settings_notice`, `workbench_client::startup_directory`, `workbench_client::startup_preference`, `workbench_client::terminal_selection`, `workbench_client::terminal_viewport`, `workbench_client::ui::action_menus::episode`, `workbench_client::ui::history_positions`, `workbench_client::ui::retained_frames` |
| Connection lifecycle | Owns the queues, correlation, generations, retained evidence, and wakeups of one Client Connection; the `workflow` connection task drives it                            | `workbench_client::mailbox`, `workbench_client::wakeup`                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

`ClientView` and `WorkbenchClientUi` share four values. Each has one writer, and the other holder
receives the owner's value through composition:

- Host capabilities are a compile-time admission. Each holder receives them once at construction,
  and nothing writes them afterwards.
- `ClientView::apply_user_preferences` writes the confirmed user preferences. `WorkbenchClientUi`
  receives the owner's values in the same settlement.
- The configuration watcher writes the notification policy into the `ClientView` notifications.
  `WorkbenchClientUi` receives that policy with the revision that admitted it.
- `workbench_client::copy_episode` owns copy-mode activity. `ClientView` keeps a mirror that only
  `settle_copy_mode` writes from the owner's current answer.

Client-local notices in `ClientView` and Workbench command notices in
`workbench_client::command_notice` are separate facts with separate writers.

## Components

The levels from [TUI components](../../../../../docs/tui/components.md#ownership-and-admission)
map to layers as follows.

| Level                 | Placement                                                                                                                                                                                                                                                                          |
| --------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Primitive             | Shared component modules in `presentation` under `tui_render::controls`, inventoried in the [TUI components reference](tui-components.md); their control-state types belong in `values`, and the regions they paint are recorded into the frame hit map. Features never fork them. |
| Recurring composition | Shared `presentation` modules that arrange primitives, such as lists, pickers, modal windows, and shortcut bars; the overlay frame and modal compositions live under `tui_render::modal`. They own no feature effects.                                                             |
| Business surface      | The owning feature module. Each child keeps one layer: state in `state`, view model and painter in `presentation`, gesture-to-intent mapping in `input`, and effects in `workflow`. The assignment table names every child whose layer differs from its parent.                    |

## Input port

`TerminalInputPort` in `workbench_client::input` is the only way `input` modules reach
composition. `WorkbenchClient` implements it, and `workbench_client::ui` routes each entry to the
feature composition that owns its target. The port contract defines the input and action
vocabulary, grouped by the feature that names each action, because the witnesses it carries come
from `state`, `presentation`, and `input` modules. The port has one entry per responsibility, never
one per gesture:

| Group             | Entries                                                                                                                                                   | Carries                                                                                                         |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Facts             | `native_capture_claims`, `native_capture_context`, `copy_mode_active`, `pointer_probe`, drag sources, divider and resize projections, `navigator_catalog` | What dispatch and gesture resolution read                                                                       |
| Terminal          | `forward`, `observe_terminal`                                                                                                                             | Terminal forwarding and host or input-stream observations                                                       |
| Local input       | `pane_input`, `surface_input`, `scroll_list`                                                                                                              | Pane steps, input for the surface that holds input, and painted-list scrolling; the result stays with the owner |
| Committed results | `surface_action`, `apply_client_action`, `tab_create_action`, action-menu entries, `activate_navigator`, `copy_picker_identifier`                         | A feature-defined `SurfaceAction`, or a Client action resolved from bindings, menus, the navigator, or a picker |

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

## Module assignment

Each row is the target layer. A note names the part that must move when a module mixes layers
today.

| Module                                        | Layer          | Role and reason                                                                                                                                     |
| --------------------------------------------- | -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `action_menu`                                 | `state`        | Client-local menu capture and selection, and which prefix binding a menu target advertises.                                                         |
| `action_projection`                           | `values`       | Action identity, category, tone, and discovery metadata projected from `client_actions`.                                                            |
| `agent_provider`                              | `values`       | Recognized Agent identities, canonical command names, and the managed integration identity re-exported from `orchbee-agent-integration`.            |
| `agent_runtime`                               | `workflow`     | Agent execution ports and the observation facts that Agent start consumes.                                                                          |
| `agent_start`                                 | `workflow`     | Typed Agent startup shared by the CLI and Client surfaces.                                                                                          |
| `application`                                 | `composition`  | Export facade for the application binary.                                                                                                           |
| `background_thread`                           | `workflow`     | Client background-thread ownership and the process-exit budget under the shared exit contract.                                                      |
| `client_actions`                              | `values`       | Client, menu, and prefix action definitions shared by bindings, menus, capability policy, and discovery.                                            |
| `client_context`                              | `values`       | Identity-only Client navigation locations.                                                                                                          |
| `client_view`                                 | `state`        | Client-local chrome, prefix mode, overlays, and presentation preferences. Binding resolution moves to `input`.                                      |
| `exchange_values`                             | `values`       | Request and outcome values of Client Connection exchanges that state owners build and settle; no transport handle or clock.                         |
| `host_appearance`                             | `values`       | Outer-terminal appearance facts observed by the default-color probe.                                                                                |
| `host_capabilities`                           | `values`       | Names of optional host effects and their compile-time platform admission, including login-shell mode and the local-startup recovery checkpoint.     |
| `host_graphics`                               | `values`       | Host window geometry and the graphics facts measured against it.                                                                                    |
| `local_activation`                            | `workflow`     | Local Session activation with its locks, markers, spawn, and deadline.                                                                              |
| `local_client`                                | `workflow`     | The only doorway to the sibling transport's handles. State owners read exchange values through `exchange_values`.                                   |
| `localization`                                | `values`       | Typed message identities and the `en-US` and `zh-CN` catalogs. Locale detection from the environment moves to `composition`.                        |
| `machine_agents`                              | `workflow`     | Cross-machine Agent observation jobs and their bounded checkpoints.                                                                                 |
| `machine_discovery`                           | `workflow`     | Read-only observers of enabled saved machines.                                                                                                      |
| `machine_observation`                         | `workflow`     | Shared bounded, cancellable observation lifecycle for machine jobs.                                                                                 |
| `machine_picker`                              | `state`        | Client-local machine selection.                                                                                                                     |
| `machine_picker::paint`                       | `presentation` | Paint of the capturing machine picker, its Agent controls, and its query rows.                                                                      |
| `machine_picker::pointer`                     | `input`        | Releases and wheel movement applied to painted machine picker targets.                                                                              |
| `machine_spaces`                              | `workflow`     | Workspace metadata and local Git checkpoints read through machine observation.                                                                      |
| `machine_ssh`                                 | `workflow`     | Adapter that builds and runs SSH commands for saved machines.                                                                                       |
| `machine_tunnel`                              | `workflow`     | Saved-machine carrier adapter.                                                                                                                      |
| `machines`                                    | `state`        | Saved-machine catalog values and the catalog's single store writer.                                                                                 |
| `native_settings`                             | `state`        | Client-local Settings draft and its value rows.                                                                                                     |
| `notifications`                               | `state`        | Main-TUI notification state; its painter, transport, and host effects are the children below.                                                       |
| `notifications::paint`                        | `presentation` | Noncapturing paint of the visible notification card.                                                                                                |
| `notifications::subscription`                 | `workflow`     | Notification subscription on the Client Connection.                                                                                                 |
| `notifications::system`                       | `workflow`     | Platform system-notification and alert-tone adapter.                                                                                                |
| `onboarding`                                  | `state`        | Client-local education state; only an explicit dismissal writes its store.                                                                          |
| `startup_status`                              | `values`       | Local-startup verdicts.                                                                                                                             |
| `terminal_buffer_text`                        | `values`       | Glyph traversal over validated retained rows.                                                                                                       |
| `terminal_content`                            | `workflow`     | Bounded content inspection on an independent viewer connection.                                                                                     |
| `terminal_content::source`                    | `state`        | Content-source identity and screen witnesses that copy mode and reading positions fence against.                                                    |
| `terminal_copy`                               | `values`       | Buffer-coordinate copy policies.                                                                                                                    |
| `terminal_copy_mode`                          | `state`        | Client-local keyboard-copy state over one admitted content surface.                                                                                 |
| `terminal_copy_query`                         | `state`        | Copy query identities, requests, and outcomes that the copy-mode episode matches.                                                                   |
| `terminal_copy_query::lane`                   | `workflow`     | Correlated single-flight copy motion and search reads.                                                                                              |
| `terminal_device`                             | `workflow`     | Inherited-terminal device adapter. Admission failures move to `values`; window geometry lives in `host_graphics`.                                   |
| `terminal_events`                             | `values`       | Host event types and the deterministic byte decoder.                                                                                                |
| `terminal_graphics`                           | `workflow`     | Host graphics probes. The measured facts live in `host_graphics`.                                                                                   |
| `terminal_host_appearance`                    | `workflow`     | Outer-terminal default-color probe. The observed facts live in `host_appearance`.                                                                   |
| `terminal_input`                              | `input`        | Delivers decoded host events to the current capture owner or terminal forwarding.                                                                   |
| `terminal_lifecycle`                          | `workflow`     | Inherited-terminal mode leases, rollback, and restoration.                                                                                          |
| `terminal_link_activation`                    | `workflow`     | One-effect link activation shared by the TUI and CLI.                                                                                               |
| `terminal_links`                              | `values`       | Plain-link policy over validated glyphs.                                                                                                            |
| `terminal_live_view`                          | `presentation` | Client-local cropping of a live grid for display.                                                                                                   |
| `terminal_pointer_admission`                  | `workflow`     | One-shot admission probe for any-motion pointer reporting.                                                                                          |
| `terminal_projection`                         | `state`        | One attachment's validated text projection derived from the Server and Worker stream.                                                               |
| `terminal_scrollbar`                          | `values`       | Validated scrollbar observations and integer cell geometry.                                                                                         |
| `terminal_text`                               | `values`       | Terminal-safe projection of untrusted text and the shared prose line-breaking rule.                                                                 |
| `terminal_title`                              | `values`       | Title activity-prefix handling and the program names chrome shows for console default titles and launches.                                          |
| `termination_signal`                          | `workflow`     | Host termination observation for the exit lifecycle.                                                                                                |
| `transient_notice`                            | `state`        | One bounded, coalesced notice lifetime.                                                                                                             |
| `tui_capabilities`                            | `values`       | Policy that maps host capabilities to available actions.                                                                                            |
| `tui_composition`                             | `values`       | Stateless selection of the top capturing surface, shared by paint settlement and input dispatch.                                                    |
| `tui_graphics`                                | `workflow`     | Windows graphics surface that shares the backend frame and restoration lease.                                                                       |
| `tui_layout`                                  | `presentation` | Shell and pane layout geometry.                                                                                                                     |
| `tui_pointer`                                 | `input`        | Pointer ownership, gesture lifecycle, and wheel normalization against the published hit map.                                                        |
| `tui_pointer::hit_map`                        | `presentation` | Frame hit map and interaction projection. Regions carry owners' painted witnesses, so the map sits above `state`.                                   |
| `tui_presentation`                            | `values`       | Presentation-lifetime classification of surface incarnations.                                                                                       |
| `tui_render`                                  | `presentation` | Frame composition and painting. Primitives and recurring compositions stay as the shared component layer; business surfaces move to their features. |
| `tui_render::controls::control_state`         | `values`       | Control-state vocabulary of the shared components.                                                                                                  |
| `tui_shortcut_help`                           | `presentation` | Effective shortcut help derived from bindings and active modes.                                                                                     |
| `tui_theme`                                   | `values`       | Semantic theme roles and shipped themes.                                                                                                            |
| `windows_console_input`                       | `workflow`     | Windows console-record input adapter.                                                                                                               |
| `windows_signal`                              | `workflow`     | Windows console interrupt dispatcher.                                                                                                               |
| `workbench_client`                            | `composition`  | Connection-task lifetime and UI composition. Its children carry their own layers below.                                                             |
| `workbench_client::agent_attention`           | `workflow`     | Agent attention observation lane and paint acknowledgements.                                                                                        |
| `workbench_client::agent_board`               | `state`        | Agent board rows from the last completed read, with the board's selection and queued intents.                                                       |
| `workbench_client::agent_board::start::jobs`  | `workflow`     | Client-PATH catalog and Agent startup jobs with their platform PATH adapter; they settle outcomes into the Board's start state.                     |
| `workbench_client::agent_notifications`       | `values`       | Logical-time notification policy.                                                                                                                   |
| `workbench_client::agent_read_lane`           | `workflow`     | Bounded Agent and Panel read jobs on the shared carrier.                                                                                            |
| `workbench_client::client_control`            | `workflow`     | Remote navigation lane whose steps the UI owner advances.                                                                                           |
| `workbench_client::command_notice`            | `state`        | The bounded Workbench command notice and the context that names its subject.                                                                        |
| `workbench_client::completion_reminders`      | `state`        | Reminder state that consumes each command fact once.                                                                                                |
| `workbench_client::copy_episode`              | `state`        | The copy-mode episode: its pane, content fence, and pending query. The UI composition holds the query driver.                                       |
| `workbench_client::configuration`             | `workflow`     | Configuration publication watcher.                                                                                                                  |
| `workbench_client::connection`                | `workflow`     | The Client connection task.                                                                                                                         |
| `workbench_client::independent_popup`         | `workflow`     | Independent projection and control lane.                                                                                                            |
| `workbench_client::input`                     | `input`        | The [input port](#input-port) contract and its input and action vocabulary.                                                                         |
| `workbench_client::mailbox`                   | `state`        | Connection-lifecycle queues, correlation, and atomic admission with Client state.                                                                   |
| `workbench_client::normal_exit_hold`          | `state`        | Closing presentation of a pane whose Shell exited with code 0, held until the pane leaves membership or its bound passes.                           |
| `workbench_client::pane_graphics`             | `workflow`     | Pane canvas read owner.                                                                                                                             |
| `workbench_client::panel_reports`             | `workflow`     | Selected-Presentation observation lane.                                                                                                             |
| `workbench_client::plugin_overlays`           | `workflow`     | Plugin overlay read and close job.                                                                                                                  |
| `workbench_client::primary_directory`         | `values`       | Admitted primary root-process directory sample.                                                                                                     |
| `workbench_client::settings_lane`             | `workflow`     | Settings snapshot observation and write submission.                                                                                                 |
| `workbench_client::settings_notice`           | `state`        | The Settings outcome notice and its expiry.                                                                                                         |
| `workbench_client::startup_directory`         | `state`        | Empty-Session startup intent and retained invocation evidence.                                                                                      |
| `workbench_client::startup_preference`        | `state`        | Client-local last displayed position.                                                                                                               |
| `workbench_client::startup_preference::store` | `workflow`     | Platform position-cache storage identity, load, and the single background writer.                                                                   |
| `workbench_client::state`                     | `state`        | Attachment, membership, selection, and pending operations against validated canonical projections.                                                  |
| `workbench_client::status_recovery`           | `workflow`     | Retained-close observation task after the connection task exits.                                                                                    |
| `workbench_client::terminal_clipboard`        | `workflow`     | Clipboard effect adapter.                                                                                                                           |
| `workbench_client::terminal_effects`          | `workflow`     | Bounded external-effect queue.                                                                                                                      |
| `workbench_client::terminal_selection`        | `state`        | Attachment-local selection commands and admitted text.                                                                                              |
| `workbench_client::terminal_tail`             | `presentation` | Live-tail display decision over retained rows.                                                                                                      |
| `workbench_client::terminal_viewport`         | `state`        | Per-pane retained-history viewport state.                                                                                                           |
| `workbench_client::ui`                        | `composition`  | `WorkbenchClientUi`: feature composition, settlement, lane lifetimes, and the input port.                                                           |
| `workbench_client::ui::action_menus::episode` | `state`        | The action-menu episode: intent and stage, submenu parents, geometry, Tab-close resume handoff, and sidebar worktree discovery.                     |
| `workbench_client::ui::history_positions`     | `state`        | Content-validated reading choices kept across viewer and context replacement.                                                                       |
| `workbench_client::ui::retained_frames`       | `state`        | Bounded last-drawn frames of selected panels, kept across context switches.                                                                         |
| `workbench_client::wakeup`                    | `state`        | Connection-lifecycle wakeup coalescing.                                                                                                             |
| `workbench_client::window_title`              | `workflow`     | Window-title subscription consumer.                                                                                                                 |
| `workbench_client::workbench_projection`      | `state`        | Projection of validated canonical snapshots into Client contexts, Tabs, and Panels.                                                                 |
| `workbench_client::worktree_lane`             | `workflow`     | Bounded Worktree command lane.                                                                                                                      |
| `worktree_destination`                        | `values`       | Configured checkout destination policy.                                                                                                             |

## Maintenance

Add, remove, rename, or move a module together with its row in the same change. The check reports
unassigned top-level modules and rows that name a missing module. Run
`pnpm lint:rust-modules -- --details --member apps/orchbee/crates/workbench-client` for every cycle
and every upward reference with sample source locations; [Linting](../../../../../docs/LINTING.md)
owns the command.
