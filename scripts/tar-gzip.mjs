// Writes and reads the gzip-compressed POSIX ustar archives the macOS and Linux release rows publish
// (proposals/platform/release-distribution.md#download-names). Node built-ins only, so the archive
// is the same on every runner whatever its tar: each entry records the mode it is given, owner and
// group 0 without names, and no extended attributes, so no runner account or macOS metadata enters
// it. Every system tar extracts it.
//
// The reader admits regular files and directories, and pax headers for long paths; any other entry,
// such as a link, is an error, so a reader of the result never meets one.
import { constants } from 'node:buffer';
import { gunzipSync, gzipSync } from 'node:zlib';

const block = 512;
// The size field holds 11 octal digits.
const largestEntry = 8 ** 11 - 1;

function octal(record, offset, length, value) {
	const digits = value.toString(8).padStart(length - 1, '0');
	if (digits.length > length - 1) throw new Error(`${value} does not fit a tar header field`);
	record.write(`${digits}\0`, offset, length, 'ascii');
}

/** The name and prefix fields of `name`, split at a slash when it exceeds the name field. */
function splitName(name) {
	const bytes = Buffer.byteLength(name);
	if (bytes <= 100) return { name, prefix: '' };
	for (let index = name.indexOf('/'); index !== -1; index = name.indexOf('/', index + 1)) {
		const prefix = name.slice(0, index);
		const rest = name.slice(index + 1);
		if (Buffer.byteLength(prefix) <= 155 && Buffer.byteLength(rest) <= 100 && rest.length > 0)
			return { name: rest, prefix };
	}
	throw new Error(`${name} is too long for a ustar entry`);
}

function header({ name, mode, mtime, size, type }) {
	const fields = splitName(name);
	const bytes = Buffer.alloc(block);
	bytes.write(fields.name, 0, 100, 'utf8');
	octal(bytes, 100, 8, mode);
	octal(bytes, 108, 8, 0);
	octal(bytes, 116, 8, 0);
	octal(bytes, 124, 12, size);
	octal(bytes, 136, 12, mtime);
	bytes.fill(0x20, 148, 156);
	bytes.write(type, 156, 1, 'ascii');
	bytes.write('ustar\0', 257, 6, 'ascii');
	bytes.write('00', 263, 2, 'ascii');
	bytes.write(fields.prefix, 345, 155, 'utf8');
	let sum = 0;
	for (const byte of bytes) sum += byte;
	bytes.write(`${sum.toString(8).padStart(6, '0')}\0 `, 148, 8, 'ascii');
	return bytes;
}

/**
 * The tar.gz bytes of `entries` in the order given: `{ path, mode, mtime, bytes }` for a file and
 * `{ path, mode, mtime, directory: true }` for a directory, `path` relative with `/` separators and
 * `mtime` in whole seconds.
 */
export function tarGzip(entries) {
	const parts = [];
	for (const entry of entries) {
		const directory = entry.directory === true;
		const data = directory ? Buffer.alloc(0) : entry.bytes;
		if (data.length > largestEntry) throw new Error(`${entry.path} is too large for a ustar entry`);
		parts.push(
			header({
				name: directory ? `${entry.path.replace(/\/$/u, '')}/` : entry.path,
				mode: entry.mode,
				mtime: entry.mtime,
				size: data.length,
				type: directory ? '5' : '0',
			}),
			data,
			Buffer.alloc((block - (data.length % block)) % block),
		);
	}
	parts.push(Buffer.alloc(2 * block));
	return gzipSync(Buffer.concat(parts), { level: 9 });
}

function text(bytes, offset, length) {
	const field = bytes.subarray(offset, offset + length);
	const end = field.indexOf(0);
	return field.subarray(0, end === -1 ? field.length : end).toString('utf8');
}

function readOctal(bytes, offset, length) {
	const field = text(bytes, offset, length).trim();
	if (!/^[0-7]+$/u.test(field)) throw new Error('The archive has a malformed tar header');
	return Number.parseInt(field, 8);
}

/** The `path` record of a pax extended header, if it has one. */
function paxPath(data) {
	let offset = 0;
	let found;
	while (offset < data.length) {
		const space = data.indexOf(0x20, offset);
		const length = Number.parseInt(data.subarray(offset, space).toString('ascii'), 10);
		if (space === -1 || !Number.isSafeInteger(length) || length <= 0)
			throw new Error('The archive has a malformed pax header');
		const record = data.subarray(space + 1, offset + length - 1).toString('utf8');
		const equals = record.indexOf('=');
		if (record.slice(0, equals) === 'path') found = record.slice(equals + 1);
		offset += length;
	}
	return found;
}

/**
 * The entries of the tar.gz `bytes` as `{ path, type: 'file' | 'directory', mode, mtime, bytes }`,
 * a directory's path without its trailing slash and `mtime` in whole seconds. Throws for a malformed archive or an entry of any
 * other kind.
 */
export function readTarGzip(bytes) {
	let tar;
	try {
		tar = gunzipSync(bytes, { maxOutputLength: Math.min(constants.MAX_LENGTH, 4 * 1024 ** 3) });
	} catch {
		throw new Error('The archive is not gzip data');
	}
	const entries = [];
	let longPath;
	let offset = 0;
	while (offset + block <= tar.length) {
		const record = tar.subarray(offset, offset + block);
		if (record.every((byte) => byte === 0)) return entries;
		let sum = 0;
		for (let index = 0; index < block; index++)
			sum += index >= 148 && index < 156 ? 0x20 : record[index];
		if (readOctal(record, 148, 8) !== sum) throw new Error('The archive has a damaged tar header');
		const size = readOctal(record, 124, 12);
		const type = String.fromCharCode(record[156] || 0x30);
		const start = offset + block;
		const data = tar.subarray(start, start + size);
		if (data.length !== size) throw new Error('The archive ends inside an entry');
		offset = start + Math.ceil(size / block) * block;
		if (type === 'x') {
			longPath = paxPath(data);
			continue;
		}
		if (type === 'g') continue;
		const prefix = text(record, 345, 155);
		const name = longPath ?? (prefix === '' ? '' : `${prefix}/`) + text(record, 0, 100);
		longPath = undefined;
		const mode = readOctal(record, 100, 8) & 0o7777;
		const mtime = readOctal(record, 136, 12);
		if (type === '0') entries.push({ path: name, type: 'file', mode, mtime, bytes: data });
		else if (type === '5')
			entries.push({ path: name.replace(/\/$/u, ''), type: 'directory', mode, mtime, bytes: data });
		else throw new Error(`The archive holds ${name}, which is neither a file nor a directory`);
	}
	throw new Error('The archive has no end-of-archive marker');
}
