# Windows Agent integrations

This record owns the Windows refinement of native `agent integration list`,
`explain`, `enable`, and `disable`. The CLI owns local onboarding; it never starts
a Session. The shared provider classifier and mutation workflow remain authoritative
for ownership, payload identity, plans, receipts, and residuals.

## Model and boundaries

| Field          | Contract                                                                                                                                                                                                                                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | Windows users need to install, inspect, and remove the observation integration from the same CLI that launches their terminals.                                                                                                                                                                               |
| Inputs         | Provider ID, optional capsule source, native user profile, PATH, and Codex configuration root are untrusted paths. Empty CODEX_HOME uses the profile default; an invalid nonempty override never falls back.                                                                                                  |
| Data and owner | The provider owns its configuration. Orchbee owns only its content-addressed capsule and explicitly marked integration entries. Foreign, modified or uninspectable entries remain untouched.                                                                                                                  |
| Outputs        | Existing integration snapshots, plans, mutation receipts, typed refusals, and retained residual paths. Provider exit alone does not invent an inspected state.                                                                                                                                                |
| Workflow       | Admit platform and budget → prepare the capsule for enable → inspect provider ownership → run the provider vector → report observation or uncertainty. Cancellation stops subsequent effects and terminates the owned command tree.                                                                           |
| Publication    | Stage beside the target; copy regular entries only; validate the capsule; publish using atomic no-replace MoveFile. An occupied destination is a conflict. Backups use the same primitive.                                                                                                                    |
| Paths          | Local absolute drive paths only for managed effects. Reparse points and ambiguous components are refused. Ancestor directory handles deny delete sharing during each filesystem operation.                                                                                                                    |
| Process        | Resolve bounded absolute PATH entries. Native EXE and Windows CMD/BAT shims are admitted; PowerShell scripts are not implicitly executed. Arguments remain structured and Rust's batch-command quoting applies. Each command starts suspended, joins an owned Job Object, and resumes only after containment. |
| Bounds         | Existing 40-second invocation and 3.6-second command slot remain. Execution has a 3-second bound, stdout has a 256 KiB cap, and teardown polls without an unbounded wait.                                                                                                                                     |
| Transitions    | Before admission, cancellation/deadline is a no-effect result. After publication or provider launch, failures retain attempted steps and uncertainty. No retry may replace foreign data.                                                                                                                      |
| Dependencies   | Windows-only process-wrap 10.0.1 supplies safe suspended Job Object assignment missing from std; only std, job-object, and creation-flags are enabled. The matching windows 0.62.2 binding supplies the creation-flag type. Existing local-ipc owns nonblocking child output. No async runtime is added.      |
| Rollback       | Disable uses the existing ownership guards. Failed or interrupted publication reports retained staging/backup paths; the CLI never sweeps unrelated files.                                                                                                                                                    |
| Evidence       | Windows filesystem tests cover no-replace, Unicode paths, and reparse refusal; process tests cover argument preservation, output limits, cancellation, timeout, and descendants; isolated public CLI tests cover Codex onboarding and configuration isolation.                                                |
| Limits         | Provider authentication, remote installs, paid Agent tasks, and TUI interaction are outside onboarding evidence. TLA execution is excluded by user instruction. Review is a same-author adversarial pass, not independent evidence.                                                                           |

Original paths retain the 4,096 UTF-8 byte admission bound. After admission and ancestor pinning, MoveFile and the private ReplaceFile receive ordinary drive paths lexically normalized into extended form; existing verbatim drive paths pass unchanged. Logical paths in receipts and recovery records retain their original spelling. Long-path filesystem evidence covers publication conflicts, metadata streams and backup restoration under ordinary, mixed-separator and verbatim spellings.

An open descendant can temporarily make an atomic directory MoveFile return ACCESS_DENIED before publication. Only that directory failure permits three admission waits of 1, 5 and 10 milliseconds while the same ancestor handles remain pinned. Every attempt still uses no-replace; an arriving target returns conflict, and persistent denial returns failure. File moves and other errors do not wait. Controlled native-handle tests cover release, a competing arrival, persistent denial and unchanged source bytes; the provider workflow itself is never replayed.

Windows manages Claude Code, Codex, Grok and GitHub Copilot hooks, the OpenCode, Kilo, Pi and OMP JavaScript modules, Gemini/Qwen extensions, and Cursor JSON session hooks.
The other provider rows retain `manual-only` until their Windows hook execution path
is supported. Inspection and mutation share the same platform admission policy.

