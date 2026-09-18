# TUI Terminal Environment

## Authority and scope

This standard owns host terminal admission, the terminal mode lifecycle, capability detection,
output discipline, resize handling, and restoration for Orchbee-authored terminal interfaces. It
governs the boundary between a Client process and the terminal emulator that hosts it. Terminal
semantics inside retained panes belong to the terminal execution service and its protocol, not to
this standard.

## Admission

- A full-screen Client requires an interactive TTY on both its input and its output, and a valid
  cell geometry with nonzero rows and columns. Anything else is a usage failure written to stderr in
  the effective locale; no terminal mode is changed.
- Pixel geometry is used only when the host reports it. It is never guessed from a font assumption,
  and a missing pixel size disables pixel-dependent features rather than estimating them.
- The Client admits the TTY once and passes the same descriptors to the interface. It never
  reopens the controlling terminal by path after admission.
- Help and version output never touch terminal modes.

## Mode lifecycle

Every mode the Client enables has one paired restore step. Restore runs in reverse order of
enabling.

| Mode                       | Enable                                                              | Restore                              | When                                                  |
| -------------------------- | ------------------------------------------------------------------- | ------------------------------------ | ----------------------------------------------------- |
| Raw input                  | Disable echo and canonical processing                               | Restore the saved attributes         | Always                                                |
| Alternate screen           | DEC `1049`                                                          | Leave and restore the primary screen | Full-screen Clients                                   |
| Bracketed paste            | DEC `2004`                                                          | Disable                              | Always                                                |
| Keyboard enhancement       | Push the flags in [`keyboard-input.md`](keyboard-input.md#decoding) | Pop                                  | When the host answers the capability query            |
| Pointer reporting          | DEC `1002` + `1006`; `1016` on demand                               | Disable all pointer modes            | Per [`mouse-input.md`](mouse-input.md#capture-policy) |
| Focus reporting            | DEC `1004`                                                          | Disable                              | When a surface changes behavior on focus loss         |
| Synchronized output        | DEC `2026` around each frame                                        | End the update                       | Every frame; harmless when unsupported                |
| Color-scheme notifications | DEC `2031`                                                          | Disable                              | When the host answers the appearance query            |

- Emit mode changes as explicit command sequences owned by the terminal lifecycle module, with a
  recorded restoration step for each. No other module writes mode sequences.
- Restore runs on normal exit, error exit, the panic hook, and fatal signals. Each step is
  attempted even when an earlier step fails; the first error is reported after all steps ran.
- Keyboard-enhancement flags are popped before leaving the alternate screen so a host that ties
  the flag stack to the screen does not retain them.
- After restore, the Client writes nothing to the terminal except one final localized failure
  message on stderr.
- Suspending to a shell or handing the terminal to an external program restores every mode first
  and re-enables them on return.

## Capability detection

| Capability           | Source                                                                          | Unknown means                                   |
| -------------------- | ------------------------------------------------------------------------------- | ----------------------------------------------- |
| Color level          | `NO_COLOR`, `FORCE_COLOR`, `COLORTERM`, `TERM`, and terminfo in that precedence | 16 colors                                       |
| Default colors       | OSC `10` and `11` queries with a bounded reply deadline                         | Appearance unknown; the `terminal-native` theme |
| Appearance changes   | DEC `2031` reports, accepted only after a successful appearance query           | No live theme switching                         |
| Keyboard enhancement | The keyboard-protocol capability query with a bounded reply deadline            | Legacy decoding only                            |
| UTF-8                | `LC_ALL`, `LC_CTYPE`, `LANG` in that precedence                                 | ASCII glyph fallbacks                           |
| Multiplexer          | `TMUX`, `ZELLIJ`, and `STY` presence                                            | Not inside a multiplexer                        |
| Terminal name        | `TERM_PROGRAM` and vendor variables                                             | No vendor-specific workaround applied           |

- Every query has a deadline that fits inside the Client's startup budget. A late reply is
  discarded, never applied.
- Queries are issued and their replies consumed before ordinary key dispatch begins, or the decoder
  recognizes and removes reply sequences so they never reach a pane as typed input.
- Inside a multiplexer the multiplexer answers queries about itself. Treat its answers as the host
  and do not assume the outer terminal's capabilities.
- Unknown is a real state. It never defaults to the dark or light polarity, to truecolor, or to
  enhanced keyboard support.
- Vendor-specific workarounds are keyed on the detected name, documented with the observed defect,
  and removed when the vendor fixes it.

## Output discipline

- The renderer is the only writer to the host terminal while the interface is active. Logging goes
  to a file or an owned diagnostic channel; child process output never reaches the host TTY.
- Each frame is one diff against the previous frame, wrapped in a synchronized update, and flushed
  once. Partial frames are never flushed.
- Redraws are coalesced to a bounded cadence. Input, output, and timer events request a redraw; the
  loop draws at most once per cadence interval and immediately when idle.
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

| Change                  | Required check                                                                                            |
| ----------------------- | --------------------------------------------------------------------------------------------------------- |
| Mode addition or change | A test proves the enable sequence, the restore sequence, and that restore runs in reverse order           |
| Restoration path        | Tests cover normal exit, error exit, the panic hook, and that every step runs after an earlier step fails |
| Capability query        | Tests cover a timely reply, a late reply, no reply, and a reply arriving interleaved with key input       |
| Environment precedence  | A table-driven test proves each variable precedence including `NO_COLOR` over every other color source    |
| Output loop change      | A test proves at most one flush per cadence interval and one synchronized-update pair per frame           |
