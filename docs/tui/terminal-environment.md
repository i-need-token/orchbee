# TUI Terminal Environment

## Authority and scope

This standard owns host terminal admission, platform adaptation, the terminal mode lifecycle,
capability detection, output discipline, resize handling, and restoration for Orchbee-authored
terminal interfaces. It governs the boundary between a Client process and the terminal emulator or
native console that hosts it. Terminal semantics inside retained panes belong to the terminal
execution service and its protocol, not to this standard.

These clauses are required contracts. Naming a mode or capability here does not claim that every
current Client implements it. An owning workspace may advertise the behavior only after its adapter
and observable host path provide the evidence required by this standard.

## Admission

- A full-screen Client requires an interactive TTY on both its input and its output under the
  current same-terminal identity and descriptor-safety rules. A non-interactive stream or a failed
  geometry query is a usage failure written to stderr in the effective locale; no terminal mode is
  changed.
- A real, nonzero cell geometry must be obtained before the normal interface renders and before
  any retained terminal work begins. Cell sizes are never guessed.
- An application may declare a bounded awaiting-geometry phase in its owning contract for a zero
  or not-yet-drawable cell report. That phase uses only the already-admitted descriptors and the
  original startup deadline; wait input and its surface stay owned by the terminal mode lifecycle,
  which may enter and later restore the modes the wait surface needs. The phase starts no site,
  service, or retained terminal; cancellation, timeout, or a fault restores every acquired mode,
  and input consumed while waiting is never replayed. A path that does not declare this phase keeps
  refusing a non-drawable report.
- Pixel geometry is used only when the host reports it. It is never guessed from a font assumption;
  a missing pixel size is a legal unknown that disables pixel-dependent features rather than
  blocking the cell-based interface.
- The Client admits the TTY once and passes the same descriptors to the interface. It never
  reopens the controlling terminal by path after admission.
- Help and version output never touch terminal modes.

## Mode lifecycle

Every mode the Client enables has one paired restore step. Restore runs in reverse order of
enabling.

