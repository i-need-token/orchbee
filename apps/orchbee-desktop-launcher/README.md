# Orchbee Desktop Launcher

The window launcher of a release installation, on Windows and macOS.

On Windows a release directory carries it as `<stem>-desktop-launcher.exe`, and every window version
holds its own copy under `resources\cohort\`. The window step of the store places the active window
version's copy at the window entry `<store root>\desktop\orchbee-desktop.exe`, the path the Start
menu shortcut, the taskbar and the uninstall registration name, so starting Orchbee starts the
active window version. The design is
[How the window updates](../../proposals/platform/automatic-updates.md#window-launcher). Staging
builds it into every Windows cohort and release packaging requires it. The window step of `update`,
of the automatic helper and of `--installer-window-v1`, which Setup runs, places it
([the arrangement](../orchbee/docs/windows-cli-configuration.md)), in every build where window
updates run.

On macOS a release directory carries it as `<stem>-desktop-launcher`, inside every application
bundle's `Contents/Resources/cohort/`. The bundle itself is the entry there, so the launcher runs
only when a window relaunches into a newer window version ([macOS](#macos)). Staging builds it into
every macOS cohort and release packaging requires it; nothing writes a window version to the macOS
store yet, so nothing starts it.

On Linux it prints that it runs only on Windows and macOS and exits `4`.

## Behavior

The Windows decision is one pure function in `src/decision.rs` of the pointer, the arguments, the
parent check and which programs exist; its steps run in this order.

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
fallback keeps its own count.

## macOS

The window store is `desktop/` under the store root: the window pointer `desktop/active.json`, in
the store pointer's format, and one application bundle per window version,
`desktop/versions/<version>/<name>.app`. A window that switches starts the launcher in the cohort of
the version it switches to. The decisions are pure functions in `src/replacement.rs`, sharing the
arguments, the fallback arguments and the step after a start with `src/decision.rs`; the effects
are in `src/macos.rs`. The steps run in this order.

1. It serves the request only when its own path is
   `<desktop>/versions/<version>/<name>.app/Contents/Resources/cohort/<file>`, its first two
   arguments are `--relaunch-after-v1 <its parent's process id>`, and the third is
   `--replace-application-v1=<application>`: an absolute `.app` path outside `<desktop>`, in any
   letter case, that spells no `.` or `..` component. Otherwise it changes nothing and exits `1`.
2. It reads `<desktop>/active.json`, at most 16 KiB, with the store pointer's parser; the source is
   `<desktop>/versions/<active>/<name>.app` when `Contents/MacOS/<name>` exists in it.
3. It waits up to 30 seconds for its parent to end, which it sees when `launchd` adopts it. A window
   that still runs keeps its application: the launcher changes nothing and exits `0`.
4. It copies the source with `ditto` to `.<application name>.update-<its process id>` beside the
   application and checks it with `codesign --verify --deep --strict`. When that folder cannot be
   written, it copies into `~/Applications` instead, under the same name, creating the folder. It
   then renames what stands at that path to `.<application name>.previous-<its process id>` and
   renames the copy into place. The Session keeps running: the launcher stops no process.
5. It starts `Contents/MacOS/<name>` of the new application, in a process group of its own, with the
   remaining arguments, the environment, and `ORCHBEE_RELAUNCH_V1` naming the launcher, and watches
   it for 120 seconds. When it cannot start, or ends within them with a non-zero status or by a
   signal, the launcher puts back what stood at the path before, or removes the copy where nothing
   stood, and starts the application the window ran from with `--fallback-from-v1=<active>` first and
   without the variable. Otherwise it removes the application it replaced.
6. When the pointer names no active version whose program exists, or the copy, the check or a rename
   fails, it removes its copy, leaves every application as it was, and starts the application the
   window ran from the same way, naming the active version when it read one. It exits `1` only when
   that start fails too; it shows no message.

The renames stay in one folder, so each is atomic; a launcher that is killed between them can leave
its hidden `.update-`, `.previous-` or `.failed-` folder beside the application. The window's
Session runs from the store's version folder, since the application's bundled Client hands its
start to the store
([store start](../orchbee/docs/windows-cli-configuration.md#macos-and-linux-release-installation)),
so a replacement changes no program that Session starts later. Only a Session that the bundle's
own cohort started, because the store held no usable version then, starts its later programs from
the new bundle at the same path.

## Switches between builds

Every window version from the window self-update release on is started by the launcher of whichever
version the entry holds, so these switches are a contract between builds, like the helper modes of
[release distribution](../../proposals/platform/release-distribution.md#commands). Their grammar
never changes; a change adds a new name.

| Switch                                          | Given by                                                    | Read by                                                                                     |
| ----------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------------------------------------------- |
| `--relaunch-after-v1 <process id>`              | The window, first and as two arguments, when it relaunches  | The launcher, which drops both and passes neither on                                        |
| `--replace-application-v1=<application>`        | The macOS window, third, when it relaunches                 | The macOS launcher, which replaces that bundle and passes the switch on to no window        |
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
cargo clippy -p orchbee-desktop-launcher --all-targets --locked --target aarch64-apple-darwin -- -D warnings
```

The unit tests cover both decisions and the locale; the macOS ones run on a Mac. The wait, the
watch, the message box, the replacement and the icon need a staged test package and a real window
run.
