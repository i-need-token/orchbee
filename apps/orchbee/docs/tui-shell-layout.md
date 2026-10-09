# TUI shell layout

## Scope and ownership

The native Client shell divides the host area into a top strip, a content region, an optional side
panel, a pane area with shared boundaries, and a one-row status line. This reference documents the
concrete regions, thresholds, and degradation order that `docs/tui/layout.md` requires the owning
application to declare. It owns the layout projection only: colors, glyphs, and frame style belong
to the theme surface, overlay contents belong to their surface renderers, and terminal cell drawing
belongs to `terminal_cells`.

The projection is pure. `tui_layout::project_shell_areas` computes every region from the current
frame rectangle and the sidebar surface (including its preferred width) the view asks for; `tui_shell` computes the terminal
geometry from the same two functions (`project_shell_areas` then `native_terminal_area`), so the drawn
content area and the wire geometry can never diverge. No render code mutates business state, clocks,
or I/O, and no layout value is cached across frames.

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

| Chrome       | Full                       | Compact                           | Minimal   | TooSmall    |
| ------------ | -------------------------- | --------------------------------- | --------- | ----------- |
| Top strip    | 1 row (tabs)               | 1 row (tabs)                      | none      | none        |
| Status       | 1 row                      | 1 row                             | 1 row     | 1 row       |
| Side panel   | docked, bounded preference | overlay, bounded preference, left | hidden    | hidden      |
| Panel chrome | one title row; split frame | one title row; split frame        | not drawn | not drawn   |
| Content      | remainder                  | remainder                         | remainder | notice only |

At TooSmall the Client paints one localized too-small notice and the status line, nothing else, and
resumes normal rendering when the host grows. The sidebar surface the view requests is an intent;
`project_shell_areas` translates it per tier (`Docked` and `Overlay` both mean visible: docked at
Full, overlay at Compact, hidden below), so the rendered chrome and the wire geometry agree without
the caller knowing the tier.

The Windows sidebar allocates Workspace and Agent sections from their configured
row heights, row gaps and header/context/range rows. A short section
does not retain half the sidebar: the longer list can use that space. Sections
that both fit stay compact. Complete current Workspace and current/first Agent
rows, including observation placeholders, take priority over section spacing;
up to two blank rows separate sections when their rows and chrome fit. Below
six rows the Workspace keeps its existing heading-only degradation. Filters, row identity,
scrolling retain their existing semantics. Pointer regions
are recorded from the same final rectangles as the displayed rows.

When canonical navigation rows are available, the sidebar starts with Projects:
it omits repeated application branding and the ordinary session status already
shown in the bottom row. Before those rows arrive, its existing connection and
business placeholders remain. Both sidebar presentations keep one top padding row.
Section headings separate a primary label, secondary count and right-aligned localized
browse/filter action; the Project count is omitted for zero or one Project. A blank
row separates a heading from its body when the section can fit it. Footer rows carry
only overflow ranges; fully visible lists leave them blank.

Every navigation row reserves a two-cell focus gutter. Root names start after a
two-cell disclosure/status column; Workspace children indent two more cells. Branch
and custom continuation rows align with the Workspace name, including when a configured
first row omits the state icon. Project labels and metadata use secondary text;
Workspace names use primary text. Row menus remain aligned at the right edge.
The current Workspace retains its exact target and neutral full-row `surface-current`
while typing, with a bold name and no asterisk when the fill is distinct. A collapsed current Project
inherits that surface. Keyboard focus uses `>` in the separate gutter; its foreground
is accented except on the current row, where the surface's text remains legible.
Project disclosure uses one glyph (`▾`/`▸`, with `-`/`+` ASCII fallbacks) in its own
two-cell hit region; gutter, content and menu hits remain disjoint from disclosure.
The top strip keeps
the context breadcrumb available when sidebar rows scroll out
of view and uses padded, filled active Tabs without bracket or asterisk decoration.
In split view the shared outer frame has no duplicate generic title. Its base
style is neutral; the existing boundary segments around the focused leaf use the
focused frame role, alongside that leaf's accented title. No extra border cells
are added and pointer, divider and terminal rectangles stay unchanged. Divider
ends update the existing outer boundary's junction glyphs, leaving no half-cell
gap and preserving the frame's corner style. Single panes and zoom use a one-row
title band with no outer borders, reclaiming both edge columns and the bottom row.
Pane titles use the localized Terminal label, preserving custom metadata, ordinals
and presentation identity; an ordinary presentation label equal to its Tab label is
not repeated. Focus is visible in the title, and is suppressed while sidebar navigation
owns input. The renderer, PTY dimensions, popups and read-only layout queries share
this geometry; zoomed hidden leaves retain their canonical unzoomed dimensions.
Inactive terminal content retains normal text contrast; focus is expressed by
pane chrome and the cursor, without applying DIM to another program's content.
The shell paints the terminal canvas first, then persistent chrome backgrounds
for the sidebar, tab/status bars and split-pane title bands. Overlays paint their
own complete opaque surface; frames and titles express boundary and focus rather
than supplying the only visible distinction between regions.

