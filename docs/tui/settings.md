# TUI Settings

## Authority and scope

This standard owns reusable composition and interaction for Orchbee-authored native TUI Settings:
metadata projection, the page and its categories and search, setting rows and controls, preview,
per-change commit feedback, reset, external updates, and return behavior. The domain that owns a setting
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

| Responsibility                                                                  | Owner                                        |
| ------------------------------------------------------------------------------- | -------------------------------------------- |
| Stable identity, value domain, default, absence, scope, precedence, and source  | Setting's domain owner                       |
| Persistence, synchronization, validation, commit, side effects, and recovery    | Existing feature, protocol, or storage owner |
| Shared terminology, apply timing, effect horizon, and GUI/TUI correspondence    | Feature's common experience contract         |
| TUI page, categories, search projection, controls, preview, focus, and feedback | Native Settings composition                  |
| Geometry, visual roles, key decoding, pointer gestures, and overlay lifecycle   | Respective TUI adapter standards             |

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
| Commit             | Commit point, validation boundary, cancellation, and recovery                                           |
| Effect             | Current or future applicability, reload or restart requirement, and exceptional cost or external effect |
| Reset              | Whether reset reveals an inherited value, restores a product default, or is unavailable                 |
| Security           | Locality, sensitivity, retention, and whether the value belongs in an ordinary Settings row             |

Metadata carries localization descriptors, stable canonical values, and typed bounds. Labels,
translated values, row positions, colors, and storage keys are not substitutes for semantic identity
or an effective-source projection.

## State model

Keep these facts distinct even when one control displays several of them:

| State                    | Meaning                                                                                       |
| ------------------------ | --------------------------------------------------------------------------------------------- |
| Confirmed value          | Value in the newest confirmed snapshot, with its revision and whether the file sets it        |
| Effective value          | Confirmed value currently governing behavior after precedence and capability resolution       |
| Uncommitted edit         | Typed text or a stepped value in a row's inline editor before Enter; never sent on its own    |
| Preview value            | Reversible local presentation used to judge a candidate; never proof of persistence or commit |
| Keyboard cursor or focus | Current navigation or editor position; never the confirmed, effective, or selected value      |
| Provisional choice       | Focused option in a picker before explicit acceptance                                         |
| Pending change           | The one accepted change whose request is in flight, shown on its own row                      |
| External update          | A newer confirmed snapshot published by another writer while the page is open                 |

Rendering projects confirmed, effective, uncommitted, preview, focus, and pending state explicitly; it
never infers one from the appearance of another. A confirmed update may change the effective value
without moving the keyboard cursor or provisional choice.

## Surface composition and discovery

Settings is a full page that captures the whole client area while open. A wide page shows a
navigation column (a route back, search, and categories) beside a content column (category title,
headed groups, and rows); below the page's narrow width it shows one column at a time, and Back steps
from rows to categories before it leaves the page. The page's last row is its status line: it names
the keys for the focused row and is replaced by a Settings notice while one is shown. Refuse the page
at a size below its minimum instead of hiding its route back.

- Organize categories by user goal and product vocabulary, not source modules or storage files.
  Category headers are landmarks, not value-changing controls.
- Keep the hierarchy shallow. A collection, credential lifecycle, repeated add/edit/remove task, or
  complex conflict flow opens a purpose-named child task with a visible route back.
- A child task records the parent category, focused setting, query, scroll anchor, and preview. Back
  restores that navigation state and keeps any committed change. It reverses only the child's active
  unaccepted edit or preview and never reconstructs the parent from defaults.
- A surface with multiple categories provides global search over localized category names, labels,
  concise descriptions, and reviewed aliases for available settings. Do not index values, secrets,
  user text, hidden capability, or diagnostics.
- Search is navigation only. It changes no value, uses deterministic all-term matching and
  ordering, skips orphan category headers, and exposes an explicit empty result. Activating a result
  reveals and focuses its row without changing the value.
- Keep the page title, the current return route, and the status line visible through scrolling and
  size-tier degradation.

## Rows and typed controls

One setting row contains a persistent localized label, a one-line description, the effective or
pending state, a changed marker when the file sets the value, source or lock explanation when
exceptional, and the smallest typed control at the row's end. Descriptions explain consequence,
scope, dependency, security, effect horizon, or recovery; they do not repeat the label or narrate
ordinary key use. Settings rows show their descriptions at all times, matching the Desktop page; this
is a Settings-only exception to moving hover explanations into tooltips, because a row's description
is the decision context, not an explanation of a control.

| Value or intent                               | TUI control                                                                                      |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------ |
| Independent boolean                           | Toggle that commits on activation                                                                |
| Two or more finite choices                    | Picker with stable canonical values, a committed marker, and a separate navigation cursor        |
| Bounded integer                               | Stepper and exact value with units, bounds, and a deterministic step                             |
| Bounded string                                | Single-line editor with declared validation and length limit                                     |
| Color, theme, model, path, or runtime catalog | Domain picker with canonical value, unavailable-state handling, and preview only when reversible |
| Related child settings                        | Group row that opens one child task and restores the parent on return                            |
| Resource collection or credential             | Summary plus a named action into its dedicated owned workflow                                    |
| Reset or destructive operation                | Named action with the applicable confirmation and effect contract                                |

- Use the whole admitted control geometry for hover, focus, and activation; do not add unlabeled cell
  targets. Keyboard and pointer routes produce the same typed edit or action.
