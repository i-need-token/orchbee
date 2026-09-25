# Agent observation adapters

This packaged resource contains twelve explicitly installed, provider-bound observation adapters for
Claude Code, Codex, Grok, OpenCode, Kilo, Cursor, Qoder CLI, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code.
Each adapter validates its own upstream event contract and emits the same content-free normalized
facts through Orchbee's shared local socket/pipe plus private-APC correlation transport. Desktop's closed
registry, not an adapter payload, grants provider identity, accepted events, and maximum
capabilities.

## Windows Codex transport

The native Windows cohort uses the same v3 rich-envelope/private-APC join. Capsule
`0.4.1` adds Codex `commandWindows` hooks, using the provider's existing `cmd.exe /C`
execution contract and `PLUGIN_ROOT`. The Windows x64 cohort must include its pinned ConPTY runtime: the inbox ConHost may
consume private APC sequences. Windows does not gain Unix remote discovery or
additional provider installation support through this adapter change. Native managed
enable/disable and start/prompt remain separate CLI obligations.

| Field                    | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and owner        | Codex command hooks in an Orchbee ConPTY must reach the native Server's existing observation ingress. The shared adapter owns delivery; Server/Host retain envelope validation, generation identity and the exact join.                                                                                                                                                                                                                                                |
| Inputs and reference     | Allowlisted normalized Codex events, the inherited `ORCHBEE_AGENT_OBSERVATION_SOCKET_V3` and the provider-supplied plugin root. Codex 0.155.1's Windows hook contract accepts `commandWindows` and retains the parent's console despite redirecting standard streams. Environment strings never grant Agent or terminal identity.                                                                                                                                      |
| Endpoint policy          | Admit only the exact local pipe spelling `\\.\pipe\orchbee-<32-lowercase-hex>-observation`. Missing, malformed, remote UNC and non-observation targets produce no delivery. Windows never scans pipes, files or Unix SSH forwards. The injected endpoint and native Server ACL provide routing; the adapter does not claim to inspect pipe ownership.                                                                                                                  |
| Correlation target       | Use only the literal device `\\.\CONOUT$`, opened with read/write access and verified as a character device before writing. Never use a relative `CONOUT$`, environment-selected device, stdout or stderr. Direct visibility remains unknown. Missing console means no hint; no fallback may redirect the hint into a file.                                                                                                                                            |
| Workflow and transitions | Validate observation → resolve exact endpoint and fixed console target → send bounded content-free envelope → wait for exact bounded ACK → write the matching private APC → close the console handle. Missing endpoint, invalid input, timeout, rejected ACK and open/write failure stop the affected delivery. Rich-only events expire under the existing ingress rules.                                                                                              |
| Invariants and effects   | No prompt/output/path content enters the envelope; no APC precedes acceptance; neither half alone binds an Agent; no hook installs configuration or launches a Site. Windows never unlinks a pipe. The provider hook exits successfully on absent Node/runtime or transport failure. Socket and console handles remain locally owned and close on completion/failure.                                                                                                  |
| Rollout and recovery     | Publish a new immutable Capsule version and matching resource/consumer versions. Git preserves LF bytes across Windows checkouts for the hashed payload. Preserve older Capsules and provider declarations until an explicit managed action replaces them. The Unix command field and existing Unix transport behavior remain available. The adapter adds no dependency or wire version; Windows cohort delivery additionally requires the pinned Host ConPTY runtime. |
| Evidence and limits      | Deterministic admission/ACK-order cases, real Windows named-pipe ACK cases, silent bounded command-hook process cases and an isolated native Server/ConPTY delivery witness. Transport evidence does not prove managed installation, real Agent task completion, physical TUI interaction or every provider's Windows support. Same-author challenge only; TLA execution remains deferred by user instruction.                                                         |

## Supported contracts

| Adapter            | Reference version                       | Integration surface      | Maximum status evidence                                                                                      |
| ------------------ | --------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------ |
| Claude Code        | Current documented hook/plugin contract | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion, failure                           |
| Codex              | `0.150.1` (Unix), `0.155.1` (Windows)   | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion                                    |
| Grok               | `1.0.5`                                 | Command-hook plugin      | Session, Turn, tool, approval, compaction, subagent, idle, completion, failure, cancellation, interruption   |
| OpenCode           | `1.1.25`, `1.18.30`                               | JavaScript plugin events | Session, busy/retry/idle, tool, question, approval, error                                                    |
| Kilo | `cb67014f` | JavaScript plugin events | Session, busy/retry/idle/offline question, chat, tool, question, approval, error |
| Pi                 | `0.84.4`                                | Extension API            | Session, Agent start/settled, blocking UI prompt, tool, compaction                                           |
| OMP                | `18.0.10`                               | Extension API            | Session, Agent start/end, Ask tool, tool approval, tool, compaction, retry                                   |
| Gemini CLI         | `20f70757`                                | Command-hook extension   | Session, Agent start/end, tool, question, permission, compaction, completion                                 |
| GitHub Copilot CLI | `1.0.81`                                | Command-hook plugin      | Session, Turn, actual question/permission prompt, tool result, compaction, completion, unrecoverable failure |
| Cline              | `3.0.60`                                | Plugin runtime API       | Run/Turn, tool, question, completion, failure, interruption                                                  |
| Qwen Code          | `c3a4058a`                                | Command-hook extension   | Session, Turn, tool, question, permission, compaction, subagent, completion, failure                         |

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
catch a missing Capsule, shell command hooks test that their runtime still exists, and Gemini/Qwen entries
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
half goes to same-user private Unix-domain sockets or the injected native Windows observation pipe. A local process uses only the exact endpoint in
its environment; only a remote process without that overlay discovers same-user forwarded
candidates, and discovery requires an inherited non-empty `SSH_CONNECTION`, `SSH_CLIENT`, or
`SSH_TTY` marker. An ordinary local terminal without the exact overlay performs no socket scan.
Only after an endpoint returns the exact bounded v3 acceptance ACK does the adapter
write one content-free private APC correlation half directly to each resolved outer TTY. With no
exact ACK, the adapter writes no terminal control sequence. Desktop requires an exact
event/correlation join for a live terminal generation, so neither half can associate and authorize
rich state alone; unmatched envelopes sent to another live instance expire without canonical
effect.
On Unix, every TTY candidate must be a lexically normalized path below `/dev`; the writer opens it
nonblocking without following symlinks, verifies a character device, and requires same-user
ownership except for the kernel-owned `/dev/tty` alias. A resolver cannot redirect a hint into a
regular file or another user's terminal.

