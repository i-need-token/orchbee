# Agent observation adapters

This Capsule contains twenty explicitly installed, provider-bound observation adapters for
Claude Code, Codex, Grok, OpenCode, Kilo, Cursor, Qoder CLI, Antigravity CLI, Mastracode, Droid, Kimi, Letta, Devin, Hermes, Pi, OMP, Gemini CLI, GitHub Copilot CLI, Cline, and Qwen Code.
Each adapter validates its own upstream event contract and emits the same content-free normalized
facts through Orchbee's shared local socket/pipe plus private-APC correlation transport. The
Server's closed adapter registry, not an adapter payload, grants provider identity, accepted
events, and maximum capabilities. The [integration writer](../README.md) owns this source and
installs it; native staging and Desktop packaging copy it into each cohort.

## Windows Codex transport

The native Windows cohort uses the same v1 rich-envelope/private-APC join. The capsule
supplies Codex `commandWindows` hooks, using the provider's existing `cmd.exe /C`
execution contract and `PLUGIN_ROOT`. The Windows x64 cohort must include its pinned ConPTY runtime: the inbox ConHost may
consume private APC sequences. Windows does not gain Unix remote discovery or
additional provider installation support through this adapter change. Native managed
enable/disable and start/prompt remain separate CLI obligations.

| Field                    | Contract                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem and owner        | Codex command hooks in an Orchbee ConPTY must reach the native Server's existing observation ingress. The shared adapter owns delivery; Server/Host retain envelope validation, generation identity and the exact join.                                                                                                                                                                                                                                                |
| Inputs and reference     | Allowlisted normalized Codex events, the inherited `ORCHBEE_AGENT_OBSERVATION_SOCKET` and the provider-supplied plugin root. Codex 0.155.1's Windows hook contract accepts `commandWindows` and retains the parent's console despite redirecting standard streams. Environment strings never grant Agent or terminal identity.                                                                                                                                         |
| Endpoint policy          | Admit only the exact local pipe spelling `\\.\pipe\orchbee-<32-lowercase-hex>-observation`. Missing, malformed, remote UNC and non-observation targets produce no delivery. Windows never scans pipes, files or Unix SSH forwards. The injected endpoint and native Server ACL provide routing; the adapter does not claim to inspect pipe ownership.                                                                                                                  |
| Correlation target       | Use only the literal device `\\.\CONOUT$`, opened with read/write access and verified as a character device before writing. Never use a relative `CONOUT$`, environment-selected device, stdout or stderr. Direct visibility remains unknown. Missing console means no hint; no fallback may redirect the hint into a file.                                                                                                                                            |
| Workflow and transitions | Validate observation → resolve exact endpoint and fixed console target → send bounded content-free envelope → wait for exact bounded ACK → write the matching private APC → close the console handle. Missing endpoint, invalid input, timeout, rejected ACK and open/write failure stop the affected delivery. Rich-only events expire under the existing ingress rules.                                                                                              |
| Invariants and effects   | No prompt/output/path content enters the envelope; no APC precedes acceptance; neither half alone binds an Agent; no hook installs configuration or launches a Session. Windows never unlinks a pipe. The provider hook exits successfully on absent Node/runtime or transport failure. Socket and console handles remain locally owned and close on completion/failure.                                                                                               |
| Rollout and recovery     | Publish a new immutable Capsule version and matching resource/consumer versions. Git preserves LF bytes across Windows checkouts for the hashed payload. Preserve older Capsules and provider declarations until an explicit managed action replaces them. The Unix command field and existing Unix transport behavior remain available. The adapter adds no dependency or wire version; Windows cohort delivery additionally requires the pinned Host ConPTY runtime. |
| Evidence and limits      | Deterministic admission/ACK-order cases, real Windows named-pipe ACK cases, silent bounded command-hook process cases and an isolated native Server/ConPTY delivery witness. Transport evidence does not prove managed installation, real Agent task completion, physical TUI interaction or every provider's Windows support. Same-author challenge only; TLA execution remains deferred by user instruction.                                                         |

## Supported contracts

