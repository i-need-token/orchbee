# Rust

## Scope and authority

This standard governs authored Rust source, Cargo manifests, and the repository Cargo workspace.
Application placement remains authoritative in [Applications](APPLICATIONS.md), semantic ownership
and visibility begin with [Module Boundaries](MODULE-BOUNDARIES.md), and repository execution
remains authoritative in [Verification](VERIFICATION.md) and [Testing](TESTING.md). The
command-line behavior of a Rust executable follows the `cli/` standards starting at
[CLI Command Surface](cli/command-surface.md); its full-screen behavior follows the `tui/` standards.

The root `rust-toolchain.toml` pins the exact compiler, Cargo, rustfmt, and Clippy cohort. The root
`Cargo.toml` is the only Cargo workspace root. A Rust project never selects its own toolchain or
creates a nested workspace.

The root development profile uses `debug = "line-tables-only"`; the test profile inherits it.
Retain source filenames and line numbers in real panic backtrace frames when changing this setting.
Keep explicitly owned per-package optimization overrides.

## Placement and workspace ownership

| Artifact                         | Placement and contract                                                                                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Executable application           | `apps/<name>` with a direct `Cargo.toml` and `project.json`; list it explicitly in root `workspace.members`.                                                  |
| Reusable Rust library            | `crates/<name>` with a direct `Cargo.toml` and `project.json`; admit it only for at least two real consumers or one deliberately independent stable contract. |
| Application-private code         | Keep it in the owning application; an internal crate at `apps/<app>/crates/<seam>` requires a cohesive boundary and measured incremental compilation benefit. |
| JavaScript or TypeScript library | Keep it under `packages/*` and follow [Packages](PACKAGES.md); do not place a Cargo crate there merely to reuse the npm namespace.                            |

Cargo package names use kebab-case. An executable name matches its installed process name. An
internal library under `crates/<name>` uses `orchbee-<name>` unless the directory is already named
`orchbee`. Every repository-owned crate sets `publish = false` until an accepted release design
explicitly makes it public.

Application-internal libraries use `<app>-<seam>`, set `publish = false`, and participate in the
root workspace through `apps/*/crates/*`. They are not shared crates and do not use the `crates/*`
consumer-admission rule. Record the owning application in their `project.json` responsibility;
keep the required crate shape below. Only that application and its sibling internal crates may
consume them, through explicit Cargo dependencies and narrow re-exports. Internal crates may
depend on shared libraries and sibling internal crates, never on the application target; the
dependency graph stays acyclic across normal, development, build, optional, and target-specific
dependencies. Keep invocation parsing and process composition in the application.

Before retaining an extraction, compare real source-edit incremental checks and test compilation
on the same machine with the same commands, profile, and warmed dependency graph. Record the
changed inputs, elapsed times, accepted benefit/regression bounds, and retained or reverted result
in the existing review. A mechanism pilot may be neutral; file count, test access, and possible
future reuse alone do not justify a crate. Measure later extractions independently.

`project.json.runtime` describes where emitted code runs, not its implementation language. A Rust
executable therefore uses `native`; `Cargo.toml` is the language authority. Do not add a duplicate
language field to `project.json`.

Application crates may depend on `crates/*` and their own internal libraries through Cargo
dependencies. Shared crates never depend on applications or application-internal crates, and one
application never imports another application's source or Cargo target. Shared behavior moves only
after it has a stable semantic owner and real consumers.

## Required crate shape

Every Cargo member contains these files:

| Path                          | Responsibility                                                                     |
| ----------------------------- | ---------------------------------------------------------------------------------- |
| `Cargo.toml`                  | Identity, targets, features, and dependencies; inherits the root workspace package |
| `project.json`                | Machine-readable scope and responsibility                                          |
| `README.md`                   | Consumer-facing responsibility, public boundary, and maintenance commands          |
| `src/lib.rs` or `src/main.rs` | The one crate root; a library root is an explicit re-export allowlist              |
| `tests/`                      | Integration and conformance tests, present when the crate claims a public boundary |

An application crate may add a `src/lib.rs` beside `src/main.rs` only so that its own `tests/`
tree can reach the binary's domain operations through one reviewed boundary. That library target
is not a public API: no other workspace member depends on it, its `pub` surface is limited to
what the application's tests and binary consume, and it never becomes a reason to skip admitting a
real `crates/*` library when a second consumer appears.

