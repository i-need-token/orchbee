# CLI Command Surface

## Authority and scope

This standard owns the invocation grammar of every Orchbee-authored command-line executable in any
implementation language: command paths, options, targets, help, version, completion, and the
discoverability contract for people and Agents. Output, exit codes, and localization follow
[`output.md`](output.md); process inputs, side effects, and deadlines follow
[`process.md`](process.md); evidence follows [`verification.md`](verification.md). Full-screen
behavior after a command enters a terminal interface belongs to the `tui/` standards. The
concrete command inventory, its schemas, and its failure matrix belong to the owning application
guide; this standard governs their shape.

## Grammar authority

Each executable has one grammar authority in source: a declarative description of every command
path, option, value set, operand limit, target rule, conflict, repeatability rule, and platform availability. Help text, completion scripts, the machine schema
listing, and the application's reference documentation derive from that authority or are tested
to agree with it. A command that exists in the binary but not in help, completion, or the
reference is a defect, and so is the reverse.

A release build may withhold the command surface of a feature that is not yet polished, when the
owning application guide names it. The grammar authority marks the withheld paths and values; a
release then leaves them out of help, completion, and its Agent guide, and refuses an invocation
that reaches one, including through `help` or a usage error, with the typed release refusal
(`stage: "release"`, `reason: "withheld-in-release"`, exit `4`) before effects. Development and
test builds offer every path. The build's environment decides release-ness once; no command
compares artifact names.

Pure grammar is platform-independent: every supported host accepts the same syntax and emits
the same help and completion for a selected locale. Platform compilation gates belong to execution
adapters. A recognized operation without an implementation returns the typed platform-unavailable
failure (`stage: "platform"`, `reason: "unsupported-platform"`, exit `4`) before effects. Omitted
options remain omitted in parsed requests when execution owners have different documented defaults;
opaque cursors are decoded only by the backend that emits them. Neither boundary introduces aliases
or silently changes a supported backend's behavior.

A new option is declared once, including its scope and value/multiplicity constraints; the hand-written
parser and discovery projections consume it. Keep interpreter, effect and regression evidence that
can detect errors in those projections; remove tests that only compare tables now derived by construction.

## Command paths

| Rule        | Contract                                                                                                                                                                                                                                                                                                                                      |
| ----------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shape       | `<executable> [global options] <resource> <verb> [operands] [options]`; a resource names a domain noun and a verb names one effect class. A root-level word such as `status` is a complete path.                                                                                                                                              |
| Words       | Fixed lowercase words with no abbreviations, aliases, or alternate spellings. Adding a synonym is a grammar change, not a convenience.                                                                                                                                                                                                        |
| Verbs       | Reuse an existing verb before adding one. Read verbs never mutate; mutation verbs submit at most one effect; wait verbs observe until a condition or a deadline; activation verbs converge a runtime toward readiness and may create persistent state.                                                                                        |
| Families    | A resource path without a verb, or a verb without its required operand, is a usage failure whose diagnostic names the exact `--help` path. Nothing executes with defaults — this clause covers verb and operand inference only; an omittable option group may declare a default when the command's `--help` text states that default exactly. |
| Root        | The bare executable and its root-level modes such as launching an interface, `--version`, and `--help` are enumerated by the grammar authority like any command.                                                                                                                                                                              |
| Offline set | `help`, `--help`, `--version`, `completion`, the machine schema listing, and Windows `config default` are offline: they read no user configuration, discover no runtime, and open no endpoint. Build-owned default configuration output is locale-neutral and is selected before any environment read.                                      |

| Verb class | Verbs                                                                                                         | Effect                                                                                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Read       | `list`, `get`, `read`, `status`, `describe`, `explain`, `check`                                               | Observe existing state; create nothing that survives the invocation                                                                                          |
| Mutate     | `create`, `rename`, `move`, `split`, `close`, `delete`, `remove`, `uninstall`, `dissolve`, `invoke`, `cancel` | Submit exactly one effect under one request identity                                                                                                         |
| Wait       | `wait`, `changes`                                                                                             | Observe until a declared condition, a cursor boundary, or the deadline                                                                                       |
| Activate   | `start`                                                                                                       | Converge a runtime toward readiness under one invocation-owned deadline; may create persistent runtime state and is idempotent on an already-ready target    |
| Relay      | `connect`                                                                                                     | Converge like `start`, then carry one admitted connection's protocol bytes over standard input and output until either side ends; both streams must be pipes |

Deletion verbs name distinct effects: `close` ends live resource lifecycle; `delete` destroys a
stored record; `remove` removes managed filesystem or machine membership; `uninstall` removes
an installed package; `dissolve` removes grouping while preserving its children. These are not
synonyms. Compound verbs such as `close-all` retain their base effect and declare their scope.

