# Local Runtime

Private native runtime facilities shared by the Client, Server, and Worker. This library
owns descriptor-relative runtime admission, lock/marker primitives, and immutable development
cohort verification. Applications own process launch, service state, and terminal authorization.

## Shared ownership and consumers

This crate owns admitted shared capabilities and stable runtime lifecycle contracts. Applications retain composition and product policy; Server frame directories belong to `apps/orchbee-server/src/frame_directory`, and CLI immutable release publication belongs to `apps/orchbee/src/release_store`.

| Owner modules                                                              | Production consumers and shared responsibility                                                                                                                                                                                                              |
| -------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cohort`, `rendezvous`, `rendezvous_types`, `retained_child`               | Client activation, Server admission and Worker startup share cohort identity, locks and process lifetime primitives.                                                                                                                                        |
| `session_name`, `session_catalog`, `activation_record`, `durable_sessions` | Client enumeration/activation/deletion and Server retention/stop depend on the same identity and lifecycle records; the catalog deletion fence also guards rendezvous admission. Internal records are not exposed as mutable application APIs.              |
| `process_registration`, `process_observation`, `registered_child`          | Client-created Server, Server-created Worker and Worker-created Host share registration-before-execution and exact process identity recovery.                                                                                                               |
| `session_store`, `checkpoint_rotation`, `checkpoint_restore_notice`        | Server checkpoint publication/rotation and Client recovery diagnosis/stopped deletion share fixed slots, activation generations and storage lock guards.                                                                                                    |
| `default_interactive_launch`, `notification_policy`                        | The Client, Server and Worker consume launch snapshots or notification profiles; settings decoding uses notification types directly without platform aliases.                                                                                               |
| `data_root`, `user_settings`, `plugins`, `user_path`                       | Server configuration and the Client's minimal `config.toml` reads (activation, release channel and `config check`) share the bounded settings schema; plugins and paths also serve Worker execution, and Windows path grammar stays explicit on every host. |
| `command`, `command_outcome`, `command_child`, `directory`                 | Client commands/provisioning, Server restart and Worker commands/directory observations share encoding, outcomes and process settlement contracts.                                                                                                          |
| `private_cache`, `private_filesystem`, `release_store`, `host_appearance`  | Runtime storage, CLI release storage and Server frame reads share exact handle admission; the Client's release commands and Session start share one store reader; Client and Worker share read-only Windows theme observation.                              |

## Shared types and platform effects

Settings and plugin schemas, command outcome types, named-Session syntax, Windows path operands
and literal command encoding are available on every supported platform. Windows path admission
uses its explicit grammar even on Unix; environment and cwd are supplied by the execution owner.
OS effects stay in their owning adapter. The plugin registry refuses unsupported Unix operations
before opening a path or spawning a process with `PluginRegistryError::Unsupported`; the
[private update cache](#private-update-cache) runs on every host. The
[user settings](#user-settings) reader and editor run on every host.
Applications decide action visibility and localized feedback through their
[TUI platform contract](../../apps/orchbee/docs/tui-platforms.md).

Product types for Session identity, lifecycle, checkpoints and command outcomes are also available on every supported platform. Implementations live in `windows`, `unix`, `linux` or `macos` child modules. The Session catalog, durable and activation records, checkpoint store and persisted-data versions share one implementation over the [private storage seam](#private-storage-on-macos-and-linux); unimplemented registered-child operations return `io::ErrorKind::Unsupported` before effects. `CommandChild` retains one command and everything it starts: a Job on Windows, its own process group on Unix (see [retained commands](#retained-commands)). `NamedSessionDeleteReason::Unsupported` stays for a host without a catalog, which no supported host is now; keybinding reset uses the stable `platform` / `unsupported-platform` settings diagnosis. Existing Unix rendezvous, cohort and retained-child implementations remain with their own adapters.

## Shared launch and notification policy

`default_interactive_launch` owns pure platform shell selection and native executable admission,
shared by Desktop's matched helper and Native Worker. It starts no process.
`UserTerminalLaunchIntent::capture` takes scrollback, colors and the selected launch profile
together from one confirmed set of setting values; an explicit executable bypasses default shell
resolution while retaining the shared scrollback default, and a catalog without its default profile
refuses the creation. A custom executable must be absolute in its platform's form; POSIX login is
explicit and Windows login is refused. Every `config.toml` launch profile applies this one rule.

`UserTerminalColors` resolves terminal default colors for both native presentation and new Worker
profiles. Theme and Host take the canvas of the active slot's theme when application appearance is
explicitly Light or Dark; System leaves Host defaults with their runtime owner, which builds them
with `UserTerminalColors::from_rgb`. Custom takes precedence under every appearance; Custom and
`from_rgb` derive the dark or light scheme from background luminance, and a theme canvas keeps its
declared polarity. `UserColorThemes` carries the stored `lightTheme` and `darkTheme` ids unchanged
and resolves each through `orchbee_color_themes::select`, so an id this build does not ship, or one
of the other polarity, takes the Orchbee theme of the slot's polarity. The launch snapshot freezes
the colors and both slots with scrollback and launch selection; the resolver performs no terminal
mutation.
Retained query state and reversible Client appearance follow the [native theme boundary](../../apps/orchbee/docs/native-themes.md).

`NotificationSettings` owns portable bounded delivery/sound policy. Windows configuration
decoding accepts the declared current keys; TUI observation and card scheduling consume the
portable type. Its complete private notification profile carries the current effective delivery
and global sound policy, including the values the common setting keys set.

## Windows host appearance fallback

`read_windows_apps_use_light_theme` performs a read-only current-user lookup of
`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
It returns `Some(true)` or `Some(false)` only for the documented `REG_DWORD` values `1` and `0`;
missing, unreadable, and other values return `None`. The Client invokes this once only when its
native terminal OSC `10`/`11` probe has no valid OSC `11` background by the deadline. The Worker
invokes it for a Host-default launch before any Client has reported `host-colors`.

## macOS host appearance fallback

`read_macos_dark_appearance` reads `AppleInterfaceStyle` from the global preferences domain through
one `/usr/bin/defaults read -g AppleInterfaceStyle` child under a 300 ms deadline, without changing
any setting. The key names `Dark` while the system appears dark, under the automatic appearance
too, and is absent while it appears light. Exit status 0 with `Dark` returns `Some(true)`; status 1
with the absent-pair message returns `Some(false)`; every other answer, a timeout or a failed
start returns `None`. The Client invokes it once, at the same point as the Windows fallback. Each
caller owns its precedence; this runtime helper changes no Windows setting and reads no other theme
state.

## Windows literal command encoding

The pure Windows command encoder is shared by CLI Agent start and Server restart. It emits a bounded UTF-16LE Base64 script with one literal argument array. Native, batch and PowerShell script dispatch preserve argument boundaries, including empty values, quotes and percent signs. It excludes aliases/functions, rejects control characters, caps encoded text at 30,000 characters and refuses batch command lines over 8,000 characters. Applications select programs, shells, interactive flags and stricter carrier limits; this library starts no process. Helper functions and error preferences remain in a child scope when the caller keeps PowerShell open. The embedded PowerShell asset is maintained with this encoder and the CLI's real Windows argv test.

## Retained commands

`CommandChild` owns one spawned command and every process it starts until settlement. Callers
bound output, deadlines and cancellation; the owner gives them one contract: `try_wait` reports a
status only after every owned process has ended, `settle` ends them all within five seconds or
fails, a failed settlement never later turns into a status, and spawn failure says whether a
process was created. Drop starts termination and never waits. On Windows, settlement and
`wait_root_exit` wake when the root process exits instead of sleeping in steps; `wait_root_exit`
reports root exit only and leaves the status to `try_wait`.

| Platform | Ownership                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Windows  | A suspended-assigned Job; settlement terminates the Job and waits for its all-processes-exited notification.                                                                                                                                                                                                                                                                                                                                                                    |
| Unix     | The root leads a new process group. It is observed without reaping, so its PID keeps the group ID reserved; settlement of a running root sends `SIGTERM` to the group and allows two seconds that end early when the root exits; then the group receives `SIGKILL` (a group with no live member, which macOS refuses with `EPERM`, has nothing to kill), the root is reaped and settlement waits until the group is empty. Drop sends `SIGKILL` and reaps on a detached thread. |

A process that starts its own session or group leaves Unix ownership; Windows Jobs forbid
breakaway by default. On Windows, `spawn_contained` also marks the Job to end every member when its
last handle closes, so the owner's end, however abrupt, ends the command's whole tree; the machine
hub starts its carriers this way, and `run_contained_command_exchange`, the contained form of
`run_windows_command_exchange`, runs the updater's `curl` and `tar`. `spawn_background` is
`spawn_contained` at below-normal priority, which every process the command starts inherits;
`run_background_command_exchange` runs the Session Server's automatic update helper that way. Otherwise neither platform ends owned processes when the owning
process dies abruptly:
`PR_SET_PDEATHSIG` would need `pre_exec`, which this crate's denial of unsafe code excludes, and is
Linux-only and thread-scoped. Output pipes still close then, so a writer receives `SIGPIPE` or an
error. Unix tests cover root exit, cancellation, ignored `SIGTERM`, dropped owners and missing
programs with a real descendant in the group.

On macOS and Linux, `run_unix_command` runs one command to its end, the Unix counterpart of
`run_windows_command`: standard input is null, both outputs are drained on their own threads to
the caller's bound, and the owner is polled every 10 ms until the command ends, the deadline passes
or the caller's interruption check answers true, which settles the whole group before the result.
`run_unmasked_unix_command` runs the same loop for a caller whose thread blocks signals, such as
the Client's release commands on macOS, which capture `SIGINT` and `SIGTERM` to settle their
children: the command starts through nix's safe `posix_spawn` wrapper with an empty signal mask,
`SIGPIPE` at its default action, standard input from `/dev/null` and its outputs duplicated onto
the drained pipes, as the leader of its own process group. The standard library's spawn would pass
the blocked mask on, so the group's `SIGTERM` could not end it before the grace. The caller states
whether the environment is inherited or cleared, since `Command` does not report `env_clear`, and
a command that sets a working directory is refused. The Client's install, update and automatic
helper run the staged Client's version check, the published copy's completion, `curl` and `tar`
through it on macOS.

On Windows, a program Orchbee itself chooses from the system folder, such as `cmd.exe` or Windows
PowerShell, is named by its full path from `windows_system_program`, which reads the folder from
the operating system rather than from `PATH` or `SystemRoot`. A same-named program earlier on the
search path, such as a tool manager's shim, cannot stand in for it. Programs the person chooses,
such as an Agent, Git or a configured shell, keep their own resolution.

## Windows standard rights

`token_rights` reads this process's own token through safe calls: its integrity (above Medium,
`0x2000`, is High), whether BUILTIN Administrators is enabled (`CheckTokenMembership`), listed for
deny only or absent, and `TokenIsElevated`, which callers record but never decide on.
`TokenRights::is_standard` holds at integrity Medium or below with Administrators not enabled.
`relaunch_with_standard_rights` serves a process that fails it, such as the remote link helper
Win32-OpenSSH starts with an elevated administrator token: it derives a standard token from the
process's own, starts the same executable with one argument and that token, waits, and returns the
child's exit status. It never starts anything with the elevated token; a failure is `Derive` (a
derivation call failed), `StillElevated` (the re-read token kept integrity above Medium or an
enabled filtered group) or `Start` (the executable path, the standard handles, the start or the
wait failed).

The derived token keeps the user and the logon session. Every group UAC filters from an elevated
token (BUILTIN `S-1-5-32-544`, `-547` to `-551`, `-553`, `-554`, `-556`, `-569`, the local
administrator account group `S-1-5-114`, and the domain RIDs 498, 512, 516 and 518 to 521) becomes
deny-only; every privilege except SeChangeNotify, SeShutdown,
SeUndock, SeIncreaseWorkingSet and SeTimeZone is deleted; the integrity is Medium, the owner is the
user, and the default DACL grants `GENERIC_ALL` to the user and SYSTEM, so the child's process and
every object it creates admit a Medium process of the same user. The child stays in the parent's
Job.

