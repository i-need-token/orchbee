// The update check after publication, shared by the Windows and macOS release smoke scripts, and
// the published-release reads the Windows release gate also uses. A published release can be updated
// to when the version people have now, installed from its own published archive, updates to it
// through its own `update` reading the release's feed asset. Each platform script owns the argument
// parsing of its other steps, its exit codes and its work folder, and passes a port that says where
// that platform installs and how its programs run:
//
// - platform: the release platform, whose feed, tracked update pair and archive format apply;
// - tar: the system tar, which unpacks either archive format;
// - client(directory, stem): the Client program of an unpacked release directory;
// - environment(work): the environment every install and update runs with, without
//   ORCHBEE_UPDATE_FEED;
// - storeRoot(stem, env): the store root `install` fills under `env`;
// - updater(storeRoot, stem, env): the command a person runs for `update`;
// - expectActive(storeRoot, stem, version, env): fails unless the store runs `version`.
//
// The version people have now is the one the website's tracked update pair of the platform names
// (apps/website/public/update/stable/<platform>/, or the folder `--current` names), whose signature
// must verify. Its archive, fetched from the address that pair signs and checked against the signed
// size and SHA-256, is unpacked and installed by its own `install`; then the updater runs `update`
// with `ORCHBEE_UPDATE_FEED` naming the release's `<stem>-<version>-<platform>.feed.json` in the
// assets folder, and must activate the release. Without a tracked pair, or when the pair already
// names the release, no older version can update: the release is installed from its own archive and
// its `update` fills an empty scratch store (`ORCHBEE_INSTALL_DIR`) beside the store root, which
// still proves feed, download, unpacking, publication and activation. Every request is an anonymous
// GET with the system curl. The assets address is the release's download folder over HTTPS; only a
// test build's stem admits `http://127.0.0.1:<port>/…/`, a loopback server standing in for it,
// together with feeds that name their archive by a path relative to that address.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { readRelease } from './package-npm.mjs';
import { absoluteArchive, FEED_LIMIT, feedName, readFeed, validVersion } from './release-feed.mjs';
import { FEED_DOMAIN, keyTable, verifySignature } from './sign-release-manifest.mjs';
import { curl, fileFacts } from './website-release.mjs';

