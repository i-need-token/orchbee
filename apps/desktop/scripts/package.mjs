import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
	access,
	cp,
	mkdir,
	mkdtemp,
	readFile,
	readdir,
	realpath,
	rm,
	stat,
	writeFile,
} from 'node:fs/promises';
import { createRequire } from 'node:module';
import { homedir, tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { packager } from '@electron/packager';

import {
	DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME,
	createDesktopDevelopmentScope,
	resolveDesktopDevelopmentApplicationName,
	resolveDesktopDistributionProfile,
} from '../src/distribution/desktop-distribution-profile.ts';
import {
	createDesktopTestArtifactStem,
	encodeDesktopDistributionDescriptor,
	parseDesktopPackagingArguments,
	resolveDesktopTestApplicationName,
	resolveDesktopWindowsInstallerIdentity,
} from '../src/distribution/tooling/index.ts';
import { writeLinuxMenuFiles, writeLinuxPackages } from './linux-packages.mjs';
import { macDiskImageName, writeMacDiskImage } from './mac-disk-image.mjs';

const electronPackage = createRequire(import.meta.url)('electron/package.json');
if (typeof electronPackage?.version !== 'string') {
	throw new Error('The pinned Electron version is unavailable.');
}

const applicationRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(applicationRoot, '..', '..');
const packaging = parseDesktopPackagingArguments(process.argv.slice(2));
const profileId = packaging.profileId;
const developmentScope =
	profileId === 'dev'
		? createDesktopDevelopmentScope(await realpath(repositoryRoot), process.platform)
		: undefined;
const distributionProfile = (() => {
	if (profileId === 'dev') {
		return resolveDesktopDistributionProfile(profileId, developmentScope, {
			applicationName: resolveDesktopDevelopmentApplicationName(process.env),
		});
	}
	if (profileId === 'test') {
		return resolveDesktopDistributionProfile(profileId, undefined, {
			applicationName: resolveDesktopTestApplicationName(process.env),
			artifactStem: createDesktopTestArtifactStem(homedir()),
		});
	}
	return resolveDesktopDistributionProfile(profileId);
})();
// The environment stem every cohort executable compiles in, which the handshake compares; it also
// names the Client in the cohort and, on Windows, the packaged executable.
const cohortEnvironmentStem = distributionProfile.artifactStem;
// Only a cohort compiled with the release stem may ship as release (ORC-382).
if (distributionProfile.profileId === 'release' && cohortEnvironmentStem !== 'orchbee') {
	throw new Error(
		`Release packaging refuses the ${cohortEnvironmentStem} cohort; it requires orchbee.`,
	);
}
// A test or release cohort ships as a signed release directory, so its bundled Client's
// `orchbee install` can publish it into the store; a development cohort cannot be packaged that way.
// The installer is Setup on Windows, the disk image on macOS, and the AppImage and Debian package on
// Linux.
const signsReleaseDirectory =
	['win32', 'darwin', 'linux'].includes(process.platform) && profileId !== 'dev';
if (packaging.installer && !signsReleaseDirectory) {
	throw new Error(
		'The installer is built on Windows, macOS and Linux, for the test and release profiles only.',
	);
}
if (packaging.cohort !== undefined && !signsReleaseDirectory) {
	throw new Error(
		'A signed cohort is packaged on Windows, macOS and Linux, for the test and release profiles only.',
	);
}
// On Windows and Linux the shell is `<stem>-desktop`, so its process name differs from the Client's
// `<stem>` and from every other environment's (ORC-382); on Linux it is also the program the window
// store's entry starts.
const packagedExecutableName =
	process.platform === 'win32' || process.platform === 'linux'
		? `${cohortEnvironmentStem}-desktop`
		: distributionProfile.applicationName;
const applicationPackage = JSON.parse(
	await readFile(path.join(applicationRoot, 'package.json'), 'utf8'),
);
if (
	typeof applicationPackage?.description !== 'string' ||
	typeof applicationPackage?.version !== 'string'
) {
	throw new Error('The Desktop package identity is unavailable.');
}
// One version across every program of a release: the manifest, the Client's `--version` and the
// installer all carry the workspace version, which the release-version script also writes here.
const productVersion = await readWorkspaceVersion();
if (signsReleaseDirectory && productVersion !== applicationPackage.version) {
	throw new Error(
		`The Desktop version ${applicationPackage.version} differs from the workspace version ${productVersion}.`,
	);
}
const desktopBuild = path.join(applicationRoot, 'dist');
// Cross-runtime contract: search for `packaged-desktop-resources` before changing these names. Each
// extra resource keeps its directory's base name under the application's resources directory.
const rendererBuild = path.join(repositoryRoot, 'apps', 'web', 'dist');
const cohortDirectoryName = 'cohort';
// The icon Desktop shows from start until quit: `.ico` files in the Windows notification area,
// template PNGs in the macOS menu bar and PNGs in the Linux panel, carried as `resources/tray`.
const trayIconDirectory = {
	darwin: path.join(repositoryRoot, 'resources', 'brand', 'png', 'tray'),
	linux: path.join(repositoryRoot, 'resources', 'brand', 'png', 'linux', 'tray'),
	win32: path.join(repositoryRoot, 'resources', 'brand', 'ico', 'tray'),
}[process.platform];
// Every Windows profile carries the Orchbee mark; the installer's names tell a test build apart.
const applicationIcon = {
	darwin: path.join(repositoryRoot, 'resources', 'brand', 'icns', distributionProfile.iconFileName),
	win32: path.join(repositoryRoot, 'resources', 'brand', 'ico', 'orchbee.ico'),
}[process.platform];

await Promise.all([
	access(path.join(desktopBuild, 'main', 'main.cjs')),
	access(path.join(desktopBuild, 'preload', 'preload.cjs')),
	access(path.join(rendererBuild, 'index.html')),
	...(applicationIcon === undefined ? [] : [access(applicationIcon)]),
	...(trayIconDirectory === undefined ? [] : [access(trayIconDirectory)]),
]);

const packageWorkspace = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-package-'));
const stagingRoot = path.join(packageWorkspace, 'application');
const cohortRoot = path.join(packageWorkspace, cohortDirectoryName);
// Linux carries its menu entry template and icons as `resources/linux`, which the window store copies
// out (apps/orchbee/src/cli/update/menu_entry.rs) and the Linux packages install.
const linuxMenuRoot = path.join(packageWorkspace, 'linux');
// The packaged Electron's architecture: the host's, or a signed macOS or Linux cohort's platform,
// which a host of the other architecture may package.
let packagedArchitecture;
try {
	if (packaging.cohort === undefined) {
		// The window's Session runs this cohort, so the application carries all of it: the Client,
		// Server, Worker, Terminal Host, the observation Capsule, and the platform's terminal runtime.
		runNode('stage-local-runtime.mjs', [
			'--output',
			cohortRoot,
			'--artifact-stem',
			cohortEnvironmentStem,
		]);
	} else {
		// A cohort staged and signed earlier travels unchanged; the installer's `orchbee install`
		// verifies its signature and every file.
		await cp(packaging.cohort, cohortRoot, { recursive: true });
		await Promise.all(
			['release.json', 'release.json.sig'].map((name) => access(path.join(cohortRoot, name))),
		);
		const manifest = JSON.parse(await readFile(path.join(cohortRoot, 'release.json'), 'utf8'));
		if (manifest?.version !== productVersion) {
			throw new Error(
				`The signed cohort is version ${String(manifest?.version)}, not the workspace version ${productVersion}.`,
			);
		}
		if (process.platform === 'darwin') {
			packagedArchitecture = { 'macos-aarch64': 'arm64', 'macos-x86_64': 'x64' }[manifest.platform];
			if (packagedArchitecture === undefined)
				throw new Error(`The signed cohort is for ${String(manifest.platform)}, not macOS.`);
		} else if (process.platform === 'linux') {
			packagedArchitecture = { 'linux-aarch64': 'arm64', 'linux-x86_64': 'x64' }[manifest.platform];
			if (packagedArchitecture === undefined)
				throw new Error(`The signed cohort is for ${String(manifest.platform)}, not Linux.`);
		}
	}
	await access(path.join(cohortRoot, 'cohort.json'));
	// Staging names the Client after the stem it compiled, so this proves the cohort's stem.
	await access(
		path.join(
			cohortRoot,
			process.platform === 'win32' ? `${cohortEnvironmentStem}.exe` : cohortEnvironmentStem,
		),
	);
	if (signsReleaseDirectory && packaging.cohort === undefined) {
		// Every version is published on `stable` until `channel set preview` is released.
		runNode('package-native-release.mjs', [
			cohortRoot,
			'stable',
			productVersion,
			packaging.buildId ?? `${sourceCommit()}.local`,
		]);
		runNode('sign-release-manifest.mjs', [path.join(cohortRoot, 'release.json'), ...signingKeys()]);
	}
	// macOS signs the application after packaging; this record proves signing left the cohort as it was.
	const stagedCohortFiles =
		process.platform === 'darwin' ? await cohortFileDigests(cohortRoot) : undefined;
	if (process.platform === 'linux') {
		await writeLinuxMenuFiles({
			applicationName: distributionProfile.applicationName,
			directory: linuxMenuRoot,
			repositoryRoot,
			stem: cohortEnvironmentStem,
		});
	}
	await mkdir(stagingRoot, { mode: 0o700 });
	await cp(desktopBuild, path.join(stagingRoot, 'dist'), { recursive: true });
	await writeFile(
		path.join(stagingRoot, 'package.json'),
		`${JSON.stringify(
			{
				description: applicationPackage.description,
				// Electron takes the Linux window's Wayland application id from it, matching the menu entry.
				...(process.platform === 'linux'
					? { desktopName: `${cohortEnvironmentStem}.desktop` }
					: {}),
				main: 'dist/main/main.cjs',
				name: distributionProfile.packageName,
				private: true,
				version: applicationPackage.version,
			},
			null,
			2,
		)}\n`,
		'utf8',
	);
	await writeFile(
		path.join(stagingRoot, DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME),
		encodeDesktopDistributionDescriptor(distributionProfile),
		{ encoding: 'utf8', mode: 0o400 },
	);
	const outputs = await packager({
		appBundleId: distributionProfile.bundleIdentifier,
		...(packagedArchitecture === undefined ? {} : { arch: packagedArchitecture }),
		asar: true,
		dir: stagingRoot,
		electronVersion: electronPackage.version,
		// The oldest macOS release Orchbee runs on; packager merges this into the application's Info.plist.
		...(process.platform === 'darwin' ? { extendInfo: { LSMinimumSystemVersion: '13.0' } } : {}),
		extraResource: [
			rendererBuild,
			cohortRoot,
			...(trayIconDirectory === undefined ? [] : [trayIconDirectory]),
			...(process.platform === 'linux' ? [linuxMenuRoot] : []),
		],
		helperBundleId: `${distributionProfile.bundleIdentifier}.helper`,
		...(applicationIcon === undefined ? {} : { icon: applicationIcon }),
		name: packagedExecutableName,
		out: path.join(applicationRoot, 'out', distributionProfile.outputDirectoryName),
		overwrite: true,
		prune: false,
		// The file properties keep the display name while the process image carries the stem.
		win32metadata: {
			FileDescription: distributionProfile.applicationName,
			ProductName: distributionProfile.applicationName,
		},
	});
	for (const output of outputs) {
		const applicationFileName = `${distributionProfile.applicationName}.app`;
		const executablePath =
			process.platform === 'darwin'
				? path.join(
						output,
						applicationFileName,
						'Contents',
						'MacOS',
						distributionProfile.applicationName,
					)
				: path.join(
						output,
						process.platform === 'win32' ? `${packagedExecutableName}.exe` : packagedExecutableName,
					);
		await access(executablePath);
		if (process.platform === 'darwin') {
			const application = path.join(output, applicationFileName);
			signMacApplication(application);
			await verifyBundledCohort(
				path.join(application, 'Contents', 'Resources', cohortDirectoryName),
				stagedCohortFiles,
			);
			// The window file list of the signed bundle, beside it, since a file inside the bundle's
			// root would break its seal; it keeps the bundle's links and execute bits.
			if (signsReleaseDirectory)
				runNode('desktop-files.mjs', ['write', output, productVersion, applicationFileName]);
			console.info(`Packaged Orchbee Desktop (${distributionProfile.profileId}): ${application}`);
			if (packaging.installer) await createMacDiskImage(application);
			continue;
		}
		// The window file list, from the final bytes and before Setup is compiled, so Setup and the
		// release's window archive carry the same list; it refuses links and names outside the
		// release path grammar.
		if (signsReleaseDirectory) runNode('desktop-files.mjs', ['write', output, productVersion]);
		console.info(`Packaged Orchbee Desktop (${distributionProfile.profileId}): ${output}`);
		if (packaging.installer && process.platform === 'linux') await createLinuxPackages(output);
		else if (packaging.installer) await compileWindowsInstaller(output);
	}
} finally {
	await rm(packageWorkspace, { force: true, recursive: true });
}

/** Runs one repository script with this Node.js, in the repository root. */
function runNode(script, arguments_) {
	execFileSync(process.execPath, [path.join(repositoryRoot, 'scripts', script), ...arguments_], {
		cwd: repositoryRoot,
		stdio: 'inherit',
		windowsHide: true,
	});
}

/** The `[workspace.package] version` of the root `Cargo.toml`, the release's one version. */
async function readWorkspaceVersion() {
	const manifest = await readFile(path.join(repositoryRoot, 'Cargo.toml'), 'utf8');
	const workspacePackage =
		/^\[workspace\.package\]\r?\n([\s\S]*?)(?=^\[|(?![\s\S]))/mu.exec(manifest)?.[1] ?? '';
	const version = /^version = "([^"]+)"$/mu.exec(workspacePackage)?.[1];
	if (version === undefined) throw new Error('Cargo.toml declares no workspace package version.');
	return version;
}