OpenCode uses its ESM entry; the Pi/OMP installed CommonJS bootstrap dynamically imports
the capsule's **ESM** entry and calls its named registration function. Importing their
CommonJS wrapper and looking for that named export would silently skip registration.
The deterministic bootstrap bytes and file URL establish ownership. A different identity under the same managed parent requires explicit repair; changed bodies and foreign roots remain refused.
Windows doctor derives the capsule root from the full `bundle/<provider>/<entry>` suffix,
so a healthy installed module is inspected against its capsule manifest instead of the
bundle directory. This is an offline file check and does not establish a live connection.

Public CLI witnesses use an isolated Unicode profile to install, inspect and remove all
four modules. Real Node loads each installed bootstrap and reports ordered provider
events through a private Windows pipe; a separate native ConPTY witness checks console
correlation into canonical sessions. These controlled provider APIs establish the
integration boundary, not provider authentication or the behavior of arbitrary releases.

## Dependency review

[process-wrap](https://docs.rs/process-wrap/10.0.1/process_wrap/) is MIT OR Apache-2.0,
has no build script, and adds indexmap 2.14.2 with already-resolved hashbrown 0.17.1.
The Windows binding adds windows-core, collections, future, numerics, result, strings,
threading, and its implement/interface proc macros. The proc macros reuse the existing
syn/quote/proc-macro2 versions. No duplicate package version or async runtime is added. Its Windows unsafe calls remain
inside the dependency; authored Rust keeps `unsafe_code = "forbid"`. The adapter
never calls its blocking wait method. It explicitly terminates the Job on every
exit path, including normal leader completion; dropping the std wrapper alone
does not provide kill-on-drop. Abrupt termination of Orchbee by an uncatchable OS
action remains outside this cooperative teardown claim.

## Maintenance filtering

Windows `agent integration list --outdated-only` performs the usual bounded local inspection once and shows only update-required entries, including owned installations needing repair. It never installs or downloads. `agent integration enable --provider <id>` remains the explicit guarded update/repair action; conflicting and uninspectable entries are not replaced.

Filtered lists use v1 with outdatedOnly true, inspectedCount, unresolvedProviderIds for unknown inspections and blockedProviderIds for conflicting entries. Complete is calculated from the entire selected inspection before filtering; an empty list cannot hide inspection failures. Text includes these counts and the same filtered entries. The default list uses v1. The current schema admits all 20 canonical providers. The flag has no non-Windows runtime; original 40-second deadline, cancellation, profile isolation and local-only ownership remain. Detailed installed-version reporting and remaining provider Windows hook support are separate obligations.

## Claude exec hooks

Claude Code is also managed on Windows with the current capsule. Its hook declaration uses the provider's `command` plus `args` exec form: Node receives the plugin script path as a literal argument, including spaces, apostrophes, Unicode, dollar signs and backticks. A fixed dynamic-import guard ignores a removed script; the existing bounded sanitizer and Windows pipe/console transport remain the only observation path. No Bash or PowerShell launcher is required. Node must remain on the provider PATH; if it is absent, Claude reports its non-blocking spawn diagnostic, and no observation is produced. This replaces the old Claude-only promise of silent success without Node. Other providers retain their own launch contracts. The [upstream exec contract](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form) defines argument substitution.

Enable, disable and repair reuse the existing user-scoped plugin commands and bounded Windows process owner. Old owned capsules require explicit repair; current enabled entries are no-ops, and foreign or modified entries remain refused. Isolated provider-command and shipped-hook witnesses cover onboarding and sanitized delivery. They do not establish authenticated Claude task completion. The shared portable hook declaration changes, but this work adds no macOS/Linux workflow or runtime validation.

## Copilot command compatibility

Windows targets the plugin contract verified with official Copilot CLI 1.0.88: `plugin list --json` returns a flat array, and plugin commands do not accept the retired `--kind` or `--plugin` switches. One fixed command owner supplies inspection, plans and effects. Inspection requires the exact marketplace, local installed origin, live source and declared provider metadata; foreign and malformed observations never authorize repair. The Unix adapter retains its earlier command and parsing contract. No speculative mutation retry is performed.

The capsule uses Copilot's exec hook form with Node. The fixed import guard reads the provider's COPILOT_PLUGIN_ROOT environment variable as a path, with no shell expansion, and keeps the existing PascalCase event input contract. Missing roots or scripts are silent; Node remains a prerequisite. The [plugin reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference) and [hook reference](https://docs.github.com/en/copilot/reference/hooks-reference) own upstream semantics. The existing local mutation budget, Job Object teardown, immutable publication, receipt and content-free observation rules remain. Controlled installation and hook delivery do not establish authenticated Agent task completion.

Provider package versions remain required inventory metadata, but never order capsule identities. Missing metadata is uninspectable; a different managed source digest requires repair.

## Windows TOML and Grok local state

Windows Codex inspection and mutation share one validated configuration root: a nonempty
`CODEX_HOME`, or `USERPROFILE/.codex` when it is absent or empty. Every owned Codex command
receives that root as its child-only `CODEX_HOME`; provider known-folder discovery cannot
select another profile. Invalid explicit roots refuse without fallback. A missing profile
default is created under guarded ancestors before provider commands, recorded as
`prepare-codex-home`, and included as a guarded write in the plan. Existing roots stay pinned
through the command sequence; explicit nonempty roots must already exist. Successful provider
commands are followed by a fresh bounded inspection of the same configuration; an unobserved
installation/removal returns `verification-failed` instead of a completed receipt. Isolated
CLI tests cover default and explicit roots, empty overrides and successful exits with no effect.

Windows Codex and Grok inspection parses each complete UTF-8 TOML document within the existing 512 KiB read bound. Syntax errors, duplicate assignments, invalid UTF-8 and unreadable data yield unknown; valid quoted/dotted keys, inline tables and trailing comments retain their meaning. Parsing never writes configuration or echoes its contents. Typed plugin booleans and exact local source ownership remain required. The Windows-only toml 1.1.6 dependency enables std, parse and serde with defaults disabled; serialization/display, unsafe parser acceleration and SIMD are not enabled. Its added tree is serde_spanned 1.1.1, toml_datetime 1.1.1, toml_parser 1.1.3 and winnow 1.0.4, reusing existing serde_core 1.0.229. The read-only toml dependency does not require display; Kimi editing below additionally compiles the already-locked toml_writer 1.1.2. These add no build scripts or proc macros; toml-family licenses are MIT OR Apache-2.0 and winnow is MIT. Winnow contains internal unsafe slice operations; authored code remains unsafe-forbidden. The legacy Unix parser is unchanged.

Grok's installed JSON inventory does not report activation. Windows joins the exact owned bundle source identity to the user configuration in nonempty GROK_HOME, otherwise USERPROFILE/.grok. Invalid overrides never fall back. Explicit disabled entries win; an unlisted user plugin is disabled. This is local onboarding state: project/managed policies and already-running sessions remain provider-owned. Enable uses the existing install/trust or enable commands; disable removes only the source-proven plugin. Provider package version metadata does not override the source identity.

The capsule supplies Grok's command-only hook as a fixed Node bootstrap encoded into the declaration. No user path or hook input becomes shell source. Node reads GROK_PLUGIN_ROOT and imports the joined script with pathToFileURL; missing roots/scripts are silent, while Node is a prerequisite. Existing input, event, transport and hook deadline bounds remain. Real PowerShell 5.1/7 witnesses exercise the shipped command and redirected JSON stdin in isolated profiles; no authenticated Grok task or new macOS/Linux workflow is implied.

## OpenCode automatic discovery

Windows installs the guarded OpenCode bootstrap as `plugins/orchbee-agent-observation.js`. The provider's directory loader scans .js/.ts, so direct Node loading of the previous .mjs target did not establish automatic discovery. The capsule source stays .mjs. A shared Windows path owner resolves nonempty absolute XDG_CONFIG_HOME/opencode, otherwise USERPROFILE/.config/opencode, for integration and doctor. Invalid explicit roots never fall back.

Inspection combines the current .js and obsolete .mjs targets. Foreign, modified or unreadable entries block mutation; an owned obsolete entry requires repair. Enable publishes/verifies the current entry before reinspecting and removing the obsolete owned entry. Failure or cancellation preserves committed effects and reports their receipts; it never deletes a usable current entry as compensation. Disable applies the same guards to both targets. Explain reports the guarded write/removal from its existing inspection, without another read. Doctor checks the discovered .js entry; old-only installations are absent there and require repair in the integration query. Existing bounds and no-replace primitives remain. An isolated official OpenCode 1.18.30 debug-config witness confirms automatic discovery; authentication and paid Agent tasks are outside this evidence.

Windows regular-module inspection validates the entire fixed bootstrap against its decoded absolute file URL, then classifies the exact capsule root and payload-relative suffix. Old owned modules can be repaired; malformed or unverifiable entries refuse replacement; edited bodies and foreign roots remain conflicts. The 16 KiB reader never imports the referenced module. This applies to the existing OpenCode, Kilo, Pi and OMP module owner; Unix inspection is unchanged.

## Kilo Windows integration and shared event model

Kilo is a Windows-managed native integration with the same guarded module ownership, payload-identity, deadline, cancellation and receipt rules as OpenCode. Its discovered entry is plugin/orchbee-agent-observation.js under a nonempty absolute KILO_CONFIG_DIR, otherwise XDG_CONFIG_HOME/kilo, otherwise USERPROFILE/.config/kilo. Invalid explicit roots refuse without fallback; doctor shares this resolution. Enable publishes the immutable capsule and exact marked bootstrap; disable removes only the proven entry. No provider command, project configuration or authentication is changed. Other platforms remain manual-only for Kilo. Local installation does not override KILO_PURE or establish an authenticated task.

The shared OpenCode-family normalizer consumes untrusted event/callback objects and returns only canonical observation identities and enums. Created/deleted events accept sessionID or legacy info.id; supplied IDs must be valid and agree. Other events require sessionID. Current status objects map busy/retry/idle; Kilo also accepts legacy status strings and maps its offline request to a question. Chat and tool callbacks never inspect or alter their output/arguments. Unknown shapes are silent. No content, path, prompt, output or permission decision crosses this boundary.

Each plugin instance owns a FIFO of at most 64 outstanding normalized observations. Delivery is serial, best effort and bounded by the existing transport. At overflow it stops admission until reload and discards the queued suffix; an in-flight send may finish. Dispose uses the same stop rule. It never resumes after dropping unknown lifecycle evidence or fabricates completion. Transport failure is silent and does not poison later admitted sends. Deterministic tests cover normalization, identity ambiguity, ordering, overflow, failure and disposal; a real Node bootstrap/private Windows pipe witness covers installation through delivery. These controlled witnesses do not prove authenticated provider work.

References: [Kilo plugin API](https://github.com/Kilo-Org/kilocode/blob/cb67014ffdb39ffe890e3a47a0a9c35c0e63538b/packages/plugin/src/index.ts), [Kilo global paths](https://github.com/Kilo-Org/kilocode/blob/cb67014ffdb39ffe890e3a47a0a9c35c0e63538b/packages/core/src/global.ts), [OpenCode legacy lifecycle](https://github.com/anomalyco/opencode/blob/v1.1.25/packages/opencode/src/session/index.ts), and [OpenCode current lifecycle](https://github.com/anomalyco/opencode/blob/v1.18.30/packages/schema/src/v1/session.ts). The capsule carries the shared module; the canonical observation registry adds Kilo, while Desktop onboarding retains its own provider boundary.

## Gemini and Qwen Windows extensions

Windows manages self-contained Gemini and Qwen extension directories using the existing guarded publication and removal workflow. Gemini resolves an absolute nonempty GEMINI_CLI_HOME as a home parent and installs under .gemini/extensions/orchbee-agent-observation; Qwen resolves QWEN_HOME as the configuration root and installs under extensions/orchbee-agent-observation. Missing overrides use USERPROFILE/.gemini and USERPROFILE/.qwen respectively. Invalid explicit roots refuse without fallback. Doctor shares these paths. Global/project activation policies and already-running sessions remain provider-owned; the local artifact state does not establish effective delivery.

After proving the directory marker and complete file fingerprint, Windows also requires the exact extension name and declared provider version metadata. Positive non-current owned contracts require reinstall; missing metadata is unknown and modified or foreign content conflicts. Existing cancellation, backup restoration, committed-prefix receipts and residual ownership remain. Enable copies the source and shared scripts into one self-contained directory; disable removes only a reverified owned directory.

The capsule uses a fixed Node command that reads the provider-hydrated ORCHBEE_AGENT_EXTENSION_ROOT environment value. User paths never enter shell source. Gemini recursively hydrates its hooks/hooks.json; Qwen recursively hydrates inline hooks in qwen-extension.json, so Qwen has no duplicate external hooks file. The guard imports the joined forward-hook.mjs, preserving existing input bounds and event semantics. Node is required; missing roots/scripts or invalid input are silent. Source contracts are [Gemini extension loading](https://github.com/google-gemini/gemini-cli/blob/20f707577424c0c60cde3470b1495f228773e307/packages/cli/src/config/extension-manager.ts) and [Qwen extension loading](https://github.com/QwenLM/qwen-code/blob/c3a4058a0c7207de7b421e9da0622a9ec890dbad/packages/core/src/extension/extensionManager.ts). Controlled PowerShell 5.1/7 and cmd witnesses exercise the shipped commands with raw JSON stdin and a private Windows observation pipe; no authenticated provider task is implied.

On Windows, the staged Qwen manifest explicitly selects `shell: powershell` for each owned command hook before its fingerprint is written. The upstream default cmd spawn can quote the complete Node expression into an inert string; provider exit 0 alone is insufficient evidence. Gemini already selects PowerShell on Windows. The shared capsule remains portable; only native Windows Qwen installation applies this projection. The extra direct cmd witness passes verbatim command source and is distinct from upstream default-shell dispatch.

## Qoder Windows JSON hooks

Qoder uses the same Windows publication workflow with a different JSON shape. Provider `qodercli`
resolves executable `qoder` and nonempty absolute `QODER_CONFIG_DIR`, otherwise
`USERPROFILE/.qoder/settings.json`. Invalid explicit roots never fall back. Its `SessionStart`
entry is one group containing one command hook; only the exact generated group is owned. Changed
matchers, extra hooks within that group, modified commands or duplicate/misplaced owned groups
conflict. Other groups, events and raw settings are preserved, including unrelated `version` fields.
Cursor keeps its flat version-1 hook format.

The fixed encoded launcher selects `shell: powershell`, uses a ten-second provider timeout, forwards
only a valid `session_id` from `hook_event_name: SessionStart` as `session-identified`, and returns
`{}`. Paths remain encoded data. Resume/compaction may repeat the identity without changing turn
state. Node remains a prerequisite. Local artifact ownership does not prove effective project or
organization policy, process liveness or authenticated work. Tests exercise public setup, override
resolution, repair/refusal, foreign preservation, doctor agreement and installed commands through
PowerShell 5.1/7 and cmd. Source contracts: [Qoder hooks](https://docs.qoder.com/cli/hooks),
[configuration paths](https://docs.qoder.com/cli/settings-reference), and
[native Windows installation](https://docs.qoder.com/cli/installation).

## Antigravity Windows named hooks

Integration `antigravity-cli` uses executable `agy` and canonical Agent identity `antigravity`.
It edits the documented `USERPROFILE/.gemini/config/hooks.json`, a global customization location
shared with other Antigravity surfaces. Other settings and plugin locations remain provider-owned.

Only the `orchbee-agent-observation` named block belongs to this integration, and its exact fixed
launcher plus single `PreInvocation` handler must prove ownership. Other named blocks remain opaque
raw JSON; commands copied under another name are not owned. Optional boolean `enabled` defaults to
true. Explicit false reports disabled; malformed flags or edited owned content refuse mutation.
Enable writes the current enabled block; disable removes an active owned block. An already-disabled
block retains the existing no-op behavior. Source ownership guards still prevent replacement of foreign or modified entries. Doctor reports the disabled fact with the separately inspected capsule root.

The encoded PowerShell command returns `{}` and observes only valid `conversationId` as
`session-identified`; a supplied event name must be `PreInvocation`. It never forwards transcript
paths, content or permissions and cannot infer idle/readiness from repeated model invocations.
Existing bounded JSON publication and metadata preservation apply. Core cases and installed-command
Windows pipe tests cover ownership, disabled state, refusal and identity delivery; authenticated
Agent execution remains separate. Sources: [Antigravity hooks](https://antigravity.google/docs/hooks?app=antigravity)
and [native installation](https://antigravity.google/docs/cli/install/).

## Cursor Windows JSON hooks

Windows JSON hook onboarding owns only its exact generated command entries. It preserves unrelated raw JSON values, rejects duplicate members in edited objects, and refuses malformed, changed or foreign entries. Cursor uses version-1 `hooks.json` in nonempty absolute CURSOR_CONFIG_DIR, otherwise USERPROFILE/.cursor; its adapter reports session identity only. The fixed PowerShell EncodedCommand launcher decodes the capsule path as data, invokes the bounded Node observation runtime and returns an empty JSON object. It cannot approve or modify Agent work.

Configuration replacement compares the original bytes before and after moving them aside, publishes without replacing an arrival, verifies the result and removes only the unchanged backup. Read handles deny in-place writes during this frontier. Cancellation, deadline or conflict keeps unrestored backups and uncertain stages in the mutation receipt. This optimistic workflow is not global filesystem CAS or crash-atomic replacement: the target may briefly be absent, and an abrupt process loss can leave sibling recovery files. It does not protect against a malicious same-user rename/delete actor. User configuration policies remain provider-owned.

The Windows CLI reuses base64 0.22.1 with alloc only for canonical PowerShell encoding; it has no runtime dependencies, build scripts or proc macros and forbids unsafe code. Existing serde_core 1.0.229 with std supplies duplicate-rejecting object visitors; serde_json's raw_value feature preserves unrelated numbers and subtrees without numeric conversion. These reuse locked versions and add no transitive package. Tests cover changed snapshots, destination arrivals, cancellation, exact ownership and raw-value preservation; controlled Hook execution does not establish an authenticated Cursor session.

Before publication, Windows ReplaceFile merges the verified private original’s DACL, attributes and named streams into the staged result with no ignored merge errors. It keeps the original under a second private name; that exact backup remains until verification completes. The live target still receives only a no-replace publication. Partial metadata-merge failures re-inspect both original locations and retain unproven names; custom DACL and alternate-stream witnesses cover the successful path.

Cursor supports the native Windows `agent` executable. The [Hook reference](https://cursor.com/docs/hooks) owns its flat event arrays and session identity fields; [CLI installation](https://cursor.com/docs/cli/installation) owns native Windows availability. The encoded PowerShell launcher keeps fixed JavaScript single quotes inside a double-quoted argument so Windows PowerShell 5.1 preserves them. A controlled public CLI witness covers installed commands through PowerShell 5.1, PowerShell 7 and cmd, malformed input, missing runtime, content identity, custom DACL and alternate streams.

## Mastracode Windows lifecycle hooks

The native manager edits `USERPROFILE/.mastracode/hooks.json`, whose root maps events to flat command arrays. It preserves foreign entries and opaque values. Each selected event owns exactly one generated command with `timeout: 10000` milliseconds. All owned commands must name the same capsule root; duplicates, altered entries and misplaced commands conflict. Missing entries in an otherwise exact owned set require repair. Exact root ownership governs repair and refusal. Disable removes only proven owned entries. The shared bounded publication preserves Windows metadata. Doctor records the root and reports an incomplete set as update-required after checking that root. Project hooks remain provider-owned.

SessionStart identifies the session without inferring idle; SessionEnd ends it. AgentStart starts a run keyed by `run_id`. AgentEnd is the sole terminal signal: complete succeeds, aborted cancels, error fails, and suspended emits nothing, preserving pending attention. Tool events carry activity; permission request/result events create and resolve attention without making permission decisions. Subagent events use `tool_call_id` as child identity. Invalid identifiers and unknown reasons are rejected; content, tool arguments, paths and results never leave the sanitizer.

UserPromptSubmit is not installed because its unkeyed start would precede the keyed AgentStart. Stop and Interrupt would duplicate termination; asynchronous Notification lacks the same operation ordering. These events are not normalized. Controlled installed-command Windows pipe and canonical ingress traces prove the selected lifecycle, not authenticated provider execution, policy activation or interactive readiness. The fixed encoded PowerShell launcher returns `{}` through the provider's verbatim `cmd /c` dispatch. Node remains required.

The upstream [event types](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/types.ts), [configuration loader](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/config.ts) and [executor](https://github.com/mastra-ai/mastra/blob/3d2534080417711d1baf2ad947d1205ca95a34cd/mastracode/sdk/src/hooks/executor.ts) define the provider boundary.

## Droid Windows configuration sources

Droid uses the first existing supported user hook source: `.factory/hooks.json`, legacy `.factory/hooks/hooks.json`, then the `hooks` field of `.factory/settings.json`, all under USERPROFILE. An unreadable or unsafe higher-priority path prevents fallback. If all are absent, onboarding creates the preferred hooks.json. The selected source is edited in place; other sources are not copied, archived or migrated. Thus enabling preserves existing fallback hooks without creating a file that shadows them. External changes to source precedence can change which configuration is effective; unselected files remain provider-owned.

The shared path authority returns typed candidates, including whether the document is a standalone event map or nested settings. Onboarding and doctor read those candidates through their own bounded filesystem adapters. Mutation resolves its source again and performs the existing guarded publication; fresh inspection observes the resulting active configuration. This does not make several files atomic or reload an already-running Droid session.

One exact generated SessionStart matcher group is owned, with a command timeout of ten seconds. Foreign groups and raw settings remain intact. The adapter requires exact SessionStart and a valid session_id, then emits only session-identified. Startup, resume and compaction do not infer idle or readiness. The encoded Windows launcher returns `{}`; it never supplies context or permission decisions. Managed-only and disabled-hook policy are not changed, and installed configuration alone does not prove effective policy or delivery.

Sources: [Factory hooks](https://docs.factory.ai/harness/hooks) and [Windows settings](https://docs.factory.ai/droid-cli/settings). Native source-priority, preservation and installed-command pipe witnesses cover these local contracts; authenticated Agent execution remains separate.

## Kimi Windows TOML hooks

Kimi uses the current native Windows `kimi` command and nonempty absolute
`KIMI_CODE_HOME/config.toml`, otherwise `USERPROFILE/.kimi-code/config.toml`.
Invalid explicit roots refuse without fallback. Legacy `.kimi` data is untouched;
installation does not invoke migration or change credentials, models or permissions.

The configuration owner manages exactly two `[[hooks]]` rules: `SessionStart` and
`SessionEnd`, each with only event, fixed encoded command and timeout 10 seconds.
Inline hook arrays are also supported. Full command round-trip, exact fields and
one common capsule root prove ownership. Missing owned rules require repair;
duplicated, modified, misplaced, mixed-root or foreign entries refuse.
Other rules and settings retain their TOML values, comments and formatting through
a syntax-preserving editor. Newline spelling can normalize. The existing bounded
Windows publication, metadata preservation, recovery and residual rules apply to
both JSON and TOML; path selection is shared with offline doctor.

The pure JSON, Kimi and event-set scanners share the original six ownership states with doctor.
They preserve the raw parsed document for Windows mutation and use explicit Windows root-admission
and equality policies. Candidate location, bounded reads, resource-root observation and publication
remain Windows-owned; common parsing does not enable configuration observation on unsupported hosts.

Only valid `session_id` from exact `SessionStart`/`SessionEnd` inputs is forwarded,
as identity-only/session-ended respectively. No prompt, path or response is sent.
Node remains required; the fixed PowerShell command returns `{}` and is exercised
through the provider's Windows cmd shell. This slice establishes session tracking,
not complete Kimi working/blocked/idle parity: current `Stop` runs before blocking
hooks can continue the turn, and asynchronous lifecycle events do not supply one
ordered, uniformly correlated stream. Those status gaps remain open. Local setup
also does not prove effective policy or authenticated task completion.

The common `toml_edit` 0.25.15+spec-1.1.0 dependency enables only parse/display, with defaults
disabled. It has no unsafe source, build script or proc macro, is MIT OR Apache-2.0,
and reuses the existing indexmap, toml_datetime, toml_parser and winnow versions.
Previously locked toml_writer 1.1.2 now supplies display. Winnow enables its standard
ASCII/binary parser features; SIMD remains disabled. Its optional memchr edge appears
in the lockfile without enabling that feature. No duplicate version or new runtime is required. Existing transitive unsafe code stays dependency-owned.
Shared Kimi inspection and existing configuration witnesses use the same dependency as Windows
editing. Remove it when the last inspection, editing and fixture consumer disappears.

Evidence includes lossless foreign-rule fixtures, both TOML representations,
partial repair and refusal, isolated public lifecycle/doctor/override checks,
installed PowerShell 5.1/7 and cmd delivery, and shared JSON regression. No paid
provider session is needed. The capsule and matching receivers roll out together;
older receivers reject the new adapter. Disable removes only proven owned rules.

Sources: [Kimi configuration](https://moonshotai.github.io/kimi-code/en/configuration/config-files),
[hooks](https://moonshotai.github.io/kimi-code/en/customization/hooks), and
[pinned provider implementation](https://github.com/MoonshotAI/kimi-code/tree/be7d5f5fea7800778e4660cd5f36780ba783bddd/packages/agent-core-v1/src/features/externalHooks).

## Letta Windows session hooks

Letta resolves nonempty absolute `HOME`, otherwise the native user profile, and
uses `.letta/settings.json`. Invalid explicit roots refuse without fallback;
`LETTA_HOME` is not treated as an upstream configuration override. Windows manages
one exact `hooks.SessionStart` command group with timeout 10000 milliseconds and
`quiet: true`. Foreign groups, settings and raw future fields are preserved,
including `hooks.disabled`. Existing ownership, content identity, metadata,
backup/recovery and cancellation rules apply. This is local artifact inspection;
project/global hook policy is unchanged and effective-policy diagnosis remains open.

The fixed encoded launcher is silent on stdout and stderr: Letta consumes hook
stdout as prompt context, so even `{}` would alter the user's conversation. Other
providers retain their required responses. Node remains a prerequisite. The adapter
requires exact `event_type: SessionStart`, boolean `is_new_session` and a valid
`conversation_id`. A `default` conversation requires valid `agent_id` and uses
`default:<agent_id>` within the same identity bound, preventing distinct Agents from
merging. Only `session-identified` is emitted; no prompt, idle, readiness or outcome
is inferred. Missing/invalid payloads remain silent. Resume preserves turn state.

Pure ownership and identity tests plus isolated public setup/repair/refusal/disable,
HOME override, doctor and installed PowerShell 5.1/7/cmd pipe witnesses cover this
contract. They do not establish authenticated Letta work. The capsule and its
receivers form one cohort; rollback selects the previous cohort, and disable removes
only proven owned declarations. No new dependency or other-platform workflow is added.
Source: [Letta 0.33.2 hook implementation](https://github.com/letta-ai/letta-code/tree/46e1f82ba44145e8b8d0ac283c2470b59822715e/src/hooks).

## Devin Windows session hooks

Devin uses the documented Windows user configuration at absolute nonempty
`APPDATA/devin/config.json`. Missing or invalid APPDATA refuses; no speculative
profile fallback or undocumented Windows XDG override is applied. Project hooks,
configuration imports, permissions and MCP remain provider-owned and unchanged.
The bounded strict JSON reader refuses unparseable input without rewriting it;
provider support for JSON comments is not established by this implementation.

Windows manages six exact command groups: SessionStart, UserPromptSubmit,
PreToolUse, PostToolUse, PermissionRequest and Stop. Each uses timeout 10 seconds
and the fixed encoded launcher returning `{}` without a decision or context.
The complete set must reference one owned capsule root. Missing entries require
repair; duplicate, modified, misplaced, mixed-root or foreign entries
refuse. Other settings/groups retain their raw values. The existing multi-event
owner now serves Mastracode's flat map and Devin's nested groups; publication,
metadata preservation, cancellation and recovery rules remain shared.

Only exact event names and valid session_id emit session-identified. Repeated
identity preserves turn state. No prompt_id, content, cwd-based session search,
idle, permission decision or completion is inferred. Local
artifact inspection does not establish effective policy or a live Agent task.

Pure set-ownership tests, isolated public installation/repair/refusal/removal and
doctor checks, plus six installed commands through PowerShell 5.1/7 and cmd/private
pipe establish the controlled boundary. They do not independently establish the
real Devin hook dispatcher. Discovery used only the installed CLI's help/version;
no Devin Agent task or orchestration runs.
The capsule rolls out with its matching receivers; rollback uses the previous
cohort. Fixtures isolate APPDATA as well as the native profile. No new dependency
or other-platform workflow is introduced.

Sources: [Devin hooks](https://docs.devin.ai/cli/extensibility/hooks/overview),
[lifecycle](https://docs.devin.ai/cli/extensibility/hooks/lifecycle-hooks), and
[Windows configuration](https://docs.devin.ai/cli/reference/configuration/config-file).

## Hermes Windows session plugin

Hermes owns its configuration and plugin activation. Windows resolves a nonempty
trimmed absolute literal HERMES_HOME, otherwise LOCALAPPDATA/hermes, falling back
to the native profile's AppData/Local/hermes when LOCALAPPDATA is empty. A safe
single-component HERMES_DATA_DIR_SUFFIX applies only to the default. Relative or
unexpanded explicit roots refuse. Provider commands receive the resolved
HERMES_HOME, so inspection and activation select the same profile.

Orchbee owns only plugins/orchbee-agent-observation/**init**.py and plugin.yaml.
The manifest is JSON, a YAML subset; each entire generated file proves its capsule
root. Foreign or edited members refuse. One missing member or independently
proven outdated/current identities require repair, including interrupted two-file updates.
Enable publishes the entry then manifest through the shared bounded Windows file
publication primitive, preserving metadata and recoverable preimages. It then
runs Hermes's own plugins enable command and verifies plugins list --json. Disable
runs plugins disable and retains both files. Python caches, other plugin files,
configuration YAML and capability grants remain provider-owned. No directory is
removed. A disabled partial/older entry still reports update-required for repair.
Provider admission failure or timeout preserves observed effects and
recovery paths; activation requires a subsequent provider session.

The Python plugin registers on_session_start, on_session_reset and pre_llm_call.
Only cli/tui/desktop/acp platforms are admitted, and pre_llm_call is CLI-only.
Strict session identity crosses a bounded, hidden, shell-free Node subprocess to
the existing observation transport. Callbacks return no context or decisions;
only session-identified is emitted. Content, working/idle and completion are not
inferred. Offline doctor reports provider-command-inspection-required rather than
claiming that an existing plugin is enabled.

Pure file/list classification, isolated public activation/repair/refusal/disable,
and Python import plus real Windows pipe witnesses cover these boundaries. They
do not establish authenticated Hermes work or live-session reload. The capsule
and matching receivers roll out together; rollback selects the previous cohort.
No new dependency or other-platform implementation is added.
Source: [pinned Hermes plugin implementation](https://github.com/NousResearch/hermes-agent/tree/2a05208012cf50bbe8ff7b7b54a3825fd1e4692a/hermes_cli)
and [native Windows](https://hermes-agent.nousresearch.com/docs/user-guide/windows-native).

## Current Capsule identity

The Capsule contract is 1 and its identity is `payloadSha256`, with the exact manifest and canonical digest rules in the [adapter guide](../../desktop/resources/agent-observation-adapters/README.md#packaged-payload-identity). CLI capsule objects expose that digest; the directory is `~/.orchbee/agent-observation-adapters/<payloadSha256>`. No semver ceiling, special legacy root or automatic migration is retained. Any final payload-byte change produces a different identity, including documentation and formatting.

Doctor checks installed bytes against their own manifest and the current sidecar identity. A self-consistent older identity is `update-required`. Non-current capsule metadata also requires reinstall only after verifying the actual digest, required runtime files and named Orchbee package marker. Corrupt, unsafe or unreadable inputs retain their protected outcomes; this read-only diagnosis never admits a non-current source as ready. Explicit repair uses `orchbee agent integration enable --provider codex` (select the provider). Conflicting or uninspectable entries still require manual review. Existing directories remain untouched; disable follows the existing provider-entry removal rules. Rollback selects a complete retained cohort.

The Windows public doctor witness copies the executable, current sidecar and installed tree into an isolated profile, changes runtime and documentation bytes, refreshes the source manifest with Desktop's digest implementation, and checks for the update state and localized reinstall command without modifying the installed tree. The Unicode digest vector distinguishes UTF-16 path order from UTF-8 byte order.
