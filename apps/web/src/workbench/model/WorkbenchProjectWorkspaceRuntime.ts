import { TabModel, type TabModelInit } from '@orchbee/tab-strip/model';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import {
	projectWorkbenchClientView,
	reconcileWorkbenchClientView,
	type WorkbenchClientTabPanelView,
	type WorkbenchClientView,
} from '../document/workbench-client-view';
import {
	createProjectWorkspaceState,
	transitionProjectWorkspace,
	type ProjectWorkspaceEvent,
	type ProjectWorkspaceState,
	type ProjectWorkspaceTransition,
	findWorkbenchWorkspace,
} from '../../project-workspace';
import {
	PanelWorkspaceStore,
	type WorkbenchTabTitlePolicy,
} from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import {
	collectPanelLeaves,
	findTerminal,
	validatePanelWorkspaceState,
	type PanelLayoutNode,
	type PanelWorkspaceState,
	type TerminalPresentation,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import { terminalCloseImpact, type TerminalCloseImpact } from './terminal-close-impact';
import type {
	WorkbenchPublication,
	WorkbenchPublishedTerminalContext,
} from './workbench-publication';
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

interface PublishedWorkbenchTerminalContext extends WorkbenchTerminalContext {
	/** The publication's fingerprint of the context these models were built from. */
	readonly fingerprint: string;
	/** The Tabs the authority holds that this window leaves out for now. */
	readonly withheldTabIds: ReadonlySet<string>;
}

type TerminalContextView = WorkbenchClientView['terminalContexts'][number];
type PanelViews = ReadonlyMap<string, WorkbenchClientTabPanelView['panels'][number]>;

/** What this Client keeps of a Tab a publication withholds, which the Tab takes up again. */
interface WithheldTabView {
	readonly panelFocusHistory: readonly string[];
	readonly view: WorkbenchClientTabPanelView;
}

/** Withheld Tabs' views, by terminal context and then by Tab. */
type WithheldTabViews = ReadonlyMap<string, ReadonlyMap<string, WithheldTabView>>;

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
	private readonly contexts = new Map<string, PublishedWorkbenchTerminalContext>();
	private readonly contextSubscriptions = new Map<string, () => void>();
	private clientViewFingerprint: string | undefined;
	private readonly clientViewListeners = new Set<Listener>();
	private readonly listeners = new Set<Listener>();
	private terminalTopologyFingerprint: string | undefined;
	private readonly terminalTopologyListeners = new Set<Listener>();
	private state = createProjectWorkspaceState();
	private withheldTabViews: WithheldTabViews = new Map();

	public constructor(options: WorkbenchProjectWorkspaceRuntimeOptions) {
		this.adoptPublication(options.initialPublication, options.initialClientView);
	}

	public readonly getSnapshot = (): ProjectWorkspaceState => this.state;

	public getClientView(): WorkbenchClientView {
		return projectWorkbenchClientView(this.runtimeSnapshot());
	}

	/** Takes what an outside authority published as the whole shared state of this runtime. */
	public applyPublishedWorkbench(publication: WorkbenchPublication): void {
		this.adoptPublication(publication, this.getClientView());
		this.emit();
	}

	/**
	 * Keeps every choice of `candidate` that the publication still allows and every observation of a
	 * terminal it still holds. A context whose fingerprint did not change keeps its live models.
	 */
	private adoptPublication(
		publication: WorkbenchPublication,
		candidate: WorkbenchClientView | undefined,
	): void {
		const returning = this.withholdTabViews(publication, candidate);
		const view = reconcileWorkbenchClientView(
			publication,
			candidate,
			new Map(
				[...returning].map(([terminalContextId, tabs]) => [
					terminalContextId,
					new Map([...tabs].map(([tabId, withheld]) => [tabId, withheld.view])),
				]),
			),
		);
		const contextViews = new Map(
			view.terminalContexts.map((context) => [context.terminalContextId, context]),
		);
		const observedTerminals = collectRuntimeTerminals(this.contexts);
		this.state = publishedProjectWorkspaceState(this.state, publication, view);
		const previousContexts = new Map(this.contexts);
		for (const unsubscribe of this.contextSubscriptions.values()) unsubscribe();
		this.contextSubscriptions.clear();
		this.contexts.clear();
		for (const published of publication.terminalContexts) {
			const existing = previousContexts.get(published.terminalContextId);
			if (existing?.fingerprint === published.fingerprint) {
				this.contexts.set(published.terminalContextId, existing);
				continue;
			}
			const hydrated = hydrateTerminalContext(
				published,
				requireDefined(contextViews.get(published.terminalContextId)),
				observedTerminals,
				new Map([
					...[...(returning.get(published.terminalContextId) ?? [])].map(
						([tabId, withheld]): [string, readonly string[]] => [tabId, withheld.panelFocusHistory],
					),
					...(existing?.panelWorkspaceStore.getSnapshot().tabs ?? []).map(
						(tab): [string, readonly string[]] => [tab.tabId, tab.panelFocusHistory],
					),
				]),
			);
			const tabModel = new TabModel(hydrated.tabCollection);
			this.contexts.set(published.terminalContextId, {
				fingerprint: published.fingerprint,
				panelWorkspaceStore: new PanelWorkspaceStore(tabModel, {
					initialState: hydrated.panelWorkspace,
					tabTitlePolicies: hydrated.tabTitlePolicies,
				}),
				tabModel,
				terminalContextId: published.terminalContextId,
				withheldTabIds: new Set(published.withheldTabIds),
			});
		}
	}

	/**
	 * Keeps this Client's view of each Tab the publication withholds, as the TUI keeps the navigation
	 * memory of a Tab whose close is on its way, and answers the kept views of the Tabs that return.
	 * A Tab the publication neither withholds nor holds is gone, and its view with it.
	 */
	private withholdTabViews(
		publication: WorkbenchPublication,
		candidate: WorkbenchClientView | undefined,
	): WithheldTabViews {
		const candidateContexts = new Map(
			(candidate?.terminalContexts ?? []).map((context) => [context.terminalContextId, context]),
		);
		const withheld = new Map<string, ReadonlyMap<string, WithheldTabView>>();
		const returning = new Map<string, ReadonlyMap<string, WithheldTabView>>();
		for (const published of publication.terminalContexts) {
			const { terminalContextId } = published;
			const kept = this.withheldTabViews.get(terminalContextId);
			const shownViews = new Map(
				(candidateContexts.get(terminalContextId)?.panelWorkspace.tabs ?? []).map((tab) => [
					tab.tabId,
					tab,
				]),
			);
			const shownTabs = new Map(
				(this.contexts.get(terminalContextId)?.panelWorkspaceStore.getSnapshot().tabs ?? []).map(
					(tab) => [tab.tabId, tab],
				),
			);
			const contextWithheld = new Map<string, WithheldTabView>();
			for (const tabId of published.withheldTabIds) {
				const view = shownViews.get(tabId);
				const held =
					view === undefined
						? kept?.get(tabId)
						: {
								panelFocusHistory: shownTabs.get(tabId)?.panelFocusHistory ?? [view.focusedPanelId],
								view,
							};
				if (held !== undefined) contextWithheld.set(tabId, held);
			}
			const contextReturning = new Map<string, WithheldTabView>();
			for (const tab of published.tabs) {
				const held = kept?.get(tab.id);
				if (held !== undefined && !shownViews.has(tab.id)) contextReturning.set(tab.id, held);
			}
			if (contextWithheld.size > 0) withheld.set(terminalContextId, contextWithheld);
			if (contextReturning.size > 0) returning.set(terminalContextId, contextReturning);
		}
		this.withheldTabViews = withheld;
		return returning;
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
		this.syncContextSubscriptions();
		return () => {
			this.clientViewListeners.delete(listener);
			if (this.clientViewListeners.size === 0) this.clientViewFingerprint = undefined;
			this.syncContextSubscriptions();
		};
	};

	/** Observe navigation without treating grouping or newly published contexts as navigation. */
	public readonly subscribeNavigation = (listener: Listener): (() => void) => {
		const identity = () => {
			const context = this.getActiveTerminalContext();
			if (context === null) return JSON.stringify({ location: this.state.activeLocation });
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

	public readonly getTerminalTopologyRevision = (): string =>
		JSON.stringify(this.runtimeSnapshot());

	public readonly subscribeTerminalTopology = (listener: Listener): (() => void) => {
		if (this.terminalTopologyListeners.size === 0) {
			this.terminalTopologyFingerprint = this.getTerminalTopologyRevision();
		}
		this.terminalTopologyListeners.add(listener);
		this.syncContextSubscriptions();
		return () => {
			this.terminalTopologyListeners.delete(listener);
			if (this.terminalTopologyListeners.size === 0) {
				this.terminalTopologyFingerprint = undefined;
			}
			this.syncContextSubscriptions();
		};
	};

	/** The active Workspace's context; null where no Workspace is open, which has none. */
	public getActiveTerminalContext(): WorkbenchTerminalContext | null {
		const activeLocation = this.state.activeLocation;
		if (activeLocation.kind === 'no-workspace') return null;
		const workspace = findWorkbenchWorkspace(this.state, activeLocation.workspaceId);
		if (workspace === undefined) {
			throw new Error('The active Workspace has no terminal context.');
		}
		return this.requireContext(workspace.terminalContextId);
	}

	public getTerminalContext(terminalContextId: string): WorkbenchTerminalContext | null {
		return this.contexts.get(terminalContextId) ?? null;
	}

	/** Whether the authority still holds the Tab, shown or left out for now while its close runs. */
	public holdsTab(terminalContextId: string, tabId: string): boolean {
		const context = this.contexts.get(terminalContextId);
		return (
			context !== undefined &&
			(context.withheldTabIds.has(tabId) ||
				context.tabModel.getSnapshot().tabs.some((tab) => tab.id === tabId))
		);
	}

	public resolveActiveTerminalCurrentDirectoryTarget(
		targetPanelId?: string,
	): CurrentDirectoryTerminalTargetResolution {
		const context = this.getActiveTerminalContext();
		if (context === null) return { kind: 'unavailable', reason: 'active-tab-unavailable' };
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
		const context = this.getActiveTerminalContext();
		if (context === null) return { kind: 'unavailable', reason: 'terminal-target-stale' };
		return revalidateCurrentDirectoryTerminalTarget({
			activeTabId: context.tabModel.getSnapshot().activeTabId,
			state: context.panelWorkspaceStore.getSnapshot(),
			target,
			terminalContextId: context.terminalContextId,
		});
	}

	/** What closing these Tabs does to every terminal they hold. */
	public getActiveTerminalTabsCloseImpact(tabIds: readonly string[]): TerminalCloseImpact {
		const requestedTabIds = new Set(tabIds);
		return terminalCloseImpact(
			(this.getActiveTerminalContext()?.panelWorkspaceStore.getSnapshot().tabs ?? [])
				.filter((tab) => requestedTabIds.has(tab.tabId))
				.flatMap((tab) => collectPanelLeaves(tab.root))
				.flatMap((panel) => [panel.primaryPresentation, ...panel.childPresentations]),
		);
	}

	public applyTerminalMetadataObservation(
		presentationId: TerminalPresentationId,
		observation: WorkbenchTerminalMetadataObservation,
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

	private requireContext(terminalContextId: string): PublishedWorkbenchTerminalContext {
		const context = this.contexts.get(terminalContextId);
		if (context === undefined) {
			throw new Error(`Terminal context ${terminalContextId} does not exist.`);
		}
		return context;
	}

	private emit(): void {
		this.syncContextSubscriptions();
		for (const listener of this.listeners) {
			try {
				listener();
			} catch {
				// Observers cannot take ownership of or relabel an authoritative transition.
				continue;
			}
		}
		this.emitTopologyAndClientView();
	}

	private readonly emitTopologyAndClientView = (): void => {
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
				panelWorkspace: context.panelWorkspaceStore.getSnapshot(),
				tabCollection: context.tabModel.getSnapshot(),
				tabTitlePolicies: context.panelWorkspaceStore.getTabTitlePolicies(),
				terminalContextId,
			})),
		};
	}

	private syncContextSubscriptions(): void {
		if (this.clientViewListeners.size === 0 && this.terminalTopologyListeners.size === 0) {
			for (const unsubscribe of this.contextSubscriptions.values()) unsubscribe();
			this.contextSubscriptions.clear();
			return;
		}
		for (const [terminalContextId, unsubscribe] of this.contextSubscriptions) {
			if (this.contexts.has(terminalContextId)) continue;
			unsubscribe();
			this.contextSubscriptions.delete(terminalContextId);
		}
		for (const [terminalContextId, context] of this.contexts) {
			if (this.contextSubscriptions.has(terminalContextId)) continue;
			this.contextSubscriptions.set(
				terminalContextId,
				context.panelWorkspaceStore.subscribe(this.emitTopologyAndClientView),
			);
		}
	}
}

