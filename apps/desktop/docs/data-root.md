# Desktop Data Root

## Authority and scope

This reference owns how Desktop resolves and prepares its data root and distribution-profile
isolation. The data root scopes Electron `userData`, the single-instance lock, the window-state and
notification-area records, and the root under which Desktop runs `orchbee`. The
[data root owner](../../../crates/local-runtime/README.md#data-root) defines the shared layout. This
reference does not define Session state, which the Server owns, migration between profiles or from
earlier locations, or automatic deletion.

## Resolution

| Distribution profile | Data root                       |
| -------------------- | ------------------------------- |
| Dev                  | `<home>/.ob-dev-<scope-digest>` |
| Test                 | `<home>/.ob-test-<home-digest>` |
| Release              | `<home>/.orchbee`               |

`ORCHBEE_HOME` in Main's environment replaces the profile's folder, as it does for the native
runtime; a relative value fails startup instead of falling back. Desktop keeps its own files under
`internal/`:

| Path                                 | Contents                                                        |
| ------------------------------------ | --------------------------------------------------------------- |
| `internal/desktop/`                  | Electron `userData`: browser storage, Chromium caches, and lock |
| `internal/state/desktop-window.json` | Window bounds and state, created by the first save              |
| `internal/state/desktop-tray.json`   | Whether the one-time notification-area notice has been shown    |
| `internal/development-checkout.json` | Development scope marker                                        |
| `internal/version.json`              | Layout record and the newest version that opened the root       |

On Windows Main appends `started`, `startup-failed`, `renderer-gone` with Electron's reason,
diagnostic bundle outcomes, and `exited` to the data root's
[daily diagnostic log](../../../crates/local-runtime/README.md#windows-diagnostic-log), creating
`logs/` with its first record. It writes the natives' records with role `desktop-main` under their
retention, cap, and exclusive-open rules; its event names and text values are string literals in
the source.

Electron 42 ignores `--disk-cache-dir`, and moving `sessionData` would move browser storage with
the caches, so Chromium's caches stay in `internal/desktop/` rather than `internal/cache/`.

Dev resolves the canonical source-checkout root before hashing, whether launched live by
`pnpm desktop` or from `pnpm desktop:package:dev`. The scope digest is the first 16 lowercase
hexadecimal characters of SHA-256 over a version-tagged identity containing the platform-normalized
real path. The version tag keeps a future identity migration explicit without making process CWD or
a nested package path part of the identity. The live development launcher supplies the checkout
root explicitly; the packaged Dev descriptor records it and startup re-resolves the same canonical
path before accepting the scope.

Every development root contains one bounded scope marker,
`internal/development-checkout.json`, with the complete canonical checkout path and digest version. Startup accepts an existing root only when the marker agrees. A mismatch fails
closed instead of opening another checkout's state. A moved checkout receives a new root; the old
root remains untouched.

## Startup order and ownership

Desktop Main resolves the explicit distribution profile and creates its data root, `internal/`, and
`internal/desktop/` before requesting Electron's single-instance lock. Before it reads anything else
in the root it reads `internal/version.json`: a `layout` above 1, or a record it cannot read, stops
startup with the localized refusal in an error dialog, and nothing in the root is read, created, or
logged. When the record is absent Main writes `{"layout":1,"newestVersion":"<version>"}`
exclusively; when its own version takes Semantic Versioning precedence over `newestVersion`, it
replaces the record with that entry raised and every other entry kept. The native writers apply
the same rules under the `data-root-version-record` contract. Live development accepts only the launcher-provided source root;
packaged applications accept only the strict descriptor inside `app.asar`, and process environment
cannot override that profile; `ORCHBEE_HOME` moves only the folder. Main sets Electron `userData` to `internal/desktop/` first, so one profile scope
controls both disk state and lock identity. The same checkout therefore admits one Dev instance,
while Dev, Test, Release, and different development checkouts may run concurrently.

On Windows the bundled Client is compiled with the profile's artifact stem and derives the same
root, and an inherited `ORCHBEE_HOME` names it for both, so Main passes its environment through
unchanged. On macOS and Linux, which keep the release names until ORC-328, a development or test
Main runs `orchbee` with `ORCHBEE_HOME` set to its data root, so a Session it starts keeps this
root's configuration and preferences rather than the installed release's. A release runs `orchbee`
with the environment unchanged. Main creates no configuration file.

On POSIX, Main creates product-owned data directories with owner-only access. The renderer receives
no data-root path, enumeration, location override, or generic filesystem capability. Tests use an
explicitly injected temporary home and source root.

## Window state record

`internal/state/desktop-window.json` holds `version` 1, the normal bounds, the maximized state, and
an optional `extensions` object that a later release fills; Main keeps it unread and writes it back
unchanged. Main is the record's only writer, so loading an earlier `version` applies the steps in
`DESKTOP_WINDOW_STATE_STEPS`, of which none ships yet, and requires the result to pass the current
strict parser. Main then copies the original into
`internal/backups/<YYYY-MM-DD>-upgrade-<version>/internal/state/desktop-window.json`, flushed, and
atomically replaces the record before using it. The upgrade folders are shared with the native
writers, which keep the three newest and number a differing copy `-2`, `-3`; search for
`upgrade-backup-folders` before changing either side. A record whose `version` is newer, or earlier
and not migrated, backed up, and replaced, is never rewritten: the window opens at its default size,
and placement changes last only until Main exits. The first launch that finds such a record logs
`record-unusable` with record `desktop-window`, the problem `newer-format` or `not-updated`, and the
version, and shows the matching localized message once in an information dialog.

## Notification-area record

`internal/state/desktop-tray.json` holds `version` 1, `noticeShown`, and an optional `extensions`
object kept unread and written back unchanged. Windows Main writes it once, when the first notice
is shown, through the same flushed atomic replacement as the window state record. A missing,
unreadable, or malformed record shows the notice again; a newer `version` means a newer Orchbee
showed it, so the notice stays hidden and the record is never rewritten.

## Invariants

- Dev, Test, and Release roots are pairwise distinct.
- Packaged profile identity and data scope come only from the strict embedded descriptor;
  `ORCHBEE_HOME` changes only where the root lives.
- Equal canonical development checkout paths resolve to equal roots.
- An accepted development root marker names exactly the path used to derive its digest.
- Different paths may not share one accepted root, including after a truncated-digest collision.
- Changing or moving a checkout never deletes or migrates the prior root.
- Desktop never writes the top level of a data root; its files live under `internal/`.
- Desktop never replaces an existing `internal/version.json`.

## Verification

`prepareDesktopDataRoot` tests cover the fixed Release root with its files under `internal/`, the
`ORCHBEE_HOME` override and its refusal of a relative value, the stable development root,
different-checkout isolation, marker creation, reuse, and mismatch, and a version record written
once. `FileDesktopWindowStateStore` tests drive a synthetic earlier step: the backup at its relative
path, the replacement, a current second load, and a failed step, backup, or replacement leaving the
record byte-identical; upgrade backup tests cover folder reuse, retention, and numbered copies.
`resolveDesktopApplicationDistribution` tests cover live development from the canonical checkout,
strict packaged descriptors, revalidation of a packaged Dev descriptor against the current checkout,
and environment non-override. No test drives the Main entry, which selects `userData` before
requesting the single-instance lock and, outside a release on macOS and Linux, passes
`ORCHBEE_HOME` to the Client.
