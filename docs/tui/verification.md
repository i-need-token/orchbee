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

## Render evidence

- Render through `ratatui::backend::TestBackend` and compare the buffer as text with an expected
  value kept inline in the test or in a fixture beside it. Adding a snapshot library is a dependency
  decision under [`DEVELOPMENT.md`](../DEVELOPMENT.md#decision-authority).
- Every surface renders at these geometries: 80×24, 120×40, one Compact and one Minimal geometry
  from [`layout.md`](layout.md#size-tiers), and the too-small geometry.
- Every surface renders under the `terminal-native` theme and one RGB theme, and under `en-US` and
  `zh-CN`. Wide-character fixtures are present wherever text truncates or wraps.
- Style assertions check roles through the theme, not literal colors, except the quantization test
  that proves a 16-color level yields no `Rgb` or `Indexed` cell.
- An expected-text update is reviewed as a visual diff. A snapshot never changes without the
  reviewer seeing the before and after rows.

## Input evidence

| Class        | Required cases                                                                                               |
| ------------ | ------------------------------------------------------------------------------------------------------------ |
| Decoding     | Every C0 normalization, uppercase implies `Shift`, `Ctrl+]`, lone `ESC`, split sequences, atomic paste       |
| Binding      | Match, non-match with extra modifiers, label on Apple and non-Apple hosts, alternates display the first      |
| Ownership    | Table-driven precedence for every owner pair; unconsumed bytes pass through in order                         |
| Mode         | Indicator visible, `Esc` exits, unbound keys ignored, modes do not nest                                      |
| Editor       | Every editing key, the length bound, prefill, atomic paste, composition not consumed                         |
| Confirmation | Only the exact key confirms, `Esc` cancels, trailing bytes in the decision packet do not reach the terminal  |
| Pointer      | Hit testing against the last drawn frame, passthrough decision, gesture-to-intent table, wheel normalization |

Input tests drive typed events through the decision function and assert the whole resulting intent
and state, not individual fields.

## Environment evidence

- Mode sequences are tested as byte strings against a writer: enable, restore, reverse order, and
  every step running after an earlier failure.
- Capability queries are tested with a timely reply, a late reply, no reply, and a reply interleaved
  with key input.
- Environment-variable precedence is table-driven and includes `NO_COLOR`.
- The output loop is tested for one flush per cadence interval and one synchronized-update pair per
  frame.

## Host evidence

Some behavior exists only in a real emulator: keyboard-protocol negotiation, pointer passthrough
encoding, appearance reports, and multiplexer interception.

- Exercise it through a pseudo-terminal harness that drives the Client binary and asserts on the
  bytes it writes, or through a documented manual run.
- Record the emulator name and version, the multiplexer if any, and the operating system with the
  evidence.
- Host evidence supplements unit evidence; it never replaces the classes above.

## Change checklist

| Change                      | Evidence                                                                                                |
| --------------------------- | ------------------------------------------------------------------------------------------------------- |
| New surface                 | Render evidence at every geometry, theme, and locale; ownership and footer tests when it captures input |
| Theme role or value         | Re-render of every consumer; quantization test                                                          |
| Binding or mode             | Binding, ownership, and mode classes; help output derived from the registry                             |
| Gesture or region           | Pointer class; keyboard equivalent proven                                                               |
| Terminal mode or query      | Environment class; host evidence for negotiation                                                        |
| Layout tier or degradation  | Render evidence per tier; one test per degradation step                                                 |
| Notice or pending indicator | Injected-time tests for expiry, replacement, and constant width                                         |
| Repository gate             | `pnpm verify` passes                                                                                    |
