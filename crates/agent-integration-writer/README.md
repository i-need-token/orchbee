# orchbee-agent-integration-writer

Node-local Agent integration writer: provider inspection, enable and disable plans, and their
filesystem and process effects, using the cohort-admitted observation-adapter capsule and
retained mutation outcomes. Values and report vocabularies come from the
[Agent integration contract](../agent-integration/README.md); this crate never defines them.

## Windows observation channel withdrawn

On Windows the Agent observation channel is withdrawn: the Server binds no observation pipe, and
the Worker never puts `ORCHBEE_AGENT_OBSERVATION_SOCKET` into an Agent's environment. Unix keeps the
channel, whose socket sits in the Session's private runtime directory. The two effects are gated
where they happen, the Server's `bind_for_session` and the Worker's `launch_overlay`, and no setting
turns the channel back on. The Windows listener and the ingress behind it, the adapters' Windows
transport and this crate's Windows hook launchers stay in place, unused, for a revival.

**Why.** Every adapter opens the pipe through `node:net`, which sets no security quality of
service, so the pipe's server receives the person's token at impersonation level, and Node has no
option that limits an open to identification. While a Server holds the name, that server is the
Server itself; after a Server ends while its Agents keep running, or for an Agent that outlives its
Session, any process can take the name and receive the token, and one holding
`SeImpersonatePrivilege`, as service accounts do, can act as the person with it. The name follows
from the profile path and shows in pipe enumeration. A corrected adapter would be new Capsule
payload, and it cannot reach the copies already installed without rewriting each Agent's
configuration: a running copy sits in the Capsule version folder its Agent entry names, or in the
copy its provider made, and only an enable job moves either, which no command or Settings control
starts in this release. Withdrawing the channel reaches everyone through an ordinary update and
writes into no Agent's configuration.

**What earlier enablers lose on Windows.** People who enabled an integration in an earlier version
keep its entries, but nothing its adapter sends reaches Orchbee:

- [Agent session resume](../../apps/orchbee-server/docs/agent-session-resume.md#process-source)
  keeps only its process sources, Claude Code's own run table and a conversation id on the Agent's
  own command line, so an Agent whose session only its hook named comes back as a plain shell after
  a restart.
- Agent rows, needs-attention and turn results come from screen detection alone, as they do for
  everyone who never enabled an integration.

**What a revival needs.** Both must hold: an adapter client that opens the pipe at identification
level, as every Rust and native client of [`orchbee-local-ipc`](../local-ipc/README.md) does, and a
way to deliver that client that writes into no Agent's configuration. A revival must also close a
second defect: the adapter accepts any server's acknowledgement, so it cannot tell a real Server
from a stand-in holding the name, and a stand-in receives every sanitized observation.

**Known limits.** A terminal opened before the update keeps the variable in its shell's environment
until it closes, and an Agent started there still opens that name at every hook event. No Server
holds the name any more, so the open fails at once and the adapter stays silent, but a process that
takes the name before such a terminal closes still receives the token. `uninstall` still disables every
integration this environment may have enabled. The Capsule's own README still describes the Windows
transport, because every byte of the Capsule except `capsule.json` is payload and stays as
installed; this section supersedes it.

## Ownership

- One invocation runs under one cooperative 40-second budget. Callers pass a
  `TerminationSource`; the writer observes it between bounded steps and settles as cancelled.
- Unix and Windows are supported. Each platform has its own provider configuration locations:
  `configuration_windows.rs` on Windows, and `configuration.rs` elsewhere, taken from macOS
  sources. Linux compiles the latter unverified and keeps the configuration-hook providers and
  Hermes manual-only. The hook command each declaration carries is the platform's launcher:
  PowerShell on Windows, a POSIX shell line elsewhere.
- The actual consumers are `orchbee-worker`, whose job owner runs every mutation a job requests, and `orchbee`, which reuses the read-only configuration
  helpers for Windows update verification, and, after the environment's Sessions have stopped,
  runs the disable of each provider `uninstall` finds this environment may have enabled. It
  decides from `list_owned`, which reads Hermes's own state from its two entry files alone, so it
  starts no Agent program for a provider without such evidence.

## What each operation starts

Most providers are read from files. Claude Code, Grok and GitHub Copilot report their plugins only
through `plugin list` and `plugin marketplace list` commands of their own program, and Hermes
through `plugins list`; a provider's program may create its own first-run files whenever it
starts. `list` reads files only and starts no provider program, so those rows stay `unknown` with
no issue, not checked, unless Orchbee's own Hermes entry files already prove a conflict.
`explain` is the explicit check of one provider and runs those read commands; `enable` and
`disable` run them before and after their effects. The
[reading rule](../agent-integration/README.md#reading-rule) names the providers for Clients.

## Observation adapter Capsule

[`agent-observation-adapters/`](agent-observation-adapters/README.md) is the source of the Capsule
this writer installs. Native staging and Desktop packaging copy it into each cohort, and
`bundled_payload_sha256` embeds its manifest's identity. Every byte of it except `capsule.json` is
payload, so any edit, including its README, needs a new `payloadSha256` and makes existing
installations `update-required` until an enable job repairs them; no command or Settings control starts one in this release. Its adapter
tests run from the repository root:

```sh
pnpm test:agent-observation-adapters
```

## Capsule location and ownership

Each environment installs Capsules in its own [data root](../local-runtime/README.md#data-root),
under `internal/integrations/agent-observation/<version>/`. The version is
`bundle/package.json`'s; a name that already holds a different payload takes the first free
`<version>-2`, `<version>-3`, and so on, and an install whose manifest declares the current digest
is reused. `capsule.json` keeps the digest as the identity. Publishing writes the owner record
`bundle/scripts/environment.json`, `{"environment":"<stem>","version":1}`, which the digest
excludes, so every copy a provider makes of `bundle/` names the environment that installed it.

An Agent entry belongs to the environment whose Capsule root it names. A root in another data
root's Capsule folder is that environment's. A folder of `~/.orchbee/agent-observation-adapters`,
where every environment installed before each kept its own, belongs to no build: every build leaves
an entry naming it as it is, and no environment ever deletes the shared folder. A copied scripts
folder belongs to the environment its owner record names, and to release when it has none.
`owned_by_another_environment`
is the one predicate every provider uses for this. Enable and disable add, update and remove only
this environment's entries. Hook-configuration documents keep other environments' entries in
place beside this environment's. A provider that admits one entry, such as the Antigravity named
block or Cline's copied package, reports another environment's entry as `conflict`, and enable and
disable both refuse it. A command-line registration of Claude Code, Codex, Grok or GitHub Copilot
whose every named Capsule root belongs to another environment also reads `conflict`, and enable
refuses it; disable treats it as absent and completes without a command, so `uninstall` is not
stopped. A registration that also names this environment's or a foreign root stays a conflict for
disable too.

### Whole-file entries

Some Agents load Orchbee from a file or folder at a fixed path that every environment on the
machine shares: the OpenCode module file, its Windows client folder
`orchbee-agent-observation-tui` and its obsolete `.mjs` name, the Kilo, Pi and OMP module files,
the Gemini and Qwen Code extension folders, and Hermes's `__init__.py` and `plugin.yaml`. The
writer keeps no record of what it wrote, so it judges each such entry by what the entry carries:

- **Another environment's.** A module or entry file whose bytes are exactly a bootstrap the writer
  renders, current or superseded, for the Capsule root it names, when the predicate above gives
  that root to another environment; or an extension folder whose marker, integration id and
  complete file fingerprint match and whose copied owner record names another environment.
- **This environment's.** The same exact bytes, or a proven folder, naming this environment's
  Capsule folder: `enabled`, or `update-required` for an older Capsule or template.
- **Anything else.** An edited body, a changed fingerprint, a link, another kind of entry, an
  unreadable owner record, or a bootstrap naming a root outside every Capsule folder. An edit
  breaks the exact match, so an entry this environment wrote and a person then changed cannot be
  told from content that was never Orchbee's. Both read `conflict`; enable and disable refuse them
  and leave them unchanged.

On every platform another environment's entry is left exactly as it is. Its row still reads
`conflict`, so no Client shows the integration as enabled for this environment, and enable refuses
before any effect instead of replacing it; the person enables it from the environment that owns it,
or removes it there. Disable, and so `uninstall`, treats it as absent: it removes only this
environment's targets of the provider and completes without an effect when none is left. A folder
or file pair whose members belong to different owners is `conflict`, and Hermes counts as another
environment's only when every entry file present is, which leaves its activation to that
environment too.

At hook time the transport reads the owner record beside it and reports only when the inherited
`ORCHBEE_ENVIRONMENT` equals the owner, or when the variable is absent and the owner is release;
otherwise the hook exits silently and successfully. After a completed enable or disable the writer
removes the other version folders of its own Capsule folder once a fresh inspection shows that no
entry can name one; the current version stays until uninstall.

Because only an installed Capsule lets an entry name an environment, the local runtime's
[`agent_integration_may_have_been_enabled`](../local-runtime/README.md#data-root) answers from the
presence of these two folders without running provider commands or reading provider
configuration, and implicit callers skip inspection when it is `false`.

### Global skill discovery entries

Supported harnesses with an established global `SKILL.md` directory (`claude-code`, `codex`,
`antigravity-cli`, `gemini`, `qwen-code`, `opencode`, `kilo`, `cursor`, `github-copilot`,
`qoder-cli`, `droid`, `pi`, `omp`, `kimi`, and `hermes`) also manage
`<skills_root>/orchbee/SKILL.md` when enabled. Each file tells the Agent to run `orchbee --skill`
when `ORCHBEE_TERMINAL_ID` is set. It carries the marker
`<!-- orchbee-agent-integration-skill:v1:<base64(capsule_root)> -->`; the base64 encoding keeps a
root path that contains `-->` from ending the marker early. The file holds no guide text beyond
that instruction: `orchbee --skill` prints the guide of the CLI version that is running, so the
pointer never goes stale when the CLI updates.

Ownership is decided by the marker alone: the recorded capsule root decides it, not the file's other
bytes. A file whose marker names this environment's root stays owned after a body edit. It reads
`outdated` until enable rewrites it, and `disable` removes it. A file whose marker names another
environment's root is skipped by enable and disable, and `uninstall` leaves it in place. A
user-authored file, a file with more than one marker, a symbolic link, or a reparse point is foreign
and is skipped the same way.

The skill is an accessory. A skipped skill never changes the integration's state, its ownership, or
whether enable and disable proceed; the mutation report does not record the skip. An
outdated or missing owned file is written by enable. An owned file is removed by `disable` even when
the provider program is missing or cannot be inspected, because removing a file needs no program.
`disable` deletes the `orchbee/` folder only when it is empty.

The skill body is English in every locale. Enable rewrites any body that differs from the canonical
text, so a localized body would be overwritten on every enable.

Known limitation: a skill that another environment left behind, for example one from a development
cohort removed without `disable`, is skipped by every other environment and removed by none of them.
Only that environment's own `disable` removes it.

Where a provider's hook or extension path reads the same override, its skill root reads it through
the same `provider_roots` helpers, so each formula has one source. An unset or empty override falls
back to the default. Any other value must be absolute: a relative, unexpanded, or whitespace-only
value refuses without fallback. Hermes alone trims `HERMES_HOME`, so a blank value counts as unset
there, as its hook path already did.

| Harness           | Global skill folder (default)                                              | Override                                                                                                             | Official source   |
| ----------------- | -------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------- |
| `claude-code`     | `~/.claude/skills`                                                         | `CLAUDE_CONFIG_DIR` replaces `~/.claude`                                                                             | none verified yet |
| `codex`           | `~/.codex/skills`                                                          | `CODEX_HOME` replaces `~/.codex`                                                                                     | none verified yet |
| `antigravity-cli` | `~/.gemini/config/skills`                                                  | none                                                                                                                 | none verified yet |
| `gemini`          | `~/.gemini/skills`                                                         | `GEMINI_CLI_HOME` replaces `~`, so the folder is `$GEMINI_CLI_HOME/.gemini/skills`                                   | none verified yet |
| `qwen-code`       | `~/.qwen/skills`                                                           | `QWEN_HOME` replaces `~/.qwen`                                                                                       | none verified yet |
| `opencode`        | `~/.config/opencode/skills`                                                | `XDG_CONFIG_HOME` replaces `~/.config`                                                                               | none verified yet |
| `kilo`            | `~/.config/kilo/skills`                                                    | `KILO_CONFIG_DIR` replaces `~/.config/kilo`; else `$XDG_CONFIG_HOME/kilo/skills`                                     | none verified yet |
| `cursor`          | `~/.cursor/skills`                                                         | `CURSOR_CONFIG_DIR` replaces `~/.cursor`                                                                             | none verified yet |
| `github-copilot`  | `~/.copilot/skills`                                                        | `COPILOT_HOME` replaces `~/.copilot`                                                                                 | none verified yet |
| `qoder-cli`       | `~/.qoder/skills`                                                          | `QODER_CONFIG_DIR` replaces `~/.qoder`                                                                               | none verified yet |
| `droid`           | `~/.factory/skills`                                                        | none                                                                                                                 | none verified yet |
| `pi`              | `~/.pi/agent/skills`                                                       | `PI_CODING_AGENT_DIR` replaces `~/.pi/agent`                                                                         | none verified yet |
| `omp`             | `~/.omp/agent/skills`                                                      | none                                                                                                                 | none verified yet |
| `kimi`            | `~/.kimi-code/skills`                                                      | `KIMI_CODE_HOME` replaces `~/.kimi-code`                                                                             | none verified yet |
| `hermes`          | `~/.hermes<suffix>/skills`; Windows `%LOCALAPPDATA%\hermes<suffix>\skills` | `HERMES_HOME` replaces the profile folder; `HERMES_DATA_DIR_SUFFIX` sets `<suffix>` (letters, digits, `.`, `_`, `-`) | none verified yet |

None of these roots has been checked against provider documentation yet. Until a source is recorded
here for a harness, its root is an assumption made by Orchbee, not a documented provider location.

## Conformance fixtures

`tests/fixtures/` holds the corpora replayed by this crate's harnesses. The `test-support`
feature exposes both replays, and the CLI tests run them with the CLI's integration exit mapping:

- `provider-conformance/` — `payloads.json` plus `cases/<provider>.cases.json`:
  per-provider inspection, enable, and disable cases pinning scripted command
  replies, expected argv, filesystem effects, and the rendered CLI verdict.
- `provider-effects/` — `cases.json`: filesystem-effect machine cases —
  capsule publish, move-aside, backup digests, remove walks, cohort and
  version gates, and budget or cancellation frontiers.

The harnesses resolve both corpora through `CARGO_MANIFEST_DIR`, so any
checkout replays them with no environment override or machine-local path.
A case edit is a semantic contract change and belongs in the same reviewed
change as the engine behavior it pins; a new provider adds one file under
`provider-conformance/cases/`.