The private `src/standard_rights_native.rs` adapter is one of this crate's two authored unsafe
boundaries, admitted by the [Rust policy](../../docs/RUST.md#lint-policy-and-enforcement):

- **Handles.** Each handle a call returns moves at once into one owner, an `OwnedHandle` or, for the
  derived token that safe code reads through winsafe, a `CloseHandleGuard`, and is closed exactly
  once. A failed call returns nothing to close; the current-process pseudo handle is never wrapped.
- **Token information.** `GetTokenInformation` runs twice: a null probe for the size, then a read
  into `u64` storage of at least that size, so the structure at its start and the SIDs the system
  places after it are 8-aligned. The reported length must fit the storage; an entry count is checked
  against it before a slice is formed; a SID pointer must lie inside the same block and its
  sub-authority count must keep it there, or the derivation refuses.
- **SIDs and ACLs.** The SIDs passed to `CreateRestrictedToken`, `TokenOwner` and
  `AddAccessAllowedAce` point into token-information blocks or 8-aligned `SECURITY_MAX_SID_SIZE`
  storage that outlive each call, and the system copies them. The Medium label structure and its SID
  share one value, so the length passed with it covers only memory that value owns. The ACL storage
  is sized for its header and both entries, DWORD-aligned, initialized before entries are added, and
  outlives `SetTokenInformation`.
- **Attribute list.** A null probe sizes it; it is initialized into 8-aligned storage and deleted
  exactly once by `Drop`, before its storage and the handle array it names are freed. That array is
  a boxed slice, which never moves while the list refers to it.
- **Inheritance.** The parent takes its three standard handles: it reads each slot, clears all
  three with `SetStdHandle`, and owns each handle once; a slot that repeats an earlier one receives
  its own duplicate, so the handle list holds only distinct values. Only these handles are marked
  inheritable, and `PROC_THREAD_ATTRIBUTE_HANDLE_LIST` limits inheritance to them; without a
  standard handle the child inherits nothing.
- **Start.** `CreateProcessAsUserW` receives the executable path as the application name, a
  writable NUL-terminated command line `"<executable>" <argument>`, a pointer that reaches the whole
  `STARTUPINFOEXW`, `CREATE_NO_WINDOW | CREATE_UNICODE_ENVIRONMENT` without breakaway, and a NULL
  environment, directory and desktop, which inherit the parent's. A restricted copy of the caller's own token needs no
  SeAssignPrimaryToken.
- **Closing and relay.** Right after the start the parent closes the thread handle, the attribute
  list and its copies of the standard handles, so the child is their only holder and its exit ends
  the parent's channel; the parent then waits on the process handle and returns
  `GetExitCodeProcess`'s status unchanged. The caller turns a status above 255 into a refusal.

The adjacent safe test derives a token from the test process's own, reads it back through winsafe
(integrity at most Medium, no enabled filtered group, privileges within the kept set, the user as
owner), starts `cmd.exe` with a test pipe as its only standard handle, reads the child process's
owner and DACL (the user; allow entries for the user and SYSTEM only), and receives its output and
exit status 7. It runs non-elevated, so it does not prove the High-to-Medium step, sshd's window
station or sshd's Job.

## macOS retained spawn

macOS marks a new socket or pipe close-on-exec only after creating it, and the standard library's
spawn passes every descriptor not yet marked. A Server, hub, Worker or Host started while another
thread of its parent was accepting a connection could keep that connection open, so the parent's
peer would never see it close. On macOS, `RetainedChild` therefore starts its child through the
private `src/retained_spawn_native.rs` adapter, this crate's second authored unsafe boundary, with
`POSIX_SPAWN_CLOEXEC_DEFAULT`: the child receives only `/dev/null` as its three standard streams.
It otherwise keeps the standard library's launch: `/` as the working directory, the parent's
environment without the relaunch protocol's variable (and with the given `PATH`), an empty signal
mask and `SIGPIPE` at its default action, so a signal the spawning thread blocks still reaches the
child. The executable must be an absolute path. Linux
creates these descriptors close-on-exec atomically and keeps the standard library's spawn; Windows
is unchanged. Short-lived commands, such as Git, `run_unix_command` and
`run_unmasked_unix_command`, keep the standard library's spawn or the safe wrapper too, because they end before any caller waits on a peer end they might hold.

- **Strings.** The executable, each argument and each `NAME=value` entry become owned NUL-terminated
  strings, refused with `InvalidInput` when one holds a NUL byte; the argument and environment
  vectors point into them, end with a null pointer, and outlive `posix_spawn`, which copies them.
- **Spawn objects.** The file actions and attributes are initialized into owned storage, used only
  after a successful initialization, and destroyed exactly once by `Drop`, after `posix_spawn`
  returns. `posix_spawn_file_actions_addchdir_np`, which libc does not declare for Apple targets,
  is declared locally with its macOS 10.15 signature.
- **Waits.** `waitpid` runs only for an identifier this process spawned and has not reaped, with
  `EINTR` retried. The safe owner keeps the first status it reads and never waits again, because a
  reaped identifier may later name another process.

The adjacent safe test starts a child while a descriptor without close-on-exec is open and sees its
peer close while the child still runs; checks the child's directory, argument boundaries, exact
environment and exit status; and checks that a missing executable starts nothing.

## Data root

`DataRoot` is the one owner of every path in an environment's data root, the folder that holds
everything Orchbee keeps: `ORCHBEE_HOME` when it is set, otherwise `<home>/.<stem>`, where home is
`USERPROFILE` on Windows, as Node's `os.homedir()` resolves it so Desktop derives the same folder,
and `HOME` elsewhere. Resolution reads only the environment its caller supplies and touches no
file; an empty or relative `ORCHBEE_HOME` is `invalid-path` and never falls back to home. The
[layout proposal](../../proposals/platform/data-root-layout.md) owns the tree and its later phases.

| Path                                                 | Owner and use                                                                                                                                |
| ---------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `config.toml`                                        | Default settings file; `ORCHBEE_CONFIG_FILE` moves only this file                                                                            |
| `sessions/<session>/`                                | One Session's folder, named by the Session: its catalog record, checkpoints, kept output and TUI position                                    |
| `agent-detection/<provider>.toml`                    | Detection rule overrides a person writes                                                                                                     |
| `logs/<YYYY-MM-DD>.jsonl`                            | Daily diagnostic log of every process, named by local date                                                                                   |
| `logs/destructive-commands.jsonl`                    | Destructive request log and its `.1` generation                                                                                              |
| `internal/version.json`                              | Layout record and the newest Orchbee version that used the root; `version.lock` and `version.pending` guard its replacement                  |
| `internal/config.lock`                               | Writer lock of the default settings file                                                                                                     |
| `internal/sessions.lock`                             | Namespace lock that serializes Session activation and deletion                                                                               |
| `internal/state/<record>.json`                       | `StateRecord`: the machine catalog                                                                                                           |
| `internal/cache/agent-detection/`                    | Downloaded detection rules and `catalog.json`, which names the URL                                                                           |
| `internal/backups/<date>-upgrade-<version>/`         | Originals of the records one product version migrated, at their paths relative to the root; the three newest folders stay                    |
| `internal/plugins/registry.json`                     | Plugin registry, folder assignments, and recorded cleanup                                                                                    |
| `internal/plugins/<folder>/`                         | One plugin's `config/`, `data/`, and `source/<version>/`                                                                                     |
| `internal/plugins/source.pending/`                   | The source a running install fetches before it moves                                                                                         |
| `internal/integrations/agent-observation/<version>/` | Agent observation Capsules the [integration writer](../agent-integration-writer/README.md#capsule-location-and-ownership) installs and names |
| `internal/machines/hub.lock`                         | The machine hub's single-instance lock; it and `machine_hub_endpoint` freeze at the first public release                                     |
| `internal/runtime/<session>/`                        | A Session's runtime folder in every environment, unless `ORCHBEE_RUNTIME_BASE` names another base                                            |
| `internal/tmp/`                                      | Scrollback documents an earlier build left; nothing writes here, and a default uninstall removes the folder                                  |

On macOS and Linux each folder and file is admitted by how it behaves, never by file system name,
and nothing is repaired: an item that breaks a rule is refused with an `AdmissionRefusal` that names
it and the `RefusalReason`, which the TUI turns into a message saying how to fix it. The data root
is opened through its canonical path, so it and its ancestors may be symbolic links; it must belong
to the effective user and be writable by no one else, and a root Orchbee creates is set to 0700 and
read back. A root under the home folder also needs that folder to be the effective user's, so
`sudo` that keeps the invoking user's `HOME` is refused. `internal/` and every folder below it are
real folders of the effective user with no access for anyone else. A file system that does not keep
a mode Orchbee set, or refuses a file lock, is refused; before an existing folder is refused for
letting others in, Orchbee clears those bits on it, reads the mode back and restores it, so a file
system that ignores modes is refused as not keeping permissions rather than with `chmod` advice that
cannot take effect there. Linux checks no ACL, and macOS refuses an
extended ACL and a volume mounted to ignore ownership. `prepare_default_runtime_base` creates and
admits the root, `internal/` and `internal/runtime/`, and returns the runtime base through the
root's canonical path; `create_private_directory` does the same for a folder whose files a writer in
another crate places.

`agent_integration_may_have_been_enabled` answers `false` only when this environment's Capsule
folder does not exist, which proves that no provider entry names this environment; an unresolved root or unreadable folder answers `true`. It runs no provider command
and reads no provider configuration, so implicit callers use it to skip integration inspection
and to give an enabled integration's Session a head start over observed Agent startup.

Only the top level holds what a person reads, edits, or keeps. Every writer that creates a folder
inside the root also writes `internal/version.json` when it is absent, so a root that holds anything
Orchbee wrote records its layout. It writes the record under `internal/version.lock` as
`version.pending` and moves it into place without replacing one another writer placed, so processes
that start together on a fresh root, and every reader, find the record whole or absent, never empty. The record is `{"layout":1,"newestVersion":"<version>"}`, and
Desktop Main reads and writes the same shape; search for `data-root-version-record` before changing
it. Later releases only add entries, which every rewrite keeps. `DataRoot::admit_layout` reads only
this record and refuses a root whose `layout` is higher than `LAYOUT`, or whose record cannot be
read, and every process that opens a root calls it before any other read: the Client before it
dispatches any command that opens the root, and the Server and Worker before their first log
record. `DataRoot::carries_layout_record` answers whether a folder holds a readable record of any
layout, which uninstall requires before it removes anything from the folder. The first writer of
each process also refuses such a root before it creates anything, and
raises `newestVersion` to `PRODUCT_VERSION` under `internal/version.lock` when the record names an
older version, using Semantic Versioning precedence, so a refusal can name the newest version.
`compare_product_versions` exposes that one precedence; the release store's lower-version refusal
uses it too. Nothing migrates or removes what an earlier build
left in `%APPDATA%\Orchbee`, `%LOCALAPPDATA%\Orchbee` (its Session catalogs, checkpoints, TUI
positions and audit log), `internal/session-catalog/`, `internal/native-sessions/` and
`internal/tui-position/` of a development or test root, the `%TEMP%\orchbee-native-*` and
`%TEMP%\orchbee-session-*` runtime folders, the POSIX `/tmp/orchbee-native-<uid>` and
`/private/tmp/orchbee-native-<uid>` runtime folders, the `%TEMP%\orchbee-scrollback-*.txt` editor
documents, or the top level of a data root, nor the earlier plugin
store layout (`internal/plugins/config/p-<digest>`, `state/p-<digest>`, and
`managed/`); a registry written in that layout reads as invalid until it is deleted, after which
plugins are linked or installed again. `PluginStorePaths` places everything in the plugin store;
the store chooses the folder and version names.

## Persisted data versions

`record_migration` owns the version rules of the
[layout proposal](../../proposals/platform/data-root-layout.md#persisted-data-versions-and-migration)
for every record versioned by a top-level integer `version` (`VersionedRecord`). A store receives a
record's bytes only through one of two openers, each given the store's current strict parser:
`open_to_read`, for a process that does not write the record, migrates an earlier record in memory
and never writes; `open_to_write`, called under the store's writer lock with the store's own flushed
atomic replacement, persists the migration before the store sees the bytes. Both answer
`OpenedRecord`: `Absent`; `Readable` with the current or migrated bytes, or with the original of a
record without an integer version, which the store treats as its ordinary invalid record; or
`Unusable` with the reported `RecordNotice` for a newer record or one no step can update.
`VersionedFile` builds both on a private file whose lock and staging file sit beside it: `read`
takes no lock, and `acquire` returns the `VersionedWriter` with the opening.

Earlier versions are converted by pure N to N+1 steps in `VersionedRecord::steps`, and the result
must pass the store's parser. No step has shipped: until the first automatic-update release,
[Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries) applies and a format changes in place.
From that release a format change adds a step instead, and steps are never removed, so a record
that sat unopened for many releases still reads. Since no other path hands a store a record, a step
added there reaches every store, backup and persistence included, with no store change; the
module's tests drive every store through an injected step.

A stored sample of every released format keeps opening through today's opener. Each sample is a
test file named `<record>-v<version>` beside the tests of the store that opens it:
`src/tests/record_migration/record-samples/` holds the Session record, the restore notice and the
plugin registry, Workbench Client keeps the machine and TUI position records, and the Server's
checkpoint load tests keep a checkpoint slot with the kept terminal output it names, since the
Server parses both payloads. A format change adds its sample beside the earlier ones and removes
none; until the first automatic-update release seals them, a format changed in place replaces its
sample. The `layouts` folder has no sample, since nothing reads it.

A writer's opening copies the original into `internal/backups/<YYYY-MM-DD>-upgrade-<product
version>/` at its path relative to the root and flushes it before the replacement runs; every record
one product version migrates shares that folder, and creating another keeps only the three newest.
Desktop Main backs its window state record up into the same folders; search for
`upgrade-backup-folders` before changing the naming, the copies, or retention. A failed step, backup,
or replacement leaves the record byte-identical, and the store treats it as not updated: the notice
is reported, and the process keeps treating that record so until it ends, so its readers and writers
agree. The folder's date is the [local date](#local-clock), or the UTC date on a host without a
local clock.

After a [fallback](#fallback-after-a-failed-move), while the store's counting move carries
`restore` for the version this process runs from a store version folder, a writer's opening of a
record in a newer format restores it instead of reporting it: it looks for the unnumbered copy at
the record's relative path in an upgrade folder of each held version, the version held last first,
and takes the first copy that opens as current for this build. The writer's own replacement puts
that copy back, the writer uses it, and `record-restored` is logged with the record and the newer
version it replaced; the copy stays in place. Without such a copy, or when the replacement fails,
the record stays as a newer record does. The store is read only for a record in a newer format.
`session.json` is never restored, since two installed versions read it at once, and a reader
restores nothing: `VersionedFile::read` and `open_to_read` leave the record to its writer's next
opening.

| Record                            | Version and extension object | While its version is one this build cannot use                                                          |
| --------------------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------------- |
| `internal/plugins/registry.json`  | `version`; `extensions`      | Readers see no plugin; link, install, folder assignment and cleanup are refused with `UnusableRegistry` |
| `sessions/<session>/session.json` | `version`; `extensions`      | The catalog lists the Session as unrecognized with reason `Version`; it is neither started nor deleted  |

Each store keeps an `extensions` object it reads as opaque and writes back unchanged, so a later
release adds an optional field there without a migration. A record migrates only in a process that
writes it: the plugin registry when a writer changes it, and
`session.json` when it is published, which outside the Server happens only after proving no Server
of the Session runs. Workbench Client owns the machine and TUI position records, kept
through `VersionedFile`, with the same rules. `inspect_records` reads every versioned
record of a root, or one Session's, creating nothing, and reports the unusable ones; `config check`
lists them and the TUI shows the first when it starts, since the Server reads the records it holds.

`report_record_notice` logs a record a process cannot use, once per record and problem, and answers
whether this was the first time, so the process shows one of the two localized messages once;
`reported_record_notices` lists them for a surface that shows them, and `take_unshown_record_notices`
hands over only those no surface has shown yet: the CLI prints them on stderr after its command, and
`config check` takes the ones it lists. Applications own the message text and each record's
consequence.

## Private storage on macOS and Linux

The private update cache, persisted-data versions, the Session catalog, the durable and activation
records and the checkpoint store are one implementation on every host. They reach the file system
only through `private_filesystem` (opening, admission, `protect_new`, `replace` and `move_aside`)
and `data_root` (`create_state_directory` and `open_state_root`), whose Windows and Unix adapters
sit side by side, so the records, their shapes and their names are the same on every host. Each
Windows mechanism maps to a POSIX one:

| Guarantee                   | Windows                                                                                                                                                                          | macOS and Linux                                                                                                                                                                                                  |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| No one else reads or writes | The token user, SYSTEM or Administrators owns it; every allow entry names one of the three, and a new object's inherited DACL is replaced by a protected DACL granting only them | The effective user owns it; a new folder is set to 0700 and a new file to 0600 by descriptor and read back, or refused as `PermissionsNotKept`; an existing one with any group or other bit is refused           |
| No redirection              | Reparse points are refused                                                                                                                                                       | Every component is opened from `/` relative to its parent with `O_NOFOLLOW`, folders with `O_DIRECTORY`; a link below the data root is refused as `SymbolicLink`; the root is reached through its canonical path |
| No alias                    | A regular file with one link                                                                                                                                                     | A regular file with one link                                                                                                                                                                                     |
| Pinned ancestors            | Handles held without delete sharing, so no folder on the path can be renamed                                                                                                     | Descriptors held for the same lifetime; every folder below the root is private, so no other user can rename an entry in it, and the same user's writers take the lock                                            |
| Writer lock                 | `File::try_lock` on the record's lock file                                                                                                                                       | The same `File::try_lock`, an `flock`; a file system that refuses it is refused                                                                                                                                  |
| Replace a record            | Flush the staging file, then `MoveFileEx(REPLACE_EXISTING)`                                                                                                                      | Flush the staging file (`F_FULLFSYNC` on macOS), then `rename`, then flush the folder where the file system allows it                                                                                            |
| Move a file aside           | `MoveFileEx(WRITE_THROUGH)`, which fails when the target exists                                                                                                                  | `renameat2(RENAME_NOREPLACE)` on Linux and `renameatx_np(RENAME_EXCL)` on macOS; a file system without that flag takes a check and `rename` under the store's writer lock; then the folder is flushed            |
| Consistent reader           | A snapshot handle without delete sharing blocks a replacement while it is open                                                                                                   | A held descriptor keeps reading the version it opened, since no writer changes a published file in place; a reader never blocks a writer                                                                         |
| Other platform checks       | None                                                                                                                                                                             | macOS also refuses an extended ACL and a volume mounted to ignore ownership; Linux checks no ACL                                                                                                                 |

Windows admits SYSTEM and Administrators as owners beside the token user because an elevated
session of the same account owns what it creates through Administrators, and an elevated installer
creates the store root the same way; SYSTEM and every member of Administrators can take ownership
of any object and rewrite its DACL whatever it says, so an object they own is no more open than
one the user owns. Another account as owner, an allow entry for any other account (Users,
Authenticated Users and Everyone included), a missing DACL and an entry of another kind are
refused. A folder an Orchbee writer creates inherits the DACL of a parent that is not Orchbee's,
such as a home folder or `%LOCALAPPDATA%\Programs`, which on some profiles carries an inheritable
entry for another account. A new object holds nothing yet, so `protect_new` replaces that inherited
DACL with the protected one rather than judging it, and leaves the parent unchanged; it refuses only
a reparse point or another account as owner. `create_private_root` creates a root that way below
its existing parent, pinning the ancestors without judging them, as the data root below the home
and the release store root below `Programs`. An existing root gets the same DACL only when
admission refuses nothing but its DACL, one of the three owns it, it is no reparse point, and every
entry it holds is a real one the caller names: none for the data root, which a failed first start
can leave empty, and the Desktop installer's own `desktop`, `desktop.previous`, `unins000.exe` and
`unins000.dat` for the store root. Windows carries the new DACL down to what such a root holds.
Every other refusal stands, and no other existing object is repaired. A Windows refusal carries the
shared `AdmissionRefusal` once the caller that opened the object by path names it: `OtherOwner`,
`OthersCanRead` for a foreign allow entry, `OthersCanChange` for a missing DACL, `AccessControlList`
for an entry kind admission cannot judge, and `SymbolicLink` for any reparse point.

A writer stopped at any point leaves the published name holding a complete flushed version; only
its staging file can be partial, and the next writer reclaims it under the lock. Two records are
written in place on every host and fail closed instead: a `version.json` cut short makes the root's
layout unreadable, so the root is refused until the file is removed, and a cut-short activation
record reads as invalid and grants no restart. Neither host claims power-loss durability: a folder
flush is best effort.

Process registration is not ported. On macOS and Linux a durable record's Server, Worker and Host
registry stays `not-started`, no observation ever reports a role absent, and `spawn_server` and
`spawn_session_child` return `io::ErrorKind::Unsupported`. Recovering an Active Session there rests
on its free activation and lifetime locks and a free Host lock. A socket outlives its listener until
its owner removes it, unlike a Windows pipe, so socket files never count as a live process: the
Server removes the Session's stale ones under its lifetime lock before it binds, and its own once its
stop completes, and only a held Host lock refuses recovery, activation and deletion.

## User path operands

`UserPathOperand::parse` admits bounded drive-absolute, home-relative or current-directory-relative syntax without reading process context or probing the filesystem. The opaque operand preserves its spelling; `resolve` uses only the final execution owner's supplied environment or current directory. Absolute literals need neither capability. Invalid required home values never fall through to a lower-priority home source. `normalize_user_path` remains the combined entry point for callers that already own execution context. Applications own grammar completion, routing and the point at which resolution is permitted.

The portable grammar preserves Windows verbatim drive prefixes, including their implicit root. Resolution joins an ordinary relative operand to the supplied context before lexical reconstruction: a slash within an existing verbatim context component stays part of that component during parent traversal. Home suffixes append one component at a time. These rules preserve existing Windows admission and normalization on every host; they do not resolve or probe a Windows path on Unix.

`normalize_posix_user_path` is the POSIX grammar with the same signature and errors: an absolute path as written, `~` or `~/...` under a nonempty absolute `HOME`, or a path relative to the supplied current directory, resolved lexically like a shell's logical `cd`. `~name` is an ordinary relative name, and a drive or `\` spelling is `Input`. It runs on every host, reads no process context and never probes the filesystem. The settings reader's POSIX path keys, the macOS TUI's typed folders and the macOS CLI's Worktree path operands use it. `UserPathGrammar` names either grammar as data.

## Windows named runtime identity

The compiled artifact stem names this build's environment on every platform: `orchbee` for
release, otherwise `ob-dev-<digest>` or `ob-test-<digest>`. `build.rs` takes it from
`ORCHBEE_ARTIFACT_STEM`; a build that sets none, including every plain `cargo build` and
`cargo test`, is this checkout's test environment, `ob-test-<checkout digest>`. Only a build that
names `orchbee` is release and may act on the release installation, so release staging and
packaging pass it explicitly and release packaging refuses any other cohort. The checkout digest
is the one `createDesktopDevelopmentScope` in `apps/desktop` defines and native staging mirrors:
the first 16 hex digits of SHA-256 over `orchbee-development-data-root-v1\0` and the checkout's
canonical path, with a lowercase drive letter on Windows, so one checkout's development and test
stems share a digest. The build script hashes the path as the build host's file system reports it;
Node also applies NFC, which differs only for a path stored in decomposed form.

Cohort file names derive from the stem on every platform, `<stem>`, `<stem>-server`,
`<stem>-worker` and `<stem>-terminal-host`, with `.exe` on Windows, so Cargo's own `orchbee*`
outputs are never run as they are: tests and staging start a copy named by `CohortArtifact`.
Default staging, `pnpm desktop`, and Desktop packaging pass the environment's stem. macOS and Linux
socket folders carry it too, so release, development, and test Sessions of one user never meet.
`CohortArtifact::has_release_names` answers whether the cohort carries the release names, which
`is_release` decides.

`environment_stem` exposes the stem, and `environment_name` gives it as the handshake's
`EnvironmentName`, which every negotiated connection compares with its peer's.
`valid_environment_stem` checks another installation's stem against the same grammar, so a rule
about a remote executable name never restates it. `environment_kind` reads release, development or
test from the stem, and `checkout_name` gives the checkout's final directory name when the stem
carries that checkout's digest: a development build, or a build that named no stem. Release and the
packaged test build, whose digest names a home, have none. The TUI marks its environment from these
two. The [data root](#data-root) owner derives every Orchbee
state location from it, so applications never spell a state directory or choose a home variable
themselves. On every platform the data root is `<home>/.orchbee` for release and `<home>/.<stem>`
otherwise; the Windows locations are:

| Location             | Release                        | Development or test            |
| -------------------- | ------------------------------ | ------------------------------ |
| Data root            | `<home>\.orchbee`              | `<home>\.<stem>`               |
| Session folders      | `<data root>\sessions`         | `<data root>\sessions`         |
| Default runtime base | `<data root>\internal\runtime` | `<data root>\internal\runtime` |

Resolving a location touches no file. Storage owners take the data root, pin its ancestors and require the root itself to be private, creating it private on first write;
its parent may be a home directory whose ACL other principals share. `PrivateCache` applies the
same rule when it creates a missing root whose parent exists, in every environment; a root whose
parent is missing is created by the private recursive creation used for any other directory.
Activation creates the
default runtime base before admitting it, while an explicit `ORCHBEE_RUNTIME_BASE` must already
exist. Until then, read-only `session list`, `doctor` and `config check` report
nothing started.
Pipe names keep the `orchbee-` prefix in every environment because each runtime directory derives
its own endpoint id. `machine_hub_endpoint` names the machine hub's pipe the same way from the
data root's canonical path, with the role `hub`, and the
[machines guide](../../apps/orchbee/docs/machines.md#names-that-freeze) owns why. Data written
under earlier non-release locations is not migrated.

A managed terminal's `ORCHBEE_ENVIRONMENT` names the environment of its inherited
`ORCHBEE_RUNTIME_BASE` and `ORCHBEE_RUNTIME_SCOPE`. `session_route_environment` is the single
reader rule: when the tag names another environment, both route variables read as unset and the
Client uses its own defaults, without error. Without the tag they keep their explicit meaning, and
`ORCHBEE_SESSION` and `--session` are never filtered.

`SessionName` admits 1–64 ASCII letters, digits, `.`, `_` and `-` that neither start nor end with
`.` and are no Windows device name (`con`, `prn`, `aux`, `nul`, `com1`–`com9`, `lpt1`–`lpt9`) in any
letter case, with or without an extension, so one name is one folder on every file system. Names
compare without letter case: `Work` and `work` are one Session, which keeps the spelling it was
created with, and any spelling of `default` is `default`. On Windows a Session's runtime folder and
its Session folder both take its name, so `SessionScope` admits exactly the Session name grammar
there, and `SessionCatalog::spelling` gives the spelling of an existing folder. The endpoint id
lowercases the runtime path before hashing, so two spellings of one Session reach the same pipes.
Nothing in the runtime folder records the name; the catalog record is the Session's identity.

`SessionCatalog::unrecognized_records` reports, read-only, what refuses one Session: a catalog
record its admission rejects (`unreadable`), and each folder when two folders spell the name in
different letter case (`letter-case`), which only a case-sensitive directory can hold. POSIX and
Windows rendezvous refuse runtimes containing the obsolete `site-lifetime.lock`, never creating a
parallel `session-lifetime.lock`. There is no old-name alias, dual reader or automatic migration.

## Durable session catalog

Named and default Sessions keep one bounded private lifecycle record each,
`sessions/<session>/session.json`, with its writer lock `session.lock` and staging file
`session.pending` beside it and the Session's checkpoints in the same folder. Each record binds the
name, which matches its folder without letter case, and the runtime folder in use to Active,
Crashed, Stopped with a nonzero stop identity, or Deleting. Only current numeric version 1 with all
fields, including the Server/Worker/Host PID and creation-FILETIME registry, which macOS and Linux
leave unstarted (see [process registration](#private-storage-on-macos-and-linux)), is admitted. Other
versions or shapes return InvalidData; reads never migrate records. Effect paths derive from
admitted inputs, never serialized paths.

The configured runtime base must still exist; removing a stopped runtime leaf does not remove its
record or checkpoint. Reads list Session folders without creating or migrating files, and report a
Session's runtime as the folder its record names. Unsupported or malformed records are listed as
unreadable without repair. Linked or redirected files still fail admission. Two folders that spell
one name in different letter case list that name as conflicted, and activation and deletion refuse
it; a folder in `sessions/` whose name is no Session name fails enumeration. A folder that holds only
a record's lock or staging file, which an interrupted activation or deletion can leave, is no
Session, and deletion removes it.

The pinned-base namespace gate covers default and named activation. A missing runtime with a
validated stopped record, free checkpoint lock and absent endpoints can be recreated at the same
route. Stopped records may use any fixed slot or exactly named owned corrupt original. Active
recovery requires all exact registered identities gone, complete registration, free guards and absent
endpoints. Recovery publishes Crashed before reconciling its marker and beginning a fresh generation.
Live/unknown identity, incomplete registration, pending deletion and orphan storage refuse recovery.
macOS and Linux register no process identities, so there recovery requires the free activation and
lifetime guards and a free Host lock, which the Host holds for its whole life; socket files are
never evidence, and a Stopped record needs only its receipt until those platforms keep checkpoints.
A record that names another runtime folder, after `ORCHBEE_RUNTIME_BASE` changed, starts through the
new folder only once Stopped; Active and Crashed refuse until their own folder recovers them.
Activation atomically publishes the current Active record before marker, Server or storage effects.

Each activation carries a fresh nonzero 128-bit identity. The Server retains that exact generation before Worker effects and publishes Stopped after confirmed shutdown,
before the runtime stopped receipt and lifetime release. Failed publication never proves a clean stop.

Deletion publishes durable Deleting before any data removal. It removes the Session folder's data and
the runtime folder before removing the record last; interrupted cleanup remains discoverable even if
the runtime folder disappears. Publication uses bounded
private staging, flush and atomic replacement. This is ordinary process/failure recovery, not a
power-loss guarantee. A per-record writer lock serializes direct-child registration, recovery and
clean-stop publication. `spawn_server` requires the catalog and matching activation owner;
`spawn_server_outside_job`, on Windows, takes the same owners and creates the Server with
`CREATE_BREAKAWAY_FROM_JOB`, or inside the activator's Job when that Job refuses, and answers
which as `JobPlacement`; the remote link helper uses it so a Session outlives the `ssh`
connection's Job. `spawn_session_child` requires the exact registered direct parent for Worker or
Host. Each
child is created suspended and released only after its identity is flushed. A failed start's I/O
error keeps the failed operation's kind and operating-system code and names its step,
`SessionChildStep::of`: registration, creation or release. Explicit confirmed
named-session deletion can replace an unreadable record with its deletion fence only after the
existing owner, endpoint and storage guards pass; ordinary publication cannot overwrite it.
Live Host adoption remains separate.

An instant's hold never fails a Session's start, stop, deletion or read. A reader of the catalog
(`SessionCatalog::list`, `session list`, `doctor`, `server stop` waiting for the release,
the machine hub, a Client reading its restore notice) holds a record, a folder or a Session lock
only for one read or probe, and a writer holds a record only while it moves it into place. On
Windows such a hold refuses the other side: a file another handle holds open cannot be replaced,
even when that handle shares delete access; a file or folder held or pinned without delete sharing
cannot be removed; and a record being moved into place cannot be opened by a reader that shares no
delete access. A lock probe on either platform makes a taker's `try_lock` contend. So every atomic
replacement, every removal in deletion, every read of a Session or layout record, the namespace
lock's owners, deletion's activation and lifetime takes, and the Server's and the activator's
lifetime takes (`acquire_past_probes`) try again every 5 ms for at most half a second
(`MOMENTARY_HOLD`) before the refusal stands. A hold lasts well under a millisecond, tens of
milliseconds on a loaded machine, so the bound outlasts it many times over while an owner's hold is
still refused promptly, inside the 5 s deletion, 20 s start and 60 s stop deadlines; an operation
nobody holds never waits. POSIX opens, renames and removals ignore open descriptors, so only the
lock takes retry there.

The [Windows crash-recovery contract](docs/windows-crash-recovery.md) owns guards, interrupted
publication, compatibility and evidence limits. Check the registered bounded model with:

```sh
node scripts/tla/run-models.mjs --models session-crash-recovery
```

## Named session lifecycle

`SessionCatalog` serializes default/named activation and named deletion with one private per-user
namespace lock, `internal/sessions.lock`. Activation holds it from before runtime creation through
readiness. Read-only enumeration creates nothing, includes default, sorts names without letter case
and rejects unsafe or corrupt entries instead of hiding them. Inventory is limited to 256 sessions.

The cleanup owner checks the original deadline and its caller's interruption predicate between filesystem phases. Interruption reports whether a published intent remains; completed removals are not rolled back.

Deletion refuses default, live or uncertain processes, unsafe files and unknown directory content,
and a name two folders spell. Any spelling of a name deletes the Session created with it. An
admitted stopped receipt (or never-activated runtime), free lifetime, checkpoint and TUI position
locks, and absent endpoints are required before publishing the durable Deleting record, which fences
activation and survives partial cleanup. The owner removes only known Session and runtime files,
including `terminal-output/` entries, a `layouts/` folder an earlier build kept, and the Terminal
Host's `host-fatal.json` and `host-diagnostics.jsonl` records in the `host` folder, releases their
exact handles before removal, retains ancestor pins and removes the record and the emptied Session
folder last. Retrying resumes that deletion; absent state succeeds. It never recursively deletes
arbitrary trees or stops a process. The shared namespace lock intentionally outlives individual
sessions. This covers cooperating current-user invocations and ordinary process interruption;
power-loss durability and inventory recovery after external removal of the runtime base are
unclaimed.

## Session checkpoint store

`SessionStore` retains bounded checkpoint bytes in the Session folder,
`<data root>/sessions/<session>`, named by the admitted Session's runtime folder. Its caller supplies
the data root and admitted Session; the Server owns checkpoint validation and recovery. Each Session
has its own store. Private ancestor handles, private new folders, no-link admission and single-link
file checks apply on every host. A private `state.lock`
is held exclusively for the store lifetime; a competing owner receives `WouldBlock`.

The store retains three versioned, integrity-checked slots, ordered by their private generation.
The Server supplies whole-document validation; the newest valid payload wins. Invalid copies remain
intact and produce an explicit recovered/empty startup notice. Unframed files and unsupported
payloads use this same recovery policy. Missing data and invalid data remain distinct;
filesystem/private-path admission errors still refuse startup. Current durable and checkpoint
formats each use version 1 and reject preceding numeric versions without migration. See the
[Windows checkpoint storage contract](docs/windows-checkpoint-storage.md).

Publication writes and flushes one private `checkpoint.pending`, preserves any displaced invalid
original as `checkpoint.corrupt-<UTC second>.raw`, with `-2`, `-3` and on when that name is taken, then atomically replaces a slot in the same directory
(`MoveFileEx(REPLACE_EXISTING)` on Windows, `rename` elsewhere; see the
[mapping](#private-storage-on-macos-and-linux)). Pending bytes never supply recovery authority; failure keeps
prior valid slots and reports any unconfirmed staging cleanup. Unchanged payloads do not consume
history. Raw originals are user recovery data retained until explicit Session deletion or authorized
repair. Other users are excluded by the private ACL or mode; same-user writers respect the lock. The
adapter does not claim directory-entry write-through; the acknowledged pending-file flush and
atomic replacement do not prove power-loss or hardware durability.

The store does not interpret JSON, select a default data directory, relaunch a process, replay an
operation or grant Session restart authority. The Server owns startup/stop use of this primitive;
native process-crash/restart and machine-restart witnesses remain separate evidence obligations.

## Local clock

`local_clock` converts an instant to the local date and minute for display, with the system time
zone's rules in force at that instant; daily log and backup folder names and displayed times use
it. Windows asks the system. macOS and Linux parse the TZif file `/etc/localtime` links to, with
its POSIX `TZ` footer rule past the last transition, and keep the parsed zone until that file's
identity changes; like Windows they ignore `TZ`. A missing or malformed zone file offers no local
clock rather than a guessed offset, and leap-second records are skipped. The parser's evidence is
its own tests against published zone rules; reading the system zone needs a native run.

## Diagnostic log

`install_diagnostic_log(role)` is called only by an executable entry point, the Client, Server and
Worker, and `log_event` writes nothing until it has been, so library code that tests exercise never
logs. A record is one JSON line in `logs/<YYYY-MM-DD>.jsonl`, named by local date:
`{"time","level","role","pid","event","fields"}`, with UTC time to the millisecond and at most
eight camelCase fields. Event names and text values are `&'static str` tokens of letters, digits,
`.`, `_` and `-`, at most 64 bytes, and other values are counts and flags; a token outside the
grammar is written as `invalid`. A record therefore cannot carry terminal content, typed input,
prompts, arguments, environment or configuration values, file contents or credentials. Host
diagnostics reach the log only through the Worker's records: its Terminal Host start and failure,
the Host anomalies it contains, and the wait that ended it. Event names
and record fields are unstable: a release may add, rename or drop them without migration, so read
the log as diagnostics, not as an interface.

A writer holds the day's file against every other writer while it appends, waiting up to 500 ms
for another writer, so Desktop Main's exclusive open and the natives admit one record at a time.
Windows holds the file without write sharing. macOS and Linux open the private regular file without
following a link, hold its exclusive `flock` lock and append through `O_APPEND`, so a writer that
skips the lock never overwrites a record; a
reader takes no lock and drops a record still being written as an incomplete line. The
first append of a process on a new day removes daily files dated more than six days earlier; the
destructive request log, its generation, and any other name are never daily files. A record is
admitted while the file plus the record plus a 256-byte reserve stays within
`DAILY_LOG_LIMIT_BYTES`, 7 MiB; the first record refused closes the day with one `log-capped`
record, so the folder holds at most 51 MiB with the destructive log. Every error is a non-event.
Desktop Main writes the same records with role `desktop-main` on Windows and macOS, where its open
takes the same `flock` lock through `O_EXLOCK`; search for `daily-diagnostic-log` before changing
the shape, a limit, or the names. Linux processes write the same files; only
Windows and macOS runs prove them.

## Diagnostic bundle redaction

`recent_diagnostic_logs` reads today's file and the six before it without creating anything and
keeps only lines that are exactly records this grammar writes, counting the rest, so a bundle never
carries text a writer did not produce. `redact_settings` writes `config.toml` again from its values,
so its comments are not carried, and replaces a value under a command, executable or program key with `<command>`, every argument under
an args or arguments key with `<argument>`, every value under an env, environment or
environmentVariables key with `<environment>`, every value under a key that names a token, secret,
password, API key or credential with `<credential>`, and a settings value that no documented option
covers with `<unrecognized>`. `DiagnosticRedactor` then scrubs every remaining string and object key:
the home folder becomes `<home>`, the value of an `ORCHBEE_*`, `APPDATA`, `LOCALAPPDATA`, `TEMP` or
`TMP` variable becomes `<env:NAME>`, URL user information becomes `<credential>`, and a URL query
becomes `?<query>`. `diagnostic_archive` writes the entries as one stored ZIP archive. The
[CLI command](../../apps/orchbee/README.md#diagnostic-bundle) chooses what a bundle
holds.

## Destructive request log

`append_destructive_request` appends one caller-supplied JSON record as one line to
`logs/destructive-commands.jsonl` in the data root. It creates the root, the private `logs`
directory and the file on first use under the state root rules above, and admits existing ones only
when they are private regular files and directories. Concurrent appends serialize on the file's own
lock and give up after 500 ms. The current file never exceeds `DESTRUCTIVE_LOG_ROTATION_BYTES`,
1 MiB: an append that would pass it first copies the current lines over
`destructive-commands.1.jsonl`,
replacing the generation kept there, and empties the current file in place, so at most two
generations remain. Nothing reads the log back. Callers treat every error as a non-event, so a
failed append never blocks or changes their command. The
[CLI release confirmation](../../apps/orchbee/README.md#release-destructive-confirmation)
owns the record fields and which platforms write them.

## Local startup model

Bare Linux and macOS `orchbee` admit their inherited input/output TTY and validate geometry before
runtime creation or process launch. The Session is the inherited `ORCHBEE_RUNTIME_SCOPE` when this
environment's managed terminal routes one, otherwise `default`. The runtime base is the routed
`ORCHBEE_RUNTIME_BASE`, which must already exist, or `<data root>/internal/runtime`, which
activation creates and admits by the [data root](#data-root) rules and names through the data
root's canonical path. Runtime directories must be owned by the effective user with mode 0700; lock files
are regular, single-link, owned files with mode 0600. Existing invalid paths are rejected, never
repaired, followed through symlinks, unlinked, or replaced.

No file system is admitted or refused by type. A folder Orchbee creates is set with `fchmod` and
read back, and a mode that does not stay refuses the file system; an existing data folder whose mode
lets others in is probed the same way, with its original mode restored, before its bits are judged;
a lock it refuses with `ENOLCK`,
`ENOTSUP` or `EOPNOTSUPP` refuses it too. Linux checks no POSIX ACL. macOS rejects every extended
ACL through descriptor-anchored inspection and every volume mounted to ignore ownership, and strips
nothing. Other Unix targets are unclaimed.
First-creation concurrency assumes an umask that preserves owner bits (for example 0022 or 0077);
a user-bit-restrictive umask may expose an incomplete directory and fails closed.

The Client owns one absolute 20-second startup deadline, from TTY admission through the first
validated Host frame applied to its projection. Connect, OBLC, initial OBCS requests, readiness
polling, and first-frame waiting consume this same budget. A delayed localized progress notice
appears after 500 ms; normal fast startup is silent. This is a cooperative deadline, excluding
kernel/filesystem stalls and process suspension. Timeout closes only the Client connection.

| State / observation                                                        | Action and next state                                                                        |
| -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| Admitted compatible Client socket                                          | Reuse that exact connection; no process launch                                               |
| Definite absent/refused socket, activation lock contended                  | Wait and retry connection until the original deadline                                        |
| Activation lock acquired                                                   | Recheck socket and lifetime before any launch                                                |
| No marker, lifetime free                                                   | Client exclusively creates `activation.started`, then spawns Server once                     |
| Marker present, lifetime held                                              | Wait for compatible Server + Worker readiness                                                |
| Marker present, lifetime and Host lock free                                | Catalog records Crashed, then a fresh generation commits and spawns Server once              |
| Marker present, lifetime free, Host lock held past 3 s                     | RecoveryRequired; never launch                                                               |
| No marker, lifetime held                                                   | A Server is completing its stop; wait, then start again once it releases the lifetime lock   |
| Socket refuses with `stopping`                                             | A stop is ending that Server; drop the activation lock, then start over once it ends         |
| Invalid socket / cohort / permissions, incompatible or ambiguous admission | Explicit failure; no fallback                                                                |
| Launch failure after marker creation                                       | Retain marker; this attempt reports RecoveryRequired and a later one recovers it as above    |
| Server + Worker ready                                                      | Release activation lock, read Workspace membership, attach an existing binding when selected |
| First validated frame applied before deadline                              | Enter TUI with the same inherited TTY and prepared Session                                   |
| Client exit / deadline                                                     | Release Client resources; retain Server, Worker, Host, and terminal                          |
| Worker / Host loss                                                         | Unavailable or unknown outcome; never respawn, adopt, or replay                              |
| Server `SIGTERM` / `SIGINT`, Worker exited and Host gone                   | Server's normal stop; Stopped receipt, socket files removed, lifetime released               |

`activation.started` is an immutable, empty, regular, single-link, effective-user-owned 0600
file. The Client is its sole writer, using descriptor-relative O_EXCL, CLOEXEC, and NOFOLLOW.
It is a launch-attempt commit, not a PID or success report. The activation guard spans that commit,
spawn, and OBLC readiness. The Server alone holds the lifetime lock, binds the Worker listener,
starts exactly one cohort Worker, and publishes the Client listener only after Worker registration.
The Client listener is bound under a private pending name, chmodded, then published with an atomic
no-replace rename; clients never observe its pre-permission state. Worker spawning follows final
Worker socket permissions. The Worker waits for its newly spawned Host socket to reach exact 0600
before the sole handshake attempt; a pre-existing endpoint still fails immediately.
The Server starts a new session with `setsid`; Client exit does not signal or kill its children.
Child owners reap exits without killing retained services; no reaper grants restart authority.
Every runtime child starts in a neutral working directory, the system directory on Windows and `/`
on Unix, never in the activating Client's. A Session outlives that Client, and Windows keeps a
process's working directory in use, so an inherited one would block removing a Worktree the person
started Orchbee in. On macOS and Linux the Client starts the Server through `spawn_session_server`,
which appends a release or test cohort's folder to the Server's `PATH` when an inherited, non-empty
`PATH` names no such entry, so the Worker, Host and terminals inherit it and a terminal finds that
cohort's Client by name even when no shell profile names `~/.local/bin`. The folder goes last, so
an earlier `orchbee` still wins; a development cohort's build folder is never added. On macOS
every runtime child inherits no descriptor beyond its standard streams (see
[macOS retained spawn](#macos-retained-spawn)), so a Worker's death closes the Server's last end of
its connection even while a terminal it started lives on.

The Worker registers without starting a Host. Its authorization kernel admits the first Launch
and prepares launch facts before spawning one exact cohort Host, which keeps its lock in a private
`host` directory and binds its endpoint in the socket folder.
Invalid authorization produces the existing typed result without a Host. The admitted dispatch
and kernel survive Host startup; the Host handoff revalidates authorization. The absolute Host
endpoint address is checked against the platform socket-address ABI before the `host` directory
or the Host process exists; an address that cannot fit settles the same pre-handoff failure
without either side effect. Preparation or startup failure settles a pre-handoff failure when
deliverable and retires the Worker. Any existing Host endpoint is an explicit recovery failure.
No Host reconnect/adoption is allowed.

## Development cohort contract

`cohort.json` is bounded to 16 KiB, strict JSON with `schemaVersion: 1` and exactly five
`artifacts` fields, in this canonical order: `orchbee`, `orchbee-server`,
`orchbee-worker`, `orchbee-terminal-host`, `agent-observation-adapters`. The first four values are
the lowercase 64-character SHA-256 of each complete executable. The fifth declares the lowercase
SHA-256 payload digest of the observation-adapters capsule staged beside the executables — the same
value its `capsule.json` carries as `payloadSha256`. A manifest carrying any other `schemaVersion`,
is incompatible; a `schemaVersion: 1` manifest missing a required member, carrying
duplicated or unknown members, or declaring a malformed digest is an invalid manifest. Both fail
closed through the existing typed rejection. Members name the roles; the staged files carry the
environment stem (`<stem>`, `<stem>-server`, `<stem>-worker`, `<stem>-terminal-host`). The
directory and files belong to the effective user or to root and are not writable by group or
other, so a cohort installed by root is admitted as well as a staged one. Files are regular and
single-link with no setuid, setgid or sticky bit; executables have an owner execute bit. macOS
also requires no extended ACL on the directory and each opened file, on a volume that keeps
ownership. Manifest and executables are opened relative to the verified
directory with NOFOLLOW and CLOEXEC. Each managed Rust role (Client, Server, Worker) validates all
four executables, including on warm startup. A role finds its cohort beside `current_executable`,
its own program file; macOS reports the path a program was started through, such as the command
link `~/.local/bin/orchbee`, so there the path is resolved first.
Host consumes the verified sibling executable; it does not implement the manifest reader.

The root development and test profiles compile the existing `sha2` dependency at optimization
level 3 so large debug executables can be hashed efficiently. Application code retains its normal
debug profile. Cohort validation still consumes the caller's original deadline.

The `agent-observation-adapters` sibling directory is a lazily admitted sidecar. `Cohort`
never opens, traverses, or validates the payload during startup; it exposes only the fixed sidecar
directory and the manifest-declared digest through `observation_adapters_sidecar()`. The first
integration consumer validates that directory privately and answers a typed unavailable or invalid
result, so an absent or damaged sidecar never blocks Session startup or Agent reads.

The exact cohort identity is lowercase SHA-256 of UTF-8 `orchbee-native-cohort-v1\n` followed,
in the above fixed order, by each staged executable name, one NUL byte, its hexadecimal hash, and LF —
the four executables only; the adapters declaration is deliberately excluded so a payload bump does
not churn admission identity.
JSON key order is immaterial. Sibling paths come only from `CohortArtifact` names. Missing, changed,
mixed, malformed, or unsafe artifacts fail admission before activation. Explicit endpoint test
modes retain their existing version cohort and do not acquire managed startup authority.

The staging script builds all four executables and copies them under the environment stem's names
into a new private directory, copies the
canonical `crates/agent-integration-writer/agent-observation-adapters` capsule verbatim to the
sidecar directory, verifies its strict `capsule.json` members (`contractVersion` 1, a declared
`payloadSha256`, the required payload entries, and a recomputed canonical digest equal to the
declared one), then writes the manifest last. A capsule that fails that verification stops staging
rather than emitting a bad package. This is unsigned, per-user development staging: release signing,
installers,
same-user malicious mutation, and verification-to-exec races are excluded. The manifest establishes
a coherent exact artifact set, not publisher authenticity.

## Correctness case

The LS claims below describe the Linux and macOS development startup boundary. Managed Windows cold recovery
has the separate [generation and process registration contract](docs/windows-crash-recovery.md).

| Claim                                                                                      | Owner / refinement                                                                       | Evidence                                                                   |
| ------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| LS01 at most one launch attempt per activation generation                                  | Exclusive activation marker; Client under activation guard                               | LocalSessionStartup model; contention and failed-marker process traces     |
| LS02 no recovery while a generation's Server or Host may live; no Client-owned termination | Lifetime guard, Host lock, catalog recovery under both guards, dropped Client connection | Model losses/timeouts; injected-lock recovery tests; signal process traces |
| LS03 no Host before authorized Launch; one retained dispatch                               | Worker authorization kernel and lazy startup adapter                                     | Model authorization; Worker invalid/valid Launch process tests             |
| LS04 coherent roles and private rendezvous before effects                                  | Cohort and ModeCheckedLocalSessionRendezvous                                             | Manifest corruption, mode, symlink, and existing lock tests                |
| LS05 startup readiness includes a valid first frame within one budget                      | Client deadline, TerminalSession projection, same TTY handoff                            | Admission/submission deadline tests and bare TTY process tests             |

`formal/tla/LocalSessionStartup.tla` bounds one activation generation: two Clients, one Server
launch attempt, and one Host attempt. Its `Recovery` is an attempt that launches nothing in that
generation; the [catalog](#durable-session-catalog) closes a generation that ended without its stop
and the next one starts as a fresh instance. It abstracts filesystem checks as admitted facts,
abstracts deadline observation as an expiry event, and checks safety without scheduler fairness.
Per-Client timeout fairness supports bounded-attempt termination only, not service availability.
Real kernel locks, peers, hashing, PTYs, signals and cancellation are exercised by Linux and macOS
tests. A Server whose activating Client ended before that Server took the lifetime lock looks lost
until it takes it; a recovery in that window lets the two generations' Servers race for the lock,
which keeps one owner, and the recovering attempt then reports RecoveryRequired. Remote, Workbench
membership, daemon restart, persistence across Server loss, and other Unix targets are outside this
contract.

## POSIX local Session rendezvous foundation

The crate-private `rendezvous` module exposes mode-checked Unix rendezvous primitives to the
Client, Server, and Worker. `SessionScope` is the Session name, under the one `SessionName` grammar
every platform uses; cohort version and digest do not belong in it, so an update keeps one Session
identity. The environment's stem keeps release, development, and test Sessions apart.

For one trusted, existing, exactly normalized absolute per-user runtime base, the derived paths are:

| Role                       | Path                                                                 |
| -------------------------- | -------------------------------------------------------------------- |
| Scope directory            | `<runtime-base>/<session>`                                           |
| Activation lock            | `<runtime-base>/<session>/activation.lock`                           |
| Session lifetime lock      | `<runtime-base>/<session>/session-lifetime.lock`                     |
| Host directory             | `<runtime-base>/<session>/host`, which holds the Host's lock         |
| Socket folder              | `/tmp/<stem>-<euid>` on Linux, `/private/tmp/<stem>-<euid>` on macOS |
| Client socket              | `<socket-folder>/<id>-client.sock`                                   |
| Client pending name        | `<socket-folder>/<id>-pending.sock`                                  |
| Worker socket              | `<socket-folder>/<id>-worker.sock`                                   |
| Host endpoint              | `<socket-folder>/<id>-host.sock`                                     |
| Agent observation endpoint | `<socket-folder>/<id>-agent.sock`                                    |

`<id>` is the first 32 hex digits of SHA-256 over `orchbee-posix-runtime-v1`, a NUL byte, the
runtime base as activation names it, `/`, and the lowercase Session name. Every socket name thus
has one length whatever the home folder, base, or Session name; the longest, with a 10-digit user
ID, is 94 bytes, inside the 103 that macOS allows. The pending name is the transient pre-publish
bind the Server renames onto the Client socket; the Host receives `<id>` and the socket folder as
`--endpoint-id` and `--endpoint-directory`. The derived paths are pure joins that answer even when
the folders cannot be inspected.

The socket folder's parent must belong to root and carry the sticky bit. Preparation creates the
socket folder as 0700 when it is missing; every open requires a real folder of the effective user
with exact mode `0700`, and refuses anything else as `UnsafeSocketFolder` without repair. A
validation finds no folder before an activation has created one. The Server holds a shared lock on
the folder while the Session lives, so a temporary-file cleaner that honours locks leaves it alone.
Only `remove_endpoints`, below, removes a socket file.

`ModeCheckedLocalSessionRendezvous` retains its mode-checked name for other Unix targets. Preparation
creates only the scope directory and socket folder when absent. Preparation and validation open `/`
as the trusted root, then open every runtime-base component and the controlled scope
descriptor-relatively with `O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC`. They neither canonicalize nor ask
the operating system to normalize the supplied base. Apart from `/` itself, an accepted base has
exactly one leading slash, one or more nonempty normal components separated by single slashes, no
trailing slash, no `.` or `..` component, and no NUL byte. A malformed base is rejected before
filesystem access beyond the trusted root; a symbolic-link or non-directory component is rejected at
that component. The retained scope must be a directory owned by the effective UID with exact mode
`0700`. A new scope is inspected without following its leaf, checked for directory type and
effective-UID ownership, changed descriptor-relatively to `0700` before a permissions-dependent
open, and then admitted only when the opened descriptor has the same device and inode and the mode
reads back as set. This reaches exact `0700` even when the process umask removes every requested
permission, without changing the process umask. An existing wrong mode is rejected without repair.

macOS also refuses an extended ACL or a volume that ignores ownership on its runtime base, scope,
lock files, marker, cohort directory, and opened cohort artifacts.

Each lock operation opens or creates only its fixed leaf through the retained directory descriptor
with `O_RDWR | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC`; a file system that refuses the lock is
refused. The leaf must be a regular file owned by the effective UID with exact mode `0600` and
exactly one hard link. An existing valid stale file is reused without truncation or replacement.
`try_acquire_activation_lock` and `try_acquire_session_lifetime_lock` each perform one nonblocking
exclusive kernel lock attempt and return either an owned guard or `Contended`; they do not wait,
retry, or infer process health. File creation and advisory-lock acquisition are separate: a caller
returning `Contended` may have created and normalized a previously missing durable lock inode before
another descriptor won the lock. A contended attempt never truncates, replaces, unlinks, or changes
the ownership of an existing inode, and never repairs a pre-existing invalid entry. Dropping a guard
releases only its advisory lock. The runtime directory and durable lock files are never removed by
this module.

`observe_*` entry probes expose the same finite names to read-only diagnostics: a
descriptor-relative `fstatat` with `AT_SYMLINK_NOFOLLOW` answers each entry's raw kind,
ownership, mode, size, and link count — the caller owns every verdict. Socket probes look in the
admitted socket folder and answer `Absent` when none exists. `observe_session_lifetime_lock` never
creates the leaf: only an entry already stat-checked as an owned regular `0600` single-link file is
opened, probed with one `try_lock`, and released on the same descriptor — `Held` is a lock fact at
that instant, never a process claim. No observation connects, creates, writes, unlinks, repairs, or
infers process health.

On Windows `observe_session_holder` reads the lifetime lock only while it holds the folder's
existing activation lock, which a Server's activation and its stop hold around the lifetime
acquire and release, so a reader that repeats never takes a free lifetime lock at the instant a
Server takes it. A missing activation lock is `NoActivation`, a held one `ActivationHeld`; neither
file is created. The machine hub reads Sessions this way.

Socket metadata and same-user peer proof are enforced by the managed consumers. Client, Server, and
Worker share this crate, and Server owns the lifetime guard. `remove_endpoints` takes that held guard
and unlinks only a socket owned by the effective user at the Session's five derived names, inspected
with `fstatat` without following links; another entry kind stays, and the Host endpoint stays while
another descriptor holds the Host's lock. The primitive does not itself select application startup
policy, retry, or grant recovery authority.

### Modeling and evidence

| Field        | Rendezvous primitive contract                                                                                                                                                                                                                                                                                                                         |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Provide shared path, permission, lock and attempt-marker admission; applications own launch, readiness and recovery policy.                                                                                                                                                                                                                           |
| Inputs       | A trusted existing exactly normalized absolute base, canonical scope, effective UID, and potentially invalid directory, lock, marker or socket entries.                                                                                                                                                                                               |
| Data         | Stable paths, retained scope descriptor, owned advisory-lock guards, and the bounded activation record in the attempt marker. No PID or process-health inference.                                                                                                                                                                                     |
| Outputs      | Validated rendezvous or typed failure; Acquired/Contended lock outcome; marker and socket existence with metadata validation. Path getters perform no I/O.                                                                                                                                                                                            |
| Invariants   | Descriptor-relative no-follow traversal; exact final type, owner, mode and regular-file link count; new directory identity matches its inspected inode and its mode reads back. macOS final objects have no extended ACL on a volume that keeps ownership. Existing invalid entries are never repaired or removed. Guard drop only releases its lock. |
| Decisions    | Normalized path and scope syntax, create-versus-existing metadata, lock kind, one nonblocking lock outcome, and exclusive marker creation determine results.                                                                                                                                                                                          |
| Model        | Atomic POSIX primitives without application retry or process observation. LocalSessionStartup composes these facts with Client/Server/Worker state; Client implementation tests exercise the connect-first behavior.                                                                                                                                  |
| Transitions  | Missing scope/lock becomes a validated durable inode. Lock creation and lock acquisition may have different winners. Guard acquisition/drop changes only lock ownership. An absent marker is committed as Active; only a Stopped or Crashed receipt is cleared back to Active.                                                                        |
| Dependencies | Safe nix filesystem wrappers and File::try_lock; macOS mount-flag and extended ACL inspection. No child-process or service dependency in rendezvous.                                                                                                                                                                                                  |
| Ownership    | This crate owns derivation and admission. Client chooses the development base/scope and owns the activation guard and marker commit; Server owns the lifetime guard.                                                                                                                                                                                  |
| Verification | Scope/path partitions, symlink/hardlink/FIFO rejection, wrong owner/mode, restrictive-umask initialization, concurrent owner-preserving creation, stale inode preservation, contention/reacquisition, marker exclusivity, socket impostors, and the longest socket name across every legal stem and a 10-digit user ID.                               |
| Limits       | One effective user; wrong-owner tests inject the expected UID. Kernel metadata and advisory locks are trusted. Concurrent creation requires owner-preserving umask. Durable means no unlink/truncate, not fsync. Cooperative deadlines exclude kernel stalls and process suspension. The model does not prove filesystem or kernel semantics.         |

## Windows refinement

`RetainedChild::terminate_before` explicitly terminates only the exact held process and confirms
its exit before a cooperative deadline. It does not terminate descendants. Managed startup uses
it only before the Worker can create a Host; later cleanup belongs to the Worker shutdown protocol.
Dropping `RetainedChild` continues to release observation ownership without terminating anything.
`RetainedChild::id` reports the held process identifier, which the Server lists for its Worker in
the handshake's `query` reply.

The native Windows adapter retains activation/lifetime-lock ownership; the explicit
stop record below refines the original immutable attempt marker. It admits an absolute local path only after
rejecting reparse points and retaining directory handles without delete sharing.
The runtime parent and leaf must be owned by the token user, SYSTEM or Administrators, with a
DACL granting access only to those three, as the private storage table above states. Newly created leaves receive
a protected inheritable DACL in place of the one they inherited; an existing parent or leaf that
breaks the rule is rejected, never repaired.
Named-pipe addresses are derived from the canonical runtime path; the default scope is keyed by the token SID.
Host identities use the same path digest so different Sessions cannot collide. Connection
admission still proves the peer's token user. Pipe existence is advisory and never
proves readiness or process health. Windows executables use exact `.exe` siblings
and the existing cohort manifest digest rules. Child processes use detached,
native creation flags with handle inheritance disabled and remain owned by the Session after Client detach.

The safe `windows-permissions` dependency owns descriptor/ACL queries that `std`
cannot express. `winapi-util` supplies handle-based hard-link counts; `winsafe` supplies
non-inheriting process creation and exact wait/exit observation. Its WinAPI FFI and legacy bitflags dependency remain outside the
authored unsafe-code prohibition; no async runtime or shell-based security probe
is introduced. Native tests cover ACL rejection, hard-link and junction refusal, marker
exclusivity, lock contention, distinct endpoints, and cohort tampering. This is a
self-reviewed refinement, not an independent review: the challenge cases include
existing broad ACLs, path replacement, duplicate starts, and pipe impersonation.

Windows read-only diagnostics reuse admission without creating a scope or file.
The marker must be regular, single-link, privately owned and contain a valid activation record. The
activation lock is inspected without locking. The lifetime observation opens only
an existing admitted file, tries one nonblocking exclusive lock, and immediately
releases it: absent/free/held/unsafe are observations, not service health. A probe
contends with startup only for its instant, which the lifetime's takers outlast (see the
[durable session catalog](#durable-session-catalog)); failure never authorizes repair or deletion.
Invalid parents prevent descendant inspection. Tests cover zero creation,
free/held transitions, malformed markers and hard-link rejection.

### Windows explicit-stop activation record

The Windows stop refinement replaces the immutable-attempt-only interpretation
with a bounded record in the same admitted `activation.started` file. Empty bytes
mean Active; the exact ASCII prefix `orchbee-stopped-v1:` followed by a nonzero
32-character lowercase hexadecimal stop request ID means Stopped. Absence remains
distinct. Every other size, prefix or encoding is invalid and grants no restart.
macOS and Linux read and write the same record in the same file, with the same commit, completion
and consumption rules; there a retained descriptor pins the file, and the private runtime folder
keeps other users from renaming it.

The managed Server retains an existing Active file handle without delete sharing.
Only its lifecycle owner may complete that record, after exact terminal containment,
Host exit and Worker exit have been confirmed. Completion seeks, writes the bounded
record, fixes its length and flushes data. Partial I/O is not success. The retained
owner can retry only the same attempted ID; a different ID is refused. An exact
already-completed ID is idempotent. Filesystem stalls are outside cooperative time
bounds; crash-torn or malformed records fail closed. No pathname is deleted or
replaced and no receipt proves that an unrelated process exited.

Under the existing activation/lifetime guards, startup may consume only a valid
Stopped record by truncating that same admitted file to Active before the single
Server spawn. Failure after this boundary still requires recovery. A subsequent
start consumes the previous stop receipt, so its absence never disproves an older
stop. Older cohorts reject Stopped bytes and must be upgraded before restart.

The decision partitions are absent/Active/Stopped/invalid, same/different stop ID,
first completion/retry, and start before/after completion. Runtime tests establish
strict parsing, zero versus malformed data, completion identity and restart cycles;
managed lifecycle and process witnesses must establish the exit precondition before
the public stop command can claim completion. This record primitive itself does not
stop processes, acquire lifecycle authority, or perform automatic recovery.

## Directory observations and Windows directory primitives

`reported_directory` decodes a terminal-reported directory, an OSC 7 report or a sampled process directory, by this machine's rules, since a Server and its Terminal Host share a machine; every Server, Client and CLI reader of such a report uses it. On Windows it is the shared pure `windows_reported_directory` decoder, which preserves valid terminal-reported drive/UNC observations. On POSIX it is the pure `posix_reported_directory` decoder: an absolute path stays as spelled, and a `file://` URI counts only when its host is empty, `localhost` or this machine's name, ignoring ASCII case, because a shell reached over SSH reports a directory of another machine. Its path is percent-decoded once and must be absolute; a query, fragment, control character or invalid UTF-8 refuses the value. Only when a URI names some other host does it read this machine's name, the `local_host_name` that `gethostname` reports and zsh, bash and fish write into OSC 7; when that read fails only an empty or `localhost` host counts. Callers separately enforce narrower launch path bounds. `directory/windows/initialization.ps1` synchronizes PowerShell process cwd after the original prompt, preserving status and leaving non-filesystem providers unchanged. Its first prompt clears the startup pipeline's own session-history entry by that pipeline's history ID, so `Get-History` in a `-NoExit -Command` shell lists only the user's commands; PSReadLine never reads the startup text. After a line submitted with the `<#orchbee-run:<id>#> ` prefix, the prompt also removes that command's session-history entry. Ordinary Worker shells use this single hook and child-local UTF-8 native-output decoding and outgoing pipeline encoding.

`windows_interactive_powershell_initialization` concatenates that common asset with
`directory/windows/reader.ps1`, which wraps the existing interactive reader with semantic
read-phase markers. While it waits for a line, it swaps in a PSReadLine `AddToHistoryHandler`
that rejects a run-prefixed line and passes every other line to the handler it replaced, then
restores that handler. Orchbee-submitted lines therefore never reach PSReadLine history, its
saved file or Up-arrow recall. `directory/windows/start.ps1` comes last: when the launch sets
`AGENT_START_VARIABLE`, it clears the variable, mints a run token, writes the run's start
marker and runs the line `windows_agent_start_command` composed, so an Agent starts as the shell's
first command with the same resolution a typed start uses (`windows_powershell_application`).
A command that is not found ends only the startup script; the interactive shell and its first
prompt follow. Without the variable it does nothing. `AGENT_START_VARIABLE` is not Windows-only:
the Terminal Host's managed zsh and bash startup on macOS and Linux reads the same variable. The Worker prepares this exact startup for
admitted plain PowerShell argv; the Host build reads the same three assets, in the same order, for
injection and exact recognition. The noninteractive `windows_encoded_command` helper sets only
UTF-8 console and pipeline encoding; it carries no prompt or reader asset, so its typed text stays
short. No asset writes a profile or changes an external PowerShell process. Actual 5.1/7 pipeline,
prompt, first-command and PTY startup witnesses cover the managed path; arbitrary startup scripts
remain outside it.
The assets stay compact, without comments and with tab indentation, so the complete interactive
command line with a `MAX_PATH` executable fits the Host's 4096 UTF-16 process-text limit. A longer
command line is sampled as unreadable, and Agent start can no longer recognize the idle managed shell.

Reader admission requires standard PSReadLine module/export identity and a recognized definition
fingerprint; custom and unsupported definitions stay untouched and unknown. Repeated/skipped reader
installation cannot abort the enclosing initializer. The modern adapter preserves the preceding
command's status before marker output, and passes it to native ReadLine with the original module
context; the legacy reader retains its original two-argument behavior.

Retained-shell admission recognizes only the exact current shared PowerShell startup.
Historical directory-only/UTF-8 argv fingerprints, arbitrary suffixes and extra argv are rejected.
Initialization and encoded helpers consume only the owned assets.

## User settings

`config.toml` is the one user settings file; settings pages will become a view over it. The
registry, `SETTING_KEYS` in `src/user_settings/registry.rs`, declares every key once: its dotted
path, value type with allowed values or range, default, page category and group ids, its scope,
the Clients it applies to, when a change applies and whether the settings page shows it. The
reader, the `orchbee config default` reference, `config check` key knowledge, the editor and the
published key file all derive from it, so adding a key starts there.

A key's `SettingScope` says whose file holds it. `Client` keys describe how one Client looks and
takes input (`general.language`, `appearance.*`, the terminal font, padding, opacity, Option,
selection-copy and blank-row keys, `notifications.*`, `keys.*`) and are read from the file on that
Client's machine. `Server` keys describe what the Server does (`terminal.new_cwd`,
`scrollback_bytes`, `restore_output`, the launch profiles and default profile,
`worktrees.directory`, `agents.resume_on_restore`, `update.channel`, `update.automatic`)
and are read from the attached Server's file. With a local Server both are the same file.

### Compatibility

`config.toml` has no version field. From the first public release a key is never removed or
reshaped in place: a key that must change gets a new name, and the old name stays an alias that
still applies and reports a deprecation diagnostic. A key this version does not know, such as one
a later release added, is reported as a warning and ignored, so an older Orchbee keeps working with
a newer file. Orchbee never rewrites the file to migrate it; only an explicit edit changes it.

### Published key file

[`schemas/workbench/v1/setting-keys-v1.json`](../../schemas/workbench/v1/setting-keys-v1.json)
is the registry as data: every key with its `path`, `type`, bounds or `choices`, `default` (`null`
when unset), `scope`, `clients`, `effect`, `category`, `group` and `page`, in registry order. The
test `setting_key_file_matches_the_registry` fails while the file differs from the registry, and
running it with `ORCHBEE_REGENERATE_SETTING_KEYS=1` rewrites the file. The output already has the
repository formatter's layout, so commit it as written.

### Tables and keys

Each top-level table names the page category whose keys it mainly holds: `[general]`,
`[appearance]`, `[terminal]`, `[notifications]`, `[keys]` (Keyboard) and `[agents]`;
`[worktrees]` and `[update]` hold one key each. The page category is metadata,
so a key can appear under another category's group, as the table below shows. Themes is the one
category without a table of its own: its two keys keep their `[appearance]` paths, the single
exception to a table naming its category. `<platform>` stands for `windows`, `macos` or `linux`,
`<action>` for a registered Client action and `*` for any name. `orchbee config default` prints
every key's purpose, allowed values and default.

| Key                                                                                         | Default                          | Page (category / group)              |
| ------------------------------------------------------------------------------------------- | -------------------------------- | ------------------------------------ |
| `general.language`                                                                          | `"system"`                       | General / Basics                     |
| `appearance.mode`                                                                           | `"system"`                       | Appearance / Theme                   |
| `appearance.light_theme`                                                                    | `"orchbee-light"`                | Themes / Light                       |
| `appearance.dark_theme`                                                                     | `"orchbee-dark"`                 | Themes / Dark                        |
| `appearance.terminal_colors`                                                                | `"theme"`                        | Appearance / Terminal colors         |
| `appearance.custom_foreground`, `custom_background`                                         | `"#141414"`, `"#fcfcfc"`         | Appearance / Terminal colors         |
| `appearance.window_title`                                                                   | `"{hostname}: {workspace}"`      | file only                            |
| `appearance.sidebar.workspaces.rows`                                                        | workspace and Git, then location | file only                            |
| `appearance.sidebar.workspaces.row_gap`                                                     | `0`                              | file only                            |
| `terminal.new_cwd`                                                                          | unset (no override)              | General / Basics                     |
| `terminal.copy_on_select`                                                                   | `false`                          | Terminal / Input                     |
| `terminal.macos_option_as_alt`                                                              | `"off"`                          | Terminal / Input (Desktop)           |
| `terminal.scrollback_bytes`                                                                 | `10000000`                       | Terminal / Output                    |
| `terminal.restore_output`                                                                   | `false`                          | General / Restore                    |
| `terminal.font_family`, `font_size`                                                         | built-in stack, `13`             | Appearance / Terminal text (Desktop) |
| `terminal.font_weight`, `font_weight_bold`, `font_ligatures`, `padding`                     | `400`, `700`, `true`, `0`        | file only (Desktop)                  |
| `terminal.background_opacity`                                                               | `100`                            | file only (Desktop)                  |
| `terminal.default_profile.<platform>`                                                       | `"system-default"`               | Terminal / Profiles                  |
| `[[terminal.profiles.<platform>]]` `id`, `name`, `kind`, `login`, `executable`, `arguments` | only the built-in profile        | Terminal / Profiles                  |
| `worktrees.directory`                                                                       | unset (beside the repository)    | file only                            |
| `notifications.agent_reminder`                                                              | `"never"`                        | Notifications / When                 |
| `notifications.completion_reminder`                                                         | `"never"`                        | Notifications / When                 |
| `notifications.completion_minimum_seconds`                                                  | `5`                              | file only                            |
| `notifications.sound`                                                                       | `false`                          | Notifications / How                  |
| `notifications.cli_delivery`, `delay_seconds`, `position`                                   | `"off"`, `1`, `"bottom-right"`   | file only                            |
| `notifications.sound_by_agent.*`                                                            | no overrides                     | file only                            |
| `keys.prefix`                                                                               | `"ctrl+b"`                       | Keyboard / Prefix (TUI)              |
| `keys.<action>`                                                                             | the action's built-in keys       | Keyboard (TUI)                       |
| `[[keys.command]]` `command`, `type`, `key`, `description`, `width`, `height`               | no commands                      | file only                            |
| `agents.resume_on_restore`                                                                  | `true`                           | Agents / Restore                     |
| `update.channel`                                                                            | `"stable"`                       | file only, not in the reference      |
| `update.automatic`                                                                          | `true`                           | General / Updates, once switched on  |

`update.automatic` stays off the settings page and out of the reference until the
[automatic updates](../../proposals/platform/automatic-updates.md#phases) slice 1 switch marks it
for both.

The old `[ui]` names are gone without migration or aliases: `[ui]` is one unknown key. The keys
that carry a `UserSettingsField` and the launch profiles parse into `UserSettings::values`, the
common values that are the Session Server's and the TUI's runtime source. The Desktop font,
padding, opacity and Option keys parse into `UserSettings::desktop`, but their runtime source is
still the Desktop's own state until a later phase moves it onto the file. The notification profile the
Server publishes uses the file's spelling: one `[notifications]` table with `cli_delivery`,
`agent_delivery` (profile only), `delay_seconds`, `position` and `sound`, and
`[notifications.sound_by_agent]`. `NotificationSettings::from_profile` refuses with field
`notifications` and still reads a profile spelled `[ui.toast]` and `[ui.sound]`, which nothing
writes any more.

`settings_values(lookup, launch_profiles)` is the one derivation of the common values from key
values: `lookup` returns a key path's `Scalar`, or `None` when the file leaves it unset, and a
value its key does not admit (`SettingKey::admit`) keeps its default. The reader builds
`UserSettings::values` with it over the file, and a Client holding a copy of the file's values
builds the same values with it.

`settings_snapshot(load, revision)` builds the `workbench.settings` snapshot of a load: each
concrete scalar key's file value or default, the keys the file sets, the effective launch catalog,
the bounded diagnostics and the file status, which a load that kept its last valid settings reports
as `syntax-error` and a startup load that could not read the file as `unreadable`.
`snapshot_values(snapshot)` derives the common values from a snapshot through `settings_values`.
`value_changes(baseline, values, restored)` goes the other way: the edit, in the file's spelling,
that turns one set of values into another, restoring each listed `UserSettingsField` that did not
change. `setting_changes` admits a wire edit's changes, and
`settings_refusal` maps an edit refusal onto the wire reason and its members.

Each `[[terminal.profiles.<platform>]]` table is admitted by its own platform's launch rules on
every host: a custom executable must be an absolute path in that platform's form (a drive or
`\\server` path on Windows, `/` elsewhere), and a Windows system shell cannot be a login shell. A
table that breaks them falls back as a whole with `invalid-value`.

The built-in `system-default` profile is `UserLaunchCommand::BUILT_IN`: the system shell, started
as a login shell on macOS, where terminals start login shells so `path_helper` and the person's
profile set `PATH`, and without login elsewhere. A profile either Client adds starts with the same
command. A table entry with that id replaces it, so a
`login = false` the person saved for it stays as written; a catalog that leaves the built-in
profile as built writes no entry for it, so it follows this default.

### Reading

The reader is strict per key and tolerant per file:

- A file over 2 MiB (`MAXIMUM_USER_SETTINGS_BYTES`), a directory, an unreadable file, a missing
  explicit file and an invalid location variable are errors; a missing default file is the
  defaults. A Session start never stops on the file, because the person needs a terminal to
  repair it: `read_startup_settings`, which Client activation and the Server's first read use on
  every host, and `read_startup_settings_at`, its form at a location a Windows Client froze, start
  with the defaults instead and name the file's reason in `UserSettingsLoad::unreadable`, which the
  snapshot publishes as `unreadable`. Only an invalid location variable still refuses the start.
  An edit is refused while the file exists but cannot be read; the first edit creates a missing
  file.
- A file that is not valid UTF-8 or TOML gives the defaults, or the last valid settings through
  `UserSettingsLoad::or_last_valid`, plus a `file` diagnostic. `UserSettingsLoad::syntax` names the
  reason and the 1-based line and column, counted in characters. Startup paths (Client activation
  and the managed Server's first read) continue on that result; `config check`, `channel show`,
  `channel set`, `update`, `config reload` and the Server's sampler treat it as fatal and
  keep what they had. The CLI reads `UserSettingsLoad::syntax` itself so its failures keep the line
  and column; the Server calls `UserSettingsLoad::require_valid_syntax`, which keeps only the
  reason.
- An unknown key is reported as `unknown-key` with disposition `ignored`, left in the file and
  never echoed.
- A value of the wrong type or outside its allowed values falls back to that key's default and is
  reported (`wrong-type`, `invalid-value`, `invalid-mode`, `value-too-long` or a key-specific
  reason); an invalid layout or launch-profile table falls back as a whole.

Diagnostics are deduplicated before display truncation, sorted by field and reason, capped at 128
entries with an exact omitted count, and never retain source values or commands. See
[configuration and precedence](../../apps/orchbee/docs/windows-cli-configuration.md). The reader
and the editor run on every host and read `config.toml` as a plain file under the data root. Path
keys (`terminal.new_cwd` folders and `worktrees.directory`) use the host's grammar. Windows takes
the Windows user-path grammar below. macOS and Linux take an absolute path as written, `~` or
`~/...` under `HOME`, or a path relative to the settings file's folder, resolved lexically; a
Windows drive or share path there is `invalid-path`.

The same reader owns `UpdateChannel` (`stable`/`preview`, default stable) with default/file
provenance; native `channel show` consumes it offline and infers nothing about the installed
release. The pure Windows user-path adapter normalizes relative and tilde operands lexically and
keeps absolute literals verbatim; applications own wire projection, activation commit and private
child arguments.

`user_settings_path` resolves the same file without opening it. It preserves explicit
`ORCHBEE_CONFIG_FILE` precedence over `config.toml` in the data root; `ORCHBEE_CONFIG_FILE` takes
the same absolute-path admission as `ORCHBEE_HOME`. A missing home leaves the location absent, and
an invalid `ORCHBEE_HOME` is a settings error. Detection rules and other records follow the data
root, never the settings file's folder.

### Editing

`edit_settings(changes, env)` applies one to 64 changes (`SettingChanges`, no two naming the same
key), each `SettingChange::Set` or `SettingChange::Restore`, in one write or none, keeps every other
byte, and returns whether the file changed. `set_setting(key, value, env)` and
`restore_setting_default(key, env)` are the same edit with one change.

1. Refuse an unregistered key, or one part of a key that is edited whole (`unknown-key`), and a
   structured value over 64 entries or 64 KiB of JSON (`value-too-long`), before opening anything.
2. Take `internal/config.lock` in the data root, waiting up to two seconds (`locked`).
3. Read the file, following a symbolic link to the file it names, and record its SHA-256.
4. Apply every change to one candidate with `toml_edit`: a key already present keeps its comments
   and inline comment; a new key goes into its table, and a missing table is created with a header
   after any comments that end the file. A structured key such as `terminal.profiles.<platform>`
   takes its whole value, an array of tables written as `[[...]]` entries. Restoring removes the key
   and any table it leaves empty, unless that table carries a comment. Setting a scalar key at a
   concrete path to its registered default is that restore, so every Client and the CLI leave the
   key to follow a later default; a `<platform>` key's default is the one registered for every
   platform. A structured value and a key under a map of names are written as given.
5. Validate the candidate once with the reader. A diagnostic the file did not already have, on a
   changed key or on any registered key, refuses the whole edit as `invalid-value` with the
   reader's field and reason; a default profile left naming a removed profile is refused this way.
6. Write `config.pending` beside the file and flush it, then compare the file with the recorded
   digest; when another program changed it, redo the edit once from the new text, then report
   `changed`.
7. Replace the file atomically. A symbolic link keeps its link and the target is replaced.

A missing file is created holding only the changed keys. A file that does not parse is never
written: the edit returns `Syntax` with its line and column. A file that cannot be read, or is not a
regular file, is `Source` with the reader's own reason (`io-error`, `not-file`, `too-large`), even
where the platform reports a denied open, as Windows does for a folder; only a readable file marked
read-only is `read-only`. On Windows a file another program holds open is retried for two seconds,
then `in-use`. Nothing
changes when the file already says what the changes ask for.

`SettingsEditError` keeps one variant per refusal: `UnknownKey { key }`, `InvalidValue { key, field,
reason }`, `Syntax` (reason, line, column), `ReadOnly`, `InUse`, `Locked`, `Changed`, `TooLarge`,
`Source` for an unusable location or file, and `Io` for any other write failure. `key` names the
change a refusal is charged to; `field` can name another key the change affects.
`set_update_channel` is `set_setting` on `update.channel` and returns the `SettingsEditError`
itself; `SettingsEditError` has no conversion into `SettingsError`, so a caller keeps each
refusal's own reason.

### Reference

`default_user_settings_toml_with_actions` renders the registry as comments only, headers included:
a fixed-English header, then each key's purpose, when a change applies, its default and allowed
values, and a commented example line; the application supplies each Client action's current
default keys. `config check` accepts it unchanged, and parsing it gives the same settings as an
absent file.

## Private update cache

`PrivateCache` owns pinned private ancestors, no-follow regular single-link files and a
cross-process writer lock for one named record in a folder. Applications retain logical
filenames, content validation, versions and policy. Missing descendants are created only below an
admitted private parent; unsafe existing data is refused without repair. Bounded reads preserve
absence. Replacement flushes a complete staging file, then atomically replaces the admitted leaf.
Removal deletes only an admitted leaf under the writer lock; an absent leaf is already removed.
The caller names the record: `<record>.lock` beside it is the persistent writer lock and
`<record>.pending` the one owned staging leaf, removed after settlement or recovered under the next
exclusive lock. Both names are reserved and refused as leaves; another record in the same folder
locks independently. `acquire_with_lock` names the lock separately, for a folder whose one writer
lock guards more than the record: the release store's `install.lock` guards `active.json`, staged
in `active.pending`. The default settings file keeps its lock in `internal/config.lock`, so its
folder gains only `config.pending` while a write runs. `prepare` creates a private folder without a
lock, for a folder whose contents another program owns. Cleanup failures are returned, never converted into successful publication.
Applications own what each record holds; this adapter alone implements no update policy.

`read_existing` pins and validates an existing private directory tree and performs a bounded leaf read without creating directories, lock files or staging files. It serves startup/reload status inspection; missing cache paths remain absent.

Settings retain explicit-location status. Invalid supported leaves default independently; invalid key actions remain explicit disabled overrides, while valid chords from the same action survive. `read_user_settings_at` reads a complete candidate at a frozen path, reapplies the strict resume environment override, preserves missing-default/missing-explicit behavior and creates no files. Applications retain atomic settings/diagnostics publication, the authoritative Server startup baseline and automatic scheduling policy.

## Keybinding profiles and plugin catalog

The settings owner encodes and validates canonical `[keys]` profiles for live Client publication, capped at 64 KiB and 144 expanded triggers per action. User files keep the 16-alternative bound; Server and Client own publication and installation.

The Windows plugin catalog boundary supplies bounded manifest parsing, normalized identifiers, platform inheritance, source availability and atomic private user registration for native plugin management. Reads do not create storage; unavailable sources retain explicitly marked inspection metadata. Each plugin's folder is spelled from its ID, recorded in the registry, and keeps its `config` and `data` folders after unlink. The shared Windows command owner provides bounded output capture and descendant settlement for Worker commands and offline provisioning; applications own execution policy. Managed provisioning owns serialized source download/build/commit/cleanup, including update from a recorded source, through an application-supplied executor, with one strict version 1 catalog including required provenance/recovery fields; see the [native plugin contract](../../apps/orchbee/docs/windows-plugins.md).

## Release store reading

`release_store` is the read side of the per-user release store that the Client's
[release installation](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation)
writes. `release_store_root` is the one store-root rule: `ORCHBEE_INSTALL_DIR`, otherwise
`%LOCALAPPDATA%\Programs\Orchbee` for release and `%LOCALAPPDATA%\Programs\<stem>` for a
development or test build, refusing a relative path and one holding a PATH delimiter, an expansion
token, a quote or a control character. On macOS and Linux it is `ORCHBEE_INSTALL_DIR`, otherwise
`$HOME/.local/share/orchbee` for release and `$HOME/.local/share/<stem>` for a development or test
build, the npm launcher's rule; it is used as spelled, never resolved, and refuses a relative path,
a `.` or `..` component, one over 1024 bytes or not UTF-8, and one holding `:`, `$`, a backtick, a
backslash, a quote or a control character. `active_release`
reads `active.json` through `PrivateCache::read_existing` and parses it with
`orchbee-release-records`: a missing root or pointer, or bytes that are no pointer record, mean no
active version, and an unsafe root or pointer, or one over 16 KiB, is an error. A read that meets
the writer's momentary rename fails like any other and may be retried. `is_release_version_directory`
answers whether a folder has a store version folder's shape: a version name below a folder named
`versions` in any letter case. The Client's install, update and uninstall commands and its Session
start read the store through these, and the CLI's `release_store` writer takes the folder and
pointer names and the bound from here. Reading takes no lock, creates nothing and starts no
process.

### Fallback after a failed move

Beside the read side, `release_store::fallback` owns the one pointer write besides the Client's
release store: the `fallback` member of the pointer's `extensions`, whose format
`orchbee-release-records` owns. Each write takes `install.lock`, reads `active.json`, applies one
transition and replaces the file through the store's flushed rename, so no two writes interleave
and a crash leaves the old file or the new one; readers still take no lock. All of it runs only
where the [automatic-update switch](#automatic-update-switch) is on. A build where it is off
records nothing and keeps `extensions` verbatim, so a move it makes leaves a record whose move no
longer counts.

- `complete_move` is the completion of a build whose switch is on. It refuses a held target with
  `VersionHeld`; otherwise it moves the pointer as `Pointer::activate` does and drops held versions
  at or below the target. A real move writes a fresh move whose `back` is true only when the
  record's move counted for the replaced entry, which proves that the replaced version records,
  and the caller accepts that entry: its `release.json` still hashes to its manifest, and it meets
  every component `securityFloor` of the target's own verified manifest. Activating the active
  entry again keeps the record as it is.
- `FallbackRecorder` writes only the store its executable runs from. It decides from a read
  without the lock, so a start long after a move never takes the lock, then tries the lock every
  50 ms for at most 2 s and decides again from the pointer read under it; a record it cannot write
  is dropped and logged as `fallback-record-dropped`. `record_event` is that decision: only a
  counting move whose `back` is true records; each role's window opens with its first record and
  lasts 600 000 ms, and a record dated before it counts nothing; a TUI or Session start needs the
  recorder's own version to be the moved one; a Client failure counts only for the moved version;
  a Session failure counts once, for a generation in `starts`.
- A role's third failure inside its window falls back when the version folder `previous` names
  exists: `previous` becomes active and is removed, the failed version joins `held`, and a fresh
  move with `back` false and `restore` true keeps that version from falling back further. The
  committing process logs `fallback` with both versions. Without that folder the failure is
  recorded, nothing falls back, and the role records nothing more for the move.
- `held_versions` lists the held versions above the active one. Nothing makes them active until a
  higher version has been, which drops them.
- `FallbackRecorder::for_current_executable` is the recorder of the program the process runs, and
  `activation_text` the 32 lowercase hexadecimal digits an activation identifier takes in the
  record. The observers that use them belong to their processes: the activator records each
  generation's start just before it starts the Server and its failure when the Server cannot start
  or ends before its endpoint, and a generation `SessionCatalog::recover_inactive` proves dead,
  whose identifier it answers; the Server records a Worker that exits outside a stop, reading its
  own generation with `RetainedSessionRecord::current_activation_id`; the TUI records its start on
  a background thread it waits for before it exits; and the
  [Client-failure helper](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation)
  records a Client failure. Only the TUI start uses a thread; each other record is written on its
  failing path, within the 2 s lock bound and only while a move counts.
- While the counting move carries `restore` for the version the process runs, a writer restores a
  record it meets in a newer format, as [persisted data versions](#persisted-data-versions) says.

## Update channel writes

`set_update_channel` saves the channel through the [settings editor](#editing), so it shares its lock, validation, digest check and atomic replacement, and may create the absent file. It never downloads, and never rolls back the saved channel after a caller's update failure.

## Automatic-update switch

`automatic_updates_live()` is the one switch that every automatic-update behavior a release build
would show or run consults, so release builds turn them all on in one change. It is a constant
expression of the [environment kind](#windows-named-runtime-identity): every test build answers
true, so tests and rehearsals drive the behaviors; a release build answers the private
`IN_RELEASE_BUILDS` constant, false while the behaviors land dark; and a development build answers
true because the switch does not gate development builds, where each behavior keeps its own rule,
such as never scheduling an update and never showing Update. It decides nothing else: platforms,
settings and each behavior's own conditions stay with their owners. The
[automatic updates proposal](../../proposals/platform/automatic-updates.md#phases) says when release
builds turn it on. The npm launcher, plain JavaScript that cannot read it, keeps the release-build
half as its own constant, `automaticUpdatesInReleaseBuilds` in
`apps/orchbee/npm/orchbee/lib/launcher.mjs`; the change that sets `IN_RELEASE_BUILDS` sets both.

`AUTOMATIC_UPDATE_HELPER` is `--automatic-update-v1`, the one argument with which a Session Server
runs the store's active Client for an automatic update and by which that Client recognizes the
[automatic helper](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation).
A Client answers it only while `automatic_updates_live()` holds, and its spelling never changes.
`CLIENT_FAILURE_HELPER` is `--record-client-failure-v1`, the word with which either launcher runs
the previous version's Client, followed by the failed version, to record a Client failure for the
[fallback after a failed move](#fallback-after-a-failed-move); the same switch and the same rule
apply.

`Cohort::artifact_sha256` retains each executable digest from successful admission. Installation provenance compares its declared digests against these observations, so a cohort change between reads cannot be presented as the previously admitted release.

## Sidebar presentation settings

The shared settings owner admits the `appearance.sidebar.workspaces` layout. Token styling and
raw-value first-match rules have one admission owner, which rejects unsupported tokens and non-text
rules on icons and Git counts. The closed canonical profile caps at 32 KiB. Startup, CLI checks and
Session reload all consume this owner. Agent rows have no layout: `AGENT_SIDEBAR_PROFILE` is the
fixed profile the configuration snapshot and `config check` still carry, and a file that sets
`appearance.sidebar.agents` gets an `unknown-key` diagnostic. See the [user guide](../../apps/orchbee/docs/windows-cli-configuration.md#windows-workspace-sidebar-layouts).

## Windows handoff pipe identity

An explicit Worker handoff derives a separate same-user named pipe from the already admitted
runtime hash and its nonzero 128-bit ticket. `handoff_endpoint_path` never creates a filesystem
leaf or grants authority to an arbitrary path. Applications own ticket validation, target cohort
admission, the standby lease and process/Server ownership; ordinary client/worker/Host endpoints
keep their existing identity. Native pipe tests verify separate scope/ticket addresses and bounded
names even for long Unicode runtime input. [The Worker contract](../../apps/orchbee-worker/docs/windows-server-handoff.md)
owns the connection transition.
