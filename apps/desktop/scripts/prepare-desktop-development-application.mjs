import { createHash } from 'node:crypto';
import { execFile } from 'node:child_process';
import {
	access,
	cp,
	mkdir,
	mkdtemp,
	readFile,
	realpath,
	rename,
	rm,
	writeFile,
} from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';

import {
	createDesktopDevelopmentScope,
	resolveDesktopDevelopmentApplicationName,
	resolveDesktopDistributionProfile,
} from '../src/distribution/desktop-distribution-profile.ts';
import { materializeDarwinApplicationIcon } from './materialize-darwin-application-icon.mjs';

const executeFile = promisify(execFile);
const DEVELOPMENT_APPLICATION_IDENTITY_SCHEMA_VERSION = 1;
const DEVELOPMENT_APPLICATION_MARKER_FILE_NAME = 'desktop-development-application-v1.json';
const ELECTRON_HELPER_NAMES = [
	'Electron Helper',
	'Electron Helper (GPU)',
	'Electron Helper (Plugin)',
	'Electron Helper (Renderer)',
];

export async function prepareDesktopDevelopmentExecutable(options) {
	if (process.platform !== 'darwin') return options.electronExecutable;
	const [sourceRoot, sourceExecutable, baseIconSvg] = await Promise.all([
		realpath(options.sourceRoot),
		realpath(options.electronExecutable),
		readFile(options.baseIconPath, 'utf8'),
	]);
	const sourceApplicationPath = resolveSourceApplicationPath(sourceExecutable);
	const electronPackage = createRequire(import.meta.url)('electron/package.json');
	if (typeof electronPackage?.version !== 'string') {
		throw new Error('The pinned Electron version is unavailable.');
	}
	const profile = resolveDesktopDistributionProfile(
		'dev',
		createDesktopDevelopmentScope(sourceRoot, process.platform),
		{ applicationName: resolveDesktopDevelopmentApplicationName(options.environment) },
	);
	const marker = `${JSON.stringify(
		{
			applicationName: profile.applicationName,
			bundleIdentifier: profile.bundleIdentifier,
			electronVersion: electronPackage.version,
			iconDigest: createHash('sha256').update(baseIconSvg).digest('hex'),
			schemaVersion: DEVELOPMENT_APPLICATION_IDENTITY_SCHEMA_VERSION,
			sourceApplicationPath,
		},
		null,
		2,
	)}\n`;
	const cacheKey = createHash('sha256').update(marker).digest('hex').slice(0, 24);
	const outputRoot = path.resolve(options.outputRoot);
	const cacheDirectory = path.join(outputRoot, `${profile.dataScope.digest}-${cacheKey}`);
	const applicationPath = path.join(cacheDirectory, `${profile.applicationName}.app`);
	const executablePath = path.join(applicationPath, 'Contents', 'MacOS', 'Electron');
	if (await isPreparedApplication(cacheDirectory, executablePath, marker, profile)) {
		return executablePath;
	}

	await mkdir(outputRoot, { mode: 0o700, recursive: true });
	const stagingDirectory = await mkdtemp(path.join(outputRoot, '.development-application-'));
	let stagingOwned = true;
	try {
		const stagedApplicationPath = path.join(stagingDirectory, `${profile.applicationName}.app`);
		await cloneApplicationBundle(sourceApplicationPath, stagedApplicationPath);
		await applyApplicationIdentity({
			applicationPath: stagedApplicationPath,
			baseIconSvg,
			profile,
			stagingDirectory,
		});
		await writeFile(path.join(stagingDirectory, DEVELOPMENT_APPLICATION_MARKER_FILE_NAME), marker, {
			encoding: 'utf8',
			mode: 0o600,
		});
		try {
			await rename(stagingDirectory, cacheDirectory);
			stagingOwned = false;
		} catch (error) {
			if (
				!isDestinationConflict(error) ||
				!(await isPreparedApplication(cacheDirectory, executablePath, marker, profile))
			) {
				throw error;
			}
		}
		return executablePath;
	} finally {
		if (stagingOwned) await rm(stagingDirectory, { force: true, recursive: true });
	}
}

