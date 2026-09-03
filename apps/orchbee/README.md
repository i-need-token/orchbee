# Orchbee Native Entry

Experimental, unshipped Rust process entry for the accepted Phase 3A native-command foundation.
The emitted executable is named `orchbee`. It participates in the root Cargo verification gate but
is not consumed by Desktop, packaging, an installer, or a package manager.

Simplified Chinese maintenance translation:
[README.zh-CN.md](README.zh-CN.md).
English remains canonical.

## Command surface

The exact development surface is:

```text
orchbee
orchbee --help
orchbee --version
```

Bare invocation selects the future TUI route and currently reports a localized
development-unavailable result. It does not enter terminal mode or render a placeholder interface.
`--help` and `--version` complete without TUI or backend initialization. Every other, additional,
or non-Unicode argument is a usage failure. There are no short flags, aliases, abbreviations,
business commands, target discovery, current-directory inference, or compatibility behavior.

Help writes only stdout. The unavailable and usage results write only stderr. Version writes one
locale-neutral `orchbee <version>` line to stdout. Development exit statuses are `0` for help and
version, `1` for the unavailable bare TUI route, and `2` for usage failure. They are not the future
stable public exit table.

## Localization and authority

Authored help and diagnostics ship in `en-US` and `zh-CN`. Locale selection checks nonempty
`LC_ALL`, `LC_MESSAGES`, then `LANG`; a `zh` language prefix selects `zh-CN`, and every other value
falls back to canonical `en-US`. Locale data affects presentation only and is never persisted or
used as authority.

Production code uses only Rust standard-library argument, environment, stdout, and stderr
capabilities. It opens no file or socket, starts no process or thread, initializes no terminal
mode, and reads no product state. The delivered TypeScript [`apps/cli`](../cli/README.md) remains
the current CLI authority until a separately accepted packaged cutover.

## Measurement

Collect the reference release baseline from the repository root:

```sh
node apps/orchbee/scripts/measure-release-baseline.mjs
```

The script builds the exact locked release package, warms and repeatedly invokes
`orchbee --version`, reports artifact bytes and warm latency percentiles, and reports peak resident
memory where `/usr/bin/time` exposes it. Results describe only the printed host, toolchain, profile,
and invocation. They are not acceptance budgets, cold-cache guarantees, or evidence for future TUI
performance.

The Phase 3A reference run on 2026-09-02 used Apple M1 arm64, Darwin 23.6.0, Rust 1.97.0, the
default Cargo release profile, 5 warmups, and 50 measured invocations:

| Measurement          | Reference observation |
| -------------------- | --------------------- |
| Executable size      | 432,144 bytes         |
| Warm startup minimum | 1.179 ms              |
| Warm startup p50     | 1.466 ms              |
| Warm startup p95     | 2.921 ms              |
| Warm startup maximum | 4.046 ms              |
| Peak resident memory | 1,343,488 bytes       |

Rerun and replace this reference after a later accepted slice changes the executable's dependency
graph or startup path. Supported-platform acceptance budgets remain reserved.

## Verification

Run from the repository root:

```sh
cargo test -p orchbee --all-targets --all-features --locked
cargo clippy -p orchbee --all-targets --all-features --locked -- -D warnings
pnpm verify
```
