# Windows reusable layouts

The native Windows CLI owns portable layout export and the composition of canonical Workbench
membership, Client view state and read-only terminal observations. The pinned Herdr reference is
`e0507237ad8d41f60aece10d00ef29ce2339ca34`, `src/app/api/layouts.rs` (R053/R054).

## Export

`layout export [--tab <target> | --panel <target> | --current] [--client <id>]
[--timeout <milliseconds>] [--format text|json]` emits `orchbee.layout.export` v1 JSON on success
in either format. Failure follows the requested text/JSON format. The default total timeout is
30,000 ms. Ctrl+C exits 130 and releases temporary viewers and the admitted session.

Resolve an explicit Client, otherwise the validated caller's unique current Client, otherwise the
sole current Client. A Ready Client is required even for an explicit Tab: ratios, zoom and focus
belong to that Client. Missing or ambiguous observations are failures, never guessed defaults.
An omitted target uses the validated caller Panel or selected Client Panel. Explicit targets ignore
caller target metadata. Tab selection accepts an exact ID, suffix selector or unique label; Panel
selection accepts an exact ID or suffix selector. Exporting a Panel exports its containing Tab.
The operation never activates a Tab, focuses a Panel, resizes a Host or acquires terminal control.

The result contains `layout:{workspaceId,terminalContextId,tabId,zoomed,focusedPanelId,root}`;
`workspaceId` is null for standalone Tabs. Each split has `type:split`, `direction:right|down`,
`ratio`, `first`, and `second`. Each leaf has `type:panel`, descriptive `panelId`, optional manual
`label`, observed `cwd`, optional `command` argv including the executable, and empty `env`.
Absence of cwd means the process sample provided no directory. Default-shell launches omit command.
Explicit commands come from the immutable admitted launch selection, including deleted profiles;
they are never reconstructed from process command-line text. No environment values are exported.

At most 24 leaves and depth 16 (root depth zero) are admitted; output is bounded to 1 MiB. All split
identities, ordered slots and leaf bindings must match the captured canonical tree. Child
Presentations cannot be silently discarded. Untracked launch facts, stale topology, unavailable
observations and unsupported structures fail without emitting a partial success document.
The source records Site/revision and the Client/view revision used to fence the read request;
that view revision is the preflight observation, not a later publication revision. Terminal cwd samples are
sequential observations, not one atomic process snapshot. A final canonical revision check rejects
interleaved membership changes. Exported IDs convey no authority to reuse processes during apply.

## Ownership and evidence

The deterministic projector owns topology bounds, exact split matching and portable leaf fields.
The Windows adapter owns admission, Client selection, temporary viewer cleanup, deadlines,
cancellation and final revision checks. The Server remains the sole membership/launch-fact owner;
the Client remains the view owner. No durable format or new dependency is introduced.

Deterministic evidence covers orientation/order, non-default ratios, labels, literal argv,
empty environments, missing/extra/duplicate facts, stale split slots and structural bounds. Windows
acceptance covers real cwd and frozen programs, selected/inactive and zoomed Tabs, multiple Clients,
unchanged focus/process identity, cancellation and schema validation. These checks do not establish
layout apply, crash restoration, plugin persistence or generic event delivery.
