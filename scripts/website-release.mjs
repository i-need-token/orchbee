// Moves the website to a published release: its update channels and its download buttons together.
//
//   pnpm website:release <version>
//   node scripts/website-release.mjs <version> [--from <absolute folder | http://127.0.0.1:<port>/<path>/>]
//
// Fetches the release's SHA256SUMS anonymously with the system curl, never with `gh`, so a draft
// release, whose assets answer 404 to anyone signed out, cannot pass, and a proxy variable works as
// it does for curl. The feed documents it lists, `orchbee-<version>-<platform>.feed.json`, name
// the platform rows the release published; `windows-x86_64` must be among them, and each must be a
// platform the website offers a download for (`downloads` below). For every row it fetches the
// feed pair, verifies the feed signature with RELEASE_KEYS of
// apps/orchbee/src/cli/update/signature.rs before reading the feed, checks the feed against the
// reader's rules, its written form and the requested version, confirms the row's download is
// public, downloads the archive the feed names, bounded by its signed size, and checks its size and
// SHA-256. Only then does it write, in one step:
//
//   apps/website/public/update/<channel>/<platform>/feed.json.signed   the exact feed bytes of each
//                                                                      row, served as feed.json
//   apps/website/public/update/<channel>/<platform>/feed.json.sig      its signature document
//   apps/website/src/website/release.ts                                version and each row's
//                                                                      download
//
// and removes the pair of every platform of the channel the release did not publish, so each
// tracked feed names the version the download buttons offer. The first failed check stops the
// command, and nothing is written or removed. It needs no credentials and
// sends nothing but anonymous GET requests. Commit, build and deploy the website afterwards;
// docs/RELEASING.md has the steps. Running it with an older version moves the channel back.
//
// `--from` stands in for the release address in the tooling's own checks: a folder or a loopback
// address holding a test build's pair. It trusts TEST_KEYS instead of RELEASE_KEYS and requires the
// feed to name its archive by a relative path, resolved beside the feed, so it never reaches the
// network; the website's own test refuses a pair that a release key did not sign.
//
// Exit codes: 0 written; 1 a check failed or a request failed, nothing written; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	closeSync,
	existsSync,
	lstatSync,
	mkdirSync,
	mkdtempSync,
	openSync,
	readFileSync,
	readdirSync,
	readSync,
	renameSync,
	rmdirSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
	absoluteArchive,
	FEED_LIMIT,
	feedName,
	readFeed,
	validVersion,
	writtenForm,
} from './release-feed.mjs';
import { FEED_DOMAIN, keyTable, verifySignature } from './sign-release-manifest.mjs';

const repository = fileURLToPath(new URL('..', import.meta.url));
const signatureSource = 'apps/orchbee/src/cli/update/signature.rs';
const releaseModule = 'apps/website/src/website/release.ts';
const updateRoot = 'apps/website/public/update';
// Each platform the website offers a download for, with that download's release asset name, in the
// order release.ts lists them. A release that publishes another platform is refused until it is
// added here and to `ReleaseDownloads` in release.ts.
const downloads = {
	'windows-x86_64': (version) => `Orchbee-${version}-windows-x86_64-setup.exe`,
	'macos-aarch64': (version) => `Orchbee-${version}-macos-aarch64.dmg`,
	'macos-x86_64': (version) => `Orchbee-${version}-macos-x86_64.dmg`,
};
// Every release publishes it; the calls to action offer its download.
const requiredPlatform = 'windows-x86_64';
const stem = 'orchbee';
const signatureLimit = 4 * 1024;
const checksumsName = 'SHA256SUMS';
const checksumsLimit = 64 * 1024;
const usage =
	'Usage: node scripts/website-release.mjs <version> [--from <absolute folder | http://127.0.0.1:<port>/<path>/>]';
const releaseBlock = /^export const release: Release = \{[\s\S]*?\n\};\n/mu;

class UsageError extends Error {}

/** The download asset the website offers for a platform of a version, by the download names. */
export function installerName(version, platform = requiredPlatform) {
	return Object.hasOwn(downloads, platform) ? downloads[platform](version) : undefined;
}

/**
 * The platform rows a release published, by the feed documents its SHA256SUMS lists, in the order
 * of `downloads`, or the sentences that say why the website cannot offer them.
 */
