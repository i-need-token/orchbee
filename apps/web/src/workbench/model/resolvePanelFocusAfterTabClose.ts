/** A Panel and the Tab that holds it. */
export interface PanelFocusPlace {
	readonly panelId: string;
	readonly tabId: string;
}

/**
 * Where focus goes when a close of the Tabs in `closedTabIds` ends. Focus moves only when those Tabs
 * hold the focused Panel: then to the most recently focused entry of `history` that lies outside them
 * and is still open. `history` is most recent first. Null means focus stays where it is, or that no
 * such entry exists, so the post-close active Panel takes the keyboard as before.
 */
export function resolvePanelFocusAfterTabClose<Entry extends PanelFocusPlace>({
	closedTabIds,
	focused,
	history,
	isOpen,
}: {
	readonly closedTabIds: ReadonlySet<string>;
	readonly focused: PanelFocusPlace | null;
	readonly history: readonly Entry[];
	readonly isOpen: (entry: Entry) => boolean;
}): Entry | null {
	if (focused === null || !closedTabIds.has(focused.tabId)) return null;
	return history.find((entry) => !closedTabIds.has(entry.tabId) && isOpen(entry)) ?? null;
}
