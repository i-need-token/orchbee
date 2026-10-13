# Native Windows directory startup

## Admission and ownership

Bare local Windows startup makes `cd <directory>; orchbee` open the first terminal in that directory. Automatic creation requires no Workspace in global navigation, no tabs in any terminal context, no valid saved selection, and no all-checkpoints-corrupt restore notice. Empty organizational Projects do not prevent admission and do not become the new Workspace's owner. An existing empty Workspace still prevents automatic creation. Existing Sessions use [startup selection](native-context-selection.md); changing launch directory does not create another Workspace. Explicit Context and Agent targets keep their meaning. Linux startup and [interactive Workspace setup](native-workspace-setup.md) have separate owners.

`local_startup` captures cwd once and shares its original 20-second absolute deadline. The deterministic startup policy consumes the validated snapshot, optional saved position, restore admission and optional absolute UTF-8 directory. Invalid or absent directory input refuses before any launch; it does not choose home. The Workspace label is the bounded 512-scalar basename or localized root fallback. Worker probes the literal directory and launches its configured default shell. This policy creates no filesystem directory or Git worktree and starts no AI program automatically.

The Server owns canonical navigation and terminal admission. Startup submits exactly one revision-fenced `WorkspaceLaunch` with `WorkspacePlacement::Ungrouped {}` and cwd, default geometry, no explicit launch and no profile. No ProjectCreate, Project receipt or Project-stage refresh occurs. Workspace, Directory, context, Tab, Panel and presentation IDs come from the launch receipt, whose `project_id` is nullable. Invocation evidence never replaces canonical state or grants terminal input.

## Workflow and failure evidence

| Transition                                                         | Effect and retained evidence                                                                         |
| ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------- |
| Existing Workspace/tab, saved position or corrupt-restore fallback | Existing startup selection; no automatic Workspace launch                                            |
| Eligible empty Session, including empty Projects                   | Prepare cwd and label; skip standalone Open                                                          |
| Launch                                                             | One request under the initial snapshot revision and original deadline                                |
| Definite rejection or no handoff                                   | Stop with no retained commit; no rebase or retry                                                     |
| Unknown handoff                                                    | Retain the original launch request identity and stop without later requests or replay                |
| Known terminal pending, failed, unavailable or unknown             | Retain the known Workspace receipt; refuse automatic attachment                                      |
| Known terminal started                                             | Read a same-Session snapshot at or beyond the receipt revision and validate the exact returned scope |
| Validated member                                                   | Bootstrap the exact member through existing Viewer/controller/geometry/first-frame owners            |
| Later read, attachment, frame or deadline failure                  | Preserve known receipt; do not create a replacement or fall back to another directory                |
| Successful first draw                                              | Existing preferences remember the created selection; other Clients retain their own focus            |

Scope validation checks Workspace/context/Tab/Panel/primary presentation identity, global Directory ID and cwd, and the Directory's Worker against the live binding. It rejects stale or foreign snapshots, missing membership and non-started launches. A Project created or changed after the ungrouped launch does not replace Workspace identity. Initial launch receipts must describe the requested ungrouped placement.

Completion still requires first-frame readiness. Cancellation stops waiting, not Server work already accepted. Both locales report the known Workspace ID or unknown request ID before startup returns failure; `orchbee workspace list` inspects retained state. There is no new startup-specific wire, checkpoint or preference format, and no automatic replay of pending or unknown launch.

## Evidence and limits

`src/terminal_session/startup_directory/tests.rs` covers admission with empty Projects, rejection with an empty Workspace, path/label rules, exact scope and retained evidence in both locales. `src/terminal_session/connection/native/tests/startup_directory.rs` checks one launch before Bootstrap, no standalone Open or ProjectCreate, retained known receipts after read failure, and unknown handoff without follow-up or replay.

Use the owning `startup_directory` Rust tests with the matched native dependency build. These tests cover bounded implementation traces; actual terminal appearance and full cohort acceptance belong to the native end-to-end suite. They do not claim exhaustive interleavings or other-platform coverage.
