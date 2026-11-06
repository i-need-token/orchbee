# Cross-client Interface Design

## Authority and scope

This standard owns the common experience contract for Orchbee GUI and TUI Clients: shared meaning,
recognizable presentation, and predictable ways to transfer a learned operation between Clients.
Design a product capability for both Clients by default. A Client-specific capability or staged
delivery must have an explicit reason and scope in the owning feature reference and active task.

Use this standard before the adapter rules in [`frontend/`](frontend/visual-foundations.md) and
[`tui/`](tui/visual-foundations.md). Those standards own rendering, geometry, event decoding, and
host conventions; they may adapt the representation without changing the shared product meaning.
Concrete screens, bindings, settings, and supported capabilities remain with their semantic owner.
This contract does not assert that existing Clients already share every implementation or capability.
CLI grammar and output remain governed by [`cli/command-surface.md`](cli/command-surface.md) and
[`cli/output.md`](cli/output.md).

## Responsibility boundaries

| Responsibility                                                                   | Authority                                                                                                   | Adapter responsibility                                                                                  |
| -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| Entity identity, relationships, lifecycle, and durable state                     | Owning domain capability and its public contract                                                            | Project the authoritative state; never infer identity or lifecycle from labels, colors, or row position |
| Operation meaning, target, eligibility, validation, and outcomes                 | Feature that owns the semantic result                                                                       | Invoke the operation through its supported boundary and present its result                              |
| Product terminology, information priority, action grouping, and default mnemonic | [Glossary](GLOSSARY.md) for cross-workspace terms; owning feature's common experience contract for the rest | Localize and render the same meaning using the Client's interaction vocabulary                          |
| Focus, hover, navigation highlight, open overlays, and scroll position           | Narrowest Client interaction owner                                                                          | Keep local unless an existing contract explicitly gives the value a shared scope                        |
| Theme values, typography, geometry, glyphs, and input decoding                   | Respective theme and Client adapter owners                                                                  | Implement the shared roles within platform and host capabilities                                        |

Shared data does not make every view state shared. Distinguish a domain's current object from a
Client's displayed context, keyboard focus, and provisional selection. Declare persistence and
scope before sharing a preference or view state; one Client moving focus must not silently move
another Client's focus. Rendering adaptations and theme changes do not themselves mutate domain
state; an operation that edits an owned layout still goes through that layout's semantic owner.

Reuse existing domain and protocol owners under [`ARCHITECTURE.md`](ARCHITECTURE.md). Do not create a
second business writer inside a renderer, a universal UI Store, or a new package merely to place
both Clients' names on the same module. Cross-language agreement uses the owning serialized
contracts and meaningful shared examples; it does not require sharing React and Rust components.

## Common feature contract

Before implementing a capability exposed in either Client, inspect its counterpart and record the
following decisions in the existing owning reference, types, or tests. Keep task sequencing in the
active task; do not create a separate per-task specification or duplicate feature catalog.

| Decision             | Required agreement                                                                                                                                              |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Intent and target    | Stable semantic operation, entity identity, scope, and authoritative owner                                                                                      |
| State                | Available, unavailable, pending, completed, failed, cancelled, and unknown outcomes where applicable; distinguish unsupported capability from unavailable state |
| Information          | Shared terminology, object hierarchy, task-critical fields, ordering meaning, and primary versus supporting information                                         |
| Entry and completion | Visible entry point, default mnemonic, target selection, commit boundary, cancellation, and return location                                                     |
| Presentation         | Semantic roles, recurring interaction pattern, and the GUI/TUI mapping                                                                                          |
| Differences          | Host constraint or accepted product reason, supported alternative, user-visible explanation where needed, and evidence                                          |

Given equivalent authoritative state, capabilities, and user choices, both Clients must identify
the same target, apply the same domain rules, and explain the same outcome. A local projection may
filter, group, or defer information, but must expose its scope and preserve access to information
needed for the task. Missing data, loading, disconnection, and an empty result are different states.

Use stable action identity across presenters. If existing runtimes use different local identifiers,
keep their explicit correspondence with the feature contract; never infer it from translated labels.
GUI invocation follows [`frontend/actions.md`](frontend/actions.md); native TUI invocation follows
[`tui/actions.md`](tui/actions.md). Both project the existing semantic owner instead of creating a
second operation or result model in the presenter.

## Keyboard transfer rules

