# Native Client attention

## Scope and reference

The native Client owns whether it has presented an Agent observation to its user. Canonical Agent lifecycle, semantic status and freshness stay Server-owned. Focus never changes them. Herdr reference `e0507237` separates per-Client acknowledgement in `src/client/shell/endpoint_agent_state.rs` from legacy shared Pane seen state. Adopt Client-local ownership; CLI projections must select a Client rather than invent a global focus.

## Host focus input model

| Element    | Contract                                                                                                                                                                                                                                               |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Problem    | A background terminal must not acknowledge new Agent state as viewed.                                                                                                                                                                                  |
| Inputs     | Windows console focus records and complete DEC focus reports outside bracketed paste; no process output is parsed as host input.                                                                                                                       |
| State      | Each native Client begins with unknown outer focus, then holds the last focused/unfocused host observation. State dies with that Client.                                                                                                               |
| Owner      | Console adapter encodes reports; the shared decoder classifies them; dispatcher sends typed observations to the native Client before prefix/confirmation/board handling. Modal editing must preserve host metadata while keeping pasted bytes literal. |
| Transition | Focus gain records focused; focus loss records unfocused; repeated reports are idempotent. They do not acknowledge a frame by themselves.                                                                                                              |
| Effects    | Consume focus protocol without forwarding its bytes or resolving a key binding. Preserve surrounding ordinary input order. No OS window activation, topology write, Agent mutation or controller change.                                               |
| Lease      | Windows TUI enables DEC1004 through terminal lifecycle. Mark the restoration obligation before attempting enable. Restore on normal/error/panic teardown, attempt other modes after failure, preserve failed obligations.                              |
| Evidence   | Console event encoding; split CSI and paste/capture routing; failed mode enable and restoration traces. Per-Client acknowledgement/CLI projection and real Windows integration remain separate dependent evidence.                                     |

Focus loss forbids acknowledgement. Unknown focus does not establish an OS fact; its eventual presentation policy must be explicitly documented with the acknowledgement model. This initial input/lifecycle boundary alone does not establish seen/unseen parity. No new macOS/Linux runtime or TLA execution is included.
