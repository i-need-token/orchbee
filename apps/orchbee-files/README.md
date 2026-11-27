# Orchbee Files

`orchbee-files` serves the File Bar for one Worker generation: it lists, reads and watches files and
runs Git for every Workspace of its Session on its machine. The Worker starts it when the Server
asks, and it then talks only to that Server, over the private OBFL link that
[`orchbee-files-protocol`](../../crates/files-protocol/README.md) defines. It accepts no Client
connection, reads no Orchbee setting and runs no command a Client names. Scope and containment are
checked here, against the Workspace roots only the Server sends; the Server relays File Bar records
without reading their paths.

## Executable interface

The Worker is the only launcher, so this interface is a contract between the two programs rather
than a command.

| Fact            | Contract                                                                                                                                             |
| --------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| Invocation      | `--local-endpoint <endpoint>`: the absolute address of the one-shot endpoint the Server bound for this start                                         |
| Standard input  | The start's token, 64 lowercase hexadecimal digits and a line feed. It never appears among the arguments, which other processes of the user can read |
| Standard output | `ready` and a line feed once the Server admitted the hello                                                                                           |
| Exit            | 0 after the Server's drain; `128 + signal` after `SIGTERM` or `SIGINT` on macOS and Linux; 1 for every other end, which the Server counts as a crash |

Every end cancels the open calls first and gives their operations three seconds to end their
children, so a Git or ripgrep process tree does not outlive the call that started it. A panic on
any thread ends the whole process instead of unwinding that thread, because a half-serving process
would leave Client streams open; the Server restarts it on the next demand.

## Diagnostic log

The process writes to the [daily diagnostic log](../../crates/local-runtime/README.md#diagnostic-log)
with role `files`: `started` once the Server admitted it, and one `exited` with `outcome`
(`drained` with its `reason`, `idle` or `session`; `signalled` with its `signal`; or `failed` with
the `stage` it stopped at and the I/O `error` kind that caused it, when one did).
