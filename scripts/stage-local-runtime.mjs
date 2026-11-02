import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import {
	constants,
	copyFileSync,
	cpSync,
	existsSync,
	lstatSync,
	mkdtempSync,
	readFileSync,
	readdirSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stageWindows } from './stage-native-windows.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const artifacts = ['orchbee', 'orchbee-server', 'orchbee-worker', 'orchbee-terminal-host'];
const CAPSULE_DIRECTORY = 'agent-observation-adapters';
const CAPSULE_CONTRACT_VERSION = 1;
const MAX_CAPSULE_ENTRIES = 128;
const MAX_CAPSULE_DEPTH = 8;
const MAX_CAPSULE_FILE_BYTES = 512 * 1024;
const MAX_CAPSULE_TOTAL_BYTES = 2 * 1024 * 1024;
const MAX_MANIFEST_BYTES = 1024;
const SHA256_PATTERN = /^[0-9a-f]{64}$/u;
const REQUIRED_PAYLOAD_FILES = [
	'bundle/package.json',
	'bundle/scripts/forward-hook.mjs',
	'bundle/scripts/hook-sanitizers.mjs',
	'bundle/scripts/observation-transport.mjs',
];

// Mirrors the capsule verifier in crates/agent-integration-writer/src/capsule.rs — same bounds,
// same entry shape, same digest. The strict manifest contains only contractVersion and payloadSha256.
function readCapsuleManifest(filePath) {
	try {
		const metadata = lstatSync(filePath);
		if (!metadata.isFile() || metadata.size > MAX_MANIFEST_BYTES) return null;
		const bytes = readFileSync(filePath);
		if (bytes.byteLength > MAX_MANIFEST_BYTES) return null;
		const value = JSON.parse(bytes.toString('utf8'));
		return typeof value === 'object' && value !== null && !Array.isArray(value) ? value : null;
	} catch {
		return null;
	}
}
function capsulePayloadDigest(rootPath) {
	try {
		if (!lstatSync(rootPath).isDirectory()) return null;
	} catch {
		return null;
	}
	let entryCount = 0;
	let totalBytes = 0;
	const payloadEntries = [];
	const pending = [{ depth: 0, directory: rootPath }];
	while (pending.length > 0) {
		const current = pending.pop();
		if (current.depth > MAX_CAPSULE_DEPTH) return null;
		let entries;
		try {
			entries = readdirSync(current.directory, { withFileTypes: true });
		} catch {
			return null;
		}
		for (const entry of entries) {
			entryCount += 1;
			if (entryCount > MAX_CAPSULE_ENTRIES || entry.isSymbolicLink()) return null;
			const entryPath = path.join(current.directory, entry.name);
			const relativePath = path.relative(rootPath, entryPath).split(path.sep).join('/');
			if (entry.isDirectory()) {
				payloadEntries.push({ kind: 'directory', path: relativePath });
				pending.push({ depth: current.depth + 1, directory: entryPath });
				continue;
			}
			if (!entry.isFile()) return null;
			if (relativePath === 'capsule.json') continue;
			let bytes;
			try {
				const metadata = lstatSync(entryPath);
				if (!metadata.isFile() || metadata.size > MAX_CAPSULE_FILE_BYTES) return null;
				bytes = readFileSync(entryPath);
			} catch {
				return null;
			}
			if (bytes.byteLength > MAX_CAPSULE_FILE_BYTES) return null;
			totalBytes += bytes.byteLength;
			if (totalBytes > MAX_CAPSULE_TOTAL_BYTES) return null;
			payloadEntries.push({
				kind: 'file',
				path: relativePath,
				sha256: createHash('sha256').update(bytes).digest('hex'),
				size: bytes.byteLength,
			});
		}
	}
	payloadEntries.sort(compareCapsulePayloadEntries);
	return createHash('sha256').update(JSON.stringify(payloadEntries)).digest('hex');
}
function compareCapsulePayloadEntries(left, right) {
	if (left.path < right.path) return -1;
	return left.path > right.path ? 1 : 0;
}
export function verifiedCapsuleDigest(rootPath) {
	const manifest = readCapsuleManifest(path.join(rootPath, 'capsule.json'));
	const declared = manifest?.payloadSha256;
	if (
		manifest?.contractVersion !== CAPSULE_CONTRACT_VERSION ||
		typeof declared !== 'string' ||
		!SHA256_PATTERN.test(declared) ||
		Object.keys(manifest).toSorted().join(',') !== 'contractVersion,payloadSha256'
	)
		return null;
	for (const relativePath of REQUIRED_PAYLOAD_FILES) {
		try {
			if (!lstatSync(path.join(rootPath, relativePath)).isFile()) return null;
		} catch {
			return null;
		}
	}
	const computed = capsulePayloadDigest(rootPath);
	return computed === declared ? declared : null;
}
export function options(args, platform = process.platform) {
	const result = {};
	for (let i = 0; i < args.length; i += 2) {
		if (!['--wsl', '--output', '--cargo-target-dir'].includes(args[i]) || !args[i + 1]) {
			throw new Error(
				'Usage: node scripts/stage-local-runtime.mjs --output ABSOLUTE_NEW_DIRECTORY [--wsl DISTRO] [--cargo-target-dir ABSOLUTE_DIRECTORY]',
			);
		}
		if (result[args[i]] !== undefined) throw new Error('Duplicate staging option');
		result[args[i]] = args[i + 1];
	}
	const nativeWindows = platform === 'win32' && !result['--wsl'];
	if (result['--wsl'] && platform !== 'win32')
		throw new Error('--wsl is available only from a Windows host');
	if (
		!result['--output'] ||
		!(nativeWindows ? path.win32 : path.posix).isAbsolute(result['--output'])
	)
		throw new Error('An absolute output directory is required');
	if (!['darwin', 'linux', 'win32'].includes(platform))
		throw new Error('Local startup staging currently supports macOS, Windows and Linux');
	return result;
}
function main() {
	const selected = options(process.argv.slice(2));
	const distro = selected['--wsl'];
	if (process.platform === 'win32' && !distro) {
		stageWindows(root, selected['--output'], selected['--cargo-target-dir'], verifiedCapsuleDigest);
		return;
	}
	const runCommand = (command, args, capture = false) => {
		const executable = distro ? 'wsl.exe' : command;
		const argv = distro
			? ['-d', distro, '--exec', 'bash', '-lc', 'exec "$@"', 'native-stage', command, ...args]
			: args;
		return execFileSync(executable, argv, {
			cwd: root,
			encoding: 'utf8',
			stdio: capture ? 'pipe' : 'inherit',
		});
	};
	const accessible = (nativePath) =>
		distro ? runCommand('wslpath', ['-w', nativePath], true).trim() : nativePath;
	const output = selected['--output'];
	const normalized = path.posix.normalize(output);
	if (normalized !== output || output === '/')
		throw new Error('Output must be a normalized new directory');
	if (existsSync(accessible(output))) throw new Error('Refusing to overwrite an existing cohort');
	const checkout = createHash('sha256').update(root).digest('hex').slice(0, 12);
	const target =
		selected['--cargo-target-dir'] ??
		(distro ? '/tmp/orchbee-native-' + checkout + '-target' : path.join(root, 'target'));
	if (!path.posix.isAbsolute(target)) throw new Error('Cargo target directory must be absolute');
	const destination = accessible(output);
	let temporary;
	let outputCreated = false;
	try {
		runCommand('cargo', [
			'build',
			'--release',
			'--locked',
			'--target-dir',
			target,
			'-p',
			'orchbee',
			'-p',
			'orchbee-server',
			'-p',
			'orchbee-worker',
		]);
		temporary = mkdtempSync(path.join(tmpdir(), 'orchbee-native-host-'));
		const hostPrefix = path.join(temporary, 'host');
		const zigArguments = ['build', '-Doptimize=ReleaseSafe', '--prefix', hostPrefix];
		if (distro) {
			if (runCommand('uname', ['-m'], true).trim() !== 'x86_64')
				throw new Error('Windows cross-staging currently requires x86_64 Linux');
			zigArguments.push('-Dtarget=x86_64-linux-musl');
		}
		execFileSync('zig', zigArguments, {
			cwd: path.join(root, 'apps', 'terminal-host'),
			stdio: 'inherit',
		});
		runCommand('mkdir', ['-m', '700', '--', output]);
		outputCreated = true;
		const hashes = {};
		for (const artifact of artifacts) {
			const source =
				artifact === 'orchbee-terminal-host'
					? path.join(hostPrefix, 'bin', artifact)
					: accessible(path.posix.join(target, 'release', artifact));
			const staged = path.join(destination, artifact);
			copyFileSync(source, staged, constants.COPYFILE_EXCL);
			runCommand('chmod', ['700', path.posix.join(output, artifact)]);
			hashes[artifact] = createHash('sha256').update(readFileSync(staged)).digest('hex');
		}
		const capsuleDestination = path.join(destination, CAPSULE_DIRECTORY);
		cpSync(path.join(root, 'apps', 'desktop', 'resources', CAPSULE_DIRECTORY), capsuleDestination, {
			errorOnExist: true,
			recursive: true,
			verbatimSymlinks: true,
		});
		const adaptersDigest = verifiedCapsuleDigest(capsuleDestination);
		if (adaptersDigest === null)
			throw new Error('Staged observation-adapters capsule failed canonical verification');
		runCommand('chmod', ['700', path.posix.join(output, CAPSULE_DIRECTORY)]);
		hashes[CAPSULE_DIRECTORY] = adaptersDigest;
		writeFileSync(
			path.join(destination, 'cohort.json'),
			JSON.stringify({ schemaVersion: 1, artifacts: hashes }) + '\n',
			{ flag: 'wx', mode: 0o600 },
		);
		runCommand('chmod', ['600', path.posix.join(output, 'cohort.json')]);
		const platform = process.platform === 'darwin' ? 'macOS' : 'Linux';
		const displayPlatform = distro ? `${platform} (WSL)` : platform;
		process.stdout.write(
			`Staged ${displayPlatform} development cohort: ${output}\n` +
				`Run in an interactive ${displayPlatform} terminal: ${path.posix.join(output, 'orchbee')}\n`,
		);
	} catch (error) {
		if (outputCreated) rmSync(destination, { force: true, recursive: true });
		throw error;
	} finally {
		if (temporary) rmSync(temporary, { force: true, recursive: true });
	}
}
if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
	main();