export function publishedPlatforms(checksums, version) {
	const prefix = `${stem}-${version}-`;
	const listed = checksums
		.toString('utf8')
		.split('\n')
		.map((line) => /^[0-9a-f]{64} {2}(\S+)$/u.exec(line)?.[1])
		.filter((name) => name?.startsWith(prefix) && name.endsWith('.feed.json'))
		.map((name) => name.slice(prefix.length, -'.feed.json'.length));
	const problems = [];
	const unknown = listed.filter((platform) => !Object.hasOwn(downloads, platform));
	if (unknown.length > 0)
		problems.push(
			`The release publishes ${unknown.join(', ')}, which the website offers no download for. Add each to \`downloads\` in scripts/website-release.mjs and to ReleaseDownloads in ${releaseModule} first.`,
		);
	if (!listed.includes(requiredPlatform))
		problems.push(`${checksumsName} lists no ${feedName(stem, version, requiredPlatform)}.`);
	if (problems.length > 0) return { problems, platforms: [] };
	return { problems, platforms: Object.keys(downloads).filter((key) => listed.includes(key)) };
}

/** The repository address release.ts names, or undefined when it names none. */
export function repositoryUrlOf(source) {
	return /^export const REPOSITORY_URL = '(https:\/\/[^'\s]+)';$/mu.exec(source)?.[1];
}

/**
 * release.ts with the release value set to `version` and the downloads of `platforms`, or undefined
 * when its block is not found.
 */
export function releaseSource(source, version, platforms = [requiredPlatform]) {
	if (!releaseBlock.test(source)) return undefined;
	const block =
		'export const release: Release = {\n' +
		'\tpublished: true,\n' +
		`\tversion: '${version}',\n` +
		'\tdownloads: {\n' +
		platforms
			.map(
				(platform) =>
					`\t\t'${platform}': \`\${REPOSITORY_URL}/releases/download/v${version}/${installerName(version, platform)}\`,\n`,
			)
			.join('') +
		'\t},\n' +
		'};\n';
	return source.replace(releaseBlock, block);
}

/**
 * The feed of a fetched pair when it may move the channel to `version`, and otherwise the sentences
 * that say why not. The signature is verified before the feed is read.
 */
export function admitFeed({ version, platform, feed, signature, table, standIn }) {
	const { outcome } = verifySignature(feed, signature, table, FEED_DOMAIN);
	if (outcome !== 'trusted')
		return {
			problems: [
				`The ${platform} feed signature does not verify with a key of ${standIn ? 'TEST_KEYS' : 'RELEASE_KEYS'} (${outcome}). Do not move the channel to this release.`,
			],
		};
	const admitted = readFeed(feed, { platform });
	if (admitted.problems.length > 0)
		return {
			problems: admitted.problems.map(
				(problem) => `The ${platform} feed breaks a reader rule: ${problem}.`,
			),
		};
	// The updater's reader refuses some spellings JSON.parse admits; the written form has none.
	if (!writtenForm(feed, admitted.feed))
		return {
			problems: [
				`The ${platform} feed is not in its written form, so the updater may refuse it. Do not move the channel to this release.`,
			],
		};
	const problems = [];
	if (admitted.feed.version !== version)
		problems.push(`The ${platform} feed names version ${admitted.feed.version}, not ${version}.`);
	const absolute = absoluteArchive(admitted.feed.archive.url);
	if (!standIn && !absolute)
		problems.push(
			`The feed names its archive by the relative path ${admitted.feed.archive.url}, which would resolve on the website, where no archive is. Only a release build's feed can move the channel.`,
		);
	if (standIn && absolute)
		problems.push(
			'A stand-in source must name its archive by a relative path; it never fetches from the network.',
		);
	return problems.length > 0 ? { problems } : { feed: admitted.feed, problems: [] };
}

/**
 * Every file the command writes, as `{ path, bytes }` relative to the repository, and every tracked
 * file it removes, or the sentences that say why nothing may change. Pure: the caller fetched every
 * input. `rows` holds one `{ platform, feed, signature, archive, installerPublic }` per published
 * platform; `tracked` names each tracked pair folder as `<channel>/<platform>`.
 */
