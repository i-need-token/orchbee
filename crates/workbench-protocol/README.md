# Workbench protocol

This crate owns the typed requests, responses, validation, and framed transport shared by native Workbench clients and the Server. It defines contracts without opening sockets or executing operations; runtime owners interpret admitted requests.

## Native plugin runtime contract

Closed plugin invocation/log/get/cancel requests and replies use matched-cohort OBWP kinds 45/46, capped at 32 KiB and 8 MiB respectively. Context carries display data only; admitted Server resolves executable intent and Worker owns processes. Log cursors pin Worker identity/generation and descending sequence; uncertain mutation delivery is explicit. Rust, TypeScript and JSON share the plugin conformance corpus. [Native plugin behavior](../../apps/orchbee/docs/windows-plugins.md) owns runtime lifecycle and remaining integration.

Plugin Pane launch options may carry `pluginPane: { pluginId, entrypointId }` on `tab.create` and `panel.split`. Both identities are bounded ASCII catalog names (120 bytes; entrypoint excludes dots). The selector is exclusive with executable/profile and rejected on Workspace or child-terminal creation. Empty and null optional selectors normalize away; legacy request serialization remains unchanged. Server resolution and protected execution environment belong to terminal admission, not this display-free selector.

Plugin PaneGet is read-only and returns PaneFound with a nonzero canonical revision, closed selector, exact context/Tab/Panel/Presentation and binding. Response correlation requires the requested Panel; the result is not an action Job receipt or caller display context.

## Workspace Git status reads

`WorkspaceGitRequest`/`WorkspaceGitResult` add the closed `workbench.workspace-git`
and `workbench.workspace-git.result` directions, OBWP kinds 49/50, with independent
8 KiB pre-allocation limits. Required nullable branch/divergence fields distinguish
absence from zero; branch text is control-free UTF-8 at most 1024 bytes, counts are
uint32, and detached branches cannot carry counts. Exact request/Site/Workspace
correlation is mandatory. The [schema and shared corpus](../../schemas/workbench/README.md#workspace-git-status-reads)
cover both Rust and TypeScript. This read grants no mutation or Desktop command.
Configuration publication additionally carries separate required 32 KiB
`spacesSidebarProfile`; its matched-cohort frame budget is 256 KiB.

## Plugin overlay observation and Client return identity

Read-only `PaneObserve` carries exact `PluginPaneInfo`; `PaneObserved` echoes it with a nonzero canonical revision and a boolean settlement fact. Correlation includes selector, context/Tab/Panel/Presentation and binding. It grants no lifecycle mutation. Private Client `Overlay` control carries the created bound member, exact nullable return member and previous zoom. Validation rejects self-return identities; Applied requires the requested focused member, zoom and multiple leaves. Required nullability and semantic failures share the Rust/TypeScript corpus. [Windows plugins](../../apps/orchbee/docs/windows-plugins.md#overlay-lifecycle-model) owns runtime records, observation cleanup and canonical Close. These extensions require the matched native cohort and add no Desktop command.

Pane launch options additionally carry optional bounded `pluginContext` only with `pluginPane` on Tab/split creation. The shared command corpus covers null normalization, unknown fields, UTF-8/total context bounds and unsupported creation kinds. Context is display data: the Server validates source membership, supplies canonical labels and fixes invocation source/correlation before freezing protected plugin environment. It never selects executable, launch directory or Worker authority. Omission preserves legacy admission. Rust redacts the context from launch Debug; TypeScript shares its context schema without a launch/plugin import cycle and preserves the existing plugin export. Runtime semantics and acceptance scope belong to [Pane launch source context](../../apps/orchbee/docs/windows-plugins.md#pane-launch-source-context).

## Independent plugin popup contract

Private plugin PopupOpen/PopupRead/PopupClose use the existing matched-cohort plugin transport and explicit correlated PopupOpened/PopupState/PopupClosing/PopupClosed/PopupUnknown results. PopupOpen carries a closed catalog selector, exact context/Tab/Panel and full Client Presentation selection, bounded display context, required nullable cwd/dimensions, and the shared protected-name launch-environment admission. Bound attachment generation/incarnation and nonzero unbound selection generation remain part of the source identity. These requests do not create canonical Pane membership. Client PopupOpen/PopupClose require explicit matching completion; generic Applied cannot complete them. A hadPopup=false close may preserve an unrelated Busy dialog. Rust, TypeScript and JSON share valid/invalid conformance vectors. [Independent popup behavior](../../apps/orchbee/docs/windows-plugins.md#independent-terminal-popup) owns physical lifecycle, UI input and acceptance. Peers deploy together; no Desktop operation or new Host capability is granted.
