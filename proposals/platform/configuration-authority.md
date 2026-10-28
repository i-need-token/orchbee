# Configuration Authority

| Field              | Value                                                                                                                                                                                                                                         |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status             | Accepted, 2026-10-24                                                                                                                                                                                                                          |
| Owner              | Maintainers of the native Client, Server, Worker, and Settings boundaries                                                                                                                                                                     |
| Current authority  | [`windows-cli-configuration.md`](../../apps/orchbee/docs/windows-cli-configuration.md), [`native-settings.md`](../../apps/orchbee/docs/native-settings.md), [`user-preferences.md`](../../crates/workbench-protocol/docs/user-preferences.md) |
| Decision authority | The product owner accepted the store split, Server authority, subscription delivery, and Client minimal read set on 2026-10-24                                                                                                                |
| Exit condition     | Every delivery step below is implemented, its current behavior lives in the current authorities, and this proposal is deleted                                                                                                                 |

Simplified Chinese review translation:
[configuration-authority.zh-CN.md](configuration-authority.zh-CN.md).
English remains canonical.

## Decision

- `preferences.json` owns every setting that a TUI or Desktop Settings surface edits. `config.toml` owns settings that users edit only by hand. A setting never exists in both stores; an overlapping key is deleted, with its reader, from the store that does not own it.
- The Server is the only runtime reader. It reads both stores, validates them, merges defaults, and publishes one effective snapshot per revision to Clients over the shared-carrier state subscriptions. Workers receive the user intent a launch needs from Server dispatch and resolve device facts, such as System shell discovery, themselves. A rejected candidate keeps the previous valid snapshot and publishes diagnostics.
- Workers read neither store.
- Clients read `config.toml` directly only for the minimal set below. Every other Client consumer uses the Server snapshot.
- Desktop keeps reading and writing `preferences.json` through `orchbee-settings`. The Server observes those writes through its preference sampling deadline.
- `user_settings` stays in `orchbee-local-runtime` while the Server and the Client minimal set share it.

## Store ownership

| Store              | Settings                                                                                                                                                                                                                                                                                                               |
| ------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `preferences.json` | `locale`, `applicationTheme`, `terminalDisplayTheme`, `customForeground`, `customBackground`, `copyOnSelect`, `trimTrailingBlankRowsForAgents`, `agentNotificationMode`, `completionReminderMode`, `completionReminderMinimumSeconds`, `soundEnabled`, `scrollbackBytes`, `launchProfiles`                             |
| `config.toml`      | `terminal.new_cwd`, `terminal.focus_new_panel`, `worktrees.directory`, `update.channel`, `update.manifest_check`, `keys.*` including custom commands, `ui.window_title`, `ui.sidebar.agents.*`, `ui.sidebar.spaces.*`, `ui.toast.delivery`, `ui.toast.delay_seconds`, `ui.toast.overlay.position`, `ui.sound.agents.*` |

## Overlap resolution

| Setting                | Owner                                                                                                                                                                               | Deleted                                                                                                                                                                    |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Chrome theme           | `applicationTheme`                                                                                                                                                                  | The parsed-but-unset settings theme slot, Configuration `themeName`, and `themeName` in `config check` and `server config` receipts                                        |
| Default launch program | `launchProfiles`                                                                                                                                                                    | `terminal.default_shell`, `terminal.shell_mode`, their private startup arguments, Server default-shell admission, and `defaultShell`/`shellMode` in Configuration receipts |
| Notifications          | Split by meaning; Agent reminder mode and global sound stay shared preferences, while manual delivery, delay, overlay position, and per-Agent sound overrides stay in `config.toml` | Nothing                                                                                                                                                                    |

## Client minimal read set

| Reader                    | Settings                                 | Reason                                                                    |
| ------------------------- | ---------------------------------------- | ------------------------------------------------------------------------- |
| Local Session activation  | Configuration location and file validity | Rejects an unreadable file and freezes the location the Server reads once |
| Update channel and update | `update.channel`                         | Release operations run without a Session                                  |
| `config check`            | The whole file                           | Validates the file the user edits; applies no runtime policy              |

## Delivery

| Step | Result                                                                                                                                                             | Status    |
| ---- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------- |
| 1    | Delete both overlaps; Workers stop reading `config.toml`.                                                                                                          | Delivered |
| 2    | The Server reads `config.toml` once at startup and once per reload; Agent manifest reload and process observation consume the committed configuration.             | Delivered |
| 3    | The Server samples the frozen `config.toml` location and reloads and publishes a changed file with explicit-reload semantics.                                      | Delivered |
| 4    | Server admission captures launch defaults from `preferences.json`; Workers stop opening it.                                                                        | Delivered |
| 5    | Configuration snapshots carry `terminal.focus_new_panel`, `terminal.new_cwd`, and `worktrees.directory`; TUI startup, Worktree, and CLI creation reads move to it. | Delivered |
| 6    | Current authorities describe the delivered behavior; this proposal is deleted.                                                                                     | Open      |

Each step passes the affected Rust checks and existing configuration tests. The issue closes after
`pnpm verify:changed` and `pnpm verify:rust-native` pass.
