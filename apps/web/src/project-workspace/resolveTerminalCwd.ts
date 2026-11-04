import type { WorkbenchDirectory } from './project-workspace-state';

type TerminalCwdSource =
	| 'explicit-directory'
	| 'project-primary-directory'
	| 'split-terminal'
	| 'system'
	| 'workspace-primary-directory';

interface ResolveTerminalCwdInput {
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
			readonly reason:
				| 'explicit-directory-not-found'
				| 'explicit-directory-unavailable'
				| 'workspace-directory-remote';
	  };

export function resolveTerminalCwd(input: ResolveTerminalCwdInput): ResolveTerminalCwdResult {
	if (input.explicitDirectoryId !== undefined) {
		const explicitDirectory =
			input.directories.find((directory) => directory.id === input.explicitDirectoryId) ?? null;
		if (explicitDirectory === null) {
			return { kind: 'rejected', reason: 'explicit-directory-not-found' };
		}
		if (!isAvailableLocalDirectory(explicitDirectory)) {
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
	if (input.workspacePrimaryDirectoryId !== undefined) {
		const workspaceDirectory = findDirectory(input.directories, input.workspacePrimaryDirectoryId);
		if (workspaceDirectory?.placement.kind === 'environment') {
			return { kind: 'rejected', reason: 'workspace-directory-remote' };
		}
		if (workspaceDirectory !== null && isAvailableLocalDirectory(workspaceDirectory)) {
			return {
				cwd: workspaceDirectory.path,
				kind: 'resolved',
				source: 'workspace-primary-directory',
			};
		}
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

function findDirectory(
	directories: readonly WorkbenchDirectory[],
	directoryId: string,
): WorkbenchDirectory | null {
	return directories.find((candidate) => candidate.id === directoryId) ?? null;
}

function findAvailableDirectory(
	directories: readonly WorkbenchDirectory[],
	directoryId: string | undefined,
): WorkbenchDirectory | null {
	if (directoryId === undefined) return null;
	const directory = directories.find((candidate) => candidate.id === directoryId) ?? null;
	return directory !== null && isAvailableLocalDirectory(directory) ? directory : null;
}

function isAvailableLocalDirectory(directory: WorkbenchDirectory): boolean {
	return directory.available && directory.placement.kind === 'local';
}
