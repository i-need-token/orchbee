# Native location labels

## Scope and ownership

The native Client renames its current Workspace with `Ctrl+b r`. A Project is renamed from its sidebar row menu; Rename Project has no default key. `NoWorkspace` has no shortcut target. These unavailable actions return locally without a command. A drawn Project's Rename action still addresses that exact Project, including empty or noncurrent Projects, without selecting a child Workspace. Existing `project.rename` and `workspace.rename` own shared labels; [Workspace setup](native-workspace-setup.md) and [context selection](native-context-selection.md) retain their mutation/selection ownership.

The editor starts with the canonical label, preserves exact text up to 512 Unicode scalars, and
uses Enter to submit, Escape to cancel, Backspace to delete and Ctrl+u to clear. Each opening has
a fresh token and requires a complete first draw. Shared modal UTF-8, paste, fault and quiet-drain
rules remain unchanged. Target IDs, Session, revision and original label are captured at opening;
concurrent changes never retarget or automatically rebase a draft.

Workspace names must contain a non-whitespace Unicode scalar. The Rename dialog has one unlabeled
field and no context line. A blank or over-long draft disables Rename and shows "Enter a name." or
"Use at most 512 characters." under the field as soon as it is edited; Enter on it shows the error
without closing, changing its draft or changing the canonical label.
Accepted names retain their exact leading, trailing and internal spaces. The shared command
boundary applies this rule to Workspace create, launch and rename in Rust, TypeScript and JSON
Schema. Historical snapshot labels remain readable: blank Workspace labels display as localized
Unnamed Workspace in navigation and current-location chrome, and the normal rename editor can
repair the exact target. Tab and Project naming policies remain independently owned.

## Automatic Workspace names

Server owns optional boolean `Workspace.automaticLabel`. Omission means `false`, serialization
omits `false`, and readers never infer automatic naming from text, so legacy Workspaces keep their
exact names. Quick creation whose generated name was not edited, directory startup and
`workspace create` without `--label` launch automatic Workspaces; explicit names, worktree
checkouts, Panel relocation and `workspace.create` stay fixed.

An automatic Workspace follows its first terminal: the primary terminal of the first Tab's leading
Panel. Server's existing process sampler supplies that terminal's directory (a valid
terminal-reported directory, otherwise the root process directory). The first directory the
running Server samples for a Workspace, and each later sample that differs from the last one,
ignoring ASCII case and trailing separators, names it: Server commits the directory's final name
(on Windows the long spelling, when the sample gives an 8.3 short name) in one revision, adding
` (2)`, ` (3)` and onward while another Workspace uses it, exactly like
generated names. At creation that name is already the label; after a restart it corrects a name
the restored terminal's directory no longer matches. A repeated directory commits nothing, so
another Workspace's rename never reshuffles a suffix. A root directory, a name the label rule refuses, or an
unchanged name commits nothing. A replacement first terminal, such as after the first Tab closes,
takes over from its own next sample. The committed label reaches every Client, the CLI and the
checkpoint like any other rename; nothing is retried and the label is never reverted.

Server also owns optional `Workspace.terminalDirectory`, the first terminal's directory from the
same samples, for every Workspace whether its name is automatic or fixed. It drops a trailing
separator except at a drive root and is present only while it differs from the primary Directory
under the same comparison, so a sample back in the primary Directory omits it again. On Windows that
comparison also equates an 8.3 short spelling with the long spelling of the same existing folder, so a
long-form report from a short-form primary Directory records nothing and commits no revision. A new
directory and any new automatic name commit in one revision, and the checkpoint retains both. It
is an observation and never Directory identity; no Directory association changes. It is the Workspace's current directory: a Tab or Panel created without an explicit directory starts there, otherwise in the primary Directory, on the primary Directory's Worker. The sidebar's
location line names its folder, otherwise the primary Directory's, and a followed name and its
folder change together; [Machines](machines.md) owns how the row moves through a change.

`workspace.rename` fixes the name: it sets the text and clears `automaticLabel` together. Equal
text on an automatic Workspace is therefore a real change with a new revision, while equal text
on a fixed Workspace stays Unchanged. The editor prefill is the current label, so confirming it
unedited also fixes the name. Tab naming keeps its own [automatic/fixed contract](native-tab-labels.md).

Renaming preserves location identity, terminal bindings, directory associations and live shell
execution. Another Client observes new labels through explicit refresh. Existing Tab rename remains
independent.

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

| Event                                 | Result                                                                                                                                                                                                                                                                         |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Project-menu admission                | Require a live exact target/source proof and unexpired deadline; open one draft without navigation or a command.                                                                                                                                                               |
| Target/source/focus invalidation      | Discard the unsubmitted draft, release structural ownership and report refusal without retargeting.                                                                                                                                                                            |
| Submit                                | Freeze one exact Project or Workspace rename request after validating target, input and first draw.                                                                                                                                                                            |
| Unchanged                             | Require the captured revision and an unchanged original fixed label; preserve the retained known result.                                                                                                                                                                       |
| Renamed                               | Require checked successor revision, changed text or automatic mode; preserve the request before snapshot/UI handoff.                                                                                                                                                           |
| Exact result snapshot                 | Require only the requested label change, or no change for Unchanged; preserve all other projected canonical facts.                                                                                                                                                             |
| Later snapshot                        | Validate Session and exact target ownership, then display actual canonical labels and members, including peer changes.                                                                                                                                                         |
| Snapshot unavailable                  | Retain the known rename result; explicit `w` observes again without replaying the mutation.                                                                                                                                                                                    |
| Definite refusal or queued retirement | Preserve exact NotApplied evidence and existing terminal identity.                                                                                                                                                                                                             |
| Stale refusal                         | Report a short not-renamed fact that fits Compact status, resynchronize once and offer a separate rename-again hint.                                                                                                                                                           |
| Rename after stale refusal            | Prefill the refused text once for the same owner; another owner keeps the canonical label.                                                                                                                                                                                     |
| Ambiguous delivery                    | On a live multiplexed carrier, settle without retiring or locking structure, owe a publication read, and keep the persistent unknown-outcome notice until keypress; on a lost connection, retire the carrier with exact unknown evidence without retry or a fabricated result. |

Snapshot installation uses the existing membership owner. Label changes alone neither detach nor
reattach a terminal; real peer membership changes follow the existing exact cleanup rules.
Retirement preserves executor and mailbox evidence independently, including a settled prior action
and a newly queued action. Cancellation never undoes a committed label.

## Correctness evidence

Run `cargo test --locked -p orchbee -p orchbee-local-client --all-targets --all-features`. This
workflow has no separate TLA model. Shared identity, ownership and commit protocols retain their
separately registered core models.
