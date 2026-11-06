# Native Panel labels

## Scope and ownership

`panel rename <panel> --label <text>` sets the exact leaf Panel's primary Presentation title
override; `--clear` removes that override. An empty or Unicode whitespace-only label also clears
it. The two flags are exclusive and one is required. Targets keep the existing Panel ID or
`@panel:0x<hex>` grammar; labels, implicit selection and current-Panel routing are not targets
of this operation. Windows is the runtime acceptance platform.

The native Panel context menu and the global action menu offer Rename Panel without a new
default key. The context row uses Rename because its frame already names the Panel. Both open
the existing identity-bound name editor, prefilled with the primary Presentation's override.
An empty field shows the creation name as a placeholder and accepts a clear update; the localized
help explains that leaving it empty restores the original name. Editing and Escape have no Server
effect. Submit rechecks the exact captured Panel and snapshot and uses the same canonical
set/clear request as the CLI. A visible child terminal does not change the primary target.
The dialog uses the shared rename draw, input-capture, suspension, single-submission and unknown
outcome recovery owners; only Tab renames use the inline Tab editor.

The Server owns the shared override. `Presentation.label` remains the immutable creation fallback,
and required `Presentation.customLabel` holds a nonempty, trimmed override or explicit `null`.
Omission is invalid; null means no override. The action's nullable
label update accepts a missing value, `null`, empty text or whitespace-only text as clear. Supplied
text is bounded to 512 Unicode code points before trimming. Trimming follows Unicode White_Space,
including NEL and excluding BOM; Rust, TypeScript and schema evidence must agree on that boundary.

Clear restores the existing creation fallback. It does not start deriving a Host natural title,
change a Tab title, change child Presentations or set the outer terminal window title. This is a
label-lifetime boundary of the native model, rather than a hidden replacement of fallback text.
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
terminal controller nor changes focus, zoom, ratios or attachments. A later split, move or terminal
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

In a leaf title, a provided custom Presentation title or metadata title is displayed without an added Panel ordinal, including explicitly provided numeric titles. When neither is available, the generated title retains its localized resource prefix and Panel ordinal. A bare ASCII number inherited as the creation label is omitted from that title; a nonnumeric creation label follows the ordinal. This display choice never rewrites the immutable label. All forms use the same sanitization and display-cell truncation.
