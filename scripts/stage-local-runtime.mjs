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
	realpathSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import {
	ARTIFACT_STEM_PATTERN,
	stageWindows,
	terminalHostBuildOptions,
} from './stage-native-windows.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const artifacts = ['orchbee', 'orchbee-server', 'orchbee-worker', 'orchbee-terminal-host'];
const CAPSULE_DIRECTORY = 'agent-observation-adapters';
// The Host gives zsh and bash their managed startup from these scripts beside its executable.
const SHELL_INTEGRATION_DIRECTORY = 'shell-integration';
const CAPSULE_CONTRACT_VERSION = 1;
// A Mac stages either architecture's cohort; the other one builds with Cargo's and Zig's targets.
const MACOS_TARGETS = new Map([
	['macos-aarch64', { arch: 'arm64', rust: 'aarch64-apple-darwin', zig: 'aarch64-macos' }],
	['macos-x86_64', { arch: 'x64', rust: 'x86_64-apple-darwin', zig: 'x86_64-macos' }],
]);
// A Linux release row links every program statically against musl, on a host of its architecture,
// so the cohort names no C library and runs on any distribution.
const LINUX_TARGETS = new Map([
	['linux-x86_64', { arch: 'x64', rust: 'x86_64-unknown-linux-musl', zig: 'x86_64-linux-musl' }],
	[
		'linux-aarch64',
		{ arch: 'arm64', rust: 'aarch64-unknown-linux-musl', zig: 'aarch64-linux-musl' },
	],
]);
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
const CAPSULE_OWNER_RECORD = 'bundle/scripts/environment.json';
// The payload digest computed here, independently of the Rust writer, for `verifiedCapsuleDigest`.
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
			// The manifest and the owner record that publishing adds are outside the payload identity.
			if (relativePath === 'capsule.json' || relativePath === CAPSULE_OWNER_RECORD) continue;
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
/**
 * The development stem of a checkout, so a cohort staged without `--artifact-stem` never carries the
 * release identity. Mirrors `createDesktopDevelopmentScope` in desktop-distribution-profile.ts, so the
 * Desktop development cohort and a hand-staged cohort of one checkout share one environment.
 */
export function developmentArtifactStem(canonicalSourceRoot, platform = process.platform) {
	return 'ob-dev-' + checkoutDigest(canonicalSourceRoot, platform);
}
/**
 * The 16-digit digest of a checkout's canonical path that names its development and test
 * environments. `crates/local-runtime/build.rs` derives a plain Cargo build's `ob-test-` stem from
 * the same digest, and the Terminal Host package build gives its Host that stem.
 */
