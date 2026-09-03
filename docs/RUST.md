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
- The first real member creates the one root `Cargo.lock`; commit it and use `--locked` in required
  verification and release commands. Nested lockfiles are forbidden.
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
- Treat `pub` as an architectural boundary. Prefer private items and `pub(crate)`; expose a public
  item only through the owning crate boundary required by a real consumer.
- Keep composition and process startup in the binary crate. Domain modules do not depend on CLI,
  TUI, transport, persistence, or service-lifecycle adapters.
- Do not create `util`, `common`, `helpers`, or `shared` catch-all modules. Name a module for its one
  owned responsibility and split it when ownership, invariants, or dependency direction diverge.
- Avoid glob imports and glob re-exports in production code. They hide the reviewed public surface
  and make cross-module ownership changes non-local.

## Failures, resources, and concurrency

- Return typed errors for invalid input, protocol rejection, operating-system failure, unavailable
  services, cancellation, timeouts, and exhausted limits. Add context at adapter boundaries without
  converting stable machine-readable causes into presentation text.
- Do not use `unwrap`, `expect`, indexing that may panic, or deliberate panic for external input or
  recoverable runtime state. A panic represents an internal invariant violation and must terminate
  only the owning failure domain promised by the process design.
- Give every file, socket, process, PTY attachment, task, channel, timer, and temporary artifact one
  lifetime owner. Shutdown stops admission, cancels or drains bounded work, joins owned tasks, and
  releases each resource exactly once.
- Bound input lengths, decoded collections, queues, retries, concurrency, and per-dispatch work.
  Every bound has an observable rejection, backpressure, truncation, or cancellation outcome.
- Never hold a synchronous lock across `.await`, IPC, filesystem or process waits, callbacks, or
  blocking computation. Isolate unavoidable blocking work from the async executor.

Workspace policy denies authored `unsafe` code. If an unavoidable operating-system or foreign-code
boundary cannot be expressed safely, it requires a separately accepted change that narrows the
exception to one adapter, documents every safety invariant, and adds boundary-focused tests before
the workspace lint policy changes.

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

- Put unit-test bodies in an adjacent test module file or in the crate's `tests/` tree. A production
  module may contain the narrow `#[cfg(test)] mod tests;` declaration needed to attach that file,
  but not inline scenarios or fixtures.
- Test public behavior, invalid and maximum inputs, cleanup after partial failure, cancellation,
  stale identity, backpressure, and deterministic shutdown. Use property or model-based evidence
  when ordering or state-space risk justifies it.
- Integration tests use public crate or process boundaries. Test-only visibility must not widen the
  production API or introduce product behavior.
- Cross-compilation proves only compilation. Runtime, IPC, PTY, installer, and service-lifecycle
  claims require a native runner on every claimed operating-system family.

## Required commands and release input

The root pnpm gate invokes `scripts/run-rust-workspace.mjs` so one command continues to verify the
whole repository. While the accepted Cargo workspace has no members, the runner verifies the exact
toolchain and workspace metadata and reports the empty state. Once the first member is added, the
same commands run real workspace work:

| Repository task     | Cargo evidence                                                                  |
| ------------------- | ------------------------------------------------------------------------------- |
| `pnpm format:check` | `cargo fmt --all -- --check`                                                    |
| `pnpm lint`         | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` |
| `pnpm build`        | `cargo build --workspace --all-targets --all-features --locked`                 |
| `pnpm test`         | `cargo test --workspace --all-targets --all-features --locked`                  |

Focused iteration may use a package-selected Cargo command, but completion uses the root gate.
Release automation builds an exact package and binary with the pinned toolchain, `--locked`, and
`--release`, then stages the verified artifact into the owning distribution cohort. It never treats
an arbitrary file already present under `target/` as a release input. Supported targets, signing,
install paths, update cohorts, and rollback remain owned by each accepted application release slice.
