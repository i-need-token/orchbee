import type { WorkbenchDirectory } from './project-workspace-state';

export type TerminalCwdSource =
	| 'explicit-directory'
	| 'project-primary-directory'
	| 'split-terminal'
	| 'system'
	| 'workspace-primary-directory';

export interface ResolveTerminalCwdInput {
	readonly directories: readonly WorkbenchDirectory[];
	readonly explicitDirectoryId?: string | undefined;
	readonly projectPrimaryDirectoryId?: string | undefined;
	readonly splitCwd?: string | null | undefined;
	readonly systemCwd: string;
	readonly workspacePrimaryDirectoryId?: string | undefined;
}

export type ResolveTerminalCwdResult =
	| {
			readonly cwd: string;
			readonly kind: 'resolved';
			readonly source: TerminalCwdSource;
	  }
	| {
			readonly kind: 'rejected';
			readonly reason: 'explicit-directory-not-found' | 'explicit-directory-unavailable';
	  };

export function resolveTerminalCwd(input: ResolveTerminalCwdInput): ResolveTerminalCwdResult {
	if (input.explicitDirectoryId !== undefined) {
		const explicitDirectory =
			input.directories.find((directory) => directory.id === input.explicitDirectoryId) ?? null;
		if (explicitDirectory === null) {
			return { kind: 'rejected', reason: 'explicit-directory-not-found' };
		}
		if (!explicitDirectory.available) {
			return { kind: 'rejected', reason: 'explicit-directory-unavailable' };
		}
		return {
			cwd: explicitDirectory.path,
			kind: 'resolved',
			source: 'explicit-directory',
		};
	}
	if (input.splitCwd !== undefined && input.splitCwd !== null) {
		return { cwd: input.splitCwd, kind: 'resolved', source: 'split-terminal' };
	}
	const workspaceDirectory = findAvailableDirectory(
		input.directories,
		input.workspacePrimaryDirectoryId,
	);
	if (workspaceDirectory !== null) {
		return {
			cwd: workspaceDirectory.path,
			kind: 'resolved',
			source: 'workspace-primary-directory',
		};
	}
	const projectDirectory = findAvailableDirectory(
		input.directories,
		input.projectPrimaryDirectoryId,
	);
	if (projectDirectory !== null) {
		return {
			cwd: projectDirectory.path,
			kind: 'resolved',
			source: 'project-primary-directory',
		};
	}
	return { cwd: input.systemCwd, kind: 'resolved', source: 'system' };
}

function findAvailableDirectory(
	directories: readonly WorkbenchDirectory[],
	directoryId: string | undefined,
): WorkbenchDirectory | null {
	if (directoryId === undefined) return null;
	const directory = directories.find((candidate) => candidate.id === directoryId) ?? null;
	return directory?.available === true ? directory : null;
}
