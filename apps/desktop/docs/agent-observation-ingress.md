# Agent Observation Ingress

## Authority and scope

Desktop Main owns the volatile join between one sanitized rich Agent-hook event and the bounded
terminal correlation hints that identify its live presentations. The private Unix-socket half has
semantic authority but no presentation authority; a private APC version-3 hint has presentation
authority for one live terminal generation but no semantic authority. Visibility belongs to the
trusted socket envelope, not the terminal half. Neither half alone may mutate canonical Agent
state.

This contract covers endpoint acceptance, exact pairing, either arrival order, bounded settlement,
duplicate handling, presentation-generation replacement, independent fan-out, expiry, and disposal.
It does not change provider event meanings, Agent reducer, Workbench projection, notification
policy, persistence, or user-visible copy.

## Transport admission

Desktop creates one user-private Unix-domain endpoint and advertises only
`ORCHBEE_AGENT_OBSERVATION_SOCKET_V3`. A hook sends one strict, content-free v3 rich envelope and
must receive exactly `{"accepted":true,"version":3}\n` before it may emit any terminal half. No
endpoint, rejection, timeout, partial or extra ACK bytes, disconnect, and malformed input all stop
silently. Desktop writes no ACK until schema validation and closed-registry admission succeed.

The only terminal frame is `ESC _ orchbee.agent.v3;<base64url-payload> ESC \\`. Its decoded JSON
contains only `kind`, `version`, opaque `eventId`, and opaque `correlationId`. It contains no provider,
activity, visibility, path, hostname, mux identity, content, or notification copy. Public
notification channels including OSC 9, OSC 777, OSC 99, and OSC 1337 are not observation transports.
Adapters may write the private frame directly to resolved outer tmux and Zellij client TTYs after
socket acceptance; no mux passthrough is required.

## Reference scope

The following local source revisions were inspected on 2026-08-31 as bounded design evidence:

| Product          | Reviewed mechanism                                                                                                                                        | Orchbee decision                                                                                                                                                               |
| ---------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Herdr `c2637dc1` | Foreground-process detection followed by one lifecycle-hook or live-screen authority; its own guide excludes agents launched inside an inner tmux session | Keep passive detection capability-free and separate evidence capture from deterministic transitions; do not infer exact nested-mux semantics from process or screen inspection |
| Muxy `a5d276e1`  | A nonblocking Unix-socket hook client with a total execution budget, bounded retries, and an acknowledgement                                              | Adopt the private socket, exact ACK, and execution bounds; retain Orchbee's content-free terminal half and generation-fenced two-half join                                     |
| tty7 `a2b5ae56`  | Provider hook diagnostics plus direct controlling-TTY delivery over OSC 777                                                                               | Adopt explicit missing/outdated integration diagnostics; reject its public OSC notification transport                                                                          |
| Warp `086dade6`  | OSC 777 invokes the pluggable desktop-notification callback, while non-Kitty APC payloads are ignored by the reviewed parser                              | Treat this as concrete compatibility evidence for private APC and against OSC 777, not as a claim about every Warp or terminal version                                         |

This is not a compatibility or replication claim. Competitor screen rules, payloads, flat status,
terminal UI, provider lifecycle, and timing constants remain excluded. Orchbee's validated rich
hook facts, exact two-half correlation, presentation generations, and canonical Agent runtime
remain authoritative when the implementations differ.

## Inputs and trust boundaries

| Input                     | Admission and absence semantics                                                                                                                                                                                     |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rich hook event           | The socket boundary has enforced its body limit, strict v3 schema, trusted visibility, exact ACK, and closed-registry provider/event grant. Without a matching hint it remains pending and has no canonical effect. |
| Terminal correlation hint | Terminal Host admitted the exact private APC prefix and bounded base64url UTF-8 payload; Desktop then applied the strict hint schema. The join admits it only for the exact current presentation generation.        |
| Presentation registration | Establishes the only generation later hints for that presentation may name. Replacement retires the previous generation first.                                                                                      |
| Presentation retirement   | Removes pending and canonical navigation membership for only the named presentation generation.                                                                                                                     |
| Settlement deadline       | Names the exact pending-event incarnation that scheduled it. A stale deadline, expired event, or event with no live match is a no-op.                                                                               |
| Expiry observation        | Logical time comes from the ingress. Expiry is evaluated before accepting either half and before settlement.                                                                                                        |
| Disposal                  | Cancels every deadline and clears noncanonical join state without publishing Agent or terminal state.                                                                                                               |

