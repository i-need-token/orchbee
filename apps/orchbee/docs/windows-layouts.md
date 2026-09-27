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

At most 24 leaves and 16 levels (root level one) are admitted; output is bounded to 1 MiB. All split
identities, ordered slots and leaf bindings must match the captured canonical tree. Child
Presentations cannot be silently discarded. Untracked launch facts, stale topology, unavailable
observations and unsupported structures fail without emitting a partial success document.
The source records Site/revision and the Client/view revision used to fence the read request;
that view revision is the preflight observation, not a later publication revision. Terminal cwd samples are
sequential observations, not one atomic process snapshot. A final canonical revision check rejects
interleaved membership changes. Exported IDs convey no authority to reuse processes during apply.

## Apply

`layout apply (--file <path> | --stdin) [--tab <target> --confirm | --workspace <target> |
--standalone] [--label <label>] [--client <id>] [--focus | --no-focus]
[--timeout <milliseconds>] [--format text|json]` creates a new Tab or replaces the selected Tab.
An omitted destination uses the selected Client context. Replacement requires explicit confirmation;
exported identifiers never select a destination or reuse a terminal. A Ready Client is required for
ratios and final focus. Default focus is false; replacing that Client's active Tab selects the new Tab.
An explicit label overrides the replacement's old custom label; automatic numbering remains automatic.

The file or inherited noninteractive stdin is fully acquired and validated before runtime effects.
The limit is 1 MiB, with optional UTF-8 BOM, at most 24 leaves and 16 levels including the root.
Accept a native export envelope, a layout description containing `root`, or a bare portable node.
Herdr `type:pane` and descriptive `pane_id` are accepted aliases for native `type:panel`/`panelId`.
Description identifiers and exported focus/zoom are descriptive; the template creates fresh, unzoomed
terminals. Reject duplicate/unknown operative fields, malformed nested values, invalid directories,
empty commands, invalid environment overlays and bounds violations across the entire tree. Split
ratios must be finite and are clamped to 0.1..0.9. Each leaf can supply a manual label, cwd, literal
command argv and environment overlay under the existing launch contracts. Program names are resolved
before effects without evaluating shell syntax; environment values are never printed in receipts.
Root cwd defaults to the replaced Tab's remembered focused terminal, otherwise the selected context's
terminal/default directory. Omitted child cwd follows its split source. Explicit cwd takes precedence.

Canonical membership, launch settlement and Client layout retain their existing owners. One workflow
records every submitted request and known effect under a single total deadline. Build all new
terminals, wait for Started facts, apply labels and exact split identities/ratios, and verify the ready
tree before closing an old Tab. Both the owned tree and the original replacement target are fenced
against concurrent changes. Required focus is acknowledged before destructive Close, avoiding an
inferred focus target after automatic navigation repair. Close acceptance is not completion: observe
the retained Closed status and the Client publication at that revision before reporting replacement
success. Final observation verifies the owned topology, template ratios and labels again. A later failure preserves completed
phases, request IDs, created resource IDs and recovery state; no mutation is blindly replayed.

Before the new tree is ready, definite failure may reclaim only the exact unchanged owned Tab.
Concurrent additions/edits or unknown outcomes preserve resources and explicit incomplete receipts.
Recovery consumes a reserved portion of the original timeout; it never extends the total deadline.
Once ready, failures in focus, replacement closure or final observation preserve the usable new tree.
The old tree remains usable until its explicit Close is submitted; an accepted or unknown Close is
reported honestly and is not described as rolled back. Ctrl+C settles known bounded recovery and
exits 130, preserving uncertain effects. General lifecycle events remain the automation owner's work.

The result is `orchbee.layout.apply` v1, with a completed/incomplete outcome, stage, old/new Tab
IDs, retained canonical and Client receipts, recovery status, final layout and structured failure.
Environment values are excluded. `api schema` catalog v75 includes this result and
`orchbee.cli.failure.layout-apply` v1. Text failures retain localized source diagnostics.

## Ownership and evidence

The deterministic projector owns topology bounds, exact split matching and portable leaf fields.
The Windows adapter owns admission, Client selection, temporary viewer cleanup, deadlines,
cancellation and final revision checks. The Server remains the sole membership/launch-fact owner;
the Client remains the view owner. No durable format or new dependency is introduced.

Deterministic evidence covers orientation/order, non-default ratios, labels, literal argv,
empty environments, missing/extra/duplicate facts, stale split slots and structural bounds. Windows
acceptance covers real cwd and frozen programs, selected/inactive and zoomed Tabs, multiple Clients,
unchanged focus/process identity, cancellation and schema validation. These checks do not establish
crash restoration, plugin persistence or generic event delivery.

Apply evidence covers complete-input validation, duplicate rejection, phase ordering, each failure
boundary, exact ownership and final-layout fences. Packaged Windows acceptance includes BOM files
and inherited pipes, nested and balanced layouts, clamped ratios, literal/empty/Unicode argv and
real environment delivery, remembered inactive cwd and child follow, two independent Clients,
active/inactive replacements, automatic labels, zoom reset, unchanged old Tabs after later launch
failure, bounded rollback, concurrent edits, stalled input, deadline and console cancellation.
Closed and Client-publication observations are both required before replacement succeeds.
Retained unknown effects are evidence of incomplete work, never successful compensation.
