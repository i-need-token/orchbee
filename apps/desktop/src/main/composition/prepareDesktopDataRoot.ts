import { chmodSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { z } from 'zod';

import type { DesktopDistributionProfile } from '../../distribution';

const DEVELOPMENT_SCOPE_DIGEST_VERSION = 1;
// Everything Desktop keeps for itself lives under `internal/`; the top level of a data root holds
// only what a person reads, edits, or keeps.
const INTERNAL_DIRECTORY_NAME = 'internal';
const DEVELOPMENT_SCOPE_MARKER_NAME = 'development-checkout.json';
const ELECTRON_DIRECTORY_NAME = 'desktop';
const LOG_DIRECTORY_NAME = 'logs';
const STATE_DIRECTORY_NAME = 'state';
const WINDOW_STATE_FILE_NAME = 'desktop-window.json';
const VERSION_FILE_NAME = 'version.json';
// Cross-runtime contract: search for `data-root-version-record` before changing this shape; native
// writers write the same record when they create the root.
const DATA_ROOT_LAYOUT = 1;

const developmentScopeMarkerSchema = z.strictObject({
	digest: z.string().regex(/^[0-9a-f]{16}$/u),
	digestVersion: z.literal(DEVELOPMENT_SCOPE_DIGEST_VERSION),
	kind: z.literal('development'),
	sourceRoot: z.string().min(1),
});

export interface DesktopDataRootInput {
	/** The Orchbee version recorded when this start creates the root. */
	readonly applicationVersion: string;
	readonly distribution: DesktopDistributionProfile;
	readonly homeDirectory: string;
	/** `ORCHBEE_HOME`, which names the root in place of the profile's folder below home. */
	readonly homeOverride: string | undefined;
	readonly platform: NodeJS.Platform;
}

interface DesktopDataRootLocations {
	/** Electron `userData`: browser storage, Chromium caches, and the single-instance lock. */
	readonly electronDirectory: string;
	/** The daily diagnostic logs, created by the first record. */
	readonly logDirectory: string;
	readonly path: string;
	readonly windowStateFile: string;
}

export type DesktopDataRoot = DesktopDataRootLocations &
	(
		| { readonly scope: { readonly kind: 'test' | 'release' } }
		| {
				readonly scope: {
					readonly digest: string;
					readonly kind: 'development';
					readonly sourceRoot: string;
				};
		  }
	);

interface DesktopDataRootDependencies {
	readonly chmod: (filePath: string, mode: number) => void;
	readonly mkdir: (
		filePath: string,
		options: { readonly mode: number; readonly recursive: true },
	) => void;
	readonly readFile: (filePath: string, encoding: 'utf8') => string;
	readonly writeFile: (
		filePath: string,
		data: string,
		options: { readonly encoding: 'utf8'; readonly flag: 'wx'; readonly mode: number },
	) => void;
}

const defaultDependencies: DesktopDataRootDependencies = {
	chmod: (filePath, mode) => chmodSync(filePath, mode),
	mkdir: (filePath, options) => void mkdirSync(filePath, options),
	readFile: (filePath, encoding) => readFileSync(filePath, encoding),
	writeFile: (filePath, data, options) => writeFileSync(filePath, data, options),
};

/**
 * Creates the root, `internal/`, and Electron's folder, then records the layout in
 * `internal/version.json` when the root has no record yet. The window state's folder is left to
 * the first save.
 */
export function prepareDesktopDataRoot(
	input: DesktopDataRootInput,
	dependencies: DesktopDataRootDependencies = defaultDependencies,
): DesktopDataRoot {
	const resolved = resolveDesktopDataRoot(input);
	const platformPath = input.platform === 'win32' ? path.win32 : path.posix;
	const internal = platformPath.join(resolved.path, INTERNAL_DIRECTORY_NAME);
	for (const directory of [resolved.path, internal, resolved.electronDirectory]) {
		dependencies.mkdir(directory, { mode: 0o700, recursive: true });
		if (input.platform !== 'win32') dependencies.chmod(directory, 0o700);
	}
	ensureVersionRecord(platformPath.join(internal, VERSION_FILE_NAME), input, dependencies);
	if (resolved.scope.kind === 'development') {
		ensureDevelopmentScopeMarker(
			platformPath.join(internal, DEVELOPMENT_SCOPE_MARKER_NAME),
			resolved.scope,
			dependencies,
		);
	}
	return resolved;
}

function resolveDesktopDataRoot(input: DesktopDataRootInput): DesktopDataRoot {
	const platformPath = input.platform === 'win32' ? path.win32 : path.posix;
	const root = resolveRootPath(input, platformPath);
	const internal = platformPath.join(root, INTERNAL_DIRECTORY_NAME);
	const locations: DesktopDataRootLocations = {
		electronDirectory: platformPath.join(internal, ELECTRON_DIRECTORY_NAME),
		logDirectory: platformPath.join(root, LOG_DIRECTORY_NAME),
		path: root,
		windowStateFile: platformPath.join(internal, STATE_DIRECTORY_NAME, WINDOW_STATE_FILE_NAME),
	};
	if (input.distribution.dataScope.kind !== 'development') {
		return { ...locations, scope: { kind: input.distribution.dataScope.kind } };
	}
	return {
		...locations,
		scope: {
			digest: input.distribution.dataScope.digest,
			kind: 'development',
			sourceRoot: input.distribution.dataScope.sourceRoot,
		},
	};
}

/** `ORCHBEE_HOME` when set, as the native runtime resolves it, otherwise the profile's folder. */
function resolveRootPath(input: DesktopDataRootInput, platformPath: typeof path.posix): string {
	if (input.homeOverride !== undefined) {
		if (!platformPath.isAbsolute(input.homeOverride)) {
			throw new Error('ORCHBEE_HOME must name an absolute folder.');
		}
		return normalizePath(input.homeOverride, input.platform);
	}
	if (!platformPath.isAbsolute(input.homeDirectory)) {
		throw new Error('Desktop data-root resolution requires an absolute home directory.');
	}
	const normalizedHome = normalizePath(input.homeDirectory, input.platform);
	if (input.distribution.dataScope.kind === 'development') {
		return platformPath.join(normalizedHome, `.${input.distribution.artifactStem}`);
	}
	const directoryName = input.distribution.dataRootDirectoryName;
	if (directoryName === null) {
		throw new Error('Packaged Desktop distribution requires a fixed data-root directory.');
	}
	return platformPath.join(normalizedHome, directoryName);
}

/** Writes the layout record only when it is absent; an existing record stays as it is. */
function ensureVersionRecord(
	recordPath: string,
	input: DesktopDataRootInput,
	dependencies: DesktopDataRootDependencies,
): void {
	const record = { layout: DATA_ROOT_LAYOUT, newestVersion: input.applicationVersion };
	try {
		dependencies.writeFile(recordPath, `${JSON.stringify(record)}\n`, {
			encoding: 'utf8',
			flag: 'wx',
			mode: 0o600,
		});
	} catch (error: unknown) {
		if (!isAlreadyExistsError(error)) throw error;
	}
}

function ensureDevelopmentScopeMarker(
	markerPath: string,
	scope: Extract<DesktopDataRoot['scope'], { readonly kind: 'development' }>,
	dependencies: DesktopDataRootDependencies,
): void {
	const marker = developmentScopeMarkerSchema.parse({
		digest: scope.digest,
		digestVersion: DEVELOPMENT_SCOPE_DIGEST_VERSION,
		kind: 'development',
		sourceRoot: scope.sourceRoot,
	});
	const encoded = `${JSON.stringify(marker, null, 2)}\n`;
	try {
		dependencies.writeFile(markerPath, encoded, { encoding: 'utf8', flag: 'wx', mode: 0o600 });
		return;
	} catch (error: unknown) {
		if (!isAlreadyExistsError(error)) throw error;
	}
	const current = developmentScopeMarkerSchema.safeParse(
		JSON.parse(dependencies.readFile(markerPath, 'utf8')),
	);
	if (!current.success || JSON.stringify(current.data) !== JSON.stringify(marker)) {
		throw new Error('Development Desktop data root belongs to another source checkout.');
	}
}

function normalizePath(value: string, platform: NodeJS.Platform): string {
	if (platform === 'win32') {
		const normalized = path.win32.normalize(value).normalize('NFC');
		return /^[A-Z]:/u.test(normalized)
			? `${normalized[0]?.toLowerCase() ?? ''}${normalized.slice(1)}`
			: normalized;
	}
	return path.posix.normalize(value).normalize('NFC');
}

function isAlreadyExistsError(error: unknown): boolean {
	return error instanceof Error && 'code' in error && error.code === 'EEXIST';
}
