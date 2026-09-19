# CLI Output

## Authority and scope

This standard owns what an Orchbee command-line invocation writes and how it exits: the split
between standard output and standard error, the human and machine formats, the machine document
envelope, the exit-code vocabulary, escaping, bounds, streamed records, and localization. Grammar
follows [`command-surface.md`](command-surface.md); the effects behind the output follow
[`process.md`](process.md). Each executable's concrete schemas, failure kinds, and byte limits are
owned by its application guide and must satisfy the shapes below.

## Streams

| Stream          | Carries                                                                                                                    | Never carries                                                                     |
| --------------- | -------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Standard output | Exactly one contract document, or one ordered stream of records, for the requested command                                 | Progress, prompts, diagnostics, log lines, or a second document after a failure   |
| Standard error  | Localized human diagnostics: the failure cause, the echoed bounded input that caused it, and one help hint                 | Machine fields, payload data, secrets, stack traces, or free-form internal detail |
| Exit code       | One value from the vocabulary below, derived from the document's failure kind or terminal result through one total mapping | Anything a caller must parse text to recover                                      |

A caller can therefore automate on standard output and the exit code alone and read standard error
only when a person is present.

## Formats

- `--format text|json` selects the projection; `text` is the default. `json` is the machine format
  and is stable under the owning schema. `text` is a localized human projection that automation
  must not parse, and its layout may change without a version bump.
- Commands whose only consumer is a machine, such as a schema listing or a snapshot export, emit the
  same JSON document under both formats.
- Fixed-text commands such as `help`, `--version`, and `completion` do not accept `--format`. When
  a syntactically valid `--format json` accompanies a grammar failure, the failure is reported as a
  JSON failure document; under `text` the failure keeps standard output empty.
- Under `json`, a failure is one failure document on standard output and the localized diagnostic
  on standard error. Under `text`, a failure writes nothing to standard output.

## Machine documents

| Rule                | Contract                                                                                                                                                                  |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shape               | One JSON object ending in a newline; two-space indentation for documents, one line per record for streams.                                                                |
| Schema identity     | Every document names its schema identifier, and the identified schema is retrievable offline from the same build. A shape change is a new schema identifier or version.   |
| Closed vocabularies | Failure kinds, stages, reasons, resource names, and enum values are finite, lowercase, kebab-case, documented values validated by the embedded schema.                    |
| Failure envelope    | A failure document carries the failure kind, a details object whose shape is fixed per kind, and, only when derived from an authoritative source, that source's identity. |
| No prose            | Machine documents contain no `message`, no localized text, and no exception or `Display` text. Presentation belongs to standard error and the `text` projection.          |
| Provenance          | Every field has a trusted source. `null` means the source said none; `"unknown"` means the source cannot say; the two never mix, and no field is fabricated.              |
| Untrusted data      | User-controlled labels, paths, and terminal text are preserved losslessly with standard JSON encoding; they are never truncated, sanitized, or translated in `json`.      |
| Request identity    | A mutation result and a detached acceptance include the request identity so the caller can `get` or `wait` on it later.                                                   |

## Exit codes

The code classifies who decided the outcome, not which command ran. The owning application maps
every failure kind and terminal result to exactly one code, and the mapping is tested to be total.

| Code  | Class           | Meaning                                                                                                                         |
| ----- | --------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `0`   | Success         | The read completed, including an empty result; the mutation completed or was unchanged; a detached submission was accepted      |
| `1`   | Local failure   | A local configuration check failed, output could not be prepared or written, or an internal invariant failed before any request |
| `2`   | Usage           | Invalid grammar, option, value, target syntax, input document, or argument envelope                                             |
| `3`   | Refused         | The authority definitively declined before applying anything: not found, ambiguous, denied, rejected, blocked, or conflicting   |
| `4`   | Unavailable     | No reachable service, capacity exhausted, or the current platform cannot serve the request                                      |
| `5`   | Broken channel  | Transport loss or a protocol violation                                                                                          |
| `6`   | Incompatible    | Protocol, cohort, or version mismatch                                                                                           |
| `7`   | Cancelled       | The operation was accepted and ended in a cancelled terminal result                                                             |
| `8`   | Unknown outcome | An effect may have been applied and cannot be confirmed                                                                         |
| `9`   | Failed          | The operation was accepted and ended in a failed terminal result                                                                |
| `130` | Interrupted     | Default `SIGINT` semantics                                                                                                      |

- A refused, unavailable, broken-channel, or incompatible outcome on a mutation means nothing was
  applied. Only unknown outcome admits that an effect may exist.
- An interrupted wait does not cancel the observed operation; see
  [`process.md`](process.md#signals-and-cancellation).
- Codes not in the table are reserved and unreachable.

## Text rendering

- User-controlled strings render as quoted, escaped values: backslash and quote are escaped; C0
  and C1 controls including `ESC`, U+2028, U+2029, and bidirectional formatting controls render as
  `\uXXXX` escapes; other Unicode is preserved.
- Tables truncate only display columns of labels to a declared width with an ellipsis, keeping the
  escapes and closing quote intact. Identifiers are never truncated, and `get` output is complete.
- Display width is measured in terminal cells per
  [TUI Visual Foundations](../tui/visual-foundations.md#display-width-and-text-safety), never in
  bytes or scalars.
- Text output carries no color or styling sequences; a command-line invocation does not query the
  terminal.

## Bounds and write failures

- The complete document is prepared in a bounded buffer before the first byte reaches standard
  output. Exceeding the declared limit is a local failure that writes nothing to standard output.
- A write or flush failure on standard output exits nonzero, never appends a second document, and
  reports on standard error when that stream still works.
- Echoed untrusted tokens on standard error are cut to a declared scalar prefix with an ellipsis;
  non-Unicode and oversized argument lists are described, never echoed.
- Standard error diagnostics are bounded to one cause and one hint per failure; read paths print
  no progress.

## Streamed records

- A command that emits many results over time writes one JSON record per line. Every record names
  its kind; the stream ends with one terminal record stating why it ended.
- A deadline, stall, or channel loss ends the stream with a typed terminal record and a nonzero
  exit; a partial stream is never presented as success.
- Records within one stream keep the source order, and a record is never split across lines.
- Under `text`, a streamed command renders each record as it arrives with the same escaping rules.

## Localization

- Standard error diagnostics, help, and the `text` projection are localized under
  [Rust](../RUST.md#localized-product-copy); `json` documents, schema identifiers, enum values,
  completion scripts, and exit codes are identical under every locale.
- A help hint names a real path, spelled exactly as the grammar authority spells it, that exits
  `0` without a runtime.
- Every diagnostic exists in both shipped locales in the same change.
