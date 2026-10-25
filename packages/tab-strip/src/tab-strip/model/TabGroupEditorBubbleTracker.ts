const TAB_GROUP_EDITOR_IGNORE_SCROLL_MS = 250;

type TabGroupEditorCloseReason = 'explicit' | 'scroll' | 'vertical-tab-mode-change';

type OpenListener = () => void;
type CloseListener = (reason: TabGroupEditorCloseReason) => void;

/** Browser-independent port of Chromium's TabGroupEditorBubbleTracker. */
export class TabGroupEditorBubbleTracker {
	private openState = false;
	private ignoreScrollUntil = Number.NEGATIVE_INFINITY;
	private readonly openedListeners = new Set<OpenListener>();
	private readonly closedListeners = new Set<CloseListener>();

	public get isOpen(): boolean {
		return this.openState;
	}

	public open(timestamp = Date.now()): void {
		this.openState = true;
		this.ignoreScrollUntil = timestamp + TAB_GROUP_EDITOR_IGNORE_SCROLL_MS;
		this.openedListeners.forEach((listener) => listener());
	}

	public close(reason: TabGroupEditorCloseReason = 'explicit'): boolean {
		if (!this.openState) return false;
		this.openState = false;
		this.ignoreScrollUntil = Number.NEGATIVE_INFINITY;
		this.closedListeners.forEach((listener) => listener(reason));
		return true;
	}

	public onContentsScrolled(timestamp = Date.now()): boolean {
		if (!this.openState || timestamp < this.ignoreScrollUntil) return false;
		return this.close('scroll');
	}

	public onVerticalTabStripModeWillChange(): boolean {
		return this.close('vertical-tab-mode-change');
	}

	public registerOnOpened(listener: OpenListener): () => void {
		this.openedListeners.add(listener);
		return () => this.openedListeners.delete(listener);
	}

	public registerOnClosed(listener: CloseListener): () => void {
		this.closedListeners.add(listener);
		return () => this.closedListeners.delete(listener);
	}
}
