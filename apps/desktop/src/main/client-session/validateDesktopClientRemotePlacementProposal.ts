import type {
	WorkbenchSharedDocument,
	WorkbenchSharedPanelNode,
	WorkbenchSharedPresentation,
} from '@orchbee/workbench/document';

type SharedResources = WorkbenchSharedDocument['projectWorkspace']['resourceScopes'][number];
type SharedWorkspace = WorkbenchSharedDocument['projectWorkspace']['workspaces'][number];
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

	for (const project of candidate.projectWorkspace.resourceScopes) {
		for (const directory of project.directories) {
			const key = directoryKey(project.id, directory.id);
			const canonicalRemote = canonicalRemoteDirectories.get(key);
			if (directory.placement.kind === 'environment') {
				if (canonicalRemote === undefined || !valuesEqual(canonicalRemote, directory)) {
					return false;
				}
				continue;
			}
			if (canonicalRemote !== undefined) {
				return false;
			}
		}
		for (const workspace of candidate.projectWorkspace.workspaces.filter(
			(entry) => entry.resourceScopeId === project.id,
		)) {
			const workspaceKey = workspace.id;
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
				!valuesEqual(
					{ ...canonicalRemote.workspace, projectId: null },
					{ ...workspace, projectId: null },
				) ||
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
	if (
		canonical.nextTabSequence !== candidate.nextTabSequence ||
		canonical.panelWorkspace.nextPanelSequence !== candidate.panelWorkspace.nextPanelSequence ||
		canonical.panelWorkspace.nextPanelSlotSequence !==
			candidate.panelWorkspace.nextPanelSlotSequence ||
		canonical.panelWorkspace.nextSplitSequence !== candidate.panelWorkspace.nextSplitSequence ||
		canonical.panelWorkspace.nextTerminalSequence !==
			candidate.panelWorkspace.nextTerminalSequence ||
		!remoteTabCollectionProposalIsAllowed(canonical, candidate) ||
		canonical.panelWorkspace.tabs.length !== candidate.panelWorkspace.tabs.length
	) {
		return false;
	}
	const canonicalRemovals = canonical.panelWorkspace.pendingRemovals;
	const candidateRemovals = candidate.panelWorkspace.pendingRemovals;
	if (
		candidateRemovals.length < canonicalRemovals.length ||
		!valuesEqual(candidateRemovals.slice(0, canonicalRemovals.length), canonicalRemovals)
	) {
		return false;
	}
	const newRemovals = candidateRemovals.slice(canonicalRemovals.length);
	const removalByTabId = new Map<string, (typeof newRemovals)[number]>();
	const removalIds = new Set(canonicalRemovals.map((removal) => removal.removalId));
	for (const removal of newRemovals) {
		if (
			removal.kind !== 'tab' ||
			removalByTabId.has(removal.tabId) ||
			removalIds.has(removal.removalId)
		) {
			return false;
		}
		removalByTabId.set(removal.tabId, removal);
		removalIds.add(removal.removalId);
	}
	const operationIds = new Set<string>();
	for (const [index, canonicalTab] of canonical.panelWorkspace.tabs.entries()) {
		const candidateTab = candidate.panelWorkspace.tabs[index];
		if (candidateTab === undefined || candidateTab.tabId !== canonicalTab.tabId) return false;
		const removal = removalByTabId.get(canonicalTab.tabId);
		if (removal !== undefined) {
			const presentationIds = presentationsInNode(canonicalTab.root).map(
				(presentation) => presentation.presentationId,
			);
			if (!valuesEqual(removal.presentationIds, presentationIds)) return false;
		}
		if (
			!remotePanelNodeProposalIsAllowed(
				canonicalTab.root,
				candidateTab.root,
				removal?.removalId,
				operationIds,
			)
		) {
			return false;
		}
	}
	return [...removalByTabId.keys()].every((tabId) =>
		canonical.panelWorkspace.tabs.some((tab) => tab.tabId === tabId),
	);
}

function remoteTabCollectionProposalIsAllowed(
	canonical: SharedTerminalContext,
	candidate: SharedTerminalContext,
): boolean {
	const canonicalCollection = canonical.tabCollection;
	const candidateCollection = candidate.tabCollection;
	if (
		!valuesEqual(canonicalCollection.groups, candidateCollection.groups) ||
		canonicalCollection.tabs.length !== candidateCollection.tabs.length
	) {
		return false;
	}
	return canonicalCollection.tabs.every((tab, index) => {
		const candidateTab = candidateCollection.tabs[index];
		return (
			candidateTab !== undefined && candidateTab.id === tab.id && candidateTab.group === tab.group
		);
	});
}

function remotePanelNodeProposalIsAllowed(
	canonical: WorkbenchSharedPanelNode,
	candidate: WorkbenchSharedPanelNode,
	removalId: string | undefined,
	operationIds: Set<string>,
): boolean {
	if (canonical.kind !== candidate.kind || canonical.id !== candidate.id) return false;
	if (canonical.kind === 'split' && candidate.kind === 'split') {
		return (
			canonical.orientation === candidate.orientation &&
			canonical.children.length === candidate.children.length &&
			canonical.children.every((child, index) => {
				const candidateChild = candidate.children[index];
				return (
					candidateChild !== undefined &&
					child.id === candidateChild.id &&
					remotePanelNodeProposalIsAllowed(child.node, candidateChild.node, removalId, operationIds)
				);
			})
		);
	}
	if (canonical.kind !== 'panel' || candidate.kind !== 'panel') return false;
	if (canonical.childPresentations.length !== candidate.childPresentations.length) return false;
	return (
		remotePresentationProposalIsAllowed(
			canonical.primaryPresentation,
			candidate.primaryPresentation,
			removalId,
			operationIds,
		) &&
		canonical.childPresentations.every((presentation, index) => {
			const candidatePresentation = candidate.childPresentations[index];
			return (
				candidatePresentation !== undefined &&
				remotePresentationProposalIsAllowed(
					presentation,
					candidatePresentation,
					removalId,
					operationIds,
				)
			);
		})
	);
}

function remotePresentationProposalIsAllowed(
	canonical: WorkbenchSharedPresentation,
	candidate: WorkbenchSharedPresentation,
	removalId: string | undefined,
	operationIds: Set<string>,
): boolean {
	if (
		canonical.launchCwd !== candidate.launchCwd ||
		canonical.presentationId !== candidate.presentationId ||
		canonical.title !== candidate.title
	) {
		return false;
	}
	if (removalId !== undefined) {
		if (
			candidate.intent.kind !== 'close' ||
			candidate.intent.removalId !== removalId ||
			operationIds.has(candidate.intent.operationId)
		) {
			return false;
		}
		operationIds.add(candidate.intent.operationId);
		return canonical.intent.kind !== 'close' && canonical.intent.kind !== 'stop';
	}
	return (
		valuesEqual(canonical.intent, candidate.intent) ||
		(canonical.intent.kind === 'provision' && candidate.intent.kind === 'retain')
	);
}

function presentationsInNode(
	node: WorkbenchSharedPanelNode,
): readonly WorkbenchSharedPresentation[] {
	return node.kind === 'panel'
		? [node.primaryPresentation, ...node.childPresentations]
		: node.children.flatMap((child) => presentationsInNode(child.node));
}

function collectCanonicalRemoteDirectories(canonical: WorkbenchSharedDocument | null) {
	const directories = new Map<string, SharedResources['directories'][number]>();
	if (canonical === null) return directories;
	for (const project of canonical.projectWorkspace.resourceScopes) {
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
	for (const project of canonical.projectWorkspace.resourceScopes) {
		const directories = new Map(project.directories.map((directory) => [directory.id, directory]));
		for (const workspace of canonical.projectWorkspace.workspaces.filter(
			(candidate) => candidate.resourceScopeId === project.id,
		)) {
			if (directories.get(workspace.primaryDirectoryId)?.placement.kind !== 'environment') continue;
			const terminalContext = terminalContexts.get(workspace.terminalContextId);
			if (terminalContext === undefined) return null;
			workspaces.set(workspace.id, {
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

function valuesEqual(left: unknown, right: unknown): boolean {
	return JSON.stringify(left) === JSON.stringify(right);
}
