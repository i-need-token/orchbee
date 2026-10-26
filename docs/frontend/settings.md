# Frontend Settings

## Authority and scope

This standard owns the presentation and interaction contract for Orchbee-authored Settings
experiences across browser and Desktop React interfaces. It governs Settings surfaces, navigation,
search, setting rows, scope and source disclosure, control selection, commit feedback, reset, and
entry points into related management tasks.

The domain that owns a setting still owns its value semantics, supported scopes, default,
precedence, persistence, synchronization, security boundary, side effects, and compatibility. A
Settings surface presents those facts and emits semantic intent; it does not become a second owner
or infer policy from rendered controls.

Cross-client setting meaning and scope follow [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md).
Design the native counterpart with the GUI setting; distinguish shared value semantics from
Client-local presentation and from whether synchronization is actually supported.

Surface admission and bounded editing tasks follow
[`dialogs-and-forms.md`](dialogs-and-forms.md), visual geometry follows
[`visual-foundations.md`](visual-foundations.md), semantic effects follow
[`actions.md`](actions.md), keyboard entry points follow
[`keyboard-shortcuts.md`](keyboard-shortcuts.md), and product copy follows
[`localization.md`](localization.md). Reviewed compositions and their ownership follow
[`experience-components.md`](experience-components.md).

This document defines reusable cross-workspace rules. Package-owned references remain authoritative
for individual setting catalogs, value schemas, persistence adapters, platform capabilities, and
current product behavior. Existing surfaces receive this review when materially changed; a touched
surface must not introduce or expand a nonconforming pattern.

## Setting model

A **setting** is a durable, revisitable choice or policy that changes subsequent product behavior.
The model distinguishes:

- the **explicit value** written at one scope;
- the **effective value** that currently governs behavior after precedence and availability are
  resolved;
- the **scope** whose state may be edited, such as account, organization, project, workspace,
  device, or session; and
- the **source** that supplied the effective value, such as an explicit override, inherited policy,
  platform default, or product default.

Every new or materially changed setting has one owning record with these fields. The record may live
in types, a package reference, a manifest, or a combination of them, but it must be reviewable
without reverse-engineering the rendered page.

| Field                | Required decision                                                                             |
| -------------------- | --------------------------------------------------------------------------------------------- |
| Identity and owner   | Stable identifier, domain owner, and allowed writers                                          |
| Placement            | Category, section, localized label, description, and search aliases                           |
| Value                | Type, valid domain, canonical default, and absence semantics                                  |
| Scope and source     | Supported write scopes, precedence, inheritance, enforcement, and effective-source projection |
| Storage and security | Persistence owner, synchronization policy, locality, sensitivity, and retention               |
| Availability         | Platform, capability, permission, plan, policy, and dependency guards                         |
| Commit               | Immediate or staged policy, validation boundary, commit point, and failure recovery           |
| Effect horizon       | Whether the result affects current work, new sessions, new resources, reload, or restart      |
| Recovery             | Per-setting reset, inherited fallback, undo when supported, and destructive consequences      |
| Evidence             | Model, deterministic tests, Stories, accessibility path, and applicable system evidence       |

Settings metadata is a shared contract, not a mandate for one schema-driven renderer. A generic
renderer must not erase domain-specific controls, explanations, validation, or management flows.

## Admission and separation

- Admit a value to Settings only when people may reasonably revisit it and expect the choice to
  persist for its declared scope. A value needed only for one foreground outcome stays in that task.
- Do not ask for values already known from authoritative context, safely derivable, or irrelevant
  until a later task.
- A command, status, diagnostic, migration, credential exchange, or collection of resources is not
  a setting merely because it is reachable from Settings. Present it as a clearly named action,
  summary, or dedicated management surface.
- A row with no available value and no current prerequisite is roadmap, not Settings. Do not use a
  disabled control or “coming soon” badge to advertise future capability.
- Hide a capability that is irrelevant in the current environment. Keep it visible only when it
  explains an effective behavior or the person can satisfy a current prerequisite; then state the
  reason and recovery beside the control.
- Do not turn legal consent, irreversible deletion, or acknowledgement of a severe consequence into
  a casual toggle. Use the owning reviewed flow and confirmation boundary.

## Surface and information architecture

Settings is a persistent navigation surface: a page, a dedicated window, or a bounded modal panel.
Desktop and wide browser hosts default to the bounded composition below. A full-window panel is an
exception when a bounded surface cannot preserve necessary context or management space; a modal
version still provides complete modal semantics, inert background behavior, dismissal, and focus
restoration. A compact Dialog is only for a bounded editing or setup task admitted by the Dialog and
Forms standard; it is not a container for the Settings catalog.

