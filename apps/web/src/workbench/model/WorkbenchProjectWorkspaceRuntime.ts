import { TabModel } from '@orchbee/tab-strip/model';
import {
	workbenchSharedDocumentRevision,
	type TerminalPresentationId,
	type WorkbenchSharedDocument,
	type WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench-client-protocol';

import type { WorkbenchClientView } from '../document/workbench-client-view';
import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceEvent,
	type ProjectWorkspaceState,
	type ProjectWorkspaceTransition,
	findWorkbenchWorkspace,
	type WorkbenchWorkspace,
} from '../../project-workspace';
import { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import {
	collectPanelLeaves,
	countTerminalsRequiringCloseConfirmation,
	findTerminal,
	type PanelLayoutNode,
	type PanelWorkspaceState,
	type TerminalPresentation,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import {
	hydrateWorkbenchDocuments,
	projectWorkbenchClientView,
	projectWorkbenchSharedDocument,
	type HydratedWorkbenchDocumentTerminalContext,
} from './workbench-document-runtime';
import { terminalCloseImpact, type TerminalCloseImpact } from './terminal-close-impact';
import type { WorkbenchPublication } from './workbench-publication';
import { workbenchActiveLocationsEqual } from './workbench-terminal-location-activation';
import type { WorkbenchTerminalMetadataObservation } from './resolveTerminalMetadataObservation';
import {
	revalidateCurrentDirectoryTerminalTarget,
	resolveCurrentDirectoryTerminalTarget,
	type CurrentDirectoryTerminalTarget,
	type CurrentDirectoryTerminalTargetResolution,
} from './resolve-current-directory-terminal-target';

type Listener = () => void;

export interface WorkbenchTerminalContext {
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly tabModel: TabModel;
	readonly terminalContextId: string;
}

interface MutableWorkbenchTerminalContext extends WorkbenchTerminalContext {
	nextTabSequence: number;
}

interface WorkbenchProjectWorkspaceRuntimeOptions {
	/** The view this window recovers, when it reopens; otherwise the publication's default view. */
	readonly initialClientView?: WorkbenchClientView | undefined;
	/** What the Server published when this window connected. */
	readonly initialPublication: WorkbenchPublication;
}

/** Whether a terminal's self-report changed this window's view of it. */
type TerminalMetadataObservationResult =
	| { readonly kind: 'applied' | 'unchanged' }
	| { readonly kind: 'rejected'; readonly reason: string };

export class WorkbenchProjectWorkspaceRuntime {
	private readonly contexts = new Map<string, MutableWorkbenchTerminalContext>();
	private readonly contextDocumentSubscriptions = new Map<string, () => void>();
	private clientViewFingerprint: string | undefined;
	private readonly clientViewListeners = new Set<Listener>();
	private readonly documentListeners = new Set<Listener>();
	private documentFingerprint: string | undefined;
	private readonly listeners = new Set<Listener>();
	private publishedContextFingerprints: ReadonlyMap<string, string> = new Map();
	private terminalTopologyFingerprint: string | undefined;
	private readonly terminalTopologyListeners = new Set<Listener>();
	private state = createProjectWorkspaceState();

	public constructor(options: WorkbenchProjectWorkspaceRuntimeOptions) {
		this.adoptPublication(options.initialPublication, options.initialClientView);
	}

	public readonly getSnapshot = (): ProjectWorkspaceState => this.state;

	public getSharedDocument(revision: WorkbenchSharedDocumentRevision): WorkbenchSharedDocument {
		return projectWorkbenchSharedDocument(this.runtimeSnapshot(), revision);
	}

	public getClientView(): WorkbenchClientView {
		return projectWorkbenchClientView(this.runtimeSnapshot());
	}

	/** Takes what an outside authority published as the whole shared state of this runtime. */
	public applyPublishedWorkbench(publication: WorkbenchPublication): void {
		this.adoptPublication(publication, this.getClientView());
		this.emit();
	}

	private adoptPublication(
		publication: WorkbenchPublication,
		candidate: WorkbenchClientView | undefined,
	): void {
		const hydrated = hydrateWorkbenchDocuments(publication.document, candidate);
		const observedTerminals = collectRuntimeTerminals(this.contexts);
		const previousFingerprints = this.publishedContextFingerprints;
		this.publishedContextFingerprints = publication.contextFingerprints;
		this.state = publishedProjectWorkspaceState(this.state, hydrated.projectWorkspace);
		this.replaceTerminalContexts(
			hydrated.terminalContexts,
			(terminalContextId) =>
				previousFingerprints.get(terminalContextId) ===
				publication.contextFingerprints.get(terminalContextId),
			(terminalContext, currentState) =>
				publishedPanelWorkspaceState(
					terminalContext.panelWorkspace,
					currentState,
					observedTerminals,
					publication,
				),
		);
	}

	private replaceTerminalContexts(
		terminalContexts: readonly HydratedWorkbenchDocumentTerminalContext[],
		isUnchanged: (terminalContextId: string) => boolean,
		panelWorkspaceFor: (
			terminalContext: HydratedWorkbenchDocumentTerminalContext,
			currentState: PanelWorkspaceState | undefined,
		) => PanelWorkspaceState,
	): void {
		const previousContexts = new Map(this.contexts);
		for (const unsubscribe of this.contextDocumentSubscriptions.values()) unsubscribe();
		this.contextDocumentSubscriptions.clear();
		this.contexts.clear();
		for (const terminalContext of terminalContexts) {
			const existing = previousContexts.get(terminalContext.terminalContextId);
			if (existing !== undefined && isUnchanged(terminalContext.terminalContextId)) {
				this.contexts.set(terminalContext.terminalContextId, existing);
				continue;
			}
			const tabModel = new TabModel(terminalContext.tabCollection);
			this.contexts.set(terminalContext.terminalContextId, {
				nextTabSequence: terminalContext.nextTabSequence,
				panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
					initialState: panelWorkspaceFor(
						terminalContext,
						existing?.panelWorkspaceStore.getSnapshot(),
					),
					tabTitlePolicies: terminalContext.tabTitlePolicies,
				}),
				tabModel,
				terminalContextId: terminalContext.terminalContextId,
			});
		}
	}

	public readonly subscribe = (listener: Listener): (() => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	public readonly subscribeClientView = (listener: Listener): (() => void) => {
		if (this.clientViewListeners.size === 0) {
			this.clientViewFingerprint = JSON.stringify(this.getClientView());
		}
		this.clientViewListeners.add(listener);
		this.syncContextDocumentSubscriptions();
		return () => {
			this.clientViewListeners.delete(listener);
			if (this.clientViewListeners.size === 0) this.clientViewFingerprint = undefined;
			this.syncContextDocumentSubscriptions();
		};
	};

	/** Observe navigation without treating grouping or newly published contexts as navigation. */
	public readonly subscribeNavigation = (listener: Listener): (() => void) => {
		const identity = () => {
			const context = this.getActiveTerminalContext();
			const activeTabId = context.tabModel.getSnapshot().activeTabId;
			const tab = context.panelWorkspaceStore
				.getSnapshot()
				.tabs.find((candidate) => candidate.tabId === activeTabId);
			const view = this.getClientView().terminalContexts.find(
				(candidate) => candidate.terminalContextId === context.terminalContextId,
			);
			return JSON.stringify({
				location: this.state.activeLocation,
				activeTabId,
				panel: tab?.focusedPanelId,
				panels: view?.panelWorkspace.tabs.find((candidate) => candidate.tabId === activeTabId)
					?.panels,
			});
		};
		let previous = identity();
		return this.subscribeClientView(() => {
			const next = identity();
			if (next === previous) return;
			previous = next;
			listener();
		});
	};

	public readonly subscribeDocument = (listener: Listener): (() => void) => {
		if (this.documentListeners.size === 0) {
			this.documentFingerprint = sharedDocumentFingerprint(
				this.getSharedDocument(workbenchSharedDocumentRevision('0')),
			);
		}
		this.documentListeners.add(listener);
		this.syncContextDocumentSubscriptions();
		return () => {
			this.documentListeners.delete(listener);
			if (this.documentListeners.size === 0) this.documentFingerprint = undefined;
			this.syncContextDocumentSubscriptions();
		};
	};

	public readonly getTerminalTopologyRevision = (): string =>
		JSON.stringify(this.runtimeSnapshot());

	public readonly subscribeTerminalTopology = (listener: Listener): (() => void) => {
		if (this.terminalTopologyListeners.size === 0) {
			this.terminalTopologyFingerprint = this.getTerminalTopologyRevision();
		}
		this.terminalTopologyListeners.add(listener);
		this.syncContextDocumentSubscriptions();
		return () => {
			this.terminalTopologyListeners.delete(listener);
			if (this.terminalTopologyListeners.size === 0) {
				this.terminalTopologyFingerprint = undefined;
			}
			this.syncContextDocumentSubscriptions();
		};
	};

	public getActiveTerminalContext(): WorkbenchTerminalContext {
		const terminalContextId =
			this.state.activeLocation.kind === 'standalone'
				? this.state.standaloneTerminalContextId
				: this.requireActiveWorkspaceTerminalContextId();
		return this.requireContext(terminalContextId);
	}

	public getTerminalContext(terminalContextId: string): WorkbenchTerminalContext | null {
		return this.contexts.get(terminalContextId) ?? null;
	}

	public resolveActiveTerminalCurrentDirectoryTarget(
		targetPanelId?: string,
	): CurrentDirectoryTerminalTargetResolution {
		const context = this.getActiveMutableContext();
		return resolveCurrentDirectoryTerminalTarget({
			activeTabId: context.tabModel.getSnapshot().activeTabId,
			state: context.panelWorkspaceStore.getSnapshot(),
			targetPanelId: targetPanelId ?? null,
			terminalContextId: context.terminalContextId,
		});
	}

	public resolveTerminalCurrentDirectoryTarget(
		target: CurrentDirectoryTerminalTarget,
	): CurrentDirectoryTerminalTargetResolution {
		const context = this.getActiveMutableContext();
		return revalidateCurrentDirectoryTerminalTarget({
			activeTabId: context.tabModel.getSnapshot().activeTabId,
			state: context.panelWorkspaceStore.getSnapshot(),
			target,
			terminalContextId: context.terminalContextId,
		});
	}

	public getActiveTerminalTabsCloseConfirmationCount(tabIds: readonly string[]): number {
		const requestedTabIds = new Set(tabIds);
		return this.getActiveMutableContext()
			.panelWorkspaceStore.getSnapshot()
			.tabs.reduce(
				(count, tab) =>
					requestedTabIds.has(tab.tabId)
						? count + countTerminalsRequiringCloseConfirmation(tab.root)
						: count,
				0,
			);
	}

	/** What closing these Tabs does to every terminal they hold. */
	public getActiveTerminalTabsCloseImpact(tabIds: readonly string[]): TerminalCloseImpact {
		const requestedTabIds = new Set(tabIds);
		return terminalCloseImpact(
			this.getActiveMutableContext()
				.panelWorkspaceStore.getSnapshot()
				.tabs.filter((tab) => requestedTabIds.has(tab.tabId))
				.flatMap((tab) => collectPanelLeaves(tab.root))
				.flatMap((panel) => [panel.primaryPresentation, ...panel.childPresentations]),
		);
	}

	public applyTerminalMetadataObservation(
		presentationId: TerminalPresentationId,
		observation: WorkbenchTerminalMetadataObservation,
	): TerminalMetadataObservationResult {
		return this.#applyTerminalMetadataObservation(
			presentationId,
			observation,
			observation.closeActivity,
		);
	}

	#applyTerminalMetadataObservation(
		presentationId: TerminalPresentationId,
		observation: Omit<WorkbenchTerminalMetadataObservation, 'closeActivity'>,
		closeActivity?: WorkbenchTerminalMetadataObservation['closeActivity'],
	): TerminalMetadataObservationResult {
		const matches: Array<{
			readonly store: PanelWorkspaceStore;
			readonly tabId: string;
			readonly terminal: TerminalPresentation;
		}> = [];
		for (const context of this.contexts.values()) {
			for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
				const terminal = findTerminal(tab.root, presentationId);
				if (terminal !== null) {
					matches.push({ store: context.panelWorkspaceStore, tabId: tab.tabId, terminal });
				}
			}
		}
		const match = matches.length === 1 ? matches[0] : undefined;
		if (match === undefined) {
			return {
				kind: 'rejected',
				reason: matches.length === 0 ? 'terminal-not-found' : 'terminal-presentation-conflict',
			};
		}
		const transition = match.store.transition({
			...(observation.currentDirectory.kind === 'reported'
				? { currentCwd: observation.currentDirectory.currentCwd }
				: {}),
			closeActivity: closeActivity ?? match.terminal.closeActivity,
			naturalTitle: observation.naturalTitle,
			presentationId,
			tabId: match.tabId,
			type: 'terminal-metadata-changed',
		});
		return transition.outcome === 'rejected'
			? { kind: 'rejected', reason: transition.rejection ?? 'terminal-event-rejected' }
			: { kind: transition.outcome };
	}

	public transition(
		event: ProjectWorkspaceEvent,
		beforeCommit?: (() => void) | undefined,
	): ProjectWorkspaceTransition {
		const result = transitionProjectWorkspace(this.state, event);
		if (result.outcome !== 'applied') return result;
		beforeCommit?.();
		this.state = result.state;
		this.emit();
		return result;
	}

	private activeTerminalContextId(): string {
		return this.state.activeLocation.kind === 'standalone'
			? this.state.standaloneTerminalContextId
			: this.requireActiveWorkspaceTerminalContextId();
	}

	private getActiveMutableContext(): MutableWorkbenchTerminalContext {
		return this.requireContext(this.activeTerminalContextId());
	}

	private requireActiveWorkspaceTerminalContextId(): string {
		const activeLocation = this.state.activeLocation;
		if (activeLocation.kind !== 'workspace') {
			throw new Error('The active location is not a Workspace.');
		}
		const workspace = findWorkbenchWorkspace(this.state, activeLocation.workspaceId);
		if (workspace === undefined) {
			throw new Error('The active Workspace has no terminal context.');
		}
		return workspace.terminalContextId;
	}

	private requireContext(terminalContextId: string): MutableWorkbenchTerminalContext {
		const context = this.contexts.get(terminalContextId);
		if (context === undefined) {
			throw new Error(`Terminal context ${terminalContextId} does not exist.`);
		}
		return context;
	}

	private emit(): void {
		this.syncContextDocumentSubscriptions();
		for (const listener of this.listeners) {
			try {
				listener();
			} catch {
				// Observers cannot take ownership of or relabel an authoritative transition.
				continue;
			}
		}
		this.emitDocumentAndClientView();
	}

	private readonly emitDocumentAndClientView = (): void => {
		if (this.terminalTopologyListeners.size > 0) {
			const fingerprint = this.getTerminalTopologyRevision();
			if (fingerprint !== this.terminalTopologyFingerprint) {
				this.terminalTopologyFingerprint = fingerprint;
				for (const listener of this.terminalTopologyListeners) {
					try {
						listener();
					} catch {
						// Topology observers cannot take ownership of an authoritative transition.
						continue;
					}
				}
			}
		}
		if (this.documentListeners.size > 0) {
			const fingerprint = sharedDocumentFingerprint(
				this.getSharedDocument(workbenchSharedDocumentRevision('0')),
			);
			if (fingerprint !== this.documentFingerprint) {
				this.documentFingerprint = fingerprint;
				for (const listener of this.documentListeners) {
					try {
						listener();
					} catch {
						// Document observers cannot take ownership of an authoritative transition.
						continue;
					}
				}
			}
		}
		if (this.clientViewListeners.size > 0) {
			const fingerprint = JSON.stringify(this.getClientView());
			if (fingerprint !== this.clientViewFingerprint) {
				this.clientViewFingerprint = fingerprint;
				for (const listener of this.clientViewListeners) {
					try {
						listener();
					} catch {
						// Client View observers cannot take ownership of an authoritative transition.
						continue;
					}
				}
			}
		}
	};

	private runtimeSnapshot() {
		return {
			projectWorkspace: this.state,
			terminalContexts: [...this.contexts].map(([terminalContextId, context]) => ({
				nextTabSequence: context.nextTabSequence,
				panelWorkspace: context.panelWorkspaceStore.getSnapshot(),
				tabCollection: context.tabModel.getSnapshot(),
				tabTitlePolicies: context.panelWorkspaceStore.getTabTitlePolicies(),
				terminalContextId,
			})),
		};
	}

	private syncContextDocumentSubscriptions(): void {
		if (
			this.documentListeners.size === 0 &&
			this.clientViewListeners.size === 0 &&
			this.terminalTopologyListeners.size === 0
		) {
			for (const unsubscribe of this.contextDocumentSubscriptions.values()) unsubscribe();
			this.contextDocumentSubscriptions.clear();
			return;
		}
		for (const [terminalContextId, unsubscribe] of this.contextDocumentSubscriptions) {
			if (this.contexts.has(terminalContextId)) continue;
			unsubscribe();
			this.contextDocumentSubscriptions.delete(terminalContextId);
		}
		for (const [terminalContextId, context] of this.contexts) {
			if (this.contextDocumentSubscriptions.has(terminalContextId)) continue;
			this.contextDocumentSubscriptions.set(
				terminalContextId,
				context.panelWorkspaceStore.subscribe(this.emitDocumentAndClientView),
			);
		}
	}
}

