# Agent observation adapters

This packaged resource contains six explicitly installed, provider-bound observation adapters for
Claude Code, Codex, Grok, OpenCode, Pi, and OMP. Each adapter validates its own upstream event
contract and emits the same content-free normalized facts through Orchbee's shared Unix-socket plus
OSC 777 correlation transport. Desktop's closed registry, not an adapter payload, grants provider
identity, accepted events, and maximum capabilities.

## Supported contracts

| Adapter     | Reference version                       | Integration surface      | Maximum status evidence                                                                                    |
| ----------- | --------------------------------------- | ------------------------ | ---------------------------------------------------------------------------------------------------------- |
| Claude Code | Current documented hook/plugin contract | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion, failure                         |
| Codex       | `0.150.1`                               | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion                                  |
| Grok        | `1.0.5`                                 | Command-hook plugin      | Session, Turn, tool, approval, compaction, subagent, idle, completion, failure, cancellation, interruption |
| OpenCode    | `1.18.25`                               | JavaScript plugin events | Session, busy/retry/idle, tool, question, approval, error                                                  |
| Pi          | `0.84.4`                                | Extension API            | Session, Agent start/settled, blocking UI prompt, tool, compaction                                         |
| OMP         | `18.0.10`                               | Extension API            | Session, Agent start/end, Ask tool, tool approval, tool, compaction, retry                                 |

These are separate compatibility contracts. Pi and OMP deliberately do not share an adapter.
Versions outside the matrix are unsupported until their event fixtures and capability grants are
reviewed.

## Explicit setup

### Execution host

Install the selected adapter on every machine where that Agent process runs. For
Orchbee → SSH → remote tmux/Zellij → Agent, installation therefore occurs on the remote host; the
local shell integration and OpenSSH reverse Unix-socket forward provide transport only. Orchbee
never installs or changes an adapter automatically. On the local Desktop device, the user may
explicitly enable or disable Orchbee's exact entry through Agent Integrations Settings. Orchbee
never copies this resource to another host, edits remote Agent configuration, or runs remote setup;
remote installation remains manual.

Use the absolute path to this directory as `<adapter-root>`. A packaged Orchbee application ships
the equivalent `agent-observation-adapters` resource directory.

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
`orchbee-agent-observation` to roll back.

### OpenCode

Add the absolute entry path
`<adapter-root>/bundle/opencode/orchbee-agent-observation.mjs` as a local plugin, or place a symlink
to it in `~/.config/opencode/plugins/`. Remove that configured path or symlink to roll back. Keep
the full bundle at its original path because the entry imports the shared transport beside it.

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

## Status fidelity

An adapter emits only evidence its upstream surface actually provides. OpenCode, Pi, and OMP
settlement becomes affirmative `Ready` without inventing a successful outcome, so it emits no
success notification. Exact errors remain failures where available. Grok cancellation is scoped by
`promptId`, preventing a delayed cancellation report from ending a newer Turn. Missing events,
disabled hooks, unsupported versions, transport failure, and a fully disconnected SSH path degrade
to stale or unknown rather than guessed completion.

## Security and failure boundary

Desktop injects only `ORCHBEE_AGENT_OBSERVATION_SOCKET` through a non-persistent terminal
environment overlay. Before transport, each entry selects only bounded event, session, Turn,
tool-operation, and subagent identifiers and discards prompts, questions, tool input/output,
transcript paths, working directories, error text, notification text, and unknown fields. The rich
half goes to same-user private Unix-domain sockets; a separate content-free OSC 777 correlation
half goes directly to every resolved outer TTY. Desktop requires an exact event/correlation join
for a live terminal generation, so neither half can associate and authorize rich state alone.

The shared resolver supports direct terminals, every attached tmux client, conservative Zellij
client visibility, and nested mux environments. An explicitly sourced Orchbee shell integration
can add a random mode-`0600` remote Unix socket that OpenSSH reverse-forwards to Desktop. There is no
HTTP listener, TCP port, Bearer token, inherited terminal secret, automatic remote installation, or
durable remote cache. Every adapter is passive, writes no shell output, and fails open on missing
configuration, invalid input, timeout, resolver failure, or listener failure.

## Verification

From the repository root:

```sh
claude plugin validate apps/desktop/resources/agent-observation-adapters --strict
grok plugin validate apps/desktop/resources/agent-observation-adapters/bundle
pnpm --filter @orchbee/desktop test:unit
```

The owning runtime and transport contract is documented in the [Desktop guide](../../README.md).
