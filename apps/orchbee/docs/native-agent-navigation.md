# Native Agent navigation

## Authority and grammar

Windows `agent focus` resolves a canonical or physical Agent to one navigable primary Panel and routes an
acknowledged focus request to one existing TUI Client. It accepts one positional native Agent
target, `--session`, `--name` or `--panel`, with the existing precedence and mutual exclusions.
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

No new command path or schema identity is added. `orchbee.client.control` v5 carries retained
Agent-focus operations; older versions remain closed. Existing Agent/Client selection failures keep
their source ownership. Human output and help are localized; JSON is locale independent.

Verify names, explicit IDs, Panel selectors, caller versus explicit Client routing, live primary
bindings and rejection of ended/missing/ambiguous/child-only bindings. Trace a single Submit and
retained Read after a lost reply; exercise partial response cancellation and one total deadline.
Actual two-TUI Windows evidence must check the selected Panel and unchanged other Client/OS focus,
with all created Agent processes, Sites and consoles stopped afterward. Review is by the author;
TLA execution and independent review are not claimed. Final live focus/acknowledgement acceptance
remains pending the combined Windows verification; targeted checks alone do not discharge it.

The Client must be ready; otherwise Agent reads are skipped and the existing Client selection failure reports `client-not-ready` with its Client ID. Catalog v51 extends this v1 reason vocabulary without adding a schema.

## Agent identity and explicit acknowledgement

`agent-focus` retains the resolved canonical Session or full physical binding with its exact
TerminalContext, Tab and primary Panel. The CLI resolves once and checks the owning Agent source;
the Client revalidates the target before effects and completion. A physical target additionally
requires the selected frame's actual Host epoch/terminal identity before success. An old frame or
replacement cannot authorize focus acknowledgement. Child-only presentations are not substituted
with parent content; their navigable surface remains a separate lifecycle obligation. Busy modals
and history still use the existing refusal; matching Herdr's mode dismissal remains an open
interaction obligation.

The Client applies ordinary ordered navigation and completes after a successful draw, including an
already-focused target. Successful explicit Agent focus acknowledges the current known Agent
versions in the selected Tab, including zoom-hidden primary panes, even when outer focus is false.
This is an explicit command effect, matching fixed Herdr `app/agents.rs` `mark_active_tab_seen`, not
an automatic paint rule. Read-only attention and ordinary paint keep their existing restrictions.
Current snapshots are consumed at one UI boundary; later state changes remain unseen. Unreadable
target state prevents a success claim; an unavailable unrelated observation source supplies no
acknowledgement. No provider input or OS window activation is added.

Target loss, replacement, an interrupted operation or expiry cannot acknowledge. A refusal before
navigation is rejected; failure after an applied navigation step is incomplete. The retained
operation reports existing outcomes, and status reads never repeat the effect. Client control v5
carries this intent; older versions remain closed. The matched Client/Server cohort must be updated
together. Pure identity/acknowledgement and bounded controller/protocol checks precede final live
Windows acceptance; they do not by themselves prove multi-Client or authenticated-provider behavior.
