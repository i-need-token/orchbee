# Windows custom commands

## Configuration and catalog model

The shared Windows settings owner admits up to 64 `[[keys.command]]` entries. Each has a nonblank NUL-free `command` up to 4096 UTF-8 bytes, `type` (`shell`, `pane`, `popup` or `plugin_action`, default `shell`), optional `key` (one chord or up to 16 chords) and optional `description` up to 512 bytes. Popup-only `width` and `height` accept positive cell counts up to 4096 or percentages 1%-100%. Missing keys produce an unbound catalog entry. If any configured command has an unknown field, duplicate/unsafe/reserved chord, invalid command, or action-incompatible size, the entire configured-command set is disabled and a bounded warning is emitted. No partial or unvalidated command can run; other valid settings and built-in actions remain admitted. The whole settings file/profile remains bounded at 64 KiB.

Custom and built-in keys share normalization, the effective prefix and conflict policy. Runtime publication uses the existing canonical keys profile; configuration reload installs the complete admitted command/binding set with its diagnostics, and a later corrected publication restores configured commands. Fatal file/TOML failure retains the preceding publication. Catalog IDs bind the complete normalized profile and ordered entry index. They are opaque, change when any command/binding definition changes, and never accept executable text or grant process authority. A current command ID resolves one declared action; stale or absent IDs fail before its effect.

## Invocation ownership model

Native command discovery, CLI invocation and TUI bindings consume the same installed configuration. Invocation validates requested Workspace/Tab/Panel ancestry and the current source member, then uses the existing observed focus path when a target is specified. Plugin selections are read against their exact terminal content revision; unrelated or stale selection is refused. CLI selection coordinates and the Server source observer use one pure text-row extractor in the terminal protocol binding, including wide glyphs, wrapped rows and the plugin context byte limit. The temporary observer owns detach settlement before invocation can proceed. Shell and plugin actions may have no Pane context; Pane/popup actions require an admitted current source.

Shell executes the configured Windows command shell through a Worker-owned process, without terminal input. Pane execution uses ordinary canonical terminal creation and focus; popup uses the independent typed terminal lane and its input/resize/close owner. Plugin action invocation uses the existing action resolver and logs. None impersonates an installed plugin. Source context is display metadata, not launch authority; managed Session routing and actual Worker placement remain authoritative. A submitted effect is not replayed after timeout, cancellation, lost or malformed answers. Waiting cancellation never undoes an accepted effect.

Worker waits up to five seconds for physical shell-start confirmation, within the carrier's twenty-second response budget. Slow process creation inside that window can still produce a confirmed receipt. A missing or uncertain confirmation retains the process owner and reports physical uncertainty; a late result never authorizes replay or silently clears that uncertainty. Focused tests cover delayed confirmation, duplicate admission, timeout, late completion and joined shutdown.

The TUI command exchange has a separate cancellable connection and joined thread, so waiting for a command cannot pause Client heartbeats or popup delivery. Only one command may be queued or awaiting its receipt. Empty Workspaces may invoke global shell/plugin actions; Pane/popup actions still require the exact bound source. Shortcut discovery sanitizes display text while preserving the literal command in configuration. Nested receipts must describe the declared action and match its request, Session and popup selector; an unrelated valid protocol result is refused.

A shortcut queues its opaque installed command ID and exact Ready source observation, then publishes
that observation through the normal Client heartbeat lane before binding the invocation to the
acknowledged Client/view revision. An older in-flight Busy observation from the armed prefix cannot
reject or authorize that invocation. A newer or different source refuses without retargeting. This
same path applies to initial and reloaded bindings and needs no prior CLI catalog or invocation.
Server-side command, source and permission checks remain unchanged. Refusal feedback includes the
command ID and distinguishes busy, stale source, changed command/configuration, permission,
unavailability and unknown effects, with the corresponding wait, return-live, reload, reconnect or
inspect-before-retry guidance. Controlled lane witnesses live in
`src/workbench_client/client_control/lane/tests/configured_invocation.rs`; popup input, close and
return behavior remains with the existing independent-popup owner.

## Reference and evidence

The reference is Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, `app/custom_commands.rs` and `api/schema/commands.rs`: four configured actions, endpoint-issued IDs, optional target focus and revision-bound plugin selection. Native configuration profiles supply the command identity boundary; existing runtime owners supply process and physical terminal authority. Parser/profile checks establish the shared configuration foundation only. Completion additionally requires real catalog/invoke/keybinding consumers for every action, correlated refusal/unknown and lifecycle evidence, and final Windows workflow acceptance. No Desktop or other-platform command implementation is added by this model.

Focused evidence includes real Windows shell environment/exit handling, Worker background ownership and duplicate/conflict/shutdown checks, executable CLI catalog/invoke/refusal/unknown cases, installed Server catalog/source/permission checks, TUI binding/discovery/source invalidation and actor cancellation, and the shared Rust/TypeScript/JSON [conformance corpus](../../../schemas/workbench/v1/workbench-configured-command-conformance-v1.json). Native command output is `orchbee.command` v1; stale IDs use the closed `orchbee.cli.failure.command` v1 document. Configured popup controls use v1 and remain invalid under v1. These focused checks do not claim final full-cohort or interactive Windows acceptance.

The configured-shell witness waits for the complete command-ID receipt within its original three-second budget; file creation alone does not establish a completed write. Its fixture explicitly checks cleanup on success. During panic unwinding, cleanup failure reports the exact retained directory without replacing the original panic. Counterexamples cover an initially empty receipt and a directory consumer that denies deletion until the original panic is observed and the consumer releases its handle.
