# Environment Isolation

| Field             | Value                                                                                                                                                                                                                                           |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Status            | Accepted by the product owner on 2026-11-07 in ORC-382. Windows first; macOS and Linux adopt the same rules with ORC-328.                                                                                                                       |
| Owner             | Maintainers of `crates/local-runtime`, `apps/orchbee`, `apps/orchbee-worker`, `apps/desktop`, and `crates/agent-integration-writer`                                                                                                             |
| Current authority | [Windows named runtime identity](../../crates/local-runtime/README.md#windows-named-runtime-identity), [Windows CLI configuration](../../apps/orchbee/docs/windows-cli-configuration.md), and the [Desktop guide](../../apps/desktop/README.md) |
| Exit condition    | Delete this proposal once every delivery slice ships and its rules live in those owners.                                                                                                                                                        |

## Environments

One machine runs up to three environments side by side. The artifact stem fixed at compile time
through `ORCHBEE_ARTIFACT_STEM` identifies the environment; no runtime argument or environment
variable changes it.

| Environment | Stem                       | Built by                                                                                         |
| ----------- | -------------------------- | ------------------------------------------------------------------------------------------------ |
| Release     | `orchbee`                  | Release staging and packaging that pass `--artifact-stem orchbee`                                |
| Development | `ob-dev-<checkout digest>` | `pnpm desktop` and default native staging; one per checkout                                      |
| Test        | `ob-test-<digest>`         | The packaged test build (home digest) and every build without an explicit stem (checkout digest) |

The packaged test build replaces the former Local profile; no name, path, badge, or script says
Local. Release keeps every current name and location unless a section below changes it.

Already true on Windows: executable names derive from the stem, default native staging uses the
development stem, and non-release builds refuse `update` and `channel set`.

## Environment data root

Every environment keeps all of its state under one data root, `<home>/.<stem>`, which is
`~/.orchbee` for release. One owner in `crates/local-runtime` resolves the root from the compiled
stem, and every caller derives its location from that owner. The
[data root layout](data-root-layout.md) defines the tree in every environment and replaces the
shipped release locations. On Windows, slice 1 already keeps non-release state in these roots, and
phase 1 of that layout puts it under `internal/`, with the default runtime base at
`<root>/internal/runtime`.

- `ORCHBEE_CONFIG_FILE` and an explicitly set `ORCHBEE_RUNTIME_BASE` keep their override meaning in
  every environment; under [accepted choice 1](data-root-layout.md#accepted-choices),
  `ORCHBEE_HOME` moves the whole root and `ORCHBEE_CONFIG_FILE` names only another settings file.
  Desktop no longer writes `ORCHBEE_CONFIG_FILE`: on Windows the Client derives the same root, and
  on macOS and Linux, whose natives keep release names until ORC-328, a non-release Desktop passes
  `ORCHBEE_HOME` instead.
- Runtime directories differ per environment, so Session scopes and pipe names cannot collide; the
  release `default` Session belongs only to release.
- Pipe names keep the `orchbee-` prefix in every environment, because each environment's runtime
  directory, and so the endpoint id that names its pipes, differs.
- Desktop development data moves from `~/.orchbee-dev-<digest>` to `~/.ob-dev-<digest>`, and the
  test package from `~/.orchbee-local` to `~/.ob-test-<digest>`. Pre-release data is not migrated.

## Inherited Session routing

Managed terminals inherit `ORCHBEE_RUNTIME_BASE` and `ORCHBEE_RUNTIME_SCOPE` so that a command run
inside one reaches that terminal's Session.

- The Worker also injects `ORCHBEE_ENVIRONMENT=<stem>`.
- When `ORCHBEE_ENVIRONMENT` names another environment, a Client treats inherited
  `ORCHBEE_RUNTIME_BASE` and `ORCHBEE_RUNTIME_SCOPE` as unset, without error, and uses its own
  defaults. A development build started in a release terminal therefore reaches its own Session.
- Without `ORCHBEE_ENVIRONMENT`, the variables keep their explicit meaning. The handshake
  environment check in ORC-384 rejects any cross-environment connection that remains.
- `ORCHBEE_SESSION` and command-line Session selection resolve inside the Client's own environment
  and are unaffected.

## Agent integrations

- Each environment installs capsules under its own root, so a provider hook entry is owned by the
  environment whose capsule root its command names.
- Integration commands add, update, and remove only entries they own. No environment changes
  another environment's entries.
- A hook invocation acts only when the Agent's inherited `ORCHBEE_ENVIRONMENT` equals the owning
  stem, or is absent and the owner is release; otherwise it exits silently. Entries from several
  environments therefore coexist without double reporting.
- Where a provider admits only one hook command, an integration refuses to replace an entry another
  environment owns and reports the conflict.

## Release destructive commands

- Release `server stop`, `session stop`, and `session delete` run only when standard input and
  standard output are both interactive consoles and the person types the exact Session name at the
  prompt. Release refuses `--confirm` and every non-interactive invocation, including Agents,
  scripts, and pipes, with a localized message that names the command to run by hand.
- Development and test builds keep `--confirm`.
- TUI and Desktop stop actions keep their confirmation dialogs.
- `update` is not on this list: it never stops a process, and non-release builds already refuse it.
- This slows unintended Agent actions; it is not a security boundary, because a same-user process
  that drives a console can still type the name.

## Destructive request log

Every `server stop`, `session stop`, and `session delete` invocation that passes grammar, accepted
or refused, appends one JSON line to the environment's destructive request log, which the
[data root layout](data-root-layout.md#locations-replaced) places: time, environment, command,
target Session, interactive state, parent PID and image path, and outcome. A grammar failure, such as a development stop without `--confirm`, is not a request and is
not logged. The file rotates at a fixed size, and a failed write never blocks or changes the
command.

## Build defaults

- A build without an explicit stem uses `ob-test-<checkout digest>`, so only release staging produces
  release names and a plain Cargo build cannot act on the release installation.
- Release packaging refuses a cohort whose stem is not `orchbee`.
- Tests start every Session from a staged copy named by `CohortArtifact` file names, never from
  Cargo's output directory, so no test process carries a release image name.

## Names and markers

- Packaged Desktop executables are `<stem>-desktop.exe` (release `orchbee-desktop.exe`), distinct
  from the Client `<stem>.exe`. `pnpm desktop` runs its Electron shell under the same name.
- Desktop display names are `Orchbee Dev` and `Orchbee Test`; the test package's bundle identifier,
  badge, and package script say Test.
- TUI and Desktop show the environment and, for checkout builds, the checkout directory name;
  release shows no marker. Placement follows the
  [cross-client interface design](../../docs/INTERFACE-DESIGN.md), with the TUI designed first.
- Remote SSH admission accepts a Client of the same environment kind: `orchbee.exe` for release, any
  valid development or test Client name otherwise.
- PowerShell and Elvish completion register the running executable's own name.
- The Desktop renderer and Web development servers listen on a port derived from the checkout digest
  within a documented range, so checkouts run concurrently. Packaged builds run no development
  server.
- Development and test CLIs stay off `PATH`; run them through `pnpm` scripts or the path that staging
  prints.
- "Environment" (release, development, test) and "release channel" (stable, preview) are separate
  terms; only release has channels. `docs/GLOSSARY.md` defines Environment.

## Out of scope

- Connection-level environment checks belong to the handshake in ORC-384.
- A resident guardian that restarts a vanished release Session. Recovery follows ORC-384: Hosts
  outlive Server and Worker and the next activation adopts them; when a Host is lost, the next
  activation restores the layout and resumes supported Agents.
- macOS and Linux, until ORC-328.
- Process-object DACLs, restricted tokens, Windows services, and command hooks.

## Delivery slices

| Slice                | Content                                                                                              | Depends on       |
| -------------------- | ---------------------------------------------------------------------------------------------------- | ---------------- |
| 1. Data roots        | Data root owner and every derived location; Desktop data roots; Local renamed Test                   | —                |
| 2. Routing           | `ORCHBEE_ENVIRONMENT` injection and the inherited-variable rule                                      | 1                |
| 3. Build defaults    | Non-release default stem, staged test cohorts, release packaging check                               | —                |
| 4. Integrations      | Capsule roots, owned hook entries, hook-time environment check                                       | 1, 2             |
| 5. Release commands  | Interactive Session-name confirmation, destructive request log, Agent skill text                     | 1 (log location) |
| 6. Names and markers | Desktop executable and shell names, SSH admission, completion, environment marker, development ports | 3 (marker label) |

Slice 1 ships before ORC-384 removes cohort admission, so two environments never share a Session
location once connections stop rejecting each other.

## Acceptance

On Windows, with release, one development checkout, and the test package running at once:

- Each environment starts, stops, and reconfigures its own Session without touching another's
  processes, configuration, catalog, or capsules.
- No image name appears in two environments, and `cargo test` starts no process with a release
  image name.
- A development build started in a release terminal reaches its own Session.
- A development integration install leaves release hook entries unchanged, and an Agent in a release
  terminal reports only to release.
- Release `server stop` from a pipe or an Agent is refused; from a console it requires the Session
  name; both outcomes appear in the log.
- Two checkouts run `pnpm desktop` concurrently.