| Adapter            | Reference version                       | Integration surface      | Maximum status evidence                                                                                      |
| ------------------ | --------------------------------------- | ------------------------ | ------------------------------------------------------------------------------------------------------------ |
| Claude Code        | Current documented hook/plugin contract | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion, failure                           |
| Codex              | `0.150.1` (Unix), `0.155.1` (Windows)   | Command-hook plugin      | Session, Turn, tool, question, approval, compaction, subagent, completion                                    |
| Grok               | `1.0.5`                                 | Command-hook plugin      | Session, Turn, tool, approval, compaction, subagent, idle, completion, failure, cancellation, interruption   |
| OpenCode           | `1.3.4`–`1.18.30`; `2.2.5` Windows TUI  | JavaScript plugin events | Session, busy/retry/idle, tool, question, approval, error (1.x); displayed root session, busy/idle (2.x)     |
| Kilo               | `cb67014f`                              | JavaScript plugin events | Session, busy/retry/idle/offline question, chat, tool, question, approval, error                             |
| Pi                 | `0.84.4`                                | Extension API            | Session, Agent start/settled, blocking UI prompt, tool, compaction                                           |
| OMP                | `18.0.10`                               | Extension API            | Session, Agent start/end, Ask tool, tool approval, tool, compaction, retry                                   |
| Gemini CLI         | `20f70757`                              | Command-hook extension   | Session, Agent start/end, tool, question, permission, compaction, completion                                 |
| GitHub Copilot CLI | `1.0.81`                                | Command-hook plugin      | Session, Turn, actual question/permission prompt, tool result, compaction, completion, unrecoverable failure |
| Cline              | `3.0.60`                                | Plugin runtime API       | Run/Turn, tool, question, completion, failure, interruption                                                  |
| Qwen Code          | `c3a4058a`                              | Command-hook extension   | Session, Turn, tool, question, permission, compaction, subagent, completion, failure                         |

These are separate compatibility contracts. Pi and OMP deliberately do not share an adapter;
Gemini CLI and Qwen Code likewise retain separate manifests and event interpretation. Versions
outside the matrix are unsupported until their event fixtures and capability grants are reviewed.

## Explicit setup

### Execution host

Install the selected adapter on every machine where that Agent process runs. For
Orchbee → SSH → remote tmux/Zellij → Agent, installation therefore occurs on the remote host, and
an OpenSSH reverse Unix-socket forward provides transport only. Orchbee currently sets up no such
forward, so a remote adapter finds no endpoint. Orchbee never installs or changes an adapter
automatically. On the local machine, the user may explicitly enable or disable Orchbee's exact
entry with `orchbee agent integration enable` or `disable`, or through Desktop Settings, which
runs the same Worker operation. Orchbee never copies this resource to another host, edits remote
Agent configuration, or runs remote setup; remote installation remains manual.

Use a durable standalone copy of this directory as `<adapter-root>`. Do not persist a path into an
Orchbee application bundle: deleting or replacing the application must not break Agent startup.
A managed enable first publishes a validated Capsule under
`~/.orchbee/agent-observation-adapters/<payloadSha256>` and points the provider's entry at that
durable copy. Provider-local module entries
catch a missing Capsule, shell command hooks test that their runtime still exists, and Gemini/Qwen entries
are self-contained copies. An uninstaller must retain every Capsule still referenced by any
provider declaration; explicit disable owns removal of the corresponding provider entry.

Local Claude and Copilot inspection verifies both the exact plugin and its marketplace source;
Grok inspection verifies the copied plugin's recorded source and version. A sibling Capsule version
under the same installation parent reads as `update-required`, not an enabled current adapter, and
changes only through an explicit enable. Any same-named entry with a different source remains an
untouched conflict.

The [integration writer](../README.md) defines the managed transaction, exact managed entries,
conflict handling, and rollback, and the
[Windows integration contract](../../../apps/orchbee/docs/windows-agent-integrations.md) records
Windows paths and evidence. The commands below remain the manual setup path and are always
required on remote hosts.

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
`orchbee-agent-observation` to roll back. A managed disable uninstalls only a source-proven
Orchbee copy.

### OpenCode

