# Releasing

The owner's steps for the release path of `.github/workflows/release.yml` and the update check of
`.github/workflows/update-check.yml` after it; what each job does and checks is in the comments of
those two files, and the names it produces are in
the [release distribution proposal](../proposals/platform/release-distribution.md). Commands run
from the repository root.

## One-time setup

1. **Release keys.** Outside the repository, create two Ed25519 keys in PKCS #8 PEM form, a working
   key and an offline spare, for example `openssl genpkey -algorithm ed25519 -out <file outside the
repository>`. Print each public half with the command in
   [Windows release installation](../apps/orchbee/docs/windows-cli-configuration.md#windows-release-installation)
   and add both to `RELEASE_KEYS` in `apps/orchbee/src/cli/update/signature.rs`, one
   `TrustedKey::from_hex("<64 hex digits>"),` line each, in an ordinary commit. Never commit or paste
   a private key anywhere else.
2. **npm.** On npmjs.com, create the organization `orchbee`, which owns the `@orchbee` scope. The
   unscoped name `orchbee` is taken by its first publication. Create a granular access token:
   Packages and scopes "Read and write", "All Packages" (a new name cannot be selected before it
   exists) plus the `orchbee` scope, "Bypass two-factor authentication" checked, and an expiry that
   covers the release.
3. **GitHub environments.** Settings › Environments:
   - New environment `release`:
     - Required reviewers: none. A pushed tag publishes with no approval.
     - Deployment branches and tags: Selected branches and tags, branch `main` and tag `v*`.
     - Environment secrets: `ORCHBEE_RELEASE_SIGNING_KEY`, the working key's whole PEM text, which
       signs the manifest and, in the job "Release feed", the update feed document and the release
       gate's copy of it, set with
       `gh secret set ORCHBEE_RELEASE_SIGNING_KEY --env release < <working key file>`; and
       `NPM_TOKEN`, the npm token, set with `gh secret set NPM_TOKEN --env release`, which asks for
       the value.
   - New environment `website`, used by `website-release` in `.github/workflows/update-check.yml`:
     - Required reviewers: none.
     - Deployment branches and tags: Selected branches and tags, branch `main` and tag `v*`.
     - Environment secrets: `CLOUDFLARE_API_TOKEN`, a Cloudflare API token made from the "Edit
       Cloudflare Workers" template and limited to this account and the `orchbee.com` zone (the
       Worker serves custom domains), and `CLOUDFLARE_ACCOUNT_ID`, each set with
       `gh secret set <NAME> --env website`, which asks for the value.
     - The job pushes the website channel commit to `main` with its own `GITHUB_TOKEN`
       (`contents: write`). That works while `main` has no branch protection; protecting `main`
       later needs a push credential for this job.
   - On Cloudflare, keep Bot Fight Mode, Browser Integrity Check, and challenge rules off for
     `/update/*` and `/install.sh`, because CLI clients and `install.sh` fetch them anonymously with
     `curl` and fail closed on any 403 or HTML challenge page.
4. **Apple Developer ID.** Create a Developer ID Application certificate and export it with its
   private key as a password-protected PKCS #12 file. In the GitHub `release` environment, set
   `MACOS_SIGNING_CERTIFICATE_BASE64` to the base64-encoded file as one line and
   `MACOS_SIGNING_CERTIFICATE_PASSWORD` to its export password. Set `APPLE_TEAM_ID` to the Team ID
   from the Apple Developer membership that issued the certificate.

   For notarization, create a Team key under App Store Connect › Users and Access › Integrations ›
   App Store Connect API with the Developer role, and download its `AuthKey_<key ID>.p8` file, which
   Apple offers only once. Set `APPLE_API_KEY_BASE64` to that file base64-encoded as one line,
   `APPLE_API_KEY_ID` to its key ID and `APPLE_API_ISSUER_ID` to the issuer ID shown above the key
   list. The key can notarize and manage the team's apps but reaches no personal Apple Account data,
   and revoking it on that page stops it at once.

   Run a release rehearsal after adding or rotating these credentials. Keep a protected copy of
   the certificate and private key outside the repository.

5. **Tag rule.** Settings › Rules › Rulesets › New tag ruleset, target `v*`, active, restricting
   creations, updates and deletions, with the repository administrator role on the bypass list, so
   only the owner can create or move a release tag.

