// Stages the rg program that a development or test cohort carries beside the File Bar's process,
// `orchbee-files`. The program for one release platform comes from the npm archive that
// `apps/orchbee-files/ripgrep.json` pins, checked against its archive, program and notices digests,
// so a cohort carries only the reviewed bytes; the notices of everything compiled into rg travel
// with it. A release cohort carries neither until the File Bar ships.
import { createHash } from 'node:crypto';
import { existsSync, lstatSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { gunzipSync } from 'node:zlib';

const owner = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'..',
	'apps',
	'orchbee-files',
);
const manifest = JSON.parse(readFileSync(path.join(owner, 'ripgrep.json'), 'utf8'));
const maximumArchiveBytes = 64 * 1024 * 1024;
const maximumUnpackedBytes = 128 * 1024 * 1024;

/** The path of rg's notices inside a cohort, and the digest that file has. */
export const RIPGREP_NOTICES = Object.freeze({
	path: `licenses/${manifest.notices.name}`,
	sha256: manifest.notices.sha256,
});

/** The file name rg has in a cohort of `platform`, a release platform token. */
export function ripgrepFileName(platform) {
	return platform.startsWith('windows-') ? 'rg.exe' : 'rg';
}

/** The digest `ripgrep.json` pins for the rg program of `platform`. */
export function ripgrepDigest(platform) {
	const target = manifest.targets[platform];
	if (target === undefined) throw new Error(`ripgrep.json pins no rg for ${platform}`);
	return target.sha256;
}

const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');

/** The bytes of the regular file `name` in the POSIX tar archive `tar`. */
function tarEntry(tar, name) {
	let offset = 0;
	while (offset + 512 <= tar.length) {
		const header = tar.subarray(offset, offset + 512);
		if (header.every((byte) => byte === 0)) break;
		const field = (start, length) => {
			const bytes = header.subarray(start, start + length);
			const end = bytes.indexOf(0);
			return bytes.subarray(0, end === -1 ? length : end).toString('latin1');
		};
		const size = Number.parseInt(field(124, 12).trim(), 8);
		if (!Number.isSafeInteger(size) || size < 0) throw new Error('The rg archive is malformed');
		const prefix = field(257, 6) === 'ustar' ? field(345, 155) : '';
		const entry = prefix === '' ? field(0, 100) : `${prefix}/${field(0, 100)}`;
		const type = field(156, 1);
		const start = offset + 512;
		if (start + size > tar.length) throw new Error('The rg archive is truncated');
		if (entry === name && (type === '0' || type === '')) return tar.subarray(start, start + size);
		offset = start + Math.ceil(size / 512) * 512;
	}
	throw new Error(`The rg archive holds no ${name}`);
}

/**
 * Writes the rg program of `platform` and its notices into the existing cohort directory `output`,
 * as `rg[.exe]` and `licenses/<notices>`, after checking every pinned digest.
 */
export async function stageRipgrep(platform, output) {
	const target = manifest.targets[platform];
	if (target === undefined) throw new Error(`ripgrep.json pins no rg for ${platform}`);
	if (!path.isAbsolute(output) || !lstatSync(output).isDirectory())
		throw new Error('The rg output must be an existing absolute directory');
	const program = path.join(output, ripgrepFileName(platform));
	const notices = path.join(output, ...RIPGREP_NOTICES.path.split('/'));
	if (existsSync(program) || existsSync(notices))
		throw new Error('Refusing to replace a staged rg');
	const text = readFileSync(path.join(owner, 'licenses', manifest.notices.name));
	if (sha256(text) !== RIPGREP_NOTICES.sha256) throw new Error('The rg notices digest differs');
	const response = await fetch(manifest.url, { signal: AbortSignal.timeout(120_000) });
	if (!response.ok || response.body === null) throw new Error('The rg archive download failed');
	const chunks = [];
	let size = 0;
	for await (const chunk of response.body) {
		size += chunk.byteLength;
		if (size > maximumArchiveBytes) throw new Error('The rg archive exceeds its bound');
		chunks.push(chunk);
	}
	const archive = Buffer.concat(chunks, size);
	if (sha256(archive) !== manifest.sha256) throw new Error('The rg archive digest differs');
	const binary = tarEntry(
		gunzipSync(archive, { maxOutputLength: maximumUnpackedBytes }),
		target.source,
	);
	if (sha256(binary) !== target.sha256) throw new Error(`The rg digest differs for ${platform}`);
	writeFileSync(program, binary, { flag: 'wx', mode: 0o700 });
	mkdirSync(path.dirname(notices), { recursive: true });
	writeFileSync(notices, text, { flag: 'wx', mode: 0o600 });
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const [platformFlag, platform, outputFlag, output] = process.argv.slice(2);
	if (process.argv.length !== 6 || platformFlag !== '--platform' || outputFlag !== '--output')
		throw new Error('Usage: ripgrep.mjs --platform PLATFORM --output EXISTING_ABSOLUTE_DIRECTORY');
	await stageRipgrep(platform, output);
}
