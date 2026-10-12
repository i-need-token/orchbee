# Native Client attention

## Scope and reference

The native Client owns whether it has presented an Agent observation to its user. Canonical Agent lifecycle, semantic status and freshness stay Server-owned. Focus never changes them. Herdr reference `e0507237` separates per-Client acknowledgement in `src/client/shell/endpoint_agent_state.rs` from legacy shared Pane seen state. Adopt Client-local ownership; CLI projections must select a Client rather than invent a global focus.

## Host focus input model

| Element    | Contract                                                                                                                                                                                                                                               |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem    | Known host focus loss must prevent new Agent state from being acknowledged as viewed.                                                                                                                                                                  |
| Inputs     | Windows console focus records and complete DEC focus reports outside bracketed paste; no process output is parsed as host input.                                                                                                                       |
| State      | Each native Client begins with unknown outer focus, then holds the last focused/unfocused host observation. State dies with that Client.                                                                                                               |
| Owner      | Console adapter encodes reports; the shared decoder classifies them; dispatcher sends typed observations to the native Client before prefix/confirmation/board handling. Modal editing must preserve host metadata while keeping pasted bytes literal. |
| Transition | Focus gain records focused; focus loss records unfocused; repeated reports are idempotent. They do not acknowledge a frame by themselves.                                                                                                              |
| Effects    | Consume focus protocol without forwarding its bytes or resolving a key binding. Preserve surrounding ordinary input order. No OS window activation, topology write, Agent mutation or controller change.                                               |
| Lease      | Windows TUI enables DEC1004 through terminal lifecycle. Mark the restoration obligation before attempting enable. Restore on normal/error/panic teardown, attempt other modes after failure, preserve failed obligations.                              |
| Evidence   | Console event encoding; split CSI and paste/capture routing; failed mode enable and restoration traces. Per-Client acknowledgement/CLI projection and real Windows integration remain separate dependent evidence.                                     |

Focus loss forbids acknowledgement. Unknown focus does not establish an OS fact; its eventual presentation policy must be explicitly documented with the acknowledgement model. This initial input/lifecycle boundary alone does not establish seen/unseen parity. No new macOS/Linux runtime or TLA execution is included.

## Client observation and acknowledgement model

The Windows Client owns at most 512 Agent summaries and their acknowledged state versions. A version combines the canonical observation generation with the revision of the latest meaningful state change for that session. Status, freshness, lifecycle and generation changes advance that version; unrelated Agent changes and attachment-only metadata do not. Initial complete baseline observations begin seen, matching Herdr. Later versions remain unseen until successfully presented. Canonical status and pending requests are never rewritten.

A background read lane drains retained changes and obtains a coherent session list under one five-second deadline. Align the list revision with the drained journal head before publishing the round; retry alignment only within a fixed work bound, retain at most 512 changes, and reject incomplete or spliced evidence. The same read lane infrastructure serves the Agent board through a separate owned handle. Only one attention round is outstanding. No endpoint I/O runs on the UI thread. Cancellation, retirement and Drop interrupt the lane, disconnect and join its worker.

| Event                                                   | Client transition                                                                                                                |
| ------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- |
| First coherent round                                    | Install summaries and seed current versions as seen.                                                                             |
| Later coherent round                                    | Fold each session's semantic transitions, preserving unrelated acknowledgements; publish aligned metadata and versions together. |
| Transport/read failure                                  | Retain display data as unavailable; disable acknowledgements until coherent evidence returns.                                    |
| Invalid cursor or retention reset                       | Invalidate continuity and acknowledgements; recover a new coherent baseline conservatively unseen.                               |
| Complete successful paint                               | Acknowledge only exact versions on concrete visible pane presentations, with a matching Client snapshot token.                   |
| Failed, stale, overlaid, zero-sized or scrollback paint | No acknowledgement.                                                                                                              |
| Known outer focus loss                                  | No acknowledgement even if output is painted.                                                                                    |

Unknown outer focus follows Herdr's compatibility fallback: a complete valid paint may acknowledge until the host reports loss. This is not an operating-system foreground guarantee; supported focus reporting supplies the stronger distinction. A focus event alone does not acknowledge. Pane identity, current presentation binding and exact observation versions are checked again after drawing; hidden zoom siblings and Agent board summaries do not count as viewed panes. All actually visible panes may acknowledge, independently in each Client. Query consumers must distinguish unavailable observation from seen or unseen.

The renderer returns bounded concrete paint proofs; a receipt for an earlier snapshot cannot acknowledge a newer event. Acknowledgement may request one follow-up redraw to remove the unseen indicator, without starting an unbounded redraw loop. Evidence covers two independent Clients, rapid intervening transitions, metadata-only changes, resets, focus loss, stale receipts, hidden/overlaid panes and reader shutdown. The Client query below exposes this observation without acknowledging it.

