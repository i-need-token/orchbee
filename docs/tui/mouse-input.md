# TUI Mouse Input

## Authority and scope

This standard owns pointer capture policy, hit testing, interaction state, the gesture vocabulary,
passthrough to programs inside terminal panes, wheel normalization, and selection for
Orchbee-authored terminal interfaces. Host mode negotiation follows
[`terminal-environment.md`](terminal-environment.md); keyboard ownership follows
[`keyboard-input.md`](keyboard-input.md); component state and composition follow
[`components.md`](components.md). The owning application documents which painted regions accept
which semantic intents; this standard defines the rules those mappings must satisfy.

This is the required contract for new and materially changed TUI pointer behavior. It does not
assert that current Clients already conform; runtime adoption is established only by source and
evidence under [`verification.md`](verification.md#contract-review). Record a staged gap with its
owner and completion condition. Until adoption, application-owned references remain the descriptive
authority for current capture and pointer behavior.

## Capture policy

- A full-screen Client prefers any-motion reporting (DEC mode `1003`) with SGR encoding (`1006`)
  while its interface is active. This is the admitted level for hover on Orchbee-painted chrome
  and overlays. When any-motion reporting is unavailable or has not been admitted, use
  button-event tracking (`1002`) with `1006`; click, drag, and wheel remain available, while hover
  is honestly absent. Exactly one of `1002` and `1003` is active at a time.
- Any-motion reporting is an outer-Client transport, not ownership of a pane program's input.
  Orchbee consumes movement over its chrome and forwards events inside a pane viewport only under
  [Passthrough](#passthrough). Unrequested movement never reaches the pane program.
- Pixel-position reporting (`1016`) is enabled only while a pane program requests pixel coordinates
  and is disabled when that need ends. It does not change which surface owns the gesture.
- The Client provides one effective binding and one command that release capture. Release disables
  every Orchbee-owned pointer mode, including `1002`, `1003`, `1006`, and `1016`, cancels any armed
  gesture, and clears hover. While released, a persistent status indicator states that the host
  owns selection, links, and scrolling and that Client pointer interaction is unavailable. The
  same operation restores the previously admitted capture level.
- The Client never binds a `Shift`-modified gesture. A host that supports `Shift` bypass retains it
  for native selection. Because not every host can prove that bypass, the explicit release command
  remains the portable route and discovery never promises a capability the host did not admit.
- Before suspending or handing the host terminal to an external program, release all pointer modes.
  Restore the admitted capture level only after Orchbee resumes and redraws its interface.

## Interaction state

Pointer interaction preserves these separate facts:

| Fact                  | Meaning                                                                                                     |
| --------------------- | ----------------------------------------------------------------------------------------------------------- |
| Hover                 | The pointer is over one hit region from the last presented frame; it is ephemeral Client-local presentation |
| Keyboard focus        | One painted region owns contextual key input and displays focus independently of hover                      |
| Provisional selection | A candidate within a list, picker, or editor that has not crossed its commit boundary                       |
| Current context       | The persistent domain object or location currently open; pointer movement never changes it                  |
| Armed gesture         | A press or drag in progress, tied to one stable target identity and its interaction revision                |
| Activation            | A semantic intent dispatched after its gesture satisfies the owner's commit rule; it is an event, not focus |

- Movement may change hover styling or reveal a local affordance. It never moves keyboard focus or
  provisional selection, activates an action, commits data, mutates shared domain state, or writes
  terminal input.
- A press may move focus or selection only when the owning component declares that step. Immediate
  action rows, selection-first pickers, disclosures, and editable value rows are distinct
  interaction contracts; a renderer never gives every row one invented click rule.
- Losing terminal focus or releasing capture clears hover and any armed gesture without
  activation. A target's armed gesture is also cancelled when its geometry, availability,
  occlusion, ownership, accepted gesture, or semantic intent changes. A visual-only repaint or a
  change to an unrelated target does not cancel it merely because the frame sequence advanced. The
  Client retains a last pointer position only while capture is active and terminal focus is held;
  after either is lost, hover stays absent until a new movement report arrives.
- After an accepted Orchbee UI press is cancelled, its owner may retain only typed cleanup
  ownership for that press's primary release. Cleanup is resolved before ordinary capture, focus,
  or hit-map admission: the matching late release ends the gesture once and keeps its complete
  physical packet, including paste and unfinished decoder fragments, Client-owned without
  re-hit, activation, or another effect. Intervening pointer reports cannot re-hit through that
  debt. A new non-wheel press retires it. An orphan release creates neither debt nor a packet fence.
  Pane-program gestures, links, and text selection retain their own owner-specific cleanup
  contracts.

## Hit testing and redraw

- The renderer records the rectangle, layer, stable target identity, accepted gestures, semantic
  target, availability, and interaction revision of every interactive region it actually paints.
  A target's revision changes when any of those interaction properties changes; text, color,
  progress animation, and other presentation-only changes preserve it. The candidate hit map
  becomes authoritative atomically with a successfully flushed frame.
- Region kinds are finite and documented by the owner: navigation entry, pane viewport, pane
  boundary, collection row, disclosure, action, editor, value control, scroll indicator, status
  segment, and overlay control. Adding another kind requires an interaction contract, not only a
  rectangle.
- Peer regions do not overlap within one layer. A declared child target, such as a row's auxiliary
  action, is tested before its parent and wins on its painted extent. Other layers resolve front to
  back. A capturing overlay owns gestures outside its controls according to its dismissal contract
  and prevents targets beneath it from hovering or arming.
- The last successfully flushed map remains authoritative while a repaint is pending when the
  renderer can prove that every interaction property is unchanged. Its candidate never becomes
  authoritative merely because bytes were attempted; a partial or unknown result cannot authorize
  new geometry. A resize or structural layout replacement invalidates affected authority before
  the next input decision, and events are dropped until a new map is successfully presented. If a
  write with interaction changes is partial or its result is unknown, authority remains suspended
  rather than falling back to the stale map.
- After a new map is published, reconcile transient state by stable target identity and interaction
  revision. Keep an armed gesture only when its target remains present with the same revision;
  otherwise cancel it without activation. A target omitted for width, capability, availability,
  or occlusion has no hit region and cannot retain hover, focus authority, or activation authority
  through old geometry. Any pointer event dropped while authority is suspended cancels the active
  gesture so a later frame cannot complete it.
- A pointer move updates hover only when the resolved target changes. Repeated reports in the same
  cell or target request no redraw. A real change requests one redraw through the bounded cadence
  in [`terminal-environment.md`](terminal-environment.md#output-discipline); movement never forces a
  frame per report. Boundary crossings for an armed target are applied to gesture state in event
  order before redraw coalescing; retaining only the latest hover presentation never erases a
  leave-and-return excursion.
- The Client retains the last pointer position. After a new frame is presented it may recompute
  hover against the new map and request a later bounded redraw if the target changed. This
  reconciliation cannot focus, select, arm, or activate anything.
- A primary press arms the resolved stable target identity and interaction revision. Release
  activates only when it resolves to that same still-available target and revision in the current
  authoritative map. Leaving the target during a drag retains gesture ownership for cleanup or
  preview but cancels click activation; returning before release does not silently re-arm it.
  Capture loss or a revision change caused by geometry, availability, occlusion, ownership,
  accepted-gesture, or semantic-target changes cancels the gesture.

## Gesture and intent vocabulary

Every accepted gesture resolves to at most one scoped semantic intent. Pointer and keyboard routes
for a committed operation use the same action identity, target, availability rule, and outcome.
Hover, caret placement, and drag preview are modality-local presentation; the resulting operation
must still be reachable from the keyboard.

| Region contract            | Pointer behavior                                                                                   | Required keyboard route                                     |
| -------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Immediate action           | Primary click invokes the action on a valid release                                                | Focus plus `Enter` or `Space`, or its effective binding     |
| Immediate navigation       | Primary click opens or makes the documented target current                                         | Focus plus `Enter`, or its effective navigation binding     |
| Selection-first collection | Primary click moves provisional selection; acceptance is a separately documented gesture or action | Focus movement plus the same explicit accept action         |
| Disclosure                 | Primary click on the disclosure target toggles expansion                                           | `←`/`→` or the documented toggle while the row has focus    |
| Editor                     | Primary click focuses it and may place the caret                                                   | `Tab` or documented focus movement; editing keys then apply |
| Value control              | Primary click invokes its declared edit, picker, or immediate-toggle contract                      | Focus plus the same semantic edit or toggle action          |
| Pane viewport              | Primary press focuses the pane, then passthrough may receive the event                             | The documented pane-focus route                             |
| Pane boundary              | Primary drag previews resize; release commits one resize                                           | A resize mode that can preview, commit, and cancel          |
| Scrollable region          | Wheel moves that region without changing focus                                                     | Arrow or page movement in the same region                   |
| Scroll indicator           | Primary click moves the viewport to the represented position                                       | Page or jump commands reaching the same viewport positions  |

- A collection owner declares whether acceptance is a second click, a double click, a dedicated
  control, or another explicit gesture. A first click never activates merely because the target is
  drawn as a row. Multi-click detection uses a bounded interval, the same target identity, and the
  same cell; moving to another target starts a new sequence.
- Disabled targets never acquire hover or pressed treatment and cannot arm or activate. Their
  persistent disabled presentation and the keyboard or help route expose the localized reason.
- Pane text selection and a boundary-resize drag may begin on press so movement has an anchor; they
  still commit only at their documented release boundary and can be cancelled before it.
- Right and middle presses are reserved. Context surfaces and pointer paste require separate
  accepted contracts and keyboard-accessible outcomes before they are bound.
- Scroll is Client-local view state. A retained terminal selection belongs to its Terminal Host
  attachment and is projected by the Client. Shared workbench state changes only at the semantic
  operation's declared commit boundary.

## Passthrough

- A pane program owns pointer events inside its viewport only for the event classes it requested:
  button events under its button mode, held-button motion under `1002`, all motion under `1003`,
  and pixel coordinates under `1016`. Orchbee translates admitted events to pane-local coordinates
  and re-encodes them in the program's requested format. It never forwards outer hover merely
  because the Client itself uses `1003`.
- Events outside the viewport are never forwarded. Pane boundaries and all Orchbee chrome remain
  Client-owned regardless of the program's modes.
- A primary press that focuses a pane does so locally before forwarding. The forwarded press and
  its later drag or release retain the same pane target unless the gesture is cancelled by capture
  loss or invalid geometry.
- Wheel movement over an alternate-screen program that does not otherwise report the pointer is
  forwarded as arrow-key presses only while DEC `1007` is enabled, three per tick by default.
  Disabling the mode ends queued arrow work and returns subsequent wheel movement to the Client.
- `Shift` bypass and explicit capture release belong to the host path and are never synthesized as
  input to a pane program.

## Wheel normalization

- Wheel reports carry direction only. Treat consecutive reports separated by less than a short gap
  as one stream, and convert a stream to line deltas with a documented lines-per-tick default of
  three.
- Coalesce scroll application to the redraw cadence; never draw once per report.
- A single flush scrolls at most one viewport height. Excess remains in the stream and applies on
  later flushes.
- Trackpad-like streams may accumulate fractional lines; a direction reversal clears the
  accumulator.
- Direction inversion and speed are user settings owned by the application, applied after
  normalization.

## Selection and clipboard

- The Terminal Host owns pane-selection endpoints and exported text. The Client projects that
  selection over live output or retained history with `surface-selection` from
  [`visual-foundations.md`](visual-foundations.md#color-roles-and-themes). A named selection owner
  may expose `Esc` as its local clear step; otherwise pane `Esc` remains passthrough. A press outside
  the selection or invalidation of its interaction revision clears local gesture work. Host
  endpoint eviction clears the selection with a notice.
- An application declares one clipboard sink for each copy. It never broadcasts one copy to
  multiple sinks and never treats an unacknowledged OSC 52 write as proof that a platform fallback
  is safe. Clipboard work never blocks rendering, and failure produces a status notice.
- Exported text is complete Host-produced plain text with the pane's own logical line breaks;
  chrome cells and pane boundaries are never included. A partial, malformed, over-limit, or
  invalid UTF-8 export writes nothing.
- A selection whose rows are evicted from retained history is cleared with a notice, not silently
  moved.

## Verification

| Change                     | Required check                                                                                                                                                                                                                         |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Capture policy change      | Byte traces prove preferred `1003` and fallback `1002` enable, exact release and restoration, `Shift` non-binding, hover clearing, and the released-state indicator                                                                    |
| Hover behavior             | Tests prove enter, move, leave, occlusion, same-target coalescing, no focus or selection movement, no activation, and honest absence without any-motion reporting                                                                      |
| Region addition            | Tests resolve every edge and the exterior against the last successfully presented frame; visual-only repaint preserves authority, while stale, unpainted, occluded, resized, and unknown structural writes cannot authorize activation |
| Gesture mapping change     | A table-driven test maps each gesture to one intent, preserves an arm across visual-only and unrelated repaints, cancels it on interaction-revision changes, exercises its commit boundary, and proves an equivalent keyboard route    |
| Passthrough change         | Tests cover each requested event class, unrequested outer motion, pane-local translation, reporting off on primary and alternate screens, DEC `1007`, and chrome or boundary input                                                     |
| Wheel normalization change | Tests cover a wheel stream, a trackpad stream, reversal, same-cadence coalescing, and the per-flush cap                                                                                                                                |
| Selection change           | Tests cover gesture order, live/history projection, retained-history revision movement, interaction-revision cancellation, owner-local `Esc`, pane passthrough, multi-line Unicode export, and eviction                                |
