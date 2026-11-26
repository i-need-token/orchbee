import type { WorkspaceFilesOutcome } from '@orchbee/workbench-client-protocol';

import { type ChangeGroup, type ChangeItem, groupChanges } from '../../change-groups';

type ChangesFailure = Extract<WorkspaceFilesOutcome, { kind: 'failed' }>['reason'];

/** One row of the Changes view: a group's header, or one change in it. */
export type ChangeRow =
	| { readonly kind: 'header'; readonly group: ChangeGroup; readonly items: readonly ChangeItem[] }
	| { readonly kind: 'item'; readonly item: ChangeItem };

/** The Changes view's last read of the repository. */
export type Changes =
	| { readonly state: 'loading' }
	| { readonly state: 'failed'; readonly reason: ChangesFailure }
	| {
			readonly state: 'loaded';
			readonly branch: string | null;
			readonly rows: readonly ChangeRow[];
			readonly staged: number;
			/** Changes that staging everything would take: unstaged ones, never conflicts. */
			readonly unstaged: number;
			readonly conflicts: number;
			readonly truncated: boolean;
	  };

/** Lays a status read out as the Changes view lists it: conflicts, staged, then unstaged. */
export function changeRows(
	outcome: Extract<WorkspaceFilesOutcome, { kind: 'git-status' }>,
): Changes {
	const groups = groupChanges(outcome.entries);
	const rows: ChangeRow[] = [];
	for (const group of ['conflicts', 'staged', 'unstaged'] as const) {
		const items = groups[group];
		if (items.length === 0) continue;
		rows.push({ group, items, kind: 'header' });
		for (const item of items) rows.push({ item, kind: 'item' });
	}
	return {
		branch: outcome.branch,
		conflicts: groups.conflicts.length,
		rows,
		staged: groups.staged.length,
		state: 'loaded',
		truncated: outcome.truncated,
		unstaged: groups.unstaged.length,
	};
}