- Focus, hover, current effective value, provisional choice, changed row, locked state, validation,
  and pending state remain visually distinct under [`visual-foundations.md`](visual-foundations.md).
- Keep unavailable values visible only when they explain current behavior or have a current recovery
  path. Show the reason; disabled styling alone is insufficient.
- Keyboard navigation and an activated search result reveal their current row. Manual pointer-wheel
  input moves only the column under the pointer and never changes a value, focus, or activation. If
  scrolling or resize makes the focused interactive target nonpainted, the component's declared
  policy moves focus to a painted fallback or vacates keyboard ownership without committing.
- A dynamically resolved value absent from the current catalog stays disclosed as unavailable or
  stale. Focusing a fallback option never silently clears the confirmed value.

## Commit and preview

Every change applies at once. There is no page draft and no Save; one accepted change is one request
to the persistence boundary, naming only that setting.

- A toggle or a picker choice commits when activated. A text or number row edits in place and commits
  on Enter; Esc keeps the confirmed value. Navigation, filtering, scrolling, resize, and focus changes
  never commit.
- A coherent multi-value task, such as a collection child page, commits as one change when it is
  accepted or left, and an invalid result is corrected or discarded rather than sent.
- One change is in flight at a time. Its row shows the pending state while every other row, the
  navigation, and the route back stay usable; another change waits until the pending one settles.
  Repeated keys, clicks, or redraws cannot resubmit it.
- A preview is admitted only when it is local, reversible, and safe to apply repeatedly. Moving the
  cursor in an open picker or over an uncommitted option may update it; Esc or return restores the
  confirmed projection. Acceptance ends the preview and commits.
- Settings whose change sends a request beyond the settings file, changes authorization, drains work,
  affects privacy, or may fail externally do not preview through navigation.
- Validation runs through the value owner. Invalid input keeps the confirmed value and explains the
  reason in the Settings notice.
- Closing the page discards only an uncommitted inline edit or preview; a change already sent is
  neither cancelled, replayed, nor rolled back.

The TUI sends a typed patch to the existing persistence boundary. It never sends a path, serialized
configuration document, or alternate storage selector unless that public owner explicitly requires
one. Only a confirmed snapshot becomes the confirmed value and may drive confirmed product effects. A disk
write receipt alone does not prove that every host effect applied.

## External updates and outcomes

A confirmed external update becomes the newest authoritative effective state and shows on the open
page at once, preserving focus by semantic identity. A pending row keeps its pending state until its
own outcome settles. A settings file that cannot be read or parsed is named on the page; rows keep the
last valid values and refuse changes with the reason until the file is usable again.

| Outcome   | Required TUI behavior                                                                                                      |
| --------- | -------------------------------------------------------------------------------------------------------------------------- |
| Confirmed | Adopt the returned snapshot and show the effective value and effect horizon; no success notice                             |
| Refused   | Put the row back to the confirmed value and show the localized reason on the Settings status line                          |
| Unknown   | Put the row back to the confirmed value, state that the change was not confirmed, and wait for a fresh snapshot to show it |

Submission completions carry the surface instance, execution identity, source identity, and request
generation. A completion for a replaced surface, prior editor episode, or stale request
cannot alter current values, preview, focus, feedback, or pending state. No refused or unknown request
is replayed automatically.

## Reset, dependencies, and sensitive workflows

- A per-setting reset is available on a changed row and names whether it removes an explicit
  override, reveals an inherited value, or restores the canonical product default. Reset uses the
  domain operation and commits at once; it is not another default writer. Do not offer a page-wide
  reset.
- Keep a dependent setting visible and unavailable when a nearby prerequisite can enable it. Show
  the dependency and preserve the confirmed effective value.
- A scope switch settles any pending change before showing another scope. Never write to a scope
  inferred from the last visited category or row.
- Secrets and external credentials go directly to an approved bounded workflow and are cleared from
  UI memory afterward. They never enter ordinary rows, search indexes, notices, diagnostics, or
  persisted UI state.
- Account deletion, credential revocation, retained-history deletion, migration, and diagnostics are
  named workflows with their own effect and recovery boundaries, not setting toggles.

## Verification

| Changed claim         | Required evidence                                                                                                                                  |
| --------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| Metadata or admission | Tests cover identity, type and bounds, default and absence, scope/source, availability, apply timing, reset meaning, and both locales              |
| Composition or search | Cell-grid tests cover categories, descriptions, deterministic filtering, no orphan headers, empty results, child return, scrolling, and size tiers |
| Typed control         | Keyboard and pointer tests reach the same edit or action and distinguish effective value, uncommitted edit, cursor, hover, lock, and pending state |
| Preview               | Tests prove navigation-only preview, acceptance behavior, exact restoration on return, no persistence, and stale-preview rejection                 |
| Instant apply         | Tests prove one accepted change sends one request naming its setting, only its row waits, navigation stays free, and a refusal puts the row back   |
| External update       | Tests cover immediate refresh, semantic focus preservation, a pending row during refresh, and an unusable file                                     |
| Result handling       | Controlled completions cover confirmed change, refusal, unknown, stale generation, no replay, and effect-horizon copy                              |
| Reset or dependency   | Tests cover inherited versus default reset, reset only on changed rows, unavailable reason, scope safety, and sensitive-value exclusion            |

Use render, input, resize, locale, reduced-color, and supported-host evidence under
[`verification.md`](verification.md). Model competing writers, compare-and-set publication, or unknown
commit outcomes when their risk meets
[`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md); ordinary row rendering does not
require a separate formal model.
