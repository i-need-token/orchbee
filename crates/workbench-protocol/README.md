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
