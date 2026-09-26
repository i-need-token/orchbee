# Windows CLI Workspace creation

## Scope and compatibility

This record owns optional Workspace creation inputs, first-Project setup, and the separate Client focus phase. Server owns canonical Project/Workspace/Directory identities and terminal admission; CLI composes existing commands and never starts a shell itself. The [native setup interaction](native-workspace-setup.md) retains its separate TUI draft and drawing rules.

Existing valid fully explicit commands retain their mutation-v4 output: `workspace create --project <target> --label <label>` creates an empty Workspace; adding an absolute `--cwd` launches its first terminal. With no new options, these legacy forms keep their behavior. Omitted required operands or new source, Client, focus, timeout or directory-policy options select the enhanced Windows launch workflow. For example, adding `--no-focus` deliberately opts into enhanced creation without submitting Client focus. Mac/Linux runtime behavior is unchanged.

Enhanced creation accepts optional `--project`, `--label`, source `--workspace`, exact `--client`, mutually exclusive `--focus`/`--no-focus` (default), and `--timeout` (1–3600000 ms, default 30000). Existing env, exec/args, profile and all-or-none terminal geometry admission remains authoritative. Focus requires an existing ready Client before any mutation; it never launches a TUI.

## Source, directory and Project decisions

Explicit path, `home` and `current` directory policies ignore unrelated caller/source-Workspace identity. Paths use the existing Windows absolute, relative and tilde normalization contract; no shell expansion or filesystem probing occurs in the parser. Default `follow` and explicit `workspace` policies may use a source Workspace. An explicit Client overrides an implicit caller; otherwise a validated caller precedes a unique current ready Client. Invalid/partial caller identity, stale Clients and ambiguity refuse instead of guessing. Headless no-focus creation is allowed when no caller or Client is present.

| Decision                                                                   | Result                                                                                   |
| -------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| Explicit cwd path                                                          | Use the normalized literal path                                                          |
| `home` / `current`                                                         | Use the validated Windows home / CLI cwd                                                 |
| `workspace`                                                                | Require the source Workspace's canonical primary directory                               |
| Default / `follow` with proven matching Panel                              | Sample only its primary terminal root-process cwd                                        |
| No matching Panel, or an explicitly absent root cwd                        | Use the source Workspace primary directory, then Windows home                            |
| Missing root process, malformed cwd, process-query failure or interruption | Refuse before Project creation; never guess a child process or another Panel             |
| Explicit destination Project                                               | Use its exact resolved identity; source and destination Projects may differ              |
| No explicit Project, proven source Workspace                               | Use that Workspace's Project                                                             |
| No source Project, one Project                                             | Use the sole Project                                                                     |
| No source Project, zero Projects                                           | Create one localized Default Project under the captured revision                         |
| No source Project, multiple Projects                                       | Refuse ambiguity; never select by label or create another default                        |
| No explicit label                                                          | Use a bounded directory basename; a root directory uses the localized Workspace fallback |

The Default Project has an ordinary stable Project ID. Its label is not a role, lookup key or singleton lock. A competing creation changes the revision and makes the stale attempt fail; CLI does not retry or silently adopt a different Project. [Windows CLI configuration](windows-cli-configuration.md) owns persistent directory defaults. Arbitrary custom-shell cwd tracking and dynamic Git-derived Workspace names are separate contracts.

## Commit and focus model

One invocation owns a prepared directory/provenance, optional selected Client, optional Project receipt, optional Workspace receipt, optional focus receipt, stage and full structured failure. Preparation validates available local inputs before effects. Metadata and control exchanges are bounded at two seconds; Workspace launch consumes the remaining invocation budget with the existing signal wake. No phase automatically resubmits after uncertainty.

