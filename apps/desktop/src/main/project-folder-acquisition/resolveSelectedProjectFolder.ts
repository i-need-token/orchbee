import { isAbsolute, resolve } from 'node:path';

import type { DesktopProjectFolderAcquisitionFailureCode } from '@orchbee/desktop-client-protocol';

export interface SelectedProjectFolderValidationDependencies {
	readonly isAbsolute: (path: string) => boolean;
	readonly resolve: (path: string) => string;
}

export type SelectedProjectFolderValidationResult =
	| { readonly kind: 'selected'; readonly path: string }
	| {
			readonly code: Extract<DesktopProjectFolderAcquisitionFailureCode, 'validation-failed'>;
			readonly kind: 'failed';
	  };

const defaultDependencies: SelectedProjectFolderValidationDependencies = {
	isAbsolute,
	resolve,
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
	return { kind: 'selected', path: dependencies.resolve(selectedPath) };
}
