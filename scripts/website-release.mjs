// Moves the website to a published release: its update channels and its download buttons together,
// and guards automated channel rollouts and live edge delivery.
//
//   pnpm website:release <version>
//   node scripts/website-release.mjs <version> [--from <absolute folder | http://127.0.0.1:<port>/<path>/>]
//   node scripts/website-release.mjs should-advance <version>
//   node scripts/website-release.mjs preconditions
//   node scripts/website-release.mjs assert-clean-diff [--since <git-ref>]
//   node scripts/website-release.mjs verify-live [--origin <https://host>] [--retries <n>] [--retry-delay-ms <ms>]
//
// Fetches the release's SHA256SUMS anonymously with the system curl, never with `gh`, so a draft
// release, whose assets answer 404 to anyone signed out, cannot pass, and a proxy variable works as
// it does for curl. The feed documents it lists, `orchbee-<version>-<platform>.feed.json`, name
// the platform rows the release published; `windows-x86_64` must be among them, and each must be a
// platform the website offers a download for (`downloads` below). For every row it fetches the
// feed pair, verifies the feed signature with RELEASE_KEYS of
// apps/orchbee/src/cli/update/signature.rs before reading the feed, requires schemaVersion 1 (the
// /update/<channel>/ endpoints are frozen at v1), checks the feed against the
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
// docs/RELEASING.md has the steps. Running it by hand with an older version moves the channel back;
// `should-advance <version>` skips older or equal versions in automated rollouts.
//
// `--from` stands in for the release address in the tooling's own checks: a folder or a loopback
// address holding a test build's pair. It trusts TEST_KEYS instead of RELEASE_KEYS and requires the
// feed to name its archive by a relative path, resolved beside the feed, so it never reaches the
// network; the website's own test refuses a pair that a release key did not sign.
//
// Exit codes: 0 written or check passed; 1 a check failed or a request failed; 2 malformed arguments.
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	appendFileSync,
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
import { compareVersions } from './release-version.mjs';
import { FEED_DOMAIN, keyTable, verifySignature } from './sign-release-manifest.mjs';

const repository = fileURLToPath(new URL('..', import.meta.url));
const signatureSource = 'apps/orchbee/src/cli/update/signature.rs';
const releaseModule = 'apps/website/src/website/release.ts';
const installScriptPath = 'apps/website/public/install.sh';
const updateRoot = 'apps/website/public/update';
const defaultLiveOrigin = 'https://orchbee.com';
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
const installScriptLimit = 256 * 1024;
const checksumsName = 'SHA256SUMS';
const checksumsLimit = 64 * 1024;
const websiteSecretNames = ['CLOUDFLARE_API_TOKEN', 'CLOUDFLARE_ACCOUNT_ID'];
const usage =
	'Usage: node scripts/website-release.mjs <version> [--from <absolute folder | http://127.0.0.1:<port>/<path>/>]\n' +
	'       node scripts/website-release.mjs should-advance <version>\n' +
	'       node scripts/website-release.mjs preconditions\n' +
	'       node scripts/website-release.mjs assert-clean-diff [--since <git-ref>]\n' +
	'       node scripts/website-release.mjs verify-live [--origin <https://host>] [--retries <n>] [--retry-delay-ms <ms>]';
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
 * Reads the current `release` export from `release.ts`: `{ published: false }`, or
 * `{ published: true, version }` when published with a valid SemVer version.
 */
export function currentReleaseOf(source) {
	const match = releaseBlock.exec(source);
	if (match === null)
		throw new Error(`${releaseModule} has no \`export const release: Release = {…};\` block.`);
	const [block] = match;
	const published = /^\tpublished: (true|false),$/mu.exec(block)?.[1];
	if (published === 'false') return { published: false };
	if (published === 'true') {
		const version = /^\tversion: '([^']+)',$/mu.exec(block)?.[1];
		if (!validVersion(version))
			throw new Error(`${releaseModule} has no valid SemVer release.version.`);
		return { published: true, version };
	}
	throw new Error(`${releaseModule} has an unrecognized published state.`);
}

/**
 * Decides whether automated rollout should advance `release.ts` to `version` under SemVer 2.0.0
 * precedence: skips cleanly when `release.published === true` and `version <= release.version`.
 */
