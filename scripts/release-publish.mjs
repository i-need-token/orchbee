// The steps of the release workflow around the build: what a run releases, its release notes,
// whether its secrets and trusted key are in place, and the publication of the collected artifacts
// as a GitHub release and on npm. Each step either finishes or stops with sentences that name what
// is wrong; none prints a secret, and a step run again after a partial publication finishes it
// without duplicating or replacing anything already published.
//
//   node scripts/release-publish.mjs plan --event <name> --ref <git ref> [--version <version>]
//   node scripts/release-publish.mjs notes --tag v<version> --repository <owner/name>
//   node scripts/release-publish.mjs gate-paths --version <version> --repository <owner/name>
//   node scripts/release-publish.mjs preconditions --signing-key-env <VARIABLE> [--npm-token-env <VARIABLE>]
//   node scripts/release-publish.mjs github <artifacts> --tag v<version> --repository <owner/name>
//   node scripts/release-publish.mjs npm <artifacts> --version <version> --repository-url <https URL>
//   node scripts/release-publish.mjs publish <artifacts> --tag v<version> --repository <owner/name>
//
// - plan: a pushed tag `v<version>` releases and publishes that version from the rows with `build`
//   and `publish`; a manual run names the version to rehearse, takes every row with `build`, and
//   publishes nothing. Reads the platform rows from RELEASE_PLATFORMS, refuses a row of a platform
//   family the workflow has no jobs for and a run without a Windows row, and writes `version`,
//   `tag`, `publish`, `prerelease`, `dist-tag`, `matrix` (every row), `windows-matrix`,
//   `macos-matrix` and `macos` (whether it has a macOS row) to GITHUB_OUTPUT.
// - notes: prints the release notes, the message of the annotated tag read through the GitHub API
//   (a checkout drops tag annotations) without a trailing signature block. A lightweight tag or an
//   empty message fails with the command that tags correctly.
// - gate-paths: the older releases the release gate updates from to <version>, chosen from the
//   repository's releases read through the GitHub API by the rule of selectGatePaths below; writes
//   `gate` (whether there is any) and `gate-matrix`, one row `{"older": "v<version>"}` per path, to
//   GITHUB_OUTPUT. No path is not a failure: the workflow then skips the gate jobs.
// - preconditions: the signing key variable holds an Ed25519 PKCS #8 PEM key whose public half is in
//   RELEASE_KEYS of apps/orchbee/src/cli/update/signature.rs, and the npm token variable, when named,
//   is not empty.
// - github: creates the draft release of the tag with every top-level file of <artifacts>, or
//   completes the draft a stopped run left; a published release is only confirmed, never changed.
//   Every update feed document must carry its `.sig`, verified with a key of RELEASE_KEYS. The
//   draft is never a pre-release, so publishing it makes it the repository's Latest release even
//   for a beta. Its notes are an Install section linking the files a person downloads, followed by
//   the tag's notes.
// - npm: publishes every tarball of <artifacts>, platform packages first and the main package last,
//   with provenance and the dist-tag `next` for a pre-release or `latest` otherwise. A version the
//   registry already holds with the same tarball is skipped; one with another tarball fails. A
//   pre-release then also takes `latest` while `latest` is missing or an older pre-release, so a
//   plain `npm install` gets the newest beta until a stable version exists.
// - publish: after npm, publishes the tag's draft release, holding exactly the files of
//   <artifacts>, as the repository's Latest release and not a pre-release; a published release is
//   only confirmed.
//
// notes, gate-paths, github and publish run `gh` with the token in GH_TOKEN. <artifacts> is a directory
// `node scripts/release-artifacts.mjs collect --profile release` wrote.
// Exit codes: 0 done; 1 a step failed or a check found a problem; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { createHash, createPrivateKey, createPublicKey } from 'node:crypto';
import { appendFileSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';

import { SEMVER } from './release-version.mjs';
import { FEED_DOMAIN, keyTable, verifySignature } from './sign-release-manifest.mjs';

const repository = fileURLToPath(new URL('..', import.meta.url));
const signatureSource = 'apps/orchbee/src/cli/update/signature.rs';
const registry = 'https://registry.npmjs.org';
const releaseStem = 'orchbee';
const platformPackage = /^@orchbee\/cli-[a-z\d]+-[a-z\d]+$/u;
// The platform token prefixes the release workflow has build, package and smoke jobs for; each
// family's jobs take its rows, and the signing, feed and join jobs take every row.
const releaseFamilies = ['windows', 'macos'];
const usage =
	'Usage: node scripts/release-publish.mjs plan --event <name> --ref <git ref> [--version <version>]\n' +
	'       node scripts/release-publish.mjs notes --tag v<version> --repository <owner/name>\n' +
	'       node scripts/release-publish.mjs gate-paths --version <version> --repository <owner/name>\n' +
	'       node scripts/release-publish.mjs preconditions --signing-key-env <VARIABLE> [--npm-token-env <VARIABLE>]\n' +
	'       node scripts/release-publish.mjs github <artifacts> --tag v<version> --repository <owner/name>\n' +
	'       node scripts/release-publish.mjs npm <artifacts> --version <version> --repository-url <https URL>\n' +
	'       node scripts/release-publish.mjs publish <artifacts> --tag v<version> --repository <owner/name>';

class UsageError extends Error {}
class StepFailure extends Error {}

/** `next` for a version with a pre-release part, `latest` for any other. */
export function distTagFor(version) {
	return version.includes('-') ? 'next' : 'latest';
}

/**
 * What one workflow run releases. A pushed tag publishes; a manual run with a version rehearses.
 * Throws a StepFailure naming the problem for anything else.
 */
export function planRelease({ event, ref, version: requested, platforms }) {
	let version;
	if (event === 'push') {
		const tag = /^refs\/tags\/v(.+)$/u.exec(ref ?? '');
		if (tag === null || !SEMVER.test(tag[1]))
			throw new StepFailure(`A release starts from a pushed tag v<version>, not ${ref}`);
		if (requested !== undefined && requested !== '')
			throw new StepFailure('A tag run takes its version from the tag; give no version');
		version = tag[1];
	} else if (event === 'workflow_dispatch') {
		if (requested === undefined || !SEMVER.test(requested))
			throw new StepFailure(
				`A rehearsal names a version such as 0.1.0-beta.1, without the v, not ${JSON.stringify(requested ?? '')}`,
			);
		version = requested;
	} else {
		throw new StepFailure(`The release path starts from a tag or by hand, not from ${event}`);
	}
	if (!Array.isArray(platforms)) throw new StepFailure('RELEASE_PLATFORMS is not a list of rows');
	const publish = event === 'push';
	// A tag publishes the rows with build and publish; a rehearsal builds and proves every row with
	// build, so a row is rehearsed before its publish flag is set.
	const rows = platforms.filter(
		(row) => row?.build === true && (!publish || row?.publish === true),
	);
	if (rows.length === 0)
		throw new StepFailure(
			publish
				? 'No row of RELEASE_PLATFORMS has both build and publish set'
				: 'No row of RELEASE_PLATFORMS has build set',
		);
	const familyOf = (row) =>
		releaseFamilies.find((family) => String(row.platform ?? '').startsWith(`${family}-`));
	for (const row of rows) {
		if (familyOf(row) === undefined)
			throw new StepFailure(
				`${row.platform} has build set, but the release workflow has no build jobs for it`,
			);
	}
	const rowsOf = (family) => rows.filter((row) => familyOf(row) === family);
	// The Windows jobs take their rows unconditionally, and a job with no row fails.
	if (rowsOf('windows').length === 0)
		throw new StepFailure('The release path needs a Windows row with build set');
	const prerelease = version.includes('-');
	return {
		version,
		tag: `v${version}`,
		publish,
		prerelease,
		distTag: distTagFor(version),
		matrix: { include: rows },
		windowsMatrix: { include: rowsOf('windows') },
		macosMatrix: { include: rowsOf('macos') },
		macos: rowsOf('macos').length > 0,
	};
}

/**
 * The first release whose own automatic helper can install a newer one: slice 1 of
 * proposals/platform/automatic-updates.md#phases. No release below it enters the release gate's
 * window.
 */
const gateFirstVersion = '0.1.0-beta.14';
// The release gate's window: the releases published in the last 90 days.
const gateWindowDays = 90;

/**
 * The older releases the release gate updates from to `version`
 * (proposals/platform/automatic-updates.md#the-release-gate), as tags: the previous release, the one
 * before it, the one in the middle of the window (the older of two middle ones) and the oldest,
 * each counted once, so at most four and every release of a window of four or fewer. The window
 * holds the published releases below `version`, from `firstVersion` on, published within 90 days
 * before `now`; drafts and tags that are not `v<version>` never count. `releases` are
 * `{ tag, draft, publishedAt }` rows as the GitHub API lists them, in any order.
 */
export function selectGatePaths({ releases, version, now, firstVersion = gateFirstVersion }) {
	const since = now.getTime() - gateWindowDays * 24 * 60 * 60 * 1000;
	const window = new Set();
	for (const release of releases) {
		const tag = /^v(.+)$/u.exec(release?.tag ?? '');
		const published = Date.parse(release?.publishedAt ?? '');
		if (
			tag === null ||
			!SEMVER.test(tag[1]) ||
			release.draft !== false ||
			Number.isNaN(published) ||
			published < since ||
			compareVersions(tag[1], firstVersion) < 0 ||
			compareVersions(tag[1], version) >= 0
		)
			continue;
		window.add(tag[1]);
	}
	const newestFirst = [...window].toSorted((left, right) => compareVersions(right, left));
	const count = newestFirst.length;
	if (count === 0) return [];
	// Ascending positions, so the paths list the newest older release first.
	const picks = new Set([0, Math.min(1, count - 1), Math.floor(count / 2), count - 1]);
	return [...picks].map((index) => `v${newestFirst[index]}`);
}

/** The 64-hex public keys of RELEASE_KEYS in the Rust source, or undefined when the table is not found. */
export function releaseTable(source) {
	return keyTable(source, 'RELEASE_KEYS');
}

function shippedReleaseTable() {
	return releaseTable(readFileSync(path.join(repository, ...signatureSource.split('/')), 'utf8'));
}

/** The first 16 hexadecimal digits of SHA-256 over the raw public key, as signatures name a key. */
function keyIdOf(publicHex) {
	return createHash('sha256').update(Buffer.from(publicHex, 'hex')).digest('hex').slice(0, 16);
}

/**
 * Every precondition the release path has before it builds, as sentences that name the missing
 * piece and never a value. None means the run may build.
 */
export function checkPreconditions({ environment, signingKeyVariable, npmTokenVariable, source }) {
	const problems = [];
	const notes = [];
	const pem = environment[signingKeyVariable];
	let publicHex;
	if (pem === undefined || pem.trim() === '') {
		problems.push(
			`${signingKeyVariable} is empty: add the release signing key as the secret ORCHBEE_RELEASE_SIGNING_KEY of the GitHub environment release.`,
		);
	} else {
		try {
			const key = createPrivateKey({ key: pem, format: 'pem' });
			if (key.asymmetricKeyType !== 'ed25519') throw new Error('not Ed25519');
			const spki = createPublicKey(key).export({ format: 'der', type: 'spki' });
			publicHex = spki.subarray(spki.length - 32).toString('hex');
		} catch {
			problems.push(
				`${signingKeyVariable} does not hold an Ed25519 private key in PKCS #8 PEM form.`,
			);
		}
	}
	const table = releaseTable(source);
	if (table === undefined) {
		problems.push(`${signatureSource} has no RELEASE_KEYS table this check can read.`);
	} else if (table.length === 0) {
		problems.push(
			`RELEASE_KEYS in ${signatureSource} is empty: add the public half of the release signing key, or a release build trusts no key and cannot install itself.`,
		);
	} else if (publicHex !== undefined && !table.includes(publicHex)) {
		problems.push(
			`The release signing key (key id ${keyIdOf(publicHex)}) is not in RELEASE_KEYS in ${signatureSource}: add its public half, or a release build refuses its own manifest.`,
		);
	} else if (publicHex !== undefined) {
		notes.push(`The release signing key (key id ${keyIdOf(publicHex)}) is in RELEASE_KEYS.`);
	}
	if (npmTokenVariable !== undefined) {
		const token = environment[npmTokenVariable];
		if (token === undefined || token.trim() === '')
			problems.push(
				`${npmTokenVariable} is empty: add an npm token that may publish orchbee and @orchbee/* as the secret NPM_TOKEN of the GitHub environment release.`,
			);
		else notes.push(`${npmTokenVariable} is set.`);
	}
	return { problems, notes };
}

function fileDigest(filename, algorithm, encoding) {
	return createHash(algorithm).update(readFileSync(filename)).digest(encoding);
}

function byName(left, right) {
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/**
 * The top-level regular files of an artifacts directory with their SHA-256, in name order. Every
 * feed document must carry its signature, and the signature must verify with a key of `table`, so a
 * draft never holds a feed `orchbee update` would refuse.
 */
export function releaseAssets(artifacts, table = shippedReleaseTable()) {
	const names = readdirSync(artifacts, { withFileTypes: true })
		.filter((entry) => entry.isFile())
		.map((entry) => entry.name)
		.toSorted(byName);
	if (!names.includes('SHA256SUMS')) throw new StepFailure(`${artifacts} holds no SHA256SUMS`);
	const problems = [];
	for (const name of names.filter((entry) => entry.endsWith('.feed.json'))) {
		if (!names.includes(`${name}.sig`)) {
			problems.push(
				`${name} has no ${name}.sig: the job release-feed signs it, and its artifact release-feed-signature-<platform> belongs beside the feed.`,
			);
			continue;
		}
		const { outcome } = verifySignature(
			readFileSync(path.join(artifacts, name)),
			readFileSync(path.join(artifacts, `${name}.sig`)),
			table ?? [],
			FEED_DOMAIN,
		);
		if (outcome !== 'trusted')
			problems.push(`${name}.sig does not verify with a key of RELEASE_KEYS (${outcome}).`);
	}
	for (const name of names.filter((entry) => entry.endsWith('.feed.json.sig'))) {
		if (!names.includes(name.slice(0, -'.sig'.length)))
			problems.push(`${name} signs no feed document of this run.`);
	}
	if (problems.length > 0) throw new StepFailure(problems.join('\n'));
	return names.map((name) => ({
		name,
		path: path.join(artifacts, name),
		sha256: fileDigest(path.join(artifacts, name), 'sha256', 'hex'),
	}));
}

/**
 * What to do with the GitHub release of a tag, given the releases that already name it. Only a draft
 * this run can finish is written to; every other difference is a problem.
 */
export function planGithubRelease({ existing, assets }) {
	if (existing.length > 1)
		return {
			action: 'none',
			upload: [],
			problems: [
				`${existing.length} releases name the tag: delete the extra drafts, then run the job again.`,
			],
		};
	if (existing.length === 0)
		return { action: 'create', upload: assets.map((asset) => asset.name), problems: [] };
	const [release] = existing;
	const local = new Map(assets.map((asset) => [asset.name, asset.sha256]));
	const problems = [];
	const present = new Set();
	const state = release.draft ? 'The draft release' : 'The published release';
	for (const asset of release.assets) {
		present.add(asset.name);
		if (!local.has(asset.name)) {
			problems.push(`${state} holds ${asset.name}, which this run did not make.`);
		} else if (typeof asset.digest !== 'string' || !asset.digest.startsWith('sha256:')) {
			problems.push(`${state} reports no SHA-256 for ${asset.name}, so it cannot be confirmed.`);
		} else if (asset.digest.slice('sha256:'.length) !== local.get(asset.name)) {
			problems.push(`${state} holds another ${asset.name} than this run made.`);
		}
	}
	const missing = assets.map((asset) => asset.name).filter((name) => !present.has(name));
	if (!release.draft && missing.length > 0)
		problems.push(
			`The published release lacks ${missing.join(', ')}; a published release is never changed.`,
		);
	if (problems.length > 0) return { action: 'none', upload: [], problems };
	return { action: missing.length > 0 ? 'upload' : 'none', upload: missing, problems };
}

/** The `package.json` inside an npm tarball, read from its `package/package.json` entry. */
export function packedManifest(tarball) {
	const archive = gunzipSync(tarball);
	let offset = 0;
	let longName;
	while (offset + 512 <= archive.length) {
		const header = archive.subarray(offset, offset + 512);
		if (header.every((byte) => byte === 0)) break;
		// A header field ends at its first NUL byte.
		const field = (start, length) => {
			const bytes = header.subarray(start, start + length);
			const end = bytes.indexOf(0);
			return bytes.subarray(0, end === -1 ? bytes.length : end).toString('utf8');
		};
		const size = Number.parseInt(field(124, 12).trim() || '0', 8);
		const type = field(156, 1);
		const prefix = field(345, 155);
		const body = archive.subarray(offset + 512, offset + 512 + size);
		const name = longName ?? (prefix === '' ? field(0, 100) : `${prefix}/${field(0, 100)}`);
		longName = undefined;
		if (type === 'x') {
			// A pax header may carry the next entry's long path.
			const pathRecord = /\d+ path=([^\n]*)\n/u.exec(body.toString('utf8'));
			if (pathRecord !== null) longName = pathRecord[1];
		} else if ((type === '0' || type === '') && name === 'package/package.json') {
			return JSON.parse(body.toString('utf8'));
		}
		offset += 512 + Math.ceil(size / 512) * 512;
	}
	throw new StepFailure('The tarball holds no package/package.json');
}

/** `https://host/owner/name` from a package `repository` URL in any of npm's spellings. */
function normalRepositoryUrl(value) {
	return String(value ?? '')
		.replace(/^git\+/u, '')
		.replace(/\.git$/u, '')
		.replace(/\/+$/u, '')
		.toLowerCase();
}

/**
 * The tarballs in publication order, platform packages before the main package, after checking that
 * they are exactly one release of `version`: the release names, one version, the main package listing
 * exactly the platform packages, the dist-tag rule, and the repository provenance will name.
 */
export function npmPublicationOrder(packages, { version, repositoryUrl }) {
	const problems = [];
	const tag = distTagFor(version);
	for (const { file, manifest } of packages) {
		if (manifest.version !== version)
			problems.push(`${file} is version ${manifest.version}, not ${version}.`);
		if (manifest.publishConfig?.tag !== tag)
			problems.push(`${file} would publish under ${manifest.publishConfig?.tag}, not ${tag}.`);
		if (normalRepositoryUrl(manifest.repository?.url) !== normalRepositoryUrl(repositoryUrl))
			problems.push(
				`${file} names the repository ${manifest.repository?.url ?? 'nowhere'}, but provenance will name ${repositoryUrl}.`,
			);
	}
	const mains = packages.filter(({ manifest }) => manifest.bin !== undefined);
	const platforms = packages.filter(({ manifest }) => manifest.bin === undefined);
	if (mains.length !== 1 || mains[0].manifest.name !== releaseStem)
		problems.push(`The artifacts must hold exactly one main package named ${releaseStem}.`);
	for (const { file, manifest } of platforms) {
		if (!platformPackage.test(manifest.name ?? ''))
			problems.push(`${file} is ${manifest.name}, not a release platform package.`);
	}
	if (platforms.length === 0) problems.push('The artifacts hold no platform package.');
	if (mains.length === 1) {
		const listed = mains[0].manifest.optionalDependencies ?? {};
		const expected = Object.fromEntries(
			platforms.map(({ manifest }) => [manifest.name, version]).toSorted(),
		);
		const sorted = Object.fromEntries(Object.entries(listed).toSorted());
		if (JSON.stringify(sorted) !== JSON.stringify(expected))
			problems.push(
				`${releaseStem} lists ${JSON.stringify(listed)} as optional dependencies, not exactly the platform packages beside it.`,
			);
	}
	if (problems.length > 0) throw new StepFailure(problems.join('\n'));
	return [
		...platforms.toSorted((left, right) => (left.manifest.name < right.manifest.name ? -1 : 1)),
		mains[0],
	];
}

/**
 * `publish`, `skip`, or a problem, from the registry's answer about one name and version: the
 * integrity of the tarball it already holds, or undefined when it holds none.
 */
export function npmDecision({ name, version, local, published }) {
	if (published === undefined) return { action: 'publish' };
	if (published === local) return { action: 'skip' };
	return {
		action: 'fail',
		problem: `${name}@${version} is already on npm with another tarball. A published version is never replaced; release the next version.`,
	};
}

/** The registry's document for the package `name`, or undefined when the registry has none. */
async function registryDocument(name, fetchImpl) {
	const response = await fetchImpl(`${registry}/${name.replace('/', '%2f')}`, {
		headers: { accept: 'application/json', 'cache-control': 'no-cache' },
	});
	if (response.status === 404) return undefined;
	if (!response.ok)
		throw new StepFailure(`The npm registry answered ${response.status} for ${name}`);
	return response.json();
}

/** Numeric identifiers compare as numbers and rank below alphanumeric ones, which compare as text. */
function compareIdentifiers(left, right) {
	const leftNumeric = /^\d+$/u.test(left);
	const rightNumeric = /^\d+$/u.test(right);
	if (leftNumeric && rightNumeric) return Number(left) - Number(right);
	if (leftNumeric !== rightNumeric) return leftNumeric ? -1 : 1;
	if (left === right) return 0;
	return left < right ? -1 : 1;
}

/** A version's three numbers and its pre-release identifiers, empty for a stable version. */
function versionParts(version) {
	const dash = version.indexOf('-');
	return {
		core: (dash === -1 ? version : version.slice(0, dash)).split('.').map(Number),
		pre: dash === -1 ? [] : version.slice(dash + 1).split('.'),
	};
}

/** Semantic Versioning precedence of two versions without build metadata: below, at or above zero. */
function compareVersions(left, right) {
	const a = versionParts(left);
	const b = versionParts(right);
	for (let index = 0; index < 3; index += 1)
		if (a.core[index] !== b.core[index]) return a.core[index] - b.core[index];
	// A version without a pre-release part ranks above every pre-release of the same core.
	if (a.pre.length === 0 || b.pre.length === 0) return b.pre.length - a.pre.length;
	for (let index = 0; index < Math.min(a.pre.length, b.pre.length); index += 1) {
		const order = compareIdentifiers(a.pre[index], b.pre[index]);
		if (order !== 0) return order;
	}
	return a.pre.length - b.pre.length;
}

/**
 * Whether the pre-release `version` also takes the dist-tag `latest`, which names `current` now: while
 * `latest` is missing or an older pre-release. Until a stable version exists a plain `npm install`
 * then gets the newest beta, and a job run again for an older beta never moves `latest` back.
 */
function takesLatest(version, current) {
	if (!version.includes('-')) return false;
	if (current === undefined) return true;
	return current.includes('-') && compareVersions(current, version) < 0;
}

/** The integrity the registry records for `name@version`, or undefined when it has none. */
export async function registryIntegrity(name, version, fetchImpl = fetch) {
	const document = await registryDocument(name, fetchImpl);
	const entry = document?.versions?.[version];
	if (entry === undefined) return undefined;
	const integrity = entry?.dist?.integrity;
	if (typeof integrity !== 'string')
		throw new StepFailure(`The npm registry records no integrity for ${name}@${version}`);
	return integrity;
}

function log(line) {
	process.stdout.write(`${line}\n`);
}

/** Runs a program with captured output; a start failure or a nonzero exit fails the step. */
function run(program, argv) {
	const result = spawnSync(program, argv, {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		maxBuffer: 64 * 1024 * 1024,
	});
	if (result.error !== undefined) throw new StepFailure(`${program}: ${result.error.message}`);
	if (result.status !== 0)
		throw new StepFailure(`${program} ${argv[0]} exited ${result.status}\n${result.stderr}`);
	return result;
}

/** The releases of `repositoryName` whose tag is `tag`, drafts included. */
export function githubReleases(tag, repositoryName, gh = (argv) => run('gh', argv)) {
	const result = gh([
		'api',
		'--paginate',
		`repos/${repositoryName}/releases?per_page=100`,
		'--jq',
		`.[] | select(.tag_name == ${JSON.stringify(tag)}) | {id, draft, assets: [.assets[] | {name, digest}]}`,
	]);
	return result.stdout
		.split('\n')
		.filter((line) => line.trim() !== '')
		.map((line) => JSON.parse(line));
}

/** Every release of `repositoryName` as `{ tag, draft, publishedAt }`, drafts included. */
function githubReleaseList(repositoryName, gh = (argv) => run('gh', argv)) {
	const result = gh([
		'api',
		'--paginate',
		`repos/${repositoryName}/releases?per_page=100`,
		'--jq',
		'.[] | {tag: .tag_name, draft, publishedAt: .published_at}',
	]);
	return result.stdout
		.split('\n')
		.filter((line) => line.trim() !== '')
		.map((line) => JSON.parse(line));
}

/** The one sentence a run without usable release notes stops with. */
function untaggedNotes(tag, why) {
	return `${tag} ${why}: delete the tag, then tag again with \`git tag -a ${tag} --cleanup=verbatim -F <notes file>\`, whose message becomes the release notes.`;
}

// git ends a signed tag's message with the signature block, starting at the last line that opens one.
const signatureStart =
	/^-----BEGIN (?:PGP SIGNATURE|PGP MESSAGE|SSH SIGNATURE|SIGNED MESSAGE)-----$/u;

/**
 * The release notes of `tag`: its annotated tag's message from the GitHub API, without a trailing
 * signature block or surrounding blank lines. A lightweight tag or an empty message is a failure.
 */
export function tagNotes(tag, repositoryName, gh = (argv) => run('gh', argv)) {
	const reference = JSON.parse(
		gh(['api', `repos/${repositoryName}/git/ref/tags/${tag}`, '--jq', '.object']).stdout,
	);
	if (reference.type !== 'tag')
		throw new StepFailure(untaggedNotes(tag, 'is a lightweight tag, which carries no notes'));
	const message = gh([
		'api',
		`repos/${repositoryName}/git/tags/${reference.sha}`,
		'--jq',
		'.message',
	]).stdout;
	const lines = message.split('\n');
	const start = lines.findLastIndex((line) => signatureStart.test(line.trimEnd()));
	const notes = (start === -1 ? lines : lines.slice(0, start))
		.join('\n')
		.replace(/^(?:[ \t]*\n)+/u, '')
		.trimEnd();
	if (notes === '') throw new StepFailure(untaggedNotes(tag, 'has an empty message'));
	return notes;
}

/**
 * The notes of a new draft release: an Install section linking the files a person downloads, then
 * the tag's notes.
 */
function releaseNotes({ version, tag, repositoryName, names, changes }) {
	const link = (pattern) => {
		const name = names.find((candidate) => pattern.test(candidate));
		if (name === undefined) return undefined;
		const address = `https://github.com/${repositoryName}/releases/download/${tag}/${encodeURIComponent(name)}`;
		return `[${name}](${address})`;
	};
	const installer = link(/-windows-x86_64-setup\.exe$/u);
	const diskImages = [
		['macOS Apple silicon', link(/-macos-aarch64\.dmg$/u)],
		['macOS Intel', link(/-macos-x86_64\.dmg$/u)],
	].filter(([, address]) => address !== undefined);
	// The runtime archives; the window archive `orchbee-desktop-<version>-…` serves updates only.
	const archives = [
		link(/^orchbee-\d[^/]*-windows-x86_64\.zip$/u),
		link(/^orchbee-\d[^/]*-macos-aarch64\.tar\.gz$/u),
		link(/^orchbee-\d[^/]*-macos-x86_64\.tar\.gz$/u),
	].filter((address) => address !== undefined);
	const npm = `\`npm install -g orchbee@${distTagFor(version)}\``;
	return [
		'## Install',
		'',
		...(installer === undefined
			? []
			: [`- Windows x64 installer, with the window and the command line: ${installer}`]),
		...diskImages.map(
			([label, address]) =>
				`- ${label} disk image, with the window and the command line: ${address}`,
		),
		`- Command line only: ${archives.length === 0 ? npm : `${archives.join(', ')}, or ${npm}`}`,
		// Only Windows release builds update themselves so far.
		diskImages.length === 0
			? '- Already installed: `orchbee update`'
			: '- Already installed on Windows: `orchbee update`',
		'',
		'The `.feed.json`, `.sig`, `orchbee-desktop-*.zip` and `.tgz` files serve updates and npm; `SHA256SUMS` holds the checksums.',
		'',
		changes,
	].join('\n');
}

function publishGithub({ artifacts, tag, repositoryName }, gh = (argv) => run('gh', argv)) {
	const version = tag.slice(1);
	const assets = releaseAssets(artifacts);
	const plan = planGithubRelease({ existing: githubReleases(tag, repositoryName, gh), assets });
	if (plan.problems.length > 0) throw new StepFailure(plan.problems.join('\n'));
	const files = (names) => names.map((name) => assets.find((asset) => asset.name === name).path);
	if (plan.action === 'create') {
		const names = assets.map((asset) => asset.name);
		const changes = tagNotes(tag, repositoryName, gh);
		const notes = releaseNotes({ version, tag, repositoryName, names, changes });
		gh([
			'release',
			'create',
			tag,
			'--repo',
			repositoryName,
			'--draft',
			'--verify-tag',
			'--title',
			tag,
			'--notes',
			notes,
			...files(plan.upload),
		]);
		log(`Created the draft release ${tag} with ${plan.upload.join(', ')}.`);
	} else if (plan.action === 'upload') {
		// Without --clobber, so an asset that appeared meanwhile is never replaced.
		gh(['release', 'upload', tag, '--repo', repositoryName, ...files(plan.upload)]);
		log(`Completed the draft release ${tag} with ${plan.upload.join(', ')}.`);
	} else {
		log(`The release ${tag} already holds every asset of this run; nothing changed.`);
	}
}

function publishRelease({ artifacts, tag, repositoryName }, gh = (argv) => run('gh', argv)) {
	const existing = githubReleases(tag, repositoryName, gh);
	const plan = planGithubRelease({ existing, assets: releaseAssets(artifacts) });
	if (plan.action === 'create')
		throw new StepFailure(
			`No release names ${tag}: run the publication job again, which creates its draft first.`,
		);
	if (plan.problems.length > 0) throw new StepFailure(plan.problems.join('\n'));
	if (plan.action !== 'none')
		throw new StepFailure(
			`The draft release ${tag} lacks ${plan.upload.join(', ')}: run the publication job again, which completes it first.`,
		);
	const [release] = existing;
	if (!release.draft) {
		log(`The release ${tag} is already published; nothing changed.`);
		return;
	}
	gh([
		'api',
		'--method',
		'PATCH',
		`repos/${repositoryName}/releases/${release.id}`,
		'-F',
		'draft=false',
		'-F',
		'prerelease=false',
		'-f',
		'make_latest=true',
		'--silent',
	]);
	log(`Published the release ${tag} as the Latest release.`);
}

async function publishNpm({ artifacts, version, repositoryUrl }) {
	const packages = readdirSync(artifacts, { withFileTypes: true })
		.filter((entry) => entry.isFile() && entry.name.endsWith('.tgz'))
		.map((entry) => {
			const file = path.join(artifacts, entry.name);
			const tarball = readFileSync(file);
			return {
				file: entry.name,
				path: file,
				manifest: packedManifest(tarball),
				integrity: `sha512-${createHash('sha512').update(tarball).digest('base64')}`,
			};
		});
	const ordered = npmPublicationOrder(packages, { version, repositoryUrl });
	const tag = distTagFor(version);
	for (const entry of ordered) {
		const { name } = entry.manifest;
		const decision = npmDecision({
			name,
			version,
			local: entry.integrity,
			published: await registryIntegrity(name, version),
		});
		if (decision.action === 'fail') throw new StepFailure(decision.problem);
		if (decision.action === 'skip') {
			log(`${name}@${version} is already on npm with this tarball; skipped.`);
			continue;
		}
		log(`> npm publish ${entry.file} --tag ${tag} --access public --provenance`);
		// The credential comes from npm's own configuration; nothing here reads or passes it.
		const result = spawnSync(
			'npm',
			['publish', entry.path, '--tag', tag, '--access', 'public', '--provenance'],
			{ stdio: ['ignore', 'inherit', 'inherit'] },
		);
		if (result.error !== undefined) throw new StepFailure(`npm: ${result.error.message}`);
		if (result.status !== 0)
			throw new StepFailure(`npm publish ${entry.file} exited ${result.status}`);
		log(`Published ${name}@${version} under ${tag}.`);
	}
	// `latest` moves in the publication order, platform packages first and the main package last.
	const currents = await Promise.all(
		ordered.map(async ({ manifest }) => {
			const value = (await registryDocument(manifest.name, fetch))?.['dist-tags']?.latest;
			return typeof value === 'string' ? value : undefined;
		}),
	);
	for (const [index, { manifest }] of ordered.entries()) {
		const { name } = manifest;
		const current = currents[index];
		if (!takesLatest(version, current)) {
			if (version.includes('-')) log(`${name} keeps latest at ${current}.`);
			continue;
		}
		log(`> npm dist-tag add ${name}@${version} latest`);
		const result = spawnSync('npm', ['dist-tag', 'add', `${name}@${version}`, 'latest'], {
			stdio: ['ignore', 'inherit', 'inherit'],
		});
		if (result.error !== undefined) throw new StepFailure(`npm: ${result.error.message}`);
		if (result.status !== 0)
			throw new StepFailure(`npm dist-tag add ${name}@${version} latest exited ${result.status}`);
		log(`Moved latest of ${name} to ${version}.`);
	}
}

function writeOutputs(values) {
	const lines = Object.entries(values).map(([key, value]) => `${key}=${value}\n`);
	const target = process.env.GITHUB_OUTPUT;
	if (target === undefined || target === '') process.stdout.write(lines.join(''));
	else appendFileSync(target, lines.join(''));
}

function parseOptions(rest, allowed) {
	const positional = [];
	const options = {};
	for (let index = 0; index < rest.length; index++) {
		const argument = rest[index];
		if (!argument.startsWith('--')) {
			positional.push(argument);
			continue;
		}
		const key = argument.slice(2);
		const value = rest[++index];
		if (!allowed.includes(key) || value === undefined || options[key] !== undefined)
			throw new UsageError(usage);
		options[key] = value;
	}
	return { positional, options };
}

async function main(argv) {
	const [command, ...rest] = argv;
	if (command === 'plan') {
		const { positional, options } = parseOptions(rest, ['event', 'ref', 'version']);
		if (positional.length > 0 || options.event === undefined || options.ref === undefined)
			throw new UsageError(usage);
		let platforms;
		try {
			platforms = JSON.parse(process.env.RELEASE_PLATFORMS ?? '');
		} catch {
			throw new StepFailure('RELEASE_PLATFORMS is not JSON');
		}
		const plan = planRelease({ ...options, platforms });
		log(
			`${plan.publish ? 'Release' : 'Rehearsal'} of ${plan.version}: ${plan.matrix.include.map((row) => row.platform).join(', ')}; npm dist-tag ${plan.distTag}.`,
		);
		writeOutputs({
			version: plan.version,
			tag: plan.tag,
			publish: String(plan.publish),
			prerelease: String(plan.prerelease),
			'dist-tag': plan.distTag,
			matrix: JSON.stringify(plan.matrix),
			'windows-matrix': JSON.stringify(plan.windowsMatrix),
			'macos-matrix': JSON.stringify(plan.macosMatrix),
			macos: String(plan.macos),
		});
		return;
	}
	if (command === 'gate-paths') {
		const { positional, options } = parseOptions(rest, ['version', 'repository']);
		if (
			positional.length > 0 ||
			!SEMVER.test(options.version ?? '') ||
			!/^[\w.-]+\/[\w.-]+$/u.test(options.repository ?? '')
		)
			throw new UsageError(usage);
		const paths = selectGatePaths({
			releases: githubReleaseList(options.repository),
			version: options.version,
			now: new Date(),
		});
		log(
			paths.length > 0
				? `The release gate of ${options.version} updates from ${paths.join(', ')}.`
				: `The release gate of ${options.version} runs no path: no release from ${gateFirstVersion} on was published in the last ${gateWindowDays} days.`,
		);
		writeOutputs({
			gate: String(paths.length > 0),
			'gate-matrix': JSON.stringify({ include: paths.map((older) => ({ older })) }),
		});
		return;
	}
	if (command === 'preconditions') {
		const { positional, options } = parseOptions(rest, ['signing-key-env', 'npm-token-env']);
		const variable = /^[A-Z][A-Z\d_]*$/u;
		if (
			positional.length > 0 ||
			!variable.test(options['signing-key-env'] ?? '') ||
			(options['npm-token-env'] !== undefined && !variable.test(options['npm-token-env']))
		)
			throw new UsageError(usage);
		const { problems, notes } = checkPreconditions({
			environment: process.env,
			signingKeyVariable: options['signing-key-env'],
			npmTokenVariable: options['npm-token-env'],
			source: readFileSync(path.join(repository, ...signatureSource.split('/')), 'utf8'),
		});
		for (const note of notes) log(note);
		if (problems.length > 0) throw new StepFailure(problems.join('\n'));
		return;
	}
	if (command === 'notes' || command === 'github' || command === 'publish') {
		const { positional, options } = parseOptions(rest, ['tag', 'repository']);
		const tag = /^v(.+)$/u.exec(options.tag ?? '');
		if (
			positional.length !== (command === 'notes' ? 0 : 1) ||
			tag === null ||
			!SEMVER.test(tag[1]) ||
			!/^[\w.-]+\/[\w.-]+$/u.test(options.repository ?? '')
		)
			throw new UsageError(usage);
		if (command === 'notes') {
			log(`The release notes of ${options.tag}:\n${tagNotes(options.tag, options.repository)}`);
			return;
		}
		const step = command === 'github' ? publishGithub : publishRelease;
		step({
			artifacts: path.resolve(positional[0]),
			tag: options.tag,
			repositoryName: options.repository,
		});
		return;
	}
	if (command === 'npm') {
		const { positional, options } = parseOptions(rest, ['version', 'repository-url']);
		if (
			positional.length !== 1 ||
			!SEMVER.test(options.version ?? '') ||
			!/^https:\/\/\S+$/u.test(options['repository-url'] ?? '')
		)
			throw new UsageError(usage);
		await publishNpm({
			artifacts: path.resolve(positional[0]),
			version: options.version,
			repositoryUrl: options['repository-url'],
		});
		return;
	}
	throw new UsageError(usage);
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		await main(process.argv.slice(2));
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
