# Release Records

Private, experimental Rust library for the closed JSON records of Orchbee's native release
distribution. The Client's install and update owners and the Windows launcher read the same
grammar from here, so the store pointer has one parser.

## Contract

- `parse_object`, `closed_object` and `check_extensions` read one strict JSON object: no repeated
  member at any depth, at most 32 nested arrays and objects, a closed member set, and an optional
  `extensions` object as the only open slot. Callers own every value rule.
- `valid_version` admits a SemVer 2.0.0 version of at most 64 bytes without build metadata, which
  is also a safe store directory name; `lowercase_sha256` admits a digest in 64 lowercase
  hexadecimal digits.
- `Pointer` is the store's `active.json`: format `1`, the `active` entry, the optional `previous`
  rollback target, and `extensions`, which every rewrite keeps. Bytes that are not such a record
  parse to `None`, which means no active version.

The record layouts are owned by
[Release distribution](../../proposals/platform/release-distribution.md#runtime-store). This crate
reads no file, takes no lock and starts no process.

## Verification

Run from the repository root:

```sh
cargo clippy -p orchbee-release-records --all-targets --locked -- -D warnings
cargo test -p orchbee --bins --locked update::tests
```

The Client's release feed suite exercises manifest, signature and pointer admission through these
functions.
