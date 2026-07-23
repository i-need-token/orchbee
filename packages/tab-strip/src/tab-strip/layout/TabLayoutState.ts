import { TabActive, TabOpen, TabPinned } from '../model/tab-types';

/**
 * Contains every state bit needed to lay out a tab while it animates.
 * Port of TabLayoutState in tab_layout_state.{h,cc}.
 */
export class TabLayoutState {
	public open: TabOpen;
	public pinned: TabPinned;
	public active: TabActive;

	public constructor(
		open: TabOpen = TabOpen.Open,
		pinned: TabPinned = TabPinned.Unpinned,
		active: TabActive = TabActive.Active,
	) {
		this.open = open;
		this.pinned = pinned;
		this.active = active;
	}

	public isClosed(): boolean {
		return this.open === TabOpen.Closed;
	}
}