| Mode                       | Enable                                                                                      | Restore                              | When                                                  |
| -------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------ | ----------------------------------------------------- |
| Raw input                  | Platform adapter disables echo and canonical or processed input                             | Restore the saved attributes         | Always                                                |
| Alternate screen           | DEC `1049`                                                                                  | Leave and restore the primary screen | Full-screen Clients                                   |
| Bracketed paste            | DEC `2004`                                                                                  | Disable                              | Always                                                |
| Keyboard enhancement       | One owned push of the flags in [`keyboard-input.md`](keyboard-input.md#decoding)            | One paired pop                       | After the capability is admitted                      |
| Pointer reporting          | Global DEC `1003` or `1002`, with `1006`; scoped menu fallback per policy; `1016` on demand | Disable all owned pointer modes      | Per [`mouse-input.md`](mouse-input.md#capture-policy) |
| Focus reporting            | DEC `1004`                                                                                  | Disable                              | When a surface changes behavior on focus loss         |
| Synchronized output        | DEC `2026` around each frame                                                                | End the update                       | Every frame; harmless when unsupported                |
| Color-scheme notifications | DEC `2031`                                                                                  | Disable                              | When the host answers the appearance query            |

- Emit mode changes as explicit command sequences owned by the terminal lifecycle module, with a
  recorded restoration step for each. No other module writes mode sequences.
- Create a pending restoration entry before issuing each owned acquisition. Mark it acquired after
  success; keep it conservatively restorable after a partial or unknown write and never replay the
  acquisition; retire it when failure proves that no effect occurred. If acquisition stops after
  only some steps succeed, unwind the acquired or possibly acquired subset in reverse order before
  returning the failure. A later entry never overwrites an original state that remains owned by an
  earlier entry.
- Restore runs on normal exit, error exit, the panic hook, supported catchable termination signals,
  and platform shutdown callbacks that permit cleanup. An uncatchable kill or forced process
  termination provides no in-process restoration opportunity. Each step is attempted even when an
  earlier step fails; the first error is reported after all steps ran.
- A keyboard-enhancement activation issues at most one owned push. Its paired pop is consumed at
  most once and is never replayed; it runs before leaving the alternate screen so a host that ties
  the flag stack to the screen does not retain the flags. A duplicate or late capability reply
  cannot push another frame onto the host stack.
- A global pointer tracking-level switch disables the old level before enabling the new one; the
  scoped menu request preserves button fallback under [`mouse-input.md`](mouse-input.md#capture-policy). `1006` is the
  encoding used with the selected tracking level; `1016` is an orthogonal, on-demand coordinate
  mode. Releasing capture disables all owned pointer modes. Release or a downgrade from `1003`
  clears hover and armed or pressed presentation, cancels the active pointer gesture, and consumes
  movement received during the transition without forwarding unsolicited motion to a pane program.
- After restore, the Client writes nothing to the terminal except one final localized failure
  message on stderr.
- Suspending to a shell or handing the terminal to an external program restores every mode first
  and re-enables the still-admitted set on return. Pointer capture resumes only after the Client has
  redrawn a frame with a current hit map.

## Platform adapters

- The lifecycle is semantic rather than escape-sequence-specific. A POSIX adapter owns the admitted
  TTY descriptors and saved terminal attributes. A native Windows adapter owns the exact admitted
  console input and output handles, their saved mode words, and any other console attribute it
  changes. Both adapters expose the same acquisition, suspension, resumption, and restoration
  outcomes to the Client.
- An adapter captures an original attribute before its first mutation and restores that exact value,
  not a reconstructed default. On Windows this includes input and output console modes separately;
  if the Client changes the console title or another native attribute, it saves and restores that
  value under the same ownership rule.
- Each successful native mutation records its cleanup immediately. If a later handle lookup, mode
  change, VT-path activation, or command write fails, the adapter restores every earlier mutation
  it owns. Cleanup continues after an individual native or stream restore failure.
- A Windows adapter admits a VT-backed feature only after the required input and output paths are
  enabled or already present. Successfully writing an ANSI sequence is not evidence that the
  corresponding input reports will arrive. Unsupported features use their declared fallback or
  fail admission without leaving partially changed console modes.
- The lifecycle owner serializes native API calls relative to VT command writes and submits those
  writes through the output coordinator. A platform adapter does not create a second terminal
  writer.
- The Client owns only the host descriptors or handles admitted at startup. A child or pane program
  expresses its terminal modes through the terminal execution service; it never mutates the outer
  host's native console modes or writes lifecycle sequences directly.

## Capability detection

| Capability           | Source                                                                                                                                                    | Unknown means                                   |
| -------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| Color level          | `NO_COLOR`, `FORCE_COLOR`, `COLORTERM`, `TERM`, and terminfo in that precedence; documented vendor fallback without explicit descriptors or a multiplexer | 16 colors                                       |
| Default colors       | One ordered OSC `10`/`11` query; classify a valid OSC `11` background; on Windows only, use `AppsUseLightTheme` if no valid OSC `11` background arrives by the deadline | Appearance unknown; System uses `terminal-native` |
| Appearance changes   | DEC `2031` reports, accepted only after a successful appearance query                                                                                     | No live theme switching                         |
| Keyboard enhancement | The keyboard-protocol capability query with a bounded reply deadline                                                                                      | Legacy decoding only                            |
| Pointer any-motion   | Explicit host or adapter capability result, a product-owned platform contract with maintained conformance evidence, or an explicit supported user choice  | DEC `1002` + `1006` outside scoped menus        |
| UTF-8                | `LC_ALL`, `LC_CTYPE`, `LANG` in that precedence                                                                                                           | ASCII glyph fallbacks                           |
| Multiplexer          | `TMUX`, `ZELLIJ`, and `STY` presence                                                                                                                      | Not inside a multiplexer                        |
| Terminal name        | `TERM_PROGRAM` and vendor variables                                                                                                                       | No vendor-specific workaround applied           |

- Every query has a deadline that fits inside the Client's startup budget. A late reply is
  discarded, never applied.
- The Client sends OSC `10` and `11` through the lifecycle writer before its first frame, but does
  not wait for the 350 ms appearance deadline to draw. A valid OSC `11` background determines Light
  or Dark; the foreground reply is consumed but does not classify appearance. At the deadline,
  Windows reads the current user's `AppsUseLightTheme` registry value if no valid OSC `11` background
  arrived, including when the reply is foreground-only, malformed, partial, or absent. Other
  platforms settle Unknown when no valid background arrived.
- Both ordinary and modal decoders consume recognized OSC `10`/`11` reply strings. Payloads are
  bounded; malformed and oversized replies do not classify appearance. At the deadline Windows
  uses its registry fallback if no valid background arrived, while other platforms settle Unknown.
  Late replies cannot change the settled appearance or reach a pane as input.
- Queries are issued and their replies consumed before ordinary key dispatch begins, or the decoder
  recognizes and removes reply sequences so they never reach a pane as typed input.
- Inside a multiplexer the multiplexer answers queries about itself. Treat its answers as the host
  and do not assume the outer terminal's capabilities.
- A documented vendor color indicator may supply a capability only when explicit color descriptors
  are absent and no multiplexer is detected. Validate its bounded format before using it; an
  inherited outer-host indicator never overrides an explicit descriptor or multiplexer.
- Unknown is a real state. It never defaults to the dark or light polarity, to truecolor, or to
  enhanced keyboard support. Pointer any-motion has three outcomes: admitted, unavailable, and
  unknown. Both unavailable and unknown use `1002` with `1006` outside the scoped capturing-menu
  request in [`mouse-input.md`](mouse-input.md#capture-policy); that request does not promote the
  global capability and never re-arms released capture.
- There is no assumed universal query for DEC `1003`. A successful write, lack of an immediate
  error, terminal name, multiplexer name, or vendor environment variable does not admit global any-motion.
  If the evidence that admitted it is withdrawn, the lifecycle performs the serialized downgrade
  and state clearing defined above.
- Vendor-specific workarounds are keyed on the detected name, documented with the observed defect,
  and removed when the vendor fixes it. A workaround cannot promote pointer any-motion without one
  of its admitted evidence sources.

## Output discipline

- One output coordinator is the only writer to the host terminal while the interface is active. It
  serializes renderer frames and admitted terminal effects. Logging goes to a file or an owned
  diagnostic channel; child process output never reaches the host TTY directly.
- Each candidate frame is one diff against the previous frame, submitted as one synchronized update
  with one flush request. The Client never intentionally submits a partial frame. If the underlying
  write is partial or its result is unknown, that candidate does not become presented authority:
  the render baseline and the hit map in
  [`mouse-input.md`](mouse-input.md#hit-testing-and-redraw) are invalidated, interaction stays
  suspended until a verified full redraw establishes a new authoritative frame, and ambiguous bytes
  are never replayed.
- A terminal effect is dispatched only after the current synchronized frame closes and its flush
  succeeds. A new frame cannot begin while terminal-effect bytes are in flight. An effect with an
  unknown write result is retired without replay.
- Redraws are coalesced to a bounded cadence. Input, output, and timer events request a redraw; the
  loop draws at most once per cadence interval and immediately when idle.
- Ordered pointer events that cross an armed target's hit region are applied to gesture state before
  redraw coalescing. Coalescing may retain only the latest hover presentation, final resize
  geometry, or repaint request, but it preserves the excursion fact: returning to the same target
  does not re-arm click activation. It never discards a decoded key, button transition, pointer
  release, capture-release command, capability reply, or terminal-effect settlement. Each
  scheduling turn bounds pane-output and rendering work and returns to pending input and lifecycle
  replies within a bounded interval; output or motion floods cannot indefinitely delay
  cancellation, release, or restoration.
- The cursor is hidden while chrome is drawn and shown only inside an editor or a pane that owns it,
  at its reported position.
- Terminal title and other OSC side effects are written only through the lifecycle module and
  restored on exit.

## Resize

- A size-change signal or event re-queries cell and pixel geometry and triggers a full redraw.
- Zero or absent geometry is ignored; the last valid geometry remains in effect.
- Resize events are coalesced, and the final geometry of a burst is always applied.
- Retained panes receive a resize only when their inner geometry changed, as governed by
  [`layout.md`](layout.md#resize).

## Verification

| Change                    | Required check                                                                                                                                                                                                                                                                                               |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Mode addition or change   | A byte trace proves the enable sequence, one recorded cleanup per acquired step, reverse restoration, partial-acquisition unwind, and at-most-once keyboard push and pop                                                                                                                                     |
| Restoration path          | Tests cover normal and error exit, the panic hook, supported catchable termination signals and platform shutdown callbacks, suspend and resume, external handoff, and that every acquired step is attempted after an earlier restore fails                                                                   |
| Platform adapter          | POSIX and Windows adapter tests use injected handles or descriptors to prove exact saved-attribute restoration, separate Windows input and output modes, partial failure, idempotent repeated cleanup, and no second writer                                                                                  |
| Capability query          | Tests cover a timely reply, a late reply, no reply, and a reply arriving interleaved with key input                                                                                                                                                                                                          |
| Pointer admission         | Traces cover admitted, unavailable, and unknown any-motion; `1003`/`1002` exclusivity; `1006`; downgrade and re-admission; release and resume; state clearing; and no unsolicited pane motion or terminal-name promotion                                                                                     |
| Environment precedence    | A table-driven test proves each variable precedence including `NO_COLOR` over every other color source                                                                                                                                                                                                       |
| Output loop or scheduling | A trace proves at most one flush per cadence interval, one synchronized-update pair per frame, no authority or replay after a partial or unknown frame write, effects only between frames, ordered armed-target boundary transitions under repaint coalescing, and bounded input/reply progress under floods |