## Each release

1. On `main`, set the version and commit it: `node scripts/release-version.mjs set 0.1.0-beta.1`,
   then `node scripts/release-version.mjs check v0.1.0-beta.1`. Tags ascend in SemVer order and a
   pre-release is `<version>-beta.<n>`.
2. Wait for the release candidate. Once Verify passed for the newest commit on `main`, the Release
   workflow starts a run named "Release candidate" for that commit by itself. It builds, signs and
   smoke-tests the release identity of the version the commit records on every row with `build`
   set, presses Update in a real Test window of each of those rows, runs the release gate's paths a
   tag run would select, joins the rows the way a tag run would, and publishes nothing; a newer
   commit's candidate replaces one still waiting to start. A tag of that commit reuses the files of
   the rows it publishes for a week: the tag run checks and joins them again and publishes minutes
   after the tag, leaving out a row the candidate built without publishing, and a tag pushed while
   the candidate is still building waits for it. A tag of a commit with no passed
   candidate builds everything. Never install a candidate's files: unless a tag reuses them, the
   release is another build of the same version. A rehearsal does the same by hand: Actions ›
   Release › Run workflow on `main` with `version` `0.1.0-beta.1`, or
   `gh workflow run release.yml --ref main -f version=0.1.0-beta.1`; `-f rebuild=true` builds even
   when a passed run can be reused. Start a rehearsal that builds once the newest Verify run on
   `main` has finished: both workflows use macOS runners, GitHub runs at most five macOS jobs of
   this account at once, and a rehearsal started beside Verify waited 17 minutes for its macOS
   builds (2026-11-25). A candidate starts when Verify finishes, so it shares them only with the
   Verify run of a later push.
3. Write the release notes in Markdown in a file outside the repository, with their own headings;
   they follow the generated Install section verbatim. Tag the commit with them and push the tag:
   `git tag -a v0.1.0-beta.1 --cleanup=verbatim -F <notes file>`, then
   `git push origin v0.1.0-beta.1`. `--cleanup=verbatim` keeps lines that start with `#`, which
   git's default cleanup deletes. A lightweight tag or an empty message fails the run's first job,
   before any build: delete the tag and tag again. The notes end with how to update:

   > Update with `orchbee update`. Running terminals and Agents are not interrupted: they keep the
   > old version until they are restarted. Open Orchbee again to use the new one.

   Until 0.1.0, they add:

   > 0.1.0-beta.1 has no update command. Coming from it, install this version once by hand: run the
   > installer again, or `npm install -g orchbee`. Close the terminals you opened from the Orchbee
   > window before you run the installer.

4. The run then needs no human step. To stop a release, cancel the run (Cancel workflow on the run
   page) before its job "Publish v0.1.0-beta.1" starts; until then nothing is public. Publication
   also waits until Verify passed for the tagged commit (job "Verify passed for v0.1.0-beta.1"), so
   tag a commit that Verify on `main` checks, such as the newest one.
5. After packaging, each job "Release feed <platform>" signs its row's update feed document and the
   release gate's copy of it with the release key, in a rehearsal too; the gate copy names the
   archives by their file names and is never published. Then the jobs "Release gate from
   v<older>" (on fresh Windows runners) and "Release gate macos-aarch64 from v<older>" (on a fresh
   `macos-15` runner from the newest published macOS release) each install an older release and its
   window version into `<store root>/desktop/` from its published archives and let its own automatic
   helper update both runtime and window to this release from the gate copy while a terminal keeps
   running; the job "Release version" names the older releases it selected, or says a gate runs no
   path when no qualifying release exists yet, which lets publication go on. Beside the build, each
   job "Release window Update <platform>" builds a Test window and presses Update in it, in
   Settings › About and in the sidebar footer, as the
   [real-window Update check](../scripts/release-window-update.mjs) describes; publication waits for
   it as for the gate. Publication places the
   feed's signature beside it, then creates
   a draft release `v0.1.0-beta.1` holding the setup file, the `.zip`, the
   window archive `orchbee-desktop-0.1.0-beta.1-windows-x86_64.zip`, the feed
   document `orchbee-0.1.0-beta.1-windows-x86_64.feed.json` and its `.sig`, for each macOS row
   the disk image, the `.tar.gz`, the window archive, the feed document and its `.sig`, the npm
   tarballs and `SHA256SUMS`, then publishes the platform packages (`@orchbee/cli-win32-x64`,
   `@orchbee/cli-darwin-arm64` and `@orchbee/cli-darwin-x64`) and `orchbee` under the dist-tag
   `next` and moves their `latest` to this version while `latest` is still an older beta, and then
   publishes the draft as the Latest release. Its notes are an Install section that links the
   setup file, the disk images and the archives and gives the npm command, followed by the tag
   message. The release
   is not a pre-release even for a beta: GitHub shows a Latest release on the repository page, and
   answers `/releases/latest`, only for a release that is not one.
