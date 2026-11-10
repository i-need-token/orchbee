# CLI Process

## Authority and scope

This standard owns what happens between the first argument and the exit code of an Orchbee
command-line invocation: the trust boundary of its inputs, its independence from the terminal, the
order in which parsing precedes any effect, deadlines, side effects, request identity, signals, and
how the process locates its runtime. Grammar follows [`command-surface.md`](command-surface.md)
and output follows [`output.md`](output.md). Operating-system boundaries shared with every native
executable are authoritative in [Rust](../RUST.md#local-process-and-ipc-boundaries).

## Inputs and trust boundary

| Input             | Contract                                                                                                                                                                                                                                                                                                              |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Arguments         | Hostile. The total argument size has a declared bound; a non-Unicode argument or an overflow is a usage failure with empty standard output, whatever else the arguments spell.                                                                                                                                        |
| Environment       | Only documented `ORCHBEE_*`, locale, and explicitly declared platform user-directory inputs are read. User-directory inputs are scoped to operand expansion, never runtime discovery. A malformed documented value is a usage or local failure that names the variable; it is never defaulted.                        |
| Standard input    | Read only when the command declares an input operand such as `--input-file -`, or for one capped line of a release confirmation from a console. Input is capped before decoding and decoded as strict UTF-8 JSON or the declared format.                                                                              |
| Files             | Read only from an explicitly named operand, capped before decoding, and never discovered from the current directory, the repository, or a project file.                                                                                                                                                               |
| Terminal          | Never read, except that a [release confirmation](command-surface.md#destructive-and-interactive-commands) checks that standard input and output are consoles. No command depends on a terminal's size or capabilities; a command that launches an interface hands the terminal to the `tui/` lifecycle after parsing. |
| Runtime responses | Trusted only after the typed transport and protocol layers admitted them. Verdicts are never re-derived from display text, error-kind guesses, or byte counts.                                                                                                                                                        |
| Snapshot data     | Labels, paths, and terminal text are arbitrary Unicode treated as hostile for every human surface and preserved as data in machine output.                                                                                                                                                                            |

## Parse before effect

An invocation proceeds through these phases in order, and a failure in one phase prevents every
later phase:

1. Scan the argument envelope for size, encoding, and the format selection, so that a later grammar
   failure already knows its output format.
2. Validate the complete grammar: command path, options, operands, targets, and value sets. A
   syntactic failure touches no configuration, filesystem, cohort, or endpoint.
3. Answer offline commands — help, version, default configuration, completion, and schema listings — from the build alone.
4. Discover the runtime through the configured base and reject unsafe paths without repairing them.
5. Admit exactly one session. Ordinary commands perform one read and one effect, repeated only
   by the bounded stale-base retry under the same deadline. Declared observation commands may
   repeat bounded reads; declared workflows may execute a finite sequence of named effect phases
   under the same original deadline.
6. Prepare the complete output, then write it once.

## Deadlines and waits

- Every network step has a deadline owned by the invocation, not by the service. One deadline
  covers admission plus the read and command, including every stale-base retry; a wait command's
  deadline is `--timeout` in milliseconds with a documented default and upper bound.
- Defaults are finite unless the owning observation command contract expressly defines an indefinite
  wait. Every indefinite mode documents its default or explicit option, preserves bounded admission
  and individual reads, and supports interruption with exit `130`. Ordinary mutations stay bounded.
- An explicitly owned local-child workflow may declare a bounded settlement allowance in addition to its work deadline; both form the original total invocation budget. After the work deadline, only settlement and reported recovery may continue.
- A deadline expiry is reported as its own typed outcome. It never claims that the observed
  operation stopped, failed, or was never delivered.
- Long-polling and change feeds return a cursor and an explicit reset signal when the caller fell
  behind; a reset is never presented as a continuous feed.

## Side effects and request identity

| Rule               | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reads              | Create no directory, lock, marker, terminal, viewer, or grant. A declared read-side attachment is held minimally, released before output, and named in the owning guide.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| One submission     | An ordinary mutation has one effect phase. A declared workflow names each finite phase in its owning contract; each phase submits once under its own identity generated or admitted after grammar validation, repeated only by the stale-base retry. A workflow never retries a phase after any other outcome or hides earlier committed effects when a later phase fails.                                                                                                                                                                                                                                                                                                                                                                                                                                                |
| No replay          | The invocation never replays or resubmits an effect that may have been applied. An undetermined result is reported as unknown outcome with the request identity, not guessed or repeated.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Stale-base retry   | A refusal the service settles before any effect because the submission rested on a stale view, such as a revision mismatch, may be followed by a fresh read under the original deadline, at most three times. Each attempt resolves every target again and is submitted only when the resulting request is identical; otherwise the refusal stands. Each attempt carries a fresh request identity, because the service retains a refusal under its identity; the attempts remain one logical request, reported under the last identity. A caller-supplied identity is never replaced, so its submission is not repeated, nor is one carrying values prepared from the earlier read outside target resolution. Unknown, partial, and pending outcomes are never repeated. Owning contracts name the qualifying refusals.   |
| Precommit refusals | A definite precommit refusal means that phase applied nothing. Earlier workflow effects remain independently recorded; a later refusal never authorizes replaying the whole workflow. Transport ambiguity remains unknown, never a guessed refusal.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| Detachment         | `--detach` returns once the submission is accepted and yields the request identity; a later `get` or `wait` observes it. Detachment never changes what was submitted.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Reads of output    | Terminal or transcript reads return current content only, are never redacted automatically, and are documented as possibly containing secrets so callers limit what they forward.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                         |
| Durable state      | The process owns none for reads and mutations: no history, cache, lock file, or PID file; the service is the only durable authority. Declared offline user-configuration workflows, activation and stopped-runtime cleanup are exceptions. Offline configuration workflows may serialize on their configuration owner and retain explicitly owned recovery records; they never become an alternate live runtime authority. Activation may create runtime state and spawn the service; cleanup may serialize against activation and remove only proven-owned stopped state. A declared append-only request log is the remaining exception: it is private and size-bounded, nothing reads it as authority, and a failed append never blocks or changes the command. The owning guide names each artifact and recovery rule. |

A declared workflow records its phase order, authoritative commit points, refusal and unknown
outcomes, cancellation cuts, and bounded recovery before implementation. Every later phase must
consume the exact earlier receipt it depends on. A known commit survives failed publication or
Client acknowledgment; a compound success requires every declared success condition. No cross-owner
atomicity is implied by one CLI invocation. Recovery or release effects must be explicitly declared,
bounded by the original deadline, and separately reported. No hidden compensating mutation is allowed.

## Signals and cancellation

- `SIGINT` and `SIGTERM` abort the invocation's own waiting and observation. They never issue
  a semantic cancellation; cancelling an accepted operation requires the explicit `cancel`
  verb. A declared offline provisioning workflow may stop and reap its own untransferred child processes; its owning guide defines commit and recovery cuts. A declared Session-run provisioning job that exposes no request identity, such as provider onboarding, instead registers cooperative cancellation of the job its own invocation started and waits for that job's final report; its owning guide defines the cancellation cut.
- A declared `terminal send` input source watches exactly `SIGHUP`, `SIGINT`, `SIGQUIT`, and
  `SIGTERM`, minus any signal inherited as ignored or blocked, and only inside its
  materialization window. A captured signal ends acquisition as a typed source failure —
  reported on the `orchbee.cli.failure.source` envelope — before any request exists; the
  invocation exits the conventional `128 + signo`. This watch set is the source stage's own:
  no other command adopts it, and inherited dispositions are restored unchanged when
  acquisition ends.
- An interrupted invocation exits with the conventional code, writes no partial machine document
  after interruption, and restores nothing because it changed no terminal modes. An interrupted
  activation may additionally leave committed runtime state behind; the owning guide names the
  residue, and a later invocation reports it rather than repairing it.
- A wait interrupted after the operation was accepted leaves the operation running; the request
  identity already printed or supplied remains the handle for a later `get` or `wait`.

## Locating the runtime

- The runtime base comes from one documented configuration source; the current directory, the
  repository, a project file, and `PATH` never select it.
- Discovery validates the base, the scoped directory, and the endpoint by owner, mode, type, and
  symlink policy through the shared rendezvous owner. An unsafe path is a local failure, an absent
  endpoint is an unavailable outcome, and neither is repaired.
- A sibling executable or cohort is verified by identity before it is trusted or launched, per
  [Rust](../RUST.md#local-process-and-ipc-boundaries). A version or cohort mismatch is an
  incompatible outcome.
- An invocation never starts, activates, repairs, or locks a service unless its verb declares that
  effect. A verb that declares activation may create the scoped runtime directory, take its locks,
  commit its marker, or spawn the service; it still validates the cohort
  before any runtime-directory work, holds one invocation-owned deadline for the whole attempt,
  reports a half-activated state as a refusal, and never repairs, stops, or deletes runtime state.

A declared stopped-runtime cleanup validates ownership and lifecycle evidence before deleting
exact owned artifacts, serializes against activation, and refuses running or uncertain state.
If cleanup spans multiple filesystem effects, its durable intent fences admission until bounded,
explicit recovery completes. Partial deletion is reported as incomplete; cleanup never kills
processes or silently repairs unrelated state.

## Concurrency between invocations

- Invocations do not coordinate through files or shared memory, except that declared activation and
  stopped-runtime cleanup serialize on the runtime owner's locks so activation converges on one
  service and cannot race deletion. Two concurrent invocations are otherwise independent sessions to the same authority,
  and the authority owns ordering. Declared offline configuration workflows serialize through their shared configuration owner, including bounded interruption and recovery.
- Pagination cursors and request identities are opaque to the caller. Declared lifecycle cleanup
  may additionally retain its private recovery intent until the cleanup completes.

Executable-name admission must remain syntax-only before routing. If a command owns PATH resolution, the final executing CLI resolves a bounded absolute executable before configuration, discovery and mutation, passes arguments literally, and records the resolved path without claiming process success.
