# TUI Components

## Authority and scope

This standard owns the reusable visual and interaction contracts for controls and composed controls
painted by Orchbee into a terminal cell grid. It governs component state, affordance, shared chrome,
hit geometry, and composition. Domain identity, action eligibility, effects, and product copy remain
with the owning feature.

Colors, modifiers, glyphs, and display width follow
[`visual-foundations.md`](visual-foundations.md); region allocation and degradation follow
[`layout.md`](layout.md); input ownership and decoding follow
[`keyboard-input.md`](keyboard-input.md) and [`mouse-input.md`](mouse-input.md); overlay admission and
capture follow [`overlays-and-status.md`](overlays-and-status.md). Shared product meaning follows
[`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md). Semantic action identity and execution follow
[`actions.md`](actions.md); settings value, scope, commit, and reset follow
[`settings.md`](settings.md).

These contracts govern new components and changes to existing components. They do not assert that
every current surface already implements them. An existing surface is conforming only when its
implementation and the evidence required below establish that result.

## Ownership and admission

| Owner              | Responsibility                                                                                                                                               |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Feature            | Stable action and entity identity, labels, availability and reasons, current and checked value models with declared commit boundaries, effects, and outcomes |
| Component          | Minimum usable geometry, slots, state combination, rendering, recorded hit regions, navigation intent, and degradation inside its rectangle                  |
| Surface compositor | Which components are present, reading and focus order, layout tier, and the space assigned to each component                                                 |
| Theme              | Concrete styles for every semantic role and reduced-color fallbacks                                                                                          |
| Input adapter      | Raw event decoding, ownership precedence, pointer-capture policy, and delivery of normalized events                                                          |

- A component accepts typed semantic state and emits typed intents. It never infers identity,
  availability, or an operation from a localized label, color, row position, or glyph.
- Shared component code belongs to the narrowest owner used by its real consumers. Promote a local
  control only when more than one real consumer needs the same complete contract. Naming a component
  family in this standard defines its behavior; it does not require a shared implementation. Do not
  create a universal widget bucket or move feature-specific rows into a shared module merely because
  they use the same library types.
- A shared component owns its whole recurring contract: rest and interaction states, minimum and
  compact geometry, keyboard and pointer equivalence, hit regions, truncation, localization, and
  verification. A helper that only draws a border or color is not a component.
- Variants share one state model and interaction grammar. Unrelated controls do not become one
  component through boolean flags. Product screens compose components through documented slots
  instead of adding screen-specific branches to a shared renderer.
- Component state is plain data. Layout and rendering are pure over state, geometry, theme, and
  locale. Effects run only after the owner receives a component intent.
- An application or package that owns a reusable primitive or recurring composition maintains a
  reference within that owner. It links the canonical source entry point, semantic state and intent
  types, behavior tests, and consumer adoption tests. A move, promotion, replacement, or removal
  updates those links in the same change. This is a maintenance discovery path rather than an
  inventory of business surfaces; do not introduce a parallel registry, manifest schema, or catalog
  command.

Use three ownership levels:

| Level                 | Contents                                                                                               | Reuse boundary                                                              |
| --------------------- | ------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------- |
| Primitive             | Button, inline action, row interaction layer, tab, editor, value control, shortcut item, and scrollbar | Reused when state and gesture meaning are the same                          |
| Recurring composition | List or tree, picker, modal window, and shortcut bar                                                   | Owns arrangement and coordination of primitives, not feature effects        |
| Business surface      | A feature navigator, settings surface, activity dashboard, or workflow                                 | Remains with the feature and composes primitives and recurring compositions |

Before authoring a control:

1. Start at the owning application's or package's maintenance reference. For a reusable primitive or
   composition, follow its links to the one canonical source entry point, semantic types, behavior
   tests, and consumer adoption evidence. For a local control, find the equivalent feature-owned
   source and tests. There is no parallel TUI component catalog.
2. Read the closest primitive or recurring composition through its owner and compare state axes,
   slots, gestures, degradation, and evidence rather than comparing screenshots alone.
3. Reuse it when the contract matches. Extend its semantic owner when a compatible recurring need
   is missing. Replace a local duplicate when the shared contract now covers it. Create a new local
   control when the behavior is feature-specific, and promote it only after the admission rules
   above are met.
4. Update the owning reference links, types, behavior tests, and affected consumer adoption tests. A
   business surface records which shared contracts it adopts; it does not fork their visual or input
   rules.

## State model

The following axes are independent. A component declares every axis it supports and renders valid
combinations instead of compressing them into one `selected` flag.

| Axis                  | States                        | Contract                                                                                                            |
| --------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Information hierarchy | heading, primary, secondary   | Describes reading priority only; interaction never promotes metadata into primary content                           |
| Keyboard ownership    | unfocused, focused            | Focus identifies the event owner and remains visible without relying on a background fill                           |
| Pointer               | rest, hovered, pressed        | Hover and press never commit a model value; an action menu may move its provisional candidate through its owner     |
| Navigation            | none, provisional             | Provisional is the reversible row or value under navigation before acceptance                                       |
| Identity and value    | current, checked              | Current identifies active context; checked identifies a chosen option in the control's declared value model         |
| Availability          | enabled, disabled with reason | Disabled blocks activation but preserves identity and discoverability                                               |
| Operation             | idle, pending, outcome        | Pending describes work after activation; success, failure, cancellation, and unknown are outcomes, not availability |

Backgrounds use exactly one role with this precedence: `surface-pressed`, `surface-selection`,
`surface-hover`, `surface-current`, then the component's base surface. Other channels remain
independent:

- keyboard focus remains visible through `frame-focused` or a stable `accent-focus` marker;
- current identity remains visible through `BOLD`, a stable current marker, or a fill its owner
  documents as the only current cue, underlined where no fill can show;
- a checked value retains its checked glyph using `accent-selection`;
- disabled content uses `text-disabled`, but a current or checked marker remains recognizable;
- pending retains focus, current, and checked cues and adds a fixed-width `status-pending` glyph and
  localized operation verb.

A collection declares where its one keyboard-owner focus cue is painted. A framed collection uses
the enclosing `frame-focused` border; an unframed row owner reserves a dedicated focus track. This
projection is explicit: omitting a track is never an implicit fallback, and a framed row does not
repeat the frame's focus cue. Provisional navigation keeps its own shared row marker in either case.

Checked belongs to the control's declared value model and commit boundary. It may project a parent
draft, a staged multi-choice value, or a confirmed domain value, and remains independent of the
provisional cursor. It never proves persistence by itself. When that projected value differs from
the confirmed or effective basis, the owner supplies a visible changed-draft cue that remains
distinct from focus, provisional navigation, validation, and pending state.

Pointer hover never moves keyboard focus, current context, or a checked value, and never adds
`BOLD`. It changes the background. An action whose own label or glyph rests in `text-secondary`,
such as an inactive tab, an auxiliary button, or an icon action, rises to `text-primary`; list rows
keep their content hierarchy, including secondary headings and metadata. `BOLD` marks press and
any current identity whose owner documents bold. An open action menu updates its provisional
candidate on real unmodified, button-free movement over an enabled painted row, using the same
full-row selection fill and navigation marker as keyboard navigation. This does not execute an
action or open a submenu; a click on a submenu row opens it. Leaving retains the candidate; arrows
continue from it. Disabled rows, separators and group headings do not become pointer candidates. A
menu opened by the pointer, and a submenu opened by a click, starts without a candidate. The first
navigation or confirmation key only reveals one: Up and End reveal the last row; Down, Home, Right
and Enter reveal the first. A menu opened from the keyboard starts on its first row. While a
cascaded submenu is open, its parent's opening row keeps the selection fill without the navigation
marker. Other collections retain their provisional cursor on hover. Pressing arms one enabled
target; leaving that target removes the pressed treatment. The pointer policy determines activation
on release. A disabled target never acquires hover or pressed treatment.

Pending is not a synonym for disabled. The feature supplies whether the operation may be invoked
again, cancelled, or run concurrently; the component presents that policy and does not invent it.
Status colors appear only on the status glyph or status word. No interaction or hierarchy state
uses success, warning, or error for emphasis.

Changing state never moves neighboring labels or columns. Reserve the marker, glyph, status, and
auxiliary-action slots required by any state that the component can enter.

## Action controls

| Family        | Rest affordance                                                     | Use                                                                                                       |
| ------------- | ------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Button        | `[ Label ]`, with one blank cell inside each bracket                | A standalone commit, cancel, confirmation, or form action                                                 |
| Inline action | A stable action glyph or formatted key followed by a localized verb | A command in a header, toolbar, action row, or footer where the group already communicates action context |
| Row action    | The complete labeled row is the primary target                      | Opening, choosing, or activating the entity represented by a list or tree row                             |
| Icon action   | A bracketed, width-stable glyph such as the close control           | A universally recognized action whose label cannot remain visible at the admitted geometry                |

- A bare noun in ordinary text is not an action affordance. A quiet inline action remains
  recognizable at rest through its action glyph or displayed key and verb; hover is never its only
  action cue. Row overflow-menu triggers follow the collection rule below.
- A button's brackets, inner padding, label, and reserved state slots form one painted and clickable
  extent. The primary button in a decision context uses `accent-action` at rest; secondary buttons
  use `text-primary`. Neither state looks like keyboard focus. Status roles never distinguish
  primary, default, or destructive actions.
- An explicitly destructive action may declare the shared destructive intent tone. Its label uses
  `accent-destructive` independently of primary/secondary hierarchy; disabled content still uses
  `text-disabled`. Row and button owners retain their ordinary hover, pressed, focus, and provisional
  tracks. They never construct a local warning color or borrow a status role for this intent.
- A decision button keeps its owner-formatted shortcut inside the brackets, after two blank cells.
  Shortcut text uses `text-secondary` and normal weight; terminal cells retain their host font size.
  It shares the button's complete fixed paint and hit extent and never creates a second action.
- Buttons in a list's auxiliary-action slot use `text-secondary` at rest and inherit the row's
  surface. Their own hover and press feedback remains local to the painted action extent, and
  raises their label to `text-primary`.
- A button group reserves a focus-marker cell for each button. The focused button uses
  `accent-focus` on that marker and its brackets. Pointer hover paints the full button extent with
  `surface-hover`; press uses `surface-pressed` without erasing the focus marker. An enabled button
  label takes `BOLD` only while pressed.
- A pending button reserves a status-glyph slot and sizes its label area for the larger of its idle
  and pending localized labels. Its outer extent does not change when the operation starts.
- An inline action has an explicit group boundary and spacing from adjacent content. Its effective
  hit rectangle is painted as one unit on hover and press. It never masquerades as a heading,
  metadata value, or link.
- An icon action has a localized discoverable name and a keyboard path shown by the current help
  owner. If the layout can keep a label, use an inline action instead. A glyph without both a name
  and keyboard equivalent is decorative and cannot receive a hit region.
- An enabled icon action rests in `text-secondary`. Hover strengthens its glyph and brackets to
  `text-primary` without `BOLD` or a fill; press keeps `text-primary` and adds `BOLD` and
  `surface-pressed`.
  A frame-borne icon action keeps one blank surface cell between each bracket and the frame rule;
  those cells are outside its hit extent. An owner that documents a destructive hover, such as a row
  close, may replace the hover content role.
- The action owner supplies enabled, disabled-reason, pending, and concurrency state. The component
  does not optimistically report success or clear a pending state before the authoritative result.

## Value controls

| Family           | Rest affordance                                                                                                 | Value contract                                                                                                                         |
| ---------------- | --------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- |
| Boolean          | `[ ] Label` for off and `[x] Label` for on                                                                      | A mixed marker exists only when the domain admits a third or aggregated value                                                          |
| Single choice    | `○ Label` and `● Label`, with `( )` and `(*)` as the declared ASCII fallbacks                                   | The checked marker follows the declared value model; the provisional cursor remains separate                                           |
| Bounded stepper  | Stable decrement, formatted-value, and increment tracks, such as `[-] value [+]`                                | The owner supplies bounds and step semantics; an unavailable endpoint stays visible and disabled                                       |
| Editor or picker | An editor has a labeled field boundary; a picker shows its label and current or draft value plus an open action | Editing keeps a draft and caret; opening a picker follows the preview and acceptance contract below; plain value text is not a control |

- The owner supplies the typed value, formatting, bounds, mixed-state admission, confirmed or effective
  basis, draft-change fact, availability, validation, and pending state. The component emits typed
  intents and never parses its glyphs or localized copy to recover value.
- A boolean or single-choice target includes its marker, spacing, and complete label. A stepper gives
  decrement and increment their full painted targets. Its formatted value is static text with no hit
  region unless the owner admits an editor or jump action, in which case that value target owns its
  complete painted field. A picker trigger likewise owns its complete field. There are no unlabeled
  gaps or hover-only targets.
- Hover and press cover the effective target while focus, provisional navigation, checked value, and
  the changed-draft cue remain independent. A draft may change the checked marker, but the separate
  changed cue continues to show that the confirmed or effective basis differs.
- Every value operation has an equivalent keyboard path. State changes preserve all tracks and the
  outer control footprint; unavailable endpoints, validation, and pending work do not shift labels.

## Lists and trees

A shared list owns navigation, scrolling, row interaction layers, and hit geometry. Each item owns
its semantic content and identity. Item renderers paint text and content-specific status; the list
applies current, hover, provisional, focus, and search overlays afterward so rows cannot invent
different interaction styling.

Rows use consistent tracks in this order. A collection may place operational status in a fixed
trailing metadata slot instead of the leading state slot; its declared placement stays the same
for every sibling and interaction state. Identity and checked markers retain their own tracks.
Optional supporting metadata drops before primary content; a relocated state slot stays reserved.
A tree may place its disclosure immediately after the displayed first-line identity instead of in a
leading track. Reserve one separator cell and the glyph before fitting text; keep sibling label
starts stable. Only the painted glyph cell owns disclosure hover, press, and activation; leading
padding and continuation rows retain their declared owner.

| Track            | Contract                                                                                                                          |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| Focus            | Present only for an unframed row focus owner; a framed collection presents focus once on its enclosing frame                      |
| Navigation       | Shared one-cell provisional cursor (`›`, ASCII `>`); never reused for current, checked, changed, or an ordinal                    |
| Indent           | One constant step per tree depth within the component                                                                             |
| Disclosure       | Leading placement reserves a slot for sibling leaves; inline placement budgets the glyph beside identity                          |
| State            | Fixed-width identity, checked, or operational glyph slot                                                                          |
| Primary          | Flexible object label; truncates only after the other required tracks are budgeted                                                |
| Metadata         | Right-aligned or second-line supporting information; dropped before primary content                                               |
| Auxiliary action | Reserved trailing extent; uses secondary hierarchy at rest, strengthens on row focus or hover, and never shifts the primary label |

- Sibling labels start in the same column whether or not a row has children, status, metadata, or a
  visible auxiliary action. Numbers appear only when ordinal position has product meaning.
- A trailing shortcut in the metadata slot is a formatted key in `hint-key`, as in a shortcut bar;
  a count or note in that slot uses `text-secondary`. A row shows one of them, the shortcut first.
- The collection, rather than an item renderer, owns the navigation marker and its
  `accent-selection` treatment. Feature code supplies provisional identity and fixed geometry, not
  a replacement glyph.
- The primary row target spans every cell and line owned by the row, including internal breathing
  space, so there are no dead strips. External group separators remain outside the row's background
  and interaction extent. A separately painted auxiliary action has its own hit region
  and wins hit testing inside that region. Its keyboard equivalent remains discoverable.
- A quiet auxiliary action remains recognizable through its action glyph or formatted key when
  painted. A row overflow-menu trigger may be hidden at rest and revealed by row or child hover,
  or by the row's active keyboard focus. Reserve its extent, publish no hidden action target, keep
  existing context-menu and keyboard routes available, and retain a revealed trigger while its
  own control is hovered or pressed. Revealing it never shifts identity or cancels an armed row.
- A row close icon action may follow the same reveal rule on row or child hover. It occupies the
  first line's trailing edge, covering that line's right-aligned metadata and trailing markers
  only while revealed; identity keeps a fixed one-cell clearance, so it truncates identically at
  rest and on hover. It uses `text-secondary` on the row
  surface and `accent-destructive` only on its own hover and press. It publishes no hidden target,
  keeps the existing menu and keyboard close routes, and enters the owner's ordinary close
  confirmation.
- `surface-current` identifies the active entity after focus moves elsewhere. `surface-selection`
  identifies the provisional cursor. Ordinary collection hover uses `surface-hover` and does not move
  either fact; action menus follow the candidate contract above. A checked glyph identifies the chosen multi-selection at its declared value boundary,
  independently of all three.
- A current, checked, or provisional marker survives truncation and reduced-color rendering.
  Optional metadata never becomes the only indication of identity or state.
- A disclosure row toggles its own group. Expanding or collapsing preserves selection by stable
  identity and moves it deterministically only when the selected descendant becomes hidden.
- A group heading is either static text with no hit region or a complete disclosure row with focus,
  hover, and keyboard activation. It is never half-clickable through only its glyph.
- Filtering preserves provisional and checked values by stable identity. If the provisional item is
  removed, the list chooses the nearest remaining item by its documented order; hover does not
  influence that choice.
- When a row has multiple visual lines, continuation content aligns with the primary track. The
  interaction background and focus marker cover the row's complete visual height.

## Tabs

- The tab list is one focus owner. The active tab is persistent current context and uses neutral
  `surface-current` plus `BOLD` or a stable current marker. Keyboard focus uses a separate
  `accent-focus` marker or focused frame; a manual-activation tab list may additionally show its
  provisional cursor with `surface-selection`.
- Every tab label is an action target across its complete painted extent. Hover and press follow the
  action-state rules and never activate a tab before the admitted gesture commits. An inactive label
  rests in `text-secondary` and rises to `text-primary` under the pointer; `BOLD` marks only the
  active tab and press.
- Activating a tab changes content and leaves focus on that tab unless the owning flow explicitly
  transfers focus into the new panel. Focus movement alone never changes the active tab.
- Overflow follows the owning layout tier through wrapping, scrolling, or a documented overflow
  control. The exit path remains visible, and the active tab does too unless the person scrolls
  the tabs; a hidden label is represented by the overflow control, never silently dropped.

## Search and editors

- A search control has a visible localized scope label or search prefix at rest. Placeholder text is
  supplemental and never carries persistent instructions or serves as the only input affordance.
- An overlay query slot follows [`overlays-and-status.md`](overlays-and-status.md#frame-chrome): the
  activation key and prompt at rest, then `Label: draft` with the terminal cursor while editing. It
  has no bracket boundary; form editors keep theirs.
- The control separates query, focus, match count, loading, empty result, and error. Empty query and
  no matches are different states. Search errors do not erase the query.
- A focused editor uses `frame-focused`, its unframed `accent-focus` marker, or its query-slot label,
  and shows the terminal cursor. An unfocused editor retains its boundary and draft but never paints
  a fake cursor.
- Prefix, mode, validation, pending, and auxiliary-action slots remain stable as the draft changes.
  Contextual hints may change, but the editable origin and submit boundary do not move.
- Editing behavior, atomic paste, composition, and maximum length follow
  [`keyboard-input.md`](keyboard-input.md#editing-and-drafts). Validation and refusal appear in the
  owning footer or status surface without replacing the draft with raw error text.
- Opening a picker, help surface, or temporary subtask records and preserves the parent draft and
  cursor. Returning before acceptance restores that entry snapshot. Acceptance ends any preview and
  updates the parent draft or domain according to the owner's commit policy; a later return preserves
  the accepted result. The parent draft clears or is replaced only after its owning feature accepts
  the parent commit.
- Suggestions and completions use the picker state model. Previewing a suggestion may alter a
  reversible draft projection; hover alone never previews or commits it.

## Pickers

A picker composes a search control, one divider, a scrolling list body, and a contextual footer.
When placed in an overlay, the shared modal owns its outer frame and title. Search and footer remain
fixed while only the result body scrolls.

- The picker owns its query, provisional cursor, scroll position, and reversible preview. The
  feature supplies stable item identity, current and checked values, availability, and commit
  effects.
- Moving the keyboard cursor changes only provisional selection. A single-choice acceptance updates
  the parent draft or authoritative value according to the declared boundary. In a multi-choice
  picker, a toggle changes only the targeted checked projection; the picker acceptance action
  crosses its declared parent draft or domain boundary. Hover changes neither.
- Preview is derived from the provisional item and is reverted when the picker returns before
  acceptance. Acceptance ends that preview and updates the parent draft or domain according to the
  declared boundary; a later return preserves the accepted result and reverses only a newer,
  unaccepted preview. A picker never writes a durable value merely because an item became visible,
  focused, or hovered.
- Filtering preserves the cursor by stable identity when possible and uses the list fallback when
  not. Loading, no source items, no matches, failure, and disabled results have distinct localized
  presentations.
- The footer is derived from the active picker mode and effective bindings. It always retains the
  exit path, then the commit or toggle action, then optional navigation help as space permits.
- Compact or embedded presentation may remove the outer modal frame, but it keeps the same state,
  list tracks, cancellation, and commit boundaries.

## Modal windows

Compact context menus use their dedicated composition in
[`overlays-and-status.md`](overlays-and-status.md#overlay-composition), rather than borrowing a
modal's title, close control or footer. Their row and capture owners still retain all input guards.

The shared modal component implements the slots defined by
[`overlays-and-status.md`](overlays-and-status.md#overlay-composition). It owns the complete outer
surface, frame, title, optional tabs, close action, content rectangle, and footer; feature content
renders only inside the returned content slot.

- Paint and clear the complete modal surface before child content so cells from the underlying frame
  or a prior, larger modal cannot bleed through.
- Compute all slots from the admitted area each frame. If a new modal request cannot meet the minimum
  usable geometry, refuse that request through the owning surface and publish no hit regions for it.
  If an open modal stops fitting, suspend the same instance in the owning too-small state,
  preserve its model, accepted submissions, and pending work, and publish only the valid return,
  exit, or recovery targets. Resume it when geometry returns. Both paths clear stale modal and child
  hit regions.
- The close control is an icon action with a localized name and the effective cancel binding. It
  sits on the frame's top-right edge as defined in
  [`overlays-and-status.md`](overlays-and-status.md#frame-chrome) and follows the same hover,
  pressed, disabled, and focus rules as other actions.
- One modal owner is active at a time. A nested task replaces content, title context, and footer
  actions inside that owner; it does not stack another modal or leave two active footers.
- Modal chrome handles cancel, tab, close, and footer-action intents consistently. Feature content
  receives only events not consumed by that chrome.
- Opening records the prior focus owner. Closing or cancelling restores it without changing current
  context, list order, or draft state as an incidental effect.
- Hit testing respects paint order. Modal and child regions occlude every underlying target even
  where a theme uses the terminal default background.

## Shortcut bars and scrollbars

- A shortcut item is an atomic formatted key plus localized action label using `hint-key` and
  `hint-label`. Generate items from effective binding definitions or structured local key maps; do
  not maintain handwritten key strings beside the binding owner.
- The bar reflects the current input owner and mode. It keeps the exit action, primary action, and
  mode indicator before optional navigation hints, and drops only whole items through layout tiers.
- Persistent chrome separates items with a middle dot; an overlay footer separates them with a
  spaced vertical rule.
- When a surface admits a scrollbar and overflow may appear as content changes, the component
  reserves one track for the lifetime of that view. The content width does not jump when the thumb
  appears.
- The reserved track uses the component's base surface, and the thumb uses `text-secondary` at rest.
  Hover and press apply `surface-hover` and `surface-pressed` across the effective thumb target while
  its glyph remains visible, including when the pane lacks focus. Pointer interaction never becomes
  the only scrolling path.
- Scrollbar geometry derives from total content, viewport, and offset with saturating arithmetic.
  Empty or fully visible content paints no thumb and exposes no scrollbar hit region.

## Rendering and hit regions

- Layout returns component rectangles and relative tracks before rendering. Rendering paints only
  those rectangles and records hit regions from the same geometry; input never reconstructs layout.
- Build a fresh candidate hit registry for every render. Publish it only with a successfully flushed
  frame. A target that is hidden, clipped, unavailable at the current tier, occluded, or refused
  for size has no hit region in that candidate map.
- The effective hit rectangle and hover treatment cover the same cells. Extra pointer padding is
  allowed only when the component owns and paints those cells; invisible targets are forbidden.
- Resolve overlapping regions from the last painted child toward its parent. An auxiliary action,
  close action, or scrollbar wins inside its rectangle; the surrounding row or modal receives the
  remaining cells.
- Rendering paints each owned background completely, including padding and blank cells. It does not
  perform effects, read time, query the terminal, or mutate authoritative domain state.
- Pointer decisions use the map from the last successfully flushed frame. Resize and structural
  layout replacement invalidate that map before the next input decision, and events are dropped
  until a new map is published. A visual-only or unrelated repaint preserves an armed gesture when
  the same stable target identity and interaction revision remain. Geometry, availability,
  occlusion, ownership, accepted gestures, or semantic target changes revise the target and cancel
  the gesture.

## Verification

| Change                             | Required evidence                                                                                                                                                                                                                                                |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Component addition or state change | Render rest, hover, pressed, focused, disabled, pending, and outcome; include current + focus, current + hover, checked + provisional, disabled + current, disabled + checked, disabled + outcome, and pending + focus while secondary hierarchy stays secondary |
| Action family change               | Keyboard and pointer produce the same typed intent; hover preserves focus and committed selection, with only the declared menu candidate exception; press-drag-out does not activate; disabled and pending policies are covered                                  |
| Value control change               | Off/on and admitted mixed values, checked draft versus confirmed basis, provisional radio navigation and ASCII fallback, bounded endpoints, editor and picker rest affordance, whole-target pointer and keyboard parity                                          |
| List or tree change                | Stable columns with and without children, status, metadata, and auxiliary actions; full-height interaction bands; identity-preserving filter and collapse cases                                                                                                  |
| Editor or picker change            | Draft and cursor restoration, retention of accepted results after return, rollback of unaccepted previews, distinct loading/empty/no-match/error states, acceptance, parent commit, cancellation, and wide character input                                       |
| Modal change                       | Slot geometry at every required tier, complete clearing, paint-order occlusion, focus restoration, refusal when newly requested, suspension and resumption after opening, and stale hit removal                                                                  |
| Theme interaction role             | `terminal-native`, one RGB theme, and 16-color output keep focus, current, checked, hover, provisional, pressed, disabled, and pending recognizable through required non-color cues                                                                              |
| Shared component change            | Owner reference links the canonical source, types, behavior tests, and affected consumer adoption tests; render each state in isolation and run [`verification.md`](verification.md#whole-screen-visual-review) for every affected consumer                      |

All render evidence includes `en-US` and `zh-CN`, wide content, the required geometries in
[`verification.md`](verification.md#render-evidence), and a check that hidden targets leave no stale
hit region. Passing a component render does not establish that an existing screen has adopted the
component.
