// The steps of the release workflow around the build: what a run releases, whether its secrets and
// trusted key are in place, and the publication of the collected artifacts as a draft GitHub
// pre-release and on npm. Each step either finishes or stops with sentences that name what is wrong;
// none prints a secret, and a step run again after a partial publication finishes it without
// duplicating or replacing anything already published.
//
//   node scripts/release-publish.mjs plan --event <name> --ref <git ref> [--version <version>]
//   node scripts/release-publish.mjs preconditions --signing-key-env <VARIABLE> [--npm-token-env <VARIABLE>]
//   node scripts/release-publish.mjs github <artifacts> --tag v<version> --repository <owner/name>
//   node scripts/release-publish.mjs npm <artifacts> --version <version> --repository-url <https URL>
//
// - plan: a pushed tag `v<version>` releases and publishes that version; a manual run names the
//   version to rehearse and publishes nothing. Reads the platform rows from RELEASE_PLATFORMS and
//   writes `version`, `tag`, `publish`, `prerelease`, `dist-tag` and `matrix` to GITHUB_OUTPUT.
// - preconditions: the signing key variable holds an Ed25519 PKCS #8 PEM key whose public half is in
//   RELEASE_KEYS of apps/orchbee/src/cli/update/signature.rs, and the npm token variable, when named,
//   is not empty.
// - github: creates the draft release of the tag with every top-level file of <artifacts>, or
//   completes the draft a stopped run left; a published release is only confirmed, never changed.
//   Runs `gh` with the token in GH_TOKEN.
// - npm: publishes every tarball of <artifacts>, platform packages first and the main package last,
//   with provenance and the dist-tag `next` for a pre-release or `latest` otherwise. A version the
//   registry already holds with the same tarball is skipped; one with another tarball fails.
//
// <artifacts> is a directory `node scripts/release-artifacts.mjs collect --profile release` wrote.
// Exit codes: 0 done; 1 a step failed or a check found a problem; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { createHash, createPrivateKey, createPublicKey } from 'node:crypto';
import { appendFileSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';

import { SEMVER } from './release-version.mjs';

const repository = fileURLToPath(new URL('..', import.meta.url));
const signatureSource = 'apps/orchbee/src/cli/update/signature.rs';
const registry = 'https://registry.npmjs.org';
const releaseStem = 'orchbee';
const platformPackage = /^@orchbee\/cli-[a-z\d]+-[a-z\d]+$/u;
const usage =
	'Usage: node scripts/release-publish.mjs plan --event <name> --ref <git ref> [--version <version>]\n' +
	'       node scripts/release-publish.mjs preconditions --signing-key-env <VARIABLE> [--npm-token-env <VARIABLE>]\n' +
	'       node scripts/release-publish.mjs github <artifacts> --tag v<version> --repository <owner/name>\n' +
	'       node scripts/release-publish.mjs npm <artifacts> --version <version> --repository-url <https URL>';

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
	const rows = platforms.filter((row) => row?.build === true && row?.publish === true);
	if (rows.length === 0)
		throw new StepFailure('No row of RELEASE_PLATFORMS has both build and publish set');
	// One main npm package must list every platform package of its version, and each row's collect
	// writes its own; combining rows is not built yet.
	if (rows.length > 1)
		throw new StepFailure(
			`The release path publishes one platform row today, and ${rows.length} rows have build and publish set`,
		);
	const prerelease = version.includes('-');
	return {
		version,
		tag: `v${version}`,
		publish: event === 'push',
		prerelease,
		distTag: distTagFor(version),
		matrix: { include: rows },
	};
}

/** The 64-hex public keys of RELEASE_KEYS in the Rust source, or undefined when the table is not found. */
export function releaseTable(source) {
	const table = /const RELEASE_KEYS: &\[TrustedKey\] = &\[([\s\S]*?)\];/u.exec(source);
	if (table === null) return undefined;
	const body = table[1].replace(/\/\/[^\n]*/gu, '');
	return [...body.matchAll(/TrustedKey::from_hex\(\s*"([0-9a-f]{64})"\s*,?\s*\)/gu)].map(
		(match) => match[1],
	);
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

/** The top-level regular files of an artifacts directory with their SHA-256, in name order. */
export function releaseAssets(artifacts) {
	const names = readdirSync(artifacts, { withFileTypes: true })
		.filter((entry) => entry.isFile())
		.map((entry) => entry.name)
		.toSorted(byName);
	if (!names.includes('SHA256SUMS')) throw new StepFailure(`${artifacts} holds no SHA256SUMS`);
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

/** The integrity the registry records for `name@version`, or undefined when it has none. */
export async function registryIntegrity(name, version, fetchImpl = fetch) {
	const response = await fetchImpl(`${registry}/${name.replace('/', '%2f')}`, {
		headers: { accept: 'application/json', 'cache-control': 'no-cache' },
	});
	if (response.status === 404) return undefined;
	if (!response.ok)
		throw new StepFailure(`The npm registry answered ${response.status} for ${name}`);
	const document = await response.json();
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
		`.[] | select(.tag_name == ${JSON.stringify(tag)}) | {draft, assets: [.assets[] | {name, digest}]}`,
	]);
	return result.stdout
		.split('\n')
		.filter((line) => line.trim() !== '')
		.map((line) => JSON.parse(line));
}

function publishGithub({ artifacts, tag, repositoryName }, gh = (argv) => run('gh', argv)) {
	const version = tag.slice(1);
	const assets = releaseAssets(artifacts);
	const plan = planGithubRelease({ existing: githubReleases(tag, repositoryName, gh), assets });
	if (plan.problems.length > 0) throw new StepFailure(plan.problems.join('\n'));
	const files = (names) => names.map((name) => assets.find((asset) => asset.name === name).path);
	if (plan.action === 'create') {
		const notes =
			`Orchbee ${version}. Draft made by the release workflow: write the notes, check the ` +
			'assets against SHA256SUMS, then publish.';
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
			...(version.includes('-') ? ['--prerelease'] : []),
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
	if (command === 'github') {
		const { positional, options } = parseOptions(rest, ['tag', 'repository']);
		const tag = /^v(.+)$/u.exec(options.tag ?? '');
		if (
			positional.length !== 1 ||
			tag === null ||
			!SEMVER.test(tag[1]) ||
			!/^[\w.-]+\/[\w.-]+$/u.test(options.repository ?? '')
		)
			throw new UsageError(usage);
		publishGithub({
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
