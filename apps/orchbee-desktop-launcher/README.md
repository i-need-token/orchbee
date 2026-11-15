# Orchbee Desktop Launcher

The Windows window launcher of a release installation. A Windows release directory carries it as
`<stem>-desktop-launcher.exe`, and every window version holds its own copy under
`resources\cohort\`. The window step of the store places the active window version's copy at the
window entry `<store root>\desktop\orchbee-desktop.exe`, the path the Start menu shortcut, the
taskbar and the uninstall registration name, so starting Orchbee starts the active window version.
The design is [How the window updates](../../proposals/platform/automatic-updates.md#window-launcher).
Staging builds it into every Windows cohort and release packaging requires it; Setup and the window
step do not place it yet, so nothing starts it.

## Behavior

The decision is one pure function in `src/decision.rs` of the pointer, the arguments, the parent
check and which programs exist; its steps run in this order.

1. From its own path `<root>\<name>.exe` it reads `<root>\active.json`, at most 16 KiB, with the
   store pointer's parser from `orchbee-release-records`, and starts
   `<root>\versions\<active>\<name>.exe`. It reads no other file and no configuration.
2. When its first two arguments are `--relaunch-after-v1 <process id>`, it drops them. When that
   process is its parent, which started no later than the launcher, it first waits up to 30 seconds
   for it to end, because the window's single-instance lock would otherwise hand the new command
   line to the old window. After 30 seconds it starts anyway.
3. It passes the remaining arguments one by one and the environment unchanged, with the version
   folder as working folder, and starts no Job: the window outlives the launcher.
4. When the pointer names a `previous` version whose program exists, the active window receives
   the [launcher protocol](../orchbee-launcher/README.md#relaunch-protocol) variable
   `ORCHBEE_RELAUNCH_V1` with the launcher's process id, and the launcher watches it for 120
   seconds. If it cannot start, or ends within them with a non-zero status, the launcher starts the
   previous version once with `--fallback-from-v1=<active version>` before the same arguments and
   without the variable. A window that ends with status 0, as a second instance does, is no failure.
   Without such a version, no start receives the variable, and the launcher exits once the window
   has started.
5. When no window starts, it shows the command launcher's message for the same failure in a message
   box, in the Windows display language (`zh` is `zh-CN`, anything else `en-US`), and exits `1`.
   Otherwise it exits `0`.

It moves no pointer and records no failure: a window failure says nothing about the runtime, whose
fallback keeps its own count. On macOS and Linux it prints that it runs only on Windows and exits
`4`.

## Switches between builds

Every window version from the window self-update release on is started by the launcher of whichever
version the entry holds, so these switches are a contract between builds, like the helper modes of
[release distribution](../../proposals/platform/release-distribution.md#commands). Their grammar
never changes; a change adds a new name.

| Switch                                          | Given by                                                    | Read by                                                                                     |
| ----------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `--relaunch-after-v1 <process id>`              | The window, first and as two arguments, when it relaunches  | The launcher, which drops both and passes neither on                                        |
| `--fallback-from-v1=<version>`                  | The launcher, first, when it starts the previous version    | The window: no Update for that version while it runs                                        |
| `--select-terminal-v1=<TerminalPresentationId>` | The window, when it relaunches                              | The window it relaunches, which opens at that terminal; the launcher passes it on unchanged |
| `--select-workspace-v1=<WorkspaceId>`           | The window, when it relaunches without a displayed terminal | The window it relaunches, which opens that Workspace; the launcher passes it on unchanged   |

Only the exact `--relaunch-after-v1` in first place is the launcher's, and the argument after it
names a process only as a decimal number from 1 without a sign or a leading zero; the launcher drops
both either way. Anywhere else, or written as `--relaunch-after-v1=<id>`, it is an ordinary argument
that the window receives.

## Icon

`build.rs` writes a compiled resource file holding `resources/brand/ico/orchbee.ico` into
`OUT_DIR` and passes it to the linker on Windows MSVC targets, so the entry shows the application
icon. No resource compiler runs and no crate is added.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee-desktop-launcher --locked
cargo clippy -p orchbee-desktop-launcher --all-targets --locked -- -D warnings
cargo clippy -p orchbee-desktop-launcher --all-targets --locked --target x86_64-unknown-linux-gnu -- -D warnings
```

The unit tests cover the decision and the locale. The wait, the watch, the message box and the
icon need a staged test package and a real window run.