function sharedDocumentFingerprint(document: WorkbenchSharedDocument): string {
	return JSON.stringify([document.version, document.projectWorkspace, document.terminalContexts]);
}

function collectRuntimeTerminals(
	contexts: ReadonlyMap<string, MutableWorkbenchTerminalContext>,
): ReadonlyMap<string, TerminalPresentation> {
	const terminals = new Map<string, TerminalPresentation>();
	for (const context of contexts.values()) {
		for (const tab of context.panelWorkspaceStore.getSnapshot().tabs) {
			for (const panel of collectPanelLeaves(tab.root)) {
				for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
					terminals.set(terminal.presentationId, terminal);
				}
			}
		}
	}
	return terminals;
}

function retainActiveLocation(
	previous: ProjectWorkspaceState['activeLocation'],
	next: ProjectWorkspaceState['activeLocation'],
): ProjectWorkspaceState['activeLocation'] {
	return workbenchActiveLocationsEqual(previous, next) ? previous : next;
}

/**
 * This Client's location outlives a publication unless its Workspace is gone, which selects the
 * first Workspace in display order. A Project it has not seen before starts expanded, even when
 * the view it starts from, such as a reloaded document's, already expands it.
 */
function publishedProjectWorkspaceState(
	previous: ProjectWorkspaceState,
	hydrated: ProjectWorkspaceState,
): ProjectWorkspaceState {
	const knownProjectIds = new Set(previous.projects.map((project) => project.id));
	const viewExpandedProjectIds = new Set(hydrated.expandedProjectIds);
	const firstWorkspace = firstWorkspaceInDisplayOrder(hydrated);
	return {
		...hydrated,
		activeLocation: retainActiveLocation(
			previous.activeLocation,
			hydrated.activeLocation.kind === 'standalone' && firstWorkspace !== undefined
				? { kind: 'workspace', workspaceId: firstWorkspace.id }
				: hydrated.activeLocation,
		),
		expandedProjectIds: hydrated.projects
			.map((project) => project.id)
			.filter(
				(projectId) => viewExpandedProjectIds.has(projectId) || !knownProjectIds.has(projectId),
			),
	};
}

