# Agent Integrations

## Ownership and default behavior

Desktop Main owns local Agent-integration inspection and the only allowed local configuration
effects. The renderer receives provider-neutral inventory and correlated results through the
`agent-integration-management` port; it receives no executable path, resource path, command output,
filesystem capability, or generic process capability.

Startup constructs an uninspected snapshot and executes no provider command. A recognized launch
may inspect only that provider; selecting **Refresh** in Settings inspects all ten providers.
Orchbee never installs, enables, disables, updates, copies, or repairs an integration until
the user explicitly changes that Agent's switch in Settings. A launch reminder opens Settings and
never performs the mutation itself. Browser and Gateway compositions receive no management control.

## Local integration registry

The registry is closed and ordered as Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI,
GitHub Copilot CLI, Cline, and Qwen Code. Each entry projects only availability, management mode,
integration state, and a bounded issue.

| Agent              | Read-only inspection                                                           | Explicit enable/update effect                                                                          | Explicit disable effect                                      |
| ------------------ | ------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ |
| Claude Code        | Exact plugin plus `plugin marketplace list --json` source/version inspection   | Migrate an exact legacy source, then install and enable from the stable Capsule                        | Disable the current plugin; remove an exact legacy entry     |
| Codex              | Bounded exact read of Codex-owned marketplace/plugin declarations              | Repair an exact legacy marketplace, retarget it to the stable Capsule, then reinstall the exact plugin | Remove the exact plugin; also remove an exact legacy source  |
| Grok               | Exact `plugin list --json` source/version inspection                           | Replace an exact legacy copy, or install/enable the stable Capsule copy                                | Uninstall only the exact source-proven plugin copy           |
| OpenCode           | Exact provider-local bootstrap and legacy-symlink inspection                   | Atomically publish a guarded local module bootstrap                                                    | Remove only the byte-exact bootstrap or exact legacy symlink |
| Pi                 | Exact provider-local bootstrap and legacy-symlink inspection                   | Atomically publish a guarded local CommonJS bootstrap                                                  | Remove only the byte-exact bootstrap or exact legacy symlink |
| OMP                | Exact provider-local bootstrap and legacy-symlink inspection                   | Atomically publish a guarded local CommonJS bootstrap                                                  | Remove only the byte-exact bootstrap or exact legacy symlink |
| Gemini CLI         | Bounded manifest-and-digest inspection of the provider-local extension         | Atomically copy a self-contained guarded extension                                                     | Remove only the digest-verified owned directory              |
| GitHub Copilot CLI | Exact plugin plus `plugins marketplace list --json` source inspection          | Repoint an exact legacy marketplace, then install or enable the exact plugin                           | Disable the current plugin; remove an exact legacy entry     |
| Cline              | Bounded scan of Cline's official local-install layout and exact Orchbee marker | Install the stable Capsule bundle; replace an exact older contract only after the user requests it     | Uninstall only an exact current or older Orchbee package     |
| Qwen Code          | Bounded manifest-and-digest inspection of the provider-local extension         | Atomically copy a self-contained guarded extension                                                     | Remove only the digest-verified owned directory              |

The versioned Capsule lives under Desktop user data, outside the application bundle and provider
cache. It is materialized only immediately before explicit local enable/update and must be retained
while any provider declaration references it, including after application removal. Provider-local
entries either remain self-contained or catch a missing Capsule before provider code can report an
error. Every persisted command hook checks both its runtime and script, redirects stdout and
stderr, and exits zero when observation is unavailable. Cline inspection trusts only one package
under its official local-install layout with Orchbee's exact contract marker. Orchbee performs no
background update. Windows projects every entry as `manual-only`; no managed local mutation runs.

Claude and GitHub Copilot inspection joins the exact plugin inventory with the exact local
marketplace declaration. Grok's provider-owned installed copy is self-contained, but inspection
still verifies its recorded source and payload version. An exact Orchbee application-bundle source
or older sibling Capsule is `update-required`; only explicit Enable/Update can repoint or replace
it. A same-named marketplace, plugin, or copy with any other source is a conflict and is never
changed. Grok disable removes the source-proven copy so its current CLI inventory cannot conceal a
still-installed hook behind a provider-specific enablement representation.

The packaged adapter contracts, compatibility floors, manual commands, and observation privacy
boundary are authoritative in the [adapter guide](../resources/agent-observation-adapters/README.md).

## State and transaction

