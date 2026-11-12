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
  no other file and no configuration.
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

The unit tests cover pointer-to-Client resolution and the command-line tail. Console, signal, exit
code and process-lifetime behavior need a staged test package and a real terminal.
