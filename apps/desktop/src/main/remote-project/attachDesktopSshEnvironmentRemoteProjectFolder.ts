import {
	createWorkbenchSharedDocument,
	workbenchSharedDocumentRevision,
	type WorkbenchSharedDocument,
} from '@orchbee/workbench/document';
import {
	deriveProjectNameFromPath,
	transitionProjectWorkspace,
	type ProjectWorkspaceState,
	type WorkbenchFolderDirectory,
} from '@orchbee/workbench/project-workspace';
import type { SshProjectRootFailureReason } from '@orchbee/ssh-environment-protocol';

import type { DesktopClientSessionCoordinator } from '../client-session';
import {
	EnvironmentIdSchema,
	type DesktopEnvironmentRegistry,
	type EnvironmentId,
} from '../environment';
import type {
	SshEnvironmentConnectionObservation,
	SshProjectRootInspectionOutcome,
} from '../ssh-environment';

interface DesktopSshEnvironmentRemoteProjectFolderConnectionPort {
	inspectProjectRoot(
		environmentId: EnvironmentId,
		path: string,
		signal?: AbortSignal,
	): Promise<SshProjectRootInspectionOutcome>;
	observation(
		environmentId: EnvironmentId,
	):
		| { readonly kind: 'inactive' }
		| { readonly kind: 'ready'; readonly observation: SshEnvironmentConnectionObservation };
}

interface AttachDesktopSshEnvironmentRemoteProjectFolderOptions {
	readonly allocateDirectoryToken: () => string;
	readonly connections: DesktopSshEnvironmentRemoteProjectFolderConnectionPort;
	readonly coordinator: Pick<DesktopClientSessionCoordinator, 'mutateCanonicalDocument'>;
	readonly environmentId: EnvironmentId;
	readonly projectId: string;
	readonly registry: Pick<DesktopEnvironmentRegistry, 'get'>;
	readonly requestedPath: string;
	readonly signal?: AbortSignal | undefined;
}

export type DesktopSshEnvironmentRemoteProjectFolderAttachmentResult =
	| {
			readonly canonicalPath: string;
			readonly directoryId: string;
			readonly kind: 'attached' | 'existing';
			readonly projectId: string;
	  }
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'indeterminate'; readonly reason: 'durability-unconfirmed' }
	| {
			readonly kind: 'failed';
			readonly reason:
				| SshProjectRootFailureReason
				| 'environment-not-connected'
				| 'environment-not-found'
				| 'project-environment-conflict'
				| 'project-folder-already-owned'
				| 'project-not-found'
				| 'registry-unavailable'
				| 'workbench-missing'
				| 'workbench-unavailable';
	  };