## Sidebar Agent counts and discovery

The sidebar projects the existing Machine picker rows without changing observation,
filtering, selection, activation or wheel ownership. An empty Agent result must not
look like one Agent merely because an observation placeholder occupies a row.

| Input / decision                         | Projection and invariant                                                                                                                                                                                                                                                                                                                                                                                                      |
| ---------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Complete Agent rows                      | The heading counts actual Agent entries after the existing view filters, omitting zero. Footer ranges count those entries only, excluding observation placeholders; complete lists that fit omit their redundant range.                                                                                                                                                                                                       |
| Loading or unavailable source            | Keep its explicit status; omit a zero heading count when no Agent count is known. A nonzero partial count and range retain `+`.                                                                                                                                                                                                                                                                                               |
| No visible Agent in the window           | Show `0/total` when known Agent entries exist outside the window; omit the range when there are no Agent entries. No placeholder receives an Agent ordinal or activation target.                                                                                                                                                                                                                                              |
| Empty result on the only enabled Machine | Use one status row without repeating the Machine label. Multiple enabled Machines retain their labels and independent status rows.                                                                                                                                                                                                                                                                                            |
| Context row                              | Retain the current view/filter summary for populated results and a no-match result; omit it for an empty, loading or unavailable result without Agents.                                                                                                                                                                                                                                                                       |
| Entry points                             | Section headings, their right-aligned browse/filter actions and visible range footers retain the existing picker targets. Blank footer rows are not actions. A fixed bottom entry, localized as `[Menu]` or `[菜单]`, opens the existing global menu for creation and navigation. Its first configured shortcut is secondary and shown whole when it fits. The status bar supplies this entry only when the sidebar does not. |
| Geometry and input                       | Reserve one bottom sidebar row for Menu before allocating sections. Paint and hit regions use the same rectangles; hidden sidebars publish no sidebar entries. The menu and picker retain their existing admission, cancellation and keyboard ownership.                                                                                                                                                                      |

This is a stateless projection of trusted picker rows, their existing window, current
geometry and the binding registry. It owns no new focus, observation, backend command
or persistence. `tui_render::agent_sidebar` evidence covers complete/empty/partial
counts, placeholder-only windows, both locales, configured shortcuts, and the existing
Full/Compact/Minimal/TooSmall render matrix. Existing picker and menu input checks own
activation and capture; rendered entries alone do not prove a host interaction.

## Sidebar navigation model

The native sidebar renders Project → Workspace navigation from the existing complete
Machine checkpoints, retaining a separate Standalone row and Machine/Site identity.
`machine_spaces::Checkpoint` carries Project IDs and labels from the same fenced
Snapshot as its Workspace rows, including Projects with no Workspaces. It adds no
request, backend command or persistent state. Until that checkpoint is available,
the existing context projection supplies its known Projects and Workspaces.

`machine_picker::Picker` alone owns Project expansion, keyboard row focus and the
existing section window. `SidebarTarget::Project` carries Machine, Site and Project
IDs; `SidebarTarget::Context` carries Machine and the full `NativeContextTarget`.
Labels and positions never authorize activation. Equal visible labels retain ID
annotations. Canonical Project order and each Project's canonical Workspace order
are preserved; Standalone precedes Projects for each Machine.