Cardinal `--direction` values are `left|right|up|down`. Ordered Tab movement uses
`--order previous|next|first|last`; text search uses `--search-direction forward|backward`.
A Panel-family target is positional, with `--current` for contextual selection; other families
use typed `--panel` or `--terminal` selectors. Names use `--label` for display metadata and
`--name` for managed identity. Provider selection uses `--provider` and canonical provider names,
except that a command whose sole subject is one provider to start, such as `agent start`, takes it
as its first positional operand with the same closed set; capability subsets must be documented.
Key modifiers use `ctrl+`, `alt+`,
`shift+`, `super+`, with canonical lowercase named keys and literal printable scalar input.

## Options

| Rule           | Contract                                                                                                                                                                                                        |
| -------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Spelling       | Long options only, `--kebab-case`; the only short forms are `-h` for help and `-v` or `-V` for version when the executable offers them.                                                                         |
| Values         | `--name value` and `--name=value` are equivalent. The next token is consumed as the value even when it begins with a dash; a bare `--` is never consumed and yields a usage failure.                            |
| Booleans       | A boolean option takes no value and has no `--no-` counterpart unless the grammar authority declares one.                                                                                                       |
| Multiplicity   | Every option is single-use unless the grammar authority declares it repeatable; a repeated single-use option is a usage failure.                                                                                |
| Declaration    | An option is valid only on commands that declare it. An undeclared, conflicting, or missing required option is a usage failure before any runtime access.                                                       |
| Global options | Options that select the target runtime, machine, or output format are positioned before the command path when the executable defines them as global; a command-scoped option never changes meaning by position. |
| Terminator     | `--` ends option recognition; every later token is an operand.                                                                                                                                                  |
| Value sets     | Closed word sets are exact and lowercase; numbers are decimal with declared bounds; a missing, empty, unknown, or out-of-range value is a usage failure that names the option.                                  |

## Targets and filters

- A `<target>` accepts a complete typed identifier or a stable compact identity
  `@<kind>:0x<hex>`. The kind is `project`, `workspace`, `tab`, `panel` or `terminal`;
  hex contains every significant digit of that nonzero 128-bit ID, lowercase, without leading
  zeros (1–32 digits). Resolution pads with leading zeros and compares the exact identity.
  Human lists and candidate diagnostics print this stable form; JSON retains canonical IDs.
- Recognized selector kinds must match the operand's resource. Empty, uppercase, malformed,
  oversized or zero compact identities fail before runtime access. Other `@` tokens keep
  their label interpretation. Labels are exact and case-sensitive where admitted; no prefix,
  fuzzy or case-insensitive matching occurs. ID-shaped labels require their real identity.
- Zero matches is not-found. Multiple label matches are ambiguous, reporting
  a bounded candidate list and true total. A command never picks one or falls back to a label.
- A printed stable handle does not depend on peers, list filtering or display order. Creating
  or removing other resources cannot change its meaning; deleting its exact identity makes
  it not-found. Identity lifetime and runtime scope remain those of the underlying ID: use
  the selected Session, and rediscover runtime terminal IDs after Server restart. Persisted
  Workbench identities retain their existing non-reuse/high-water contract. Handles add no
  global alias registry or cross-Session identity guarantee. Every operand still resolves
  inside its declared scope, with no global fallback.
- Resources whose labels are not unique by contract accept identifiers and their own selectors
  only.
- Filters use the same target rules as operands and are declared per command. A filter naming an
  absent entity is not-found; an existing entity with an empty result is a successful empty list.
- Pagination options, when a command declares them, are `--limit <n>` with declared bounds and an
  opaque `--cursor <cursor>` returned by the previous page.

Terminal input/output commands may declare a Terminal-or-Panel target: `terminal run`,
`terminal read`, `terminal send` and `terminal wait-output` accept Panel IDs, `@panel:` and
`--panel`. A Panel resolves only to its primary terminal. Child terminals require their own
terminal identity. A bare ID that selects different resources across both kinds is ambiguous;
`@terminal:` or `@panel:` disambiguates. Other commands retain their declared target kind.

## Help, version, and completion

- A single `-h` or `--help` anywhere before `--` emits the same text as `help <path>` for the
  longest recognized command path, ignoring supplied operands and business requirements.
  Recognized option values are scanned but never applied; help always emits text even with
  `--format json`. Unknown options, missing values and duplicate options remain usage failures.
  Family paths must match exactly, so a misspelled verb never becomes its parent's help.
  Command-specific options without a command path remain out of scope.
  Help exits `0` without any runtime; an unrecognized path is a usage failure. Literal tokens
  after `--` retain their ordinary operand meaning.
