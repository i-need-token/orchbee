# Windows reusable layouts

The native Windows and macOS CLI owns portable layout export and composition of canonical Workbench
membership, Client view state and read-only terminal facts. Linux answers unsupported-platform.

## Export

`layout export [--tab <target> | --panel <target> | --current] [--client <id>]
[--timeout <milliseconds>] [--format text|json]` emits `orchbee.layout.export` v1 JSON in either
format on success; failures follow the requested format. The default total timeout is 30,000 ms.
Ctrl+C exits 130 and releases owned temporary viewers and the admitted session.

An explicit Tab/Panel without `--client` reads canonical shared structure without a TUI.
Its source has `scope:shared`, null `clientId`/`viewRevision`, and null maximized-state and focus observations.
With `--client` or an omitted target, select the explicit, caller-matched or sole Ready Client; its source uses `scope:client`.
Missing or ambiguous required Client observations fail. Ratios always come from the shared tree. An omitted target uses the validated caller Panel or
selected Client Panel. Explicit targets ignore caller metadata. Tab targets admit ID, suffix or
unique label; Panel targets admit ID or suffix. Exporting a Panel exports its containing Tab.
Export never activates a Tab, changes focus, resizes a Host or acquires terminal control.

The result contains `layout:{workspaceId,terminalContextId,tabId,maximized,focusedPanelId,root}`.
The Workspace ID is required. Splits contain `type:split`, `direction:right|down`, `ratio`,
`first` and `second`. Panels contain `type:panel`, descriptive `panelId`, `primary`
and ordered `children`. Members contain descriptive `presentationId`, fallback `name`, required-nullable
manual `label`, `cwd` and `command`, plus `start` and empty `env`. Bound members carry available cwd
and immutable explicit command argv; unavailable cwd and default-shell command are null. Unbound members have `start:false`
and cause neither terminal reads nor a later default-shell launch. No environment values are exported.

At most 24 Panels, 16 levels including the root, 16 children per Panel and 1 MiB are admitted.
Split IDs, ordered slots and every bound primary/child must match the captured tree. Runtime facts
are joined by exact Presentation identity and binding. Unknown launch facts or unavailable runtime
observations fail without partial success. The source contains Session/revision and, for Client export, the Client/view
revision used for preflight, not a later publication revision. Sequential cwd samples are not an
atomic process snapshot. A final canonical revision check rejects interleaved membership changes.

## Apply

`layout apply (--file <path> | --stdin) [--tab <target> --confirm | --workspace <target>] [--label <label>] [--client <id>] [--focus | --no-focus]
[--timeout <milliseconds>] [--format text|json]` creates a new Tab or replaces the selected Tab.
Omitted destination uses the selected Client context. Replacement requires confirmation; exported
IDs never select a destination or reuse processes. Explicit destinations without `--client` or
`--focus` run headlessly (v1), including confirmed replacement. Ratios use the shared Server CAS.
An omitted destination, `--client` or `--focus` requires a Ready Client (v1). Default focus is false;
replacing that selected Client's active Tab selects the new Tab. Explicit label wins;
otherwise the old custom Tab label is retained or numbering remains automatic.

Acquire the entire file or inherited noninteractive stdin before effects, with optional UTF-8 BOM
and the export bounds. Accept only current v1 native export envelopes, descriptions containing root or bare
nodes. Native envelopes require all fields and exact current shapes; other versions/shapes return a
typed layout-input error with an expected-version-1 diagnostic. No migration occurs. A Panel
`rightClick` of `orchbee` or `panel`, written by earlier builds, is accepted and ignored. Flat `type:panel` templates describe one starting primary with numeric fallback names;
layout identity fields use camelCase. Structured members preserve their fallback names, labels and start intent.
Description IDs and focus and maximized state remain descriptive; new layouts start unmaximized. Mixing flat launch
fields with structured members, duplicate/unknown fields and malformed nested children is invalid.
A non-starting member cannot supply cwd, command or nonempty env. Ratios are finite and clamped
to 0.1..0.9. Validate every cwd, literal argv and environment overlay and resolve every intended
program before creation, without evaluating shell syntax. Environment values never enter receipts.
A member cwd resolves in the host's path grammar: the Windows drive grammar, or on macOS the POSIX
grammar with `~` from `HOME` and relative paths from the caller's directory. A bare program name
searches the member's `PATH` overlay or the caller's `PATH`, at most 256 absolute entries: Windows
matches the overlay name in any case and adds `.exe` to a name without an extension; macOS matches
`PATH` exactly and takes the first executable regular file. A name with a slash follows the
caller's directory.

