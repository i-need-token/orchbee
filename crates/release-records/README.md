# Release Records

Private, experimental Rust library for the closed JSON records of Orchbee's native release
distribution. The Client's install and update owners, the local runtime's store reader, which
Session start uses, and the Windows launcher read the same grammar from here, so the store pointer
has one parser.

## Contract

- `parse_object`, `closed_object` and `check_extensions` read one strict JSON object: no repeated
  member at any depth, at most 32 nested arrays and objects, a closed member set, and an optional
  `extensions` object as the only open slot. Callers own every value rule.
- `valid_version` admits a SemVer 2.0.0 version of at most 64 bytes without build metadata, which
  is also a safe store directory name; `lowercase_sha256` admits a digest in 64 lowercase
  hexadecimal digits.
- `Pointer` is the store's `active.json`: format `1`, the `active` entry, the optional `previous`
  rollback target, and `extensions`, which every rewrite keeps. Its `noPath` member is `true` after an install with `--no-path`, and `Pointer::no_path` reads only that value. Bytes that are not such a record
  parse to `None`, which means no active version.
- `Fallback` is the pointer's `extensions.fallback` member: the held versions, at most eight, and
  the `Move` that made an entry active, with `back`, `restore` and the bounded Client and Session
  windows. Every object is closed and every value bounded; a member of any other shape reads as
  absent. `counting_move` returns the move only while its entry is the active one. This crate owns
  the format alone; the local runtime's
  [release store](../local-runtime/README.md#fallback-after-a-failed-move) owns its transitions.
- `client_failure_helper` names the previous version's Client that records a failed start or join
  of a store Client, from that Client's path and the pointer read again, only while the failed
  version is active through a counting move whose `back` is true. The Windows launcher and a macOS
  TUI that no launcher started share it; the npm launcher keeps its own copy of the rule.

The record layouts are owned by
[Release distribution](../../proposals/platform/release-distribution.md#runtime-store). This crate
reads no file, takes no lock and starts no process.

## Verification

Run from the repository root:

```sh
cargo clippy -p orchbee-release-records --all-targets --locked -- -D warnings
cargo test -p orchbee-release-records --locked
cargo test -p orchbee --bins --locked update::tests
```

The Client's release feed suite exercises manifest, signature and pointer admission through these
functions.
