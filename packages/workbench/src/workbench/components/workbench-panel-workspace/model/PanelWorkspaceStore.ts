import type { TabModel, TabModelSnapshot } from '@orchbee/tab-strip';

import type {
	TerminalPresentationCloseRequest,
	TerminalPresentationCreation,
	TerminalPresentationId,
} from '../../../../terminal-presentation';
import type {
	WorkbenchTerminalPresentationCommand,
	WorkbenchTerminalPresentationComposition,
} from '../../../model/createWorkbenchPreviewTerminalPresentationComposition';
import {
	initializePanelWorkspace,
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
	getTerminalDisplayTitle,
	transitionPanelWorkspace,
	type PanelWorkspaceEvent,
	type PanelWorkspaceState,
	type PanelWorkspaceTransition,
} from './panel-workspace-state';

type Listener = () => void;

interface PanelWorkspaceStoreOptions {
	readonly composition: WorkbenchTerminalPresentationComposition;
	readonly initialCwd?: string | null | undefined;
	readonly initialPresentations?:
		| readonly {
				readonly cwd: string | null;
				readonly presentationId: TerminalPresentationId;
		  }[]
		| undefined;
	readonly initialPresentationId?: TerminalPresentationId | undefined;
	readonly initialState?: PanelWorkspaceState | undefined;
	readonly initialTabTitlePolicies?: readonly WorkbenchTabTitlePolicy[] | undefined;
}

export interface WorkbenchTabTitlePolicy {
	readonly customTitle: string | null;
	readonly fallbackTitle: string;
	readonly tabId: string;
}

export type WorkbenchTabTitleUpdateResult =
	| { readonly kind: 'applied' | 'unchanged' }
	| { readonly kind: 'rejected'; readonly reason: 'tab-not-found' };

export class PanelWorkspaceStore {
	private readonly listeners = new Set<Listener>();
	private readonly pendingTabCwds = new Map<string, string | null>();
	private readonly pendingTabPresentations = new Map<string, TerminalPresentationCreation>();
	private readonly tabTitlePolicies = new Map<string, WorkbenchTabTitlePolicy>();
	private readonly composition: WorkbenchTerminalPresentationComposition;
	private synchronizingTabTitles = false;
	private terminalCreationSealed = false;
	private readonly tabModel: TabModel;
	private unsubscribeTabModel: (() => void) | null = null;
	private state: PanelWorkspaceState;

	public constructor(tabModel: TabModel, options: PanelWorkspaceStoreOptions) {
		this.tabModel = tabModel;
		this.composition = options.composition;
		this.initializeTabTitlePolicies(tabModel.getSnapshot(), options.initialTabTitlePolicies);
		if (options.initialState !== undefined) {
			this.state = options.initialState;
			this.syncTabTitles();
			return;
		}
		const snapshot = tabModel.getSnapshot();
		if (
			options.initialPresentations !== undefined &&
			options.initialPresentations.length !== snapshot.tabs.length
		) {
			throw new Error('Initial terminal presentations must match the initial Tab collection.');
		}
		const initialTabs = snapshot.tabs.map((tab, index) => ({
			cwd: options.initialPresentations?.[index]?.cwd ?? options.initialCwd ?? null,
			id: tab.id,
			presentation: this.createPresentation(
				options.initialPresentations?.[index]?.presentationId ??
					(index === 0
						? (options.initialPresentationId ?? options.composition.initialPresentationId)
						: undefined),
			),
			title: this.requireTabTitlePolicy(tab.id).fallbackTitle,
		}));
		const initialization = initializePanelWorkspace(initialTabs);
		this.state = initialization.state;
		this.dispatchCommands(initialization.commands);
	}

	public readonly getSnapshot = (): PanelWorkspaceState => this.state;
	public readonly getActiveTabId = (): string | null => this.tabModel.getSnapshot().activeTabId;
	public getTabTitlePolicies(): readonly WorkbenchTabTitlePolicy[] {
		const snapshot = this.tabModel.getSnapshot();
		this.captureExternalTabTitleChanges(snapshot);
		this.ensureTabTitlePolicies(snapshot);
		return snapshot.tabs.map((tab) => {
			const policy = this.requireTabTitlePolicy(tab.id);
			return {
				customTitle: policy.customTitle,
				fallbackTitle: policy.fallbackTitle,
				tabId: policy.tabId,
			};
		});
	}

