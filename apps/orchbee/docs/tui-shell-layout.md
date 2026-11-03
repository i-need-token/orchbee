# TUI shell layout

## Scope and ownership

The native Client shell divides the host area into a top strip, a content region, an optional side
panel, a pane area with shared boundaries, and an on-demand floating status chip. This reference documents the
concrete regions, thresholds, and degradation order that `docs/tui/layout.md` requires the owning
application to declare. It owns the layout projection only: colors, glyphs, and frame style belong
to the theme surface, overlay contents belong to their surface renderers, and terminal cell drawing
belongs to `terminal_cells`.

Shortcut discovery belongs to `tui_shortcut_help`: it selects effective bindings and active modes, filters capability availability, and merges stable action identities before rendering. Localization owns semantic labels and groups, never a `ClientView` or its filtering policy. Shell hints receive the effective prefix and action labels selected by `Bindings::shell_bindings`; their placeholder substitution retains the original ordering and unbound text.

The projection is pure. `tui_layout::project_shell_areas` computes every region from the current
frame rectangle and the sidebar surface (including its preferred width) the view asks for; `tui_shell` computes the terminal
geometry from the same two functions (`project_shell_areas` then `native_terminal_area`), so the drawn
content area and the wire geometry can never diverge. No render code mutates business state, clocks,
or I/O, and no layout value is cached across frames.

## Reusable controls and modal composition

This application keeps reusable terminal controls and recurring composition private to the
application until a real cross-package consumer exists. The repository-wide
[TUI component standard](../../../docs/tui/components.md) defines the required contract; this
section records the currently implemented reuse points and their evidence. A surface is conformant
only where its adoption tests exercise the shared implementation.

| Responsibility                     | Canonical source and semantic types                                                                                                                                                                                                                                                                                                                                                                         | Behavior and adoption evidence                                                                                                                                                                                                                                                                                                                                                      |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Actions and fixed icon actions     | [`controls.rs`](../crates/workbench-client/src/tui_render/controls.rs), [`action.rs`](../crates/workbench-client/src/tui_render/controls/action.rs), and [`icon.rs`](../crates/workbench-client/src/tui_render/controls/icon.rs); `ActionSpec`, `IconActionSpec`, `InteractionState`, `Availability`, and `PointerPresentation`; `frame_close_rect` and `render_frame_icon_action` for frame-borne controls | [`controls/tests.rs`](../crates/workbench-client/src/tui_render/controls/tests.rs), [`icon/tests.rs`](../crates/workbench-client/src/tui_render/controls/icon/tests.rs), and adopted Settings and modal renderer tests                                                                                                                                                              |
| Structured key hints               | [`hint.rs`](../crates/workbench-client/src/tui_render/controls/hint.rs); `KeyHint`, `measure_key_hints`, and `render_key_hints` for persistent chrome; `measure_overlay_hints` and `render_overlay_hints` for overlay footers                                                                                                                                                                               | [`hint/tests.rs`](../crates/workbench-client/src/tui_render/controls/hint/tests.rs) plus [`settings/tests.rs`](../crates/workbench-client/src/tui_render/settings/tests.rs), [`workspace_setup/tests.rs`](../crates/workbench-client/src/tui_render/workspace_setup/tests.rs), and [`worktree_launch/tests.rs`](../crates/workbench-client/src/tui_render/worktree_launch/tests.rs) |
| Overlay query slot                 | [`query.rs`](../crates/workbench-client/src/tui_render/controls/query.rs); `QueryField`, `render_query_field`, and `render_divider`                                                                                                                                                                                                                                                                         | [`settings/tests.rs`](../crates/workbench-client/src/tui_render/settings/tests.rs), [`help/tests.rs`](../crates/workbench-client/src/tui_render/help/tests.rs), and [`machine_picker/tests/agents.rs`](../crates/workbench-client/src/machine_picker/tests/agents.rs)                                                                                                               |
| Full-row interaction composition   | [`row.rs`](../crates/workbench-client/src/tui_render/controls/row.rs); `RowTracks`, `RowContentBand`, and `RowFocusCue`                                                                                                                                                                                                                                                                                     | [`controls/tests.rs`](../crates/workbench-client/src/tui_render/controls/tests.rs) plus the Settings, Workspace setup, and Worktree renderer tests linked above                                                                                                                                                                                                                     |
| Recurring native modal composition | [`modal.rs`](../crates/workbench-client/src/tui_render/modal.rs); `ModalLayout`, `SingleModalLayout`, and `ModalAction`                                                                                                                                                                                                                                                                                     | [`modal/tests.rs`](../crates/workbench-client/src/tui_render/modal/tests.rs), feature renderer tests, and the real [modal pointer round trip](../crates/workbench-client/src/workbench_client/ui/tests/modal_pointer_roundtrip.rs)                                                                                                                                                  |

The modal composition owns pure geometry, frame-local key labels and cursor placement, shared
action and close painting, and exact pointer projection and publication. Each feature renderer owns
its localized content, typed token and `HitTarget`, availability reason, presentation phase, effect,
and draw receipt. The shared module never imports a feature-specific state or token.

Form and confirmation `ModalAction` values supply their effective bindings to `ActionSpec`'s
optional shortcut slot. It measures and paints the subdued key inside the button brackets. Dialog
footers carry only their right-aligned buttons; editing keys such as Backspace and Ctrl+U are
conventions and never get a hint row, and only pickers keep a hint row. Settings adopts the same
rule for Close, Back and its child decisions. The complete `[x]` paint also registers its exact
typed dismissal target for backdrop gestures.

