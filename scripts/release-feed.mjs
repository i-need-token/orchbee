// The update feed document, version 1: one small signed document per version and platform that
// names one archive by address, exact size and SHA-256. `orchbee update` verifies its signature,
// then downloads the archive and checks size and digest before unpacking it. The format, the
// signature domain (sign-release-manifest.mjs) and the served address are frozen from the first
// version that ships them; see proposals/platform/release-distribution.md#release-feed.
//
// The written form is `JSON.stringify` of the members below in this order, then one LF. A reader
// accepts any JSON spelling of the same members, and the signature covers the exact bytes, so a
// written document is never reformatted. Node built-ins only: the release workflow's publish job
// imports this module in the step that holds the signing key.
import { createHash } from 'node:crypto';

// A reader's bounds; they never shrink.
export const FEED_LIMIT = 64 * 1024;
export const ARCHIVE_LIMIT = 1024 * 1024 * 1024;

const members = [
	'schemaVersion',
	'product',
	'channel',
	'platform',
	'version',
	'buildId',
	'manifest',
	'archive',
];
const channels = ['stable', 'preview'];
const semver =
	/^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)(?:-(?:0|[1-9]\d*|\d*[a-zA-Z-][\da-zA-Z-]*)(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][\da-zA-Z-]*))*)?$/u;
const sha256Hex = /^[0-9a-f]{64}$/u;
const reservedLeaf = /^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/u;

/** The release asset name of the feed document of one version and platform. */
export function feedName(stem, version, platform) {
	return `${stem}-${version}-${platform}.feed.json`;
}

/** The lowercase SHA-256 of `bytes`. */
export function sha256(bytes) {
	return createHash('sha256').update(bytes).digest('hex');
}

function isObject(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/** True when `value` has exactly the `required` members plus any of the `optional` ones. */
function closed(value, required, optional = []) {
	if (!isObject(value)) return false;
	const names = Object.keys(value);
	return (
		required.every((name) => names.includes(name)) &&
		names.every((name) => required.includes(name) || optional.includes(name))
	);
}

function token(value, limit, underscore) {
	return (
		typeof value === 'string' &&
		value.length > 0 &&
		value.length <= limit &&
		(underscore ? /^[a-z0-9_-]+$/u : /^[a-z0-9-]+$/u).test(value)
	);
}

/** SemVer 2.0.0 of at most 64 bytes, without build metadata. */
export function validVersion(value) {
	return typeof value === 'string' && value.length <= 64 && semver.test(value);
}

function label(value) {
	return (
		typeof value === 'string' &&
		value.length > 0 &&
		value.length <= 96 &&
		/^[A-Za-z0-9.\-_+]+$/u.test(value)
	);
}

/** A literal HTTPS address curl receives unchanged: no userinfo, controls, fragment or query. */
function validHttps(value) {
	if (typeof value !== 'string' || !value.startsWith('https://') || value.length > 4096)
		return false;
	if (!/^[\x21-\x7e]+$/u.test(value) || /[`\\"'<>#?]/u.test(value)) return false;
	const rest = value.slice('https://'.length);
	const authority = rest.split('/')[0];
	return rest.includes('/') && authority.length > 0 && /^[A-Za-z0-9.:-]+$/u.test(authority);
}

/** A relative release path, resolved beside the feed. */
function validReleasePath(value) {
	if (typeof value !== 'string' || value.length === 0 || value.length > 240) return false;
	const parts = value.split('/');
	return (
		parts.length <= 12 &&
		parts.every(
			(part) =>
				part.length > 0 &&
				part.length <= 120 &&
				part !== '.' &&
				part !== '..' &&
				!part.endsWith('.') &&
				/^[A-Za-z0-9._-]+$/u.test(part) &&
				!reservedLeaf.test(part.split('.')[0].toUpperCase()),
		)
	);
}

/**
 * The feed of `bytes` and every reader rule it breaks; none means `orchbee update` admits it.
 * `channel` and `platform`, when given, must match.
 */
export function readFeed(bytes, { channel, platform } = {}) {
	if (bytes.length > FEED_LIMIT) return { problems: ['The feed document exceeds 64 KiB'] };
	let feed;
	try {
		feed = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
	} catch {
		return { problems: ['The feed document is not UTF-8 JSON'] };
	}
	if (!closed(feed, members, ['extensions']))
		return { problems: [`The feed document must have exactly the members ${members.join(', ')}`] };
	const problems = [];
	const check = (valid, problem) => {
		if (!valid) problems.push(problem);
	};
	check(feed.extensions === undefined || isObject(feed.extensions), 'extensions is not an object');
	check(feed.schemaVersion === 1, 'schemaVersion is not 1');
	check(feed.product === 'orchbee', 'product is not orchbee');
	check(token(feed.channel, 32, false) && channels.includes(feed.channel), 'channel is invalid');
	check(token(feed.platform, 32, true), 'platform is invalid');
	check(validVersion(feed.version), 'version is not a SemVer version of at most 64 bytes');
	check(label(feed.buildId), 'buildId is invalid');
	check(
		closed(feed.manifest, ['sha256']) && sha256Hex.test(feed.manifest.sha256),
		'manifest must be exactly a lowercase sha256',
	);
	const { archive } = feed;
	if (!closed(archive, ['format', 'url', 'size', 'sha256'])) {
		problems.push('archive must have exactly format, url, size and sha256');
	} else {
		check(archive.format === 'zip', 'archive.format is not zip');
		check(
			validHttps(archive.url) || validReleasePath(archive.url),
			'archive.url is neither an HTTPS address without a query nor a relative release path',
		);
		check(
			Number.isSafeInteger(archive.size) && archive.size >= 1 && archive.size <= ARCHIVE_LIMIT,
			'archive.size is not an integer from 1 to 1 GiB',
		);
		check(sha256Hex.test(archive.sha256), 'archive.sha256 is not lowercase SHA-256');
	}
	if (channel !== undefined && feed.channel !== channel)
		problems.push(`The feed is for the channel ${feed.channel}, not ${channel}`);
	if (platform !== undefined && feed.platform !== platform)
		problems.push(`The feed is for the platform ${feed.platform}, not ${platform}`);
	return { feed, problems };
}

/** True when an archive address is absolute; otherwise it resolves beside the feed. */
export function absoluteArchive(url) {
	return validHttps(url);
}

/**
 * The exact bytes of a feed document. Throws when the values break a reader rule, so the release
 * tooling never writes a document `orchbee update` refuses.
 */
export function feedDocument({ channel, platform, version, buildId, manifestSha256, archive }) {
	const bytes = Buffer.from(
		JSON.stringify({
			schemaVersion: 1,
			product: 'orchbee',
			channel,
			platform,
			version,
			buildId,
			manifest: { sha256: manifestSha256 },
			archive: { format: 'zip', url: archive.url, size: archive.size, sha256: archive.sha256 },
		}) + '\n',
		'utf8',
	);
	const { problems } = readFeed(bytes);
	if (problems.length > 0) throw new Error(problems.join('\n'));
	return bytes;
}

/** True when `bytes` is the written form of its own members. */
export function writtenForm(bytes, feed) {
	return Buffer.from(JSON.stringify(feed) + '\n', 'utf8').equals(bytes);
}