On OpenCode 1.3.4 or later, add the absolute entry path
`<adapter-root>/bundle/opencode/orchbee-agent-observation.mjs` as a local plugin. Keep the durable
standalone root in place because the entry imports the shared transport beside it, and remove the
configured path to roll back. OpenCode 2.x admits only a directory as a configured plugin, so use a
managed enable there. A managed enable writes a provider-local guarded bootstrap into the
discovered `plugins` directory and, on Windows, the `orchbee-agent-observation-tui` client
directory whose TUI entry observes OpenCode 2.x; if the Capsule is later unavailable, each
bootstrap catches the failed import and returns no hooks or output.

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

Remove only that exact copied extension to roll back. A managed enable publishes and fingerprints
the same bounded self-contained layout atomically; modified, expanded, or unmarked directories remain
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

Remove only that exact copied extension to roll back. A managed enable publishes and fingerprints
the same bounded self-contained layout atomically; modified, expanded, or unmarked directories remain
untouched conflicts.

## Status fidelity

An adapter emits only evidence its upstream surface actually provides. OpenCode, Pi, and OMP
settlement becomes affirmative `Ready` without inventing a successful outcome, so it emits no
success notification. Copilot enters attention only for its actual `permission_prompt` or
`elicitation_dialog` notifications. Qwen starts a user Turn only when `submitted_prompt` proves user
provenance; its continuation callbacks are ignored. Cline intentionally claims no session
lifecycle, approval, or subagent capability because its plugin runtime does not prove those facts.
Exact errors remain failures where available. Cline keys every event by the root session id its
plugin `setup` context reports (`ctx.session.sessionId`), which is the id `cline --id` resumes.
Each sandboxed plugin instance serves one root session; if `setup` is absent, reports no valid id,
or reports a different id later, the adapter falls back for good to the run snapshot's
`conversationId`, which no resume accepts. Events whose snapshot names a non-empty
`parentAgentId` are subagent runs and are dropped, so they never appear as extra root sessions.
Cline records a completion before delivery and suppresses later events for that exact session and
run while its tombstone remains retained. The bounded cache keeps the 64 newest completed runs for
each of at most 128 tracked sessions. Run tombstones use completion-order FIFO eviction, and a new
completion refreshes its session before the least recently completed session is evicted. Either
eviction permits an older identity to be observed again rather than retaining unbounded plugin
state.
`ClineObservationOrdering` checks completion-before-start, duplicate completion, activity, session
isolation, and bounded eviction with two conversations and a two-run retention bound. Grok
cancellation is scoped by `promptId`, preventing a delayed cancellation report from ending a newer
Turn. Missing events, disabled hooks, unsupported versions, transport failure, and a fully
disconnected SSH path degrade to stale or unknown rather than guessed completion.

## Security and failure boundary

