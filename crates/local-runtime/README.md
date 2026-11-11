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
| `user_preferences`, `default_interactive_launch`, `notification_policy`    | The Desktop settings bridge, Client, Server and Worker consume shared preferences, launch snapshots or notification profiles; settings decoding uses notification types directly without platform aliases.                                                  |
| `data_root`, `user_settings`, `plugins`, `user_path`                       | Server configuration and the Client's minimal `config.toml` reads (activation, release channel and `config check`) share the bounded settings schema; plugins and paths also serve Worker execution, and Windows path grammar stays explicit on every host. |
| `command`, `command_outcome`, `command_child`, `directory`                 | Client commands/provisioning, Server restart and Worker commands/directory observations share encoding, outcomes and process settlement contracts.                                                                                                          |
| `private_cache`, `private_filesystem`, `host_appearance`                   | Runtime storage, CLI release storage and Server frame reads share exact handle admission; Client and Worker share read-only Windows theme observation.                                                                                                      |

## Shared types and platform effects

Settings and plugin schemas, command outcome types, named-Session syntax, Windows path operands
and literal command encoding are available on every supported platform. Windows path admission
uses its explicit grammar even on Unix; environment and cwd are supplied by the execution owner.
OS effects stay in their owning adapter. The Windows private cache, legacy settings source and
plugin registry refuse unsupported Unix operations before opening a path or spawning a process;
private-cache errors use `io::ErrorKind::Unsupported`, the settings error uses the stable
`platform` / `unsupported-platform` diagnosis, and the registry uses `PluginRegistryError::Unsupported`.
Existing cross-platform `UserPreferencesStore` support is independent of these legacy facilities.
Applications decide action visibility and localized feedback through their
[TUI platform contract](../../apps/orchbee/docs/tui-platforms.md).

Product types for Session identity, lifecycle, checkpoints and command outcomes are also available on every supported platform. Implementations live in `windows`, `unix`, `linux` or `macos` child modules; unimplemented named lifecycle, checkpoint, registered-child and retained-command operations return `io::ErrorKind::Unsupported` before effects. Deletion uses `NamedSessionDeleteReason::Unsupported`; configuration reset and channel writes use the same `platform` / `unsupported-platform` diagnosis. Existing Unix rendezvous, cohort, retained-child and shared preference implementations remain with their own adapters.

## Shared user preference storage

