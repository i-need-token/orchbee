// The window file list and the window archive that carries it; see
// proposals/platform/automatic-updates.md#window-archive-and-file-list.
//
// Every packaged window directory holds `desktop-files.json` at its root:
// `{"schemaVersion": 1, "version", "files": [{"path", "size", "sha256"}]}`, plus an optional
// `extensions` object that readers check is an object and otherwise ignore. It names every other
// regular file of the directory once, by a `/`-separated path, under the release manifest's file
// rules (scripts/package-native-release.mjs), and is itself at most 1 MiB. The update feed binds it
// by its SHA-256 in `extensions.desktop.manifest` (scripts/release-feed.mjs), so the feed signature
// covers every file of the window. The release zips the directory, list included, as
// `<stem>-desktop-<version>-<platform>.zip` under one top-level folder of that name.
//
// On macOS the window directory holds the list beside the signed `<name>.app`, since a file at a
// bundle's root breaks its signature's seal, and the list names only the bundle. There a path
// starts with `<name>.app/` and a part also admits spaces and parentheses; a file its owner may run
// carries `"executable": true`; and the bundle's symbolic links form the optional member
// `links: [{"path", "target"}]`, each target relative, inside the bundle, and naming a listed file,
// a listed link or a folder holding listed entries. Files and links share the 4096 entries and
// their uniqueness ignoring case. Windows lists carry none of these.
//
//   node scripts/desktop-files.mjs write <absolute packaged window directory> <version> [<name>.app]
//
// apps/desktop/scripts/package.mjs runs `write` after the packager returns, after signing on macOS,
// and before Setup or the disk image is made, so the installer and the archive carry the same list.
// The written form is `JSON.stringify` of the members in this order, files and links in path order,
// then one LF. Node built-ins only.
import {
	closeSync,
	fsyncSync,
	lstatSync,
	openSync,
	readFileSync,
	readdirSync,
	readlinkSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

import { releaseFiles, validReleaseFilePath } from './package-native-release.mjs';
import { sha256, validVersion } from './release-feed.mjs';

/** The list's name at the root of a window directory. */
export const DESKTOP_FILES = 'desktop-files.json';
// A reader's bounds; they never shrink.
const listLimit = 1024 * 1024;
const fileLimit = 512 * 1024 * 1024;
const totalLimit = 2 * 1024 * 1024 * 1024;
const fileCountLimit = 4096;
const sha256Hex = /^[0-9a-f]{64}$/u;

/** The name of the window archive of one version and platform, and of its top-level folder. */
export function desktopReleaseName(stem, version, platform) {
	return `${stem}-desktop-${version}-${platform}`;
}

function isObject(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function closed(value, required, optional = []) {
	if (!isObject(value)) return false;
	const names = Object.keys(value);
	return (
		required.every((name) => names.includes(name)) &&
		names.every((name) => required.includes(name) || optional.includes(name))
	);
}

/** A macOS list path: the release path grammar, whose parts also admit spaces and parentheses. */
function validBundlePath(value) {
	return (
		typeof value === 'string' &&
		value.length <= 240 &&
		value.split('/').length <= 12 &&
		value
			.split('/')
			.every(
				(part) =>
					part.length > 0 &&
					part.length <= 120 &&
					/^[A-Za-z0-9._ ()-]+$/u.test(part) &&
					part.trim() === part &&
					!part.endsWith('.'),
			)
	);
}

/**
 * The regular files and symbolic links of the bundle `application` inside `directory`, by
 * `<application>/`-prefixed paths in path order; folders are walked, never followed through a link.
 */
function bundleEntries(directory, application) {
	const files = [];
	const links = [];
	let count = 0;
	function visit(folder, prefix) {
		for (const entry of readdirSync(folder, { withFileTypes: true })) {
			const relative = `${prefix}/${entry.name}`;
			if (++count > 2 * fileCountLimit) throw new Error('The bundle holds too many entries');
			const filename = path.join(folder, entry.name);
			const metadata = lstatSync(filename);
			if (metadata.isSymbolicLink()) links.push({ path: relative, target: readlinkSync(filename) });
			else if (metadata.isDirectory()) visit(filename, relative);
			else if (metadata.isFile() && metadata.nlink === 1) {
				const bytes = readFileSync(filename);
				files.push({
					path: relative,
					size: bytes.length,
					sha256: sha256(bytes),
					...((metadata.mode & 0o100) === 0 ? {} : { executable: true }),
				});
			} else throw new Error(`${relative} is neither a regular file with one link nor a link`);
		}
	}
	const root = path.join(directory, application);
	const metadata = lstatSync(root);
	if (!application.endsWith('.app') || !metadata.isDirectory() || metadata.isSymbolicLink())
		throw new Error(`${application} is not an application bundle`);
	visit(root, application);
	return { files: files.toSorted(byPath), links: links.toSorted(byPath) };
}

function byPath(left, right) {
	return left.path < right.path ? -1 : Number(left.path > right.path);
}

/**
 * The exact bytes of the list of the window directory `directory` at `version`; on macOS, of the
 * bundle `application` inside it.
 */
export function desktopFilesDocument(directory, version, application = undefined) {
	if (!validVersion(version))
		throw new Error('The window version must be SemVer of at most 64 bytes');
	let document;
	if (application === undefined) {
		const files = releaseFiles(directory, { exclude: [DESKTOP_FILES] }).map((file) => ({
			path: file.path,
			size: file.size,
			sha256: file.sha256,
		}));
		document = { schemaVersion: 1, version, files };
	} else {
		const { files, links } = bundleEntries(directory, application);
		document = { schemaVersion: 1, version, files, ...(links.length > 0 ? { links } : {}) };
	}
	const bytes = Buffer.from(JSON.stringify(document) + '\n', 'utf8');
	const { problems } = readDesktopFiles(bytes, { macos: application !== undefined });
	if (problems.length > 0) throw new Error(problems.join('\n'));
	return bytes;
}

/**
 * Where the link at `link` points, as a path of the list, or undefined when its relative `target`
 * leaves the bundle, the first part of `link`.
 */
function linkDestination(link, target) {
	if (typeof target !== 'string' || target.length === 0 || target.length > 240) return undefined;
	const parts = link.split('/').slice(0, -1);
	for (const part of target.split('/')) {
		if (part === '..') {
			if (parts.length <= 1) return undefined;
			parts.pop();
		} else if (validBundlePath(part)) parts.push(part);
		else return undefined;
	}
	return parts.length > 1 ? parts.join('/') : undefined;
}

/**
 * The list of `bytes` and every reader rule it breaks; none means a reader admits it. `macos`
 * admits the bundle's paths, the executable mark and the links.
 */
export function readDesktopFiles(bytes, { macos = false } = {}) {
	if (bytes.length > listLimit) return { problems: [`${DESKTOP_FILES} exceeds 1 MiB`] };
	let list;
	try {
		list = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
	} catch {
		return { problems: [`${DESKTOP_FILES} is not UTF-8 JSON`] };
	}
	if (
		!closed(
			list,
			['schemaVersion', 'version', 'files'],
			macos ? ['extensions', 'links'] : ['extensions'],
		)
	)
		return {
			problems: [`${DESKTOP_FILES} must have exactly the members schemaVersion, version, files`],
		};
	const problems = [];
	if (list.extensions !== undefined && !isObject(list.extensions))
		problems.push('extensions is not an object');
	if (list.schemaVersion !== 1) problems.push('schemaVersion is not 1');
	if (!validVersion(list.version))
		problems.push('version is not a SemVer version of at most 64 bytes');
	const links = list.links ?? [];
	if (
		!Array.isArray(list.files) ||
		!Array.isArray(links) ||
		list.files.length + links.length > fileCountLimit
	) {
		problems.push(`files and links are not lists of at most ${fileCountLimit} entries in all`);
		return { list, problems };
	}
	const validPath = macos ? validBundlePath : validReleaseFilePath;
	// On macOS every entry lies inside the one bundle the first entry names.
	const bundle = macos ? String(list.files[0]?.path ?? '').split('/')[0] : undefined;
	const inside = (value) => !macos || (bundle.endsWith('.app') && value.startsWith(`${bundle}/`));
	const seen = new Set();
	let total = 0;
	for (const file of list.files) {
		if (!closed(file, ['path', 'size', 'sha256'], macos ? ['executable'] : [])) {
			problems.push('a file must have exactly path, size and sha256');
			continue;
		}
		const name = String(file.path);
		if (!validPath(file.path) || file.path === DESKTOP_FILES || !inside(file.path))
			problems.push(`${name} is not a release path`);
		else if (seen.has(file.path.toLowerCase())) problems.push(`${name} is listed twice`);
		else seen.add(file.path.toLowerCase());
		if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > fileLimit)
			problems.push(`${name}: size is not an integer from 0 to 512 MiB`);
		else total += file.size;
		if (typeof file.sha256 !== 'string' || !sha256Hex.test(file.sha256))
			problems.push(`${name}: sha256 is not lowercase SHA-256`);
		if (file.executable !== undefined && file.executable !== true)
			problems.push(`${name}: executable is not true`);
	}
	if (total > totalLimit) problems.push('the files exceed 2 GiB in all');
	const targets = new Map();
	for (const link of links) {
		if (!closed(link, ['path', 'target'])) {
			problems.push('a link must have exactly path and target');
			continue;
		}
		const name = String(link.path);
		if (!validPath(link.path) || !inside(link.path)) problems.push(`${name} is not a release path`);
		else if (seen.has(link.path.toLowerCase())) problems.push(`${name} is listed twice`);
		else {
			seen.add(link.path.toLowerCase());
			targets.set(link.path, link.target);
		}
	}
	for (const [link, target] of targets)
		if (!reachesListed(linkDestination(link, target), list.files, targets))
			problems.push(`${link}: target does not name a listed entry inside the bundle`);
	return { list, problems };
}

/**
 * Whether `destination` names a listed file or a folder holding one, after following the listed
 * links on its way, at most 40 of them, as the system resolves a path.
 */
function reachesListed(destination, files, targets) {
	let current = destination;
	for (let hops = 0; hops < 40 && current !== undefined; hops += 1) {
		const here = current;
		if (files.some((file) => file.path === here || String(file.path).startsWith(`${here}/`)))
			return true;
		const parts = here.split('/');
		const at = parts.findLastIndex((_, index) => targets.has(parts.slice(0, index + 1).join('/')));
		if (at < 1) return false;
		const link = parts.slice(0, at + 1).join('/');
		const next = linkDestination(link, targets.get(link));
		current = next === undefined ? undefined : [next, ...parts.slice(at + 1)].join('/');
	}
	return false;
}

/** Writes the list into `directory`, never replacing a file. */
function writeDesktopFiles(directory, version, application) {
	if (!path.isAbsolute(directory))
		throw new Error('Expected an absolute path to a packaged window directory');
	const destination = path.join(directory, DESKTOP_FILES);
	const bytes = desktopFilesDocument(directory, version, application);
	const fd = openSync(destination, 'wx');
	try {
		writeFileSync(fd, bytes);
		fsyncSync(fd);
	} catch (error) {
		closeSync(fd);
		unlinkSync(destination);
		throw error;
	}
	closeSync(fd);
	return destination;
}

const endRecord = 0x06054b50;
const zip64EndLocator = 0x07064b50;
const zip64EndRecord = 0x06064b50;
const centralRecord = 0x02014b50;
const localRecord = 0x04034b50;
const linkMode = 0o120000;
const typeMask = 0o170000;

/** The central directory entries of the zip `bytes`, zip64 sizes and offsets included. */
function zipEntries(bytes) {
	let end = -1;
	for (let at = bytes.length - 22; at >= Math.max(0, bytes.length - 22 - 0xffff); at -= 1)
		if (bytes.readUInt32LE(at) === endRecord) {
			end = at;
			break;
		}
	if (end === -1) throw new Error('has no end of central directory');
	let count = bytes.readUInt16LE(end + 10);
	let offset = bytes.readUInt32LE(end + 16);
	if (count === 0xffff || offset === 0xffffffff) {
		if (end < 20 || bytes.readUInt32LE(end - 20) !== zip64EndLocator)
			throw new Error('has no zip64 end of central directory locator');
		const record = Number(bytes.readBigUInt64LE(end - 12));
		if (bytes.readUInt32LE(record) !== zip64EndRecord)
			throw new Error('has no zip64 end of central directory');
		count = Number(bytes.readBigUInt64LE(record + 32));
		offset = Number(bytes.readBigUInt64LE(record + 48));
	}
	const entries = [];
	let at = offset;
	for (let index = 0; index < count; index += 1) {
		if (bytes.readUInt32LE(at) !== centralRecord)
			throw new Error('has a malformed central directory');
		const nameLength = bytes.readUInt16LE(at + 28);
		const extraLength = bytes.readUInt16LE(at + 30);
		const entry = {
			flags: bytes.readUInt16LE(at + 8),
			method: bytes.readUInt16LE(at + 10),
			compressed: bytes.readUInt32LE(at + 20),
			size: bytes.readUInt32LE(at + 24),
			mode: bytes.readUInt32LE(at + 38) >>> 16,
			local: bytes.readUInt32LE(at + 42),
			name: bytes.toString('utf8', at + 46, at + 46 + nameLength),
		};
		const extraEnd = at + 46 + nameLength + extraLength;
		for (let extra = at + 46 + nameLength; extra + 4 <= extraEnd;) {
			const length = bytes.readUInt16LE(extra + 2);
			if (bytes.readUInt16LE(extra) === 1) {
				let field = extra + 4;
				for (const member of ['size', 'compressed', 'local'])
					if (entry[member] === 0xffffffff) {
						entry[member] = Number(bytes.readBigUInt64LE(field));
						field += 8;
					}
			}
			extra += 4 + length;
		}
		entries.push(entry);
		at = extraEnd + bytes.readUInt16LE(at + 32);
	}
	return entries;
}

/** The unpacked bytes of one zip entry, stored or deflated. */
function entryBytes(bytes, entry) {
	if (bytes.readUInt32LE(entry.local) !== localRecord) throw new Error('has a malformed entry');
	if ((entry.flags & 1) !== 0) throw new Error(`holds the encrypted ${entry.name}`);
	const start =
		entry.local + 30 + bytes.readUInt16LE(entry.local + 26) + bytes.readUInt16LE(entry.local + 28);
	const raw = bytes.subarray(start, start + entry.compressed);
	if (raw.length !== entry.compressed) throw new Error(`ends inside ${entry.name}`);
	let data;
	if (entry.method === 0) data = raw;
	else if (entry.method === 8)
		data = inflateRawSync(raw, { maxOutputLength: Math.max(1, entry.size) });
	else throw new Error(`stores ${entry.name} with the unsupported method ${entry.method}`);
	if (data.length !== entry.size) throw new Error(`holds ${entry.name} at another size`);
	return data;
}

/**
 * Checks the window archive `bytes` against the shape a release writes: one top-level folder
 * `folder`, and a list at its root that admits and names exactly the archive's other files, each by
 * size and SHA-256. A Windows archive holds no links; a macOS one (`macos`) holds exactly the links
 * its list names, with their targets, and each file's owner execute bit as the list marks it.
 * Returns the list's bytes, the list, and every problem found.
 */
export function checkDesktopArchive(bytes, folder, { macos = false } = {}) {
	let entries;
	try {
		entries = zipEntries(bytes);
	} catch (error) {
		return { problems: [error instanceof RangeError ? 'is not a zip archive' : error.message] };
	}
	const problems = [];
	const files = new Map();
	const links = new Map();
	for (const entry of entries) {
		if (entry.name !== `${folder}/` && !entry.name.startsWith(`${folder}/`)) {
			problems.push(`holds ${entry.name} outside the one top-level folder ${folder}`);
			continue;
		}
		const name = entry.name.slice(folder.length + 1);
		if ((entry.mode & typeMask) === linkMode) {
			if (macos) links.set(name, entry);
			else problems.push(`holds the link ${entry.name}`);
		} else if (!entry.name.endsWith('/')) files.set(name, entry);
	}
	const listEntry = files.get(DESKTOP_FILES);
	if (listEntry === undefined) return { problems: [...problems, `holds no ${DESKTOP_FILES}`] };
	files.delete(DESKTOP_FILES);
	try {
		const listBytes = entryBytes(bytes, listEntry);
		const { list, problems: listProblems } = readDesktopFiles(listBytes, { macos });
		problems.push(...listProblems.map((problem) => `${DESKTOP_FILES}: ${problem}`));
		if (listProblems.length > 0) return { problems };
		for (const file of list.files) {
			const entry = files.get(file.path);
			files.delete(file.path);
			if (entry === undefined) {
				problems.push(`lacks the listed ${file.path}`);
				continue;
			}
			const data = entryBytes(bytes, entry);
			if (data.length !== file.size || sha256(data) !== file.sha256)
				problems.push(`holds ${file.path} with other bytes than ${DESKTOP_FILES} lists`);
			if (macos && ((entry.mode & 0o100) !== 0) !== (file.executable === true))
				problems.push(`holds ${file.path} with another execute bit than ${DESKTOP_FILES} marks`);
		}
		for (const link of list.links ?? []) {
			const entry = links.get(link.path);
			links.delete(link.path);
			if (entry === undefined) problems.push(`lacks the listed link ${link.path}`);
			else if (entryBytes(bytes, entry).toString('utf8') !== link.target)
				problems.push(
					`holds the link ${link.path} with another target than ${DESKTOP_FILES} lists`,
				);
		}
		for (const name of links.keys())
			problems.push(`holds the link ${name}, which ${DESKTOP_FILES} omits`);
		for (const name of files.keys()) problems.push(`holds ${name}, which ${DESKTOP_FILES} omits`);
		return { listBytes, list, problems };
	} catch (error) {
		return {
			problems: [...problems, error instanceof RangeError ? 'is not a zip archive' : error.message],
		};
	}
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const [command, directory, version, application, ...rest] = process.argv.slice(2);
	if (command !== 'write' || directory === undefined || version === undefined || rest.length > 0) {
		process.stderr.write(
			'Usage: node scripts/desktop-files.mjs write <absolute packaged window directory> <version> [<name>.app]\n',
		);
		process.exitCode = 2;
	} else {
		try {
			process.stdout.write(writeDesktopFiles(directory, version, application) + '\n');
		} catch (error) {
			process.stderr.write(`${error.message}\n`);
			process.exitCode = 1;
		}
	}
}
