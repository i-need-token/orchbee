# CLI Verification

## Authority and scope

This standard states what must hold after a change to an Orchbee-authored command-line surface and
how to show it. [`command-surface.md`](command-surface.md), [`output.md`](output.md), and
[`process.md`](process.md) own the rules; [`TESTING.md`](../TESTING.md) decides which tests are
admitted. Nothing here is a list of test cases that each change must add. A change to the exit-code
vocabulary or a machine schema is a public contract under
[`DEVELOPMENT.md`](../DEVELOPMENT.md#decision-authority).

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

## Outcomes

A change is complete when these hold for the commands and contracts it changed:

| Outcome             | Holds when                                                                                                                                                                                                                                                   |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| One grammar         | Help, completion, and the machine schema listing name exactly the commands the grammar authority declares. Every help hint a usage diagnostic prints is a command that exits `0` without a runtime.                                                          |
| Parse before effect | A usage failure and an offline command touch no filesystem, endpoint, or cohort capability.                                                                                                                                                                  |
| Reads only read     | A read command performs one admission and one read: no create, lock, or grant. An observation command repeats bounded reads without an effect.                                                                                                               |
| One submission      | A mutation submits once per effect and is never replayed. A failure after acceptance reports an unknown outcome that carries the request identity. A declared stale-base retry and a declared workflow stay inside their bounds and keep known effects.      |
| Activation          | A cold invocation brings the service up, a repeat reports it already running, a half-activation refuses without changing committed state, and one invocation-owned deadline bounds the attempt.                                                              |
| Exit codes          | Every failure kind and terminal result maps to exactly one exit code.                                                                                                                                                                                        |
| Envelope            | Every success and failure document validates against the embedded schema. A failure under `text` leaves standard output empty and under `json` writes exactly one document. Detached acceptance and unknown outcome both carry the request identity.         |
| Both locales        | Every diagnostic and help text renders under `en-US` and `zh-CN`, selected in the order `LC_ALL`, `LC_MESSAGES`, `LANG`. A machine document is byte-identical under both.                                                                                    |
| Text safety         | Control characters, `ESC`, U+2028, and bidirectional controls in a label never reach the terminal raw; identifiers are never truncated.                                                                                                                      |
| Bounds              | Output over the bound leaves standard output empty and exits with the local-failure code. A failing standard output writer exits nonzero with a diagnostic on standard error and no second document. A stream ends with a terminal record, typed on failure. |
| Signals             | `SIGINT` during a wait exits `130` with no cancel request and no partial machine document. A termination signal to an activation verb ends the process by the signal disposition, never as a synthesized cancellation document.                              |
| No terminal         | Every command except a declared interactive one behaves the same without a controlling terminal.                                                                                                                                                             |

Show an outcome at the lowest layer that can judge it:

- Deny or script each injected capability: a denied capability that is touched fails the test, and a
  scripted endpoint records every request it receives.
- Stream separation, exit-code propagation, signal handling, and launcher resolution exist only in a
  real process. Run the built executable as a child process with piped streams and assert standard
  output, standard error, and the exit code separately, and state the operating system and the
  endpoint fixture with the result.
- Review an expected-text update as a visual diff of the before and after output.
