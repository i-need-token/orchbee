# Native location labels

## Scope and ownership

The native Client renames its current Workspace with `Ctrl+B r`, or that Workspace's Project with `Ctrl+B Shift+R`. An ungrouped Workspace has no Project shortcut target; `NoWorkspace` has neither shortcut target. These unavailable actions return locally without a command. A drawn Project's Rename action still addresses that exact Project, including empty or noncurrent Projects, without selecting a child Workspace. Existing `project.rename` and `workspace.rename` own shared labels; [Workspace setup](native-workspace-setup.md) and [context selection](native-context-selection.md) retain their mutation/selection ownership.

The editor starts with the canonical label, preserves exact text up to 512 Unicode scalars, and
uses Enter to submit, Escape to cancel, Backspace to delete and Ctrl+U to clear. Each opening has
a fresh token and requires a complete first draw. Shared modal UTF-8, paste, fault and quiet-drain
rules remain unchanged. Target IDs, Session, revision and original label are captured at opening;
concurrent changes never retarget or automatically rebase a draft.

Workspace names must contain a non-whitespace Unicode scalar. The editor refuses blank input
without closing, changing its draft or changing the canonical label, and shows localized validation.
Accepted names retain their exact leading, trailing and internal spaces. The shared command
boundary applies this rule to Workspace create, launch and rename in Rust, TypeScript and JSON
Schema. Historical snapshot labels remain readable: blank Workspace labels display as localized
Unnamed Workspace in navigation and current-location chrome, and the normal rename editor can
repair the exact target. Tab and Project naming policies remain independently owned.

Renaming preserves location identity, terminal bindings, directory associations and live shell
execution. Another Client observes new labels through explicit refresh. Zero-Workspace Project
navigation, deletion, reordering Projects or Workspaces, directory rebinding, Panel layouts, remote,
persistence and packaging are outside this slice. Existing Tab rename remains independent.

## State and result contract

The existing structural slot, carrier, monotonic operation budget and immutable worker-return
evidence serve these actions. Drafts, pending commands and unresolved known results exclude
competing structural work. No command is allocated on open, edit, cancel or failed admission.

A Project-menu request captures the complete target Project separately from the initiating
Session, revision, context, desired pane and focused pane incarnation. The original menu deadline
bounds mailbox lock acquisition and is checked again under the admission lock. Successful admission
ends that deadline; human editing is untimed and submission starts the existing operation budget.
The draft displays and edits the requested Project, even when another Project has the same label.
Removed or replaced targets and changed source/focus actively discard the draft and release capture
with a localized refusal. A fresh frame is required before terminal input resumes. This proof ends
at accepted submission: the committed rename cannot invalidate its own retained receipt or recovery.

| Event                                 | Result                                                                                                                 |
| ------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| Project-menu admission                | Require a live exact target/source proof and unexpired deadline; open one draft without navigation or a command.       |
| Target/source/focus invalidation      | Discard the unsubmitted draft, release structural ownership and report refusal without retargeting.                    |
| Submit                                | Freeze one exact Project or Workspace rename request after validating target, input and first draw.                    |
| Unchanged                             | Require the captured revision and unchanged original label; preserve the retained known result.                        |
| Renamed                               | Require checked successor revision and changed text; preserve the exact request before snapshot or UI handoff.         |
| Exact result snapshot                 | Require only the requested label change, or no change for Unchanged; preserve all other projected canonical facts.     |
| Later snapshot                        | Validate Session and exact target ownership, then display actual canonical labels and members, including peer changes. |
| Snapshot unavailable                  | Retain the known rename result; explicit `w` observes again without replaying the mutation.                            |
| Definite refusal or queued retirement | Preserve exact NotApplied evidence and existing terminal identity.                                                     |
| Stale refusal                         | Report a short not-renamed fact that fits Compact status, resynchronize once and offer a separate rename-again hint.   |
| Rename after stale refusal            | Prefill the refused text once for the same owner; another owner keeps the canonical label.                             |
| Ambiguous delivery                    | Retire the carrier with exact unknown evidence, without retry or a fabricated result.                                  |

Snapshot installation uses the existing membership owner. Label changes alone neither detach nor
reattach a terminal; real peer membership changes follow the existing exact cleanup rules.
Retirement preserves executor and mailbox evidence independently, including a settled prior action
and a newly queued action. Cancellation never undoes a committed label.

## Correctness evidence

The owning implementation tests cover Unicode and byte limits, duplicate labels, Empty and noncurrent targets, origin preservation, exact publication, stale draw and unknown-result non-replay.

Maintain the concrete witnesses in `src/localization/tests/location_labels.rs`, `crates/local-client/src/local_client/tests/workbench/workspace_setup/location_labels.rs` and the adjacent state, carrier, input and rendering suites. Use controlled completion/clock order and generated trace replay where the workflow has multiple outcomes.

Run `cargo test --locked -p orchbee -p orchbee-local-client --all-targets --all-features`. The application and transport witnesses run with their owning crates. This workflow has no separate TLA model. The evidence covers named/generated implementation traces; it does not establish exhaustive interleavings, physical terminal appearance or untested platforms. Shared identity, ownership and commit protocols retain their separately registered core models.
