# Native settings

## Interaction and scope

The Windows global action menu opens Settings. The capturing overlay edits a local
draft: one of the three shipped themes, automatic background-Agent reminders, and
sound. Save and Apply is the sole submission action. Opening, editing and cancelling
an unsubmitted draft do not write configuration or change the installed theme.
Pane output continues. Tab/Up/Down (or j/k) selects a row; Left/Right or Space changes
its value. Enter on Save and apply, or a click on that drawn row, submits once.
Esc cancels an unsubmitted draft; after submission it hides the surface while the
save continues. The footer exposes the applicable cancellation or close action.
Settings replaces the complete status row and its background, so the editor's hint
does not retain an earlier notice, Shell hint or Machines indicator. The underlying
pane cursor stays hidden while Settings captures input.
The complete overlay requires at least 36 columns and 10 rows of content; a smaller
area closes it with an actionable notice. Opening/saving shows a fixed pending
indicator after 200 ms, with a two-second exchange deadline. Captured packet and
fragmented-paste tails remain owned through cancellation and the next redraw.

The Server writes the configuration location frozen at Site startup. Clients never
supply arbitrary file paths or TOML. Other configuration values and comments remain
intact. Turning reminders on preserves an already enabled delivery channel; an off
channel becomes the existing in-app channel. Manual notifications, delay, position,
per-Agent sound policy and unrelated settings keep their existing semantics.
Integration installation and first-run onboarding are separate surfaces.

## Domain and ownership

| Record field    | Contract                                                                                                                                                                                                                                                                                     |
| --------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem         | A local Windows user changes routine settings without editing TOML or interrupting running tasks.                                                                                                                                                                                            |
| Inputs          | A complete admitted configuration, exact source-content/absence token, publication revision, typed draft, drawn episode token, and one explicit save.                                                                                                                                        |
| Data            | One bounded editor episode per Client; three controlled settings; Server-owned source token, transaction and immutable publication.                                                                                                                                                          |
| Outputs         | Opened draft; no change; saved and applied; saved but not applied; pre-write refusal; or transport outcome unknown.                                                                                                                                                                          |
| Decisions       | Hidden/drafting/submitted phases; valid/invalid candidate; current/stale source and revision; write success/failure; live-apply success/failure; delivered/lost reply.                                                                                                                       |
| Dependencies    | Bounded settings reader, existing private atomic file owner, complete configuration preparation, authenticated local Workbench exchange and successful drawing.                                                                                                                              |
| Ownership       | Local-site-runtime owns TOML transformation and guarded file publication. The existing Server configuration Registry serializes saves and reloads. The existing Client configuration watcher owns complete admission and active application. Settings owns only its draft and one operation. |
| Model           | Finite safety workflow for two Clients, explicit draw and submission, a single writer, source conflicts, disk commit before live publication, cancellation and reply loss.                                                                                                                   |
| Reference scope | Windows local Site, three shipped themes and reminder/sound switches; no provider installation, remote writes, appearance auto-detection or persistent history.                                                                                                                              |

## Transitions and results

| State / event          | Guard and result                                                                                                                                                                           |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Hidden → open          | Read the Server authority and bounded source; freeze revision and content/absence token.                                                                                                   |
| Draft → edit / draw    | Keep changes local; revoke stale drawing authority until a successful new frame.                                                                                                           |
| Draft → save           | Require the current drawn episode; consume submission before dispatch, capture its input packet, and refuse repeated saves.                                                                |
| Submit → prepare       | One configuration transaction; reject observed revision/source conflict and invalid or unsafe source before writing. Validate the complete candidate and possible response before effects. |
| Prepare → disk         | Preserve all uncontrolled TOML values/comments; recheck the observed source and publish one flushed candidate atomically.                                                                  |
| Disk → active          | The original configuration owner applies and publishes the complete candidate; Clients use their existing complete admission.                                                              |
| Write fails            | Report no configuration save; keep the active publication.                                                                                                                                 |
| Apply fails after disk | Report saved but not applied; keep the old active publication. Do not roll back or repeat the saved write. Explicit reload remains available.                                              |
| Reply is lost          | Report outcome unknown; no automatic replay. Reopening is an explicit new observation.                                                                                                     |
| Cancel / close         | Before submission, discard the draft. After submission, hiding the surface cannot undo an accepted Server operation. A late result cannot act on a replacement episode.                    |

Source tokens compare bounded file bytes and distinguish absence from an empty
file. Cooperating writers use the existing private file lock. Observed changes
refuse saving. A noncooperating editor can race a final filesystem comparison;
this is not a transaction spanning arbitrary external editors. An absent target
uses no-replace creation. Unsafe, unreadable, malformed or over-limit files are
preserved with an actionable refusal, rather than repaired implicitly.

A saved receipt describes the completed save, not a promise that no subsequent
editor can change the file. Publication is complete and immutable per revision;
disk persistence and live application are distinct facts. No control, membership,
pane geometry, focus, clipboard policy or terminal program palette is changed.

A known apply must answer the exact request with the immediately next revision;
unchanged and retained-active results answer the captured revision. A requested
named theme must agree with a known applied or unchanged result. A mismatched
receipt is uncertain after submission and cannot authorize live application.

## Correctness and evidence

| Claim                                           | Required evidence                                                                           | Bounds                                                                                            |
| ----------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------- |
| Explicit submission has no automatic replay     | NativeSettingsSave safety model; deterministic editor/transport traces                      | Two Clients and two bounded episodes; no fairness or delivery guarantee                           |
| One transaction and complete active publication | NativeSettingsSave plus WindowsConfigurationPublication; Server failure/conflict traces     | Settings payloads are atoms; complete parser/profile admission remains an implementation boundary |
| Unknown settings and comments survive           | TOML decision cases, Unicode/inline/dotted source cases and exact uncontrolled-value checks | Three owned fields; 64 KiB existing source boundary                                               |
| Save and apply have independent outcomes        | File/write fault cases, preflight refusal, live-apply failure and lost-reply traces         | No compensation after a committed save; filesystem race limit above                               |
| Draw/input ownership and discoverable settings  | Dual-locale geometry/input cases and isolated native dual-window save                       | Normal Windows ConPTY; physical emulator appearance and authenticated providers are separate      |

The two abstract Clients are interchangeable model values; symmetry removes only
their labeling permutations, with both Clients and both episodes still admitted.
The abstract source generation refines exact observed content/absence tokens.
The model permits external edits between transaction phases, preserves other
fields at its write step, and allows disk and active state to differ. Publication
atoms refine complete admitted settings and diagnostics; the existing publication
model owns Client revision admission. Parser decisions and filesystem/process
boundaries require their own evidence and are not proved by the finite model.

Run `node scripts/tla/run-models.mjs --models native-settings-save,windows-configuration-publication`
and the required root `pnpm verify`. Evidence must match the final source; the table
lists obligations, not an assertion that each check has already completed.