// An install or an update gets this long.
const setupDeadline = 600_000;
const signatureSource = fileURLToPath(
	new URL('../apps/orchbee/src/cli/update/signature.rs', import.meta.url),
);
// The updater's bound on a signature document.
export const SIGNATURE_LIMIT = 4 * 1024;
const updateStem = /^(?:orchbee|ob-test-[0-9a-f]{16})$/u;
const loopbackAssets = /^http:\/\/127\.0\.0\.1:\d{1,5}\/(?:[^\s?#]*\/)?$/u;
const httpsAssets = /^https:\/\/[^\s?#]+\/$/u;

export class UsageError extends Error {}
export class StepFailure extends Error {}

function log(line) {
	process.stdout.write(`${line}\n`);
}

/** The website's tracked update pair of `platform`'s stable channel. */
export function trackedPair(platform) {
	return fileURLToPath(
		new URL(`../apps/website/public/update/stable/${platform}/`, import.meta.url),
	);
}

/** Whether `stem` names a build the update check and the release gate admit. */
export function admittedStem(stem) {
	return updateStem.test(stem);
}

/**
 * The options of `update v<version> --release <assets address> --work <absolute new directory>
 * [--stem <stem>] [--current <absolute folder>]`, given without the step's name.
 */
export function parseUpdateArguments(argv, usage, platform) {
	const [tag, ...rest] = argv;
	const options = {};
	for (let index = 0; index < rest.length; index += 2) {
		const [option, value] = [rest[index], rest[index + 1]];
		if (!['--release', '--work', '--stem', '--current'].includes(option) || value === undefined)
			throw new UsageError(usage);
		options[option.slice(2)] = value;
	}
	const version = tag?.startsWith('v') ? tag.slice(1) : undefined;
	const stem = options.stem ?? 'orchbee';
	if (
		!validVersion(version) ||
		options.release === undefined ||
		options.work === undefined ||
		!path.isAbsolute(options.work) ||
		!admittedStem(stem) ||
		(options.current !== undefined && !path.isAbsolute(options.current))
	)
		throw new UsageError(usage);
	// Before the caller creates the work folder.
	assetsSource(options.release, stem, usage);
	return {
		step: 'update',
		version,
		stem,
		release: options.release,
		work: options.work,
		current: options.current ?? trackedPair(platform),
	};
}

/** Where the check's requests go: the release's download folder, or a loopback stand-in. */
export function assetsSource(address, stem, usage) {
	if (loopbackAssets.test(address)) {
		if (stem === 'orchbee')
			throw new UsageError('Only a test build reads a loopback stand-in for the release assets');
		return { kind: 'loopback', base: address };
	}
	if (httpsAssets.test(address)) return { kind: 'https', base: address };
	throw new UsageError(usage);
}

/** Downloads `url` into `output`, at most `limit` bytes; `missing` says what a 404 means. */
export function fetchAsset(source, url, output, limit, missing) {
	const { status, tooLarge } = curl(source, url, { output, limit });
	if (tooLarge) throw new StepFailure(`${url} is longer than ${limit} bytes`);
	if (status === 404) throw new StepFailure(missing);
	if (status !== 200) throw new StepFailure(`${url} answered ${status}`);
}

/** The archive `feed` names, downloaded into `work` and checked against its signed size and SHA-256. */
export function fetchArchive(source, feed, work, name) {
	const { format, url, size, sha256: digest } = feed.archive;
	// A release names its archive absolutely; a stand-in's lies beside its assets.
	if (absoluteArchive(url) === (source.kind === 'loopback'))
		throw new StepFailure(
			`The feed of ${feed.version} names its archive by ${url}, which a ${source.kind === 'loopback' ? 'stand-in' : 'release'} check does not fetch`,
		);
	const address = new URL(url, source.base).href;
	const output = path.join(work, `${name}.${format}`);
	fetchAsset(
		source,
		address,
		output,
		size,
		`The archive of ${feed.version} is not public at ${address}`,
	);
	const facts = fileFacts(output);
	if (facts.size !== size || facts.sha256 !== digest)
		throw new StepFailure(
			`The archive of ${feed.version} at ${address} does not match the size and SHA-256 its feed signs`,
		);
	log(`Fetched ${address}: ${size} bytes, SHA-256 as signed`);
	return output;
}

/** Unpacks `archive` with the port's tar into `work/<name>` and reads its one release directory. */
export function unpackRelease(port, archive, work, name, version) {
	const folder = path.join(work, name);
	mkdirSync(folder);
	log(`> ${port.tar} -x -f ${archive} -C ${folder}`);
	const result = spawnSync(port.tar, ['-x', '-f', archive, '-C', folder], {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		timeout: setupDeadline,
		windowsHide: true,
	});
	if (result.error !== undefined) throw new StepFailure(`${port.tar}: ${result.error.message}`);
	process.stdout.write(`${result.stdout ?? ''}${result.stderr ?? ''}`);
	if (result.status !== 0)
		throw new StepFailure(`tar -x ${path.basename(archive)} exited ${result.status}, expected 0`);
	const entries = readdirSync(folder, { withFileTypes: true });
	if (entries.length !== 1 || !entries[0].isDirectory())
		throw new StepFailure(`${path.basename(archive)} does not hold exactly one release directory`);
	const directory = path.join(folder, entries[0].name);
	return { directory, ...readRelease(directory, version) };
}

/** Runs `program <argv> --format json`; `document` is the parsed output, or null. */
export function runDocument(program, argv, env) {
	log(`> ${program} ${argv.join(' ')} --format json`);
	const result = spawnSync(program, [...argv, '--format', 'json'], {
		encoding: 'utf8',
		env,
		stdio: ['ignore', 'pipe', 'pipe'],
		timeout: setupDeadline,
		maxBuffer: 64 * 1024 * 1024,
		windowsHide: true,
	});
	if (result.error !== undefined)
		throw new StepFailure(`${program} ${argv.join(' ')}: ${result.error.message}`);
	if (result.stdout) process.stdout.write(result.stdout);
	if (result.stderr) process.stdout.write(result.stderr);
	const ran = { status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
	try {
		return { ...ran, document: JSON.parse(ran.stdout) };
	} catch {
		return { ...ran, document: null };
	}
}

/** What a failed `install` or `update` document says, for the failure sentence. */
function documentFailure(what, ran) {
	const result = ran.document?.result ?? {};
	const facts = ['error', 'phase', 'activated', 'upToDate', 'releaseVersion']
		.filter((member) => member in result)
		.map((member) => `${member} ${JSON.stringify(result[member])}`);
	return `${what} exited ${ran.status}${facts.length > 0 ? ` (${facts.join(', ')})` : ''}`;
}

/**
 * Installs an unpacked release directory of the port's platform into a store root that does not
 * exist yet.
 */
export function installRelease(port, release, storeRoot, env) {
	if (release.manifest.platform !== port.platform)
		throw new StepFailure(`${release.directory} is a ${release.manifest.platform} release`);
	if (existsSync(storeRoot))
		throw new StepFailure(`Refusing to install into an existing ${storeRoot}`);
	const installed = runDocument(port.client(release.directory, release.stem), ['install'], env);
	if (installed.status !== 0 || installed.document?.result?.activated !== true)
		throw new StepFailure(documentFailure('install', installed));
	port.expectActive(storeRoot, release.stem, release.manifest.version, env);
}

/**
 * The feed document `feed` of `platform` with its signature `signature`, verified with the key table
 * of `stem`'s build and read; `what` names the pair in a failure.
 */
export function verifiedFeed(feed, signature, stem, platform, what) {
	if (feed.length > FEED_LIMIT || signature.length > SIGNATURE_LIMIT)
		throw new StepFailure(`${what} exceeds the updater's bounds`);
	const table = keyTable(
		readFileSync(signatureSource, 'utf8'),
		stem === 'orchbee' ? 'RELEASE_KEYS' : 'TEST_KEYS',
	);
	const { outcome: verified } = verifySignature(feed, signature, table ?? [], FEED_DOMAIN);
	if (verified !== 'trusted') throw new StepFailure(`${what} does not verify (${verified})`);
	const admitted = readFeed(feed, { platform });
	if (admitted.problems.length > 0)
		throw new StepFailure(`${what}: ${admitted.problems.join('; ')}`);
	return admitted.feed;
}

/** The feed of the tracked update pair in `folder`, verified, or undefined when none is tracked. */
function trackedFeed(folder, stem, platform) {
	const feedFile = path.join(folder, 'feed.json.signed');
	if (!existsSync(feedFile)) return undefined;
	return verifiedFeed(
		readFileSync(feedFile),
		readFileSync(path.join(folder, 'feed.json.sig')),
		stem,
		platform,
		`The tracked update pair in ${folder}`,
	);
}

/**
 * The update check after publication in `options.work`, a new folder the caller created;
 * `progress.stage` names what a failed step was doing.
 */
export function updateCheck(options, progress, port, usage) {
	const { version, stem, work } = options;
	const source = assetsSource(options.release, stem, usage);
	const name = feedName(stem, version, port.platform);
	const feedAddress = new URL(name, source.base).href;

	progress.stage = `fetching ${name}`;
	const feedFile = path.join(work, name);
	fetchAsset(
		source,
		feedAddress,
		feedFile,
		FEED_LIMIT,
		`${name} is not public at ${feedAddress}. If v${version} is still a draft, re-run the failed jobs of its release run, which also starts this check again`,
	);
	fetchAsset(
		source,
		`${feedAddress}.sig`,
		`${feedFile}.sig`,
		SIGNATURE_LIMIT,
		`${name}.sig is not public at ${feedAddress}.sig`,
	);
	// `update` verifies the feed itself; the check reads it only to find the archive.
	const offered = readFeed(readFileSync(feedFile), { platform: port.platform });
	if (offered.problems.length > 0 || offered.feed.version !== version)
		throw new StepFailure(
			`${feedAddress} is not a feed of ${version}: ${offered.problems.join('; ') || `it names ${offered.feed.version}`}`,
		);

	progress.stage = 'reading the version people have now from the tracked update pair';
	const current = trackedFeed(options.current, stem, port.platform);
	const env = port.environment(work);
	const storeRoot = port.storeRoot(stem, env);
	let updated = { root: storeRoot, env: { ...env, ORCHBEE_UPDATE_FEED: feedAddress } };
	if (current !== undefined && current.version !== version) {
		log(`The tracked update pair names ${current.version}, the version people have now.`);
		progress.stage = `installing ${current.version}, the version people have now`;
		const archive = fetchArchive(source, current, work, 'current');
		installRelease(
			port,
			unpackRelease(port, archive, work, 'current', current.version),
			storeRoot,
			env,
		);
	} else {
		log(
			current === undefined
				? `No update pair is tracked, so no older version can update: ${version} updates an empty store.`
				: `The tracked update pair already names ${version}: it updates an empty store.`,
		);
		progress.stage = `installing ${version} from its own archive`;
		const archive = fetchArchive(source, offered.feed, work, 'release');
		installRelease(port, unpackRelease(port, archive, work, 'release', version), storeRoot, env);
		// Beside the store root, below the folder its install just admitted as private: the store
		// refuses to create a missing root below a folder other accounts can read, such as the
		// runner's temporary folder.
		const empty = `${storeRoot}-update-check`;
		if (existsSync(empty)) throw new StepFailure(`Refusing to update into an existing ${empty}`);
		updated = { root: empty, env: { ...updated.env, ORCHBEE_INSTALL_DIR: empty } };
	}

	progress.stage = `running orchbee update to ${version}`;
	const ran = runDocument(port.updater(storeRoot, stem, env), ['update'], updated.env);
	const result = ran.document?.result;
	if (
		ran.status !== 0 ||
		result?.activated !== true ||
		result?.upToDate === true ||
		result?.releaseVersion !== version
	)
		throw new StepFailure(documentFailure('update', ran));

	progress.stage = `checking that ${version} is active`;
	port.expectActive(updated.root, stem, version, updated.env);
	log(`${version} can be updated to. Move the update channel: pnpm website:release ${version}`);
}

/** What the owner must do after a failed update check: the channel must not move. */
export function updateFailureAdvice(version, stage) {
	return `The update check of ${version} failed while ${stage}. Do not run pnpm website:release ${version}. If a download failed for a passing reason, run this check again; otherwise leave the update channel where it is and release the next version.\n`;
}
