# Native Panel labels

## Scope and ownership

`panel rename <panel> --label <text>` sets the exact leaf Panel's primary Presentation title
override; `--clear` removes that override. An empty or Unicode whitespace-only label also clears
it. The two flags are exclusive and one is required. Targets keep the existing Panel ID or
`@panel:<suffix>` grammar; labels, implicit selection and current-Panel routing are not targets
of this operation. Windows is the runtime acceptance platform.

The Server owns the shared override. `Presentation.label` remains the immutable creation fallback,
and optional `Presentation.customLabel` holds only a nonempty, trimmed override. An absent override
is omitted from snapshots; explicit `null` is not a valid stored override. The action's nullable
label update accepts a missing value, `null`, empty text or whitespace-only text as clear. Supplied
text is bounded to 512 Unicode code points before trimming. Trimming follows Unicode White_Space,
including NEL and excluding BOM; Rust, TypeScript and schema evidence must agree on that boundary.

Clear restores the existing creation fallback. It does not start deriving a Host natural title,
change a Tab title, change child Presentations or set the outer terminal window title. This is a
label-lifetime boundary of the native model, rather than a hidden replacement of fallback text.
Like other native canonical Workbench state, overrides last for the Site process; this operation
does not add restart persistence.

The owning implementation consists of the protocol label types and Presentation/action contracts,
the Server's pure Workbench authority, the shared native label-rename carrier, the CLI mutation
adapter, and `terminal_session/workbench_projection.rs`'s display projection. Maintain this
reference with those owners and the independent TypeScript contracts and JSON schemas.

## Command model

| Element        | Contract                                                                                                                                                                       |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Input          | An admitted navigation grant, request ID, exact Site and expected revision, context ID, Tab ID, Panel ID and normalized set/clear update.                                      |
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

The CLI's dedicated `orchbee.panel.rename` version-1 result records the original request/Site/base,
target identities, result revision, `renamed` or `unchanged`, and nullable normalized `customLabel`.
Request-side metadata is correlated input, not an invented Server observation. Existing text/JSON
failure envelopes and one absolute submission deadline apply. The CLI performs one snapshot read
and one mutation; it does not wait for a Client or submit a separate focus operation.

## Client projection

`NativePanel` keeps its immutable creation label and optional custom label separately. Display uses
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

The existing `WorkbenchNavigationCommit` label/ledger abstraction covers concurrent Clients,
retained results, stale bases, no-ops and lost responses; its target domain must also distinguish
the exact Panel target. Concrete pure authority tests establish the single-field footprint,
pending-creation preservation and byte/ledger/revision boundaries. Protocol fixtures independently
exercise Rust, TypeScript and JSON schema admission, normalization and stored omission semantics.
Native generated histories add set/clear and publication interleavings while retaining fallback,
identity and binding adversaries, with deterministic replay and reduced counterexamples.

Windows process evidence uses an owned Site and two TUIs to check global display convergence,
set/clear/no-op, nested Panel isolation and unchanged terminal identities and selection. It is
separate from deterministic carrier tests for definitely-not-submitted and ambiguous outcomes.
The root merge gate is still required. TLA assets and inventory are maintained; model execution is
deferred at the user's request and is not claimed as passing evidence.

Snapshots without overrides preserve their existing wire shape. The additive field and action
require a coordinated development cohort: older strict Clients may reject a snapshot containing
an override, and no mixed-version compatibility is claimed. There is no durable migration. Before
integration the change can be reverted as one contract slice; after integration use a matched
cohort, or clear overrides with that cohort before reverting. Never silently rewrite fallback
labels or discard canonical overrides to make an older reader accept them.

CLI Panel reads keep the creation `label` and add nullable `customLabel` plus effective
`displayLabel`; raw snapshots retain the canonical omitted-when-absent override. Current
schema versions and the catalog are owned by [Native CLI queries](native-cli-query.md).

In a leaf header, an explicit custom title precedes the ordinal so narrow panes keep user text visible. Uncustomized headers retain their existing localized resource prefix. Both forms use the same sanitization and display-cell truncation.