| State             | Meaning                                                                      |
| ----------------- | ---------------------------------------------------------------------------- |
| `enabled`         | Exact Orchbee integration is present and enabled                             |
| `disabled`        | Exact Orchbee integration is present but disabled                            |
| `not-installed`   | No exact Orchbee integration occupies the managed entry                      |
| `update-required` | An exact older or dependency-missing Orchbee entry needs consented migration |
| `conflict`        | A different file or source occupies an Orchbee-owned name or target          |
| `unknown`         | Inspection could not establish an exact state                                |

One process-wide controller serializes mutation. An explicit request follows this order:

1. Initialize or reuse the latest read-only inventory.
2. Reject unavailable enable, manual-only management, conflicts, and concurrent work before a
   write.
3. Preflight and atomically publish Capsule `0.4.7` without changing provider configuration.
4. Re-inspect provider-owned state and execute only fixed command vectors or an exact guarded-entry
   operation; only exact legacy entries are eligible for migration.
5. Re-inspect the selected adapter after every attempted effect.
6. Publish the observed state. Command success without the requested observed state is
   `verification-failed`; failure never fabricates a successful state.

### Executable discovery

Finder-launched macOS applications and comparable POSIX desktop launches can inherit a restricted
`PATH` that differs from the user's interactive terminal. Desktop Main owns the process-local
`AgentExecutableSearchPath`: the inherited `PATH` is its first source, and one successful or failed
interactive-login-shell probe is its optional second source. The probe is lazy, shared by concurrent
adapter inspection, and cached for the Desktop process lifetime. Restarting Desktop is the recovery
path after the user changes shell startup configuration.

The probe runs the configured absolute user shell in interactive login mode with ignored stdin,
discarded stderr, a 256 KiB stdout limit, and a three-second timeout. A marker-bounded helper emits
only the resulting `PATH`; Desktop does not import other shell environment variables. Shell startup
files can therefore run once when inherited discovery misses, but no shell command is used for an
Agent inspection or mutation. A failed, malformed, oversized, or timed-out probe contributes no
search path and remains a read-only `not-found` result.

`resolveAgentExecutableCandidates` is the deterministic resolver. It accepts only the registry's
bounded lowercase command names, preserves source and directory order, removes duplicates, rejects
empty, relative, control-character, and overlong entries, and caps each source at 256 directories.
The platform adapter accepts the first candidate whose followed target is a regular file and is
executable. It preserves the selected symlink path so version-manager shims keep their invocation
identity. Aliases, functions, current-directory lookup, recursive home-directory scans, and a fixed
inventory of package-manager installation directories are excluded.

Commands discovered through the shell source receive only that resolved `PATH` overlaid on the
inherited command environment, which preserves script interpreter lookup without harvesting shell
secrets. Actual Agent commands use the selected absolute path directly, ignore stdin, discard
stderr, bound stdout to 256 KiB, and time out after three seconds. A timeout that may indicate an
interactive trust or review step becomes `manual-action-required`. No path, environment value, or
command output crosses the renderer boundary.

