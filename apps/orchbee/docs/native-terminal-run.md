# Native terminal command submission

## Command and ownership

`orchbee terminal run <target> <command>` submits one quoted, nonempty UTF-8 command to an existing Terminal. Use `--` before operands resembling options. Command bytes, whitespace, quotes and newlines remain unchanged; separate command operands are rejected. The argument envelope is bounded to 64 KiB and paste to 256 KiB. The selected terminal's current program interprets the command.

Without `--wait`, one snapshot resolves the target and one input request sends paste followed by Enter. The five-second send budget and `orchbee.terminal.send` receipt govern this path. A receipt confirms input progress only. Partial or unknown delivery is never replayed. No viewer, controller, resize or focus is acquired.

## Correlated completion

On Windows, `--wait [--timeout <ms>]` uses a read-only viewer and the managed PowerShell integration. Its total deadline defaults to 30000 ms, including discovery, readiness observation, submission, polling and cleanup. Unsupported or unready integration refuses before input. The caller must own an empty editable command line; concurrent user input is not excluded by a viewer.

Orchbee `cmd` marks its prompt and input start for the Host's prompt gate, but never its commands, and mints no run token. `--wait` therefore refuses before input in `cmd`, and an Agent run started from `cmd` is never `confirmed`.

The CLI generates a random invocation ID and sends one paste/Enter request with a comment prefix. The verified PSReadLine wrapper strips exactly that prefix and returns the original command to PowerShell without evaluation in a new scope. The prefixed line enters neither PSReadLine history nor `Get-History`, so Up-arrow recall never resubmits a stale ID. Private APC markers echo the ID at command start and completion. The wrapper exports the ID as `ORCHBEE_RUN_TOKEN` to that command only, and the prompt removes it on completion. A named Agent start reuses this prefix with a Server-minted run token; see [Windows Agent startup](windows-agent-start.md#launch-receipt). An unrelated OSC 133 completion cannot finish this wait. The Host retains only the latest correlated invocation; missing or overwritten evidence can time out and never authorizes replay. This is cooperative shell evidence, not an authentication boundary against the terminal process.

JSON uses `orchbee.terminal.run` v1. `submission` preserves the complete input receipt; `completion` separately carries `exitCode`, `statusKind: powershell-success`, and physical output coordinates. PowerShell status is 0 for `$? = true`, 1 otherwise; it is not a guessed native process exit code. The output range's `retained` field is false after generation/origin changes. `failure` retains timeout, cancellation or transport facts without erasing accepted input or observed completion. `cleanup` records the viewer obligation. Cancellation does not stop the shell command. JSON is the recovery format; choose it before submitting scripted work.

## Typed-run boundaries

The same wrapper marks every other nonblank line PowerShell accepts from the user. It mints a random 32-digit lowercase hexadecimal token in the shell, exports it as `ORCHBEE_RUN_TOKEN` to that command only, and echoes it in the same private start and finish markers. The line, PSReadLine history, `$?` and `$LASTEXITCODE` stay unchanged, nothing visible is printed, and no OSC 133 completion is added, so command feedback is the same as before. A prefixed line keeps its submitted ID, so Agent start and `--wait` behave as before. A typed command can replace the Host's latest invocation, as any later command already could.

After each process sample the Server reads the Terminal's open invocation from the frame captured after that sample. An Agent run is `confirmed` while the invocation it appeared in stays open, provided the invocation started its anchor: every process from the anchor up to the shell, the sample's Job root, was created after the Host read that invocation's start marker. Samples cannot prove this: a frame names only the latest invocation, and an Agent left by an earlier command can appear between a sample and the next start. The Host therefore reads its real-time clock when the marker arrives. The shell exports the token before it writes the marker, so the anchor inherits it. A process an earlier command left running can still start an Agent during a later invocation, which is why the whole chain counts and not the anchor alone. Without the Host's time, with a missing or older process in the chain, or once the invocation finishes, the run stays `recognized`. The Agent's hooks inherit the token and bind its session exactly, as described in [Windows Agent integrations](windows-agent-integrations.md). cmd, an unverified PSReadLine, and unmanaged shells mint no token and stay at `recognized`.

- A token reaches only the command that the wrapper marked, and the prompt removes it on return.
- Typed tokens are shell-minted and never submitted as input; only the Host's echoed markers carry them to the Server.
- The grade never rises without an open invocation whose start precedes every process between the shell and the anchor.

## Fresh output boundaries

`terminal read --format json` supplies an opaque cursor when the Host can identify the primary screen's row origin. Pass it to `terminal wait-output <target> --after <cursor> --contains <text>`, or use `--after now` to begin at the first observed frame. Only later physical rows participate; the occupied boundary row and prompt/input rows are excluded. Existing output is never counted as fresh.

Host restart, generation changes, history eviction and gaps beyond the chosen window reject the cursor. Select a sufficient `--lines` window and an explicit `--timeout`. A new cursor is a new observation boundary, not recovery of lost output. See [terminal output](native-terminal-output.md).

## Evidence and compatibility

Parser, cursor, wire-decoder and Host reader-batch cases cover exact correlation, unrelated completions, invalid records and non-replay. Windows named-pipe cases cover the existing single paste/Enter receipt path. Same-author implementation tests are the evidence; no independent review or exhaustive schedule proof is claimed. Help, completion adapters, the offline Agent guide and schemas evolve together. Older binaries reject new options; existing sessions need newly initialized managed PowerShell for correlated waiting.