| State / event                     | Decision and observable result                                                                                                                                                                                                                                                                               |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Enter navigation                  | An available Picker enters an explicit keyboard owner, including an empty tree. Retain a valid prior row focus, otherwise focus the visible active Context, its visible Project parent, or the first row. Entry never changes Project expansion. Entry may precede the newly revealed sidebar's first paint. |
| Arrow / Home / End                | Move only the stable row focus within visible rows and reveal it with the existing height-aware window; do not change the active Workspace or enqueue a request.                                                                                                                                             |
| Right / Left                      | Right expands a Project or moves into its first child. Left collapses a Project or moves from its child to the parent. Enter on a Project toggles expansion. None activates a terminal.                                                                                                                      |
| Enter on Context                  | Return one identity-checked `Activate(Selection)` intent. The existing activation owner applies it; focus movement alone never writes `Picker.request`.                                                                                                                                                      |
| Menu                              | Return `OpenMenu(SidebarTarget)` for the focused row. Existing menu admission validates the drawn source, current Machine and Site; opening and cancellation preserve the navigation focus and active Context.                                                                                               |
| F1                                | Return `Help`; the dispatcher shows the existing shortcut-help overlay while retaining navigation ownership and row focus.                                                                                                                                                                                   |
| Source replacement / collapse     | Retain an exact visible target; otherwise choose its visible Project parent, then the neighbor at its previous bounded position. An empty tree has no focused target. Removed IDs never fall back to an equal label.                                                                                         |
| Wheel / key reveal                | Reuse the existing coalesced, viewport-capped wheel model. A later keyboard move resumes focus reveal. Wheel scrolling does not activate a Context.                                                                                                                                                          |
| Escape / effective sidebar hidden | Leave keyboard ownership. Unmatched navigation keys are consumed by the dispatcher; ordinary terminal input stays with its existing owner outside this mode.                                                                                                                                                 |

Agent ordinals occupy a fixed three-cell gutter; configured continuation rows align
with the first row's name after its optional state icon. Secondary rows use metadata
text, with explicit token overrides preserved. Current Agents use the same neutral
context surface and bold identity as Workspaces. A collapsed color palette retains the
theme's current-context text marker independently of token emphasis.

The active Workspace keeps its bold identity and neutral context surface regardless of keyboard
ownership. The focused row uses `>` while sidebar navigation owns input; a focused
non-current row does not acquire the current context's background. Project disclosure, row content
and the visible row menu have disjoint rectangles from the same rendered row.
`SIDEBAR_BINDINGS` is the shared source for matching and localized mode discovery.
The dispatcher owns entry, capture precedence and packet-tail fencing; the menu
owner retains capability checks and execution. A Project menu never chooses a child
Workspace: context-dependent actions require the explicitly active Context to belong
to that Project. GUI correspondence covers hierarchy and identity semantics, not
pixel or backend parity.

| Claim                                        | Executable evidence and limits                                                                                                                                                                                                                                   |
| -------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Stable grouping and no implicit activation   | Picker deterministic cases cover duplicate labels across Machines/Sites/Projects, Standalone, empty Projects, explicit Enter and repeated menu/collapse events.                                                                                                  |
| Focus remains valid through topology changes | Bounded generated Picker traces cover movement, reorder, removal, collapse, hide and re-entry, with deterministic seeds and deletion shrinking. No unbounded proof is claimed.                                                                                   |
| Render and input share identity and geometry | Workspace sidebar render cases cover active versus keyboard focus, long/wide labels, height-aware reveal and disjoint disclosure/menu hits. Shared dispatcher/pointer traces cover capture, stale draws and focus return under the native mouse-input authority. |

These synchronous projection and focus transitions do not own asynchronous effects.
Existing menu and pointer models own independent completion, capture and cancellation
ordering; screenshots supplement their deterministic interaction evidence.

## Sidebar width and restoration model

`tui_layout::SidebarWidth` admits 24 through 60 columns, defaulting to 30. This is an
explicit Client-local preference owned by Picker, not a content-driven size or a
persisted setting. A visible sidebar uses `min(preferred, host columns / 2)` columns;
the existing Full and Compact floors ensure that this cap remains above the minimum.
`SidebarSurface` carries the preferred value through the existing single shell
projector, terminal geometry and local pane-layout validation. The status row keeps
the full host width. Host shrinkage clamps only the effective rectangle, retaining
the preference for later restoration.

