import type { TabModel } from './tab-model';

export const HOVER_TAB_TRANSITION_DELAY_MS = 500;

/** Port of Chromium's spring-loaded HoverTabSelector. */
export class HoverTabSelector {
	private timer: ReturnType<typeof setTimeout> | null = null;
	private targetIndex = -1;
	private readonly model: TabModel;

	public constructor(model: TabModel) {
		this.model = model;
	}

	public startTabTransition(index: number): void {
		if (this.timer !== null) {
			if (index === this.targetIndex) return;
			this.cancelTabTransition();
		}
		const snapshot = this.model.getSnapshot();
		if (snapshot.tabs[index]?.id === snapshot.activeTabId) return;
		if (index < 0 || index >= snapshot.tabs.length) return;
		this.targetIndex = index;
		this.timer = setTimeout(() => this.performTabTransition(), HOVER_TAB_TRANSITION_DELAY_MS);
	}

	public cancelTabTransition(): void {
		if (this.timer !== null) clearTimeout(this.timer);
		this.timer = null;
		this.targetIndex = -1;
	}

	public destroy(): void {
		this.cancelTabTransition();
	}

	private performTabTransition(): void {
		const index = this.targetIndex;
		this.timer = null;
		this.targetIndex = -1;
		const tab = this.model.getSnapshot().tabs[index];
		if (tab) this.model.selectTab(tab.id);
	}
}
