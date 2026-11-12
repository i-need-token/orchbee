// Write the publishable npm folders of one product version from signed release directories: the main
// package, whose launcher installs and starts the carried program, and one platform sub-package per
// release directory holding that directory unchanged under `release/`. The main package lists exactly
// the sub-packages written in the same run as exact-version optional dependencies. Nothing is packed,
// published, or sent anywhere; publishing the folders is the owner's step.
//
//   node scripts/package-npm.mjs --version <version> --output <absolute-new-directory> <release-directory>...
//
// A release directory is the output of `package-native-windows.mjs` plus `sign-release-manifest.mjs`.
// The release stem names the packages `orchbee` and `@orchbee/cli-<platform>-<arch>`; a test-stem
// rehearsal names them `<stem>` and `@<stem>/cli-<platform>-<arch>`, so it can never produce a release
// name.
import { createHash } from 'node:crypto';
import {
	chmodSync,
	copyFileSync,
	existsSync,
	lstatSync,
	mkdirSync,
	readFileSync,
	writeFileSync,
} from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repository = fileURLToPath(new URL('..', import.meta.url));
const templates = path.join(repository, 'apps', 'orchbee', 'npm');
// The updater reads at most 1 MiB of manifest; this bound never shrinks.
const maximumManifest = 1024 * 1024;
const packagedStem = /^(?:orchbee|ob-test-[0-9a-f]{16})$/u;
// The oldest Node the launcher supports; see apps/orchbee/docs/windows-cli-configuration.md.
const nodeFloor = '>=18';
const description = 'A terminal that runs and orchestrates AI agents';

// Release platform token → npm `os` and `cpu`, which also name the sub-package.
const platforms = new Map([
	['windows-x86_64', { os: 'win32', cpu: 'x64', label: 'Windows x64' }],
	['windows-aarch64', { os: 'win32', cpu: 'arm64', label: 'Windows ARM64' }],
	['macos-aarch64', { os: 'darwin', cpu: 'arm64', label: 'macOS ARM64' }],
	['macos-x86_64', { os: 'darwin', cpu: 'x64', label: 'macOS x64' }],
	['linux-x86_64', { os: 'linux', cpu: 'x64', label: 'Linux x64' }],
	['linux-aarch64', { os: 'linux', cpu: 'arm64', label: 'Linux ARM64' }],
]);
// Each program's suffix to the stem; on macOS and Linux each must stay executable.
const programSuffixes = ['', '-server', '-worker', '-terminal-host'];
// Names npm leaves out of every package whatever `files` says; a release holding one could not be
// carried whole.
const npmDropped =
	/^(?:\.git|\.svn|\.hg|CVS|node_modules|\.npmrc|\.npmignore|\.gitignore|\.DS_Store|npm-debug\.log|\._.*|\..*\.swp|.*\.orig)$/u;

function validReleasePath(value) {
	return (
		typeof value === 'string' &&
		value.length > 0 &&
		value.length <= 240 &&
		value
			.split('/')
			.every((part) => /^[A-Za-z0-9._-]{1,120}$/u.test(part) && part !== '.' && part !== '..')
	);
}

function fileDigest(filename) {
	return createHash('sha256').update(readFileSync(filename)).digest('hex');
}

function writeJson(filename, value) {
	writeFileSync(filename, JSON.stringify(value, null, '\t') + '\n', { flag: 'wx' });
}

/** The manifest of one signed release directory, checked against the version this run publishes. */
function readRelease(directory, version) {
	if (!path.isAbsolute(directory))
		throw new Error(`Release directory must be absolute: ${directory}`);
	const manifestPath = path.join(directory, 'release.json');
	for (const name of ['release.json', 'release.json.sig']) {
		const metadata = lstatSync(path.join(directory, name), { throwIfNoEntry: false });
		if (metadata === undefined || !metadata.isFile())
			throw new Error(`${directory} is not a signed release directory: ${name} is missing`);
	}
	if (lstatSync(manifestPath).size > maximumManifest)
		throw new Error('Release manifest exceeds 1 MiB');
	const manifest = JSON.parse(readFileSync(manifestPath, 'utf8'));
	if (
		manifest.schemaVersion !== 2 ||
		manifest.product !== 'orchbee' ||
		!Array.isArray(manifest.files)
	)
		throw new Error(`${manifestPath} is not an Orchbee release manifest`);
	if (manifest.version !== version)
		throw new Error(`${manifestPath} is version ${manifest.version}, not ${version}`);
	const platform = platforms.get(manifest.platform);
	if (platform === undefined) throw new Error(`Unknown release platform ${manifest.platform}`);
	const executable = platform.os === 'win32' ? '.exe' : '';
	for (const file of manifest.files) {
		if (
			!validReleasePath(file.path) ||
			typeof file.sha256 !== 'string' ||
			!/^[0-9a-f]{64}$/u.test(file.sha256) ||
			!Number.isSafeInteger(file.size) ||
			file.path === 'release.json' ||
			file.path === 'release.json.sig'
		)
			throw new Error(`Invalid release file entry ${JSON.stringify(file.path)}`);
		if (file.path.split('/').some((part) => npmDropped.test(part)))
			throw new Error(`npm would leave out the release file ${file.path}`);
	}
	const stems = manifest.files
		.map((file) => file.path)
		.filter((name) => name.endsWith(executable))
		.map((name) => name.slice(0, name.length - executable.length))
		.filter((name) => packagedStem.test(name));
	if (stems.length !== 1)
		throw new Error(`${manifestPath} must list exactly one orchbee or ob-test-<16 hex> program`);
	return { manifest, platform, stem: stems[0], executable };
}