`UserPreferencesStore` owns the versioned, bounded device preference record described by the
[shared contract](../../schemas/workbench/README.md#shared-user-preferences). Its scope is frozen
from the data root: the record is `internal/state/preferences.json`, an explicit settings file does
not move it, and only another data root does. Reads create nothing; invalid or unreadable records are refused.
Updates acquire an OS-held writer lock, compare the observed source, apply only supplied leaves,
validate the complete candidate and publish a flushed atomic replacement. A reset claims its keys
just like an edit. Windows uses the pinned private-cache boundary; Unix uses descriptor-relative,
no-follow private files and synchronizes the containing directory after replacement. A failure after
replacement reports an unknown outcome and never authorizes automatic replay.

Reads preserve validated current values, claims and source without reinterpreting an unclaimed old default or rewriting bytes. A record whose version this build cannot use is never rewritten: reads give the defaults, and updates apply to values the store keeps in memory, with a `memory-` source token, until the Session stops. Resetting every setting is the one update that replaces it, after a copy goes to the `reset-settings` backup folder ([persisted data versions](#persisted-data-versions)). Desktop import fields are outside the current strict record.

An absent preference record returns canonical defaults, no claimed keys and the `missing` source token. Only the preference record participates in source observation. Applications own UI drafts, polling, publication into running terminals, and process launch. The helper and native Session consume this same storage owner, not separate stores.

An explicit full-null reset with recovery source `unreadable` may replace a safely readable but
invalid record under the same OS writer lock. A valid/missing record refuses that recovery source,
so a concurrent repair is preserved; partial resets and ordinary edits cannot use it. Unsafe
paths, ACL failures and bounded-read failures retain their storage refusal. Reads never repair a
record. The [shared preference contract](../workbench-protocol/docs/user-preferences.md#explicit-recovery-from-a-refused-record)
owns the recovery token and Client notice.

## Shared launch and notification policy

`default_interactive_launch` owns pure platform shell selection and native executable admission,
shared by Desktop's matched helper and Native Worker. It starts no process. The preference store
captures scrollback and the selected launch profile together; an explicit executable bypasses
default shell resolution while retaining the shared scrollback default. POSIX login is explicit;
Windows login is refused. Invalid shared storage fails the operation before Host handoff.

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
and global sound policy, including values governed by shared preferences.

## Windows host appearance fallback

`read_windows_apps_use_light_theme` performs a read-only current-user lookup of
`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\AppsUseLightTheme`.
It returns `Some(true)` or `Some(false)` only for the documented `REG_DWORD` values `1` and `0`;
missing, unreadable, and other values return `None`. The Client invokes this once only when its
native terminal OSC `10`/`11` probe has no valid OSC `11` background by the deadline. The Worker
invokes it for a Host-default launch before any Client has reported `host-colors`. Each
caller owns its precedence; this runtime helper changes no Windows setting and reads no other theme
state.

## Windows literal command encoding

The pure Windows command encoder is shared by CLI Agent start and Server restart. It emits a bounded UTF-16LE Base64 script with one literal argument array. Native, batch and PowerShell script dispatch preserve argument boundaries, including empty values, quotes and percent signs. It excludes aliases/functions, rejects control characters, caps encoded text at 30,000 characters and refuses batch command lines over 8,000 characters. Applications select programs, shells, interactive flags and stricter carrier limits; this library starts no process. Helper functions and error preferences remain in a child scope when the caller keeps PowerShell open. The embedded PowerShell asset is maintained with this encoder and the CLI's real Windows argv test.

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
| `sessions/<session>/`                                | One Session's folder, named by the Session: its catalog record, checkpoints, earlier layouts, kept output and TUI position                   |
| `agent-detection/<provider>.toml`                    | Detection rule overrides a person writes                                                                                                     |
| `logs/<YYYY-MM-DD>.jsonl`                            | Daily diagnostic log of every process, named by local date                                                                                   |
| `logs/destructive-commands.jsonl`                    | Destructive request log and its `.1` generation                                                                                              |
| `internal/version.json`                              | Layout record and the newest Orchbee version that used the root; `version.lock` and `version.pending` guard its replacement                  |
| `internal/config.lock`                               | Writer lock of the default settings file                                                                                                     |
| `internal/sessions.lock`                             | Namespace lock that serializes Session activation and deletion                                                                               |
| `internal/state/<record>.json`                       | `StateRecord`: preferences, onboarding, and the machine catalog                                                                              |
| `internal/cache/agent-detection/`                    | Downloaded detection rules and `catalog.json`, which names the URL                                                                           |
| `internal/backups/<date>-reset-keys/`                | Copy of the settings file taken by `config reset-keys`                                                                                       |
| `internal/backups/<date>-upgrade-<version>/`         | Originals of the records one product version migrated, at their paths relative to the root; the three newest folders stay                    |
| `internal/backups/<date>-<reset>/`                   | Copy of a record taken before a reset the person requested; never deleted automatically                                                      |
| `internal/plugins/registry.json`                     | Plugin registry, folder assignments, and recorded cleanup                                                                                    |
| `internal/plugins/<folder>/`                         | One plugin's `config/`, `data/`, and `source/<version>/`                                                                                     |
| `internal/plugins/source.pending/`                   | The source a running install fetches before it moves                                                                                         |
| `internal/integrations/agent-observation/<version>/` | Agent observation Capsules the [integration writer](../agent-integration-writer/README.md#capsule-location-and-ownership) installs and names |
| `internal/runtime/<session>/`                        | A Session's runtime folder in every environment, unless `ORCHBEE_RUNTIME_BASE` names another base                                            |
| `internal/tmp/`                                      | Scrollback documents the Terminal Host writes for an editor, removed a day after their last write                                            |

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

Release Orchbee once installed Capsules for every environment in `~/.orchbee/agent-observation-adapters`;
`legacy_agent_observation_capsules` names that folder for release only.
`agent_integration_may_have_been_enabled` answers `false` only when neither this environment's
Capsule folder nor that legacy folder exists, which proves that no provider entry names this
environment; an unresolved root or unreadable folder answers `true`. It runs no provider command
and reads no provider configuration, so implicit callers use it to skip integration inspection
and to give an enabled integration's Session a head start over observed Agent startup.

Only the top level holds what a person reads, edits, or keeps. Every writer that creates a folder
inside the root also writes `internal/version.json` when it is absent, so a root that holds anything
Orchbee wrote records its layout. The record is `{"layout":1,"newestVersion":"<version>"}`, and
Desktop Main reads and writes the same shape; search for `data-root-version-record` before changing
it. Later releases only add entries, which every rewrite keeps. `DataRoot::admit_layout` reads only
this record and refuses a root whose `layout` is higher than `LAYOUT`, or whose record cannot be
read, and every process that opens a root calls it before any other read: the Client before it
dispatches any command that opens the root, and the Server and Worker before their first log
record. The first writer of each process also refuses such a root before it creates anything, and
raises `newestVersion` to `PRODUCT_VERSION` under `internal/version.lock` when the record names an
older version, using Semantic Versioning precedence, so a refusal can name the newest version. Nothing migrates or removes what an earlier build
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
must pass the store's parser. No step ships yet, so an earlier version is not updated until a format
changes. Since no other path hands a store a record, a step added there reaches every store,
backup and persistence included, with no store change; the module's tests drive every store through
an injected step.

A writer's opening copies the original into `internal/backups/<YYYY-MM-DD>-upgrade-<product
version>/` at its path relative to the root and flushes it before the replacement runs; every record
one product version migrates shares that folder, and creating another keeps only the three newest.
Desktop Main backs its window state record up into the same folders; search for
`upgrade-backup-folders` before changing the naming, the copies, or retention. A failed step, backup,
or replacement leaves the record byte-identical, and the store treats it as not updated: the notice
is reported, and the process keeps treating that record so until it ends, so its readers and writers
agree. `back_up_for_reset` copies a record into `internal/backups/<YYYY-MM-DD>-<reset>/`, or the next
free `-2`, `-3`, before a reset the person requested replaces it, and nothing deletes those folders.
Backups are Windows effects; on POSIX a writer's opening reports an earlier record as not updated
until ORC-328.

| Record                            | Version and extension object | While its version is one this build cannot use                                                                                                          |
| --------------------------------- | ---------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `internal/state/preferences.json` | `version`; `extensions`      | Reads give the defaults, and accepted changes stay in the store's memory until the Session stops; resetting every setting backs it up, then replaces it |
| `internal/plugins/registry.json`  | `version`; `extensions`      | Readers see no plugin; link, install, folder assignment and cleanup are refused with `UnusableRegistry`                                                 |
| `sessions/<session>/session.json` | `version`; `extensions`      | The catalog lists the Session as unrecognized with reason `Version`; it is neither started nor deleted                                                  |

Each store keeps an `extensions` object it reads as opaque and writes back unchanged, so a later
release adds an optional field there without a migration. A record migrates only in a process that
writes it: preferences on an accepted update, the plugin registry when a writer changes it, and
`session.json` when it is published, which outside the Server happens only after proving no Server
of the Session runs. Workbench Client owns the onboarding, machine and TUI position records, kept
through `VersionedFile`, with the same rules. `inspect_records` reads every versioned
record of a root, or one Session's, creating nothing, and reports the unusable ones; `config check`
lists them and the TUI shows the first when it starts, since the Server reads the records it holds.

`report_record_notice` logs a record a process cannot use, once per record and problem, and answers
whether this was the first time, so the process shows one of the two localized messages once;
`reported_record_notices` lists them for a surface that shows them, and `take_unshown_record_notices`
hands over only those no surface has shown yet: the CLI prints them on stderr after its command, and
`config check` takes the ones it lists. Applications own the message text and each record's
consequence.

## Windows user path operands

`UserPathOperand::parse` admits bounded drive-absolute, home-relative or current-directory-relative syntax without reading process context or probing the filesystem. The opaque operand preserves its spelling; `resolve` uses only the final execution owner's supplied environment or current directory. Absolute literals need neither capability. Invalid required home values never fall through to a lower-priority home source. `normalize_user_path` remains the combined entry point for callers that already own execution context. Applications own grammar completion, routing and the point at which resolution is permitted.

The portable grammar preserves Windows verbatim drive prefixes, including their implicit root. Resolution joins an ordinary relative operand to the supplied context before lexical reconstruction: a slash within an existing verbatim context component stays part of that component during parent traversal. Home suffixes append one component at a time. These rules preserve existing Windows admission and normalization on every host; they do not resolve or probe a Windows path on Unix.

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
exist. Until then, read-only `session list`, `server diagnose` and `config check` report
nothing started.
Pipe names keep the `orchbee-` prefix in every environment because each runtime directory derives
its own endpoint id. Data written under earlier non-release locations is not migrated.

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

## Windows durable session catalog

Native Windows named and default Sessions keep one bounded private lifecycle record each,
`sessions/<session>/session.json`, with its writer lock `session.lock` and staging file
`session.pending` beside it and the Session's checkpoints in the same folder. Each record binds the
name, which matches its folder without letter case, and the runtime folder in use to Active,
Crashed, Stopped with a nonzero stop identity, or Deleting. Only current numeric version 1 with all
fields, including the Server/Worker/Host PID and creation-FILETIME registry, is admitted. Other
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
`spawn_session_child` requires the exact registered direct parent for Worker or Host. Each
child is created suspended and released only after its identity is flushed. Explicit confirmed
named-session deletion can replace an unreadable record with its deletion fence only after the
existing owner, endpoint and storage guards pass; ordinary publication cannot overwrite it.
Live Host adoption remains separate.

The [Windows crash-recovery contract](docs/windows-crash-recovery.md) owns guards, interrupted
publication, compatibility and evidence limits. Check the registered bounded model with:

```sh
node scripts/tla/run-models.mjs --models session-crash-recovery
```

## Windows named session lifecycle

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
including `layouts/` and `terminal-output/` entries, releases their exact handles before removal,
retains ancestor pins and removes the record and the emptied Session folder last. Retrying resumes
that deletion; absent state succeeds. It never recursively deletes arbitrary trees or stops a
process. The shared namespace lock intentionally outlives individual sessions. This covers
cooperating current-user invocations and ordinary process interruption; power-loss durability and
inventory recovery after external removal of the runtime base are unclaimed.

## Windows editor documents

`remove_stale_editor_documents(folder, now)` removes the scrollback documents the Terminal Host
wrote to `DataRoot::tmp()` and last modified more than one day before `now`; one written exactly a
day ago stays. It takes only regular files named `scrollback-<UTC time>.txt` or
`scrollback-<UTC time>-<n>.txt`, the time spelled exactly as `time_name` writes it, so a misspelled
time, an editor's swap file or a name of another case stays. It pins the folder and its ancestors
first, refusing a folder that is not private or that is or lies under a link, reads at most 1,024
entries, never follows an
entry that is a link, never creates the folder, and returns how many documents it removed and how
many it could not remove; a missing folder is an empty pass. The Host's
[naming rule](../../apps/terminal-host/README.md#executable-interface) is the other half of this
contract; search for `editor-document-name` before changing either. The
[Worker](../../apps/orchbee-worker/README.md#windows-scrollback-editor-documents) chooses when it
runs.

## Windows diagnostic log

`install_diagnostic_log(role)` is called only by an executable entry point, the Client, Server and
Worker, and `log_event` writes nothing until it has been, so library code that tests exercise never
logs. A record is one JSON line in `logs\<YYYY-MM-DD>.jsonl`, named by local date:
`{"time","level","role","pid","event","fields"}`, with UTC time to the millisecond and at most
eight camelCase fields. Event names and text values are `&'static str` tokens of letters, digits,
`.`, `_` and `-`, at most 64 bytes, and other values are counts and flags; a token outside the
grammar is written as `invalid`. A record therefore cannot carry terminal content, typed input,
prompts, arguments, environment or configuration values, file contents or credentials. Host
diagnostics reach the log only as the Worker's Terminal Host start and failure records.

A writer holds the day's file without write sharing while it appends, waiting up to 500 ms for
another writer, so Desktop Main's exclusive open and the natives admit one record at a time. The
first append of a process on a new day removes daily files dated more than six days earlier; the
destructive request log, its generation, and any other name are never daily files. A record is
admitted while the file plus the record plus a 256-byte reserve stays within
`DAILY_LOG_LIMIT_BYTES`, 7 MiB; the first record refused closes the day with one `log-capped`
record, so the folder holds at most 51 MiB with the destructive log. Every error is a non-event.
Desktop Main writes the same records with role `desktop-main`; search for `daily-diagnostic-log`
before changing the shape, a limit, or the names. POSIX processes do not log until ORC-328.

## Diagnostic bundle redaction

`recent_diagnostic_logs` reads today's file and the six before it without creating anything and
keeps only lines that are exactly records this grammar writes, counting the rest, so a bundle never
carries text a writer did not produce. `redact_settings` writes `config.toml` again from its values,
so its comments are not carried, and `redact_preferences` keeps the preference record's shape; both
replace a value under a command, executable or program key with `<command>`, every argument under
an args or arguments key with `<argument>`, every value under an env, environment or
environmentVariables key with `<environment>`, every value under a key that names a token, secret,
password, API key or credential with `<credential>`, and a settings value that no documented option
covers with `<unrecognized>`. `DiagnosticRedactor` then scrubs every remaining string and object key:
the home folder becomes `<home>`, the value of an `ORCHBEE_*`, `APPDATA`, `LOCALAPPDATA`, `TEMP` or
`TMP` variable becomes `<env:NAME>`, URL user information becomes `<credential>`, and a URL query
becomes `?<query>`. `diagnostic_archive` writes the entries as one stored ZIP archive. The
[CLI command](../../apps/orchbee/README.md#windows-diagnostic-bundle) chooses what a bundle
holds.

## Windows destructive request log

`append_destructive_request` appends one caller-supplied JSON record as one line to
`logs\destructive-commands.jsonl` in the data root. It creates the root, the private `logs`
directory and the file on first use under the state root rules above, and admits existing ones only
when they are private regular files and directories. Concurrent appends serialize on the file's own
lock and give up after 500 ms. The current file never exceeds `DESTRUCTIVE_LOG_ROTATION_BYTES`,
1 MiB: an append that would pass it first copies the current lines over
`destructive-commands.1.jsonl`,
replacing the generation kept there, and empties the current file in place, so at most two
generations remain. Nothing reads the log back. Callers treat every error as a non-event, so a
failed append never blocks or changes their command. The
[CLI release confirmation](../../apps/orchbee/README.md#windows-release-destructive-confirmation)
owns the record fields.

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
| No marker, no socket residue, lifetime free                                | Client exclusively creates `activation.started`, then spawns Server once                     |
| Marker present, lifetime held                                              | Wait for compatible Server + Worker readiness                                                |
| Marker present, lifetime free; or no marker with lifetime held             | RecoveryRequired; never launch                                                               |
| Invalid socket / cohort / permissions, incompatible or ambiguous admission | Explicit failure; no fallback                                                                |
| Launch failure after marker creation                                       | Retain marker; this and later attempts require explicit recovery                             |
| Server + Worker ready                                                      | Release activation lock, read Workspace membership, attach an existing binding when selected |
| First validated frame applied before deadline                              | Enter TUI with the same inherited TTY and prepared Session                                   |
| Client exit / deadline                                                     | Release Client resources; retain Server, Worker, Host, and terminal                          |
| Worker / Host loss                                                         | Unavailable or unknown outcome; never respawn, adopt, or replay                              |

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
started Orchbee in.

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
four executables, including on warm startup.
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

| Claim                                                                 | Owner / refinement                                                  | Evidence                                                               |
| --------------------------------------------------------------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------- |
| LS01 at most one launch attempt per runtime lifetime                  | Exclusive activation marker; Client under activation guard          | LocalSessionStartup model; contention and failed-marker process traces |
| LS02 no implicit recovery or Client-owned termination                 | Persistent marker, Server lifetime guard, dropped Client connection | Model losses/timeouts; disconnect/reopen real process trace            |
| LS03 no Host before authorized Launch; one retained dispatch          | Worker authorization kernel and lazy startup adapter                | Model authorization; Worker invalid/valid Launch process tests         |
| LS04 coherent roles and private rendezvous before effects             | Cohort and ModeCheckedLocalSessionRendezvous                        | Manifest corruption, mode, symlink, and existing lock tests            |
| LS05 startup readiness includes a valid first frame within one budget | Client deadline, TerminalSession projection, same TTY handoff       | Admission/submission deadline tests and bare TTY process tests         |

`formal/tla/LocalSessionStartup.tla` bounds two Clients, one Server launch attempt, and one Host
attempt. It abstracts filesystem checks as admitted facts, abstracts deadline observation as an expiry event, and
checks safety without scheduler fairness. Per-Client timeout fairness supports bounded-attempt
termination only, not service availability. Real kernel locks, peers, hashing, PTYs, and
cancellation are exercised by Linux and macOS tests. Remote, Workbench membership, daemon restart,
automatic cleanup/recovery, persistence across Server loss, and other Unix targets are
outside this contract.

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
Nothing removes a socket another Session left behind.

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
releases only its advisory lock. The runtime directory, durable lock files, socket paths, and any
entries at those paths are never removed by this module.

`observe_*` entry probes expose the same finite names to read-only diagnostics: a
descriptor-relative `fstatat` with `AT_SYMLINK_NOFOLLOW` answers each entry's raw kind,
ownership, mode, size, and link count — the caller owns every verdict. Socket probes look in the
admitted socket folder and answer `Absent` when none exists. `observe_session_lifetime_lock` never
creates the leaf: only an entry already stat-checked as an owned regular `0600` single-link file is
opened, probed with one `try_lock`, and released on the same descriptor — `Held` is a lock fact at
that instant, never a process claim. No observation connects, creates, writes, unlinks, repairs, or
infers process health.

Socket metadata and same-user peer proof are enforced by the managed consumers. Client, Server, and
Worker share this crate, and Server owns the lifetime guard. The primitive does not itself select
application startup policy, retry, remove stale endpoints, or grant recovery authority.

### Modeling and evidence

| Field        | Rendezvous primitive contract                                                                                                                                                                                                                                                                                                                         |
| ------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem      | Provide shared path, permission, lock and attempt-marker admission; applications own launch, readiness and recovery policy.                                                                                                                                                                                                                           |
| Inputs       | A trusted existing exactly normalized absolute base, canonical scope, effective UID, and potentially invalid directory, lock, marker or socket entries.                                                                                                                                                                                               |
| Data         | Stable paths, retained scope descriptor, owned advisory-lock guards, and an immutable empty attempt marker. No PID or process-health inference.                                                                                                                                                                                                       |
| Outputs      | Validated rendezvous or typed failure; Acquired/Contended lock outcome; marker and socket existence with metadata validation. Path getters perform no I/O.                                                                                                                                                                                            |
| Invariants   | Descriptor-relative no-follow traversal; exact final type, owner, mode and regular-file link count; new directory identity matches its inspected inode and its mode reads back. macOS final objects have no extended ACL on a volume that keeps ownership. Existing invalid entries are never repaired or removed. Guard drop only releases its lock. |
| Decisions    | Normalized path and scope syntax, create-versus-existing metadata, lock kind, one nonblocking lock outcome, and exclusive marker creation determine results.                                                                                                                                                                                          |
| Model        | Atomic POSIX primitives without application retry or process observation. LocalSessionStartup composes these facts with Client/Server/Worker state; Client implementation tests exercise the connect-first behavior.                                                                                                                                  |
| Transitions  | Missing scope/lock becomes a validated durable inode. Lock creation and lock acquisition may have different winners. Guard acquisition/drop changes only lock ownership. An absent marker can be committed once and is never cleared.                                                                                                                 |
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

`SessionStore` retains bounded checkpoint bytes in the Session folder,
`<data root>/sessions/<session>`, named by the admitted Session's runtime folder. Its caller supplies
the data root and admitted Session; the Server owns checkpoint validation and recovery. Each Session
has its own store. Private ancestor handles, protected
new-directory ACLs, no-reparse admission and single-link file checks apply. A private `state.lock`
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
with `MoveFileEx(REPLACE_EXISTING)`. Pending bytes never supply recovery authority; failure keeps
prior valid slots and reports any unconfirmed staging cleanup. Unchanged payloads do not consume
history. Raw originals are user recovery data retained until explicit Session deletion or authorized
repair. Other users are excluded by the private ACL; same-user writers respect the lock. The
adapter does not claim directory-entry write-through; the acknowledged pending-file flush and
atomic replacement do not prove power-loss or hardware durability.

The store does not interpret JSON, select a default data directory, relaunch a process, replay an
operation or grant Session restart authority. The Server owns startup/stop use of this primitive;
native process-crash/restart and machine-restart witnesses remain separate evidence obligations.

The native Windows adapter retains activation/lifetime-lock ownership; the explicit
stop record below refines the original immutable attempt marker. It admits an absolute local path only after
rejecting reparse points and retaining directory handles without delete sharing.
The runtime parent and leaf must be owned by the token user with a DACL granting
access only to that user, SYSTEM, or Administrators. Newly created leaves receive
a protected inheritable DACL; existing unsafe entries are rejected, never repaired.
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
can briefly contend with startup; failure never authorizes repair or deletion.
Invalid parents prevent descendant inspection. Tests cover zero creation,
free/held transitions, malformed markers and hard-link rejection.

### Windows explicit-stop activation record

The Windows stop refinement replaces the immutable-attempt-only interpretation
with a bounded record in the same admitted `activation.started` file. Empty bytes
mean Active; the exact ASCII prefix `orchbee-stopped-v1:` followed by a nonzero
32-character lowercase hexadecimal stop request ID means Stopped. Absence remains
distinct. Every other size, prefix or encoding is invalid and grants no restart.
Linux retains its immutable empty-marker contract.

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

## Windows directory primitives

The shared pure `windows_reported_directory` decoder preserves valid terminal-reported drive/UNC observations; callers separately enforce narrower launch path bounds. `directory/windows/initialization.ps1` synchronizes PowerShell process cwd after the original prompt, preserving status and leaving non-filesystem providers unchanged. Its first prompt clears the startup pipeline's own session-history entry by that pipeline's history ID, so `Get-History` in a `-NoExit -Command` shell lists only the user's commands; PSReadLine never reads the startup text. After a line submitted with the `<#orchbee-run:<id>#> ` prefix, the prompt also removes that command's session-history entry. Ordinary Worker shells use this single hook and child-local UTF-8 native-output decoding and outgoing pipeline encoding.

`windows_interactive_powershell_initialization` concatenates that common asset with
`directory/windows/reader.ps1`, which wraps the existing interactive reader with semantic
read-phase markers. While it waits for a line, it swaps in a PSReadLine `AddToHistoryHandler`
that rejects a run-prefixed line and passes every other line to the handler it replaced, then
restores that handler. Orchbee-submitted lines therefore never reach PSReadLine history, its
saved file or Up-arrow recall. The Worker prepares this exact startup for admitted plain
PowerShell argv; the Host build reads the same two assets, in the same order, for injection and
exact recognition. The noninteractive `windows_encoded_command` helper sets only UTF-8 console and
pipeline encoding; it carries no prompt or reader asset, so its typed text stays short.
Neither asset writes a profile or changes an external PowerShell process. Actual 5.1/7 pipeline,
prompt and PTY startup witnesses cover the managed path; arbitrary startup scripts remain outside it.
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

## Windows user settings

The same reader owns `UpdateChannel` (`stable`/`preview`, default stable) and default/file provenance. An invalid file value defaults that leaf and enters the warning record; the documented environment boundary remains fatal. Native `channel show` consumes the effective preference offline; it does not infer installed release identity or run an updater.

The shared bounded settings reader and pure Windows user-path adapter, which normalizes resolved relative and tilde operands lexically while keeping absolute literals verbatim, serve CLI configuration, common Client activation and direct managed Server startup. They own fatal file/path/environment admission, field-local fallback to existing safe defaults, resolved restore policy and a deterministic warning record; applications own wire projection, activation commit and private child arguments. The load result keeps effective settings and diagnostics together. Diagnostics are deduplicated before display truncation, sorted by field and reason, capped at 128 entries with an exact omitted count, and never retain source values or commands. See [configuration and precedence](../../apps/orchbee/docs/windows-cli-configuration.md). The pinned `toml` dependency parses at this file boundary. The reader has no Workbench dependency.

The pure `user_settings_path` resolver exposes the same configuration-file location for companion assets without opening the file or applying settings. It preserves explicit `ORCHBEE_CONFIG_FILE` precedence over `config.toml` in the data root; a missing home leaves the location absent, and an invalid `ORCHBEE_HOME` is a settings error. Detection rules, preferences and other records follow the data root, never the settings file's folder.

## Windows private update cache

`PrivateCache` owns pinned private ancestors, no-follow regular single-link files and a
cross-process writer lock for one named record in a folder. Applications retain logical
filenames, content validation, versions and policy. Missing descendants are created only below an
admitted private parent; unsafe existing data is refused without repair. Bounded reads preserve
absence. Replacement flushes a complete staging file, then atomically replaces the admitted leaf.
Removal deletes only an admitted leaf under the writer lock; an absent leaf is already removed.
The caller names the record: `<record>.lock` beside it is the persistent writer lock and
`<record>.pending` the one owned staging leaf, removed after settlement or recovered under the next
exclusive lock. Both names are reserved and refused as leaves; another record in the same folder
locks independently. The default settings file keeps its lock in `internal/config.lock`, so its
folder gains only `config.pending` while a write runs. `prepare` creates a private folder without a
lock, for a folder whose contents another program owns. Cleanup failures are returned, never converted into successful publication.
Applications own what each record holds; this adapter alone implements no update policy.

`read_existing` pins and validates an existing private directory tree and performs a bounded leaf read without creating directories, lock files or staging files. It serves startup/reload status inspection; missing cache paths remain absent.

Windows settings also admit `[update] manifest_check` (default true) and retain default/file provenance plus explicit-location status. Invalid supported leaves default independently; invalid key actions remain explicit disabled overrides, while valid chords from the same action survive. `read_user_settings_at` reads a complete candidate at a frozen path, reapplies the strict resume environment override, preserves missing-default/missing-explicit behavior and creates no files. Applications retain atomic settings/diagnostics publication, the authoritative Server startup baseline and automatic scheduling policy.

## Windows keybinding configuration repair

The Windows settings owner exposes bounded offline keybinding reset with semantic preservation, exclusive-file admission, a flushed private backup and explicit recovery results. The application owns CLI receipts and localization; [the configuration contract](../../apps/orchbee/docs/windows-cli-configuration.md#reset-custom-keybindings) owns policy, failure behavior and backup lifecycle. The library does not reload or launch a process.

The same keys owner encodes and validates canonical `[keys]` profiles for live Client publication, capped at 64 KiB and 144 expanded triggers per action. User files keep the 16-alternative bound; Server and Client own publication and installation.

The Windows plugin catalog boundary supplies bounded manifest parsing, normalized identifiers, platform inheritance, source availability and atomic private user registration for native plugin management. Reads do not create storage; unavailable sources retain explicitly marked inspection metadata. Each plugin's folder is spelled from its ID, recorded in the registry, and keeps its `config` and `data` folders after unlink. The shared Windows command owner provides bounded output capture and descendant settlement for Worker commands and offline provisioning; applications own execution policy. Managed provisioning owns serialized source download/build/commit/cleanup, including update from a recorded source, through an application-supplied executor, with one strict version 1 catalog including required provenance/recovery fields; see the [native plugin contract](../../apps/orchbee/docs/windows-plugins.md).

## Windows update channel writes

`set_update_channel` is the shared explicit settings writer: it validates the entire current file, preserves TOML syntax with the already pinned toml_edit parser, serializes cooperating writers, rechecks for edits, and flushes an atomic private replacement. It may create the selected absent file. It never downloads or rolls back a saved preference after a caller's update failure.

`Cohort::artifact_sha256` retains each executable digest from successful admission. Installation provenance compares its declared digests against these observations, so a cohort change between reads cannot be presented as the previously admitted release.

## Windows sidebar presentation settings

The shared settings owner admits separate `ui.sidebar.agents` and `ui.sidebar.spaces`
layouts. Token styling/raw-value first-match rules have one common admission owner;
family-specific wrappers reject unsupported tokens and non-text rules. Each closed
canonical profile caps at 32 KiB independently. Workspace layouts use rows/row_gap;
Agent layouts retain provider overrides. Startup, CLI checks and Session reload all
consume this owner. See the [user guide](../../apps/orchbee/docs/windows-cli-configuration.md#windows-workspace-sidebar-layouts).

## Windows handoff pipe identity

An explicit Worker handoff derives a separate same-user named pipe from the already admitted
runtime hash and its nonzero 128-bit ticket. `handoff_endpoint_path` never creates a filesystem
leaf or grants authority to an arbitrary path. Applications own ticket validation, target cohort
admission, the standby lease and process/Server ownership; ordinary client/worker/Host endpoints
keep their existing identity. Native pipe tests verify separate scope/ticket addresses and bounded
names even for long Unicode runtime input. [The Worker contract](../../apps/orchbee-worker/docs/windows-server-handoff.md)
owns the connection transition.