export function shouldAdvanceWebsite(source, version) {
	if (!validVersion(version)) throw new UsageError(usage);
	const current = currentReleaseOf(source);
	if (!current.published) {
		return {
			advance: true,
			currentVersion: null,
			message: `${releaseModule} has no published release yet; advancing to ${version}.`,
		};
	}
	if (compareVersions(version, current.version) > 0) {
		return {
			advance: true,
			currentVersion: current.version,
			message: `Advancing the website channel from ${current.version} to ${version}.`,
		};
	}
	return {
		advance: false,
		currentVersion: current.version,
		message: `The website channel is already at ${current.version}, which is at or above ${version}; skipping rollout.`,
	};
}

/**
 * Checks that every deployment secret of the GitHub environment `website` is present and non-empty,
 * without ever echoing a secret value.
 */
export function checkWebsitePreconditions(environment = process.env) {
	const problems = [];
	const notes = [];
	for (const name of websiteSecretNames) {
		const value = environment[name];
		if (value === undefined || value.trim() === '') {
			problems.push(
				`${name} is empty: set it on the GitHub environment website with \`gh secret set ${name} --env website\`.`,
			);
		} else {
			notes.push(`${name} is set.`);
		}
	}
	return { problems, notes };
}

/**
 * Whether `relativePath` is one of the exact repository paths `website:release` is allowed to
 * write or remove.
 */
export function allowedWebsiteReleasePath(relativePath) {
	return (
		relativePath === releaseModule ||
		/^apps\/website\/public\/update\/(?:stable|preview)\/[a-z0-9][a-z0-9_-]*\/feed\.json\.(?:signed|sig)$/u.test(
			relativePath,
		)
	);
}

/** Parses NUL-delimited `git status --porcelain=v1 -z --untracked-files=all` output. */
export function parseGitPorcelainZ(output) {
	const tokens = output.split('\0');
	if (tokens.at(-1) === '') tokens.pop();
	const paths = [];
	const problems = [];
	for (let index = 0; index < tokens.length; index += 1) {
		const entry = tokens[index];
		if (entry === undefined || entry.length < 4 || entry[2] !== ' ') {
			problems.push(`Malformed git status entry: ${JSON.stringify(entry)}`);
			continue;
		}
		const status = entry.slice(0, 2);
		const file = entry.slice(3);
		paths.push(file);
		if (status.includes('U') || status === 'AA' || status === 'DD') {
			problems.push(`Unmerged path in worktree: ${file}`);
		}
		if (status.includes('R') || status.includes('C')) {
			const original = tokens[++index];
			if (original !== undefined) paths.push(original);
			problems.push(`Unexpected rename or copy (${status.trim()}): ${original ?? '?'} -> ${file}`);
		}
	}
	return { paths, problems };
}

/**
 * Verifies that only allowed website release files changed: in the worktree/index when `since` is
 * omitted, or in `since..HEAD` with a clean worktree when `since` is given.
 */
export function checkWebsiteDiff({ statusZ, committedZ = '', since }) {
	const parsed = parseGitPorcelainZ(statusZ);
	const problems = [...parsed.problems];
	if (since !== undefined) {
		for (const dirty of parsed.paths) {
			problems.push(`Worktree is not clean after commit: ${dirty}`);
		}
		const committed = committedZ.split('\0').filter((entry) => entry !== '');
		if (committed.length === 0) {
			problems.push(`No website release files changed between ${since} and HEAD.`);
		}
		for (const file of committed) {
			if (!allowedWebsiteReleasePath(file)) {
				problems.push(`Disallowed path in ${since}..HEAD: ${file}`);
			}
		}
		return { problems, paths: committed };
	}
	if (parsed.paths.length === 0) {
		problems.push('No website release files changed in worktree.');
	}
	for (const file of parsed.paths) {
		if (!allowedWebsiteReleasePath(file)) {
			problems.push(`Disallowed modified or untracked path: ${file}`);
		}
	}
	return { problems, paths: parsed.paths };
}

function runGitCapture(root, argv) {
	const result = spawnSync('git', argv, {
		cwd: root,
		encoding: 'utf8',
		stdio: ['ignore', 'pipe', 'pipe'],
		maxBuffer: 16 * 1024 * 1024,
		windowsHide: true,
	});
	if (result.error !== undefined) throw new Error(`git could not start: ${result.error.message}`);
	if (result.status !== 0)
		throw new Error(`git ${argv[0]} exited ${result.status}: ${result.stderr.trim()}`);
	return result.stdout;
}

