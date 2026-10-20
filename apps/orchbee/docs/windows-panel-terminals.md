# Windows Panel terminal creation

`orchbee panel create-terminal [<panel>|--panel <target>|--current] [--client <id>]
[--child|--presentation <id>] [--label <label>] [--exec <path|name> --arg <value>...|--profile <selector>]
[--cwd follow|home|current|workspace|<absolute-path>] [--env NAME=VALUE] [--timeout <ms>]`
creates a terminal inside an existing Panel without splitting its geometry. It is Windows-only.
Omitted member selection binds the unbound primary; `--presentation` selects an exact unbound
primary or child in that Panel; `--child` appends a fresh child, up to 16. A bound target is refused,
and a Presentation in another Panel is never selected. `--label` is child-only; its default is the
next numeric member label. Binding an existing member preserves its labels and input policy.

An explicit Panel works without a TUI. An omitted/current Panel uses the validated caller, then
one Ready Client; `--client` selects that Client and conflicts with an explicit Panel. Targets use
the existing Panel IDs/handles. The command does not alter Client focus. Exact member focus and
individual member removal are separate lifecycle obligations.

Launch/profile are mutually exclusive. Geometry uses the existing columns/rows/cell-pixel flags.
Arguments and environment values remain literal. Existing creation configuration and cwd policies
apply: follow reads only the owning Panel primary's process cwd, never a child in place of an
unbound primary; missing runtime cwd uses the launch default. Input validation occurs before
runtime discovery. The default total deadline is 30000 ms.

The CLI resolves one canonical snapshot and sends exactly one retained `panel.create-terminal`
request. The Server owns grants, revision/Close fences, identity allocation, launch and settlement.
`orchbee.panel.terminal-create` v1 retains Session/request/base revision, exact target, directory
provenance and the full pending or final outcome without exporting argv or environment values.
Pending means accepted, not yet started. Known physical failure/unavailability and unknown outcomes
return nonzero while retaining committed identities. Lost, invalid or mismatched post-write replies
retire the connection and report unknown; no retry or compensating close occurs. Signal cleanup
preserves the recorded outcome and produces the cancellation exit when required.

The shared creation adapter also serves layout materialization/binding. Matched native CLI/Server
binaries roll out and roll back together; no durable migration is introduced. Parser/target/receipt
checks and scoped Windows CLI peers verify the boundary; real-provider and complete TUI acceptance
remain part of the final Windows regression.

Executable names use the final CLI PATH and retain the resolved absolute path in receipts; see [Executable names](native-terminal-creation.md#executable-names).