export function checkoutDigest(canonicalSourceRoot, platform = process.platform) {
	const normalized = (platform === 'win32' ? path.win32 : path.posix)
		.normalize(canonicalSourceRoot)
		.normalize('NFC');
	const canonical =
		platform === 'win32' && /^[A-Z]:/u.test(normalized)
			? normalized[0].toLowerCase() + normalized.slice(1)
			: normalized;
	return createHash('sha256')
		.update('orchbee-development-data-root-v1\0')
		.update(canonical)
		.digest('hex')
		.slice(0, 16);
}
export function options(args, platform = process.platform) {
	const result = {};
	for (let i = 0; i < args.length; i += 2) {
		if (
			!['--wsl', '--output', '--cargo-target-dir', '--artifact-stem', '--platform'].includes(
				args[i],
			) ||
			!args[i + 1]
		) {
			throw new Error(
				'Usage: node scripts/stage-local-runtime.mjs --output ABSOLUTE_NEW_DIRECTORY [--wsl DISTRO] [--cargo-target-dir ABSOLUTE_DIRECTORY] [--artifact-stem STEM] [--platform macos-aarch64|macos-x86_64|linux-x86_64|linux-aarch64]',
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
	const artifactStem = result['--artifact-stem'];
	if (artifactStem !== undefined && !ARTIFACT_STEM_PATTERN.test(artifactStem))
		throw new Error('--artifact-stem must be orchbee, ob-dev-<16 hex>, or ob-test-<16 hex>');
	const targets = new Map([
		['darwin', MACOS_TARGETS],
		['linux', LINUX_TARGETS],
	]).get(platform);
	if (result['--platform'] !== undefined && !targets?.has(result['--platform']))
		throw new Error(
			'--platform is macos-aarch64 or macos-x86_64 on a macOS host, or linux-x86_64 or linux-aarch64 on a Linux host',
		);
	if (!['darwin', 'linux', 'win32'].includes(platform))
		throw new Error('Local startup staging currently supports macOS, Windows and Linux');
	return result;
}
function main() {
	const selected = options(process.argv.slice(2));
	const distro = selected['--wsl'];
	// Release staging names `orchbee` explicitly; anything else is this checkout's development cohort.
	const artifactStem =
		selected['--artifact-stem'] ?? developmentArtifactStem(realpathSync.native(root));
	if (process.platform === 'win32' && !distro) {
		stageWindows(
			root,
			selected['--output'],
			selected['--cargo-target-dir'],
			verifiedCapsuleDigest,
			artifactStem,
		);
		return;
	}
	// `wsl.exe` forwards no environment, so added variables travel as `env` arguments there.
	const runCommand = (command, args, capture = false, environment = {}) => {
		const assignments = Object.entries(environment).map(([name, value]) => `${name}=${value}`);
		const executable = distro ? 'wsl.exe' : command;
		const argv = distro
			? [
					'-d',
					distro,
					'--exec',
					'bash',
					'-lc',
					'exec "$@"',
					'native-stage',
					...(assignments.length > 0 ? ['env', ...assignments] : []),
					command,
					...args,
				]
			: args;
		return execFileSync(executable, argv, {
			cwd: root,
			encoding: 'utf8',
			env: distro ? process.env : { ...process.env, ...environment },
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
	// A macOS or Linux cohort also carries the window launcher (apps/orchbee-desktop-launcher/README.md):
	// a macOS window version's bundle runs it to replace the application when the window relaunches,
	// and the Linux window store copies it out as the window entry. A WSL cohort serves no window.
	const windowLauncher =
		process.platform === 'darwin' || process.platform === 'linux'
			? 'orchbee-desktop-launcher'
			: undefined;
	const macosTarget = MACOS_TARGETS.get(selected['--platform'] ?? '');
	const linuxTarget = LINUX_TARGETS.get(selected['--platform'] ?? '');
	if (linuxTarget !== undefined && linuxTarget.arch !== process.arch)
		throw new Error(`${selected['--platform']} stages on a host of its own architecture`);
	// Cargo's and Zig's target when the host's own does not build the row: the other Mac
	// architecture, or musl on Linux.
	const cross = linuxTarget ?? (macosTarget?.arch === process.arch ? undefined : macosTarget);
	const built = cross === undefined ? 'release' : path.posix.join(cross.rust, 'release');
	let temporary;
	let outputCreated = false;
	try {
		// The stem names the environment and every staged file name.
		runCommand(
			'cargo',
			[
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
				...(windowLauncher === undefined ? [] : ['-p', windowLauncher]),
				...(cross === undefined ? [] : ['--target', cross.rust]),
			],
			false,
			{ ORCHBEE_ARTIFACT_STEM: artifactStem },
		);
		temporary = mkdtempSync(path.join(tmpdir(), 'orchbee-native-host-'));
		const hostPrefix = path.join(temporary, 'host');
		const zigArguments = [
			'build',
			'-Doptimize=ReleaseSafe',
			'--prefix',
			hostPrefix,
			...terminalHostBuildOptions(root, artifactStem),
		];
		if (distro) {
			if (runCommand('uname', ['-m'], true).trim() !== 'x86_64')
				throw new Error('Windows cross-staging currently requires x86_64 Linux');
			zigArguments.push('-Dtarget=x86_64-linux-musl');
		}
		if (cross !== undefined) zigArguments.push(`-Dtarget=${cross.zig}`);
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
					: accessible(path.posix.join(target, built, artifact));
			// Cargo and Zig name the sources; the staged cohort carries the environment stem.
			const stagedName = artifactStem + artifact.slice('orchbee'.length);
			const staged = path.join(destination, stagedName);
			copyFileSync(source, staged, constants.COPYFILE_EXCL);
			runCommand('chmod', ['700', path.posix.join(output, stagedName)]);
			hashes[artifact] = createHash('sha256').update(readFileSync(staged)).digest('hex');
		}
		if (windowLauncher !== undefined) {
			// No cohort role, so `cohort.json` does not list it; the release manifest inventories it.
			const stagedName = `${artifactStem}-desktop-launcher`;
			copyFileSync(
				accessible(path.posix.join(target, built, windowLauncher)),
				path.join(destination, stagedName),
				constants.COPYFILE_EXCL,
			);
			runCommand('chmod', ['700', path.posix.join(output, stagedName)]);
		}
		cpSync(
			path.join(hostPrefix, 'bin', SHELL_INTEGRATION_DIRECTORY),
			path.join(destination, SHELL_INTEGRATION_DIRECTORY),
			{ errorOnExist: true, recursive: true },
		);
		runCommand('chmod', ['700', path.posix.join(output, SHELL_INTEGRATION_DIRECTORY)]);
		const capsuleDestination = path.join(destination, CAPSULE_DIRECTORY);
		cpSync(
			path.join(root, 'crates', 'agent-integration-writer', CAPSULE_DIRECTORY),
			capsuleDestination,
			{
				errorOnExist: true,
				recursive: true,
				verbatimSymlinks: true,
			},
		);
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
			`Staged ${displayPlatform} ${artifactStem} cohort: ${output}\n` +
				`Run in an interactive ${displayPlatform} terminal: ${path.posix.join(output, artifactStem)}\n`,
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