| State / event            | Decision and invariant                                                                                                                                                                                                                                                                                                                                                                  |
| ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Full / Compact / Minimal | Keep existing tier and visibility intent rules: Full docks, Compact shows an explicitly requested overlay, lower tiers hide the sidebar. Width does not automatically open an overlay or change wide-window visibility intent.                                                                                                                                                          |
| Keyboard width           | While sidebar navigation owns input, exact Shift+Left and Shift+Right emit a two-column narrower/wider intent; `0` emits reset to 30. The shared binding registry also supplies localized help. Require an exact current painted sidebar target even when mouse capture is released. No target means no mutation.                                                                       |
| Adjustment at a limit    | Start from the effective width. If the clamped result equals that width, preserve the original preferred value; reset explicitly writes the default.                                                                                                                                                                                                                                    |
| Pointer press / motion   | A plain left press on the existing one-column right border captures its exact viewport, full preferred surface and effective sidebar rectangle. Absolute displacement produces a reversible line preview, without changing width preference, terminal geometry, row focus, expansion or navigation requests.                                                                            |
| Matching left release    | Revalidate the painted target and compare the original preferred width with the current preference. Commit the changed candidate once through Picker, retire old hit geometry and run the existing layout boundary. Equal effective rectangles cannot authorize a stale preferred value.                                                                                                |
| Cancel / stale source    | Escape, Shift, host resize, hidden sidebar, covering capture or loss of the painted target discards the preview. Zero displacement and clamped no-op releases preserve the original preference. The input owner retains release and packet retirement.                                                                                                                                  |
| Width commit             | Clear queued sidebar wheel work and resume height-aware focus reveal. Preserve keyboard ownership, expansion, active Context and request. The next frame recomputes labels, row heights, hits and terminal geometry from the same width.                                                                                                                                                |
| Hidden restoration       | When Full or Compact can present the sidebar and no covering capture owns the surface, a localized status entry invokes the existing `FocusBusinessSidebar` action. It shares the configured navigation entry route; the existing visibility toggle and global Menu remain available. Minimal and TooSmall keep their existing reduced hints and refusal behavior until the host grows. |

The border hit uses the sidebar control layer, above only its own surface and below
covering modals. Row, menu and border rectangles are disjoint. Preview rendering uses
the existing divider graphics-cover and rejected-preview repaint lifecycle; it never
introduces a second terminal projection or sends a backend resize during motion.

The pure width model covers admitted bounds, half-host clamping, offset viewports,
absolute motion, no-op preference retention and 120 → 60 → 50 → 120 restoration.
Picker cases cover compare-and-set rejection, focus preservation and collapsed active
Workspace entry without disclosure. Sidebar renders cover widths 24/30/60, both
locales, tier degradation and exact border hits. Shared input evidence under the
native mouse-input authority owns capture, stale draws, cancellation, matching release,
graphics cleanup and terminal-geometry agreement; pure geometry alone does not prove
those effects. The existing bounded hierarchy traces remain the hierarchy authority.

## Status line

Exactly one bottom row, always drawn, never covered by an overlay. Segments left to right follow
`docs/tui/overlays-and-status.md`: input indicator (the armed prefix badge, empty in the ordinary
layer), context or notice, then right-aligned hints. A segment is drawn whole or dropped; the
context segment drops first, hint text shortens by tier (full hint at Full, `Ctrl+B ?` at Compact,
the exit hint at Minimal and TooSmall). The status text area is reported back to the caller only
when it was drawn completely, preserving the existing draw gate that refuses a confirmation receipt
on a partial draw.

On Windows the ordinary status layout reserves distinct cells for the Menu button when the sidebar
does not provide it and, when the
owner's complete hint still fits, the Machines indicator. The optional indicator uses the label's
display width, capped at 28 columns; it yields to notices, prefix/resize modes and capturing
surfaces. Each frame passes its allocation to the Machines painter, so the final paint and pointer
region cannot cover another status segment. The armed prefix replaces the Menu button with its
badge and an `Esc` cancellation hint, shortening to the key when needed, then to the badge alone. Menus and Settings
replace the complete status row, including its background, before painting their owner's hint.

## Content area and overlays

