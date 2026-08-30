# Frontend Dialogs and Forms

## Authority and scope

This standard owns surface selection and interaction behavior for Orchbee-authored product flows
that ask a person to decide, confirm, or enter data. It governs inline editors, anchored
disclosures, persistent panels, form Dialogs, Alert Dialogs, and dedicated task surfaces across
browser and Desktop React interfaces.

Visual geometry follows [`visual-foundations.md`](visual-foundations.md), reviewed compositions
follow [`experience-components.md`](experience-components.md), semantic effects follow
[`actions.md`](actions.md), and product copy follows [`localization.md`](localization.md). A `Dialog`,
`Sheet`, `Popover`, or `Form` primitive provides mechanics; its name never proves that the surface
is appropriate for the task.

## Interruption budget

- Start with no new surface and no question. Invoke a reversible action directly when authoritative
  context or a safe default already determines the result; provide contextual feedback or undo when
  needed.
- Ask only for information required by the current outcome. Reuse authoritative values already
  supplied in the journey, derive safe values, and defer editable metadata.
- Preserve the person's current context. Background status, education, optional setup, success, and
  recoverable background failure never open a modal or steal focus.
- Open an interrupting surface only from the action or event that needs the response. Do not present
  recurring prompts merely because a capability is available.
- Choose the lowest-interruption surface that can preserve the task's state, consequence, and
  accessibility requirements. Implementation convenience is not an admission reason.

Independent reversible settings apply at their owning control and expose reset or undo. Use an
explicit submit boundary only when several values form one atomic configuration, validation must
consider them together, or submission starts an external effect.

## Surface selection

