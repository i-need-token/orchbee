// The update check after publication, shared by the Windows and macOS release smoke scripts, and
// the published-release reads and release gate flow both platforms share. A published release can
// be updated to when the version people have now, installed from its own published archive, updates
// to it through its own `update` reading the release's feed asset, including the window store
// (`<store root>/desktop/`) when both feeds carry `extensions.desktop`. Each platform script owns
// the argument parsing of its other steps, its exit codes and its work folder, and passes a port
// that says where that platform installs and how its programs run:
//
// - platform: the release platform, whose feed, tracked update pair and archive format apply;
// - tar: the system tar, which unpacks either archive format;
// - client(directory, stem): the Client program of an unpacked release directory;
// - versionClient(storeRoot, stem, version): the installed Client program of `version` in `storeRoot`;
// - bundledWindowClient(windowDirectory, stem): the bundled Client of an unpacked window archive;
// - environment(work): the environment every install and update runs with, without
//   ORCHBEE_UPDATE_FEED;
// - storeRoot(stem, env): the store root `install` fills under `env`;
// - updater(storeRoot, stem, env): the command a person runs for `update`;
// - expectActive(storeRoot, stem, version, env): fails unless the store runs `version`;
// - expectWindowActive(storeRoot, stem, version, env): fails unless `desktop/` runs `version`.
//
// The version people have now is the one the website's tracked update pair of the platform names
// (apps/website/public/update/stable/<platform>/, or the folder `--current` names), whose signature
// must verify. Its archive, fetched from the address that pair signs and checked against the signed
// size and SHA-256, is unpacked and installed by its own `install`; when both feeds carry
// `extensions.desktop`, its window archive is also fetched, verified, unpacked and installed into
// `<store root>/desktop/` with `--installer-window-v1` (skipped only when an older pre-window
// release does not recognize that helper). Then the updater runs `update` with `ORCHBEE_UPDATE_FEED`
// naming the release's `<stem>-<version>-<platform>.feed.json` in the assets folder, and must
// activate the release (and the new window version when `desktop/` was seeded). Without a tracked
// pair, or when the pair already names the release, no older version can update: the release is
// installed from its own archive and its `update` fills an empty scratch store
// (`ORCHBEE_INSTALL_DIR`) beside the store root, which still proves feed, download, unpacking,
// publication and activation. Every request is an anonymous GET with the system curl. The assets
// address is the release's download folder over HTTPS; only a test build's stem admits
// `http://127.0.0.1:<port>/…/`, a loopback server standing in for it, together with feeds that name
// their archive by a path relative to that address.
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { DESKTOP_FILES, desktopReleaseName } from './desktop-files.mjs';
import { readRelease } from './package-npm.mjs';
import {
	absoluteArchive,
	desktopProblems,
	FEED_LIMIT,
	feedName,
	GATE_FEED_SUFFIX,
	readFeed,
	sha256,
	validVersion,
} from './release-feed.mjs';
import { compareVersions } from './release-version.mjs';
import { FEED_DOMAIN, keyTable, verifySignature } from './sign-release-manifest.mjs';
import { curl, fileFacts } from './website-release.mjs';

// An install or an update gets this long.
const setupDeadline = 600_000;
const commandDeadline = 120_000;
const terminalDeadline = 60_000;
const terminalPoll = 500;
// The older Server runs its helper a minute after its Session admits Clients; the helper then
// installs the new release and window. The idle move waits one quiet minute, closes its checks and stops.
const activationDeadline = 900_000;
const idleMoveDeadline = 600_000;
const gatePoll = 2_000;
// The probe ends itself after 30 minutes of lines, should the gate stop without ending it.
const probeLimit = 18_000;
const probeLine = /probe-(\d+)-(\d+)/gu;
// `server restart` ends within its own 150 seconds.
const restartDeadline = 180_000;
// The first release whose Terminal Host offers `handover` (proposals/platform/host-handover.md, H1):
// a path from it on whose new release moves terminals also checks the move (see `handoverStart`).
const handoverSince = '0.1.0-beta.23';
// The new Worker updates the Keeper, then moves one idle terminal every two seconds, and an older
// Host left without a terminal and its retired Keeper then exit.
const handoverDeadline = 120_000;
// How the Session reaches the new release on a gate path; see `gatePathModes`.
const gateModes = ['move', 'restart', 'adopt'];
// Release builds support `--installer-window-v1` and window self-updates from 0.1.0-beta.14 on.
const firstWindowRelease = '0.1.0-beta.14';
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

