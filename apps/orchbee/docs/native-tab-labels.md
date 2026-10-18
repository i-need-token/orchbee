# Native Tab labels

## Canonical state and display

Server owns `Tab.label` and optional boolean `Tab.automaticLabel`. Omission means `false`;
serialization omits `false`. A legacy Tab keeps its exact name, including an explicitly chosen
number. Readers never infer automatic naming from text.

Windows `tab create` without `--label`, native TUI Tab creation, the first implicit standalone
Tab, and the first Tab of `workspace.launch` request automatic naming. Workspace launch keeps its
Workspace label separate and initializes the Tab and primary Presentation with raw label `1`.
Explicit Tab labels and Panel relocation labels keep their existing fixed-name behavior.
`tab.create` carries the same optional boolean;
the request fingerprint includes it. The supplied nonempty bounded `label` still initializes
both the raw Tab label and primary Presentation label. Duplicate placement instead copies the exact
source raw label and automatic/fixed mode, using that raw label for its fresh Presentation; it never
turns an automatic displayed ordinal into a fixed name. Ordinary group creation remains automatic.

Within each terminal context, an automatic Tab displays its current one-based position. A fixed
Tab displays its raw label. This is a pure projection of the canonical ordered vector, including
after move or close; it never renames a survivor or commits another revision. Raw labels,
identities, Presentation labels and bindings remain unchanged by reordering. Duplicate display
names are permitted.

Native Tab-strip and navigator chrome localize automatic positions as `Tab 1`, `Tab 2`, … in
`en-US` and `标签页 1`, `标签页 2`, … in `zh-CN`. The rename dialog uses that same display name for
its current-name line. Automatic-name drafts start empty and show the current display name as an
informational placeholder; it is not submitted as the new label. Fixed names, including numeric
names, remain exact and prefill the editor. The provisional first Tab uses the same title during
Workspace creation. Canonical ordinal labels, CLI selectors, and launch-profile selection keep their
existing semantics.

`tab rename` always chooses a fixed name. Renaming an automatic Tab to the same visible or raw
text is a real change: it clears automatic naming and advances one revision. Repeating that
rename on the fixed Tab is unchanged. Blank-name reset and terminal-title following are not
part of this contract.

## Consumers and model

CLI Tab rows, Workspace Tab references, and all Tab label selectors use the effective display
name. Exact IDs and suffix selectors retain their existing precedence; multiple matching names
produce the existing ambiguity result. `api snapshot` exposes raw canonical state. Query row
shapes remain unchanged. `orchbee.api.snapshot` v4 admits the optional metadata; the native schema
catalog is v36 with 69 documents.

The native strict projector retains raw text and naming mode. Exact-revision move/split/close
checks preserve the complete surviving metadata; a rename changes only its target text and
mode. Same-revision refresh remains exact, while later canonical revisions may contain a
rename. Create validation pins the submitted mode at the commit revision. A surviving fixed Tab cannot become automatic again; an automatic survivor keeps its raw
creation text until a rename switches it to fixed mode. Completion
cannot overwrite an intervening rename. The strip, rename draft, close confirmation and Agent
board derive labels using the target's position in the captured context. Selection, input
ownership, geometry and terminal controller rights are independent of naming.

The naming fact is the complete pair `(raw label, automatic mode)`, not only visible text. Creating or renaming can clear automatic mode without changing text. Movement preserves that metadata and derives display from order. Concrete codec, authority, projection and Windows process tests check these obligations; there is no separate label workflow model.

## Compatibility and verification

This is a coordinated experimental OBWP v1 cohort. Rust and TypeScript accept only a boolean
when the new field is present; legacy omission remains fixed. Older strict readers may reject
the field, so deploy matching binaries. No persistent migration or reinterpretation of existing
numeric labels occurs. Rolling back an unpersisted test Session requires restarting that Session.

Verification covers legacy and invalid-field codecs; explicit numeric names; create, reorder,
close and rename; equal-text mode changes and replay; unrelated Tab and Presentation stability;
cross-context numbering and ambiguous selectors; strict native projection and actual Windows
TUI display with unchanged focus identities. Other operating-system runtime behavior is outside
the Windows acceptance evidence.
