# TUI Settings

## Authority and scope

This standard owns reusable composition and interaction for Orchbee-authored native TUI Settings:
metadata projection, categories and search, setting rows and controls, draft and preview state,
commit feedback, reset, conflict presentation, and return behavior. The domain that owns a setting
continues to own its identity, value semantics, valid domain, default, scope, precedence,
persistence, synchronization, side effects, security, and compatibility.

Cross-client meaning and apply timing follow [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md) and the
feature's common contract. The frontend adapter follows
[`frontend/settings.md`](../frontend/settings.md). Native actions follow [`actions.md`](actions.md),
input follows [`keyboard-input.md`](keyboard-input.md) and [`mouse-input.md`](mouse-input.md), controls
follow [`components.md`](components.md), and shell geometry, capture, status, and return follow
[`layout.md`](layout.md) and [`overlays-and-status.md`](overlays-and-status.md).

This is the required contract for new and materially changed TUI Settings behavior. It does not
assert that every current setting or surface already conforms. Record a staged gap with its owner and
completion condition; do not advertise unsupported behavior. Application-owned references, including
[`native-settings.md`](../../apps/orchbee/docs/native-settings.md), remain authoritative for current
catalogs, protocols, and persistence behavior.

## Admission and ownership

A setting is a durable, revisitable choice or policy that changes later product behavior. A command,
one-task answer, status, diagnostic, migration, credential exchange, resource collection, or
destructive data operation remains an action or dedicated workflow even when Settings links to it.
Do not show roadmap rows or disabled placeholders for unavailable future capability.

| Responsibility                                                                         | Owner                                        |
| -------------------------------------------------------------------------------------- | -------------------------------------------- |
| Stable identity, value domain, default, absence, scope, precedence, and source         | Setting's domain owner                       |
| Persistence, synchronization, validation, commit, side effects, and recovery           | Existing feature, protocol, or storage owner |
| Shared terminology, apply timing, effect horizon, and GUI/TUI correspondence           | Feature's common experience contract         |
| TUI categories, search projection, controls, draft state, preview, focus, and feedback | Native Settings composition                  |
| Geometry, visual roles, key decoding, pointer gestures, and overlay lifecycle          | Respective TUI adapter standards             |

A metadata catalog may improve consistency, but this standard does not require one registry, schema,
generic renderer, or new package. Reuse existing owner types and references. A generic projection
must not erase domain-specific validation, explanations, controls, or management tasks.

## Setting projection

Every setting shown in the TUI supplies or references enough metadata to render and operate without
reverse-engineering a storage record.

| Field              | Required decision                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------------------- |
| Identity and owner | Stable nonlocalized key, domain owner, and allowed writers                                              |
| Placement          | Stable category, optional section, localized label and concise description, and reviewed search aliases |
| Value              | Type, valid domain, canonical default, units, and absence semantics                                     |
| Scope and source   | Editable scopes, precedence, inheritance, enforcement, and effective-source projection                  |
| Availability       | Platform, capability, permission, policy, and dependency guards with reasons                            |
| Commit             | Immediate or atomic-draft policy, validation boundary, commit point, cancellation, and recovery         |
| Effect             | Current or future applicability, reload or restart requirement, and exceptional cost or external effect |
| Reset              | Whether reset reveals an inherited value, restores a product default, or is unavailable                 |
| Security           | Locality, sensitivity, retention, and whether the value belongs in an ordinary Settings draft           |

Metadata carries localization descriptors, stable canonical values, and typed bounds. Labels,
translated values, row positions, colors, and storage keys are not substitutes for semantic identity
or an effective-source projection.

## State model

Keep these facts distinct even when one control displays several of them:

| State                    | Meaning                                                                                          |
| ------------------------ | ------------------------------------------------------------------------------------------------ |
| Saved basis              | Confirmed values plus source or revision identity from which an atomic draft began               |
| Effective value          | Confirmed value currently governing behavior after precedence and capability resolution          |
| Draft value              | Uncommitted edit owned by the current setting task                                               |
| Preview value            | Reversible local presentation used to judge a draft; never proof of persistence or domain commit |
| Keyboard cursor or focus | Current navigation or editor position; never the saved, effective, or selected value             |
| Provisional choice       | Focused option in a picker before explicit acceptance                                            |
| Pending submission       | Frozen validated patch, source identity, and one execution identity accepted for commit          |
| External update          | A newer confirmed snapshot that either refreshes a clean view or conflicts with an active draft  |

Opening an atomic editor freezes a complete confirmed basis and source identity. Rendering projects
saved, effective, draft, preview, focus, and pending state explicitly; it never infers one from the
appearance of another. A confirmed domain update may change the effective value without moving the
keyboard cursor or provisional choice.