## Manifests, dependencies, and generated state

- Every member inherits `edition`, `rust-version`, and workspace lints from the root manifest.
- Commit the one root `Cargo.lock` and use `--locked` in required verification and release
  commands. Nested lockfiles are forbidden.
- Declare every third-party dependency used by at least two distinct members in
  `[workspace.dependencies]`, with its version and base features. Members inherit it with
  `workspace = true` and may append features. Count normal, development, build, and target-specific
  declarations by package identity, including renamed dependencies and inactive targets.
- Write ordinary version requirements, such as `"1.0.151"`; use an exact `=` requirement only with
  an adjacent comment explaining why. Registry dependencies are resolved by the committed lockfile.
  Git dependencies pin an immutable
  revision. Path dependencies stay inside this repository; machine-local paths and sibling
  checkouts are forbidden.
- Every third-party declaration sets `default-features = false`, directly or through workspace
  inheritance. List required features explicitly, including any required default behavior such as
  regex Unicode support, and treat feature expansion as a dependency and binary-size change.
- A `build.rs` requires a concrete build-time responsibility. It writes only to Cargo's `OUT_DIR`,
  declares every rerun input, performs no network access, and never edits tracked source.
- Keep `target/` and generated build output untracked. Generated protocol bindings identify their
  source and are checked for staleness by their owning contract.

`pnpm lint:rust` enforces internal-crate identity, publishing restrictions, and application ownership,
as well as shared third-party workspace inheritance and disabled default features before Clippy.
The ownership check uses resolved package IDs from locked, all-feature Cargo metadata, including
patched dependencies and nonmember consumers; it rejects internal dependency cycles explicitly.
The runner reuses locked Cargo metadata for package identity, dependency kind,
target, and effective features, and reads manifest keys to verify inheritance and unused workspace
declarations. Repository path dependencies and nonmember third-party manifests are outside the
third-party dependency checks. Version-requirement rationale and whether the selected features preserve behavior
remain review obligations. Real positive and negative Cargo fixtures live in
[`scripts/run-rust-workspace.test.mjs`](../scripts/run-rust-workspace.test.mjs).

## Dependency admission

