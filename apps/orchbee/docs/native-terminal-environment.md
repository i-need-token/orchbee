# Native terminal environment

## Scope and ownership

This reference governs the native host-terminal environment that Orchbee owns at runtime: each new
Windows child environment, retained Windows endpoints, raw-record input and Unicode VT output,
viewport observation against transient zero geometry, synchronized-update framing of each drawn
frame, and the pointer-capture mode pair. Maintain it with the Terminal Host
`windows_environment.zig` adapter, `terminal_device.rs`, `terminal_device/windows.rs`,
`windows_console_input/records.rs`, `startup_wait.rs`, `terminal_lifecycle.rs`, `tui_backend.rs`,
`tui_shell.rs`, and the owning `crates/windows-console-host` boundary. Admission verifies TTY identity
and descriptor safety (interactive TTY on both streams, one shared terminal identity, a working
geometry query); obtaining a drawable cell geometry is a separate readiness concern governed by
[`native-startup-wait.md`](native-startup-wait.md). Host-reported pixel geometry that arrives
as zero is reported honestly as "pixels unknown" — a normal state, never a fabricated one.
Maintain pointer capability settlement with `terminal_pointer_admission.rs` and the outer-host
decoders under `terminal_input/` as well.

## Windows input ownership

The Windows adapter duplicates the admitted standard input and output handles once into the
`OwnedHandle`-backed `ConsoleInput` and `ConsoleOutput` ports supplied by
[`orchbee-windows-console-host`](../../../crates/windows-console-host/README.md). Mode operations,
raw record intake, output-window observation and Unicode output all use those exact retained
handles; the adapter does not reopen `CONIN$` or `CONOUT$`, nor resolve a process-standard endpoint
again after admission. The boundary crate's private `sys` module is the reviewed unsafe boundary
for these Win32 calls and tagged-union reads. The Client consumes its safe records and ports.

The admitted inherited console owns one canonical record reader across startup wait, the TUI and
direct attach. It retains UTF-16 surrogate and bounded repeat state through those phase changes;
there is no competing Crossterm event reader or background input thread. Each read yields at most
4096 bytes with a four-millisecond active-drain budget. Zero-virtual-key VT characters, including
C0 and bracketed-paste delimiters, reach the semantic decoder unchanged. Physical key and mouse
records reuse the existing byte encoding; raw Ctrl+C stays application input. The native readiness
count and following one-record read are separate Win32 calls, not an atomic reservation against an
external consumer. The bounded-read contract therefore requires that no outside reader drain the
same console input buffer between them; the canonical Client reader enforces that condition inside
the process.

UTF-16 text is assembled before UTF-8 encoding. A high surrogate stays pending across read
boundaries and character-free key records, including modifier and IME/process-key records;
ConPTY's character-bearing Alt releases use the same assembly path as zero-virtual-key text.
The completed pair emits one scalar with its repeat count, never two replacement characters.
An unpaired low surrogate, a high surrogate followed by another text/key boundary, or mismatched
repeat/modifier metadata refuses the read with `InvalidData` through the existing localized
interface-failure path; malformed input cannot become a saved label or shell payload. An idle
poll does not complete or replace a pending high surrogate. A literal user-supplied U+FFFD remains
ordinary text. `windows_console_input/records/tests/unicode.rs` covers native record splits,
character-free interleaving and bounded UTF-8 repeat tails; `workbench_client/ui/tests/input_provenance.rs`
and `workbench_state/tests/location_labels.rs` cover exact rename requests, grapheme editing,
cancel and location-label publication. These controlled implementation witnesses do not provide
exhaustive TLC assurance or establish real-window glyph rendering.