- Organize categories by a person's goal and vocabulary, not source folders, packages, storage
  keys, or implementation teams.
- Keep the primary hierarchy shallow: named groups may contain categories, while a category owns
  stable purpose-named sections. Do not add another permanent navigation tier merely to reduce row
  count.
- Give every category one localized heading and an optional concise description. Group related rows
  on one quiet surface with separators; do not wrap every row in an independent card.
- Use a dedicated subpage when a domain owns a collection, history, credential lifecycle, complex
  conflict resolution, or repeated add/edit/remove tasks. Preserve a clear path back to the parent
  category.
- Do not create an “Advanced” dumping ground. Name the outcome or domain. When an expert-only value
  is intentionally file- or command-owned, link to that governed path and explain its scope instead
  of duplicating a partial editor.
- Keep category identifiers and deep links stable when the host supports linking. Reordering for
  presentation never changes the setting's identity or owner.

## Visual composition

Desktop and wide browser Settings surfaces use the shell, density, hierarchy and row treatment below, implemented with Orchbee's semantic tokens and shared primitives.

- Use one centered, bounded surface with visible breathing room on a wide viewport. Expand toward
  the viewport only as available space requires; do not make an ordinary Settings catalog feel like
  another Workbench canvas.
- Divide the shell into one quiet navigation rail and one content pane. The rail owns the top-level
  close or back affordance, search, and category navigation; the content pane owns one category
  heading and its settings. One subtle boundary between them is enough.
- Give the rail a one-step semantic surface contrast from the content pane. Show the selected
  category with one restrained neutral fill; keep icons monochrome and subordinate to labels. Group
  headings appear only when they materially improve scanning and never compete with categories.
- Keep the content measure moderate and the hierarchy plain: a compact page heading, optional short
  introduction, then direct setting rows separated by quiet hairlines. Do not add a hero, dashboard
  chrome, decorative illustration, or card around each section or row.
- Align controls to a consistent trailing column on wide layouts. Put descriptions below their
  labels, let exceptional status or recovery expand the owning row, and avoid floating explanatory
  panels that obscure other settings.
- Reserve radius and shadow for the outer modal boundary and transient anchored surfaces. Interior
  structure relies on spacing and separators; accent color identifies interaction or selection, not
  decoration.
- Keep the shell and navigation stable while the appropriate pane scrolls. The rail may scroll
  independently only when its categories overflow; avoid nested content scrollers and preserve the
  selected category in view.
- Open a complex management destination inside the same stable shell with an explicit back path, or
  move to a dedicated persistent surface. Do not stack another catalog-sized modal over Settings.
- At narrow widths, replace the two-column shell with one pane at a time: category list, category
  content, or management destination. Preserve the same hierarchy and back path instead of squeezing
  the rail beside the content.

## Search and discovery

A Settings surface with more than one category provides global search. Search is navigation, never
mutation.

- Index localized category names, labels, concise descriptions, and reviewed aliases for currently
  available settings and management entry points. Never index secret values, user-entered content,
  hidden roadmap, or diagnostic output.
- Normalize case and whitespace, require every entered term to match, and make ranking and
  tie-breaking deterministic. Rebuild the index when locale or availability changes.
- A result names the setting and its category. Show scope, source, platform, or status only when the
  exception helps distinguish the result; do not decorate every result with redundant metadata.
- Typing preserves the currently displayed page. Activating a result navigates to and reveals the
  row, then moves focus to its heading or row container without changing the value or unexpectedly
  opening its control.
- Up and Down move through results, Enter activates the highlighted result, and Escape first clears
  a nonempty query. Provide an explicit empty state without replacing the stable navigation model.
- Search aliases are product copy and ship in every supported locale. A locale switch does not
  require people to know labels from the previous locale.

## Scope, source, and inheritance

- When more than one write scope is supported, identify the active scope before the controls and
  never silently write to a scope inferred from the last page visit. A scope switch must resolve any
  staged draft before displaying another scope.
- Project the effective value and its source from the owning domain model. The UI never reimplements
  precedence by comparing optional values or storage records.
- Distinguish “Reset to inherited” from “Reset to default.” Removing an explicit override reveals
  the next valid source; it does not manufacture a new explicit value equal to today's default.
- When organization, permission, or platform policy enforces a value, show the effective value with
  a lock or source explanation and make the unavailable edit affordance programmatically clear.
  Disabled styling alone is not an explanation.
- Mark a setting local-only, unsynced, or platform-specific when that exception affects expectation.
  Do not badge the ordinary synchronized case on every row.
