# Agent screen detection model

The Server owns fallback screen interpretation separately from explicit Agent reports and process identity. A successful bounded terminal sample can describe working, idle or blocked UI; it cannot prove which provider owns a process, authenticate it or create a canonical Agent Session. The Windows periodic sampler compiles the deterministic engine once before startup and publishes admitted screen fallback. Physical explain consumes the same engine through the bounded observer queue. Startup now loads bounded local overrides and, with a configured update catalog, eligible remote cache with typed provenance warnings. Manifest queries and retained live reload now use the observer owner; explicit remote updates retain separate cache and activation facts; automatic scheduling follows the policy below.

## Inputs and rule ownership

The private engine accepts a known provider or absence, recent live terminal text, OSC title and OSC progress as separate fields. Screen input is limited to 64 KiB and each OSC field to 4 KiB; over-limit input is rejected whole, never truncated into misleading evidence. Text is already decoded terminal content, not raw escape sequences. Unknown providers remain unknown even when text resembles another provider. Known providers with no matching rule remain unknown, with no visible confidence flag; absence of evidence cannot prove idle or completion. Mastracode and OpenClaw have no bundled screen manifest and use this known-provider fallback. CodeBuddy Code, Command Code and DevEco Code bundle only their startup blockers, so their idle and working screens also stay unknown.

Bundled TOML files are retained under `resources/agent-detection`; files derived from Herdr keep its Apache-2.0 license and source attribution, and `NOTICE` names the files Orchbee authored. Server maintainers own review and pinned updates. Parsing rejects unknown fields, invalid states/regions, empty rule identifiers, unsupported engine versions, invalid skip-state combinations and excessive complexity before compiling any executable rule set. Each manifest allows at most 128 rules, 512 gates, depth 8 and 1,024 matchers; each gate allows 32 direct matchers of at most 512 Unicode characters. Regex compilation has explicit memory limits. The already locked `regex` and `toml` packages supply syntax that the standard library cannot implement; no new versions or runtime framework are required.

