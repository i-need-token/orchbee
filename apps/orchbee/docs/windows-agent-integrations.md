# Windows Agent integrations

This record owns the Windows refinement of native `agent integration list`,
`explain`, `enable`, and `disable`. The CLI owns local onboarding; it never starts
a Site. The shared provider classifier and mutation workflow remain authoritative
for ownership, version direction, plans, receipts, and residuals.

## Model and boundaries

| Field          | Contract                                                                                                                                                                                                                                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Problem        | Windows users need to install, inspect, and remove the observation integration from the same CLI that launches their terminals.                                                                                                                                                                               |
| Inputs         | Provider ID, optional capsule source, native user profile, PATH, and Codex configuration root are untrusted paths. Empty CODEX_HOME uses the profile default; an invalid nonempty override never falls back.                                                                                                  |
| Data and owner | The provider owns its configuration. Orchbee owns only its versioned capsule and explicitly marked integration entries. Foreign or newer entries remain untouched.                                                                                                                                            |
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

Windows manages Claude Code, Codex, Grok and GitHub Copilot hooks, the OpenCode, Kilo, Pi and OMP JavaScript modules, and Gemini/Qwen extensions.
The other provider rows retain `manual-only` until their Windows hook execution path
is supported. Inspection and mutation share the same platform admission policy.

OpenCode uses its ESM entry; the Pi/OMP installed CommonJS bootstrap dynamically imports
the capsule's **ESM** entry and calls its named registration function. Importing their
CommonJS wrapper and looking for that named export would silently skip registration.
The module refinement keeps its existing bootstrap behavior with payload 0.4.6. The deterministic bootstrap bytes and file URL remain the ownership
proof: changed, foreign or newer entries are refused; older owned entries can be repaired.
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

Windows `agent integration list --outdated-only` performs the usual bounded local inspection once and shows only update-required entries, including owned installations needing repair. It never installs or downloads. `agent integration enable --provider <id>` remains the explicit guarded update/repair action; conflict and newer entries are not replaced.

Filtered lists use v2 with outdatedOnly true, inspectedCount, unresolvedProviderIds for unknown inspections and blockedProviderIds for conflict/newer entries. Complete is calculated from the entire selected inspection before filtering; an empty list cannot hide inspection failures. Text includes these counts and the same filtered entries. The default list keeps v1 and its existing behavior. The flag has no non-Windows runtime; original 40-second deadline, cancellation, profile isolation and local-only ownership remain. Detailed installed-version reporting and remaining provider Windows hook support are separate obligations.

## Claude exec hooks

Claude Code is also managed on Windows with capsule 0.4.6. Its hook declaration uses the provider's `command` plus `args` exec form: Node receives the plugin script path as a literal argument, including spaces, apostrophes, Unicode, dollar signs and backticks. A fixed dynamic-import guard ignores a removed script; the existing bounded sanitizer and Windows pipe/console transport remain the only observation path. No Bash or PowerShell launcher is required. Node must remain on the provider PATH; if it is absent, Claude reports its non-blocking spawn diagnostic, and no observation is produced. This replaces the old Claude-only promise of silent success without Node. Other providers retain their own launch contracts. The [upstream exec contract](https://code.claude.com/docs/en/hooks#exec-form-and-shell-form) defines argument substitution.

Enable, disable and repair reuse the existing user-scoped plugin commands and bounded Windows process owner. Old owned capsules require explicit repair; current enabled entries are no-ops, and foreign/newer entries remain refused. Isolated provider-command and shipped-hook witnesses cover onboarding and sanitized delivery. They do not establish authenticated Claude task completion. The shared portable hook declaration changes, but this work adds no macOS/Linux workflow or runtime validation.

## Copilot command compatibility

Windows targets the plugin contract verified with official Copilot CLI 1.0.88: `plugin list --json` returns a flat array, and plugin commands do not accept the retired `--kind` or `--plugin` switches. One fixed command owner supplies inspection, plans and effects. Inspection requires the exact marketplace, local installed origin, live source and payload version; foreign, newer and malformed observations never authorize repair. The Unix adapter retains its earlier command and parsing contract. No speculative mutation retry is performed.