function collectRuntimeTerminals(
	contexts: ReadonlyMap<string, WorkbenchTerminalContext>,
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
 * first Workspace in display order, or the no-Workspace location once none is left. A Project it has
 * not seen before starts expanded, even when the view it starts from, such as a recovered one,
 * already expands it.
 */
function publishedProjectWorkspaceState(
	previous: ProjectWorkspaceState,
	publication: WorkbenchPublication,
	view: WorkbenchClientView,
): ProjectWorkspaceState {
	const knownProjectIds = new Set(previous.projects.map((project) => project.id));
	const viewExpandedProjectIds = new Set(view.expandedProjectIds);
	return {
		activeLocation: retainActiveLocation(previous.activeLocation, view.activeLocation),
		expandedProjectIds: publication.projects
			.map((project) => project.id)
			.filter(
				(projectId) => viewExpandedProjectIds.has(projectId) || !knownProjectIds.has(projectId),
			),
		projects: publication.projects,
		recentsExpanded: view.recentsExpanded,
		workspaces: publication.workspaces,
	};
}

/** What one terminal context's stores start from. */
interface HydratedTerminalContext {
	readonly panelWorkspace: PanelWorkspaceState;
	readonly tabCollection: TabModelInit;
	readonly tabTitlePolicies: readonly WorkbenchTabTitlePolicy[];
}

/**
 * Builds a published context's stores around this Client's reconciled view of it. Each terminal
 * keeps what this window saw it report, and each Tab the Panel focus order it last held.
 */
function hydrateTerminalContext(
	published: WorkbenchPublishedTerminalContext,
	view: TerminalContextView,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
	panelFocusHistories: ReadonlyMap<string, readonly string[]>,
): HydratedTerminalContext {
	const tabViews = new Map(view.panelWorkspace.tabs.map((tab) => [tab.tabId, tab]));
	const panelWorkspace: PanelWorkspaceState = {
		tabs: published.tabs.map((tab) => {
			const tabView = requireDefined(tabViews.get(tab.id));
			return {
				focusedPanelId: tabView.focusedPanelId,
				maximizedPanelId: tabView.maximizedPanelId,
				panelFocusHistory: survivingPanelFocusHistory(
					tab.root,
					tabView.focusedPanelId,
					panelFocusHistories.get(tab.id),
				),
				root: hydratePanelNode(
					tab.root,
					new Map(tabView.panels.map((panel) => [panel.panelId, panel])),
					observedTerminals,
				),
				tabId: tab.id,
			};
		}),
	};
	const invariantErrors = validatePanelWorkspaceState(panelWorkspace);
	if (invariantErrors.length > 0) {
		throw new Error(`Hydrated Panel workspace is invalid: ${invariantErrors.join('; ')}`);
	}
	const collapsedGroupIds = new Set(view.tabCollection.collapsedGroupIds);
	return {
		panelWorkspace,
		tabCollection: {
			activeTabId: view.tabCollection.activeTabId,
			groups: published.groups.map((group) => ({
				...group,
				collapsed: collapsedGroupIds.has(group.id),
			})),
			mostRecentGroupId: view.tabCollection.mostRecentGroupId,
			selectedTabIds: view.tabCollection.selectedTabIds,
			selectionAnchorId: view.tabCollection.selectionAnchorId,
			tabs: published.tabs.map((tab) => ({
				group: tab.group,
				id: tab.id,
				pinned: tab.pinned,
				title: tab.customTitle ?? tab.title,
			})),
		},
		tabTitlePolicies: published.tabs.map((tab) => ({
			customTitle: tab.customTitle,
			fallbackTitle: tab.title,
			tabId: tab.id,
		})),
	};
}

function hydratePanelNode(
	node: PanelLayoutNode,
	panelViews: PanelViews,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
): PanelLayoutNode {
	if (node.kind === 'panel') {
		return {
			...node,
			activeChildPresentationId: requireDefined(panelViews.get(node.id)).activeChildPresentationId,
			childPresentations: node.childPresentations.map((terminal) =>
				observedTerminal(terminal, observedTerminals),
			),
			primaryPresentation: observedTerminal(node.primaryPresentation, observedTerminals),
		};
	}
	return {
		...node,
		children: node.children.map((child) => ({
			...child,
			node: hydratePanelNode(child.node, panelViews, observedTerminals),
		})),
	};
}

/** What this window saw a terminal report outlives a publication; everything else is published. */
function observedTerminal(
	published: TerminalPresentation,
	observedTerminals: ReadonlyMap<string, TerminalPresentation>,
): TerminalPresentation {
	const observed = observedTerminals.get(published.presentationId);
	return observed === undefined
		? published
		: {
				...published,
				currentCwd: observed.currentCwd,
				naturalTitle: observed.naturalTitle,
			};
}

function survivingPanelFocusHistory(
	root: PanelLayoutNode,
	focusedPanelId: string,
	panelFocusHistory: readonly string[] | undefined,
): readonly string[] {
	const livePanelIds = new Set(collectPanelLeaves(root).map((panel) => panel.id));
	return [
		...(panelFocusHistory ?? []).filter(
			(panelId) => livePanelIds.has(panelId) && panelId !== focusedPanelId,
		),
		focusedPanelId,
	];
}

function requireDefined<Value>(value: Value | undefined): Value {
	if (value === undefined) throw new Error('Expected a reconciled Workbench value.');
	return value;
}
