# Native location labels

## Scope and ownership

The native Client renames its current Workspace with `Ctrl+B r`, or that Workspace's Project
with `Ctrl+B Shift+R`. Standalone has neither shortcut target and rejects these actions locally without a command.
The Rename action on a drawn Project opens the same editor for that exact Project, including
empty and noncurrent Projects, from a Workspace or Standalone. It never selects a child Workspace.
The Server's existing `project.rename` and `workspace.rename` actions own shared labels. This
reference extends [Workspace setup](native-workspace-setup.md) and
[context selection](native-context-selection.md); it does not introduce another mutation owner.

The editor starts with the canonical label, preserves exact text up to 512 Unicode scalars, and
uses Enter to submit, Escape to cancel, Backspace to delete and Ctrl+U to clear. Each opening has
a fresh token and requires a complete first draw. Shared modal UTF-8, paste, fault and quiet-drain
rules remain unchanged. Target IDs, Site, revision and original label are captured at opening;
concurrent changes never retarget or automatically rebase a draft.

Renaming preserves location identity, terminal bindings, directory associations and live shell
execution. Another Client observes new labels through explicit refresh. Zero-Workspace Project
navigation, deletion, reordering Projects or Workspaces, directory rebinding, Panel layouts, remote,
persistence and packaging are outside this slice. Existing Tab rename remains independent.

## State and result contract

The existing structural slot, carrier, monotonic operation budget and immutable worker-return
evidence serve these actions. Drafts, pending commands and unresolved known results exclude
competing structural work. No command is allocated on open, edit, cancel or failed admission.

A Project-menu request captures the complete target Project separately from the initiating
Site, revision, context, desired pane and focused pane incarnation. The original menu deadline
bounds mailbox lock acquisition and is checked again under the admission lock. Successful admission
ends that deadline; human editing is untimed and submission starts the existing operation budget.
The draft displays and edits the requested Project, even when another Project has the same label.
Removed or replaced targets and changed source/focus actively discard the draft and release capture
with a localized refusal. A fresh frame is required before terminal input resumes. This proof ends
at accepted submission: the committed rename cannot invalidate its own retained receipt or recovery.

| Event                                 | Result                                                                                                              |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------- |
| Project-menu admission                | Require a live exact target/source proof and unexpired deadline; open one draft without navigation or a command.    |
| Target/source/focus invalidation      | Discard the unsubmitted draft, release structural ownership and report refusal without retargeting.                 |
| Submit                                | Freeze one exact Project or Workspace rename request after validating target, input and first draw.                 |
| Unchanged                             | Require the captured revision and unchanged original label; preserve the retained known result.                     |
| Renamed                               | Require checked successor revision and changed text; preserve the exact request before snapshot or UI handoff.      |
| Exact result snapshot                 | Require only the requested label change, or no change for Unchanged; preserve all other projected canonical facts.  |
| Later snapshot                        | Validate Site and exact target ownership, then display actual canonical labels and members, including peer changes. |
| Snapshot unavailable                  | Retain the known rename result; explicit `w` observes again without replaying the mutation.                         |
| Definite refusal or queued retirement | Preserve exact NotApplied evidence and existing terminal identity.                                                  |
| Ambiguous delivery                    | Retire the carrier with exact unknown evidence, without retry or a fabricated result.                               |

Snapshot installation uses the existing membership owner. Label changes alone neither detach nor
reattach a terminal; real peer membership changes follow the existing exact cleanup rules.
Retirement preserves executor and mailbox evidence independently, including a settled prior action
and a newly queued action. Cancellation never undoes a committed label.

## Correctness evidence

A bounded location-label model covers both targets, unchanged and changed results, first draw,
revision races, exact identities, retained known results, explicit observation and at-most-once
submission. Existing Server and membership models retain their own canonical-commit and cleanup
obligations. Deterministic tests cover typed outcome compatibility, ownership, no-op and overflow,
same-packet input, stale draw and retirement evidence. Linux native process tests exercise both
rename controls, peer refresh, cancellation and retained shell identity. These development
witnesses do not establish installed or cross-platform readiness.

[`NativeLocationLabels`](../formal/tla/NativeLocationLabels.tla) explores one Client and Site,
one Project owning one Workspace and an independent Project, two label values, two request/draft identities, and one peer
label or membership write. Its 57 required actions include explicit target handoff, deadline expiry,
proof invalidation and active refusal settlement, alongside overlapping prior executor and later
queued mailbox evidence. These finite bounds do not limit product interactions. Exact UTF-8 and
scalar accounting, integer overflow, physical mailbox cells and full Worker behavior remain
deterministic-test or lower-model obligations.

`ProjectRenamePreservesOrigin` separates the target from the initiating location;
`ProjectRenameAdmissionIsFresh` bounds admission, while `SubmissionCapturesExactDraft` and existing
receipt properties retain the chosen identity. Proof invalidation abstracts target, source and focus
changes; deterministic cases distinguish their records and nonfirst Tab/Panel focus. Owner cases
cover duplicate labels, Empty sources, cancellation, one handoff, unchanged/changed receipts, exact
snapshot deltas and read-only recovery. The target extension is source-reviewed evidence; no executed
TLC, native test or host result is claimed for it here.
