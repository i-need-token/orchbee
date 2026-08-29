# Agent Integrations

## Ownership and default behavior

Desktop Main owns local Agent-integration inspection and the only allowed local configuration
effects. The renderer receives provider-neutral inventory and correlated results through the
`agent-integration-management` port; it receives no executable path, resource path, command output,
filesystem capability, or generic process capability.

Startup and refresh are read-only. Orchbee never installs, enables, disables, updates, copies, or
repairs an Agent integration until the user selects **Enable** or **Disable** for that Agent in
Settings. Browser and Gateway compositions receive no management control.

## Local integration registry

The registry is closed and ordered as Claude Code, Codex, Grok, OpenCode, Pi, and OMP. Each entry
projects only availability, management mode, integration state, and a bounded issue.

| Agent       | Read-only inspection                     | Explicit enable effect                                                                                     | Explicit disable effect                                            |
| ----------- | ---------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| Claude Code | `claude plugin list --json`              | Verify or add exact local `orchbee-local` marketplace, then install and enable `orchbee-agent-observation` | Disable `orchbee-agent-observation@orchbee-local`                  |
| Codex       | `codex plugin list --json`               | Verify or add exact local `orchbee-local` marketplace, then add `orchbee-agent-observation`                | Remove `orchbee-agent-observation@orchbee-local`                   |
| Grok        | `grok plugin list --json`                | Install the packaged Orchbee bundle with trust only after the button click, or enable its exact plugin     | Disable exact plugin `orchbee-agent-observation`                   |
| OpenCode    | Exact target-path and symlink inspection | Add exact symlink `~/.config/opencode/plugins/orchbee-agent-observation.mjs`                               | Remove that symlink only when it still points to the packaged file |
| Pi          | Exact target-path and symlink inspection | Add exact symlink `~/.pi/agent/extensions/orchbee-agent-observation.cjs`                                   | Remove that symlink only when it still points to the packaged file |
| OMP         | Exact target-path and symlink inspection | Add exact symlink `~/.omp/agent/extensions/orchbee-agent-observation.cjs`                                  | Remove that symlink only when it still points to the packaged file |

Claude Code and Codex disable retain the exact local marketplace because another explicitly
installed Orchbee plugin may use it. Grok disable retains the installed plugin. Orchbee performs no
background update. Windows projects every entry as `manual-only`; no managed local mutation runs.

The packaged adapter contracts, compatibility floors, manual commands, and observation privacy
boundary are authoritative in the [adapter guide](../resources/agent-observation-adapters/README.md).

## State and transaction

| State           | Meaning                                                             |
| --------------- | ------------------------------------------------------------------- |
| `enabled`       | Exact Orchbee integration is present and enabled                    |
| `disabled`      | Exact Orchbee integration is present but disabled                   |
| `not-installed` | No exact Orchbee integration occupies the managed entry             |
| `conflict`      | A different file or source occupies an Orchbee-owned name or target |
| `unknown`       | Inspection could not establish an exact state                       |

One process-wide controller serializes mutation. An explicit request follows this order:

1. Initialize or reuse the latest read-only inventory.
2. Reject unavailable enable, manual-only management, conflicts, and concurrent work before a
   write.
3. Re-inspect provider-owned state and execute only the adapter's fixed executable and argument
   vectors or exact symlink operation.
4. Re-inspect the selected adapter after every attempted effect.
5. Publish the observed state. Command success without the requested observed state is
   `verification-failed`; failure never fabricates a successful state.

Executable discovery accepts only a fixed lowercase command name and an absolute result from the
user's login shell. Actual Agent commands do not pass through a shell, ignore stdin, discard stderr,
bound stdout to 256 KiB, and time out after three seconds. A timeout that may indicate an
interactive trust or review step becomes `manual-action-required`. No command output crosses the
renderer boundary.

Symlink enable creates only the parent directory and exact link. A regular file, directory, broken
foreign link, or link to another source is a conflict and remains unchanged. Disable unlinks only a
link whose resolved source exactly equals the current packaged adapter file.

## Settings and reminder policy

Settings uses explicit buttons rather than optimistic switches. It shows all six local entries,
refresh, bounded failures, the no-auto-install disclosure, and the remote manual-only boundary. A
user-requested disable records that Agent's device-local reminder dismissal before attempting the
effect. A verified successful enable clears that dismissal so a later external removal can be
discovered again.

The renderer may show one non-modal reminder per application session when an Agent is locally
available and its managed, conflict-free integration is `not-installed` or `disabled`. The reminder
does not perform a mutation or request operating-system notification permission. Closing it stores
only the dismissed integration ID in `orchbee:web:agent-integration-reminders:v1`; **Review
integration** opens the Agent Integrations Settings category. Storage failure keeps the in-memory
dismissal for the current session.

## Remote boundary

Desktop local management never gains authority from an SSH terminal. For Orchbee → SSH → remote
tmux or Zellij → Agent, the adapter must be reviewed and enabled manually on the machine where the
Agent process runs. Orchbee does not execute a remote command, copy the packaged resource, write a
remote file, inspect a remote home directory, or persist a remote credential.

After manual remote setup, the existing reverse Unix-socket and OSC 777 correlation transport can
carry observations through SSH and nested muxes. Transport reachability does not grant integration
management authority.

## Failure and recovery

`agent-unavailable`, `busy`, `conflict`, `control-unavailable`, `manual-action-required`,
`operation-failed`, and `verification-failed` are the complete renderer-visible failure set. A
failed or ambiguous effect is followed by read-only reconciliation; Settings shows that observation
and requires another explicit user action. Conflicts require manual review and are never
overwritten. Removing Orchbee's exact managed entry is the rollback.

## Verification

Run from the repository root:

```sh
node scripts/tla/run-models.mjs --models desktop-agent-integration-management
pnpm --filter @orchbee/desktop-client-protocol test
pnpm --filter @orchbee/desktop test:unit
pnpm --filter @orchbee/web test
```
