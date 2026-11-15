// The window file list and the window archive that carries it; see
// proposals/platform/automatic-updates.md#window-archive-and-file-list.
//
// Every packaged window directory holds `desktop-files.json` at its root:
// `{"schemaVersion": 1, "version", "files": [{"path", "size", "sha256"}]}`, plus an optional
// `extensions` object that readers check is an object and otherwise ignore. It names every other
// regular file of the directory once, by a `/`-separated path, under the release manifest's file
// rules (scripts/package-native-windows.mjs), and is itself at most 1 MiB. The update feed binds it
// by its SHA-256 in `extensions.desktop.manifest` (scripts/release-feed.mjs), so the feed signature
// covers every file of the window. The release zips the directory, list included, as
// `<stem>-desktop-<version>-<platform>.zip` under one top-level folder of that name.
//
//   node scripts/desktop-files.mjs write <absolute packaged window directory> <version>
//
// apps/desktop/scripts/package.mjs runs `write` after the packager returns and before Setup is
// compiled, so Setup and the archive carry the same list. The written form is `JSON.stringify` of
// the members in this order, files in path order, then one LF. Node built-ins only.
import { closeSync, fsyncSync, openSync, unlinkSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { inflateRawSync } from 'node:zlib';

import { releaseFiles, validReleaseFilePath } from './package-native-windows.mjs';
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

/** The exact bytes of the list of the window directory `directory` at `version`. */
export function desktopFilesDocument(directory, version) {
	if (!validVersion(version))
		throw new Error('The window version must be SemVer of at most 64 bytes');
	const files = releaseFiles(directory, { exclude: [DESKTOP_FILES] }).map((file) => ({
		path: file.path,
		size: file.size,
		sha256: file.sha256,
	}));
	const bytes = Buffer.from(JSON.stringify({ schemaVersion: 1, version, files }) + '\n', 'utf8');
	const { problems } = readDesktopFiles(bytes);
	if (problems.length > 0) throw new Error(problems.join('\n'));
	return bytes;
}

/** The list of `bytes` and every reader rule it breaks; none means a reader admits it. */
export function readDesktopFiles(bytes) {
	if (bytes.length > listLimit) return { problems: [`${DESKTOP_FILES} exceeds 1 MiB`] };
	let list;
	try {
		list = JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
	} catch {
		return { problems: [`${DESKTOP_FILES} is not UTF-8 JSON`] };
	}
	if (!closed(list, ['schemaVersion', 'version', 'files'], ['extensions']))
		return {
			problems: [`${DESKTOP_FILES} must have exactly the members schemaVersion, version, files`],
		};
	const problems = [];
	if (list.extensions !== undefined && !isObject(list.extensions))
		problems.push('extensions is not an object');
	if (list.schemaVersion !== 1) problems.push('schemaVersion is not 1');
	if (!validVersion(list.version))
		problems.push('version is not a SemVer version of at most 64 bytes');
	if (!Array.isArray(list.files) || list.files.length > fileCountLimit) {
		problems.push(`files is not a list of at most ${fileCountLimit} files`);
		return { list, problems };
	}
	const seen = new Set();
	let total = 0;
	for (const file of list.files) {
		if (!closed(file, ['path', 'size', 'sha256'])) {
			problems.push('a file must have exactly path, size and sha256');
			continue;
		}
		const name = String(file.path);
		if (!validReleaseFilePath(file.path) || file.path === DESKTOP_FILES)
			problems.push(`${name} is not a release path`);
		else if (seen.has(file.path.toLowerCase())) problems.push(`${name} is listed twice`);
		else seen.add(file.path.toLowerCase());
		if (!Number.isSafeInteger(file.size) || file.size < 0 || file.size > fileLimit)
			problems.push(`${name}: size is not an integer from 0 to 512 MiB`);
		else total += file.size;
		if (typeof file.sha256 !== 'string' || !sha256Hex.test(file.sha256))
			problems.push(`${name}: sha256 is not lowercase SHA-256`);
	}
	if (total > totalLimit) problems.push('the files exceed 2 GiB in all');
	return { list, problems };
}

/** Writes the list into `directory`, never replacing a file. */
function writeDesktopFiles(directory, version) {
	if (!path.isAbsolute(directory))
		throw new Error('Expected an absolute path to a packaged window directory');
	const destination = path.join(directory, DESKTOP_FILES);
	const bytes = desktopFilesDocument(directory, version);
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
 * `folder`, no links, and a list at its root that admits and names exactly the archive's other
 * files, each by size and SHA-256. Returns the list's bytes, the list, and every problem found.
 */
export function checkDesktopArchive(bytes, folder) {
	let entries;
	try {
		entries = zipEntries(bytes);
	} catch (error) {
		return { problems: [error instanceof RangeError ? 'is not a zip archive' : error.message] };
	}
	const problems = [];
	const files = new Map();
	for (const entry of entries) {
		if (entry.name !== `${folder}/` && !entry.name.startsWith(`${folder}/`)) {
			problems.push(`holds ${entry.name} outside the one top-level folder ${folder}`);
			continue;
		}
		if ((entry.mode & typeMask) === linkMode) problems.push(`holds the link ${entry.name}`);
		else if (!entry.name.endsWith('/')) files.set(entry.name.slice(folder.length + 1), entry);
	}
	const listEntry = files.get(DESKTOP_FILES);
	if (listEntry === undefined) return { problems: [...problems, `holds no ${DESKTOP_FILES}`] };
	files.delete(DESKTOP_FILES);
	try {
		const listBytes = entryBytes(bytes, listEntry);
		const { list, problems: listProblems } = readDesktopFiles(listBytes);
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
		}
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
	const [command, directory, version, ...rest] = process.argv.slice(2);
	if (command !== 'write' || directory === undefined || version === undefined || rest.length > 0) {
		process.stderr.write(
			'Usage: node scripts/desktop-files.mjs write <absolute packaged window directory> <version>\n',
		);
		process.exitCode = 2;
	} else {
		try {
			process.stdout.write(writeDesktopFiles(directory, version) + '\n');
		} catch (error) {
			process.stderr.write(`${error.message}\n`);
			process.exitCode = 1;
		}
	}
}