export async function attachDesktopSshEnvironmentRemoteProjectFolder(
	options: AttachDesktopSshEnvironmentRemoteProjectFolderOptions,
): Promise<DesktopSshEnvironmentRemoteProjectFolderAttachmentResult> {
	const environmentId = EnvironmentIdSchema.parse(options.environmentId);
	const registered = await options.registry.get(environmentId);
	if (registered.kind === 'unavailable') {
		return { kind: 'failed', reason: 'registry-unavailable' };
	}
	if (registered.value === null) return { kind: 'failed', reason: 'environment-not-found' };
	if (signalIsAborted(options.signal)) return { kind: 'cancelled' };

	const connection = options.connections.observation(environmentId);
	if (connection.kind !== 'ready' || !connection.observation.connected) {
		return { kind: 'failed', reason: 'environment-not-connected' };
	}
	const inspected = await options.connections.inspectProjectRoot(
		environmentId,
		options.requestedPath,
		options.signal,
	);
	if (inspected.kind === 'cancelled' || signalIsAborted(options.signal)) {
		return { kind: 'cancelled' };
	}
	if (inspected.kind === 'failed') return inspected;

	let attached:
		| {
				readonly directoryId: string;
				readonly kind: 'attached' | 'existing';
				readonly projectId: string;
		  }
		| undefined;
	const mutation = await options.coordinator.mutateCanonicalDocument((canonical) => {
		const project = canonical.projectWorkspace.projects.find(
			(candidate) => candidate.id === options.projectId,
		);
		if (project === undefined) return { kind: 'rejected', reason: 'project-not-found' };

		const existing = findRemoteDirectory(canonical, environmentId, inspected.canonicalPath);
		if (existing !== null) {
			if (existing.projectId !== project.id) {
				return { kind: 'rejected', reason: 'project-folder-already-owned' };
			}
			attached = {
				directoryId: existing.directoryId,
				kind: 'existing',
				projectId: project.id,
			};
			return { kind: 'unchanged' };
		}

		const projectEnvironmentIds = new Set(
			project.directories.flatMap((directory) =>
				directory.placement.kind === 'environment' ? [directory.placement.environmentId] : [],
			),
		);
		if (
			projectEnvironmentIds.size > 1 ||
			(projectEnvironmentIds.size === 1 && !projectEnvironmentIds.has(environmentId))
		) {
			return { kind: 'rejected', reason: 'project-environment-conflict' };
		}

		const directory: WorkbenchFolderDirectory = {
			available: false,
			git: null,
			id: allocateDirectoryId(options.allocateDirectoryToken),
			kind: 'folder',
			name: deriveProjectNameFromPath(inspected.canonicalPath),
			path: inspected.canonicalPath,
			placement: { environmentId, kind: 'environment' },
			source: 'selected',
		};
		const transition = transitionProjectWorkspace(projectWorkspaceState(canonical), {
			directory,
			projectId: project.id,
			type: 'environment-directory-registered',
		});
		if (transition.outcome !== 'applied' || transition.commands.length !== 0) {
			return {
				kind: 'rejected',
				reason:
					transition.rejection === 'directory-environment-conflict'
						? 'project-environment-conflict'
						: 'remote-directory-transition-rejected',
			};
		}
		attached = { directoryId: directory.id, kind: 'attached', projectId: project.id };
		return {
			document: createWorkbenchSharedDocument({
				projectWorkspace: sharedProjectWorkspaceState(transition.state),
				revision: workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
				terminalContexts: canonical.terminalContexts,
			}),
			kind: 'commit',
		};
	});

	if (mutation.kind === 'indeterminate') return mutation;
	if (mutation.kind === 'unavailable') {
		return { kind: 'failed', reason: 'workbench-unavailable' };
	}
	if (mutation.kind === 'rejected') {
		switch (mutation.reason) {
			case 'document-missing':
				return { kind: 'failed', reason: 'workbench-missing' };
			case 'project-environment-conflict':
			case 'project-folder-already-owned':
			case 'project-not-found':
				return { kind: 'failed', reason: mutation.reason };
			default:
				return { kind: 'failed', reason: 'workbench-unavailable' };
		}
	}
	if (attached === undefined) {
		return { kind: 'failed', reason: 'workbench-unavailable' };
	}
	return { ...attached, canonicalPath: inspected.canonicalPath };
}

function findRemoteDirectory(
	document: WorkbenchSharedDocument,
	environmentId: EnvironmentId,
	canonicalPath: string,
): { readonly directoryId: string; readonly projectId: string } | null {
	for (const project of document.projectWorkspace.projects) {
		const directory = project.directories.find(
			(candidate) =>
				candidate.path === canonicalPath &&
				candidate.placement.kind === 'environment' &&
				candidate.placement.environmentId === environmentId,
		);
		if (directory !== undefined) return { directoryId: directory.id, projectId: project.id };
	}
	return null;
}

function projectWorkspaceState(document: WorkbenchSharedDocument): ProjectWorkspaceState {
	return {
		...document.projectWorkspace,
		activeLocation: { kind: 'standalone' },
		expandedProjectIds: [],
	};
}

function sharedProjectWorkspaceState(
	state: ProjectWorkspaceState,
): WorkbenchSharedDocument['projectWorkspace'] {
	return {
		nextProjectSequence: state.nextProjectSequence,
		nextTerminalContextSequence: state.nextTerminalContextSequence,
		nextWorkspaceSequence: state.nextWorkspaceSequence,
		projects: state.projects.map((project) => ({
			...project,
			directories: [...project.directories],
			launchConfigurations: [...project.launchConfigurations],
			workspaces: [...project.workspaces],
		})),
		standaloneTerminalContextId: state.standaloneTerminalContextId,
	};
}

function allocateDirectoryId(allocateToken: () => string): string {
	const token = allocateToken();
	if (!/^[0-9a-f]{32}$/u.test(token)) {
		throw new Error('Remote Directory identity allocation must return 128-bit lowercase hex.');
	}
	return `environment-directory:${token}`;
}

function signalIsAborted(signal: AbortSignal | undefined): boolean {
	return signal?.aborted ?? false;
}