async function applyApplicationIdentity(options) {
	const iconWorkspace = path.join(options.stagingDirectory, 'icon-workspace');
	try {
		const generatedIcon = await materializeDarwinApplicationIcon({
			baseIconSvg: options.baseIconSvg,
			outputDirectory: iconWorkspace,
			profile: options.profile,
		});
		const iconFileName = 'orchbee-dev.icns';
		await cp(
			generatedIcon,
			path.join(options.applicationPath, 'Contents', 'Resources', iconFileName),
		);
		await replacePlistString(
			path.join(options.applicationPath, 'Contents', 'Info.plist'),
			'CFBundleIconFile',
			iconFileName,
		);
		const applicationInfoPlist = path.join(options.applicationPath, 'Contents', 'Info.plist');
		await replacePlistString(
			applicationInfoPlist,
			'CFBundleDisplayName',
			options.profile.applicationName,
		);
		await replacePlistString(
			applicationInfoPlist,
			'CFBundleIdentifier',
			options.profile.bundleIdentifier,
		);
		await replacePlistString(applicationInfoPlist, 'CFBundleName', options.profile.applicationName);
		await Promise.all(
			ELECTRON_HELPER_NAMES.map(async (helperName) => {
				await replacePlistString(
					helperInformationPropertyList(options.applicationPath, helperName),
					'CFBundleIdentifier',
					`${options.profile.bundleIdentifier}.helper`,
				);
			}),
		);
		await executeFile(
			'/usr/bin/codesign',
			['--force', '--deep', '--sign', '-', '--timestamp=none', options.applicationPath],
			{ encoding: 'utf8' },
		);
		await validatePreparedApplication(options.applicationPath, options.profile);
	} finally {
		await rm(iconWorkspace, { force: true, recursive: true });
	}
}

async function cloneApplicationBundle(sourceApplicationPath, destinationApplicationPath) {
	try {
		await executeFile('/bin/cp', ['-cR', sourceApplicationPath, destinationApplicationPath], {
			encoding: 'utf8',
		});
	} catch {
		await cp(sourceApplicationPath, destinationApplicationPath, { recursive: true });
	}
}

async function isPreparedApplication(cacheDirectory, executablePath, marker, profile) {
	try {
		const storedMarker = await readFile(
			path.join(cacheDirectory, DEVELOPMENT_APPLICATION_MARKER_FILE_NAME),
			'utf8',
		);
		if (storedMarker !== marker) return false;
		await access(executablePath);
		await validatePreparedApplication(
			path.dirname(path.dirname(path.dirname(executablePath))),
			profile,
		);
		return true;
	} catch {
		return false;
	}
}

async function validatePreparedApplication(applicationPath, profile) {
	const informationPropertyList = path.join(applicationPath, 'Contents', 'Info.plist');
	const [applicationName, bundleIdentifier, iconFileName, ...helperBundleIdentifiers] =
		await Promise.all([
			readPlistString(informationPropertyList, 'CFBundleDisplayName'),
			readPlistString(informationPropertyList, 'CFBundleIdentifier'),
			readPlistString(informationPropertyList, 'CFBundleIconFile'),
			...ELECTRON_HELPER_NAMES.map((helperName) =>
				readPlistString(
					helperInformationPropertyList(applicationPath, helperName),
					'CFBundleIdentifier',
				),
			),
		]);
	if (
		applicationName !== profile.applicationName ||
		bundleIdentifier !== profile.bundleIdentifier ||
		iconFileName !== 'orchbee-dev.icns' ||
		helperBundleIdentifiers.some(
			(identifier) => identifier !== `${profile.bundleIdentifier}.helper`,
		)
	) {
		throw new Error('The generated Desktop development application identity is invalid.');
	}
	await access(path.join(applicationPath, 'Contents', 'Resources', iconFileName));
	await executeFile('/usr/bin/codesign', ['--verify', '--deep', '--strict', applicationPath], {
		encoding: 'utf8',
	});
}

async function replacePlistString(filePath, key, value) {
	await executeFile('/usr/bin/plutil', ['-replace', key, '-string', value, filePath], {
		encoding: 'utf8',
	});
}

async function readPlistString(filePath, key) {
	const { stdout } = await executeFile('/usr/bin/plutil', ['-extract', key, 'raw', filePath], {
		encoding: 'utf8',
	});
	return stdout.trim();
}

function resolveSourceApplicationPath(sourceExecutable) {
	const macOsDirectory = path.dirname(sourceExecutable);
	const contentsDirectory = path.dirname(macOsDirectory);
	const applicationPath = path.dirname(contentsDirectory);
	if (
		path.basename(macOsDirectory) !== 'MacOS' ||
		path.basename(contentsDirectory) !== 'Contents' ||
		path.extname(applicationPath) !== '.app'
	) {
		throw new Error(
			'The Electron development executable is not inside a macOS application bundle.',
		);
	}
	return applicationPath;
}

function helperInformationPropertyList(applicationPath, helperName) {
	return path.join(
		applicationPath,
		'Contents',
		'Frameworks',
		`${helperName}.app`,
		'Contents',
		'Info.plist',
	);
}

function isDestinationConflict(error) {
	return (
		error instanceof Error &&
		'code' in error &&
		(error.code === 'EEXIST' || error.code === 'ENOTEMPTY')
	);
}
