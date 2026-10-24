# Terminal Launch Profiles

## Scope and authority

Launch profiles are one leaf of the device's
[shared preferences](../../../crates/workbench-protocol/docs/user-preferences.md).
Desktop Main and Native Worker capture the same catalog for new terminals.
Desktop owns launch adaptation, shell integration and durable bindings; the shared runtime owns
platform admission and system-shell discovery. Browser Preview has no native launch capability.

The catalog owns executable and literal arguments. Workbench owns CWD; Desktop retains product
environment, geometry and Host profile policy. Existing bindings retain captured launches across
retries, page replacement and restart. Profile edits never relaunch a running terminal.

## Catalog and validation

The strict version-1 catalog contains `defaultProfileId` and an ordered `profiles` array.
Each profile has a stable ASCII ID, a trimmed display name or built-in null label, and exactly
one command: `system-shell(login)` or `custom-executable(executable, arguments)`.
Only `system-default` may have a null name. Names may repeat; IDs may not.
The default must exist. Deleting the default or final profile is refused.

The common built-in System shell profile disables login on all platforms.
POSIX users may explicitly enable login; Windows rejects it.
Custom executables must be absolute for the platform. Arguments are ordered literal strings,
including empty values; no shell parsing, interpolation or command-string joining occurs.
The shared contract owns the 64-profile, 128-argument, UTF-8 string and 1 MiB catalog bounds.
Catalog storage does not promise executable availability or a future Host request's separate
wire bound. Concrete launch admission precedes the first Host creation effect.

## Discovery and provisioning

Each new System shell launch uses the matched helper and shared platform resolver.
Main captures the selected profile before awaiting discovery. Custom profiles do not depend on
System shell availability. Missing System shell keeps application startup available and fails
only that terminal request, without a fabricated executable or another profile fallback.

Concurrent provision requests for one presentation share pending discovery.
Closing the presentation or disposing its adapter prevents later binding creation and Host effects.
An admitted binding captures one complete launch. Supported POSIX shells subsequently receive
[automatic shell integration](kitty-automatic-shell-integration-alignment.md).
An unavailable custom executable fails through the existing terminal lifecycle.

## Commands and persistence

Read, create from a profile, update, select default, delete and reset use the sandboxed correlated
capability. Each port permits one pending request. Invalid messages or transferred ports retire it.
Destructive UI actions require confirmation; pending controls prevent overlap.

Mutations carry the source frozen when editing began and update the complete catalog leaf through
the shared store. A stale source or refused write preserves the committed catalog and user draft.
Reset writes null, claims the leaf and restores the common built-in catalog.
Lost replies are never replayed; explicit reload discovers the durable result.
Main observes confirmed shared snapshots, including TUI edits.

Production uses shared native storage without importing the old `userData` catalog. The current standalone controller and file adapter remain in Desktop initialization and its standalone capability; replacing them requires migration of the current Main runtime path in the next phase.

## Verification and limits

Cross-language conformance covers catalog bounds. Main and renderer tests cover CRUD, literal
argv, platform validation, source conflicts, pending/disposal, strict admission and failure retention.
Lifecycle tests cover failed discovery, close during discovery, coalesced provision and durable
capture. Real process tests use isolated scopes. Applicable system tests and the affected root
gate exclude the manual native-window test.

Profiles contain no environment, secrets, startup-directory policy, SSH or Workspace configuration.
One-off non-default selection, remote synchronization, arbitrary executable behavior and native
execution on untested systems are outside this contract.