/** Runs the strict git path whitelist check against `root`. */
export function assertCleanWebsiteDiff({ root = repository, since } = {}) {
	const statusZ = runGitCapture(root, ['status', '--porcelain=v1', '-z', '--untracked-files=all']);
	const committedZ =
		since === undefined
			? ''
			: runGitCapture(root, ['diff', '--no-renames', '--name-only', '-z', since, 'HEAD', '--']);
	return checkWebsiteDiff({ statusZ, committedZ, since });
}

/**
 * The feed of a fetched pair when it may move the channel to `version`, and otherwise the sentences
 * that say why not. The signature is verified before the feed is read, and `/update/<channel>/`
 * refuses any feed whose `schemaVersion` is not 1.
 */
export function admitFeed({ version, platform, feed, signature, table, standIn }) {
	const { outcome } = verifySignature(feed, signature, table, FEED_DOMAIN);
	if (outcome !== 'trusted')
		return {
			problems: [
				`The ${platform} feed signature does not verify with a key of ${standIn ? 'TEST_KEYS' : 'RELEASE_KEYS'} (${outcome}). Do not move the channel to this release.`,
			],
		};
	let rawSchemaVersion;
	try {
		const raw = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(feed));
		if (typeof raw === 'object' && raw !== null && !Array.isArray(raw))
			rawSchemaVersion = raw.schemaVersion;
	} catch {
		rawSchemaVersion = undefined;
	}
	if (rawSchemaVersion !== undefined && rawSchemaVersion !== 1)
		return {
			problems: [
				`The ${platform} feed has schemaVersion ${JSON.stringify(rawSchemaVersion)}, not 1: ${updateRoot}/<channel>/ is frozen at schemaVersion 1 for v1 clients; serve breaking feed formats under a new versioned path (such as /update/v2/) while keeping ${updateRoot}/<channel>/ on a bridge release.`,
			],
		};
	const admitted = readFeed(feed, { platform });
	if (admitted.problems.length > 0)
		return {
			problems: admitted.problems.map(
				(problem) => `The ${platform} feed breaks a reader rule: ${problem}.`,
			),
		};
	if (admitted.feed.schemaVersion !== 1)
		return {
			problems: [
				`The ${platform} feed has schemaVersion ${JSON.stringify(admitted.feed.schemaVersion)}, not 1: ${updateRoot}/<channel>/ only serves schemaVersion 1 feeds.`,
			],
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
		admittedRows.push({
			...row,
			channel: admitted.feed.channel,
			schemaVersion: admitted.feed.schemaVersion,
		});
	}
	const channels = [...new Set(admittedRows.map((row) => row.channel))];
	if (channels.length > 1)
		problems.push(`The feeds name several channels, ${channels.join(' and ')}.`);
	if (admittedRows.some((row) => row.schemaVersion !== 1))
		problems.push(`${updateRoot}/<channel>/ only accepts schemaVersion 1 feeds.`);
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

function sleep(milliseconds) {
	if (milliseconds > 0) Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, milliseconds);
}

function liveOriginFor(origin) {
	const trimmed = String(origin ?? '').replace(/\/+$/u, '');
	if (/^https:\/\/[A-Za-z0-9.:-]+$/u.test(trimmed)) return { kind: 'https', origin: trimmed };
	if (/^http:\/\/127\.0\.0\.1:\d{1,5}$/u.test(trimmed))
		return { kind: 'loopback', origin: trimmed };
	throw new UsageError(usage);
}

function looksLikeHtml(bytes) {
	return /^\s*<(?:!doctype\s+html|html)\b/iu.test(bytes.subarray(0, 512).toString('utf8'));
}

function sha256HexOf(bytes) {
	return createHash('sha256').update(bytes).digest('hex');
}

/**
 * Every live URL that clients and the install script fetch, paired with the tracked repository
 * bytes each must match byte for byte.
 */
