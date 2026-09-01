# Agent observation adapters

This packaged resource contains ten explicitly installed, provider-bound observation adapters for
Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code.
Each adapter validates its own upstream event contract and emits the same content-free normalized
facts through Orchbee's shared Unix-socket plus private-APC correlation transport. Desktop's closed
registry, not an adapter payload, grants provider identity, accepted events, and maximum
capabilities.

## Supported contracts

| Adapter            | Reference version                       | Integration surface      | Maximum status evidence                                                                                      |
| ------------------ | --------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------ |
| Claude Code        | Current documented hook/plugin contract | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion, failure                           |
| Codex              | `0.150.1`                               | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion                                    |
| Grok               | `1.0.5`                                 | Command-hook plugin      | Session, Turn, tool, approval, compaction, subagent, idle, completion, failure, cancellation, interruption   |
| OpenCode           | `1.18.25`                               | JavaScript plugin events | Session, busy/retry/idle, tool, question, approval, error                                                    |
| Pi                 | `0.84.4`                                | Extension API            | Session, Agent start/settled, blocking UI prompt, tool, compaction                                           |
| OMP                | `18.0.10`                               | Extension API            | Session, Agent start/end, Ask tool, tool approval, tool, compaction, retry                                   |
| Gemini CLI         | `0.57.0`                                | Command-hook extension   | Session, Agent start/end, tool, question, permission, compaction, completion                                 |
| GitHub Copilot CLI | `1.0.81`                                | Command-hook plugin      | Session, Turn, actual question/permission prompt, tool result, compaction, completion, unrecoverable failure |
| Cline              | `3.0.60`                                | Plugin runtime API       | Run/Turn, tool, question, completion, failure, interruption                                                  |
| Qwen Code          | `0.22.3`                                | Command-hook extension   | Session, Turn, tool, question, permission, compaction, subagent, completion, failure                         |

These are separate compatibility contracts. Pi and OMP deliberately do not share an adapter;
Gemini CLI and Qwen Code likewise retain separate manifests and event interpretation. Versions
outside the matrix are unsupported until their event fixtures and capability grants are reviewed.

## Explicit setup

### Execution host

Install the selected adapter on every machine where that Agent process runs. For
Orchbee → SSH → remote tmux/Zellij → Agent, installation therefore occurs on the remote host; the
local shell integration and OpenSSH reverse Unix-socket forward provide transport only. Orchbee
never installs or changes an adapter automatically. On the local Desktop device, the user may
explicitly enable or disable Orchbee's exact entry through Agent Integrations Settings. Orchbee
never copies this resource to another host, edits remote Agent configuration, or runs remote setup;
remote installation remains manual.

Use a durable standalone copy of this directory as `<adapter-root>`. Do not persist a path into an
Orchbee application bundle: deleting or replacing the application must not break Agent startup.
Local Settings first publishes a validated, versioned Capsule under the user's application-data
directory and points managed CLI integrations at that durable copy. Provider-local module entries
catch a missing Capsule, command hooks test that their runtime still exists, and Gemini/Qwen entries
are self-contained copies. An uninstaller must retain every Capsule still referenced by any
provider declaration; explicit disable owns removal of the corresponding provider entry.

Local Claude and Copilot inspection verifies both the exact plugin and its marketplace source;
Grok inspection verifies the copied plugin's recorded source and version. An exact old Orchbee
application-bundle path or sibling Capsule version is migration evidence, not an enabled current
adapter. It is changed only by an explicit Settings action. Any same-named entry with a different
source remains an untouched conflict.

The local Settings transaction, exact managed entries, conflict handling, and rollback behavior are
defined in the [Agent integration guide](../../docs/agent-integrations.md). The commands below remain
the manual setup path and are always required on remote hosts.

### Claude Code

```sh
claude plugin marketplace add <adapter-root>
claude plugin install orchbee-agent-observation@orchbee-local
claude plugin enable orchbee-agent-observation@orchbee-local
```

Disable or uninstall that fully qualified plugin to roll back. Merely adding the marketplace does
not enable observation.

### Codex

```sh
codex plugin marketplace add <adapter-root>
codex plugin add orchbee-agent-observation@orchbee-local
```

Review and trust the bundled hooks when Codex asks. Use
`codex plugin remove orchbee-agent-observation@orchbee-local` to roll back.

### Grok

```sh
grok plugin install <adapter-root>/bundle
```

Review the plugin before accepting installation. Disable or uninstall
`orchbee-agent-observation` to roll back. Local Settings uninstalls only a source-proven Orchbee
copy when its switch is turned off.

### OpenCode

Add the absolute entry path
`<adapter-root>/bundle/opencode/orchbee-agent-observation.mjs` as a local plugin. Keep the durable
standalone root in place because the entry imports the shared transport beside it, and remove the
configured path to roll back. Local Settings instead writes a provider-local guarded bootstrap; if
the Capsule is later unavailable, that bootstrap catches the failed import and returns no hooks or
output.

### Pi

```sh
pi install <adapter-root>/bundle/pi/orchbee-agent-observation.cjs
```

Use `pi remove` with the same local source to roll back. Pi `0.84.4` is the compatibility floor for
the required `agent_settled` and `ui_prompt_*` events.

### OMP

For one explicit run:

```sh
omp --extension <adapter-root>/bundle/omp/orchbee-agent-observation.cjs
```

For persistent opt-in, add that absolute entry to OMP's configured extensions. Remove the
configured path to roll back.

### Gemini CLI

For manual setup, copy a self-contained extension after confirming that the target does not already
exist:

