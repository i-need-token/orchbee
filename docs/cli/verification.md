# CLI Verification

## Authority and scope

This standard owns the evidence required for changes to Orchbee-authored command-line surfaces. It
applies the repository test obligations in [`TESTING.md`](../TESTING.md) and
[`RUST.md`](../RUST.md#testing) to the contracts in [`command-surface.md`](command-surface.md),
[`output.md`](output.md), and [`process.md`](process.md). Change classification follows
[`DEVELOPMENT.md`](../DEVELOPMENT.md#change-classification): a new command family, mutation verb,
exit-code mapping, or request-identity rule is Modeled work, and a change to the exit-code
vocabulary or a machine schema is Consequential.

## Structural prerequisites

Evidence below is only possible when the executable keeps these boundaries:

| Responsibility | Contract                                                                                                  |
| -------------- | --------------------------------------------------------------------------------------------------------- |
| Grammar        | A pure function from the argument list to one typed request or one typed usage failure                    |
| Projection     | A pure function from a validated response to the machine document and to the text rendering               |
| Mapping        | A total function from failure kind or terminal result to one exit code                                    |
| Capabilities   | Filesystem, endpoint, clock, randomness, and signal sources injected so tests can deny or script each one |
| Output         | One bounded sink for standard output and one for standard error, both observable in tests                 |

A change that reads the environment, the filesystem, or a socket inside grammar or projection is
refused in review regardless of its tests.

## Grammar evidence

- Every command path, option, value set, and target rule has a positive case and the usage cases
  it rejects: unknown word, duplicate option, undeclared option, missing value, missing operand,
  invalid value, misplaced help or version, and the `--` terminator.
- Help, completion, and the machine schema listing are generated from or compared against the
  grammar authority in a test that fails when any command exists in one and not the others.
- Every help hint emitted by a usage diagnostic is executed in a test and exits `0` with no
  runtime.
- Argument-envelope bounds are exercised at the limit, one past the limit, and with a non-Unicode
  argument.

## Parse-before-effect evidence

- Every usage failure is run with denied filesystem, endpoint, and cohort capabilities; the test
  fails if any capability is touched.
- Every offline command is run the same way and must succeed.
- A read command is run against a scripted endpoint that records every request; the test asserts
  exactly one admission and one read, and no create, lock, or grant. A declared observation command
  additionally proves bounded repeated reads without an effect.
- A mutation is run against a scripted endpoint that fails after accepting the submission; the
  test asserts one submission, no retry, and an unknown-outcome document carrying the request
  identity.
- A declared workflow faults every phase boundary and submission response. Evidence checks its
  finite phase order, exact receipt dependencies, one submission per effect phase, unchanged total
  deadline, and retained known effects on later failure, cancellation or unknown outcome. Rejected
  and no-op paths prove which later phases cannot run.
- An activation verb is run against a real or staged runtime fixture: a cold invocation brings
  the service up, a repeat invocation reports already-running, a staged half-activation refuses
  without mutating the committed state, and the whole attempt honors one invocation-owned
  deadline.

## Exit code and envelope evidence

| Class      | Required cases                                                                                                                        |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Mapping    | A table-driven test covering every failure kind and every terminal result kind, asserting the code and that no two kinds are unmapped |
| Envelope   | Every success and failure document validates against the embedded schema, including the closed `details` shape per failure kind       |
| Format     | Every failure kind under `text` leaves standard output empty and under `json` writes exactly one document                             |
| Provenance | `source` appears only when derived from a validated snapshot; `null` and `"unknown"` cases are both exercised                         |
| Identity   | Detached acceptance and unknown outcome both carry the request identity                                                               |

## Text and localization evidence

- Text rendering is tested with control characters, `ESC`, U+2028, bidirectional controls,
  quotes, backslashes, wide characters, and labels beyond the truncation width; identifiers are
  asserted untruncated.
- Every standard error diagnostic and every help text renders under `en-US` and `zh-CN`, and the
  locale-selection order over `LC_ALL`, `LC_MESSAGES`, and `LANG` is table-driven.
- A machine document is byte-identical under both locales.

## Bounds and stream evidence

- The output bound is exercised one byte under and one byte over; the over case asserts empty
  standard output and the local-failure code.
- A failing standard output writer asserts a nonzero exit, no second document, and a diagnostic on
  standard error.
- A streamed command is tested for one record per line, source order, a terminal record on normal
  end, and a typed terminal record with a nonzero exit on deadline and channel loss.

## Process evidence

Some behavior exists only in a real process: stream separation, exit-code propagation, signal
handling, and launcher resolution.

- Run the built executable as a child process with piped streams and assert standard output,
  standard error, and the exit code separately.
- Send `SIGINT` during a wait against a scripted endpoint and assert exit `130`, no cancel
  request, and no partial machine document.
- Send a termination signal to an activation verb and assert the process dies by the signal
  disposition — never as a synthesized typed cancellation document.
- Run without a controlling terminal and assert that every command except a declared interactive
  one behaves identically.
- Record the operating system and the endpoint fixture with the evidence; process evidence
  supplements the deterministic classes above and never replaces them.

## Change checklist

| Change                       | Evidence                                                                                                        |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------- |
| New command or option        | Grammar class; help, completion, and schema agreement; parse-before-effect for its usage failures               |
| New failure kind             | Mapping and envelope classes; both locales for its diagnostic                                                   |
| Exit-code vocabulary         | Every mapping test across every Orchbee executable; owning guides updated in the same change                    |
| Mutation verb                | One-submission, no-replay, unknown-outcome, and detachment cases against a scripted endpoint                    |
| Wait verb or deadline        | Deadline expiry, interruption, cursor reset, and upper-bound cases                                              |
| Activation verb              | Cold start, idempotent re-run, half-activation refusal with unchanged committed state, and one bounded deadline |
| Text rendering or truncation | Text class; expected-text update reviewed as a visual diff                                                      |
| Runtime discovery            | Unsafe path, absent endpoint, and cohort mismatch cases with denied repair                                      |
| Repository gate              | `pnpm verify` passes                                                                                            |
