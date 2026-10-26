# Automatic shell integration

## Integration scope

Orchbee detects Zsh, Bash and Fish from the configured executable for new local Desktop terminals. It changes only the child launch, loads the existing application-owned OSC 133 and OSC 7 resources without editing startup files, restores the normal shell startup path and skips injection when a launch cannot be handled safely. This contract excludes cursor/title changes, completions, sudo or PATH wrapping, command transport, shell cloning, remote or SSH injection, new-user detection, global Zsh environment probing, arbitrary Bash invocations and other shells.

## Problem and scope

Prompt navigation, last-command output, current-directory projection, and completion reminders need
semantic shell marks. Requiring every Desktop user to copy a `source` command into a startup file
makes those common terminal workflows absent by default and leaves setup coupled to user-managed
configuration.

Desktop Main therefore refines the already selected launch profile for a supported local interactive
POSIX shell. The refinement applies only to terminal bindings created after profile resolution. It
does not mutate a startup file, run a setup command in Main, change a running or restored terminal,
modify the selected executable, inject into Windows or a remote connection, or infer a shell from
terminal output.

## Inputs, data, and outputs

`AutomaticShellIntegrationConfiguration` is trusted Desktop startup data:

- `directory` is the code-resolved application resource directory; ambient environment cannot
  replace it;
- `availableShells` contains only shells whose automatic loader and shared Orchbee integration
  script were readable during Desktop startup;
- `inheritedEnvironment` captures presence and exact values for `XDG_DATA_DIRS` and `ZDOTDIR`
  before the independently launched Terminal Host can become the process parent.

The principal pure operation is
`resolveAutomaticShellIntegration(launch, platform, configuration)`. `launch` is admitted by
`TerminalLaunchSchema` after the launch profile has selected its executable and literal arguments.
The result is exactly one of:

- `injected`, carrying the complete schema-valid launch and detected shell;
- `unchanged`, carrying the original launch and one stable reason: unsupported platform, missing
  resources, unsupported executable, unsupported arguments, unsafe resource path, or invalid
  candidate.

An unchanged result is an intentional no-op. Terminal creation continues with the original launch,
and `ORCHBEE_SHELL_INTEGRATION_DIR` keeps the existing manual fallback discoverable. The operation
has no storage, filesystem, process, transport, time, or UI dependency.

## Decision model

Shell identity is the exact POSIX basename of the selected executable. A symlink or wrapper with a
different basename is not guessed. The resolver admits only interactive launch shapes whose startup
semantics it owns:

| Shell | Admitted arguments                                                                                               | Refinement                                                                                                                                                 |
| ----- | ---------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Zsh   | Empty, or only `-i`, `--interactive`, `-l`, `--login`, and combined `-il` / `-li` flags                          | Preserve arguments; point `ZDOTDIR` at the automatic loader and carry the exact prior value in one private marker                                          |
| Fish  | The same bounded interactive/login forms as Zsh                                                                  | Preserve arguments; prepend the automatic Fish data directory to `XDG_DATA_DIRS` and carry exact prior presence/value in one private marker                |
| Bash  | Empty or `-i` / `--interactive`, optionally with `--noprofile` and one complete `--rcfile` or `--init-file` pair | Replace the user rc-file selection with the automatic loader, preserve the admitted flags, and carry the exact selected user rc file in one private marker |

Bash rejects login mode, `--norc`, `-c`, `-s`, `--posix`, a positional script, an incomplete option
pair, and unknown options. Zsh and Fish reject every argument outside their table. A Fish resource
path containing the POSIX list separator is unsafe and leaves the launch unchanged. Candidate
construction is all-or-nothing: a protocol bound failure returns the exact original launch.

## Loader contracts

The automatic loaders are application resources and never become user configuration:

1. The Zsh `.zshenv` restores or unsets the original `ZDOTDIR`, sources the user's original
   `.zshenv` with Zsh's absence semantics, and sources `orchbee.zsh` only for an interactive shell.
   Normal Zsh startup then continues using the restored directory.
2. The Fish vendor configuration restores the exact original `XDG_DATA_DIRS` presence/value before
   sourcing `orchbee.fish` for an interactive shell. The temporary data-directory entry cannot leak
   to later commands or nested shells.
3. Bash itself performs its normal non-login interactive system startup, then selects the automatic
   loader as its one rc file. The loader clears its marker, sources the exact user-selected rc file
   or the normal readable `~/.bashrc`, and finally sources `orchbee.bash`. It never changes POSIX,
   login, `ENV`, or `HISTFILE` semantics. Bash older than 4.4 keeps its normal startup behavior while
   the shared script reports the existing typed unsupported status and installs no hooks.

Every loader clears its private markers before returning. The shared scripts remain idempotent, so
an existing manual `source` line does not install duplicate hooks. Runtime loader failure may leave
integration unavailable, but it must restore the shell-owned environment and startup path before
attempting the optional Orchbee script.

## Invariants, ownership, and lifetime

- The launch profile remains the sole owner of executable and literal user arguments; automatic
  integration is a post-profile Desktop refinement.
- Desktop runtime composition owns trusted resource admission and inherited-environment capture.
  The pure resolver is the only writer of automatic launch deltas.
- The Terminal Host receives one complete resolved launch and remains the PTY/process owner. It
  does not detect shells or source integration.
- A new terminal binding captures the refined launch before its first process side effect. Retry,
  renderer replacement, Main restart, profile edits, and recovery reuse that captured launch.
- Unsupported and failed preparation never changes executable, arguments, environment, CWD,
  geometry, terminal configuration, or terminal profile.
- Automatic injection never reads, creates, edits, backs up, or deletes a user startup file in
  Main. Only the launched shell reads the same startup files it owns during normal initialization.
- No command text, output, directory, environment value, or integration marker crosses into the Web
  settings capability or product diagnostics.

There is no long-lived automatic-integration state machine. Startup performs a read-only resource
admission, profile resolution performs one deterministic refinement, binding creation commits the
complete launch, and the child loader consumes and clears its one-shot environment markers.

## Verification and traceability

| Claim                                                                                                         | Evidence                                                          | Limit                                                                   |
| ------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Shell and argument admission, exact environment restoration, all-or-nothing fallback, and bounds              | Deterministic automatic-integration resolver tests                | Synthetic trusted configuration and launch values                       |
| Profile selection precedes refinement; unsupported profiles remain unchanged; captured bindings remain stable | Launch-profile controller and terminal-lifecycle tests            | Mocked Host process boundary                                            |
| Loader resources preserve startup behavior, clear markers, remain idempotent, and emit OSC 133/7              | Resource tests that execute installed local shells when available | Native shell versions installed on the test operating system            |
| A new Desktop Zsh terminal receives marks without manual setup while preserving user startup state            | `terminal-parity/SI-03-automatic-shell-integration`               | macOS Zsh, real Electron/PTY/Terminal Host; no global `zshenv` override |
| Settings explains automatic scope and manual fallback in both locales                                         | i18n and Settings component tests                                 | Presentation only; it does not prove child launch                       |

No TLA+ model is selected because refinement has no mutable state, concurrency, asynchronous
completion, retry ordering, or cancellation. No Lean proof is selected because schema validation and
the finite shell/argument decision table are exercised directly. The real-shell and Desktop traces
are bounded platform evidence, not a claim about every shell version, startup plugin, system-wide
configuration, symlinked executable, or damaged installation.
