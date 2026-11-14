// Moves the website to a published release: its update channel and its download button together.
//
//   pnpm website:release <version>
//   node scripts/website-release.mjs <version> [--from <absolute folder | http://127.0.0.1:<port>/<path>/>]
//
// Fetches the release's feed pair (`orchbee-<version>-windows-x86_64.feed.json` and `.sig`)
// anonymously with the system curl, never with `gh`, so a draft release, whose assets answer 404 to
// anyone signed out, cannot pass, and a proxy variable works as it does for curl. It verifies the
// feed signature with RELEASE_KEYS of apps/orchbee/src/cli/update/signature.rs before reading the
// feed, checks the feed against the reader's rules, its written form and the requested version, confirms the
// installer is public, downloads the archive the feed names, bounded by its signed size, and checks
// its size and SHA-256. Only then does it write, in one step:
//
//   apps/website/public/update/<channel>/<platform>/feed.json.signed   the exact feed bytes, served
//                                                                      as feed.json
//   apps/website/public/update/<channel>/<platform>/feed.json.sig      the signature document
//   apps/website/src/website/release.ts                                version and installerUrl
//
// The first failed check stops the command, and nothing is written. It needs no credentials and
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
	readSync,
	renameSync,
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
// The one platform the release workflow publishes today.
const platform = 'windows-x86_64';
const stem = 'orchbee';
const signatureLimit = 4 * 1024;
const usage =
	'Usage: node scripts/website-release.mjs <version> [--from <absolute folder | http://127.0.0.1:<port>/<path>/>]';
const releaseBlock = /^export const release: Release = \{[\s\S]*?\n\};\n/mu;

class UsageError extends Error {}

/** The installer asset of a version, by the release distribution's download names. */
export function installerName(version) {
	return `Orchbee-${version}-${platform}-setup.exe`;
}

/** The repository address release.ts names, or undefined when it names none. */
export function repositoryUrlOf(source) {
	return /^export const REPOSITORY_URL = '(https:\/\/[^'\s]+)';$/mu.exec(source)?.[1];
}

/** release.ts with the release value set to `version`, or undefined when its block is not found. */
export function releaseSource(source, version) {
	if (!releaseBlock.test(source)) return undefined;
	const block =
		'export const release: Release = {\n' +
		'\tpublished: true,\n' +
		`\tversion: '${version}',\n` +
		`\tinstallerUrl: \`\${REPOSITORY_URL}/releases/download/v${version}/${installerName(version)}\`,\n` +
		'};\n';
	return source.replace(releaseBlock, block);
}

/**
 * The feed of a fetched pair when it may move the channel to `version`, and otherwise the sentences
 * that say why not. The signature is verified before the feed is read.
 */
export function admitFeed({ version, feed, signature, table, standIn }) {
	const { outcome } = verifySignature(feed, signature, table, FEED_DOMAIN);
	if (outcome !== 'trusted')
		return {
			problems: [
				`The feed signature does not verify with a key of ${standIn ? 'TEST_KEYS' : 'RELEASE_KEYS'} (${outcome}). Do not move the channel to this release.`,
			],
		};
	const admitted = readFeed(feed, { platform });
	if (admitted.problems.length > 0)
		return {
			problems: admitted.problems.map((problem) => `The feed breaks a reader rule: ${problem}.`),
		};
	// The updater's reader refuses some spellings JSON.parse admits; the written form has none.
	if (!writtenForm(feed, admitted.feed))
		return {
			problems: [
				'The feed is not in its written form, so the updater may refuse it. Do not move the channel to this release.',
			],
		};
	const problems = [];
	if (admitted.feed.version !== version)
		problems.push(`The feed names version ${admitted.feed.version}, not ${version}.`);
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
 * Every file the command writes, as `{ path, bytes }` relative to the repository, or the sentences
 * that say why nothing may be written. Pure: the caller fetched every input.
 */
export function planWebsiteRelease({
	version,
	feed,
	signature,
	table,
	standIn = false,
	archive,
	installerPublic,
	release,
}) {
	const admitted = admitFeed({ version, feed, signature, table, standIn });
	if (admitted.problems.length > 0) return { problems: admitted.problems, writes: [] };
	const expected = admitted.feed.archive;
	const problems = [];
	if (!installerPublic)
		problems.push(`${installerName(version)} is not public; the download button would break.`);
	if (archive === undefined) {
		problems.push('The archive was not fetched.');
	} else {
		if (archive.size !== expected.size)
			problems.push(
				`The archive has ${archive.size} bytes, but the signed feed says ${expected.size}. Do not move the channel to this release.`,
			);
		if (archive.sha256 !== expected.sha256)
			problems.push(
				'The archive does not match the SHA-256 the signed feed names. Do not move the channel to this release.',
			);
	}
	const source = releaseSource(release, version);
	if (source === undefined)
		problems.push(`${releaseModule} has no \`export const release: Release = {…};\` block to set.`);
	if (problems.length > 0) return { problems, writes: [] };
	const folder = `${updateRoot}/${admitted.feed.channel}/${platform}`;
	return {
		problems: [],
		channel: admitted.feed.channel,
		writes: [
			{ path: `${folder}/feed.json.signed`, bytes: feed },
			{ path: `${folder}/feed.json.sig`, bytes: signature },
			{ path: releaseModule, bytes: Buffer.from(source, 'utf8') },
		],
	};
}

/** Writes every planned file under `root`, each by a flushed rename of a sibling. */
export function applyWrites(root, writes) {
	for (const { path: relative, bytes } of writes) {
		const target = path.join(root, ...relative.split('/'));
		mkdirSync(path.dirname(target), { recursive: true });
		const pending = `${target}.pending`;
		writeFileSync(pending, bytes, { flag: 'w' });
		renameSync(pending, target);
	}
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
function curl(source, url, { probe = false, output, limit }) {
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
	return `${name} is not public at ${where}. If v${version} is still a draft, publish it on GitHub (Publish release) and run this again; if the release lacks the file, release the next version.`;
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
function fileFacts(filename) {
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

function installerIsPublic(source, version) {
	const name = installerName(version);
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
		const name = feedName(stem, version, platform);
		const feed = fetchBytes(source, version, name, FEED_LIMIT, scratch);
		if (feed.problem !== undefined) return { problems: [feed.problem], writes: [] };
		const signature = fetchBytes(source, version, `${name}.sig`, signatureLimit, scratch);
		if (signature.problem !== undefined) return { problems: [signature.problem], writes: [] };
		const admitted = admitFeed({
			version,
			feed: feed.bytes,
			signature: signature.bytes,
			table,
			standIn,
		});
		if (admitted.problems.length > 0) return { problems: admitted.problems, writes: [] };
		log(`The feed of ${version} verifies; checking the installer and the archive.`);
		const installerPublic = installerIsPublic(source, version);
		const archive = fetchArchive(source, version, admitted.feed, scratch);
		if (archive.problem !== undefined) return { problems: [archive.problem], writes: [] };
		const plan = planWebsiteRelease({
			version,
			feed: feed.bytes,
			signature: signature.bytes,
			table,
			standIn,
			archive: archive.archive,
			installerPublic,
			release,
		});
		if (plan.problems.length === 0) applyWrites(root, plan.writes);
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
			process.stdout.write(
				`The ${plan.channel} channel and the download button name ${options.version}. Commit these files, then build and deploy the website.\n`,
			);
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = error instanceof UsageError ? 2 : 1;
	}
}
