# Data Root Layout

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                       |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | The product owner accepted the [decisions](#accepted-decisions) on 2026-11-08 (ORC-381 item 2, ORCL-29), chose the [tree](#tree) with its `internal/` folder on 2026-11-08, and did not veto the [accepted choices](#accepted-choices), which stay revisable. ORC-397 proposes the names, migration mechanism, and phases. Windows first; macOS and Linux follow with ORC-328.                              |
| Owner             | Maintainers of `crates/local-runtime`, `apps/orchbee`, `apps/orchbee-server`, `apps/orchbee-worker`, `apps/terminal-host`, `apps/desktop`, and `crates/agent-integration-writer`                                                                                                                                                                                                                            |
| Current authority | [Data root](../../crates/local-runtime/README.md#data-root), [Windows named runtime identity](../../crates/local-runtime/README.md#windows-named-runtime-identity), [Windows CLI configuration](../../apps/orchbee/docs/windows-cli-configuration.md#file-and-validation), the [Desktop data root](../../apps/desktop/docs/data-root.md), and [Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries) |
| Exit condition    | Delete this proposal once every phase ships and its rules live in those owners.                                                                                                                                                                                                                                                                                                                             |

## Accepted decisions

- Every environment keeps all of its data under one data root, `<home>/.<stem>`: `~/.orchbee` for
  release on Windows, macOS, and Linux, and `~/.ob-dev-<digest>` or `~/.ob-test-<digest>` otherwise.
  Program files stay in each system's install location.
- The root holds subdirectories by purpose whose names a person can read; no name inside it is a
  hash.
- Whatever Orchbee puts on a machine is necessary, complete, and free of redundancy, and a person
  browsing the folders understands them (ORCL-29).
- The top level holds only what a person reads, edits, or keeps: settings, Sessions, logs, and
  detection rules they write. Everything Orchbee manages for itself goes under `internal/`
  (product owner, 2026-11-08).
- From the first public release, a persisted format change migrates automatically after a backup; a
  failed migration keeps the old file and tells the person. Before then nothing migrates, including
  today's release locations and pre-release data.
- Uninstall keeps user data by default and offers to delete all of it.
- Logs go to `logs/`, one file per day, kept 7 days, under a total size cap. A diagnostic bundle
  gathers versions, system information, settings without secrets, and recent logs.

## Tree

```text
~/.orchbee/
├── config.toml
├── sessions/<session>/
│   ├── session.json
│   ├── checkpoint.json, checkpoint.previous-1.json, checkpoint.previous-2.json
│   ├── checkpoint.restore-notice
│   ├── tui-position.json
│   ├── layouts/<time>.json
│   └── terminal-output/<number>.bin
├── logs/<date>.jsonl, logs/destructive-commands.jsonl
├── agent-detection/<provider>.toml
└── internal/
    ├── version.json
    ├── state/
    │   ├── onboarding.json
    │   ├── machines.json
    │   ├── desktop-window.json
    │   └── desktop-tray.json
    ├── integrations/agent-observation/<version>/
    ├── desktop/
    ├── backups/<date>-<reason>/
    ├── cache/agent-detection/
    ├── machines/hub.lock
    ├── runtime/<session>/
    ├── tmp/
    └── plugins/
        ├── registry.json
        ├── source.pending/
        └── <plugin id>/{config,data,source/<version>}/
```

| Entry                   | Holds                                                                                                      | Default uninstall              | Safe to delete by hand                              |
| ----------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------ | --------------------------------------------------- |
| `config.toml`           | Settings the person writes                                                                                 | Kept                           | No                                                  |
| `sessions`              | One folder per Session: lifecycle record, checkpoints, earlier layouts, kept output, TUI position          | Kept                           | Only through `session delete`                       |
| `logs`                  | Daily logs and destructive-command records                                                                 | Removed                        | Yes                                                 |
| `agent-detection`       | Detection rule overrides the person writes; absent unless created                                          | Kept                           | Yes; bundled rules return                           |
| `internal`              | Everything below; a person has no reason to open it                                                        | Partly removed, as below       | No                                                  |
| `internal/version.json` | The layout version, and the newest Orchbee version that used the root so a refusal can name it             | Kept                           | No                                                  |
| `internal/state`        | Device records: onboarding dismissal, saved machines, Desktop window geometry and notification-area notice | Kept                           | No                                                  |
| `internal/integrations` | The Agent observation Capsule that Agent configurations run                                                | Removed with the Agent entries | Only through `agent integration disable`            |
| `internal/desktop`      | Electron `userData`, in Chromium's own layout                                                              | Kept                           | While Desktop is closed; resets Desktop preferences |
| `internal/backups`      | Copies taken before a migration or a reset                                                                 | Kept                           | Yes                                                 |
| `internal/cache`        | Downloaded detection rules                                                                                 | Removed                        | Yes; rebuilt on demand                              |
| `internal/machines`     | The machine hub's single-instance lock, whose location freezes at the first public release                 | Removed                        | While no hub runs                                   |
| `internal/runtime`      | Locks, markers, Host registrations, and frame files of live Sessions                                       | Removed                        | Only a stopped Session's folder                     |
| `internal/tmp`          | Scrollback documents an earlier build left; nothing writes here                                            | Removed                        | Yes                                                 |
| `internal/plugins`      | Plugin registry, managed sources, and each plugin's own configuration and data                             | Kept                           | No                                                  |

- Only `config.toml`, `agent-detection/*.toml`, and a plugin's `config/` folder are meant for hand
  editing. Every other file is internal, and Orchbee refuses an edited record instead of guessing.
- Nothing is created until it has content, and reading never creates a file or directory. A native
  writer that creates the root or `internal/` writes `internal/version.json` in the same step, so a
  root that holds anything Orchbee wrote holds it.
- Desktop's renderer keeps keyboard overrides and terminal appearance in browser storage, so
  `internal/desktop/` is not a cache. Chromium keeps its HTTP, code, and GPU caches beside that
  storage: Electron 42 ignores `--disk-cache-dir`, and moving `sessionData` moves browser storage
  with the caches, so they stay in `internal/desktop/` and `internal/cache/` holds no Desktop data.

## Naming

- Fixed names are lowercase English words joined by hyphens.
- Variable names come from a name the person chose (Session, plugin id), a version, a local date
  `YYYY-MM-DD`, a UTC time `YYYY-MM-DDTHH-MM-SSZ`, or a sequence number. When such a name is already
  taken by different content, the next free `-2`, `-3`, and so on is appended.
- Format versions, digests, random identifiers, and account SIDs live inside files, never in names;
  `window-state-v1.json`, for example, becomes `internal/state/desktop-window.json` with its version
  inside.
- A writer lock and a staging file sit beside the record they protect, named after it with `.lock`
  and `.pending`: `onboarding.lock` and `onboarding.pending` beside `onboarding.json`. A record
  made of several files names them after its index file, as `catalog.lock` beside
  `internal/cache/agent-detection/catalog.json`. Both are internal; delete neither while Orchbee
  runs.
- `config.toml` keeps the top level clean: its writer lock is `internal/config.lock`, and its staging
  file `config.pending` exists beside it only while a write runs. `sessions/` does the same: the lock
  that serializes Session activation and deletion is `internal/sessions.lock`, while each Session's
  `session.lock` and `session.pending` sit beside its `session.json`.
- A Session folder takes the Session name. Session names stay 1–64 ASCII letters, digits, `.`, `_`,
  and `-`, and additionally neither start nor end with `.` nor equal a Windows device name in any
  letter case (`con`, `prn`, `aux`, `nul`, `com1`–`com9`, `lpt1`–`lpt9`, with or without an
  extension), so one name gives the same folder on every file system. Letter case does not
  distinguish Sessions ([accepted choice 2](#accepted-choices)).
- A plugin folder spells the plugin id, and a managed source folder spells the manifest version:
  ASCII letters, digits, `.`, `_`, and `-` stay, every other run becomes one `-`, leading and
  trailing `.` and `-` go, the name stops at 64 characters, and a Windows device name gains `_`
  after its stem. Letter case does not distinguish folders. `registry.json` records the folder each
  plugin received, so a folder never passes to another plugin, and no plugin receives the store's
  own names. An install fetches into `source.pending/` and moves the source to `source/<version>/`
  before it builds, so every name is known before anything is built; updating a plugin whose
  version did not change takes the next free suffix.

## Runtime files

- A Session's runtime folder is `internal/runtime/<session>/` in every environment, created private
  on activation. A stopped Session's folder may be removed; the next start recreates it.
- An explicit `ORCHBEE_RUNTIME_BASE` keeps its meaning and replaces `internal/runtime/`.
  `session.json` records the runtime folder in use, and a start through another folder is refused
  while the recorded one is live.
- Windows named pipes keep their names; the endpoint id derived from the canonical runtime folder
  keeps them distinct per Session and environment.
- Unix sockets cannot live in that folder: `sun_path` holds 104 bytes on macOS and 108 on Linux,
  which a home path plus a 64-character Session name exceeds. Sockets go to a private per-user
  directory, `/tmp/<stem>-<uid>/` on Linux and `/private/tmp/<stem>-<uid>/` on macOS, as
  `<endpoint id>-<role>.sock`, where the endpoint id is 32 hex digits of a hash of the canonical
  runtime base and the lowercase Session name; with roles of at most seven characters, the longest
  such path is 94 bytes. The directory's parent must belong to root and carry the sticky bit; the
  directory is created with mode 0700, and one with another owner or mode is refused, never
  repaired. The Server holds a shared lock on it while the Session lives. Locks and markers stay in
  `internal/runtime/<session>/`, out of reach of temporary-directory cleaners.
- Because that folder then survives a machine restart on macOS and Linux, POSIX activation adopts
  the [Windows crash-recovery contract](../../crates/local-runtime/docs/windows-crash-recovery.md)
  with ORC-328.

## Environments

- Development and test roots hold the same tree; only the root name differs. A development root
  also holds `internal/development-checkout.json`, the Desktop checkout marker formerly named
  `development-scope-v1.json`.
- `ORCHBEE_HOME` names another data root for every environment that reads it
  ([accepted choice 1](#accepted-choices)); the stem still names files, pipes, and the environment.
- POSIX socket directories take the stem, for example `/tmp/ob-dev-<digest>-<uid>/`.
- A test that writes state points the data root at its own temporary directory, never at the
  developer's roots. A phase that changes how test builds name or locate state runs every affected
  test binary in full and checks that the real home and the earlier release locations gained
  nothing.

## Locations replaced

Data at the earlier locations is neither migrated nor deleted. Nothing reads it once its phase
ships, and the person removes it.

| Earlier location                                                                                                                                                                                                                                                                     | New location                                                                                                                                                                                                                                                                                                                                      | Owner                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `%APPDATA%\Orchbee\config.toml`; macOS `~/Library/Application Support/Orchbee/config.toml`; Linux `$XDG_CONFIG_HOME/Orchbee/config.toml`                                                                                                                                             | `config.toml`                                                                                                                                                                                                                                                                                                                                     | `user_settings` in `crates/local-runtime`            |
| `preferences-<sha256>/preferences.json` beside the configuration file                                                                                                                                                                                                                | Not moved: its settings live in `config.toml`, and a leftover file, there or at `internal/state/preferences.json`, is ignored and left in place                                                                                                                                                                                                   | `user_settings` in `crates/local-runtime`            |
| `config.toml.native-state/onboarding.json`                                                                                                                                                                                                                                           | `internal/state/onboarding.json`                                                                                                                                                                                                                                                                                                                  | `onboarding` in `workbench-client`                   |
| `machines/catalog.json` beside the configuration file                                                                                                                                                                                                                                | `internal/state/machines.json`                                                                                                                                                                                                                                                                                                                    | `machines::store` in `workbench-client`              |
| `plugins/` beside the configuration file, then `internal/plugins/` with the same inner layout: `registry.json`, `config/p-<id digest>`, `state/p-<id digest>`, `managed/checkouts.lock`, `managed/i-<installation id>/checkout`; that `registry.json` reads as invalid until deleted | `internal/plugins/registry.json`, which records each plugin's folder, `internal/plugins/<id>/config`, `internal/plugins/<id>/data`, `internal/plugins/<id>/source/<version>`                                                                                                                                                                      | `plugins::store`                                     |
| `agent-detection/<provider>.toml` beside the configuration file                                                                                                                                                                                                                      | `agent-detection/<provider>.toml`                                                                                                                                                                                                                                                                                                                 | `agent_screen::loading` in `orchbee-server`          |
| `agent-detection/remote/<catalog URL digest>/` beside the configuration file                                                                                                                                                                                                         | `internal/cache/agent-detection/`, with the catalog URL recorded inside; another URL discards it                                                                                                                                                                                                                                                  | `agent_screen::updating` in `orchbee-server`         |
| `%LOCALAPPDATA%\Orchbee\session-catalog\<runtime base digest>\<scope>.json` and `<scope>.record.lock`                                                                                                                                                                                | `sessions/<session>/session.json` and `session.lock`                                                                                                                                                                                                                                                                                              | `session_catalog`, `durable_sessions`                |
| `%LOCALAPPDATA%\Orchbee\native-sessions\<runtime digest>\`: slots, `state.lock`, `checkpoint.restore-notice`, `checkpoint.corrupt-<hex>.raw`, `layout-history/layout-<hex>.json`, `terminal-output/output-<hex>.bin`                                                                 | `sessions/<session>/`: slots, `state.lock`, `checkpoint.restore-notice`, `checkpoint.corrupt-<time>.raw`, `layouts/<time>.json`, `terminal-output/<number>.bin`                                                                                                                                                                                   | `session_store`, `layout_history`, `terminal_output` |
| `%LOCALAPPDATA%\Orchbee\tui-position\<endpoint id>\position.json`, never removed                                                                                                                                                                                                     | `sessions/<session>/tui-position.json`, removed with its Session                                                                                                                                                                                                                                                                                  | `startup_preference::store` in `workbench-client`    |
| `%LOCALAPPDATA%\Orchbee\audit\destructive.jsonl` and `destructive.1.jsonl`                                                                                                                                                                                                           | `logs/destructive-commands.jsonl` and `destructive-commands.1.jsonl`                                                                                                                                                                                                                                                                              | `destructive_log`                                    |
| `%TEMP%\orchbee-native-<SID digest>`, `%TEMP%\orchbee-session-<digest>`, POSIX `/tmp/orchbee-native-<uid>`                                                                                                                                                                           | `internal/runtime/<session>/`, with POSIX sockets under [Runtime files](#runtime-files)                                                                                                                                                                                                                                                           | `environment`, `rendezvous`, `session_name`          |
| `~/.orchbee/agent-observation-adapters/<payload SHA-256>/` in every environment, never removed                                                                                                                                                                                       | `internal/integrations/agent-observation/<version>/` in each environment's root, `<version>-2` and on when a name holds another payload; a version no Agent entry names is removed. The old folder is never removed: its entries are release's, doctor reports them as `stale`, and release's explicit enable or disable replaces or removes them | `agent-integration-writer`                           |
| The data root itself as Electron `userData`                                                                                                                                                                                                                                          | `internal/desktop/`                                                                                                                                                                                                                                                                                                                               | Desktop main `entry`                                 |
| `window-state-v1.json` in Electron `userData`                                                                                                                                                                                                                                        | `internal/state/desktop-window.json`                                                                                                                                                                                                                                                                                                              | Desktop `window-state`                               |
| `development-scope-v1.json` in a development root                                                                                                                                                                                                                                    | `internal/development-checkout.json`                                                                                                                                                                                                                                                                                                              | Desktop `prepareDesktopDataRoot`                     |
| `.orchbee-update.lock` and `.orchbee-update.pending` in each private store                                                                                                                                                                                                           | `<record>.lock` and `<record>.pending` beside each record; `internal/config.lock` for `config.toml`                                                                                                                                                                                                                                               | `private_cache`                                      |
| `config.toml.bak-keys-…` beside the configuration file                                                                                                                                                                                                                               | `internal/backups/<date>-reset-keys/config.toml`                                                                                                                                                                                                                                                                                                  | `user_settings::reset_keys`                          |

## Outside the data root

- Program files stay in each system's install location, `%LOCALAPPDATA%\Programs\Orchbee` on
  Windows, where version directories take product-version names under `versions\` and
  `active.json` names the active one, as the
  [shared runtime store](versioning-and-upgrade.md#shared-runtime-store) requires.
- Agent configurations keep Orchbee's entries where each Agent reads them. The
  [integration writer](../../crates/agent-integration-writer/README.md) is the only owner of those
  entries — plugin registrations through an Agent's own CLI, guarded plugin and extension files, and
  hook entries in provider configuration files — and every uninstall runs its disable for each
  integration. A `.orchbee-agent-backup-…` file beside a provider configuration exists only while a
  change runs, unless restoring it after a failed change also fails.
- Installer and system entries: the user `PATH` entry that release installation adds, Desktop
  shortcuts and uninstall registration from the installer ORC-381 selects, and the files macOS keeps
  under `~/Library` for each bundle identifier. Uninstall removes those that Orchbee or its
  installer created.
- Git Worktrees and other content Orchbee creates at the person's request belong to the person; no
  uninstall touches them.
- Orchbee writes nothing else to the system temporary directory; POSIX sockets are the only
  exception.

## Uninstall

- The default uninstall stops the environment's Sessions after the person confirms, runs the
  integration disable, removes program files and installer entries, and removes `logs/`,
  `internal/integrations/`, `internal/cache/`, `internal/runtime/`, and `internal/tmp/`. Everything
  else stays, so a reinstall finds settings and Sessions again; the person enables Agent
  integrations again.
- Deleting all data additionally removes every top-level entry the Tree names, `internal/version.json`
  last, then the root when nothing else is in it.
- Uninstall acts only on its own environment's root, and only when the root carries the layout
  record: a root that holds anything uninstall would remove but no readable `internal/version.json`
  refuses the run before anything changes, naming the folder and the variable that chose it.
  Anything else found in the root stays and is reported.
- One CLI command performs these steps; each platform's uninstaller runs it, and ORC-381 decides how
  macOS, which has no uninstaller, offers it.

## Logs and diagnostics

- The Client, Server, Worker, and Desktop Main append whole JSON Lines records to
  `logs/<YYYY-MM-DD>.jsonl`, named by local date; Host diagnostics reach it through the Worker. A
  record holds time, level, process role, PID, event, and bounded fields.
- No record contains terminal content, typed input, command arguments, environment values,
  configuration values, or credentials.
- When a process opens a day's file, it removes daily files dated more than six days earlier. A
  day's file stops growing at 7 MiB after one `log-capped` record, so the folder holds at most
  51 MiB, the destructive log included.
- `destructive-commands.jsonl` keeps the size rotation and fields of the
  [destructive request log](environment-isolation.md#destructive-request-log); daily pruning skips it
  and its rotated generation.
- A CLI command and a Desktop menu action write the diagnostic bundle as one archive where the
  person chooses; nothing stays in the data root. It holds product and build versions of every
  component and running process, the operating system, architecture, locale, and terminal,
  `config check` output, `config.toml` with every command, argument, environment value, home path,
  and URL credential replaced by a placeholder, the Session list with states, integration status,
  and the logs of the last 7 days. It excludes checkpoints,
  layouts, terminal output, plugin data, and saved machines.
- Phase 6 delivers this on Windows as `orchbee diagnostics create --output <path>`, a stored ZIP
  archive. It also replaces settings values no documented option covers and URL queries, leaves
  the destructive log out, and reports a running Session's version as whether it runs this build's
  Server. The Desktop action sits in the Help menu and the packaged build's Jump List; the TUI has
  no entry of its own.

## Persisted data versions and migration

Until the first public release, [Data boundaries](../../docs/ARCHITECTURE.md#data-boundaries)
applies and nothing migrates. From the first public release:

| Data                                                                                           | Version                           | On a format change                                                                                                                                                                          |
| ---------------------------------------------------------------------------------------------- | --------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `config.toml`                                                                                  | None                              | Never migrated: a renamed key keeps its old name as an alias with a diagnostic, and unknown keys warn and are ignored ([compatibility](../../crates/local-runtime/README.md#compatibility)) |
| `agent-detection/*.toml`                                                                       | Each rule's `min_engine_version`  | Never rewritten; an override this version cannot read is reported, and the bundled rule applies                                                                                             |
| `internal/version.json`                                                                        | `layout`                          | Entries are only added. A release that must move one raises `layout` and moves it only while no Orchbee process of the environment runs                                                     |
| `internal/state/*.json`, `internal/plugins/registry.json`, `session.json`, `tui-position.json` | A top-level integer `version`     | Migrated                                                                                                                                                                                    |
| Checkpoint slots, the restore notice, earlier layouts, kept terminal output                    | The frame header or record prefix | Migrated                                                                                                                                                                                    |
| `internal/plugins/<id>/config/`, `internal/plugins/<id>/data/`                                 | The plugin's own                  | The plugin's own                                                                                                                                                                            |
| `internal/cache/`                                                                              | Each owner's version              | Discarded and rebuilt; never migrated or backed up                                                                                                                                          |
| `internal/runtime/`                                                                            | Protocol generations              | Not migrated; [versioning and upgrade](versioning-and-upgrade.md) decides compatibility                                                                                                     |
| `internal/integrations/`                                                                       | The Capsule manifest              | Replaced by the integration writer, never migrated                                                                                                                                          |
| `internal/desktop/`                                                                            | Chromium's own                    | Chromium's own                                                                                                                                                                              |
| `logs/`, `internal/backups/`, `internal/tmp/`, plugin sources                                  | None                              | Never migrated                                                                                                                                                                              |

- A build refuses to open a root whose `layout` is higher than its own, before any other read.
- One migration module in `crates/local-runtime` owns every native step and the backup, retention,
  and failure behavior. Each store calls it when it opens a record, under the store's own writer
  lock. `internal/state/desktop-window.json` and `internal/state/desktop-tray.json` are the only
  records Desktop Main writes; their owners apply the same rules.
- A step is a pure function from version N to N+1. The result must pass the current strict parser
  before the store replaces the file through its flushed atomic replacement.
- Before the first replacement, the module copies the original into
  `internal/backups/<YYYY-MM-DD>-upgrade-<product version>/` at the same relative path and flushes
  it; all records that one product version migrates share that folder. Orchbee keeps the three
  newest upgrade folders and deletes older ones when it creates another. A reset the person requests
  copies the file into `internal/backups/<YYYY-MM-DD>-<reset name>/` first, and those folders stay
  until the person deletes them.
- A failed step changes nothing: the original stays at its path, and the store treats it as a record
  in a newer format until a later release migrates it or the person resets it.
- A record in a newer format is never rewritten, so rollback stays possible. The store works as it
  does without the record and keeps changes to it in memory only; each store's README states that
  consequence.
- Each process that needs such a record shows its message once, logs it, and `config check` lists
  it. The English source texts, shipped with `zh-CN` through the localization system, are "Orchbee
  couldn't update {item} from an earlier version and left it unchanged. {consequence}" and "{item}
  was saved by a newer version of Orchbee, so this version won't change it. {consequence}". A
  status line too narrow for the message shows its shorter form, "{item} couldn't be updated" or
  "{item} is from a newer Orchbee". The TUI's notice is persistent, since the record stays
  unusable: it lasts until the person's next action.
- Each record migrates only in a process that writes it: Session store records in the Server that
  holds the store, `tui-position.json` in the TUI, and device records in the first newer process
  that opens them. A Client therefore never rewrites what an older running Server owns. When an
  older running process can no longer read a migrated record, the release raises that component's
  `recommendedFloor`, and the [restart decision](versioning-and-upgrade.md#restart-decision) offers
  Restart Session.
- Records that two installed versions read at once, `internal/state/*.json`,
  `internal/plugins/registry.json`, and `session.json`, follow
  [message evolution](versioning-and-upgrade.md#message-evolution): strictly parsed known fields plus
  one declared extension object, so an added optional field needs no migration.

The first public release already has:

1. `internal/version.json` with `layout` 1 and the refusal of a higher layout.
2. A version in every persisted record at the place above, and none in any file name.
3. A distinct newer-format outcome in every reader; no automatic repair replaces a newer-format
   record, and a reset the person requests backs it up first.
4. The migration module with its backup folders, retention, and localized messages, exercised by
   tests through a synthetic step.
5. The extension object in each record that two installed versions read at once.

## Delivery phases

Each phase merges on its own and compiles for macOS and Linux, so its hand-back includes
`cargo clippy --target x86_64-unknown-linux-gnu --all-targets -p <crate> -- -D warnings` for every
crate it lists. "Real-home check" means running every affected test binary in full and confirming
that the real home and the earlier release locations gained nothing.

| Phase                      | Changes                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               | Crates                                                                                             | Tests                                                                                                  | Depends on                       |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------ | -------------------------------- |
| 1. Root and device records | One `data_root` owner derives every path; `environment`, `user_settings` with `reset_keys` backups, `user_preferences`, `private_cache` lock names, and `destructive_log`; `onboarding` and `machines`; `agent_screen`; the plugin root moves to `internal/plugins/` with its inner layout unchanged; `internal/version.json`; `ORCHBEE_HOME`; Desktop main `entry`, `prepareDesktopDataRoot`, and `window-state`; CLI help, failure, and configuration text; owning READMEs. Session stores keep their release location, and a development or test root keeps them under `internal/` | `orchbee-local-runtime`, `orchbee`, `orchbee-workbench-client`, `orchbee-server`                   | Real-home check; Desktop data-root and window-state tests                                              | ORC-391 merged                   |
| 2. Plugins                 | `plugins::store` and provisioning: `internal/plugins/registry.json`, per-plugin `config`, `data`, and `source/<version>`                                                                                                                                                                                                                                                                                                                                                                                                                                                              | `orchbee-local-runtime`, `orchbee`                                                                 | Plugin store and provisioning tests                                                                    | 1                                |
| 3. Sessions by name        | Session catalog, durable sessions, session store, layout history, terminal output, Windows rendezvous, case-insensitive Session name rules, TUI position; `sessions/<session>/`; Windows release runtime in `internal/runtime/<session>/`; checkpoint and crash-recovery references                                                                                                                                                                                                                                                                                                   | `orchbee-local-runtime`, `orchbee`, `orchbee-workbench-client`, `orchbee-server`, `orchbee-worker` | Real-home check; `WindowsCheckpointStorage` and `SessionCrashRecovery` models where their files change | 1, ORC-391 (Session name help)   |
| 4. Capsule location        | `internal/integrations/agent-observation/<version>/` per root, Agent doctor, removal of Capsules no entry names, both Agent skill texts; the Capsule-root part of [environment isolation](environment-isolation.md#delivery-slices) slice 4, delivered with it: the owner record `bundle/scripts/environment.json`, entries owned by the environment whose Capsule they name, and the hook-time environment check. Doctor reports entries naming the old folder as `stale` and another environment's as `other-environment`; only release changes the old entries                     | `orchbee-agent-integration-writer`, `orchbee`                                                      | Real-home check; provider conformance and effect corpora                                               | 1                                |
| 5. Temporary folder        | `internal/tmp/` keeps documents an earlier build left until a default uninstall removes the folder; no current process writes there                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `orchbee-local-runtime`                                                                            | Real-home check                                                                                        | 1                                |
| 6. Logs and diagnostics    | Shared log writer with retention and cap, Desktop Main logging, the bundle command and Desktop action with localized text                                                                                                                                                                                                                                                                                                                                                                                                                                                             | `orchbee-local-runtime`, `orchbee`, `orchbee-server`, `orchbee-worker`                             | Real-home check; retention, cap, and redaction rules                                                   | 1, ORC-391 (help)                |
| 7. Versions and migration  | Migration module, layout refusal, the newest version kept current in `internal/version.json`, newer-format outcomes, extension objects, backups, retention, and messages in every store                                                                                                                                                                                                                                                                                                                                                                                               | `orchbee-local-runtime`, `orchbee`, `orchbee-workbench-client`, `orchbee-server`                   | Synthetic-step migration with backup, newer-format refusal, failure keeping the original               | 1–3, 6, ORC-391 (`config check`) |
| 8. Uninstall               | The cleanup command in both modes, integration disable for every integration, `PATH` entry removal, installer hooks                                                                                                                                                                                                                                                                                                                                                                                                                                                                   | `orchbee`, `orchbee-agent-integration-writer`                                                      | Uninstalling the test package on Windows in both modes, checking what stays                            | 1, 4, ORC-381 installer, ORC-391 |

Phase 8 is built on Windows: `orchbee uninstall` in both modes and the uninstaller helper
`--installer-uninstall-v1`; the platform uninstaller that runs the helper comes with the installer.

ORC-328 has moved POSIX runtimes into the root, named POSIX cohort files by the environment stem,
added the socket directory, and replaced the ext4 and tmpfs list with admission by behaviour; the
Windows recovery model follows.

## Accepted choices

The product owner saw these choices on 2026-11-08 and did not veto them; each stays revisable until
the phase that applies it ships.

1. Moving the whole data folder: `ORCHBEE_HOME` moves the whole root, as `CODEX_HOME` does, and
   `ORCHBEE_CONFIG_FILE` names only another settings file; tests use `ORCHBEE_HOME`. The rejected
   alternative made the folder of an explicit `ORCHBEE_CONFIG_FILE` the whole root.
2. Letter case in Session names: `Work` and `work` are one Session on every system, so
   `session attach work` finds `Work`, which keeps the spelling it was created with. The rejected
   alternative kept names case-sensitive and refused creating `work` while `Work` exists.
3. Unix sockets live in `/tmp/<stem>-<uid>/`, the only Orchbee files outside the data root, because
   a socket path must fit in 104 bytes. The rejected alternative, short hashed names under
   `internal/runtime/`, breaks the readable-name rule and still fails for long home paths.
4. Every uninstall removes Orchbee's Agent integration entries and the Capsule, even when it keeps
   data. The rejected alternative kept them, so Agents keep running hooks for a missing Orchbee
   until it returns.
5. The default uninstall removes logs with the caches.
6. Each day's log stops at 7 MiB, capping the log folder at 51 MiB, and the three newest upgrade
   backup folders are kept.

## Acceptance

- On Windows, a release Client, Session, and Desktop write only under `~/.orchbee`, the install
  location, the entries in [Outside the data root](#outside-the-data-root), and Windows' own pipe
  namespace; `%APPDATA%`, `%TEMP%`, and `%LOCALAPPDATA%` outside `Programs\Orchbee` gain nothing
  named Orchbee.
- The top level of a root holds only `config.toml`, `sessions/`, `logs/`, `agent-detection/`, and
  `internal/`.
- Every name under the root reads as words, versions, dates, times, Session names, plugin ids, or
  sequence numbers.
- A development checkout and the test package produce the same tree under their roots.
- The default uninstall leaves exactly the kept entries, and deleting all data leaves no root unless
  the root holds items that are not Orchbee's, which stay.
- A record from the previous format version migrates with a backup; a failed step leaves the
  original unchanged and shows the message; a newer record is refused and left unchanged.