	public setTabCustomTitle(
		tabId: string,
		customTitle: string | null,
	): WorkbenchTabTitleUpdateResult {
		const policy = this.tabTitlePolicies.get(tabId);
		if (policy === undefined) return { kind: 'rejected', reason: 'tab-not-found' };
		if (customTitle === null) {
			if (policy.customTitle === null) return { kind: 'unchanged' };
			this.tabTitlePolicies.set(tabId, { ...policy, customTitle: null });
		} else {
			const normalized = customTitle.trim();
			if (normalized.length === 0 || this.resolveTabTitle(tabId) === normalized) {
				return { kind: 'unchanged' };
			}
			this.tabTitlePolicies.set(tabId, { ...policy, customTitle: normalized });
		}
		this.syncTabTitle(tabId);
		this.emit();
		return { kind: 'applied' };
	}

	public readonly subscribe = (listener: Listener): (() => void) => {
		if (this.unsubscribeTabModel === null) {
			this.unsubscribeTabModel = this.tabModel.subscribe(this.reconcileTabs);
			this.reconcileTabs();
		}
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
			if (this.listeners.size === 0) {
				this.unsubscribeTabModel?.();
				this.unsubscribeTabModel = null;
			}
		};
	};

	public transition(event: PanelWorkspaceEvent): PanelWorkspaceTransition {
		const result = transitionPanelWorkspace(this.state, event);
		if (result.state !== this.state) {
			this.state = result.state;
			this.syncTabTitles();
			this.emit();
		}
		this.dispatchCommands(result.commands);
		return result;
	}

	public createTerminalPresentation(): TerminalPresentationCreation {
		this.assertTerminalCreationAvailable();
		return this.createPresentation();
	}

	public sealTerminalCreation(): void {
		this.terminalCreationSealed = true;
		this.pendingTabCwds.clear();
		this.pendingTabPresentations.clear();
	}

	public requestAllTerminalSessionsStop(): PanelWorkspaceTransition {
		const composition = this.composition;
		if (composition.kind !== 'managed') {
			return rejectedTransition(this.state, 'invalid-close-operations');
		}
		const operations = this.collectTerminals()
			.filter(
				(terminal) =>
					terminal.lifecycle.kind !== 'closing' &&
					terminal.lifecycle.kind !== 'close-failed' &&
					terminal.lifecycle.kind !== 'closed' &&
					terminal.lifecycle.kind !== 'stopping' &&
					terminal.lifecycle.kind !== 'stopped',
			)
			.map((terminal) => ({
				operationId: composition.createOperationId(),
				presentationId: terminal.presentationId,
			}));
		return this.transition({ operations, type: 'terminal-sessions-stop-requested' });
	}

	public retryFailedTerminalSessionStops(): PanelWorkspaceTransition {
		const composition = this.composition;
		if (composition.kind !== 'managed') {
			return rejectedTransition(this.state, 'invalid-close-operations');
		}
		let aggregate: PanelWorkspaceTransition = {
			commands: [],
			outcome: 'unchanged',
			rejection: null,
			state: this.state,
		};
		for (const tab of this.state.tabs) {
			for (const panel of collectPanelLeaves(tab.root)) {
				for (const terminal of [panel.primaryPresentation, ...panel.childPresentations]) {
					if (
						terminal.lifecycle.kind !== 'stop-failed' &&
						terminal.lifecycle.kind !== 'close-failed'
					) {
						continue;
					}
					if (!terminal.lifecycle.failure.retryable) {
						return rejectedTransition(this.state, 'stale-operation');
					}
					const operationId = composition.createOperationId();
					const result = this.transition(
						terminal.lifecycle.kind === 'stop-failed'
							? {
									operationId,
									presentationId: terminal.presentationId,
									tabId: tab.tabId,
									type: 'terminal-stop-retried',
								}
							: {
									operationId,
									presentationId: terminal.presentationId,
									removalId: terminal.lifecycle.removalId,
									tabId: tab.tabId,
									type: 'terminal-close-retried',
								},
					);
					if (result.outcome === 'rejected') return result;
					aggregate = {
						commands: [...aggregate.commands, ...result.commands],
						outcome: 'applied',
						rejection: null,
						state: result.state,
					};
				}
			}
		}
		return aggregate;
	}

	public startStoppedTerminalSession(
		presentationId: TerminalPresentationId,
		presentation: Extract<TerminalPresentationCreation, { readonly kind: 'managed' }>,
	): PanelWorkspaceTransition {
		this.assertTerminalCreationAvailable();
		const composition = this.composition;
		if (composition.kind !== 'managed') {
			return rejectedTransition(this.state, 'terminal-session-not-stopped');
		}
		const tab = this.state.tabs.find((candidate) =>
			collectPanelLeaves(candidate.root).some((panel) =>
				[panel.primaryPresentation, ...panel.childPresentations].some(
					(terminal) => terminal.presentationId === presentationId,
				),
			),
		);
		if (tab === undefined) return rejectedTransition(this.state, 'terminal-not-found');
		return this.transition({
			presentation,
			presentationId,
			tabId: tab.tabId,
			type: 'terminal-stopped-session-started',
		});
	}

	public createTerminalCloseRequest(
		presentationIds: readonly TerminalPresentationId[],
	): TerminalPresentationCloseRequest {
		if (this.composition.kind === 'preview') return { kind: 'preview' };
		const composition = this.composition;
		return {
			kind: 'managed',
			operations: presentationIds.map((presentationId) => ({
				operationId: composition.createOperationId(),
				presentationId,
			})),
			removalId: composition.createRemovalId(),
		};
	}

	public requestTabClose(tabIds: readonly string[]): PanelWorkspaceTransition {
		const uniqueTabIds = new Set(tabIds);
		if (tabIds.length === 0 || uniqueTabIds.size !== tabIds.length) {
			return {
				commands: [],
				outcome: 'rejected',
				rejection: 'invalid-close-operations',
				state: this.state,
			};
		}
		if (this.composition.kind === 'preview') {
			const changed = this.tabModel.closeTabs(tabIds);
			if (this.unsubscribeTabModel === null) this.reconcileTabs();
			return {
				commands: [],
				outcome: changed ? 'applied' : 'unchanged',
				rejection: null,
				state: this.state,
			};
		}

		let nextState = this.state;
		const commands: WorkbenchTerminalPresentationCommand[] = [];
		for (const tabId of tabIds) {
			const tab = nextState.tabs.find((candidate) => candidate.tabId === tabId);
			if (tab === undefined) {
				return {
					commands: [],
					outcome: 'rejected',
					rejection: 'tab-not-found',
					state: this.state,
				};
			}
			const presentationIds = collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((terminal) => terminal.presentationId),
			]);
			const close = this.createTerminalCloseRequest(presentationIds);
			if (close.kind !== 'managed') {
				return {
					commands: [],
					outcome: 'rejected',
					rejection: 'invalid-close-operations',
					state: this.state,
				};
			}
			const transition = transitionPanelWorkspace(nextState, {
				close,
				tabId,
				type: 'tab-close-requested',
			});
			if (transition.outcome === 'rejected') {
				return { ...transition, state: this.state };
			}
			nextState = transition.state;
			commands.push(...transition.commands);
		}

		this.state = nextState;
		this.emit();
		this.dispatchCommands(commands);
		return { commands, outcome: 'applied', rejection: null, state: nextState };
	}

	public prepareTabCwd(tabId: string, cwd: string | null): void {
		this.assertTerminalCreationAvailable();
		this.pendingTabCwds.set(tabId, cwd);
		this.pendingTabPresentations.set(tabId, this.createPresentation());
	}

	public readonly reconcileTabs = (): void => {
		if (this.synchronizingTabTitles) return;
		const snapshot = this.tabModel.getSnapshot();
		this.captureExternalTabTitleChanges(snapshot);
		this.ensureTabTitlePolicies(snapshot);
		const result = this.transition({
			tabs: this.toTabInputs(snapshot),
			type: 'tabs-reconciled',
		});
		const retainedTabIds = new Set(snapshot.tabs.map((tab) => tab.id));
		for (const tabId of this.tabTitlePolicies.keys()) {
			if (!retainedTabIds.has(tabId)) this.tabTitlePolicies.delete(tabId);
		}
		this.syncTabTitles();
		if (result.state === this.state && result.outcome === 'unchanged') {
			this.emit();
		}
		for (const tab of result.state.tabs) {
			this.pendingTabCwds.delete(tab.tabId);
			this.pendingTabPresentations.delete(tab.tabId);
		}
	};

	private emit(): void {
		for (const listener of this.listeners) {
			listener();
		}
	}

	private captureExternalTabTitleChanges(snapshot: TabModelSnapshot): void {
		for (const tab of snapshot.tabs) {
			const policy = this.tabTitlePolicies.get(tab.id);
			if (policy === undefined) continue;
			const projectedTitle = this.resolveTabTitle(tab.id);
			if (tab.title === projectedTitle) continue;
			const normalized = tab.title.trim();
			if (normalized.length === 0 || normalized === projectedTitle) continue;
			this.tabTitlePolicies.set(tab.id, { ...policy, customTitle: normalized });
		}
	}

	private ensureTabTitlePolicies(snapshot: TabModelSnapshot): void {
		for (const tab of snapshot.tabs) {
			if (this.tabTitlePolicies.has(tab.id)) continue;
			this.tabTitlePolicies.set(tab.id, {
				customTitle: null,
				fallbackTitle: tab.title,
				tabId: tab.id,
			});
		}
	}

	private initializeTabTitlePolicies(
		snapshot: TabModelSnapshot,
		initialPolicies: readonly WorkbenchTabTitlePolicy[] | undefined,
	): void {
		if (initialPolicies === undefined) {
			this.ensureTabTitlePolicies(snapshot);
			return;
		}
		const tabIds = new Set(snapshot.tabs.map((tab) => tab.id));
		const policyTabIds = new Set(initialPolicies.map((policy) => policy.tabId));
		if (
			initialPolicies.length !== tabIds.size ||
			policyTabIds.size !== initialPolicies.length ||
			initialPolicies.some((policy) => !tabIds.has(policy.tabId))
		) {
			throw new Error('Initial Tab title policies must exactly match the Tab collection.');
		}
		for (const policy of initialPolicies) {
			this.tabTitlePolicies.set(policy.tabId, { ...policy });
		}
	}

	private requireTabTitlePolicy(tabId: string): WorkbenchTabTitlePolicy {
		const policy = this.tabTitlePolicies.get(tabId);
		if (policy === undefined) throw new Error(`Tab ${tabId} has no title policy.`);
		return policy;
	}

	private resolveTabTitle(tabId: string): string {
		const policy = this.requireTabTitlePolicy(tabId);
		if (policy.customTitle !== null) return policy.customTitle;
		const tab = this.state?.tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) return policy.fallbackTitle;
		const panel = findPanel(tab.root, tab.focusedPanelId);
		return panel === null
			? policy.fallbackTitle
			: getTerminalDisplayTitle(getDisplayedTerminal(panel));
	}

	private syncTabTitle(tabId: string): void {
		const tab = this.tabModel.getSnapshot().tabs.find((candidate) => candidate.id === tabId);
		if (tab === undefined) return;
		const title = this.resolveTabTitle(tabId);
		if (tab.title === title) return;
		this.synchronizingTabTitles = true;
		try {
			this.tabModel.updateTab(tabId, { title });
		} finally {
			this.synchronizingTabTitles = false;
		}
	}

	private syncTabTitles(): void {
		for (const tabId of this.tabTitlePolicies.keys()) this.syncTabTitle(tabId);
	}

	private createPresentation(
		presentationId?: TerminalPresentationId,
	): TerminalPresentationCreation {
		this.assertTerminalCreationAvailable();
		const resolvedPresentationId = presentationId ?? this.composition.createPresentationId();
		return this.composition.kind === 'preview'
			? { kind: 'preview', presentationId: resolvedPresentationId }
			: {
					kind: 'managed',
					operationId: this.composition.createOperationId(),
					presentationId: resolvedPresentationId,
				};
	}

	private assertTerminalCreationAvailable(): void {
		if (this.terminalCreationSealed) {
			throw new Error('Terminal creation is sealed for application shutdown.');
		}
	}

	private collectTerminals() {
		return this.state.tabs.flatMap((tab) =>
			collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation,
				...panel.childPresentations,
			]),
		);
	}

	private dispatchCommands(commands: PanelWorkspaceTransition['commands']): void {
		if (commands.length === 0 || this.composition.kind !== 'managed') return;
		const terminalCommands = commands.filter((command) => {
			if (command.type !== 'tab-close-ready') return true;
			if (this.tabModel.closeTab(command.tabId) && this.unsubscribeTabModel === null) {
				this.reconcileTabs();
			}
			return false;
		});
		if (terminalCommands.length > 0) this.composition.dispatchCommands(terminalCommands);
	}

	private toTabInputs(snapshot: TabModelSnapshot, initialCwd?: string | null) {
		const existingTabIds = new Set(this.state.tabs.map((tab) => tab.tabId));
		return snapshot.tabs.map((tab) => {
			let presentation = this.pendingTabPresentations.get(tab.id);
			if (!existingTabIds.has(tab.id) && presentation === undefined) {
				presentation = this.createPresentation();
				this.pendingTabPresentations.set(tab.id, presentation);
			}
			return {
				cwd: this.pendingTabCwds.get(tab.id) ?? initialCwd ?? null,
				id: tab.id,
				...(presentation === undefined ? {} : { presentation }),
				title: this.requireTabTitlePolicy(tab.id).fallbackTitle,
			};
		});
	}
}

function rejectedTransition(
	state: PanelWorkspaceState,
	rejection: Exclude<PanelWorkspaceTransition['rejection'], null>,
): PanelWorkspaceTransition {
	return { commands: [], outcome: 'rejected', rejection, state };
}