The feature owns an action's mnemonic and semantic scope together with its GUI and TUI default
bindings. Choose them together. Keep the same mnemonic key for the same action where host input
allows it; adapt the invocation mechanism through the platform modifier, prefix, or named mode.

For example, a search action can map GUI `Mod+f` to TUI `<prefix> f`: `Mod` becomes `Ctrl` or `⌘`
through the GUI formatter, and `<prefix>` becomes the configured TUI prefix. If that prefix is
`Ctrl+b`, the TUI sequence is `Ctrl+b`, then `f`. This illustrates the translation rule and does
not assign product defaults. A prefix is an input layer, not an action mnemonic.

- Match action identity and search or operation scope before matching letters. The same letter
  must not suggest equivalence between unrelated actions or between local and global search.
- Preserve a family's mnemonic and qualifier relationships when adding related actions. Check
  active scopes and collisions on both Clients before reserving a new default.
- Do not mechanically translate every GUI chord into a prefix sequence. Editing keys, terminal
  passthrough, input-method composition, host-reserved chords, and modal ownership retain their
  adapter contracts. A mnemonic never justifies stealing ordinary terminal input.
- When a collision or host limitation prevents the common mnemonic, first use an existing scope
  or qualifier that remains discoverable. Otherwise record the reason and alternative with the
  owning binding; do not let each Client silently choose an unrelated key.
- Bindings and user overrides refer to semantic actions and their scopes. Preserve valid explicit
  overrides when defaults change. If a setting supports cross-client transfer, validate its target
  mapping and report conflicts or unsupported bindings; never silently remap or drop them.
- Help, menus, tooltips, and command results show the effective binding, including user overrides,
  using the adapter's formatter. Derive any displayed counterpart binding from the owning binding
  data; do not maintain a second handwritten shortcut chart or show unsupported actions as usable.

[`frontend/keyboard-shortcuts.md`](frontend/keyboard-shortcuts.md) owns GUI normalization, `Mod`,
scope resolution, and display. [`tui/keyboard-input.md`](tui/keyboard-input.md) owns prefix and mode
routing, terminal protection, and TUI display. Shared mnemonic rules do not equate physical GUI key
codes with terminal bytes or require identical event decoders.

## Visual roles and themes

Select presentation by meaning before choosing a Client token. Existing adapter vocabularies map
to common roles as follows; the mapping does not require identical token names or new aliases.

| Common meaning                                   | GUI mapping                                                                                                                    | TUI mapping                                                       |
| ------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------- |
| Primary and supporting information               | `--ob-sys-color-text-primary`, `--ob-sys-color-text-secondary`                                                                 | `text-primary`, `text-secondary`                                  |
| De-emphasized grouping context                   | `--ob-sys-color-text-muted`                                                                                                    | `text-muted`                                                      |
| Keyboard focus                                   | `--ob-sys-color-border-focus` and control focus geometry                                                                       | `frame-focused` or `accent-focus`                                 |
| Primary action                                   | `--ob-sys-color-action-primary` and its control variants                                                                       | `accent-action` on the action control                             |
| Pointer hover                                    | `--ob-sys-color-surface-hovered` or the control's declared hover variant                                                       | `surface-hover` across the effective hit target                   |
| Pointer press                                    | The control's declared pressed feedback under frontend foundations                                                             | `surface-pressed` with independent focus and identity cues        |
| Persistent current context                       | Neutral `--ob-sys-color-surface-selected` in navigation                                                                        | Neutral `surface-current`                                         |
| Provisional selection                            | Selection surface and indicator owned by the control                                                                           | `surface-selection` and `accent-selection`                        |
| Checked value                                    | The control's checked indicator for its declared value model, independent of its navigation cursor                             | Stable checked glyph with `accent-selection`                      |
| Success, warning, error, and information         | `--ob-sys-color-status-success`, `--ob-sys-color-status-warning`, `--ob-sys-color-status-danger`, `--ob-sys-color-status-info` | `status-success`, `status-warning`, `status-error`, `status-info` |
| Work in progress                                 | Busy indicator and localized operation text                                                                                    | `status-pending` indicator and localized operation text           |
| Canvas, persistent chrome, and temporary overlay | `--ob-sys-color-surface-canvas`, `--ob-sys-color-surface-raised`, `--ob-sys-color-surface-overlay` as the surface requires     | `surface-canvas`, `surface-chrome`, `surface-overlay`             |

