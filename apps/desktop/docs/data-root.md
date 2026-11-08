# Desktop Data Root

## Authority and scope

This reference owns the Desktop product-data root and distribution-profile isolation. The data root
scopes Electron `userData`, the single-instance lock, the window-state record, and, outside a
release, the configuration file under which Desktop runs `orchbee`. It does not define Session
state, which the Server owns, user-selectable storage locations, migration between profiles or from
earlier Electron-managed locations, or automatic deletion.

## Resolution

| Distribution profile | Data root                       |
| -------------------- | ------------------------------- |
| Dev                  | `<home>/.ob-dev-<scope-digest>` |
| Test                 | `<home>/.ob-test-<home-digest>` |
| Release              | `<home>/.orchbee`               |

Dev resolves the canonical source-checkout root before hashing, whether launched live by
`pnpm desktop` or from `pnpm desktop:package:dev`. The scope digest is the first 16 lowercase
hexadecimal characters of SHA-256 over a version-tagged identity containing the platform-normalized
real path. The version tag keeps a future identity migration explicit without making process CWD or
a nested package path part of the identity. The live development launcher supplies the checkout
root explicitly; the packaged Dev descriptor records it and startup re-resolves the same canonical
path before accepting the scope.

Every development root contains one bounded scope marker with the complete canonical checkout path
and digest version. Startup accepts an existing root only when the marker agrees. A mismatch fails
closed instead of opening another checkout's state. A moved checkout receives a new root; the old
root remains untouched.

## Startup order and ownership

Desktop Main resolves the explicit distribution profile and creates its data root before requesting
Electron's single-instance lock. Live development accepts only the launcher-provided source root;
packaged applications accept only the strict descriptor inside `app.asar`, and process environment
cannot override it. Main sets Electron `userData` to the resolved root first, so one profile scope
controls both disk state and lock identity. The same checkout therefore admits one Dev instance,
while Dev, Test, Release, and different development checkouts may run concurrently.

Outside a release on Windows, the bundled Client is compiled with the profile's artifact stem and
derives `<data-root>/config.toml` itself, so Main passes its environment through unchanged and
creates no file. On macOS and Linux, which keep the release names until ORC-328, Main runs `orchbee`
with `ORCHBEE_CONFIG_FILE` set to `<data-root>/config.toml`, so a Session it starts keeps this data
root's preferences rather than the installed release's. The runtime refuses an explicit
configuration file that does not exist, so Main creates an empty, owner-only file when none is
present and leaves an existing one as it is. A release, or an `ORCHBEE_CONFIG_FILE` already in
Main's environment, runs `orchbee` with the environment unchanged.

On POSIX, Main creates product-owned data directories with owner-only access. The renderer receives
no data-root path, enumeration, location override, or generic filesystem capability. Tests use an
explicitly injected temporary home and source root.

## Invariants

- Dev, Test, and Release roots are pairwise distinct.
- Packaged profile identity and data scope come only from the strict embedded descriptor.
- Equal canonical development checkout paths resolve to equal roots.
- An accepted development root marker names exactly the path used to derive its digest.
- Different paths may not share one accepted root, including after a truncated-digest collision.
- Changing or moving a checkout never deletes or migrates the prior root.
- Desktop never replaces an existing configuration file.

## Verification

`prepareDesktopDataRoot` tests cover the fixed Release root, the stable development root,
different-checkout isolation, and marker creation, reuse, and mismatch.
`resolveDesktopApplicationDistribution` tests cover live development from the canonical checkout,
strict packaged descriptors, revalidation of a packaged Dev descriptor against the current checkout,
and environment non-override. No test drives the Main entry, which selects `userData` before
requesting the single-instance lock and, outside a release on macOS and Linux, creates the
configuration file.
