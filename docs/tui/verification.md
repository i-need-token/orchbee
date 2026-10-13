# TUI Verification

## Authority and scope

This standard owns the evidence required for changes to Orchbee-authored terminal interfaces. It
applies the repository test obligations in [`TESTING.md`](../TESTING.md) and
[`RUST.md`](../RUST.md#testing) to the cell grid and selects the evidence classes the other `tui/`
standards reference. Change classification and modeling follow
[`DEVELOPMENT.md`](../DEVELOPMENT.md#change-classification) and
[`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md); a new mode, gesture, overlay
lifecycle, or ownership change is Modeled work.

## Structural prerequisites

Evidence below is only possible when the interface keeps these boundaries:

| Responsibility | Contract                                                                                   |
| -------------- | ------------------------------------------------------------------------------------------ |
| State          | Plain data with no handles to the terminal, the network, or the clock                      |
| Decision       | A pure function from one input event and the current state to at most one typed intent     |
| Layout         | A pure function from geometry and state to regions and recorded hit rectangles             |
| Render         | A pure function from state, regions, theme, and locale to a buffer; no mutation and no I/O |
| Effects        | Applied by the owner after the decision, never inside decision, layout, or render          |
| Time           | Injected; no reading of the wall clock inside decision, layout, or render                  |

A change that pulls I/O, time, or mutation into decision, layout, or render is refused in review
regardless of its tests.

## Contract review

Before implementation, review the owning contract against
[`components.md`](components.md), [`actions.md`](actions.md), and [`settings.md`](settings.md) as
applicable. Identify the existing component to reuse or the justified extension, replacement, or
new local component; specify state axes, slots, commit and return boundaries, effective bindings,
hit targets, degradation, and effect ownership. Review common GUI/TUI semantics through
[`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md). Keep this decision with the existing owning
reference, types, or tests, not a separate task catalog.

A standards-only change is accepted through semantic, bilingual, link, and applicable repository
checks. It does not establish runtime conformance. Each affected implementation must supply the
evidence below before claiming adoption. When using a reference product, distinguish observed
behavior, source behavior at a named revision, and the proposed Orchbee requirement; differences
between the installed product and source are not evidence that they implement the same revision.

## Render evidence

- Render through `ratatui::backend::TestBackend` and compare the buffer as text with an expected
  value kept inline in the test or in a fixture beside it. Adding a snapshot library is a dependency
  decision under [`DEVELOPMENT.md`](../DEVELOPMENT.md#decision-authority).
- Every surface renders at these geometries: 80×24, 120×40, one Compact and one Minimal geometry
  from [`layout.md`](layout.md#size-tiers), and the too-small geometry.
- Every surface renders under the `terminal-native` theme and one RGB theme, and under `en-US` and
  `zh-CN`. Wide-character fixtures are present wherever text truncates or wraps.
- Changed controls cover applicable rest, hover, pressed, focus, current, provisional, checked,
  disabled, pending, and outcome states. Include meaningful combinations from
  [`components.md`](components.md#state-model), rather than every Cartesian combination.
- Style assertions check roles through the theme, not literal colors, except the quantization test
  that proves a 16-color level yields no `Rgb` or `Indexed` cell. Also exercise `NO_COLOR` and
  collapsed host-default colors: focus, current context, checked value, provisional selection,
  availability, and operation status remain distinguishable through their required markers, text,
  or modifiers. Absence of RGB cells alone does not prove usable contrast.
- An expected-text update is reviewed as a visual diff. A snapshot never changes without the
  reviewer seeing the before and after rows.

## Whole-screen visual review

Presentation changes also require review of the final composed screen as an image or terminal
capture, starting with the default theme. Compare equivalent geometry, locale, data and input
ownership; review changed regions together with surrounding navigation, content and status.
Check current-context recognition, focus distinction, text hierarchy, duplicate labels and actions,
empty states, grouping, and the space available to primary content. Include the affected ordinary,
navigation and split/zoom states; a component render alone does not establish whole-screen quality.
Review at-rest action affordance, hover without focus movement, stable label tracks, complete
primary and exit hints at narrow geometry, and the identity of the operation's target.

The application owns its accepted screen composition and observable visual invariants. Later
feature work must preserve those outcomes or explicitly revisit the owning decision. Tests protect
mechanically checkable outcomes; updating expected rows or passing role assertions alone does not
accept a visual regression. Record the comparison and judgment in the active review, not a new
tracked task report.

Identify the image source. A rendered buffer preview can establish composition, but cannot establish
a physical host's palette, font, contrast or glyph rendering. Claims about those properties require
a real-host capture with the host evidence below; report them as unverified when unavailable.

## Input evidence

| Class        | Required cases                                                                                                                                                                                                               |
| ------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Decoding     | Every C0 normalization, uppercase implies `Shift`, `Ctrl+]`, lone `ESC`, split sequences, atomic paste                                                                                                                       |
| Binding      | Match, non-match with extra modifiers, host labels, effective overrides where supported, shadowing, and displayed hints matching the active definitions                                                                      |
| Ownership    | Table-driven precedence for every owner pair; unconsumed bytes pass through in order; hidden owners and stale targets cannot act                                                                                             |
| Mode         | Indicator visible, local `Esc` step, unbound keys ignored, modes do not nest, and pane `Esc` remains protected without a named owner                                                                                         |
| Editor       | Every editing key, length bound, prefill, atomic paste, composition not consumed, and draft/cursor preservation across temporary tasks                                                                                       |
| Confirmation | Only the exact key confirms, `Esc` cancels, trailing bytes in the decision packet do not reach the terminal                                                                                                                  |
| Pointer      | Last successfully flushed hit map, child-target precedence, exact pane event-class filtering, hover without selection, same-target release, leave/capture-loss cancellation, stale-map invalidation, and wheel normalization |

Input tests drive typed events through the decision function and assert the whole resulting intent
and state, not individual fields.

## Lifecycle evidence

| Changed contract     | Required cases                                                                                                                                                                                                                                        |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Semantic action      | Keyboard, pointer, and discovery paths resolve the same action and typed target; eligibility is rechecked before effects; repeated invocation follows declared concurrency; each execution has one classified result                                  |
| Nested surface       | Child return restores query, selection, logical scroll anchor, draft, preview, and valid focus; root close restores the invoker or declared fallback; closing does not silently cancel accepted work                                                  |
| Preview and settings | Saved, effective, draft, preview, and cursor values stay distinct; preview cancellation restores its entry value; validation/refusal preserves the draft; Save, reset, external updates, and unknown write results follow the owner's commit contract |
| Async update         | A replaced request, removed target, closed/reopened surface, and stale generation cannot mutate a new view or take focus; valid results settle through the semantic owner                                                                             |
| Viewport             | Resize, filtering, insertion, removal, and wrapped-content changes preserve the declared identity or logical content anchor; explicit bottom following is distinct from manual reading                                                                |

Use injected time and controlled completion order. Do not replace stale-result tests with sleeps or
assertions that only the last rendered label looks correct.

## Environment evidence

- Mode sequences or platform-adapter calls are tested for enable, restore, reverse order, partial
  acquisition, repeated teardown, and every step running after an earlier failure. Include normal,
  error, panic, supported signal, and external-program handoff paths.
- Capability queries are tested with a timely reply, a late reply, no reply, and a reply interleaved
  with key input.
- Environment-variable precedence is table-driven and includes `NO_COLOR`.
- The output loop is tested for one flush per cadence interval, one synchronized-update pair per
  frame, effects only between frames, no replay after an unknown write, and bounded progress for
  input under output and pointer-motion bursts. Hover redraws occur only on resolved-target changes
  and follow the admitted capture and cadence policy.

## Host evidence

Some behavior exists only in a real emulator: keyboard-protocol negotiation, pointer passthrough
encoding, appearance reports, and multiplexer interception.

- Exercise it through a pseudo-terminal harness that drives the Client binary and asserts on the
  bytes it writes, or through a documented manual run.
- Record the emulator name and version, the multiplexer if any, the operating system, geometry,
  locale, theme, and negotiated capabilities with the evidence. A PTY or ConPTY byte replay proves
  protocol behavior and cell composition; it does not prove physical host palette or font quality.
- Host evidence supplements unit evidence; it never replaces the classes above.

## Change checklist

| Change                      | Evidence                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------- |
| New surface                 | Render evidence at every geometry, theme, and locale; ownership and footer tests when it captures input |
| Component or composition    | Contract review, applicable state combinations, consumer adoption, and whole-screen visual review       |
| Theme role or value         | Re-render of every consumer; quantization and non-color distinguishability                              |
| Binding or mode             | Binding, ownership, and mode classes; help reflects effective definitions and local key maps            |
| Gesture or region           | Pointer class; keyboard equivalent proven                                                               |
| Action or settings          | Applicable lifecycle cases, semantic target, commit, failure, and concurrency evidence                  |
| Terminal mode or query      | Environment class; host evidence for negotiation                                                        |
| Layout tier or degradation  | Render evidence per tier; one test per degradation step                                                 |
| Notice or pending indicator | Injected-time tests for expiry, replacement, and constant width                                         |
| Repository gate             | The [applicable root gate](../VERIFICATION.md#merge-gate) passes                                        |