Focus, hover, press, current context, provisional selection, checked value, availability, and
operation status are independent facts. Define their combined appearance at the owning surface;
moving focus does not erase current context, and hover never commits a value or changes focus.
Checked state belongs to the control's declared editing boundary, which may project a parent draft
or an authoritative value; the indicator alone never proves persistence.
Primary actions and focus use action or focus roles, never success merely for emphasis. Pending
and unknown outcomes must not look successful. Color is accompanied by meaningful text, shape,
glyph, or emphasis so the distinction survives reduced color and unavailable modifiers.

Select corresponding GUI/TUI product palettes together, including product-controlled defaults,
and declare the intended correspondence with the theme owner. Different names alone do not justify
unrelated appearance. For a corresponding theme pair, including the same named product theme,
declare one authority for its palette and shared role intent. Derive or explicitly verify adapter
mappings against it in the same change; separately
maintained, unchecked copies of color values are not a common theme. Equivalent roles preserve
their recognizable color family and contrast hierarchy. Transparency, compositing, color-space
conversion, and terminal quantization are adapter decisions with render evidence, not reasons to
choose unrelated palettes. Verify correspondence explicitly rather than inferring it from names.

The CSS registry remains a GUI implementation authority under
[`frontend/design-tokens.md`](frontend/design-tokens.md). The native theme boundary remains the
cell-grid authority under [`tui/visual-foundations.md`](tui/visual-foundations.md). Host-relative
`terminal-native`, user overrides, unknown appearance, and reduced color retain their admitted
behavior; preserve recognizable meaning through non-color cues instead of promising exact RGB
parity. Program-authored terminal content is outside Orchbee chrome styling.

## Typography, density, and layout

- Preserve information priority: object identity and task-critical state precede supporting
  metadata and optional hints. GUI font roles and TUI emphasis express the same hierarchy without
  translating pixels directly into cells or requiring the same number of type levels.
- Preserve object relationships, navigation meaning, action grouping, and reading order. A GUI
  sidebar may become a TUI overlay at narrow geometry; the user still reaches the same objects and
  operations through a discoverable entry point.
- Keep related items closer than unrelated groups. GUI spacing, control sizes, radii, and touch
  targets follow frontend foundations; integer cell spacing, frames, and size tiers follow TUI
  foundations. Neither adapter copies measurements from the other.
- Reduce optional decoration and metadata before task-critical context. Do not drop an action,
  hide the current target, shrink text, or obscure cancellation solely to mimic another Client's
  density. Use the adapter's documented overflow, scrolling, or too-small behavior.
- Preserve access to full data when a label is truncated. Verify translated labels, wide characters,
  narrow layouts, and focus visibility in both Clients; identical screenshots are not the oracle.

## Recurring interaction patterns

| Pattern                      | Common behavior                                                                                                             | Permitted adaptation                                                                                                                                        |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Navigation and resource rows | Same identity, hierarchy, current-context meaning, and distinction between highlight and activation                         | GUI tree or list; TUI rows or navigator; pointer and keyboard entry                                                                                         |
| Menus and commands           | Same action names, target scope, relative order of common groups, availability reasons, and destructive meaning             | GUI menu or command surface; TUI menu or prefix help; Client-only actions remain in their relevant group                                                    |
| Search and filtering         | Same search scope, query meaning, default options, result identity, next/previous ordering, and clear/close effects         | GUI field or panel; TUI editor or overlay; different visible result counts                                                                                  |
| Editing and pickers          | Same field meaning, defaults, validation, draft/commit boundary, and cancellation result                                    | Inline control, Dialog, or capturing overlay according to task and host                                                                                     |
| Direct manipulation          | Same operation, target, constraints, committed result, and cancellation before commit                                       | GUI pointer gesture and keyboard alternative; TUI command or mode with the same effect                                                                      |
| Confirmation                 | Same effect, affected scope, consequence, and safe cancellation                                                             | GUI action control or exact TUI confirm key; geometry failure never grants consent                                                                          |
| Dialogs                      | Same kind, admission, window-centered placement, content order, copy, outcome verbs, and close outcomes                     | GUI Dialog or Alert Dialog with a backdrop; TUI capturing overlay without dimming, with filled fields and bracketed buttons; sizes from each Client's scale |
| Async feedback               | Same pending, success, failure, cancellation, and unknown meanings and recovery action                                      | Inline status, notice, or status line; background work does not take focus                                                                                  |
| Settings                     | Same semantic setting identity, units, domain default, validation, application timing, and reset meaning for the same scope | GUI control or native configuration; presentation-only settings may remain Client-local                                                                     |

