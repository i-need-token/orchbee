# Native Agent navigation

## Authority and grammar

Windows `agent focus` resolves a live Agent to one navigable primary Panel and routes an
acknowledged focus request to one existing TUI Client. It accepts one positional native Agent
target, `--session`, `--name` or `--panel`, with the existing precedence and mutual exclusions.
`--client`, `--request-id`, `--detach`, `--timeout` and `--format text|json` use the existing
Client control contract. The default total timeout is 30000 ms. Help, completion and parsing
remain offline. There is no new macOS or Linux runtime.

The command changes Client selection. It does not acknowledge Herdr-style seen/unseen state;
that parity obligation remains open. Canonical Agent lifecycle, semantic status and freshness
are observations, not state that a focus operation may rewrite.

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
Client selection is followed by the bounded native Agent resolver and one Agent Get. An explicit
Panel selects its primary Presentation; a Session/name must have exactly one currently navigable
primary Panel in the snapshot. Child-only attachments are not substituted for primary content.

The resolved Panel identity becomes an existing `ClientViewIntent::Panel`, fenced by the selected
Client view revision and minimum Workbench revision. The shared pure intent builder never performs
Agent I/O. Target disappearance or view changes retain existing Client control refusals. Agent and
Workbench observations are not atomic with later focus; the receipt confirms Client navigation,
not that an Agent is still running or has completed a turn at render time.

Every resolver/Get/control wait borrows the cancellation source and original deadline. A failed or
expired preflight submits nothing. Once submitted, known outcomes and uncertainty survive later
read failure, cancellation and timeout. The Server's retained operation can be inspected without
re-resolving an Agent name. No rollback or automatic replacement focus is attempted.

## Delivery and verification

This adds one command path (84 total) and no new schema: the embedded catalog remains v41/72.
The shared `orchbee.client.control` v1 receipt and existing Agent/Client selection failures retain
their source ownership. Human output and help are localized; JSON is locale independent.

Verify names, explicit IDs, Panel selectors, caller versus explicit Client routing, live primary
bindings and rejection of ended/missing/ambiguous/child-only bindings. Trace a single Submit and
retained Read after a lost reply; exercise partial response cancellation and one total deadline.
Actual two-TUI Windows evidence must check the selected Panel and unchanged other Client/OS focus,
with all created Agent processes, Sites and consoles stopped afterward. Review is by the author;
TLA execution and independent review are not claimed. Seen/unseen behavior remains a separate,
explicit report obligation.

The Client must be ready; otherwise Agent reads are skipped and the existing Client selection failure reports `client-not-ready` with its Client ID. Catalog v41 extends this v1 reason vocabulary without adding a schema.