Idle rules need positive live composer chrome and lose to any busy or question chrome; a bare word, a clipped footer or an unknown title stays unknown. Opencode 2.x idle needs the prompt-box bottom and the command footer without an interrupt hint, progress squares or question footer; an interrupt hint in that footer is working. Pi 1.x idle needs the editor border and the footer context-usage line without a status spinner; status loaders for turns, shell commands, retries and compaction are working. Omp 18.x writes its state into the OSC title as `π <glyph> <label>`: `>` idle, `!` attention (blocked), a spinner frame or `:` working; a disabled title animation (`π: <label>`) carries no state. Its screen rules mark the Working row, the shell Running border and a spinner status line as working, and the `π ·` status line under the composer as idle, so a lagging idle title cannot hide a visible turn. `agent_screen/tests.rs` covers these rules with redacted screens from opencode 2.2.5, pi 1.5.0 and omp 18.16.0. All three upstream versions also report their own state through OSC 7501, which outranks these rules while it is fresh; see [Program status precedence](#program-status-precedence).

The bundled Devin Thinking rule requires an anchored live footer with elapsed time and an interrupt hint; a transcript mention or the word Thinking alone is insufficient. A startup choice that the user must answer before the Agent proceeds is a visible blocker. Devin's workspace trust prompt and its first-run theme question (`Detected light theme but configured for dark. Switch?`) need the question and the live choice footer; the question text alone stays unknown. Sampled rows lose trailing whitespace, so needles must not depend on a trailing space. Screen manifests reject `done` and `error`: those outcomes require explicit state authority, not text inference.

MiMo Code is built on OpenCode 1.x, so its rules are the OpenCode 1.x rules of `opencode.toml`: the
permission blocker, the interrupt hints and the busy spinner, which MiMo draws as a UFO with a dot
trail instead of progress squares. The OpenCode 1.x rules have no idle rule, so MiMo adds one: the
prompt-box bottom and a key-hint footer ending in `commands`, without an interrupt hint or the
spinner; a narrow pane wraps each hint, so a lone `interrupt` row is busy chrome. Its `MC |`-prefixed
title only names the session.

Orchbee authored the AtomCode, Reasonix, Snow CLI, MiniMax Code, MothX, Prime Agent and J-Code
rules from Windows captures. Where the CLI writes its state into the OSC title,
the title decides: AtomCode's green or yellow status light, MiniMax Code's `Ready`, `Working`,
`Done` or `Stopped` prefix, J-Code's `· work ~` suffix and Snow's `Action Required` prefix. A visible
busy line still outranks an idle title that lags. Reasonix, MothX and Prime Agent leave
the title to the shell, and titles that only name the session carry no state, so screens decide: a
braille or glyph spinner status with elapsed time, or a footer interrupt hint, is working, and the
CLI's composer or key-hint footer without it is idle. A startup question that waits for an answer is a visible
blocker: workspace trust (MiMo Code, DevEco Code, CodeBuddy Code), sign-in or login method (Command
Code, CodeBuddy Code), the agreements page (DevEco Code), telemetry or trace sharing (Reasonix,
Prime Agent) and J-Code's y/n/d setup questions. Each needs its live choices or empty answer row, so
an answered question stays unknown. Only Snow's tool confirmation has been observed; the other CLIs
ran the captured shell command without asking, so their approval screens have no rule apart from
MiMo Code's OpenCode 1.x permission rule.
`agent_screen/tests.rs` covers these rules with redacted excerpts.

The bundled Grok rules include browser authentication chrome from Grok
Build `2bdd1d6a6369de0e8c68132ea4539e9abd9e14a8`. An anchored approval/login waiting caption plus
authentication context is a visible blocker and outranks idle or busy OSC signals. A caption
without that context stays unknown through `browser_login_unconfirmed`, rather than claiming
readiness from a generic title. Replacing the login screen with the ready composer permits idle
again; process exit keeps its existing precedence. `agent_screen/tests.rs` covers context, clipped
context, separate OSC conflicts, transcript mentions and the ready-screen replacement. This is
physical-screen fallback and never creates or changes a canonical Agent Session.

## Program status precedence

Programs that implement the [OSC 7501 Program Status protocol](https://www.superlogical.com/rex/docs/build/program-status)
report their own state; opencode, pi and omp do. The Host answers the protocol's query and projects
the terminal's root record in optional OBTF kind 7 with the real time it received the report, and
removes it on `state=clear`, RIS, Host reset and, for `idle`, `working` and `blocked`, at the next
shell prompt (see the [Host runtime model](../../terminal-host/docs/runtime-model.md)).

For the Agent process a sample selects, the sampler uses the record from the same fresh frame
instead of reading rows and applying screen rules when the record is fresh: its receipt time,
converted to process creation-time units and rounded down, is no earlier than that process's
creation time. A record received before the process existed was left by an earlier program in the
same terminal, such as an Agent that exited without clearing it, and is stale. An absent or stale
record leaves the decision to the screen rules. There is no time-based expiry, because the
protocol forbids requiring a heartbeat, and switching screens does not end a record. Process exit
needs no record rule: without a selected Agent there is no screen evidence, and a later Agent
process is newer than any record it did not send.

| Program state | Panel state | Evidence                                                                 |
| ------------- | ----------- | ------------------------------------------------------------------------ |
| `idle`        | Idle        | Visible idle: no startup grace and no idle hold                          |
| `working`     | Working     | Visible working                                                          |
| `blocked`     | Blocked     | Visible blocker, with a visible blocker's precedence over custom reports |
| `done`        | Done        | Explicit, without startup grace                                          |
| `error`       | Error       | Explicit, without startup grace                                          |

A record is the program's explicit report, so unlike screen rules it may publish Done and Error.
It is still fallback: it enters the same stability, startup and publication path as screen
evidence, both surrounding process samples must select the same Agent, custom report authority
keeps its precedence, and process exit keeps its own. The record is unauthenticated: any program
writing to the terminal after the Agent started, including a tool the Agent runs in the same
console, can change it. It only changes the Panel state, never identity, provider or authority.
Physical `agent explain` evaluates screen rules only; when its state differs from the reported
state and no custom report source is present, a program status record decided.

## Deterministic decisions

| Input or match                          | Result                                                                                                    |
| --------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| No known provider                       | Unknown, no visible evidence                                                                              |
| Multiple matching rules                 | Highest priority; first declared rule wins equal priority                                                 |
| Contains                                | All needles, Unicode lowercase comparison                                                                 |
| Regex / line regex                      | All patterns; case-sensitive unless the expression changes flags; line patterns may match different lines |
| Nested all / any / not                  | All children / at least one child if present / no child matches                                           |
| No matching rule for known provider     | Unknown without state evidence                                                                            |
| Transcript/history viewer rule          | Skip publication, preserve existing fallback and clear pending idle confirmation                          |
| Visibility flag inconsistent with state | Flag is false in the derived result                                                                       |

Regions select live prompts, response markers, prompt boxes, top/bottom nonempty lines and separate OSC fields. Line boundaries retain correct UTF-8 offsets for LF and CRLF, including blank lines and a final unterminated line; no byte offset may split a Unicode character. All matching is pure and does not access files, processes, clocks or mutable global state. Compiled rules may be retained by the sampling owner and reused.

## Publication integration contract

The Windows Host adapter validates the exact attachment, Worker generation and Host identity and associates screen evidence with the same observed PID/creation identity. It may not reinterpret a failed/missing frame as an empty screen or derive idle from unavailable data. Existing process exits retain precedence. A newer visible blocker may override a matching custom report through the report aggregate.

Each owned read-only viewer reads process identity before consuming its initial full frame, requests a fresh snapshot with StoreReset for that viewer alone, then reads the bottom live-height rows at that exact frame revision and generation. The same fresh frame supplies directory/projection facts and screen interpretation; a second snapshot is unnecessary. UI scroll position does not choose detection text. Both surrounding complete process samples must select the same PID, creation time and provider. A failed or stale frame/row read supplies no screen evidence; the successful complete process sample can still establish presence or exit. Transport rows are bounded to 8 MiB and decoded text to 64 KiB, preserving physical newlines and trimming trailing whitespace/blank tail rows. Title and progress come from the same frame; typed progress projects the current OSC 9;4 state, not raw escape history. No Client display frame is acknowledged.

The coordinator retains screen stability and a three-second startup grace for evidence without a visible working, idle or blocker flag per physical process identity. Explicit visible evidence bypasses that grace, so a short first turn can publish Working and return to Idle before three seconds elapse. Process-only observations preserve screen timing; identity departure/replacement resets it. Changed explicit-report input permits fresh matching screen evidence to republish the settled fallback without bypassing idle confirmation. Prepared timing and fallback commit only after report/metadata admission succeeds. Bundled rule license and attribution ship in the Windows cohort under `licenses/agent-detection`.

Working-to-plain-idle transitions hold for three further confirmations or 700 ms; explicit visible idle/blockers, provider change or exit clear that hold. Stable visible blockers refresh at 800 ms so newer weak reports can be challenged without publishing every sampling tick. Startup grace and sampling cadence are adapter responsibilities. Screen and process sampling must retain the existing required-completion and uncertain-viewer-cleanup ownership. No controller/input/focus effect is admitted.

## Evidence and delivery

The accepted rule-maintenance model gives the Server one immutable compiled ruleset and its
provenance. Windows startup resolves the [data root](../../../crates/local-runtime/README.md#data-root)
without rewriting user settings; an explicit settings file does not move it. For each bundled
provider, `<root>/agent-detection/<report-label>.toml` overrides
`<root>/internal/cache/agent-detection/<report-label>.toml`, then bundled rules. The remote cache
takes part only while a valid update catalog is configured and the cache's `catalog.json` records it
as the source of its rules; otherwise its rules are never read, and a local override precedes
bundled rules. Without a configured catalog the Server never reads the cache. Reads admit regular UTF-8
files of at most 256 KiB with the same provider, engine and complexity validation as bundled rules.
Present versions must be dotted numeric; remote caches must also declare an engine version and
be no older than bundled versions. Trailing zero segments compare equally. Normalized manifest
identifiers or aliases may match the provider. Invalid override/cache inputs produce typed warnings and fall back to the
next valid source. Providers without bundled rules retain unknown fallback. The sampler and
Explain consume the same installed ruleset. Explain provenance identifies the actual path, cached
version, local shadowing and warnings; it does not manufacture update status. Canonical CLI v1 and
historical physical v2/v3 schemas remain closed; current physical explanations use v4/v5 for the six-state vocabulary. The full maintenance
workflow includes current-manifest queries, retained bounded reload and stabilization reset, but still requires an
owned real updater with atomic cache commits, truthful origin/status and cancellation/cleanup.

Physical `agent explain` is a read, owned by the same detector and temporary-viewer lifecycle.
An exact binding and current Workbench/report context are prepared under the Worker/Session fences.
The effective known provider, or the detected provider behind a custom label,
selects the rules. A fresh frame/row/OSC capture is bracketed by stable complete process samples;
the same binding and report context must still hold before publication. A changed context refuses
without retry. Raw rule prediction is distinct from the effective, stabilized reported state.

The process-observer thread owns at most one executing and one queued diagnostic request, together
with its existing sole pending viewer cleanup. [Current-directory reads](../README.md#workspace-current-directory-reads)
keep their own such pair on the same thread, which runs one read at a time. CLI cancellation ends the caller wait; it is not a wire cancellation request. The Server gives
each diagnostic one four-second queue/read budget; timeout and Session shutdown stop new work, while
admitted Required exchanges still settle. Unconfirmed Detach keeps ownership and blocks
new attachments. Queued expired work cannot attach. Diagnostics never send input, acquire control,
change Client focus or publish a new Agent state. Periodic observations keep their existing owner.

Explain exposes the actual manifest source/version, every evaluated rule's identifier, priority,
region, state, match and top-level matcher evidence, gate counts and region byte count. Region
previews take 240 Unicode scalars and append `...` when longer. First-declared highest-priority
matching rule wins; fallback and skip reasons describe the same deterministic evaluation. The
existing input/manifest/complexity limits and 512 KiB PanelAgent result bound apply; over-limit
results refuse whole. Update metadata comes from the retained maintenance owner. Pure rule witnesses, exact-context refusals, queue cancellation/cleanup traces and CLI
transport/schema checks precede final Windows Host/provider acceptance.

Rule, region, bounds, priority and malformed-manifest tests exercise the engine before transport integration. Pinned-reference cases and differential evaluation must cover every bundled provider, nested gates, transcript skip and OSC precedence; a passing static engine does not close the live CLI/TUI requirement. Host acceptance must prove real Windows screen changes and report arbitration without user query effects. Source/cohort revert is rollback; no durable state migration is involved. Complete the affected host gate and a controlled live Devin Thinking footer check before delivering this state correction; broader provider parity is a separate scope.

## Installed manifest maintenance

The Windows process observer owns the installed detector and a Session-lifetime revision starting at 1.
`workbench.agent-manifests` (OBWP 37/38) reads this snapshot, submits a reload identified by an
operation ID, or reads that retained operation. Queries never reread files. Reload reads the same
configuration parent resolved at startup. It runs between observer turns; sampling and Explain
switch together. Under the maintenance lock and Session gate, installation clears screen stabilization
without discarding process identity or explicit reports, then publishes the new engine and revision.

Reload shares one queued/running slot with Update. Up to 1,024 operation receipts are retained for
the Session lifetime without eviction; duplicates return the existing receipt, and busy/capacity
refusals occur before admission. Reload transitions pending -> applied(revision) or failed(reason).
Stopping cancels pending operations before installation. A failed load retains the prior engine.
Read/status use read policy; Reload/Update use navigation-management policy. CLI uses one bounded budget,
submits once, and retains the operation ID with pending/unknown results after lost acknowledgement
or interruption. Status recovery never resubmits a mutation. Receipts end with the Session generation;
this is not durable replay across Server restart. Explicit remote Update and its separate cache/activation outcomes are specified below.
Automatic scheduling follows the configuration and ownership rules below.

## Live Agent self-test

Rerun the live self-test after an Agent CLI upgrade or a bundled rule change, from a staged Windows
cohort:

```sh
node apps/orchbee-server/scripts/agent-detection-self-test.mjs --orchbee <cohort>\orchbee.exe --scratch <new directory> [--agents opencode,pi,omp] [--path-prepend <dir>] [--env NAME=VALUE]
```

The script creates the scratch directory, points `APPDATA`, `LOCALAPPDATA`, `ORCHBEE_HOME` and
`ORCHBEE_RUNTIME_BASE` into it, starts its own Session and opens each Agent in a `cmd.exe`
workspace. `--path-prepend` adds directories, such as a Node toolchain, to that shell because Worker
terminals start from the user's registry environment; `--env` sets a variable in that shell before
launch, such as `PI_PROGRAM_STATUS=0` to run pi without OSC 7501 reports. It drives terminals only
through `orchbee terminal send` and records the Panel's reported state and detection screen for each
step: launch (idle, or blocked by a trust or login prompt), one turn (working, then idle or done),
and for opencode a question (blocked, then idle or done after Escape), and exit (no reported Agent).
Beside each reported state it samples the state physical `agent explain` derives from screen rules
alone, and names the input that decided the step: `report:<source>` for a custom report,
`program-status` when the reported and screen-rule states over the step differ, and `agree` when
they match and either could have decided. A turn sends one trivial model request through the Agent's
existing login, or a local `ping` shell command for pi and omp. pi and omp report a user shell
command as idle, so with program status their shell turn passes while staying idle, provided the
screen rules alone saw it working. The script types nothing unless launch reported idle, never
accepts a trust, login or approval prompt, and leaves prompts only with Escape or the CLI's quit
keys. Prompt text and Enter are sent separately, because some composers treat one burst as a paste; quit keys go in one burst for CLIs
that need a quick double press. A step passes when the reported transitions match its expectation;
a failure needs its capture reviewed before it is called a detector defect, because CLI quotas,
missing models, first-run prompts and slow exits also fail steps. It stops its Session and writes
`report.json` and per-step captures into the scratch directory; the caller deletes it. Captures
contain local paths and account details, so a regression fixture must be redacted before it enters
`agent_screen/tests.rs`.

## Remote update ownership

Orchbee ships no default rule catalog. Remote maintenance runs only when the Server starts with
`ORCHBEE_AGENT_DETECTION_MANIFEST_CATALOG_URL` set to a valid HTTP(S) catalog URL; the value is
frozen for the Session. Without it the Server uses only bundled rules and local overrides: it
schedules no check, refuses explicit Update as unavailable, makes no network request and ignores any
existing remote cache, whose origin can no longer be confirmed. A set but invalid value has the same
effect and is reported as a typed source error. A catalog and every manifest are
bounded to 256 KiB; known-provider duplicate entries, unsupported schema, unsafe relative paths,
invalid UTF-8/provider/engine/version/complexity are refused before committing executable rules.
Cached versions never decrease. Equal numeric versions must have identical bytes; changed bytes
without a version increase fail. Each newer valid manifest commits atomically, so partial provider
failures retain independent outcomes and never masquerade as a successful whole update.

A data root keeps one cache, holding the rules and the `catalog.json` status that records the
catalog URL the last update used; the writer lock makes one updater its only writer. Within a
catalog the status is replaced after the rules it describes. When the status names another catalog,
or none exists, an update removes that status and then every cached bundled-provider rule before
recording its own; a failed removal stops the update, so no status ever attributes one catalog's
rules to another. A missing status, or one that names another catalog, means no cache: rule
selection and maintenance status ignore it until the configured catalog's next update. Two Sessions
of one data root configured with different catalogs therefore discard each other's cache; a Session
that started while the other's rules were current may keep using them until it reloads. An
unreadable status remains a cache status error and refuses updates. Rules that earlier versions
cached under `agent-detection/remote` beside the settings file are no longer read.

The Server owns logical cache/update state. The Windows filesystem adapter pins admitted private
ancestors/files and provides one cross-process writer lock, `catalog.lock`, in the cache.
It preserves malformed or unsafe user data instead of repairing it. Bounded named pending leaves
belong to this cache owner; explicit settlement removes them after child/file consumers end and
later recovery only touches these owned leaves under the same lock. The downloader resolves the
inbox curl from the Windows System Directory, disables its implicit configuration, uses no shell,
and owns its bounded HTTP(S) fetch, output and process settlement. Its child environment contains only
`SystemRoot`, derived from the parent of the OS System Directory for Winsock provider loading.
URL credentials, query strings, fragments, controls and unsafe catalog-relative paths are refused.
Each fresh-output attempt is limited to 15 seconds, with at most three fetch attempts and a ten-minute
overall update budget. Confirmed Job exit and pipe-reader join precede returning fetch results.
Network work belongs to a
separate retained worker; the observer remains the sole rule installation point. Download/cache
completion and activation are distinct facts, including cancellation and partial success.

The production `server update-agent-manifests` operation consumes this updater and retains the full
cache report separately from activation. Explicit and automatic updates use the same cache/version and cleanup rules. Automatic network
activity requires a configured catalog, the release/background policy and update setting; source/cohort rollback leaves overrides and committed cache
versions intact.

## Retained production update operations

Reload and Update share a single active maintenance slot and the bounded Session-lifetime receipt
ledger. An accepted operation ID is bound to its kind; cross-kind reuse is a conflict, and a repeated
same-kind request reads the original receipt. Update executes on a separately owned thread. The
observer polls only finished work, joins it, and installs its validated detector between sampling
turns. Cache commits and activation have distinct receipts. Partial provider failures remain visible
and cannot produce a successful whole-update exit. A load/activation failure never erases committed
cache results. A missing catalog disables Update, and an invalid one also reports a typed
diagnostic; terminal startup and local Reload remain available. With a configured catalog, Startup
and Reload read persisted status without creating cache paths; ordinary Current queries read the
owner's retained snapshot.

Stop closes admission, cancels network work and drains/joins the update before the observer exits.
An executing update remains pending until its actual cache report and child cleanup settle; it is
never installed after Stop. Worker panic yields an unknown effect receipt and disables subsequent
Update admission for this Session. CLI Update submits once, then reads status within its single budget;
late cancellation or missing confirmation preserves the ID without resubmitting the mutation.
Native v1/v2 remain closed; automatic policy metadata uses v3. Scheduling and configuration
ownership are specified below.

## Automatic checks and configuration snapshots

Windows `agent_detection.auto_update` is a strict boolean, default true; it takes effect only with a
configured catalog. Only persistent managed Sessions in
release, non-test builds allow background checks. Standalone, debug and test execution do not
schedule network work. An enabled owner schedules once at startup, then every thirty minutes by
monotonic time. Busy maintenance defers the due check until the shared slot is free. Automatic work
uses the same downloader, cache, activation and Stop lifecycle as explicit Update. It has a distinct
owner and one retained latest result; it never consumes or evicts the manual operation ledger.

Client activation passes only the validated configuration path and explicit-location status. The
Server reads that file once at startup; the check value and provenance come from that read, and the
observer never rereads configuration. Only a committed configuration publication (`server
reload-config`) changes the manifest-check setting. Explicit rule Reload never reads `config.toml`:
it loads the detector and status from the startup configuration parent, and a failed installation
retains the old detector. Disabling clears the next check; re-enabling schedules immediately. This
is rule-maintenance reload, not a general settings or keybinding reload. An active Update owns the slot
until settled. Current metadata exposes check/provenance, background/build gates, effective enabled
state and the latest automatic operation; native v3 retains closed v1/v2. Automatic panic disables
future updates for that Session; known failures retain the next interval. Stop cancels and joins the
same owned worker before the observer returns. Source/cohort rollback preserves user configuration
and cache files; older strict settings readers require removal of the newly admitted update section.

Automatic activation follows the pinned cache-reload predicate: a newly committed provider or a successfully checked current cache version different from the loaded cached version requires activation. An unchanged current check records NotAttempted and retains detector revision/stabilization. Explicit Update retains its requested activation behavior.
