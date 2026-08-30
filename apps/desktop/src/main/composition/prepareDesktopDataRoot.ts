import { chmodSync, mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';

import { z } from 'zod';

import type { DesktopDistributionProfile } from '../../distribution';

const DEVELOPMENT_SCOPE_DIGEST_VERSION = 1;
const DEVELOPMENT_SCOPE_MARKER_NAME = 'development-scope-v1.json';

const developmentScopeMarkerSchema = z.strictObject({
	digest: z.string().regex(/^[0-9a-f]{16}$/u),
	digestVersion: z.literal(DEVELOPMENT_SCOPE_DIGEST_VERSION),
	kind: z.literal('development'),
	sourceRoot: z.string().min(1),
});

export interface DesktopDataRootInput {
	readonly distribution: DesktopDistributionProfile;
	readonly homeDirectory: string;
	readonly platform: NodeJS.Platform;
}

export type DesktopDataRoot =
	| {
			readonly path: string;
			readonly scope: { readonly kind: 'local' | 'release' };
	  }
	| {
			readonly path: string;
			readonly scope: {
				readonly digest: string;
				readonly kind: 'development';
				readonly sourceRoot: string;
			};
	  };

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

export function prepareDesktopDataRoot(
	input: DesktopDataRootInput,
	dependencies: DesktopDataRootDependencies = defaultDependencies,
): DesktopDataRoot {
	const resolved = resolveDesktopDataRoot(input);
	dependencies.mkdir(resolved.path, { mode: 0o700, recursive: true });
	if (input.platform !== 'win32') dependencies.chmod(resolved.path, 0o700);
	if (resolved.scope.kind === 'development') {
		ensureDevelopmentScopeMarker({ path: resolved.path, scope: resolved.scope }, dependencies);
	}
	return resolved;
}

function resolveDesktopDataRoot(input: DesktopDataRootInput): DesktopDataRoot {
	const platformPath = input.platform === 'win32' ? path.win32 : path.posix;
	if (!platformPath.isAbsolute(input.homeDirectory)) {
		throw new Error('Desktop data-root resolution requires an absolute home directory.');
	}
	const normalizedHome = normalizePath(input.homeDirectory, input.platform);
	if (input.distribution.dataScope.kind !== 'development') {
		const directoryName = input.distribution.dataRootDirectoryName;
		if (directoryName === null) {
			throw new Error('Packaged Desktop distribution requires a fixed data-root directory.');
		}
		return {
			path: platformPath.join(normalizedHome, directoryName),
			scope: { kind: input.distribution.dataScope.kind },
		};
	}
	return {
		path: platformPath.join(normalizedHome, `.orchbee-dev-${input.distribution.dataScope.digest}`),
		scope: {
			digest: input.distribution.dataScope.digest,
			kind: 'development',
			sourceRoot: input.distribution.dataScope.sourceRoot,
		},
	};
}

function ensureDevelopmentScopeMarker(
	resolved: Extract<DesktopDataRoot, { readonly scope: { readonly kind: 'development' } }>,
	dependencies: DesktopDataRootDependencies,
): void {
	const marker = developmentScopeMarkerSchema.parse({
		digest: resolved.scope.digest,
		digestVersion: DEVELOPMENT_SCOPE_DIGEST_VERSION,
		kind: 'development',
		sourceRoot: resolved.scope.sourceRoot,
	});
	const markerPath = path.join(resolved.path, DEVELOPMENT_SCOPE_MARKER_NAME);
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
