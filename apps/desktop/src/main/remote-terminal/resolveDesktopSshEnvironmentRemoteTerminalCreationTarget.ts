import type {
	WorkbenchSharedDocument,
	WorkbenchSharedTerminalContext,
} from '@orchbee/workbench/document';

import { EnvironmentIdSchema, type EnvironmentId } from '../environment';

export type DesktopSshEnvironmentRemoteTerminalCreationTarget =
	| {
			readonly context: WorkbenchSharedTerminalContext;
			readonly directoryPath: string;
			readonly environmentId: EnvironmentId;
			readonly kind: 'resolved';
	  }
	| {
			readonly kind: 'rejected';
			readonly reason:
				| 'project-not-found'
				| 'remote-context-invalid'
				| 'workspace-environment-mismatch'
				| 'workspace-not-found';
	  };

export function resolveDesktopSshEnvironmentRemoteTerminalCreationTarget(
	document: WorkbenchSharedDocument,
	projectId: string,
	workspaceId: string,
): DesktopSshEnvironmentRemoteTerminalCreationTarget {
	const project = document.projectWorkspace.projects.find(
		(candidate) => candidate.id === projectId,
	);
	if (project === undefined) return { kind: 'rejected', reason: 'project-not-found' };
	const workspace = project.workspaces.find((candidate) => candidate.id === workspaceId);
	if (workspace === undefined) return { kind: 'rejected', reason: 'workspace-not-found' };
	const directory = project.directories.find(
		(candidate) => candidate.id === workspace.primaryDirectoryId,
	);
	if (directory?.placement.kind !== 'environment') {
		return { kind: 'rejected', reason: 'workspace-environment-mismatch' };
	}
	const environmentId = EnvironmentIdSchema.safeParse(directory.placement.environmentId);
	if (!environmentId.success) {
		return { kind: 'rejected', reason: 'workspace-environment-mismatch' };
	}
	const context = document.terminalContexts.find(
		(candidate) => candidate.terminalContextId === workspace.terminalContextId,
	);
	return context === undefined
		? { kind: 'rejected', reason: 'remote-context-invalid' }
		: {
				context,
				directoryPath: directory.path,
				environmentId: environmentId.data,
				kind: 'resolved',
			};
}
