import type { TabModel, TabModelSnapshot } from '@orchbee/tab-strip/model';

import {
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
	/** The Panels the Server published for this terminal context. */
	readonly initialState: PanelWorkspaceState;
	readonly initialTabTitlePolicies?: readonly WorkbenchTabTitlePolicy[] | undefined;
}

export interface WorkbenchTabTitlePolicy {
	readonly customTitle: string | null;
	readonly fallbackTitle: string;
	readonly tabId: string;
}

export class PanelWorkspaceStore {
	private readonly listeners = new Set<Listener>();
	private readonly tabTitlePolicies = new Map<string, WorkbenchTabTitlePolicy>();
	private synchronizingTabTitles = false;
	private readonly tabModel: TabModel;
	private unsubscribeTabModel: (() => void) | null = null;
	private state: PanelWorkspaceState;

	public constructor(tabModel: TabModel, options: PanelWorkspaceStoreOptions) {
		this.tabModel = tabModel;
		this.initializeTabTitlePolicies(tabModel.getSnapshot(), options.initialTabTitlePolicies);
		this.state = options.initialState;
		this.syncTabTitles();
	}

	public readonly getSnapshot = (): PanelWorkspaceState => this.state;
	public readonly getActiveTabId = (): string | null => this.tabModel.getSnapshot().activeTabId;
	/** The title the Tab strip shows, while the Tab is there. */
	public getTabTitle(tabId: string): string | null {
		return this.tabModel.getSnapshot().tabs.find((tab) => tab.id === tabId)?.title ?? null;
	}
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
		return result;
	}

	public readonly reconcileTabs = (): void => {
		if (this.synchronizingTabTitles) return;
		const snapshot = this.tabModel.getSnapshot();
		this.captureExternalTabTitleChanges(snapshot);
		this.ensureTabTitlePolicies(snapshot);
		const result = this.transition({
			tabIds: snapshot.tabs.map((tab) => tab.id),
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
}