```sh
mkdir -p ~/.gemini/extensions/orchbee-agent-observation
cp -R <adapter-root>/bundle/gemini/. ~/.gemini/extensions/orchbee-agent-observation/
cp -R <adapter-root>/bundle/scripts ~/.gemini/extensions/orchbee-agent-observation/scripts
```

Remove only that exact copied extension to roll back. Local Settings publishes and fingerprints the
same bounded self-contained layout atomically; modified, expanded, or unmarked directories remain
untouched conflicts.

### GitHub Copilot CLI

```sh
copilot plugins marketplace add <adapter-root>
copilot plugins install orchbee-agent-observation@orchbee-local
```

Use `copilot plugins disable orchbee-agent-observation@orchbee-local --plugin` to disable, and the
matching `enable` command to restore it. Orchbee does not register `PreToolUse` or
`PermissionRequest`: those hooks run before Copilot's permission policy is resolved and therefore
cannot prove that a user-facing wait occurred.

### Cline

```sh
cline plugin install <adapter-root>/bundle --json
```

Use `cline plugin uninstall orchbee-agent-observation --json` to roll back. Read-only inspection is
bounded to Cline's official local-install layout and requires Orchbee's exact package marker; an
unmarked same-named entry is a conflict, not an Orchbee-owned installation.

### Qwen Code

For manual setup, copy a self-contained extension after confirming that the target does not already
exist:

```sh
mkdir -p ~/.qwen/extensions/orchbee-agent-observation
cp -R <adapter-root>/bundle/qwen-code/. ~/.qwen/extensions/orchbee-agent-observation/
cp -R <adapter-root>/bundle/scripts ~/.qwen/extensions/orchbee-agent-observation/scripts
```

Remove only that exact copied extension to roll back. Local Settings publishes and fingerprints the
same bounded self-contained layout atomically; modified, expanded, or unmarked directories remain
untouched conflicts.

## Status fidelity

An adapter emits only evidence its upstream surface actually provides. OpenCode, Pi, and OMP
settlement becomes affirmative `Ready` without inventing a successful outcome, so it emits no
success notification. Copilot enters attention only for its actual `permission_prompt` or
`elicitation_dialog` notifications. Qwen starts a user Turn only when `submitted_prompt` proves user
provenance; its continuation callbacks are ignored. Cline intentionally claims no session,
approval, or subagent capability because its plugin runtime does not prove those facts. Exact
errors remain failures where available. Cline records a completion before delivery and suppresses
later events for that exact conversation and run while its tombstone remains retained. The bounded
cache keeps the 64 newest completed runs for each of at most 128 tracked conversations. Run
tombstones use completion-order FIFO eviction, and a new completion refreshes its conversation
before the least recently completed conversation is evicted. Either eviction permits an older
identity to be observed again rather than retaining unbounded plugin state.
`ClineObservationOrdering` checks completion-before-start, duplicate completion, activity, session
isolation, and bounded eviction with two conversations and a two-run retention bound. Grok
cancellation is scoped by `promptId`, preventing a delayed cancellation report from ending a newer
Turn. Missing events, disabled hooks, unsupported versions, transport failure, and a fully
disconnected SSH path degrade to stale or unknown rather than guessed completion.

## Security and failure boundary

Desktop injects only `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` through a non-persistent terminal
environment overlay. Before transport, each entry selects only bounded event, session, Turn,
tool-operation, and subagent identifiers and discards prompts, questions, tool input/output,
transcript paths, working directories, error text, notification text, and unknown fields. The rich
half goes to same-user private Unix-domain sockets. A local process uses only the exact endpoint in
its environment; only a remote process without that overlay discovers same-user forwarded
candidates, and discovery requires an inherited non-empty `SSH_CONNECTION`, `SSH_CLIENT`, or
`SSH_TTY` marker. An ordinary local terminal without the exact overlay performs no socket scan.
Only after an endpoint returns the exact bounded v3 acceptance ACK does the adapter
write one content-free private APC correlation half directly to each resolved outer TTY. With no
exact ACK, the adapter writes no terminal control sequence. Desktop requires an exact
event/correlation join for a live terminal generation, so neither half can associate and authorize
rich state alone; unmatched envelopes sent to another live instance expire without canonical
effect.
Every TTY candidate must be a lexically normalized path below `/dev`; the writer opens it
nonblocking without following symlinks, verifies a character device, and requires same-user
ownership except for the kernel-owned `/dev/tty` alias. A resolver cannot redirect a hint into a
regular file or another user's terminal.

The shared resolver supports direct terminals, every attached tmux client, conservative Zellij
client visibility, and nested mux environments. It asserts `hidden` only when an attached mux
client is showing another pane; direct and active-pane routes remain `unknown` because the hook
cannot prove application-window visibility. Workbench combines that mux evidence with its own
current Tab and Panel view. An explicitly sourced Orchbee shell integration can add a random
mode-`0600` remote Unix socket that OpenSSH reverse-forwards to Desktop. There is no
HTTP listener, TCP port, Bearer token, inherited terminal secret, automatic remote installation, or
durable remote cache. Every adapter is passive, writes no shell output, and fails open on missing
configuration, invalid input, timeout, resolver failure, or listener failure. Persisted command
hooks first test for Node and their readable runtime and always end successfully if either is gone.
Orchbee never uses OSC 9, OSC 777, OSC 99, OSC 1337, BEL, or provider notification hooks as its
correlation channel, so unrelated terminals such as Warp do not turn observation into user-facing
notification popups.

## Verification

From the repository root:

```sh
claude plugin validate apps/desktop/resources/agent-observation-adapters --strict
grok plugin validate apps/desktop/resources/agent-observation-adapters/bundle
pnpm --filter @orchbee/desktop test:unit
```

The owning runtime and transport contract is documented in the [Desktop guide](../../README.md).