Explicit `agent focus` has a separate command effect: after validated navigation and draw it marks
the selected Tab's current known Agent versions seen, including zoom-hidden primary panes and
known outer focus loss. It does not weaken ordinary paint gates or make read-only attention
acknowledge. [Agent navigation](native-agent-navigation.md#agent-identity-and-explicit-acknowledgement)
owns identity checks, interruption and the post-draw boundary.

## Selected Client query

### Physical terminal observation and paint

Terminal-backed Agents use the report owner's semantic sequence and full physical binding, not a
canonical Session or provider observation counter. A bounded `states` read names at most 64 unique
presentations, including children, and returns compact current binding/sequence/Agent/state rows in
request order. A null row means no current physical observation; a missing presentation refuses the
read. Site membership, retained Host creation/epoch/terminal identity and the current Worker gate
are checked together. These reads never change reports or acknowledge user attention.

The Client observes all Site presentations on an isolated background read lane with one outstanding
round, a five-second read budget, at most 4,096 physical entries and 8 MiB of retained compact data.
Workbench membership and report revisions must remain coherent across a complete batched round.
The Client attention owner stores physical acknowledgements separately from canonical versions;
unavailable canonical observations cannot be used as physical evidence or vice versa.

| Event                                                           | Physical Client transition                                                                                  |
| --------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| First complete baseline                                         | Seed current physical sequences as seen.                                                                    |
| Same binding, newer semantic sequence                           | Replace the current state; its new sequence is unseen.                                                      |
| Message/reference-only update                                   | Preserve acknowledgement because the semantic sequence is unchanged.                                        |
| New physical binding or reappearing entry                       | Start unseen after the initial baseline.                                                                    |
| Removed or unavailable binding                                  | Remove its readable entry; no seen/unseen fact is manufactured.                                             |
| Sequence regression or changed Agent/state at the same sequence | Reject the round before any partial publication.                                                            |
| Read failure, retirement or shutdown                            | Disable acknowledgement; interrupt and join owned readers.                                                  |
| Successful visible paint                                        | Acknowledge only the exact binding/sequence, Host frame route and paint serial still current after drawing. |

The existing overlay, scrollback, zoom, geometry and outer-focus gates apply to physical paint too.
An old Host frame cannot acknowledge a replacement terminal. Canonical and physical observations of
one presentation do not create duplicate unseen badges. An ordinary terminal without a reported
Agent does not create an Agent badge. A read never counts as a paint.

Viewer attach returns the actual Host terminal ID together with its epoch and attachment ID. The
projection retains this identity; frame acceptance already validates epoch/attachment routing.
Paint joins both epoch and terminal ID to the report binding before and after drawing. Epoch alone
is insufficient because one Host owns multiple terminals. Host terminal IDs are unique within an
epoch; the full creation-qualified report binding remains the acknowledgement key. This private
attach contract requires a matched Client/Server cohort and leaves Host frame encoding unchanged.

`terminal-agent-attention` uses the existing ClientView read lifecycle for 1–100 unique physical
bindings. Ordered entries contain a nullable current sequence/seen value, the report revision and
nullable outer-focus observation. CLI joins require the exact selected Client and physical binding,
an equal semantic sequence, and a Server report revision covering the Client sample. Missing or
stale Client evidence leaves seen unknown. The state projection always retains the report meaning: idle, working, blocked, explicit done, error or unknown. Seen never derives completion or clears an outcome.
Physical Get, attention, wait and prompt consumers share this join; input and focus remain separate
effects. Wait/prompt conditions consume state evidence independently of acknowledgement; done keeps its existing completed-outcome alias, while succeeded and failed select explicit outcomes.

Targeted correctness checks cover deterministic baseline/change/replacement/quiet-update traces,
two independent in-memory Clients, stale and mismatched paint proofs, coherent batch bounds and
ordered protocol correlation. Two live Windows Clients and process/paint/query integration remain
part of the final combined acceptance. The fixed reference is Herdr
`e0507237ad8d41f60aece10d00ef29ce2339ca34`, `client/shell/endpoint_agent_state.rs` and
`app/api_helpers.rs`; no authenticated-provider, OS foreground, full visual or unbounded proof is
implied by these bounded witnesses.

The existing connection-scoped ClientView lane routes `agent-attention` reads to one explicit, caller-derived or uniquely discovered Client. An intent names 1–100 unique Agent session IDs. `attention-read` completes immediately from one coherent Client snapshot, without navigation, input, focus changes or acknowledgement. Reads may complete while an overlay makes the Client busy; one-delivery, freshness, exact view revision, deadline and receipt-capacity rules still apply.

The snapshot carries the canonical changes cursor and revision, nullable outer-focus observation, and entries in request order. Each entry has either a current generation, observation sequence, semantic-change revision and seen flag, or a null state when absent. An unreadable Client observation returns `projection-unavailable`. A query is an observation at its recorded revision, not a promise that the Agent cannot subsequently change. CLI consumers authenticate the returned cursor through the live changes service before exposing seen/unseen; canonical detail joins additionally require matching generation and sequence. Foreign-boot cursors and inconsistent joins never manufacture an acknowledgement.

Workbench protocol consumes bounded Agent protocol identity, cursor and safe-integer DTOs, with no runtime dependency. Attention submissions and completions use bounded 64 KiB request kind 29; result kind 26 already has that bound. Server receipt storage reserves attention payload capacity and validates exact request/result session correspondence before publication. A retained receipt does not refresh itself or repeat a read. Shared Rust, TypeScript and schema checks cover shape, bounds, duplicates, ordering, revision and attribution; Windows process and two-Client witnesses cover routing and actual paint acknowledgement.

`agent attention <target> [--client <id>]` resolves a session/name/Panel once and reads that Client without focusing it. `--session`, `--name` and `--panel` select explicitly. It supports the existing `--request-id`, `--detach`, `--timeout` (default 30000 ms) and `--format` receipt lifecycle. `client control-status` authenticates attention cursors too. Text reports seen, unseen or not observed; JSON uses `orchbee.client.control` v1. Catalog v51 retains 72 schemas and admits 86 native command paths.

## Joining canonical Agent reads

Windows `agent get <target> --client <id>` joins one exact Client observation with a later canonical Get on one admitted session and five-second total. The read routes one retained attention operation, authenticates its cursor, then requires the canonical revision to cover that attention head. Generation and sequence must both agree before exposing joined seen/state-change revision. A bounded changes read also proves no transition for this session between the Client checkpoint and the canonical sample; equal counters alone cannot exclude a freshness transition. Unrelated-session changes preserve the join, while lost history or any intervening target event conservatively makes it stale. Missing and stale Client versions remain explicit, with null seen; no canonical status is rewritten. Windows Get without `--client` uses v9 with terminal, runtime and management information; composed Get uses v10. Both revalidate the Agent binding after the process read inside the same total budget.

The separate Client status maps idle to idle, succeeded to done, and failed/canceled/interrupted to error, independently of seen or missing acknowledgement. Working, blocked, unknown, ended and stale/disconnected freshness use canonical facts. Focus means the matching primary Panel selected in a Ready Client view, with no known outer focus loss; null outer focus retains the documented compatibility fallback and does not prove OS foreground. Busy reads remain allowed but do not claim focused content. No read acknowledges, navigates or sends input.

The CLI results owner holds this projection; Agent Client view composition owns reads and version joining. Windows `agent wait --client <id>` reuses this projection in v8: idle/done depend on canonical state evidence, including retained changes, and never on Client seen. Canonical historical events do not prove past Client acknowledgements. Get already includes process and submission preflight information; managed interaction readiness remains separately owed. Evidence covers exact Client/operation correlation, generation and sequence lag, missing entries, foreign boot, current revision, independent focus and seen, and original deadline/cancellation cleanup.

Windows `agent prompt --wait --client <id>` also consumes this current projection after its canonical activity gate. It preflights the exact Client before input, retains the send receipt when later observation fails, and emits v4 with the selected Client. See the [CLI query contract](native-cli-query.md) for deadlines, sampling and no-resubmission rules.

Windows `agent list --client <id>` uses one bounded canonical page to select up to 100 session IDs for a single retained attention read, then rereads the same page/cursor. A single bounded change scan corroborates all entries against that later canonical revision. Intervening target events or mismatched counters make the corresponding Client projection stale; newly appearing IDs are missing, while unrelated events preserve other rows. A stale page cursor fails without restarting pagination. An empty page validates the selected Client but performs no empty attention operation. The v2 result adds `clientId` and a `client` projection to each canonical item; no-flag list remains v1. One five-second total, exact Client selection and ordinary cancellation/cleanup apply. No list read focuses, acknowledges or scans each Agent process.

Windows Agent Get now emits v9 (v10 with `--client`), adding a separate `management` observation with source, revision and Server-owned startup state. Active records come only from completed startup; manually named sessions remain unmanaged. Pending startup makes promptReady false and `agent prompt` refuses before input. The bounded Status read shares the original command deadline and can fail with Agent read failure v3 (`operation: management`); it never retries or completes a launch. `agent start` v2 explicitly reports interactiveReady after its existing successful completion. The [startup contract](windows-agent-start.md) owns lifecycle and limits.

## Automatic notification acknowledgement boundary

The automatic physical-Agent notification scheduler consumes this Client's physical state and
acknowledgements without becoming a second seen owner. A changed exact sequence may schedule local
work. A successful exact visible paint cancels the matching notification version even when outer
focus is known lost, but only this document's focus-qualified acknowledgement marks the attention
version seen. The notification-only cancellation cannot acknowledge, rewrite or infer an
observation, cannot suppress a later sequence, and another Client's paint cannot suppress its work.
Baseline, reset, unavailable and replacement rounds provide no notification transition. See
[Windows CLI notifications](windows-notifications.md#automatic-physical-agent-notifications) for
the trigger matrix, delay, configuration, sink and correctness contract.