/** The commit the build names: `ORCHBEE_SOURCE_COMMIT`, as the release workflow sets it, or HEAD. */
function sourceCommit() {
	const configured = process.env['ORCHBEE_SOURCE_COMMIT'];
	if (configured !== undefined && configured !== '') return configured;
	return execFileSync('git', ['rev-parse', 'HEAD'], {
		cwd: repositoryRoot,
		encoding: 'utf8',
		windowsHide: true,
	}).trim();
}

/**
 * The manifest's signing keys: those given, otherwise the repository's test key for a test build,
 * which only test builds trust, and the release workflow's secret for a release.
 */
function signingKeys() {
	if (packaging.signingKeys.length > 0) return packaging.signingKeys;
	return profileId === 'test'
		? [
				'--key-file',
				path.join(
					repositoryRoot,
					'apps',
					'orchbee',
					'src',
					'cli',
					'update',
					'tests',
					'fixtures',
					'test-only-signing-key.pem',
				),
			]
		: ['--key-env', 'ORCHBEE_RELEASE_SIGNING_KEY'];
}

/**
 * Signs the packaged application ad hoc, its only identity until Developer ID signing is decided:
 * Apple silicon runs no unsigned code, and the packager's renames and property-list edits broke
 * Electron's own signatures. `--deep` signs the nested helpers and frameworks; the cohort under
 * `Contents/Resources` is sealed as resources, so its bytes stay as staged. A Developer ID identity
 * replaces only this function.
 */