The standard library is the default implementation. A third-party crate is admitted for one named
capability that `std` cannot provide correctly, such as OS bindings, serialization, terminal
control, or Unicode segmentation; convenience, familiarity, and shorter code are not capabilities.
Adding a dependency is a reserved decision under
[`DEVELOPMENT.md`](DEVELOPMENT.md#decision-authority).

| Concern            | Contract                                                                                                                                                                                    |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Runtime frameworks | The workspace uses no async runtime, argument-parsing framework, error-aggregation crate, or logging framework. Introducing one is Consequential work, not a per-crate choice.              |
| Features           | Declare `default-features = false` and list the features actually used; enabling a feature is reviewed as a dependency change.                                                              |
| Version unity      | Direct dependencies resolve to one version across the workspace; a change that adds a duplicate transitive version to `Cargo.lock` names it in review with the reason it cannot be unified. |
| Transitive review  | Review the added transitive tree, its `unsafe` footprint, build scripts, proc macros, maintenance status, and license before acceptance.                                                    |
| Development scope  | A crate used only by tests, conformance checks, or build tooling stays in `[dev-dependencies]` or `[build-dependencies]`.                                                                   |
| Platform scope     | A crate needed on one platform is declared under the matching `[target.'cfg(...)'.dependencies]` table.                                                                                     |
| Removal            | A dependency whose last consumer disappears is removed in the same change.                                                                                                                  |

## Modules and visibility

- Use `UpperCamelCase` for types and traits, `snake_case` for modules, functions, and values, and
  `SCREAMING_SNAKE_CASE` for constants and statics.
- Treat `pub` as an architectural boundary. Choose the smallest scope: private, `pub(super)`, or
  `pub(in ...)` before `pub(crate)`. A crate-wide or public item needs a real consumer at that
  boundary. Prefer private child modules with explicit re-exports of the owning operations.
- Compiler visibility is necessary but does not authorize lateral dependencies. Keep the owning
  module contract and dependency direction explicit; test access never widens production visibility.
- Keep composition and process startup in the binary crate. Domain modules do not depend on CLI,
  TUI, transport, persistence, or service-lifecycle adapters.
- Do not create `util`, `common`, `helpers`, or `shared` catch-all modules. Name a module for its one
  owned responsibility and split it when ownership, invariants, or dependency direction diverge.
- Avoid glob imports and glob re-exports in production code. They hide the reviewed public surface
  and make cross-module ownership changes non-local.
- A module with children is the file `name.rs` beside the directory `name/`; `mod.rs` is not used
  under `src/`. Under `tests/`, shared helpers live in `tests/support/mod.rs` so Cargo does not
  compile them as their own test crate.
- The parent file owns the `mod` declarations, visibility, and re-exports of its children; a child
  never declares a sibling.

The Rust lint runner checks every workspace member's `src/` tree for `mod.rs` before invoking
Clippy, including files inactive on the current platform. This filename check leaves integration
`tests/support/mod.rs` and nonmember dependency trees outside its scope; compiler checks and
semantic review still own module contents and boundaries. The
[runner fixtures](../scripts/run-rust-workspace.test.mjs) exercise both rejected and admitted layouts.

A member that states its [lateral-import contract](MODULE-BOUNDARIES.md#native-source-trees) as
layers keeps it in `docs/module-layers.md`. The first table under `## Layers` lists layer IDs from
lowest to highest, or the section links to another member's contract to reuse its table. Tables
under `## Module assignment` map module paths to layers; a descendant inherits the nearest assigned
ancestor's layer. `pnpm lint:rust-modules` reads the file and reports cycles and upward references
without failing ([Linting](LINTING.md#enforcement-boundary)); its
[fixtures](../scripts/rust-module-graph.test.mjs) exercise graph extraction and contract parsing.

## Platform-specific code

- Operating-system APIs and behavior live in adapter modules whose path names the platform or the
  capability, such as `linux_permissions` or `connection/native`, or in the composition root.
  Deterministic domain, state-owner, and protocol modules contain no `cfg(unix)`,
  `cfg(windows)`, or `cfg(target_os = ...)`.
- Gate with `#[cfg(...)]` on the item: the `mod` declaration, import, field, function, `impl`, or
  match arm. Code for another platform must not compile into this platform's build. Use `cfg!(...)`
  only for a policy constant whose every branch compiles on every target.
- Choose the widest correct predicate. `cfg(unix)` covers POSIX behavior; narrow to
  `cfg(target_os = "linux")` only for an API that exists on one operating system, and name that API
  in a comment when the reason is not visible in the call.
- An unsupported platform fails through the same typed outcome as any other rejection. A
  `#[cfg(not(unix))]` stub exists only at the adapter boundary, returns the typed
  unsupported-platform outcome, and never panics, silently degrades, or fabricates a result.
- Tests carry the same predicate as the behavior they exercise. Platform behavior is proven on a
  native runner for that platform according to [Testing](TESTING.md#native-application-tests);
  compiling the gated code elsewhere is not evidence.

## Documentation comments

- Every crate root and every module file that owns a contract, state, or adapter begins with a
  `//!` comment stating its responsibility and non-goals in one paragraph. It does not restate the
  module name or list its children.
- Every `pub` and `pub(crate)` item at a crate or module boundary carries a `///` comment that
  states what the item guarantees. Add `# Errors` when the returned error has variants the caller
  must distinguish and `# Panics` when a proven invariant is asserted.
- Private items follow [Code Style](CODESTYLE.md#comments): a comment explains a constraint or a
  non-obvious decision, not the signature.
- Comments describe current behavior. Task history, staged-slice markers, and references to
  conversations do not belong in source.
- Doc examples that claim to compile participate in doctests under [Testing](#testing).
- Missing documentation is a review finding; `missing_docs` is not enabled, and enabling it is a
  lint policy change under [Lint policy and enforcement](#lint-policy-and-enforcement).

## Types and validated boundaries

Apply the shared [domain modeling rules](MODELING-AND-CORRECTNESS.md#domain-contract) using Rust's
native types; do not reproduce TypeScript file-count or syntax conventions.

- Use distinct newtypes for identities with different meanings. Keep fields private when arbitrary
  construction or mutation would violate an invariant, and expose checked constructors or `TryFrom`.
  Use `NonZero*` and domain enums where they represent the accepted value space directly.
- Model alternatives with enum variants carrying their required data. Do not encode a state machine
  as unrelated booleans and optional fields that admit impossible combinations.
- Validate untrusted CLI, environment, file, IPC, and foreign-code data at the owning boundary.
  Deserialization proves only the checks implemented by that deserializer. A derived `Deserialize`
  must not bypass the checked construction of a domain value; validate a wire representation or
  delegate custom deserialization to the same constructor before admitting it into domain logic.
- Required facts become nonoptional after admission. Distinguish absence, rejection, unavailability,
  and an effect whose outcome is unknown; do not replace malformed input with a plausible default.
- APIs borrow data when they only inspect it and take ownership when they retain or consume it.
  Choose `Clone`, shared ownership, and interior mutability for their semantics and lifetime, not
  merely to silence a borrowing error. Cloning a handle does not grant another authoritative writer.
- Introduce traits and generic abstractions for an actual capability or substitutable behavior.
  A trait per struct, a shared crate per module, and a type-state encoding for every helper are not
  required. Keep the smallest design that expresses the accepted invariants.

## Numbers and layouts

- Use fixed-width integers for wire, ABI, and persisted values, and `usize` for in-memory lengths
  and indexes. Rust's default memory layout is not a serialization format.
- Prefer `From` for lossless conversion and `TryFrom` for fallible conversion. A numeric `as` cast
  requires a local proof that truncation, signedness changes, or precision loss cannot violate the
  contract. Do not cast an untrusted length or identifier into range.
- Check lengths, tags, ranges, and capacity arithmetic before allocation, indexing, or slicing.
  Use `checked_*` when overflow is a failure; use `saturating_*` or `wrapping_*` only when clamping
  or modular arithmetic is the specified behavior. Identity, generation, and sequence exhaustion
  have an explicit outcome instead of silently wrapping or reusing a value.
- Represent byte counts, Unicode scalar counts, and terminal display-cell widths as different
  quantities in names and operations; a byte boundary need not be a valid text boundary.
- When a foreign ABI requires an exact layout, isolate it in the owning adapter and verify size,
  alignment, and offsets. Serialize protocol fields explicitly according to their authority.

## Errors and panic boundaries

- Domain and protocol operations return matchable error variants for expected rejection and
  failure. Preserve machine-readable causes through adapter translation and error chaining;
  presentation text is not a discriminant and must not drive retry or recovery.
- Use `io::Error` at an I/O boundary when its semantics suffice. Type-erased errors may aggregate
  failures at a terminal application boundary that only reports or terminates; do not propagate
  them through an operation whose caller must distinguish domain outcomes.
- Do not use `unwrap`, `expect`, indexing that may panic, or deliberate panic for external input or
  recoverable runtime state. A locally proven invariant may justify an assertion or `expect`, with
  the proof visible in the type or nearby explanation. Test assertions may fail the test directly.
- A panic represents an internal invariant violation and must terminate only the owning failure
  domain promised by the process design. Define how thread or task panics reach their owner;
  neither catching a panic nor ignoring a join result establishes successful recovery.
- Never silently discard an operation or cleanup failure. An intentionally best-effort action
  needs an owned reason and observable failure handling where the contract requires it.
- A failure translated into an external classification keeps its operation, failing stage, and
  typed cause in the error value up to that translation. Internal detail leaves the process only
  through the owning component's documented diagnostic exit, rendered from static identifiers,
  numeric correlation, and error kinds — never record bytes, terminal content, credentials, paths,
  environment values, or nested error text. Product standard error and machine documents are not
  diagnostic exits.

## Resources and concurrency

- Give every file, socket, process, PTY attachment, task, channel, timer, and temporary artifact one
  lifetime owner. Use RAII guards for acquisition and partial-initialization rollback. Shared
  references or cloned handles do not remove the need for one shutdown owner.
- Use explicit close, shutdown, or join operations when cleanup can fail or must wait for work.
  Define `Drop` as the fallback appropriate to that owner; it must not panic or hide an unbounded
  blocking wait. Dropping a thread or task handle is not proof that its work has stopped.
- Shutdown stops admission, cancels or drains bounded work, joins owned work, and releases each
  resource exactly once. Cancellation of a local wait does not retract an already submitted
  external effect; preserve its identity and the contract's reconciliation or failure outcome.
- Bound input lengths, decoded collections, queues, retries, concurrency, and per-dispatch work.
  Every bound has an observable rejection, backpressure, truncation, or cancellation outcome.
- State which invariant each lock protects, the order for acquiring multiple locks, and the
  failure policy for a poisoned lock. Never hold a synchronous lock across `.await`, IPC,
  filesystem or process waits, callbacks, or blocking computation. Isolate unavoidable blocking
  work from the async executor; deadlines do not make blocking work asynchronous.

## Local process and IPC boundaries

These rules apply to every Rust executable that opens a local endpoint, connects to one, or
launches a sibling process.

| Concern         | Contract                                                                                                                                                                                                                                                                                                                                                                                                                          |
| --------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Process exit    | `main` returns `ExitCode` and maps one typed outcome to it. Library, domain, and adapter code never calls `process::exit`; a failure travels to `main` as a value so owners drop and restoration runs.                                                                                                                                                                                                                            |
| Peer identity   | A local endpoint verifies the connecting peer's operating-system credentials against the expected effective user before reading protocol bytes. Knowing an endpoint path grants nothing.                                                                                                                                                                                                                                          |
| Runtime paths   | Runtime directories and sockets derive from one explicitly configured base, never from the current directory, the repository, or `PATH`. Creation sets an explicit mode; traversal is no-follow and owner- and mode-checked.                                                                                                                                                                                                      |
| Path defects    | An unexpected owner, mode, symlink, or file type is a typed rejection reported to the caller. The process never repairs, removes, or replaces another owner's path to proceed.                                                                                                                                                                                                                                                    |
| Sibling launch  | A process that launches another repository executable resolves the exact expected artifact and verifies its identity before launch; it never runs whatever `PATH` or the current directory resolves.                                                                                                                                                                                                                              |
| Signals         | One owner installs signal handling and translates termination signals into one typed shutdown event. Handlers do no work beyond signaling; interruption exits follow the conventional `128 + signal` code.                                                                                                                                                                                                                        |
| Child processes | Every spawned child has one lifetime owner that reaps it. A deliberately retained child is an explicit, named ownership transfer with a documented survivor contract, not a forgotten handle.                                                                                                                                                                                                                                     |
| Environment     | Variables the process reads are named `ORCHBEE_*`, documented by the owning workspace, and validated at startup. Beyond them, only explicitly declared platform user-directory inputs for user-path operands, the locale variables in [Localized product copy](#localized-product-copy) and the terminal capability variables in [TUI Terminal Environment](tui/terminal-environment.md#capability-detection) influence behavior. |

## Lint policy and enforcement

Workspace policy uses `unsafe_code = "forbid"` for authored code. A local `allow` or `expect`
cannot weaken it. If an unavoidable operating-system or foreign-code boundary cannot be expressed
safely, it requires a separately accepted policy change that narrows the exception to one adapter,
documents every safety invariant, and adds boundary tests. This policy does not audit unsafe code
inside third-party dependencies; dependency and feature review remain necessary.

The accepted operating-system exceptions are limited to these private modules:

| Package and location                                            | Private module              | Owned safety contract                                                                                         |
| --------------------------------------------------------------- | --------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `orchbee-windows-console-host` at `crates/windows-console-host` | `src/sys.rs`                | [Console handle, count, Unicode, and tagged-union boundaries](../crates/windows-console-host/README.md)       |
| `orchbee-local-ipc` at `crates/local-ipc`                       | `src/windows_name_query.rs` | [Exact Win32 name query, immediate error capture, and search-handle ownership](../crates/local-ipc/README.md) |

These crates declare `unsafe_code = "deny"` instead of inheriting the workspace `forbid`;
only the listed private module has a reasoned `allow`. The Rust runner pins each package,
manifest, private module declaration, adjacent safe test attachment, and mirrored Clippy policy.
It rejects another member, location, module, or broader lint escape, including unsafe code in
tests or other children of the admitted module. All remaining sources and tests in these crates
stay denied; other members retain the workspace `forbid`. The linked package contracts own
each adapter's safety invariants and native boundary evidence.

`clippy::all` is the default lint group, not every available Clippy check. Select additional checks
for demonstrated risks and distinguish production invariants from deliberate invalid test inputs.
Do not enable an entire optional group merely to maximize lint count. Each enforcement change
updates its authority, configuration, diagnostic when owned, and meaningful positive and negative
regression evidence together under the pinned toolchain.

The workspace also denies `clippy::wildcard_imports` to keep ordinary production module imports
explicit. The real Cargo-runner fixture rejects a used module glob and accepts the same source with
an explicit import plus `use super::*` in its test module. This selected check is narrower than the
source policy: Clippy's default exemptions, including preludes, re-exports, macro expansions and
test contexts, remain; enum-variant globs use a separate lint. Review those surfaces rather than
claiming that every glob or public boundary is automatically checked.

New or changed local lint exceptions name the specific lint, use the narrowest applicable scope,
and give a `reason` describing the invariant or tradeoff. Prefer `#[expect(...)]` when the warning
is reliably present so an obsolete exception is diagnosed. Explain configuration-dependent `allow`
uses; do not suppress a whole module's correctness checks or bypass a repository hard constraint.
The `reason` requirement is a review obligation: `clippy::allow_attributes_without_reason` is not
enabled, and enabling it is a lint policy change that must fix every existing exception in the
same change.

| Concern                          | Evidence and limits                                                                                                                                                                                                                                                                                             |
| -------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Toolchain and Cargo execution    | The root runner checks the compiler pin and locked Cargo metadata, then invokes the selected workspace task.                                                                                                                                                                                                    |
| Compiler and lint diagnostics    | Members inherit workspace lints; rustfmt checks formatting and Clippy denies enabled warnings. This does not prove all source-policy obligations.                                                                                                                                                               |
| Manifest and architecture policy | The lint runner checks application-internal ownership, package identity and publishing, shared third-party workspace inheritance, and disabled default features. Review member registration, remaining dependency direction, exact-version rationale, feature behavior, nested workspaces, and path boundaries. |
| Semantic safety and test value   | Review checked construction, invariant proofs, cancellation, shutdown, and claim-mapped evidence; compilation and a test-file count cannot judge them.                                                                                                                                                          |

## Cross-language contracts

Rust, TypeScript, and Zig bindings never become competing authorities for one serialized contract.
Every accepted cross-language slice names:

- one semantic owner and one language-neutral schema or encoding specification;
- canonical valid, boundary, and invalid fixtures owned beside that authority;
- identifier widths, byte order, limits, version negotiation, and unknown-field behavior;
- which bindings are generated and which are independently implemented; and
- stale-generation checks plus differential or conformance tests for every implementation.

An existing language-owned protocol remains current authority until its accepted migration says
otherwise. A Rust implementation consumes that authority and its fixtures; it does not copy the
model and silently reinterpret it. Presentation text and localized diagnostics are not wire
contracts.

## Localized product copy

Native product copy — diagnostics, help, labels, and confirmations shown to a person — ships in
`en-US` and `zh-CN` like every other product message. Frontend catalogs follow
[Frontend Localization](frontend/localization.md); native executables follow these rules.

- One `localization` module per executable owns every human-facing string. Domain, protocol, and
  adapter modules return typed outcomes and never format prose.
- The supported locales are a closed enum. Every message is a `match` over that enum with no
  wildcard arm, so adding a locale fails to compile until every message is translated.
- Locale selection reads `LC_ALL`, `LC_MESSAGES`, and `LANG` in that order, ignores empty values,
  and falls back to `en-US` for an absent, non-Unicode, or unsupported value. Traditional Chinese
  variants resolve to `zh-CN`.
- Machine-readable output is never localized: wire fields, schema identifiers, enum values, exit
  codes, and identifiers stay identical under every locale. User-controlled text is carried as
  data, escaped for the surface, and never translated.
- Translations preserve commands, flags, paths, identifiers, and placeholders exactly and use the
  target language's punctuation. Tests render every message under both locales according to
  [Testing](TESTING.md#localization-contracts).

## Testing

Do not add Rust unit tests by default. Follow the shared
[admission and removal rule](TESTING.md#unit-test-admission-and-removal) and
[test layers and cost](TESTING.md#test-layers-and-cost). One behavior suite may cover several Rust
modules; neither a new source file nor a new function requires a companion test file. Simple
constructors, accessors, derived traits, and standard-library behavior need no dedicated tests
without a meaningful owned risk. Retain evidence for validation, ownership, failure, and protocol
contracts that compilation cannot establish.

- Put unit-test bodies in an adjacent test module file or in the crate's `tests/` tree. A production
  module may contain the narrow `#[cfg(test)] mod tests;` declaration needed to attach that file,
  but not inline scenarios or fixtures.
- The attached module is named `tests` and lives in `tests.rs` beside the module, or in a `tests/`
  directory whose `tests.rs` declares behavior-named children such as `mod backpressure;`. Do not
  attach differently named or abbreviated test modules to production files. Shared unit-test
  fixtures are children of that same `tests` owner, with only the visibility their test consumers need.
- When the owning application's tests also need an application-internal crate's fixtures, the crate
  exposes them from a `test_support` module beside the owning module, gated by
  `#[cfg(any(test, feature = "test-support"))]`. Only development dependencies within the owning
  application enable `test-support`, directly or through a sibling crate's own `test-support`
  feature; fixtures that only the crate's own unit tests use remain under `tests`.
  When such an application test asserts private crate state, the crate keeps the scenario body in
  `test_support` and the application test passes in its entry-layer driver.
- Crates with integration or conformance tests use `autotests = false` and one `integration`
  target at `tests/main.rs`. Each boundary or fixture family remains a child module under `tests/`;
  shared helpers live in `tests/support/mod.rs`. Do not create an empty target for a crate without
  these tests. Keep suites that require process isolation as explicit separate `[[test]]` targets
  and explain the protected signal, state, fixture-admission, or deadline boundary beside each target.
- Test public behavior, invalid and maximum inputs, cleanup after partial failure, cancellation,
  stale identity, backpressure, and deterministic shutdown. Use property or model-based evidence
  when ordering or state-space risk justifies it.
- Integration tests use public crate or process boundaries. Test-only visibility must not widen the
  production API or introduce product behavior.
- A [benchmark](TESTING.md#benchmarks) is an explicitly declared `[[bench]]` target with
  `harness = false`, timed with `std::time::Instant` and `std::hint::black_box`; a benchmark
  framework is a dependency that needs admission. A multi-file benchmark lives at
  `benches/<name>/main.rs` with its child modules beside it. The harness measures only when its
  arguments include `--bench`, which `cargo bench` passes; any other invocation, including
  `cargo test --all-targets`, runs every case once.
- Rust documentation examples that claim to compile or run participate in doctests. Do not use
  `ignore` to hide a broken example; a `compile_fail` or `no_run` example states its actual claim.
- Cross-compilation proves only compilation. Runtime, IPC, PTY, installer, and service-lifecycle
  claims require a native runner on every claimed operating-system family.

## Required commands and release input

The root pnpm gate invokes `scripts/run-rust-workspace.mjs` for every Cargo member. The test task
runs target tests and then library documentation tests separately; `--all-targets` does not include
doctests. `pnpm test:rust-runner` exercises that boundary, dependency inheritance and defaults,
source-module filenames, and the authored unsafe-code policy with isolated Cargo fixtures,
including failures that must make the runner fail.

| Repository task     | Cargo evidence                                                                                                                                  |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm format:check` | `cargo fmt --all -- --check`                                                                                                                    |
| `pnpm lint`         | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`, followed by `cargo clippy --workspace --locked -- -D warnings` |
| `pnpm build:full`   | Explicit host build: `cargo build --workspace --all-targets --all-features --locked`                                                            |
| `pnpm test`         | `cargo test --workspace --all-targets --all-features --locked`, followed by `cargo test --workspace --doc --all-features --locked`              |

Clippy and Cargo tests compile their own inputs; ordinary verification does not run a preceding
full Cargo build. The root test gate prepares only the host and peer executables required by
selected real-process witnesses. Build or cross-compile explicitly when the changed delivery or
platform contract needs that evidence, as governed by [Verification](VERIFICATION.md#merge-gate).

`--all-features` does not prove every supported feature combination or operating-system target.
When introducing a feature or expanding support, the owner declares the supported default,
minimal, and release configurations and the compilation and native runtime evidence they need.
A workspace build's unified dependency features do not prove that an independently shipped package
builds with its own declared configuration. The second lint pass compiles library and binary
targets with default features because development dependencies re-enable test-support features
under `--all-targets`.

Focused iteration may use a package-selected Cargo command, but completion uses the root gate.
Release automation builds an exact package and binary with the pinned toolchain, `--locked`, and
`--release`, then stages the verified artifact into the owning distribution cohort. It never treats
an arbitrary file already present under `target/` as a release input. Supported targets, signing,
install paths, update cohorts, and rollback remain owned by each accepted application release slice.
