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
never performs the mutation itself. Browser Preview receives no management control.

## Local integration registry

The registry is closed and ordered as Claude Code, Codex, Grok, OpenCode, Pi, OMP, Gemini CLI,
GitHub Copilot CLI, Cline, and Qwen Code. Each entry projects only availability, management mode,
integration state, and a bounded issue.

| Agent              | Read-only inspection                                | Explicit enable/update effect                                                       | Explicit disable effect                                            |
| ------------------ | --------------------------------------------------- | ----------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Claude Code        | Exact plugin and local marketplace content identity | Repair an outdated managed source, then install and enable the current Capsule      | Disable the current plugin; remove an outdated managed entry       |
| Codex              | Bounded Codex-owned marketplace/plugin declarations | Retarget an outdated managed marketplace, then reinstall the exact plugin           | Remove the plugin and any outdated managed marketplace declaration |
| Grok               | Exact plugin source identity and activation         | Replace an outdated managed copy or enable the current copy                         | Uninstall only the source-proven plugin                            |
| OpenCode           | Exact provider-local bootstrap                      | Atomically publish a guarded module bootstrap                                       | Remove only the byte-exact owned bootstrap                         |
| Pi                 | Exact provider-local bootstrap                      | Atomically publish a guarded CommonJS bootstrap                                     | Remove only the byte-exact owned bootstrap                         |
| OMP                | Exact provider-local bootstrap                      | Atomically publish a guarded CommonJS bootstrap                                     | Remove only the byte-exact owned bootstrap                         |
| Gemini CLI         | Bounded owned marker and file digests               | Atomically copy a self-contained guarded extension                                  | Remove only the digest-verified owned directory                    |
| GitHub Copilot CLI | Exact plugin and local marketplace source identity  | Retarget an outdated managed marketplace, then install or enable the plugin         | Disable the current plugin; remove an outdated managed entry       |
| Cline              | Bounded official install layout and Orchbee marker  | Install the current Capsule bundle or explicitly replace an outdated marked package | Uninstall only the marked Orchbee package                          |
| Qwen Code          | Bounded owned marker and file digests               | Atomically copy a self-contained guarded extension                                  | Remove only the digest-verified owned directory                    |

The content-addressed Capsule lives at
`~/.orchbee/agent-observation-adapters/<payloadSha256>`, outside the application
bundle and provider cache. It is materialized only before explicit enable/update
and retained while referenced, including after application removal. The strict
contract-1 manifest and actual payload digest must agree with the bundled source.
A changed identity under the same managed installation parent is `update-required`;
package semver does not order identities. Application-bundle and checkout roots
are no longer recognized as managed installations. Symlinks and foreign or modified
entries remain conflicts. There is no automatic migration or directory cleanup.

Only contract 1 is current. A positive non-current guarded or Cline contract is
`update-required` after its Orchbee ownership marker is verified; guarded directories
also require every actual file fingerprint to match. Numeric ordering grants no
special status. Reinstall/removal still requires an explicit action, and symlinked
Cline packages or manifests do not establish ownership.

Provider-local entries stay self-contained or catch a missing Capsule before
provider code reports an error. Persisted hooks preserve their runtime/script
guards and bounded output policy. Cline trusts one official local installation
with the exact Orchbee marker. Desktop performs no background update; its Windows
registry remains `manual-only`.

The packaged adapter contracts, compatibility floors, manual commands, and observation privacy
boundary are authoritative in the [adapter guide](../resources/agent-observation-adapters/README.md).

## State and transaction

| State             | Meaning                                                                                                    |
| ----------------- | ---------------------------------------------------------------------------------------------------------- |
| `enabled`         | Exact Orchbee integration is present and enabled                                                           |
| `disabled`        | Exact Orchbee integration is present but disabled                                                          |
| `not-installed`   | No exact Orchbee integration occupies the managed entry                                                    |
| `update-required` | An outdated content identity, non-current owned contract or dependency-missing entry needs explicit repair |
| `conflict`        | A different file or source occupies an Orchbee-owned name or target                                        |
| `unknown`         | Inspection could not establish an exact state                                                              |

One process-wide controller serializes mutation. An explicit request follows this order:

1. Initialize or reuse the latest read-only inventory.
2. Reject unavailable enable, manual-only management, conflicts, and concurrent work before a
   write.
3. Preflight and atomically publish the current content-addressed Capsule without changing provider configuration.
4. Re-inspect provider-owned state and execute only fixed command vectors or an exact guarded-entry
   operation; only proven managed entries are eligible for repair.
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

Provider-local bootstrap installation stages before publication. Module bootstraps
catch missing or invalid Capsule imports without output. Extension directories
contain their runtime and an owned manifest recording each file digest. Symlinks,
foreign roots, modified bootstraps, extra files and digest mismatches are conflicts
and remain unchanged. Disable removes only a byte-exact owned bootstrap or a
digest-verified directory. No managed entry persists an application-bundle path.
Removing the application leaves the retained Capsule or self-contained provider
entry; full cleanup first disables integrations.

## Settings and reminder policy

Settings uses snapshot-controlled switches. A switch change is the explicit user request, but its
position remains at the last inspected state until Main publishes the post-attempt inspection; it
never moves optimistically. Pending work labels the affected row and disables all competing
switches. The surface also shows all ten local entries, refresh, bounded failures, the
no-auto-install disclosure, and the remote manual-only boundary. A user-requested disable records
that Agent's device-local reminder dismissal before attempting the effect. A verified successful
enable clears that dismissal so a later external removal can be discovered again.

The local Zsh, Bash, and Fish resources emit a version-`1` launch hint only when the first command
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

After manual remote setup, the v1 reverse Unix socket and private APC correlation transport can
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

Run from the repository root:

```sh
pnpm --filter @orchbee/desktop test
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/web test
```

The shared observation boundary additionally accepts Cursor, Qoder and Antigravity session-identified identities, plus legacy Cursor session-started. Native Windows owns JSON onboarding; the Desktop management registry remains separate.

The observation registry also admits Mastracode lifecycle facts. Windows native management owns its hook configuration; Desktop management remains separate.

Droid session identity is accepted at the shared observation boundary. Native Windows owns configuration management; Desktop management remains separate.

Kimi session identity/end events are accepted at the shared observation boundary. Native Windows owns TOML onboarding; Desktop management remains separate.

Letta session identity is accepted at the shared observation boundary; native Windows owns onboarding and Desktop management remains separate.

Devin session identity is accepted at the shared observation boundary; Windows native CLI owns its configuration management.

Hermes session identity is accepted by the shared receiver; its Windows native onboarding remains separate from Desktop management.