function signMacApplication(application) {
	execFileSync(
		'/usr/bin/codesign',
		['--force', '--deep', '--sign', '-', '--timestamp=none', application],
		{ stdio: 'inherit' },
	);
	execFileSync(
		'/usr/bin/codesign',
		['--verify', '--deep', '--strict', '--verbose=2', application],
		{
			stdio: 'inherit',
		},
	);
}

/** Every file below `root` as `{ path, sha256, size }` in path order; a link is refused. */
async function cohortFileDigests(root) {
	const files = [];
	for (const entry of await readdir(root, { recursive: true, withFileTypes: true })) {
		const filename = path.join(entry.parentPath, entry.name);
		if (entry.isDirectory()) continue;
		if (!entry.isFile()) throw new Error(`The cohort holds a link or special file: ${filename}`);
		const bytes = await readFile(filename);
		files.push({
			path: path.relative(root, filename).split(path.sep).join('/'),
			sha256: createHash('sha256').update(bytes).digest('hex'),
			size: bytes.length,
		});
	}
	return files.toSorted(byPath);
}

function byPath(left, right) {
	if (left.path === right.path) return 0;
	return left.path < right.path ? -1 : 1;
}

/**
 * Proves the signed application's cohort is the staged one, byte for byte, and that its programs and
 * files still match `cohort.json` and a signed release directory's `release.json`, which the
 * handshake and `orchbee install` check.
 */
