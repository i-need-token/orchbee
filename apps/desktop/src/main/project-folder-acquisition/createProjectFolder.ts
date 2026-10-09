import { mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';

import {
	DesktopNewProjectFolderNameSchema,
	DesktopProjectFolderPathSchema,
	type DesktopProjectFolderAcquisitionFailureCode,
} from '@orchbee/desktop-client-protocol';

import {
	resolveSelectedProjectFolder,
	type SelectedProjectFolderValidationResult,
} from './resolveSelectedProjectFolder';

export type NewProjectFolderCreationResult =
	| { readonly kind: 'failed'; readonly code: DesktopProjectFolderAcquisitionFailureCode }
	| { readonly kind: 'created'; readonly path: string }
	| { readonly kind: 'creation-unknown'; readonly path: string }
	| {
			readonly kind: 'created-unavailable';
			readonly path: string;
			readonly code: DesktopProjectFolderAcquisitionFailureCode;
	  };

export interface NewProjectFolderCreationDependencies {
	readonly mkdir: (path: string, options: { readonly recursive: false }) => Promise<unknown>;
	readonly resolveSelectedFolder: (path: string) => Promise<SelectedProjectFolderValidationResult>;
}

const defaultDependencies: NewProjectFolderCreationDependencies = {
	mkdir,
	resolveSelectedFolder: resolveSelectedProjectFolder,
};

export async function createProjectFolder(
	parentPath: string,
	name: string,
	isCurrent: () => boolean,
	dependencies: NewProjectFolderCreationDependencies = defaultDependencies,
): Promise<NewProjectFolderCreationResult> {
	if (!DesktopNewProjectFolderNameSchema.safeParse(name).success) {
		return { code: 'invalid-name', kind: 'failed' };
	}
	let parent: SelectedProjectFolderValidationResult;
	try {
		parent = await dependencies.resolveSelectedFolder(parentPath);
	} catch {
		return { code: 'validation-failed', kind: 'failed' };
	}
	if (!isCurrent()) return { code: 'control-unavailable', kind: 'failed' };
	if (parent.kind === 'failed') return parent;
	const path = resolve(parent.path, name);
	if (
		parent.path !== parentPath ||
		dirname(path) !== parent.path ||
		!DesktopProjectFolderPathSchema.safeParse(path).success
	) {
		return { code: 'validation-failed', kind: 'failed' };
	}
	try {
		// The last lifetime check and the atomic, non-recursive syscall have no intervening await.
		await dependencies.mkdir(path, { recursive: false });
	} catch (error: unknown) {
		const code =
			typeof error === 'object' && error !== null && 'code' in error ? error.code : undefined;
		if (code === 'EEXIST') return { code: 'already-exists', kind: 'failed' };
		if (code === 'EACCES' || code === 'EPERM' || code === 'EROFS')
			return { code: 'permission-denied', kind: 'failed' };
		if (code === 'ENOENT' || code === 'ENOTDIR') return { code: 'not-found', kind: 'failed' };
		if (code === 'EINVAL' || code === 'ENAMETOOLONG')
			return { code: 'invalid-name', kind: 'failed' };
		return { kind: 'creation-unknown', path };
	}
	// mkdir committed. Retirement and validation failure cannot erase or relabel that effect.
	try {
		const selected = await dependencies.resolveSelectedFolder(path);
		return selected.kind === 'selected' && selected.path === path
			? { kind: 'created', path }
			: {
					code: selected.kind === 'failed' ? selected.code : 'validation-failed',
					kind: 'created-unavailable',
					path,
				};
	} catch {
		return { code: 'validation-failed', kind: 'created-unavailable', path };
	}
}
