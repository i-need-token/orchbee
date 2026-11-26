import type { ChangeItem } from './change-groups';

/** What a float tab shows: a working-tree file, or one change's two sides compared. */
export type FloatTarget =
	| { readonly kind: 'file'; readonly path: string }
	| { readonly kind: 'diff'; readonly item: ChangeItem };

export interface FloatTab {
	/** Stable for the tab's life; the same target always has the same id. */
	readonly id: string;
	readonly target: FloatTarget;
	/** A preview tab shows in italics and gives way to the next file opened as a preview. */
	readonly preview: boolean;
}

export interface FloatState {
	readonly tabs: readonly FloatTab[];
	readonly activeId: string | null;
	/** Collapsed keeps the tabs behind the pill; open shows the window over the terminals. */
	readonly open: boolean;
}

/**
 * The working-tree file a target edits: a file's own, or a diff's later side when that is the
 * working tree. One file's tab and its diffs against the disk edit the same text.
 */
export function documentPathOf(target: FloatTarget): string | null {
	if (target.kind === 'file') return target.path;
	return target.item.after === 'worktree' ? target.item.path : null;
}

export const EMPTY_FLOAT: FloatState = { activeId: null, open: false, tabs: [] };

/**
 * The tab identity of a target. A diff is one per path and compared pair, so the staged and the
 * unstaged change of one file are two tabs, as the Changes view lists them twice.
 */
function floatTabId(target: FloatTarget): string {
	if (target.kind === 'file') return `file:${target.path}`;
	const { item } = target;
	return `diff:${item.before ?? 'none'}:${item.after}:${item.path}`;
}

/**
 * Opens a target and shows the window. A target that already has a tab activates it, and an
 * open that is not a preview keeps it. Otherwise a preview replaces the current preview tab in its
 * place, and anything else is inserted after the active tab, as an editor group does.
 */
export function openFloatTab(
	state: FloatState,
	target: FloatTarget,
	options: {
		readonly preview: boolean;
		/** Paths with unsaved edits: a preview showing one is kept rather than replaced. */
		readonly unsaved?: ReadonlySet<string>;
	},
): FloatState {
	const id = floatTabId(target);
	const existing = state.tabs.find((tab) => tab.id === id);
	if (existing !== undefined) {
		const tabs = options.preview
			? state.tabs
			: state.tabs.map((tab) => (tab.id === id ? { ...tab, preview: false } : tab));
		return { activeId: id, open: true, tabs };
	}
	const created: FloatTab = { id, preview: options.preview, target };
	const previewIndex = options.preview ? state.tabs.findIndex((tab) => tab.preview) : -1;
	const replaced = state.tabs[previewIndex];
	const replacedPath = replaced === undefined ? null : documentPathOf(replaced.target);
	if (
		replaced !== undefined &&
		(replacedPath === null || options.unsaved?.has(replacedPath) !== true)
	) {
		const tabs = state.tabs.slice();
		tabs[previewIndex] = created;
		return { activeId: id, open: true, tabs };
	}
	const activeIndex = state.tabs.findIndex((tab) => tab.id === state.activeId);
	const tabs = state.tabs.map((tab) => (tab === replaced ? { ...tab, preview: false } : tab));
	tabs.splice(activeIndex === -1 ? tabs.length : activeIndex + 1, 0, created);
	return { activeId: id, open: true, tabs };
}

/** Shows the window on one of its tabs. */
export function activateFloatTab(state: FloatState, id: string): FloatState {
	if (!state.tabs.some((tab) => tab.id === id)) return state;
	return { ...state, activeId: id, open: true };
}

/** Keeps a preview tab open: a double click, an edit or a save does this. */
export function keepFloatTab(state: FloatState, id: string): FloatState {
	if (!state.tabs.some((tab) => tab.id === id && tab.preview)) return state;
	return {
		...state,
		tabs: state.tabs.map((tab) => (tab.id === id ? { ...tab, preview: false } : tab)),
	};
}

/** Keeps every preview tab that shows one of `paths`, the files with unsaved edits. */
export function keepFloatTabsShowing(state: FloatState, paths: ReadonlySet<string>): FloatState {
	const shows = (tab: FloatTab): boolean => {
		const path = documentPathOf(tab.target);
		return tab.preview && path !== null && paths.has(path);
	};
	if (!state.tabs.some(shows)) return state;
	return {
		...state,
		tabs: state.tabs.map((tab) => (shows(tab) ? { ...tab, preview: false } : tab)),
	};
}

/**
 * Closes tabs. When the active one goes, its right neighbor takes over, or its left one at the
 * end; the window closes with its last tab.
 */
export function closeFloatTabs(state: FloatState, ids: ReadonlySet<string>): FloatState {
	if (!state.tabs.some((tab) => ids.has(tab.id))) return state;
	const tabs = state.tabs.filter((tab) => !ids.has(tab.id));
	if (tabs.length === 0) return EMPTY_FLOAT;
	let activeId = state.activeId;
	if (activeId === null || ids.has(activeId)) {
		const from = state.tabs.findIndex((tab) => tab.id === state.activeId);
		const after = state.tabs.slice(from + 1).find((tab) => !ids.has(tab.id));
		const before = state.tabs
			.slice(0, Math.max(from, 0))
			.reverse()
			.find((tab) => !ids.has(tab.id));
		activeId = (after ?? before ?? tabs[0])?.id ?? null;
	}
	return { ...state, activeId, tabs };
}

/** Collapses the window to its pill, keeping every tab. */
export function collapseFloat(state: FloatState): FloatState {
	return state.open ? { ...state, open: false } : state;
}

/** Brings a collapsed window back, on the tab that was active. */
export function expandFloat(state: FloatState): FloatState {
	return state.open || state.tabs.length === 0 ? state : { ...state, open: true };
}
