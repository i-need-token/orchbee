import { access, stat } from 'node:fs/promises';
import { constants as fileSystemConstants } from 'node:fs';
import { isAbsolute, resolve } from 'node:path';

import type { DesktopProjectFolderAcquisitionFailureCode } from '@orchbee/desktop-client-protocol';

interface FolderStats {
	isDirectory(): boolean;
}

export interface SelectedProjectFolderValidationDependencies {
	readonly access: (path: string, mode: number) => Promise<void>;
	readonly isAbsolute: (path: string) => boolean;
	readonly resolve: (path: string) => string;
	readonly stat: (path: string) => Promise<FolderStats>;
}

export type SelectedProjectFolderValidationResult =
	| { readonly kind: 'selected'; readonly path: string }
	| {
			readonly code: Extract<
				DesktopProjectFolderAcquisitionFailureCode,
				'not-directory' | 'not-found' | 'permission-denied' | 'validation-failed'
			>;
			readonly kind: 'failed';
	  };

const defaultDependencies: SelectedProjectFolderValidationDependencies = {
	access,
	isAbsolute,
	resolve,
	stat,
};

export async function resolveSelectedProjectFolder(
	selectedPath: string,
	dependencies: SelectedProjectFolderValidationDependencies = defaultDependencies,
): Promise<SelectedProjectFolderValidationResult> {
	if (
		selectedPath.length === 0 ||
		selectedPath.length > 32_768 ||
		selectedPath.includes('\0') ||
		!dependencies.isAbsolute(selectedPath)
	) {
		return { code: 'validation-failed', kind: 'failed' };
	}
	const normalizedPath = dependencies.resolve(selectedPath);
	try {
		const selectedStats = await dependencies.stat(normalizedPath);
		if (!selectedStats.isDirectory()) return { code: 'not-directory', kind: 'failed' };
		await dependencies.access(normalizedPath, fileSystemConstants.R_OK | fileSystemConstants.X_OK);
		return { kind: 'selected', path: normalizedPath };
	} catch (error: unknown) {
		return { code: fileSystemFailureCode(error), kind: 'failed' };
	}
}

function fileSystemFailureCode(
	error: unknown,
): Extract<
	DesktopProjectFolderAcquisitionFailureCode,
	'not-found' | 'permission-denied' | 'validation-failed'
> {
	const code =
		typeof error === 'object' && error !== null && 'code' in error ? error.code : undefined;
	if (code === 'ENOENT' || code === 'ENOTDIR') return 'not-found';
	if (code === 'EACCES' || code === 'EPERM') return 'permission-denied';
	return 'validation-failed';
}