| Transition                                        | Guard, result and recovery                                                                                                              |
| ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| Prepare                                           | Resolve source, path, label and Project choice; pin any required ready Client before effects                                            |
| Create Default Project                            | Only a zero-Project snapshot admits one revision-fenced ProjectCreate; record the committed identity before any next read               |
| Refresh after Project creation                    | Require same Site, sufficient revision and the exact created Project; preserve its receipt if refresh fails                             |
| Launch Workspace                                  | One existing WorkspaceLaunch owns directory probing and the canonical Workspace/Directory/Tab/Panel commit                              |
| Project or Workspace submission unknown           | Preserve full request/source failure data and earlier receipts; stop without rollback or replay                                         |
| Workspace terminal pending or started             | Preserve canonical IDs; optional focus may select the pending placeholder using the existing render/seen owner                          |
| Workspace terminal failed, unavailable or unknown | Preserve canonical IDs, stop focus and return the corresponding nonzero exit                                                            |
| Focus preparation                                 | Fresh same-Site snapshot, exact Project/Workspace membership, sufficient canonical revision and exact current ready Client              |
| User selection guard                              | Require unchanged semantic selection, or the exact newly created Workspace/Tab/Panel already selected by first-Workspace reconciliation |
| Focus submission                                  | Use the fresh view revision for one Context intent and existing rendered/seen acknowledgement                                           |
| Later failure, timeout or cancellation            | Preserve earlier commits and their identities; cancellation stops waiting, not accepted Server work                                     |

The already-created selection allowance is specific to Workspace creation. Worktree focus retains its stricter unchanged-selection guard. Derived panel counts and ordinary canonical refresh revisions are not navigation identity. A different observed context/Tab/Panel or zoom state stops focus. None of these checks proves an atomic Git/process/UI transaction.

Enhanced output uses `orchbee.workspace.creation` v1 and records Project setup, Workspace mutation, directory provenance and Client effects independently. Known failures retain the original structured failure, including request identity for uncertain submission. The outcome is complete only when the requested phases succeed; a pending terminal remains explicitly pending. Parser/preparation failures use existing failure envelopes. Legacy mutation output remains unchanged.

## Worktree first-Project reuse

The same zero-Project preparation serves [Worktree Create/Open](windows-worktrees.md). Source and focus admission complete before Project creation. Existing Project inference and ambiguity rules remain unchanged. A Default Project attempt uses `orchbee.worktree.operation` v3 with a separate setup receipt/stage/full failure; normal no-setup calls retain v1/v2. Known Project creation survives a later pre-Start failure, Git failure or uncertain submission. The retained Git intent and Server continuation are unchanged; configured checkout paths and failed-removal restoration have different owners.

## Evidence and limits

The deterministic evidence covers legacy/enhanced grammar, path redaction, zero/one/multiple Project selection, source precedence, directory absence versus invalid observations, labels and focus guards. Scripted Windows CLI traces cover competing revisions, Project commit followed by failure, uncertain handoffs with no replay, terminal pending/failure, first-Workspace reconciliation, changed selection and timeout. Versioned schema checks preserve each partial receipt. Fixtures own and release their profiles, pipes and processes.

Reference semantics are Herdr `e0507237ad8d41f60aece10d00ef29ce2339ca34`, WorkspaceCreate and the focused-pane/seed/home creation policy. Orchbee's explicit Project ambiguity, exact Client routing and retained partial receipts remain visible adaptations. These are bounded same-author checks; full two-TUI acceptance is separate and TLA is deferred. The matched cohort rolls out together; rollback does not undo already committed user Projects or Workspaces.

Composite failure observations preserve `kind`, nullable `details`, nullable `source`, original `schema` and `version`. This nested record belongs to the composite schema; local guards without details do not pretend to be standalone failure envelopes requiring details. API catalog v65 contains 74 schemas; command paths remain 86.

Creation preparation errors without more specific details use generic `orchbee.cli.failure` v2, restricted to the `workspace-creation` stage and missing/unavailable/protocol/transport/timeout/cancelled kinds. The existing v1 failure matrix is unchanged. Nested failure observations validate each producer's exact detail/source constraints; local guards without details must also lack a source.