The Windows [Workbench navigator](native-context-selection.md#windows-capturing-navigator)
uses the existing centered surface with two labeled columns and a window around local focus.
The active marker continues to identify the selected terminal; the selection background shows
the row Enter will open. Complete visible rows publish exact proofs with the successful frame.
Esc/Enter instructions precede optional toggle and movement details; source cases cover both
locales, configured prefix labels, long scrolled columns, 120x41, 120x40 and 80x24. At less than
40x7 inner cells it refuses presentation and releases capture through the fresh-frame fence.
The navigator suppresses pane cursor/focus emphasis and the ordinary Menu button. Shortcut Help
retains its nonmodal behavior.

The content area overlays position against is `ShellAreas.panel`: the workbench region below the
top strip and beside a docked side panel. `tui_layout` exposes two pure functions for surface
renderers:

- `overlay_area(&ShellAreas) -> Rect` — the content region every centered overlay uses.
- `place_overlay(area, preferred_width, preferred_height) -> Option<Rect>` — resolves to
  `min(preferred, area - 4)` by `min(preferred, area - 2)`, centered with the extra cell rounding
  toward the top-left, and returns `None` when the resolved inner area would be below 4x3.

A refused overlay is not drawn; the caller reports through the status line instead. Because the
status row is drawn after every overlay and modal, the never-covered invariant holds even while
older surface renderers still compute their own geometry.

## Split panes

`divide_panes(area, orientation)` returns `PaneDivision { first, divider, second }` where `divider`
is exactly one shared cell column or row between the children, or `None` when either child would be
smaller than the documented 4x3 minimum. The old 50/50 intent is preserved: the divider cell is
taken out of the second child's share, so `first` keeps the width a 50% split used to give it.

A leaf draws no edge borders: edges on the region boundary are served by the region frame and edges
facing a divider are served by the divider cell. Each leaf owns a one-row title line inside its
rectangle; the active leaf's title carries focus emphasis while the pane owns input, and the leaf
below the title receives
the terminal cells. An inactive leaf paints a localized empty state naming the retained terminal
and the focus binding — it never paints fabricated live content. Divider cells are collected during
the walk and drawn once with junction glyphs (`├ ┤ ┬ ┴ ┼`) resolved from neighboring divider cells.

Under a Client-local zoom (`Ctrl+B z`; the session contract lives in `native-tab-session.md`),
the drawn set narrows to the focused leaf: it renders across the whole content area and records
the only pane hit region, while undrawn leaves still report their canonical leaf rectangles so
their viewers stay alive. The focused leaf reports the whole content area to the wire so its
terminal converges to the zoomed size; divider cells are not drawn while zoomed. This split
between the drawn/hit set and the observed set is the only zoom geometry effect — the canonical
projection itself is unchanged.

Capturing surfaces replace the pane's focus emphasis with the unfocused frame and secondary title
styles. Reference overlays preserve pane focus and use an unfocused frame. Sidebar section headings
use primary bold text rather than focus color.
On Windows the Machine picker and Worktree draft also suppress pane focus and
the terminal cursor while they own input.
The Site status colors only its glyph; its text uses the primary text role.
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
rule, the one-row status and strip, sidebar presentation per tier, overlay clamping, centering, and
refusal, pane division geometry and the shared divider, plus TestBackend renders at 120x40, 80x24,
60x12, 50x10, 15x4 and mixed-axis geometries in `en-US` and `zh-CN` proving the status row is never
covered, the too-small notice is localized, and active/long-label tab tokens stay visible through
display-width truncation.

Whole-screen review follows `docs/tui/verification.md`: the current Workspace remains marked
independently of keyboard focus, ordinary layouts expose one global Menu, complete lists omit redundant
ranges, and single-pane edges belong to terminal content. Review the default theme together with
both named RGB themes, empty and populated sidebars, navigation, hidden sidebar and split/zoom.
These outcomes remain acceptance constraints for later feature changes.

## Tab metadata annotations

The native compact strip and navigator keep one token/row per canonical Tab. Pinned Tabs use the
localized `Pinned` / `固定` annotation; grouped Tabs show `group label / Tab label`. The full text
passes the existing sanitizer and cell-width budget, with active/selected styles supplied by the
ordinary Tab theme roles. Drawn widths alone define the stable Tab-ID pointer regions. Annotations
never become raw names, automatic ordinals, extra focus stops or synthetic terminal rows.
