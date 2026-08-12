# Projected Terminal Experience Compatibility

| Field             | Value                                                                                                                                                                                                           |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Accepted recovery design; implementation is incomplete and the reserved capability decisions below remain unaccepted                                                                                            |
| Owner             | `@orchbee/terminal` maintainers, with Terminal Host, protocol, Workbench, Desktop, and Browser adapters owning the boundaries identified below                                                                  |
| Current authority | [`packages/terminal/README.md`](../../packages/terminal/README.md), [`apps/terminal-host/docs/runtime-model.md`](../../apps/terminal-host/docs/runtime-model.md), and each linked protocol or application guide |
| Reference         | Orchbee `be7b4d0d531be6bab4c6cb4fb164addb997c951d`; pre-Host hardening `44daa5a98f263d146cda6588f22eb6d1325cb97e`; Ghostty `91f66da24527fa02d92b5fd0b41cd020f553a64c`                                           |
| Exit condition    | Satisfy every non-reserved claim, resolve or deliberately retain every reserved claim, move maintained facts into owning guides and executable evidence, then delete this proposal and its review translation   |

Simplified Chinese review translation:
[projected-terminal-experience-compatibility.zh-CN.md](projected-terminal-experience-compatibility.zh-CN.md).
English remains canonical.

## Authorization

The terminal experience must recover the observable behavior of the mature pre-cutover terminal
while retaining Terminal Host as the only PTY and Ghostty VT authority. Desktop and an authenticated
LAN Browser must consume the same canonical Host state through the projected client. Recovery is
split into independently verifiable slices and must not infer completion from a screenshot or from
the presence of an implementation path.