/** The sidebar lists each Project's Workspaces in Project order, then Recents. */
function firstWorkspaceInDisplayOrder(
	state: ProjectWorkspaceState,
): WorkbenchWorkspace | undefined {
	for (const project of state.projects) {
		const workspace = state.workspaces.find((candidate) => candidate.projectId === project.id);
		if (workspace !== undefined) return workspace;
	}
	return state.workspaces.find((workspace) => workspace.projectId === null);
}

function publishedPanelWorkspaceState(
	state: PanelWorkspaceState,
	currentState: PanelWorkspaceState | undefined,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	publication: WorkbenchPublication,
): PanelWorkspaceState {
	const currentTabs = new Map(currentState?.tabs.map((tab) => [tab.tabId, tab]) ?? []);
	return {
		...state,
		tabs: state.tabs.map((tab) => ({
			...tab,
			panelFocusHistory: survivingPanelFocusHistory(tab, currentTabs.get(tab.tabId)),
			root: publishedPanelNode(tab.root, observedTerminals, publication),
		})),
	};
}

function publishedPanelNode(
	node: PanelLayoutNode,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	publication: WorkbenchPublication,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			...node,
			childPresentations: node.childPresentations.map((terminal) =>
				publishedTerminal(terminal, observedTerminals, publication),
			),
			primaryPresentation: publishedTerminal(
				node.primaryPresentation,
				observedTerminals,
				publication,
			),
		};
	}
	const weights = publication.splitWeights.get(node.id);
	if (weights === undefined) throw new Error(`Split ${node.id} was published without weights.`);
	return {
		...node,
		children: node.children.map((child, index) => ({
			...child,
			node: publishedPanelNode(child.node, observedTerminals, publication),
			weight: index === 0 ? weights[0] : weights[1],
		})),
	};
}

function publishedTerminal(
	hydrated: TerminalPresentation,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	publication: WorkbenchPublication,
): TerminalPresentation {
	const published = publication.presentations.get(hydrated.presentationId);
	if (published === undefined) {
		throw new Error(`Terminal ${hydrated.presentationId} was published without its facts.`);
	}
	const observed = observedTerminals.get(hydrated.presentationId);
	return {
		...hydrated,
		...(observed === undefined
			? {}
			: {
					closeActivity: observed.closeActivity,
					currentCwd: observed.currentCwd,
					naturalTitle: observed.naturalTitle,
				}),
		...(published.customTitle === null ? {} : { customTitle: published.customTitle }),
		lifecycle: published.lifecycle,
	};
}

function survivingPanelFocusHistory(
	tab: PanelWorkspaceState['tabs'][number],
	currentTab: PanelWorkspaceState['tabs'][number] | undefined,
): readonly string[] {
	const livePanelIds = new Set(collectPanelLeaves(tab.root).map((panel) => panel.id));
	return [
		...(currentTab?.panelFocusHistory ?? []).filter(
			(panelId) => livePanelIds.has(panelId) && panelId !== tab.focusedPanelId,
		),
		tab.focusedPanelId,
	];
}
