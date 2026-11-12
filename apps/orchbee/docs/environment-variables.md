# Environment variables

This table is the one list of the `ORCHBEE_*` variables Orchbee programs read or inject, as the
[command surface](../../../docs/cli/command-surface.md#environment-variables) requires. Other
documents link here and describe only the behavior they own.

The level says who may rely on a variable:

- **Public**: people, scripts and Agents may set or read it; a rename after the first public
  release keeps the old name working with a diagnostic.
- **Private**: Orchbee sets it for its own processes; do not set or parse it, and expect it to
  change without notice.
- **Unstable**: usable, but its name, values and output may change in any release with no
  migration.

## Read by Orchbee

| Variable                              | Level    | Meaning                                                                                                                                                                              |
| ------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `ORCHBEE_HOME`                        | Public   | An absolute data root that replaces `~/.orchbee` (or the build's own data folder); see [data root](../../../crates/local-runtime/README.md).                                         |
| `ORCHBEE_CONFIG_FILE`                 | Public   | An absolute `config.toml` path that replaces the data root's; see [user settings](../../../crates/local-runtime/README.md#user-settings).                                            |
| `ORCHBEE_SESSION`                     | Public   | The Session a command or a release Desktop uses when no `--session` names one; see [Session selection](native-cli-query.md).                                                         |
| `ORCHBEE_UPDATE_MANIFEST`             | Public   | Advanced: another location for the release manifest `orchbee update` reads; the release's built-in key still verifies it ([update](windows-cli-configuration.md)).                   |
| `ORCHBEE_TUI_MEASUREMENTS`            | Unstable | Diagnostics: a new absolute file the TUI writes timing samples to; the file's format is unstable ([measurement](../README.md#measurement)).                                          |
| `ORCHBEE_AGENT_DETECTION_CATALOG_URL` | Unstable | Experimental: an HTTP(S) catalog of Agent detection rules; there is no default source and rules are not signed ([detection rules](../../orchbee-server/docs/agent-screen-model.md)). |
| `ORCHBEE_INSTALL_DIR`                 | Private  | The installation store the installer and tests use in place of the default.                                                                                                          |
| `ORCHBEE_DESKTOP_SESSION`             | Private  | The Session a development or test Desktop joins; a packaged release ignores it ([Desktop](../../desktop/README.md)).                                                                 |
| `ORCHBEE_DESKTOP_WINDOW_HIDDEN`       | Private  | Automation: a development or test Desktop window that is never shown; a packaged release ignores it.                                                                                 |
| `ORCHBEE_DESKTOP_*` (others)          | Private  | Development runs of the Desktop entry, listed in the [Desktop guide](../../desktop/README.md#window).                                                                                |
| `ORCHBEE_TERMINAL_HOST_TRACE`         | Private  | Benchmark runs: interaction timing records from the Terminal Host ([Terminal Host](../../terminal-host/README.md)).                                                                  |
| `ORCHBEE_TUI_CAPTURE_DIR`             | Private  | Tests: where render tests write captured frames.                                                                                                                                     |

## Set by Orchbee in the processes it starts

| Variable                                                               | Level    | Meaning                                                                                                                                             |
| ---------------------------------------------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ORCHBEE_TERMINAL_ID`                                                  | Public   | Present in every Orchbee terminal: the program runs inside Orchbee. The value is an opaque ID.                                                      |
| `ORCHBEE_COMMAND_ID`                                                   | Public   | In a [custom command](windows-custom-commands.md): the catalog ID of the command that started it.                                                   |
| `ORCHBEE_BIN_PATH`                                                     | Public   | In a custom command, plugin action or plugin pane: the absolute path of the `orchbee` that started it.                                              |
| `ORCHBEE_WORKSPACE_ID`, `ORCHBEE_TAB_ID`, `ORCHBEE_PANEL_ID`           | Public   | In a custom command, plugin action or plugin pane: the focused Workspace, Tab and Panel when it started, if any.                                    |
| `ORCHBEE_PANEL_CWD`                                                    | Public   | In a custom command: the focused Panel's reported working directory, if any.                                                                        |
| `ORCHBEE_RUNTIME_BASE`, `ORCHBEE_RUNTIME_SCOPE`, `ORCHBEE_ENVIRONMENT` | Private  | In a Session's terminals: the runtime folder, Session and build that an `orchbee` command run there reaches.                                        |
| `ORCHBEE_WORKER_ID`                                                    | Private  | In every Orchbee terminal: the Worker that owns it.                                                                                                 |
| `ORCHBEE_AGENT_OBSERVATION_SOCKET`                                     | Private  | In Agent terminals: where Agent hooks report ([Worker](../../orchbee-worker/README.md)).                                                            |
| `ORCHBEE_AGENT_START`, `ORCHBEE_RUN_TOKEN`                             | Private  | In managed PowerShell: the Agent start line and the per-command run marker ([Agent start](windows-agent-start.md), [run](native-terminal-run.md)).  |
| `ORCHBEE_AGENT_EXTENSION_ROOT`, `ORCHBEE_AGENT_HOOK_ROOT`              | Private  | In Agent hooks: the installed observation Capsule they run from ([integrations](windows-agent-integrations.md)).                                    |
| `ORCHBEE_PLUGIN_*`                                                     | Unstable | In plugin processes: plugin identity, folders, context and event, as [Windows plugins](windows-plugins.md) lists; unstable like plugins themselves. |

`ORCHBEE_SESSION` and `ORCHBEE_RUNTIME_SCOPE` stay separate: the first is a person's choice, while
the second is the route a terminal inherits from its Session, takes precedence, and applies only to
the build `ORCHBEE_ENVIRONMENT` names.

Build scripts also pass `ORCHBEE_ARTIFACT_STEM`, `ORCHBEE_ENVIRONMENT_STEM`, `ORCHBEE_CHECKOUT_NAME`
and `ORCHBEE_SOURCE_COMMIT` to the compiler; a built program never reads them.
