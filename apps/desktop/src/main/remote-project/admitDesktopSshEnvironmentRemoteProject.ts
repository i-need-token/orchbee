import {
	createEmptyWorkbenchSharedTerminalContext,
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

interface DesktopSshEnvironmentRemoteProjectConnectionPort {
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

interface DesktopSshEnvironmentRemoteProjectAdmissionOptions {
	readonly allocateDirectoryToken: () => string;
	readonly connections: DesktopSshEnvironmentRemoteProjectConnectionPort;
	readonly coordinator: Pick<DesktopClientSessionCoordinator, 'mutateCanonicalDocument'>;
	readonly environmentId: EnvironmentId;
	readonly registry: Pick<DesktopEnvironmentRegistry, 'get'>;
	readonly requestedPath: string;
	readonly signal?: AbortSignal | undefined;
}

export type DesktopSshEnvironmentRemoteProjectAdmissionResult =
	| {
			readonly canonicalPath: string;
			readonly directoryId: string;
			readonly kind: 'created' | 'existing';
			readonly projectId: string;
			readonly workspaceId: string;
	  }
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'indeterminate'; readonly reason: 'durability-unconfirmed' }
	| {
			readonly kind: 'failed';
			readonly reason:
				| SshProjectRootFailureReason
				| 'environment-not-connected'
				| 'environment-not-found'
				| 'registry-unavailable'
				| 'workbench-missing'
				| 'workbench-unavailable';
	  };

export async function admitDesktopSshEnvironmentRemoteProject(
	options: DesktopSshEnvironmentRemoteProjectAdmissionOptions,
): Promise<DesktopSshEnvironmentRemoteProjectAdmissionResult> {
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

	let admitted:
		| {
				readonly directoryId: string;
				readonly kind: 'created' | 'existing';
				readonly projectId: string;
				readonly workspaceId: string;
		  }
		| undefined;
	const mutation = await options.coordinator.mutateCanonicalDocument((canonical) => {
		const existing = findRemoteProject(canonical, environmentId, inspected.canonicalPath);
		let state = projectWorkspaceState(canonical);
		let projectCreated = false;
		if (existing === null) {
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
			const projectTransition = transitionProjectWorkspace(state, {
				directory,
				projectName: directory.name,
				type: 'environment-project-registered',
			});
			if (projectTransition.outcome !== 'applied' || projectTransition.commands.length !== 0) {
				return { kind: 'rejected', reason: 'remote-project-transition-rejected' };
			}
			state = projectTransition.state;
			projectCreated = true;
		}
		const remote = findRemoteProjectInState(state, environmentId, inspected.canonicalPath);
		if (remote === null) {
			return { kind: 'rejected', reason: 'remote-project-transition-rejected' };
		}
		const workspaceTransition = transitionProjectWorkspace(state, {
			directoryId: remote.directoryId,
			projectId: remote.projectId,
			type: 'environment-workspace-registered',
			workspaceName: deriveProjectNameFromPath(inspected.canonicalPath),
		});
		if (workspaceTransition.outcome === 'rejected') {
			return { kind: 'rejected', reason: 'remote-workspace-transition-rejected' };
		}
		const workspace = findRemoteProjectInState(
			workspaceTransition.state,
			environmentId,
			inspected.canonicalPath,
		);
		if (workspace?.workspaceId === null || workspace === null) {
			return { kind: 'rejected', reason: 'remote-workspace-transition-rejected' };
		}
		const emptyContextCommands = workspaceTransition.commands.filter(
			(command) => command.type === 'terminal-context-created-empty',
		);
		if (
			(workspaceTransition.outcome === 'applied' && emptyContextCommands.length !== 1) ||
			(workspaceTransition.outcome === 'unchanged' && workspaceTransition.commands.length !== 0)
		) {
			return { kind: 'rejected', reason: 'remote-workspace-transition-rejected' };
		}
		const created = projectCreated || workspaceTransition.outcome === 'applied';
		admitted = {
			directoryId: workspace.directoryId,
			kind: created ? 'created' : 'existing',
			projectId: workspace.projectId,
			workspaceId: workspace.workspaceId,
		};
		if (!created) return { kind: 'unchanged' };
		return {
			document: createWorkbenchSharedDocument({
				projectWorkspace: sharedProjectWorkspaceState(workspaceTransition.state),
				revision: workbenchSharedDocumentRevision((BigInt(canonical.revision) + 1n).toString()),
				terminalContexts: [
					...canonical.terminalContexts,
					...emptyContextCommands.map((command) =>
						createEmptyWorkbenchSharedTerminalContext(command.terminalContextId),
					),
				],
			}),
			kind: 'commit',
		};
	});

	if (mutation.kind === 'indeterminate') return mutation;
	if (mutation.kind === 'unavailable') {
		return { kind: 'failed', reason: 'workbench-unavailable' };
	}
	if (mutation.kind === 'rejected') {
		return {
			kind: 'failed',
			reason:
				mutation.reason === 'document-missing' ? 'workbench-missing' : 'workbench-unavailable',
		};
	}
	if (admitted === undefined) {
		return { kind: 'failed', reason: 'workbench-unavailable' };
	}
	return { ...admitted, canonicalPath: inspected.canonicalPath };
}

function findRemoteProject(
	document: WorkbenchSharedDocument,
	environmentId: EnvironmentId,
	canonicalPath: string,
): {
	readonly directoryId: string;
	readonly projectId: string;
	readonly workspaceId: string | null;
} | null {
	return findRemoteProjectInState(document.projectWorkspace, environmentId, canonicalPath);
}

function findRemoteProjectInState(
	state: Pick<ProjectWorkspaceState, 'projects'>,
	environmentId: EnvironmentId,
	canonicalPath: string,
): {
	readonly directoryId: string;
	readonly projectId: string;
	readonly workspaceId: string | null;
} | null {
	for (const project of state.projects) {
		const directory = project.directories.find(
			(candidate) =>
				candidate.path === canonicalPath &&
				candidate.placement.kind === 'environment' &&
				candidate.placement.environmentId === environmentId,
		);
		if (directory !== undefined) {
			return {
				directoryId: directory.id,
				projectId: project.id,
				workspaceId:
					project.workspaces.find((workspace) => workspace.primaryDirectoryId === directory.id)
						?.id ?? null,
			};
		}
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
