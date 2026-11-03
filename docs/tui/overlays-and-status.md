# TUI Overlays and Status

## Authority and scope

This standard owns surface selection, overlay composition, input capture, interaction lifecycle,
dialogs and confirmations, the status line, hints, pointer tooltips, transient notices, and asynchronous outcome
presentation for Orchbee-authored terminal interfaces.
Geometry follows [`layout.md`](layout.md#overlays); colors and hierarchy follow
[`visual-foundations.md`](visual-foundations.md); key ownership follows
[`keyboard-input.md`](keyboard-input.md#input-ownership); reusable overlay shells and controls follow
[`components.md`](components.md).

These rules are required for new and materially changed TUI behavior; their presence does not claim
every current Client conforms. Runtime adoption requires source and the evidence in
[`verification.md`](verification.md#contract-review). Record any staged gap with its owner and
completion condition.

The interruption budget in
[`frontend/dialogs-and-forms.md`](../frontend/dialogs-and-forms.md#interruption-budget) applies
unchanged: start with no surface and no question, ask only what the outcome requires, and never
steal focus for background status.

## Surface selection

| Surface               | Use when                                                                              | Avoid when                                                            |
| --------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Status notice         | A result, state change, or refusal can be stated in one line and needs no response    | The person must choose or type                                        |
| Status-line editor    | One short value is edited in place in the status row                                  | The value is long, multi-field, or needs surrounding context to judge |
| Non-capturing overlay | A passive preview while typing continues to the pane                                  | The content must be acted on before the pane can continue             |
| Capturing overlay     | A choice, picker, draft, or reference surface whose keys must not reach the pane      | One line of status or a status-line editor suffices                   |
| Confirmation          | A brief decision precedes an effect that loses something, as in [Dialogs](#admission) | The effect is reversible, or the surface merely reports information   |
| Pointer tooltip       | A resting pointer asks what a glyph, bracket control, or cut name in chrome means     | The words are already painted, or the information is needed to act    |

Any surface that consumes every key is capturing regardless of its size or name. A surface never
changes capture kind while open.

## Overlay composition

An overlay is one bordered region with the following reusable slots. Required slots are always
present; optional slots are omitted without reordering the remaining slots or recreating them
inside content.

Context menus use a compact composition instead: action rows in a rectangular border whose top edge
names the root menu's target. An owner-required reason takes the bottom edge rather than a row, so
availability never changes the frame's height. Because the border already names
the target, a row that acts on that target alone shows only its verb, such as `Rename` or `Close`;
rows that name another object or a counted set keep their full text. Global discovery, dialog
titles and tooltips keep the object noun. Separators and `text-secondary`
group headings divide the rows; a heading is never selectable. Context menus omit the header close
control, footer hints and position counter. A submenu cascades beside its parent as an untitled
frame of the same composition. The parent stays painted but inert: its opening row keeps the
selection fill without the navigation marker, and the child holds the candidate. The visible menu
retains exclusive input ownership. Esc closes a root menu; in a child, Left, Esc or a click on the
parent frame returns to the parent, and a click on, or a rest over, another parent row returns
with that row as the candidate. An outside click dismisses the whole cascade through the exact
admitted menu surface. General modal slots below continue to govern forms, pickers, confirmations
and global action discovery.

| Slot       | Requirement | Contract                                                                                                                  |
| ---------- | ----------- | ------------------------------------------------------------------------------------------------------------------------- |
| Header     | Required    | One localized task title; an optional one-line description names its target or consequence                                |
| Navigation | Optional    | Return path, breadcrumb, or tabs for movement within the same task                                                        |
| Query      | Optional    | Search or filter editor; query text is draft state, not a committed domain value                                          |
| Status     | Optional    | Validation, freshness, pending, failure, or recovery information for this task                                            |
| Content    | Required    | List, editor, prompt, reference text, or progress; the only slot that scrolls by default                                  |
| Actions    | Optional    | Selectable actions when footer bindings alone would be ambiguous                                                          |
| Footer     | Required    | Exit or return first, primary binding second, then at most two secondary bindings; decision buttons may carry these hints |

- One overlay is open at a time. A child task replaces the shell's task slots under the same capture
  owner; it never opens a second overlay above the first. A cascaded context submenu is the one
  exception: its parent stays painted beneath it as inert presentation, and only the child owns
  input.
- A capturing overlay may admit the configured global prefix for close or switch actions through
  its established action owner. Modal tasks and confirmations keep first refusal; other keys and
  paste remain with the overlay and never reach the covered pane.
- The header states what the overlay is about, not the action name alone. A confirmation header
  names the object being affected.
- A non-capturing overlay states in its footer that typing reaches the pane.
- An overlay whose content depends on remote state shows the content's age or a refresh binding; it
  never presents stale content as current without indication.
- Header, navigation, query, status, actions, and footer remain fixed while content scrolls. If the
  fixed slots leave less than the content minimum in [`layout.md`](layout.md#overlays), refuse a
  newly requested overlay; suspend an existing one instead of hiding its return or primary control.

### Frame chrome

- An overlay uses one neutral single-line `frame-unfocused` border, with no outer ring, shadow, or
  doubled frame, and it never dims the content around it. The overlay already owns input through
  exclusive occlusion; its heading and its terminal caret or provisional marker identify that
  ownership, so the border never repeats a focus accent.
- The header title sits on the top-left edge as `─ Title ` in heading text. Every dialog and
  reference surface shows the close control on the top-right edge as `─ [x] ─`, with one blank cell
  beside each bracket and one rule cell before the corner; [Closing](#closing) defines its outcome.
  Other title controls, such as page controls, precede the close control with one blank cell
  between controls. The title truncates before any title control loses a cell.
- One blank row separates the top edge from the first slot when height permits. It is the first space
  removed under height pressure, before any content, status, or footer row.
- Footer hints separate whole items with a spaced vertical rule, as in `Key label  │  Key label`.
  Keys use `hint-key` and labels use `hint-label`. Persistent chrome such as the status line keeps
  its middle-dot separator.
- A query slot is one row followed by a divider. At rest with an empty query, it shows the activation
  key and a localized prompt in secondary text, such as `/ to search`. A query retained at rest shows
  `Label: query` in secondary text. While editing, it shows the localized label, a colon, and the
  draft, such as `Search: draft`, with the terminal caret following the draft and no bracket
  boundary. A query that receives every typed character keeps its editing presentation. Form fields
  use the field band in [`components.md`](components.md#search-and-editors).

### Content hierarchy

- Paint the task title once as the frame heading described in [Frame chrome](#frame-chrome).
  Supporting target context and description use secondary text; the value, exact decision target,
  and consequence needed to act use primary text. Do not make all prose bold or accent-colored.
- Align body text and field groups to declared content tracks. Put form field labels directly above
  their boundaries; a compact editor may keep its label inline. Put field help directly below.
  Separate the header, field groups, and footer with a
  blank row when space permits; reduce those gaps before removing useful content. A field's active
  boundary and caret remain visible, independently of row navigation or selection.
- Keep validation and recovery beside the affected task or field. A status role colors its glyph or
  status word, while explanatory prose keeps its reading hierarchy. Disabled controls retain their
  text-disabled role and reason; neither an error nor selection turns supporting metadata into a
  heading.
- Size the surface for readable task content and complete required controls. Optional help never
  widens a form. Wrap prose and shorten supporting context through the owning layout policy before
  sacrificing the target, field, exit, or primary action. Refuse or suspend an unusable task rather
  than paint a partial required control.
- Show each binding once in the footer. When an action has a button, put its formatted key inside
  that button's shared shortcut slot and omit the duplicate key-and-verb hint. Dialogs choose
  between an action row and a hint row as described in [Dialogs](#structure); other overlays place
  the fixed button row after any secondary navigation hints. Hints drop only as whole records.
  Button labels retain their primary, secondary, destructive, or disabled role while shortcut text
  stays subdued.

## Lifecycle and return

- Opening an overlay records the invoking region and focus identity. Entering a child task also
  records the parent's focus, selection, query or filter, viewport anchor, draft, and reversible
  preview. A visible return action exists whenever that parent state exists, except that a
  cascaded context submenu uses its Left/Esc return bindings and its painted parent frame without
  additional chrome.
- Returning from a child restores the recorded parent state. Closing the root restores the invoker
  when it still exists; otherwise focus moves to the nearest surviving item in the same region,
  then that region's documented default. Neither path reorders panes or changes domain selection as
  a side effect; a selection that already crossed its commit boundary remains committed.
- `Esc` takes the next local back step derived from the current surface state. That step may dismiss
  completed feedback, leave editing, clear a provisional selection, revert a preview, return from a
  child task, or close the root; the surface model defines which one is current rather than applying
  a universal cancellation order. Each step restores that layer's entry state. `Esc`, Back, and
  closing a surface never cancel running work; cancellation is a separate, explicitly named domain
  action.
- Dismissal discards only the current layer's uncommitted UI state. It does not roll back an
  immediate setting or another operation that already crossed its declared commit boundary. Hiding
  or closing after an accepted submission neither undoes the submission nor cancels the work it
  started.
- Replacing or closing a surface releases capture before focus restoration. The decoded input
  packet containing that decision remains owned and is consumed through its end, so trailing bytes
  never reach the new owner. Only a subsequent, separately decoded input event is resolved by the
  new owner; it is never delivered to the dismissed surface. Prefix chords in that tail follow
  [Input ownership](keyboard-input.md#input-ownership).
- A visible popup may admit dismissal from its outside backdrop. Its feature owner may withhold
  that route to retain an active draft, child task, required decision or pending operation while
  keeping an explicit close or return path. An admitted dismissal reuses its exact painted close
  action, respects the current capture owner, and consumes the complete gesture and packet tail.
  Body padding and wheel reports do not dismiss or fall through. An admitted dismissal has the
  same draft and accepted-work policy as the close action.
  A compact context menu without a close control instead uses its exact successfully painted
  surface, target and token proof for that same local dismissal; it invents no invisible X target.
- A surface that stops fitting after it opened is suspended, not implicitly dismissed. Preserve its
  parent state, draft, required decision, accepted submission, pending work, and recovery. The
  too-small state exposes only a currently valid return, exit, or recovery binding and never
  accepts a hidden submit or confirmation; restoring sufficient geometry resumes the same surface
  instance. A required decision follows the parking or close-refusal policy of its owning action.

## Preview and commitment

- Typing a query changes only the visible result set. Moving focus or selection may preview an item
  only when the preview is reversible; neither operation activates the item or writes domain state.
- Every editor, picker, and setting declares its commit boundary through its domain owner. Immediate
  settings commit at their control; an atomic staged draft exposes one outcome-named primary action.
  Do not infer a page-wide Save action from the presence of an overlay.
- Activation accepts the focused choice only after current validation succeeds and crosses the
  surface's declared boundary. The owner decides whether that acceptance updates a parent draft or
  commits domain state. Back or `Esc` restores only an active, unaccepted reversible preview. Once
  accepted, later return or `Esc` preserves the edit in its parent draft or domain and dismisses
  only the current uncommitted layer.
- Asynchronous validation and preview results bind to the exact target and draft revision that
  produced them. A result for an older draft cannot enable commitment or replace current feedback.

## Dialogs

A dialog is a capturing overlay that holds one bounded task: a confirmation, a form, a picker, or a
notice. Reference surfaces such as Help, the navigator, the Agent board and Settings share its
placement, frame and closing rules but keep their own sizes, navigation and footers. Context menus
keep their compact composition. Geometry follows [`layout.md`](layout.md#dialog-size); buttons and
fields follow [`components.md`](components.md).

### Admission

| Kind         | Use when                                                                                              | Otherwise                                                                               |
| ------------ | ----------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| Confirmation | This invocation would lose something, as listed below                                                 | Run the action and report the result with a status notice                               |
| Form         | The task cannot run without values the person types                                                   | Use a safe default and let the person change it later, or edit inline on the object     |
| Picker       | The person chooses one item from a list that is long, filterable, or described by more than its label | Use a menu for a few fixed choices; skip the step when context already decides the item |
| Notice       | A failure or result needs a recovery choice, or detail that cannot fit one status line                | Use a status notice                                                                     |

- Start without a dialog. An action that is reversible, or that loses nothing in this invocation,
  runs immediately and reports its result through a status notice, unless the result is
  [already visible](#pending-completion-and-attention) where the person acted.
- Confirm only what is lost: a running program, files or a checkout on disk, an unsent draft,
  terminals while another Client is attached, or the terminals of more than one Tab at once, as
  when closing a Workspace. Decide per invocation from the actual state.
- A shell that is known to be idle at its prompt is not a running program. Closing one Tab or Panel
  whose terminals run no program, while no other Client is attached, closes it at once; its leaving
  the screen reports the close.
- Names, grouping, and order are organization, not content. Renaming a Project, moving a Workspace
  between Projects, and dissolving a Project end nothing and delete nothing: every Workspace,
  terminal, program, and file stays. They run without a confirmation, and the status notice states
  what stays, as in `Project dissolved · Workspaces retained`. When nothing remained to keep, as
  for an empty Project, the notice states only the result.
- Progress and waiting stay in the dialog that started the work, in its status row; they never open
  a separate dialog or replace the task with a progress-only stage. Work that continues after the
  dialog closes reports its result as a status notice.
- A follow-up step, such as a confirmation that a submitted form needs, replaces the dialog's content
  inside the same frame, and `Esc` returns to the step before it.
- A dialog that cannot be drawn when requested is refused with a status notice; its effect does not
  run. A dialog already open follows the suspension contract in
  [Lifecycle and return](#lifecycle-and-return).

### Structure

Every dialog uses the same rows from top to bottom. An omitted row closes up without leaving a gap.

1. Top edge: the title and the close control.
2. One blank row.
3. Target or context: at most one line, followed by one blank row.
4. Body: the prose of a confirmation or notice, the fields of a form, or the query and list of a
   picker. Each field is a label row, the field row, and its help or error row.
5. Status: one row for the pending, failure, or recovery state of the whole task.
6. One blank row.
7. Footer: the action row in a confirmation, form, or notice; the hint row in a picker.

- A dialog shows an action row or a hint row, never both. A form has no hint row: editing keys are
  listed in Help, and a key that only one field uses is named in that field's help.
- Content starts at the left content edge, two cells inside the frame. The action row ends at the
  right content edge.
- Multi-field forms separate field groups with one blank row.
- A field may attach a short suggestion list after its help row: a one-word `text-secondary`
  heading, then picker rows that fill the field when chosen. The list needs no key help; its picker
  cursor on the row that matches the field shows that `Up` and `Down` move it.

### Titles, targets, and labels

- The title names the task in a few words: `Rename Project`, `New Workspace`,
  `Move Workspace to Project`. A confirmation title is a question that names the action and the kind
  of object: `Close Workspace?`, `Remove Worktree?`. A title may carry a count, as in
  `Close 3 Tabs?`, but never a name or path.
- The target line names the exact object in `text-primary`: the Workspace name, the Tab label, or
  the checkout path. Names truncate at the end and paths in the middle.
- A context line names what the task acts within when neither the title nor a field shows it, as
  `Noun: value` in `text-secondary`, for example `Project: orchbee` in a New Workspace form. It never
  repeats the title, the target, or a prefilled value, and it uses the singular noun.
- A field label is the shortest noun for the value, such as `Name`, `Directory`, or `Branch`. It
  never repeats the title's noun or carries instructions. A single-field dialog whose title already
  names the value, such as `Rename Project`, omits the label; its prefilled field shows the current
  value.
- en-US titles, labels, and buttons use title case; prose uses sentence case. zh-CN titles carry no
  trailing punctuation except the full-width question mark of a confirmation, as in `关闭工作区？`.

### Help and validation

- Help is one optional `text-secondary` line below its field. It states an effect, format, or rule
  that the person could not guess, such as `Leave blank to restore the default name.` It never
  restates the title or label, a limit that the field's error states, or that the change is shared
  with other Clients. Help never widens the dialog.
- A field that can fail validation keeps its help row even without help text, so an error never
  moves the rows below it.
- The primary action is disabled while any field is invalid and while a submission is pending. It
  keeps its label, key, and width, uses `text-disabled`, and shows no hover or pressed treatment.
- An error appears in its field's help row as soon as an edit makes the value invalid, and
  disappears as soon as the value is valid again. A field the person has not edited shows no error,
  even when its first value is invalid, such as an empty name.
- `Enter`, or a press on the disabled primary, submits nothing and exposes the reason instead: every
  invalid field shows its error, and the caret moves to the first one. The reason the primary is
  disabled is therefore always one key away.
- An error replaces its field's help: the `status-error` glyph and one `text-primary` sentence that
  states the fix, such as `Enter a name.` or `Use at most 512 characters.` A refusal from the owner
  that concerns the value uses the same row and keeps the primary disabled until that field
  changes; any other failure uses the status row.

### Actions and keys

- The action row is the last content row. The secondary action comes first and the primary last,
  and the row ends at the right content edge. A dialog has at most three buttons and exactly one
  primary.
- The primary button names the outcome with the title's verb: `Rename`, `Create`, `Move`, `Close`,
  `Remove`. Generic labels such as `Submit`, `OK`, `Yes`, `Confirm`, `提交`, or `确定` are not used.
  Add the object only when the verb alone is ambiguous.
- Each button carries its key inside its brackets: `Esc` on the cancel or close action, `Enter` on
  the primary, and a lowercase letter on any other button. These keys never appear again in a hint
  row.
- A confirmation that ends programs, deletes files or a checkout, or discards a draft gives its
  primary button the destructive tone. The title, frame, and prose keep their ordinary roles.
- A picker's hint row lists the exit key, the choice key, and then navigation, as in
  `Esc close  │  Enter choose  │  ↑↓ select`.
- Unmodified `Enter` takes the focused button's action and `Esc` cancels. A confirmation interprets
  no other key, and its decision packet is captured as described in
  [`keyboard-input.md`](keyboard-input.md#editing-and-drafts).

### Closing

- Every dialog shows the `[x]` close control on its top edge, also when its action row has a cancel
  or close button. `[x]`, `Esc`, and that button close the dialog without effect; `[x]` never
  commits and never takes the destructive tone.
- Closing discards the dialog's own unsubmitted fields without a confirmation. It never undoes or
  cancels work that a submission already started; running work offers its own named action.
- In a follow-up step, `Esc` and the cancel button return to the step before it, and `[x]` closes
  the whole dialog.
- While a submission is pending, the primary is disabled and the cancel button reads `Close`.
  `Esc`, `[x]`, and `Close` close the dialog; the work continues and reports its result as a status
  notice.
- A press outside the dialog closes it like `[x]` when nothing would be lost: a picker, a notice, a
  reference surface, or a form whose fields are unchanged. A confirmation, a changed form, and a
  pending step ignore the press and stay open.

### Focus and pointer

- A form opens with the caret at the end of the first field's value. A confirmation or notice opens
  with focus on its primary button. A picker opens with its query empty and the current item, or
  else the first item, selected.
- `Tab` and `Shift+Tab` move through the fields and then the enabled buttons in reading order,
  wrapping inside the dialog. `[x]` is not a stop because `Esc` is its key. When the focused button
  becomes disabled, focus moves to the cancel or close button. `Enter` in a single-line field
  submits the form, or shows why it cannot be submitted.
- A click places the caret in a field, activates a button, or chooses a picker row. Hover follows
  [`components.md`](components.md#action-controls) and never makes text bold.

### Copy

- Write the fewest words that let the person decide. A consequence is one sentence of facts with
  counts: what ends and what is deleted. Add one sentence of what is kept only when people would
  otherwise worry, as in `Ends 3 terminals.` followed by `Branches and folders are kept.` Each
  of these sentences starts on its own line.
- Mention other Clients once, with the glossary term, and only when another Client is attached.
- Name in-product actions. A CLI command appears only when it is the only recovery, as code text on
  its own line.
- Every string comes from the localization catalog in both en-US and zh-CN, and fits its row by
  display cells: a CJK character takes two cells. Each sentence of prose, help, errors, and status
  starts on its own line and wraps to at most two lines at the dialog's width; rewrite a sentence
  that needs more.
- zh-CN uses full-width punctuation (`，。：？`), one space between CJK text and Latin words or
  digits, glossary nouns such as `工作区`, `项目`, `标签页`, `面板`, `工作树`, and `客户端`, and
  Latin key names such as `Esc` and `Enter`. Copy in one locale contains no words from the other,
  except key names, commands, paths, and user data.

## Status line

The status line is a floating one-row chip on the host's last row, end-aligned one cell in from
the right edge of the content beside any docked side panel. It is shown for an input owner,
meaningful notice, recovery, persistent state, or explicit mode such as released pointer capture.
Content reaches the last row: the status reserves no row, opening and closing it never changes
terminal geometry, and closing restores the covered cells. The chip occludes only its own painted
cells, including pointer input and the terminal cursor beneath them; the rest of the row remains
live content. While visible it stays above other overlays. Its segments, from start to end:

| Segment         | Contract                                                                                              |
| --------------- | ----------------------------------------------------------------------------------------------------- |
| Input indicator | The armed prefix, active mode, capture kind, or released pointer capture; empty in the ordinary layer |
| Context         | The current location such as workspace and tab; optional detail dropped first when width is short     |
| Notice          | The most recent transient or persistent notice; replaces context detail while shown                   |
| Hints           | The current owner's key hints after the lead, dropped as whole records by priority                    |

- The input indicator is mandatory whenever any owner other than passthrough holds the keyboard.
  It is the last segment to be dropped.
- Ordinary passthrough floats nothing at rest. Prefix hints appear only once the person presses the
  prefix and leave when the prefix ends.
- In prefix mode, the current effective cancel binding comes first, followed by effective hints for
  help, side-panel focus when a side panel exists, the action menu when available, tab creation,
  splits, the Agent board, and detach. Width drops trailing hints as whole records.
- A persistent state that leaves the focused content unusable, such as a missing directory, a lost
  connection, or an ended program, shows one or two words with its severity marker, followed by
  the binding that fixes it when one exists. Its full explanation belongs to the affected surface.
- Segments collapse in the order documented in [`layout.md`](layout.md#size-tiers). A partial
  segment is never drawn; it is dropped or replaced by its shorter documented form.
- Status text is sanitized like every external string before painting.

## Notices

| Kind       | Lifetime                                                             | Presentation                         |
| ---------- | -------------------------------------------------------------------- | ------------------------------------ |
| Transient  | A documented duration, then removed by a scheduled redraw            | `text-secondary`                     |
| Persistent | Until the next key event that changes state or an explicit dismissal | `status-*` role matching its meaning |

- At most one notice is visible. A newer notice replaces an older one; a persistent error is not
  replaced by a transient notice.
- Expiry schedules a redraw; a notice never lingers because the interface was idle.
- A notice states the outcome and, when the person can act, the binding that acts. It never
  contains raw error text; failures map to localized messages with stable codes.
- Notices never block input and never move layout.

## Pointer tooltips

A pointer tooltip states in words what one painted chrome target shows only as glyphs, a bracket
control, or a cut name. It supplements discovery: everything it says stays reachable without a
pointer through help, menus, or the binding table.

- Content is a legend, a full name, or an action. A legend names each glyph the target shows, in
  the glyph's own role, followed by its word; items stand two cells apart with no separator. A cut
  name appears whole before its legend. An action states its label, then the key that does the same
  to the same target now, taken from the effective binding table and never hard-coded. A target
  already explained in words gets none; a tooltip never names the current marker, the row kind, or
  help. A glyph inside a larger target, such as a row's state glyph, names itself only while the
  pointer rests on that glyph; so does a short name that stands for a longer one, such as a folder
  named in place of its directory, which gives the longer one in full. The rest of the target
  gives only its cut name.
- The box is the context-menu frame without a heading, marker, or arrow: `surface-overlay` inside a
  `frame-unfocused` border with one padding cell each side, so it is its text width plus four cells
  by its line count plus two rows. Text is `text-primary`, keys use `hint-key`, and the box looks
  the same in every theme without an inverted fill.
- Text takes at most 40 cells by two lines and wraps at spaces; wide characters may break between
  characters. Whatever is left out ends the last line with the ellipsis, a path keeps both ends, an
  action key stays whole while its label truncates, and a cut legend ends with a `text-secondary`
  ellipsis.
- The box sits wholly below the target's last row, else wholly above its first row, else it does
  not open. It starts at the pointer's column and shifts left to stay on screen. It never covers its
  target, the pointer, or the floating status, and it stays put while the pointer moves within its
  target.
- It opens after the pointer rests 300 ms, counted from a real button-free movement report; content
  moving beneath a still pointer never opens one. While one is open, or within 400 ms after the
  pointer left one, the next target's opens at once. It never times out, and its content follows its
  target.
- Leaving the target closes it. So does any key, which still reaches its owner unchanged; `Esc` is
  not consumed. A press or wheel notch closes it and keeps that target quiet until the pointer leaves
  and returns. Focus loss, released capture, lost any-motion reporting, a resize, a menu, overlay,
  help, prefix, mode, drag, or rename starting, and the target moving, disappearing, or changing what
  it does also close it.
- The box is not a target. Its cells occlude what lies beneath like the status chip: moving onto it
  closes it, a press there reaches nothing, and hover resolves beneath after the next frame.
- Keyboard focus never opens one. Without any-motion reporting or native console motion it never
  opens, and nothing substitutes for it, such as a long press.
- Only the sidebar, the Tab strip, and pane titles offer tooltips, under their owners' documented
  content. Menus, overlays, Settings, links, and program output never do.

## Pending, completion, and attention

- An admitted foreground intent receives local acceptance feedback at its initiating control or
  intended destination on the next available frame. It does not wait for background preparation,
  completion, or the progress-indicator delay. The acceptance projection must not invent a canonical
  resource; admission alone does not establish commitment, readiness, or success.
- An operation that has not completed within a documented short delay additionally shows a pending
  indicator in the affected region or the status line. This delay suppresses unnecessary progress
  animation, not acceptance feedback. Local acceptance, progress, first useful content, and
  authoritative completion remain separate observations.
- A completion already visible where the person acted, such as the object they closed leaving the
  screen, adds no status notice, and work that finishes within its delay leaves the status line
  untouched. Failure and unknown outcomes are still reported at once.
- A running count ends with its work. Its last value becomes a brief completed notice stating the
  result, as in `2 Tabs closed`, never a running form such as `2 closed, 0 remaining`.
- A pending indicator is a fixed-width glyph sequence plus a localized verb. It never blocks
  drawing or input for other regions.
- A surface request carries the surface instance, target identity, and request generation. Only a
  completion that still matches all three may change selection, preview, pending state, or visible
  feedback. Replacing or closing the surface invalidates its presentation ownership without
  cancelling domain work. The effect owner may still settle authoritative domain state, but the
  completion cannot reopen the surface or mutate a replacement surface instance.
- An operation with a deadline shows the deadline's effect on completion: success, a definite
  failure, or an unknown outcome with the recovery binding. Unknown is presented as unknown, never
  as success or failure.
- Failure feedback stays beside the affected task or control, preserves retryable non-secret draft
  state, and provides a localized recovery action. Raw errors remain outside product copy.
- Attention is ordered as: a decision currently captured from the person, recovery for the current
  task, other current-task status, then background or unread state. Lower-priority feedback never
  displaces a required decision or persistent failure.
- Progress and completion that can continue in the background do not open an overlay, move focus or
  selection, replace the current draft, or switch context. They update their owning region or an
  unread/status indication until the person navigates to them.

## Empty states

- A content region with nothing to show paints one localized statement of why and the binding that
  creates or loads content. It never paints an unlabeled blank area.
- An empty state uses `text-secondary` and never a status role.

## Verification

| Change                  | Required check                                                                                                                                                                                                                                                |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Overlay addition        | Rendered at every required geometry in [`verification.md`](verification.md#render-evidence); fixed slots and footer order remain visible; new-size refusal, open-size suspension and resumption, and capture kind are tested                                  |
| Child or dismissal flow | Tests restore focus, selection, query, viewport anchor, draft, and preview; each `Esc` unwinds one layer and leaves running work unchanged                                                                                                                    |
| Preview or commit flow  | Tests distinguish focus, unaccepted reversible preview, validation, activation, immediate commit, staged commit, and cancellation; cancellation reverts only the active unaccepted preview and preserves an accepted parent draft or domain value             |
| Confirmation addition   | Tests prove only the exact key confirms, `Esc` cancels, and the refused-geometry path does not run the effect                                                                                                                                                 |
| Status segment change   | Tests cover every tier and prove the input indicator survives to the smallest tier                                                                                                                                                                            |
| Notice change           | A test with injected time proves expiry schedules a redraw and a persistent error is not replaced                                                                                                                                                             |
| Pointer tooltip change  | Tests with injected time prove the rest from real motion only, adjacent opening, key dismissal with the key still delivered, press quieting, and closing when a published frame moves the target; placement tests cover below, above, left shift, and refusal |
| Asynchronous operation  | Tests prove local acceptance before background completion, delayed progress, active-generation completion, stale-result rejection, cancellation/ownership retirement, definite failure, unknown recovery, and that background completion does not focus       |