| Surface                     | Use when                                                                                       | Avoid when                                                                                     |
| --------------------------- | ---------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| No added surface            | Existing context and a safe default determine a reversible result                              | The outcome is ambiguous or has a consequence that must be reviewed                            |
| Inline editor or disclosure | One contextual value can be changed without hiding the object being edited                     | The task needs an atomic multi-value draft or exclusive attention                              |
| Menu or anchored Popover    | A small finite choice or one short contextual input belongs to a visible trigger               | Content is long, high-risk, multistep, or must survive interaction elsewhere                   |
| Persistent panel or page    | People need to compare context, tune optional settings, revisit values, or continue other work | The parent must remain inert until one bounded decision completes                              |
| Modal Dialog                | One explicit task meets the [modal admission](#modal-admission) contract                       | The surface merely reports information, collects convenience metadata, or contains a long flow |
| Dedicated task surface      | The task is long, multistep, navigable, or needs substantial context and recovery              | One brief decision can safely return to the current context                                    |

Any surface that makes the rest of the application inert is modal, regardless of whether its
component is named Dialog, Sheet, Drawer, or Popover. Responsive presentation may change geometry,
but it preserves modality, commit, cancellation, and focus behavior.

## Direct manipulation and expert entry

When one semantic value supports both direct selection and precise text entry, keep both paths in
one stable task surface:

- Default to the lower-effort direct representation, such as breadcrumbs and a folder list. Reveal
  text entry only when the person requests it.
- Replace the direct representation in place with its editor. Do not append a second form row, open
  another overlay, or move the surrounding content and actions merely to expose expert input.
- Reveal the edit affordance on both hover and keyboard focus. Its button has a localized accessible
  name; a Tooltip may reinforce that name but never provides the only discovery path.
- Keep the last canonical value separate from the text draft. Typing may filter or suggest bounded
  candidates, but it does not commit, navigate, or erase the last valid view by itself.
- Treat candidate results as live validation. A partial or invalid draft keeps the last valid
  context visible and explains why the primary action is unavailable; disabled styling alone is not
  an explanation.
- Enter accepts an exact or explicitly highlighted candidate only when the outcome is unambiguous.
  Escape leaves text-entry mode and restores the canonical representation; the task's Cancel action
  still cancels the complete task.
- Coalesce asynchronous candidate requests and bind results to the latest draft and source scope. A
  stale response never replaces newer candidates, selection, or canonical context.

## Modal admission

A modal requires at least one of these reasons:

- the parent must not change while one atomic draft is reviewed and committed;
- an explicit foreground operation cannot continue without an immediate response, and ignoring or
  postponing that response is not a valid outcome; or
- a brief decision protects against a severe consequence that is not readily reversible.

Every admitted modal also has one bounded semantic outcome, a short path to completion, an explicit
safe exit before commit, and enough context inside the surface to make the decision. Move a task to
a persistent panel or dedicated surface when people must inspect or manipulate the obscured parent,
compare substantial information, navigate a hierarchy, or return repeatedly.

Do not use a modal for passive information, success acknowledgement, ordinary validation errors,
optional onboarding, reminders, routine single-value edits, or progress that can safely continue in
the background. Do not stack modals. A temporary subtask replaces the current modal body and actions
under the same owner, or the first modal closes before another opens.

Prefer direct execution with undo for readily reversible actions. Use an Alert Dialog only when a
brief important message requires a decision, typically before an irreversible or difficult-to-
reverse effect. An Alert Dialog contains no ordinary form, progress workflow, or informational
acknowledgement; its least destructive action receives initial focus when accidental confirmation
would be costly.

## Compact Desktop form-dialog typography

A compact Desktop form Dialog does not inherit the larger generic primitive typography as its
product hierarchy. Use this composition consistently across its editing, prompt, progress, and
recoverable-failure states:

| Part                               | Typography role                  | Presentation rule                                                             |
| ---------------------------------- | -------------------------------- | ----------------------------------------------------------------------------- |
| One state-specific title           | `--ob-sys-typography-heading-xs` | Normal weight; position and spacing establish hierarchy                       |
| Description, help, error, progress | `--ob-sys-typography-body-sm`    | Normal weight; semantic color establishes priority or status                  |
| Field labels and footer actions    | `--ob-sys-typography-control-sm` | Normal weight; control geometry and primary surface establish emphasis        |
| Code-like input and exact prompt   | `--ob-sys-typography-code-sm`    | Use only when the value is genuinely code-like                                |
| Supplemental key hint              | `--ob-sys-typography-label-xs`   | Subordinate to the localized action label and hidden from its accessible name |

Do not increase weight merely because text is a title, field label, or primary action. Reserve
stronger label and heading roles for a concrete hierarchy that remains ambiguous after position,
spacing, semantic color, and control surface are applied. A nested step changes the copy and
actions, not this type hierarchy.

## Question budget

Classify every proposed value before designing controls:

| Classification | Treatment                                                                                         |
| -------------- | ------------------------------------------------------------------------------------------------- |
| Required now   | Ask only when the current outcome cannot be validly produced without a person-supplied value      |
| Derived        | Compute from authoritative context and do not ask the person to repeat or confirm it              |
| Defaulted      | Apply a safe, unsurprising default; expose it only when changing it affects the current decision  |
| Deferred       | Omit from the flow and let the owning resource or Settings surface edit it later                  |
| Optional now   | Show only when it materially changes the immediate result and a reasonable person may need it now |

If an optional value can be changed later without loss, cost, or a materially different result, it
is deferred rather than displayed. Group remaining optional-now values after the primary path under
a purpose-named disclosure that starts closed. Do not use “Advanced” as a substitute for explaining
the effect. A disclosure preserves its draft while open and collapsed during one interaction;
cancel discards the complete draft.

Do not render known context as a disabled or read-only form control. Present it as text, metadata,
or a summary, with a separate copy action when copying is useful. An empty value may represent an
intentional restore-to-default operation only when the label and supporting text make that outcome
clear.

## Field contract

- Give every control a persistent visible label. Append a localized “(optional)” to an individually
  visible optional field unless an enclosing titled group already states that the complete group is
  optional. Never mark required fields with an asterisk.
- Write labels as the shortest unambiguous question or value name. Supporting text explains only a
  non-obvious reason, consequence, format, privacy boundary, or recovery rule.
- A placeholder is optional. It never replaces a label, carries persistent instructions, or exists
  merely to fill an empty control; prefer a visible example when the example remains useful after
  entry.
- Prepopulate authoritative current values. Do not ask a person to re-enter information Orchbee
  already knows unless security requires fresh proof.
- Choose the semantic control and attributes for the value, including `type`, `autocomplete`,
  `inputmode`, and `spellcheck` where applicable. Required state and validation relationships are
  programmatically exposed.
- Secret responses state where they are sent and whether they are stored. They remain only in the
  input until direct submission and are cleared on submission, cancellation, failure, and unmount.

## Validation and actions

- Keep one primary action in the current task. Its localized label names the outcome, such as
  “Connect” or “Save”; use “Continue” only when it advances to another visible step.
- Place the non-destructive cancel action before the primary action. A close control, Escape,
  backdrop dismissal, and Cancel have the same no-commit meaning whenever those paths are enabled.
- Do not disable submit as the only validation explanation. When a person can reasonably attempt
  submission, accept the attempt, reveal localized field errors, and focus the first invalid field.
  Disable only for an active duplicate submission or an unmet prerequisite already explained in the
  surface.
- Validate after a meaningful interaction such as blur or submit. Do not show an error before the
  person can answer, clear an error while its condition is repaired, or expose raw exceptions.
- Preserve non-secret input after a recoverable failure. Submission commits the complete validated
  draft once; cancellation before commit produces no semantic mutation.
- Preserve native form behavior. Enter submits only when the focused control, input method editor,
  and current step make submission unambiguous; it never submits from a multiline response.
- In a compact Desktop form Dialog, show `Kbd` hints inside the corresponding footer buttons when
  Escape always cancels the current task and unmodified Enter always invokes the current single-line
  form's submission path. The hints are supplemental and hidden from the accessible name because the
  localized action label already names the outcome.
- A visible key hint is a claim about the step's default editing context, not an override of native
  control activation. Omit it when composition, multiline input, confirmation safety, or workflow
  state can make the key invoke a different action or no action from the initial or editable focus
  target. An explicitly focused button continues to receive native Enter activation.

## Cancellation and asynchronous work

Classify submitted work as foreground, background, or external under
[`actions.md`](actions.md#asynchronous-lifecycle). A modal remains open for foreground work only
when leaving would make the outcome unsafe or unobservable. If accepted work can continue safely,
close the modal and report progress through the owning nonintrusive activity surface.

- Define the commit point. Before it, every enabled dismissal requests cancellation and releases
  operation-owned resources; after it, the UI never relabels committed work as cancelled.
- Prevent duplicate work by semantic operation identity and scope, not by a process-wide or
  component-global “dialog busy” flag.
- Cancellation, close, failure, transport loss, and unmount are idempotent cleanup paths. None may
  leave a lease or lock that blocks a fresh invocation.
- A late completion cannot reopen a surface, steal focus, overwrite a newer draft, or navigate away
  from later work. Publish it only through the still-owning interaction token or a nonintrusive result
  channel.
- Reopening after cancellation starts from current authoritative state unless the feature explicitly
  owns a resumable draft. Never restore a secret response.
- Do not add discard confirmation for a cheap, easily recreated draft. When closing would lose
  substantial user-authored content, explain that consequence and offer explicit continue-editing
  and discard outcomes.

## Focus, keyboard, and accessibility

- Use the shared primitives for modal semantics, inert background, focus containment, and accessible
  naming. Do not hand-roll a focus trap or mark a surface modal when pointer or keyboard interaction
  can still reach the parent.
- Place initial focus deliberately: the first unresolved required control for a short form, a static
  heading or introduction when structured content must be read first, or the least destructive
  action for a costly irreversible decision.
- Tab and Shift+Tab remain inside a modal. Escape closes the innermost cancellable task. Closing
  restores focus to the invoker or to the logical result created by the completed action when the
  invoker no longer exists.
- Keep a visible close or cancel control in the tab order. A Dialog has a visible localized title;
  associate a concise description only when it can be announced as one understandable unit.
- Connect hint and error text to its control. Announce asynchronous progress politely and use an
  alert only for a failure that requires immediate attention.
- Verify pointer, keyboard, assistive-technology, coarse-pointer, narrow-viewport, zoom, and both
  shipped-locale behavior without changing the semantic order.

## Modeling and evidence

Before implementing a new or materially changed form or interrupting surface:

1. State the user intent and why each lower-interruption surface is insufficient.
2. Record every proposed value with its question-budget classification and remove derived,
   defaulted, or deferred controls from the primary path.
3. Run the experience-component discovery workflow and choose reuse, extend, replace, or new. A
   reusable Dialog composition does not itself satisfy modal admission.
4. Model applicable closed, editing, disclosed, invalid, submitting, cancelling, completed, failed,
   stale-completion, and re-entry transitions, including alternate-input modes, canonical versus
   draft values, the commit point, and effect owner.
5. Map Stories and tests to surface selection, both locales, initial and restored focus, keyboard
   paths, validation timing, direct-selection and text-entry parity when applicable, stale candidate
   results, duplicate submission, every cancellation path, recoverable failure, late completion,
   and clean re-entry.

The owning Manifest records `useWhen`, `avoidWhen`, and invariants that preserve the chosen surface,
question budget, cancellation, and focus behavior. Existing surfaces receive this review when they
are materially changed; a touched surface must not expand a nonconforming interruption or field
pattern.

## References

- [WAI-ARIA Authoring Practices: Dialog (Modal) Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)
- [Apple Human Interface Guidelines: Modality](https://developer.apple.com/design/human-interface-guidelines/modality)
- [GOV.UK Design System: Question pages](https://design-system.service.gov.uk/patterns/question-pages/)
- [GOV.UK Design System: Text input](https://design-system.service.gov.uk/components/text-input/)
