import type { TabModel } from '@orchbee/tab-strip/model';

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
	/** The published names of every Tab the Tab model holds. */
	readonly tabTitlePolicies: readonly WorkbenchTabTitlePolicy[];
}

/**
 * The names a publication gave one Tab: the one a person chose, if any, and the one it falls back
 * to. Only a publication changes them, and it builds a new store.
 */
export interface WorkbenchTabTitlePolicy {
	readonly customTitle: string | null;
	readonly fallbackTitle: string;
	readonly tabId: string;
}

/**
 * Holds this Client's view of the published Panels and keeps each Tab strip title current: a Tab a
 * person named shows that name, and any other Tab shows the title of the terminal its focused
 * Panel displays, which changes with what that terminal reports.
 */
export class PanelWorkspaceStore {
	private readonly listeners = new Set<Listener>();
	private readonly tabTitlePolicies: ReadonlyMap<string, WorkbenchTabTitlePolicy>;
	private synchronizingTabTitles = false;
	private readonly tabModel: TabModel;
	private unsubscribeTabModel: (() => void) | null = null;
	private state: PanelWorkspaceState;

	public constructor(tabModel: TabModel, options: PanelWorkspaceStoreOptions) {
		this.tabModel = tabModel;
		this.tabTitlePolicies = new Map(
			options.tabTitlePolicies.map((policy) => [policy.tabId, policy]),
		);
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
		return this.tabModel.getSnapshot().tabs.map((tab) => this.requireTabTitlePolicy(tab.id));
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
		const result = this.transition({
			tabIds: this.tabModel.getSnapshot().tabs.map((tab) => tab.id),
			type: 'tabs-reconciled',
		});
		this.syncTabTitles();
		if (result.outcome === 'unchanged') {
			this.emit();
		}
	};

	private emit(): void {
		for (const listener of this.listeners) {
			listener();
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
		const tab = this.state.tabs.find((candidate) => candidate.tabId === tabId);
		if (tab === undefined) return policy.fallbackTitle;
		const panel = findPanel(tab.root, tab.focusedPanelId);
		return panel === null
			? policy.fallbackTitle
			: getTerminalDisplayTitle(getDisplayedTerminal(panel));
	}

	private syncTabTitles(): void {
		for (const tab of this.tabModel.getSnapshot().tabs) {
			const title = this.resolveTabTitle(tab.id);
			if (tab.title === title) continue;
			this.synchronizingTabTitles = true;
			try {
				this.tabModel.updateTab(tab.id, { title });
			} finally {
				this.synchronizingTabTitles = false;
			}
		}
	}
}