The TUI dialog contract in [`tui/overlays-and-status.md`](tui/overlays-and-status.md#dialogs) is
the reference composition for both Clients. A GUI confirmation is an Alert Dialog; forms, pickers,
and notices use the Dialog shell in
[`frontend/dialogs-and-forms.md`](frontend/dialogs-and-forms.md#compact-desktop-dialog-shell). Both
Clients use the same wording for titles, targets, labels, help, errors, and buttons, end the footer
with the primary action, and give the close control, `Esc`, the cancel action, and an admitted
outside press the same no-commit outcome. A close confirmation opens with focus on its Close action
in both Clients, so Enter closes and Escape cancels. Three details follow each Client's own
standard: a GUI Alert Dialog has no header close control and, outside close confirmations, focuses
its least destructive action when a mistake would be costly, and a GUI form validates on submission
instead of disabling its primary action.

Product terminology is stable within each locale. Short TUI labels are documented variants of the
same term; they must not rename an entity or operation. Localized copy uses the existing catalog
owners and must distinguish, for example, closing a view from terminating its underlying work.

Dismissal, draft cancellation, and cancellation of running work are distinct operations. Closing a
surface does not imply stopping its work. Return focus to a valid prior location after a temporary
task; apply the adapter's fallback if that location no longer exists. Escape, Enter, pointer, and
terminal input follow the active input owner rather than a cross-client global interception rule.

A shared setting's meaning does not imply that it is already synchronized. Its existing storage
and configuration owner declares scope, persistence, propagation, and conflicts. Never silently
claim a Client-local setting is global or reinterpret a shared setting at one renderer.
The same setting keeps the same commit and application timing across adapters. Saved, effective,
draft, preview, and navigation values remain distinct. Presentation-only preview must be reversible;
closing a picker must not silently persist it. GUI surface rules follow
[`frontend/settings.md`](frontend/settings.md), and native surface rules follow
[`tui/settings.md`](tui/settings.md).

Reuse reviewed compositions within each Client. The GUI discovery workflow remains
[`frontend/experience-components.md`](frontend/experience-components.md); TUI discovery, ownership,
and recurring control contracts follow [`tui/components.md`](tui/components.md), and TUI surface
selection follows [`tui/overlays-and-status.md`](tui/overlays-and-status.md). Share the interaction contract
and domain operation across adapters, while retaining components with their existing owners.

## Development and verification

1. Start a feature or behavior change by identifying both Client entry points and their common
   owner. Specify the common contract and mappings before implementing the first adapter.
2. Implement the domain or protocol change at its owner, then update both affected projections,
   effective bindings, localized copy, and owning references in the same delivery scope. A shared
   default change includes both Clients in impact analysis even when only one request named it.
3. For staged delivery, record the missing counterpart, concrete limitation, responsible owner,
   and completion condition in the existing task. The feature reference states current support
   and removes the limitation when resolved. Present unavailable capability honestly; completion
   of one adapter does not establish cross-client completion.
4. Verify the common semantics and each adapter's behavior below, then run the applicable host
   gate under [`VERIFICATION.md`](VERIFICATION.md). Review product differences explicitly; a
   passing CSS or Rust check alone does not establish cross-client consistency.

| Changed claim             | Evidence                                                                                                                                                                                              |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared operation or state | Equivalent domain snapshots and intent traces produce the same target, eligibility, committed result, and outcome classification through both adapters; use existing contract fixtures when available |
| Shortcut or mapping       | Both effective bindings invoke the same scoped action; test mnemonic correspondence, platform normalization, conflicts, overrides where supported, and protected editing or terminal input            |
| Interaction lifecycle     | Cover selection versus activation, cancellation before commit, pending/failure/unknown results, and valid focus return through both entry paths as applicable                                         |
| Visual role or layout     | Inspect the same scenario in both Clients under comparable product themes, then the adapter's required host, color, geometry, and locale cases                                                        |
| Setting                   | Check the same scope, value interpretation, validation, default/reset, and propagation contract; distinguish unsupported and Client-local settings                                                    |

Use browser and Story evidence under [`frontend/storybook.md`](frontend/storybook.md) and cell-grid,
input, and host evidence under [`tui/verification.md`](tui/verification.md). Assert shared meaning
and adapter-specific behavior rather than pixel equality or identical raw events. Keep evidence
limits and unresolved differences in the existing task. These cross-client requirements are review
and feature-test obligations; no repository-wide automated parity checker currently enforces them.