First create the complete unbound tree with one `tab.create-layout` transaction. Labels and ordered
members are present immediately. Then bind only starting members through
exact `panel.create-terminal` commands, primary before children. Missing root cwd follows the
replaced Tab's remembered focused primary with a Client, or its first primary headlessly;
otherwise it follows the selected context's primary/default.
A split primary follows its source primary's cwd or inherited directory; children follow their
primary or the same inherited directory. An unbound primary never borrows a child's runtime.
Explicit cwd wins. Every intended member must reach Started before readiness and old-Tab Close.

One workflow retains every request and known effect within one total deadline; only a Windows stdin
pipe reader's start moves that deadline, and its recovery reserve, later by the start's length. A binding receipt
permits only that exact unbound-to-bound change in the owned tree, plus the launch failure the
Server records on that binding in its next revision when the launch fails; that member reports
`launch-failed`, and recovery still closes the owned Tab. No concurrent topology or label edit is
adopted. The original replacement target is also fenced. Apply exact split identities and
ratios, then verify readiness. Required focus is acknowledged before old-Tab Close. Close acceptance
is not completion: observe retained Closed status and, when a Client is selected, its publication before success. Final
observation rechecks topology, labels, member order/start intent and ratios.

Before readiness, definite failure may close only the exact unchanged owned Tab. Concurrent edits
or unknown outcomes preserve resources and explicit incomplete receipts. Recovery uses a reserved
part of the original timeout. Once ready, focus/Close/final-observation failure preserves the new
layout. The old tree remains usable until its Close is submitted; accepted or unknown Close is
never called rolled back. Ctrl+C finishes bounded known recovery and preserves uncertain effects.

The result is `orchbee.layout.apply` v1: completed/incomplete, stage, old/new Tab IDs, up to 448
bounded effect receipts, recovery, structured final layout and failure, under the native 8 MiB
output ceiling. The final export still has its own 1 MiB limit; exceeding it leaves an incomplete
post-ready receipt with the new Tab and effects preserved. Layout effects record only
action, Session, request, base revision and validated outcome, never launch arguments or environment.
The catalog describes both output versions and the unchanged layout-apply failure v1.
Headless v1 records null source Client identity and retains each shared ratio CAS effect separately. Text diagnostics
remain localized. Group/pin roundtrip and complete saved-focus restoration remain separate work.

## Ownership and evidence

The pure template/projector owns shape, bounds, member intent and identity-free reconstruction.
The Server owns canonical membership and launches; the Client owns view observations. The CLI
adapter owns exact binding joins, send-once submission, ownership fences, cancellation and cleanup.
Shared ratios persist in the current version 1 checkpoint and catalog; no dependency is introduced. Mixed releases require a matched private cohort.

Headless export, topology reads, ratio changes, pair swaps and apply reuse existing Server transitions; no virtual viewport, new canonical mutation or hidden
Client registration is introduced.

Server shares and persists split ratios. Export records the current nominal ratios; apply uses the same canonical mutation path. Each window retains its own focus, maximized state and control. See the [shared ratio contract](../../orchbee-server/docs/shared-split-ratios.md) for compatibility.

Each known shared-ratio receipt advances the workflow-owned Tab copy by exactly that identified ratio change. The next Server read still fences the complete new and replaced Tabs, including every other ratio, label, binding and ordered identity. Unknown results do not widen or refresh ownership; no concurrent edit is adopted as part of the workflow.
