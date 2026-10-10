# Native terminal command submission

## Command and ownership

`orchbee terminal run <target> <command>` submits one quoted, nonempty UTF-8 command
string to an existing Terminal. Use `--` before operands that resemble CLI options.
The global `--format text|json` still selects the input receipt projection. The
complete argument envelope remains bounded to 64 KiB; the existing paste limit is
256 KiB. Command bytes, whitespace, quotes and newlines are preserved. Separate
unquoted command operands are rejected; Orchbee does not reassemble shell argv.

The current program in that Terminal interprets the content. The CLI does not start
another process, choose a Shell, inspect a prompt, or claim that the Shell is ready.
A submission receipt confirms input progress, not command completion or its exit
code. Observe output or canonical Agent state separately when needed.

## Input lifecycle model

| Stage     | Rule                                                                                                                                                      |
| --------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parse     | Resolve grammar, command bounds and target syntax before discovery or runtime effects. Empty text, extra operands and send-source flags are usage errors. |
| Resolve   | Reuse the Terminal ID or typed-suffix resolver over one snapshot and one admitted session. No implicit focus target is selected.                          |
| Submit    | Lower to one existing terminal input request with a paste record followed by semantic Enter. Host encodes paste according to the current terminal mode.   |
| Settle    | Preserve the existing `orchbee.terminal.send` receipt, exact request/target identities, accepted-prefix evidence and cleanup status.                      |
| Uncertain | A lost answer remains unknown; a confirmed prefix remains partial. Never replay, append another Enter, or infer execution success.                        |

The existing five-second send deadline, cancellation and failure vocabulary govern
this entry point. It acquires no viewer, controller, resize or focus authority and
creates no new shared protocol or durable state. `terminal send` keeps its explicit
source/key forms. The shortcut carries two ordered records in one request; it does
not add stronger all-or-nothing delivery to the existing send contract.

## Evidence and compatibility

Parser evidence covers opaque quoted/Unicode/multiline text, the argument separator,
missing/empty/extra operands and forbidden source flags. Windows named-pipe process
evidence pins a single snapshot plus a single paste/Enter request, accepted/partial/
unknown results and no replay. A staged Windows cohort exercises actual PowerShell
5.1 and 7 execution while a TUI retains its controller and focus.

Help, all completion adapters, the offline Agent Skill and this reference evolve
together. The schema catalog remains v36/69 because no document shape changes.
Older executables reject the new verb; rollback needs no data migration. Evidence
is Windows-only and same-author; This workflow uses implementation tests. Root verification
is required before integration.