export function planWebsiteRelease({
	version,
	rows,
	table,
	standIn = false,
	release,
	tracked = [],
}) {
	const problems = [];
	const admittedRows = [];
	for (const row of rows) {
		const { platform, feed, signature, archive, installerPublic } = row;
		const admitted = admitFeed({ version, platform, feed, signature, table, standIn });
		if (admitted.problems.length > 0) {
			problems.push(...admitted.problems);
			continue;
		}
		const expected = admitted.feed.archive;
		if (!installerPublic)
			problems.push(
				`${installerName(version, platform)} is not public; the download button would break.`,
			);
		if (archive === undefined) {
			problems.push(`The ${platform} archive was not fetched.`);
		} else {
			if (archive.size !== expected.size)
				problems.push(
					`The ${platform} archive has ${archive.size} bytes, but the signed feed says ${expected.size}. Do not move the channel to this release.`,
				);
			if (archive.sha256 !== expected.sha256)
				problems.push(
					`The ${platform} archive does not match the SHA-256 the signed feed names. Do not move the channel to this release.`,
				);
		}
		admittedRows.push({ ...row, channel: admitted.feed.channel });
	}
	const channels = [...new Set(admittedRows.map((row) => row.channel))];
	if (channels.length > 1)
		problems.push(`The feeds name several channels, ${channels.join(' and ')}.`);
	const source = releaseSource(
		release,
		version,
		rows.map((row) => row.platform),
	);
	if (source === undefined)
		problems.push(`${releaseModule} has no \`export const release: Release = {…};\` block to set.`);
	if (problems.length > 0) return refused(problems);
	const [channel] = channels;
	const published = new Set(rows.map((row) => row.platform));
	return {
		problems: [],
		channel,
		writes: [
			...admittedRows.flatMap(({ platform, feed, signature }) => [
				{ path: `${updateRoot}/${channel}/${platform}/feed.json.signed`, bytes: feed },
				{ path: `${updateRoot}/${channel}/${platform}/feed.json.sig`, bytes: signature },
			]),
			{ path: releaseModule, bytes: Buffer.from(source, 'utf8') },
		],
		removals: tracked
			.filter((folder) => {
				const [trackedChannel, platform] = folder.split('/');
				return trackedChannel === channel && !published.has(platform);
			})
			.flatMap((folder) => [
				`${updateRoot}/${folder}/feed.json.signed`,
				`${updateRoot}/${folder}/feed.json.sig`,
			]),
	};
}

function refused(problems) {
	return { problems, writes: [], removals: [] };
}

/**
 * Writes every planned file under `root`, each by a rename of a sibling, then removes every planned
 * removal and the folder it empties.
 */
export function applyWrites(root, writes, removals = []) {
	for (const { path: relative, bytes } of writes) {
		const target = path.join(root, ...relative.split('/'));
		mkdirSync(path.dirname(target), { recursive: true });
		const pending = `${target}.pending`;
		writeFileSync(pending, bytes, { flag: 'w' });
		renameSync(pending, target);
	}
	for (const relative of removals) {
		const target = path.join(root, ...relative.split('/'));
		rmSync(target, { force: true });
		const folder = path.dirname(target);
		if (existsSync(folder) && readdirSync(folder).length === 0) rmdirSync(folder);
	}
}

/** Each tracked pair folder under public/update, as `<channel>/<platform>`. */
function trackedPairs(root) {
	const update = path.join(root, ...updateRoot.split('/'));
	if (!existsSync(update)) return [];
	return readdirSync(update).flatMap((channel) =>
		readdirSync(path.join(update, channel)).map((platform) => `${channel}/${platform}`),
	);
}