export function liveTargets(root = repository) {
	const pairs = trackedPairs(root).toSorted();
	if (pairs.length === 0) {
		throw new Error(`No tracked update pairs exist under ${updateRoot}.`);
	}
	const targets = [];
	for (const folder of pairs) {
		const [channel, platform] = folder.split('/');
		const localSigned = `${updateRoot}/${channel}/${platform}/feed.json.signed`;
		const localSig = `${updateRoot}/${channel}/${platform}/feed.json.sig`;
		const signedPath = path.join(root, ...localSigned.split('/'));
		const sigPath = path.join(root, ...localSig.split('/'));
		if (!existsSync(signedPath) || !existsSync(sigPath)) {
			throw new Error(`${updateRoot}/${folder} must hold both feed.json.signed and feed.json.sig.`);
		}
		targets.push(
			{
				remotePath: `/update/${channel}/${platform}/feed.json`,
				localPath: localSigned,
				expected: readFileSync(signedPath),
				limit: FEED_LIMIT,
			},
			{
				remotePath: `/update/${channel}/${platform}/feed.json.sig`,
				localPath: localSig,
				expected: readFileSync(sigPath),
				limit: signatureLimit,
			},
		);
	}
	const installFile = path.join(root, ...installScriptPath.split('/'));
	if (!existsSync(installFile)) {
		throw new Error(`${installScriptPath} is missing.`);
	}
	targets.push({
		remotePath: '/install.sh',
		localPath: installScriptPath,
		expected: readFileSync(installFile),
		limit: installScriptLimit,
	});
	return targets;
}

/**
 * Fetches one live asset anonymously with `--fail`, matching how clients and `install.sh` invoke
 * system `curl`.
 */
