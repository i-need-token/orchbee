# Native Agent navigation

## Authority and grammar

Windows `agent focus` resolves a canonical or physical Agent to one navigable primary Panel and routes an
acknowledged focus request to one existing TUI Client. It accepts one positional native Agent
target, `--agent-session`, `--name` or `--panel`, with the existing precedence and mutual exclusions.
`--client`, `--request-id`, `--detach`, `--timeout` and `--format text|json` use the existing
Client control contract. The default total timeout is 30000 ms. Help, completion and parsing
remain offline. There is no new macOS or Linux runtime.

The command changes Client selection and explicitly acknowledges current known versions in the
selected Tab through the [Client attention owner](native-client-attention.md). This explicit effect
is separate from ordinary visible-paint acknowledgement and its outer-focus gate. Canonical Agent
lifecycle, semantic status and freshness remain Server observations.

## Model and owners

| Element    | Contract                                                                                                                                                                      |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem    | A script can start or resolve a named Agent but must manually translate its location into a Panel focus command.                                                              |
| Inputs     | Typed Agent target, optional Client/request identity, wait/format options, validated caller metadata, one snapshot, Agent and Client observations, deadline and cancellation. |
| State      | Parsed, admitted, Client selected, Agent resolved, exact primary Panel selected, one control submitted, observing or settled.                                                 |
| Output     | Existing Client control receipt, retained operation identity and wait state, or the owning typed read/selection/usage failure.                                                |
| Invariants | One Session and total deadline; no activation or reconnect; no terminal/process start, input, resize, controller transfer or operating-system activation.                     |
| Identity   | Agent resolver owns name/ID/Panel precedence; Workbench owns primary Presentation and binding; Client owns its view revision and applied focus.                               |
| Ambiguity  | Missing/ended/non-navigable Agent bindings and multiple matching primary Panels fail before control. Multiple eligible Clients require explicit disambiguation.               |
| Lifetime   | The CLI releases its Session before presentation and settles signal ownership through the common output finalizer.                                                            |
| Recovery   | A lost Submit is never repeated; its retained Client operation ID supports `client control-status`. Cancellation/timeout ends observation, not an accepted focus request.     |
| Evidence   | Pure grammar/selection counterexamples, scripted named-pipe traces, real Agent/TUI focus in Windows, locale/schema/completion checks and the root gate.                       |

## Transition boundaries

Parsing rejects conflicting selectors, extra operands and unknown options before runtime effects.
An explicit Client bypasses caller metadata; otherwise existing caller validation and unique-current
Client selection apply. Server admission and the initial snapshot share the original deadline.
Client selection is followed by the bounded Agent resolver and a canonical Get or physical Observe. An explicit
Panel selects its primary Presentation; a Session/name must have exactly one currently navigable
primary Panel in the snapshot. Child-only attachments are not substituted for primary content.

The resolved identity becomes `ClientViewIntent::AgentFocus`, retaining the Agent target and primary
Presentation as well as the Panel, selected Client view revision and minimum Workbench revision.
The Client rechecks its source and physical frame identity. Target disappearance or view changes
retain existing refusals. Agent/Workbench reads are not atomic with later navigation: the receipt
confirms the applied selection and current Client acknowledgement, not provider liveness or turn
completion at render time.

Every resolver/Get/control wait borrows the cancellation source and original deadline. A failed or
expired preflight submits nothing. Once submitted, known outcomes and uncertainty survive later
read failure, cancellation and timeout. The Server's retained operation can be inspected without
re-resolving an Agent name. No rollback or automatic replacement focus is attempted.

## Delivery and verification

No new command path or schema identity is added. `orchbee.client.control` v1 carries retained
Agent-focus operations. Existing Agent/Client selection failures keep
their source ownership. Human output and help are localized; JSON is locale independent.