async function verifyBundledCohort(directory, stagedFiles) {
	const bundled = await cohortFileDigests(directory);
	const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
	if (stagedFiles === undefined || !same(bundled, stagedFiles)) {
		throw new Error(`The cohort in ${directory} differs from the staged cohort after signing.`);
	}
	const cohortManifest = JSON.parse(await readFile(path.join(directory, 'cohort.json'), 'utf8'));
	for (const [role, suffix] of [
		['orchbee', ''],
		['orchbee-server', '-server'],
		['orchbee-worker', '-worker'],
		['orchbee-terminal-host', '-terminal-host'],
	]) {
		const file = bundled.find((entry) => entry.path === `${cohortEnvironmentStem}${suffix}`);
		if (file === undefined || file.sha256 !== cohortManifest?.artifacts?.[role]) {
			throw new Error(`The bundled ${role} does not match cohort.json.`);
		}
	}
	if (!signsReleaseDirectory) return;
	const releaseManifest = JSON.parse(await readFile(path.join(directory, 'release.json'), 'utf8'));
	const listed = (Array.isArray(releaseManifest?.files) ? releaseManifest.files : [])
		.map((file) => ({ path: file?.path, sha256: file?.sha256, size: file?.size }))
		.toSorted(byPath);
	const present = bundled.filter(
		(file) => file.path !== 'release.json' && file.path !== 'release.json.sig',
	);
	if (!same(listed, present)) {
		throw new Error('The bundled cohort does not match its release.json.');
	}
}

