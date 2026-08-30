# Agent Observation Ingress

## Authority and scope

Desktop Main owns the volatile join between one sanitized rich Agent-hook event and the bounded
terminal correlation hints that identify its live presentations. The private Unix-socket half has
semantic authority but no presentation authority; an OSC 777 version-2 hint has presentation
authority for one live terminal generation but no semantic authority. Neither half alone may
mutate canonical Agent state.

This contract covers exact pairing, either arrival order, bounded settlement, duplicate handling,
presentation-generation replacement, independent fan-out, expiry, and disposal. It does not change
the wire contracts, provider event meanings, Agent reducer, Workbench projection, SSH forwarding,
notification policy, persistence, or user-visible copy.

## Reference scope

Herdr source revision `c2637dc1` was inspected on 2026-08-30 as design evidence. Orchbee adopts two
limited principles: evidence may precede identity acquisition, and retained evidence is cleared
only when its owner is actually replaced. Orchbee also follows the separation of passive evidence
capture from deterministic transition decisions.

This is not a compatibility or replication claim. Herdr's screen/process detection, flat status,
terminal UI, provider lifecycle, and timing constants remain excluded. Orchbee's validated rich
hook facts, exact two-half correlation, presentation generations, and canonical Agent runtime
remain authoritative when the implementations differ.

## Inputs and trust boundaries

| Input                     | Admission and absence semantics                                                                                                                                                                                                   |
| ------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Rich hook event           | The socket boundary has enforced its body limit and strict schema, and the closed registry has granted provider provenance and accepted-event capability. Without a matching hint it remains pending and has no canonical effect. |
| Terminal correlation hint | The terminal signal has passed its strict schema. The join admits it only for the exact current presentation generation. Without a matching rich event it remains pending and has no canonical effect.                            |
| Presentation registration | Establishes the only generation later hints for that presentation may name. Replacement retires the previous generation first.                                                                                                    |
| Presentation retirement   | Removes pending and canonical navigation membership for only the named presentation generation.                                                                                                                                   |
| Settlement deadline       | Names the exact pending-event incarnation that scheduled it. A stale deadline, expired event, or event with no live match is a no-op.                                                                                             |
| Expiry observation        | Logical time comes from the ingress. Expiry is evaluated before accepting either half and before settlement.                                                                                                                      |
| Disposal                  | Cancels every deadline and clears noncanonical join state without publishing Agent or terminal state.                                                                                                                             |

Provider payloads, prompts, tool content, transcript paths, paths, error text, and terminal screen
text never enter this state machine.

## Data, ownership, and outputs

`AgentObservationJoinStateMachine` is the sole writer of current presentation generations, pending
events and hints, correlation-to-event claims, matched identities and attachments, historical
presentation-generation matches, and the bounded recent-event replay fence.

The state machine has no socket, clock, timer, Agent runtime, or renderer dependency. It consumes
validated inputs and logical time, then returns commands. `AgentHookObservationIngress` owns the
mechanisms that execute them:

| Command              | Effect owner and result                                                                            |
| -------------------- | -------------------------------------------------------------------------------------------------- |
| Schedule settlement  | Ingress creates one timer keyed by event identity and expiry incarnation.                          |
| Cancel settlement    | Ingress clears only the timer with that exact identity and incarnation.                            |
| Apply observation    | Ingress dispatches the rich event once with all currently matched attachments.                     |
| Attach presentations | Ingress adds late matches to an already applied live Session without replaying its semantic event. |
| Retire generation    | Ingress detaches that exact generation from every canonical Agent Session.                         |

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

Visibility merging is conservative: `hidden` dominates, two `visible` hints remain `visible`, and
every other combination is `unknown`. Multiple correlations for one presentation generation
therefore produce one attachment.

## Bounds and timing

Production admits at most 512 pending rich events and 1,024 pending hints. Each half has a 10-second
join lifetime. The first valid partial match schedules settlement after 25 milliseconds. An applied
event remains pending until join expiry and then remains replay-fenced for another 60 seconds.
Cleanup is demand-driven, but every deadline executes cleanup first, so event-loop delay cannot
publish an expired event.

These bounds do not claim delivery after Desktop restart, delivery while every SSH attachment is
disconnected, unbounded replay protection, provider termination, or wall-clock liveness.

## Verification and traceability

| Claim                                   | Executable evidence                                                                                                    | Limits                                   |
| --------------------------------------- | ---------------------------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| Exact authority and order independence  | State-machine and ingress unit traces; `AgentObservationJoin.tla` property `ApplicationRequiresExactLiveJoin`          | Validated inputs; finite TLA identities  |
| At-most-once semantics and late fan-out | State-machine and multi-presentation ingress traces; properties `ApplicationIsAtMostOnce` and `LateJoinNeverReapplies` | Bounded pending and recent windows       |
| Generation fencing and local retirement | Replacement regressions; properties `MatchedPresentationNeverChangesGeneration` and `RetirementIsPresentationLocal`    | Two presentations and generations in TLC |
| Expiry, stale deadline, and disposal    | Logical-time and delayed-timer regressions; properties `ExpiredOrDisposedCannotApply` and `StaleDeadlineCannotApply`   | Bounded logical time; no progress claim  |
| Fail-closed collision and capacity      | State-machine boundary tests and strict ingress tests                                                                  | Documented same-user trust assumption    |

Run Desktop unit tests, the registered `desktop-agent-observation-join` TLA+ check, and the root
merge gate after changes to this contract or implementation.
