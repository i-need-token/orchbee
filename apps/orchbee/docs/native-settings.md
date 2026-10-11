# Native settings

## Interaction and scope

On Windows, the fixed sidebar Settings row, `Ctrl+B F4`, or the global action
menu opens Settings. Opening ends sidebar capture and closes its narrow overlay, so
cancellation returns input to the visible terminal. The capturing overlay edits a local draft: one of the three shipped themes, automatic background-Agent reminders, and
sound. Save and Apply is the sole submission action. Opening, editing and cancelling
an unsubmitted draft do not write configuration or change the installed theme.
Pane output continues. Tab/Up/Down (or j/k) selects a row; Left/Right or Space changes
its value. Enter on Save and apply, or a click on that drawn row, submits once.
An unchanged draft keeps Save and apply disabled even when selected, with a
visible no-changes reason. Selecting an available save shows Enter in the
contextual footer; other settings show their applicable editing keys.
Esc cancels an unsubmitted draft; after submission it hides the surface while the
save continues. The footer exposes the applicable cancellation or close action.
Settings replaces the complete status row and its background, so the editor's hint
does not retain an earlier notice or Shell hint. The underlying
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

The selected theme supplies the rendered default foreground and background for
live and retained terminal cells. Terminal default uses the host's default colors
through `Color::Reset`, independently of the frame-provided RGB defaults.
Program colors explicitly set on cells retain their exact values;
default markers, rather than RGB equality, determine which colors follow the
theme. This presentation does not rewrite the terminal program palette or its
color-query responses. See [native themes](native-themes.md) for default-marker
projection and OSC default-color behavior.

A known apply must answer the exact request with the immediately next revision;
unchanged and retained-active results answer the captured revision. A requested
named theme must agree with a known applied or unchanged result. A mismatched
receipt is uncertain after submission and cannot authorize live application.

## Correctness and evidence

`SettingsCommitPublication.tla/.cfg` checks two independently prepared saves, one Server writer and one external edit. Each Client submits at most once. `Prepare` freezes file/publication versions; `Acquire` reserves the writer; `Write` rechecks the source; `Publish` installs one complete revision after a successful write. Reply loss records an unknown outcome without automatic replay. Apply failure preserves the committed write without claiming successful live publication. Payloads are atoms; there is no delivery or filesystem-atomicity assumption.

The checked invariants cover single-writer ownership, unrelated-setting preservation, committed-write provenance, complete immutable publication and honest receipts. The mutation suite removes the write-time source check and must find an overwrite counterexample. Editor drawing, draft validation, all configuration fields, atomic replacement and concrete conflict/failure receipts remain covered by native settings, Server configuration and local-site-runtime settings tests. This core does not enumerate UI episodes or prove filesystem durability.

Run `node scripts/tla/run-models.mjs --models native-settings-save` and the owning Cargo tests. The root gate remains required.
