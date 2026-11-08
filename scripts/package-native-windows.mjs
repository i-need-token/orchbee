// Produce the updater's explicit release feed from a completed staged Windows cohort.
// No upload, application launch, installation or PATH mutation occurs here.
import { createHash } from 'node:crypto';
import {
	closeSync,
	fsyncSync,
	lstatSync,
	openSync,
	readFileSync,
	readdirSync,
	readSync,
	unlinkSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { verifiedCapsuleDigest } from './stage-local-runtime.mjs';
import { verifyConptyBundle } from '../apps/terminal-host/scripts/windows-conpty.mjs';

const peers = ['orchbee', 'orchbee-server', 'orchbee-worker', 'orchbee-terminal-host'];
const maximumFile = 512 * 1024 * 1024;
const maximumTotal = 2 * 1024 * 1024 * 1024;
function digest(filename) {
	const fd = openSync(filename, 'r');
	try {
		const hash = createHash('sha256');
		const buffer = Buffer.alloc(64 * 1024);
		let size = 0;
		while (true) {
			const count = readSync(fd, buffer);
			if (count === 0) break;
			size += count;
			if (size > maximumFile) throw new Error('Release file exceeds 512 MiB');
			hash.update(buffer.subarray(0, count));
		}
		return { sha256: hash.digest('hex'), size };
	} finally {
		closeSync(fd);
	}
}
function validPath(value) {
	return (
		value.length <= 240 &&
		value.split('/').length <= 12 &&
		value
			.split('/')
			.every(
				(part) =>
					part.length > 0 &&
					part.length <= 120 &&
					/^[A-Za-z0-9._-]+$/u.test(part) &&
					!part.endsWith('.') &&
					!/^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])(?:\.|$)/iu.test(part),
			)
	);
}

export function packageWindows(cohort, channel, version, buildId) {
	if (
		!path.isAbsolute(cohort) ||
		!['stable', 'preview'].includes(channel) ||
		![version, buildId].every(
			(value) => typeof value === 'string' && /^[A-Za-z0-9._+-]{1,96}$/u.test(value),
		)
	)
		throw new Error('Expected absolute cohort, stable|preview channel, version and build identity');
	const destination = path.join(cohort, 'release.json');
	// Never overwrite a previously published manifest or follow its symlink.
	try {
		lstatSync(destination);
		throw new Error('release.json already exists');
	} catch (error) {
		if (error.code !== 'ENOENT') throw error;
	}
	const files = [];
	const seen = new Set();
	let total = 0;
	let entryCount = 0;
	function visit(directory, prefix = '') {
		const metadata = lstatSync(directory);
		if (!metadata.isDirectory() || metadata.isSymbolicLink())
			throw new Error('Unsafe release directory');
		for (const entry of readdirSync(directory, { withFileTypes: true })) {
			const relative = prefix + entry.name;
			if (++entryCount > 8192 || !validPath(relative))
				throw new Error('Invalid or oversized release tree');
			const filename = path.join(directory, entry.name);
			const stat = lstatSync(filename);
			if (stat.isSymbolicLink()) throw new Error('Release links are not supported');
			const folded = relative.toLowerCase();
			if (seen.has(folded)) throw new Error('Case alias in release');
			seen.add(folded);
			if (stat.isDirectory()) {
				visit(filename, relative + '/');
				continue;
			}
			if (!stat.isFile() || stat.nlink !== 1) throw new Error('Unsafe release file');
			const content = digest(filename);
			total += content.size;
			if (files.length >= 4096 || total > maximumTotal)
				throw new Error('Release size bound exceeded');
			files.push({ path: relative, url: relative, ...content });
		}
	}
	visit(cohort);
	files.sort((left, right) => (left.path < right.path ? -1 : Number(left.path > right.path)));
	const manifest = JSON.parse(readFileSync(path.join(cohort, 'cohort.json'), 'utf8'));
	if (
		manifest.schemaVersion !== 1 ||
		Object.keys(manifest).length !== 2 ||
		Object.keys(manifest.artifacts ?? {}).length !== 5
	)
		throw new Error('Invalid native cohort manifest');
	for (const peer of peers) {
		const file = files.find((entry) => entry.path === peer + '.exe');
		if (!file)
			throw new Error(
				`Missing ${peer}.exe: a release feed packages a cohort staged with --artifact-stem orchbee`,
			);
		if (file.sha256 !== manifest.artifacts[peer]) throw new Error('Cohort binary digest mismatch');
	}
	const capsule = verifiedCapsuleDigest(path.join(cohort, 'agent-observation-adapters'));
	if (capsule === null || capsule !== manifest.artifacts['agent-observation-adapters'])
		throw new Error('Cohort adapter capsule digest mismatch');
	verifyConptyBundle(path.join(cohort, 'conpty'));
	const bytes =
		JSON.stringify({
			schemaVersion: 1,
			product: 'orchbee',
			platform: 'windows-x86_64',
			channel,
			version,
			buildId,
			files,
		}) + '\n';
	if (Buffer.byteLength(bytes) > 64 * 1024) throw new Error('Release manifest exceeds 64 KiB');
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

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	const [cohort, channel, version, buildId, extra] = process.argv.slice(2);
	if (!cohort || !channel || !version || !buildId || extra !== undefined)
		throw new Error(
			'Usage: node scripts/package-native-windows.mjs <absolute-cohort> <stable|preview> <version> <build-id>',
		);
	process.stdout.write(packageWindows(cohort, channel, version, buildId) + '\n');
}
