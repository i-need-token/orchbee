/** What a sidebar row is to the arrow keys: a group heading, a Workspace, or one of its Agents. */
export type SidebarNavigationRowKind = 'agent' | 'heading' | 'workspace';

export interface SidebarNavigationRow {
	/** Whether the row's children show; null where the row has nothing to fold. */
	readonly expanded: boolean | null;
	readonly kind: SidebarNavigationRowKind;
}

export type SidebarNavigationKey =
	| 'ArrowDown'
	| 'ArrowLeft'
	| 'ArrowRight'
	| 'ArrowUp'
	| 'End'
	| 'Enter'
	| 'Home';

export type SidebarNavigationIntent =
	| { readonly index: number; readonly kind: 'focus' }
	| { readonly index: number; readonly kind: 'fold' }
	| { readonly index: number; readonly kind: 'open' }
	| { readonly index: number; readonly kind: 'unfold' };

const LEVEL: Readonly<Record<SidebarNavigationRowKind, number>> = {
	agent: 2,
	heading: 0,
	workspace: 1,
};

/**
 * The sidebar's tree keys, as in the TUI: ↑ ↓ Home End move between the visible rows; → unfolds a
 * row, or moves into it once open; ← folds it, or moves to its parent once folded or on an Agent;
 * Enter folds or unfolds a heading and opens a Workspace or Agent. Null leaves the key unused.
 */
export function resolveSidebarNavigationKey(
	rows: readonly SidebarNavigationRow[],
	index: number,
	key: SidebarNavigationKey,
): SidebarNavigationIntent | null {
	const row = rows[index];
	if (row === undefined) return null;
	switch (key) {
		case 'ArrowDown':
			return index + 1 < rows.length ? { index: index + 1, kind: 'focus' } : null;
		case 'ArrowUp':
			return index > 0 ? { index: index - 1, kind: 'focus' } : null;
		case 'Home':
			return rows.length > 0 ? { index: 0, kind: 'focus' } : null;
		case 'End':
			return rows.length > 0 ? { index: rows.length - 1, kind: 'focus' } : null;
		case 'ArrowRight': {
			if (row.expanded === false) return { index, kind: 'unfold' };
			if (row.expanded !== true) return null;
			const child = rows[index + 1];
			return child !== undefined && LEVEL[child.kind] > LEVEL[row.kind]
				? { index: index + 1, kind: 'focus' }
				: null;
		}
		case 'ArrowLeft': {
			if (row.expanded === true) return { index, kind: 'fold' };
			for (let parent = index - 1; parent >= 0; parent -= 1) {
				const candidate = rows[parent];
				if (candidate !== undefined && LEVEL[candidate.kind] < LEVEL[row.kind]) {
					return { index: parent, kind: 'focus' };
				}
			}
			return null;
		}
		case 'Enter':
			if (row.kind !== 'heading') return { index, kind: 'open' };
			if (row.expanded === null) return null;
			return { index, kind: row.expanded ? 'fold' : 'unfold' };
	}
}

/**
 * Where Ctrl+↑ or Ctrl+↓ (⌘ on macOS) moves an item among its siblings, as the id it then sits
 * before, null for the end. Undefined leaves an item already at that edge in place.
 */
export function resolveSiblingReorder(
	siblingIds: readonly string[],
	itemId: string,
	direction: 'down' | 'up',
): { readonly beforeId: string | null } | undefined {
	const position = siblingIds.indexOf(itemId);
	if (position === -1) return undefined;
	if (direction === 'up') {
		const previous = siblingIds[position - 1];
		return previous === undefined ? undefined : { beforeId: previous };
	}
	if (position + 1 >= siblingIds.length) return undefined;
	return { beforeId: siblingIds[position + 2] ?? null };
}
