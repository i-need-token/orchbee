import { TabActive, TabOpen, TabPinned, type SplitTabId } from '../model';

/**
 * Contains every state bit needed to lay out a tab while it animates.
 * Port of TabLayoutState in tab_layout_state.{h,cc}.
 */
export class TabLayoutState {
	public open: TabOpen;
	public pinned: TabPinned;
	public active: TabActive;
	public readonly split: SplitTabId | null;

	public constructor(
		open: TabOpen = TabOpen.Open,
		pinned: TabPinned = TabPinned.Unpinned,
		active: TabActive = TabActive.Active,
		split: SplitTabId | null = null,
	) {
		this.open = open;
		this.pinned = pinned;
		this.active = active;
		this.split = split;
	}

	public isClosed(): boolean {
		return this.open === TabOpen.Closed;
	}
}