Verify names, explicit IDs, Panel selectors, caller versus explicit Client routing, live primary
bindings and rejection of ended/missing/ambiguous/child-only bindings. Trace a single Submit and
retained Read after a lost reply; exercise partial response cancellation and one total deadline.
Actual two-TUI Windows evidence must check the selected Panel and unchanged other Client/OS focus,
with all created Agent processes, Sessions and consoles stopped afterward. Review is by the author;
TLA execution and independent review are not claimed. Final live focus/acknowledgement acceptance
remains pending the combined Windows verification; targeted checks alone do not discharge it.

The Client must report Ready or Busy; Starting and Unavailable skip Agent reads and report
`client-not-ready` with its Client ID. Busy admission permits the Client-owned mode-exit preflight
below; it does not guarantee that the Client can navigate.

## Agent identity and explicit acknowledgement

`agent-focus` retains the resolved canonical Session or full physical binding with its exact
TerminalContext, Tab and primary Panel. The CLI resolves once and checks the owning Agent source;
the Client revalidates the target before effects and completion. A physical target additionally
requires the selected frame's actual Host epoch/terminal identity before success. An old frame or
replacement cannot authorize focus acknowledgement. Child-only presentations are not substituted
with parent content; their navigable surface remains a separate lifecycle obligation.

The Client applies ordinary ordered navigation and completes after a successful draw, including an
already-focused target. Successful explicit Agent focus acknowledges the current known Agent
versions in the selected Tab, including zoom-hidden primary panes, even when outer focus is false.
This acknowledgement is an explicit command effect. Read-only attention and ordinary paint keep their existing restrictions.
Current snapshots are consumed at one UI boundary; later state changes remain unseen. Unreadable
target state prevents a success claim; an unavailable unrelated observation source supplies no
acknowledgement. No provider input or OS window activation is added.

Target loss, replacement, an interrupted operation or expiry cannot acknowledge. A refusal before
navigation is rejected; failure after an applied navigation step is incomplete. The retained
operation reports existing outcomes, and status reads never repeat the effect. Client control v1
carries this intent. The matched Client/Server cohort must be updated
together. Pure identity/acknowledgement and bounded controller/protocol checks precede final live
Windows acceptance; they do not by themselves prove multi-Client or authenticated-provider behavior.

## Returning to terminal mode

Agent focus is an effectful intent that can be delivered while the Client reports Busy. Ordinary
navigation and passive-read classification do not change. Before cancelling modes, the Client
checks the exact Agent source, usable terminal geometry, Ready or Empty session state, and absence
of pending refresh, submitted or uncertain structural operations and close recovery. A failed
preflight leaves modes intact. Unsubmitted rename/setup drafts, close confirmations and the Agent
board can be cancelled through their owners; retained operation receipts cannot be discarded.

The controller owns one mode-exit preparation per command. It requests a single shell-owned
overlay/prefix dismissal and asks every retained history viewport to exit through its existing
Recover queue. No provider input is sent. A recovering viewport remains owned until recovery and
a subsequent live frame complete; focus waits under the original operation deadline. Mode exit
counts as a partial effect even if later navigation fails. A later local input, new modal, target
replacement or expiry produces rejected/incomplete as appropriate and never acknowledges the
Agent. A repeated tick or status read cannot dismiss a newly opened mode. Decoded key presses,
paste and active pointer input notify the pending Agent-focus owner before input routing; focus
metadata, key releases, hover and previously captured packet tails do not interrupt it.

After mode exit, ordinary ordered navigation and post-draw identity/acknowledgement apply. Shared
admission and pure transition checks cover Busy versus Starting, one-shot dismissal, interruption,
expiry and failure before effects. Native owner checks cover drafts and history recovery with no
input; final combined Windows acceptance covers real shell overlays and Server/Client delivery.

## Sidebar display identity

The default Agent row prefers the user-assigned name, then a nonempty nonnumeric task title, then its provider. The bounded names read includes committed bindings and pending launch reservations, shares the observation deadline and byte limit, joins by the complete physical binding, and precedes the final physical report fence. Pending aliases do not claim canonical startup completion. Pane numbers remain navigation ordinals, not Agent names. Custom token rows keep their configured meaning.
