# Native Tab labels

## Canonical state and display

Server owns `Tab.label` and optional boolean `Tab.automaticLabel`. Omission means `false`;
serialization omits `false`. A legacy Tab keeps its exact name, including an explicitly chosen
number. Readers never infer automatic naming from text.

Windows `tab create` without `--label`, native TUI Tab creation, and the first Tab of `workspace.launch` request automatic naming. Workspace launch keeps its
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

Native Tab-strip, navigator, status and Tab-menu chrome share one display name. Fixed names remain
exact. Automatic names prefer a selected Presentation's custom label, then the title its program set:
the valid live OSC title, or for inactive Tabs the last observed title from the bounded frame cache
only while the exact Session/context/Tab/Panel/Presentation/binding remains canonical. A console
host's default title is the absolute path of the launched Windows executable, which cmd extends to
`<path> - <command>` while it runs a command; that is launch identity rather than a program title, so
chrome names the program instead and keeps the command. Without a title, the launched program from
this Client's failed-creation record, then from the Presentation's launch failure, is the next
fallback, then the localized position (`Tab 1` in `en-US`, `标签页 1` in `zh-CN`). Activity prefixes
are stripped and all displayed text is sanitized. This presentation never renames canonical state or
claims an observed title is authoritative process identity; raw title observations remain unchanged.
Canonical ordinal labels, CLI selectors, and launch-profile selection keep their existing semantics.

Program names follow one rule on every platform. A recognized shell shows its product name: `pwsh`
as `PowerShell`, `powershell` as `Windows PowerShell`, `cmd` as `Command Prompt` (`命令提示符` in
`zh-CN`), `bash` as `Bash` and `zsh` as `Zsh`. Any other program shows its file name without its
directory or a `.exe`, `.com`, `.bat` or `.cmd` extension, so `C:\tools\node.exe` shows `node`.
Matching ignores ASCII case. Launch-profile names are not Presentation facts, so chrome does not use
them. Outside Windows console titles, Clients learn a launched program only from a launch failure, so
a started POSIX program that sets no title shows its position. The GUI Workbench does not yet attach
to native Sessions and keeps its own title resolution; this naming is a staged native-only delivery.

Automatic rename drafts start empty with the same current display name as an informational
placeholder; only typed text can submit. Fixed names prefill the editor. The native TUI renders the existing
rename draft over its whole painted Tab, close control included, widening along the strip to at least
24 cells when possible; when fewer than eight cells fit it uses the complete
modal. Enter submits and Esc cancels; the same exact shown token and input fences apply in both forms.

Compact Tabs show the active Tab as a bold label on the current surface, without a marker,
and end labels longer than 24 cells in an ellipsis. A Tab with a reported Agent leads with its state
glyph and follows with an unread dot. Each wide enough token reserves a stable three-cell close track
that shows `[x]` on the active Tab and while the pointer is on the token, so revealing it never
moves the strip; a hovered `[x]` uses the destructive accent. A hidden track still selects its Tab,
and narrow tokens keep their whole label selectable. Pinned Tabs are separated from the rest by one
vertical rule instead of a text annotation. Close validates the painted context/revision and exact Tab ID, then
opens the existing confirmation for that single Tab. Two releases on the same unchanged Tab within
500 ms and one horizontal cell request rename only if that Tab is now selected. Dragging, focus loss,
other targets, changed Session/context identity, and capture release break the pair. A newer revision
from the first selection preserves the pair; the second click still validates the newly painted source.
These controls never forward input.

`tab rename` always chooses a fixed name. Renaming an automatic Tab to the same visible or raw
text is a real change: it clears automatic naming and advances one revision. Repeating that
rename on the fixed Tab is unchanged. Blank-name reset is not part of this contract.

## Consumers and model

CLI Tab rows, Workspace Tab references, and all Tab label selectors use the effective display
name. Exact IDs and suffix selectors retain their existing precedence; multiple matching names
produce the existing ambiguity result. `api snapshot` exposes raw canonical state. Query row
shapes remain unchanged. `orchbee.api.snapshot` v1 admits the optional metadata; the native schema
The catalog exposes the current documents and command paths.

The native strict projector retains raw text and naming mode. Exact-revision move/split/close
checks preserve the complete surviving metadata; a rename changes only its target text and
mode. Same-revision refresh remains exact, while later canonical revisions may contain a
rename. Create validation pins the submitted mode at the commit revision. A surviving fixed Tab cannot become automatic again; an automatic survivor keeps its raw
creation text until a rename switches it to fixed mode. Completion
cannot overwrite an intervening rename. Close confirmation and Agent board retain names derived from the target's position in the captured
context; native chrome uses the display projection above. Selection, input
ownership, geometry and terminal controller rights are independent of naming.

The naming fact is the complete pair `(raw label, automatic mode)`, not only visible text. Creating or renaming can clear automatic mode without changing text. Movement preserves that metadata and derives display from order. Concrete codec, authority, projection and Windows process tests check these obligations; there is no separate label workflow model.

## Compatibility and verification

This is a coordinated experimental OBWP v1 cohort. Rust and TypeScript accept only a boolean
when the new field is present; legacy omission remains fixed. Older strict readers may reject
the field, so deploy matching binaries. No persistent migration or reinterpretation of existing
numeric labels occurs. Rolling back an unpersisted test Session requires restarting that Session.

Verification covers legacy and invalid-field codecs; explicit numeric names; create, reorder,
close and rename; equal-text mode changes and replay; unrelated Tab and Presentation stability;
cross-context numbering and ambiguous selectors; program naming and automatic-name precedence in
both locales; strict native projection and actual Windows TUI display with unchanged focus
identities. Other operating-system runtime behavior is outside the Windows acceptance evidence.

An overflowing compact strip preserves the active token and its group context and, when space permits, paints an ellipsis marker at each hidden edge. Each marker is a glyph action that pages the strip once on release: the trailing marker opens the window at the first token the current window leaves out or shortens, and the leading marker opens the window that ends just before the current one. A paged window starts at that token wherever the current Tab is, without a group prefix. It holds while the current Tab is unchanged or changes through a pointer activation of a Tab painted in it; any other change of the current Tab, such as keyboard navigation or Tab creation, returns the strip to the window around the current Tab. Closing the Tab that opens the window moves its start to the next surviving Tab, then the previous one. Paging is client-local and changes no canonical state. Close tracks remain separate from those markers. A truncated label keeps at least its first three cells before the ellipsis, or the whole of a shorter label; a token that cannot keep that minimum beside its state glyph is not painted as a clipped stub, and the edge marker stands for it.
