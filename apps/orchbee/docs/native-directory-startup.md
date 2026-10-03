# Native Windows directory startup

## Accepted model and ownership

Bare local Windows startup should make `cd <project>; orchbee` open the first terminal in that
directory. Only a canonical Site snapshot with no Project, Workspace or Tab, no valid saved
selection, and no all-checkpoints-corrupt restore notice admits this automatic creation.
Existing Sites use [startup selection](native-context-selection.md); a different launch directory
does not create another Workspace. Explicit Context and Agent targets retain their meanings.
Linux startup and the [interactive setup wizard](native-workspace-setup.md) are separate owners.

`local_startup` captures the process cwd once and shares its original 20-second absolute deadline.
The deterministic startup-directory policy consumes the validated Snapshot, optional saved
position, restore admission and an optional absolute UTF-8 directory. Absence or invalid directory
input refuses creation before ProjectCreate; it does not choose home. The directory label uses
the existing 512-scalar basename and localized root fallback; the Project uses the existing
localized Default label. Worker probes the literal directory and chooses the configured default
shell through the existing WorkspaceLaunch contract. No filesystem directory or Git worktree is
created by this policy.

Server remains the sole canonical navigation and terminal owner. The native worker issues one
revision-fenced ProjectCreate and one WorkspaceLaunch, using the existing typed LocalClient
submissions. Project, Workspace, Directory, context, Tab, Panel and presentation IDs come from
the Server receipts. A known receipt is retained before a fallible follow-up read. The shared
startup report owns only immutable invocation evidence; it never replaces canonical state or
grants input. Preferences keep their existing private format and draw acknowledgment.

## Workflow and outputs

| Input or transition                                                | Output and effect                                                                                                                                               |
| ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Saved position, nonempty Site or corrupt-checkpoint empty fallback | Existing startup path; no automatic Project or Workspace creation.                                                                                              |
| Eligible empty Site                                                | Prepare the exact cwd and labels before effects; do not issue standalone Open.                                                                                  |
| Project submission                                                 | One request under the captured revision and original deadline.                                                                                                  |
| Definite rejection or no handoff                                   | Stop; never automatically rebase or resubmit.                                                                                                                   |
| Unknown submission                                                 | Retain its request identity, retire the carrier and issue no later mutation.                                                                                    |
| Known Project commit                                               | Retain its identity, then require a same-Site snapshot at or beyond its revision containing that exact Project.                                                 |
| Workspace submission                                               | One existing WorkspaceLaunch atomically owns Directory, Workspace and first-terminal admission.                                                                 |
| Known terminal pending, failed, unavailable or unknown             | Retain all returned structural IDs; refuse automatic attachment, show retained Project/Workspace evidence and inspect commands.                                 |
| Known terminal started                                             | Require a same-Site snapshot at or beyond the receipt revision and the exact returned directory/context/Tab/Panel/primary presentation, cwd and Worker binding. |
| Validated created member                                           | Bootstrap that exact member through the existing Viewer, controller, geometry and first-frame owner.                                                            |
| Later read, attachment, frame or deadline failure                  | Keep known receipts visible; do not undo canonical data, create a replacement or fall back to another directory.                                                |
| Successful first draw                                              | Existing startup preference publication remembers the created selection; other live Clients keep their own focus.                                               |

The operation is complete only after existing first-frame readiness. Cancellation stops waiting,
not accepted Server work. A retained known or unknown outcome is reported in both locales before
startup returns failure. `orchbee project list` and `orchbee workspace list` inspect retained
membership. Automatic recovery of a pending terminal and TUI one-step Workspace creation/close
are later slices. There is no new wire, checkpoint, settings or persistent preference version.

## Correctness evidence and limits

`NativeStartupDirectory` models two Clients, eligible/ineligible startup input, one shared Site,
one possible automatic Project and Workspace, one independent revision write, one handoff per
phase, known/unknown results, read failures and first-frame publication. Revision fencing, empty
admission, preserved known receipts and attachment only after known Started are safety claims.
The peer write also abstracts competing ordinary standalone work. Deadlines and cancellation
project to failure; byte parsing, Worker directory probing, filesystem progress, complete Viewer
state and the previous selection model remain outside this abstraction. No liveness or fairness
claim is made, and no state constraint or symmetry reduction removes reachable model states.

Run `node scripts/tla/run-models.mjs --models native-startup-directory` for the maintained bounded
model and required action coverage. Deterministic policy and receipt tests, actual native
Worker/mailbox command-order traces, and Windows ConPTY empty-Site cwd/reopen/nonempty-Site
traces refine the owned claims. Root `pnpm verify` owns implementation regression checks.
These same-author development witnesses do not establish Windows Terminal manual acceptance,
real-provider behavior or completion of other pending model configurations.
