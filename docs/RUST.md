# Rust

## Scope and authority

This standard governs authored Rust source, Cargo manifests, and the repository Cargo workspace.
Application placement remains authoritative in [Applications](APPLICATIONS.md), semantic ownership
and visibility begin with [Module Boundaries](MODULE-BOUNDARIES.md), and repository execution
remains authoritative in [Verification](VERIFICATION.md) and [Testing](TESTING.md).

The root `rust-toolchain.toml` pins the exact compiler, Cargo, rustfmt, and Clippy cohort. The root
`Cargo.toml` is the only Cargo workspace root. A Rust project never selects its own toolchain or
creates a nested workspace.

## Placement and workspace ownership

| Artifact                         | Placement and contract                                                                                                                                        |
| -------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Executable application           | `apps/<name>` with a direct `Cargo.toml` and `project.json`; list it explicitly in root `workspace.members`.                                                  |
| Reusable Rust library            | `crates/<name>` with a direct `Cargo.toml` and `project.json`; admit it only for at least two real consumers or one deliberately independent stable contract. |
| Application-private code         | Keep it as modules inside the owning application crate; file count, test access, and possible future reuse do not justify a shared crate.                     |
| JavaScript or TypeScript library | Keep it under `packages/*` and follow [Packages](PACKAGES.md); do not place a Cargo crate there merely to reuse the npm namespace.                            |

Cargo package names use kebab-case. An executable name matches its installed process name. An
internal library under `crates/<name>` uses `orchbee-<name>` unless the directory is already named
`orchbee`. Every repository-owned crate sets `publish = false` until an accepted release design
explicitly makes it public.

`project.json.runtime` describes where emitted code runs, not its implementation language. A Rust
executable therefore uses `native`; `Cargo.toml` is the language authority. Do not add a duplicate
language field to `project.json`.

Application crates may depend on `crates/*` through workspace dependencies. A crate never depends
on an application crate, and one application never imports another application's source or Cargo
target. Shared behavior moves only after it has a stable semantic owner and real consumers.

## Manifests, dependencies, and generated state

- Every member inherits `edition`, `rust-version`, and workspace lints from the root manifest.
- Commit the one root `Cargo.lock` and use `--locked` in required verification and release
  commands. Nested lockfiles are forbidden.
- Declare a dependency in `[workspace.dependencies]` when more than one member intentionally shares
  its version or features. A member still opts into that dependency explicitly with `workspace = true`.
- Registry dependencies are resolved by the committed lockfile. Git dependencies pin an immutable
  revision. Path dependencies stay inside this repository; machine-local paths and sibling
  checkouts are forbidden.
- Keep default features only when their complete behavior belongs in the product. Declare the
  features actually used, and treat feature expansion as a dependency and binary-size change.
- A `build.rs` requires a concrete build-time responsibility. It writes only to Cargo's `OUT_DIR`,
  declares every rerun input, performs no network access, and never edits tracked source.
- Keep `target/` and generated build output untracked. Generated protocol bindings identify their
  source and are checked for staleness by their owning contract.

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

## Lint policy and enforcement

Workspace policy uses `unsafe_code = "forbid"` for authored code. A local `allow` or `expect`
cannot weaken it. If an unavoidable operating-system or foreign-code boundary cannot be expressed
safely, it requires a separately accepted policy change that narrows the exception to one adapter,
documents every safety invariant, and adds boundary tests. This policy does not audit unsafe code
inside third-party dependencies; dependency and feature review remain necessary.

`clippy::all` is the default lint group, not every available Clippy check. Select additional checks
for demonstrated risks and distinguish production invariants from deliberate invalid test inputs.
Do not enable an entire optional group merely to maximize lint count. Each enforcement change
updates its authority, configuration, diagnostic when owned, and meaningful positive and negative
regression evidence together under the pinned toolchain.

New or changed local lint exceptions name the specific lint, use the narrowest applicable scope,
and give a `reason` describing the invariant or tradeoff. Prefer `#[expect(...)]` when the warning
is reliably present so an obsolete exception is diagnosed. Explain configuration-dependent `allow`
uses; do not suppress a whole module's correctness checks or bypass a repository hard constraint.

| Concern                          | Evidence and limits                                                                                                                                                                                      |
| -------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Toolchain and Cargo execution    | The root runner checks the compiler pin and locked Cargo metadata, then invokes the selected workspace task.                                                                                             |
| Compiler and lint diagnostics    | Members inherit workspace lints; rustfmt checks formatting and Clippy denies enabled warnings. This does not prove all source-policy obligations.                                                        |
| Manifest and architecture policy | Review member registration, lint inheritance, dependency direction, version and feature sharing, nested workspaces, and path boundaries. Cargo metadata alone does not check these repository contracts. |
| Semantic safety and test value   | Review checked construction, invariant proofs, cancellation, shutdown, and claim-mapped evidence; compilation and a test-file count cannot judge them.                                                   |

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

## Testing

Follow the shared [test obligations](TESTING.md#required-unit-tests) and
[test layers and cost](TESTING.md#test-layers-and-cost). One behavior suite may cover several Rust
modules; neither a new source file nor a new function requires a companion test file.

- Put unit-test bodies in an adjacent test module file or in the crate's `tests/` tree. A production
  module may contain the narrow `#[cfg(test)] mod tests;` declaration needed to attach that file,
  but not inline scenarios or fixtures.
- Test public behavior, invalid and maximum inputs, cleanup after partial failure, cancellation,
  stale identity, backpressure, and deterministic shutdown. Use property or model-based evidence
  when ordering or state-space risk justifies it.
- Integration tests use public crate or process boundaries. Test-only visibility must not widen the
  production API or introduce product behavior.
- Rust documentation examples that claim to compile or run participate in doctests. Do not use
  `ignore` to hide a broken example; a `compile_fail` or `no_run` example states its actual claim.
- Cross-compilation proves only compilation. Runtime, IPC, PTY, installer, and service-lifecycle
  claims require a native runner on every claimed operating-system family.

## Required commands and release input

The root pnpm gate invokes `scripts/run-rust-workspace.mjs` for every Cargo member. The test task
runs target tests and then library documentation tests separately; `--all-targets` does not include
doctests. `pnpm test:rust-runner` exercises that boundary and the authored unsafe-code policy with
isolated Cargo fixtures, including failures that must make the runner fail.

| Repository task     | Cargo evidence                                                                                                                     |
| ------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `pnpm format:check` | `cargo fmt --all -- --check`                                                                                                       |
| `pnpm lint`         | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings`                                                    |
| `pnpm build`        | `cargo build --workspace --all-targets --all-features --locked`                                                                    |
| `pnpm test`         | `cargo test --workspace --all-targets --all-features --locked`, followed by `cargo test --workspace --doc --all-features --locked` |

`--all-features` does not prove every supported feature combination or operating-system target.
When introducing a feature or expanding support, the owner declares the supported default,
minimal, and release configurations and the compilation and native runtime evidence they need.
A workspace build's unified dependency features do not prove that an independently shipped package
builds with its own declared configuration.

Focused iteration may use a package-selected Cargo command, but completion uses the root gate.
Release automation builds an exact package and binary with the pinned toolchain, `--locked`, and
`--release`, then stages the verified artifact into the owning distribution cohort. It never treats
an arbitrary file already present under `target/` as a release input. Supported targets, signing,
install paths, update cohorts, and rollback remain owned by each accepted application release slice.
