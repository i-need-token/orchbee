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

The Windows sidebar reserves its bottom Menu row and uses the remaining height for
one Project → Workspace → Agent tree; Standalone owns its Agent children in the same
window. Agent children appear only under their explicitly expanded owner. Configuration
still owns each row's height, gap and token styles.

Short sidebars omit heading padding before clipping entries. The unified window fits
complete rows where possible and clips an oversized row to the available body. Keyboard
focus, or the visible current Agent with its Context as fallback, determines focus reveal.
The heading retains Workspace and Agent picker entry points. Paint and pointer regions
use the same final rectangles; tree disclosure never changes terminal content geometry.

When canonical navigation rows are available, the sidebar starts with Projects:
it omits repeated application branding and the ordinary session status already
shown in the bottom row. Before those rows arrive, its existing connection and
business placeholders remain. Both sidebar presentations keep one top padding row.
Section headings separate a primary label, secondary count and right-aligned localized
browse/filter action; the Project count is omitted for zero or one Project. A blank
row separates a heading from its body when the section can fit it. Footer rows carry
only overflow ranges; fully visible lists leave them blank.

Every navigation row reserves a two-cell focus gutter. Root names start after a
two-cell disclosure/status column; Workspace children indent two more cells and their
Agents indent another two. Branch
and custom continuation rows align with the Workspace name, including when a configured
first row omits the state icon. Project labels and metadata use secondary text;
Workspace names use primary text. Row menus remain aligned at the right edge.
The current Workspace retains its exact target and neutral full-row `surface-current`
while typing, with a bold name and no asterisk when the fill is distinct. A collapsed current Project
inherits that surface. Keyboard focus uses `>` in the separate gutter; its foreground
is accented except on the current row, where the surface's text remains legible.
Project and eligible Context disclosure use one glyph (`▾`/`▸`, with `-`/`+` ASCII fallbacks) in their own
two-cell hit region; gutter, content and menu hits remain disjoint from disclosure.
The top strip keeps
the context breadcrumb available when sidebar rows scroll out
of view and uses padded, filled active Tabs without bracket or asterisk decoration.
In split view the shared outer frame has no duplicate generic title. Its base
style is neutral; the existing boundary segments around the focused leaf use the
focused frame role, alongside that leaf's accented title. No extra border cells
are added around leaves and divider rectangles stay unchanged. Divider
ends update the existing outer boundary's junction glyphs, leaving no half-cell
gap and preserving the frame's corner style. Single panes and zoom use a one-row
title band with no outer borders, reclaiming the left edge and bottom row. Each pane
reserves one right chrome column for its [history scrollbar](native-terminal-viewport.md#scrollbar-interaction).
The column stays reserved when history is empty or pending, so output never changes PTY width.
It is disjoint from the content, title and split-divider hit regions. A one-column pane omits
the gutter; a track shorter than two rows has no thumb. Split leaves and zoom use the same
content-width deduction in native geometry, drawing, input coordinates and layout queries.
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

## Quick creation controls

On Windows the top strip reserves three cells for `+` after the last visible Tab,
including an empty context. A long breadcrumb uses at most half the strip so the current
Tab and create control retain space. Controls disappear with the strip below Compact.
The Projects heading has a separate `+` beside Browse; current-Machine Project rows
have a `+` beside their existing row menu. Disclosure, content, create and menu cells
are disjoint. The glyph is locale-neutral; action labels and unavailable reasons use
the existing English and Chinese catalogs.

| Entry                | Existing action and authority                                                                                                                                                                                                                                                                                                                    |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Tab-strip `+`        | `DrawnTabCreate` freezes the Site, snapshot revision and context. An unmodified left press arms a local pointer gesture; matching release inside the same still-exposed rectangle asks `NativeUi` to revalidate that proof, then hands `ClientAction::CreateTab` to the ordinary dispatcher and membership admission. No effect occurs on press. |
| Projects-heading `+` | Opens the creation subset of the global action menu: Workspace picker, quick Workspace and Worktree create/open. The original picker can choose or create a Project. No backend creation semantics are added.                                                                                                                                    |
| Project-row `+`      | Opens the same creation subset over the exact painted Project, with targeted quick Workspace first. Existing capability checks and disabled reasons remain; no child Workspace is selected implicitly.                                                                                                                                           |

Tab-create gestures cancel on changed identity, geometry, covering capture, released pointer
capture or an outside release. The release packet's remaining bytes, including paste, never
reach a pane even when the proof is refused. Menu entries retain the existing capture and
cancellation lifecycle. The keyboard routes remain: configured New Tab (default `Ctrl+B c`),
Workspace creation (`Ctrl+B a`), quick Workspace (`Ctrl+B Shift+N`) and the global/Project
menus. These controls own no new business state, terminal identity or persistence.

Render and hit-region cases cover both locales and default/RGB themes, empty and populated
lists, every viewport tier and long labels. Pointer traces cover complete clicks and invalidation;
NativeUi tests check stale context/snapshot refusal and delegation to the original command/draft
owners. These deterministic tests and composed buffer previews do not establish physical-host
palette or font behavior.

## Sidebar Agent counts and discovery

The unified sidebar joins the existing eligible Agent projection to exact owners.
Only actual Agent rows retain global ordinals; loading, unavailable, empty and
no-match placeholders remain in the global Agents picker. The tree footer counts
visible navigation entries, including Project and Context rows, and reports a
range only on overflow. It does not present that count as an Agent total.

The heading opens the Workspace picker and retains a separate localized Agent
filter action. A visible range footer opens the Workspace picker; a blank footer
is not an action. The fixed bottom `[Menu]` entry (`[菜单]` in Chinese) opens the
existing global menu, with its first configured shortcut when it fits. The status
bar supplies that entry only when the sidebar does not. Hidden sidebars publish
no sidebar hits. All entry points retain existing admission and capture ownership.

Rendering consumes trusted picker rows, the shared window, geometry and binding
registry. It performs no observation or backend command. Render and input tests
cover both locales, themes, configured rows, global ordinals, picker/Menu entry
points and the Full/Compact/Minimal/TooSmall matrix. Host interaction requires
separate native evidence.

## Sidebar navigation model

The native sidebar renders Project → Workspace → Agent from complete Machine
checkpoints. Standalone precedes Projects for each Machine and owns its own Agent
children. `machine_spaces::Checkpoint` retains Project IDs and labels from the
same fenced Snapshot as Workspaces, including empty Projects. Until it arrives,
the existing Context projection supplies its known Projects and Workspaces.

`machine_picker::Picker` owns Project expansion, one optional expanded Agent
Context, keyboard row focus and one shared window. `SidebarTarget::Project`
carries Machine/Site/Project identity; `SidebarTarget::Context` carries Machine
and the full `NativeContextTarget`; `SidebarTarget::Agent` also retains exact
member and physical binding identity. Agent joining never uses labels or row
positions. Equal sibling labels retain ID annotations. Canonical Project and
Workspace order and the global eligible Agent ordinals remain intact.

| State / event                      | Decision and observable result                                                                                                                                                                                                                                          |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Initial disclosure / Agent arrival | All Agent lists start collapsed. New observations, active selection and entry into navigation never open a Context implicitly. At most one Context's Agents are expanded.                                                                                               |
| Context disclosure press           | Toggle that exact visible Context without activating a terminal or taking keyboard focus. Opening another closes the previous one. Preserve a manually scrolled viewport, cancel pending wheel geometry and retire the painted hit map and packet tail.                 |
| Enter navigation                   | An available Picker enters explicit keyboard ownership, including an empty tree. Retain a valid prior row focus, otherwise reveal the active Context, its visible Project parent or the first row. Entry may precede the first paint.                                   |
| Arrow / Home / End                 | Move and reveal only the stable keyboard focus among visible rows; do not activate a Context or Agent.                                                                                                                                                                  |
| Right / Left                       | Right expands a Project or eligible Context, then enters its first child on another press. Left moves from an Agent to its Context, collapses an expanded Context before moving to its Project, or collapses a Project. Enter on a Project toggles it.                  |
| Enter on Context / Agent           | Return one identity-checked `Activate(Selection)` for the exact visible target. The existing owner applies it; the current Agent is a no-op and focus movement never writes `Picker.request`.                                                                           |
| Menu / F1                          | Context and Project rows retain their existing menu intent and admission; Agent rows do not invent a Context menu. F1 opens shortcut help while preserving row focus and navigation ownership.                                                                          |
| Source replacement / collapse      | Clear Agent disclosure when its Context vanishes, its Project collapses, or its last eligible Agent disappears. Retain an exact visible focus, otherwise use its nearest visible ancestor, then the bounded prior position. Removed IDs never redirect to equal labels. |
| Wheel / key reveal                 | Workspace and Agent hits use the same coalesced, viewport-capped wheel window. Keyboard movement resumes focus reveal; wheel movement does not activate a target.                                                                                                       |
| Escape / hidden sidebar            | Leave keyboard ownership and consume its packet tail. Hiding the last observation consumer releases source rows and obsolete disclosure; ordinary terminal input retains its existing owner.                                                                            |

Agent ordinals occupy a fixed three-cell gutter inside their nested row. Configured
continuation rows align with the first name after its optional state icon. Secondary
rows use metadata text and retain explicit token overrides. Current Contexts and
Agents use the same neutral surface and bold identity, with the theme's text marker
when a collapsed palette needs it. Keyboard focus uses a separate `>` gutter.

Project/Context disclosure, row content and row menus have disjoint painted
rectangles. `SIDEBAR_BINDINGS` owns key matching and localized discovery. The
dispatcher owns capture precedence, stale-paint rejection and packet-tail fencing;
the menu owner retains capability checks and execution. Project menus never choose
a child Workspace. GUI correspondence covers hierarchy and identity semantics,
without claiming pixel or backend equivalence.

| Claim                                     | Executable evidence and limits                                                                                                                                                           |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Exact grouping and no implicit activation | Deterministic Picker cases cover Machine/Site/Context joins, duplicate labels, Standalone, one disclosure, pruning and explicit activation.                                              |
| Valid focus and scroll                    | Bounded generated navigation traces cover topology changes; focused cases cover Agent-parent movement, current-Agent reveal and manual-scroll disclosure. No unbounded proof is claimed. |
| Shared geometry and input authority       | Render, pointer and dispatcher cases cover configured heights/gaps, both locales/themes, wide glyphs, stale hits, wheel cancellation and disclosure without focus transfer.              |

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

Pinned Tabs use the localized `Pinned` / `固定` annotation. The Windows compact strip paints separate
colored group headers with disclosure and member counts; expanded members retain their ordinary
Tab tokens, and a collapsed header identifies an active member. The navigator keeps every canonical
Tab available with its group annotation. Text passes the existing sanitizer and cell-width budget;
painted widths define the exact Tab/group pointer regions. Display annotations never become raw
names or automatic ordinals. The [Tab group contract](native-tab-groups.md) owns disclosure,
configuration and cross-host capability coverage.
