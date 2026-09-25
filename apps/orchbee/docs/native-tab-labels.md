# Native Tab labels

## Canonical state and display

Server owns `Tab.label` and optional boolean `Tab.automaticLabel`. Omission means `false`;
serialization omits `false`. A legacy Tab keeps its exact name, including an explicitly chosen
number. Readers never infer automatic naming from text.

Windows `tab create` without `--label`, native TUI Tab creation, and the first implicit standalone
Tab request automatic naming. Explicit labels, Workspace launch labels, and Panel relocation
labels keep their existing fixed-name behavior. `tab.create` carries the same optional boolean;
the request fingerprint includes it. The supplied nonempty bounded `label` still initializes
both the raw Tab label and primary Presentation label.

Within each terminal context, an automatic Tab displays its current one-based position. A fixed
Tab displays its raw label. This is a pure projection of the canonical ordered vector, including
after move or close; it never renames a survivor or commits another revision. Raw labels,
identities, Presentation labels and bindings remain unchanged by reordering. Duplicate display
names are permitted.

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

The abstract label values in `WorkbenchNavigationCommit` and the native rename model represent
the complete naming fact `(raw label, automatic mode)`, not just visible text. The create model's
created/renamed distinction also represents clearing automatic mode with unchanged text.
Movement models preserve this metadata and derive display from order. Concrete codec, authority,
projection and Windows process tests establish this refinement; bounded model execution remains
separate evidence and is not implied by inventory checks.

## Compatibility and verification

This is a coordinated experimental OBWP v1 cohort. Rust and TypeScript accept only a boolean
when the new field is present; legacy omission remains fixed. Older strict readers may reject
the field, so deploy matching binaries. No persistent migration or reinterpretation of existing
numeric labels occurs. Rolling back an unpersisted test Site requires restarting that Site.

Verification covers legacy and invalid-field codecs; explicit numeric names; create, reorder,
close and rename; equal-text mode changes and replay; unrelated Tab and Presentation stability;
cross-context numbering and ambiguous selectors; strict native projection and actual Windows
TUI display with unchanged focus identities. Other operating-system runtime behavior is outside
the Windows acceptance evidence.
