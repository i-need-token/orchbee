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

Native Tab-strip, navigator, status, Tab-menu, close-confirmation and Agent board chrome share one display name. Fixed names remain
exact. An automatic name comes from one Panel of the Tab: the focused Panel while the Tab is selected,
otherwise the Panel selecting it would focus, which is the one this Client last left focused, else the first.
Selecting another Tab therefore never changes a Tab's name; moving focus inside the selected Tab does.
Automatic names prefer that Panel's selected Presentation's custom label, then the title its program set
for the Presentation's current binding, by the [background title rule](#background-terminal-titles):
the live OSC title of a frame this Client draws or the title the Server publishes, whichever changed
last. A console
host's default title is the absolute path of the launched Windows executable, which cmd extends to
`<path> - <command>` while it runs a command; that is launch identity rather than a program title, so
chrome names the program instead and keeps the command. Without a title, the program the Server
committed with the Presentation's binding (`launchProgram`) is the next fallback, then the launched
program from this Client's failed-creation record, then from the Presentation's launch failure, then
the localized position (`Tab 1` in `en-US`, `标签页 1` in `zh-CN`). Titles are
stripped as in [Panel terminal observations](native-cli-query.md#panel-terminal-observations), which
removes one Windows elevation prefix and one activity prefix, and all displayed text is sanitized.
A title the Server published arrives stripped and is never stripped again.
This presentation never renames canonical state or claims an observed title is authoritative process
identity; raw title observations remain unchanged. Canonical ordinal labels, CLI selectors, and
launch-profile selection keep their existing semantics.

Program names follow one rule on every platform. A recognized shell shows its product name: `pwsh`
as `PowerShell`, `powershell` as `Windows PowerShell`, `cmd` as `Command Prompt` (`命令提示符` in
`zh-CN`), `bash` as `Bash` and `zsh` as `Zsh`. Any other program shows its file name without its
directory or a `.exe`, `.com`, `.bat` or `.cmd` extension, so `C:\tools\node.exe` shows `node`.
Matching ignores ASCII case. Launch-profile names are not Presentation facts, so chrome does not use
them. The Windows console host reports no title until the program sets one (Command Prompt does when
it runs a command), and POSIX programs report only titles they set. The Server therefore commits each
binding's launched program with that binding, taken from the same launch it dispatches: an explicit
launch's executable, the custom default program, or the System shell that the Worker resolved for
its generation. The fact records the program the launch requested, not proof that it started. On
every platform, a program that has reported no title, and a Tab whose terminal this Client has not
yet observed, show that program; only a Presentation without that fact shows its position. A
Presentation lacks it when it is unbound, and a bound one lacks it when its launch's preference
capture was unavailable, its Worker reported no System shell, or the program's file name exceeds 128
bytes. A GUI on a native Session names Tabs and terminals by this rule too, and
[its README](../../web/README.md) records where it differs.

Automatic rename drafts start empty with the same current display name as an informational
placeholder; only typed text can submit. Fixed names prefill the editor wholly selected, so typing
replaces them. The native TUI shows the rename draft in its modal Rename Tab dialog, the shared name
dialog every rename and group name uses: the field holds the name, with no line repeating it. Enter
submits the name with leading and trailing whitespace trimmed, as the GUI does, and Esc cancels,
under the same exact shown token and input fences. A trimmed copy of a fixed Tab's current name is
unchanged, and a whitespace-only name is refused.

[Panel display names](native-panel-labels.md#automatic-display-names) reuse this terminal naming
rule for the Panel's primary Presentation, even when the displayed terminal is a child. They
fall back to the Panel's own position; they never borrow another Panel's focused terminal name.

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

## Background terminal titles

A Tab keeps naming its terminal by the title its program set while this Client draws no frame for
it. The Server publishes each bound Presentation's title on the `panel-agent` `presentation-titles`
State stream, which the [Server README](../../orchbee-server/README.md) and the
[Workbench schema](../../../schemas/workbench/README.md) define: already stripped and bounded to
1 KiB, within about 250 ms of a commit or title change. The native TUI follows every bound
Presentation of the Session, across all Workspaces, in canonical Workspace, Tab, Panel and member
order, as groups of at most 64 with one stream each, and reopens only a group whose members changed.
A failed outcome, a gap or an ended stream clears no title, and an ended stream opens again 5 seconds
later. The [GUI README](../../web/README.md) records the GUI's own subscription.

Both Clients name each Presentation by one rule:

- Of the title a live frame this Client draws reports and the title the stream publishes, the one
  that changed last wins. A value arrives only when it differs from the previous value of its
  source, so a group republished for another member never overrides a newer frame title.
- The latest row of each Presentation is kept and applies only while its binding equals the
  snapshot's current binding. A row that arrives before its snapshot applies once the snapshot
  catches up; a row without a binding never applies.
- A cleared title shows the program. `unavailable` keeps the title last shown for that binding, or
  the program when none was shown, so a Server restart never flashes a Tab back to its program.
- When the snapshot binds the Presentation to another terminal, it shows the program until that
  terminal reports a title. Otherwise a Tab keeps the title it last showed, including while its
  own frame is pending, blocked or faulted.
- A Disconnected Client keeps the names it last showed. It draws no live frame and receives no
  row, so no name changes until the person leaves or a new connection replaces the view.
- A title marks nothing unread and changes no notification or Agent state.

The window title's `{tab}` and terminal title placeholders still read the live frame only. The
Agent board names other Workspaces' Presentations by the same titles.

## Consumers and model

CLI Tab rows, Workspace Tab references, and all Tab label selectors use the effective display
name. Exact IDs and suffix selectors retain their existing precedence; multiple matching names
produce the existing ambiguity result. `api snapshot` exposes raw canonical state. Query row
shapes remain unchanged. `orchbee.api.snapshot` v1 admits the optional metadata; the native schema
The catalog exposes the current documents and command paths.

The native strict projector retains raw text and naming mode. Exact-revision move/split/close
checks preserve the complete surviving metadata; a rename changes only its target text and
mode. Same-revision refresh remains exact, while later canonical revisions may contain a
rename. Create validation pins the submitted mode at the commit revision and takes the Server-chosen binding and launched program from the candidate, so that fact never breaks exact Create validation. A surviving fixed Tab cannot become automatic again; an automatic survivor keeps its raw
creation text until a rename switches it to fixed mode. Completion
cannot overwrite an intervening rename. Close confirmation names a live Tab as the strip does and
otherwise keeps the name captured when it opened. An Agent board row names its attached
Presentation and that Presentation's Tab by the same rule: for a Tab of the current context it
uses the strip's own names; for another context it applies the rule to snapshot facts and the
titles the Server publishes there (custom label, then title, then committed or failed launch
program, then position). A Presentation with no name of its own takes its Tab's name,
and the row omits a Tab name equal to its title. Selection, input
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

A pointer resting on the strip opens a [pointer tooltip](../../../docs/tui/overlays-and-status.md#pointer-tooltips) where glyphs alone carry meaning or a label is cut. A Tab or group header whose label the strip cuts gives the whole name; a whole label offers nothing. Glyphs name themselves only under the pointer: a Tab's state glyph names its Agent state, a collapsed group header's glyph each distinct state among its hidden Tabs, most urgent first, and an unread dot unread activity. The rest of the token names no glyph. `[x]` names Close Tab…, and only the current Tab's adds the key that closes it, from the binding table. `[+]` names New Tab and its key. The leading edge marker names Show earlier Tabs, and the trailing one Show later Tabs.
