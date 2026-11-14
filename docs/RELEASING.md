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
   - Required reviewers: the owner. Leave "Prevent self-review" off, because the owner also pushes
     the tag.
   - Deployment branches and tags: Selected branches and tags, branch `main` and tag `v*`.
   - Environment secrets: `ORCHBEE_RELEASE_SIGNING_KEY`, the working key's whole PEM text, which
     signs the manifest and, in the publication job, the update feed document, set with
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
   the release identity and publishes nothing. Its artifacts are a different build of the same
   version: never install them.
3. Tag the commit and push the tag: `git tag -a v0.1.0-beta.1 -m "Orchbee 0.1.0-beta.1"`, then
   `git push origin v0.1.0-beta.1`.
4. Approve each job that waits for `release` on the run page (Review deployments › `release` ›
   Approve and deploy): the preconditions at the start, signing after the build, and publication
   after both smoke jobs pass. A rehearsal asks for the first two. Approve publication within 14
   days, while the run's artifacts are kept.
5. Publication signs the update feed document with the release key, then creates a draft release
   `v0.1.0-beta.1`, marked pre-release, holding the setup file, the `.zip`, the feed document
   `orchbee-0.1.0-beta.1-windows-x86_64.feed.json` and its `.sig`, the npm tarballs and
   `SHA256SUMS`, and then publishes `@orchbee/cli-win32-x64` and
   `orchbee` under the dist-tag `next`. Open the draft, write the notes, and press Publish release.
   The npm packages are public as soon as the job ends, before the draft is published.
6. Wait for the update check: pressing Publish release starts Actions › Update check for the tag,
   which on a fresh runner installs the version people have now and updates it to the release
   through `orchbee update` from the published assets
   ([Update check workflow](VERIFICATION.md#update-check-workflow)). Go on only when it passes; to
   run it again, Run workflow with the tag, or
   `gh workflow run update-check.yml --ref main -f tag=v0.1.0-beta.1`.
7. Move the website to the release once the update check passed: on `main`, run
   `pnpm website:release 0.1.0-beta.1`, commit the update feed pair under
   `apps/website/public/update/` and `apps/website/src/website/release.ts` it wrote, then build and
   deploy the website as [its README](../apps/website/README.md#static-delivery) says. The command
   needs no credentials; it refuses, writing nothing, while the release is a draft or lacks a file,
   and when the feed signature or the archive does not check out. The update channel has moved
   when the deploy finishes, and the download button names the same version.
8. Check what clients read, after the deploy and from a computer signed in nowhere: run
   `%SystemRoot%\System32\curl.exe --disable --fail --silent --show-error --output feed.json https://orchbee.com/update/stable/windows-x86_64/feed.json`,
   the same for `feed.json.sig`, and compare both byte for byte with the tracked
   `feed.json.signed` and `feed.json.sig` (`fc.exe /b`). Clients send exactly such a request:
   curl's own user agent, no cookie, no script. A challenge page, a 403 or other bytes make every
   client report `download` or `signature-invalid`, so keep Cloudflare's challenge features (Bot
   Fight Mode, Browser Integrity Check, challenge rules) off for `/update/*`, then check again.
9. Move `main` to the next version in a new commit, for example
   `node scripts/release-version.mjs set 0.1.0-beta.2`.

## When a step fails

- Before publication nothing is public. Fix the cause on `main`; for a tag run, delete the tag
  (`git push origin :refs/tags/v0.1.0-beta.1` and `git tag -d v0.1.0-beta.1`) and tag the fixed
  commit. A failed precondition names the missing secret, key or table entry.
- During publication, use "Re-run failed jobs" on the same run, never "Re-run all jobs": the re-run
  publishes from the same artifacts, signs the feed document again to the same bytes, completes the
  draft with the assets it lacks, skips each npm
  package already published with the same tarball, and publishes the rest. This is how a run that
  published `@orchbee/cli-win32-x64` but not `orchbee` is finished. A rebuild would produce other
  bytes, and the job refuses them.
- If the artifacts have expired, or the job reports a release or npm version with other content, the
  version cannot be finished: npm never accepts the same version twice. Delete the draft release, run
  `npm deprecate @orchbee/cli-win32-x64@0.1.0-beta.1 "Incomplete release; install the next version"`
  for each package that was published, and release the next version.
- An npm refusal to publish over an existing version just after a failed attempt can come from the
  registry's cache: wait five minutes and re-run the failed job.
- A published version that must not be offered any more: run
  `pnpm website:release <last good version>`, commit, build and deploy the website, then delete
  the withdrawn release's archive:
  `gh release delete-asset v<version> orchbee-<version>-windows-x86_64.zip`. The update channel
  and the download button move back at once; installations already above that version see a lower
  feed and stay as they are, so the fix is the next version. The archive must go because a signed
  feed never expires and stays a public asset: whoever can answer for the feed address to one
  client could replay it to a client below that version, and only a missing archive turns that
  replay into `download` with nothing installed. Apart from that asset, a published release is
  never changed or signed again.
- `pnpm website:release` refusing a published release whose signature or archive does not check
  out means the channel must not move to it: leave the website as it is and release the next
  version.
- A failed update check means the channel must not move yet: leave the website as it is. Its last
  lines name the stage that failed. A download that failed for a passing reason, such as a
  timeout or an answer in the 500s, is cleared by running the check again. Any other failure means
  the release cannot be updated to: fix the cause on `main` and release the next version; the
  failed release stays public for people who install it by hand.

## What clients rely on

- A published archive stays at its address for as long as a channel's feed names it. A feed is
  signed with a key every client still in use trusts; while rotating keys, sign with the old and
  the new one.
- What `orchbee update` reports, for reading a person's report:
  - A 404 for the feed or the archive: `download`.
  - An archive longer or shorter than the feed's signed size, or other bytes: `checksum`. A body
    cut short while the server had sent its length: `download`. Either way nothing is installed.
  - Another update holding the download lock: `busy`, before any archive request.
  - No `System32\tar.exe` or `System32\curl.exe`, or curl older than 8.4 for HTTPS:
    `download-unavailable`.
  - An archive transfer below 4 KiB/s for 60 seconds, or past the signed size at that rate plus
    600 seconds (at most six hours): `timeout`. The rest of the run has its own 600 seconds.
- `<store root>\.download\` is an entry of the store: `download.lock` always, and `archive.zip` and
  `unpacked\` during an update. `orchbee uninstall` removes it with the store's other entries.
- Only a test build admits a loopback feed address, `http://127.0.0.1:<port>/…`, for rehearsals; a
  release build reads HTTPS or a local path.

## Moving npm to trusted publishing

The token is for the first publication, which creates the package names. Afterwards, add a trusted
publisher for `orchbee` and `@orchbee/cli-win32-x64` on npmjs.com (repository, workflow
`release.yml`, environment `release`), then in one commit: install npm 11.5.1 or newer in
`release-publish` before publishing, remove the `NODE_AUTH_TOKEN` line of its npm step, and remove
the npm token check of `release-preconditions`. Provenance stays, and the `NPM_TOKEN` secret can then
be deleted and the token revoked.