## Surface composition and discovery

Use a capturing overlay for a multi-setting draft unless the owning application admits a persistent
page or window. Compose it from the shared slots in
[`overlays-and-status.md`](overlays-and-status.md#overlay-composition): stable task header, optional
navigation and query, scrollable content, status, actions when needed, and a fixed footer.

- Organize categories by user goal and product vocabulary, not source modules or storage files.
  Category headers are landmarks, not value-changing controls.
- Keep the hierarchy shallow. A collection, credential lifecycle, repeated add/edit/remove task, or
  complex conflict flow opens a purpose-named child task with a visible route back.
- A child task records the parent category, focused setting, query, scroll anchor, draft, and preview.
  Back restores that navigation state and retains any accepted edit in the parent draft or domain
  according to its commit policy. It reverses only the child's active unaccepted edit or preview and
  never reconstructs the parent from defaults.
- A surface with multiple categories provides global search over localized category names, labels,
  concise descriptions, and reviewed aliases for available settings. Do not index values, secrets,
  user text, hidden capability, or diagnostics.
- Search is navigation only. It preserves the draft, uses deterministic all-term matching and
  ordering, skips orphan category headers, and exposes an explicit empty result. Activating a result
  reveals and focuses its row without changing the value.
- Keep task identity, the current return route, validation or conflict status, and commit action
  visible through scrolling and size-tier degradation. Refuse the surface at unsupported geometry
  instead of hiding its return or commit controls.

## Rows and typed controls

One setting row contains a persistent localized label, optional decision-relevant description,
effective or draft state, source or lock explanation when exceptional, and the smallest typed control
that expresses the value. Descriptions explain consequence, scope, dependency, security, effect
horizon, or recovery; they do not repeat the label or narrate ordinary key use.

| Value or intent                               | TUI control                                                                                      |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Independent boolean                           | Toggle that edits the declared draft or invokes the declared immediate action                    |
| Two or more finite choices                    | Picker with stable canonical values, a committed marker, and a separate navigation cursor        |
| Bounded integer                               | Stepper and exact value with units, bounds, and deterministic small and large steps              |
| Bounded string                                | Single-line editor with declared validation and length limit                                     |
| Color, theme, model, path, or runtime catalog | Domain picker with canonical value, unavailable-state handling, and preview only when reversible |
| Related child settings                        | Group row that opens one child task and restores the parent on return                            |
| Resource collection or credential             | Summary plus a named action into its dedicated owned workflow                                    |
| Reset or destructive operation                | Named action with the applicable confirmation and effect contract                                |

- Use the whole admitted control geometry for hover, focus, and activation; do not add unlabeled cell
  targets. Keyboard and pointer routes produce the same typed edit or action.
- Focus, hover, current effective value, provisional choice, changed draft, locked state, validation,
  and pending state remain visually distinct under [`visual-foundations.md`](visual-foundations.md).
- Keep unavailable values visible only when they explain current behavior or have a current recovery
  path. Show the reason; disabled styling alone is insufficient.
- Keyboard navigation and an activated search result reveal their current row. Manual pointer-wheel
  input moves only the viewport and never changes a draft, effective value, focus, or activation. If
  scrolling or resize makes the focused interactive target nonpainted, the component's declared
  policy moves focus to a painted fallback or vacates keyboard ownership without committing.
- A dynamically resolved value absent from the current catalog stays disclosed as unavailable or
  stale. Focusing a fallback option never silently clears the saved value.

## Draft, preview, and commit

The feature's common GUI/TUI contract owns apply timing. A presenter does not infer immediate commit
from a toggle or infer an atomic draft from the presence of an overlay.

- An editor that owns an atomic shared draft has one explicit `Save` boundary. Child edits, toggles,
  resets, and previews modify only that draft; navigation, filtering, scrolling, resize, and focus
  changes do not persist it.
- An immediate setting is valid only when its owner declares activation as the commit point and the
  control is outside any enclosing atomic draft that would promise a later Save. Never mix immediate
  writes into an atomic draft without a separate visible boundary and owning protocol.
- A preview is admitted only when it is local, reversible, and safe to apply repeatedly. The owner
  records the projection at entry, and moving the cursor may update it. Before acceptance, local
  return restores that entry projection. Acceptance ends the preview and either updates the parent
  draft or commits according to the declared policy; later return retains the accepted result and
  reverses only a newer unaccepted preview.
- Settings whose change sends a request, changes authorization, drains work, affects privacy, or may
  fail externally do not preview through navigation. Accept the choice before invoking their action.
- Validation runs through the value owner. Invalid input preserves the last confirmed effective
  value, keeps the draft available for correction when safe, explains the error beside the control,
  and focuses the first invalid editor after a Save attempt.
- Save requires a current successfully presented surface, a valid nonempty patch, the frozen source
  identity, and an unused submission authority. Acceptance freezes those facts, consumes authority
  once, and creates one pending execution. Repeated keys, clicks, or redraws cannot resubmit it.
- Closing before submission discards the atomic draft and reverses preview. Hiding after accepted
  Save releases presentation but does not cancel, replay, or roll back the commit.

The TUI sends a typed patch to the existing persistence boundary. It never sends a path, serialized
configuration document, or alternate storage selector unless that public owner explicitly requires
one. Only a confirmed snapshot becomes saved basis and may drive confirmed product effects. A disk
write receipt alone does not prove that every host effect applied.

## External updates and outcomes

A confirmed external update becomes the newest authoritative effective state. It refreshes a clean
view while preserving valid focus by semantic identity. A dirty draft, active editor, child task,
preview, or confirmation retains its original saved basis for comparison and shows the newer
effective value as a conflict; it is never silently rebased or overwritten. Reloading is an explicit
action that confirms discarding local work and reads a fresh complete basis.

| Outcome             | Required TUI behavior                                                                                                                       |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| Confirmed change    | Adopt the returned complete snapshot and source, clear committed draft state, and show the effective value and effect horizon               |
| Confirmed unchanged | Adopt the confirmed snapshot, clear the submitted patch, and report that no value changed without a success toast                           |
| Refused             | Preserve the draft and last confirmed effective value; show the localized reason and an owner-defined correction, reload, or retry route    |
| Unknown             | Preserve the draft and last confirmed effective value; state that commit is unknown and require fresh observation before another submission |

Submission completions carry the surface instance, execution identity, source identity, and request
generation. A completion for a replaced surface, older draft, prior editor episode, or stale request
cannot alter current values, preview, focus, feedback, or pending state. No refused or unknown request
is replayed automatically.

## Reset, dependencies, and sensitive workflows

- A per-setting reset names whether it removes an explicit override, reveals an inherited value, or
  stages the canonical product default. Reset uses the domain operation; it is not another default
  writer.
- Resetting a coherent section or atomic draft states the scope and affected values, requires
  confirmation when breadth is difficult to reverse, and remains draft state until its owning Save.
- Keep a dependent setting visible and unavailable when a nearby prerequisite can enable it. Show
  the dependency and preserve the confirmed effective value.
- A scope switch resolves any active draft before showing another scope. Never write to a scope
  inferred from the last visited category or row.
- Secrets and external credentials go directly to an approved bounded workflow and are cleared from
  UI memory afterward. They never enter ordinary drafts, search indexes, notices, diagnostics, or
  persisted UI state.
- Account deletion, credential revocation, retained-history deletion, migration, and diagnostics are
  named workflows with their own effect and recovery boundaries, not setting toggles.

## Verification

| Changed claim         | Required evidence                                                                                                                                                                 |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Metadata or admission | Tests cover identity, type and bounds, default and absence, scope/source, availability, apply timing, reset meaning, and both locales                                             |
| Composition or search | Cell-grid tests cover categories, descriptions, deterministic filtering, no orphan headers, empty results, child return, scrolling, and size tiers                                |
| Typed control         | Keyboard and pointer tests reach the same edit or action and distinguish effective value, draft, cursor, hover, lock, validation, and pending state                               |
| Preview               | Tests prove navigation-only preview, acceptance behavior, exact restoration on return, no persistence, and stale-preview rejection                                                |
| Atomic draft and Save | Tests prove edits remain local, only Save writes, validation blocks dispatch, one authority yields one request, cancel restores the basis, and hide does not cancel accepted work |
| External update       | Tests cover clean refresh, dirty conflict, explicit reload, semantic focus preservation, source comparison, and no silent overwrite                                               |
| Result handling       | Controlled completions cover confirmed change, confirmed unchanged, refusal, unknown, stale generation, no replay, and effect-horizon copy                                        |
| Reset or dependency   | Tests cover inherited versus default reset, reset remaining draft state, confirmation, unavailable reason, scope safety, and sensitive-value exclusion                            |

Use render, input, resize, locale, reduced-color, and supported-host evidence under
[`verification.md`](verification.md). Model competing writers, compare-and-set publication, or unknown
commit outcomes when their risk meets
[`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md); ordinary row rendering does not
require a separate formal model.
