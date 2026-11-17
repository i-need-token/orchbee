# Releasing

The owner's steps for the release path of `.github/workflows/release.yml` and the update check of
`.github/workflows/update-check.yml` after it; what each job does and checks is in
[Release workflow](VERIFICATION.md#release-workflow) and
[Update check workflow](VERIFICATION.md#update-check-workflow), and the names it produces are in
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
3. **GitHub environment.** Settings › Environments › New environment `release`:
   - Required reviewers: none. A pushed tag publishes with no approval.
   - Deployment branches and tags: Selected branches and tags, branch `main` and tag `v*`.
   - Environment secrets: `ORCHBEE_RELEASE_SIGNING_KEY`, the working key's whole PEM text, which
     signs the manifest and, in the job "Release feed", the update feed document and the release
     gate's copy of it, set with
     `gh secret set ORCHBEE_RELEASE_SIGNING_KEY --env release < <working key file>`; and `NPM_TOKEN`,
     the npm token, set with `gh secret set NPM_TOKEN --env release`, which asks for the value.
4. **Tag rule.** Settings › Rules › Rulesets › New tag ruleset, target `v*`, active, restricting
   creations, updates and deletions, with the repository administrator role on the bypass list, so
   only the owner can create or move a release tag.

## Each release

1. On `main`, set the version and commit it: `node scripts/release-version.mjs set 0.1.0-beta.1`,
   then `node scripts/release-version.mjs check v0.1.0-beta.1`. Tags ascend in SemVer order and a
   pre-release is `<version>-beta.<n>`.
2. Optional rehearsal: Actions › Release › Run workflow on `main` with `version` `0.1.0-beta.1`, or
   `gh workflow run release.yml --ref main -f version=0.1.0-beta.1`. It builds, signs and smoke-tests
   the release identity on every row with `build` set, runs
   the release gate's paths a tag run would select, joins the rows the way a tag run would, and
   publishes nothing. Its artifacts are a different build of the same version: never install them.
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
   page) before its job "Publish v0.1.0-beta.1" starts; until then nothing is public.
5. After packaging, each job "Release feed <platform>" signs its row's update feed document and the
   release gate's copy of it with the release key, in a rehearsal too; the gate copy names the
   archives by their file names and is never published. Then the jobs "Release gate from
   v<older>" each install an older release from its published archive on a fresh Windows runner
   and let its own automatic helper update it to this release from the gate copy while a terminal
   keeps running; the job "Release version" names the older releases it selected, or says the gate
   runs no path, which until a release after the first self-updating one is always the case and
   lets publication go on. Publication places the feed's signature beside it, then creates
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
6. Wait for the update check: the publication job starts Actions › Update check for the tag,
   which on a fresh runner of each platform whose release builds update themselves, Windows and
   both macOS rows, installs the version people have now and updates it to the release through
   `orchbee update` from the published assets
   ([Update check workflow](VERIFICATION.md#update-check-workflow)). Go on only when every row
   passes; to run it again, Run workflow with the tag, or
   `gh workflow run update-check.yml --ref main -f tag=v0.1.0-beta.1`. A release published by
   hand starts it too.
7. Move the website to the release once the update check passed: on `main`, run
   `pnpm website:release 0.1.0-beta.1`, then commit what it changed: the update feed pair of each
   platform the release published under `apps/website/public/update/stable/<platform>/`, any pair
   it removed for a platform the release did not publish, and
   `apps/website/src/website/release.ts`, with every published platform's download. Also read the
   site's copy for claims the release changes (platforms, requirements, install and update steps,
   shipped features), and update `packages/i18n/src/locales/*/website-*.ts` in the same commit.
   Then build and deploy the website as [its README](../apps/website/README.md#static-delivery)
   says. The
   command needs no credentials; it refuses, writing nothing, while the release is a draft or lacks
   a file, and when a feed signature or an archive does not check out. The update channel has moved
   when the deploy finishes, and the download buttons name the same version. The first release
   with macOS rows is also the first that shows the macOS downloads, the `install.sh` command and
   the Open Anyway steps.
8. Check what clients read, after the deploy and from a computer signed in nowhere. For each
   platform folder under `apps/website/public/update/stable/`, run
   `%SystemRoot%\System32\curl.exe --disable --fail --silent --show-error --output feed.json https://orchbee.com/update/stable/<platform>/feed.json`,
   the same for `feed.json.sig`, and compare both byte for byte with that folder's tracked
   `feed.json.signed` and `feed.json.sig` (`fc.exe /b`). Fetch
   `https://orchbee.com/install.sh` the same way and compare it with
   `apps/website/public/install.sh`. Clients and the install script send exactly such a request:
   curl's own user agent, no cookie, no script. A challenge page, a 403 or other bytes make every
   client report `download` or `signature-invalid`, so keep Cloudflare's challenge features (Bot
   Fight Mode, Browser Integrity Check, challenge rules) off for `/update/*` and `/install.sh`,
   then check again.
9. Move `main` to the next version in a new commit, for example
   `node scripts/release-version.mjs set 0.1.0-beta.2`.

## When a step fails

- Before publication nothing is public. Fix the cause on `main`; for a tag run, delete the tag
  (`git push origin :refs/tags/v0.1.0-beta.1` and `git tag -d v0.1.0-beta.1`) and tag the fixed
  commit. A failed precondition names the missing secret, key or table entry; a lightweight tag or
  an empty tag message gives the tag command.
- A failed job "Release gate from v<older>" means an Orchbee of that release could not update to
  this one, so nothing was published. Its last lines name the stage that failed, the probe record's
  last line and the diagnostic log's tail. A download that failed for a passing reason, such as a
  timeout, is cleared by "Re-run failed jobs"; any other failure is fixed on `main` like any other
  failure before publication.
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
- A failed update check means the channel must not move yet: leave the website as it is. Its last
  lines name the stage that failed. A download that failed for a passing reason, such as a
  timeout or an answer in the 500s, is cleared by running the check again. Any other failure means
  the release cannot be updated to: fix the cause on `main` and release the next version; the
  failed release stays public for people who install it by hand.

## Release directory per platform

Each platform row stages its cohort with `--artifact-stem orchbee` and writes `release.json` with
`scripts/package-native-release.mjs` on a runner of that platform: the manifest's `platform` is the
runner's own token, such as `windows-x86_64` or `macos-aarch64`, never an argument. A Windows
directory holds `orchbee.exe` and its peers, both launchers and the pinned ConPTY bundle; a macOS
directory holds `orchbee`, `orchbee-server`, `orchbee-worker` and `orchbee-terminal-host`, the
adapter capsule and `cohort.json`, and the tool refuses a ConPTY bundle in it. The Windows row and
both macOS rows build and publish, so a failure on any of them stops the tag. A macOS row adds the
disk image `Orchbee-<version>-macos-<cpu>.dmg`, the `.tar.gz`, the window archive
`orchbee-desktop-<version>-macos-<cpu>.zip`, the feed document, and the platform package
`@orchbee/cli-darwin-<cpu>`. The macOS floor is macOS 13: the Terminal Host and the desktop
application's `Info.plist` name it as their minimum, whatever release the runner has. A platform
row is published only after a rehearsal in which its build, package, smoke and join jobs passed;
then set `"publish": true` on its row of `RELEASE_PLATFORMS` in `.github/workflows/release.yml`.

## What clients rely on

- A published archive stays at its address for as long as a channel's feed names it. A feed is
  signed with a key every client still in use trusts; while rotating keys, sign with the old and
  the new one.
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
