# Native Panel labels

## Scope and ownership

`panel rename <panel> --label <text>` sets the exact leaf Panel's primary Presentation title
override; `--clear` removes that override. An empty or Unicode whitespace-only label also clears
it. The two flags are exclusive and one is required. Targets keep the existing Panel ID or
`@panel:0x<hex>` grammar; labels, implicit selection and current-Panel routing are not targets
of this operation. Windows is the runtime acceptance platform.

`panel split --label <label>` names the new Panel the same way inside its creation commit. The
`panel.split` action carries an optional nullable `customLabel` with the override grammar below;
the Server stores it on the new primary Presentation, and omission or `null` leaves no override.
The CLI sends the explicit label as both the creation label and the trimmed override, so the
result equals a split followed by a rename. TUI and GUI splits and configured commands omit it.

The native Panel context menu and the global action menu offer Rename Panel without a new
default key. The context row uses Rename because its frame already names the Panel. Both open
the existing identity-bound name editor, prefilled with the primary Presentation's override.
An empty field shows the primary Presentation's automatic name as a placeholder and accepts a clear update; the localized
help explains that leaving it empty restores the original name. Editing and Escape have no Server
effect. Submit rechecks the exact captured Panel and snapshot and uses the same canonical
set/clear request as the CLI. A visible child terminal does not change the primary target.
The dialog uses the shared rename draw, input-capture, suspension, single-submission and unknown
outcome recovery owners, which Tab renames share in their modal Rename Tab dialog.

The Server owns the shared override. `Presentation.label` remains the immutable creation fallback,
and required `Presentation.customLabel` holds a nonempty, trimmed override or explicit `null`.
Omission is invalid; null means no override. The action's nullable
label update accepts a missing value, `null`, empty text or whitespace-only text as clear. Supplied
text is bounded to 512 Unicode code points before trimming. Trimming follows Unicode White_Space,
including NEL and excluding BOM; Rust, TypeScript and schema evidence must agree on that boundary.

Clear resumes automatic display naming. It never rewrites the immutable creation label,
changes a Tab's naming mode, changes child Presentations or sets the outer terminal window title.
The existing Windows Workbench checkpoint retains these shared overrides; renaming adds no
separate store.

The GUI Panel menu and Action search offer the same `workbench.panel.rename` operation. Its
Name Dialog uses the same primary scope and set/clear rule, even while a child terminal is shown.
An exact captured Panel and primary Presentation qualify submission; changing focus never
redirects a name, and removing the target closes its editor. Only the Server publication updates
the displayed override. Both Clients use Rename in context, Rename Panel in the editor and
global discovery, and the same localized explanation of a blank name. Neither adds a default key
or a confirmation for this reversible organizational edit.

The owning implementation consists of the protocol label types and Presentation/action contracts,
the Server's pure Workbench authority, the shared native label-rename carrier, the CLI mutation
adapter, and `workbench_projection.rs`'s display projection. Maintain this
reference with those owners and the independent TypeScript contracts and JSON schemas.

## Command model

| Element        | Contract                                                                                                                                                                       |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Input          | An admitted navigation grant, request ID, exact Session and expected revision, context ID, Tab ID, Panel ID and normalized set/clear update.                                   |
| State          | One validated canonical snapshot and the existing bounded command ledger; no separate label cache or private fallback map.                                                     |
| Target         | The exact context contains the exact Tab and leaf Panel. Only its primary Presentation receives the update, including while terminal creation is pending.                      |
| Changed result | Prepare the entire candidate, encoded projection and ledger result before committing exactly one revision. Return `renamed` under the original request ID.                     |
| No-op          | Equal normalized override, including clearing an absent override, returns `unchanged` without a revision, identity or Host effect. Existing ledger capacity rules still apply. |
| Failure        | Authorization, stale revision, missing target, revision exhaustion and encoded/ledger capacity refusal preserve all canonical state.                                           |
| Replay         | Existing semantic request equality and retained result precedence apply. A duplicate cannot apply twice; conflicting reuse cannot replace the first receipt.                   |
| Lost response  | The carrier retires ambiguous submissions and reports unknown outcome. It never retries, undoes, or invents an acknowledgment.                                                 |
| Acknowledgment | Canonical commit or no-op; it does not assert that any Client has drawn the new title.                                                                                         |

