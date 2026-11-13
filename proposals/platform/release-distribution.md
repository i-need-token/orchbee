# Release Distribution

| Field             | Value                                                                                                                                                                                                                                                                                                                                                                                                  |
| ----------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Status            | Proposed on 2026-11-11 for the first public build. Rows marked Decided restate product-owner decisions and rows marked Built restate shipped code; neither is open. Rows marked Chosen are this page's values: each stays revisable until the first candidate is tagged and is frozen from then on.                                                                                                    |
| Owner             | Maintainers of `apps/orchbee`, `apps/desktop`, `crates/local-runtime`, the release scripts under `scripts/`, and the release workflow                                                                                                                                                                                                                                                                  |
| Current authority | [Windows release installation](../../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation), [package versioning](../../docs/PACKAGES.md#versioning), the [CLI command surface](../../docs/cli/command-surface.md), the [shared runtime store](versioning-and-upgrade.md#shared-runtime-store), and [locations outside the data root](data-root-layout.md#outside-the-data-root) |
| Exit condition    | Delete this proposal once the installer, the npm package, the release workflow, and `update` ship and each rule lives in its owning guide or standard.                                                                                                                                                                                                                                                 |

Every name below is read by a person, a script, or an installed build that cannot be changed
afterwards. A builder takes names from this page and adds none. In the Basis column, Verified names
what was checked and Unverified names what a slice must still prove. Development and test builds
replace `orchbee` and `Orchbee` in every name by their
[environment](environment-isolation.md#environments) names and never create a name on this page.

## Platforms and programs

| Item            | Value                                                                                                                                                                                     | Used by                                                                       | Basis                                                                                                                                                                                                                                                                                 |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Platform tokens | `windows-x86_64`, `windows-aarch64`, `macos-aarch64`, `macos-x86_64`, `linux-x86_64`, `linux-aarch64`                                                                                     | Manifest `platform`, feed paths, download names, store admission              | Decided. Built: `manifest::own_platform()` yields exactly these                                                                                                                                                                                                                       |
| Programs        | `orchbee`, `orchbee-server`, `orchbee-worker`, `orchbee-terminal-host`, with `.exe` on Windows                                                                                            | Release directory, process names, `cohort.json`                               | Built: `scripts/stage-native-windows.mjs`                                                                                                                                                                                                                                             |
| Launcher        | `orchbee-launcher.exe` in a Windows release directory, installed as `bin\orchbee.exe`                                                                                                     | [Runtime store](#runtime-store)                                               | Chosen. Built on Windows                                                                                                                                                                                                                                                              |
| Application     | `Orchbee`; its Windows executable is `orchbee-desktop.exe`                                                                                                                                | Window title, shortcuts, installer, bundle                                    | Built: `apps/desktop/src/distribution/desktop-distribution-profile.ts`                                                                                                                                                                                                                |
| Linux C library | One build per Linux token, statically linked against musl, so `linux-*` names no C library. A build that ever needs one takes a new token; `linux-x86_64` and `linux-aarch64` keep theirs | Manifest `platform`, npm sub-packages without a `libc` field, remote installs | Chosen. Verified by reading: `Cargo.lock` holds no C-library dependency beyond `libc`, and staging already builds the Terminal Host for `x86_64-linux-musl`. Unverified: no Rust program was compiled or run for a musl target; the first Linux row of the release workflow proves it |
| Linux Desktop   | Runs on glibc only, because Electron does; its floor is Electron's                                                                                                                        | `orchbee-desktop-*` archive                                                   | Unverified: Electron's floor was not looked up                                                                                                                                                                                                                                        |

## Versions and tags

| Item           | Value                                                                                                                                                                                                                                                                                                      | Used by                                                                  | Basis                                                                                                                    |
| -------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------ |
| Scheme         | SemVer 2.0.0 without build metadata; pre-releases are `<version>-beta.<n>`. `0.1.0` is the public beta                                                                                                                                                                                                     | `--version`, manifest `version`, handshake, data root                    | Decided; `-beta.<n>` is the [package rule](../../docs/PACKAGES.md#versioning)                                            |
| One version    | Client, Server, Worker, Terminal Host, Desktop, and every npm package of a release carry the same version                                                                                                                                                                                                  | Every artifact                                                           | Decided                                                                                                                  |
| Source         | The root `Cargo.toml` `[workspace.package] version`. The Client, Server, Worker and Windows launcher crates, `crates/local-runtime`, and `apps/orchbee/crates/workbench-client` inherit it; the release-version script also writes `apps/desktop/package.json` and the Terminal Host's `build.zig` default | Release-version script, tag check                                        | Decided                                                                                                                  |
| Tag            | `v<version>`, as `v0.1.0-beta.1`. The GitHub Release takes the tag as its name and is a pre-release exactly when the version has a pre-release part                                                                                                                                                        | Release workflow trigger, release notes                                  | Chosen                                                                                                                   |
| Built once     | One tag gives one workflow run, and every download, npm package, and feed directory of that version comes from that run. A published version is never rebuilt or signed again; a fix is the next version                                                                                                   | Store directory names, feed paths, `SHA256SUMS`                          | Chosen                                                                                                                   |
| Build identity | Manifest `buildId` is `<40-hex commit>.<workflow run number>`; the commit is also `ORCHBEE_SOURCE_COMMIT`                                                                                                                                                                                                  | Diagnostics                                                              | Chosen. Verified: fits the `buildId` grammar                                                                             |
| npm dist-tag   | `next` for a version with a pre-release part, `latest` otherwise                                                                                                                                                                                                                                           | `npm i -g orchbee@next`                                                  | Chosen; the package rule requires a tag other than `latest`                                                              |
| Sequence       | Tags ascend strictly in SemVer precedence: `0.1.0-beta.1`, `0.1.0-beta.2`, …, `0.1.0`, `0.1.1`. The tree carries the next version to tag: it becomes `0.1.0-beta.1` before any release-named build leaves the machine that built it, and moves on in the commit after each tag                             | Lower-version refusal, data root `newestVersion`, upgrade backup folders | Chosen. Verified against `version_record.rs`, `data_root/state.rs`, and Desktop's `prepareDesktopDataRoot.ts`; see below |

The tree says `0.1.0` today, which SemVer orders above `0.1.0-beta.1`. The sequence rule, not a data
reset, is what keeps the first candidate safe:

- A data root is admitted by its `layout` alone. `newestVersion` is only ever raised, never compared
  for admission, and a record migrates by its own format `version`. A root that a `0.1.0` build
  wrote therefore opens under `0.1.0-beta.1` without refusal or migration and keeps saying
  `newestVersion: "0.1.0"`.
- The store does compare product versions: with `0.1.0` active, the
  [lower-version refusal](#update-minimum) would refuse `0.1.0-beta.1`. So no build that calls
  itself `0.1.0` is published into a store before the public `0.1.0`.
- Records written before the first candidate otherwise stay under the
  [pre-release rule](data-root-layout.md#persisted-data-versions-and-migration): nothing migrates,
  and the person resets a record a candidate refuses.

## Download names

`<version>` and `<platform>` are the values above. Every name is the same on GitHub Releases and on
the [feed](#release-feed).

| File                                          | Platforms            | Holds                                                                                                           | Basis                                                                                           |
| --------------------------------------------- | -------------------- | --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `Orchbee-<version>-<platform>-setup.exe`      | `windows-*`          | The per-user installer: window application, runtime, and the `orchbee` command                                  | Chosen                                                                                          |
| `Orchbee-<version>-<platform>.dmg`            | `macos-*`            | `Orchbee.app`, ad-hoc signed, with its runtime inside                                                           | Chosen                                                                                          |
| `orchbee-desktop-<version>-<platform>.tar.gz` | `linux-*`            | The window application's per-user tree; `.deb` and AppImage are not built                                       | Chosen; the product owner still decides between this and the earlier AppImage and `.deb` choice |
| `orchbee-<version>-<platform>.zip`            | `windows-*`          | The signed release directory, for the command line alone                                                        | Chosen                                                                                          |
| `orchbee-<version>-<platform>.tar.gz`         | `macos-*`, `linux-*` | The same, with executable modes kept                                                                            | Chosen                                                                                          |
| `SHA256SUMS`                                  | One per version      | One `sha256sum` line, `<64 hex>`, two spaces, file name, LF, for every other file of the version, in name order | Chosen                                                                                          |

- An archive holds one top-level directory named like the archive without its extension.
- A signed release directory is `release.json`, `release.json.sig`, and every file the manifest
  lists at its `path`. Each manifest `url` is the relative path equal to `path`, so the one signed
  manifest of a version and platform serves the archive, the installer's bundled runtime, the npm
  sub-package, and the feed.
- The Windows installer and its programs carry no Authenticode signature in the first version
  (Decided); the README gives the SmartScreen steps.

## npm packages

| Package                     | Role                                                                                                                                                                                          | Basis                                                                                                                                                                                   |
| --------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `orchbee`                   | The only package a person installs. Its `bin` is `orchbee`, a Node launcher; it lists the sub-packages of its own version as exact `optionalDependencies`, and only those that were published | Decided. Verified 2026-11-11: the registry answers 404 for `orchbee`, and for `orch-bee` and `orch_bee`, which npm's similar-name rule compares                                         |
| `@orchbee/cli-win32-x64`    | `windows-x86_64` release directory                                                                                                                                                            | Chosen: `@orchbee/cli-<process.platform>-<process.arch>`. Verified 2026-11-11: the registry knows no `orchbee` scope and no package in it; no `packages/*` directory starts with `cli-` |
| `@orchbee/cli-win32-arm64`  | `windows-aarch64` release directory                                                                                                                                                           | As above                                                                                                                                                                                |
| `@orchbee/cli-darwin-arm64` | `macos-aarch64` release directory                                                                                                                                                             | As above                                                                                                                                                                                |
| `@orchbee/cli-darwin-x64`   | `macos-x86_64` release directory                                                                                                                                                              | As above                                                                                                                                                                                |
| `@orchbee/cli-linux-x64`    | `linux-x86_64` release directory                                                                                                                                                              | As above                                                                                                                                                                                |
| `@orchbee/cli-linux-arm64`  | `linux-aarch64` release directory                                                                                                                                                             | As above                                                                                                                                                                                |

- A sub-package holds the signed release directory under `release/`, declares `os` and `cpu`, and
  has no `bin` and no install script.
- The launcher runs nothing at install time. On each start it reads the
  [pointer](#runtime-store); when no version is active or the carried version is higher than the
  active one, it runs the carried `orchbee install --no-path`; then it runs the active `orchbee`
  with the same arguments, streams, and signals and returns its exit code. On a platform without a
  sub-package it prints one line naming the supported platforms.
- Built, not published: `scripts/package-npm.mjs` writes `orchbee` and the sub-packages of the
  release directories it is given from the templates in `apps/orchbee/npm/`. The launcher reads the
  store root by the rule `orchbee install` uses, waits while another start's install answers
  `busy`, and stops when the install reports a version directory outside that root.
- These packages are carriers of the product, not libraries under `packages/*`: their version is
  always the product version, an exception that [Packages](../../docs/PACKAGES.md#versioning)
  states.
- `npm rm -g orchbee` removes only the carrier; `orchbee uninstall` comes first.

## Commands

| Command                                                                           | Effect                                                                                                                                                                                                                                                                                                                                                     | Basis                                                                                                                                                      |
| --------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `orchbee install [--no-path] [--allow-downgrade] [--format text\|json]`           | Installs the signed release directory the running `orchbee` sits in and makes it the active version. Run from a version directory of the store, it verifies and activates that version again. `--no-path` skips the [PATH step](#runtime-store). It asks nothing, so it takes no `--yes`                                                                   | Chosen. Built on Windows, PATH step included                                                                                                               |
| `orchbee uninstall [--delete-data] [--format text\|json]`                         | The one [uninstall](data-root-layout.md#uninstall): program files, PATH entry or link, installer entries, Agent integrations; data stays unless `--delete-data`. A release build lists what goes, with open terminals and Agents, and runs only when the person types `orchbee` at an interactive console; development and test builds require `--confirm` | `uninstall` Decided; options Chosen under the [destructive rule](../../docs/cli/command-surface.md#destructive-and-interactive-commands); Built on Windows |
| `orchbee update [--format text\|json]`                                            | Installs the newest version of the selected channel from the feed                                                                                                                                                                                                                                                                                          | Built; withheld in release builds until it is released                                                                                                     |
| `curl -fsSL https://orchbee.com/install.sh \| sh`                                 | macOS and Linux: reads `SHA256SUMS` of the `stable` alias, downloads and checks the platform's `orchbee-*` archive, runs its `orchbee install`, and removes the unpacked copy. After `sh -s --`: `--version <version>`, `--no-path`, and on Linux `--desktop`, which also installs the window application                                                  | Chosen                                                                                                                                                     |
| `Orchbee-<version>-<platform>-setup.exe /VERYSILENT /SUPPRESSMSGBOXES /NORESTART` | Unattended install; the installer's own switches                                                                                                                                                                                                                                                                                                           | Inno Setup's documented switches; not rechecked                                                                                                            |

Two helper modes are a contract between builds, not commands: help, completion, and the Agent guide
leave them out, the owning guide documents them, and their grammar never changes. A change adds a
new name, as with [`--machine-link-v1`](../../apps/orchbee/docs/machines.md#link-helper).

| Helper                                                          | Run by                                                             | Effect                                                                                                                                                                                         |
| --------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `orchbee --complete-install-v1 [--no-path] [--allow-downgrade]` | The Client that just published this copy, by `install` or `update` | [Completes the install](#update-minimum) of the version it belongs to                                                                                                                          |
| `orchbee --installer-uninstall-v1 [--delete-data]`              | A platform uninstaller, after its own confirmation dialog          | The same uninstall without a prompt. It ends no terminal: with one open in any Session of the environment it changes nothing, and the uninstaller says to quit Orchbee first. Built on Windows |

| Variable                               | Meaning                                                                                                           | Basis                                                |
| -------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- |
| `ORCHBEE_INSTALL_DIR`                  | Another store root, on every platform. `XDG_DATA_HOME` and `XDG_BIN_HOME` are not read                            | Decided; Built on Windows                            |
| `ORCHBEE_HOME`                         | Another data root                                                                                                 | Built                                                |
| `ORCHBEE_UPDATE_MANIFEST`              | Another manifest address, with `{channel}`; the signature is still required                                       | Built                                                |
| `HTTPS_PROXY`, `ALL_PROXY`, `NO_PROXY` | Passed to the download of `update`, with the system proxy as the default; today's download clears its environment | Decided as a default; the `update` slice declares it |

## Locations and identity

| Item                 | Windows                                                                                                                                                      | macOS                                                        | Linux                                                                            | Basis                                                                                                     |
| -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------ | -------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| Store root           | `%LOCALAPPDATA%\Programs\Orchbee\`                                                                                                                           | `~/.local/share/orchbee/`                                    | `~/.local/share/orchbee/`                                                        | Decided                                                                                                   |
| `orchbee` on PATH    | One user `Path` entry, `<store root>\bin`, added once                                                                                                        | Link `~/.local/bin/orchbee`                                  | Link `~/.local/bin/orchbee`                                                      | Decided: one entry or link, no shell profile edit. What the entry names is Chosen                         |
| Data root            | `%USERPROFILE%\.orchbee`                                                                                                                                     | `~/.orchbee`                                                 | `~/.orchbee`                                                                     | Decided, Built                                                                                            |
| Window application   | `<store root>\desktop\`                                                                                                                                      | `Orchbee.app` in Applications                                | `<store root>/desktop/`                                                          | Decided                                                                                                   |
| Application identity | AppUserModelID `com.orchbee.desktop`                                                                                                                         | Bundle identifier `com.orchbee.desktop`                      | Desktop entry `~/.local/share/applications/orchbee.desktop`, icon name `orchbee` | Chosen; replaces `com.electron.orchbee`. Development and test profiles append `.dev` and `.test` as today |
| System entries       | Start menu shortcut `Orchbee`; uninstall registration `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\com.orchbee.desktop_is1`, shown as `Orchbee` | Files macOS keeps under `~/Library` by the bundle identifier | Icons under `~/.local/share/icons/hicolor/`                                      | Chosen. Verified: Inno Setup names the key `<AppId>_is1` and writes it under `HKCU` in a per-user install |

- The Windows installer is built with Inno Setup: `AppId` `com.orchbee.desktop` for both
  architectures, `PrivilegesRequired=lowest`, one architecture per installer. A later installer with
  the same `AppId` replaces the installation. It shows no directory, components, or licence page:
  Ready to Install, progress, then a finish page with a Launch Orchbee box, in English and
  Simplified Chinese. Chosen. Verified 2026-11-11: both GitHub Windows runner images carry Inno
  Setup 6.7.1, and an installer can be limited to `arm64`. The workflow fails when the compiler
  version differs from the one it pins. Unverified: Inno Setup appears to ship no Simplified Chinese
  translation, so the installer carries a reviewed copy of that file.
- The installer writes only `desktop\`, the shortcut, and the registration, then runs the bundled
  `orchbee install`; a higher active version stays active. Settled by the install slice: the store
  opens only its own names and never reads `desktop\`. Whichever writer comes first creates the
  root: a root `orchbee install` creates carries a protected private access rule that `desktop\`
  then inherits, and an existing root, such as the one the installer creates for `desktop\`, is
  admitted only when its inherited access rule is already private, and is never repaired. So this
  order works when `%LOCALAPPDATA%\Programs` carries only the person, SYSTEM, and Administrators, as
  in a standard profile; another allow entry there makes the install refuse with `storage` in
  either order. Verified 2026-11-12 with a scratch root whose `desktop\` existed first. Unverified:
  no installer has run.
- On macOS and Linux the window application runs the bundled `orchbee install` when it starts and
  the store holds no active version or a lower one.
- The store holds one architecture; an install that finds another refuses without writing.

## Runtime store

```text
<store root>/
├── versions/<version>/     one immutable signed release directory per installed version
├── active.json             the pointer
├── install.lock            the writer lock
├── bin/orchbee.exe         Windows only: the launcher
└── desktop/                Windows and Linux only: the window application
```

- A version directory is named by its manifest `version` and holds that release directory
  unchanged, `release.json` and `release.json.sig` included. Publication stages inside the root and
  renames into `versions/`; an existing directory is verified and reused, never overwritten.
  `versions/` keeps version names apart from the fixed names beside it.
- `active.json` is closed JSON, replaced by a flushed rename of `active.pending` under
  `install.lock`. `version` is the record's format. `active` and the optional `previous`, the
  rollback target, each name a version directory and the lowercase SHA-256 of its `release.json`,
  which binds the directory to the manifest verified when it was installed. An optional
  `extensions` object is the only open slot.

  ```json
  {
  	"version": 1,
  	"active": { "product": "0.1.0", "manifest": "<64 hex digits>" },
  	"previous": { "product": "0.1.0-beta.3", "manifest": "<64 hex digits>" }
  }
  ```

- The pointer alone names the active version. Desktop, the npm launcher, the Windows launcher, and
  a remote start read it; nothing infers the active version from `PATH`, a link, or the highest
  directory name. A missing or unreadable pointer means no active version, which an install
  repairs.
- Windows PATH step: copy the active version's `orchbee-launcher.exe` to `bin\orchbee.exe` when its
  bytes differ, and add `<store root>\bin` to the user `Path` when it is absent. Entries of the user
  `Path` naming another directory inside the store root were written by the earlier layout; the same
  write removes them, and the result says so. The command writes the value and does not broadcast
  the change: a platform installer broadcasts at its own end (Inno Setup's `ChangesEnvironment`),
  and a broadcast from the command itself waits for an unsafe-code policy decision. The launcher
  reads `active.json`, starts `versions\<product>\orchbee.exe` with the unchanged command line,
  console, standard handles, and environment, ends it when the launcher itself is ended, and returns
  its exit code; it reads nothing else. Verified 2026-11-12 on a test build: `bin\<stem>.exe --version` took a median 17 ms
  against 9 ms for the version's own Client (two sets of 40 warm runs each on a quiet machine), so
  the launcher adds about 8 ms per `orchbee` invocation; with other builds loading the machine, both
  doubled (34–39 against 19–22 ms).
- macOS and Linux PATH step: replace `~/.local/bin/orchbee` by rename with a link to the active
  version's `orchbee`. A file there that is not a link into the store is left alone and reported.
  When `~/.local/bin` is not on `PATH`, print the one line to add.
- Every process starts its children from its own version directory. Installing only adds
  directories. A version directory is deleted only under `install.lock`, by the active version or a
  newer one, when it is neither `active` nor `previous`, no process runs from it, and no Session,
  Host, or Capsule references it. Uninstall removes only the store's own entries, and only from a
  root whose `active.json` reads as a pointer: the entries above, `active.pending` and publication
  stages, then the pointer, then the root when nothing else is in it.
- This replaces the directories named by manifest SHA-256 and the `Path` entry that named one.
  The pointer cannot be that entry, because the npm and remote routes add none. Built on Windows:
  `versions\<version>`, `active.json`, `install.lock`, the completion helper, the pointer readers,
  the launcher, the PATH step, and uninstall.

## Release feed

| Address                                              | Answers                                                                                              |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `https://dl.orchbee.com/<version>/<platform>/<path>` | The signed release directory of that version and platform: `release.json`, `release.json.sig`, files |
| `https://dl.orchbee.com/<version>/<file>`            | Each [download](#download-names) of that version, `SHA256SUMS` included                              |
| `https://dl.orchbee.com/<channel>/<rest>`            | A temporary redirect to `https://dl.orchbee.com/<version>/<rest>` for the channel's current version  |
| `https://orchbee.com/install.sh`                     | The install script                                                                                   |

- The compiled default manifest address is
  `https://dl.orchbee.com/{channel}/<own platform>/release.json`. Verified: the updater resolves a
  relative file `url` beside the configured address and follows up to five HTTPS redirects, so
  every file request of an update goes through the channel address. An update that straddles a
  channel move fails a digest check and succeeds when run again.
- A channel is `stable` or `preview` and a version starts with a digit, so the first path segment
  is never ambiguous.
- A version's objects are written once and never replaced. The channel redirect stays on the same
  host, is never permanent, and is cached for at most five minutes.
- The workflow uploads a version, publishes the GitHub Release, passes the install check on a fresh
  machine, and only then moves the channel.
- The feed keeps the three newest versions of each channel and every version published in the last
  90 days; GitHub Releases keeps every version.
- Served from Cloudflare R2 (Decided). Verified 2026-11-11: `orchbee.com` is delegated to
  Cloudflare name servers. Unverified: that the `dl` record exists, and how the redirect is served.

## Update minimum

The first build offered as updatable, the public `0.1.0`, already holds all of this, because an
installed build is changed only by itself or by a reinstall. A candidate without it says "reinstall
to upgrade" and keeps `update` withheld.

1. **Keys.** `RELEASE_KEYS` holds two Ed25519 public keys from the first candidate on: the working
   key, whose private half is the secret `ORCHBEE_RELEASE_SIGNING_KEY` of the protected GitHub
   environment `release`, and a spare whose private half stays offline. Every manifest carries the
   working key's signature; the spare signs only after the working key is lost or compromised.
   [Rotation](versioning-and-upgrade.md#release-manifest) is unchanged.
2. **Feed.** The compiled default address and layout of [Release feed](#release-feed).
3. **Channel.** Until `channel set preview` is released, every version, pre-releases included, is
   published with `channel: "stable"`, because an updater refuses a manifest whose channel is not
   the selected one (Verified: `engine.rs`) and no release build can select `preview`. The `stable`
   redirect moves to each candidate until `0.1.0` and from then on never names a version with a
   pre-release part.
4. **Store.** The layout, pointer, and PATH steps of [Runtime store](#runtime-store).
5. **Completion.** `install` and `update` verify the signature and every file, publish
   `versions/<version>/`, and then run that copy's `orchbee --complete-install-v1`, passing on
   `--no-path` and `--allow-downgrade`. The completing Client, never the one that downloaded it,
   writes `active.json`, performs the PATH step, and refreshes what its version owns outside the
   store. `update` always performs the PATH step. A failed completion leaves the pointer where it
   was.
6. **Lower version.** The installing Client refuses, before any download, a version that SemVer
   orders below the active one, and the completing Client refuses again before it moves the
   pointer. Only the person's choice lifts it: `install --allow-downgrade`, or a channel switch.
   An equal version is verified and activated again.
7. **Desktop identity.** The identities of [Locations and identity](#locations-and-identity), so a
   later installer replaces the installation instead of adding a second one.

Background download, the "Update available" word, restart without losing terminals, and
cross-version tests come after this minimum.