The modal's `paint_modal_frame` paints the task title once on the frame's top edge, beside the
frame-borne close control described by [frame
chrome](../../../docs/tui/overlays-and-status.md#frame-chrome). Its `title_line`, `description_line`
and `status_line` project the [overlay
hierarchy](../../../docs/tui/overlays-and-status.md#content-hierarchy) inside the body: bold section
headings, secondary context and field help, primary labels and values, and status color confined to
its glyph. A single-field rename has no label or context line; multi-field forms keep a label above
each field and separate their field pairs with one blank row. Confirmations retain primary target and consequence text. Settings
keeps its title on the frame, with a bold category and secondary position inside. Existing shared
row and action controls continue to own selection, availability, hover, press and focus cues.

Dialogs resolve through `tui_render::modal::dialog`: a `Dialog` lists semantic rows in the fixed
order of [dialog anatomy](../../../docs/tui/overlays-and-status.md), and `dialog_layout` sizes it by
[dialog size](../../../docs/tui/layout.md#dialog-size) (narrow 40–56 and medium 48–72 content
columns, outer width = content + 6) and centers it in the whole window. A command that is the only
recovery may widen it up to 88 columns. Each sentence starts on its own line and wraps to at most
two lines. Small windows follow [dialogs in small
windows](../../../docs/tui/layout.md#dialogs-in-small-windows): the margin goes first, then width
and height degrade separately, and a dialog below its minimum is refused with a status notice.
`paint_dialog` paints the frame, title, `[x]`, rows and footer and records every hit target;
owners paint field values with `paint_field`, which uses the `surface-field` band where the theme
has one and brackets otherwise. Settings remains a wide reference surface.
The [modal witnesses](../crates/workbench-client/src/tui_render/modal/tests.rs) cover centering, class width, small-window degradation and field bands, while the
[composed popup witness](../crates/workbench-client/src/workbench_client/ui/tests/modal_pointer_roundtrip.rs) covers the
Project-name flow, localized hierarchy, field cursor, fault/disabled treatment and suspended targets.

All modal geometry, including complete hints, actions, fields, and the fixed three-cell close action,
must be admitted before `start_modal`, frame mutation, cursor placement, or hit publication. A refused
surface returns an empty or `NotPresented` receipt with zero frame and hit-map mutation. An admitted
control uses the same exact rectangle for pointer projection, paint, and publication. Existing
feature-local preparation and test seams, including panel close's `render_prepared`, stay with their
feature owners so tests can verify both domain preparation and the shared composition without moving
business behavior into the renderer.

## Viewport tiers

Width and height are classified independently and the lower tier wins.

| Tier     | Width tier (columns) | Height tier (rows) |
| -------- | -------------------- | ------------------ |
| Full     | >= 100               | >= 24              |
| Compact  | 60..=99              | 12..=23            |
| Minimal  | 20..=59              | 5..=11             |
| TooSmall | < 20                 | < 5                |

`ViewportTier::from_size(columns, rows)` in `tui_layout` is the single classifier. Chrome row and
column counts per tier:

| Chrome       | Full                             | Compact                                                                       | Minimal        | TooSmall      |
| ------------ | -------------------------------- | ----------------------------------------------------------------------------- | -------------- | ------------- |
| Top strip    | 1 row (tabs)                     | 1 row (tabs)                                                                  | none           | none          |
| Status       | chip, last row                   | chip, last row                                                                | chip, last row | full last row |
| Side panel   | docked, bounded preference       | docked while content keeps 60 columns, else overlay, bounded preference, left | hidden         | hidden        |
| Panel chrome | split/zoom frame and leaf titles | split/zoom frame and leaf titles                                              | not drawn      | not drawn     |
| Content      | remainder                        | remainder                                                                     | remainder      | notice only   |

At TooSmall the Client paints one localized too-small notice and the status line, nothing else, and
resumes normal rendering when the host grows. The sidebar surface the view requests is an intent;
`project_shell_areas` translates it per tier (`Docked` and `Overlay` both mean visible: docked at
Full; at Compact a docked intent stays docked while the content keeps the 60-column Compact minimum
beside the effective sidebar, and becomes an overlay otherwise; hidden below), so the rendered chrome
and the wire geometry agree without the caller knowing the tier. Hosts at least as wide as the
default sidebar plus that minimum (90 columns) are wide and show the sidebar by default; narrower
hosts open it only as an explicitly requested overlay.

When directory input is supported, the sidebar fixes two action rows at the top: `[ New workspace ]` and
`[ Search ]` (`[ 新建工作区 ]` and `[ 搜索 ]` in Chinese). The bracketed action extent is
both the visible affordance and the exact pointer target; the unused row remainder stays ordinary
chrome. Hover and press change only the action surface. These rows and the bottom Settings action
remain outside the scrolling navigation tree. Undismissed first-run guidance also reserves space above Settings, leaving at least four navigation rows; it never paints into the terminal. See [native onboarding](native-onboarding.md). Projects in their canonical
display order and their children follow, then the Workspaces outside any Project under a localized
Recents heading (`Recents`; `最近` in Chinese), always last. Headings show no member count; only the
label truncates. Optional Projects group
ordinary and Git-worktree-backed Workspaces as peers. Agents appear directly under
each Workspace, with independent lists expanded by default. Configuration still owns item row height, gap and token styles.

Fallback chrome without projected navigation rows never invents a legacy Workspace row. When
`sidebar_navigation_rows` is empty and no Workspace creation preview exists, the sidebar
omits Session, terminal-status, Business, Projects and Workspaces placeholders. New workspace,
Search and Settings keep their complete existing targets and footer geometry. A creation preview
retains its pending row.
The no-Workspace main content stays blank, without a Terminal title, onboarding statement or repeated
creation hint. Fixed actions and effective keyboard routes remain available. Global actions retain
`Ctrl+B F2` at every presentable tier, with no permanent Actions button. An empty canonical Workspace
keeps its localized directory and Tab creation guidance. The first workbench row remains available
to authored guidance or terminal content; no PTY geometry is reserved.

Fixed-action shortcut hints end at the sidebar divider, with at least two blank cells after the
button. A hint that cannot fit in full is omitted; its cells are outside
the button's pointer target. The Settings action sits alone on the sidebar's last row without a shortcut
hint, and at every size the list runs directly down to it with no divider, hint or blank row between
them. The prefix status teaches the sidebar-focus binding as `Tab` sidebar; while sidebar navigation
owns the keyboard, the floating status shows its Back and Help bindings. The default focus switch
is `Ctrl+B Tab`, with the previous
`Ctrl+B Shift+B` alias retained. Repeating the focus switch returns to the terminal without activating
a Workspace. Blank sidebar cells do not change focus; the explicit binding and row controls own it.

Hosts shorter than 24 rows omit the spacer after the actions. The unified window fits
complete rows where possible and clips an oversized row to the available body. Keyboard
focus, or the visible current Agent with its Context as fallback, determines focus reveal.
The list has no heading: Project and Recents headings name its groups. Paint and pointer
regions use the same final rectangles; tree disclosure never changes terminal content geometry.

A blank row normally separates the fixed actions from the first group. When canonical rows
are available, the list omits repeated application branding and ordinary session status.
An empty navigation tree leaves the space below the actions blank until rows or a creation preview arrive.
Both sidebar presentations keep one top padding row and one right padding cell, with no
outer left padding. Row surfaces, the footer
divider and fixed-action shortcut hints continue through the right padding cell to the sidebar
divider; pointer targets keep the list's own columns. Only an overflowing list reserves its last
row for the range; a list that fits reaches the row directly above Settings.

The tree shape comes from shared columns. Project, Recents and Workspace rows keep a one-cell
navigation marker at the list's left edge, followed at the next column by their identity: headings
use a regular-weight muted label and Workspaces a primary-text name. Workspace rows have no
leading state column, so their names align with the group headings, and their surface starts at
the list's left edge. Agent rows place a one-cell vertical guide (`│`; ASCII `|`) under the first
cell of the Workspace name, then a one-cell chrome gap, one cell of leading row padding, their
state glyph and a navigation-marker cell, so Agent titles start six cells from the edge. The guide
spans all Agent lines, including configured continuations and gaps between siblings, and ends with
the last child. It has no horizontal branches or corners and never crosses the Workspace's second
line or the gap before its first Agent. Guides use the low-contrast `frame-guide` role on chrome:
an Agent row's surface starts one cell before its state glyph, so hover, press, navigation and
current fills include that blank padding while leaving the guide and its adjacent chrome gap
unfilled. Eligible disclosures follow the displayed first-line identity.
The unread marker is the only trailing marker: a small `•` in `status-unread` (ASCII `*`), shared
with the Tab and pane title cues, in the row's last list cell after one blank cell. A Workspace shows
its Agents' unread marker only while they are folded; open Agent rows carry their own. An Agent title
leaves those two cells only while the Agent is unread. As in common Git tools, the ahead count
(commits to push) uses `status-success` and the behind count (commits to pull) `status-warning`;
their `↑`/`↓` arrows keep them apart where no color shows, as in Monochrome.
On the identity line they end at the row's trailing edge, where the close control appears, or
before the unread marker's two cells; on other lines they right-align at the end of the primary
track. The default layout places them on the identity line and shows the Workspace location on the
second line: its Git branch, or without one its folder's name. A location never shows a whole
directory: the folder reads as its last name, such as `Desktop`, a local home folder itself as `~`,
and only a drive or file-system root keeps its own spelling. The name repeats even when it equals
the Workspace name, so every row keeps its height when Git settles. The folder is the first
terminal's current directory, otherwise the Workspace's own directory; see
[automatic Workspace names](native-location-labels.md#automatic-workspace-names). The branch is
that directory's, so a Workspace whose first terminal stands in a folder outside any repository,
such as the folder holding its Worktree checkouts, names that folder even when its own checkout is
a repository; a Git read that fails or has not answered also leaves the folder.
Continuation lines, the location included, use the whole primary track whether or not the identity
line reserves a disclosure. A branch and a folder's name truncate at the end.
A configured continuation line appears only once one of its values is visible, so a context with
neither branch nor folder shows one line and pending metadata never reserves a blank row. Branch
and custom continuation rows align with the Workspace name. A configured `state_icon` paints nothing
on a Workspace row. Project and Recents labels use regular-weight muted text, quieter than the metadata under them,
and secondary text on a hover, press, current or related fill; Workspace names use primary text
and descriptive metadata uses secondary text.
Each Workspace with Agents folds them away on its own, independent of its siblings, and starts
expanded. While folded, its second line becomes a status line in place of the configured second
line: the states of its Agents from the complete physical Agent checkpoint, most urgent first
(Blocked, Error, Done, Working, Idle, Unknown), up to five glyphs one cell apart, or four glyphs
followed by a secondary-text `+N` remainder. A one-line configured layout gains that second line
while folded; configured lines after the second remain. Eligible Project
create controls are revealed with the row's other controls, right-aligned, with no overflow-menu slot after them. At 52 columns
and above they use a localized Create action; narrower sidebars use the fixed three-cell `[+]` icon
action. Project and Workspace rows never paint or publish an ellipsis button, including during
hover or keyboard navigation. Secondary click and the existing keyboard menu route open their exact
context menu. Ineligible create controls reserve no space and publish no hit regions.
Current-site Workspace rows reveal a three-cell `[x]` close icon action flush with the row's
trailing edge on the identity line while the row or one of its controls is hovered or pressed. It
replaces that line's Git counts and unread marker. The identity always leaves four cells for it, so
it truncates the same at rest and under the pointer, and nothing shifts. It uses secondary
text on the row surface, turns `accent-destructive` on its own hover or press, and requests the
existing Close Workspace confirmation. Remote-machine, other-site and Agent rows never
reveal it, and the context menu and keyboard close routes remain available.
Hover or press on a disclosure, Create or close control also keeps its complete parent row hovered; the
control retains its own local pressed feedback. Internal row padding and external group separators
keep their existing owners. Agent rows carry no ordinal. The default Agent row is one primary
title line, falling back to its provider when no meaningful title is available; its state glyph
carries state, which a pointer resting on that glyph names. Explicitly configured Agent rows retain their selected tokens
and line count.
Project and Recents groups reserve at least one blank row
after preceding items; configured gaps can widen it. A Project's first Workspace sits directly
under its heading. The default Workspace gap is zero rows. These external separators retain the chrome
background and an inert list-surface target; hover, press, current and navigation surfaces never
include them. Only the gap between Agent siblings stays inside the following Agent row so its trunk continues.
Current Workspace entries retain their exact targets and neutral full-width `surface-current`
while typing. A visible active Agent shares that same fill, so the current Workspace and its current
Agent read as one current group; the Agent's tree lead stays on chrome. When that Agent is folded
away, the Workspace keeps the current fill and repeats the hidden Agents' unread marker. A
noncurrent Workspace receives the quieter `surface-related` background while one of its Agents
owns provisional keyboard navigation, so that tint never reads as current. Direct press, navigation and hover feedback on the Workspace takes precedence over its
current and related backgrounds. This presentation never activates or focuses a second target.
Unknown or reduced-color hosts, including `terminal-native` and Monochrome, leave the related fill
empty; the guide then connects the navigated Agent to its Workspace. Project and Recents headings are
never current or related, even while folded around the current Workspace. A pointer folds them only
through the disclosure glyph; a click elsewhere on the heading does nothing. Their row takes no
hover or press fill, because their first Workspace sits directly beneath them: pointer presence
on the heading raises the disclosure glyph to `text-primary` instead, and the glyph itself never
takes a hover or press fill. A Workspace with Agents keeps its
whole-row hover and click; pointer presence on that row also raises its disclosure glyph to
`text-primary`, as it does on headings. Keyboard navigation keeps its
selection surface, and Enter still toggles the focused heading.
The same continuous rectangle includes the name, branch, configured metadata and
identity continuations, including their gutters and trailing space. It stops at the entry's own
last line, excluding separators, group headings and neighboring entries.
Keyboard focus uses the theme navigation marker in the separate track and `surface-selection` across
the focused entry. Named palettes retain secondary metadata colors; host-relative text uses the
surface's paired foreground. Explicit token colors and emphasis retain their authority. The fill
alone marks the current row in the named themes: current names keep their normal weight and no
marker is added. Terminal-native also underlines the complete current row over its ANSI gray fill.
Where no fill can show, because quantization collapses it onto chrome or the host is Monochrome, the
current row is underlined instead. While keyboard navigation or the pointer is on the current row, its
selection or hover surface replaces the current fill until it leaves; Agent state and unread glyphs
remain in their own cells. Agent continuations retain the same whole-row surface as their identity.
Project, Recents and eligible Context disclosure use one glyph (`▾`/`▸`, with `v`/`>` ASCII fallbacks) after
one blank cell following the displayed first-line identity. Text measurement and painting reserve
these two cells on the identity line before truncation, using display width for wide glyphs; other
lines never reserve them. Identity starts stay fixed.
Disclosure always shows on Project, Recents and Workspace-with-Agents rows, expanded or folded, so
every foldable row reads as one at rest. Create appears only while the row or one of its controls is
hovered or pressed, or the row has keyboard focus. A resting row publishes no target for a hidden
control. Each revealed child is
recorded before its row, and pointer projection uses that same exposure so hover and press
survive the reveal frame. Only the painted glyph cell publishes a disclosure target, excluding
leading gaps and continuations.
If the identity area cannot fit text, a separator and the glyph, disclosure is omitted and its
existing keyboard route remains available.
Project and Workspace primary rows support primary-button dragging; disclosure and Create remain
independent controls. The Recents heading is neither a drag source nor a drop destination. Movement of two columns or one row latches a drag, even if the pointer returns
to its origin. Dropping a Workspace on a Project appends it to that Project. Dropping on a Workspace
entry, its Agent rows included, places it before that entry from the entry's upper half and after it
from the lower half; on a one-line entry the side away from the dragged row decides. Grouping follows
the destination Workspace. Projects reorder on other Project headers as complete groups, by the same
one-line rule. A bottom footer drop area appears only during dragging: No Project moves a Workspace
out of its group; End of Projects appends a Project. One underlined line on the gap the drop fills,
the destination's last line for after or the line above it for before, and the localized status
describe the actual before/after/into result. Invalid and unchanged destinations publish no move. Holding at the
list edge scrolls at most once every 120 ms through the existing bounded list-scroll owner; wheel remains local.

Until movement latches a drag, the press is a click on its row: only that row's painted target, the
Sidebar catalog and surface decide its release, so a background refresh or another Client's unrelated
commit and repaint never cancel it. A press read together with the motion onto the row lands before
the frame that reveals the row's close control; that frame recognizes the pressed row and its
pointer reveal keeps the click, as [mouse input](native-mouse-input.md#region-map) describes, while
a release on the revealed control resolves to the control and activates nothing. A drag latches
against the canonical source painted at that moment.
Dragging is preview-only until matching plain primary release. Escape, loss of focus/capture,
resize, an occluding overlay, or changed canonical source cancels it; a refresh still in flight does
not, and the drop rechecks the source. Cleanup consumes the late
release without selecting another row or entering a terminal. Successful scroll redraws preserve
the gesture while retaining the same source identities. Preview geometry resolves from the candidate
frame being painted, so a scroll cannot display a stale insertion target. One `navigation.configure` changes
membership/order through the existing serialized setup and known-receipt recovery lane; current
terminal and keyboard focus remain unchanged. No operation is automatically rebased or resubmitted.

An admitted drop immediately displays its reversible order through the active Client navigation
projection. The setup lane still owns canonical confirmation, refusal, unknown outcomes and recovery;
the independent metadata/Git polling interval never delays this local feedback or confirmed handoff.

In sidebar keyboard navigation, Ctrl+Up and Ctrl+Down move the focused Project, or move a Workspace
within its current Project or Recents list. The existing row-menu binding retains grouping commands
and shares the same stable target proof as secondary click. These local bindings appear in sidebar
help from the binding registry; the gesture does not add a separate shortcut system.

At Full and Compact tiers, the top strip contains Tabs, their close and creation controls,
and a sidebar restore control when needed. It rests on `surface-strip`, one step lighter than the
terminal in named themes and derived from the observed host colors under System, so a bare pane
below it stays distinct. Each Tab leads with the state glyph
of the Agent the current machine observation places in it and follows with an unread dot; a
collapsed group carries its members' most urgent state. While the sidebar is hidden, the attached
machine alone stays observed for this purpose. Summaries stay in the sidebar and Agent Board.
Global actions are available through `Ctrl+B F2` and configured menu bindings. Every platform
projects the same canonical navigation rows; the retired context breadcrumb is not a separate
platform layout.
Active Tabs use padding, `surface-current` and bold identity without a marker; checked Tabs use
`surface-selection` at the same width.
Compact Tabs show their labels without ordinal prefixes; navigation lists retain keyboard positions.
In split view the shared outer frame has no duplicate generic title. Its base
style is neutral; the existing boundary segments around the focused leaf use the
focused frame role, alongside that leaf's accented title. No extra border cells
are added around leaves and divider rectangles stay unchanged. Divider
ends update the existing outer boundary's junction glyphs, leaving no half-cell
gap and preserving the frame's corner style. A hovered or dragged divider takes the bold focused
frame role without filling the canvas. A single pane is bare: no frame and no title, so its terminal
fills the panel and moves inward when the first split appears. Zoom keeps the outer frame and titles
the zoomed pane on its top line, with a localized badge naming how many panes the zoom hides. Each pane
reserves one right chrome column for its [history scrollbar](native-terminal-viewport.md#scrollbar-interaction).
The column stays reserved when history is empty or pending, so output never changes PTY width.
It is disjoint from the content, title and split-divider hit regions. A one-column pane omits
the gutter; a track shorter than two rows has no thumb. Split leaves and zoom use the same
content-width deduction in native geometry, drawing, input coordinates and layout queries.
Leaf titles sit on the frame or divider line above each leaf, after one rule cell, so they take no
terminal row. They prefer custom metadata, then the stable creation label. Numeric creation labels
use the localized Terminal prefix and never a current layout ordinal. A reported Agent's state glyph
leads the title and an unread dot follows it. The focused title uses the bold focus accent without a
marker; its complete perimeter is emphasized after divider glyphs are painted, including shared
edges, and every title paints after that emphasis. Sidebar navigation suppresses terminal focus cues.
Multiple-Presentation arrows and their count end the leaf's title line in split and zoomed views; a
bare pane's selector ends the Tab strip instead. Selectors on a divider line take pointer precedence
over that divider; other title cells keep the divider's drag target. A pointer resting on a title's
text opens a [pointer tooltip](../../../docs/tui/overlays-and-status.md#pointer-tooltips) that gives
a cut name in full, then names its state glyph and unread dot. The title records no target for it,
so those cells still drag the divider.
The renderer, PTY dimensions, popups and read-only layout queries share
this geometry; zoomed hidden leaves retain their canonical unzoomed dimensions.
Inactive terminal content retains normal text contrast; focus is expressed by
pane chrome and the cursor, without applying DIM to another program's content.
The shell paints the terminal canvas first, then persistent chrome backgrounds
for the sidebar and the Tab strip's own surface. Overlays paint their
own complete opaque surface; frames and titles express boundary and focus rather
than supplying the only visible distinction between regions.

The top strip reserves its space for terminal navigation. Copy and resize modes replace it with
their controls, or overlay the first workbench row when no strip exists.
These controls reserve no additional terminal geometry; painted control cells take pointer
precedence over the content beneath them. Their behavior belongs to the owning action, copy
and resize references.

When help cannot fit a complete page in a presentable tier, the status chip shows the
localized resize notice with a complete three-cell `[x]` button. This button can remain visible
in Minimal and dismisses help without changing the underlying mode or PTY geometry. TooSmall
paints only the root size notice and status; it retains the help episode without publishing
hidden controls. A restored presentable frame can resume the retained help page.

## Quick creation controls

The top strip reserves three cells for `+` after the last visible Tab,
including an empty canonical Workspace. The current Tab and create control share the strip;
Presentation selectors belong to pane title lines, or to the strip's end for a bare pane. Controls
disappear with the strip below Compact.
The fixed New workspace row immediately creates a Workspace through the existing quick action;
an eligible current-site Project row has a create action beside its row menu whenever the painted
width admits a separate control. Current-site rows belong to the attached Client's Session, which
the Client's own topology identifies even while no Workspace is open, so a Project left empty keeps
its menu and create action. Disclosure, content, create and menu cells are disjoint. Compact
glyphs are locale-neutral; full action labels and unavailable reasons use
the existing English and Chinese catalogs.

| Entry             | Existing action and authority                                                                                                                                                                                                                                                                                                                                |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Tab-strip `+`     | `DrawnTabCreate` freezes the Session, snapshot revision and context. An unmodified left press arms a local pointer gesture; matching release inside the same still-exposed rectangle asks `WorkbenchClientUi` to revalidate that proof, then hands `ClientAction::CreateTab` to the ordinary dispatcher and membership admission. No effect occurs on press. |
| New workspace row | An unmodified left press arms the exact painted action; a matching release inside the still-exposed extent delegates `ClientAction::RequestQuickWorkspace` to the existing creation owner. It submits one independent Workspace and first shell without opening a form. No effect occurs on press.                                                           |
| Project-row `+`   | Opens the same creation subset over the exact painted Project, with targeted quick Workspace first. Existing capability checks and disabled reasons remain; no child Workspace is selected implicitly.                                                                                                                                                       |

Right-clicking the Tab-strip `+` lists the saved launch profiles when there is more than one; the
global menu's new-Tab submenu also offers the recent group and a new group. The profile menu
retains the same exact `DrawnTabCreate` context proof. See [Tab menus](native-tab-groups.md#pointer-arrangement-and-menu-navigation).

Tab-create gestures cancel on changed identity, geometry, covering capture, released pointer
capture or an outside release. The release packet's remaining bytes, including paste, never
reach a pane even when the proof is refused. Menu entries retain the existing capture and
cancellation lifecycle. The keyboard routes remain: configured New Tab (default `Ctrl+B c`),
Workspace creation (`Ctrl+B a`), quick Workspace (`Ctrl+B Shift+N`) and the global/Project
menus. These controls own no new business state, terminal identity or persistence.

Render and hit-region cases cover both locales and default/RGB themes, empty and populated
lists, every viewport tier and long labels. Pointer traces cover complete clicks and invalidation;
WorkbenchClientUi tests check stale context/snapshot refusal and delegation to the original command/draft
owners. These deterministic tests and composed buffer previews do not establish physical-host
palette or font behavior.

## Sidebar Agent discovery

The unified sidebar joins the existing eligible Agent projection to exact owners.
Only actual Agent rows appear in the tree; loading, unavailable, empty and
no-match placeholders remain in the global Agents picker. The tree footer counts
visible navigation entries, including Project and Context rows, and reports a
range only on overflow. It does not present that count as an Agent total.

A folded Project or Recents heading shows no Agent summary. A folded Workspace's status line
lists the complete physical Agent checkpoint, so waiting evidence stays visible through folding and
Agent filters. Current-Workspace styling remains on the Context and never moves to a folded group
heading; the active terminal does not change.

Workspace rows carry no state or identity glyph. Their Agents' states appear on the Agent rows, or
on the status line while folded.

Agent, Workspace status-line and Tab state glyphs keep completed turns green after focus or reading; only the
independent unread marker clears. Accepted terminal text, paste, deletion, completion or line-editing
input changes the exact observed completion to a Client-local idle presentation. Focus, hover,
scrolling, cursor movement, copy and rejected input do not. The observation, outcome and notification
inputs remain canonical; a newer state or terminal incarnation cannot inherit the edited version.
The Agent Board applies the same presentation override to its matching observed session version.

Sidebar, Tab, pane-title and Agent Board Working glyphs use a stationary solid circle `●`, breathing from
white to gray over two seconds on the shared 100 ms clock; terminal-native Working uses the host
foreground and adopts the current Tab's paired foreground. Idle uses gray `○`; attention, done and
error use the fixed traffic-light `agent-attention` `#FEBC2E`, `agent-done` `#28C840` and
`agent-error` `#FF5F57` `●` in every theme and host, which 16-color hosts show as Yellow, Green
and Red; their state words keep the status text roles. Unknown and the default Workspace identity
use secondary-gray `•`. Attention alternates full and half intensity every 500 ms. The Picker owns
the phase and only advances while observed Agents or the visible Board need animation. Rendering stays pure. ASCII
uses `-\|/` Working frames and distinct `#`, `+`, `x`, `o`, `.` status fallbacks; monochrome
keeps distinguishable outcomes without color breathing. The shared palette is owned by
[`@orchbee/design-tokens`](../../../packages/design-tokens/README.md): Working is neutral and idle
retains its GrokNight color, while the TUI's attention, done and error glyphs use the fixed values
above. Light themes use a dark breathing circle for
contrast, and reduced-color hosts use bright/dim intensity. Truecolor System themes retain smooth
interpolation while their canvas stays host-relative: observed outer-host colors or neutral endpoints
for the theme polarity drive the fade, independently of child-terminal color preferences.

The fixed Search row opens the Workspaces picker with its name query focused. Existing Workspaces
are immediately visible even without Agents; typing filters names, Enter opens the selected exact
target and Esc returns to the terminal. Machine and Agent modes remain available through Tab or
their painted controls; Agent scope and filters retain their existing semantics. See the
[workspace search owner](machines.md#cross-machine-workspace-navigation).
Within explicit sidebar keyboard ownership, `Shift+N` invokes the same
New workspace action and `/` enters the same search editor; F1 lists these bindings. The fixed
actions show those local key labels only while that owner is active, and derive them from
`SIDEBAR_BINDINGS` rather than embedded copy. If the complete key label does not fit, it is omitted.
An exclusive input capture may preserve the Picker's remembered row focus for recovery, but it
suppresses the sidebar focus border, row navigation cue and owner-local labels so the capturing
surface remains the only visible keyboard owner.
The existing global `Ctrl+B Shift+N` creation route remains available. Query editing and
cancellation retain the picker's capture rules, and opening either action consumes the
input packet tail. A visible range footer opens the Workspace picker; a blank footer
is not an action. Workspace browsing and Agent filters remain reachable through the existing
Machine picker modes; this layout does not assign new browse/filter controls to group headings. The fixed bottom Settings entry opens the existing settings editor directly,
matching the GUI sidebar entry. It stays outside the scrolling tree and has no Menu or machine
label. Its complete configured key path appears when it fits, using the binding registry rather
than a hard-coded default. Keyboard users open Settings with the configured action (default `Ctrl+b F4`); the existing comma binding
continues to rename a Tab. Global actions remain on `Ctrl+B F2` and machine selection on
`Ctrl+B Shift+M`. Hidden sidebars publish no sidebar hits. All entry points retain existing
admission and capture ownership.

Rendering consumes trusted picker rows, the shared window, geometry and binding
registry. It performs no observation or backend command. Render and input tests
cover both locales, themes, configured rows, tree connectors, picker/Settings entry
points and the Full/Compact/Minimal/TooSmall matrix. Host interaction requires
separate native evidence.

## Sidebar navigation model

Clicking or pressing Enter on a Workspace activates its exact target directly,
without painting a machine, workspace or Agent picker as a transition. Same-machine
activation reuses the current Session and its per-location selection memory;
cross-machine candidates retain the existing drawable handoff. Current Workspace,
Tabs and content follow the installed Session selection. Pending, superseded and
cancelled requests follow [machine navigation](machines.md#cross-machine-workspace-navigation).

The native sidebar renders Workspaces and optional Project groups from complete Machine
checkpoints and the active Client's canonical snapshot. For each Machine, Projects and their
children come first, followed by a Recents heading over the Workspaces outside any Project.
Grouped ordinary and Git-worktree-backed Workspaces share the same level; every Workspace
owns its Agent children. `machine_spaces::Checkpoint` retains Project IDs and labels from
the same fenced Snapshot as Workspaces, including empty Projects with their own menu and
creation controls. The active Client supplies validated topology on each UI tick, plus at most one
admitted move as a reversible display. The independent observer supplies exact-identity metadata
and Git evidence; a newer canonical revision supersedes older local topology. New topology never
waits for a metadata read. Other Machines retain their observation lifecycle.

`machine_picker::Picker` owns Project and Recents expansion, independent Agent-list collapse
preferences per Context, keyboard row focus and one shared window. `SidebarTarget::Project`
carries Machine/Session/Project identity; `SidebarTarget::Recents` carries Machine/Session
identity and parents that Session's Workspaces outside any Project; `SidebarTarget::Context` carries Machine
and the full `ClientContextTarget`; `SidebarTarget::Agent` also retains exact
member and physical binding identity. Optional Project membership is Context metadata,
resolved by `Picker::sidebar_parent` from the current projection. Moving a Workspace or
dissolving its Project never changes its terminal identity, Tab or Panel target. Agent
joining never uses labels or row positions. Duplicate names are allowed: equal sibling labels
paint alone, with no ID annotation or extra line, and stay separate rows by identity.
Canonical Project order, Workspace order within each group or root list, and global
eligible Agent order remain intact.

The shared window stores its manually positioned top row by identity and old neighbor order.
Insertion, reorder, disclosure, background source updates and resize retain that row within viewport bounds; removal uses
the next surviving old neighbor, then the previous one, then the empty origin. Queued wheel work
is discarded when its geometry changes. A provisional creation slot participates in measured
geometry without becoming an identity or shifting the saved real-row anchor. Explicit navigation
or foreground creation resumes the smallest required focus reveal. Domain selection never follows
ordinary viewport movement.

Each successful frame builds one immutable `SidebarSource` from those existing identities,
parentage and action eligibility, then shares it across the frame's `DrawnSidebarTarget` and
`DrawnSidebarFixedTarget` values.
Labels, status text, theme, keyboard focus, pointer presentation and scroll position are absent from
that source. Ordinary row and disclosure presses arm an exact source, semantic part, target and
painted window; release rebuilds the live source once and invokes an existing Picker transition only
when they still match. Fixed actions arm that same source plus the action-specific effect scope;
New workspace and Browse include the current Machine and Context, while Search and Settings require
no selection scope. Their release likewise rebuilds once and requires the exact source and scope.
A stale release is fenced without an effect. Hover and press never change the current Context, row
focus or selection. Menu, Create, right-click and wheel retain their specialized gesture owners.

| State / event                      | Decision and observable result                                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Initial disclosure / Agent arrival | Each Context's Agent list starts expanded, including newly observed Agents. Multiple lists remain open together. Explicit collapse persists through filtering, Agent absence and arrival, and Project collapse while the Context still exists.                                                                                                                        |
| Context disclosure release         | A matching release toggles only that exact visible Context without activating a terminal or taking keyboard focus. Other lists retain their state. Preserve a manually scrolled viewport, cancel pending wheel geometry and retire the painted hit map and packet tail.                                                                                               |
| Enter navigation                   | An available Picker enters explicit keyboard ownership, including an empty tree. Retain a valid prior row focus, otherwise reveal the active Context, its visible Project parent or the first row. Entry may precede the first paint.                                                                                                                                 |
| Arrow / Home / End                 | Move and reveal only the stable keyboard focus among visible rows; do not activate a Context or Agent.                                                                                                                                                                                                                                                                |
| Right / Left                       | Right opens a folded Project, Recents or eligible Context; on an expanded row it enters the first child immediately. Left moves from an Agent to its Context, folds an expanded Context before moving to its Project or Recents heading, or folds a group heading. Enter on a Project or Recents toggles it.                                                          |
| Enter on Context / Agent           | Return one identity-checked `Activate(Selection)` for the exact visible target, including an Agent's existing Tab and Panel. The existing owner applies it; the current Agent is a no-op and focus movement never writes `Picker.request`.                                                                                                                            |
| Menu / F1                          | Context and Project rows retain their existing menu intent and admission; Recents and Agent rows invent no menu. F1 opens shortcut help while preserving row focus and navigation ownership.                                                                                                                                                                          |
| Source replacement / collapse      | Prune a disclosure preference only when its target disappears. Group and Context folding preserve the active terminal and hidden disclosure preferences; a folded Context takes its hidden current Agent's fill. Retain exact visible focus; otherwise use the nearest visible ancestor, then the bounded prior position. Removed IDs never redirect to equal labels. |
| Regroup / dissolve                 | Resolve an existing Workspace or Agent's ancestors from current Project metadata. Retain its exact focus when visible; if regrouped under a folded Project or Recents, focus that heading. For a removed identity, recover through its last known surviving ancestors before the prior position.                                                                      |
| Wheel / key reveal                 | Workspace and Agent hits use the same coalesced, viewport-capped wheel window. Keyboard movement resumes focus reveal; wheel movement does not activate a target.                                                                                                                                                                                                     |
| Escape / hidden sidebar            | Leave keyboard ownership and consume its packet tail. Hiding the last observation consumer releases source rows and obsolete disclosure; ordinary terminal input retains its existing owner.                                                                                                                                                                          |

Agent vertical guides occupy a fixed one-cell track under the Workspace name. Configured
continuation rows align with the Workspace name. Secondary rows use metadata text and retain
explicit token overrides. Current Contexts and Agents use the same neutral current surface, including a
current Workspace whose current Agent is visible. The fill alone marks current
identity in the named themes; terminal-native also underlines it, and so does any collapsed palette
or Monochrome that leaves no fill. Agent state, unread state and keyboard navigation remain in
separate cells.

Project, Recents and Context disclosure glyphs, identity text and row menus have disjoint painted extents.
The primary row target covers every continuation, internal configured leading space and all cells
beneath frontmost disclosure, Create and revealed close targets. External group gaps remain inert. `SIDEBAR_BINDINGS` owns key matching and localized discovery. The
dispatcher owns capture precedence, stale-paint rejection and packet-tail fencing;
the menu owner retains capability checks and execution. Project menus never choose
a child Workspace. Mouse and keyboard disclosure share Picker transitions; navigation
keys apply only during explicit sidebar input ownership. GUI correspondence covers hierarchy and identity semantics,
without claiming pixel or backend equivalence.

| Claim                                     | Executable evidence and limits                                                                                                                                                                                                                   |
| ----------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Exact grouping and no implicit activation | Deterministic Picker cases cover Machine/Session/Context joins, duplicate labels, Recents grouping and folding, preserved legacy content, multiple default-expanded lists, retained collapse preferences and exact Tab/Panel activation.         |
| Valid focus and scroll                    | Bounded generated navigation traces cover topology and grouping changes; focused cases cover regroup/dissolve, removed ancestors, Agent-parent movement, current-Agent reveal and manual-scroll disclosure. No unbounded proof is claimed.       |
| Shared geometry and input authority       | Render, pointer and dispatcher cases cover configured heights/gaps, both locales/themes, wide glyphs, folded status lines and current evidence, revealed Project controls, stale hits, wheel cancellation and disclosure without focus transfer. |

These synchronous presentation transitions add no asynchronous effect or terminal
acknowledgement. Existing sidebar-input, menu and pointer owners and their deterministic
tests retain independent completion, capture and cancellation ordering. New tree projection
and disclosure semantics require the deterministic evidence above in addition to those focused checks.

## Sidebar width and restoration model

`tui_layout::SidebarWidth` admits 24 through 60 columns, defaulting to 30. This is an
explicit Client-local preference owned by Picker, not a content-driven size or a
persisted setting. A visible sidebar uses `min(preferred, host columns / 2)` columns;
the existing Full and Compact floors ensure that this cap remains above the minimum.
`SidebarSurface` carries the preferred value through the existing single shell
projector, terminal geometry and local pane-layout validation. The status chip floats within the columns to the right of a visible sidebar. Host shrinkage clamps only the effective rectangle, retaining
the preference for later restoration.

| State / event            | Decision and invariant                                                                                                                                                                                                                                                                                                                                                                     |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Full / Compact / Minimal | Keep existing tier and visibility intent rules: Full docks, Compact docks while content keeps 60 columns and otherwise shows an explicitly requested overlay, lower tiers hide the sidebar. Width does not automatically open an overlay or change wide-window visibility intent.                                                                                                          |
| Keyboard width           | While sidebar navigation owns input, exact Shift+Left and Shift+Right emit a two-column narrower/wider intent; `0` emits reset to 30. The shared binding registry also supplies localized help. Require an exact current painted sidebar target even when mouse capture is released. No target means no mutation.                                                                          |
| Adjustment at a limit    | Start from the effective width. If the clamped result equals that width, preserve the original preferred value; reset explicitly writes the default.                                                                                                                                                                                                                                       |
| Pointer press / motion   | A plain left press on the existing one-column right border captures its exact viewport, full preferred surface and effective sidebar rectangle. Absolute displacement produces a reversible live width: rows, border hits and docked terminal geometry move together. Width preference, row focus, expansion and navigation requests remain unchanged while held.                          |
| Matching left release    | Revalidate the painted target and compare the original preferred width with the current preference. Commit the changed candidate once through Picker, retire old hit geometry and run the existing layout boundary. Equal effective rectangles cannot authorize a stale preferred value.                                                                                                   |
| Cancel / stale source    | Escape, Shift, host resize, hidden sidebar, covering capture or loss of the painted target discards the preview. Zero displacement and clamped no-op releases preserve the original preference. The input owner retains release and packet retirement.                                                                                                                                     |
| Width commit             | Clear queued sidebar wheel work and resume height-aware focus reveal. Preserve keyboard ownership, expansion, active Context and request. The accepted preference takes over the already projected width; subsequent frames keep labels, row heights, hits and terminal geometry consistent.                                                                                               |
| Hidden restoration       | When Full or Compact can present the sidebar and no covering capture owns the surface, a localized top-strip entry invokes the existing `FocusBusinessSidebar` action. It shares the configured navigation entry route; the existing visibility toggle and global Menu remain available. Minimal and TooSmall keep their existing reduced hints and refusal behavior until the host grows. |

The border hit uses the sidebar control layer, above only its own surface and below
covering modals. Row, menu and border rectangles are disjoint. A read-only local
`ResizeProjection` supplies the same effective width to paint, hit mapping and
terminal-size observation. The pointer keeps the immutable preferred source and
last successful draft-frame proof separately; its own layout changes preserve the
drag, while external owner/viewport/source changes cancel it. Compact overlay width
changes leave the underlying terminal geometry unchanged. Graphics use the existing
cover and rejected-draft repaint lifecycle. Only the actual one-column border uses
the focus accent foreground; neighboring cells render normally at their new geometry. Press, motion, release and
cancellation never add width values or drag instructions to the status area or an overlay.

The pure width model covers admitted bounds, half-host clamping, offset viewports,
absolute motion, no-op preference retention and 120 → 60 → 50 → 120 restoration.
Picker cases cover compare-and-set rejection, focus preservation and collapsed active
Workspace entry without disclosure. Sidebar renders cover widths 24/30/60, both
locales, tier degradation and exact border hits. Shared input evidence under the
native mouse-input authority owns capture, stale draws, cancellation, matching release,
two held moves across rendered frames, graphics cleanup and terminal-geometry agreement; pure geometry alone does not prove
those effects. The existing bounded hierarchy traces remain the hierarchy authority.

## Status line

The status reserves no row. At every presentable tier the workbench, `ShellAreas.panel` and all
terminal, pane, scrollbar and read-only layout projections reach the host's last row.
`ShellAreas.status` names that row to the right of a visible sidebar, and `status_chip_area` insets
it by one cell on each side. The status paints after every overlay as an end-aligned chip within that
area and records only its painted cells as an occluding `HitLayer::Status` surface. Panes return
their cursor instead of placing it, and the shell places it after the status, so a chip hides only a
cursor it covers. The rest of the row stays terminal content. Opening, changing and closing the chip
never resize a PTY, and closing repaints the covered cells. At TooSmall the whole last row carries
the exit hint.

Nothing floats at rest. Prefix hints appear only after the effective prefix is pressed and leave when
the prefix ends. The chip otherwise appears for other input modes, sidebar navigation, meaningful
notices, connection or recovery,
released pointer capture and persistent states. Directory missing, Disconnected and Terminal
exited show one or two words with their severity marker, followed by the fixing binding when it
fits, such as `Ctrl+B c` to create a Tab for an ended terminal. Every retired connection is
Disconnected, including one whose last operation has an unknown outcome; the pane keeps that full
fact. An exit that does not fit in two words uses the localized compact exit copy; the full exit
result remains with `Locale::native_status` and the pane title. A transient notice leads the exact primary fact and carries no hints; a persistent
state yields to it. Ready and zoom context is available with prefix hints; ordinary controlling status
is omitted, while read-only input remains explicit.
Segments follow `docs/tui/overlays-and-status.md`. Prefix hints start with cancel, help and
`Tab` sidebar; trailing hints drop as whole records. The prefix badge shortens its cancellation hint
to `Esc`, then to the badge alone. Sidebar navigation keeps Back and Help; its Row menu hint yields
first, then Open. One owner paints the status cells per frame: recovery, copy mode, the help refusal,
menus and Settings replace the ordinary chip instead of stacking a second one, and copy mode keeps
its full status row while open.
Runtime producers provide localized facts without embedded shortcut text. The footer appends
an ordered set of complete `KeyHint` controls from the enabled effective dispatcher bindings
or the active local input owner's canonical bindings, using the resolved theme's glyph policy.
It does not rewrite embedded defaults or substitute fallback chords; a hint that does not fit
is omitted as a whole. A recoverable Close status publishes its draw receipt only after the
entire primary fact has been painted.
At TooSmall, a captured independent popup offers only localized resize recovery. Its program
input semantics remain unchanged, but the suspended surface consumes Esc, prefix, paste and
pointer input without forwarding them. Once a usable surface returns, input resumes for the
same popup program and identity.
Covered cells do not forward pointer input or attest unobscured terminal graphics.
The status text area is reported only when completely drawn, preserving the recovery
confirmation gate. The sidebar's Settings action remains outside this rectangle.

## Content area and overlays

An unconnected workbench paints its localized connection guidance and header in the area still
visible beside an overlay sidebar. This authored guidance uses a separate paint rectangle;
canonical shell areas and terminal geometry remain unchanged. While an overlay sidebar is shown,
the panes beneath it are covered: their cursor, graphics and attention cues stay hidden.

The [Workbench navigator](native-context-selection.md#capturing-navigator)
uses the existing centered surface with two labeled columns and a window around local focus.
The active marker continues to identify the selected terminal; the selection background shows
the row Enter will open. Complete visible rows publish exact proofs with the successful frame.
Esc/Enter instructions precede optional toggle and movement details; source cases cover both
locales, configured prefix labels, long scrolled columns, 120x41, 120x40 and 80x24. At less than
40x7 inner cells it refuses presentation and releases capture through the fresh-frame fence.
The navigator suppresses pane cursor/focus emphasis. Prefix input also hides the terminal cursor. Shortcut Help
retains its nonmodal behavior.

Dialogs, reference surfaces (Help, the navigator, the Agent board and Application Settings) and
anchored context menus use the full host viewport, including the sidebar, top strip and last row in
their reference; the status chip paints above them. Sidebar visibility and width never shift these
surfaces, and their placement leaves terminal geometry unchanged. Help, the navigator and the Agent
board place through `tui_layout::place_overlay(area, preferred_width, preferred_height) ->
Option<Rect>`, which resolves to `min(preferred, area - 4)` by `min(preferred, area - 2)`, centered
with the extra cell rounding toward the top-left, and returns `None` when the resolved inner area
would be below 4x3.

A refused overlay is not drawn; the caller reports through the status line instead. Because the
status chip is drawn after every overlay and modal, and hit testing follows the same order, the
never-covered invariant holds even while older surface renderers still compute their own geometry.

## Split panes

`divide_panes(area, orientation)` returns `PaneDivision { first, divider, second }` where `divider`
is exactly one shared cell column or row between the children, or `None` when either child would be
smaller than the documented 4x3 minimum. The old 50/50 intent is preserved: the divider cell is
taken out of the second child's share, so `first` keeps the width a 50% split used to give it.

A leaf draws no edge borders: edges on the region boundary are served by the region frame and edges
facing a divider are served by the divider cell. Each leaf titles itself on the frame or divider
line above its rectangle; the active leaf's title carries focus emphasis while the pane owns input,
and the whole leaf receives the terminal cells. An inactive leaf paints a localized empty state naming the retained terminal
and the focus binding — it never paints fabricated live content. Divider cells are collected during
the walk and drawn once with junction glyphs (`├ ┤ ┬ ┴ ┼`) resolved from neighboring divider cells.

Under a Client-local zoom (`Ctrl+B z`; the session contract lives in `workbench-client-runtime.md`),
the drawn set narrows to the focused leaf: it renders across the whole content area and records
the only pane hit region, while undrawn leaves still report their canonical leaf rectangles so
their viewers stay alive. The focused leaf reports the whole content area to the wire so its
terminal converges to the zoomed size; divider cells are not drawn while zoomed. This split
between the drawn/hit set and the observed set is the only zoom geometry effect — the canonical
projection itself is unchanged.

Capturing surfaces replace the pane's focus emphasis with the unfocused frame and secondary title
styles. Reference overlays preserve pane focus and use an unfocused frame. Sidebar section headings
use primary bold text rather than focus color.
The Machine picker and Worktree draft also suppress pane focus and
the terminal cursor while they own input.
The Session status colors only its glyph; its text uses the primary text role.
Ready uses the existing success role, while other phases retain their warning
role. Live and retained terminal cells share the theme-default color projection:
default-marked foreground and background follow the theme, and explicit program
colors remain unchanged.

## Degradation order

Per `docs/tui/layout.md`: drop optional context text, shorten hint labels, drop hint labels while
keeping the mode indicator, undock the side panel, hide the side panel, truncate labels. The tier
table above binds those steps to concrete rows and columns so no surface skips a step.

## Resize

Every frame recomputes the projection from the live host rectangle. A resize to a degenerate
terminal area still yields a nonzero inner projection while the host is at least 4 columns by 3
rows, so the session loop survives small geometry and the too-small screen remains exitable;
keeping the last valid geometry below that floor is the environment slice's concern.

## Correctness evidence

`cargo test -p orchbee` covers: the tier boundary matrix including the width/height lower-of-two
rule, content reaching the last row beneath the status chip, the strip, sidebar presentation per
tier, overlay clamping, centering, and refusal, pane division geometry and the shared divider, plus
TestBackend renders at 120x40, 80x24, 60x12, 50x10, 15x4 and mixed-axis geometries in `en-US` and
`zh-CN` proving the status chip is never covered, the too-small notice is localized, and active/long-label tab tokens stay visible through
display-width truncation.

Whole-screen review follows `docs/tui/verification.md`: the current Workspace remains marked
independently of keyboard focus, ordinary sidebars expose only Settings at the bottom, complete lists omit redundant
ranges, a single pane fills its panel, and split/zoom views share their frame and border titles. Review the default theme together with
both named RGB themes, empty and populated sidebars, navigation, hidden sidebar and split/zoom.
`workbench_client/ui/tests/viewport/chrome.rs` checks last-row content, prefix coverage, pointer
occlusion, a status chip hiding only a cursor it covers, the first content row directly below the Tab strip, stable geometry and exact restoration in both
locales and native/RGB themes. The [empty-workspace witnesses](../crates/workbench-client/src/workbench_client/ui/tests/quick_workspace.rs)
check blank content, retained creation, Search and Settings targets, and absence of the Actions button across tiers.
The [layout query witnesses](../crates/workbench-client/src/workbench_client/ui/client_layout/tests.rs) pin bare, zoomed
and split content geometry. The [report witnesses](../crates/workbench-client/src/workbench_client/ui/tests/panel_reports.rs)
retain exact-scope reports in each leaf; the
[Presentation witnesses](../crates/workbench-client/src/workbench_client/ui/presentations/tests.rs) check the relocated arrows,
inert count and keyboard/pointer selection.
The same viewport/chrome witnesses inject correlated terminal-exit records and check persistent
status and its fix in the floating chip, retained first-row content, unchanged geometry and hidden
ended cursors. Localized copy
checks cover maximum exit/signal values, unknown results and runtime failure in both languages.
These outcomes remain acceptance constraints for later feature changes.

The package-private [completed-frame observer](../tests/support/tui_observer.rs) is shared by the
Linux PTY process tests and the [portable observer test family](../tests/tui_observer.rs). Run
`cargo test -p orchbee --test integration --locked tui_observer::` when changing its frame grammar.
It checks completed versus partial or stale draws at the backend's synchronized-update flush boundary,
exact top-row locations and a single current Tab token, ordered
Tab and Navigator tracks, complete modal borders and close controls, and bracketed editor windows.
This completed-frame claim is limited to healthy renderer output in the controlled Linux fixtures:
terminal restoration can emit the same synchronized-update end, so the observer does not establish
output-failure atomicity or restoration success. Compact-token predicates use fixture labels that
cannot themselves be parsed as another compact token; ambiguous labels fail closed, and this grammar
does not claim support for every valid `WorkbenchLabel`. Keep the observer and both consumers aligned
with the owning renderers; portable predicate tests do not establish Linux PTY execution or physical
terminal appearance.

## Tab metadata annotations

Pinned Tabs use the localized `Pinned` / `固定` annotation in the navigator; the compact
strip separates them from other Tabs with one vertical rule. It paints separate tinted group headers
with a disclosure cell and no member count; expanded members retain their ordinary Tab tokens, and a
collapsed header identifies an active member. The navigator keeps every canonical
Tab available with its group annotation. Text passes the existing sanitizer and cell-width budget;
painted widths define the exact Tab/group pointer regions. Display annotations never become raw
names or automatic ordinals. The [Tab group contract](native-tab-groups.md) owns disclosure,
configuration and cross-host capability coverage.

Pending and canonical Workspace rows reserve the same nonempty configured token rows, including empty metadata tracks. Late Git information fills those tracks without moving neighboring entries. The heading spacer depends only on available height, never metadata readiness. Style rules may hide text without removing its row.

## Sidebar symbol discovery

F1 includes a symbol legend for unread activity, disclosure and all Agent states. A pointer resting on the sidebar opens a [pointer tooltip](../../../docs/tui/overlays-and-status.md#pointer-tooltips) where glyphs alone carry meaning. A row whose painted line cuts its Workspace name, branch, Agent title or group heading gives the first such name in full. A folder's name gives its full directory, cut in the middle if it is still too long, only while the pointer rests on the cells that name it. Glyphs name themselves only under the pointer: an Agent's state glyph names its state, a folded Workspace's status line each distinct state it shows, most urgent first, and an unread dot unread activity. The rest of the row names no glyph. A Workspace's disclosure names Hide Agents or Show Agents. Its `[x]` names Close Workspace…; only the current Workspace's adds the key that closes it, from the binding table. A Project's disclosure offers none, group headings and other rows offer only a cut name, and the list's last row keeps the overflow range. Branch text appears as observed, even when it equals the Workspace name or repeats another Workspace's branch for the same folder; divergence and stale observations remain visible.

## Shortcut discovery scope

Help opens with common tasks and any active mode controls. Tab or the painted scope button expands all actions and the sidebar symbol legend; reopening restores common tasks. A nonempty filter always searches every action. Groups retain their declared order. Within each group, whole-word prefixes match case-insensitively, with title matches before key/alias matches; Chinese text also supports contiguous substring matches. Help names identify their object and use Panel consistently, distinguishing terminals inside a Panel. PageUp/PageDown and the header arrows move one published viewport of rows; wheel and arrow keys move one row. Scope/filter changes revoke the previous draw authority and reset scrolling. The footer explains the header buttons.

The sidebar legend names disclosure glyphs as Expanded and Collapsed. Before its first successful draw, Help uses a bounded eight-row navigation estimate; after drawing, paging uses the published capacity.
