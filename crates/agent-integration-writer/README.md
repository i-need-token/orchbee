# orchbee-agent-integration-writer

Node-local Agent integration writer: provider inspection, enable and disable plans, and their
filesystem and process effects, using the cohort-admitted observation-adapter capsule and
retained mutation outcomes. Values and report vocabularies come from the
[Agent integration contract](../agent-integration/README.md); this crate never defines them.

## Ownership

- One invocation runs under one cooperative 40-second budget. Callers pass a
  `TerminationSource`; the writer observes it between bounded steps and settles as cancelled.
- Unix and Windows are supported. Windows configuration-location helpers compile only on Windows.
- The actual consumers are `orchbee-worker`, whose job owner runs `explain`, every mutation and
  the Settings `list` for its Session, and `orchbee`, which runs the files-only `list` of
  `agent integration list` in its own process, reuses the read-only configuration helpers for
  offline `agent doctor` and Windows update verification, and, after the environment's Sessions
  have stopped, runs the disable of each provider `uninstall` finds this environment may have
  enabled. It decides from `list_owned`, which reads Windows Hermes's own state from its two entry
  files alone, so it starts no Agent program for a provider without such evidence.

## What each operation starts

Most providers are read from files. Claude Code, Grok and GitHub Copilot report their plugins only
through `plugin list` and `plugin marketplace list` commands of their own program, and Windows
Hermes through `plugins list`; a provider's program may create its own first-run files whenever it
starts. `list` reads files only and starts no provider program, so those rows stay `unknown` with
no issue, not checked, unless Orchbee's own Hermes entry files already prove a conflict.
`explain` is the explicit check of one provider and runs those read commands; `enable` and
`disable` run them before and after their effects. The
[reading rule](../agent-integration/README.md#reading-rule) names the providers for Clients.

## Observation adapter Capsule

[`agent-observation-adapters/`](agent-observation-adapters/README.md) is the source of the Capsule
this writer installs. Native staging and Desktop packaging copy it into each cohort, and
`bundled_payload_sha256` embeds its manifest's identity. Every byte of it except `capsule.json` is
payload, so any edit, including its README, needs a new `payloadSha256` and makes existing
installations `update-required` until `orchbee agent integration enable` repairs them. Its adapter
tests run from the repository root:

```sh
pnpm test:agent-observation-adapters
```

## Capsule location and ownership

Each environment installs Capsules in its own [data root](../local-runtime/README.md#data-root),
under `internal/integrations/agent-observation/<version>/`. The version is
`bundle/package.json`'s; a name that already holds a different payload takes the first free
`<version>-2`, `<version>-3`, and so on, and an install whose manifest declares the current digest
is reused. `capsule.json` keeps the digest as the identity. Publishing writes the owner record
`bundle/scripts/environment.json`, `{"environment":"<stem>","version":1}`, which the digest
excludes, so every copy a provider makes of `bundle/` names the environment that installed it.

An Agent entry belongs to the environment whose Capsule root it names. A root in another data
root's Capsule folder is that environment's. A folder of `~/.orchbee/agent-observation-adapters`,
where every environment installed before each kept its own, counts as release's for every other
build, and release reports an entry naming it as `conflict`; either way the entry stays as it is,
and no environment ever deletes the shared folder. A copied scripts folder belongs to the
environment its owner record names, and to release when it has none. `owned_by_another_environment`
is the one predicate every provider uses for this. Enable and disable add, update and remove only
this environment's entries. Hook-configuration documents keep other environments' entries in
place beside this environment's. A provider that admits one entry, such as the Antigravity named
block or a plugin registered under the shared `orchbee-local` marketplace name, reports another
environment's entry as `conflict`, and enable and disable both refuse it.

### Whole-file entries

Some Agents load Orchbee from a file or folder at a fixed path that every environment on the
machine shares: the OpenCode module file, its Windows client folder
`orchbee-agent-observation-tui` and its obsolete `.mjs` name, the Kilo, Pi and OMP module files,
the Gemini and Qwen Code extension folders, and Hermes's `__init__.py` and `plugin.yaml`. The
writer keeps no record of what it wrote, so it judges each such entry by what the entry carries:

- **Another environment's.** A module or entry file whose bytes are exactly a bootstrap the writer
  renders, current or superseded, for the Capsule root it names, when the predicate above gives
  that root to another environment; or an extension folder whose marker, integration id and
  complete file fingerprint match and whose copied owner record names another environment.
- **This environment's.** The same exact bytes, or a proven folder, naming this environment's
  Capsule folder: `enabled`, or `update-required` for an older Capsule or template.
- **Anything else.** An edited body, a changed fingerprint, a link, another kind of entry, an
  unreadable owner record, or a bootstrap naming a root outside every Capsule folder. An edit
  breaks the exact match, so an entry this environment wrote and a person then changed cannot be
  told from content that was never Orchbee's. Both read `conflict`; enable and disable refuse them
  and leave them unchanged.

On Windows another environment's entry is left exactly as it is. Its row still reads `conflict`,
so no Client shows the integration as enabled for this environment, and enable refuses before any
effect instead of replacing it; the person enables it from the environment that owns it, or
removes it there. Disable, and so `uninstall`, treats it as absent: it removes only this
environment's targets of the provider and completes without an effect when none is left. A folder
or file pair whose members belong to different owners is `conflict`, and Hermes counts as another
environment's only when every entry file present is, which leaves its activation to that
environment too. Other platforms still read another environment's whole-file entry as `conflict`.

At hook time the transport reads the owner record beside it and reports only when the inherited
`ORCHBEE_ENVIRONMENT` equals the owner, or when the variable is absent and the owner is release;
otherwise the hook exits silently and successfully. After a completed enable or disable the writer
removes the other version folders of its own Capsule folder once a fresh inspection shows that no
entry can name one; the current version stays until uninstall.

Because only an installed Capsule lets an entry name an environment, the local runtime's
[`agent_integration_may_have_been_enabled`](../local-runtime/README.md#data-root) answers from the
presence of these two folders without running provider commands or reading provider
configuration, and implicit callers skip inspection when it is `false`.

## Conformance fixtures

`tests/fixtures/` holds the corpora replayed by this crate's harnesses. The `test-support`
feature exposes both replays, and the CLI tests run them with the CLI's integration exit mapping:

- `provider-conformance/` — `payloads.json` plus `cases/<provider>.cases.json`:
  per-provider inspection, enable, and disable cases pinning scripted command
  replies, expected argv, filesystem effects, and the rendered CLI verdict.
- `provider-effects/` — `cases.json`: filesystem-effect machine cases —
  capsule publish, move-aside, backup digests, remove walks, cohort and
  version gates, and budget or cancellation frontiers.

The harnesses resolve both corpora through `CARGO_MANIFEST_DIR`, so any
checkout replays them with no environment override or machine-local path.
A case edit is a semantic contract change and belongs in the same reviewed
change as the engine behavior it pins; a new provider adds one file under
`provider-conformance/cases/`.
