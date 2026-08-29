import type { WorkbenchSharedDocument } from '@orchbee/workbench/document';

type SharedProject = WorkbenchSharedDocument['projectWorkspace']['projects'][number];
type SharedWorkspace = SharedProject['workspaces'][number];
type SharedTerminalContext = WorkbenchSharedDocument['terminalContexts'][number];

interface CanonicalRemoteWorkspace {
	readonly terminalContext: SharedTerminalContext;
	readonly workspace: SharedWorkspace;
}

export function validateDesktopClientRemotePlacementProposal(
	canonical: WorkbenchSharedDocument | null,
	candidate: WorkbenchSharedDocument,
): boolean {
	const canonicalRemoteDirectories = collectCanonicalRemoteDirectories(canonical);
	const canonicalRemoteDirectoryIds = new Set(
		[...canonicalRemoteDirectories.values()].map((directory) => directory.id),
	);
	const canonicalRemoteWorkspaces = collectCanonicalRemoteWorkspaces(canonical);
	if (canonicalRemoteWorkspaces === null) return false;
	const canonicalRemoteWorkspaceIds = new Set(
		[...canonicalRemoteWorkspaces.values()].map(({ workspace }) => workspace.id),
	);
	const canonicalRemoteContextIds = new Set(
		[...canonicalRemoteWorkspaces.values()].map(
			({ terminalContext }) => terminalContext.terminalContextId,
		),
	);
	const retainedRemoteWorkspaceKeys = new Set<string>();
	const retainedRemoteContextIds = new Set<string>();

	for (const project of candidate.projectWorkspace.projects) {
		for (const directory of project.directories) {
			const key = directoryKey(project.id, directory.id);
			const canonicalRemote = canonicalRemoteDirectories.get(key);
			if (directory.placement.kind === 'environment') {
				if (canonicalRemote === undefined || !valuesEqual(canonicalRemote, directory)) {
					return false;
				}
				continue;
			}
			if (canonicalRemote !== undefined || canonicalRemoteDirectoryIds.has(directory.id)) {
				return false;
			}
		}
		for (const workspace of project.workspaces) {
			const workspaceKey = remoteWorkspaceKey(project.id, workspace.id);
			const canonicalRemote = canonicalRemoteWorkspaces.get(workspaceKey);
			const primaryDirectory = project.directories.find(
				(directory) => directory.id === workspace.primaryDirectoryId,
			);
			const usesProtectedIdentity =
				canonicalRemoteWorkspaceIds.has(workspace.id) ||
				canonicalRemoteContextIds.has(workspace.terminalContextId);
			if (primaryDirectory?.placement.kind !== 'environment') {
				if (canonicalRemote !== undefined || usesProtectedIdentity) return false;
				continue;
			}
			if (
				canonicalRemote === undefined ||
				!valuesEqual(canonicalRemote.workspace, workspace) ||
				retainedRemoteWorkspaceKeys.has(workspaceKey)
			) {
				return false;
			}
			const candidateContext = candidate.terminalContexts.find(
				(context) => context.terminalContextId === workspace.terminalContextId,
			);
			if (
				candidateContext === undefined ||
				!remoteContextProposalIsAllowed(canonicalRemote.terminalContext, candidateContext)
			) {
				return false;
			}
			retainedRemoteWorkspaceKeys.add(workspaceKey);
			retainedRemoteContextIds.add(workspace.terminalContextId);
		}
	}

	for (const context of candidate.terminalContexts) {
		if (
			canonicalRemoteContextIds.has(context.terminalContextId) &&
			!retainedRemoteContextIds.has(context.terminalContextId)
		) {
			return false;
		}
	}
	return true;
}

function remoteContextProposalIsAllowed(
	canonical: SharedTerminalContext,
	candidate: SharedTerminalContext,
): boolean {
	if (valuesEqual(canonical, candidate)) return true;
	const canonicalTab = canonical.panelWorkspace.tabs[0];
	if (
		canonical.panelWorkspace.tabs.length !== 1 ||
		canonicalTab?.root.kind !== 'panel' ||
		canonicalTab.root.childPresentations.length !== 0 ||
		canonicalTab.root.primaryPresentation.intent.kind !== 'provision'
	) {
		return false;
	}
	const completed: SharedTerminalContext = {
		...canonical,
		panelWorkspace: {
			...canonical.panelWorkspace,
			tabs: [
				{
					...canonicalTab,
					root: {
						...canonicalTab.root,
						primaryPresentation: {
							...canonicalTab.root.primaryPresentation,
							intent: { kind: 'retain' },
						},
					},
				},
			],
		},
	};
	return valuesEqual(completed, candidate);
}

function collectCanonicalRemoteDirectories(canonical: WorkbenchSharedDocument | null) {
	const directories = new Map<string, SharedProject['directories'][number]>();
	if (canonical === null) return directories;
	for (const project of canonical.projectWorkspace.projects) {
		for (const directory of project.directories) {
			if (directory.placement.kind === 'environment') {
				directories.set(directoryKey(project.id, directory.id), directory);
			}
		}
	}
	return directories;
}

function collectCanonicalRemoteWorkspaces(
	canonical: WorkbenchSharedDocument | null,
): Map<string, CanonicalRemoteWorkspace> | null {
	const workspaces = new Map<string, CanonicalRemoteWorkspace>();
	if (canonical === null) return workspaces;
	const terminalContexts = new Map(
		canonical.terminalContexts.map((context) => [context.terminalContextId, context]),
	);
	for (const project of canonical.projectWorkspace.projects) {
		const directories = new Map(project.directories.map((directory) => [directory.id, directory]));
		for (const workspace of project.workspaces) {
			if (directories.get(workspace.primaryDirectoryId)?.placement.kind !== 'environment') continue;
			const terminalContext = terminalContexts.get(workspace.terminalContextId);
			if (terminalContext === undefined) return null;
			workspaces.set(remoteWorkspaceKey(project.id, workspace.id), {
				terminalContext,
				workspace,
			});
		}
	}
	return workspaces;
}

function directoryKey(projectId: string, directoryId: string): string {
	return JSON.stringify([projectId, directoryId]);
}

function remoteWorkspaceKey(projectId: string, workspaceId: string): string {
	return JSON.stringify([projectId, workspaceId]);
}

function valuesEqual(left: unknown, right: unknown): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