/**
 * Writes `<out>/<profile>/installer/<name>.dmg`, a compressed read-only image holding the signed
 * application beside a link to `/Applications` over the brand background, named for the signed
 * manifest's platform.
 */
async function createMacDiskImage(application) {
	const releaseManifest = JSON.parse(
		await readFile(
			path.join(application, 'Contents', 'Resources', cohortDirectoryName, 'release.json'),
			'utf8',
		),
	);
	const filename = macDiskImageName(
		profileId === 'release' ? 'Orchbee' : distributionProfile.artifactStem,
		productVersion,
		releaseManifest?.platform,
	);
	const outputDirectory = path.join(
		applicationRoot,
		'out',
		distributionProfile.outputDirectoryName,
		'installer',
	);
	await rm(outputDirectory, { force: true, recursive: true });
	await mkdir(outputDirectory, { recursive: true });
	const workspace = path.join(packageWorkspace, 'disk-image');
	await mkdir(workspace);
	const image = path.join(outputDirectory, filename);
	await writeMacDiskImage({
		application,
		volumeName: distributionProfile.applicationName,
		image,
		workspace,
	});
	console.info(`Created the macOS disk image (${profileId}): ${image}`);
}

/**
 * Writes `<out>/<profile>/installer/<name>-<version>-<platform>.AppImage` and `.deb` from the
 * packaged application, named for the signed manifest's platform.
 */