The decision owner accepted this recovery scope and the claim inventory on 2026-08-11. The scope
includes one Tab with one primary terminal Panel. It excludes multi-Panel and multi-Tab product
work, browser-side VT restoration, synchronous whole-buffer APIs, unsupported graphics protocols,
and unrelated new product features. The decisions under [Reserved decisions](#reserved-decisions)
remain outside governed implementation until separately accepted.

## Problem and outcome

The `5677c6f757d2d294dfee6cdb96549bf719d4c2e8` cutover removed the mature browser terminal
composition while the projected replacement did not yet own every rendering, geometry, input,
selection, search, accessibility, and lifecycle contract. The result is a set of boundary losses,
not evidence that Host-owned VT state is the wrong architecture.

Recovery succeeds when the projected surface reproduces the adopted observable behavior through
Host-owned state, bounded transport, and source-neutral client composition. A behavior may differ
only when an owning current authority or a separately accepted reserved decision explicitly defines
that difference.

## Inputs and outputs

| Kind              | Contract                                                                                                                                                                                        |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Host input        | Validated OBTF frames, OBPB row windows, metadata, selection state, terminal effects, lifecycle events, and typed failures                                                                      |
| Application input | Attachment capabilities, lifecycle intent, presentation configuration, localized labels, clipboard and navigation capabilities, and Workbench focus/context                                     |
| Browser input     | Measured container geometry, DPR, font availability, document visibility, keyboard, IME, pointer, wheel, touch, DOM selection, and accessibility interaction                                    |
| Reference input   | Pinned Orchbee revisions, pinned Ghostty revision, canonical environment, semantic action trace, and adopted observable checkpoints                                                             |
| Rendered output   | Packed Canvas or WebGPU pixels, balanced geometry, native input anchor, off-screen accessibility tree, live announcements, and deliberate lifecycle presentation                                |
| Semantic output   | Host commands, frame acknowledgements, action requests, clipboard/navigation requests, metadata updates, telemetry, and typed feature or lifecycle failures                                     |
| No-op             | Invalid identity, stale revision, unavailable capability, repeated acknowledgement, unowned gesture, hidden animation, and rejected lifecycle transition leave the relevant authority unchanged |

Untrusted process, network, binary, JSON, DOM, and persisted inputs are validated at their first
boundary. Missing Host metadata means absence, never permission to synthesize terminal state.
Missing application capability means unavailable behavior with explicit action availability or
feedback, never a silent privileged fallback.

## Ownership and dependency boundaries

| Owner                               | Authoritative facts                                                                                                                                                                | Excluded facts                                                                             |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| `apps/terminal-host`                | PTY/process lifetime, Ghostty VT and modes, retained history, selection endpoints, mode-dependent input encoding, canonical geometry, terminal effects, and terminal revisions     | Product UI, clipboard, external navigation, Web theme, and browser gesture policy          |
| `@orchbee/terminal-protocol`        | Versioned Host wire schemas, codecs, bounds, control kinds, and conformance fixtures                                                                                               | Product policy and rendering                                                               |
| `@orchbee/terminal`                 | Packed projection commit, renderer pixels, source-neutral input/gesture adaptation, bounded history presentation, client search/link projection, IME anchor, and accessible mirror | PTY bytes, VT parsing, product topology, privileged effects, and a second terminal palette |
| `apps/web`                          | Shared Electron/Browser composition, scoped Actions, presentation theme intent, Workbench bridges, localized feature feedback, and environment capability use                      | Electron or Gateway transport internals and Host state mutation                            |
| `apps/desktop` and Gateway adapters | Capability-scoped transport, attachment routing, application lifetime, Browser-access policy, and platform effects                                                                 | VT parsing and an alternate renderer UI                                                    |
| `@orchbee/workbench`                | Project/Workspace/Tab/Panel presentation identity, focus/context composition, title and current-directory policy, and terminal lifecycle placement                                 | PTY, VT, projection decoding, and platform effects                                         |

Mutable facts have one writer. Host projection is canonical terminal state; browser overlays and
animation are reversible presentation. A client never reconstructs a VT, reparses terminal escape
sequences, or rebuilds a per-cell object graph on every frame. An application effect never treats
Host metadata as authority to open a file or external scheme.

## State and transitions

| State group       | States                                                                                                 | Accepted transitions and cancellation                                                                                                                                                                                                                                                                                                      |
| ----------------- | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Surface lifecycle | `constructed`, `opening`, `live`, `recovering`, `exited`, `failed`, `disposed`                         | Open allocates resources transactionally. Recovering retains the last committed frame and uses bounded retry plus explicit user recovery. Exit and failure revoke live input/resize capability without destroying readable final content. Dispose cancels every pending workflow and releases owned DOM and attachment state exactly once. |
| Projection        | `paused`, `snapshot-required`, `in-flight`, `ready`, `blocked`                                         | Host owns generation/revision ordering. Only the exact applied acknowledgement advances a base. Stale or invalid input requests a snapshot without mutating the committed store.                                                                                                                                                           |
| Viewport          | `following-bottom`, `viewing-history`, `returning-bottom`                                              | User history intent requests bounded Host windows. Input-owned recovery returns to bottom; focus-only repair does not. Eviction, clear, screen change, and reflow preserve or deliberately invalidate Host pins and move the client to the matching phase.                                                                                 |
| Resize            | `unmeasured`, `measured-observer`, `measured-owner`, `request-in-flight`, `canonical-committed`        | One attachment owns the Host resize lease. Every view measures and presents canonical geometry; only the owner requests PTY geometry. Lost/rejected requests remain unconverged until a canonical Host commit or typed failure resolves them.                                                                                              |
| Input             | `idle`, `composing`, `key-owned`, `mouse-owned-by-application`, `mouse-owned-by-terminal`, `repairing` | Gesture ownership is fixed for a pressed sequence. Blur, visibility loss, detach, cancellation, or capability loss repairs attachment-owned state before any focus-out consequence. Composition cancellation commits no text.                                                                                                              |
| Search            | `idle`, `debouncing`, `scanning`, `ready`, `unavailable`                                               | Query replacement cancels stale work. Navigation intent during debounce or scan is retained. Decorations stay bounded without silently bounding directional reach. Active match, reveal, and selection ownership change atomically.                                                                                                        |
| Clear/reset       | `validated`, `committed`, `published`, `unknown-result`                                                | Host validates authority and lifecycle before one canonical mutation. Clear and reset never inject PTY bytes. A lost result is not automatically replayed. The next projection is a complete snapshot and invalid client transients are repaired.                                                                                          |

Every async workflow returns typed failure or cancellation. No retry may duplicate terminal input,
clear/reset, clipboard, link activation, or another user effect whose result is unknown.

## Invariants

- Terminal Host is the only PTY, Ghostty VT, terminal-history, terminal-selection, and
  mode-dependent input authority.
- One projected frame commits metadata, geometry, rows, generation, and revision atomically.
- Live projection, row windows, search, links, accessibility, queues, and retained revisions have
  explicit byte, row, item, or time bounds.
- One terminal has at most one resize lease; non-owner views never resize-fight and never reinterpret
  clipping as a smaller canonical grid.
- Each connected page has fresh session and attachment identities; loss or backpressure retires
  only that page's resources.
- Desktop and Browser checkpoints naming the same Host generation and revision expose equivalent
  semantic rows, geometry, selection, accessibility, lifecycle, and approved visual projection.
- Terminal colors come from Host metadata. Application theme owns only application surfaces and
  overlays through an explicit composition workflow.
- Focus, key, mouse, IME, selection, search, and hover transitions have explicit ownership and
  cancellation; one client cannot silently repair another client's active input.
- Product-facing messages are localized in `en-US` and `zh-CN`.

## Reference refinement

| Field         | Contract                                                                                                                                                                                                                                                 |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Provenance    | Mature Orchbee `be7b4d0`; pre-Host hardening `44daa5a`; cutover `5677c6f`; pinned Ghostty `91f66da`; exact package protocol and fixture formats recorded by each witness                                                                                 |
| Scope         | Semantic rows, graphemes, widths, wrapping, cursor, styles, colors, geometry, focus, input/IME, mouse, history, selection, links, search, accessibility, lifecycle, and required painting details                                                        |
| Projection    | Compare canonical state first, then geometry and accessibility, then pixels where paint is part of the claim; capture the same semantic checkpoints in the reference and projected paths                                                                 |
| Normalization | Normalize only attachment/session identifiers, temporary paths, process identifiers, and nondomain timing when the claim does not observe them; never normalize text, width, wrap, color, geometry, focus, selection, ordering, or lifecycle differences |
| Authority     | The accepted local claim wins over incidental reference behavior; otherwise the pinned mature behavior is the compatibility default. Current security and capability authorities remain in force until a reserved change is accepted.                    |

The canonical visual environment pins repository-provisioned fonts, viewport, DPR, locale, theme,
motion preference, renderer backend, and logical scheduling. Platform exploration is maintenance
evidence until its inputs are pinned and provisioned.

## Claim traceability

Each witness name is stable. A completed slice maps it to an executable core test, deterministic
system trace, differential fixture, or justified non-applicable method in the owning guide.

| Claim | Observable contract                                                                                      | Semantic owner                          | Required witness                             |
| ----- | -------------------------------------------------------------------------------------------------------- | --------------------------------------- | -------------------------------------------- |
| PR-01 | CJK, combining, emoji, and ZWJ graphemes retain Ghostty boundaries and widths.                           | Host / protocol / terminal              | `terminal-parity/PR-01-graphemes`            |
| PR-02 | Narrow, wide, spacer-tail, and wrapped spacer-head topology is exact.                                    | Host / protocol / terminal              | `terminal-parity/PR-02-cell-topology`        |
| PR-03 | A cursor on a wide tail uses Ghostty presentation geometry without changing canonical coordinates.       | terminal renderer                       | `terminal-parity/PR-03-wide-cursor`          |
| PR-04 | Soft-wrap and hard-line boundaries agree across paint, search, links, and accessibility.                 | Host / protocol / terminal              | `terminal-parity/PR-04-wrap-boundaries`      |
| PR-05 | Projected style flags, colors, decorations, and blink phases match Ghostty.                              | Host / protocol / terminal renderer     | `terminal-parity/PR-05-styles`               |
| PR-06 | Dynamic defaults, palette, cursor color, overlays, and application surface resolve coherently.           | Host / terminal / web / desktop         | `terminal-parity/PR-06-colors`               |
| PR-07 | OSC 8 and OSC 133 metadata survive projection and policy-gated activation.                               | Host / protocol / terminal / web        | `terminal-parity/PR-07-semantic-links`       |
| PR-08 | Title, current directory, screen, cursor, and interaction modes commit with their frame.                 | Host / protocol / terminal              | `terminal-parity/PR-08-metadata`             |
| PR-09 | Snapshot/delta commits and completed-draw checkpoints are atomic and generation/revision pinned.         | Host / protocol / terminal / web        | `terminal-parity/PR-09-frame-commit`         |
| PR-10 | Terminal query replies retain configured pinned-Ghostty behavior and ordered routing.                    | Host                                    | `terminal-parity/PR-10-replies`              |
| PR-11 | Each Host bell sequence reaches each attachment once; product-level effects await their reserved policy. | Host / protocol / terminal / app        | `terminal-parity/PR-11-bell`                 |
| PR-12 | Terminal-originated column changes correlate Ghostty, PTY, pixel, Core, and client geometry.             | Host / terminal                         | `terminal-parity/PR-12-deccolm`              |
| PR-13 | Application clear/reset mutates Host VT state coherently and publishes a complete new baseline.          | Host / protocol / terminal / web        | `terminal-parity/PR-13-clear-reset`          |
| SU-01 | Configured Latin/CJK/emoji faces load before one shared input/canvas measurement.                        | terminal / web                          | `terminal-parity/SU-01-font-load`            |
| SU-02 | Metrics, weights, italic, ligatures, bearings, and constraints remain Ghostty-aligned.                   | terminal renderer                       | `terminal-parity/SU-02-font-metrics`         |
| SU-03 | Cursor shape, blink, focus, accent, and one-physical-pixel strokes are stable.                           | terminal renderer                       | `terminal-parity/SU-03-cursor`               |
| SU-04 | IME anchor and preedit use the same Ghostty width/cursor projection and visibility.                      | terminal input / renderer               | `terminal-parity/SU-04-ime-geometry`         |
| SU-05 | Selection, search, and link overlays layer and clip consistently.                                        | terminal renderer                       | `terminal-parity/SU-05-overlays`             |
| SU-06 | Padding is removed from fit and excess pixels balance without clipping or invalid geometry.              | terminal geometry                       | `terminal-parity/SU-06-fit`                  |
| SU-07 | Measured, external, rejected, lost, and re-entered resize paths converge on Host geometry.               | terminal / Host                         | `terminal-parity/SU-07-resize-convergence`   |
| SU-08 | DPR changes rebuild surface geometry, raster state, and PTY pixel dimensions coherently.                 | terminal / browser                      | `terminal-parity/SU-08-dpr`                  |
| SU-09 | Runtime theme replacement atomically updates Host defaults, pixels, overlays, and app surface.           | web / Host / terminal                   | `terminal-parity/SU-09-theme`                |
| SU-10 | History exposes a visible, draggable, fading in-canvas scrollbar.                                        | terminal history presentation           | `terminal-parity/SU-10-scrollbar`            |
| SU-11 | Smooth history motion is bounded, reversible presentation and cancels when hidden.                       | terminal history presentation           | `terminal-parity/SU-11-smooth-scroll`        |
| SU-12 | WebGPU and Canvas expose equivalent canonical output, health, fallback, and recovery.                    | terminal renderer                       | `terminal-parity/SU-12-backends`             |
| SU-13 | Product cursor defaults survive reset while terminal escape sequences can override them.                 | Host profile / terminal renderer        | `terminal-parity/SU-13-cursor-defaults`      |
| SU-14 | Runtime presentation configuration remeasures and repaints atomically without reopening.                 | terminal / web                          | `terminal-parity/SU-14-runtime-presentation` |
| SU-15 | Visibility and demand pause transient rendering and resume with one current draw.                        | terminal / web                          | `terminal-parity/SU-15-visibility`           |
| SU-16 | The child and first frame start at intended mounted geometry without an intermediate launch grid.        | desktop / Host / terminal               | `terminal-parity/SU-16-startup-geometry`     |
| IN-01 | Browser physical keys, modifiers, keypad, Kitty actions, and mobile backspace encode like Ghostty.       | terminal input / Host                   | `terminal-parity/IN-01-keyboard`             |
| IN-02 | IME commits once, cancellation commits nothing, and every lifecycle repair clears transient input.       | terminal input                          | `terminal-parity/IN-02-composition`          |
| IN-03 | Paste inspection, review, ordering, capability, and bracketed-mode encoding form one decision.           | terminal / web / Host                   | `terminal-parity/IN-03-paste`                |
| IN-04 | Focus repair ordering is exact, internal movement is not duplicated, and mount does not steal focus.     | terminal / Workbench                    | `terminal-parity/IN-04-focus`                |
| IN-05 | Mouse ownership is fixed for a gesture and Shift gives terminal selection/link ownership safely.         | terminal input / Host                   | `terminal-parity/IN-05-mouse-ownership`      |
| IN-06 | Pointer shape derives from selection, rectangle, application mouse, and link policy.                     | terminal presentation                   | `terminal-parity/IN-06-pointer`              |
| IN-07 | Local paging and every admitted input source perform the shared history/selection transition.            | terminal input / history                | `terminal-parity/IN-07-pre-input`            |
| IN-08 | Text drop follows paste; file drop remains unavailable without an accepted application capability.       | terminal / app                          | `terminal-parity/IN-08-drop`                 |
| IN-09 | Terminal Actions use the containing Workbench scope and deterministic conflict/confirmation policy.      | Workbench / web                         | `terminal-parity/IN-09-action-scope`         |
| IN-10 | Read-only state suppresses PTY input while preserving admitted local selection and history behavior.     | terminal / attachment owner             | `terminal-parity/IN-10-read-only`            |
| IN-11 | Context-menu intent respects mouse ownership and exposes only accepted pinned semantic context.          | terminal / Workbench                    | `terminal-parity/IN-11-context`              |
| IN-12 | Admitted terminal Actions have synchronous availability and perceivable completion/failure feedback.     | Workbench / web                         | `terminal-parity/IN-12-actions`              |
| IN-13 | Wheel policy preserves delta modes, axes, local history, mouse reporting, and alternate-screen fallback. | terminal input / Host                   | `terminal-parity/IN-13-wheel`                |
| IN-14 | Shared-terminal focus follows one accepted terminal-level aggregation policy.                            | Host / app                              | `terminal-parity/IN-14-shared-focus`         |
| IN-15 | Abrupt attachment loss cannot strand or cross-release attachment-owned key/mouse state.                  | Host / terminal                         | `terminal-parity/IN-15-detach-repair`        |
| IN-16 | Concurrent clients share one bounded terminal-global FIFO without replay or reordering.                  | Host                                    | `terminal-parity/IN-16-input-fifo`           |
| IN-17 | Mobile activation opens the virtual keyboard and preserves exactly-once input and selection.             | terminal browser input                  | `terminal-parity/IN-17-mobile`               |
| HS-01 | Bottom following and bounded history navigation retain one coherent visible window.                      | Host / terminal history                 | `terminal-parity/HS-01-history-window`       |
| HS-02 | Resize reflow preserves the Host-pinned logical top while viewing history.                               | Host / terminal history                 | `terminal-parity/HS-02-reflow`               |
| HS-03 | Clear, eviction, and screen changes produce matching viewport and live/history state.                    | Host / terminal history                 | `terminal-parity/HS-03-history-transition`   |
| HS-04 | Click count, word/line/output, threshold, endpoint, and rectangle semantics are explicit and stable.     | terminal gesture / Host selection       | `terminal-parity/HS-04-selection-gesture`    |
| HS-05 | Autoscroll, outside release, lost capture, blur, cancellation, and races have one copy consequence.      | terminal gesture / Host selection       | `terminal-parity/HS-05-selection-cancel`     |
| HS-06 | Selection eviction cannot export replacement content and clipboard completion remains pinned.            | Host selection / app                    | `terminal-parity/HS-06-selection-eviction`   |
| HS-07 | Plain export is maintained; HTML and VT exposure remain unavailable until accepted.                      | Host / terminal / app                   | `terminal-parity/HS-07-selection-export`     |
| HS-08 | Admitted programmatic and accessibility selection workflows preserve absolute endpoints.                 | Host selection / terminal               | `terminal-parity/HS-08-selection-commands`   |
| HS-09 | Scrollback is bounded, zero disables retention, and admitted updates evict deterministically.            | Host configuration                      | `terminal-parity/HS-09-scrollback`           |
| HS-10 | Copy-on-select, opt-out, manual copy, and exactly-once perceivable result remain coherent.               | terminal / app                          | `terminal-parity/HS-10-copy`                 |
| FS-01 | Search options, mappings, navigation, query seeding, reveal, and active selection remain coherent.       | terminal search / Host selection / web  | `terminal-parity/FS-01-search`               |
| FS-02 | Search scheduling, navigation intent, invalidation, cancellation, bounds, and reveal are deterministic.  | terminal search                         | `terminal-parity/FS-02-search-scheduling`    |
| FS-03 | Links cover a complete bounded logical line with precedence and exact cell mapping.                      | Host rows / terminal links              | `terminal-parity/FS-03-links`                |
| FS-04 | Hover invalidates on content, viewport, and geometry change; activation revalidates its cell.            | terminal links / geometry               | `terminal-parity/FS-04-link-invalidation`    |
| FS-05 | Filesystem activation remains unavailable without an accepted application capability.                    | app capability                          | `terminal-parity/FS-05-file-links`           |
| FS-06 | Presented external schemes and application activation policy are identical.                              | terminal / app capability               | `terminal-parity/FS-06-url-policy`           |
| AC-01 | Accessible rows retain spaces, grapheme mappings, positions, and a valid navigable structure.            | terminal accessibility                  | `terminal-parity/AC-01-accessible-rows`      |
| AC-02 | DOM and Host selection map both ways without splitting graphemes.                                        | terminal accessibility / Host selection | `terminal-parity/AC-02-dom-selection`        |
| AC-03 | Cursor and changed output announce in order without history becoming false live output.                  | terminal accessibility                  | `terminal-parity/AC-03-announcements`        |
| AC-04 | Deletion and line-kill announcements treat wide and surrogate graphemes atomically.                      | terminal accessibility / Host output    | `terminal-parity/AC-04-deletion`             |
| LC-01 | Open, rollback, retry, duplicate open, disposal, and DOM ownership are deterministic.                    | terminal lifecycle                      | `terminal-parity/LC-01-open-lifecycle`       |
| LC-02 | Failure, recovery, final frame, and exit have explicit precedence without geometry corruption.           | terminal / web lifecycle                | `terminal-parity/LC-02-failure-presentation` |
| LC-03 | Desktop and LAN Browser expose equivalent semantics for one Host checkpoint.                             | web / desktop / Gateway                 | `terminal-parity/LC-03-dual-client`          |
| LC-04 | PTY launch environment repairs locale and terminal identity and removes parent-only color policy.        | desktop / Host launch                   | `terminal-parity/LC-04-environment`          |
| LC-05 | LAN Browser capability loss changes Action availability and gives perceivable bounded feedback.          | web / Browser capability                | `terminal-parity/LC-05-lan-capability`       |
| LC-06 | Host title/current-directory metadata reaches Workbench with explicit rename and launch policy.          | terminal / web / Workbench              | `terminal-parity/LC-06-metadata-bridge`      |
| LC-07 | One resize owner and every non-owner present canonical geometry and recover ownership deterministically. | Host / desktop / web / terminal         | `terminal-parity/LC-07-resize-lease`         |
| LC-08 | Browser access defaults to loopback, requires explicit LAN intent, and revokes capabilities on stop.     | desktop / Server / Gateway              | `terminal-parity/LC-08-browser-access`       |
| LC-09 | Each page has isolated session/attachment lifetime and reopens from a full current snapshot.             | desktop / Gateway / Host                | `terminal-parity/LC-09-client-isolation`     |
| LC-10 | Exit, failure, stop, and reconnect are distinct, localized, readable, and actionable states.             | Workbench / web / terminal              | `terminal-parity/LC-10-outcomes`             |
| LC-11 | Recovery retains an honest last frame, has bounded retry, and exposes manual recovery.                   | web / terminal lifecycle                | `terminal-parity/LC-11-recovery`             |
| LC-12 | Typed feature failures preserve operation, correlation, retryability, and user consequence.              | Host / brokers / terminal / web         | `terminal-parity/LC-12-feature-failures`     |
| CF-01 | Recovery never restores browser VT authority or per-frame object-grid reconstruction.                    | all owners                              | `terminal-parity/CF-01-host-authority`       |
| CF-02 | Projection, history, search, links, accessibility, and revisions remain bounded.                         | Host / protocol / terminal              | `terminal-parity/CF-02-bounds`               |
| CF-03 | Visual evidence pins every domain-relevant environment input and normalizes no semantic difference.      | terminal / system evidence              | `terminal-parity/CF-03-visual-environment`   |

## Evidence and delivery

Every defect first gets a reduced failing regression at the earliest incorrect owner. Compatibility
claims use a pinned differential corpus or versioned golden trace. Geometry, timing, gesture,
focus, accessibility, and platform claims also require deterministic system traces with controlled
fonts, clocks, frames, geometry, capabilities, and interaction. Host concurrency or independently
ordered event changes update and run the owning TLA+ model when its abstraction changes.

Slices proceed in this dependency order:

1. projection and Host control (`PR-01` through `PR-13`);
2. surface geometry, theme, cursor, and runtime presentation (`SU-01` through `SU-16`);
3. scroll presentation and history (`SU-10`, `SU-11`, `HS-01` through `HS-03`, and `HS-09`);
4. input, focus, mouse, context, and Actions (`IN-01` through `IN-17`);
5. selection, search, links, and accessibility (`HS-04` through `AC-04`); and
6. application lifecycle, launch environment, and dual-client convergence (`LC-01` through
   `LC-12`).

One slice advances only after its owning current contract, failing-before regression, focused
checks, and affected package gates pass. Completion additionally requires the root `pnpm verify`
gate and every selected change-triggered correctness command.

## Reserved decisions

The active recovery does not authorize these choices:

- `IN-08`: a Desktop file-drop path capability;
- `FS-05`: filesystem target activation;
- `FS-06`: widening or deliberately narrowing non-Web external URL schemes;
- product-level bell effect aggregation across several attached views;
- the terminal-level shared-focus aggregation policy in `IN-14`;
- live attachment capability mutation beyond correct initial read-only behavior in `IN-10`;
- HTML or VT selection exposure in `HS-07`;
- the final product Action catalog, terminal context payload, and Workbench context-menu composition
  in `IN-09`, `IN-11`, and `IN-12`; and
- LAN Clipboard secure-context, transient-activation, and fallback policy in `LC-05`.

The implementation stops at the earliest affected boundary when one of these choices becomes
necessary. Existing safe capability restrictions remain in force until the decision owner accepts
an alternative and its evidence obligations.