- If a source disappears or becomes invalid, resolve the next effective source through the domain
  owner and disclose any degraded state. Never retain a visually selected value that no longer
  governs behavior.

## Row composition

The standard row has a persistent visible label, optional supporting text, current state, and the
smallest semantic control that can express the value. On wider surfaces, copy and control may align
in stable columns; on narrow surfaces they stack without changing reading, focus, or announcement
order.

- Supporting text explains only a non-obvious consequence, scope, dependency, privacy boundary,
  effect horizon, or recovery rule. Do not repeat the label or narrate how to operate a standard
  control.
- Keep frequently scanned values visible. A tooltip cannot be the only place that explains a
  consequence, lock, error, or reset behavior.
- Use typography, grouped surfaces, separators, spacing, and control geometry from Visual
  Foundations. Do not create hierarchy through arbitrary bold text, nested cards, or one-off widths.
- Present known context as text or metadata, not as a disabled text field. A disabled setting
  control is reserved for a real value whose edit is currently prevented and whose reason is shown.

Choose controls by value semantics:

| Value or intent                    | Preferred control and boundary                                                                    |
| ---------------------------------- | ------------------------------------------------------------------------------------------------- |
| Independent reversible boolean     | Switch with a positive label describing the on state; commit at activation                        |
| Several independent selections     | Checkbox group; never use one checkbox as a generic substitute for a standalone switch            |
| Two to five comparable choices     | Radio group; use a segmented control only for short, familiar, equal-weight choices               |
| Longer or space-constrained choice | Select or searchable combobox with stable option identifiers                                      |
| Exact text or bounded number       | Semantic input with units, limits, format, and validation; do not use an untyped generic field    |
| Approximate continuous value       | Slider, with an exact input or displayed value when precision matters                             |
| Color, theme, font, or path        | Domain picker with preview and an explicit canonical value when expert entry is useful            |
| Keyboard binding                   | Dedicated recorder with conflicts, platform rendering, removal, and reset                         |
| Resource collection or account     | Summary plus a “Manage” action into a dedicated surface                                           |
| Secret or external credential      | Bounded secure setup with explicit verification; never a persisted ordinary Settings draft        |
| Command or destructive operation   | Outcome-named button with the action, cancellation, and confirmation contract appropriate to risk |

## Commit and effect lifecycle

Every setting declares one commit policy. Do not make people guess from the presence or absence of a
footer.

- Commit an independent, reversible setting at its owning control. A switch or finite selection
  commits on activation; text and numeric editors commit only at their declared valid boundary,
  such as blur or Enter. Do not add a page-wide Save button to otherwise immediate settings.
- Use a staged draft only when several values form one atomic configuration, validation must inspect
  them together, or commitment starts an expensive or external effect. Keep that draft in one
  bounded section or dedicated task surface with one outcome-named primary action and visible dirty
  state.
- Never mix immediate rows and an undisclosed page-wide draft. If both policies are necessary, make
  their boundaries visually and semantically separate.
- State the effect horizon before commitment when a change applies only to new terminals, future
  conversations, newly created resources, reload, or application restart. “Saved” never implies
  “already applied.”
- Show pending state at the setting and scope that owns the operation. Deduplicate by semantic
  identity, ignore stale completion, and keep unrelated controls available unless the domain
  requires an atomic lock.
- Do not toast routine success. Reflect the committed effective value and any exceptional source or
  horizon in place.
- A persistence or external-apply failure never masquerades as success. Preserve or restore the
  last authoritative effective value, retain a non-secret staged draft when retry is valid, and
  show a localized inline recovery path. A session-only result is valid only when the model names
  and visibly labels that state.

## Dependencies, validation, and conflicts

- Keep a dependent setting visible and disabled when a nearby prerequisite can enable it. State the
  dependency beside the row and associate it programmatically with the control.
- Hide a permanently inapplicable platform or capability branch unless its enforced effective value
  must be explained. Plan, permission, or administrator restrictions remain visible only when the
  information helps the person understand current behavior or a concrete recovery path.
- Validate after meaningful interaction. Preserve the last valid effective value while a draft is
  incomplete or invalid, show the error next to the editor, and never expose a raw exception.
- Do not disable an explicit Save action as the only validation explanation. Accept a reasonable
  submission attempt, reveal every relevant error, and focus the first invalid editor.
- Detect conflicts at the owning model. Show the conflicting setting or binding, its scope and
  source, and concrete replace, remove, or cancel outcomes; never silently win by render order.
- Async verification names pending, verified, rejected, expired, and retryable-failure states. A
  verification result is bound to the exact draft and scope that produced it.