/** Copies the signed release directory into `destination`, checking every copy against the manifest. */
function copyRelease(directory, release, destination) {
	mkdirSync(destination, { recursive: true });
	for (const name of ['release.json', 'release.json.sig'])
		copyFileSync(path.join(directory, name), path.join(destination, name));
	for (const file of release.manifest.files) {
		const source = path.join(directory, ...file.path.split('/'));
		const metadata = lstatSync(source, { throwIfNoEntry: false });
		if (metadata === undefined || !metadata.isFile())
			throw new Error(`Missing release file ${file.path}`);
		const target = path.join(destination, ...file.path.split('/'));
		mkdirSync(path.dirname(target), { recursive: true });
		// The copy keeps the source's mode on macOS and Linux.
		copyFileSync(source, target);
		if (lstatSync(target).size !== file.size || fileDigest(target) !== file.sha256)
			throw new Error(`Release file ${file.path} does not match its manifest digest`);
	}
	if (release.platform.os === 'win32') return;
	// npm keeps a file's executable bit from the packed folder to the installed one, and sets it
	// itself only for `bin` entries, which a sub-package has none of.
	for (const suffix of programSuffixes) {
		const program = path.join(destination, release.stem + suffix);
		if (!existsSync(program)) throw new Error(`Missing release program ${release.stem + suffix}`);
		chmodSync(program, 0o755);
	}
}

/**
 * Writes the main package and one sub-package per release directory under `output`, a new directory,
 * and returns the written folders.
 */
export function packageNpm({ version, output, releases, host = process.platform }) {
	if (typeof version !== 'string' || version.length === 0)
		throw new Error('A product version is required');
	if (!path.isAbsolute(output)) throw new Error('The output directory must be absolute');
	if (existsSync(output)) throw new Error(`Refusing to write into an existing ${output}`);
	if (releases.length === 0) throw new Error('At least one release directory is required');
	const read = releases.map((directory) => ({
		directory,
		release: readRelease(directory, version),
	}));
	const stem = read[0].release.stem;
	if (read.some(({ release }) => release.stem !== stem))
		throw new Error('Every release directory of one run must carry the same program stem');
	const seen = new Set();
	for (const { release } of read) {
		if (seen.has(release.manifest.platform))
			throw new Error(`Two release directories for ${release.manifest.platform}`);
		seen.add(release.manifest.platform);
		// Windows records no executable bit, so a folder written there would lose it in the package.
		if (release.platform.os !== 'win32' && host === 'win32')
			throw new Error(
				`Write the ${release.manifest.platform} package on macOS or Linux: Windows cannot keep its executable bit`,
			);
	}
	const tag = version.includes('-') ? 'next' : 'latest';
	const publishConfig = { access: 'public', tag };
	const license = path.join(repository, 'LICENSE');
	const written = [];
	const optionalDependencies = {};
	for (const { directory, release } of read) {
		const { os, cpu, label } = release.platform;
		const name = `@${stem}/cli-${os}-${cpu}`;
		const folder = path.join(output, `cli-${os}-${cpu}`);
		copyRelease(directory, release, path.join(folder, 'release'));
		copyFileSync(path.join(templates, 'cli', 'README.md'), path.join(folder, 'README.md'));
		copyFileSync(license, path.join(folder, 'LICENSE'));
		writeJson(path.join(folder, 'package.json'), {
			name,
			version,
			description: `Orchbee for ${label}; install ${stem} instead`,
			license: 'MIT',
			os: [os],
			cpu: [cpu],
			files: ['release'],
			publishConfig,
		});
		optionalDependencies[name] = version;
		written.push(folder);
	}
	const main = path.join(output, stem);
	for (const file of ['bin/orchbee.mjs', 'lib/launcher.mjs', 'README.md']) {
		const target = path.join(main, ...file.split('/'));
		mkdirSync(path.dirname(target), { recursive: true });
		copyFileSync(path.join(templates, 'orchbee', ...file.split('/')), target);
	}
	copyFileSync(license, path.join(main, 'LICENSE'));
	writeJson(path.join(main, 'package.json'), {
		name: stem,
		version,
		description,
		license: 'MIT',
		bin: { [stem]: 'bin/orchbee.mjs' },
		files: ['bin', 'lib'],
		engines: { node: nodeFloor },
		optionalDependencies: Object.fromEntries(
			Object.entries(optionalDependencies).toSorted(([left], [right]) => (left < right ? -1 : 1)),
		),
		publishConfig,
	});
	return [main, ...written];
}

const usage =
	'Usage: node scripts/package-npm.mjs --version <version> --output <absolute-new-directory> <release-directory>...';

export function parseArguments(argv) {
	const options = { releases: [] };
	for (let index = 0; index < argv.length; index++) {
		const argument = argv[index];
		if (argument === '--version' || argument === '--output') {
			const value = argv[++index];
			const key = argument.slice(2);
			if (value === undefined || options[key] !== undefined) throw new Error(usage);
			options[key] = value;
		} else if (argument.startsWith('--')) throw new Error(usage);
		else options.releases.push(argument);
	}
	if (
		options.version === undefined ||
		options.output === undefined ||
		options.releases.length === 0
	)
		throw new Error(usage);
	return options;
}

if (
	process.argv[1] !== undefined &&
	path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
	for (const folder of packageNpm(parseArguments(process.argv.slice(2))))
		process.stdout.write(folder + '\n');
}
