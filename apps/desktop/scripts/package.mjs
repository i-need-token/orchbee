import { execFileSync } from 'node:child_process';
import { access, cp, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
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
	parseDesktopDistributionProfileArguments,
	resolveDesktopTestApplicationName,
} from '../src/distribution/tooling/index.ts';

const electronPackage = createRequire(import.meta.url)('electron/package.json');
if (typeof electronPackage?.version !== 'string') {
	throw new Error('The pinned Electron version is unavailable.');
}

const applicationRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(applicationRoot, '..', '..');
const profileId = parseDesktopDistributionProfileArguments(process.argv.slice(2));
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
// On Windows the shell is `<stem>-desktop.exe`, so its image name differs from the Client's `<stem>.exe`
// and from every other environment's (ORC-382).
const packagedExecutableName =
	process.platform === 'win32'
		? `${cohortEnvironmentStem}-desktop`
		: distributionProfile.applicationName;
const usesDarwinApplicationIcon = process.platform === 'darwin';
const applicationPackage = JSON.parse(
	await readFile(path.join(applicationRoot, 'package.json'), 'utf8'),
);
if (
	typeof applicationPackage?.description !== 'string' ||
	typeof applicationPackage?.version !== 'string'
) {
	throw new Error('The Desktop package identity is unavailable.');
}
const desktopBuild = path.join(applicationRoot, 'dist');
// Cross-runtime contract: search for `packaged-desktop-resources` before changing these names. Each
// extra resource keeps its directory's base name under the application's resources directory.
const rendererBuild = path.join(repositoryRoot, 'apps', 'web', 'dist');
const cohortDirectoryName = 'cohort';
// Windows keeps running in the notification area after its window closes, with these icons.
const trayIconDirectory =
	process.platform === 'win32'
		? path.join(repositoryRoot, 'resources', 'brand', 'ico', 'tray')
		: undefined;
const darwinApplicationIcon = path.join(
	repositoryRoot,
	'resources',
	'brand',
	'icns',
	distributionProfile.iconFileName,
);

await Promise.all([
	access(path.join(desktopBuild, 'main', 'main.cjs')),
	access(path.join(desktopBuild, 'preload', 'preload.cjs')),
	access(path.join(rendererBuild, 'index.html')),
	...(usesDarwinApplicationIcon ? [access(darwinApplicationIcon)] : []),
	...(trayIconDirectory === undefined ? [] : [access(trayIconDirectory)]),
]);

const packageWorkspace = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-package-'));
const stagingRoot = path.join(packageWorkspace, 'application');
const cohortRoot = path.join(packageWorkspace, cohortDirectoryName);
try {
	// The window's Session runs this cohort, so the application carries all of it: the Client, Server,
	// Worker, Terminal Host, the observation Capsule, and the platform's terminal runtime.
	execFileSync(
		process.execPath,
		[
			path.join(repositoryRoot, 'scripts', 'stage-local-runtime.mjs'),
			'--output',
			cohortRoot,
			'--artifact-stem',
			cohortEnvironmentStem,
		],
		{ cwd: repositoryRoot, stdio: 'inherit' },
	);
	await access(path.join(cohortRoot, 'cohort.json'));
	// Staging names the Client after the stem it compiled, so this proves the cohort's stem.
	if (process.platform === 'win32') {
		await access(path.join(cohortRoot, `${cohortEnvironmentStem}.exe`));
	}
	await mkdir(stagingRoot, { mode: 0o700 });
	await cp(desktopBuild, path.join(stagingRoot, 'dist'), { recursive: true });
	await writeFile(
		path.join(stagingRoot, 'package.json'),
		`${JSON.stringify(
			{
				description: applicationPackage.description,
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
		asar: true,
		dir: stagingRoot,
		electronVersion: electronPackage.version,
		extraResource: [
			rendererBuild,
			cohortRoot,
			...(trayIconDirectory === undefined ? [] : [trayIconDirectory]),
		],
		helperBundleId: `${distributionProfile.bundleIdentifier}.helper`,
		...(usesDarwinApplicationIcon ? { icon: darwinApplicationIcon } : {}),
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
						process.platform === 'win32'
							? `${packagedExecutableName}.exe`
							: distributionProfile.applicationName,
					);
		await access(executablePath);
		console.info(`Packaged Orchbee Desktop (${distributionProfile.profileId}): ${output}`);
	}
} finally {
	await rm(packageWorkspace, { force: true, recursive: true });
}