## Reset, recovery, and destructive actions

- Offer per-setting reset when an explicit value differs from its fallback. Name whether reset
  reveals an inherited value or restores the canonical product default.
- A section or page reset is allowed only for one coherent owner and scope. Preview the affected
  values, exclude unrelated resources and data, and use an Alert Dialog when the breadth or effect
  is difficult to reverse.
- Reset is a semantic action, not a second default writer. It removes or replaces state through the
  owning domain operation and reports partial or failed outcomes honestly.
- Separate account deletion, credential revocation, history clearing, and other destructive data
  actions from ordinary preferences in a purpose-named danger section or dedicated task surface.
  Confirmation follows the Dialog and Forms standard and states scope, consequence, and recovery.

## Management, credentials, and tools

Collections such as accounts, profiles, integrations, providers, and bindings need management state,
not an oversized setting row.

- The parent row summarizes the effective state and opens a dedicated subpage or task surface. The
  management surface owns empty, loading, populated, degraded, pending, and failed states plus
  add/edit/remove recovery.
- Adding or editing one bounded resource may use a Dialog when it independently satisfies modal
  admission. Long setup, comparison, history, or repeated management remains persistent.
- Separate an integration's enablement preference from its credential or resource lifecycle. A
  toggle never silently launches authentication, creates remote resources, or accepts legal terms.
- Secrets go directly to an approved secure capability and are cleared from UI memory after their
  bounded operation. Masking a value does not make browser storage, logs, search indexes, analytics,
  or ordinary settings files safe for it.
- Diagnostics and migration are named tools with explicit read/write boundaries. Opening or
  searching Settings never starts a remote mutation, credential prompt, destructive scan, or
  migration. Results state provenance, freshness, limitations, and the action required to change
  behavior.

## Navigation, focus, and accessibility

- Register opening Settings as a semantic action. Desktop hosts normally expose the platform
  settings shortcut unless an owned compatibility contract selects another binding.
- When opening the top-level surface without a deep link, initial focus may enter global search so
  long as the page title is already programmatically named. A deep link or activated search result
  moves focus to the destination heading or row, not directly into a value-changing control.
- Category navigation exposes the current category programmatically and supports ordinary Tab
  navigation plus the native arrow-key behavior of its chosen semantic pattern. Changing category
  never changes a setting.
- Resolve Escape from the innermost owned state outward: dismiss a picker or editor, clear a nonempty
  search, return from a management subpage, then close a cancellable Settings panel. A route or
  dedicated window follows its host navigation contract instead of pretending to be modal.
- A modal full-window panel contains focus, makes the background inert, has a visible close path,
  and restores focus to its invoker. A page or dedicated window must not use `aria-modal`.
- Associate labels, descriptions, dependencies, errors, pending status, and source explanations with
  their controls. Announce exceptional async results politely and preserve a visible focus indicator.
- At narrow widths, collapse or replace the rail with an explicit category control and back path.
  Preserve semantic order, touch targets, zoom behavior, and access to reset or recovery without
  horizontal scrolling.

## Modeling and evidence

Before implementing a new or materially changed Settings surface or setting:

1. Record the user intent, setting admission, domain owner, stable identity, supported scopes,
   precedence, effective-source projection, persistence boundary, and non-goals.
2. Complete the [setting model](#setting-model) fields and model applicable unavailable, loading,
   effective, editing, invalid, committing, committed, failed, enforced, stale-completion, reset,
   and scope-switch transitions.
3. Define decision cases for availability, inheritance, reset, effect horizon, dependencies,
   validation, conflict resolution, cancellation, and async completion. Follow
   [`../MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md) for every applicable
   correctness trigger.
4. Run experience-component discovery and choose reuse, extend, replace, or new. Record setting-row,
   category, search-result, and management-surface invariants in the owning Manifest.
5. Map deterministic tests to effective-value precedence, source projection, scope-safe writes,
   reset, unavailable and enforced states, validation, stale completion, failure recovery, search
   ranking, and non-mutating navigation.
6. Provide Stories for every control family and commit policy used, both shipped locales, light and
   dark themes, narrow and wide layouts, zoom, inherited and overridden values, local-only and
   enforced values, pending and failure, search results and empty state, keyboard navigation, and
   focus restoration.

Existing settings are not exempt when touched. Review the complete affected row or management flow,
remove any newly exposed duplicate owner, and do not broaden hidden commit, roadmap, disabled-only,
or scope-ambiguous behavior.

## References

This standard defines the composition, behavior and architecture of Orchbee Settings.

- [WAI-ARIA Authoring Practices: Switch Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/switch/)