function sleep(milliseconds) {
	Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

/** Runs `program <argv>` without adding `--format json`; `document` is parsed stdout, or null. */
export function runExactDocument(
	program,
	argv,
	env,
	{ deadline = setupDeadline, quiet = false } = {},
) {
	if (!quiet) log(`> ${program} ${argv.join(' ')}`);
	const result = spawnSync(program, argv, {
		encoding: 'utf8',
		env,
		stdio: ['ignore', 'pipe', 'pipe'],
		timeout: deadline,
		maxBuffer: 64 * 1024 * 1024,
		windowsHide: true,
	});
	if (result.error !== undefined)
		throw new StepFailure(`${program} ${argv.join(' ')}: ${result.error.message}`);
	if (!quiet) {
		if (result.stdout) process.stdout.write(result.stdout);
		if (result.stderr) process.stdout.write(result.stderr);
	}
	const ran = { status: result.status, stdout: result.stdout ?? '', stderr: result.stderr ?? '' };
	try {
		return { ...ran, document: JSON.parse(ran.stdout) };
	} catch {
		return { ...ran, document: null };
	}
}

/** Runs `program <argv> --format json`; `document` is the parsed output, or null. */
export function runDocument(program, argv, env, options = {}) {
	return runExactDocument(program, [...argv, '--format', 'json'], env, options);
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

/** Validates `extensions.desktop` when present on a feed. */
function checkDesktopMember(feed, what) {
	const desktop = feed.extensions?.desktop;
	if (desktop === undefined) return undefined;
	const problems = desktopProblems(desktop);
	if (problems.length > 0) throw new StepFailure(`${what}: ${problems.join('; ')}`);
	return desktop;
}

/** Downloads and verifies the window archive `desktop.archive` into `work/<name>.zip`. */
function fetchWindowArchive(source, desktop, work, name) {
	const { format, url, size, sha256: digest } = desktop.archive;
	if (absoluteArchive(url) === (source.kind === 'loopback'))
		throw new StepFailure(
			`The window archive of ${desktop.version} is named ${url}, which a ${source.kind === 'loopback' ? 'stand-in' : 'release'} check does not fetch`,
		);
	const address = new URL(url, source.base).href;
	const output = path.join(work, `${name}.${format}`);
	fetchAsset(
		source,
		address,
		output,
		size,
		`The window archive of ${desktop.version} is not public at ${address}`,
	);
	const facts = fileFacts(output);
	if (facts.size !== size || facts.sha256 !== digest)
		throw new StepFailure(
			`The window archive of ${desktop.version} at ${address} does not match the size and SHA-256 its feed signs`,
		);
	log(`Fetched ${address}: ${size} bytes, SHA-256 as signed`);
	return output;
}

/** Unpacks a verified window archive and checks its top-level folder and `desktop-files.json`. */
function unpackWindowDirectory(port, archive, work, name, stem, desktop) {
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
	const expectedFolder = desktopReleaseName(stem, desktop.version, port.platform);
	const entries = readdirSync(folder, { withFileTypes: true });
	if (entries.length !== 1 || !entries[0].isDirectory() || entries[0].name !== expectedFolder)
		throw new StepFailure(
			`${path.basename(archive)} must hold the one folder ${expectedFolder}, not ${entries.map((entry) => entry.name).join(', ')}`,
		);
	const directory = path.join(folder, expectedFolder);
	const listFile = path.join(directory, DESKTOP_FILES);
	if (!existsSync(listFile) || sha256(readFileSync(listFile)) !== desktop.manifest.sha256)
		throw new StepFailure(`${path.basename(archive)}'s ${DESKTOP_FILES} does not match the feed`);
	return directory;
}

/**
 * Seeds `<storeRoot>/desktop/` from `olderFeed.extensions.desktop` via `--installer-window-v1` when
 * both `olderFeed` and `offeredFeed` carry `extensions.desktop`. Pre-window releases that exit 2
 * or 4 without an `orchbee.install` document skip pre-seeding gracefully; releases from
 * `firstWindowRelease` (0.1.0-beta.14) on must exit 0 and activate the older window version.
 */
export function seedWindowStore({
	port,
	source,
	olderFeed,
	offeredFeed,
	stem,
	work,
	storeRoot,
	env,
	fetchWindow = fetchWindowArchive,
	unpackWindow = unpackWindowDirectory,
	runInstaller = runExactDocument,
}) {
	const olderDesktop = checkDesktopMember(olderFeed, `The feed of ${olderFeed.version}`);
	const offeredDesktop = checkDesktopMember(offeredFeed, `The feed of ${offeredFeed.version}`);
	if (olderDesktop === undefined || offeredDesktop === undefined) return { seeded: false };

	const archive = fetchWindow(source, olderDesktop, work, 'older-window');
	const windowDirectory = unpackWindow(
		port,
		archive,
		work,
		'older-window-unpacked',
		stem,
		olderDesktop,
	);
	const bundledClient = port.bundledWindowClient(windowDirectory, stem);
	if (!existsSync(bundledClient))
		throw new StepFailure(
			`The unpacked window of ${olderDesktop.version} holds no ${bundledClient}`,
		);
	const ran = runInstaller(bundledClient, ['--installer-window-v1'], env);
	const mustSupport =
		stem === 'orchbee' && compareVersions(olderFeed.version, firstWindowRelease) >= 0;
	if (!mustSupport && ran.document?.schema !== 'orchbee.install' && [2, 4].includes(ran.status)) {
		log(
			`Skipping window store pre-seed: ${olderFeed.version} does not support --installer-window-v1 (exited ${ran.status}).`,
		);
		return { seeded: false };
	}
	if (
		ran.status !== 0 ||
		ran.document?.schema !== 'orchbee.install' ||
		ran.document?.result?.activated !== true
	) {
		throw new StepFailure(documentFailure('--installer-window-v1', ran));
	}
	port.expectWindowActive(storeRoot, stem, olderDesktop.version, env);
	return {
		seeded: true,
		olderVersion: olderDesktop.version,
		expectedVersion: offeredDesktop.version,
	};
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
	const offeredFeed = verifiedFeed(
		readFileSync(feedFile),
		readFileSync(`${feedFile}.sig`),
		stem,
		port.platform,
		`The feed of ${version} at ${feedAddress}`,
	);
	if (offeredFeed.version !== version)
		throw new StepFailure(
			`${feedAddress} is not a feed of ${version}: it names ${offeredFeed.version}`,
		);

	progress.stage = 'reading the version people have now from the tracked update pair';
	const current = trackedFeed(options.current, stem, port.platform);
	const env = port.environment(work);
	const storeRoot = port.storeRoot(stem, env);
	let updated = { root: storeRoot, env: { ...env, ORCHBEE_UPDATE_FEED: feedAddress } };
	let windowSeed = { seeded: false };
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
		progress.stage = `seeding the window store of ${current.version}`;
		windowSeed = seedWindowStore({
			port,
			source,
			olderFeed: current,
			offeredFeed,
			stem,
			work,
			storeRoot,
			env,
		});
	} else {
		log(
			current === undefined
				? `No update pair is tracked, so no older version can update: ${version} updates an empty store.`
				: `The tracked update pair already names ${version}: it updates an empty store.`,
		);
		progress.stage = `installing ${version} from its own archive`;
		const archive = fetchArchive(source, offeredFeed, work, 'release');
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
	if (windowSeed.seeded) {
		progress.stage = `checking that the window version ${windowSeed.expectedVersion} is active`;
		const desktopResult = result?.desktop;
		if (
			desktopResult?.activated !== true ||
			desktopResult?.upToDate === true ||
			desktopResult?.error !== null ||
			desktopResult?.releaseVersion !== windowSeed.expectedVersion
		) {
			throw new StepFailure(
				`update did not activate the window version ${windowSeed.expectedVersion}: ${JSON.stringify(desktopResult)}`,
			);
		}
		port.expectWindowActive(updated.root, stem, windowSeed.expectedVersion, updated.env);
	}
	log(`${version} can be updated to.`);
}

/**
 * Parses `gate v<older version> --release <assets address> --feed <absolute gate feed>
 * --work <absolute new directory> [--stem <stem>]`, given without the step's name.
 */
export function parseGateArguments(argv, usage) {
	const [tag, ...rest] = argv;
	const options = {};
	for (let index = 0; index < rest.length; index += 2) {
		const [option, value] = [rest[index], rest[index + 1]];
		if (
			!['--release', '--feed', '--work', '--stem', '--mode'].includes(option) ||
			value === undefined
		)
			throw new UsageError(usage);
		options[option.slice(2)] = value;
	}
	const older = tag?.startsWith('v') ? tag.slice(1) : undefined;
	const stem = options.stem ?? 'orchbee';
	const mode = options.mode ?? 'move';
	if (
		!validVersion(older) ||
		options.release === undefined ||
		options.feed === undefined ||
		!path.isAbsolute(options.feed) ||
		options.work === undefined ||
		!path.isAbsolute(options.work) ||
		!admittedStem(stem) ||
		!gateModes.includes(mode)
	)
		throw new UsageError(usage);
	assetsSource(options.release, stem, usage);
	return {
		step: 'gate',
		older,
		mode,
		stem,
		release: options.release,
		feed: options.feed,
		work: options.work,
	};
}

/** The newest `logs/<YYYY-MM-DD>.jsonl` in `dataRoot`, or undefined when there is none. */
export function newestDiagnosticLog(dataRoot) {
	const folder = path.join(dataRoot, 'logs');
	const days = existsSync(folder)
		? readdirSync(folder)
				.filter((name) => /^\d{4}-\d{2}-\d{2}\.jsonl$/u.test(name))
				.toSorted()
		: [];
	return days.length === 0 ? undefined : path.join(folder, days.at(-1));
}

/** The last `lines` of `filename`, or a note when it does not exist. */
export function logTail(filename, lines = 60) {
	if (!existsSync(filename)) return `(no log at ${filename})`;
	return readFileSync(filename, 'utf8').split(/\r?\n/u).slice(-lines).join('\n');
}

/** The tail of the newest diagnostic log in `dataRoot`. */
export function diagnosticLogTail(dataRoot) {
	const file = newestDiagnosticLog(dataRoot);
	return file === undefined
		? `(no diagnostic log in ${path.join(dataRoot, 'logs')})`
		: logTail(file, 40);
}

function diagnosticRecords(dataRoot, events) {
	const file = newestDiagnosticLog(dataRoot);
	if (file === undefined) return [];
	const records = [];
	for (const line of readFileSync(file, 'utf8').split(/\r?\n/u)) {
		if (!events.some((event) => line.includes(`"event":"${event}"`))) continue;
		try {
			records.push(JSON.parse(line));
		} catch {
			// A line still being written is read again at the next poll.
		}
	}
	return records;
}

/**
 * Inspects the older Server's `automatic-update-schedule` and `automatic-update` diagnostic
 * records: `{ settled: true }` once the helper has exited cleanly and activated a version,
 * `{ failure }` when scheduling or the helper failed, or `{ settled: false }` while waiting.
 */
function automaticUpdateStatus(dataRoot) {
	for (const { event, fields = {} } of diagnosticRecords(dataRoot, [
		'automatic-update-schedule',
		'automatic-update',
	])) {
		if (event === 'automatic-update-schedule' && fields.scheduled === false)
			return {
				settled: false,
				failure: `the older Server scheduled no automatic update (${fields.reason ?? 'no reason'})`,
			};
		if (event === 'automatic-update') {
			if (fields.run !== 'answered' || fields.error !== 'none' || fields.upToDate === true)
				return {
					settled: false,
					failure: `its automatic helper ended without activating it: ${JSON.stringify(fields)}`,
				};
			return { settled: true };
		}
	}
	return { settled: false };
}

function sessionServer(program, env) {
	const status = runDocument(program, ['status', 'server'], env, {
		deadline: commandDeadline,
		quiet: true,
	});
	const server = (status.document?.result?.server?.processes ?? []).find(
		(each) => each.role === 'server',
	);
	return status.status === 0 ? server : undefined;
}

function activeVersion(storeRoot) {
	try {
		return JSON.parse(readFileSync(path.join(storeRoot, 'active.json'), 'utf8'))?.active?.product;
	} catch {
		return undefined;
	}
}

/** The process id the Session's `status server` lists for `role`, if it lists one. */
function sessionProcess(program, env, role) {
	const status = runDocument(program, ['status', 'server'], env, {
		deadline: commandDeadline,
		quiet: true,
	});
	return (status.document?.result?.server?.processes ?? []).find((each) => each.role === role)
		?.processId;
}

function processExists(pid) {
	try {
		process.kill(pid, 0);
		return true;
	} catch (error) {
		return error.code === 'EPERM';
	}
}

function layoutOf(program, env) {
	const workspaces = runDocument(program, ['workspace', 'list'], env, {
		deadline: commandDeadline,
	});
	const tabs = runDocument(program, ['tab', 'list'], env, { deadline: commandDeadline });
	if (workspaces.status !== 0)
		throw new StepFailure(`workspace list exited ${workspaces.status}, expected 0`);
	if (tabs.status !== 0) throw new StepFailure(`tab list exited ${tabs.status}, expected 0`);
	return JSON.stringify({
		workspaces: (workspaces.document?.result?.workspaces ?? []).map(({ workspaceId, label }) => ({
			workspaceId,
			label,
		})),
		tabs: (tabs.document?.result?.tabs ?? []).map(({ tabId, workspaceId, label }) => ({
			tabId,
			workspaceId,
			label,
		})),
	});
}

function probeRecord(file) {
	if (!existsSync(file)) return [];
	return readFileSync(file, 'utf8')
		.split('\n')
		.slice(0, -1)
		.map((line) => {
			const match = /^(\d+) (\d+)$/u.exec(line);
			return match === null ? { line } : { pid: Number(match[1]), number: Number(match[2]) };
		});
}

/** The probe's one process id and last number; a second process id or a gap fails. */
export function probeFacts(entries) {
	const malformed = entries.find((entry) => entry.pid === undefined);
	if (malformed !== undefined)
		throw new StepFailure(`The probe record holds ${JSON.stringify(malformed.line)}`);
	const pids = [...new Set(entries.map((entry) => entry.pid))];
	if (pids.length !== 1)
		throw new StepFailure(`The probe ran as processes ${pids.join(', ') || 'none'}`);
	entries.forEach((entry, index) => {
		if (entry.number !== index + 1)
			throw new StepFailure(
				`The probe's numbers jump from ${index} to ${entry.number}: its output had a gap`,
			);
	});
	return { pid: pids[0], last: entries.length };
}

function verifyGateArchiveBeside(feedPath, member, what) {
	if (absoluteArchive(member.url) || member.url.includes('/'))
		throw new StepFailure(`The gate feed names its ${what} by ${member.url}, not a bare file name`);
	const beside = path.join(path.dirname(feedPath), member.url);
	const facts = existsSync(beside) ? fileFacts(beside) : undefined;
	if (facts?.size !== member.size || facts?.sha256 !== member.sha256)
		throw new StepFailure(`${beside} is not the ${what} the gate feed signs`);
}

/**
 * Runs one pre-publish release gate path on Windows or macOS: installs `options.older` (and its
 * window version when both feeds carry `extensions.desktop`), starts its Session with a terminal
 * probe, waits for `--automatic-update-v1` to activate the new runtime and window versions, joins
 * with the new Client, and brings the Session to the new release as `options.mode` says. `move`
 * ends the probe, confirms zero gaps, waits for the idle move, and verifies layout preservation on
 * the new Server; `restart` and `adopt` keep the probe running (see `keepProbeAcross`).
 */
export function runGatePath(options, progress, port, usage) {
	const { older, stem } = options;
	const source = assetsSource(options.release, stem, usage);

	progress.stage = 'reading the gate feed';
	const expected = path.basename(options.feed);
	const gateBytes = readFileSync(options.feed);
	const offered = verifiedFeed(
		gateBytes,
		readFileSync(`${options.feed}.sig`),
		stem,
		port.platform,
		`The gate feed ${options.feed}`,
	);
	const version = offered.version;
	if (
		expected !==
		`${feedName(stem, version, port.platform).slice(0, -'.feed.json'.length)}${GATE_FEED_SUFFIX}`
	)
		throw new StepFailure(`${options.feed} is not the gate copy of ${stem} ${version}`);
	if (version === older)
		throw new StepFailure(`The gate feed names ${older}, the older release itself`);
	verifyGateArchiveBeside(options.feed, offered.archive, 'archive');
	const offeredDesktop = checkDesktopMember(offered, `The gate feed ${options.feed}`);
	if (offeredDesktop !== undefined) {
		verifyGateArchiveBeside(options.feed, offeredDesktop.archive, 'window archive');
	}
	options.version = version;

	const { work, env, storeRoot, dataRoot } = port.prepareGateWork(options.work, stem);
	const olderEnv = { ...env, ORCHBEE_UPDATE_FEED: options.feed };
	const olderClient = port.versionClient(storeRoot, stem, older);
	const newClient = port.versionClient(storeRoot, stem, version);
	const record = path.join(work, 'probe.txt');
	const stop = path.join(work, 'probe.stop');
	let sessionStarted = false;

	try {
		progress.stage = `installing ${older} from its published archive`;
		const name = feedName(stem, older, port.platform);
		const feedAddress = new URL(name, source.base).href;
		const feedFile = path.join(work, name);
		fetchAsset(
			source,
			feedAddress,
			feedFile,
			FEED_LIMIT,
			`${name} is not public at ${feedAddress}`,
		);
		fetchAsset(
			source,
			`${feedAddress}.sig`,
			`${feedFile}.sig`,
			SIGNATURE_LIMIT,
			`${name}.sig is not public at ${feedAddress}.sig`,
		);
		const olderFeed = verifiedFeed(
			readFileSync(feedFile),
			readFileSync(`${feedFile}.sig`),
			stem,
			port.platform,
			`The feed of ${older} at ${feedAddress}`,
		);
		if (olderFeed.version !== older)
			throw new StepFailure(`${feedAddress} names ${olderFeed.version}, not ${older}`);
		const archive = fetchArchive(source, olderFeed, work, 'older');
		installRelease(port, unpackRelease(port, archive, work, 'older', older), storeRoot, env);
		progress.stage = `seeding the window store of ${older}`;
		const windowSeed = seedWindowStore({
			port,
			source,
			olderFeed,
			offeredFeed: offered,
			stem,
			work,
			storeRoot,
			env,
		});

		progress.stage = `starting the Session of ${older}`;
		sessionStarted = true;
		const startedServer = runDocument(olderClient, ['server', 'start'], olderEnv, {
			deadline: commandDeadline,
		});
		if (startedServer.status !== 0)
			throw new StepFailure(`server start exited ${startedServer.status}, expected 0`);
		const server = sessionServer(olderClient, olderEnv);
		if (server?.version !== older || typeof server?.processId !== 'number')
			throw new StepFailure(`The Session's Server reports ${JSON.stringify(server)}, not ${older}`);
		log(`The Session of ${older} runs; its Server is process ${server.processId}.`);

		progress.stage = 'opening a terminal that runs the probe';
		const terminal = port.startProbe({
			olderClient,
			olderEnv,
			work,
			record,
			stop,
			limit: probeLimit,
			terminalDeadline,
			terminalPoll,
		});
		const probeStart = Date.now();
		while (probeRecord(record).length < 10) {
			if (Date.now() - probeStart >= terminalDeadline)
				throw new StepFailure(`The probe wrote no record within ${terminalDeadline} ms`);
			sleep(terminalPoll);
		}
		const { pid } = probeFacts(probeRecord(record));
		log(`The probe runs as process ${pid} in terminal ${terminal}.`);

		progress.stage = `waiting for the helper of ${older} to activate ${version}`;
		const until = Date.now() + activationDeadline;
		for (;;) {
			const status = automaticUpdateStatus(dataRoot);
			if (status.failure !== undefined)
				throw new StepFailure(`${version} is not active: ${status.failure}`);
			if (status.settled && activeVersion(storeRoot) === version) break;
			if (!windowSeed.seeded && activeVersion(storeRoot) === version && existsSync(newClient)) {
				break;
			}
			if (Date.now() >= until)
				throw new StepFailure(
					`${version} is not active ${activationDeadline} ms after the Session started`,
				);
			sleep(gatePoll);
		}
		port.expectActive(storeRoot, stem, version, env);
		if (windowSeed.seeded) {
			progress.stage = `checking that the helper of ${older} activated window ${windowSeed.expectedVersion}`;
			port.expectWindowActive(storeRoot, stem, windowSeed.expectedVersion, env);
		}
		log(`The helper of ${older} activated ${version}.`);

		progress.stage = `joining the Session with the Client of ${version}`;
		const versionCheck = spawnSync(newClient, ['--version'], {
			encoding: 'utf8',
			env,
			stdio: ['ignore', 'pipe', 'pipe'],
			timeout: commandDeadline,
			windowsHide: true,
		});
		if (versionCheck.status !== 0 || versionCheck.stdout.trim() !== `orchbee ${version}`)
			throw new StepFailure(
				`${newClient} --version answered ${JSON.stringify(versionCheck.stdout?.trim())}, expected "orchbee ${version}"`,
			);
		const joined = sessionServer(newClient, env);
		if (joined?.processId !== server.processId)
			throw new StepFailure(
				`The Client of ${version} reaches ${JSON.stringify(joined)}, not the running Session`,
			);
		const read = runDocument(
			newClient,
			['terminal', 'read', terminal, '--source', 'recent', '--lines', '50'],
			env,
			{ deadline: commandDeadline, quiet: true },
		);
		if (read.status !== 0)
			throw new StepFailure(`terminal read by the Client of ${version} exited ${read.status}`);
		const shown = [...String(read.document?.result?.content?.text ?? '').matchAll(probeLine)];
		if (shown.length === 0 || shown.some((match) => Number(match[1]) !== pid))
			throw new StepFailure(
				`The Client of ${version} does not read the probe's lines from process ${pid}`,
			);
		log(`The Client of ${version} joined and reads probe line ${shown.at(-1)[2]}.`);

		if (options.mode !== 'move') {
			keepProbeAcross({
				options,
				progress,
				port,
				storeRoot,
				server,
				newClient,
				env,
				record,
				terminal,
				pid,
			});
		}

		progress.stage = 'ending the probe';
		writeFileSync(stop, '', { flag: 'wx' });
		const ended = Date.now();
		for (;;) {
			const row = sessionServer(newClient, env);
			if (row?.terminals === 0) break;
			if (Date.now() - ended >= terminalDeadline)
				throw new StepFailure(
					`The Server still counts ${row?.terminals ?? 'unknown'} terminals ${terminalDeadline} ms after the probe was told to end`,
				);
			sleep(terminalPoll);
		}
		const { last } = probeFacts(probeRecord(record));
		log(`The probe printed lines 1 to ${last} as process ${pid}, without a gap, and ended.`);
		if (options.mode !== 'move') {
			log(`The path from ${older} passes.`);
			return;
		}
		const layout = layoutOf(newClient, env);

		progress.stage = `waiting for the idle move of ${older}`;
		// No Client connects now: a connection would start the Session's quiet minute again.
		const idleSince = Date.now();
		while (processExists(server.processId)) {
			if (Date.now() - idleSince >= idleMoveDeadline)
				throw new StepFailure(
					`The Server of ${older} still runs ${idleMoveDeadline} ms after the Session went idle`,
				);
			sleep(gatePoll);
		}
		log(`The Server of ${older} ended ${Date.now() - idleSince} ms after the Session went idle.`);

		progress.stage = `starting the Session on ${version}`;
		const restarted = runDocument(newClient, ['server', 'start'], env, {
			deadline: commandDeadline,
		});
		if (restarted.status !== 0)
			throw new StepFailure(`server start on ${version} exited ${restarted.status}, expected 0`);
		const moved = sessionServer(newClient, env);
		if (moved?.version !== version)
			throw new StepFailure(
				`The Session's Server reports ${JSON.stringify(moved)}, not ${version}`,
			);
		const after = layoutOf(newClient, env);
		if (after !== layout)
			throw new StepFailure(
				`The layout changed across the move:\n  before ${layout}\n  after  ${after}`,
			);
		log(`The Session runs on ${version} with its layout unchanged. The path from ${older} passes.`);
	} catch (error) {
		if (error instanceof StepFailure) {
			const entries = probeRecord(record);
			if (entries.length > 0)
				log(`Probe record: ${entries.length} lines, the last ${JSON.stringify(entries.at(-1))}`);
			log(`Diagnostic log:\n${diagnosticLogTail(dataRoot)}`);
		}
		throw error;
	} finally {
		port.finishGateSession?.({ sessionStarted, stem, olderClient, newClient, olderEnv, env, work });
	}
}

/**
 * Brings a gate path's Session to the new release while its probe runs: `restart` runs the new
 * release's `server restart`, as a click on Update does, and `adopt` ends the older Server by its
 * process id, as a crash would, and starts the Session with the new Client, which adopts the probe's
 * Host cold. No shipped command sends the Server's `exit` while a terminal runs, and both ends leave
 * the Host to the same adoption. Either way the new release's Server must serve the Session, the
 * probe must go on as the same process without a gap, the new Client must read its newer lines, and
 * the Workspaces and Tabs must stay. When the new release moves the probe's terminal off its Host,
 * the move is checked as well (see `expectHandover`).
 */
function keepProbeAcross({
	options,
	progress,
	port,
	storeRoot,
	server,
	newClient,
	env,
	record,
	terminal,
	pid,
}) {
	const { older, version } = options;
	const layout = layoutOf(newClient, env);
	const handover = handoverStart({ options, port, storeRoot, newClient, env });
	const before = probeRecord(record).length;
	if (options.mode === 'restart') {
		progress.stage = `restarting the Session with ${version} while the probe runs`;
		const restarted = runDocument(newClient, ['server', 'restart'], env, {
			deadline: restartDeadline,
		});
		if (restarted.status !== 0)
			throw new StepFailure(
				`server restart exited ${restarted.status}, expected 0: ${JSON.stringify(restarted.document?.result ?? null)}`,
			);
	} else {
		progress.stage = `ending the Server of ${older} while the probe runs`;
		const worker = sessionProcess(newClient, env, 'worker');
		process.kill(server.processId);
		const ending = Date.now();
		// The Worker ends by itself once its Server is gone.
		while (processExists(server.processId) || (worker !== undefined && processExists(worker))) {
			if (Date.now() - ending >= terminalDeadline)
				throw new StepFailure(
					`The Server or Worker of ${older} still runs ${terminalDeadline} ms after the Server was ended`,
				);
			sleep(terminalPoll);
		}
		progress.stage = `starting the Session on ${version}, which adopts the probe's Host`;
		const started = runDocument(newClient, ['server', 'start'], env, {
			deadline: commandDeadline,
		});
		if (started.status !== 0)
			throw new StepFailure(`server start on ${version} exited ${started.status}, expected 0`);
	}
	const moved = sessionServer(newClient, env);
	if (moved?.version !== version || moved.processId === server.processId)
		throw new StepFailure(`The Session's Server reports ${JSON.stringify(moved)}, not ${version}`);
	log(`The Session's Server runs ${version} as process ${moved.processId}.`);

	progress.stage = 'checking that the probe kept running';
	const went = expectProbeGoesOn({ newClient, env, record, terminal, pid, before, version });
	const after = layoutOf(newClient, env);
	if (after !== layout)
		throw new StepFailure(
			`The layout changed across the change:\n  before ${layout}\n  after  ${after}`,
		);
	log(
		`The probe went on from line ${before} to ${went.last} as process ${pid}, and the Client of ${version} reads line ${went.read}; the layout is unchanged.`,
	);
	if (handover !== undefined) {
		expectHandover({ options, progress, port, storeRoot, newClient, env, handover });
		progress.stage = 'checking that the probe kept running across its move';
		const settled = probeRecord(record).length;
		const across = expectProbeGoesOn({
			newClient,
			env,
			record,
			terminal,
			pid,
			before: settled,
			version,
		});
		log(
			`The probe went on from line ${settled} to ${across.last} as process ${pid} on the Host of ${version}, and its Client reads line ${across.read}.`,
		);
	}
}

/**
 * Waits for two more seconds of probe lines after line `before`, which prove the probe outlived
 * what came before, then checks that it still runs as process `pid` without a gap and that the
 * Client of `version` reads a line after `before`. Answers the probe's last line and the line read.
 */
function expectProbeGoesOn({ newClient, env, record, terminal, pid, before, version }) {
	const checking = Date.now();
	while (probeRecord(record).length < before + 20) {
		if (Date.now() - checking >= terminalDeadline)
			throw new StepFailure(
				`The probe wrote no line after line ${before} within ${terminalDeadline} ms`,
			);
		sleep(terminalPoll);
	}
	const facts = probeFacts(probeRecord(record));
	if (facts.pid !== pid)
		throw new StepFailure(`The probe runs as process ${facts.pid}, not ${pid}`);
	const read = runDocument(
		newClient,
		['terminal', 'read', terminal, '--source', 'recent', '--lines', '50'],
		env,
		{ deadline: commandDeadline, quiet: true },
	);
	if (read.status !== 0)
		throw new StepFailure(`terminal read after line ${before} exited ${read.status}`);
	const shown = [...String(read.document?.result?.content?.text ?? '').matchAll(probeLine)];
	if (
		shown.length === 0 ||
		shown.some((match) => Number(match[1]) !== pid) ||
		Number(shown.at(-1)[2]) <= before
	)
		throw new StepFailure(
			`The Client of ${version} does not read the probe's lines after line ${before} from process ${pid}`,
		);
	return { last: facts.last, read: Number(shown.at(-1)[2]) };
}

/**
 * What H4's checks of a gate path start from (proposals/platform/host-handover.md, H4 design): when
 * the older release's Host offers `handover` and the new release's `status server` says its Worker
 * moves that Host's terminals, the processes of the older Hosts and of the Session's Keepers from
 * the older release, which must exit once the probe moved. Otherwise `undefined`: the path keeps
 * P1c's check, as one from a release before `handoverSince` does, or one whose new release does
 * not move terminals yet.
 */
function handoverStart({ options, port, storeRoot, newClient, env }) {
	if (port.keepers === undefined || compareVersions(options.older, handoverSince) < 0)
		return undefined;
	const status = runDocument(newClient, ['status', 'server'], env, {
		deadline: commandDeadline,
		quiet: true,
	});
	if (status.status !== 0)
		throw new StepFailure(`status server before the change exited ${status.status}`);
	const hosts = status.document?.result?.server?.restart?.hosts ?? [];
	if (!hosts.some((host) => host.moves === true)) return undefined;
	return {
		hosts: hosts.map((host) => host.processId),
		keepers: port.keepers(storeRoot, options.older).map((keeper) => keeper.pid),
	};
}

/**
 * H4's checks once the Session runs the new release: the older Hosts and the older release's
 * Keepers exited, one Keeper runs the new release's program, and every Host the Session lists is
 * the new release's, with the probe's terminal on it.
 */
function expectHandover({ options, progress, port, storeRoot, newClient, env, handover }) {
	const { older, version } = options;
	progress.stage = `waiting for the probe to move to the Host of ${version} and the processes of ${older} to exit`;
	const began = Date.now();
	for (;;) {
		const status = runDocument(newClient, ['status', 'server'], env, {
			deadline: commandDeadline,
			quiet: true,
		});
		const hosts = (status.document?.result?.server?.processes ?? [])
			.filter((each) => each.role === 'terminal-host')
			.map(({ processId, terminals, version: hostVersion }) => ({
				processId,
				terminals,
				version: hostVersion,
			}));
		const facts = {
			olderHosts: handover.hosts.filter((each) => processExists(each)),
			olderKeepers: handover.keepers.filter((each) => processExists(each)),
			keepers: port.keepers(storeRoot, version).map((keeper) => keeper.pid),
			hosts,
		};
		if (
			status.status === 0 &&
			facts.olderHosts.length === 0 &&
			facts.olderKeepers.length === 0 &&
			facts.keepers.length === 1 &&
			hosts.length > 0 &&
			hosts.every((host) => host.version === version) &&
			hosts.some((host) => host.terminals > 0)
		) {
			log(
				`The probe's terminal moved to the Host of ${version}, Keeper ${facts.keepers[0]} runs ${version}, and the Hosts ${handover.hosts.join(', ')} and Keepers ${handover.keepers.join(', ') || 'none'} of ${older} exited.`,
			);
			return;
		}
		if (Date.now() - began >= handoverDeadline)
			throw new StepFailure(
				`The probe's terminal did not move to the Host of ${version}, or the processes of ${older} did not exit, within ${handoverDeadline} ms: ${JSON.stringify(facts)}`,
			);
		sleep(terminalPoll);
	}
}

/** What the owner must do after a failed update check: the channel must not move. */
export function updateFailureAdvice(version, stage) {
	return `The update check of ${version} failed while ${stage}. Do not run pnpm website:release ${version}. If a download failed for a passing reason, run this check again; otherwise leave the update channel where it is and release the next version.\n`;
}

/** What a failed pre-publish release gate path prints on standard error. */
export function gateFailureAdvice(older, version, stage) {
	return `The release gate path from ${older}${version === undefined ? '' : ` to ${version}`} failed while ${stage}. Do not publish this release.\n`;
}