The Worker injects only `ORCHBEE_AGENT_OBSERVATION_SOCKET` into the terminals it launches, through
a non-persistent environment overlay. Before transport, each entry selects only bounded event, session, Turn,
tool-operation, and subagent identifiers and discards prompts, questions, tool input/output,
transcript paths, working directories, error text, notification text, and unknown fields. When the
Agent inherited a well-formed `ORCHBEE_RUN_TOKEN` from its managed launch command, the envelope
carries it as the optional `runToken`; a missing or malformed value is omitted. The rich
half goes to same-user private Unix-domain sockets or the injected native Windows observation pipe. A local process uses only the exact endpoint in
its environment; only a remote process without that overlay discovers same-user forwarded
candidates, and discovery requires an inherited non-empty `SSH_CONNECTION`, `SSH_CLIENT`, or
`SSH_TTY` marker. An ordinary local terminal without the exact overlay performs no socket scan.
Only after an endpoint returns the exact bounded v1 acceptance ACK does the adapter
write one content-free private APC correlation half directly to each resolved outer TTY. With no
exact ACK, the adapter writes no terminal control sequence. The Server requires an exact
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
cannot prove application-window visibility. Orchbee currently ships no shell integration that adds
the mode-`0600` remote Unix socket an OpenSSH reverse forward would carry. There is no
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
claude plugin validate crates/agent-integration-writer/agent-observation-adapters --strict
grok plugin validate crates/agent-integration-writer/agent-observation-adapters/bundle
pnpm test:agent-observation-adapters
```

The Server's [Agent observation model](../../../apps/orchbee-server/docs/agent-canonical-model.md)
owns the ingress, the join, and transport admission.

## Claude direct execution

The capsule launches Claude hooks with `node` plus an explicit argument vector, following the [upstream exec contract](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form). A fixed module expression imports the literal plugin script path through pathToFileURL and catches import failures. Script removal and malformed input remain silent; Node itself must exist on the provider PATH. If Node cannot start, Claude may display its non-blocking diagnostic. The integration never produces a tool permission decision.

This shared declaration allows native Windows onboarding without a shell dependency. Existing events, hook deadlines, input bounds, sanitization and transport ownership are unchanged. The [native Windows contract](../../../apps/orchbee/docs/windows-agent-integrations.md) owns installation and controlled Windows evidence; this change does not establish authenticated Agent task completion or new macOS/Linux workflow support. Old capsules remain immutable and explicit repair installs the current payload.

## Copilot direct execution

The capsule uses the [Copilot exec hook contract](https://docs.github.com/en/copilot/reference/hooks-reference) with Node. The fixed import guard reads COPILOT_PLUGIN_ROOT directly from the provider child environment, checks that it is absolute, and passes its joined script path through pathToFileURL. It emits no output when the root or script is absent. Node is a prerequisite; an unavailable executable is a provider launch error, not a delivered observation. PascalCase event names retain the documented VS Code compatible input fields used by the existing sanitizer. No shell interprets the path. Windows onboarding uses the separately verified current plugin command/JSON contract.

### Grok command bootstrap

The capsule keeps Grok's command-only hook contract with a fixed base64-encoded Node bootstrap. The decoded code reads GROK_PLUGIN_ROOT as data, sets the literal script/provider arguments, and performs a guarded dynamic import; it never inserts a user path into shell syntax. PowerShell 5.1/7 preserve redirected JSON stdin through this command. Node is a prerequisite; missing Node is a provider command error, while missing roots/scripts remain silent. Grok's existing camelCase event inputs, sanitizer, deadlines and observation transport are unchanged. The encoding protects shell quoting, not secrets. Windows local installation state is read separately from the provider-owned TOML configuration; it does not establish effective project policy or an active session's reload.

## OpenCode-family Windows onboarding

The capsule adds Kilo and shares its event normalizer with OpenCode. The shared factory admits lifecycle identities from sessionID or legacy info.id, rejecting disagreement. A `session.created` or `session.updated` event whose `info` carries a valid `id` and a non-empty `parentID` marks a child (subagent) session; the factory remembers at most 256 child ids, first in, first out, and drops every observation for them, including the child's own session start. A child created before the plugin loaded stays unknown and reads as a second root session. Callback arguments and outputs remain private. Delivery retains at most 64 outstanding sanitized events, in order; overflow/dispose stops new admission and drops the queued suffix until reload, while an in-flight send may finish. Transport failures are silent. Windows native onboarding installs discoverable .js bootstraps and preserves foreign/newer entries.

Both entries export one plugin definition, `orchbeeAgentObservationPlugin`, that both loader generations accept once the managed bootstrap default-exports it: an `id`, the `server` factory that OpenCode 1.3.4+ and Kilo call, and the `setup` that OpenCode 2.x validates and awaits. OpenCode 1.3.4+ also admits the entry directly, since it accepts an exported object with a `server` function. OpenCode 1.0–1.3.3 call every export as a plugin function, so no single file serves them and 2.x together; they are unsupported. OpenCode 2.x runs plugins in a server process that never shares the Agent's console: a detached shared service by default, or a hidden standalone child with its own console. The private correlation hint cannot reach the Agent's terminal from there, so `setup` subscribes to nothing. The OpenCode entry also exports `orchbeeAgentObservationClientPlugin`, the `{ id, setup }` definition of the Windows client directory. Loaded as a server plugin its setup does nothing; loaded as a TUI plugin, in the TUI process that owns the Agent's console, it reads that TUI's route and session data at setup and then once a second, and reports only the root of the session the TUI displays: started when first displayed, ended when another root replaces it, never again once left, with busy and idle from its status. It remembers at most 256 left roots, first in, first out, and shares the same bounded delivery. The bootstrap delegates each entry to the Capsule's definition, so a later Capsule can change either without a new bootstrap; the named-export bootstrap earlier releases wrote reads as update-required. See the [owning Windows model](../../../apps/orchbee/docs/windows-agent-integrations.md#kilo-windows-integration-and-shared-event-model) for paths, transitions and evidence.

## Gemini and Qwen command roots

The capsule executes fixed Node guards using ORCHBEE_AGENT_EXTENSION_ROOT in the provider's per-hook environment. Gemini supplies the value by recursively hydrating its external hook file; Qwen supplies it through recursively hydrated inline hooks in qwen-extension.json. Substitution affects an environment value, never command source. Both entries import their self-contained scripts and keep existing content-free event/transport bounds. Windows native onboarding and doctor share provider-specific directory overrides and refuse invalid explicit roots. Windows directory ownership checks retain exact marker and file-digest validation; unrecognized entry contracts remain refused. See the [Windows extension model](../../../apps/orchbee/docs/windows-agent-integrations.md#gemini-and-qwen-windows-extensions).

The Cursor adapter emits session-identified from consistent session/conversation identity aliases. It never derives working/idle or permission decisions. Windows native JSON onboarding installs the fixed encoded launcher; no authenticated provider execution is implied.

Qoder CLI uses qodercli identity-only observations for exact SessionStart/session_id input. The Windows native manager installs its encoded command group; Node is required and hook output remains an empty JSON object.

Antigravity CLI admits only conversationId from PreInvocation as session-identified; antigravity-cli is the adapter ID and antigravity is the canonical provider. Named Windows hooks return an empty JSON object and make no lifecycle or permission claims.

Mastracode admits content-free run, tool, approval and subagent facts from its ten selected hooks. AgentEnd alone determines the terminal outcome; suspended does not imply completion. See the [Windows contract](../../../apps/orchbee/docs/windows-agent-integrations.md#mastracode-windows-lifecycle-hooks).

Droid admits only SessionStart/session_id as session-identified, including resume and compaction. Its Windows manager edits the active configuration source; the adapter never infers idle or readiness.

Kimi admits exact SessionStart/SessionEnd with valid session_id as session-identified/session-ended. No turn completion is inferred from the blockable Stop hook. Native Windows owns the two-rule TOML installation.

Letta accepts exact SessionStart event_type and conversation_id. Default conversations require agent_id and become default:<agent_id>; only session identity is observed. Its Windows launcher emits no prompt feedback.

Devin accepts six exact hook_event_name values and valid session_id, emitting session-identified only. Prompt, tool and permission contents are discarded; Stop does not prove completion.

Hermes supplies a stdlib-only Python plugin that registers three identity callbacks and invokes the shared Node transport without a shell. It excludes noninteractive platforms and callback content, and returns no context or permission decisions.

## Packaged payload identity

The current Capsule uses contract version 1. Its manifest contains exactly
`{contractVersion: 1, payloadSha256: "<64 lowercase hex>"}`. Its identity and
installation directory are the digest: `~/.orchbee/agent-observation-adapters/<payloadSha256>`.
Provider package versions remain provider metadata and do not identify an installation.

Compute SHA-256 over the UTF-8 encoding of `JSON.stringify(entries)`. Entries use
`/`-separated relative paths, ordered by UTF-16 code units; directories serialize
as `{kind,path}`, files as `{kind,path,sha256,size}` in that member order. Exclude
only `capsule.json`; include both documentation files and every other packaged
file and directory. Every byte change, including formatting, requires recomputing
`payloadSha256` over the final tree. The Rust writer and native staging recompute
this digest before admitting the payload, with the same bounds and required files.

Doctor verifies installed bytes against their manifest, then compares that verified
identity with the current adjacent sidecar. An unstaged development binary uses
its embedded sidecar identity. A different digest is `update-required`; corrupt
bytes and uninspectable reads retain their distinct existing outcomes. Integration
CLI documents expose `capsule.payloadSha256`. Reinstall explicitly with
`orchbee agent integration enable --provider codex` (select the relevant provider).
Old directories are retained; disable removes provider entries under their existing
ownership guards and does not sweep capsules. Non-current capsule metadata is
diagnosed as outdated only after verifying its actual digest, required runtime
files and named Orchbee package marker. This read-only diagnosis never admits a
non-current source as ready or authorizes replacing unknown files. There is no
automatic migration or background cleanup.