6. Confirm the Update check workflow: the publication job starts Actions › Update check for the tag,
   which on a fresh runner of each platform whose release builds update themselves (Windows, both
   macOS rows and both Linux rows) installs the version people have now—including `<store root>/desktop/` when
   both feeds carry `extensions.desktop`—and updates it to the release through `orchbee update`
   from the published assets ([update check workflow](../.github/workflows/update-check.yml)). To
   run it again by hand, Run workflow with the tag, or
   `gh workflow run update-check.yml --ref main -f tag=v0.1.0-beta.1`.
7. Automatic website rollout and live verification: once every `update-check` row passes, the
   `website-release` job in `update-check.yml` checks `node scripts/website-release.mjs should-advance <version>`
   against `main` (skipping cleanly with exit 0 when `main`'s `release.ts` is already above
   `<version>` in SemVer order so re-running an older tag never downgrades the channel), runs
   `pnpm website:release <version>`, enforces `node scripts/website-release.mjs assert-clean-diff`
   so only `apps/website/src/website/release.ts` and
   `apps/website/public/update/<channel>/<platform>/feed.json.{signed,sig}` can change, runs website
   tests and `pnpm website:build`, commits and pushes to `main` as `i-need-token`, deploys to
   Cloudflare via `wrangler deploy`, and runs `node scripts/website-release.mjs verify-live` to
   anonymously fetch every tracked `/update/<channel>/<platform>/feed.json`, `.sig` and
   `/install.sh` with system `curl` and compare them byte for byte against the tracked repository
   files. When a release also changes website copy in `packages/i18n/src/locales/*/website-*.ts`,
   or when moving the website manually, you can still run `pnpm website:release <version>`, build,
   deploy as [its README](../apps/website/README.md#static-delivery) says, and verify live with
   `node scripts/website-release.mjs verify-live`. The rollout deploys main as it stands, so website changes merged to main go live with the next release.
8. Pull `main` locally (so your checkout includes the website channel commit) and move `main` to
   the next version in a new commit, for example `node scripts/release-version.mjs set 0.1.0-beta.2`.

## When a step fails

- Before publication nothing is public. Fix the cause on `main`; for a tag run, delete the tag
  (`git push origin :refs/tags/v0.1.0-beta.1` and `git tag -d v0.1.0-beta.1`) and tag the fixed
  commit. A failed precondition names the missing secret, key or table entry; a lightweight tag or
  an empty tag message gives the tag command.
- If "Apple notarize desktop <platform>" times out, use "Re-run failed jobs" while the run's
  artifacts are available. The job continues waiting for the same Apple submission and staples
  the signed package it already submitted.
- A failed job "Release gate from v<older>" or "Release gate macos-aarch64 from v<older>" means an
  Orchbee of that release could not update to this one, so nothing was published. Its last lines
  name the stage that failed, the probe record's last line and the diagnostic log's tail. A download
  that failed for a passing reason, such as a timeout, is cleared by "Re-run failed jobs"; any
  other failure is fixed on `main` like any other failure before publication.
- A failed job "Release window Update <platform>" means that pressing Update in a real window of
  this build did not relaunch it into the newer version or did not show the expected notice, so
  nothing was published. Its last lines name the stage that failed, the diagnostic log's tail and
  the processes still running; the run's artifact `release-window-update-<platform>` holds the
  page's screenshot and DOM and the data root's logs.
- A failed publication job is run again: "Re-run failed jobs" on the same run, within 14 days while
  its artifacts are kept, never "Re-run all jobs". A failed npm step leaves the release a draft.
  The re-run publishes from the same artifacts, the feed signature of the job "Release feed"
  among them, completes the draft with the assets it lacks, skips each npm package already
  published with the same tarball and publishes the rest, publishes the draft as the Latest release or confirms a
  published one, and starts the update check. This is how a run that published
  `@orchbee/cli-win32-x64` but not `orchbee` is finished. A rebuild would produce other bytes, and
  the job refuses them.
- If the artifacts have expired, or the job reports a release or npm version with other content, the
  version cannot be finished: npm never accepts the same version twice. Delete the draft release, run
  `npm deprecate @orchbee/cli-win32-x64@0.1.0-beta.1 "Incomplete release; install the next version"`
  for each package that was published, and release the next version.
- An npm refusal to publish over an existing version just after a failed attempt can come from the
  registry's cache: wait five minutes and re-run the failed job.
- A published version that must not be offered any more: run
  `pnpm website:release <last good version>`, commit, build and deploy the website, then delete
  the withdrawn release's archives:
  `gh release delete-asset v<version> orchbee-<version>-windows-x86_64.zip`, and the same for
  `orchbee-desktop-<version>-windows-x86_64.zip` and, for each macOS row it published,
  `orchbee-<version>-macos-<cpu>.tar.gz`. The update channel
  and the download button move back at once; installations already above that version see a lower
  feed and stay as they are, so the fix is the next version. The archives must go because a signed
  feed never expires and stays a public asset: whoever can answer for the feed address to one
  client could replay it to a client below that version, and only a missing archive turns that
  replay into `download` with nothing installed. Apart from those assets, a published release is
  never changed or signed again.
- `pnpm website:release` refusing a published release whose signature or archive does not check
  out means the channel must not move to it: leave the website as it is and release the next
  version.
- A failed update check means the channel has not moved: `website-release` did not run. Its last
  lines name the stage that failed. A download that failed for a passing reason, such as a
  timeout or an answer in the 500s, is cleared by running the check again. Any other failure means
  the release cannot be updated to: fix the cause on `main` and release the next version; the
  failed release stays public for people who install it by hand.
- A failed `website-release` job names the missing environment secret (`gh secret set <NAME> --env website`),
  the path check, the Cloudflare deploy failure, or the live edge mismatch. If the commit to `main`
  already landed before a transient deploy or edge failure, "Re-run failed jobs" on the same
  `update-check.yml` run detects that `main` already names the version while the live edge lags and
  resumes the Cloudflare deploy and `verify-live` without creating another commit. If `verify-live`
  reports 403 or HTML challenge content, exempt `/update/*` and `/install.sh` from Cloudflare
  challenges and re-run.

## Release directory per platform

Each platform row stages its cohort with `--artifact-stem orchbee` and writes `release.json` with
`scripts/package-native-release.mjs`. On Windows the manifest's `platform` is the runner's own
token, such as `windows-x86_64`; a macOS row passes its token, since an Apple silicon Mac builds,
signs, notarizes and packages the Intel row with Cargo's and Zig's `x86_64` targets. The Intel
row's smoke test and window Update run its files on an Intel Mac. A Windows
directory holds `orchbee.exe` and its peers, both launchers and the pinned ConPTY bundle; a macOS
directory holds `orchbee`, `orchbee-server`, `orchbee-worker` and `orchbee-terminal-host`, the
adapter capsule and `cohort.json`, and the tool refuses a ConPTY bundle in it. The Windows row,
both macOS rows and both Linux rows build and publish, so a failure on any of them stops the tag. A macOS row adds the
disk image `Orchbee-<version>-macos-<cpu>.dmg`, the `.tar.gz`, the window archive
`orchbee-desktop-<version>-macos-<cpu>.zip`, the feed document, and the platform package
`@orchbee/cli-darwin-<cpu>`. The macOS floor is macOS 13: the Terminal Host and the desktop
application's `Info.plist` name it as their minimum, whatever release the runner has. A Linux row
builds on a runner of its own architecture with `--platform linux-<cpu>`, which links every program
statically against musl, so the cohort runs on any distribution; its directory holds what a macOS
directory holds. The row's package job then builds the window around the signed cohort, with the
AppImage `Orchbee-<version>-linux-<cpu>.AppImage` and the Debian package
`Orchbee-<version>-linux-<cpu>.deb`; the AppImage starts with a runtime the job builds in Docker
from the AppImage/type2-runtime commit `TYPE2_RUNTIME_COMMIT` pins, so moving that pin is a release
change like any other. `collect` adds the `.tar.gz`, the window archive
`orchbee-desktop-<version>-linux-<cpu>.tar.gz`, the feed document and the platform package
`@orchbee/cli-linux-<cpu>`. `release-smoke-linux.mjs` proves the archive, install.sh with
`--desktop`, the two packages' contents and the npm route, and its `window-update` step presses
Update in a real Test window on a virtual display. A new platform row is published only after a
rehearsal in which its build, package, smoke and join jobs passed; then set `"publish": true` on its
row of `RELEASE_PLATFORMS` in `.github/workflows/release.yml`.

## What clients rely on

- The Windows installer and its programs carry no Authenticode signature; the release manifest's
  Ed25519 signature is the only one.
- macOS release programs carry Developer ID Application signatures. The workflow notarizes the
  disk image and staples Apple's tickets to the disk image and to the application in the window
  archive.
- A published archive stays at its address for as long as a channel's feed names it. A feed is
  signed with a key every client still in use trusts; while rotating keys, sign with the old and
  the new one, and keep `RELEASE_KEYS` in `apps/orchbee/src/cli/update/signature.rs` and
  `ob_release_keys` in `apps/website/public/install.sh` identical (verified by
  `apps/website/src/website/__tests__/install-script.unit.test.ts`).
- `apps/website/public/install.sh` fetches `feed.json.sig` beside `feed.json` (bounded to 4096
  bytes) and verifies the signature under `orchbee-update-feed-v1` against `ob_release_keys` before
  reading the feed or downloading any archive, using an Ed25519-capable `openssl` (`pkeyutl -rawin`)
  or Node.js >= 18. When neither exists, as on a stock Mac (LibreSSL has no `-rawin`), it prints one
  line saying the signature could not be checked and continues over HTTPS; `orchbee install` still
  verifies the signed `release.json`. When a verifier exists and no trusted key verifies, it exits 1
  with nothing installed.
- Breaking feed or archive formats: `apps/website/public/update/<channel>/` is frozen at
  `schemaVersion: 1`, and `scripts/website-release.mjs` refuses to write any feed whose
  `schemaVersion` is not `1` there. To introduce an incompatible `v2` feed or archive format
  without stranding v1 clients, ship a v1 bridge release whose updater reads a new versioned path
  (such as `/update/v2/<channel>/<platform>/feed.json`), freeze `/update/<channel>/` on that bridge
  release, and advance `/update/v2/<channel>/` for subsequent releases.
- What `orchbee update` reports, for reading a person's report:
  - A 404 for the feed: `no-release`, exit 4, naming the channel and platform; a platform or
    channel without a published feed answers this way. A 404 for the archive: `download`.
  - An archive longer or shorter than the feed's signed size, or other bytes: `checksum`. A body
    cut short while the server had sent its length: `download`. Either way nothing is installed.
  - Another update holding the download lock: `busy`, before any archive request.
  - No `System32\tar.exe` or `System32\curl.exe` (`/usr/bin/tar` or `/usr/bin/curl` on macOS), or
    curl older than 8.4 for HTTPS: `download-unavailable`.
  - An archive transfer below 4 KiB/s for 60 seconds, or past the signed size at that rate plus
    600 seconds (at most six hours): `timeout`. The rest of the run has its own 600 seconds.
- `<store root>\.download\` is an entry of the store: `download.lock` always, and `archive.zip`
  (`archive.tar.gz` on macOS) and `unpacked\` during an update. `orchbee uninstall` removes it with the store's other entries.
- Only a test build admits a loopback feed address, `http://127.0.0.1:<port>/…`, for rehearsals; a
  release build reads HTTPS or a local path.

## Moving npm to trusted publishing

The token is for the first publication, which creates the package names. Afterwards, add a trusted
publisher for `orchbee` and each `@orchbee/cli-*` platform package on npmjs.com (repository, workflow
`release.yml`, environment `release`), then in one commit: install npm 11.5.1 or newer in
`release-publish` before publishing, remove the `NODE_AUTH_TOKEN` line of its npm step, and remove
the npm token check of `release-preconditions`. Provenance stays, and the `NPM_TOKEN` secret can then
be deleted and the token revoked.
