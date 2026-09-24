# CLI Process

## Authority and scope

This standard owns what happens between the first argument and the exit code of an Orchbee
command-line invocation: the trust boundary of its inputs, its independence from the terminal, the
order in which parsing precedes any effect, deadlines, side effects, request identity, signals, and
how the process locates its runtime. Grammar follows [`command-surface.md`](command-surface.md)
and output follows [`output.md`](output.md). Operating-system boundaries shared with every native
executable are authoritative in [Rust](../RUST.md#local-process-and-ipc-boundaries).

## Inputs and trust boundary

| Input             | Contract                                                                                                                                                                                                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Arguments         | Hostile. The total argument size has a declared bound; a non-Unicode argument or an overflow is a usage failure with empty standard output, whatever else the arguments spell.                                                                                                                 |
| Environment       | Only documented `ORCHBEE_*`, locale, and explicitly declared platform user-directory inputs are read. User-directory inputs are scoped to operand expansion, never runtime discovery. A malformed documented value is a usage or local failure that names the variable; it is never defaulted. |
| Standard input    | Read only when the command declares an input operand such as `--input-file -`. Input is capped before decoding and decoded as strict UTF-8 JSON or the declared format.                                                                                                                        |
| Files             | Read only from an explicitly named operand, capped before decoding, and never discovered from the current directory, the repository, or a project file.                                                                                                                                        |
| Terminal          | Never read. No command depends on a controlling terminal, its size, or its capabilities; a command that launches an interface hands the terminal to the `tui/` lifecycle after parsing.                                                                                                        |
| Runtime responses | Trusted only after the typed transport and protocol layers admitted them. Verdicts are never re-derived from display text, error-kind guesses, or byte counts.                                                                                                                                 |
| Snapshot data     | Labels, paths, and terminal text are arbitrary Unicode treated as hostile for every human surface and preserved as data in machine output.                                                                                                                                                     |

## Parse before effect

An invocation proceeds through these phases in order, and a failure in one phase prevents every
later phase:

1. Scan the argument envelope for size, encoding, and the format selection, so that a later grammar
   failure already knows its output format.
2. Validate the complete grammar: command path, options, operands, targets, and value sets. A
   syntactic failure touches no configuration, filesystem, cohort, or endpoint.
3. Answer offline commands — help, version, completion, and schema listings — from the build alone.
4. Discover the runtime through the configured base and reject unsafe paths without repairing them.
5. Admit exactly one session and submit at most one read and, for a mutation, at most one effect.
6. Prepare the complete output, then write it once.

## Deadlines and waits

- Every network step has a deadline owned by the invocation, not by the service. One deadline
  covers admission plus the single read or command; a wait command's deadline is `--timeout` in
  milliseconds with a documented default and upper bound.
- No invocation waits indefinitely by default. Indefinite waiting, where a command offers it, is an
  explicit option, and interruption still exits `130`.
- A deadline expiry is reported as its own typed outcome. It never claims that the observed
  operation stopped, failed, or was never delivered.
- Long-polling and change feeds return a cursor and an explicit reset signal when the caller fell
  behind; a reset is never presented as a continuous feed.

## Side effects and request identity

| Rule               | Contract                                                                                                                                                                                                                                                                                                                     |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Reads              | Create no directory, lock, marker, terminal, viewer, or grant. A declared read-side attachment is held minimally, released before output, and named in the owning guide.                                                                                                                                                     |
| One submission     | A mutation submits at most one effect per invocation under one request identity, either caller-supplied or freshly generated after grammar validation, never before.                                                                                                                                                         |
| No replay          | The invocation never retries, replays, or resubmits an effect. An undetermined result is reported as unknown outcome with the request identity, not guessed or repeated.                                                                                                                                                     |
| Precommit refusals | Refused, unavailable, broken-channel, and incompatible outcomes on a mutation are definite: nothing was applied, and the caller may resubmit under a new identity.                                                                                                                                                           |
| Detachment         | `--detach` returns once the submission is accepted and yields the request identity; a later `get` or `wait` observes it. Detachment never changes what was submitted.                                                                                                                                                        |
| Reads of output    | Terminal or transcript reads return current content only, are never redacted automatically, and are documented as possibly containing secrets so callers limit what they forward.                                                                                                                                            |
| Durable state      | The process owns none for reads and mutations: no history, cache, lock file, or PID file; the service is the only durable authority. An activation verb is the declared exception — it may create the runtime directory, hold its locks, commit its marker, and spawn the service, and the owning guide names each artifact. |

## Signals and cancellation

- `SIGINT` and `SIGTERM` abort the invocation's own waiting and observation. They never issue
  a semantic cancellation; cancelling an accepted operation requires the explicit `cancel`
  verb.
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
  effect. A verb that declares activation is the only path that may create the scoped runtime
  directory, take its locks, commit its marker, or spawn the service; it still validates the cohort
  before any runtime-directory work, holds one invocation-owned deadline for the whole attempt,
  reports a half-activated state as a refusal, and never repairs, stops, or deletes runtime state.

## Concurrency between invocations

- Invocations do not coordinate through files or shared memory, except that activation verbs
  serialize on the runtime's declared lock files so concurrent activators converge on one
  service. Two concurrent invocations are otherwise independent sessions to the same authority,
  and the authority owns ordering.
- Pagination cursors and request identities are the only state carried between invocations, and
  both are opaque to the caller.