Provider payloads, prompts, tool content, transcript paths, paths, error text, and terminal screen
text never enter this state machine.

## Data, ownership, and outputs

`AgentObservationJoinStateMachine` is the sole writer of current presentation generations, pending
events and hints, correlation-to-event claims, matched identities and attachments, historical
presentation-generation matches, and the bounded recent-event replay fence.

The state machine has no socket, clock, timer, Agent runtime, or renderer dependency. It consumes
validated inputs and logical time, then returns commands. `AgentHookObservationIngress` owns the
mechanisms that execute them:

| Command              | Effect owner and result                                                                                                                                                                                                |
| -------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Schedule settlement  | Ingress creates one timer keyed by event identity and expiry incarnation.                                                                                                                                              |
| Cancel settlement    | Ingress clears only the timer with that exact identity and incarnation.                                                                                                                                                |
| Apply observation    | Ingress dispatches the rich event once with all currently matched attachments; the Desktop runtime first retires exact same-provider passive presence for those presentation generations without a semantic end event. |
| Attach presentations | Ingress adds late matches to an already applied live Session without replaying its semantic event.                                                                                                                     |
| Retire generation    | Ingress detaches that exact generation from every canonical Agent Session.                                                                                                                                             |

Command execution does not feed hidden success back into the join. An Agent runtime rejection is
therefore still a consumed observation, preserving the existing at-most-once contract.

## Invariants

1. **Exact two-half authority**: apply requires an exact event/correlation match on at least one
   currently registered presentation generation.
2. **Order independence**: hint-first and rich-first delivery produce the same attachment set when
   the same valid halves arrive before expiry.
3. **At-most-once semantics**: one retained transport event emits at most one apply command; later
   correlations can emit only attachment commands.
4. **Generation non-retargeting**: after an event matches a presentation generation, no correlation
   from that event can bind the same presentation identity to another generation.
5. **Independent fan-out**: retiring one presentation generation cannot retire or downgrade
   another matched presentation.
6. **Expiry and deadline fencing**: an expired half, expired event, stale timer incarnation, or
   disposed state cannot emit an apply or attachment command.
7. **Fail-closed collisions**: conflicting duplicates and correlations claimed by another event
   change no admitted state.
8. **Bounded state**: overflow rejects the new untrusted input without evicting admitted state.

## Transition model

| State/event            | Guard                                                                 | State change                                                                                | Commands                                          |
| ---------------------- | --------------------------------------------------------------------- | ------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| Register generation    | No current generation                                                 | Record generation                                                                           | None                                              |
| Replace generation     | Different current generation                                          | Retire old hints and live attachments, preserve event history fences, record new generation | Retire old generation                             |
| Receive first half     | Valid and within capacity                                             | Retain until expiry                                                                         | None                                              |
| Join first correlation | Exact event and declared correlation match a current generation       | Consume hint, retain generation history, merge visibility                                   | Schedule settlement unless all correlations match |
| Join final correlation | Every declared correlation now matches                                | Mark applied and clear settlement                                                           | Cancel settlement when present, then apply once   |
| Settlement fires       | Exact incarnation, unexpired event, and a live attachment             | Mark applied and clear settlement                                                           | Apply once                                        |
| Late correlation       | Exact, unexpired, unmatched, and generation-compatible                | Consume hint and merge attachment                                                           | Attach only                                       |
| Retarget attempt       | Same event previously matched that presentation at another generation | Preserve the history fence                                                                  | None                                              |
| Event expiry           | Expiry reached                                                        | Remove claims and attachments; move an applied identity to the recent fence                 | Cancel settlement when present                    |
| Hint expiry            | Expiry reached                                                        | Remove unmatched hint                                                                       | None                                              |
| Dispose                | Any state                                                             | Clear all noncanonical facts                                                                | Ingress cancels every timer                       |

Visibility merging uses only the socket-declared value: `hidden` dominates, two `visible` values
remain `visible`, and every other combination is `unknown`. The APC cannot change visibility.
Multiple correlations for one presentation generation therefore produce one attachment. The
packaged resolver declares `hidden` only from positive inner-mux evidence and otherwise declares
`unknown`; Workbench owns the final combination with current application Tab and Panel visibility.

## Bounds and timing