Raw mode keeps `ENABLE_VIRTUAL_TERMINAL_INPUT` enabled and disables cooked processing. Turning VT
input off lets ConPTY discard paste delimiters; interpreting its zero-key-code ESC through a
keyboard-layout lookup also loses the boundary. The pointer lease separately owns the native
mouse-input bit and its VT reporting writes, so raw admission with `--mouse off` does not capture
the mouse. Restoration reinstates the original saved console input/output modes. See Microsoft's
[console-mode contract](https://learn.microsoft.com/en-us/windows/console/setconsolemode) and
[paste delimiter filtering](https://github.com/microsoft/terminal/issues/18094).

## Windows child-process environment

Each new Windows terminal reads current Machine and User environment values at the Terminal Host's
process-spawn boundary. The deterministic projection starts fresh with a closed set of inherited
volatile Windows runtime and identity values plus per-drive current-directory entries, then applies
Machine and User registry strings using case-insensitive names. Machine `USERNAME` is excluded
because HKLM commonly records `SYSTEM` instead of the interactive user. An arbitrary process-only
name is not inherited: removing it from the registry removes it from the next terminal, while a
required process-only value belongs in the launch profile. The registry's unnamed default value is
ignored. Missing registry scopes are empty; an unreadable or malformed scope fails as unavailable,
and an out-of-bounds scope fails as too large, before PTY ownership begins.

Only `REG_EXPAND_SZ` values expand `%NAME%`. Dependencies resolve from the raw User-over-Machine
snapshot and then from admitted inherited runtime and private Orchbee values, independent of registry
enumeration order. Machine `PATH` is projected first, so `%PATH%` in an expandable User `PATH`
references the fresh Machine phase rather than the Host's stale `PATH`. Unknown names and the
recursive token in other cycles remain literal, while depth and size limits fail closed. Effective
`PATH` is expanded Machine `PATH` followed by User `PATH`, with exact case-insensitive duplicate
segments removed in first-occurrence order. If neither scope owns `PATH`, no inherited `PATH` is
synthesized.

Inherited `ORCHBEE_*` values are reapplied after registry projection. Authorized launch environment
entries and removals follow, and the creation-private Orchbee overlay follows them. Live Session and
explicit launch inputs therefore outrank registry data. Deferred creation reads the registry only
when process spawn is attempted, and a retry reads a new snapshot. A running terminal keeps its
original child environment. Non-Windows terminals retain their existing inherited-environment
projection.

The two registry scopes admit at most 256 enumerated values and 512 KiB of stored WTF-8 names,
values, and entry separators in aggregate. One admitted registry value, one expanded output, and the
combined `PATH` each admit at most 256 KiB. The final projected map admits at most 1,536 entries and
1 MiB of stored WTF-8 names, values, and entry separators. One projection admits 65,536 combined
expandable-value visits and token resolutions. `PATH` deduplication scans at most 4,096 input
segments, counting empty and duplicate segments. The real reader and pure projection return typed
failures rather than truncating or continuing unbounded work. Deterministic tests cover these
boundaries, name matching, expansion dependencies,
User `%PATH%` phase semantics, unresolved names, cycles, unnamed values, missing and unreadable
scopes, `PATH` order and deduplication, inherited admission, launch priority and separate-launch
refresh. A controlled native Windows witness must additionally keep the Session alive, change a
reversible User `PATH` entry, and prove only a subsequently created terminal resolves the new command
before restoring that entry.

## VT command output

After Windows has successfully enabled VT output processing, the lifecycle, startup-wait prompt,
and fixed-viewport renderer encode Crossterm commands with `Command::write_ansi` directly into the
writer supplied by their caller. This path does not run Crossterm's process-global Windows ANSI
probe, open `CONOUT$`, or fall back to a WinAPI operation against a different console. The encoder
allocates no intermediate command buffer, retains the first writer error, and never chooses a
flush boundary: lifecycle effects flush after their command, while the prompt and renderer flush
only at their existing complete-output boundary. A reported write error may follow an emitted
prefix; the encoder never replays that prefix or advances to later command fragments.

The exact-handle Unicode adapter uses `WriteConsoleW`. Each acquired writer owns one ordered
stream's incomplete UTF-8 tail, pending low surrogate and latched fault. A trusted short native
count is mapped back to a UTF-8 prefix; a split surrogate is completed exactly once before that
same writer accepts later bytes. A zero or impossible count, invalid UTF-8, or native error faults
that writer and causes later calls on it to return the retained error without another native
write. A native error has unknown effect and can follow an emitted prefix, so neither the adapter
nor its caller replays the failed payload. A fresh writer starts with empty encoding state, which
lets bounded lifecycle cleanup attempt complete ASCII control sequences after the frame writer has
faulted. It does not repair the failed stream, prove that no prefix escaped, or make console output
atomic.

Direct encoding is gated by the successful native VT-mode mutation. If that mutation fails before
the alternate-screen command is attempted, restoration emits no ANSI alternate-screen cleanup
that could appear as literal text. On an admitted path, cursor-show and alternate-screen leave are
attempted while VT processing is still active; restoring the saved output and input mode words is
the final native step. The two native restorations settle independently: both pending words are
attempted, and a word whose restoration succeeded is not written again merely because the other
word failed. Mode capture, VT admission, output, cleanup and restoration all address the same
retained input/output ports; the lifecycle owner, rather than the boundary crate, records and
settles the original-mode restoration debt.

## Host appearance query

After raw terminal admission and before the first TUI frame, the lifecycle writer sends one ordered
OSC `10` foreground query followed by OSC `11` background query. This write does not enable a
terminal mode or create restoration debt. The first frame draws immediately; the appearance probe
settles after a 350 ms deadline without waiting in the render path.

The ordinary and modal input decoders recognize both ESC `]` and C1 OSC reply forms. Only the exact
`10;` and `11;` selectors become query metadata. RGB (`rgb:rrrr/gggg/bbbb`, with one to four hex
digits per component) and `#RRGGBB` values are parsed within a 96-byte payload bound and one-second
frame deadline. The background is scaled to RGB8 and classified Dark below the weighted luminance
threshold `299R + 587G + 114B < 128000`; otherwise it is Light. Foreground-only, malformed,
oversized, or incomplete responses do not classify appearance; the probe waits until its deadline
for a valid background. An unresolved ESC `]` candidate keeps the ordinary escape deadline until an
exact selector claims it, preserving Alt+] behavior for other input. After a targeted frame times
out or exceeds its payload bound, the decoder drains toward BEL, C1 ST, or ESC `\\` for at most 98
bytes, 50 ms without input, or one second total. A late byte after those limits returns to ordinary
input because an unframed tail cannot be distinguished from new typing. Recognized late replies
remain inert after the original appearance deadline. The probe retains each first valid foreground
and background RGB value for local display resolution. A foreground received after a valid background
can complete that same observation before the deadline; it never changes the settled polarity.

If no valid OSC `11` background arrives by the deadline, Windows uses its fallback: the Client asks
`local-runtime` to read the current user's
`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
DWORD `1` means Light and `0` means Dark. Missing, unreadable, or non-boolean values stay Unknown;
foreground-only, malformed, partial, or absent background replies all trigger this fallback. Other
platforms stay Unknown when no valid background arrives. Late replies cannot revise the fallback.
The registry read is one-shot and read-only. This appearance probe
does not alter terminal colors, Windows personalization, launched-pane query defaults, or the
terminal mode set, and needs no cleanup beyond the normal lifecycle restoration.

## Viewport observation

The runtime observation of the inherited terminal distinguishes three outcomes. The distinction
is made on typed results, never on error text.

| Observation      | Meaning                                                                    | Runtime effect                                                                                                                       |
| ---------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| Valid geometry   | The host reported nonzero rows and columns                                 | A change applies a resize and a redraw; an equal geometry is a no-op                                                                 |
| Zero or absent   | The host transiently reported zero rows or columns                         | No resize is sent, the Session is not retired, the last valid geometry stays in effect, and the next valid geometry resumes normally |
| Inspection error | The descriptor or the query itself failed (permission, closed, invalid fd) | Fatal: the failure propagates and the interface exits                                                                                |

The shell may repeat the exact last-valid viewport observation while synchronizing a local live
divider draft. The existing physical controller deduplicates identical projected geometry, so a
repeat sends no extra Resize. An owned layout change is separate from a Host observation and grants
no controller authority. A new graphics cell measurement applies only with a nonzero Host sample;
an absent sample cannot change the retained cell or pixel dimensions.

A pixel extent of zero is a normal observation, not an error: the cell-pixel pair is reported as
`(0, 0)` — "pixels unknown" — while the character grid keeps working. When the host reports only
one pixel axis, or a derived cell-pixel axis rounds down to zero, the whole pair is reported
unknown rather than fabricating the missing axis. A cell-size change whose pixel extent reads
zero is likewise never resolved by reusing the previous pixel values — old pixels are never a
measure of a new size. Known → unknown → known transitions ride the same geometry path; every
state is reported as measured, and downstream pixel-dependent features gate on the pair being
known rather than on any estimate.

## Pointer capture lifecycle

Pointer reporting owns one tracking level plus SGR cell encoding. Admission first establishes the
button-event baseline by disabling any-motion (`?1003l`) and enabling `?1002h` with `?1006h`.
After that complete baseline and the initial viewport synchronization and paint, the lifecycle-controlled output path issues one ordered `?1003$p`
DECRQM followed by DA1 as a completion sentinel. Only a `Reset (2)` DECRPM reply admits a global VT mode upgrade. A
`Set (1)` reply contradicts the established baseline and settles unknown; `0`, `3`, and `4` settle
unavailable; DA1 arriving first or the bounded deadline expiring settles unknown. Because the
reply has no query identifier, an endpoint is never probed again: late or duplicate replies are
consumed but cannot change its settled result.

The ordinary and modal host decoders recognize the same exact replies in both ESC-CSI and C1-CSI
form. They accept only `?1003;[0-4]$y` as DECRPM and a DA1 response made from one or more nonempty
decimal parameters. A complete private CSI is host metadata only when it has the DA1 `c` or DECRPM
`$y` reply final; malformed reply-shaped frames are consumed without becoming keys, editor input,
or pane bytes. Other complete private CSI remains generic non-decision input and preserves raw
passthrough. A run that reaches its time or size bound before a reply final likewise returns to the
bounded generic drain instead of extending metadata ownership indefinitely. Bracketed-paste
payloads stay literal and are never inspected for these replies.

An admitted result switches from `1002` to `1003` by disabling the old level before enabling the
new one. Unknown and unavailable results retain `1002`, so click, drag, and wheel work while idle
hover remains absent unless actual Windows console movement proves observation or the scoped menu policy below applies. Global capture selects one tracking
level; `1006` remains its encoding.
Capture release and re-arm reuse the endpoint's settled result rather than issuing another query.
An admitted switch is published to pointer interaction state only after the complete mode write
succeeds. A failed switch publishes no hover authority and is never retried by the running input
loop; the fatal path proceeds to full lifecycle cleanup. Global admitted any-motion serves every
painted chrome control, including action menus. Outside menus, ButtonOnly fallback has click, drag,
and wheel without idle hover unless Windows console movement has independently been observed.

The Windows console reader can also deliver native button-free movement independently of the VT
query. The first unmodified motion received while capture is armed admits hover presentation for
this endpoint, including sidebar, Tabs and modal controls. It does not modify the settled query,
issue a new query, enable a host mode, activate a control, or re-arm released capture. Paste content
cannot supply this evidence. Release clears hover; re-arm may reuse observation capability but
never an old position or gesture. This follows the [Windows console input contract](https://learn.microsoft.com/windows/console/setconsolemode).
Input batches retain their receive timestamp for bounded host replies; session work between read
and dispatch cannot turn an already-received reply into a timeout.

An action menu with already armed capture additionally owns a scoped `MenuMotion` request when
global any-motion is not admitted. It writes `1002h`, then `1003h`, retaining `1006`: a host ignoring
`1003` keeps the button fallback, and actual motion reports are required before feedback appears.
This request neither changes the one-shot probe result nor assumes physical delivery. Menu exit
restores `1003l`, then `1002h`; an independently admitted global upgrade supersedes the menu request
and remains active on exit. Keyboard opening while capture is released performs no mode write.
Mode effects complete before the dispatcher publishes this local report admission; a failed write
is fatal and never retried, and lifecycle teardown clears every owned mode. The existing native
ConPTY witness covers the request, semantic motion through the inherited reader, return to silent
button-only motion, global admission and full restoration; physical terminal review remains separate.

The tracking level and encoding share one `ModeLease`, armed at startup unless `tui --mouse off`
declined it. A pending restoration obligation is registered before every native or VT mutation.
A partial enable or level switch therefore enters transition-unknown state and cannot attempt a
runtime fallback; only the complete release sequence (`?1003l`, `?1006l`, `?1002l`) settles it.
Runtime release through `Ctrl+B m`, re-arm, startup arming, error cleanup, and panic cleanup use
that same lease. Gesture routing, hit testing, state clearing on downgrade or release, and pane
passthrough live in [native mouse input](native-mouse-input.md); this section owns capability
settlement and the mode lifecycle.

## Synchronized frame output

Each drawn frame — the complete cell diff plus its cursor placement — is wrapped in exactly one
synchronized-update pair and published by one flush. `terminal_lifecycle` is the single owner of
the pair: it defines the begin and end sequences, tracks the open-pair obligation on a shared
lease, and owns recovery. The backend participates through that lease only: it registers the
obligation before the begin marker is attempted, so a partially written begin still owes an end,
and it clears the obligation only after both the end marker and the frame flush have been
accepted by the writer. Whenever delivery cannot be confirmed, the pair stays pending.

| State                 | Effect                                                                                             |
| --------------------- | -------------------------------------------------------------------------------------------------- |
| Update closed         | A draw operation registers the obligation, emits the begin marker, and moves to update open        |
| Update open           | Cell, attribute, and cursor writes accumulate inside the pair                                      |
| Update close          | A flush emits the end marker and flushes the writer once; both must succeed to confirm the closure |
| Update open + failure | The first failure is retained; a later flush still attempts the end marker                         |
| Begin write failure   | The obligation stays registered; the next flush still attempts the end marker                      |
| Unconfirmed end       | The pair stays pending; the next successful flush re-emits the end marker, or restoration ends it  |

Failure keeps its first cause. A flush failure still attempts the end marker; an end marker
that itself fails is reported after the attempt, never silently retried inside one frame. A
draw failure abandons the incomplete frame instead: on the error path the frame writer is
fenced before it is dropped, so neither its buffered residue nor its drop-time retry reaches
the terminal. Bytes already written cannot be retracted; the still-open pair is ended through
the control channel at restoration. The shell also invalidates the prior frame's hit map and
armed controls before returning the fatal draw error. Neither the incomplete candidate nor
the old frame authorizes another interaction. This loop exits without replay; a recovery
implementation would have to establish a new terminal baseline and complete a full redraw.

Restoration is lifecycle-owned and runs in a fixed order: it first blocks further frame output,
then ends a still-open pair through the terminal control path inside the same bounded retry
budget as every other mode, and only then disables bracketed paste, disables pointer capture,
shows the cursor, leaves the alternate screen, and restores raw mode. On Windows, VT output stays
enabled through cursor and alternate-screen cleanup, then the raw restore reinstates both saved
native mode words. The frame writer is restoration-gated: once restoration begins, buffered frame
bytes are discarded instead of reaching a terminal that is already being restored, which also
covers a panic-path unwind flush. After an output failure the same fence is applied before the
frame writer is dropped, so the writer's drop-time retry cannot emit the abandoned frame either.
A clean close owes nothing and emits nothing extra. This contract promises neither a single
operating-system write per frame nor visual atomicity on hosts that do not support the
synchronized-update mode.

## TUI timing observations

`tui_shell/measurement.rs` owns passive loop and writer measurements. Only entry to the actual TUI
reads `ORCHBEE_TUI_MEASUREMENTS`; it validates an absolute file path and uses create-new before
acquiring TUI modes. The parent directory must already exist. An existing file is preserved and
refused. An absent variable leaves no journal or file and samples no clocks on the hot path.
The selected file belongs to the caller, who releases it after the last diagnostic consumer.

Enabled collection preallocates storage and retains at most 65,536 fixed-shape samples. The loop
performs no diagnostic file I/O, serialization, background work or terminal output. A decorator under the existing output buffer
and restoration fence forwards each writer operation exactly once, returning the original count or
error. It adds no flush, drop-time write, retry, frame acknowledgement or scheduling decision.
Lifecycle mode effects retain their existing writer and acquire only a call-duration observation.

After the loop, the shell fences a failed frame, drops the frame writer and surface, and finishes
terminal restoration before serializing diagnostics. Restoration is attempted even after runtime
failure; a diagnostic failure cannot replace the primary runtime or restoration cause. An early
initialization failure, panic or forced termination may leave an empty or incomplete file; absence
of valid completed JSON is an aborted measurement, never a successful zero-duration run.

The machine document has schema `orchbee.tui.measurements`, version `1`. It contains the process ID,
a bracketed UTC/monotonic alignment when available, lifecycle results, sample limit and
dropped/abandoned/outstanding/failed counts, an invalid-clock-or-count flag, `complete`, and samples.
Sample intervals use monotonic `started_ns`/`ended_ns` relative to the run origin. The UTC alignment
places its wall-clock value between `monotonic_before_ns` and `monotonic_after_ns`; it is not an exact
wall-clock origin. Samples are stored in completion order, so a nested writer call precedes its
enclosing frame record. No sample carries input bytes, terminal content, commands, paths,
environment values or credentials. Source, tree, build profile and executable hashes come from the
separate verified cohort provenance.

| Event              | Observation and limit                                                                                                                                                                                                    |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `input_read`       | One input-poll interval, ready/idle/failed result and byte count; each completed ready packet has a distinct `batch`. The poll includes intentional waiting and active draining, not just queue residence.               |
| `dispatch`         | Dispatch of that completed application packet, correlated by `batch`. Its start minus read completion is application-ready delay. A packet may decode zero or several events; split sequences can settle on later turns. |
| `frame`            | Frame ID, latest observed input batch, buffer-start/prepared checkpoints and completed draw result. The batch is context, never proof that this frame contains the expected feedback or useful terminal content.         |
| `writer_call`      | Forwarded `write`, `write_all` or `write_vectored` interval and requested/accepted counts. A failed call may have an unknown emitted prefix. Call wall time includes conversion, CPU and any native/host blocking.       |
| `flush`            | The existing writer's flush interval and result. Successful completion is not physical host paint.                                                                                                                       |
| `terminal_control` | An existing lifecycle-effect call interval and result; payload and user-controlled text are excluded.                                                                                                                    |

Frame and writer intervals are nested observations, not independent costs to add together. Buffer
preparation can dominate measured frames without dominating the complete run. Compute the union of
recorded intervals against the first recorded sample start through the last recorded sample end,
not the full process or Session lifetime, and leave uncovered wall time unclassified; it is not
automatically idle, CPU work or blocking. The post-preparation frame tail includes diffing, encoding,
bookkeeping and output unless a separate observation distinguishes them.

A successful short write is an accepted prefix. When the existing buffer submits its remaining
suffix, adding requested counts counts those suffix bytes again. Validate each complete forwarding
chain before interpreting byte totals: requested minus accepted is neither lost output nor proof of
backpressure. Call duration and successful flush still do not establish host paint.

Windows console records supply no arrival timestamps. These samples do not measure OS-console queue
residence, user-device time, activation before TUI entry, first useful content, or canonical Ready
acknowledgement. Real-window expected-feedback probes supply separate visible-response bounds under
[TUI verification](../../../docs/tui/verification.md#responsiveness-and-spatial-stability).

`complete` requires successful runtime and restoration, no dropped, abandoned, outstanding or failed
samples, and valid clocks/counts. A cap, exhausted identity/count, unfinished span, failed call or
serialization error cannot establish complete latency distributions. A decoder no-op or a frame's
latest input batch is not a feedback acknowledgement. Retain failure and timeout observations when
comparing runs; measure enabled/disabled perturbation with the same cohort and scenario.

The adjacent measurement tests cover disabled clock/storage absence, cap and incomplete accounting,
packet/frame correlation through the actual loop, writer short/error/vectored/default-method seams,
the real buffer/restoration fence, and restoration-before-serialization with primary-error
preservation. Non-TUI process evidence must also prove that the new environment input has no path
side effects outside TUI execution. These are instrumentation claims; they do not establish a
response budget or a different writer/scheduler architecture.

## Outer title restoration

[Windows title publication](windows-terminal-title.md) has its own lifecycle lease. The first title write captures the inherited title; restoration repays this lease after ending synchronized output and before disabling bracketed paste. Capture or write failure reports a localized status notice and keeps the TUI usable; any already-owed restoration remains registered. No title writes occur after restoration starts. Before an effective configured or explicit title is delivered, no title capture or write occurs.

## Explicit detach feedback

After a successful user detach (including the configured d/q exit aliases), the shell restores terminal modes, cursor and alternate screen before writing one localized human line to stderr. The line explains that detaching keeps terminal programs running and gives the return command; a remote machine includes its stable profile selector. It uses the final effective saved locale. Signal exits and runtime/restoration failures do not print a successful detach message. Standard output and machine documents receive no added text.