Capsule 0.4.6 uses Copilot's exec hook form with Node. The fixed import guard reads the provider's COPILOT_PLUGIN_ROOT environment variable as a path, with no shell expansion, and keeps the existing PascalCase event input contract. Missing roots or scripts are silent; Node remains a prerequisite. The [plugin reference](https://docs.github.com/en/copilot/reference/copilot-cli-reference/cli-plugin-reference) and [hook reference](https://docs.github.com/en/copilot/reference/hooks-reference) own upstream semantics. The existing local mutation budget, Job Object teardown, immutable publication, receipt and content-free observation rules remain. Controlled installation and hook delivery do not establish authenticated Agent task completion.

A newer reported Claude or Copilot plugin version blocks Windows repair even when its marketplace root is an older owned capsule. Missing version data cannot authorize replacement.

## Windows TOML and Grok local state

Windows Codex and Grok inspection parses each complete UTF-8 TOML document within the existing 512 KiB read bound. Syntax errors, duplicate assignments, invalid UTF-8 and unreadable data yield unknown; valid quoted/dotted keys, inline tables and trailing comments retain their meaning. Parsing never writes configuration or echoes its contents. Typed plugin booleans and exact local source ownership remain required. The Windows-only toml 1.1.6 dependency enables std, parse and serde with defaults disabled; serialization/display, unsafe parser acceleration and SIMD are not enabled. Its added tree is serde_spanned 1.1.1, toml_datetime 1.1.1, toml_parser 1.1.3 and winnow 1.0.4, reusing existing serde_core 1.0.229. The lockfile also records optional toml_writer 1.1.2, which is absent from the compiled feature tree. These add no build scripts or proc macros; toml-family licenses are MIT OR Apache-2.0 and winnow is MIT. Winnow contains internal unsafe slice operations; authored code remains unsafe-forbidden. The legacy Unix parser is unchanged.

Grok's installed JSON inventory does not report activation. Windows joins the exact owned bundle source and version to the user configuration in nonempty GROK_HOME, otherwise USERPROFILE/.grok. Invalid overrides never fall back. Explicit disabled entries win; an unlisted user plugin is disabled. This is local onboarding state: project/managed policies and already-running sessions remain provider-owned. Enable uses the existing install/trust or enable commands; disable removes only the source-proven plugin. A newer reported plugin version blocks repair even from an older owned capsule.

Capsule 0.4.6 supplies Grok's command-only hook as a fixed Node bootstrap encoded into the declaration. No user path or hook input becomes shell source. Node reads GROK_PLUGIN_ROOT and imports the joined script with pathToFileURL; missing roots/scripts are silent, while Node is a prerequisite. Existing input, event, transport and hook deadline bounds remain. Real PowerShell 5.1/7 witnesses exercise the shipped command and redirected JSON stdin in isolated profiles; no authenticated Grok task or new macOS/Linux workflow is implied.

## OpenCode automatic discovery

Windows installs the guarded OpenCode bootstrap as `plugins/orchbee-agent-observation.js`. The provider's directory loader scans .js/.ts, so direct Node loading of the previous .mjs target did not establish automatic discovery. The capsule source stays .mjs. A shared Windows path owner resolves nonempty absolute XDG_CONFIG_HOME/opencode, otherwise USERPROFILE/.config/opencode, for integration and doctor. Invalid explicit roots never fall back.

Inspection combines the current .js and obsolete .mjs targets. Foreign, newer or unreadable entries block mutation; an owned obsolete entry requires repair. Enable publishes/verifies the current entry before reinspecting and removing the obsolete owned entry. Failure or cancellation preserves committed effects and reports their receipts; it never deletes a usable current entry as compensation. Disable applies the same guards to both targets. Explain reports the guarded write/removal from its existing inspection, without another read. Doctor checks the discovered .js entry; old-only installations are absent there and require repair in the integration query. Existing bounds and no-replace primitives remain. An isolated official OpenCode 1.18.30 debug-config witness confirms automatic discovery; authentication and paid Agent tasks are outside this evidence.

Windows regular-module inspection validates the entire fixed bootstrap against its decoded absolute file URL, then classifies the exact capsule root and payload-relative suffix. Old owned modules can be repaired; newer modules refuse downgrade; edited bodies and foreign roots remain conflicts. The 16 KiB reader never imports the referenced module. This applies to the existing OpenCode, Kilo, Pi and OMP module owner; Unix inspection is unchanged.

## Kilo Windows integration and shared event model

Kilo is a Windows-managed native integration with the same guarded module ownership, version-direction, deadline, cancellation and receipt rules as OpenCode. Its discovered entry is plugin/orchbee-agent-observation.js under a nonempty absolute KILO_CONFIG_DIR, otherwise XDG_CONFIG_HOME/kilo, otherwise USERPROFILE/.config/kilo. Invalid explicit roots refuse without fallback; doctor shares this resolution. Enable publishes the immutable capsule and exact marked bootstrap; disable removes only the proven entry. No provider command, project configuration or authentication is changed. Other platforms remain manual-only for Kilo. Local installation does not override KILO_PURE or establish an authenticated task.

The shared OpenCode-family normalizer consumes untrusted event/callback objects and returns only canonical observation identities and enums. Created/deleted events accept sessionID or legacy info.id; supplied IDs must be valid and agree. Other events require sessionID. Current status objects map busy/retry/idle; Kilo also accepts the pinned Herdr legacy status strings and maps its offline request to a question. Chat and tool callbacks never inspect or alter their output/arguments. Unknown shapes are silent. No content, path, prompt, output or permission decision crosses this boundary.

Each plugin instance owns a FIFO of at most 64 outstanding normalized observations. Delivery is serial, best effort and bounded by the existing transport. At overflow it stops admission until reload and discards the queued suffix; an in-flight send may finish. Dispose uses the same stop rule. It never resumes after dropping unknown lifecycle evidence or fabricates completion. Transport failure is silent and does not poison later admitted sends. Deterministic tests cover normalization, identity ambiguity, ordering, overflow, failure and disposal; a real Node bootstrap/private Windows pipe witness covers installation through delivery. These controlled witnesses do not prove authenticated provider work.

References: [Kilo plugin API](https://github.com/Kilo-Org/kilocode/blob/cb67014ffdb39ffe890e3a47a0a9c35c0e63538b/packages/plugin/src/index.ts), [Kilo global paths](https://github.com/Kilo-Org/kilocode/blob/cb67014ffdb39ffe890e3a47a0a9c35c0e63538b/packages/core/src/global.ts), [OpenCode legacy lifecycle](https://github.com/anomalyco/opencode/blob/v1.1.25/packages/opencode/src/session/index.ts), and [OpenCode current lifecycle](https://github.com/anomalyco/opencode/blob/v1.18.30/packages/schema/src/v1/session.ts). Capsule 0.4.6 carries the shared module; the canonical observation registry adds Kilo, while Desktop onboarding retains its own provider boundary.

## Gemini and Qwen Windows extensions

Windows manages self-contained Gemini and Qwen extension directories using the existing guarded publication and removal workflow. Gemini resolves an absolute nonempty GEMINI_CLI_HOME as a home parent and installs under .gemini/extensions/orchbee-agent-observation; Qwen resolves QWEN_HOME as the configuration root and installs under extensions/orchbee-agent-observation. Missing overrides use USERPROFILE/.gemini and USERPROFILE/.qwen respectively. Invalid explicit roots refuse without fallback. Doctor shares these paths. Global/project activation policies and already-running sessions remain provider-owned; the local artifact state does not establish effective delivery.

After proving the directory marker and complete file fingerprint, Windows inspection also requires the exact extension name and parseable payload version. An older payload needs repair; a newer payload refuses downgrade even under an older marker contract. Missing/malformed versions are unknown, edited or foreign contents are conflicts. Existing cancellation, backup restoration, committed-prefix receipts and residual ownership remain. Enable copies the source and shared scripts into one self-contained directory; disable removes only a reverified owned directory.

Capsule 0.4.6 uses a fixed Node command that reads the provider-hydrated ORCHBEE_AGENT_EXTENSION_ROOT environment value. User paths never enter shell source. Gemini recursively hydrates its hooks/hooks.json; Qwen recursively hydrates inline hooks in qwen-extension.json, so Qwen has no duplicate external hooks file. The guard imports the joined forward-hook.mjs, preserving existing input bounds and event semantics. Node is required; missing roots/scripts or invalid input are silent. Source contracts are [Gemini extension loading](https://github.com/google-gemini/gemini-cli/blob/20f707577424c0c60cde3470b1495f228773e307/packages/cli/src/config/extension-manager.ts) and [Qwen extension loading](https://github.com/QwenLM/qwen-code/blob/c3a4058a0c7207de7b421e9da0622a9ec890dbad/packages/core/src/extension/extensionManager.ts). Controlled PowerShell 5.1/7 and cmd witnesses exercise the shipped commands with raw JSON stdin and a private Windows observation pipe; no authenticated provider task is implied.

On Windows, the staged Qwen manifest explicitly selects `shell: powershell` for each owned command hook before its fingerprint is written. The upstream default cmd spawn can quote the complete Node expression into an inert string; provider exit 0 alone is insufficient evidence. Gemini already selects PowerShell on Windows. The shared capsule remains portable; only native Windows Qwen installation applies this projection. The extra direct cmd witness passes verbatim command source and is distinct from upstream default-shell dispatch.
