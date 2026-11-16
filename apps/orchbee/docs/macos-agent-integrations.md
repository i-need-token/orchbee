# macOS Agent integrations

`orchbee agent integration list/explain/enable/disable --provider <id>` manages the
configuration-hook providers Cursor (`cursor`), Qoder CLI (`qodercli`), Antigravity CLI
(`antigravity-cli`), Mastracode (`mastracode`), Droid (`droid`), Kimi (`kimi`), Letta (`letta`)
and Devin (`devin`), and the Hermes session plugin (`hermes`), on macOS as on Windows. The
document grammar, ownership proofs, byte-preserving edits and refusals are the ones the
[Windows integration contract](windows-agent-integrations.md) describes; this page owns what
differs on macOS: where each provider keeps its configuration, the hook command, and how a file
is replaced. `agent integration list --outdated-only` filters the same inspection as on Windows.

Linux compiles the same adapter and locations, but they are not verified there, so these nine
providers stay manual-only on Linux. Kilo stays manual-only on every platform but Windows.

## Configuration locations

Each provider's user-level file, with the source it was taken from. An override variable applies
only when it is nonempty and absolute; a relative value refuses rather than falling back.

| Provider        | File                                                                                                                                                                      | Source                                                                                                                                                                                                                                                                            |
| --------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Cursor          | `$CURSOR_CONFIG_DIR/hooks.json`, otherwise `~/.cursor/hooks.json`                                                                                                         | [Hooks](https://cursor.com/docs/hooks), [CLI configuration](https://cursor.com/docs/cli/reference/configuration)                                                                                                                                                                  |
| Qoder CLI       | `$QODER_CONFIG_DIR/settings.json`, otherwise `~/.qoder/settings.json`                                                                                                     | [Hooks](https://docs.qoder.com/cli/hooks), [settings reference](https://docs.qoder.com/cli/settings-reference)                                                                                                                                                                    |
| Antigravity CLI | `~/.gemini/config/hooks.json`                                                                                                                                             | [Hooks](https://antigravity.google/docs/hooks?app=antigravity)                                                                                                                                                                                                                    |
| Mastracode      | `~/.mastracode/hooks.json`                                                                                                                                                | [`hooks/config.ts`, `constants.ts`](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/config.ts)                                                                                                                         |
| Droid           | the first existing of `~/.factory/hooks.json`, `~/.factory/hooks/hooks.json` and the `hooks` member of `~/.factory/settings.json`; a new declaration goes to `hooks.json` | [Hooks](https://docs.factory.com/harness/hooks), [settings](https://docs.factory.com/droid-cli/settings)                                                                                                                                                                          |
| Kimi            | `$KIMI_CODE_HOME/config.toml`, otherwise `~/.kimi-code/config.toml`                                                                                                       | [Configuration files](https://moonshotai.github.io/kimi-code/en/configuration/config-files), [`utils/paths.ts`](https://github.com/MoonshotAI/kimi-code/blob/be7d5f5fea7800778e4660cd5f36780ba783bddd/apps/kimi-code/src/utils/paths.ts)                                          |
| Letta           | `~/.letta/settings.json`, where `~` is `HOME`                                                                                                                             | [`hooks/loader.ts`](https://github.com/letta-ai/letta-code/blob/46e1f82ba44145e8b8d0ac283c2470b59822715e/src/hooks/loader.ts)                                                                                                                                                     |
| Devin           | `~/.config/devin/config.json`                                                                                                                                             | [Configuration file](https://docs.devin.ai/cli/reference/configuration/config-file), [hooks](https://docs.devin.ai/cli/extensibility/hooks/overview)                                                                                                                              |
| Hermes          | `$HERMES_HOME/plugins/orchbee-agent-observation/`, otherwise `~/.hermes$HERMES_DATA_DIR_SUFFIX/plugins/orchbee-agent-observation/`                                        | [`hermes_constants.py`](https://github.com/NousResearch/hermes-agent/blob/2a05208012cf50bbe8ff7b7b54a3825fd1e4692a/hermes_constants.py), [`plugins_cmd.py`](https://github.com/NousResearch/hermes-agent/blob/2a05208012cf50bbe8ff7b7b54a3825fd1e4692a/hermes_cli/plugins_cmd.py) |

Cursor's documentation does not say that `hooks.json` follows `CURSOR_CONFIG_DIR`; Orchbee reads
it there, as on Windows. Devin's documentation names `~/.config/devin` without mentioning
`XDG_CONFIG_HOME`, so that variable is not read. Hermes expands `~` and `$NAME` in `HERMES_HOME`
itself; a value that would need expansion refuses instead of being guessed, and Hermes's commands
run with `HERMES_HOME` pinned to the folder whose entry files were read.

The plugin and extension providers follow the same override variables as on Windows, under the
same nonempty-and-absolute rule:

- OpenCode: `$XDG_CONFIG_HOME/opencode/plugins/orchbee-agent-observation.mjs`, otherwise under
  `~/.config/opencode/plugins/`.
- Kilo: `$KILO_CONFIG_DIR/plugin/orchbee-agent-observation.js`, otherwise under
  `$XDG_CONFIG_HOME/kilo/`, otherwise under `~/.config/kilo/`. Kilo stays manual-only here.
- Gemini: `$GEMINI_CLI_HOME/.gemini/extensions/orchbee-agent-observation/`, otherwise under `~`.
- Qwen Code: `$QWEN_HOME/extensions/orchbee-agent-observation/`, otherwise under `~/.qwen/`.

## Hook command

The providers run a hook's `command` through a POSIX shell: Mastracode and Kimi through
`/bin/sh -c`, Qoder CLI through `bash -c` by default, Letta through `/bin/zsh -c`, and Cursor,
Antigravity, Droid and Devin through a shell their documentation does not name. The command is
one line that parses the same in sh, bash and zsh:

```sh
node -e "/*orchbee-agent-observation:<id>*/<fixed script>" '<base64 Capsule root>' >/dev/null 2>&1; printf '{}'
```

The fixed script holds no `$`, backquote, backslash, double quote or `!`, so the shell passes it to
Node unchanged. It reads the Capsule root as base64 data from its only argument, so no path
becomes shell or JavaScript source, and imports `bundle/scripts/forward-hook.mjs` with the
provider id; the hook's JSON input stays on stdin. A missing Node, Capsule or script stays silent
and never fails the hook. Every provider but Letta then receives an empty JSON object. Letta adds
any hook stdout to the prompt, so its command ends in `|| true` and prints nothing. Qoder CLI's
entry carries no `shell` member, since its POSIX default runs the line as written.

An entry is Orchbee's only when its command round-trips exactly for its provider and an admitted
absolute root; any other command carrying the marker is a conflict.

## File replacement

A configuration file is replaced as on Windows: the replacement is staged in a private sibling
folder, the original is read and later compared byte for byte, moved aside without replacing
anything, and the replacement is published with `link(2)` so an arrival is never overwritten. The
original is removed only once the published bytes verify, and restored otherwise.

- The staged file takes the original's permission bits; a new file is created `0600`. Owner,
  extended attributes and ACLs are not copied.
- The configuration file itself must be a regular file. A symbolic link at its name reads
  `unknown` and is refused, never followed or replaced; folders above it may be links, as macOS's
  own `/tmp` and `/var` are.
- Reads are bounded to 512 KiB, and a FIFO never blocks the read.

Hook documents keep other environments' entries beside this environment's, as on Windows. Hermes's
two entry files are a whole-file entry, so on macOS another environment's files read `conflict` and
both enable and disable refuse them; see the writer's
[whole-file rule](../../../crates/agent-integration-writer/README.md#whole-file-entries).

## Evidence

Unit tests run the shared grammar, launcher round trip and byte-preserving edits on macOS. Real
`explain`, `enable`, repeated `enable` and `disable` runs of a staged cohort in a scratch HOME on
macOS 14 cover every provider. A user's existing file keeps its bytes outside Orchbee's entries
and its permission bits, and disable returns it byte for byte; a file enable created keeps an empty
object after disable. A malformed or linked configuration file is refused and left as it is, and
each written command runs under `/bin/sh` and `/bin/zsh` with the expected output. Without the
Agent's program on PATH, enable refuses with `agent-unavailable`. Those runs stood in for the
absent Agent programs, and for Hermes's plugin commands, with stand-in executables. Real hook
execution by each Agent, Hermes's own activation, delivery to the Server and authenticated Agent
sessions are not established.
