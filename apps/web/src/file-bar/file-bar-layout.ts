/** The File Bar's views, in the order the toggles row shows them. */
export const FILE_BAR_VIEW_IDS = ['explorer', 'source-control'] as const;

export type FileBarViewId = (typeof FILE_BAR_VIEW_IDS)[number];

/**
 * The side of the terminal area the File Bar takes, from the user's settings: after it by default,
 * or between the Sidebar and it.
 */
export type FileBarPosition = 'left' | 'right';

/** The File Bar column's widths in pixels. */
export const FILE_BAR_WIDTH = {
	default: 300,
	maximum: 560,
	minimum: 240,
	/** Where the window draws its caption buttons over the File Bar's row, so the toggles still fit. */
	minimumBesideCaptionButtons: 280,
} as const;

/**
 * What the File Bar keeps on this device: whether it shows docked, how wide, which view, and the
 * views whose toggles the row hides, as VS Code keeps the containers its activity bar unpins.
 */
export interface FileBarLayout {
	readonly activeView: FileBarViewId;
	/** In the row's order, never every view: the menu keeps the last toggle. */
	readonly hiddenToggles: readonly FileBarViewId[];
	readonly visible: boolean;
	readonly width: number;
}

/** A File Bar never committed starts hidden, so a person who never shows it pays nothing. */
export const DEFAULT_FILE_BAR_LAYOUT: FileBarLayout = {
	activeView: 'explorer',
	hiddenToggles: [],
	visible: false,
	width: FILE_BAR_WIDTH.default,
};

/**
 * The toggles the row shows: each one not hidden, and the active view's while it is, as VS Code's
 * activity bar shows its active container whether it is pinned or not.
 */
export function shownFileBarToggles(layout: FileBarLayout): readonly FileBarViewId[] {
	return FILE_BAR_VIEW_IDS.filter(
		(view) => view === layout.activeView || !layout.hiddenToggles.includes(view),
	);
}

/**
 * Hides a view's toggle, as unchecking it in VS Code's activity bar menu unpins its container; the
 * last toggle shown stays. Hiding the active view's toggle shows the first view whose toggle still
 * shows, which is Explorer whenever its toggle does, as VS Code opens its default container then.
 */
export function hideFileBarToggle(layout: FileBarLayout, view: FileBarViewId): FileBarLayout {
	const [next] = FILE_BAR_VIEW_IDS.filter(
		(candidate) => candidate !== view && !layout.hiddenToggles.includes(candidate),
	);
	if (next === undefined || layout.hiddenToggles.includes(view)) return layout;
	const hiddenToggles = FILE_BAR_VIEW_IDS.filter(
		(candidate) => candidate === view || layout.hiddenToggles.includes(candidate),
	);
	return {
		...layout,
		activeView: layout.activeView === view ? next : layout.activeView,
		hiddenToggles,
	};
}

/**
 * Shows a hidden view's toggle and the view, as checking it in VS Code's activity bar menu pins its
 * container and opens it.
 */
export function keepFileBarToggle(layout: FileBarLayout, view: FileBarViewId): FileBarLayout {
	return {
		...layout,
		activeView: view,
		hiddenToggles: layout.hiddenToggles.filter((candidate) => candidate !== view),
	};
}
