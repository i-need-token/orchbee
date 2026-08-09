# Zig

## Scope and authority

This standard governs authored Zig source and build code across the repository. Application shape, toolchain pins, dependencies, and scripts remain authoritative in the [application standard](APPLICATIONS.md); repository-wide test execution remains authoritative in the [testing standard](TESTING.md).

Format every Zig source and build file with the workspace-pinned `zig fmt`. The repository does not require a separate Zig linter.

## Naming and modules

- Use `TitleCase` for types, `camelCase` for callable values, and `snake_case` for other declarations and namespace-module filenames. A file whose root represents one named type may use `TitleCase.zig`.
- Treat `pub` as an architectural boundary, not a convenience. Export only what an owning entry point or consumer needs, and keep composition in the owning root.
- Derive module responsibility and dependency direction from [semantic module design](MODULE-BOUNDARIES.md#semantic-module-design); Zig namespace-module filenames describe the owned domain responsibility rather than repeat their fully qualified namespace.
- Comments explain ownership, invariants, external constraints, and non-obvious decisions; names and structure explain ordinary control flow.

## Resources and failures

- Every allocation, handle, thread, process, and operating-system resource has one explicit owner. A type that retains owned resources provides `deinit` unless it transfers ownership through its API.
- Accept an allocator from the owning boundary when allocation is required. Do not introduce hidden process-global allocators.
- Place `defer` immediately after successful acquisition and use `errdefer` to roll back partially constructed state.
- Return expected input, operating-system, and resource failures. Use `catch unreachable`, `.?`, or `unreachable` only when a local invariant proves the failure impossible; never use them to dismiss external input or recoverable runtime state.

## Boundaries and safety

- Validate lengths, ranges, tags, and identifiers received from CLI, environment, IPC, PTY, file, or foreign-code boundaries before allocating, indexing, or casting.
- Use fixed-width integers for wire, ABI, and persisted layouts; use `usize` for in-memory lengths and indexes. Make narrowing and signedness conversions explicit and checked.
- Keep `undefined`, raw pointers, pointer casts, volatile access, and platform-specific imports inside the smallest owning adapter. Use `undefined` only when every byte is initialized before any read, an external ABI requires output storage, or an owned value is deliberately poisoned at the end of its lifetime.
- Keep runtime safety enabled in product code. A scoped `@setRuntimeSafety(false)` requires measured evidence, documented invariants, and explicit human review.
- Assert size, alignment, and field offsets at compile time when a C ABI or wire contract depends on exact layout.

## Concurrency and bounded work

- Give mutable state a clear thread or lock owner, and state which invariant each synchronization primitive protects.
- Keep critical sections bounded. Do not hold a lock while waiting on IPC, PTY, process, or thread completion, or while invoking an external callback.
- Bound user-controlled buffers, queues, and per-dispatch work. Define an observable error, backpressure, or truncation outcome when a limit is reached.
- Make shutdown ownership explicit: stop new work, wake blocked workers, join owned threads or processes, and release resources exactly once.

## Testing

Zig permits `test` declarations in either the source under test or a separate Zig source file. This repository keeps production and test review surfaces separate:

- Put every Zig `test` declaration, executable test helper, fixture, and test root under the owning source tree's `__tests__` directory. Production `.zig` files contain no test declarations, fixtures, or test scenarios; only the narrow conditional access seam described below may remain there.
- Name a module companion `<source-stem>_test.zig`. Name cross-module conformance, shared-fixture, and system-harness roots by their owned responsibility with the same `_test.zig` suffix. The reserved test directory is spelled exactly `__tests__`.
- Prefer public behavior over private implementation access. When a white-box invariant cannot be tested through the production boundary, expose only the required declarations through a narrow `pub const testing` namespace; it must not own mutable state or change product behavior.
- Wire every test root into the owning build graph and non-interactive test script. A test is not part of the repository gate merely because its file exists.
- Use `std.testing.allocator` for allocating tests, and exercise cleanup on error when ownership logic is non-trivial.
- Cover invalid input, configured limits, boundary values, and expected failure paths in addition to the happy path. Native behavior and cross-compilation evidence follow the [native application test contract](TESTING.md#native-application-tests).
