# Orchbee Launcher

The Windows launcher of a release installation. A Windows release directory carries it as
`<stem>-launcher.exe`; the PATH step of `orchbee install` copies the active version's copy to
`<store root>\bin\<stem>.exe` and puts `<store root>\bin` on the user `Path`, so typing `orchbee` in
a new terminal starts the active version. The contract is
[Release distribution](../../proposals/platform/release-distribution.md#runtime-store); the store
and the PATH step are described in
[Windows CLI configuration](../orchbee/docs/windows-cli-configuration.md#windows-release-installation).

## Behavior

- From its own path `<root>\bin\<name>.exe` it reads `<root>\active.json` with the Client's pointer
  parser from `orchbee-release-records`, and starts `<root>\versions\<product>\<name>.exe`. It reads
  no other file and no configuration; the one setting it follows, whether the
  [relaunch protocol](#relaunch-protocol) runs, is compiled in.
- The Client receives the launcher's command line after the program name exactly as written, the
  same console and standard handles, the environment, and the working directory. A command line
  that is not valid UTF-16 passes argument by argument instead, so the Client reads the same
  arguments.
- The Client runs in a Job with `KillOnJobClose` and `SilentBreakaway`: ending the launcher ends the
  Client, while the Server, Worker and Terminal Host the Client starts are created outside the Job
  and keep running. The Client is not created suspended: it is assigned right after creation, so
  the launcher adds about one process start to each `orchbee` invocation (the measurement is in
  the contract).
- Ctrl+C and Ctrl+Break reach the Client through the shared console. The launcher consumes them
  with a handler, which the Client does not inherit, waits for the Client, and exits with its
  exit code unchanged, including a status such as `0xC000013A`.
- When the pointer is missing, unreadable, or names no valid version, or the active Client cannot
  start, it prints one line on standard error in the locale `LC_ALL`, `LC_MESSAGES` or `LANG`
  selects and exits `1`. If the Client ran but its status cannot be read, it exits `8`.

## Relaunch protocol

Where the local runtime's
[automatic-update switch](../../crates/local-runtime/README.md#automatic-update-switch) is on, the
launcher follows the protocol of
[What a click does](../../proposals/platform/automatic-updates.md#tui); where it is off, it behaves
as above and sets no variable. The npm launcher follows the same rule on Windows, macOS and Linux.

- Every Client it starts receives the launcher's own process id in decimal as `ORCHBEE_RELAUNCH`
  and as `ORCHBEE_RELAUNCH_V1`, the only spelling releases up to 0.1.0-beta.24 read.
  A Client uses the statuses below only when that process is its parent, so a value it inherited
  from another launcher does nothing.
- Status `75`: the Client restored the console and asks for the active version. The launcher reads
  `active.json` again and starts that version with the same command line in the same console, at
  most three times in ten seconds; a fourth request inside that time ends the launcher with `1`, so
  no status outside the CLI exit codes reaches the caller.
- Status `76`: the Client could not join its Session. It uses it in place of the startup failure
  `1` it ends with without a launcher, so a caller still reads `1` once the launcher ends. Only the
  TUI's two failures that speak against its version count: its cohort check fails, or the running
  Session is older than it can join. A console, deadline, environment, or connection failure stays
  `1`, so it never sends the person back a version.
- The Client keeps the variable from every process it starts where its spawner can: on Unix, and
  for the store Client's `server start`. A Windows Server started directly inherits it, because the
  safe process creation this repository uses cannot pass a changed environment, and so do its
  Worker and Terminal Host; the parent check keeps those copies inert. No program they start for
  the person receives it: every terminal's launch environment removes it, and so do configured
  commands, plugin jobs, and the automatic update helper.
- After every start, the first one or a relaunch, the launcher tells a failed start or join from a
  normal exit by status alone: no process started, status `76`, Rust's panic status `101`, or an
  error-severity `NTSTATUS` other than `STATUS_CONTROL_C_EXIT` is a failure; every other status,
  each CLI exit code included, is a normal exit. Statuses therefore never misread a command's
  ordinary failure as a broken version.
- When the Client of a relaunch fails to start or to join, a pointer naming no version included,
  the launcher starts the version it replaced with the same command line. After a first start, or
  such a return, a failure ends the launcher with the Client's status as above.
- After any start whose Client failed, before it goes on, the launcher reads the pointer again. When
  the pointer still makes the failed version active through a counting move whose `back` is true
  and names a `previous`, it runs the previous version's Client as
  `versions\<previous>\<name>.exe --record-client-failure <version>`, spelled
  `--record-client-failure-v1` for a previous version up to 0.1.0-beta.24, outside the Client's Job
  with null standard streams, which
  [records the failure](../../proposals/platform/automatic-updates.md#what-counts-and-who-records-it)
  for the fallback after a failed move. It waits for that helper at most five seconds and leaves it
  running after that; a helper that cannot start records nothing.
- The decisions are pure functions in `src/relaunch.rs`, tested in both launchers. The failure
  record's helper comes from `client_failure_helper` in
  [release-records](../../crates/release-records/README.md), which a macOS or Linux TUI with no
  launcher also uses.
- On macOS and Linux it prints that it runs only on Windows and exits `4`; those platforms install
  a link instead.

The release packaging builds it with the cohort and lists it in the signed manifest outside every
component, because it speaks no generation contract; `cohort.json` does not list it.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-launcher --locked
cargo clippy -p orchbee-launcher --all-targets --locked -- -D warnings
cargo clippy -p orchbee-launcher --all-targets --locked --target x86_64-unknown-linux-gnu -- -D warnings
```

The unit tests cover pointer-to-Client resolution, the command-line tail, the relaunch decisions
and which failed Client is recorded. Console, signal, exit code and process-lifetime behavior need
a staged test package and a real terminal.