async function createLinuxPackages(applicationDirectory) {
	const releaseManifest = JSON.parse(
		await readFile(
			path.join(applicationDirectory, 'resources', cohortDirectoryName, 'release.json'),
			'utf8',
		),
	);
	const outputDirectory = path.join(
		applicationRoot,
		'out',
		distributionProfile.outputDirectoryName,
		'installer',
	);
	await rm(outputDirectory, { force: true, recursive: true });
	await mkdir(outputDirectory, { recursive: true });
	const workspace = path.join(packageWorkspace, 'linux-packages');
	await mkdir(workspace);
	const written = await writeLinuxPackages({
		applicationDirectory,
		applicationName: distributionProfile.applicationName,
		artifactName: profileId === 'release' ? 'Orchbee' : distributionProfile.artifactStem,
		outputDirectory,
		platform: releaseManifest?.platform,
		stem: cohortEnvironmentStem,
		version: productVersion,
		workspace,
	});
	for (const file of written) console.info(`Created the Linux package (${profileId}): ${file}`);
}

/** The total size in bytes of the regular files below `directory`. */
async function treeSize(directory) {
	let total = 0;
	for (const entry of await readdir(directory, { recursive: true, withFileTypes: true })) {
		if (entry.isFile()) total += (await stat(path.join(entry.parentPath, entry.name))).size;
	}
	return total;
}

/**
 * Compiles `installer/windows/orchbee.iss` over the packaged application with Inno Setup's
 * command-line compiler: `ORCHBEE_INNO_SETUP_COMPILER` names `ISCC.exe`, otherwise the default
 * Inno Setup 6 folder; the script refuses every compiler version but the pinned one.
 */
async function compileWindowsInstaller(applicationDirectory) {
	const identity = resolveDesktopWindowsInstallerIdentity(distributionProfile, productVersion);
	const compiler =
		process.env['ORCHBEE_INNO_SETUP_COMPILER'] ??
		path.join(
			process.env['ProgramFiles(x86)'] ?? 'C:\\Program Files (x86)',
			'Inno Setup 6',
			'ISCC.exe',
		);
	if (!path.isAbsolute(compiler)) {
		throw new Error('ORCHBEE_INNO_SETUP_COMPILER must name ISCC.exe by an absolute path.');
	}
	await access(compiler);
	const outputDirectory = path.join(
		applicationRoot,
		'out',
		distributionProfile.outputDirectoryName,
		'installer',
	);
	await rm(outputDirectory, { force: true, recursive: true });
	await mkdir(outputDirectory, { recursive: true });
	// Settings › Apps shows the window application plus the store's copy of its runtime.
	const displaySize =
		(await treeSize(applicationDirectory)) +
		(await treeSize(path.join(applicationDirectory, 'resources', cohortDirectoryName)));
	const defines = {
		AppId: identity.appId,
		AppName: identity.displayName,
		AppPublisher: 'Orchbee',
		AppUserModelId: distributionProfile.bundleIdentifier,
		AppVersion: productVersion,
		AppVersionNumeric: productVersion.replace(/[-+].*$/u, ''),
		ClientName: identity.clientName,
		DesktopExecutable: `${packagedExecutableName}.exe`,
		DisplaySize: String(displaySize),
		IconFile: applicationIcon,
		OutputBaseFilename: identity.setupBaseName,
		OutputDir: outputDirectory,
		SourceDir: applicationDirectory,
		StoreFolderName: identity.storeFolderName,
	};
	execFileSync(
		compiler,
		[
			...Object.entries(defines).map(([name, value]) => `/D${name}=${value}`),
			path.join(applicationRoot, 'installer', 'windows', 'orchbee.iss'),
		],
		{ cwd: applicationRoot, stdio: 'inherit', windowsHide: true },
	);
	const setup = path.join(outputDirectory, `${identity.setupBaseName}.exe`);
	console.info(`Compiled the Windows installer (${profileId}): ${setup}`);
}