Identity-only hooks use `session-identified`. After the normal strict envelope, provider event gate
and exact presentation join, the translator ensures the session and attaches presentations, then
performs no turn, activity, request or outcome transition. A new session retains its `unknown`
turn; repeated identity preserves any known state. The canonical runtime owns all such state.
Cursor, Qoder, Antigravity, Mastracode and Droid emit this event; Cursor's legacy `session-started` from capsule `0.4.7` has the same identity-only
meaning. Other providers retain their existing `session-started` behavior and do not gain admission
to the new event. Identity never proves readiness or process liveness.

Capsule `0.4.12` ships with matching native and Desktop receivers. Older receivers reject the new
event without acknowledgment. Cohort rollback restores prior semantics; there is no durable-state
migration. Direct boundary and native ingress tests cover new/legacy identity, unknown state,
repetition and unrelated-provider rejection. Translator tests additionally preserve active turns,
pending requests and outcomes through synthetic adapter input; they are not public admission
evidence. Historical corpus fingerprints remain unchanged. Authenticated provider execution and
full Windows workflow acceptance are separate evidence obligations.

Production admits at most 512 pending rich events and 1,024 pending hints. Each half has a 10-second
join lifetime. The first valid partial match schedules settlement after 25 milliseconds. An applied
event remains pending until join expiry and then remains replay-fenced for another 60 seconds.
Cleanup is demand-driven, but every deadline executes cleanup first, so event-loop delay cannot
publish an expired event.

These bounds do not claim delivery after Desktop restart, delivery while every SSH attachment is
disconnected, unbounded replay protection, provider termination, or wall-clock liveness.
The registered TLA+ variants cover both one declared correlation and a pair; the singleton variant
requires immediate application without scheduling settlement.

The socket accepts at most 64 concurrent connections, 16 KiB per body, and 500 milliseconds per
connection. A local Agent process that inherits `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` may use only
that exact owned socket; a stale or malformed explicit path does not fall through to another
Desktop instance. Only a remote process without the overlay scans at most 32 same-user forwarded
socket candidates, and that scan requires a non-empty inherited `SSH_CONNECTION`, `SSH_CLIENT`, or
`SSH_TTY` marker. An ordinary local terminal without the exact overlay performs no socket scan.
Each connection and outer-TTY resolver is bounded to 250 milliseconds. ACK
establishes only envelope admission, not terminal delivery, correlation, semantic application, or
notification; an accepted envelope without a matching APC expires with no canonical effect.

## Verification and traceability

| Claim                                   | Executable evidence                                                                                                    | Limits                                             |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| Exact two-half authority                | State-machine and ingress unit traces; `AgentObservationJoin.tla` property `ApplicationRequiresExactLiveJoin`          | Validated inputs; finite TLA identities            |
| Supported arrival-order equivalence     | State-machine unit trace `admits one exact event in either half-arrival order`                                         | Paired singleton trace; no formal confluence claim |
| Singleton immediate application         | State-machine singleton traces; TLA+ action `JoinSingleton` and property `SingletonNeverSchedulesSettlement`           | One-correlation finite variant                     |
| At-most-once semantics and late fan-out | State-machine and multi-presentation ingress traces; properties `ApplicationIsAtMostOnce` and `LateJoinNeverReapplies` | Bounded pending and recent windows                 |
| Generation fencing and local retirement | Replacement regressions; properties `MatchedPresentationNeverChangesGeneration` and `RetirementIsPresentationLocal`    | Two presentations and generations in TLC           |
| Expiry, stale deadline, and disposal    | Logical-time and delayed-timer regressions; properties `ExpiredOrDisposedCannotApply` and `StaleDeadlineCannotApply`   | Bounded logical time; no progress claim            |
| Fail-closed collision and capacity      | State-machine boundary tests and strict ingress tests                                                                  | Documented same-user trust assumption              |
| ACK-before-APC and private framing      | Adapter socket fixtures plus Terminal Host APC/OSC negative tests                                                      | Reviewed terminal and mux versions only            |

`ClineObservationOrdering` is a separate finite model with two fixed conversations and a two-run
retention bound per conversation. It checks completion-before-start, duplicate completion,
activity, session isolation, completion-order FIFO run eviction, and read-only retention on
noncompletion events. Conversation-cache eviction is outside this model and remains
implementation-test evidence.

Run Desktop unit tests, the registered `desktop-agent-observation-join` TLA+ check, and the root
merge gate after changes to this contract or implementation.
