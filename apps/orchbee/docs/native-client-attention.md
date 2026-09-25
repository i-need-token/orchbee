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

The renderer returns bounded concrete paint proofs; a receipt for an earlier snapshot cannot acknowledge a newer event. Acknowledgement may request one follow-up redraw to remove the unseen indicator, without starting an unbounded redraw loop. Evidence covers two independent Clients, rapid intervening transitions, metadata-only changes, resets, focus loss, stale receipts, hidden/overlaid panes and reader shutdown. This owner does not implement the subsequent CLI Client-attention query transport.