/** Where the inputs come from: the release address, a stand-in folder, or a loopback stand-in. */
function sourceFor({ version, from, repositoryUrl }) {
	if (from === undefined) {
		if (repositoryUrl === undefined)
			throw new Error(`${releaseModule} names no REPOSITORY_URL to fetch the release from`);
		return { kind: 'https', base: `${repositoryUrl}/releases/download/v${version}/` };
	}
	if (/^http:\/\/127\.0\.0\.1:\d{1,5}\/(?:[^\s?#]*\/)?$/u.test(from))
		return { kind: 'loopback', base: from };
	if (path.isAbsolute(from)) return { kind: 'folder', base: from };
	throw new UsageError(usage);
}

function curlProgram() {
	return process.platform === 'win32'
		? path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'curl.exe')
		: 'curl';
}

/**
 * Runs curl anonymously for one address: `{ status }` and the bytes in `output`, or only the status for a probe. The
 * environment is passed on so proxy variables apply; `--disable` skips any curl configuration file.
 */
export function curl(source, url, { probe = false, output, limit }) {
	const protocol = source.kind === 'loopback' ? '=http' : '=https';
	const argv = [
		'--disable',
		'--silent',
		'--show-error',
		'--location',
		'--max-redirs',
		'5',
		'--proto',
		protocol,
		'--proto-redir',
		protocol,
		'--globoff',
		'--connect-timeout',
		'20',
		'--max-time',
		'600',
		...(source.kind === 'loopback' ? ['--noproxy', '127.0.0.1'] : []),
		// A one-byte GET, not HEAD: a storage host behind a redirect may sign its address for GET only.
		...(probe
			? ['--range', '0-0', '--output', process.platform === 'win32' ? 'NUL' : '/dev/null']
			: []),
		...(output === undefined ? [] : ['--max-filesize', String(limit), '--output', output]),
		'--write-out',
		'%{http_code}',
		url,
	];
	const result = spawnSync(curlProgram(), argv, {
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		windowsHide: true,
	});
	if (result.error !== undefined) throw new Error(`curl could not start: ${result.error.message}`);
	const status = Number.parseInt(result.stdout.trim().slice(-3), 10);
	// curl 63: the answer exceeded --max-filesize.
	if (result.status === 63) return { status, tooLarge: true };
	if (result.status !== 0)
		throw new Error(`curl exited ${result.status} for ${url}: ${result.stderr.trim()}`);
	return { status };
}

function notPublic(version, name, where) {
	return `${name} is not public at ${where}. If v${version} is still a draft, re-run the failed jobs of its release run and run this again; if the release lacks the file, release the next version.`;
}

/** The bytes of `name` from the source, at most `limit`, or a refusal sentence. */
function fetchBytes(source, version, name, limit, scratch) {
	if (source.kind === 'folder') {
		const filename = path.join(source.base, ...name.split('/'));
		const metadata = lstatSync(filename, { throwIfNoEntry: false });
		if (metadata === undefined || !metadata.isFile())
			return { problem: `${name} is missing from ${source.base}.` };
		if (metadata.size > limit) return { problem: `${name} exceeds ${limit} bytes.` };
		return { bytes: readFileSync(filename) };
	}
	const url = new URL(name, source.base).href;
	const output = path.join(scratch, `download-${createHash('sha256').update(name).digest('hex')}`);
	const { status, tooLarge } = curl(source, url, { output, limit });
	if (tooLarge) return { problem: `${name} at ${url} exceeds ${limit} bytes.` };
	if (status === 404) return { problem: notPublic(version, name, url) };
	if (status !== 200) return { problem: `${url} answered ${status}.` };
	const bytes = readFileSync(output);
	rmSync(output, { force: true });
	if (bytes.length > limit) return { problem: `${name} at ${url} exceeds ${limit} bytes.` };
	return { bytes };
}

/** Size and SHA-256 of a file, read in pieces. */
export function fileFacts(filename) {
	const hash = createHash('sha256');
	const piece = Buffer.alloc(1024 * 1024);
	const fd = openSync(filename, 'r');
	let size = 0;
	try {
		for (let read = readSync(fd, piece); read > 0; read = readSync(fd, piece)) {
			hash.update(piece.subarray(0, read));
			size += read;
		}
	} finally {
		closeSync(fd);
	}
	return { size, sha256: hash.digest('hex') };
}

/**
 * Size and SHA-256 of the archive the feed names, downloaded into the scratch folder and bounded
 * by the signed size, or a refusal sentence.
 */
function fetchArchive(source, version, feed, scratch) {
	const { url: address, size } = feed.archive;
	if (source.kind === 'folder') {
		const filename = path.join(source.base, ...address.split('/'));
		const metadata = lstatSync(filename, { throwIfNoEntry: false });
		if (metadata === undefined || !metadata.isFile())
			return { problem: `${address} is missing from ${source.base}.` };
		return { archive: fileFacts(filename) };
	}
	const url = new URL(address, source.base).href;
	const output = path.join(scratch, 'archive');
	const { status, tooLarge } = curl(source, url, { output, limit: size });
	// Longer than signed: report the mismatch as the size check would.
	if (tooLarge) return { archive: { size: size + 1, sha256: '' } };
	if (status === 404) return { problem: notPublic(version, path.posix.basename(url), url) };
	if (status !== 200) return { problem: `${url} answered ${status}.` };
	return { archive: fileFacts(output) };
}

function installerIsPublic(source, version, platform) {
	const name = installerName(version, platform);
	if (source.kind === 'folder') return existsSync(path.join(source.base, name));
	const { status } = curl(source, new URL(name, source.base).href, { probe: true });
	return status === 200 || status === 206;
}

/** Fetches every input, plans, and writes only when every check passed. */
export function websiteRelease({ version, from, root = repository, log = () => {} }) {
	if (!validVersion(version)) throw new UsageError(usage);
	const release = readFileSync(path.join(root, ...releaseModule.split('/')), 'utf8');
	const source = sourceFor({ version, from, repositoryUrl: repositoryUrlOf(release) });
	const standIn = source.kind !== 'https';
	const table =
		keyTable(
			readFileSync(path.join(root, ...signatureSource.split('/')), 'utf8'),
			standIn ? 'TEST_KEYS' : 'RELEASE_KEYS',
		) ?? [];
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-website-release-'));
	try {
		const checksums = fetchBytes(source, version, checksumsName, checksumsLimit, scratch);
		if (checksums.problem !== undefined) return refused([checksums.problem]);
		const published = publishedPlatforms(checksums.bytes, version);
		if (published.problems.length > 0) return refused(published.problems);
		log(`v${version} publishes ${published.platforms.join(', ')}.`);
		const rows = [];
		for (const platform of published.platforms) {
			const name = feedName(stem, version, platform);
			const feed = fetchBytes(source, version, name, FEED_LIMIT, scratch);
			if (feed.problem !== undefined) return refused([feed.problem]);
			const signature = fetchBytes(source, version, `${name}.sig`, signatureLimit, scratch);
			if (signature.problem !== undefined) return refused([signature.problem]);
			const admitted = admitFeed({
				version,
				platform,
				feed: feed.bytes,
				signature: signature.bytes,
				table,
				standIn,
			});
			if (admitted.problems.length > 0) return refused(admitted.problems);
			log(`The ${platform} feed verifies; checking its download and its archive.`);
			const installerPublic = installerIsPublic(source, version, platform);
			const archive = fetchArchive(source, version, admitted.feed, scratch);
			if (archive.problem !== undefined) return refused([archive.problem]);
			rows.push({
				platform,
				feed: feed.bytes,
				signature: signature.bytes,
				archive: archive.archive,
				installerPublic,
			});
		}
		const plan = planWebsiteRelease({
			version,
			rows,
			table,
			standIn,
			release,
			tracked: trackedPairs(root),
		});
		if (plan.problems.length === 0) applyWrites(root, plan.writes, plan.removals);
		return plan;
	} finally {
		rmSync(scratch, { force: true, recursive: true });
	}
}

function parseArguments(argv) {
	const [version, ...rest] = argv;
	if (version === undefined || version.startsWith('--')) throw new UsageError(usage);
	if (rest.length === 0) return { version };
	if (rest.length === 2 && rest[0] === '--from') return { version, from: rest[1] };
	throw new UsageError(usage);
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		const options = parseArguments(process.argv.slice(2));
		const plan = websiteRelease({
			...options,
			log: (line) => process.stdout.write(`${line}\n`),
		});
		if (plan.problems.length > 0) {
			for (const problem of plan.problems) process.stderr.write(`${problem}\n`);
			process.stderr.write('Nothing was written.\n');
			process.exitCode = 1;
		} else {
			for (const write of plan.writes) process.stdout.write(`Wrote ${write.path}\n`);
			for (const removal of plan.removals) process.stdout.write(`Removed ${removal}\n`);
			process.stdout.write(
				`The ${plan.channel} channel and the download buttons name ${options.version}. Commit these changes, then build and deploy the website.\n`,
			);
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