The action changes only the primary Presentation's optional override. Fallback text, all identities,
bindings, topology, order, directories and lifecycle facts remain exact. It neither acquires a
terminal controller nor changes focus, the maximized state, ratios or attachments. A later split, move or terminal
creation settlement must preserve an existing override on the surviving Presentation.

The CLI's dedicated `orchbee.panel.rename` version-1 result records the original request/Session/base,
target identities, result revision, `renamed` or `unchanged`, and nullable normalized `customLabel`.
Request-side metadata is correlated input, not an invented Server observation. Existing text/JSON
failure envelopes and one absolute submission deadline apply. The CLI performs one snapshot read
and one mutation; it does not wait for a Client or submit a separate focus operation.

## Client projection

`ClientPanel` keeps its immutable creation label and optional custom label separately. Display uses
the override when present and the fallback otherwise. Panel chrome, the Agent board and confirmation
copy apply existing terminal-text sanitization and width bounds to that display label. Canonical
Panel reads expose the override and effective display text. Creation defaults continue to use the
creation label rather than changing the meaning of that existing field.

An equal-revision projection must remain exactly equal, including the custom label. At a greater
revision, custom labels may change while the retained Panel/Presentation/binding/fallback tuple
remains unchanged. A valid label refresh updates the display cache and preserves selection, pane
incarnations, viewer/controller attachments, frames and terminal geometry. Stale publications,
same-revision contradictions, and changes to the immutable fallback or binding retain their
existing rejection behavior. Custom labels are not attachment identity and never authorize input.

## Evidence and compatibility

[Server authority tests](../../orchbee-server/src/workbench_authority/tests/panel_labels.rs) cover
normalized set/clear, no-op, replay, non-target preservation, pending creation, authorization and
capacity. [Native projection traces](../crates/workbench-client/src/workbench_projection/tests/label_evolution.rs)
cover custom-label evolution and immutable identity rejection. These maintained tests do not run
two native TUIs against one Session.

Native Windows process validation must use an owned Session and two TUIs to check global display
convergence, set/clear/no-op, nested Panel isolation and unchanged terminal identity. Run the
applicable root gate and record this native witness separately; a passing owner test or gate does
not itself establish that interaction. No separate label TLA model is retained.

Snapshots and layout presentations require explicit nullable overrides. All matched Clients use
this shape; omitted fields are rejected without migration. Canonical fallback labels cannot be
whitespace-only. Never rewrite fallback text or discard an override to accommodate an old reader.

CLI Panel reads keep the creation `label` and add nullable `customLabel` plus effective
`displayLabel`; raw snapshots require the canonical nullable override. Current
schema versions and the catalog are owned by [Native CLI queries](native-cli-query.md).

## Automatic display names

A Panel's custom label and automatic name belong to its primary Presentation, including while a child terminal is displayed.
For TUI names, the primary custom override wins, followed by the guarded title from the existing
selected-Presentation [metadata observation](windows-panel-report-presentation.md#header-behavior).
Without either, it reuses the [Tab naming rule](native-tab-labels.md): the program-set
title or a normalized console default title, then the committed launched program or failed-launch
program, with its localized position last. The existing guarded live/retained observation owner
qualifies a title by exact Presentation and binding; a child's frame never names its parent.
An explicit custom number stays exact. A creation label or current directory never outranks
that rule. Agent tokens and state remain separate annotations.

The TUI Panel menu, split header, close confirmation, rename placeholder and notification context
use this name. Rename placeholders omit the current override, showing what a clear would restore.
The navigator lists Tabs rather than Panels and keeps the existing Tab naming rule. GUI Panel
headers, close confirmations and rename placeholders use the primary custom/automatic rule; child
terminal strips and terminal accessibility names still identify their own Presentation. The
human CLI `panel list` title uses the same terminal-title and launch-program naming functions;
its ordinal/handle columns and JSON remain unchanged. The `{pane}` window-title token remains
the explicitly documented manual override, not a chrome-name projection. All displayed names
retain sanitization and display-cell truncation, without changing canonical labels or identities.

The GUI Panel/terminal observation path does not carry a guarded metadata title, so this delivery
leaves GUI naming unchanged. `ServerAgentNameSource` reads a separate Agent-scoped `reportedTitle`;
that is not a Panel observation and cannot name Agent-less Panels. It is not repurposed as an OSC
or canonical title. GUI metadata-title parity remains unavailable until its Panel observation
owner carries that field with the required guards and binding lifetime.