- Help describes the command's operands, options, value sets, and effect class in the effective
  locale. It never enumerates live resources.
- Help pages at the same level share section headings and ordering, omitting inapplicable sections.
  Root help contains the executable summary, usage, optional quick start, command groups, global
  options, and navigation; it never inserts feature-specific concept sections.
- Concept explanations and onboarding belong in the owning application's guide. Command-specific
  examples and operational constraints use the shared examples and details sections, rather than
  custom headings or explanatory layouts for individual command families.
- Commands that accept `--format` show both a default text example and an explicit JSON example.
- A missing child on a family path lists its direct commands. A usage diagnostic may add at most
  three candidates from the grammar scope at the failure point: direct children for an unknown
  command, chosen by close spelling or a declared common synonym, or admitted options for the root,
  selected family, or exact command path. A stray word on a command that admits `--label` may be
  offered as `--label <word>`. Diagnostics raised before the grammar runs point at the help of the
  command path the leading words spell. Advice never changes parsing, validity, or effects.
- `--version` is legal only as the sole argument and prints one line naming the executable and its
  version.
- `completion <shell>` prints a static projection of the grammar for a declared closed set of
  shells. It enumerates commands, options, and closed value sets, never runtime state.
- A family path plus `--help` describes the family and lists its commands so a reader can
  descend without executing anything.

## Destructive and interactive commands

- A command that closes, deletes, or otherwise discards user work requires an explicit `--confirm`
  option; its absence is a usage failure, and its presence on a non-destructive command is also a
  usage failure.
- A release build may instead guard a destructive command by a typed confirmation when its owning
  guide declares it. The command then runs only when standard input and standard output are both
  interactive consoles and the person types the target's exact name at a prompt on standard error.
  It refuses `--confirm` and every non-interactive invocation, Agents and scripts included, with a
  localized `denied` failure that names the command to run by hand. Development and test builds
  keep `--confirm`. The guard slows unintended Agent actions and is not a security boundary.
  Orchbee's Windows release `server stop`, `session stop`, `session delete`, and `uninstall`, whose
  target name is `orchbee`, are guarded; their destructive admission in
  `apps/orchbee/src/cli/destructive_request.rs` enforces it.
- Commands never prompt on standard input or read the terminal. The exceptions are the release
  confirmation above, a command whose grammar authority declares it interactive setup, such as
  installation or trust review, and a sign-in question its owning guide declares. Interactive setup
  must accept `--yes` for non-interactive use and must refuse, not guess, when `--yes` is absent and
  no interactive terminal exists. A sign-in question is asked only when standard input and standard
  error are both interactive consoles, reads a secret without echo, ends the attempt definitely on
  an empty answer or an interrupt and restores the console, and without two consoles is never asked,
  so the command fails as it would with nobody to answer; Orchbee's `machine connect` is the one
  such command.
- A mutation that may take longer than its deadline declares `--detach`; the invocation then
  returns the request identity for a later `get` or `wait` instead of blocking.

## Agent discoverability

- The installed binary is the authority for its own grammar. Every command path present in the
  build is reachable through `--help` descent, `completion`, and the machine schema listing.
- An executable that ships an embedded Agent Skill prints it through one offline root option and
  the Skill describes the same build it is embedded in.
- Help for a family includes a bounded footer pointing to the embedded Skill and command help;
  it never depends on a checkout path or instructs an Agent to run a bare executable for discovery
  when the bare executable launches an interface.
- Nothing about discovery depends on a running service.

## Environment variables

| Rule                  | Contract                                                                                                                                                                                                                                               |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Namespace             | Except for the explicit platform inputs below, every variable the executable reads or injects is named `ORCHBEE_*` and listed once, with its level and meaning, in the [environment variable table](../../apps/orchbee/docs/environment-variables.md). |
| Injected              | A variable injected into managed processes by an Orchbee runtime, such as a launcher path or runtime directory, is authoritative over a caller-provided conflict.                                                                                      |
| Locale                | `LC_ALL`, `LC_MESSAGES`, and `LANG` select the diagnostic locale according to [Rust](../RUST.md#localized-product-copy); no `ORCHBEE_*` variable overrides them.                                                                                       |
| Nothing else          | No undocumented variable, current-directory content, or repository file influences an invocation.                                                                                                                                                      |
| User-directory inputs | A command may declare platform home-directory variables solely for an explicit user-path operand. The owning guide names their readers, precedence, validation and failure behavior. They never select a runtime or introduce shell expansion.         |