function curlLive(originKind, url, output, limit) {
	const protocol = originKind === 'loopback' ? '=http' : '=https';
	const argv = [
		'--disable',
		'--fail',
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
		'60',
		...(originKind === 'loopback' ? ['--noproxy', '127.0.0.1'] : []),
		'--max-filesize',
		String(limit),
		'--output',
		output,
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
	const statusText = result.stdout.trim().slice(-3);
	const status = /^\d{3}$/u.test(statusText) ? Number.parseInt(statusText, 10) : 0;
	if (result.status === 63) return { ok: false, status, tooLarge: true };
	if (result.status === 22)
		return { ok: false, status, error: result.stderr.trim() || `HTTP ${status}` };
	if (result.status !== 0)
		return {
			ok: false,
			status,
			error: result.stderr.trim() || `curl exited ${result.status}`,
		};
	const bytes = existsSync(output) ? readFileSync(output) : Buffer.alloc(0);
	rmSync(output, { force: true });
	return { ok: true, status, bytes };
}

const challengeAdvice =
	'Keep Cloudflare Bot Fight Mode, Browser Integrity Check and challenge rules disabled for /update/* and /install.sh.';

/**
 * Verifies every tracked `/update/<channel>/<platform>/feed.json`, `.sig` and `/install.sh` on the
 * live origin byte for byte against the repository's tracked files, with bounded retries for brief
 * CDN edge propagation.
 */
export function verifyLiveWebsite({
	root = repository,
	origin = defaultLiveOrigin,
	retries = 6,
	retryDelayMs = 5_000,
	fetchRemote,
	sleepImpl = sleep,
	log = () => {},
} = {}) {
	const parsedOrigin = liveOriginFor(origin);
	if (
		!Number.isSafeInteger(retries) ||
		retries < 0 ||
		retries > 30 ||
		!Number.isSafeInteger(retryDelayMs) ||
		retryDelayMs < 0 ||
		retryDelayMs > 60_000
	) {
		throw new UsageError(usage);
	}
	const targets = liveTargets(root);
	const scratch = mkdtempSync(path.join(tmpdir(), 'orchbee-verify-live-'));
	const fetcher =
		fetchRemote ??
		((url, limit, index) =>
			curlLive(parsedOrigin.kind, url, path.join(scratch, `live-${index}`), limit));
	try {
		let problems = [];
		for (let attempt = 0; attempt <= retries; attempt += 1) {
			problems = [];
			for (const [index, target] of targets.entries()) {
				const url = `${parsedOrigin.origin}${target.remotePath}`;
				const response = fetcher(url, target.limit, index);
				if (!response.ok) {
					if (response.tooLarge) {
						problems.push(`${url} exceeded ${target.limit} bytes.`);
					} else if ([401, 403, 429, 503].includes(response.status)) {
						problems.push(
							`${url} answered HTTP ${response.status} (${response.error ?? 'blocked'}). ${challengeAdvice}`,
						);
					} else {
						problems.push(
							`${url} failed (HTTP ${response.status || 'none'}): ${response.error ?? 'request failed'}.`,
						);
					}
					continue;
				}
				if (!response.bytes.equals(target.expected)) {
					const htmlNote = looksLikeHtml(response.bytes)
						? ` The response looks like an HTML page (possible CDN challenge). ${challengeAdvice}`
						: '';
					problems.push(
						`${url} (${response.bytes.length} bytes, sha256 ${sha256HexOf(response.bytes)}) does not match ${target.localPath} (${target.expected.length} bytes, sha256 ${sha256HexOf(target.expected)}).${htmlNote}`,
					);
				}
			}
			if (problems.length === 0) {
				for (const target of targets) {
					log(
						`Verified ${parsedOrigin.origin}${target.remotePath} (${target.expected.length} bytes).`,
					);
				}
				return { problems: [], verified: targets.length, attempts: attempt + 1 };
			}
			if (attempt < retries) {
				log(
					`Live check attempt ${attempt + 1} of ${retries + 1} had ${problems.length} mismatch(es); retrying in ${retryDelayMs} ms...`,
				);
				sleepImpl(retryDelayMs);
			}
		}
		return { problems, verified: 0, attempts: retries + 1 };
	} finally {
		rmSync(scratch, { force: true, recursive: true });
	}
}

function writeGithubOutputs(values) {
	const target = process.env.GITHUB_OUTPUT;
	if (target === undefined || target === '') return;
	const lines = Object.entries(values).map(([key, value]) => `${key}=${value}\n`);
	appendFileSync(target, lines.join(''));
}

export function parseArguments(argv) {
	const [first, ...rest] = argv;
	if (first === undefined || first.startsWith('--')) throw new UsageError(usage);
	if (first === 'should-advance') {
		if (rest.length !== 1 || !validVersion(rest[0])) throw new UsageError(usage);
		return { command: 'should-advance', version: rest[0] };
	}
	if (first === 'preconditions') {
		if (rest.length !== 0) throw new UsageError(usage);
		return { command: 'preconditions' };
	}
	if (first === 'assert-clean-diff') {
		if (rest.length === 0) return { command: 'assert-clean-diff' };
		if (rest.length === 2 && rest[0] === '--since' && rest[1] !== '' && !rest[1].startsWith('-'))
			return { command: 'assert-clean-diff', since: rest[1] };
		throw new UsageError(usage);
	}
	if (first === 'verify-live') {
		const options = {
			command: 'verify-live',
			origin: defaultLiveOrigin,
			retries: 6,
			retryDelayMs: 5_000,
		};
		for (let index = 0; index < rest.length; index += 2) {
			const [flag, value] = [rest[index], rest[index + 1]];
			if (value === undefined) throw new UsageError(usage);
			if (flag === '--origin') {
				options.origin = value;
			} else if (flag === '--retries' && /^\d+$/u.test(value)) {
				options.retries = Number.parseInt(value, 10);
			} else if (flag === '--retry-delay-ms' && /^\d+$/u.test(value)) {
				options.retryDelayMs = Number.parseInt(value, 10);
			} else {
				throw new UsageError(usage);
			}
		}
		liveOriginFor(options.origin);
		return options;
	}
	if (rest.length === 0) return { command: 'release', version: first };
	if (rest.length === 2 && rest[0] === '--from')
		return { command: 'release', version: first, from: rest[1] };
	throw new UsageError(usage);
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	try {
		const options = parseArguments(process.argv.slice(2));
		const log = (line) => process.stdout.write(`${line}\n`);
		if (options.command === 'should-advance') {
			const source = readFileSync(path.join(repository, ...releaseModule.split('/')), 'utf8');
			const decision = shouldAdvanceWebsite(source, options.version);
			log(decision.message);
			writeGithubOutputs({
				advance: String(decision.advance),
				'current-version': decision.currentVersion ?? '',
			});
		} else if (options.command === 'preconditions') {
			const { problems, notes } = checkWebsitePreconditions(process.env);
			for (const note of notes) log(note);
			if (problems.length > 0) {
				for (const problem of problems) process.stderr.write(`${problem}\n`);
				process.exitCode = 1;
			}
		} else if (options.command === 'assert-clean-diff') {
			const { problems, paths } = assertCleanWebsiteDiff({ since: options.since });
			if (problems.length > 0) {
				for (const problem of problems) process.stderr.write(`${problem}\n`);
				process.exitCode = 1;
			} else {
				log(`Verified ${paths.length} allowed website release path(s): ${paths.join(', ')}.`);
			}
		} else if (options.command === 'verify-live') {
			const result = verifyLiveWebsite({ ...options, log });
			if (result.problems.length > 0) {
				for (const problem of result.problems) process.stderr.write(`${problem}\n`);
				process.exitCode = 1;
			} else {
				log(
					`All ${result.verified} live website update and install endpoints match byte for byte.`,
				);
			}
		} else {
			const plan = websiteRelease({ ...options, log });
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
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
