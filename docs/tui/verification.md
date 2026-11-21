# TUI Verification

## Authority and scope

This standard states what must hold after a change to an Orchbee-authored terminal interface and how
to show it. The other `tui/` standards own the rules; [`TESTING.md`](../TESTING.md) decides which
tests are admitted, and [`MODELING-AND-CORRECTNESS.md`](../MODELING-AND-CORRECTNESS.md) says when a
new mode, gesture, overlay lifecycle, or ownership change needs a model. Nothing here is a list of
test cases that each change must add.

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

## Outcomes

A change is complete when these hold for the surfaces and contracts it changed:

| Outcome                 | Holds when                                                                                                                                                                                                                                              |
| ----------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Both locales            | Every changed surface shows its complete content under `en-US` and `zh-CN`; wide characters occupy their display width wherever text truncates or wraps.                                                                                                |
| One experience          | Keyboard, pointer, menu, and command routes reach the same action, typed target, availability, and commit point, and the GUI counterpart keeps the same meaning under [`INTERFACE-DESIGN.md`](../INTERFACE-DESIGN.md).                                  |
| Geometry                | A surface keeps its required content, controls, and hints at every [size tier](layout.md#size-tiers) that admits it and gives way to the too-small notice below them; nothing paints outside its region.                                                |
| Hit regions             | Recorded hit rectangles match the painted targets of the last presented frame; a hidden, occluded, or stale target cannot be activated.                                                                                                                 |
| State cues              | Focus, current context, checked value, provisional selection, availability, and operation status stay distinguishable under `terminal-native`, an RGB theme, 16-color output, and `NO_COLOR`, through the non-color cue their owning standard requires. |
| Input ownership         | Each input has one owner; unconsumed bytes reach the pane in order; a hidden owner cannot act; `Esc` unwinds one local step; a confirmation accepts only its exact key and passes no trailing byte to the terminal.                                     |
| Return and cancellation | Returning, closing, or cancelling restores the query, selection, scroll anchor, draft, preview, and focus its owner declares, and does not cancel accepted work.                                                                                        |
| Late results            | A replaced request, removed target, closed surface, or stale generation cannot change a newer view or take focus.                                                                                                                                       |
| Viewport                | Resize, filtering, insertion, and removal keep the declared identity or content anchor; following the bottom stays distinct from manual reading.                                                                                                        |
| Terminal state          | Every mode the Client acquired is restored on normal exit, error, panic, supported signal, suspend, and external handoff, also after an earlier restore step fails.                                                                                     |
| Output                  | Text of external origin places no control character or escape sequence in the buffer; a frame is one synchronized update; nothing is replayed after an unknown write.                                                                                   |

Show an outcome at the lowest layer that can judge it:

- Drive typed events through the decision function and assert the whole resulting intent and state.
- Render through `ratatui::backend::TestBackend` and assert invariants over the buffer. Compare
  complete expected text only for an owned contract under
  [design-pinning assertions](../TESTING.md#design-pinning-assertions); when an intended change
  breaks such an assertion, that section decides between deleting it and reviewing the text update.
  Check styles through theme roles, not literal colors.
- Use injected time and a controlled completion order. Never sleep, and never accept "the last
  rendered label looks right" in place of a stale-result check.
- Adding a snapshot library is a dependency decision under
  [`DEVELOPMENT.md`](../DEVELOPMENT.md#decision-authority).

A passing component render does not show that an existing screen adopted the component, and absence
of RGB cells does not show usable contrast. A standards-only change passes the link and repository
checks; it does not establish that any implementation conforms.

## Whole-screen visual review

Inspect the existing interface in a running terminal before choosing a presentation or interaction
change. When a reference product informs the goal, keep its observed behavior, its source at a named
revision, and the proposed Orchbee requirement apart.

Presentation and interaction changes require using the running Client in a real terminal emulator
and reviewing actual window screenshots, starting with the default theme. Compare equivalent
geometry, locale, data and input
ownership; review changed regions together with surrounding navigation, content and status.
Check current-context recognition, focus distinction, text hierarchy, duplicate labels and actions,
empty states, grouping, and the space available to primary content. Include the affected ordinary,
navigation and split/maximized states; a component render alone does not establish whole-screen quality.
Review at-rest action affordance, hover without focus movement, stable label tracks, complete
primary and exit hints at narrow geometry, and the identity of the operation's target.

- Capture the full composed screen before and after the change; retain surrounding navigation,
  content and status when inspecting a changed region.
- Capture the applicable states needed to judge the change, including hover, pressed, focus,
  selection, expansion, overlays and narrow geometry. A static resting image does not establish
  those states or their transitions.
- Complete the affected primary task and its cancel or return path with the relevant pointer and
  keyboard inputs. Record the observed result alongside the screenshots; images alone do not prove
  input routing, focus restoration or responsiveness.
- Review the result from the user's task perspective, repair visible or interaction defects, then
  repeat the affected flow and capture the final result.

The application owns its accepted screen composition and observable visual invariants. Later
feature work must preserve those outcomes or explicitly revisit the owning decision. Tests protect
mechanically checkable outcomes; updating expected rows or passing role assertions alone does not
accept a visual regression. Record the comparison and judgment in the active review, not a new
tracked task report.

Identify each image's source and the executing build or revision, and record the host information
required below. Rendered buffers and PTY/ConPTY replays are supplemental composition and protocol
evidence; they do not replace real-window screenshots or observed use. They cannot establish a
physical host's palette, font, contrast or glyph rendering.

Real-host review is a human or agent review requirement, not an automated root-gate result. When
the running interface cannot be exercised or captured, report implementation and automated checks
separately and leave experience acceptance pending. Do not declare a presentation or interaction
change complete solely because its tests, role assertions or rendered previews pass.

## Host evidence

Some behavior exists only in a real emulator: keyboard-protocol negotiation, pointer passthrough
encoding, appearance reports, and multiplexer interception.

- Exercise it through a pseudo-terminal harness that drives the Client binary and asserts on the
  bytes it writes, or through a documented manual run.
- Record the emulator name and version, the multiplexer if any, the operating system, geometry,
  locale, theme, and negotiated capabilities with the evidence. For visual review, also record the
  font, font size and display scale; identify unavailable metadata rather than inferring it.
  A PTY or ConPTY byte replay proves
  protocol behavior and cell composition; it does not prove physical host palette or font quality.
- Host evidence supplements the outcomes above; it never replaces them.

## Responsiveness and spatial stability

A responsiveness claim names the semantic action, exact target and expected local feedback before
collecting timings. Use the same host, actual inner geometry, data, locale, theme and comparable
build profile for a comparison; keep source, installed binary and executing cohort identities
separate. Record instrumentation perturbation with enabled and disabled runs. A reference product's
source mechanism does not establish its installed behavior or CI enforcement.

| Measurement                  | Required endpoint and limit                                                                                                                                                                                                                |
| ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Input to visible feedback    | Bracket input delivery and the first expected control/content change in an actual host capture. Use a semantic region/predicate; cursor blink, animation, unrelated output or title changes do not satisfy it. Report capture uncertainty. |
| Creation to useful content   | Identify the created target and its first useful terminal or task content. Immediate local acceptance, background completion and canonical readiness are separate checkpoints.                                                             |
| Input-ready or queue delay   | Name the observed queue and its admission/dequeue timestamps. If arrival is unavailable, report the measured application boundary or external upper bound; intentional polling is not queue residence.                                     |
| Preparation and output delay | Separate layout/buffer preparation, encoding, forwarded writer/flush calls and any acknowledgement. Call wall time can include CPU and blocking; completed output does not prove physical paint or semantic feedback.                      |

- Exercise the complete task and its cancellation, failure and return paths. Check immediate local
  acceptance in the expected place separately from delayed progress and eventual content; a changed
  spinner elsewhere does not establish that the requested target responded.
- Record sample counts and distributions, environment/load, failed and timed-out samples, and
  measurement bounds. A truncated, aborted, invalid or incomplete trace cannot establish a complete
  distribution. Set scenario regression targets from an observed baseline; do not infer a universal
  budget or a speed ratio across unmatched builds.
- Drive input while a bounded, reproducible content producer streams output. Verify input progress,
  cancellation/release ordering and the expected feedback, with a predicate that excludes that
  output. Diagnose the first delayed stage before changing scheduling or writer architecture.
- Describe wheel reports by direction, count, timing and burst/reversal boundaries. Exercise dense
  and sparse streams, ownership changes and pane passthrough separately from owned lists/history.
  Equivalent synthetic reports do not establish equivalent physical wheel or trackpad movement.
- Resize while reading identified logical content and while new content arrives. Verify the
  declared row/identity anchor, focus and selection, with explicit bottom following kept distinct
  from manual reading. A captured terminal grid and an authored wrapped document can own different
  reflow contracts; never substitute one product's algorithm for the other's authority.
- A live drag trace contains at least two held moves separated by rendered frames before release.
  Observe the affected content geometry at both positions, separate release retention, and the
  declared cancellation and stale-owner/viewport behavior. Endpoint-only images do not prove live
  adjustment or distinguish an owned projection from an external geometry change.

Keep recurring scenario definitions and instrumentation with their semantic owner. Point-in-time
measurements, screenshots and comparison judgments belong to the active issue/review with their
consumer and release condition, not a tracked task report. These checks supplement the outcomes and
host evidence above; neither timing nor a screenshot replaces them.
