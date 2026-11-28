/** How Changes lists its rows: VS Code's View as List and View as Tree. */
export type ScmViewMode = 'list' | 'tree';

/** The list's order: VS Code's Sort Changes by Name, Path and Status. */
export type ScmViewSortKey = 'name' | 'path' | 'status';

export const SCM_VIEW_MODES = ['list', 'tree'] as const satisfies readonly ScmViewMode[];

export const SCM_VIEW_SORT_KEYS = [
	'name',
	'path',
	'status',
] as const satisfies readonly ScmViewSortKey[];

/** What one Workspace's Changes remembers; VS Code keeps both per workspace. */
export interface ScmViewState {
	readonly mode: ScmViewMode;
	readonly sortKey: ScmViewSortKey;
}

/** A fresh VS Code's `scm.defaultViewMode` and `scm.defaultViewSortKey`. */
export const DEFAULT_SCM_VIEW_STATE: ScmViewState = { mode: 'list', sortKey: 'path' };

/** The two sections' state, which VS Code's pane view keeps for every Workspace alike. */
export interface ScmSectionsState {
	readonly changesCollapsed: boolean;
	readonly repositoriesCollapsed: boolean;
	/** The Repositories list's height the person dragged, in CSS pixels; null to fit its rows. */
	readonly repositoriesSize: number | null;
}

export const DEFAULT_SCM_SECTIONS_STATE: ScmSectionsState = {
	changesCollapsed: false,
	repositoriesCollapsed: false,
	repositoriesSize: null,
};
