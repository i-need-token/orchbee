import type { TabModel, TabModelSnapshot } from '@orchbee/tab-strip';

import {
	createPanelWorkspaceState,
	transitionPanelWorkspace,
	type PanelWorkspaceEvent,
	type PanelWorkspaceState,
	type PanelWorkspaceTransition,
} from './panel-workspace-state';

type Listener = () => void;

export class PanelWorkspaceStore {
	private readonly listeners = new Set<Listener>();
	private readonly tabModel: TabModel;
	private unsubscribeTabModel: (() => void) | null = null;
	private state: PanelWorkspaceState;

	public constructor(tabModel: TabModel) {
		this.tabModel = tabModel;
		this.state = createPanelWorkspaceState(this.toTabInputs(tabModel.getSnapshot()));
	}

	public readonly getSnapshot = (): PanelWorkspaceState => this.state;
	public readonly getActiveTabId = (): string | null => this.tabModel.getSnapshot().activeTabId;

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
			this.emit();
		}
		return result;
	}

	private readonly reconcileTabs = (): void => {
		const result = this.transition({
			tabs: this.toTabInputs(this.tabModel.getSnapshot()),
			type: 'tabs-reconciled',
		});
		if (result.state === this.state && result.outcome === 'unchanged') {
			this.emit();
		}
	};

	private emit(): void {
		for (const listener of this.listeners) {
			listener();
		}
	}

	private toTabInputs(snapshot: TabModelSnapshot) {
		return snapshot.tabs.map((tab) => ({ id: tab.id, title: tab.title }));
	}
}