This adopts only the once-cached interactive-login-shell environment-probe pattern from VS Code
[`shellEnv.ts` at `cd429513`](https://github.com/microsoft/vscode/blob/cd429513258458bcbe37b17fe714874197fe2adf/src/vs/platform/shell/node/shellEnv.ts),
observed on 2026-08-29. Orchbee does not claim source compatibility or adopt VS Code's full
environment merge, shell catalog, configuration surface, timeout, diagnostics, or CLI-launch
policy. Deterministic candidate tests cover ordering and rejection boundaries; the command-runner
test covers a restricted inherited environment, noisy concurrent shell probing, cached failure or
success, executable validation, and interpreter lookup through the selected shell `PATH`. Native
macOS verification covers the packaged-launch compatibility claim; other shell families remain
bounded by their maintained platform tests.

Provider-local bootstrap installation stages before publication. Module bootstraps catch missing
or invalid Capsule imports without output; extension directories contain their guarded runtime and
an owned manifest that records every file digest. A regular file, directory, foreign link, modified
bootstrap, extra file, or digest mismatch is a conflict and remains unchanged. An exact old
application-bundle or stable-Capsule symlink is `update-required`; migration occurs only through an
explicit switch action. Disable removes only a byte-exact bootstrap, digest-verified directory, or
recognized exact legacy symlink.

No managed provider entry persistently names an application-bundle path. A recognized exact legacy
bundle path is never silently followed or repaired: inspection reports `update-required`, and only
an explicit Settings action may migrate it. Removing the application while leaving enabled
integrations therefore leaves either a self-contained provider entry, a silent guarded bootstrap,
or its intentionally retained Capsule. Uninstall tooling must not delete a referenced Capsule; a
full cleanup disables integrations first.

## Settings and reminder policy

Settings uses snapshot-controlled switches. A switch change is the explicit user request, but its
position remains at the last inspected state until Main publishes the post-attempt inspection; it
never moves optimistically. Pending work labels the affected row and disables all competing
switches. The surface also shows all ten local entries, refresh, bounded failures, the
no-auto-install disclosure, and the remote manual-only boundary. A user-requested disable records
that Agent's device-local reminder dismissal before attempting the effect. A verified successful
enable clears that dismissal so a later external removal can be discovered again.

The local Zsh, Bash, and Fish resources emit a version-`3` launch hint only when the first command
word names a supported Agent and resolves to an external executable. The hint contains only the
closed integration ID and protocol version; command text and arguments never enter the signal.
Main treats it as untrusted reminder-gating evidence, not Agent status or configuration authority,
and records each ID once in first-observation order. Aliases, functions, wrappers, remote shells,
and terminals without Orchbee shell integration remain silent.

The renderer may show one compact, visually quiet bottom-right reminder per application session
only after the corresponding local launch was observed and its provider-scoped inspection proves
the Agent is locally available with a managed, conflict-free integration in `not-installed`,
`disabled`, or `update-required` state. First-observed eligible launch order selects the candidate.
Rendering performs no mutation and requests no operating-system notification permission. **Open
integration settings** hides the current reminder and navigates directly to Agent Integrations;
only the switch there can mutate configuration. Closing the reminder stores only the dismissed
integration ID in `orchbee:web:agent-integration-reminders:v1`; storage failure keeps the in-memory
dismissal for the current session.

## Passive presence boundary

The launch hint creates only presentation-scoped heuristic presence with no capabilities and an
unknown Turn. It cannot assert thinking, tool use, idle, attention, completion, success, failure, or
notification eligibility. Evidence carries a curated signature ID, source, generation, freshness,
and presentation identity; diagnostics expose no command or screen content. Fresh evidence becomes
stale after 30 seconds and detaches after 120 seconds unless refreshed. Retirement detaches the
presentation but never invents a semantic session end. Rich adapters remain the only authority for
semantic status and exact hidden-mux state.

## Remote boundary

Desktop local management never gains authority from an SSH terminal. For Orchbee → SSH → remote
tmux or Zellij → Agent, the adapter must be reviewed and enabled manually on the machine where the
Agent process runs. Orchbee does not execute a remote command, copy the packaged resource, write a
remote file, inspect a remote home directory, or persist a remote credential.

After manual remote setup, the v3 reverse Unix socket and private APC correlation transport can
carry observations through SSH and nested tmux/Zellij layers. Adapters resolve outer client TTYs
and write the private frame directly, so mux passthrough is unnecessary. Transport reachability
does not grant integration management authority.

## Failure and recovery

`agent-unavailable`, `busy`, `conflict`, `control-unavailable`, `manual-action-required`,
`operation-failed`, and `verification-failed` are the complete renderer-visible failure set. A
failed or ambiguous effect is followed by read-only reconciliation; Settings shows that observation
and requires another explicit user action. Conflicts require manual review and are never
overwritten. Removing Orchbee's exact managed entry is the rollback.

## Verification

[`AgentIntegrationLaunchReminder`](../formal/tla/AgentIntegrationLaunchReminder.tla) uses two
representative integration IDs and treats the inspected eligibility carried by a launch publication
as a stable bounded input. Its first-observed order is renderer delivery order: provider-scoped
inspections may settle in a different order from raw Main launch observations, and a management
response may merge Main's recorded order. The model checks deduplication, delivery merging,
eligibility including `update-required`, one offer per application session, dismissal, and the
read-only Settings callback carrying the offered integration ID. Item/revision reconciliation
between concurrent launch publications and management responses is outside this bounded model.

[`DesktopAgentIntegrationManagement`](../formal/tla/DesktopAgentIntegrationManagement.tla) uses two
representative providers and starts with the production all-`unknown`, uninspected snapshot. It checks
explicit Refresh, independent per-provider inspection failure projection, one admitted operation,
authorization tied to that admission, disable verification accepting `disabled` or `not-installed`,
and reconciliation after every attempted effect, including failed and ambiguous outcomes. The
`actual` variable denotes stable provider state at adapter preflight/effect; snapshot staleness,
concurrent third-party edits, same-state no-op responses, static availability, and `manual-only`
admission details are explicit exclusions. Scheduling competition between launch-scoped inspection
and Settings work is an explicit cross-model exclusion.

Run from the repository root:

```sh
node scripts/tla/run-models.mjs --models desktop-agent-integration-management,agent-integration-launch-reminder
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/web test
```

The shared observation boundary additionally accepts Cursor session-started identities. Native Windows owns JSON onboarding; the Desktop management registry remains separate.