The Unix resolver supports direct terminals, every attached tmux client, conservative Zellij
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

## Claude direct execution

Capsule 0.4.2 replaces Claude's `/bin/sh` hook launcher with `node` plus an explicit argument vector, following the [upstream exec contract](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form). A fixed module expression imports the literal plugin script path through pathToFileURL and catches import failures. Script removal and malformed input remain silent; Node itself must exist on the provider PATH. If Node cannot start, Claude may display its non-blocking diagnostic. The integration never produces a tool permission decision.

This shared declaration allows native Windows onboarding without a shell dependency. Existing events, hook deadlines, input bounds, sanitization and transport ownership are unchanged. The [native Windows contract](../../../orchbee/docs/windows-agent-integrations.md) owns installation and controlled Windows evidence; this change does not establish authenticated Agent task completion or new macOS/Linux workflow support. Old capsules remain immutable and explicit repair installs 0.4.2.

## Copilot direct execution

Capsule 0.4.9 uses the [Copilot exec hook contract](https://docs.github.com/en/copilot/reference/hooks-reference) with Node. The fixed import guard reads COPILOT_PLUGIN_ROOT directly from the provider child environment, checks that it is absolute, and passes its joined script path through pathToFileURL. It emits no output when the root or script is absent. Node is a prerequisite; an unavailable executable is a provider launch error, not a delivered observation. PascalCase event names retain the documented VS Code compatible input fields used by the existing sanitizer. No shell interprets the path. Windows onboarding uses the separately verified current plugin command/JSON contract; shared payload compatibility does not promote Desktop CLI parity.

### Grok command bootstrap

Capsule 0.4.9 keeps Grok's command-only hook contract with a fixed base64-encoded Node bootstrap. The decoded code reads GROK_PLUGIN_ROOT as data, sets the literal script/provider arguments, and performs a guarded dynamic import; it never inserts a user path into shell syntax. PowerShell 5.1/7 preserve redirected JSON stdin through this command. Node is a prerequisite; missing Node is a provider command error, while missing roots/scripts remain silent. Grok's existing camelCase event inputs, sanitizer, deadlines and observation transport are unchanged. The encoding protects shell quoting, not secrets. Windows local installation state is read separately from the provider-owned TOML configuration; it does not establish effective project policy or an active session's reload.

## OpenCode-family Windows onboarding

Capsule 0.4.9 adds Kilo and shares its event normalizer with OpenCode. The shared factory admits lifecycle identities from sessionID or legacy info.id, rejecting disagreement. Callback arguments and outputs remain private. Delivery retains at most 64 outstanding sanitized events, in order; overflow/dispose stops new admission and drops the queued suffix until reload, while an in-flight send may finish. Transport failures are silent. Windows native onboarding installs discoverable .js bootstraps and preserves foreign/newer entries. See the [owning Windows model](../../../orchbee/docs/windows-agent-integrations.md#kilo-windows-integration-and-shared-event-model) for paths, transitions and evidence. Desktop onboarding retains its separate provider registry.

## Gemini and Qwen command roots

Capsule 0.4.9 executes fixed Node guards using ORCHBEE_AGENT_EXTENSION_ROOT in the provider's per-hook environment. Gemini supplies the value by recursively hydrating its external hook file; Qwen supplies it through recursively hydrated inline hooks in qwen-extension.json. Substitution affects an environment value, never command source. Both entries import their self-contained scripts and keep existing content-free event/transport bounds. Windows native onboarding and doctor share provider-specific directory overrides and refuse invalid explicit roots. Windows directory ownership checks include payload version direction so old entries require repair and newer entries refuse downgrade. See the [Windows extension model](../../../orchbee/docs/windows-agent-integrations.md#gemini-and-qwen-windows-extensions).

The Cursor adapter emits session-identified from consistent session/conversation identity aliases. It never derives working/idle or permission decisions. Windows native JSON onboarding installs the fixed encoded launcher; no authenticated provider execution is implied.

Qoder CLI uses qodercli identity-only observations for exact SessionStart/session_id input. The Windows native manager installs its encoded command group; Node is required and hook output remains an empty JSON object.
